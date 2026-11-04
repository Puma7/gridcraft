//! On-demand, workbook-independent MCP over standard input/output.
//! No listener, daemon, filesystem tool or spreadsheet session is created.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo)]

mod jobs;
mod tools;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

/// Process-local quotas. Defaults admit the existing SDK example requests.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_retained_jobs: usize,
    pub max_job_memory_bytes: u64,
    pub max_reserved_memory_bytes: u64,
    pub max_job_time_millis: u64,
    pub shutdown_grace_millis: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_request_bytes: 2 * 1024 * 1024,
            max_response_bytes: 4 * 1024 * 1024,
            max_retained_jobs: 16,
            max_job_memory_bytes: 256 * 1024 * 1024,
            max_reserved_memory_bytes: 512 * 1024 * 1024,
            max_job_time_millis: 60_000,
            shutdown_grace_millis: 5000,
        }
    }
}
impl Config {
    pub fn validate(&self) -> solvercraft::types::Result<()> {
        if !(1024..=16 * 1024 * 1024).contains(&self.max_request_bytes)
            || !(64 * 1024..=16 * 1024 * 1024).contains(&self.max_response_bytes)
            || !(1..=64).contains(&self.max_retained_jobs)
            || !(1024 * 1024..=1024 * 1024 * 1024).contains(&self.max_job_memory_bytes)
            || !(self.max_job_memory_bytes..=2 * 1024 * 1024 * 1024).contains(&self.max_reserved_memory_bytes)
            || !(1..=3_600_000).contains(&self.max_job_time_millis)
            || !(1..=30_000).contains(&self.shutdown_grace_millis)
        {
            return Err(failure("invalid_mcp_config", "MCP resource limits are outside supported ranges"));
        }
        Ok(())
    }
}

pub(crate) fn failure(code: &str, message: impl Into<String>) -> solvercraft::types::SolverError {
    solvercraft::types::SolverError::new(code, message)
}

pub(crate) fn bounded_json<T: Serialize + ?Sized>(value: &T, limit: usize) -> solvercraft::types::Result<Vec<u8>> {
    struct Buffer {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| std::io::Error::other("JSON size overflow"))?;
            if next > self.limit {
                return Err(std::io::Error::other("JSON response quota"));
            }
            self.bytes.try_reserve(bytes.len()).map_err(std::io::Error::other)?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer { bytes: Vec::new(), limit };
    serde_json::to_writer(&mut buffer, value).map_err(|_| failure("output_limit", "Response exceeds the MCP byte quota"))?;
    Ok(buffer.bytes)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    params: Value,
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn rpc_success(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn valid_id(value: &Value) -> bool {
    value.as_str().is_some_and(|s| s.len() <= 256) || value.as_i64().is_some() || value.as_u64().is_some()
}

/// An MCP process owns its own retained jobs. Jobs share the SDK's global four-worker
/// quota with any other SDK caller in the same process.
pub struct Server {
    config: Config,
    initialized: bool,
    ready: bool,
    jobs: jobs::Jobs,
}
impl Server {
    pub fn new(config: Config) -> solvercraft::types::Result<Self> {
        config.validate()?;
        Ok(Self { jobs: jobs::Jobs::new(config.clone()), config, initialized: false, ready: false })
    }

    /// Handles one bounded UTF-8 JSON-RPC message. Notifications never cause tool
    /// side effects and never receive responses.
    pub fn handle_line(&mut self, line: &[u8]) -> Option<Value> {
        if line.len() > self.config.max_request_bytes {
            return Some(rpc_error(Value::Null, -32600, "MCP request exceeds its byte quota"));
        }
        let raw: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(_) => return Some(rpc_error(Value::Null, -32700, "Invalid JSON")),
        };
        let Some(object) = raw.as_object() else {
            return Some(rpc_error(Value::Null, -32600, "Expected one JSON-RPC request object"));
        };
        let response_id = object.get("id").filter(|id| valid_id(id)).cloned().unwrap_or(Value::Null);
        let has_id = object.contains_key("id");
        let request: RpcRequest = match serde_json::from_value(raw) {
            Ok(request) => request,
            Err(_) => return Some(rpc_error(response_id, -32600, "Invalid JSON-RPC request fields")),
        };
        if request.jsonrpc != "2.0"
            || request.method.is_empty()
            || request.method.len() > 128
            || has_id && request.id.as_ref().is_none_or(|id| !valid_id(id))
            || !request.params.is_null() && !request.params.is_object()
        {
            return Some(rpc_error(response_id, -32600, "Invalid JSON-RPC request"));
        }
        if !has_id {
            if request.method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        let id = request.id.unwrap_or(Value::Null);
        let params = if request.params.is_null() { json!({}) } else { request.params };
        let result = match request.method.as_str() {
            "initialize" => self.initialize(&params),
            "ping" => Ok(json!({})),
            "tools/list" | "tools/call" | "resources/list" | "resources/templates/list" | "prompts/list" if !self.ready => {
                Err((-32002, "Initialize the MCP session before using tools".to_string()))
            }
            "tools/list" => {
                if !list_parameters(&params) {
                    Err((-32602, "tools/list accepts optional _meta and no pagination cursor".to_string()))
                } else {
                    Ok(json!({"tools":tools::definitions()}))
                }
            }
            "tools/call" => self.call_tool(params),
            "resources/list" => Ok(json!({"resources":[]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates":[]})),
            "prompts/list" => Ok(json!({"prompts":[]})),
            _ => Err((-32601, "Unknown method".to_string())),
        };
        Some(match result {
            Ok(result) => rpc_success(id, result),
            Err((code, message)) => rpc_error(id, code, &message),
        })
    }

    fn initialize(&mut self, params: &Value) -> Result<Value, (i32, String)> {
        if self.initialized {
            return Err((-32600, "The MCP session is already initialized".to_string()));
        }
        let Some(version) = params.get("protocolVersion").and_then(Value::as_str) else {
            return Err((-32602, "initialize requires protocolVersion".to_string()));
        };
        if params.as_object().is_none_or(|p| p.keys().any(|k| !["protocolVersion", "capabilities", "clientInfo", "_meta"].contains(&k.as_str())))
            || params.get("capabilities").is_some_and(|v| !v.is_object())
            || params.get("clientInfo").is_some_and(|v| !v.is_object())
        {
            return Err((-32602, "Invalid initialize parameters".to_string()));
        }
        let version = if SUPPORTED_VERSIONS.contains(&version) { version } else { PROTOCOL_VERSION };
        self.initialized = true;
        Ok(json!({
            "protocolVersion":version,
            "capabilities":{"tools":{"listChanged":false}},
            "serverInfo":{"name":"solvercraft","version":solvercraft::VERSION},
            "instructions":"Standalone local Solvercraft. Start bounded jobs, poll job_get, and forget terminal jobs. Preserve termination, feasibility and optimality qualifications. No spreadsheet or network service is started."
        }))
    }

    fn call_tool(&mut self, params: Value) -> Result<Value, (i32, String)> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Call {
            name: String,
            #[serde(default = "empty")]
            arguments: Value,
            #[serde(rename = "_meta")]
            _meta: Option<Value>,
        }
        fn empty() -> Value {
            json!({})
        }
        let call: Call = serde_json::from_value(params).map_err(|_| (-32602, "Invalid tools/call parameters".to_string()))?;
        if call._meta.as_ref().is_some_and(|value| !value.is_object()) {
            return Err((-32602, "MCP _meta must be an object".to_string()));
        }
        if !call.arguments.is_object() {
            return Err((-32602, "Tool arguments must be an object".to_string()));
        }
        if !tools::known(&call.name) {
            return Err((-32602, "Unknown tool".to_string()));
        }
        let result = if call.name == "solvercraft_capabilities" {
            if !empty_object(&call.arguments) {
                return Err((-32602, "Capabilities accepts no arguments".to_string()));
            }
            Ok(json!({"sdk":solvercraft::capabilities(),"transport":"stdio","protocol_versions":SUPPORTED_VERSIONS,"limits":self.config,
                "job_storage":"process-local until forget or process exit","network_listener":false,"workbook_dependency":false}))
        } else {
            self.jobs.call(&call.name, call.arguments)
        };
        let (data, is_error) = match result {
            Ok(data) => (data, false),
            Err(error) if error.code == "invalid_arguments" => return Err((-32602, error.message)),
            Err(error) => (json!({"error":error}), true),
        };
        let text = String::from_utf8(
            bounded_json(&data, self.config.max_response_bytes / 4).map_err(|_| (-32001, "MCP tool response exceeds its byte quota".to_string()))?,
        )
        .map_err(|_| (-32603, "Could not encode tool response".to_string()))?;
        Ok(json!({"content":[{"type":"text","text":text}],"isError":is_error}))
    }

    /// Serve newline-delimited JSON. Request allocation is capped even when a
    /// caller sends a line larger than its quota. EOF cancels all process jobs.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut input: R, mut output: W) -> std::io::Result<()> {
        let mut line = Vec::new();
        loop {
            let mut oversized = false;
            loop {
                let buffer = input.fill_buf()?;
                if buffer.is_empty() {
                    break;
                }
                let take = buffer.iter().position(|b| *b == b'\n').map_or(buffer.len(), |n| n + 1);
                if !oversized {
                    if line.len().checked_add(take).is_none_or(|n| n > self.config.max_request_bytes) {
                        oversized = true;
                        line.clear();
                    } else {
                        line.try_reserve(take).map_err(std::io::Error::other)?;
                        line.extend_from_slice(&buffer[..take]);
                    }
                }
                let ended = buffer.get(take.saturating_sub(1)) == Some(&b'\n');
                input.consume(take);
                if ended {
                    break;
                }
            }
            if line.is_empty() && !oversized {
                break;
            }
            let response =
                if oversized { Some(rpc_error(Value::Null, -32600, "MCP request exceeds its byte quota")) } else { self.handle_line(&line) };
            if let Some(response) = response {
                let bytes = match bounded_json(&response, self.config.max_response_bytes) {
                    Ok(bytes) => bytes,
                    Err(_) => bounded_json(
                        &rpc_error(response.get("id").cloned().unwrap_or(Value::Null), -32001, "MCP response exceeds its byte quota"),
                        self.config.max_response_bytes,
                    )
                    .map_err(std::io::Error::other)?,
                };
                output.write_all(&bytes)?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
            line.clear();
        }
        self.shutdown().map_err(std::io::Error::other)
    }

    pub fn shutdown(&mut self) -> solvercraft::types::Result<()> {
        self.jobs.shutdown()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.jobs.cancel_all();
    }
}
fn empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|v| v.is_empty())
}
fn list_parameters(value: &Value) -> bool {
    value.as_object().is_some_and(|params| {
        params.keys().all(|key| ["_meta", "cursor"].contains(&key.as_str()))
            && params.get("_meta").is_none_or(Value::is_object)
            && params.get("cursor").is_none_or(Value::is_null)
    })
}
