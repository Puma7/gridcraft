//! Actual stdio process contracts. Numerical witnesses are independent of tool schemas.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

struct Client {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<String>,
    id: u64,
}
impl Client {
    fn spawn() -> Self {
        let mut child =
            Command::new(env!("CARGO_BIN_EXE_solvercraft-mcp")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        Self { child, input, output, id: 0 }
    }
    fn send(&mut self, value: &Value) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
    }
    fn read(&self) -> Value {
        serde_json::from_str(&self.output.recv_timeout(Duration::from_secs(10)).expect("MCP response timeout")).unwrap()
    }
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}));
        let answer = self.read();
        assert_eq!(answer["id"], self.id);
        answer
    }
    fn initialize(&mut self) {
        let initialized =
            self.rpc("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"process-test","version":"1"}}));
        assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
        self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
    fn tool_raw(&mut self, name: &str, arguments: Value) -> Value {
        self.rpc("tools/call", json!({"name":name,"arguments":arguments}))
    }
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let answer = self.tool_raw(name, arguments);
        assert!(answer.get("error").is_none(), "{answer}");
        assert_eq!(answer["result"]["isError"], false, "{answer}");
        serde_json::from_str(answer["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    fn terminal(&mut self, id: &Value) -> Value {
        let deadline = solvercraft::runtime::monotonic_now() + Duration::from_secs(10);
        loop {
            let answer = self.tool("solvercraft_job_get", json!({"job_id":id}));
            if matches!(answer["status"].as_str(), Some("completed" | "failed")) {
                return answer;
            }
            assert!(solvercraft::runtime::monotonic_now() < deadline, "job timeout");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn close(&mut self) {
        self.input.take();
        let deadline = solvercraft::runtime::monotonic_now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "MCP process failed at EOF");
                return;
            }
            assert!(solvercraft::runtime::monotonic_now() < deadline, "MCP process did not stop at EOF");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn lp() -> Value {
    json!({
        "schema_version":1,"name":"independent Wyndor witness",
        "variables":[{"name":"a","lower":0.0,"upper":4.0,"kind":"continuous"},{"name":"b","lower":0.0,"upper":6.0,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0.0,"linear":[{"variable":0,"coefficient":3.0},{"variable":1,"coefficient":5.0}],"quadratic":[]},
        "constraints":[{"name":"capacity","terms":[{"variable":0,"coefficient":3.0},{"variable":1,"coefficient":2.0}],"relation":"less_equal","rhs":18.0}]
    })
}

#[test]
fn actual_process_solves_and_retains_actual_engine_manifest() {
    let mut client = Client::spawn();
    client.initialize();
    let listed = client.rpc("tools/list", json!({}));
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 7);
    let caps = client.tool("solvercraft_capabilities", json!({}));
    assert_eq!(caps["network_listener"], false);
    assert_eq!(caps["workbook_dependency"], false);
    let started = client.tool("solvercraft_start_solve", json!({"model":lp(),"options":{"seed":"18446744073709551615"}}));
    assert!(started["job_id"].is_string());
    let terminal = client.terminal(&started["job_id"]);
    assert_eq!(terminal["status"], "completed");
    assert_eq!(terminal["result"]["ok"], true);
    let result = &terminal["result"]["value"];
    assert_eq!(result["values"], json!([2.0, 6.0]));
    assert_eq!(result["objective"], 36.0);
    assert_eq!(result["feasibility"], "feasible");
    assert_eq!(result["optimality"], "optimal");
    assert_eq!(terminal["manifest"]["actual_engine"], result["engine"]);
    assert_eq!(terminal["manifest"]["effective_configuration"]["options"]["seed"], "18446744073709551615");
    client.tool("solvercraft_job_forget", json!({"job_id":started["job_id"]}));
    let missing = client.tool_raw("solvercraft_job_get", json!({"job_id":started["job_id"]}));
    assert_eq!(missing["result"]["isError"], true);
    client.close();
}

#[test]
fn actual_process_simulates_expression_and_constant_distribution_exactly() {
    let mut client = Client::spawn();
    client.initialize();
    let started = client.tool(
        "solvercraft_start_simulation",
        json!({
            "inputs":[{"kind":"constant","value":7.0}],
            "outputs":[{"op":"multiply","left":{"op":"variable","index":0},"right":{"op":"constant","value":3.0}}],
            "options":{"trials":1000,"seed":"42","retain_samples":true}
        }),
    );
    let terminal = client.terminal(&started["job_id"]);
    let result = &terminal["result"]["value"];
    assert_eq!(result["completed_trials"], 1000);
    assert_eq!(result["summaries"][0]["mean"], 21.0);
    assert_eq!(result["summaries"][0]["variance"], 0.0);
    assert_eq!(result["samples"].as_array().unwrap().len(), 1000);
    assert!(result["samples"].as_array().unwrap().iter().all(|row| row == &json!([21.0])));
    let poll = client.tool("solvercraft_job_get", json!({"job_id":started["job_id"],"include_result":false}));
    assert!(poll.get("result").is_none());
    assert_eq!(poll["result_available"], true);
    client.close();
}

#[test]
fn actual_agent_process_does_not_certify_a_cancelled_reduced_cost_as_optimal() {
    let mut client = Client::spawn();
    client.initialize();
    // x<=y<=z on [0,1]: 1e16*(x-z)+y <= y <= 1, attained at (1,1,1).
    let problem = json!({"schema_version":1,"name":"independent cancellation witness",
        "variables":[{"name":"x","lower":0,"upper":1,"kind":"continuous"},
                     {"name":"y","lower":0,"upper":1,"kind":"continuous"},
                     {"name":"z","lower":0,"upper":1,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1e16},
            {"variable":1,"coefficient":1},{"variable":2,"coefficient":-1e16}],"quadratic":[]},
        "constraints":[{"name":"xy","terms":[{"variable":0,"coefficient":1},{"variable":1,"coefficient":-1}],"relation":"less_equal","rhs":0},
            {"name":"yz","terms":[{"variable":1,"coefficient":1},{"variable":2,"coefficient":-1}],"relation":"less_equal","rhs":0}]});
    let started = client.tool("solvercraft_start_solve", json!({"model":problem}));
    let terminal = client.terminal(&started["job_id"]);
    assert_eq!(terminal["result"]["ok"], true, "{terminal}");
    let result = &terminal["result"]["value"];
    assert_eq!(result["optimality"], "optimal", "{result}");
    assert_eq!(result["values"], json!([1.0, 1.0, 1.0]));
    assert_eq!(result["objective"], 1.0);
    assert_eq!(result["best_bound"], 1.0);
    client.close();
}

#[test]
fn actual_agent_process_reports_unrepresentable_variance_without_false_certainty() {
    let mut client = Client::spawn();
    client.initialize();
    let started = client.tool(
        "solvercraft_start_simulation",
        json!({
        "inputs":[{"kind":"uniform","low":-1e-200,"high":1e-200}],
        "options":{"trials":32,"seed":"42","retain_samples":true}}),
    );
    let terminal = client.terminal(&started["job_id"]);
    assert_eq!(terminal["result"]["ok"], false, "{terminal}");
    assert!(terminal.to_string().contains("numerical_range"), "{terminal}");
    client.close();
}

#[test]
fn actual_process_executes_portable_artcraft_model_without_a_workbook() {
    let mut client = Client::spawn();
    client.initialize();
    let started = client.tool("solvercraft_start_execute", json!({"request":{
        "schema_version":2,
        "model":{"schema_version":2,"id":"layout-model","revision":"9007199254740993","name":"portable model","definition":{"kind":"algebraic","model":lp()},"bindings":[{"parameter":0,"object_id":"artcraft-object-a","property":"width"}]},
        "context":{"run_id":"run-42","snapshot_id":"artcraft-snapshot","snapshot_revision":"9007199254740995","host":"artcraft","source_revision":null},
        "goal":{"kind":"maximize"},"limits":{"max_time_millis":1000}
    }}));
    let terminal = client.terminal(&started["job_id"]);
    let run = &terminal["result"]["value"];
    assert_eq!(run["original_objective"], 36.0);
    assert_eq!(run["result"]["values"], json!([2.0, 6.0]));
    assert_eq!(run["manifest"]["model_revision"], "9007199254740993");
    assert_eq!(run["manifest"]["context"]["snapshot_revision"], "9007199254740995");
    client.close();
}

#[test]
fn portable_nonlinear_execution_reports_stationarity_instead_of_global_optimality() {
    let mut client = Client::spawn();
    client.initialize();
    let started = client.tool("solvercraft_start_execute", json!({"request":{
        "schema_version":2,
        "model":{"schema_version":2,"id":"layout-expression","revision":"1","name":"width target","options":{"method":"projected_gradient"},
            "definition":{"kind":"expression","model":{"name":"width target","variables":[{"name":"width","lower":0,"upper":10,"kind":"continuous"}],
                "direction":"minimize","objective":{"op":"power","base":{"op":"subtract","left":{"op":"variable","index":0},"right":{"op":"constant","value":3}},"exponent":2},"constraints":[]}}},
        "context":{"run_id":"nonlinear-run","snapshot_id":"design-snapshot","snapshot_revision":"1","host":"artcraft-layout","source_revision":null}
    }}));
    let terminal = client.terminal(&started["job_id"]);
    let result = &terminal["result"]["value"]["result"];
    assert!((result["values"][0].as_f64().unwrap() - 3.0).abs() < 1e-5, "{terminal}");
    assert!(result["objective"].as_f64().unwrap() < 1e-8);
    assert_eq!(result["optimality"], "stationary");
    assert_eq!(result["feasibility"], "feasible");
    client.close();
}

#[test]
fn process_errors_preserve_identity_and_do_not_echo_protocol_garbage() {
    let mut client = Client::spawn();
    let before_init = client.rpc("tools/list", json!({}));
    assert_eq!(before_init["error"]["code"], -32002);
    client.initialize();
    assert_eq!(client.rpc("unknown.method", json!({}))["error"]["code"], -32601);
    assert_eq!(client.tool_raw("unknown_tool", json!({}))["error"]["code"], -32602);
    assert_eq!(client.tool_raw("solvercraft_start_solve", json!({"model":lp(),"not_an_option":1}))["error"]["code"], -32602);
    let invalid = client.tool_raw("solvercraft_start_solve", json!({"model":lp(),"options":{"seed":42}}));
    assert_eq!(invalid["error"]["code"], -32602);
    client.send(&json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"solvercraft_start_solve","arguments":{"model":lp()}}}));
    assert_eq!(client.rpc("ping", json!({}))["result"], json!({}));
    client.close();
}

#[test]
fn process_cancellation_and_zero_deadlines_are_real_and_eof_stops_workers() {
    let mut client = Client::spawn();
    client.initialize();
    let large = json!({"inputs":[{"kind":"normal","mean":0.0,"std_dev":1.0}],"options":{"trials":10000000,"retain_samples":false}});
    let started = client.tool("solvercraft_start_simulation", large.clone());
    let cannot_forget = client.tool_raw("solvercraft_job_forget", json!({"job_id":started["job_id"]}));
    assert_eq!(cannot_forget["result"]["isError"], true);
    client.tool("solvercraft_job_cancel", json!({"job_id":started["job_id"]}));
    let cancelled = client.terminal(&started["job_id"]);
    assert_eq!(cancelled["result"]["value"]["termination"], "cancelled");
    client.tool("solvercraft_job_forget", json!({"job_id":started["job_id"]}));
    let zero = client.tool(
        "solvercraft_start_simulation",
        json!({"inputs":[{"kind":"constant","value":1}],"options":{"trials":1000},"limits":{"max_time_millis":0}}),
    );
    let stopped = client.terminal(&zero["job_id"]);
    assert_eq!(stopped["result"]["value"]["termination"], "time_limit");
    assert_eq!(stopped["result"]["value"]["completed_trials"], 0);
    client.tool("solvercraft_job_forget", json!({"job_id":zero["job_id"]}));
    client.tool("solvercraft_start_simulation", large);
    client.close();
}

#[test]
fn process_rejects_host_memory_and_output_quotas_before_starting_jobs() {
    let mut client = Client::spawn();
    client.initialize();
    let memory = client.tool_raw("solvercraft_start_solve", json!({"model":lp(),"options":{"max_memory_bytes":1073741824}}));
    assert_eq!(memory["result"]["isError"], true);
    let samples = client.tool_raw(
        "solvercraft_start_simulation",
        json!({"inputs":[{"kind":"normal","mean":0,"std_dev":1}],"options":{"trials":1000000,"retain_samples":true}}),
    );
    assert_eq!(samples["result"]["isError"], true);
    let invalid = client.tool_raw("solvercraft_start_simulation", json!({"inputs":[{"kind":"normal","mean":0,"std_dev":-1}]}));
    assert_eq!(invalid["result"]["isError"], true);
    let started = client.tool("solvercraft_start_solve", json!({"model":lp()}));
    assert_eq!(client.terminal(&started["job_id"])["result"]["value"]["objective"], 36.0);
    client.close();
}
