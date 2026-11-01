//! Typed process boundaries for the reusable SDK's advanced workflows.
use super::{SolveRequest, fail, read, write};
use serde::{Deserialize, Serialize};
use solvercraft::{
    self as sc,
    compile::{Expr, ExpressionModel},
    types::*,
};
use std::sync::Arc;
pub fn commands() -> &'static [&'static str] {
    &[
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
    ]
}
pub fn supports(command: &str) -> bool {
    commands().contains(&command)
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QpRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    warm_start: Option<sc::engines::QpWarmStart>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BasisRequest {
    model: OptimizationModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    basis: Option<sc::engines::LpBasis>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConicRequest {
    model: sc::engines::ConicModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiscreteRequest {
    model: sc::engines::DiscreteConicModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConstrainedRequest {
    model: ExpressionModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    nonlinear: sc::nonlinear::NlpOptions,
    #[serde(default)]
    start: Option<Vec<f64>>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiStartRequest {
    model: ExpressionModel,
    #[serde(default)]
    options: SolveOptions,
    #[serde(default)]
    nonlinear: sc::nonlinear::NlpOptions,
    multistart: sc::multistart::MultiStartOptions,
    #[serde(default)]
    start: Option<Vec<f64>>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GlobalRequest {
    model: ExpressionModel,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AdvancedSimulationRequest {
    inputs: Vec<Distribution>,
    #[serde(default)]
    outputs: Vec<Expr>,
    #[serde(default)]
    options: sc::simulation::advanced::AdvancedSimulationOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GridRequest {
    axes: Vec<sc::simulation::grid::ParameterAxis>,
    inputs: Vec<Distribution>,
    #[serde(default)]
    outputs: Vec<Expr>,
    #[serde(default)]
    options: sc::simulation::grid::GridOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FitRequest {
    values: Vec<f64>,
    families: Vec<sc::simulation::fitting::FitFamily>,
    criterion: sc::simulation::fitting::RankingCriterion,
    #[serde(default)]
    options: sc::simulation::fitting::FitOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecourseRequest {
    problem: sc::stochastic::TwoStageProblem,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChanceRequest {
    problem: sc::chance::ChanceProblem,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FiniteRequest {
    model: OptimizationModel,
    specification: sc::finite::FiniteConstraints,
    #[serde(default)]
    options: SolveOptions,
}
fn memory_default() -> u64 {
    256 * 1024 * 1024
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TreeRequest {
    tree: sc::decision::DecisionTree,
    #[serde(default = "memory_default")]
    max_memory_bytes: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InformationRequest {
    problem: sc::decision::InformationProblem,
    #[serde(default = "memory_default")]
    max_memory_bytes: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MultiobjectiveRequest {
    model: OptimizationModel,
    goals: Vec<sc::multiobjective::LinearGoal>,
    method: sc::multiobjective::MultiobjectiveMethod,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SaaRequest {
    problem: sc::saa::SampleAverageProblem,
    sampling: sc::saa::SampleAverageOptions,
    #[serde(default)]
    options: SolveOptions,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RobustRequest<T> {
    model: OptimizationModel,
    uncertainties: Vec<T>,
    #[serde(default)]
    options: SolveOptions,
}

pub(super) fn expressions(expressions: &[Expr], dimensions: usize, max_memory_bytes: u64) -> Result<()> {
    if expressions.len() > 4096 || dimensions > 10000 {
        return Err(fail("model_limit", "Too many simulation expressions or inputs"));
    }
    // Inspect borrowed trees before any temporary model or evaluator allocation.
    sc::compile::validate_expressions(expressions, dimensions, &SolveOptions { max_memory_bytes, ..SolveOptions::default() })
}
pub fn run(command: &str, path: Option<&String>) -> Result<()> {
    match command {
        "execute" => {
            let r: sc::workflow::ExecutionRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.model.options, context: C = &r.context, goal: G = &r.goal, limits: L = &r.limits, nonlinear: N = &r.nonlinear),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || write(&sc::workflow::execute_owned(&r, Arc::new(NoControl))?))
        }
        "solve-qp" => {
            let r: QpRequest = read(path)?;
            let manifest =
                sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options, warm_start: W = &r.warm_start), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_qp_detailed(&r.model, &r.options, &NoControl, r.warm_start.as_ref())?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-basis" => {
            let r: BasisRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options, basis: B = &r.basis), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_lp_with_basis(&r.model, &r.options, &NoControl, r.basis.as_ref())?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-sparse" => {
            let r: SolveRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_sparse_owned(&r.model, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-conic" => {
            let r: ConicRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_socp_owned(&r.model, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-mixed-conic" => {
            let r: ConicRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_mixed_conic_owned(&r.model, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-discrete" => {
            let r: DiscreteRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::engines::solve_discrete_owned(&r.model, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&result, &result.result.engine, "stdout:$")?)
            })
        }
        "solve-constrained" => {
            let r: ConstrainedRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, nonlinear: N = &r.nonlinear, start: S = &r.start),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = match r.start.as_deref() {
                    Some(start) => sc::constrained::solve_from(&r.model, start, &r.options, &r.nonlinear, &NoControl)?,
                    None => sc::constrained::solve(&r.model, &r.options, &r.nonlinear, &NoControl)?,
                };
                write(&manifest.clone().complete(&result, &result.engine, "stdout:$")?)
            })
        }
        "solve-multistart" => {
            let r: MultiStartRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, nonlinear: N = &r.nonlinear, multistart: M = &r.multistart, start: S = &r.start),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = match r.start.as_deref() {
                    Some(start) => sc::multistart::solve_from(&r.model, start, &r.options, &r.nonlinear, &r.multistart, &NoControl)?,
                    None => sc::multistart::solve(&r.model, &r.options, &r.nonlinear, &r.multistart, &NoControl)?,
                };
                write(&manifest.clone().complete(&result, &result.solution.engine, "stdout:$")?)
            })
        }
        "solve-global" => {
            let r: GlobalRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::global::solve_box_polynomial(&r.model, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.solution.engine, "stdout:$")?)
            })
        }
        "simulate-advanced" => {
            let r: AdvancedSimulationRequest = read(path)?;
            let manifest =
                sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options, output_count: E = r.outputs.len()), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                expressions(&r.outputs, r.inputs.len(), r.options.base.max_memory_bytes)?;
                let result = sc::simulation::advanced::simulate_advanced(&r.inputs, &r.options, &NoControl, |x| {
                    if r.outputs.is_empty() { Ok(x.to_vec()) } else { r.outputs.iter().map(|e| sc::compile::evaluate(e, x)).collect() }
                })?;
                let output = super::VersionedOutput { schema_version: 2, version: env!("CARGO_PKG_VERSION"), options: &r.options, result: &result };
                write(&manifest.clone().complete(&output, &result.result.sampling_version, "stdout:$.result")?)
            })
        }
        "parameter-grid" => {
            let r: GridRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, axes: A = &r.axes, output_count: E = r.outputs.len()),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                expressions(
                    &r.outputs,
                    r.axes.len().checked_add(r.inputs.len()).ok_or_else(|| fail("model_limit", "Grid dimension overflow"))?,
                    r.options.max_memory_bytes.min(r.options.simulation.max_memory_bytes),
                )?;
                let mut values = Vec::new();
                let result = sc::simulation::grid::simulate_grid(&r.axes, &r.inputs, &r.options, &NoControl, |parameters, inputs| {
                    values.clear();
                    values.extend_from_slice(parameters);
                    values.extend_from_slice(inputs);
                    if r.outputs.is_empty() { Ok(inputs.to_vec()) } else { r.outputs.iter().map(|e| sc::compile::evaluate(e, &values)).collect() }
                })?;
                write(&manifest.clone().complete(&result, "solvercraft-parameter-grid-v1; sampling versions in runs[].result", "stdout:$")?)
            })
        }
        "fit" => {
            let r: FitRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, families: F = &r.families, criterion: C = &r.criterion),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::simulation::fitting::rank_fits(&r.values, &r.families, r.criterion, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, "solvercraft-likelihood-ranking-v1; fit methods in fits[][1].method", "stdout:$")?)
            })
        }
        "recourse" => {
            let r: RecourseRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::stochastic::solve_two_stage(&r.problem, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.solve.engine, "stdout:$")?)
            })
        }
        "benders" => {
            let r: RecourseRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::benders::solve(&r.problem, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, "solvercraft-complete-recourse-multicut-benders-v1", "stdout:$")?)
            })
        }
        "chance" => {
            let r: ChanceRequest = read(path)?;
            let manifest =
                sc::run_manifest::prepare(command, &r, configuration!(options: O = &r.options, rule: R = &r.problem.rule), super::MAX_INPUT)?;
            super::traced(&manifest, || {
                let result = sc::chance::solve(&r.problem, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.solve.engine, "stdout:$")?)
            })
        }
        "finite" => {
            let r: FiniteRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, specification: S = &r.specification),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::finite::solve(&r.model, &r.specification, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.engine, "stdout:$")?)
            })
        }
        "decision-tree" => {
            let r: TreeRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(max_memory_bytes: M = r.max_memory_bytes, utility: U = &r.tree.utility),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::decision::evaluate(&r.tree, r.max_memory_bytes)?;
                write(&manifest.clone().complete(&result, "solvercraft-decision-tree-backward-induction-v1", "stdout:$")?)
            })
        }
        "information-value" => {
            let r: InformationRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(max_memory_bytes: M = r.max_memory_bytes, utility: U = &r.problem.utility, acquisition_cost: A = r.problem.acquisition_cost,
                    state_count: S = r.problem.state_probabilities.len(), action_count: P = r.problem.action_payoffs.len(), signal_count: L = r.problem.signal_likelihoods.len()),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::decision::information_value(&r.problem, r.max_memory_bytes)?;
                write(&manifest.clone().complete(&result, "solvercraft-finite-information-value-v1", "stdout:$")?)
            })
        }
        "multiobjective" => {
            let r: MultiobjectiveRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, goals: G = &r.goals, method: M = &r.method),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::multiobjective::solve(&r.model, &r.goals, &r.method, &r.options, &NoControl)?;
                let engine = result.stages.first().map_or("solvercraft-multiobjective-v1; no stage executed", |stage| stage.engine.as_str());
                write(&manifest.clone().complete(&result, engine, "stdout:$")?)
            })
        }
        "saa" => {
            let r: SaaRequest = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, sampling: S = &r.sampling, measure: M = &r.problem.measure),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let result = sc::saa::train_and_validate(&r.problem, &r.sampling, &r.options, &NoControl)?;
                write(&manifest.clone().complete(&result, &result.training.solve.engine, "stdout:$")?)
            })
        }
        "robust-box" => {
            let r: RobustRequest<sc::risk::BoxUncertainty> = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, uncertainties: U = &r.uncertainties),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let checked = sc::risk::solve_robust_box_owned(&r.model, &r.uncertainties, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&checked, &checked.result.engine, "stdout:$.result")?)
            })
        }
        "robust-budget" => {
            let r: RobustRequest<sc::risk::BudgetUncertainty> = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, uncertainties: U = &r.uncertainties),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let checked = sc::risk::solve_robust_budget_owned(&r.model, &r.uncertainties, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&checked, &checked.result.engine, "stdout:$.result")?)
            })
        }
        "robust-ellipsoid" => {
            let r: RobustRequest<sc::ellipsoid::EllipsoidUncertainty> = read(path)?;
            let manifest = sc::run_manifest::prepare(
                command,
                &r,
                configuration!(options: O = &r.options, uncertainties: U = &r.uncertainties),
                super::MAX_INPUT,
            )?;
            super::traced(&manifest, || {
                let checked = sc::ellipsoid::solve_owned(&r.model, &r.uncertainties, &r.options, Arc::new(NoControl))?;
                write(&manifest.clone().complete(&checked, &checked.result.result.engine, "stdout:$.result")?)
            })
        }
        _ => Err(fail("unknown_command", "Unknown advanced command")),
    }
}
