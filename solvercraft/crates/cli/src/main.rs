//! Headless, JSON-native Solvercraft interface.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use solvercraft::{compile::Expr, types::*};
use std::io::{Read, Write};
const MAX_INPUT: u64 = 16 * 1024 * 1024;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SolveRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpressionRequest {
    model: solvercraft::compile::ExpressionModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    start: Option<Vec<f64>>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SimulationRequest {
    inputs: Vec<Distribution>,
    #[serde(default)]
    options: SimulationOptions,
    #[serde(default)]
    outputs: Vec<Expr>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RiskRequest {
    problem: solvercraft::risk::ScenarioProblem,
    #[serde(default)]
    options: SolveOptions,
}
fn fail(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn input(path: Option<&String>) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    match path.map(String::as_str) {
        None | Some("-") => {
            std::io::stdin().lock().take(MAX_INPUT + 1).read_to_end(&mut bytes).map_err(|e| fail("input", e.to_string()))?;
        }
        Some(p) => {
            std::fs::File::open(p)
                .map_err(|e| fail("input", e.to_string()))?
                .take(MAX_INPUT + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| fail("input", e.to_string()))?;
        }
    }
    if bytes.len() as u64 > MAX_INPUT {
        return Err(fail("input_limit", "JSON input exceeds 16 MiB"));
    }
    Ok(bytes)
}
fn read<T: serde::de::DeserializeOwned>(path: Option<&String>) -> Result<T> {
    let bytes = input(path)?;
    if bytes.iter().copied().find(|byte| !byte.is_ascii_whitespace()) != Some(b'{') {
        return Err(fail("invalid_json", "The JSON request must be an object, not a positional array or scalar"));
    }
    serde_json::from_slice(&bytes).map_err(|e| fail("invalid_json", e.to_string()))
}
fn write<T: Serialize>(value: &T) -> Result<()> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut output, value).map_err(|e| fail("output", e.to_string()))?;
    output.write_all(b"\n").map_err(|e| fail("output", e.to_string()))
}
fn example() -> SolveRequest {
    SolveRequest {
        model: OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "Wyndor production".into(),
            variables: vec![
                Variable { name: "product_a".into(), lower: Some(0.0), upper: Some(4.0), kind: VariableKind::Continuous },
                Variable { name: "product_b".into(), lower: Some(0.0), upper: Some(6.0), kind: VariableKind::Continuous },
            ],
            objective: Objective {
                direction: Direction::Maximize,
                offset: 0.0,
                linear: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 5.0 }],
                quadratic: vec![],
            },
            constraints: vec![LinearConstraint {
                name: "shared_capacity".into(),
                terms: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 2.0 }],
                relation: Relation::LessEqual,
                rhs: 18.0,
            }],
        },
        options: SolveOptions::default(),
    }
}
fn verify_original_expression_result(model: &solvercraft::compile::ExpressionModel, options: &SolveOptions, result: &mut SolveResult) -> Result<()> {
    let Some(values) = result.values.as_deref() else {
        return Ok(());
    };
    if values.len() != model.variables.len() || values.iter().any(|value| !value.is_finite()) {
        return Err(fail("numerical_failure", "Original expression candidate has invalid decision values"));
    }
    let original = solvercraft::compile::evaluate(&model.objective, values)
        .map_err(|error| fail("numerical_failure", format!("Original objective evaluation failed: {error}")))?;
    let reported = result.objective.ok_or_else(|| fail("numerical_failure", "Original expression candidate has no reported objective"))?;
    if !reported.is_finite() {
        return Err(fail("numerical_failure", "Original expression candidate has a non-finite objective"));
    }
    // Scale by final objective values, never by large intermediate terms:
    // doing the latter would hide catastrophic cancellation such as 1 vs 0.
    let scale = original.abs().max(reported.abs()).max(1.0);
    let difference = (original / scale - reported / scale).abs();
    if difference > options.optimality_tolerance {
        return Err(fail(
            "numerical_failure",
            "Compiled objective disagrees with the original expression at the candidate; rescale or reformulate the model",
        ));
    }
    let mut max_violation = result.max_violation.unwrap_or(0.0);
    for constraint in &model.constraints {
        let left = solvercraft::compile::evaluate(&constraint.left, values)
            .map_err(|error| fail("numerical_failure", format!("Original constraint evaluation failed: {error}")))?;
        let right = solvercraft::compile::evaluate(&constraint.right, values)
            .map_err(|error| fail("numerical_failure", format!("Original constraint evaluation failed: {error}")))?;
        let residual = left - right;
        if !residual.is_finite() {
            return Err(fail("numerical_failure", "Original constraint residual exceeds finite arithmetic"));
        }
        let violation = match constraint.relation {
            Relation::LessEqual => residual.max(0.0),
            Relation::Equal => residual.abs(),
            Relation::GreaterEqual => (-residual).max(0.0),
        };
        if violation > options.feasibility_tolerance {
            return Err(fail("numerical_failure", "Compiled solution violates an original expression constraint"));
        }
        max_violation = max_violation.max(violation);
    }
    result.objective = Some(original);
    result.max_violation = Some(max_violation);
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args_os()
        .map(|arg| arg.into_string().map_err(|_| fail("invalid_arguments", "Command-line arguments must be valid Unicode")))
        .collect::<Result<_>>()?;
    let command = args.get(1).map(String::as_str);
    let max_arguments = match command {
        Some("solve" | "solve-expr" | "validate" | "compile" | "simulate" | "risk") => Some(3),
        None | Some("help" | "--help" | "-h" | "capabilities" | "example-lp" | "demo") => Some(2),
        _ => None,
    };
    if max_arguments.is_some_and(|limit| args.len() > limit) {
        return Err(fail("invalid_arguments", "Unexpected command-line arguments; solver options belong in the JSON request"));
    }
    match args.get(1).map(String::as_str) {
        Some("capabilities") => write(&solvercraft::capabilities()),
        Some("example-lp") => write(&example()),
        Some("solve") => {
            let request: SolveRequest = read(args.get(2))?;
            write(&solvercraft::provenance::solve_run(&request.model, &request.options, &NoControl)?)
        }
        Some("solve-expr") => {
            let request: ExpressionRequest = read(args.get(2))?;
            let mut result = match request.options.method {
                SolveMethod::ProjectedGradient => match request.start.as_deref() {
                    Some(start) => solvercraft::nonlinear::solve_from(&request.model, start, &request.options, &NoControl)?,
                    None => solvercraft::nonlinear::solve(&request.model, &request.options, &NoControl)?,
                },
                SolveMethod::DifferentialEvolution => solvercraft::heuristic::solve(&request.model, &request.options, &NoControl)?,
                _ => {
                    let model = solvercraft::compile::compile(&request.model, &request.options)?;
                    let result = solvercraft::solve(&model, &request.options, &NoControl)?;
                    if let Some(values) = result.values.as_deref() {
                        let checked = solvercraft::model::validate_solution(&model, values, &request.options)?;
                        if !checked.feasible {
                            return Err(fail(
                                "numerical_failure",
                                "Compiled expression candidate failed original bounds and linear-constraint validation",
                            ));
                        }
                    }
                    result
                }
            };
            verify_original_expression_result(&request.model, &request.options, &mut result)?;
            write(
                &serde_json::json!({"schema_version":SCHEMA_VERSION,"version":env!("CARGO_PKG_VERSION"),"model":request.model.name,"options":request.options,"result":result}),
            )
        }
        Some("validate") => {
            let request: SolveRequest = read(args.get(2))?;
            solvercraft::model::validate(&request.model, &request.options)?;
            write(&serde_json::json!({"valid":true,"variables":request.model.variables.len(),"constraints":request.model.constraints.len()}))
        }
        Some("compile") => {
            let request: solvercraft::compile::ExpressionModel = read(args.get(2))?;
            write(&solvercraft::compile::compile(&request, &SolveOptions::default())?)
        }
        Some("simulate") => {
            let request: SimulationRequest = read(args.get(2))?;
            let result = solvercraft::simulation::simulate(&request.inputs, &request.options, &NoControl, |values| {
                if request.outputs.is_empty() {
                    return Ok(values.to_vec());
                }
                request.outputs.iter().map(|e| solvercraft::compile::evaluate(e, values)).collect()
            })?;
            write(&serde_json::json!({"schema_version":SCHEMA_VERSION,"version":env!("CARGO_PKG_VERSION"),"options":request.options,"result":result}))
        }
        Some("risk") => {
            let request: RiskRequest = read(args.get(2))?;
            write(&solvercraft::risk::optimize(&request.problem, &request.options, &NoControl)?)
        }
        Some("demo") => {
            let lp = example();
            let optimized = solvercraft::solve(&lp.model, &lp.options, &NoControl)?;
            let simulated = solvercraft::simulation::simulate(
                &[Distribution::Normal { mean: 100.0, std_dev: 10.0 }],
                &SimulationOptions { trials: 10_000, seed: 42, ..SimulationOptions::default() },
                &NoControl,
                |x| Ok(x.to_vec()),
            )?;
            write(&serde_json::json!({"optimization":optimized,"simulation":simulated}))
        }
        None | Some("help" | "--help" | "-h") => write(&serde_json::json!({"name":"Solvercraft","version":env!("CARGO_PKG_VERSION"),
            "usage":["solvercraft-cli capabilities","solvercraft-cli example-lp","solvercraft-cli solve request.json","solvercraft-cli solve-expr expressions.json","solvercraft-cli validate request.json","solvercraft-cli compile expression-model.json","solvercraft-cli simulate simulation.json","solvercraft-cli risk scenarios.json","solvercraft-cli demo"],
            "stdin":"Use - or omit the path for JSON from stdin","scope":"Pure-Rust alpha; see docs/capabilities and validation."})),
        Some(command) => Err(fail("unknown_command", format!("Unknown command '{command}'. Use help for supported commands"))),
    }
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            let _ = serde_json::to_writer(std::io::stderr().lock(), &e);
            eprintln!();
            std::process::ExitCode::FAILURE
        }
    }
}
