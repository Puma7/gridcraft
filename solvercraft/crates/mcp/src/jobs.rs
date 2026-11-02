//! Strict, bounded adapters to the shared SDK job/runtime entry points.
use crate::{Config, bounded_json, failure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use solvercraft::{
    compile::Expr,
    jobs::{JobHandle, JobStatus},
    run_manifest::{self, PreparedManifest},
    runtime::{self, RuntimeLimits},
    types::*,
    workflow::{ExecutionRequest, ExecutionRun},
};
use std::{collections::BTreeMap, time::Duration};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Configuration {
    Solve { options: SolveOptions, limits: RuntimeLimits },
    Simulate { options: SimulationOptions, limits: RuntimeLimits },
    Execute { options: SolveOptions, limits: RuntimeLimits },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolveRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    limits: RuntimeLimits,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulationRequest {
    inputs: Vec<Distribution>,
    #[serde(default)]
    outputs: Vec<Expr>,
    #[serde(default)]
    options: SimulationOptions,
    #[serde(default)]
    limits: RuntimeLimits,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecuteRequest {
    request: ExecutionRequest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRequest {
    job_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    job_id: String,
    #[serde(default = "yes")]
    include_result: bool,
}
fn yes() -> bool {
    true
}
struct Preflight;
impl RunControl for Preflight {
    fn is_cancelled(&self) -> bool {
        true
    }
}
fn parse<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| failure("invalid_arguments", format!("Invalid tool arguments: {e}")))
}

enum Job {
    Solve(JobHandle<SolveResult>, PreparedManifest<Configuration>),
    Simulation(JobHandle<SimulationResult>, PreparedManifest<Configuration>),
    Execute(JobHandle<ExecutionRun>, PreparedManifest<Configuration>),
}
impl Job {
    fn id(&self) -> u64 {
        match self {
            Self::Solve(h, _) => h.id(),
            Self::Simulation(h, _) => h.id(),
            Self::Execute(h, _) => h.id(),
        }
    }
    fn cancel(&self) {
        match self {
            Self::Solve(h, _) => h.cancel(),
            Self::Simulation(h, _) => h.cancel(),
            Self::Execute(h, _) => h.cancel(),
        }
    }
    fn done(&self) -> Result<bool> {
        match self {
            Self::Solve(h, _) => Ok(h.snapshot()?.result.is_some()),
            Self::Simulation(h, _) => Ok(h.snapshot()?.result.is_some()),
            Self::Execute(h, _) => Ok(h.snapshot()?.result.is_some()),
        }
    }
    fn view(&self, include_result: bool, max_bytes: usize) -> Result<Value> {
        match self {
            Self::Solve(h, m) => view(h, m, include_result, max_bytes, |r| r.engine.clone()),
            Self::Simulation(h, m) => {
                view(h, m, include_result, max_bytes, |r| format!("solvercraft-simulation-{}/{}", solvercraft::VERSION, r.sampling_version))
            }
            Self::Execute(h, m) => view(h, m, include_result, max_bytes, |r| r.manifest.engine.clone()),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum ResultView<'a, T> {
    Success { ok: bool, value: &'a T },
    Failure { ok: bool, error: &'a SolverError },
}
#[derive(Serialize)]
struct View<'a, T> {
    job_id: String,
    status: &'static str,
    progress: &'a Option<ProgressEvent>,
    result_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<ResultView<'a, T>>,
    manifest: &'a run_manifest::Manifest<Configuration>,
}
fn view<T: Clone + Serialize>(
    handle: &JobHandle<T>,
    prepared: &PreparedManifest<Configuration>,
    include_result: bool,
    max_bytes: usize,
    engine: impl FnOnce(&T) -> String,
) -> Result<Value> {
    let snapshot = handle.snapshot()?;
    let status = match snapshot.status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
    };
    let engine = match &snapshot.result {
        Some(Ok(value)) => engine(value.as_ref()),
        Some(Err(_)) => "unknown: failed before a typed engine result".into(),
        None => "pending: no engine result yet".into(),
    };
    let manifest = prepared.clone().complete(&(), &engine, &format!("mcp:job/{}", snapshot.id))?.manifest;
    let result = if include_result {
        match &snapshot.result {
            Some(Ok(value)) => Some(ResultView::Success { ok: true, value: value.as_ref() }),
            Some(Err(error)) => Some(ResultView::Failure { ok: false, error }),
            None => None,
        }
    } else {
        None
    };
    let view = View {
        job_id: snapshot.id.to_string(),
        status,
        progress: &snapshot.progress,
        result_available: snapshot.result.is_some(),
        result,
        manifest: &manifest,
    };
    // Serialize borrowed typed values into a bounded buffer before constructing
    // a JSON value tree or the MCP text envelope. Retained samples are not cloned.
    let bytes = bounded_json(&view, max_bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| failure("serialization", "Could not encode the bounded job view"))
}

struct Entry {
    job: Job,
    reserved_bytes: u64,
}
pub(crate) struct Jobs {
    config: Config,
    retained: BTreeMap<String, Entry>,
    stopping: bool,
}
impl Jobs {
    pub(crate) fn new(config: Config) -> Self {
        Self { config, retained: BTreeMap::new(), stopping: false }
    }
    fn admit(&self, memory: u64) -> Result<()> {
        if self.stopping {
            return Err(failure("stopping", "The MCP process is shutting down"));
        }
        if memory > self.config.max_job_memory_bytes {
            return Err(failure("quota", "Requested memory exceeds the per-job quota"));
        }
        if self.retained.len() >= self.config.max_retained_jobs {
            return Err(failure("quota", "Retained job quota reached; forget a terminal job"));
        }
        let reserved = self.retained.values().try_fold(memory, |sum, entry| sum.checked_add(entry.reserved_bytes));
        if reserved.is_none_or(|sum| sum > self.config.max_reserved_memory_bytes) {
            return Err(failure("quota", "Retained jobs exceed the process memory reservation quota"));
        }
        Ok(())
    }
    fn limits(&self, mut limits: RuntimeLimits) -> RuntimeLimits {
        limits.max_time_millis = Some(limits.max_time_millis.unwrap_or(self.config.max_job_time_millis).min(self.config.max_job_time_millis));
        limits
    }
    fn response_len<T: Serialize + ?Sized>(&self, value: &T) -> Result<usize> {
        Ok(bounded_json(value, self.config.max_request_bytes)?.len())
    }
    fn name_reserve<'a>(&self, mut names: impl Iterator<Item = &'a str>) -> Result<usize> {
        names.try_fold(0, |largest, name| Ok(largest.max(self.response_len(name)?)))
    }
    fn decision_response(&self, variables: usize, extra_bytes: usize) -> Result<()> {
        // One returned decision vector, scalar result/progress/engine fields,
        // warnings and the outer manifest. Account for caller-owned strings in
        // a possible diagnostic and portable execution manifest separately.
        let bytes = variables.checked_mul(32).and_then(|n| n.checked_add(4096)).and_then(|n| n.checked_add(extra_bytes));
        if bytes.is_none_or(|n| n > self.config.max_response_bytes / 4) {
            return Err(failure(
                "quota",
                "Decision values or execution metadata would exceed the response quota; reduce the model or increase the response budget",
            ));
        }
        Ok(())
    }
    fn store(&mut self, job: Job, reserved_bytes: u64) -> Value {
        let id = job.id().to_string();
        self.retained.insert(id.clone(), Entry { job, reserved_bytes });
        json!({"job_id":id,"result_requires_polling":true})
    }
    fn find(&self, id: &str) -> Result<&Entry> {
        if id.len() > 20 || id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(failure("invalid_arguments", "job_id must be a decimal string"));
        }
        self.retained.get(id).ok_or_else(|| failure("unknown_job", "Unknown job ID in this MCP process"))
    }
    pub(crate) fn call(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let seed = if name == "solvercraft_start_execute" {
            arguments.get("request").and_then(|v| v.get("model")).and_then(|v| v.get("options")).and_then(|v| v.get("seed"))
        } else {
            arguments.get("options").and_then(|v| v.get("seed"))
        };
        if seed.is_some_and(|value| !value.is_string()) {
            return Err(failure("invalid_arguments", "MCP seeds must be lossless decimal strings"));
        }
        match name {
            "solvercraft_start_solve" => {
                let mut request: SolveRequest = parse(arguments)?;
                self.admit(request.options.max_memory_bytes)?;
                solvercraft::model::validate(&request.model, &request.options)?;
                let names = self.name_reserve(
                    std::iter::once(request.model.name.as_str())
                        .chain(request.model.variables.iter().map(|v| v.name.as_str()))
                        .chain(request.model.constraints.iter().map(|r| r.name.as_str())),
                )?;
                self.decision_response(request.model.variables.len(), names)?;
                request.limits = self.limits(request.limits);
                let prepared = run_manifest::prepare(
                    "mcp.start_solve",
                    &request,
                    Configuration::Solve { options: request.options.clone(), limits: request.limits },
                    self.config.max_request_bytes as u64,
                )?;
                let memory = request.options.max_memory_bytes;
                let handle = solvercraft::jobs::start_shared_job(move |control| {
                    runtime::solve_with_limits_owned(&request.model, &request.options, request.limits, control)
                })?;
                Ok(self.store(Job::Solve(handle, prepared), memory))
            }
            "solvercraft_start_simulation" => {
                let mut request: SimulationRequest = parse(arguments)?;
                self.admit(request.options.max_memory_bytes)?;
                if request.inputs.is_empty() || request.inputs.len() > 4096 || request.outputs.len() > 4096 {
                    return Err(failure("quota", "Simulation requires 1..4096 inputs and at most 4096 outputs"));
                }
                let dimensions = if request.outputs.is_empty() { request.inputs.len() } else { request.outputs.len() };
                // A terminal view always contains per-output summaries, even
                // without samples. Reserve their worst-case JSON numeric tokens,
                // five quantile pairs and keys, plus the fixed SDK/job manifest.
                let summaries = (dimensions as u64).checked_mul(768).and_then(|n| n.checked_add(4096));
                let response_bytes = if request.options.retain_samples {
                    request.options.trials.checked_mul(dimensions as u64).and_then(|n| n.checked_mul(32)).and_then(|n| n.checked_add(summaries?))
                } else {
                    summaries
                };
                if response_bytes.is_none_or(|bytes| bytes > (self.config.max_response_bytes / 4) as u64) {
                    return Err(failure(
                        "quota",
                        "Simulation summaries or samples would exceed the response quota; reduce outputs/trials or disable retained samples",
                    ));
                }
                for input in &request.inputs {
                    solvercraft::math::validate_distribution(input)?;
                }
                solvercraft::compile::validate_expressions(
                    &request.outputs,
                    request.inputs.len(),
                    &SolveOptions { max_memory_bytes: request.options.max_memory_bytes, ..SolveOptions::default() },
                )?;
                // Validate options and all aggregate simulation allocation limits
                // before admitting a worker, using the SDK's own preflight.
                solvercraft::simulation::simulate(&request.inputs, &request.options, &Preflight, |_| {
                    Err(failure("preflight", "No trial evaluator should run during preflight"))
                })?;
                if request.options.trials.checked_mul(dimensions as u64).is_none_or(|work| work > 100_000_000)
                    || (dimensions as u64).checked_mul(2048).is_none_or(|bytes| bytes > request.options.max_memory_bytes)
                {
                    return Err(failure("quota", "Simulation output work or summary storage exceeds the host quota"));
                }
                request.limits = self.limits(request.limits);
                let prepared = run_manifest::prepare(
                    "mcp.start_simulation",
                    &request,
                    Configuration::Simulate { options: request.options.clone(), limits: request.limits },
                    self.config.max_request_bytes as u64,
                )?;
                let memory = request.options.max_memory_bytes;
                let handle = solvercraft::jobs::start_job(move |control| {
                    let deadline = runtime::DeadlineControl::new(control, request.limits)?;
                    let mut result = solvercraft::simulation::simulate(&request.inputs, &request.options, &deadline, |values| {
                        if request.outputs.is_empty() {
                            return Ok(values.to_vec());
                        }
                        request.outputs.iter().map(|expr| solvercraft::compile::evaluate(expr, values)).collect()
                    })?;
                    deadline.apply_simulation_termination(&mut result);
                    Ok(result)
                })?;
                Ok(self.store(Job::Simulation(handle, prepared), memory))
            }
            "solvercraft_start_execute" => {
                let mut arguments: ExecuteRequest = parse(arguments)?;
                let request = &mut arguments.request;
                self.admit(request.model.options.max_memory_bytes)?;
                request.model.validate()?;
                request.nonlinear.validate()?;
                if request.schema_version != 2
                    || [&request.context.run_id, &request.context.snapshot_id, &request.context.host]
                        .iter()
                        .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
                    || request.context.snapshot_revision.as_ref().is_some_and(|s| s.is_empty() || s.len() > 256)
                    || request.context.source_revision.as_ref().is_some_and(|s| s.is_empty() || s.len() > 128)
                {
                    return Err(failure("invalid_context", "Expected execution schema 2 and bounded run/snapshot/host identities"));
                }
                let (variables, names) = match &request.model.definition {
                    solvercraft::project::ModelDefinition::Algebraic(model) => (
                        model.variables.len(),
                        self.name_reserve(
                            std::iter::once(model.name.as_str())
                                .chain(model.variables.iter().map(|v| v.name.as_str()))
                                .chain(model.constraints.iter().map(|r| r.name.as_str())),
                        )?,
                    ),
                    solvercraft::project::ModelDefinition::Expression(model) => (
                        model.variables.len(),
                        self.name_reserve(
                            std::iter::once(model.name.as_str())
                                .chain(model.variables.iter().map(|v| v.name.as_str()))
                                .chain(model.constraints.iter().map(|r| r.name.as_str())),
                        )?,
                    ),
                };
                let metadata = [
                    self.response_len(&request.context)?,
                    self.response_len(&request.model.id)?,
                    self.response_len(&request.model.revision)?,
                    self.response_len(&request.model.options)?,
                    self.response_len(&request.nonlinear)?,
                    names,
                ]
                .into_iter()
                .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
                .ok_or_else(|| failure("quota", "Execution response size overflow"))?;
                self.decision_response(variables, metadata)?;
                request.limits = self.limits(request.limits);
                let prepared = run_manifest::prepare(
                    "mcp.start_execute",
                    &arguments,
                    Configuration::Execute { options: arguments.request.model.options.clone(), limits: arguments.request.limits },
                    self.config.max_request_bytes as u64,
                )?;
                let memory = arguments.request.model.options.max_memory_bytes;
                let handle = solvercraft::jobs::start_shared_job(move |control| solvercraft::workflow::execute_owned(&arguments.request, control))?;
                Ok(self.store(Job::Execute(handle, prepared), memory))
            }
            "solvercraft_job_get" => {
                let request: GetRequest = parse(arguments)?;
                self.find(&request.job_id)?.job.view(request.include_result, self.config.max_response_bytes / 4)
            }
            "solvercraft_job_cancel" => {
                let request: JobRequest = parse(arguments)?;
                self.find(&request.job_id)?.job.cancel();
                Ok(json!({"cancel_requested":true}))
            }
            "solvercraft_job_forget" => {
                let request: JobRequest = parse(arguments)?;
                if !self.find(&request.job_id)?.job.done()? {
                    return Err(failure("job_running", "Cancel and await a terminal job before forgetting it"));
                }
                self.retained.remove(&request.job_id);
                Ok(json!({"forgotten":true}))
            }
            _ => Err(failure("unknown_tool", "Unknown Solvercraft tool")),
        }
    }
    pub(crate) fn cancel_all(&self) {
        for entry in self.retained.values() {
            entry.job.cancel();
        }
    }
    pub(crate) fn shutdown(&mut self) -> Result<()> {
        self.stopping = true;
        self.cancel_all();
        let deadline = runtime::monotonic_now() + Duration::from_millis(self.config.shutdown_grace_millis);
        loop {
            let mut all_done = true;
            for entry in self.retained.values() {
                all_done &= entry.job.done()?;
            }
            if all_done {
                self.retained.clear();
                return Ok(());
            }
            if runtime::monotonic_now() >= deadline {
                return Err(failure("shutdown_timeout", "Jobs did not acknowledge cancellation within the shutdown grace period"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
