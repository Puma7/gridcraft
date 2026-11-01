#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use solvercraft::{
    server::{Server, ServerConfig},
    types::*,
};
use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
static SERIAL: Mutex<()> = Mutex::new(());
const TOKEN: &str = "synthetic-test-token-only-0123456789abcdef";
struct Running {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<Result<()>>>,
    address: SocketAddr,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
fn start(config: ServerConfig) -> Running {
    let server = Server::bind(config).unwrap();
    let address = server.address().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let join = std::thread::spawn(move || server.run(&flag));
    Running { stop, join: Some(join), address }
}
fn connect(server: &Running) -> BufReader<TcpStream> {
    let stream = TcpStream::connect_timeout(&server.address, Duration::from_secs(2)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    BufReader::new(stream)
}
fn call(client: &mut BufReader<TcpStream>, command: Value) -> Value {
    serde_json::to_writer(client.get_mut(), &json!({"schema_version":1,"id":"9007199254740993","command":command})).unwrap();
    client.get_mut().write_all(b"\n").unwrap();
    let mut line = String::new();
    client.read_line(&mut line).unwrap();
    let value: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(value["id"], "9007199254740993");
    value
}
fn auth(client: &mut BufReader<TcpStream>) {
    assert_eq!(call(client, json!({"method":"authenticate","params":{"token":TOKEN}}))["ok"], true);
}
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: 1,
        name: "server LP".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(10.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}
fn completed(client: &mut BufReader<TcpStream>, id: &str) -> Value {
    let start = solvercraft::runtime::monotonic_now();
    loop {
        let response = call(client, json!({"method":"get","params":{"job_id":id,"include_manifest":true}}));
        assert_eq!(response["ok"], true, "{response}");
        if !response["value"]["result"].is_null() {
            return response["value"].clone();
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn real_tcp_authentication_is_per_connection_and_request_fields_are_strict() {
    let _serial = SERIAL.lock().unwrap();
    let server = start(ServerConfig::local(TOKEN.into()));
    let mut first = connect(&server);
    assert_eq!(call(&mut first, json!({"method":"capabilities","params":{}}))["error"]["code"], "authentication");
    assert_eq!(call(&mut first, json!({"method":"authenticate","params":{"token":"wrong"}}))["ok"], false);
    auth(&mut first);
    assert_eq!(call(&mut first, json!({"method":"capabilities","params":{}}))["ok"], true);
    let mut second = connect(&server);
    assert_eq!(call(&mut second, json!({"method":"get","params":{"job_id":"1"}}))["error"]["code"], "authentication");
    // Duplicate and unknown fields are parsed directly into typed DTOs.
    for request in [
        r#"{"schema_version":1,"id":"1","command":{"method":"authenticate","params":{"token":"x","typo":1}}}"#,
        r#"{"schema_version":1,"schema_version":1,"id":"1","command":{"method":"capabilities","params":{}}}"#,
    ] {
        first.get_mut().write_all(request.as_bytes()).unwrap();
        first.get_mut().write_all(b"\n").unwrap();
        let mut line = String::new();
        first.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["error"]["code"], "invalid_json");
    }
}
#[test]
fn real_server_jobs_solve_poll_quota_forget_and_reuse() {
    let _serial = SERIAL.lock().unwrap();
    let mut config = ServerConfig::local(TOKEN.into());
    config.max_jobs = 1;
    let server = start(config);
    let mut client = connect(&server);
    auth(&mut client);
    let response = call(&mut client, json!({"method":"solve","params":{"model":model(),"options":{"seed":"18446744073709551615"}}}));
    assert_eq!(response["ok"], true, "{response}");
    let id = response["value"]["job_id"].as_str().unwrap();
    let done = completed(&mut client, id);
    assert_eq!(done["result"]["value"]["objective"], 10.0);
    assert_eq!(done["result"]["value"]["optimality"], "optimal");
    let manifest = &done["manifest"];
    assert_eq!(manifest["entrypoint"], "server.solve");
    assert_eq!(manifest["actual_engine"], done["result"]["value"]["engine"]);
    assert_eq!(manifest["effective_configuration"]["options"]["seed"], "18446744073709551615");
    assert_eq!(manifest["effective_configuration"]["limits"]["max_time_millis"], 60_000);
    assert_eq!(manifest["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(manifest["platform"]["architecture"], std::env::consts::ARCH);
    assert_eq!(manifest["result_location"], format!("server:job/{id}"));
    assert!(manifest["input"]["serialized_bytes"].as_u64().unwrap() > 0);
    assert_eq!(call(&mut client, json!({"method":"solve","params":{"model":model()}}))["error"]["code"], "quota");
    assert_eq!(call(&mut client, json!({"method":"forget","params":{"job_id":id}}))["ok"], true);
    assert_eq!(call(&mut client, json!({"method":"get","params":{"job_id":id}}))["error"]["code"], "unknown_job");
    assert_eq!(call(&mut client, json!({"method":"solve","params":{"model":model(),"limits":{"max_time_millis":0}}}))["ok"], true);
}
#[test]
fn real_simulation_job_can_cancel_and_cannot_exceed_response_quota() {
    let _serial = SERIAL.lock().unwrap();
    let server = start(ServerConfig::local(TOKEN.into()));
    let mut client = connect(&server);
    auth(&mut client);
    let response = call(
        &mut client,
        json!({"method":"simulate","params":{"inputs":[{"kind":"uniform","low":0,"high":1}],"options":{"trials":10000000,"retain_samples":false,"seed":"18446744073709551615"}}}),
    );
    assert_eq!(response["ok"], true, "{response}");
    let id = response["value"]["job_id"].as_str().unwrap();
    assert_eq!(call(&mut client, json!({"method":"cancel","params":{"job_id":id}}))["ok"], true);
    let done = completed(&mut client, id);
    assert_eq!(done["result"]["value"]["termination"], "cancelled");
    assert!(done["result"]["value"]["completed_trials"].as_u64().unwrap() < 10000000);
    assert_eq!(done["manifest"]["entrypoint"], "server.simulate");
    assert_eq!(done["manifest"]["effective_configuration"]["options"]["seed"], "18446744073709551615");
    assert!(done["manifest"]["actual_engine"].as_str().unwrap().contains(done["result"]["value"]["sampling_version"].as_str().unwrap()));
    assert_eq!(
        call(
            &mut client,
            json!({"method":"simulate","params":{"inputs":[{"kind":"normal","mean":0,"std_dev":1}],"options":{"trials":1000000,"retain_samples":true}}})
        )["error"]["code"],
        "quota"
    );
}
#[test]
fn failed_execution_keeps_its_run_identity_without_fabricating_an_engine() {
    let _serial = SERIAL.lock().unwrap();
    let server = start(ServerConfig::local(TOKEN.into()));
    let mut client = connect(&server);
    auth(&mut client);
    let response = call(
        &mut client,
        json!({"method":"simulate","params":{
            "inputs":[{"kind":"constant","value":0.0}],
            "outputs":[{"op":"ln","value":{"op":"variable","index":0}}],
            "options":{"trials":1,"seed":"9007199254740993"}
        }}),
    );
    assert_eq!(response["ok"], true, "{response}");
    let id = response["value"]["job_id"].as_str().unwrap();
    let done = completed(&mut client, id);
    assert_eq!(done["result"]["ok"], false, "{done}");
    assert_eq!(done["manifest"]["effective_configuration"]["options"]["seed"], "9007199254740993");
    assert!(done["manifest"]["actual_engine"].as_str().unwrap().starts_with("unknown:"));
    let small = call(&mut client, json!({"method":"get","params":{"job_id":id}}));
    assert_eq!(small["value"]["manifest_available"], true);
    assert!(small["value"].get("manifest").is_none());
}
#[test]
fn service_rejects_invalid_configuration_and_closes_oversized_input() {
    let _serial = SERIAL.lock().unwrap();
    let mut config = ServerConfig::local(TOKEN.into());
    config.listen = "0.0.0.0:0".into();
    assert!(Server::bind(config).is_err());
    let config = ServerConfig::local("short".into());
    assert!(Server::bind(config).is_err());
    let mut config = ServerConfig::local(TOKEN.into());
    config.max_request_bytes = 1024;
    let server = start(config);
    let mut client = connect(&server);
    client.get_mut().write_all(&vec![b'x'; 1025]).unwrap();
    let mut line = String::new();
    assert!(client.read_line(&mut line).is_err() || line.is_empty());
}
