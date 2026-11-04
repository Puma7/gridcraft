//! Real-process checks for additive, typed and reproducible CLI run manifests.
#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::panic)]
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};

fn run(command: &str, request: &Value) -> (bool, Value) {
    let mut process = Command::new(env!("CARGO_BIN_EXE_solvercraft-cli"));
    process.arg(command);
    if command != "demo" {
        process.arg("-");
    }
    let mut child = process.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    serde_json::to_writer(&mut stdin, request).unwrap();
    drop(stdin);
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
            panic!("Manifest CLI watchdog: {command}");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    assert!(stdout.len() <= 2 * 1024 * 1024 && stderr.len() <= 2 * 1024 * 1024);
    if status.success() {
        assert!(stderr.is_empty());
    } else {
        assert!(stdout.is_empty());
    }
    (status.success(), serde_json::from_slice(if status.success() { &stdout } else { &stderr }).unwrap())
}
fn ok(command: &str, request: &Value) -> Value {
    let (success, value) = run(command, request);
    assert!(success, "{command}: {value}");
    let manifest = &value["manifest"];
    assert_eq!(manifest["entrypoint"], command);
    assert_eq!(manifest["solvercraft_version"], solvercraft::VERSION);
    assert_eq!(manifest["platform"]["architecture"], std::env::consts::ARCH);
    assert_eq!(manifest["platform"]["operating_system"], std::env::consts::OS);
    assert_eq!(manifest["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(manifest["snapshot"]["value"], manifest["input"]["value"]);
    assert_eq!(manifest["input"]["algorithm"], solvercraft::run_manifest::FINGERPRINT_ALGORITHM);
    assert_eq!(manifest["input"]["value"].as_str().unwrap().len(), 16);
    assert!(manifest["effective_configuration"].is_object());
    assert!(!manifest["actual_engine"].as_str().unwrap().is_empty());
    assert!(manifest["result_location"].as_str().unwrap().starts_with("stdout:$"));
    value
}
fn linear() -> Value {
    json!({"schema_version":1,"name":"manifest","variables":[{"name":"x","lower":0,"upper":2,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1}],"quadratic":[]},"constraints":[]})
}
fn square() -> Value {
    json!({"name":"square","variables":[{"name":"x","lower":-2,"upper":2,"kind":"continuous"}],"direction":"minimize",
        "objective":{"op":"power","base":{"op":"variable","index":0},"exponent":2},"constraints":[]})
}
#[test]
fn legacy_simulation_high_seed_defaults_and_input_identity_are_exact() {
    let request = json!({"inputs":[{"kind":"uniform","low":0,"high":1}],"options":{"trials":8,"seed":"18446744073709551615","retain_samples":true}});
    let first = ok("simulate", &request);
    let second = ok("simulate", &request);
    assert_eq!(first, second);
    assert_eq!(first["result"]["completed_trials"], 8);
    let config = &first["manifest"]["effective_configuration"];
    assert_eq!(config["options"]["seed"], "18446744073709551615");
    assert_eq!(config["options"]["method"], "monte_carlo");
    assert_eq!(first["manifest"]["actual_engine"], first["result"]["sampling_version"]);
    let mut changed = request;
    changed["inputs"][0]["high"] = json!(2);
    assert_ne!(first["manifest"]["input"]["value"], ok("simulate", &changed)["manifest"]["input"]["value"]);
    assert!(config.get("inputs").is_none(), "Data should be fingerprinted, not duplicated as options");
}
#[test]
fn demo_preserves_both_real_results_and_records_its_fixed_synthetic_run() {
    let first = ok("demo", &json!({}));
    let second = ok("demo", &json!({}));
    assert_eq!(first, second);
    assert_eq!(first["optimization"]["objective"], 36.0);
    assert_eq!(first["simulation"]["completed_trials"], 10_000);
    let config = &first["manifest"]["effective_configuration"];
    assert_eq!(config["simulation_options"]["seed"], "42");
    assert_eq!(config["simulation_options"]["trials"], 10_000);
    assert_eq!(config["simulation_options"]["method"], "monte_carlo");
    assert_eq!(config["optimization_options"]["feasibility_tolerance"], 1e-7);
    let engine = first["manifest"]["actual_engine"].as_str().unwrap();
    assert!(engine.contains(first["optimization"]["engine"].as_str().unwrap()));
    assert!(engine.contains(first["simulation"]["sampling_version"].as_str().unwrap()));
}
#[test]
fn advanced_sampler_copula_and_invalid_trial_policy_are_recorded_completely() {
    let request = json!({"inputs":[{"kind":"normal","mean":0,"std_dev":1},{"kind":"uniform","low":0,"high":1}],
        "options":{"base":{"trials":8,"seed":"9007199254740993"},"sampler":{"kind":"randomized_sobol","replicate":"18446744073709551615"},
        "dependence":{"kind":"gaussian","correlation":[[1,0.4],[0.4,1]]},"invalid_trials":{"kind":"skip","maximum_invalid":2}}});
    let value = ok("simulate-advanced", &request);
    let options = &value["manifest"]["effective_configuration"]["options"];
    assert_eq!(options["base"]["seed"], "9007199254740993");
    assert_eq!(options["sampler"]["replicate"], "18446744073709551615");
    assert_eq!(options["dependence"]["correlation"][0][1], 0.4);
    assert_eq!(options["invalid_trials"]["maximum_invalid"], 2);
    assert!(options["stopping"].is_null());
    assert_eq!(value["manifest"]["actual_engine"], value["result"]["result"]["sampling_version"]);
    let (success, error) = run("simulate-advanced", &json!({"inputs":[],"options":{"sampler":{"kind":"legacy","replicate":2}}}));
    assert!(!success);
    assert_eq!(error["code"], "invalid_json");
}
#[test]
fn conic_manifest_reports_backend_instead_of_requested_auto_method() {
    let value = ok(
        "solve-conic",
        &json!({"model":{"base":linear(),"second_order_cones":[]},"options":{"seed":"18446744073709551615","feasibility_tolerance":1e-6}}),
    );
    assert_eq!(value["manifest"]["actual_engine"], value["result"]["engine"]);
    assert!(value["manifest"]["actual_engine"].as_str().unwrap().contains("clarabel"));
    let options = &value["manifest"]["effective_configuration"]["options"];
    assert_eq!(options["method"], "auto");
    assert_eq!(options["feasibility_tolerance"], 1e-6);
    assert_eq!(options["optimality_tolerance"], 1e-8);
    assert_eq!(options["seed"], "18446744073709551615");
    assert!(value["certificate"].is_object());
}
#[test]
fn legacy_solve_preserves_old_manifest_fields_and_adds_actual_engine_snapshot() {
    let (success, value) = run("solve", &json!({"model":linear(),"options":{"seed":"18446744073709551615","feasibility_tolerance":1e-6}}));
    assert!(success, "{value}");
    let manifest = &value["manifest"];
    assert_eq!(manifest["entrypoint"], "solve");
    assert_eq!(manifest["actual_engine"], value["result"]["engine"]);
    assert_eq!(manifest["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(manifest["snapshot"]["value"], manifest["model_fingerprint"]);
    assert_eq!(manifest["model_fingerprint"].as_str().unwrap().len(), 16);
    assert_eq!(manifest["fingerprint_algorithm"], "fnv1a64-diagnostic-v1");
    assert_eq!(manifest["options"]["seed"], "18446744073709551615");
    assert_eq!(manifest["options"]["feasibility_tolerance"], 1e-6);
    assert_eq!(manifest["platform"], format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS));
    assert_eq!(manifest["result_location"], "SolveRun.result");
    assert_eq!(value["result"]["objective"], 2.0);
}
#[test]
fn fit_and_grid_keep_all_configuration_and_existing_result_keys() {
    let value = ok(
        "fit",
        &json!({"values":[1,2,3,4,5],"families":[{"kind":"normal"}],"criterion":"bic",
        "options":{"seed":"18446744073709551615","bootstrap_replicates":2,"max_evaluations":100}}),
    );
    assert!(value["fits"].is_array());
    assert!(value["failures"].is_array());
    let config = &value["manifest"]["effective_configuration"];
    assert_eq!(config["families"][0]["kind"], "normal");
    assert_eq!(config["criterion"], "bic");
    assert_eq!(config["options"]["seed"], "18446744073709551615");
    assert_eq!(config["options"]["bootstrap_replicates"], 2);
    assert!(config.get("values").is_none());
    let request = json!({"axes":[{"name":"price","values":[2,5]}],"inputs":[{"kind":"uniform","low":0,"high":1}],
        "options":{"simulation":{"trials":4,"seed":"9007199254740993"},"streams":"distinct_seeds","max_runs":3,"max_total_trials":20}});
    let value = ok("parameter-grid", &request);
    assert_eq!(value["runs"].as_array().unwrap().len(), 2);
    let config = &value["manifest"]["effective_configuration"];
    assert_eq!(config["axes"][0]["values"], json!([2.0, 5.0]));
    assert_eq!(config["options"]["streams"], "distinct_seeds");
    assert_eq!(config["options"]["simulation"]["seed"], "9007199254740993");
    assert_ne!(value["runs"][0]["effective_seed"], value["runs"][1]["effective_seed"]);
}
#[test]
fn nonlinear_multistart_and_expression_paths_preserve_extra_effective_settings() {
    let value = ok(
        "solve-multistart",
        &json!({"model":square(),"options":{"seed":"9007199254740993"},"start":[1.5],
        "nonlinear":{"derivative_mode":"finite_difference","initial_penalty":20,"finite_difference_step":1e-5},
        "multistart":{"starts":2,"seed":"18446744073709551615"}}),
    );
    let config = &value["manifest"]["effective_configuration"];
    assert_eq!(config["start"][0], 1.5);
    assert_eq!(config["nonlinear"]["initial_penalty"], 20.0);
    assert_eq!(config["nonlinear"]["derivative_mode"], "finite_difference");
    assert_eq!(config["multistart"]["seed"], "18446744073709551615");
    assert_eq!(value["manifest"]["actual_engine"], value["solution"]["engine"]);
    let value = ok("solve-expr", &json!({"model":square(),"options":{"seed":"18446744073709551615"}}));
    assert_eq!(value["model"], "square");
    assert!(value["result"].is_object());
    assert_eq!(value["manifest"]["actual_engine"], value["result"]["engine"]);
    assert_eq!(value["manifest"]["effective_configuration"]["options"]["seed"], "18446744073709551615");
}
#[test]
fn risk_and_saa_report_risk_measure_and_separate_training_validation_seeds() {
    let variable = json!({"name":"x","lower":0,"upper":1,"kind":"continuous"});
    let value = ok(
        "risk",
        &json!({"problem":{"name":"risk","variables":[variable.clone()],"constraints":[],
        "scenarios":[{"weight":1,"offset":2,"coefficients":[1]}],"measure":{"kind":"conditional_value_at_risk","confidence":0.9}}}),
    );
    assert_eq!(value["manifest"]["actual_engine"], value["solve"]["engine"]);
    assert_eq!(value["manifest"]["effective_configuration"]["measure"]["confidence"], 0.9);
    let value = ok(
        "saa",
        &json!({"problem":{"name":"saa","variables":[variable],"constraints":[],"loss":{"offset":{"kind":"uniform","low":0,"high":1},"coefficients":[{"kind":"constant","value":1}]},"measure":{"kind":"expected_loss"}},
        "sampling":{"training_trials":4,"validation_trials":4,"training_seed":"18446744073709551615","validation_seed":"9007199254740993"}}),
    );
    let config = &value["manifest"]["effective_configuration"];
    assert_eq!(config["sampling"]["training_seed"], "18446744073709551615");
    assert_eq!(config["sampling"]["validation_seed"], "9007199254740993");
    assert_eq!(config["measure"]["kind"], "expected_loss");
    assert_eq!(value["manifest"]["actual_engine"], value["training"]["solve"]["engine"]);
    assert!(value["validation"].is_object());
}

#[test]
fn remaining_discrete_decision_multiobjective_and_robust_commands_are_manifested() {
    let discrete =
        ok("solve-discrete", &json!({"model":{"model":{"base":linear(),"second_order_cones":[]},"indicators":[],"special_ordered_sets":[]}}));
    assert_eq!(discrete["manifest"]["actual_engine"], discrete["result"]["engine"]);
    let finite = ok("finite", &json!({"model":linear(),"specification":{"domains":[{"variable":0,"values":[0,1]}]}}));
    assert_eq!(finite["objective"], 1.0);
    assert_eq!(finite["manifest"]["effective_configuration"]["specification"]["domains"][0]["values"], json!([0.0, 1.0]));
    let row = json!({"name":"limit","terms":[{"variable":0,"coefficient":1}],"relation":"less_equal","rhs":1});
    let chance = ok(
        "chance",
        &json!({"problem":{"base":linear(),"scenarios":[{"weight":1,"constraints":[row.clone()]}],"rule":{"kind":"joint","maximum_violation_probability":0}}}),
    );
    assert_eq!(chance["manifest"]["actual_engine"], chance["solve"]["engine"]);
    assert_eq!(chance["manifest"]["effective_configuration"]["rule"]["kind"], "joint");
    let decision =
        ok("decision-tree", &json!({"tree":{"root":0,"nodes":[{"kind":"terminal","payoff":7}],"utility":{"kind":"linear"}},"max_memory_bytes":1024}));
    assert_eq!(decision["expected_utility"], 7.0);
    assert_eq!(decision["manifest"]["effective_configuration"]["utility"]["kind"], "linear");
    let information = ok(
        "information-value",
        &json!({"problem":{"state_probabilities":[0.5,0.5],"action_payoffs":[[1,0],[0,1]],"signal_likelihoods":[[1,0],[0,1]],"acquisition_cost":0.1,"utility":{"kind":"linear"}}}),
    );
    assert!((information["net_value_of_signal"].as_f64().unwrap() - 0.4).abs() < 1e-12);
    assert_eq!(information["manifest"]["effective_configuration"]["acquisition_cost"], 0.1);
    assert_eq!(information["manifest"]["effective_configuration"]["utility"]["kind"], "linear");
    let multi = ok(
        "multiobjective",
        &json!({"model":linear(),"goals":[{"name":"x","direction":"maximize","offset":0,"terms":[{"variable":0,"coefficient":1}]}],"method":{"kind":"lexicographic","absolute_tolerances":[0.01]}}),
    );
    assert_eq!(multi["manifest"]["actual_engine"], multi["stages"][0]["engine"]);
    assert_eq!(multi["manifest"]["effective_configuration"]["method"]["absolute_tolerances"][0], 0.01);
    let mut model = linear();
    model["constraints"] = json!([row]);
    for (command, uncertainty) in [
        ("robust-box", json!({"constraint":0,"radii":[0.25]})),
        ("robust-budget", json!({"constraint":0,"radii":[0.25],"budget":1})),
        ("robust-ellipsoid", json!({"constraint":0,"factor":[[0.25]],"radius":1})),
    ] {
        let value = ok(command, &json!({"model":model,"uncertainties":[uncertainty.clone()]}));
        assert!(value["counterpart"].is_object());
        let engine = if command == "robust-ellipsoid" { &value["result"]["result"]["engine"] } else { &value["result"]["engine"] };
        assert_eq!(&value["manifest"]["actual_engine"], engine);
        assert_eq!(value["manifest"]["effective_configuration"]["uncertainties"][0]["constraint"], uncertainty["constraint"]);
    }
}

#[test]
fn admitted_failures_keep_one_error_trace_and_strict_decode_errors_stay_unchanged() {
    let undefined = json!({"op":"ln","value":{"op":"constant","value":-1}});
    for (command, request) in [
        ("simulate", json!({"inputs":[],"outputs":[undefined.clone()],"options":{"trials":1,"seed":"18446744073709551615"}})),
        ("simulate-advanced", json!({"inputs":[],"outputs":[undefined.clone()],"options":{"base":{"trials":1,"seed":"18446744073709551615"}}})),
        ("parameter-grid", json!({"axes":[{"name":"p","values":[1]}],"inputs":[],"outputs":[undefined],"options":{"simulation":{"trials":1}}})),
        ("solve-expr", json!({"model":square(),"start":[],"options":{"method":"projected_gradient"}})),
        ("solve", json!({"model":linear(),"options":{"max_memory_bytes":1}})),
        ("risk", json!({"problem":{"name":"empty","variables":[],"constraints":[],"scenarios":[],"measure":{"kind":"expected_loss"}}})),
        ("solve-conic", json!({"model":{"base":linear(),"second_order_cones":[]},"options":{"max_memory_bytes":1}})),
        ("fit", json!({"values":[0,1,0,1],"families":[{"kind":"bernoulli"},{"kind":"normal"}],"criterion":"aic"})),
        (
            "execute",
            json!({"schema_version":2,"model":{"schema_version":2,"id":"1","revision":"1","name":"bad start","definition":{"kind":"expression","model":square()},"options":{"method":"simplex"}},"context":{"run_id":"r","snapshot_id":"s","snapshot_revision":null,"host":"test","source_revision":null}}),
        ),
    ] {
        let (success, error) = run(command, &request);
        assert!(!success, "{command}: {error}");
        assert!(error["code"].is_string(), "{command}: {error}");
        assert!(error["message"].is_string(), "{command}: {error}");
        assert_eq!(error["manifest"]["entrypoint"], command, "{command}: {error}");
        assert_eq!(error["manifest"]["actual_engine"], "unknown");
        assert_eq!(error["manifest"]["result_location"], "stderr:$");
        assert_eq!(error["manifest"]["snapshot"]["kind"], "diagnostic_request_fingerprint");
        assert_eq!(error["manifest"]["input"]["value"], error["manifest"]["snapshot"]["value"]);
        if command == "simulate" {
            assert_eq!(error["manifest"]["effective_configuration"]["options"]["seed"], "18446744073709551615");
        }
    }
    // Successful parsing of stderr as exactly one JSON value is also a guard
    // against main appending the same error a second time after traced().
    for request in [json!({"inputs":[],"unknown":1}), json!({"inputs":[],"options":{"sampler":{"kind":"legacy","extra":1}}})] {
        let (success, error) = run("simulate-advanced", &request);
        assert!(!success);
        assert_eq!(error["code"], "invalid_json");
        assert!(error.get("manifest").is_none());
    }
}

#[test]
fn robust_process_paths_recheck_original_support_at_large_scales() {
    let mut fixed = linear();
    fixed["variables"][0]["lower"] = json!(1e-9);
    fixed["variables"][0]["upper"] = json!(1e-9);
    fixed["objective"] = json!({"direction":"minimize","offset":0,"linear":[],"quadratic":[]});
    fixed["constraints"] = json!([{"name":"uncertain empty row","terms":[],"relation":"less_equal","rhs":0}]);
    for command in ["robust-box", "robust-budget"] {
        let uncertainty =
            if command == "robust-box" { json!({"constraint":0,"radii":[1e12]}) } else { json!({"constraint":0,"radii":[1e12],"budget":1}) };
        let (success, value) = run(command, &json!({"model":fixed,"uncertainties":[uncertainty.clone()]}));
        if success {
            assert_ne!(value["result"]["feasibility"], "feasible", "{command} accepted original support 1e12*abs(1e-9)=1000 in a <=0 row: {value}");
            assert_ne!(value["result"]["optimality"], "optimal", "{command}: {value}");
            assert_eq!(value["manifest"]["actual_engine"], value["result"]["engine"]);
        } else {
            assert!(value["code"].is_string(), "{command}: {value}");
            assert!(value["message"].is_string());
            assert_eq!(value["manifest"]["entrypoint"], command);
        }
        let mut valid = fixed.clone();
        valid["constraints"][0]["rhs"] = json!(2000);
        let value = ok(command, &json!({"model":valid,"uncertainties":[uncertainty]}));
        assert_eq!(value["result"]["feasibility"], "feasible", "{command}: {value}");
        let x = value["result"]["values"][0].as_f64().unwrap();
        assert!((x - 1e-9).abs() < 1e-15, "Fixed original coordinate was lost: {command}: {value}");
        assert!(1e12 * x.abs() <= 2000.0 + 1e-7);
        assert!(value["counterpart"].is_object());
        assert_eq!(value["manifest"]["actual_engine"], value["result"]["engine"]);

        let mut negative = linear();
        negative["variables"][0]["lower"] = json!(-10);
        negative["variables"][0]["upper"] = json!(10);
        negative["objective"]["direction"] = json!("minimize");
        negative["constraints"] = json!([{"name":"robust signed row","terms":[{"variable":0,"coefficient":1}],"relation":"less_equal","rhs":3}]);
        let uncertainty = if command == "robust-box" { json!({"constraint":0,"radii":[2]}) } else { json!({"constraint":0,"radii":[2],"budget":1}) };
        let value = ok(command, &json!({"model":negative,"uncertainties":[uncertainty]}));
        assert_eq!(value["result"]["feasibility"], "feasible");
        let x = value["result"]["values"][0].as_f64().unwrap();
        assert!((x + 3.0).abs() < 1e-7, "{command}: {value}");
        assert!(x + 2.0 * x.abs() <= 3.0 + 1e-7);
        assert!((value["result"]["objective"].as_f64().unwrap() - x).abs() < 1e-7);
    }
}

#[test]
fn ellipsoid_process_rejects_material_formation_loss_and_keeps_valid_controls() {
    let epsilon = 2.0_f64.powi(-52);
    let large = 2.0_f64.powi(104);
    let values = [large, -(large + 2.0_f64.powi(52)), -1.0];
    let variables: Vec<_> = values.iter().enumerate().map(|(i, x)| json!({"name":format!("x{i}"),"lower":x,"upper":x,"kind":"continuous"})).collect();
    let request = json!({"model":{"schema_version":1,"name":"ellipsoid formation cancellation","variables":variables,
        "objective":{"direction":"minimize","offset":0,"linear":[],"quadratic":[]},"constraints":[{"name":"robust row","terms":[],"relation":"less_equal","rhs":0}]},
        "uncertainties":[{"constraint":0,"factor":[[1.0+epsilon,1,1]],"radius":1.0-epsilon}]});
    // In the exact original expression, (1+e)*2^104 cancels
    // -(2^104+2^52), leaving support 1-e. Forming radius*factor first
    // rounds its leading coefficient to1, leaving only e in the cone row.
    let (success, error) = run("robust-ellipsoid", &request);
    assert!(!success, "The material formation loss needs an explicit range error: {error}");
    assert_eq!(error["code"], "numeric_range", "{error}");
    assert_eq!(error["manifest"]["entrypoint"], "robust-ellipsoid");
    assert_eq!(error["manifest"]["result_location"], "stderr:$");

    let circle = json!({"model":{"schema_version":1,"name":"valid circle","variables":[{"name":"x","lower":-2,"upper":2,"kind":"continuous"},{"name":"y","lower":0.6,"upper":0.6,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1}],"quadratic":[]},"constraints":[{"name":"radius","terms":[],"relation":"less_equal","rhs":1}]},
        "uncertainties":[{"constraint":0,"factor":[[1,0],[0,1]],"radius":1}]});
    let value = ok("robust-ellipsoid", &circle);
    let result = &value["result"]["result"];
    assert_eq!(result["feasibility"], "feasible");
    let x = result["values"][0].as_f64().unwrap();
    let y = result["values"][1].as_f64().unwrap();
    assert!((x - 0.8).abs() < 1e-6, "{value}");
    assert!(x.hypot(y) <= 1.0 + 1e-7);
    assert_eq!(value["manifest"]["actual_engine"], result["engine"]);
    assert!(value["counterpart"]["second_order_cones"].is_array());

    let mut decimal = linear();
    decimal["variables"][0]["upper"] = json!(20);
    decimal["constraints"] = json!([{"name":"decimal support","terms":[],"relation":"less_equal","rhs":0.3}]);
    let value = ok("robust-ellipsoid", &json!({"model":decimal,"uncertainties":[{"constraint":0,"factor":[[0.3]],"radius":0.1}]}));
    let result = &value["result"]["result"];
    assert_eq!(result["feasibility"], "feasible");
    let x = result["values"][0].as_f64().unwrap();
    assert!((x - 10.0).abs() < 1e-5, "{value}");
    assert!(0.1 * (0.3 * x).abs() <= 0.3 + 1e-7);
}

#[test]
fn mixed_integer_process_keeps_small_binary_branches_and_offset_invariance() {
    let mut model = json!({"schema_version":1,"name":"near-integer root must branch",
        "variables":[{"name":"x","lower":0,"upper":1,"kind":"binary"},{"name":"y","lower":0,"upper":1,"kind":"binary"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1e12},{"variable":1,"coefficient":500}],"quadratic":[]},
        "constraints":[{"name":"capacity","terms":[{"variable":0,"coefficient":1e9},{"variable":1,"coefficient":1}],"relation":"less_equal","rhs":1}]});
    // Exhausting the four binary assignments leaves only (0,0) and (0,1)
    // feasible. Their objectives are0 and500, irrespective of the offset.
    // Assertions stay absolute/exact; an offset-scaled tolerance could hide
    // the entire500-unit improvement for the second control.
    for offset in [0.0, 1e16] {
        model["objective"]["offset"] = json!(offset);
        let (success, value) = run("solve", &json!({"model":model}));
        assert!(success, "{value}");
        let result = &value["result"];
        assert_eq!(result["feasibility"], "feasible", "{value}");
        assert_eq!(result["optimality"], "optimal", "{value}");
        assert_eq!(result["values"][0].as_f64().unwrap(), 0.0, "{value}");
        assert_eq!(result["values"][1].as_f64().unwrap(), 1.0, "{value}");
        assert_eq!(result["objective"].as_f64().unwrap(), offset + 500.0, "{value}");
        assert_eq!(value["manifest"]["actual_engine"], result["engine"]);
    }
    model["objective"]["offset"] = json!(0);
    let (success, value) = run("solve", &json!({"model":model,"options":{"max_nodes":1}}));
    assert!(success, "{value}");
    let result = &value["result"];
    assert_eq!(result["termination"], "node_limit", "{value}");
    assert_ne!(result["optimality"], "optimal", "{value}");
    if let Some(gap) = result["relative_gap"].as_f64() {
        assert!(gap > 0.0, "Interrupted search invented a zero optimality gap: {value}");
    }
    if let Some(bound) = result["best_bound"].as_f64() {
        assert!(bound >= 500.0, "The bound excluded the known binary assignment (0,1): {value}");
    }
    if let Some(values) = result["values"].as_array() {
        let x = values[0].as_f64().unwrap();
        let y = values[1].as_f64().unwrap();
        assert!(x == 0.0 || x == 1.0, "{value}");
        assert!(y == 0.0 || y == 1.0, "{value}");
        assert!(1e9 * x + y <= 1.0, "{value}");
        assert_eq!(result["objective"].as_f64().unwrap(), 1e12 * x + 500.0 * y, "{value}");
    }
}
