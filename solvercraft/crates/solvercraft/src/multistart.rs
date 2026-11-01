//! Reproducible configured restarts with one shared evaluation/iteration budget.
use crate::{
    constrained,
    nonlinear::{self, NlpOptions},
};
use serde::{Deserialize, Serialize};
use solvercraft_compile::{ExpressionModel, validate_expression_model};
use solvercraft_types::{
    Direction, Feasibility, Optimality, ProgressEvent, Result, RunControl, SolveOptions, SolveResult, SolveStatistics, SolverError, Termination,
    VariableKind,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MultiStartOptions {
    pub starts: u32,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
}
impl Default for MultiStartOptions {
    fn default() -> Self {
        Self { starts: 16, seed: 0 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiStartResult {
    pub solution: SolveResult,
    pub attempted_starts: u32,
    pub completed_starts: u32,
    pub failed_starts: u32,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn empty() -> SolveResult {
    SolveResult {
        termination: Termination::Completed,
        feasibility: Feasibility::Unknown,
        optimality: Optimality::Unknown,
        values: None,
        objective: None,
        best_bound: None,
        relative_gap: None,
        max_violation: None,
        statistics: SolveStatistics::default(),
        engine: "solvercraft-configured-multistart".into(),
        warnings: vec![],
    }
}
fn better(candidate: &SolveResult, best: &SolveResult, direction: Direction) -> bool {
    let feasible = candidate.feasibility == Feasibility::Feasible;
    let best_feasible = best.feasibility == Feasibility::Feasible;
    if feasible != best_feasible {
        return feasible;
    }
    if !feasible {
        return candidate.max_violation.unwrap_or(f64::INFINITY) < best.max_violation.unwrap_or(f64::INFINITY);
    }
    match (candidate.objective, best.objective) {
        (Some(a), Some(b)) => {
            if direction == Direction::Minimize {
                a < b
            } else {
                a > b
            }
        }
        (Some(_), None) => true,
        _ => false,
    }
}
pub fn solve(
    model: &ExpressionModel,
    options: &SolveOptions,
    nlp: &NlpOptions,
    multi: &MultiStartOptions,
    control: &dyn RunControl,
) -> Result<MultiStartResult> {
    validate_expression_model(model, options)?;
    solve_from(model, &vec![0.0; model.variables.len()], options, nlp, multi, control)
}
/// Use the caller's point first, followed by independent deterministic uniform
/// box starts. Multiple starts require finite bounds; there is no global proof.
pub fn solve_from(
    model: &ExpressionModel,
    start: &[f64],
    options: &SolveOptions,
    nlp: &NlpOptions,
    multi: &MultiStartOptions,
    control: &dyn RunControl,
) -> Result<MultiStartResult> {
    validate_expression_model(model, options)?;
    nlp.validate()?;
    if multi.starts == 0 || multi.starts > 100_000 {
        return Err(error("invalid_options", "multistart count must be in 1..=100000"));
    }
    if start.len() != model.variables.len() || start.iter().any(|v| !v.is_finite()) {
        return Err(error("invalid_values", "multistart starting values must be finite and match model dimensions"));
    }
    if model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(error("unsupported_problem", "local multistart accepts continuous variables only"));
    }
    if multi.starts > 1 && model.variables.iter().any(|v| v.lower.is_none() || v.upper.is_none()) {
        return Err(error("unsupported_problem", "multiple uniform starts require finite variable bounds"));
    }
    let mut best = empty();
    let mut evaluations = 0u64;
    let mut iterations = 0u64;
    let mut attempted = 0u32;
    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut warnings = Vec::new();
    let mut termination = Termination::Completed;
    for trial in 0..multi.starts {
        if control.is_cancelled() {
            termination = Termination::Cancelled;
            break;
        }
        if evaluations >= options.max_evaluations {
            termination = Termination::EvaluationLimit;
            break;
        }
        if iterations >= options.max_iterations {
            termination = Termination::IterationLimit;
            break;
        }
        let values = if trial == 0 {
            start.to_vec()
        } else {
            model
                .variables
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let (lo, hi) = v.lower.zip(v.upper).ok_or_else(|| error("unsupported_problem", "uniform multistart bounds are absent"))?;
                    let u = solvercraft_math::addressed_uniform(multi.seed, i as u64, u64::from(trial), 0);
                    let point = (1.0 - u) * lo + u * hi;
                    if !point.is_finite() {
                        return Err(error("numeric_error", "multistart point overflow"));
                    }
                    Ok(point.max(lo).min(hi))
                })
                .collect::<Result<Vec<_>>>()?
        };
        let remaining = u64::from(multi.starts - trial);
        let per_start = SolveOptions {
            max_evaluations: ((options.max_evaluations - evaluations) / remaining).max(1),
            max_iterations: ((options.max_iterations - iterations) / remaining).max(1),
            ..options.clone()
        };
        attempted += 1;
        let run = if model.constraints.is_empty() {
            nonlinear::solve_from_with_derivatives(model, &values, &per_start, control, nlp.derivative_mode, nlp.finite_difference_step)
        } else {
            constrained::solve_from(model, &values, &per_start, nlp, control)
        };
        match run {
            Ok(candidate) => {
                evaluations = evaluations.saturating_add(candidate.statistics.evaluations);
                iterations = iterations.saturating_add(candidate.statistics.iterations);
                if candidate.termination == Termination::Completed {
                    completed += 1;
                }
                let reason = candidate.termination;
                if better(&candidate, &best, model.direction) || best.values.is_none() {
                    best = candidate;
                }
                if matches!(reason, Termination::Cancelled | Termination::TimeLimit | Termination::MemoryLimit) {
                    termination = reason;
                    break;
                }
            }
            Err(e) if matches!(e.code.as_str(), "domain_error" | "nondifferentiable" | "derivative_unavailable" | "numeric_error") => {
                // A rejected start may already have used evaluations. Reserve
                // its full allowance rather than resetting that budget.
                evaluations = evaluations.saturating_add(per_start.max_evaluations);
                iterations = iterations.saturating_add(per_start.max_iterations);
                failed += 1;
                if warnings.is_empty() {
                    warnings.push("Rejected starts conservatively charge their entire per-start evaluation and iteration allowances; reported counters include these reservations.".into());
                }
                if warnings.len() < 16 {
                    warnings.push(format!("Start {} failed: {e}", trial + 1));
                }
            }
            Err(e) => return Err(e),
        }
        control.progress(&ProgressEvent {
            stage: "multistart".into(),
            iterations,
            evaluations,
            nodes: u64::from(attempted),
            best_objective: best.objective,
        });
    }
    best.termination = termination;
    best.statistics = SolveStatistics { evaluations, iterations, nodes: u64::from(attempted) };
    best.engine = "solvercraft-configured-multistart".into();
    best.best_bound = None;
    best.relative_gap = None;
    if best.optimality == Optimality::Optimal {
        best.optimality = Optimality::BestFound;
    }
    warnings.push(format!("{attempted} configured starts attempted; {completed} completed; {failed} invalid starts. Uniform seed {}. Multiple local runs provide no global guarantee.",multi.seed));
    best.warnings.extend(warnings);
    Ok(MultiStartResult { solution: best, attempted_starts: attempted, completed_starts: completed, failed_starts: failed })
}

#[cfg(test)]
#[path = "multistart_tests.rs"]
mod tests;
