//! Headless, JSON-native Solvercraft interface.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use solvercraft::{compile::Expr, types::*};
use std::io::{Read, Write};
const MAX_INPUT: u64 = 16 * 1024 * 1024;
// Each invocation creates a statically typed, borrowed configuration. In
// particular it never turns models/results into a duplicate JSON value tree.
macro_rules! configuration {
    ($($field:ident : $kind:ident = $value:expr),+ $(,)?) => {{
        #[derive(Clone, serde::Serialize)]
        struct Configuration<$($kind),+> { $($field: $kind),+ }
        Configuration { $($field: $value),+ }
    }};
}
mod advanced;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SolveRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationRequest {
    model: OptimizationModel,
    id: String,
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
static ERROR_REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Configuration clones here contain borrowed fields and scalar limits only.
/// Keep the request alive while writing its trace, without a JSON value tree or
/// owned copy of model data. The marker also prevents nested scopes/main from
/// appending a second error document after the original structured error.
fn traced<T, C: Clone + Serialize>(manifest: &solvercraft::run_manifest::PreparedManifest<C>, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let outcome = operation();
    if let Err(error) = &outcome
        && !ERROR_REPORTED.load(std::sync::atomic::Ordering::Acquire)
        && let Ok(run) = manifest.clone().complete(error, "unknown", "stderr:$")
    {
        let mut output = std::io::stderr().lock();
        if !ERROR_REPORTED.swap(true, std::sync::atomic::Ordering::AcqRel) {
            let _ = serde_json::to_writer(&mut output, &run);
            let _ = output.write_all(b"\n");
        }
    }
    outcome
}
#[derive(Serialize)]
struct VersionedOutput<'a, O, R> {
    schema_version: u32,
    version: &'static str,
    options: &'a O,
    result: &'a R,
}
#[derive(Serialize)]
struct ExpressionOutput<'a> {
    schema_version: u32,
    version: &'static str,
    model: &'a str,
    options: &'a SolveOptions,
    result: &'a SolveResult,
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
        Some("solve" | "solve-expr" | "validate" | "compile" | "simulate" | "risk" | "serve" | "project-roundtrip" | "migrate-v1") => Some(3),
        None | Some("help" | "--help" | "-h" | "capabilities" | "example-lp" | "demo") => Some(2),
        Some(command) if advanced::supports(command) => Some(3),
        _ => None,
    };
    if max_arguments.is_some_and(|limit| args.len() > limit) {
        return Err(fail("invalid_arguments", "Unexpected command-line arguments; solver options belong in the JSON request"));
    }
    match args.get(1).map(String::as_str) {
        Some("capabilities") => write(&solvercraft::capabilities()),
        Some("example-lp") => write(&example()),
        Some("project-roundtrip") => {
            let library: solvercraft::project::ModelLibrary = read(args.get(2))?;
            library.validate()?;
            write(&library)
        }
        Some("migrate-v1") => {
            let request: MigrationRequest = read(args.get(2))?;
            let bytes = serde_json::to_vec(&request.model).map_err(|e| fail("serialization", e.to_string()))?;
            write(&solvercraft::project::migrate_v1_algebraic(&bytes, request.id, request.options)?)
        }
        Some("serve") => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let config: solvercraft::server::ServerConfig = read(args.get(2))?;
                let server = solvercraft::server::Server::bind(config)?;
                write(
                    &serde_json::json!({"schema_version":1,"service":"solvercraft_json_lines","address":server.address()?.to_string(),"authentication":"required"}),
                )?;
                server.run(&std::sync::atomic::AtomicBool::new(false))
            }
            #[cfg(target_arch = "wasm32")]
            {
                Err(fail("unsupported_host", "The TCP service requires a native host"))
            }
        }
        Some("solve") => {
            let request: SolveRequest = read(args.get(2))?;
            let manifest = solvercraft::run_manifest::prepare("solve", &request, configuration!(options: O = &request.options), MAX_INPUT)?;
            traced(&manifest, || write(&solvercraft::provenance::solve_run_owned(&request.model, &request.options, std::sync::Arc::new(NoControl))?))
        }
        Some("solve-expr") => {
            let request: ExpressionRequest = read(args.get(2))?;
            let nonlinear_defaults = solvercraft::nonlinear::NlpOptions::default();
            let manifest = solvercraft::run_manifest::prepare(
                "solve-expr",
                &request,
                configuration!(options: O = &request.options, start: S = &request.start, nonlinear: N = &nonlinear_defaults),
                MAX_INPUT,
            )?;
            traced(&manifest, || {
                let mut result = match request.options.method {
                    SolveMethod::ProjectedGradient => match request.start.as_deref() {
                        Some(start) => solvercraft::nonlinear::solve_from(&request.model, start, &request.options, &NoControl)?,
                        None => solvercraft::nonlinear::solve(&request.model, &request.options, &NoControl)?,
                    },
                    SolveMethod::DifferentialEvolution => solvercraft::heuristic::solve(&request.model, &request.options, &NoControl)?,
                    _ => {
                        let model = solvercraft::compile::compile(&request.model, &request.options)?;
                        let result = solvercraft::engines::solve_owned(&model, &request.options, std::sync::Arc::new(NoControl))?;
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
                let output = ExpressionOutput {
                    schema_version: SCHEMA_VERSION,
                    version: env!("CARGO_PKG_VERSION"),
                    model: &request.model.name,
                    options: &request.options,
                    result: &result,
                };
                write(&manifest.clone().complete(&output, &result.engine, "stdout:$.result")?)
            })
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
            let manifest = solvercraft::run_manifest::prepare(
                "simulate",
                &request,
                configuration!(options: O = &request.options, output_count: E = request.outputs.len()),
                MAX_INPUT,
            )?;
            traced(&manifest, || {
                advanced::expressions(&request.outputs, request.inputs.len(), request.options.max_memory_bytes)?;
                let result = solvercraft::simulation::simulate(&request.inputs, &request.options, &NoControl, |values| {
                    if request.outputs.is_empty() {
                        return Ok(values.to_vec());
                    }
                    request.outputs.iter().map(|e| solvercraft::compile::evaluate(e, values)).collect()
                })?;
                let output = VersionedOutput {
                    schema_version: SCHEMA_VERSION,
                    version: env!("CARGO_PKG_VERSION"),
                    options: &request.options,
                    result: &result,
                };
                write(&manifest.clone().complete(&output, &result.sampling_version, "stdout:$.result")?)
            })
        }
        Some("risk") => {
            let request: RiskRequest = read(args.get(2))?;
            let manifest = solvercraft::run_manifest::prepare(
                "risk",
                &request,
                configuration!(options: O = &request.options, measure: M = &request.problem.measure),
                MAX_INPUT,
            )?;
            traced(&manifest, || {
                let result = solvercraft::risk::optimize(&request.problem, &request.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.solve.engine, "stdout:$")?)
            })
        }
        Some("demo") => {
            let lp = example();
            let inputs = [Distribution::Normal { mean: 100.0, std_dev: 10.0 }];
            let simulation_options = SimulationOptions { trials: 10_000, seed: 42, ..SimulationOptions::default() };
            #[derive(Serialize)]
            struct DemoRequest<'a> {
                optimization: &'a SolveRequest,
                inputs: &'a [Distribution],
                simulation_options: &'a SimulationOptions,
            }
            #[derive(Serialize)]
            struct DemoOutput<'a> {
                optimization: &'a SolveResult,
                simulation: &'a SimulationResult,
            }
            let request = DemoRequest { optimization: &lp, inputs: &inputs, simulation_options: &simulation_options };
            let manifest = solvercraft::run_manifest::prepare(
                "demo",
                &request,
                configuration!(optimization_options: O = &lp.options, simulation_options: S = &simulation_options),
                MAX_INPUT,
            )?;
            traced(&manifest, || {
                let optimized = solvercraft::solve(&lp.model, &lp.options, &NoControl)?;
                let simulated = solvercraft::simulation::simulate(&inputs, &simulation_options, &NoControl, |x| Ok(x.to_vec()))?;
                let engine = format!("{}; {}", optimized.engine, simulated.sampling_version);
                write(&manifest.clone().complete(&DemoOutput { optimization: &optimized, simulation: &simulated }, &engine, "stdout:$")?)
            })
        }
        None | Some("help" | "--help" | "-h") => write(&serde_json::json!({"name":"Solvercraft","version":env!("CARGO_PKG_VERSION"),
            "usage":["solvercraft-cli capabilities","solvercraft-cli example-lp","solvercraft-cli solve request.json","solvercraft-cli solve-expr expressions.json","solvercraft-cli validate request.json","solvercraft-cli compile expression-model.json","solvercraft-cli simulate simulation.json","solvercraft-cli risk scenarios.json","solvercraft-cli demo"],
            "advanced_commands":advanced::commands(),"other_commands":["serve","project-roundtrip","migrate-v1"],
            "stdin":"Use - or omit the path for JSON from stdin","scope":"Pure-Rust alpha; see per-engine capabilities and recorded validation."})),
        Some(command) if advanced::supports(command) => advanced::run(command, args.get(2)),
        Some(command) => Err(fail("unknown_command", format!("Unknown command '{command}'. Use help for supported commands"))),
    }
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            if !ERROR_REPORTED.load(std::sync::atomic::Ordering::Acquire) {
                let _ = serde_json::to_writer(std::io::stderr().lock(), &e);
                eprintln!();
            }
            std::process::ExitCode::FAILURE
        }
    }
}
