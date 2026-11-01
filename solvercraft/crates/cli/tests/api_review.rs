#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};

fn run(command: &str, request: &Value) -> (bool, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solvercraft-cli"))
        .args([command, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    serde_json::to_writer(&mut input, request).unwrap();
    drop(input);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let output = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes).unwrap();
        bytes
    });
    let errors = std::thread::spawn(move || {
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
            panic!("CLI API watchdog: {command}");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let output = output.join().unwrap();
    let errors = errors.join().unwrap();
    assert!(output.len() <= 2 * 1024 * 1024 && errors.len() <= 2 * 1024 * 1024);
    if status.success() {
        assert!(errors.is_empty());
    } else {
        assert!(output.is_empty(), "A failed typed request must not emit a partial successful result");
    }
    let result = serde_json::from_slice(if status.success() { &output } else { &errors }).unwrap();
    (status.success(), result)
}
#[test]
fn information_value_roundoff_regression_is_fixed_at_the_process_boundary() {
    let request = json!({"problem":{"state_probabilities":[0.5,0.5000000000005],"action_payoffs":[[1e17,1e17]],"signal_likelihoods":[[1,1]],"acquisition_cost":0,"utility":{"kind":"linear"}}});
    let (success, value) = run("information-value", &request);
    assert!(success, "{value}");
    assert_eq!(value["net_value_of_signal"], 0.0);
    assert_eq!(value["value_of_perfect_information"], 0.0);
    let mut invalid = request;
    invalid["problem"]["utility"]["risk_tolerance"] = json!(100);
    let (success, value) = run("information-value", &invalid);
    assert!(!success);
    assert_eq!(value["code"], "invalid_json");
}
#[test]
fn advanced_simulation_rejects_aggregate_expression_excess() {
    fn expression(depth: usize) -> Value {
        if depth == 0 { json!({"op":"constant","value":1}) } else { json!({"op":"add","left":expression(depth - 1),"right":expression(depth - 1)}) }
    }
    let request =
        json!({"inputs":[{"kind":"constant","value":1}],"outputs":vec![expression(4);4096],"options":{"base":{"trials":1,"retain_samples":false}}});
    let (success, value) = run("simulate-advanced", &request);
    assert!(!success);
    assert_eq!(value["code"], "expression_limit");
}
#[test]
fn simulation_expression_memory_is_checked_before_first_evaluation() {
    // The expression itself is intentionally undefined. A one-byte admission
    // budget must fail first, rather than executing it and returning a domain error.
    let expression = json!({"op":"ln","value":{"op":"constant","value":-1}});
    for (command, request) in [
        ("simulate", json!({"inputs":[{"kind":"constant","value":1}],"outputs":[expression.clone()],"options":{"trials":1,"max_memory_bytes":1}})),
        (
            "simulate-advanced",
            json!({"inputs":[{"kind":"constant","value":1}],"outputs":[expression.clone()],"options":{"base":{"trials":1,"max_memory_bytes":1}}}),
        ),
        (
            "parameter-grid",
            json!({"axes":[{"name":"p","values":[1]}],"inputs":[],"outputs":[expression],"options":{"max_memory_bytes":1,"simulation":{"trials":1}}}),
        ),
    ] {
        let (success, error) = run(command, &request);
        assert!(!success);
        assert_eq!(error["code"], "memory_limit", "{command}: {error}");
    }
}
#[test]
fn numerical_failure_after_valid_admission_retains_diagnostic_run_identity() {
    // The dimensions/probabilities/payoffs are valid. The selected perfect-
    // information spread exceeds finite f64 only after actual utility work.
    let payoffs: Vec<Vec<f64>> = (0..5).map(|action| (0..5).map(|state| if action == state { f64::MAX } else { -f64::MAX }).collect()).collect();
    let request = json!({"problem":{"state_probabilities":vec![0.2;5],"action_payoffs":payoffs,"signal_likelihoods":[vec![1.0;5]],"acquisition_cost":0,"utility":{"kind":"linear"}}});
    let (success, error) = run("information-value", &request);
    assert!(!success);
    assert_eq!(error["code"], "numerical_failure", "{error}");
    assert!(
        error["manifest"].is_object(),
        "A numerical run failed after valid admission but lost its input/configuration/platform identity: {error}"
    );
    let manifest = &error["manifest"];
    assert_eq!(manifest["entrypoint"], "information-value");
    assert_eq!(manifest["platform"]["operating_system"], std::env::consts::OS);
    assert_eq!(manifest["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(manifest["snapshot"]["value"], manifest["input"]["value"]);
    assert!(manifest["effective_configuration"].as_object().is_some_and(|config| !config.is_empty()));
    assert!(manifest["actual_engine"].as_str().is_some_and(|engine| !engine.is_empty() && engine != "auto"));
    let (success, repeated) = run("information-value", &request);
    assert!(!success);
    assert_eq!(repeated["manifest"]["input"], manifest["input"]);
    let mut changed = request;
    changed["problem"]["acquisition_cost"] = json!(1e290);
    let (success, changed) = run("information-value", &changed);
    assert!(!success);
    assert_ne!(changed["manifest"]["input"]["value"], manifest["input"]["value"]);
}
