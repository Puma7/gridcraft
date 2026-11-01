//! Native background jobs evaluate immutable workbook snapshots. Only `apply`
//! enters the live session's normal transaction path.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use ::solvercraft::jobs::{JobHandle, JobStatus};
use ::solvercraft::runtime::RuntimeLimits;

use super::{
    CommandSpec, Json, Result, SOLVE, Session, Snapshot, bad, cmd, has_doc, json, parameters, registry, sdk_error, simulate_snapshot, snapshot,
    solve_snapshot, str_param,
};

const MAX_RETAINED: usize = 64;
const MAX_RESERVED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_JOB_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
struct Entry {
    snapshot: Snapshot,
    kind: String,
    handle: JobHandle<Json>,
    reserved_bytes: u64,
    closed: bool,
}
static JOBS: OnceLock<Mutex<BTreeMap<u64, Entry>>> = OnceLock::new();
/// Host fixtures share the SDK's real process-wide worker quota. Their test
/// guard calls this before releasing its lock, including during unwinding.
#[cfg(test)]
pub(super) fn finish_test_jobs() -> Result<()> {
    let deadline = ::solvercraft::runtime::monotonic_now() + std::time::Duration::from_secs(10);
    loop {
        let mut jobs = lock()?;
        let mut running = false;
        for job in jobs.values() {
            job.handle.cancel();
            let status = job.handle.snapshot().map_err(sdk_error)?.status;
            running |= matches!(status, JobStatus::Queued | JobStatus::Running);
        }
        if !running {
            jobs.clear();
            return Ok(());
        }
        drop(jobs);
        if ::solvercraft::runtime::monotonic_now() >= deadline {
            return Err(bad(SOLVE, "Timed out draining background test fixtures"));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn store() -> &'static Mutex<BTreeMap<u64, Entry>> {
    JOBS.get_or_init(|| Mutex::new(BTreeMap::new()))
}
fn lock() -> Result<std::sync::MutexGuard<'static, BTreeMap<u64, Entry>>> {
    store().lock().map_err(|_| bad(SOLVE, "Solvercraft job registry is unavailable"))
}
pub(super) fn document_closed(uid: u64) {
    let Some(store) = JOBS.get() else { return };
    let Ok(mut jobs) = store.lock() else { return };
    let uid = uid.to_string();
    jobs.retain(|_, job| {
        if job.snapshot.document_uid != uid {
            return true;
        }
        job.closed = true;
        job.handle.cancel();
        job.handle.snapshot().is_ok_and(|s| matches!(s.status, JobStatus::Queued | JobStatus::Running))
    });
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.job.start", "Start Solvercraft job", [], None, "{kind?:optimization|simulation,request?:model request,model_id?:string,limits?:{max_time_millis?:integer}} -> {id:string,snapshot}. Captures an immutable workbook snapshot and returns immediately; native host only.", has_doc, start),
        cmd!(query "solvercraft.job.get", "Solvercraft job status", [], None, "{id:string} -> status/progress/stale/cancel state without copying retained result samples.", has_doc, get),
        cmd!(query "solvercraft.job.list", "List Solvercraft jobs", [], None, "{} -> retained jobs owned by this workbook only; ids and revisions are lossless decimal strings.", has_doc, list),
        cmd!(query "solvercraft.job.result", "Read Solvercraft job result", [], None, "{id:string} -> completed preview or explicit pending/error; no mutation.", has_doc, result),
        cmd!(query "solvercraft.job.cancel", "Cancel Solvercraft job", [], None, "{id:string} -> requests cooperative cancellation; a partial result may still be returned.", has_doc, cancel),
        cmd!(query "solvercraft.job.forget", "Forget Solvercraft job", [], None, "{id:string} -> releases a terminal retained job; cancel running jobs before forgetting them.", has_doc, forget),
        cmd!(
            "solvercraft.job.apply",
            "Apply Solvercraft job result",
            [],
            None,
            "{id:string} -> one undoable transaction; requires the original document revision and a revalidated feasible result.",
            has_doc,
            apply_job
        ),
    ]
}
fn id(p: &Json) -> Result<u64> {
    p.get("id").and_then(Json::as_str).and_then(|s| s.parse().ok()).ok_or_else(|| bad(SOLVE, "A decimal string job id is required"))
}
fn owned<'a>(jobs: &'a BTreeMap<u64, Entry>, s: &Session, p: &Json) -> Result<&'a Entry> {
    let job = jobs.get(&id(p)?).ok_or_else(|| bad(SOLVE, "Unknown Solvercraft job"))?;
    if job.snapshot.document_uid != s.doc()?.uid.to_string() {
        return Err(bad(SOLVE, "This job belongs to another workbook"));
    }
    Ok(job)
}
fn summary(job: &Entry, current: &Snapshot) -> Result<Json> {
    let state = job.handle.snapshot().map_err(sdk_error)?;
    let status = match state.status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
    };
    let error = state.result.as_ref().and_then(|r| r.as_ref().err());
    Ok(
        json!({"id":state.id.to_string(),"kind":job.kind,"status":status,"progress":state.progress,"snapshot":job.snapshot,"stale":job.snapshot.document_uid!=current.document_uid||job.snapshot.revision!=current.revision,"error":error}),
    )
}
fn start(s: &mut Session, p: &Json) -> Result<Json> {
    let (kind, mut request) = if let Some(model_id) = p.get("model_id") {
        let model = registry::get(s, &json!({"id":model_id}))?;
        (model["kind"].as_str().unwrap_or("optimization").to_string(), model["request"].clone())
    } else {
        (
            str_param(p, "kind").unwrap_or("optimization").to_string(),
            p.get("request").cloned().ok_or_else(|| bad(SOLVE, "request or model_id is required"))?,
        )
    };
    if !matches!(kind.as_str(), "optimization" | "simulation") {
        return Err(bad(SOLVE, "Job kind must be optimization or simulation"));
    }
    if !request.is_object() || serde_json::to_vec(&request).map_err(sdk_error)?.len() > 131_072 {
        return Err(bad(SOLVE, "Job request must be a bounded JSON object"));
    }
    super::pin_request_sheet(&s.doc()?.wb, &mut request, &kind)?;
    let limits: RuntimeLimits = serde_json::from_value(p.get("limits").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    if request.get("options").is_none() {
        request["options"] = json!({});
    }
    if !request["options"].is_object() {
        return Err(bad(SOLVE, "Job options must be an object"));
    }
    if request["options"].get("max_memory_bytes").is_none() {
        request["options"]["max_memory_bytes"] = json!(DEFAULT_JOB_MEMORY_BYTES);
    }
    let budget = request["options"]["max_memory_bytes"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| bad(SOLVE, "max_memory_bytes must be a positive integer"))?;
    // Reserve additional space for the JSON preview and result transport copies.
    let reserved_bytes = budget.checked_mul(4).ok_or_else(|| bad(SOLVE, "Job memory reservation overflows"))?;
    let mut jobs = lock()?;
    jobs.retain(|_, job| !job.closed || job.handle.snapshot().is_ok_and(|s| matches!(s.status, JobStatus::Queued | JobStatus::Running)));
    if jobs.len() >= MAX_RETAINED {
        return Err(bad(SOLVE, "Retained job limit reached; forget completed jobs"));
    }
    if jobs.values().fold(reserved_bytes, |sum, entry| sum.saturating_add(entry.reserved_bytes)) > MAX_RESERVED_BYTES {
        return Err(bad(SOLVE, "Retained job memory reservation reached; forget completed jobs or lower max_memory_bytes"));
    }
    let captured = snapshot(s)?;
    let worker_snapshot = captured.clone();
    let wb = std::sync::Arc::clone(&s.doc()?.wb);
    let calc = s.doc()?.calc.clone();
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(calc.fixed_now.unwrap_or_else(gridcraft_calc::now_serial));
    }
    let worker_kind = kind.clone();
    let handle = ::solvercraft::jobs::start_shared_job(move |control| {
        let deadline = std::sync::Arc::new(::solvercraft::runtime::OwnedDeadlineControl::new(control, limits)?);
        let active: std::sync::Arc<dyn ::solvercraft::types::RunControl> = deadline.clone();
        let output = if worker_kind == "simulation" {
            if let Some(cases) = request.get("parameter_cases") {
                parameters::execute(&wb, &calc, worker_snapshot, &request, cases, active.as_ref())
            } else {
                simulate_snapshot(&wb, &calc, worker_snapshot, &request, active.as_ref())
            }
        } else {
            solve_snapshot(&wb, &calc, worker_snapshot, &request, active)
        };
        let mut output = output.map_err(|e| ::solvercraft::types::SolverError::new("workbook_model", e.to_string()))?;
        if deadline.timed_out() && output["result"]["termination"] == json!("cancelled") {
            output["result"]["termination"] = json!("time_limit");
        }
        output["execution"] = json!("native_background_snapshot");
        super::manifest::annotate_limits(&mut output, &limits);
        Ok(output)
    })
    .map_err(sdk_error)?;
    let id = handle.id();
    jobs.insert(id, Entry { snapshot: captured.clone(), kind, handle, reserved_bytes, closed: false });
    Ok(json!({"id":id.to_string(),"snapshot":captured,"execution":"native_background_snapshot"}))
}
fn get(s: &mut Session, p: &Json) -> Result<Json> {
    let jobs = lock()?;
    summary(owned(&jobs, s, p)?, &snapshot(s)?)
}
fn list(s: &mut Session, _: &Json) -> Result<Json> {
    let current = snapshot(s)?;
    Ok(
        json!({"jobs":lock()?.values().filter(|j|j.snapshot.document_uid==current.document_uid).map(|j|summary(j,&current)).collect::<Result<Vec<_>>>()?}),
    )
}
fn result(s: &mut Session, p: &Json) -> Result<Json> {
    let jobs = lock()?;
    let state = owned(&jobs, s, p)?.handle.snapshot().map_err(sdk_error)?;
    let output = state.result.ok_or_else(|| bad(SOLVE, "Job is still pending"))?.map_err(sdk_error)?;
    Ok((*output).clone())
}
fn cancel(s: &mut Session, p: &Json) -> Result<Json> {
    let jobs = lock()?;
    let job = owned(&jobs, s, p)?;
    job.handle.cancel();
    Ok(json!({"id":job.handle.id().to_string(),"cancel_requested":true}))
}
fn forget(s: &mut Session, p: &Json) -> Result<Json> {
    let mut jobs = lock()?;
    let job = owned(&jobs, s, p)?;
    if matches!(job.handle.snapshot().map_err(sdk_error)?.status, JobStatus::Queued | JobStatus::Running) {
        return Err(bad(SOLVE, "Cancel the job and wait for completion before forgetting it"));
    }
    let id = id(p)?;
    jobs.remove(&id);
    Ok(json!({"forgotten":id.to_string()}))
}
fn apply_job(s: &mut Session, p: &Json) -> Result<Json> {
    let preview = result(s, p)?;
    super::apply(s, &preview)
}
