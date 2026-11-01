#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use solvercraft::{
    project::*,
    registry,
    runtime::{self, RuntimeLimits},
    types::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: 1,
        name: "independent API review".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}
fn portable(id: &str) -> PortableModel {
    PortableModel {
        schema_version: 2,
        id: id.into(),
        revision: "0".into(),
        name: format!("model {id}"),
        definition: ModelDefinition::Algebraic(model()),
        options: SolveOptions::default(),
        bindings: vec![],
        metadata: BTreeMap::new(),
    }
}
#[test]
fn portable_nested_json_is_strict_and_binary64_revisions_roundtrip() {
    let mut p = portable("18446744073709551615");
    p.revision = "18446744073709551615".into();
    p.options.seed = u64::MAX;
    if let ModelDefinition::Algebraic(m) = &mut p.definition {
        m.objective.offset = f64::from_bits(0x3fe0_0000_0000_0001);
        m.variables[0].lower = Some(f64::from_bits(1));
        m.variables[0].upper = Some(f64::from_bits(0x3ff0_0000_0000_0001));
    }
    let library = ModelLibrary { schema_version: 2, models: vec![p] };
    let bytes = export(&library).expect("export");
    let back = import(&bytes).expect("import");
    assert_eq!(back.models[0].revision, u64::MAX.to_string());
    assert_eq!(back.models[0].options.seed, u64::MAX);
    let ModelDefinition::Algebraic(m) = &back.models[0].definition else { panic!("algebraic variant") };
    assert_eq!(m.objective.offset.to_bits(), 0x3fe0_0000_0000_0001);
    assert_eq!(m.variables[0].lower.unwrap().to_bits(), 1);
    assert_eq!(m.variables[0].upper.unwrap().to_bits(), 0x3ff0_0000_0000_0001);
    for pointer in [
        "/models/0",
        "/models/0/options",
        "/models/0/definition",
        "/models/0/definition/model",
        "/models/0/definition/model/objective",
        "/models/0/definition/model/variables/0",
    ] {
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value.pointer_mut(pointer).unwrap().as_object_mut().unwrap().insert("unrecognized".into(), json!(true));
        assert!(import(&serde_json::to_vec(&value).unwrap()).is_err(), "Unknown field accepted at {pointer}");
    }
    for revision in ["", "+1", "01", "-1", "18446744073709551616"] {
        let mut invalid = back.models[0].clone();
        invalid.revision = revision.into();
        assert!(invalid.validate().is_err(), "Noncanonical revision {revision}");
    }
}
#[test]
fn failed_registry_mutations_are_atomic_including_exhaustion_and_collisions() {
    let mut library = ModelLibrary::default();
    let mut original = portable("a");
    original.revision = u64::MAX.to_string();
    library.insert(original.clone()).unwrap();
    library.insert(portable("b")).unwrap();
    let before = export(&library).unwrap();
    let mut collision = original.clone();
    collision.name = "MODEL B".into();
    for (revision, replacement) in [("0", original.clone()), ("18446744073709551615", collision), ("18446744073709551615", original.clone())] {
        assert!(library.replace("a", revision, replacement).is_err());
        assert_eq!(export(&library).unwrap(), before);
    }
    assert!(library.remove("a", "0").is_err());
    assert_eq!(export(&library).unwrap(), before);
    for index in 2..128 {
        library.insert(portable(&index.to_string())).unwrap();
    }
    let full = export(&library).unwrap();
    assert!(library.insert(portable("overflow")).is_err());
    assert_eq!(export(&library).unwrap(), full);
    let old = serde_json::to_vec(&model()).unwrap();
    let migrated = migrate_v1_algebraic(&old, "stable-id".into(), SolveOptions::default()).unwrap();
    assert_eq!(migrated.revision, "0");
    assert!(migrated.bindings.is_empty());
    assert_eq!(migrated.id, "stable-id");
    assert!(import(&vec![b' '; MAX_PROJECT_BYTES + 1]).is_err());
}
#[test]
fn engine_registry_is_unique_and_does_not_emulate_requested_methods() {
    let engines = registry::engines();
    let mut ids = BTreeSet::new();
    for engine in engines {
        assert!(ids.insert(engine.id.clone()));
        assert!(!engine.classes.is_empty());
        for class in engine.classes {
            assert_eq!(registry::select(class, Some(&engine.id)).unwrap().id, engine.id);
        }
    }
    assert!(registry::select(registry::ProblemClass::Linear, Some("grg")).is_err());
    assert!(registry::select(registry::ProblemClass::MixedIntegerConic, Some("clarabel_0_11_1")).is_err());
}
#[test]
fn owned_runtime_routes_integer_quadratics_and_preserves_stop_reason() {
    let mut m = model();
    m.variables[0].kind = VariableKind::Integer;
    m.variables[0].upper = Some(2.0);
    m.objective.offset = 0.09;
    m.objective.linear[0].coefficient = -0.6;
    m.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: 1.0 });
    let result = runtime::solve_with_limits_owned(&m, &SolveOptions::default(), RuntimeLimits::default(), Arc::new(NoControl)).unwrap();
    assert_eq!(result.feasibility, Feasibility::Feasible);
    assert_eq!(result.values.unwrap(), vec![0.0]);
    assert!((result.objective.unwrap() - 0.09).abs() < 1e-12);
    assert!(result.engine.contains("conic-branch-and-bound"), "{}", result.engine);
    let expired =
        runtime::solve_with_limits_owned(&m, &SolveOptions::default(), RuntimeLimits { max_time_millis: Some(0) }, Arc::new(NoControl)).unwrap();
    assert_eq!(expired.termination, Termination::TimeLimit);
    assert_ne!(expired.optimality, Optimality::Optimal);
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let cancelled =
        runtime::solve_with_limits_owned(&m, &SolveOptions::default(), RuntimeLimits { max_time_millis: Some(0) }, Arc::new(Cancel)).unwrap();
    assert_eq!(cancelled.termination, Termination::Cancelled);
}
#[test]
fn analytic_quadratic_progress_does_not_claim_simplex_execution() {
    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);
    impl RunControl for Recorder {
        fn is_cancelled(&self) -> bool {
            false
        }
        fn progress(&self, event: &ProgressEvent) {
            self.0.lock().unwrap().push(event.stage.clone());
        }
    }
    let mut m = model();
    m.objective.linear[0].coefficient = -1.0;
    m.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: 1.0 });
    let recorder = Recorder::default();
    let result = solvercraft::solve(&m, &SolveOptions::default(), &recorder).unwrap();
    assert!(result.engine.contains("separable"));
    assert!(!recorder.0.lock().unwrap().iter().any(|stage| stage == "simplex"), "The analytic QP must not advertise a simplex iteration");
}

#[cfg(not(target_arch = "wasm32"))]
mod tcp {
    use super::*;
    use solvercraft::server::{Server, ServerConfig};
    use std::{
        io::{BufRead, BufReader, Write},
        net::{SocketAddr, TcpStream},
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    static SERIAL: Mutex<()> = Mutex::new(());
    const TOKEN: &str = "independent-api-test-token-0123456789abcdef";
    struct Running {
        stop: Arc<AtomicBool>,
        join: Option<std::thread::JoinHandle<Result<()>>>,
        address: SocketAddr,
    }
    impl Running {
        fn stop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(j) = self.join.take() {
                j.join().unwrap().unwrap();
            }
        }
    }
    impl Drop for Running {
        fn drop(&mut self) {
            self.stop();
        }
    }
    fn start(config: ServerConfig) -> Running {
        let server = Server::bind(config).unwrap();
        let address = server.address().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        Running { stop, join: Some(std::thread::spawn(move || server.run(&flag))), address }
    }
    fn connect(server: &Running) -> BufReader<TcpStream> {
        let stream = TcpStream::connect_timeout(&server.address, Duration::from_secs(2)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        BufReader::new(stream)
    }
    fn send(client: &mut BufReader<TcpStream>, command: Value) {
        serde_json::to_writer(client.get_mut(), &json!({"schema_version":1,"id":"18446744073709551615","command":command})).unwrap();
        client.get_mut().write_all(b"\n").unwrap();
    }
    fn call(client: &mut BufReader<TcpStream>, command: Value) -> Value {
        send(client, command);
        let mut line = String::new();
        client.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        if value["error"]["code"] == "invalid_json" {
            assert_eq!(value["id"], "");
        } else {
            assert_eq!(value["id"], "18446744073709551615");
        }
        value
    }
    fn auth(client: &mut BufReader<TcpStream>) {
        assert_eq!(call(client, json!({"method":"authenticate","params":{"token":TOKEN}}))["ok"], true);
    }
    #[test]
    fn authentication_is_per_connection_and_failed_reauthentication_revokes_access() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let server = start(ServerConfig::local(TOKEN.into()));
        let mut first = connect(&server);
        auth(&mut first);
        let mut second = connect(&server);
        assert_eq!(call(&mut second, json!({"method":"capabilities","params":{}}))["error"]["code"], "authentication");
        assert_eq!(call(&mut first, json!({"method":"authenticate","params":{"token":"incorrect"}}))["error"]["code"], "authentication");
        assert_eq!(call(&mut first, json!({"method":"capabilities","params":{}}))["error"]["code"], "authentication");
        auth(&mut first);
        assert_eq!(
            call(&mut first, json!({"method":"solve","params":{"model":model(),"limits":{"max_time_millis":0,"typo":1}}}))["error"]["code"],
            "invalid_json"
        );
    }
    #[test]
    fn bounded_responses_preserve_connection_and_structured_request_identity() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_response_bytes = 1024;
        config.max_jobs = 1;
        let server = start(config);
        let mut client = connect(&server);
        auth(&mut client);
        let response = call(&mut client, json!({"method":"capabilities","params":{}}));
        assert_eq!(response["error"]["code"], "output_limit");
        assert!(serde_json::to_vec(&response).unwrap().len() <= 1024);
        let response = call(&mut client, json!({"method":"solve","params":{"model":model(),"limits":{"max_time_millis":0}}}));
        assert_eq!(response["ok"], true);
        let id = response["value"]["job_id"].as_str().unwrap();
        let start = runtime::monotonic_now();
        loop {
            let response = call(&mut client, json!({"method":"get","params":{"job_id":id}}));
            assert_eq!(response["ok"], true, "{response}");
            if !response["value"]["result"].is_null() {
                assert_eq!(response["value"]["result"]["value"]["termination"], "time_limit");
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        assert_eq!(call(&mut client, json!({"method":"solve","params":{"model":model()}}))["error"]["code"], "quota");
        assert_eq!(call(&mut client, json!({"method":"forget","params":{"job_id":id}}))["ok"], true);
        assert_eq!(
            call(
                &mut client,
                json!({"method":"simulate","params":{"inputs":[{"kind":"constant","value":1}],"options":{"trials":1,"retain_samples":true}}})
            )["error"]["code"],
            "quota"
        );
    }
    #[test]
    fn server_does_not_start_a_job_after_shutdown_has_returned() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut server = start(ServerConfig::local(TOKEN.into()));
        let mut client = connect(&server);
        auth(&mut client);
        // Ensure the authenticated worker has entered its next blocking read.
        std::thread::sleep(Duration::from_millis(25));
        server.stop();
        send(&mut client, json!({"method":"solve","params":{"model":model()}}));
        let mut line = String::new();
        if let Ok(count) = client.read_line(&mut line)
            && count > 0
        {
            let response: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(response["ok"], false, "Shutdown returned before a fresh job was accepted: {response}");
        }
    }
    #[test]
    fn simulation_outputs_share_one_global_expression_budget() {
        fn expression(depth: usize) -> Value {
            if depth == 0 {
                json!({"op":"constant","value":1.0})
            } else {
                json!({"op":"add","left":expression(depth - 1),"right":expression(depth - 1)})
            }
        }
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_request_bytes = 16 * 1024 * 1024;
        let server = start(config);
        let mut client = connect(&server);
        auth(&mut client);
        // Each output has only 31 nodes, but 4096 outputs contain 126976 nodes.
        // The public model compiler's shared cap is 100000, not per output.
        let outputs = vec![expression(4); 4096];
        let response = call(
            &mut client,
            json!({"method":"simulate","params":{"inputs":[{"kind":"constant","value":1}],"outputs":outputs,"options":{"trials":1,"retain_samples":false}}}),
        );
        assert_eq!(response["ok"], false, "The service admitted a model exceeding the shared expression budget: {response}");
        assert_eq!(response["error"]["code"], "expression_limit");
    }
    #[test]
    fn running_jobs_require_cancel_before_forget_and_time_caps_are_enforced() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_job_time_millis = 5000;
        let server = start(config);
        let mut client = connect(&server);
        auth(&mut client);
        let request = json!({"method":"simulate","params":{"inputs":[{"kind":"uniform","low":0,"high":1}],"options":{"trials":10000000,"retain_samples":false}}});
        let response = call(&mut client, request.clone());
        assert_eq!(response["ok"], true, "{response}");
        let id = response["value"]["job_id"].as_str().unwrap();
        assert_eq!(call(&mut client, json!({"method":"forget","params":{"job_id":id}}))["error"]["code"], "job_running");
        assert_eq!(call(&mut client, json!({"method":"cancel","params":{"job_id":id}}))["ok"], true);
        let start = runtime::monotonic_now();
        loop {
            let response = call(&mut client, json!({"method":"get","params":{"job_id":id}}));
            if !response["value"]["result"].is_null() {
                assert_eq!(response["value"]["result"]["value"]["termination"], "cancelled", "{response}");
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
        }
        let mut request = request;
        request["params"]["limits"] = json!({"max_time_millis":1});
        let response = call(&mut client, request);
        let id = response["value"]["job_id"].as_str().unwrap();
        let start = runtime::monotonic_now();
        loop {
            let response = call(&mut client, json!({"method":"get","params":{"job_id":id}}));
            if !response["value"]["result"].is_null() {
                assert_eq!(response["value"]["result"]["value"]["termination"], "time_limit", "{response}");
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
    #[test]
    fn connection_quota_is_enforced_and_released_after_disconnect() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_connections = 1;
        let server = start(config);
        let mut first = connect(&server);
        auth(&mut first);
        let mut excess = connect(&server);
        let request =
            serde_json::to_vec(&json!({"schema_version":1,"id":"quota","command":{"method":"authenticate","params":{"token":TOKEN}}})).unwrap();
        if excess.get_mut().write_all(&request).and_then(|()| excess.get_mut().write_all(b"\n")).is_ok() {
            let mut line = String::new();
            assert!(excess.read_line(&mut line).is_err() || line.is_empty(), "An excess connection was served: {line}");
        }
        drop(excess);
        drop(first);
        let started = runtime::monotonic_now();
        loop {
            let mut replacement = connect(&server);
            if replacement.get_mut().write_all(&request).and_then(|()| replacement.get_mut().write_all(b"\n")).is_ok() {
                let mut line = String::new();
                if replacement.read_line(&mut line).is_ok_and(|count| count > 0) {
                    let response: Value = serde_json::from_str(&line).unwrap();
                    assert_eq!(response["ok"], true, "{response}");
                    break;
                }
            }
            assert!(started.elapsed() < Duration::from_secs(2), "Disconnected client failed to release its quota slot");
        }
    }
    #[test]
    fn retained_simulation_response_admission_counts_per_output_summaries() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let command = json!({"method":"simulate","params":{"inputs":vec![json!({"kind":"constant","value":1});1000],"options":{"trials":1,"retain_samples":true}}});
        // Independent actual-wire control establishes that summaries, rather
        // than the tiny retained row, exceed the smaller server's byte quota.
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_response_bytes = 1_000_000;
        let control = start(config);
        let mut client = connect(&control);
        auth(&mut client);
        let started = call(&mut client, command.clone());
        assert_eq!(started["ok"], true, "{started}");
        let id = started["value"]["job_id"].as_str().unwrap();
        let clock = runtime::monotonic_now();
        loop {
            let response = call(&mut client, json!({"method":"get","params":{"job_id":id}}));
            assert_eq!(response["ok"], true, "{response}");
            if !response["value"]["result"].is_null() {
                assert!(serde_json::to_vec(&response).unwrap().len() > 100_000, "The fixture must genuinely exceed the comparison quota");
                break;
            }
            assert!(clock.elapsed() < Duration::from_secs(3));
        }
        drop(client);
        drop(control);
        let mut config = ServerConfig::local(TOKEN.into());
        config.max_response_bytes = 100_000;
        let bounded = start(config);
        let mut client = connect(&bounded);
        auth(&mut client);
        let response = call(&mut client, command);
        assert_eq!(
            response["error"]["code"], "quota",
            "Retained work whose actual-wire control exceeds the output limit must be refused before admission: {response}"
        );
    }
}
