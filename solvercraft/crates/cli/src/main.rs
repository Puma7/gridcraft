//! Headless, JSON-native Solvercraft interface.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use solvercraft::{compile::Expr, types::*};
use std::io::{Read, Write};
const MAX_INPUT: u64 = 16 * 1024 * 1024;
#[derive(Deserialize, Serialize)]
struct SolveRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize)]
struct ExpressionRequest {
    model: solvercraft::compile::ExpressionModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    start: Option<Vec<f64>>,
}
#[derive(Deserialize, Serialize)]
struct SimulationRequest {
    inputs: Vec<Distribution>,
    #[serde(default)]
    options: SimulationOptions,
    #[serde(default)]
    outputs: Vec<Expr>,
}
#[derive(Deserialize, Serialize)]
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
    serde_json::from_slice(&input(path)?).map_err(|e| fail("invalid_json", e.to_string()))
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
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("capabilities") => write(&solvercraft::capabilities()),
        Some("example-lp") => write(&example()),
        Some("solve") => {
            let request: SolveRequest = read(args.get(2))?;
            write(&solvercraft::provenance::solve_run(&request.model, &request.options, &NoControl)?)
        }
        Some("solve-expr") => {
            let request: ExpressionRequest = read(args.get(2))?;
            let result = match request.options.method {
                SolveMethod::ProjectedGradient => match request.start.as_deref() {
                    Some(start) => solvercraft::nonlinear::solve_from(&request.model, start, &request.options, &NoControl)?,
                    None => solvercraft::nonlinear::solve(&request.model, &request.options, &NoControl)?,
                },
                SolveMethod::DifferentialEvolution => solvercraft::heuristic::solve(&request.model, &request.options, &NoControl)?,
                _ => {
                    let model = solvercraft::compile::compile(&request.model, &request.options)?;
                    solvercraft::solve(&model, &request.options, &NoControl)?
                }
            };
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
        _ => write(&serde_json::json!({"name":"Solvercraft","version":env!("CARGO_PKG_VERSION"),
            "usage":["solvercraft-cli capabilities","solvercraft-cli example-lp","solvercraft-cli solve request.json","solvercraft-cli solve-expr expressions.json","solvercraft-cli validate request.json","solvercraft-cli compile expression-model.json","solvercraft-cli simulate simulation.json","solvercraft-cli risk scenarios.json","solvercraft-cli demo"],
            "stdin":"Use - or omit the path for JSON from stdin","scope":"Pure-Rust alpha; see docs/capabilities and validation."})),
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
