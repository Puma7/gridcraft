//! Optional loopback JSON-lines service. Authentication is required per connection.
//! No listener starts until the host explicitly calls `Server::bind` and `run`.
use crate::run_manifest::{self, PreparedManifest};
use crate::{
    compile::Expr,
    jobs::{self, JobHandle},
    runtime::{self, RuntimeLimits},
    types::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub listen: String,
    /// Supply a secret from the caller. It is never returned by the protocol.
    pub token: String,
    pub max_jobs: usize,
    pub max_connections: usize,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_job_memory_bytes: u64,
    pub read_timeout_millis: u64,
    pub max_job_time_millis: u64,
}
impl ServerConfig {
    pub fn local(token: String) -> Self {
        Self {
            listen: "127.0.0.1:0".into(),
            token,
            max_jobs: 16,
            max_connections: 8,
            max_request_bytes: 2 * 1024 * 1024,
            max_response_bytes: 4 * 1024 * 1024,
            max_job_memory_bytes: 256 * 1024 * 1024,
            read_timeout_millis: 2000,
            max_job_time_millis: 60_000,
        }
    }
    fn validate(&self) -> Result<SocketAddr> {
        let address = self.listen.parse::<SocketAddr>().map_err(|_| error("invalid_server_config", "Expected an IP socket address"))?;
        if !address.ip().is_loopback() {
            return Err(error(
                "invalid_server_config",
                "The built-in service binds only loopback; use an authenticated TLS gateway for remote access",
            ));
        }
        if !(32..=1024).contains(&self.token.len())
            || self.token.chars().any(char::is_control)
            || !(1..=64).contains(&self.max_jobs)
            || !(1..=32).contains(&self.max_connections)
            || !(1024..=16 * 1024 * 1024).contains(&self.max_request_bytes)
            || !(1024..=16 * 1024 * 1024).contains(&self.max_response_bytes)
            || !(1024..=1024 * 1024 * 1024).contains(&self.max_job_memory_bytes)
            || !(10..=30_000).contains(&self.read_timeout_millis)
            || !(1..=3_600_000).contains(&self.max_job_time_millis)
        {
            return Err(error("invalid_server_config", "Server token, quotas or time limits are outside supported ranges"));
        }
        Ok(address)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRequest {
    pub schema_version: u32,
    pub id: String,
    pub command: ServiceCommand,
}
#[derive(Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceCommand {
    Authenticate {
        token: String,
    },
    Capabilities {},
    Solve {
        model: OptimizationModel,
        #[serde(default)]
        options: SolveOptions,
        #[serde(default)]
        limits: RuntimeLimits,
    },
    Simulate {
        inputs: Vec<Distribution>,
        #[serde(default)]
        outputs: Vec<Expr>,
        #[serde(default)]
        options: SimulationOptions,
        #[serde(default)]
        limits: RuntimeLimits,
    },
    Get {
        job_id: String,
        /// Retrieve the full retained identity explicitly; ordinary polls stay small.
        #[serde(default)]
        include_manifest: bool,
    },
    Cancel {
        job_id: String,
    },
    Forget {
        job_id: String,
    },
}
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ServiceConfiguration {
    Solve { options: SolveOptions, limits: RuntimeLimits },
    Simulate { options: SimulationOptions, limits: RuntimeLimits },
}
#[derive(Serialize)]
struct SolveIdentity<'a> {
    model: &'a OptimizationModel,
    options: &'a SolveOptions,
    limits: RuntimeLimits,
}
#[derive(Serialize)]
struct SimulationIdentity<'a> {
    inputs: &'a [Distribution],
    outputs: &'a [Expr],
    options: &'a SimulationOptions,
    limits: RuntimeLimits,
}
enum ServiceJob {
    Solve(JobHandle<SolveResult>, PreparedManifest<ServiceConfiguration>),
    Simulation(JobHandle<SimulationResult>, PreparedManifest<ServiceConfiguration>),
}
impl ServiceJob {
    fn id(&self) -> u64 {
        match self {
            Self::Solve(h, _) => h.id(),
            Self::Simulation(h, _) => h.id(),
        }
    }
    fn cancel(&self) {
        match self {
            Self::Solve(h, _) => h.cancel(),
            Self::Simulation(h, _) => h.cancel(),
        }
    }
    fn done(&self) -> Result<bool> {
        match self {
            Self::Solve(h, _) => Ok(h.snapshot()?.result.is_some()),
            Self::Simulation(h, _) => Ok(h.snapshot()?.result.is_some()),
        }
    }
    fn json(&self, include_manifest: bool, limit: usize) -> Result<Value> {
        match self {
            Self::Solve(h, manifest) => job_json(h, manifest, include_manifest, limit, |result| result.engine.clone()),
            Self::Simulation(h, manifest) => job_json(h, manifest, include_manifest, limit, |result| {
                format!("solvercraft-simulation-{}/{}", crate::VERSION, result.sampling_version)
            }),
        }
    }
}
#[derive(Serialize)]
#[serde(untagged)]
enum JobResultView<'a, T> {
    Success { ok: bool, value: &'a T },
    Failure { ok: bool, error: &'a SolverError },
}
#[derive(Serialize)]
struct JobView<'a, T, C> {
    job_id: String,
    status: String,
    progress: &'a Option<ProgressEvent>,
    result: Option<JobResultView<'a, T>>,
    manifest_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest: Option<&'a crate::run_manifest::Manifest<C>>,
}
fn job_json<T: Clone + Serialize>(
    handle: &JobHandle<T>,
    prepared: &PreparedManifest<ServiceConfiguration>,
    include_manifest: bool,
    limit: usize,
    engine: impl FnOnce(&T) -> String,
) -> Result<Value> {
    let snapshot = handle.snapshot()?;
    let actual_engine = match &snapshot.result {
        Some(Ok(value)) => engine(value.as_ref()),
        Some(Err(_)) => "unknown: execution failed before a typed engine result".into(),
        None => "pending: no engine result yet".into(),
    };
    let manifest =
        if include_manifest { Some(prepared.clone().complete(&(), &actual_engine, &format!("server:job/{}", snapshot.id))?.manifest) } else { None };
    let result = match &snapshot.result {
        None => None,
        Some(Ok(value)) => Some(JobResultView::Success { ok: true, value: value.as_ref() }),
        Some(Err(error)) => Some(JobResultView::Failure { ok: false, error }),
    };
    let view = JobView {
        job_id: snapshot.id.to_string(),
        status: format!("{:?}", snapshot.status).to_lowercase(),
        progress: &snapshot.progress,
        result,
        manifest_available: true,
        manifest: manifest.as_ref(),
    };
    // Count the borrowed typed result before any serde_json::Value expansion.
    // Oversized results therefore stop at the byte quota, even with a tiny
    // response budget and a large retained result.
    check_json_size(&view, limit)?;
    serde_json::to_value(&view).map_err(|e| error("serialization", e.to_string()))
}
fn check_json_size<T: Serialize + ?Sized>(value: &T, limit: usize) -> Result<()> {
    struct Counter {
        remaining: usize,
    }
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.remaining = self.remaining.checked_sub(bytes.len()).ok_or_else(|| std::io::Error::other("Response quota"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Counter { remaining: limit }, value).map_err(|_| error("output_limit", "Response exceeds server byte quota"))
}
fn error(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
struct Shared {
    config: ServerConfig,
    jobs: Mutex<BTreeMap<String, ServiceJob>>,
    connections: AtomicUsize,
    stopping: AtomicBool,
}
pub struct Server {
    listener: TcpListener,
    shared: Arc<Shared>,
}
impl Server {
    pub fn bind(config: ServerConfig) -> Result<Self> {
        let address = config.validate()?;
        let listener = TcpListener::bind(address).map_err(|e| error("server_bind", e.to_string()))?;
        listener.set_nonblocking(true).map_err(|e| error("server_bind", e.to_string()))?;
        Ok(Self {
            listener,
            shared: Arc::new(Shared {
                config,
                jobs: Mutex::new(BTreeMap::new()),
                connections: AtomicUsize::new(0),
                stopping: AtomicBool::new(false),
            }),
        })
    }
    pub fn address(&self) -> Result<SocketAddr> {
        self.listener.local_addr().map_err(|e| error("server_address", e.to_string()))
    }
    /// The stop flag belongs to the embedding host. Shutdown cancels retained jobs.
    pub fn run(&self, stop: &AtomicBool) -> Result<()> {
        while !stop.load(Ordering::Acquire) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if self
                        .shared
                        .connections
                        .fetch_update(
                            Ordering::AcqRel,
                            Ordering::Acquire,
                            |n| if n < self.shared.config.max_connections { Some(n + 1) } else { None },
                        )
                        .is_err()
                    {
                        drop(stream);
                        continue;
                    }
                    let shared = Arc::clone(&self.shared);
                    if let Err(e) = std::thread::Builder::new().name("solvercraft-client".into()).spawn(move || {
                        struct Connection(Arc<Shared>);
                        impl Drop for Connection {
                            fn drop(&mut self) {
                                self.0.connections.fetch_sub(1, Ordering::AcqRel);
                            }
                        }
                        let _slot = Connection(Arc::clone(&shared));
                        let _result = serve_client(stream, &shared);
                    }) {
                        self.shared.connections.fetch_sub(1, Ordering::AcqRel);
                        return Err(error("server_thread", e.to_string()));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(2)),
                Err(e) => return Err(error("server_accept", e.to_string())),
            }
        }
        self.shared.stopping.store(true, Ordering::Release);
        let jobs = self.shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
        for job in jobs.values() {
            job.cancel();
        }
        Ok(())
    }
}
fn same_secret(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes().zip(right.bytes()).fold(0u8, |difference, (a, b)| difference | (a ^ b)) == 0
}
fn dispatch(shared: &Arc<Shared>, authenticated: &mut bool, command: ServiceCommand) -> Result<Value> {
    if shared.stopping.load(Ordering::Acquire) {
        return Err(error("server_stopping", "Server is shutting down"));
    }
    if let ServiceCommand::Authenticate { token } = command {
        *authenticated = same_secret(&token, &shared.config.token);
        return if *authenticated { Ok(json!({"authenticated":true})) } else { Err(error("authentication", "Authentication failed")) };
    }
    if !*authenticated {
        return Err(error("authentication", "Authenticate this connection first"));
    }
    match command {
        ServiceCommand::Authenticate { .. } => Err(error("authentication", "Invalid authentication state")),
        ServiceCommand::Capabilities {} => Ok(crate::capabilities()),
        ServiceCommand::Solve { model, options, mut limits } => {
            if options.max_memory_bytes > shared.config.max_job_memory_bytes {
                return Err(error("quota", "Requested memory exceeds per-job quota"));
            }
            limits.max_time_millis = Some(limits.max_time_millis.unwrap_or(shared.config.max_job_time_millis).min(shared.config.max_job_time_millis));
            crate::model::validate(&model, &options)?;
            let manifest = run_manifest::prepare(
                "server.solve",
                &SolveIdentity { model: &model, options: &options, limits },
                ServiceConfiguration::Solve { options: options.clone(), limits },
                shared.config.max_request_bytes as u64,
            )?;
            let mut store = shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
            // Admission and the shutdown cancellation sweep use the same lock.
            // A request already being decoded must not start work after shutdown.
            if shared.stopping.load(Ordering::Acquire) {
                return Err(error("server_stopping", "Server is shutting down"));
            }
            if store.len() >= shared.config.max_jobs {
                return Err(error("quota", "Retained job quota reached; forget a completed job"));
            }
            let handle = jobs::start_shared_job(move |c| runtime::solve_with_limits_owned(&model, &options, limits, c))?;
            let job = ServiceJob::Solve(handle, manifest);
            let id = job.id().to_string();
            store.insert(id.clone(), job);
            Ok(json!({"job_id":id}))
        }
        ServiceCommand::Simulate { inputs, outputs, options, mut limits } => {
            if options.max_memory_bytes > shared.config.max_job_memory_bytes || inputs.len() > 4096 || outputs.len() > 4096 {
                return Err(error("quota", "Simulation exceeds memory or dimension quota"));
            }
            let dimensions = if outputs.is_empty() { inputs.len() } else { outputs.len() };
            if options.retain_samples
                && options
                    .trials
                    .checked_mul(dimensions as u64)
                    .and_then(|n| n.checked_mul(32))
                    // Eight fixed summary fields and five quantile pairs, with
                    // 32 bytes per JSON numeric token plus keys/punctuation.
                    .and_then(|n| (dimensions as u64).checked_mul(768).and_then(|summaries| n.checked_add(summaries)))
                    .and_then(|n| n.checked_add(65536))
                    .is_none_or(|n| n > shared.config.max_response_bytes as u64)
            {
                return Err(error("quota", "Retained simulation would exceed the response quota; use streaming summaries or fewer samples"));
            }
            for input in &inputs {
                crate::math::validate_distribution(input)?;
            }
            crate::compile::validate_expressions(
                &outputs,
                inputs.len(),
                &SolveOptions { max_memory_bytes: options.max_memory_bytes, ..SolveOptions::default() },
            )?;
            limits.max_time_millis = Some(limits.max_time_millis.unwrap_or(shared.config.max_job_time_millis).min(shared.config.max_job_time_millis));
            let manifest = run_manifest::prepare(
                "server.simulate",
                &SimulationIdentity { inputs: &inputs, outputs: &outputs, options: &options, limits },
                ServiceConfiguration::Simulate { options: options.clone(), limits },
                shared.config.max_request_bytes as u64,
            )?;
            let mut store = shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
            if shared.stopping.load(Ordering::Acquire) {
                return Err(error("server_stopping", "Server is shutting down"));
            }
            if store.len() >= shared.config.max_jobs {
                return Err(error("quota", "Retained job quota reached; forget a completed job"));
            }
            let handle = jobs::start_job(move |c| {
                let deadline = runtime::DeadlineControl::new(c, limits)?;
                let mut result = crate::simulation::simulate(&inputs, &options, &deadline, |values| {
                    if outputs.is_empty() {
                        return Ok(values.to_vec());
                    }
                    outputs.iter().map(|e| crate::compile::evaluate(e, values)).collect()
                })?;
                deadline.apply_simulation_termination(&mut result);
                Ok(result)
            })?;
            let job = ServiceJob::Simulation(handle, manifest);
            let id = job.id().to_string();
            store.insert(id.clone(), job);
            Ok(json!({"job_id":id}))
        }
        ServiceCommand::Get { job_id, include_manifest } => {
            let store = shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
            store.get(&job_id).ok_or_else(|| error("unknown_job", "Unknown job ID"))?.json(include_manifest, shared.config.max_response_bytes)
        }
        ServiceCommand::Cancel { job_id } => {
            let store = shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
            store.get(&job_id).ok_or_else(|| error("unknown_job", "Unknown job ID"))?.cancel();
            Ok(json!({"cancel_requested":true}))
        }
        ServiceCommand::Forget { job_id } => {
            let mut store = shared.jobs.lock().map_err(|_| error("server_state", "Job registry lock poisoned"))?;
            if !store.get(&job_id).ok_or_else(|| error("unknown_job", "Unknown job ID"))?.done()? {
                return Err(error("job_running", "Cancel and wait before forgetting a running job"));
            }
            store.remove(&job_id);
            Ok(json!({"forgotten":true}))
        }
    }
}
fn serve_client(mut stream: TcpStream, shared: &Arc<Shared>) -> Result<()> {
    // Windows accepts inherit the listener's nonblocking mode; each bounded
    // connection worker deliberately uses blocking reads with a timeout.
    stream.set_nonblocking(false).map_err(|e| error("server_io", e.to_string()))?;
    let timeout = Some(Duration::from_millis(shared.config.read_timeout_millis));
    stream.set_read_timeout(timeout).map_err(|e| error("server_io", e.to_string()))?;
    stream.set_write_timeout(timeout).map_err(|e| error("server_io", e.to_string()))?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| error("server_io", e.to_string()))?);
    let mut authenticated = false;
    while !shared.stopping.load(Ordering::Acquire) {
        let mut bytes = vec![];
        let count = (&mut reader)
            .take((shared.config.max_request_bytes + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| error("server_io", e.to_string()))?;
        if shared.stopping.load(Ordering::Acquire) {
            return Ok(());
        }
        if count == 0 {
            return Ok(());
        }
        if bytes.len() > shared.config.max_request_bytes {
            return Err(error("input_limit", "Request exceeds server byte quota"));
        }
        let (id, result) = match serde_json::from_slice::<ServiceRequest>(&bytes) {
            Ok(request) if request.schema_version == 1 && !request.id.is_empty() && request.id.len() <= 128 => {
                (request.id, dispatch(shared, &mut authenticated, request.command))
            }
            Ok(_) => (String::new(), Err(error("invalid_request", "Unsupported schema or request ID"))),
            Err(e) => (String::new(), Err(error("invalid_json", e.to_string()))),
        };
        let response = match result {
            Ok(value) => json!({"schema_version":1,"id":id,"ok":true,"value":value}),
            Err(mut e) => {
                e.message = e.message.chars().take(1024).collect();
                json!({"schema_version":1,"id":id,"ok":false,"error":e})
            }
        };
        let bytes = bounded_json(&response, shared.config.max_response_bytes).or_else(|_| {
            bounded_json(
                &json!({"schema_version":1,"id":id,"ok":false,"error":{"code":"output_limit","message":"Response exceeds server byte quota"}}),
                shared.config.max_response_bytes,
            )
        })?;
        stream.write_all(&bytes).map_err(|e| error("server_output", e.to_string()))?;
        stream.write_all(b"\n").map_err(|e| error("server_output", e.to_string()))?;
    }
    Ok(())
}
fn bounded_json(value: &Value, limit: usize) -> Result<Vec<u8>> {
    struct Output {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().checked_add(bytes.len()).is_none_or(|n| n > self.limit) {
                return Err(std::io::Error::other("Response quota"));
            }
            self.bytes.try_reserve(bytes.len()).map_err(std::io::Error::other)?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Output { bytes: vec![], limit };
    serde_json::to_writer(&mut output, value).map_err(|e| error("output_limit", e.to_string()))?;
    Ok(output.bytes)
}

#[cfg(test)]
mod resource_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use serde::ser::SerializeSeq;

    #[derive(Clone)]
    struct ObservedRows(Arc<AtomicUsize>);
    impl Serialize for ObservedRows {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
            let mut sequence = serializer.serialize_seq(Some(1_000_000))?;
            for value in 0..1_000_000 {
                self.0.fetch_add(1, Ordering::Relaxed);
                sequence.serialize_element(&value)?;
            }
            sequence.end()
        }
    }

    #[test]
    fn oversized_job_stops_serializing_before_materializing_the_result() {
        let visited = Arc::new(AtomicUsize::new(0));
        let rows = ObservedRows(Arc::clone(&visited));
        let handle = jobs::start_job(move |_| Ok(rows)).unwrap();
        let started = runtime::monotonic_now();
        while handle.snapshot().unwrap().result.is_none() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        }
        let manifest = run_manifest::prepare(
            "server.test",
            &0,
            ServiceConfiguration::Solve { options: SolveOptions::default(), limits: RuntimeLimits::default() },
            1024,
        )
        .unwrap();
        let error = job_json(&handle, &manifest, false, 1024, |_| "test-observed-serializer".into()).unwrap_err();
        assert_eq!(error.code, "output_limit");
        let count = visited.load(Ordering::Relaxed);
        assert!(count > 0 && count < 400, "Serialized {count} rows before refusing a 1024-byte response");
    }
}
