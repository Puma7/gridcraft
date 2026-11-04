#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use solvercraft_mcp::{Config, Server};
use std::io::Cursor;

fn request(server: &mut Server, id: Value, method: &str, params: Value) -> Value {
    server.handle_line(serde_json::to_string(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})).unwrap().as_bytes()).unwrap()
}
fn initialize(server: &mut Server) {
    assert!(request(server, json!(1), "initialize", json!({"protocolVersion":"2025-06-18"})).get("result").is_some());
    assert!(server.handle_line(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
}

#[test]
fn malformed_batches_ids_and_unknown_fields_do_not_create_work() {
    let mut server = Server::new(Config::default()).unwrap();
    for malformed in [b"{".as_slice(), b"\xff", b"[]", b"[{}]", b"null", b"42"] {
        assert!(server.handle_line(malformed).unwrap().get("error").is_some());
    }
    for id in [json!(null), json!([]), json!({}), json!(1.5), json!("x".repeat(257))] {
        assert_eq!(request(&mut server, id, "ping", json!({}))["error"]["code"], -32600);
    }
    assert_eq!(server.handle_line(br#"{"jsonrpc":"2.0","id":7,"method":"ping","unexpected":42}"#).unwrap()["error"]["code"], -32600);
    assert_eq!(request(&mut server, json!(18446744073709551615_u64), "ping", json!({}))["id"], json!(18446744073709551615_u64));
    assert_eq!(request(&mut server, json!("9007199254740993"), "ping", json!({}))["id"], "9007199254740993");
}

#[test]
fn negotiation_and_initialization_are_explicit() {
    let mut server = Server::new(Config::default()).unwrap();
    assert_eq!(request(&mut server, json!(1), "tools/list", json!({}))["error"]["code"], -32002);
    assert_eq!(
        request(&mut server, json!(2), "initialize", json!({"protocolVersion":"future-unsupported"}))["result"]["protocolVersion"],
        "2025-06-18"
    );
    assert_eq!(request(&mut server, json!(3), "tools/list", json!({}))["error"]["code"], -32002);
    server.handle_line(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert_eq!(request(&mut server, json!(4), "tools/list", json!({}))["result"]["tools"].as_array().unwrap().len(), 7);
    assert_eq!(
        request(&mut server, json!(40), "tools/list", json!({"_meta":{"progressToken":"list-40"},"cursor":null}))["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(request(&mut server, json!(41), "tools/list", json!({"cursor":"unissued-cursor"}))["error"]["code"], -32602);
    assert_eq!(request(&mut server, json!(5), "initialize", json!({"protocolVersion":"2025-06-18"}))["error"]["code"], -32600);
}

#[test]
fn notifications_do_not_start_jobs_and_invalid_tool_arguments_are_protocol_errors() {
    let mut server = Server::new(Config::default()).unwrap();
    initialize(&mut server);
    assert!(server.handle_line(br#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"solvercraft_start_solve","arguments":{}}}"#).is_none());
    for params in [
        json!({"name":"solvercraft_capabilities","arguments":{"extra":1}}),
        json!({"name":"solvercraft_job_get","arguments":{"job_id":1}}),
        json!({"name":"solvercraft_job_get","arguments":{"job_id":"1","extra":true}}),
        json!({"name":"solvercraft_capabilities","arguments":[]}),
        json!({"name":"solvercraft_capabilities","arbitrary":{}}),
    ] {
        assert_eq!(request(&mut server, json!(9), "tools/call", params)["error"]["code"], -32602);
    }
}

#[test]
fn oversized_line_is_drained_without_losing_the_next_request_or_final_partial_line() {
    let mut server = Server::new(Config { max_request_bytes: 1024, ..Config::default() }).unwrap();
    let mut input = vec![b'x'; 32 * 1024];
    input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":17,\"method\":\"ping\"}");
    let mut output = Vec::new();
    server.serve(Cursor::new(input), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    let replies: Vec<Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], -32600);
    assert_eq!(replies[1]["id"], 17);
    assert_eq!(replies[1]["result"], json!({}));
    assert!(text.len() < 1024);
}

#[test]
fn invalid_process_quota_configs_are_refused() {
    for config in [
        Config { max_request_bytes: 0, ..Config::default() },
        Config { max_response_bytes: 65535, ..Config::default() },
        Config { max_retained_jobs: 0, ..Config::default() },
        Config { max_job_memory_bytes: 0, ..Config::default() },
        Config { max_reserved_memory_bytes: 1, ..Config::default() },
        Config { max_job_time_millis: 0, ..Config::default() },
        Config { shutdown_grace_millis: 30001, ..Config::default() },
    ] {
        assert!(Server::new(config).is_err());
    }
}

fn small_model() -> Value {
    json!({"schema_version":1,"name":"one bounded variable","variables":[{"name":"x","lower":0,"upper":1,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1}],"quadratic":[]},"constraints":[]})
}
fn call(server: &mut Server, name: &str, args: Value) -> Value {
    let response = request(server, json!(100), "tools/call", json!({"name":name,"arguments":args}));
    assert!(response.get("error").is_none(), "{response}");
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}
fn wait_done(server: &mut Server, id: &Value) {
    let deadline = solvercraft::runtime::monotonic_now() + std::time::Duration::from_secs(5);
    loop {
        let status = call(server, "solvercraft_job_get", json!({"job_id":id,"include_result":false}));
        if status["result_available"] == true {
            return;
        }
        assert!(solvercraft::runtime::monotonic_now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn retained_jobs_and_memory_reservations_are_released_only_by_forget() {
    let mut server = Server::new(Config { max_retained_jobs: 1, ..Config::default() }).unwrap();
    initialize(&mut server);
    let started = call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}));
    wait_done(&mut server, &started["job_id"]);
    assert_eq!(call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}))["error"]["code"], "quota");
    call(&mut server, "solvercraft_job_forget", json!({"job_id":started["job_id"]}));
    assert!(call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}))["job_id"].is_string());
    server.shutdown().unwrap();

    let mut server = Server::new(Config { max_reserved_memory_bytes: 256 * 1024 * 1024, ..Config::default() }).unwrap();
    initialize(&mut server);
    let started = call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}));
    wait_done(&mut server, &started["job_id"]);
    assert_eq!(call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}))["error"]["code"], "quota");
    call(&mut server, "solvercraft_job_forget", json!({"job_id":started["job_id"]}));
    assert!(call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}))["job_id"].is_string());
    server.shutdown().unwrap();
}

#[test]
fn streaming_summary_response_quota_is_checked_before_job_admission() {
    let mut server = Server::new(Config { max_response_bytes: 65536, max_retained_jobs: 1, ..Config::default() }).unwrap();
    initialize(&mut server);
    let wide = call(
        &mut server,
        "solvercraft_start_simulation",
        json!({"inputs":vec![json!({"kind":"constant","value":1.0});128],"options":{"trials":1,"retain_samples":false}}),
    );
    assert_eq!(wide["error"]["code"], "quota");
    // Rejection reserved no job slot. A scalar streaming result fits this same
    // valid small response budget and remains retrievable after completion.
    let scalar = call(
        &mut server,
        "solvercraft_start_simulation",
        json!({"inputs":[{"kind":"constant","value":1.0}],"options":{"trials":1,"retain_samples":false}}),
    );
    assert!(scalar["job_id"].is_string(), "{scalar}");
    wait_done(&mut server, &scalar["job_id"]);
    let result = call(&mut server, "solvercraft_job_get", json!({"job_id":scalar["job_id"]}));
    assert_eq!(result["result"]["ok"], true);
    assert_eq!(result["result"]["value"]["completed_trials"], 1);
    assert_eq!(result["result"]["value"]["summaries"][0]["mean"], 1.0);
    server.shutdown().unwrap();
}

#[test]
fn decision_vector_and_portable_metadata_response_quotas_precede_admission() {
    let mut server = Server::new(Config { max_response_bytes: 65536, max_retained_jobs: 1, ..Config::default() }).unwrap();
    initialize(&mut server);
    let mut wide = small_model();
    wide["variables"] = Value::Array(
        (0..1024)
            .map(|i| json!({"name":format!("x{i}"),"lower":1.2345678901234567e100,"upper":1.2345678901234567e100,"kind":"continuous"}))
            .collect(),
    );
    wide["objective"]["linear"] = json!([]);
    assert_eq!(call(&mut server, "solvercraft_start_solve", json!({"model":wide}))["error"]["code"], "quota");
    let context = json!({"run_id":"quota-run","snapshot_id":"snapshot","snapshot_revision":"\u{0001}".repeat(256),"host":"artcraft","source_revision":"\u{0001}".repeat(128)});
    let execute = |model: Value| json!({"request":{"schema_version":2,"model":{"schema_version":2,"id":"portable-quota","revision":"1","name":"test model","definition":{"kind":"algebraic","model":model}},"context":context}});
    assert_eq!(call(&mut server, "solvercraft_start_execute", execute(wide))["error"]["code"], "quota");
    // Neither failed request consumed the single retained slot. Both small
    // result types remain retrievable under the same 64-KiB response limit.
    let small = call(&mut server, "solvercraft_start_solve", json!({"model":small_model()}));
    wait_done(&mut server, &small["job_id"]);
    assert_eq!(call(&mut server, "solvercraft_job_get", json!({"job_id":small["job_id"]}))["result"]["value"]["objective"], 1.0);
    call(&mut server, "solvercraft_job_forget", json!({"job_id":small["job_id"]}));
    let small = call(&mut server, "solvercraft_start_execute", execute(small_model()));
    assert!(small["job_id"].is_string(), "{small}");
    wait_done(&mut server, &small["job_id"]);
    let returned = call(&mut server, "solvercraft_job_get", json!({"job_id":small["job_id"]}));
    assert_eq!(returned["result"]["value"]["original_objective"], 1.0);
    assert_eq!(returned["result"]["value"]["manifest"]["context"]["snapshot_revision"], context["snapshot_revision"]);
    server.shutdown().unwrap();
}
