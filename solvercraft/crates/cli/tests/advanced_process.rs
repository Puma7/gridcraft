#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};
fn run(command: &str, request: Value) -> (bool, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solvercraft-cli"))
        .args([command, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    serde_json::to_writer(&mut input, &request).unwrap();
    drop(input);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = vec![];
        stderr.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes).unwrap();
        bytes
    });
    let started = solvercraft::runtime::monotonic_now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI watchdog: {command}");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert!(stdout.len() <= 2 * 1024 * 1024 && stderr.len() <= 2 * 1024 * 1024);
    let value = serde_json::from_slice(if status.success() { &stdout } else { &stderr })
        .unwrap_or_else(|e| panic!("{command}: {e}; stderr {}", String::from_utf8_lossy(&stderr)));
    if status.success() {
        assert!(stderr.is_empty(), "{command}: {}", String::from_utf8_lossy(&stderr));
    }
    (status.success(), value)
}
fn ok(command: &str, request: Value) -> Value {
    let (success, value) = run(command, request.clone());
    assert!(success, "{command}: {value}");
    if command != "execute" {
        assert_manifest(command, &request, &value);
    }
    value
}
fn assert_configuration_contains(actual: &Value, requested: &Value) {
    if let Some(fields) = requested.as_object() {
        for (key, value) in fields {
            assert_configuration_contains(&actual[key], value);
        }
    } else if let Some(number) = requested.as_number() {
        if let (Some(integer), Some(text)) = (number.as_u64(), actual.as_str()) {
            assert_eq!(text.parse::<u64>().ok(), Some(integer), "canonical seed must preserve every integer bit");
        } else {
            assert_eq!(actual.as_f64(), requested.as_f64(), "effective numeric option changed");
        }
    } else {
        assert_eq!(actual, requested, "effective option changed");
    }
}
fn assert_manifest(command: &str, request: &Value, result: &Value) {
    let manifest = &result["manifest"];
    assert_eq!(manifest["schema_version"], 1, "{command}: {manifest}");
    assert_eq!(manifest["entrypoint"], command);
    assert_eq!(manifest["solvercraft_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["platform"]["operating_system"], std::env::consts::OS);
    assert_eq!(manifest["platform"]["architecture"], std::env::consts::ARCH);
    assert_eq!(manifest["platform"]["family"], std::env::consts::FAMILY);
    assert_eq!(manifest["platform"]["pointer_width"], usize::BITS);
    assert_eq!(manifest["input"]["algorithm"], "fnv1a64-serde-json-diagnostic-v1");
    let fingerprint = manifest["input"]["value"].as_str().unwrap();
    assert_eq!(fingerprint.len(), 16);
    assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(manifest["input"]["serialized_bytes"].as_u64().is_some_and(|bytes| bytes > 0 && bytes <= 16 * 1024 * 1024));
    assert_eq!(manifest["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(manifest["snapshot"]["value"], fingerprint);
    let configuration = &manifest["effective_configuration"];
    assert!(configuration.is_object(), "{command}: {configuration}");
    if let Some(options) = request.get("options") {
        assert_configuration_contains(&configuration["options"], options);
    }
    if !matches!(command, "decision-tree" | "information-value") {
        assert!(configuration["options"].is_object(), "typed defaults must be explicit");
    }
    let engine = manifest["actual_engine"].as_str().unwrap();
    assert!(!engine.is_empty());
    let returned_engine = [
        "/engine",
        "/result/engine",
        "/result/result/engine",
        "/solution/engine",
        "/solve/engine",
        "/training/solve/engine",
        "/stages/0/engine",
        "/result/result/sampling_version",
        "/result/sampling_version",
    ]
    .iter()
    .find_map(|pointer| result.pointer(pointer).and_then(Value::as_str));
    if let Some(returned_engine) = returned_engine {
        assert_eq!(engine, returned_engine, "manifest must identify the actual returned engine/sampler");
    }
    let locator = manifest["result_location"].as_str().unwrap();
    match locator {
        "stdout:$" => assert!(result.is_object()),
        "stdout:$.result" => assert!(result["result"].is_object()),
        other => panic!("unresolved result locator: {other}"),
    }
}
fn linear() -> Value {
    json!({"schema_version":1,"name":"public CLI model","variables":[{"name":"x","lower":0,"upper":10,"kind":"continuous"}],"objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1}],"quadratic":[]},"constraints":[]})
}
fn wyndor() -> Value {
    json!({"schema_version":1,"name":"Wyndor","variables":[{"name":"x","lower":0,"upper":4,"kind":"continuous"},{"name":"y","lower":0,"upper":6,"kind":"continuous"}],"objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":3},{"variable":1,"coefficient":5}],"quadratic":[]},"constraints":[{"name":"capacity","terms":[{"variable":0,"coefficient":3},{"variable":1,"coefficient":2}],"relation":"less_equal","rhs":18}]})
}
fn expression(objective: Value) -> Value {
    json!({"name":"public expression","variables":[{"name":"x","lower":-2,"upper":2,"kind":"continuous"}],"direction":"minimize","objective":objective,"constraints":[]})
}
#[test]
fn all_advanced_commands_are_real_typed_boundaries_and_reject_empty_requests() {
    for command in [
        "execute",
        "solve-qp",
        "solve-sparse",
        "solve-conic",
        "solve-mixed-conic",
        "solve-basis",
        "solve-constrained",
        "solve-multistart",
        "solve-global",
        "solve-discrete",
        "simulate-advanced",
        "parameter-grid",
        "fit",
        "recourse",
        "benders",
        "chance",
        "finite",
        "decision-tree",
        "information-value",
        "multiobjective",
        "saa",
        "robust-box",
        "robust-budget",
        "robust-ellipsoid",
    ] {
        let (success, error) = run(command, json!({}));
        assert!(!success, "{command}");
        assert_ne!(error["code"], "unknown_command", "{command}");
        assert!(error["code"].is_string(), "{command}: {error}");
    }
}
#[test]
fn execute_target_preserves_manifest_context_and_original_goal_units() {
    let value = ok(
        "execute",
        json!({"schema_version":2,"model":{"schema_version":2,"id":"18446744073709551615","revision":"9007199254740993","name":"target","definition":{"kind":"algebraic","model":linear()},"options":{"seed":"18446744073709551615"}},"context":{"run_id":"run-1","snapshot_id":"snapshot","snapshot_revision":"9007199254740993","host":"CLI test","source_revision":null},"goal":{"kind":"target","value":5}}),
    );
    assert_eq!(value["target_reached"], true);
    assert_eq!(value["original_objective"], 5.0);
    assert_eq!(value["manifest"]["options"]["seed"], "18446744073709551615");
    assert_eq!(value["manifest"]["context"]["snapshot_revision"], "9007199254740993");
    assert!(value["manifest"]["engine"].as_str().is_some_and(|s| !s.is_empty()));
}
#[test]
fn sparse_and_reusable_basis_paths_have_correct_objectives_and_true_warm_state() {
    let sparse = ok("solve-sparse", json!({"model":wyndor()}));
    assert!((sparse["result"]["objective"].as_f64().unwrap() - 36.0).abs() < 1e-6);
    assert!(sparse["certificate"].is_object());
    let basis = ok("solve-basis", json!({"model":wyndor()}));
    assert_eq!(basis["result"]["objective"], 36.0);
    assert!(basis["sensitivity"].is_object());
    let mut changed = wyndor();
    changed["constraints"][0]["rhs"] = json!(21);
    let warm = ok("solve-basis", json!({"model":changed,"basis":basis["basis"]}));
    assert_eq!(warm["warm_start_used"], true);
    assert_eq!(warm["result"]["objective"], 39.0);
}
#[test]
fn conic_and_finite_integer_quadratic_paths_use_real_supported_engines() {
    let base = json!({"schema_version":1,"name":"circle","variables":[{"name":"x","lower":-2,"upper":2,"kind":"continuous"},{"name":"y","lower":-2,"upper":2,"kind":"continuous"}],"objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":3},{"variable":1,"coefficient":4}],"quadratic":[]},"constraints":[]});
    let cone = json!({"name":"radius","components":[{"offset":0,"terms":[{"variable":0,"coefficient":1}]},{"offset":0,"terms":[{"variable":1,"coefficient":1}]}],"upper":{"offset":2,"terms":[]}});
    let result = ok("solve-conic", json!({"model":{"base":base,"second_order_cones":[cone]}}));
    assert!((result["result"]["objective"].as_f64().unwrap() - 10.0).abs() < 1e-5);
    assert!(result["result"]["best_bound"].as_f64().unwrap() >= 10.0 - 1e-7);
    let mut integer = linear();
    integer["variables"][0]["kind"] = json!("integer");
    integer["variables"][0]["upper"] = json!(3);
    integer["objective"] =
        json!({"direction":"minimize","offset":1.69,"linear":[{"variable":0,"coefficient":-2.6}],"quadratic":[{"left":0,"right":0,"coefficient":1}]});
    let result = ok("solve-mixed-conic", json!({"model":{"base":integer,"second_order_cones":[]}}));
    assert!((result["result"]["objective"].as_f64().unwrap() - 0.09).abs() < 1e-6);
    assert_eq!(result["result"]["values"][0], 1.0);
}
#[test]
fn nonlinear_multistart_and_replayable_global_commands_are_executable() {
    let mut constrained = expression(json!({"op":"variable","index":0}));
    constrained["constraints"] = json!([{"name":"growth","left":{"op":"exp","value":{"op":"variable","index":0}},"relation":"greater_equal","right":{"op":"constant","value":2}}]);
    let result = ok("solve-constrained", json!({"model":constrained,"options":{"method":"projected_gradient"}}));
    assert!((result["values"][0].as_f64().unwrap() - 2.0_f64.ln()).abs() < 1e-5);
    assert_eq!(result["optimality"], "stationary");
    assert!(result["best_bound"].is_null());
    let square = expression(json!({"op":"power","base":{"op":"variable","index":0},"exponent":2}));
    let multi = ok("solve-multistart", json!({"model":square,"multistart":{"starts":3,"seed":"18446744073709551615"}}));
    assert!(multi["solution"]["objective"].as_f64().unwrap() < 1e-8);
    assert_eq!(multi["attempted_starts"], 3);
    let global = ok("solve-global", json!({"model":square,"options":{"max_nodes":100}}));
    assert_eq!(global["solution"]["objective"], 0.0);
    assert!(global["certificate"].is_object());
}
#[test]
fn advanced_simulation_parameter_grid_and_fit_are_real_process_workflows() {
    let result = ok(
        "simulate-advanced",
        json!({"inputs":[{"kind":"uniform","low":0,"high":1}],"options":{"base":{"trials":128},"sampler":{"kind":"randomized_sobol","replicate":"1"}}}),
    );
    assert_eq!(result["result"]["result"]["completed_trials"], 128);
    assert!(result["result"]["result"]["sampling_version"].as_str().unwrap().contains("sobol"));
    let result = ok(
        "parameter-grid",
        json!({"axes":[{"name":"price","values":[2,5]},{"name":"quantity","values":[3,7]}],"inputs":[],"outputs":[{"op":"multiply","left":{"op":"variable","index":0},"right":{"op":"variable","index":1}}],"options":{"simulation":{"trials":4}}}),
    );
    for (run, expected) in result["runs"].as_array().unwrap().iter().zip([6.0, 14.0, 15.0, 35.0]) {
        assert_eq!(run["result"]["summaries"][0]["mean"], expected);
    }
    let fit = ok("fit", json!({"values":[1,2,3,4,5],"families":[{"kind":"normal"},{"kind":"uniform"}],"criterion":"aic"}));
    assert_eq!(fit["fits"][0][1]["distribution"]["kind"], "uniform");
    assert!(fit["failures"].as_array().unwrap().is_empty());
    assert!((fit["fits"][0][1]["aic"].as_f64().unwrap() - (4.0 + 10.0 * 4.0_f64.ln())).abs() < 1e-9);
}
#[test]
fn recourse_and_decomposition_agree_with_independent_inventory_breakpoint_oracle() {
    let scenarios:Vec<_>=[3.0,8.0].into_iter().map(|demand|json!({"name":format!("demand{demand}"),"weight":1,"variables":[{"name":"shortage","lower":0,"upper":null,"kind":"continuous"}],"costs":[6],"offset":0,"constraints":[{"name":"demand","first_stage_terms":[{"variable":0,"coefficient":1}],"recourse_terms":[{"variable":0,"coefficient":1}],"relation":"greater_equal","rhs":demand}]})).collect();
    let request = json!({"problem":{"name":"inventory","direction":"minimize","first_stage_variables":[{"name":"order","lower":0,"upper":10,"kind":"continuous"}],"first_stage_costs":[2],"first_stage_offset":0,"first_stage_constraints":[],"scenarios":scenarios}});
    let result = ok("recourse", request.clone());
    assert_eq!(result["expected_total_cost"], 16.0);
    assert_eq!(result["first_stage_values"][0], 8.0);
    let benders = ok("benders", request);
    assert!((benders["objective"].as_f64().unwrap() - 16.0).abs() < 1e-7);
    assert!(benders["lower_bound"].as_f64().unwrap() <= 16.0 + 1e-7);
    assert_eq!(benders["converged"], true);
}
#[test]
fn quadratic_warm_start_is_a_real_revalidated_public_contract() {
    let mut model = linear();
    model["objective"] =
        json!({"direction":"minimize","offset":4,"linear":[{"variable":0,"coefficient":-4}],"quadratic":[{"left":0,"right":0,"coefficient":1}]});
    let first = ok("solve-qp", json!({"model":model}));
    assert!(first["result"]["objective"].as_f64().unwrap().abs() < 1e-8);
    assert!(first["warm_start"].is_object());
    model["objective"]["offset"] = json!(9);
    model["objective"]["linear"][0]["coefficient"] = json!(-6);
    let second = ok("solve-qp", json!({"model":model,"warm_start":first["warm_start"]}));
    assert_eq!(second["warm_start_used"], true);
    assert!((second["result"]["values"][0].as_f64().unwrap() - 3.0).abs() < 1e-6);
}
