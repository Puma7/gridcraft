//! Process-level tests: spawn the compiled public CLI, not internal dispatch functions.
use serde_json::{Value, json};
use std::error::Error;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const CAPTURE_LIMIT: u64 = 4 * 1024 * 1024;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(20);

fn drain(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(CAPTURE_LIMIT + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn run_os(args: &[std::ffi::OsString], input: Vec<u8>) -> TestResult<Output> {
    let mut child =
        Command::new(env!("CARGO_BIN_EXE_solvercraft-cli")).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut stdin = child.stdin.take().ok_or("missing stdin pipe")?;
    let stdout = child.stdout.take().ok_or("missing stdout pipe")?;
    let stderr = child.stderr.take().ok_or("missing stderr pipe")?;
    let writer = std::thread::spawn(move || {
        let result = stdin.write_all(&input);
        drop(stdin);
        result
    });
    let out_reader = std::thread::spawn(move || drain(stdout));
    let err_reader = std::thread::spawn(move || drain(stderr));
    let started = solvercraft::runtime::monotonic_now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= PROCESS_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = writer.join();
            let _ = out_reader.join();
            let _ = err_reader.join();
            return Err(format!("CLI timed out for {args:?}").into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    match writer.join().map_err(|_| "stdin writer thread panicked")? {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => return Err(e.into()),
    }
    let stdout = out_reader.join().map_err(|_| "stdout reader thread panicked")??;
    let stderr = err_reader.join().map_err(|_| "stderr reader thread panicked")??;
    if stdout.len() as u64 > CAPTURE_LIMIT || stderr.len() as u64 > CAPTURE_LIMIT {
        return Err("CLI output exceeded the test capture budget".into());
    }
    Ok(Output { status, stdout, stderr })
}

fn run_raw(args: &[&str], input: Vec<u8>) -> TestResult<Output> {
    let args: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
    run_os(&args, input)
}

fn run(args: &[&str], input: &Value) -> TestResult<Output> {
    run_raw(args, serde_json::to_vec(input)?)
}

fn success(output: Output) -> TestResult<Value> {
    assert!(output.status.success(), "CLI failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty(), "success wrote diagnostics: {}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert!(value.is_object(), "CLI success must be one JSON object");
    Ok(value)
}

fn failure(output: Output, code: &str) -> TestResult<Value> {
    assert!(!output.status.success(), "CLI unexpectedly succeeded: {}", String::from_utf8_lossy(&output.stdout));
    assert!(output.stdout.is_empty(), "error must not leave partial success JSON on stdout");
    let error: Value = serde_json::from_slice(&output.stderr)?;
    assert_eq!(error["code"], code, "wrong error: {error}");
    assert!(error["message"].as_str().is_some_and(|s| !s.is_empty()));
    Ok(error)
}

fn close(actual: &Value, expected: f64) -> TestResult {
    let actual = actual.as_f64().ok_or("missing numeric result")?;
    assert!((actual - expected).abs() <= 1e-7 * expected.abs().max(1.0), "actual {actual}, expected {expected}");
    Ok(())
}

fn example() -> TestResult<Value> {
    success(run_raw(&["example-lp"], Vec::new())?)
}

fn expression_model() -> Value {
    json!({
        "name":"CLI expression", "direction":"minimize",
        "variables":[{"name":"x","lower":0.0,"upper":10.0,"kind":"continuous"}],
        "objective":{"op":"add","left":{"op":"multiply","left":{"op":"constant","value":2.0},"right":{"op":"variable","index":0}},"right":{"op":"constant","value":1.0}},
        "constraints":[]
    })
}

fn simulation_request() -> Value {
    json!({
        "inputs":[{"kind":"constant","value":2.0},{"kind":"constant","value":3.0}],
        "options":{"trials":8,"seed":"18446744073709551615","retain_samples":true},
        "outputs":[
            {"op":"add","left":{"op":"variable","index":0},"right":{"op":"variable","index":1}},
            {"op":"multiply","left":{"op":"variable","index":0},"right":{"op":"variable","index":1}}
        ]
    })
}

fn scenario_request(measure: Value) -> Value {
    json!({"problem":{
        "name":"CLI risk tradeoff",
        "variables":[{"name":"x","lower":0.0,"upper":2.0,"kind":"continuous"}],
        "constraints":[],
        "scenarios":[
            {"weight":1.0,"offset":0.0,"coefficients":[1.0]},
            {"weight":1.0,"offset":4.0,"coefficients":[-3.0]}
        ],
        "measure":measure
    }})
}

#[test]
fn help_is_json_for_default_and_explicit_help() -> TestResult {
    for args in [&[][..], &["help"][..], &["--help"][..], &["-h"][..]] {
        let value = success(run_raw(args, Vec::new())?)?;
        assert_eq!(value["name"], "Solvercraft");
        assert!(value["usage"].as_array().is_some_and(|x| x.len() >= 9));
    }
    Ok(())
}

#[test]
fn unknown_command_is_a_structured_failure() -> TestResult {
    failure(run_raw(&["solv"], Vec::new())?, "unknown_command")?;
    Ok(())
}

#[test]
fn capabilities_is_an_actual_process_response() -> TestResult {
    let value = success(run_raw(&["capabilities"], Vec::new())?)?;
    assert_eq!(value["schema_version"], 1);
    assert!(value["version"].as_str().is_some_and(|x| !x.is_empty()));
    let text = value.to_string();
    assert!(text.contains("linear") && text.contains("monte_carlo"));
    Ok(())
}

#[test]
fn example_can_be_validated_and_solved_through_stdin() -> TestResult {
    let request = example()?;
    let valid = success(run(&["validate", "-"], &request)?)?;
    assert_eq!(valid["valid"], true);
    assert_eq!(valid["variables"], 2);
    assert_eq!(valid["constraints"], 1);
    let solved = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(solved["result"]["termination"], "completed");
    assert_eq!(solved["result"]["feasibility"], "feasible");
    assert_eq!(solved["result"]["optimality"], "optimal");
    close(&solved["result"]["objective"], 36.0)?;
    close(&solved["result"]["values"][0], 2.0)?;
    close(&solved["result"]["values"][1], 6.0)?;
    assert!(solved["manifest"]["model_fingerprint"].is_string());
    Ok(())
}

#[test]
fn omitted_input_path_reads_stdin() -> TestResult {
    let solved = success(run(&["solve"], &example()?)?)?;
    close(&solved["result"]["objective"], 36.0)
}

#[test]
fn documented_file_input_is_read_by_the_cli() -> TestResult {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lp.json");
    let text = path.to_str().ok_or("example path is not Unicode")?;
    let solved = success(run_raw(&["solve", text], Vec::new())?)?;
    close(&solved["result"]["objective"], 36.0)
}

#[test]
fn missing_file_is_a_structured_input_error() -> TestResult {
    let path = std::env::temp_dir().join(format!("solvercraft-e2e-missing-{}.json", std::process::id()));
    assert!(!path.exists(), "test path unexpectedly exists");
    failure(run_raw(&["solve", path.to_str().ok_or("bad path")?], Vec::new())?, "input")?;
    Ok(())
}

#[test]
fn compile_and_solve_expr_use_the_documented_distinct_envelopes() -> TestResult {
    let model = expression_model();
    let compiled = success(run(&["compile", "-"], &model)?)?;
    assert_eq!(compiled["schema_version"], 1);
    close(&compiled["objective"]["offset"], 1.0)?;
    let solved = success(run(&["solve-expr", "-"], &json!({"model":model}))?)?;
    assert_eq!(solved["result"]["optimality"], "optimal");
    close(&solved["result"]["objective"], 1.0)?;
    close(&solved["result"]["values"][0], 0.0)
}

#[test]
fn malformed_scalar_array_and_trailing_json_are_failures() -> TestResult {
    for input in
        [b"{".as_slice(), b"".as_slice(), b"null".as_slice(), b"42".as_slice(), b"[]".as_slice(), b"{} {}".as_slice(), b"{\"model\": 1}".as_slice()]
    {
        failure(run_raw(&["solve", "-"], input.to_vec())?, "invalid_json")?;
    }
    Ok(())
}

#[test]
fn oversized_input_is_capped_at_the_cli_boundary() -> TestResult {
    // One byte beyond the documented bound; no large solver model is allocated.
    let input = vec![b' '; 16 * 1024 * 1024 + 1];
    failure(run_raw(&["solve", "-"], input)?, "input_limit")?;
    Ok(())
}

#[test]
fn nonfinite_json_number_is_rejected_before_solving() -> TestResult {
    failure(run_raw(&["solve", "-"], b"{\"model\":{\"x\":1e999}}".to_vec())?, "invalid_json")?;
    Ok(())
}

#[test]
fn unknown_method_is_not_silently_replaced() -> TestResult {
    let mut request = example()?;
    request["options"]["method"] = json!("not_a_method");
    failure(run(&["solve", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn unknown_request_field_is_rejected() -> TestResult {
    let mut request = example()?;
    request["modle"] = json!({});
    failure(run(&["solve", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn misspelled_budget_is_not_silently_defaulted() -> TestResult {
    let mut request = example()?;
    request["options"]["max_evaluatons"] = json!(0);
    failure(run(&["solve", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn unknown_model_field_is_rejected() -> TestResult {
    let mut request = example()?;
    request["model"]["constrants"] = json!([]);
    failure(run(&["solve", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn zero_evaluation_budget_is_successfully_framed_as_interruption() -> TestResult {
    let mut request = example()?;
    request["options"]["max_evaluations"] = json!(0);
    let value = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(value["result"]["termination"], "evaluation_limit");
    assert_ne!(value["result"]["optimality"], "optimal");
    assert!(value["result"]["objective"].is_null());
    Ok(())
}

#[test]
fn invalid_variable_reference_is_a_structured_model_error() -> TestResult {
    let mut request = example()?;
    request["model"]["objective"]["linear"][0]["variable"] = json!(999);
    failure(run(&["validate", "-"], &request)?, "invalid_variable")?;
    Ok(())
}

#[test]
fn nonconvex_quadratic_is_explicitly_rejected_without_heuristic_substitution() -> TestResult {
    let mut request = example()?;
    request["model"]["objective"]["quadratic"] = json!([{"left":0,"right":1,"coefficient":1.0}]);
    failure(run(&["solve", "-"], &request)?, "convexity_not_proven")?;
    Ok(())
}

#[test]
fn simulation_multiple_outputs_and_large_seed_roundtrip() -> TestResult {
    let value = success(run(&["simulate", "-"], &simulation_request())?)?;
    assert_eq!(value["options"]["seed"], "18446744073709551615");
    assert_eq!(value["result"]["completed_trials"], 8);
    close(&value["result"]["summaries"][0]["mean"], 5.0)?;
    close(&value["result"]["summaries"][1]["mean"], 6.0)?;
    for row in value["result"]["samples"].as_array().ok_or("samples missing")? {
        assert_eq!(row, &json!([5.0, 6.0]));
    }
    Ok(())
}

#[test]
fn simulation_repeats_in_separate_processes_with_the_same_seed() -> TestResult {
    let request = json!({
        "inputs":[{"kind":"normal","mean":0.0,"std_dev":1.0}],
        "options":{"trials":32,"seed":"9007199254740993","retain_samples":true}
    });
    let first = success(run(&["simulate", "-"], &request)?)?;
    let second = success(run(&["simulate", "-"], &request)?)?;
    assert_eq!(first["result"], second["result"]);
    Ok(())
}

#[test]
fn simulation_risk_summaries_use_upper_loss_tail() -> TestResult {
    let request = json!({
        "inputs":[{"kind":"empirical","values":[0.0,0.0,0.0,10.0]}],
        "options":{"trials":4,"method":"latin_hypercube","retain_samples":true},
        "outputs":[{"op":"variable","index":0},{"op":"negate","value":{"op":"variable","index":0}}]
    });
    let value = success(run(&["simulate", "-"], &request)?)?;
    close(&value["result"]["summaries"][0]["conditional_value_at_risk_95"], 10.0)?;
    close(&value["result"]["summaries"][1]["conditional_value_at_risk_95"], 0.0)?;
    Ok(())
}

#[test]
fn invalid_simulation_controls_have_no_success_payload() -> TestResult {
    let mut request = simulation_request();
    request["options"]["trials"] = json!(0);
    failure(run(&["simulate", "-"], &request)?, "trial_limit")?;
    request["options"]["trials"] = json!(8);
    request["inputs"][0]["kind"] = json!("unknown_distribution");
    failure(run(&["simulate", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn expected_loss_and_cvar_change_the_real_cli_decision() -> TestResult {
    let expected = success(run(&["risk", "-"], &scenario_request(json!({"kind":"expected_loss"})))?)?;
    assert_eq!(expected["solve"]["optimality"], "optimal");
    close(&expected["decision_values"][0], 2.0)?;
    close(&expected["expected_loss"], 0.0)?;
    let cvar = success(run(&["risk", "-"], &scenario_request(json!({"kind":"conditional_value_at_risk","confidence":0.5})))?)?;
    assert_eq!(cvar["solve"]["optimality"], "optimal");
    close(&cvar["decision_values"][0], 1.0)?;
    close(&cvar["cvar"], 1.0)?;
    assert!(cvar["validation_note"].as_str().is_some_and(|s| s.contains("in-sample")));
    Ok(())
}

#[test]
fn invalid_risk_measure_parameter_is_a_structured_error() -> TestResult {
    failure(run(&["risk", "-"], &scenario_request(json!({"kind":"conditional_value_at_risk","confidence":1.0})))?, "invalid_risk_model")?;
    Ok(())
}

#[test]
fn demo_is_actual_optimization_and_simulation() -> TestResult {
    let value = success(run_raw(&["demo"], Vec::new())?)?;
    close(&value["optimization"]["objective"], 36.0)?;
    assert_eq!(value["simulation"]["completed_trials"], 10_000);
    assert_eq!(value["simulation"]["termination"], "completed");
    Ok(())
}

#[test]
fn unexpected_cli_flags_are_not_silently_ignored() -> TestResult {
    failure(run(&["solve", "-", "--max-evaluations=0"], &example()?)?, "invalid_arguments")?;
    failure(run_raw(&["capabilities", "ignored"], Vec::new())?, "invalid_arguments")?;
    Ok(())
}

#[test]
fn nonlinear_expression_cli_identifies_stationarity_without_claiming_optimality() -> TestResult {
    let request = json!({"model":{
        "name":"smooth CLI test","direction":"minimize",
        "variables":[{"name":"x","lower":-2.0,"upper":2.0,"kind":"continuous"}],
        "objective":{"op":"power","base":{"op":"subtract","left":{"op":"exp","value":{"op":"variable","index":0}},"right":{"op":"constant","value":2.0}},"exponent":2},
        "constraints":[]
    },"options":{"method":"projected_gradient"},"start":[0.0]});
    let value = success(run(&["solve-expr", "-"], &request)?)?;
    assert_eq!(value["result"]["feasibility"], "feasible");
    assert_eq!(value["result"]["optimality"], "stationary");
    assert!(value["result"]["engine"].as_str().is_some_and(|x| x.contains("projected-gradient")));
    assert!(value["result"]["objective"].as_f64().is_some_and(|x| x.abs() < 1e-8));
    assert!((value["result"]["values"][0].as_f64().ok_or("missing nonlinear value")? - 2.0_f64.ln()).abs() < 1e-5);
    Ok(())
}

#[test]
fn heuristic_expression_cli_reports_verified_best_found_only() -> TestResult {
    let request = json!({"model":{
        "name":"heuristic CLI test","direction":"minimize",
        "variables":[{"name":"x","lower":-5.0,"upper":5.0,"kind":"continuous"}],
        "objective":{"op":"abs","value":{"op":"subtract","left":{"op":"variable","index":0},"right":{"op":"constant","value":3.0}}},
        "constraints":[]
    },"options":{"method":"differential_evolution","max_evaluations":2000,"seed":"42"}});
    let value = success(run(&["solve-expr", "-"], &request)?)?;
    assert_eq!(value["result"]["feasibility"], "feasible");
    assert_eq!(value["result"]["optimality"], "best_found");
    let x = value["result"]["values"][0].as_f64().ok_or("missing heuristic value")?;
    assert!((-5.0..=5.0).contains(&x));
    close(&value["result"]["objective"], (x - 3.0).abs())?;
    Ok(())
}

#[test]
fn unknown_expression_field_is_rejected() -> TestResult {
    let mut model = expression_model();
    model["objective"] = json!({"op":"variable","index":0,"indx":999});
    failure(run(&["compile", "-"], &model)?, "invalid_json")?;
    Ok(())
}

#[test]
fn unknown_distribution_field_is_rejected() -> TestResult {
    let mut request = simulation_request();
    request["inputs"][0]["valeu"] = json!(9);
    failure(run(&["simulate", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn unknown_risk_field_is_rejected() -> TestResult {
    let mut request = scenario_request(json!({"kind":"conditional_value_at_risk","confidence":0.5}));
    request["problem"]["measure"]["confidance"] = json!(0.95);
    failure(run(&["risk", "-"], &request)?, "invalid_json")?;
    Ok(())
}

#[test]
fn invalid_seed_encodings_are_rejected_in_the_process() -> TestResult {
    for seed in [json!(-1), json!(1.5), json!("abc"), json!("18446744073709551616")] {
        let mut request = example()?;
        request["options"]["seed"] = seed;
        failure(run(&["solve", "-"], &request)?, "invalid_json")?;
    }
    Ok(())
}

#[test]
fn exactly_maximum_input_size_remains_accepted() -> TestResult {
    let mut input = serde_json::to_vec(&example()?)?;
    input.resize(16 * 1024 * 1024, b' ');
    let value = success(run_raw(&["validate", "-"], input)?)?;
    assert_eq!(value["valid"], true);
    Ok(())
}

#[cfg(windows)]
#[test]
fn non_unicode_windows_argument_is_a_structured_error_instead_of_a_panic() -> TestResult {
    use std::os::windows::ffi::OsStringExt;
    let bad = std::ffi::OsString::from_wide(&[0xd800]);
    failure(run_os(&[bad], Vec::new())?, "invalid_arguments")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn non_unicode_unix_argument_is_a_structured_error_instead_of_a_panic() -> TestResult {
    use std::os::unix::ffi::OsStringExt;
    let bad = std::ffi::OsString::from_vec(vec![0xff]);
    failure(run_os(&[bad], Vec::new())?, "invalid_arguments")?;
    Ok(())
}

#[test]
fn mathematical_infeasibility_and_unboundedness_are_json_results_not_parse_errors() -> TestResult {
    let mut request = json!({"model":{
        "schema_version":1,"name":"CLI statuses",
        "variables":[{"name":"x","lower":0.0,"upper":null,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0.0,"linear":[{"variable":0,"coefficient":1.0}],"quadratic":[]},
        "constraints":[]
    }});
    let unbounded = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(unbounded["result"]["optimality"], "unbounded");
    assert!(unbounded["result"]["best_bound"].is_null());
    assert!(unbounded["result"]["relative_gap"].is_null());
    request["model"]["constraints"] = json!([
        {"name":"floor","terms":[{"variable":0,"coefficient":1.0}],"relation":"greater_equal","rhs":2.0},
        {"name":"ceiling","terms":[{"variable":0,"coefficient":1.0}],"relation":"less_equal","rhs":1.0}
    ]);
    let infeasible = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(infeasible["result"]["feasibility"], "infeasible");
    assert_ne!(infeasible["result"]["optimality"], "optimal");
    assert!(infeasible["result"]["values"].is_null());
    assert!(infeasible["result"]["objective"].is_null());
    Ok(())
}

#[test]
fn iteration_and_node_budgets_remain_visible_at_the_process_boundary() -> TestResult {
    let mut request = example()?;
    request["options"]["max_iterations"] = json!(0);
    let iterations = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(iterations["result"]["termination"], "iteration_limit");
    assert_ne!(iterations["result"]["optimality"], "optimal");
    request["options"]["max_iterations"] = json!(100000);
    request["options"]["max_nodes"] = json!(0);
    request["model"]["variables"][0]["kind"] = json!("integer");
    let nodes = success(run(&["solve", "-"], &request)?)?;
    assert_eq!(nodes["result"]["termination"], "node_limit");
    assert_ne!(nodes["result"]["optimality"], "optimal");
    Ok(())
}

#[test]
fn positional_request_arrays_are_rejected_even_when_all_fields_are_present() -> TestResult {
    let request = example()?;
    for payload in [json!([request["model"]]), json!([request["model"], request["options"]])] {
        failure(run(&["solve", "-"], &payload)?, "invalid_json")?;
    }
    let simulation = simulation_request();
    failure(run(&["simulate", "-"], &json!([simulation["inputs"], simulation["options"], simulation["outputs"]]))?, "invalid_json")?;
    Ok(())
}

#[test]
fn duplicate_budget_fields_are_rejected_without_last_value_wins() -> TestResult {
    let request = example()?;
    let input = format!("{{\"model\":{},\"options\":{{\"max_evaluations\":0,\"max_evaluations\":100000}}}}", request["model"]);
    failure(run_raw(&["solve", "-"], input.into_bytes())?, "invalid_json")?;
    Ok(())
}

fn cancellation_expression() -> Value {
    json!({"op":"subtract",
        "left":{"op":"add","left":{"op":"multiply","left":{"op":"constant","value":1e16},"right":{"op":"variable","index":0}},"right":{"op":"variable","index":0}},
        "right":{"op":"multiply","left":{"op":"constant","value":1e16},"right":{"op":"variable","index":0}}
    })
}

#[test]
fn original_expression_objective_is_rechecked_before_publishing_compiled_optimum() -> TestResult {
    let mut model = expression_model();
    model["direction"] = json!("maximize");
    model["variables"][0]["upper"] = json!(1.0);
    model["objective"] = cancellation_expression();
    let error = failure(run(&["solve-expr", "-"], &json!({"model":model}))?, "numerical_failure")?;
    assert!(error["message"].as_str().is_some_and(|s| s.to_lowercase().contains("original")));
    Ok(())
}

#[test]
fn original_expression_constraints_are_rechecked_before_publishing_feasibility() -> TestResult {
    let mut model = expression_model();
    model["direction"] = json!("maximize");
    model["variables"][0]["upper"] = json!(1.0);
    model["objective"] = json!({"op":"variable","index":0});
    model["constraints"] = json!([{
        "name":"roundoff-sensitive floor","left":cancellation_expression(),
        "relation":"greater_equal","right":{"op":"constant","value":0.5}
    }]);
    let error = failure(run(&["solve-expr", "-"], &json!({"model":model}))?, "numerical_failure")?;
    assert!(error["message"].as_str().is_some_and(|s| s.to_lowercase().contains("original")));
    Ok(())
}

#[test]
fn near_integer_relaxation_does_not_close_a_node_before_the_exact_binary_gap() -> TestResult {
    // Exact integer arithmetic enumerates all four points, independently of
    // the solver's integrality tolerance, relaxation or branch selection.
    for (direction, sign, offset) in [("maximize", 1_i128, 0_i128), ("maximize", 1, 10_000_000_000_000_000), ("minimize", -1, 0)] {
        let mut enumerated = Vec::new();
        for x in 0_i128..=1 {
            for y in 0_i128..=1 {
                if 1_000_000_000 * x + y <= 1 {
                    enumerated.push((offset + sign * (1_000_000_000_000 * x + 500 * y), [x, y]));
                }
            }
        }
        let (optimum, point) = if direction == "maximize" {
            enumerated.into_iter().max_by_key(|(objective, _)| *objective)
        } else {
            enumerated.into_iter().min_by_key(|(objective, _)| *objective)
        }
        .ok_or("no enumerated point")?;
        assert_eq!((optimum, point), (offset + sign * 500, [0, 1]));
        let request = json!({"model":{"schema_version":1,"name":"exact binary enumeration",
            "variables":[{"name":"x","lower":0,"upper":1,"kind":"binary"},{"name":"y","lower":0,"upper":1,"kind":"binary"}],
            "objective":{"direction":direction,"offset":offset,"linear":[{"variable":0,"coefficient":sign*1_000_000_000_000},{"variable":1,"coefficient":sign*500}],"quadratic":[]},
            "constraints":[{"name":"capacity","terms":[{"variable":0,"coefficient":1e9},{"variable":1,"coefficient":1}],"relation":"less_equal","rhs":1}]
        }});
        let payload = success(run(&["solve", "-"], &request)?)?;
        let result = &payload["result"];
        assert_eq!(result["feasibility"], "feasible");
        assert_eq!(result["optimality"], "optimal");
        assert_eq!(result["values"], json!([0.0, 1.0]));
        // Every expected integer is exactly representable; a relative tolerance
        // scaled by the offset would hide the entire 500-unit improvement.
        assert_eq!(result["objective"].as_f64(), Some(optimum as f64));
        assert_eq!(result["best_bound"].as_f64(), Some(optimum as f64));
        assert_eq!(result["relative_gap"].as_f64(), Some(0.0));
    }
    Ok(())
}

fn product_residual_model(exponent: u32, quadratic: bool) -> Value {
    let a = ((1_u64 << exponent) + 1) as f64;
    let b = ((1_u64 << (2 * exponent)) + (1_u64 << (exponent + 1))) as f64;
    let x = json!({"op":"variable","index":0});
    let ax = json!({"op":"multiply","left":{"op":"constant","value":a},"right":x});
    let (left, right) = if quadratic {
        (
            json!({"op":"multiply","left":ax,"right":ax}),
            json!({"op":"multiply","left":{"op":"constant","value":b},"right":{"op":"power","base":x,"exponent":2}}),
        )
    } else {
        (
            json!({"op":"multiply","left":ax,"right":{"op":"constant","value":a}}),
            json!({"op":"multiply","left":{"op":"constant","value":b},"right":x}),
        )
    };
    json!({"name":"exact product residual","variables":[{"name":"x","lower":0,"upper":1,"kind":"continuous"}],
        "direction":"maximize","objective":{"op":"subtract","left":left,"right":right},"constraints":[]})
}

#[test]
fn actual_cli_compiler_preserves_exact_integer_product_residuals() -> TestResult {
    for exponent in 27..=30 {
        for quadratic in [false, true] {
            let result = success(run(&["compile", "-"], &product_residual_model(exponent, quadratic))?)?;
            // (2^k+1)^2 - (2^(2k)+2^(k+1)) is exactly one.
            let terms = if quadratic { &result["objective"]["quadratic"] } else { &result["objective"]["linear"] };
            assert_eq!(terms.as_array().ok_or("missing compiled terms")?.len(), 1);
            assert_eq!(terms[0]["coefficient"], 1.0);
        }
    }
    Ok(())
}

#[test]
fn actual_cli_checks_rounded_original_ast_after_exact_compilation() -> TestResult {
    for exponent in 27..=30 {
        let expression = product_residual_model(exponent, false);
        let error = failure(run(&["solve-expr", "-"], &json!({"model":expression}))?, "numerical_failure")?;
        assert!(error["message"].as_str().is_some_and(|s| s.to_lowercase().contains("original")));
    }
    // The explicit algebraic reformulation retains its computed optimum.
    let mut expression = product_residual_model(27, false);
    expression["objective"] = json!({"op":"variable","index":0});
    let output = success(run(&["solve-expr", "-"], &json!({"model":expression}))?)?;
    assert_eq!(output["result"]["values"], json!([1.0]));
    assert_eq!(output["result"]["objective"], 1.0);
    Ok(())
}

#[test]
fn actual_cli_zero_confidence_cvar_uses_the_exact_mean_identity() -> TestResult {
    let problem = json!({"name":"exact zero-tail mean","variables":[{"name":"x","lower":-2,"upper":2,"kind":"continuous"}],"constraints":[],
        "scenarios":[{"weight":1,"offset":-1,"coefficients":[-1]},{"weight":1,"offset":-1,"coefficients":[0]},{"weight":1,"offset":-1,"coefficients":[1]}],
        "measure":{"kind":"conditional_value_at_risk","confidence":0}});
    let output = success(run(&["risk", "-"], &json!({"problem":problem}))?)?;
    let result = &output;
    assert_eq!(result["solve"]["feasibility"], "feasible");
    assert_eq!(result["solve"]["optimality"], "optimal");
    assert_eq!(result["solve"]["objective"], -1.0);
    assert_eq!(result["solve"]["best_bound"], -1.0);
    assert_eq!(result["cvar"], -1.0);
    assert_eq!(result["expected_loss"], -1.0);
    Ok(())
}

#[test]
fn actual_cli_reports_the_correct_variance_between_adjacent_large_losses() -> TestResult {
    let low = 1e10_f64;
    let high = low.next_up();
    let problem = json!({"name":"exact dyadic variance","variables":[{"name":"x","lower":0,"upper":0,"kind":"continuous"}],"constraints":[],
        "scenarios":[{"weight":1,"offset":low,"coefficients":[0]},{"weight":1,"offset":high,"coefficients":[0]}],
        "measure":{"kind":"mean_variance","penalty":0}});
    let output = success(run(&["risk", "-"], &json!({"problem":problem}))?)?;
    let expected = (high - low) * (high - low) / 4.0;
    assert_eq!(output["variance"].as_f64(), Some(expected));
    Ok(())
}

#[test]
fn actual_cli_rejects_an_edge_cost_that_would_change_a_policy_invisibly() -> TestResult {
    for payoff in [1e16, -1e16] {
        let request = json!({"tree":{"root":0,"utility":{"kind":"linear"},"nodes":[
            {"kind":"decision","alternatives":[{"child":1,"cost":1},{"child":1,"cost":0}]},{"kind":"terminal","payoff":payoff}]}});
        let error = failure(run(&["decision-tree", "-"], &request)?, "invalid_decision_tree")?;
        assert!(error["message"].as_str().is_some_and(|s| s.to_lowercase().contains("precision")));
    }
    Ok(())
}

#[test]
fn actual_cli_finite_differences_do_not_invent_stationarity_near_a_bound() -> TestResult {
    let request = json!({"model":{"name":"bound-neighbor FD","variables":[{"name":"x","lower":-3,"upper":3,"kind":"continuous"}],
        "direction":"minimize","objective":{"op":"add","left":{"op":"constant","value":1024},"right":{"op":"multiply","left":{"op":"constant","value":0.25},
            "right":{"op":"power","base":{"op":"add","left":{"op":"variable","index":0},"right":{"op":"constant","value":2}},"exponent":2}}},"constraints":[]},
        "nonlinear":{"derivative_mode":"finite_difference","finite_difference_step":1e-6},"start":[-3]});
    let output = success(run(&["solve-constrained", "-"], &request)?)?;
    let result = &output;
    let x = result["values"][0].as_f64().ok_or("missing nonlinear decision")?;
    assert!((x + 2.0).abs() < 1e-3, "false stationary point {result}");
    assert!((result["objective"].as_f64().ok_or("objective")? - 1024.0).abs() < 1e-7);
    assert_ne!(result["optimality"], "optimal");
    assert!(result["best_bound"].is_null() && result["relative_gap"].is_null());
    Ok(())
}

#[test]
fn actual_cli_inexact_division_cannot_produce_false_original_infeasibility() -> TestResult {
    for denominator in [3.0, 5.0, 7.0] {
        let request = json!({"model":{"name":"division identity","variables":[{"name":"x","lower":1,"upper":2,"kind":"continuous"}],"direction":"minimize",
            "objective":{"op":"constant","value":0},"constraints":[{"name":"identity","left":{"op":"subtract","left":{"op":"multiply",
                "left":{"op":"divide","left":{"op":"variable","index":0},"right":{"op":"constant","value":denominator}},
                "right":{"op":"constant","value":denominator}},"right":{"op":"variable","index":0}},"relation":"equal","right":{"op":"constant","value":0}}]}});
        let error = failure(run(&["solve-expr", "-"], &request)?, "numeric_range")?;
        assert!(error["message"].as_str().is_some_and(|m| m.to_lowercase().contains("division")));
    }
    Ok(())
}
