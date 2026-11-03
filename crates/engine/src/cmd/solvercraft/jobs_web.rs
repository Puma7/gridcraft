//! Browser job transport. One actual Web Worker owns each immutable snapshot;
//! cancellation terminates that worker and explicitly retains no unseen incumbent.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;

use ::solvercraft::runtime::{OwnedDeadlineControl, RuntimeLimits};
use ::solvercraft::types::{
    Feasibility, Optimality, ProgressEvent, RunControl, SimulationResult, SolveResult, SolveStatistics, SolverError, Termination,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{DedicatedWorkerGlobalScope, ErrorEvent, MessageEvent, Worker};

use super::{
    CommandSpec, Json, Result, SOLVE, Session, Snapshot, Workbook, bad, cmd, has_doc, json, parameters, registry, sdk_error, simulate_snapshot,
    snapshot, solve_snapshot, str_param,
};

const WORKER_LOADER: &str = "gridcraft-solver-worker_loader.js";
const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
const MAX_MESSAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_RESERVED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_MEMORY: u64 = 64 * 1024 * 1024;
thread_local! {
    static JOBS:RefCell<BTreeMap<u64,Entry>>=const {RefCell::new(BTreeMap::new())};
    static NEXT_ID:std::cell::Cell<u64>=const {std::cell::Cell::new(1)};
}
struct State {
    status: &'static str,
    progress: Option<ProgressEvent>,
    result: Option<::solvercraft::types::Result<Arc<Json>>>,
    startup_ms: Option<f64>,
}
struct Entry {
    snapshot: Snapshot,
    kind: String,
    request: Json,
    limits: RuntimeLimits,
    reserved: u64,
    worker: Worker,
    state: Rc<RefCell<State>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(ErrorEvent)>,
    _deadline: Closure<dyn FnMut()>,
    timer: i32,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
        if let Some(window) = web_sys::window() {
            window.clear_timeout_with_handle(self.timer);
        }
    }
}
fn with_jobs<T>(operation: impl FnOnce(&mut BTreeMap<u64, Entry>) -> Result<T>) -> Result<T> {
    JOBS.with(|store| {
        let mut jobs = store.try_borrow_mut().map_err(|_| bad(SOLVE, "Browser job registry is busy"))?;
        operation(&mut jobs)
    })
}
pub(super) fn document_closed(uid: u64) {
    let _ = with_jobs(|jobs| {
        let uid = uid.to_string();
        jobs.retain(|_, job| job.snapshot.document_uid != uid);
        Ok(())
    });
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Work {
    workbook: Workbook,
    request: Json,
    snapshot: Snapshot,
    kind: String,
    limits: RuntimeLimits,
}
struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().checked_add(bytes.len()).is_none_or(|size| size > self.limit) {
            return Err(std::io::Error::other("Browser message exceeds its byte budget"));
        }
        // Geometric growth keeps serialization amortized linear; the checked
        // written-byte limit also bounds spare capacity to under twice it.
        self.bytes.try_reserve(bytes.len()).map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize, limit: usize) -> std::result::Result<String, String> {
    let mut writer = LimitedWriter { bytes: Vec::new(), limit };
    serde_json::to_writer(&mut writer, value).map_err(|e| e.to_string())?;
    String::from_utf8(writer.bytes).map_err(|e| e.to_string())
}
pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.job.start","Start Solvercraft browser job",[],None,"{kind?:optimization|simulation,request?,model_id?,limits?} -> string job id and snapshot. Real separate Web Worker; bounded16MiB workbook transport; Trunk loader required.",has_doc,start),
        cmd!(query "solvercraft.job.get","Solvercraft browser job status",[],None,"{id:string} -> nonblocking status/progress/stale/timing metadata; no samplematrix copying.",has_doc,get),
        cmd!(query "solvercraft.job.list","List Solvercraft browser jobs",[],None,"{} -> retained jobs owned by this workbook only.",has_doc,list),
        cmd!(query "solvercraft.job.result","Solvercraft browser job result",[],None,"{id:string} -> completed immutable preview. Hardcancel reports Unknown feasibility and no unseen incumbent.",has_doc,result),
        cmd!(
            "solvercraft.job.report",
            "Create Solvercraft result sheet",
            [],
            None,
            "{id:string,name?:string,language?:de|en} -> new sheet with retained results; one undo step.",
            has_doc,
            report
        ),
        cmd!(query "solvercraft.job.cancel","Cancel Solvercraft browser worker",[],None,"{id:string} -> terminate this worker; no untransmitted incumbent is claimed.",has_doc,cancel),
        cmd!(query "solvercraft.job.forget","Forget Solvercraft browser job",[],None,"{id:string} -> releases terminal worker/result state.",has_doc,forget),
        cmd!(
            "solvercraft.job.apply",
            "Apply Solvercraft browser result",
            [],
            None,
            "{id:string} -> the normal revision-guarded atomic undoable apply command.",
            has_doc,
            apply_job
        ),
    ]
}
fn id(p: &Json) -> Result<u64> {
    p.get("id").and_then(Json::as_str).and_then(|id| id.parse().ok()).ok_or_else(|| bad(SOLVE, "A decimal string job id is required"))
}
fn owned<'a>(jobs: &'a BTreeMap<u64, Entry>, s: &Session, p: &Json) -> Result<&'a Entry> {
    let entry = jobs.get(&id(p)?).ok_or_else(|| bad(SOLVE, "Unknown Solvercraft job"))?;
    if entry.snapshot.document_uid != s.doc()?.uid.to_string() {
        return Err(bad(SOLVE, "This job belongs to another workbook"));
    }
    Ok(entry)
}
fn failed(state: &Rc<RefCell<State>>, message: impl Into<String>) {
    if let Ok(mut state) = state.try_borrow_mut()
        && matches!(state.status, "queued" | "running")
    {
        state.status = "failed";
        state.result = Some(Err(SolverError::new("browser_worker", message)));
    }
}
fn cancelled_preview(kind: &str, request: &Json, snapshot: &Snapshot, termination: Termination, limits: &RuntimeLimits) -> Json {
    let warning = "Browser worker terminated. No untransmitted partial sample matrix or feasible incumbent is claimed.";
    let result = if kind == "simulation" {
        json!(SimulationResult {
            termination,
            completed_trials: 0,
            summaries: Vec::new(),
            samples: None,
            sampling_version: "no_browser_result_received".into(),
            warnings: vec![warning.into()]
        })
    } else {
        json!(SolveResult {
            termination,
            feasibility: Feasibility::Unknown,
            optimality: Optimality::Unknown,
            values: None,
            objective: None,
            best_bound: None,
            relative_gap: None,
            max_violation: None,
            statistics: SolveStatistics::default(),
            engine: "solvercraft-browser-worker".into(),
            warnings: vec![warning.into()]
        })
    };
    let mut preview = json!({"result":result,"request":request,"snapshot":snapshot,"bindings":[],"execution":"browser_worker_snapshot"});
    match super::manifest::create(
        kind,
        snapshot,
        request,
        json!({"requested":request.get("options"),"worker_validation_observed":false}),
        json!({"engine":null,"execution":"browser_worker_terminated","result_observed":false}),
        json!({"resolution":"worker result not received"}),
    ) {
        Ok(manifest) => preview["manifest"] = manifest,
        Err(error) => preview["manifest_error"] = json!(error.to_string()),
    }
    super::manifest::annotate_limits(&mut preview, limits);
    preview
}
fn stop(
    worker: &Worker,
    state: &Rc<RefCell<State>>,
    kind: &str,
    request: &Json,
    snapshot: &Snapshot,
    termination: Termination,
    limits: &RuntimeLimits,
) {
    if let Ok(mut state) = state.try_borrow_mut()
        && matches!(state.status, "queued" | "running")
    {
        worker.terminate();
        worker.set_onmessage(None);
        worker.set_onerror(None);
        state.status = "completed";
        state.result = Some(Ok(Arc::new(cancelled_preview(kind, request, snapshot, termination, limits))));
    }
}
fn start(s: &mut Session, p: &Json) -> Result<Json> {
    let admitted = web_time::Instant::now();
    let (kind, mut request) = if let Some(model_id) = p.get("model_id") {
        let model = registry::get(s, &json!({"id":model_id}))?;
        (model["kind"].as_str().unwrap_or("optimization").to_string(), model["request"].clone())
    } else {
        (
            str_param(p, "kind").unwrap_or("optimization").to_string(),
            p.get("request").cloned().ok_or_else(|| bad(SOLVE, "request or model_id is required"))?,
        )
    };
    if !matches!(kind.as_str(), "optimization" | "simulation") || !request.is_object() {
        return Err(bad(SOLVE, "A supported job kind and object request are required"));
    }
    if encode(&request, 131_072).is_err() {
        return Err(bad(SOLVE, "Job request exceeds128KiB"));
    }
    super::pin_request_sheet(&s.doc()?.wb, &mut request, &kind)?;
    if request.get("options").is_none() {
        request["options"] = json!({});
    }
    if !request["options"].is_object() {
        return Err(bad(SOLVE, "Job options must be an object"));
    }
    if request["options"].get("max_memory_bytes").is_none() {
        request["options"]["max_memory_bytes"] = json!(DEFAULT_MEMORY);
    }
    let reserved = request["options"]["max_memory_bytes"]
        .as_u64()
        .filter(|n| *n > 0)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| bad(SOLVE, "Invalid browser job memory budget"))?;
    let mut limits: RuntimeLimits = serde_json::from_value(p.get("limits").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    let milliseconds = i32::try_from(limits.max_time_millis.unwrap_or(300_000))
        .map_err(|_| bad(SOLVE, "Browser time limit must fit a signed32-bit millisecond timer"))?;
    limits.max_time_millis = Some(milliseconds as u64);
    let window = web_sys::window().ok_or_else(|| bad(SOLVE, "Browser job admission requires a window host"))?;
    let captured = snapshot(s)?;
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(s.doc()?.calc.fixed_now.unwrap_or_else(gridcraft_calc::now_serial));
    }
    let payload = encode(
        &Work { workbook: (*s.doc()?.wb).clone(), request: request.clone(), snapshot: captured.clone(), kind: kind.clone(), limits },
        MAX_SNAPSHOT_BYTES,
    )
    .map_err(sdk_error)?;
    with_jobs(|jobs| {
        if jobs.len() >= 64 || jobs.values().fold(reserved, |sum, e| sum.saturating_add(e.reserved)) > MAX_RESERVED_BYTES {
            return Err(bad(SOLVE, "Browser retained job limit reached; forget completed jobs"));
        }
        let active = jobs.values().filter(|e| e.state.try_borrow().map_or(true, |s| matches!(s.status, "queued" | "running"))).count();
        if active >= 4 {
            return Err(bad(SOLVE, "Four browser workers are already active"));
        }
        let id = NEXT_ID.with(|next| {
            let value = next.get();
            let following = value.checked_add(1).ok_or_else(|| bad(SOLVE, "Browser job ids exhausted"))?;
            next.set(following);
            Ok(value)
        })?;
        let worker = Worker::new(WORKER_LOADER).map_err(|e| bad(SOLVE, format!("Cannot start Solvercraft worker: {e:?}")))?;
        let state = Rc::new(RefCell::new(State { status: "queued", progress: None, result: None, startup_ms: None }));
        let pending = Rc::new(RefCell::new(Some(payload)));
        let message = message_callback(worker.clone(), Rc::clone(&state), pending, admitted);
        worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
        let error_state = Rc::clone(&state);
        let error_worker = worker.clone();
        let error = Closure::wrap(Box::new(move |event: ErrorEvent| {
            event.prevent_default();
            failed(&error_state, event.message());
            error_worker.terminate();
        }) as Box<dyn FnMut(ErrorEvent)>);
        worker.set_onerror(Some(error.as_ref().unchecked_ref()));
        let timeout_worker = worker.clone();
        let timeout_state = Rc::clone(&state);
        let timeout_kind = kind.clone();
        let timeout_request = request.clone();
        let timeout_snapshot = captured.clone();
        let deadline = Closure::wrap(Box::new(move || {
            stop(&timeout_worker, &timeout_state, &timeout_kind, &timeout_request, &timeout_snapshot, Termination::TimeLimit, &limits)
        }) as Box<dyn FnMut()>);
        let timer = match window.set_timeout_with_callback_and_timeout_and_arguments_0(deadline.as_ref().unchecked_ref(), milliseconds) {
            Ok(timer) => timer,
            Err(error) => {
                worker.set_onmessage(None);
                worker.set_onerror(None);
                worker.terminate();
                return Err(bad(SOLVE, format!("Cannot install browser deadline: {error:?}")));
            }
        };
        jobs.insert(
            id,
            Entry {
                snapshot: captured.clone(),
                kind: kind.clone(),
                request: request.clone(),
                limits,
                reserved,
                worker,
                state,
                _message: message,
                _error: error,
                _deadline: deadline,
                timer,
            },
        );
        Ok(
            json!({"id":id.to_string(),"snapshot":captured,"execution":"browser_worker_snapshot","admission_ms":admitted.elapsed().as_secs_f64()*1000.0}),
        )
    })
}
fn message_callback(
    worker: Worker,
    state: Rc<RefCell<State>>,
    pending: Rc<RefCell<Option<String>>>,
    started: web_time::Instant,
) -> Closure<dyn FnMut(MessageEvent)> {
    Closure::wrap(Box::new(move |event: MessageEvent| {
        let Some(text) = event.data().as_string() else {
            failed(&state, "Worker returned a non-string message");
            worker.terminate();
            return;
        };
        if text.len() > MAX_MESSAGE_BYTES {
            failed(&state, "Worker result exceeds the browser message budget");
            worker.terminate();
            return;
        }
        let mut message: Json = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                failed(&state, error.to_string());
                worker.terminate();
                return;
            }
        };
        match message.get("event").and_then(Json::as_str) {
            Some("ready") => {
                if let Ok(mut status) = state.try_borrow_mut() {
                    if status.status != "queued" {
                        return;
                    }
                    status.status = "running";
                    status.startup_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
                }
                if let Ok(mut payload) = pending.try_borrow_mut()
                    && let Some(payload) = payload.take()
                    && let Err(error) = worker.post_message(&JsValue::from_str(&payload))
                {
                    failed(&state, format!("Cannot send workbook snapshot: {error:?}"));
                    worker.terminate();
                }
            }
            Some("progress") => {
                let progress = message
                    .as_object_mut()
                    .and_then(|object| object.remove("progress"))
                    .ok_or_else(|| "Missing worker progress".to_string())
                    .and_then(|value| serde_json::from_value(value).map_err(|e| e.to_string()));
                match progress {
                    Ok(progress) => {
                        if let Ok(mut status) = state.try_borrow_mut() {
                            status.progress = Some(progress);
                        }
                    }
                    Err(error) => failed(&state, error),
                }
            }
            Some("complete") => {
                let Some(preview) = message.as_object_mut().and_then(|object| object.remove("preview")) else {
                    failed(&state, "Missing worker preview");
                    worker.terminate();
                    return;
                };
                if let Ok(mut status) = state.try_borrow_mut()
                    && matches!(status.status, "queued" | "running")
                {
                    status.status = "completed";
                    status.result = Some(Ok(Arc::new(preview)));
                }
                worker.set_onmessage(None);
                worker.set_onerror(None);
                worker.terminate();
            }
            Some("failed") => {
                failed(&state, message["message"].as_str().unwrap_or("Worker evaluation failed"));
                worker.terminate();
            }
            _ => {
                failed(&state, "Unknown Solvercraft worker message");
                worker.terminate();
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>)
}
fn summary(id: u64, entry: &Entry, current: &Snapshot) -> Result<Json> {
    let state = entry.state.try_borrow().map_err(|_| bad(SOLVE, "Browser job state is busy"))?;
    let error = state.result.as_ref().and_then(|result| result.as_ref().err());
    Ok(
        json!({"id":id.to_string(),"kind":entry.kind,"status":state.status,"progress":state.progress,"snapshot":entry.snapshot,"stale":entry.snapshot.document_uid!=current.document_uid||entry.snapshot.revision!=current.revision,"startup_ms":state.startup_ms,"error":error,"execution":"browser_worker_snapshot"}),
    )
}
fn get(s: &mut Session, p: &Json) -> Result<Json> {
    with_jobs(|jobs| summary(id(p)?, owned(jobs, s, p)?, &snapshot(s)?))
}
fn list(s: &mut Session, _: &Json) -> Result<Json> {
    let current = snapshot(s)?;
    with_jobs(|jobs| {
        Ok(
            json!({"jobs":jobs.iter().filter(|(_,entry)|entry.snapshot.document_uid==current.document_uid).map(|(id,entry)|summary(*id,entry,&current)).collect::<Result<Vec<_>>>()?}),
        )
    })
}
fn result(s: &mut Session, p: &Json) -> Result<Json> {
    Ok((*retained_result(s, p)?).clone())
}
fn report(s: &mut Session, p: &Json) -> Result<Json> {
    let preview = retained_result(s, p)?;
    super::result_sheet::write(s, p, &preview)
}
fn retained_result(s: &Session, p: &Json) -> Result<Arc<Json>> {
    with_jobs(|jobs| {
        let entry = owned(jobs, s, p)?;
        let state = entry.state.try_borrow().map_err(|_| bad(SOLVE, "Browser job state is busy"))?;
        let result = state.result.as_ref().ok_or_else(|| bad(SOLVE, "Browser job is still pending"))?;
        match result {
            Ok(value) => Ok(Arc::clone(value)),
            Err(error) => Err(sdk_error(error)),
        }
    })
}
fn cancel(s: &mut Session, p: &Json) -> Result<Json> {
    with_jobs(|jobs| {
        let entry = owned(jobs, s, p)?;
        stop(&entry.worker, &entry.state, &entry.kind, &entry.request, &entry.snapshot, Termination::Cancelled, &entry.limits);
        Ok(json!({"id":id(p)?.to_string(),"cancel_requested":true,"mode":"worker_termination_no_untransmitted_incumbent"}))
    })
}
fn forget(s: &mut Session, p: &Json) -> Result<Json> {
    with_jobs(|jobs| {
        let entry = owned(jobs, s, p)?;
        let active = entry.state.try_borrow().map_err(|_| bad(SOLVE, "Browser job state is busy"))?.status;
        if matches!(active, "queued" | "running") {
            return Err(bad(SOLVE, "Cancel the browser job before forgetting it"));
        }
        let id = id(p)?;
        jobs.remove(&id);
        Ok(json!({"forgotten":id.to_string()}))
    })
}
fn apply_job(s: &mut Session, p: &Json) -> Result<Json> {
    let preview = retained_result(s, p)?;
    super::apply(s, &preview)
}

fn post(value: &Json) -> std::result::Result<(), String> {
    let global = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>().map_err(|_| "Not executing in a dedicated Worker".to_string())?;
    let payload = encode(value, MAX_MESSAGE_BYTES)?;
    global.post_message(&JsValue::from_str(&payload)).map_err(|e| format!("Worker message failed: {e:?}"))
}
struct WorkerControl {
    last: std::sync::Mutex<Option<web_time::Instant>>,
}
impl RunControl for WorkerControl {
    fn is_cancelled(&self) -> bool {
        false
    }
    fn progress(&self, event: &ProgressEvent) {
        let now = web_time::Instant::now();
        if let Ok(mut last) = self.last.lock() {
            if last.is_some_and(|last| now.duration_since(last) < std::time::Duration::from_millis(50)) {
                return;
            }
            *last = Some(now);
        }
        let _ = post(&json!({"event":"progress","progress":event}));
    }
}
fn mark_time_limit(preview: &mut Json) {
    if preview["result"]["termination"] == json!("cancelled") {
        preview["result"]["termination"] = json!("time_limit");
    }
    if let Some(cases) = preview.get_mut("cases").and_then(Json::as_array_mut) {
        for case in cases {
            if let Some(preview) = case.get_mut("preview") {
                mark_time_limit(preview);
            }
        }
    }
}
fn run_work(text: &str) -> std::result::Result<Json, String> {
    if text.len() > MAX_SNAPSHOT_BYTES {
        return Err("Worker snapshot exceeds16MiB".into());
    }
    let mut work: Work = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if !matches!(work.kind.as_str(), "optimization" | "simulation") {
        return Err("Unknown worker operation".into());
    }
    let started = web_time::Instant::now();
    let progress: Arc<dyn RunControl> = Arc::new(WorkerControl { last: std::sync::Mutex::new(None) });
    progress.progress(&ProgressEvent { stage: "prepare_snapshot".into(), iterations: 0, evaluations: 0, nodes: 0, best_objective: None });
    let deadline = Arc::new(OwnedDeadlineControl::new(progress, work.limits).map_err(|e| e.to_string())?);
    let control: Arc<dyn RunControl> = deadline.clone();
    let mut calc = gridcraft_calc::Calc::new();
    calc.fixed_now = Some(super::snapshot_time(&work.request).map_err(|e| e.to_string())?);
    calc.rebuild(&work.workbook);
    calc.recalc_all(&mut work.workbook);
    let evaluated = if work.kind == "simulation" {
        if let Some(cases) = work.request.get("parameter_cases") {
            parameters::execute(&work.workbook, &calc, work.snapshot, &work.request, cases, control.as_ref())
        } else {
            simulate_snapshot(&work.workbook, &calc, work.snapshot, &work.request, control.as_ref())
        }
    } else {
        solve_snapshot(&work.workbook, &calc, work.snapshot, &work.request, control)
    };
    let mut preview = evaluated.map_err(|e| e.to_string())?;
    if deadline.timed_out() {
        mark_time_limit(&mut preview);
    }
    preview["execution"] = json!("browser_worker_snapshot");
    super::manifest::annotate_limits(&mut preview, &work.limits);
    preview["worker_compute_ms"] = json!(started.elapsed().as_secs_f64() * 1000.0);
    Ok(preview)
}
pub(super) fn worker_main() {
    let Ok(global) = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>() else {
        log::error!("Solvercraft requires a dedicated Worker scope");
        return;
    };
    let used = std::cell::Cell::new(false);
    let handler = Closure::wrap(Box::new(move |event: MessageEvent| {
        if used.replace(true) {
            let _ = post(&json!({"event":"failed","message":"One operation is allowed per worker"}));
            return;
        }
        let output = event.data().as_string().ok_or_else(|| "Worker input must be a JSON string".to_string()).and_then(|text| run_work(&text));
        let message = match output {
            Ok(preview) => json!({"event":"complete","preview":preview}),
            Err(message) => json!({"event":"failed","message":message}),
        };
        if let Err(message) = post(&message) {
            let _ = post(&json!({"event":"failed","message":message}));
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    global.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
    let _ = post(&json!({"event":"ready","protocol_version":1}));
}
