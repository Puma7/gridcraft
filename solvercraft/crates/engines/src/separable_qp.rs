//! Analytic separable convex quadratic optimization over variable bounds.
//! No general linear constraints, cross terms, or integer domains are accepted.
use crate::{Budget, empty_result, verified_result};
use solvercraft_model::canonicalize;
use solvercraft_types::*;

pub(crate) fn solve(model: &OptimizationModel, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    if options.method != SolveMethod::Auto {
        return Err(SolverError::new(
            "unsupported_method",
            "Separable quadratic objectives use an analytic bounds-only engine; no projected-gradient implementation is implied.",
        ));
    }
    let normalized = canonicalize(model, options)?;
    if !normalized.constraints.is_empty() || normalized.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(SolverError::new(
            "unsupported_problem",
            "The analytic QP engine supports continuous variables and bounds only, not general constraints or MIQP.",
        ));
    }
    if normalized.objective.quadratic.iter().any(|t| t.left != t.right) {
        return Err(SolverError::new("unsupported_problem", "Cross-variable quadratic terms require a general QP backend."));
    }
    let n = normalized.variables.len();
    let direction = if normalized.objective.direction == Direction::Minimize { 1.0 } else { -1.0 };
    let mut diagonal = vec![0.0; n];
    let mut linear = vec![0.0; n];
    for t in &normalized.objective.quadratic {
        diagonal[t.left] = direction * t.coefficient;
    }
    for t in &normalized.objective.linear {
        linear[t.variable] = direction * t.coefficient;
    }
    if diagonal.iter().any(|a| *a < 0.0) {
        return Err(SolverError::new(
            "unsupported_problem",
            "The separable QP engine requires a convex minimization (or concave maximization) objective.",
        ));
    }
    let mut budget = Budget::new(options, control);
    let mut values = Vec::with_capacity(n);
    let mut unbounded = false;
    let mut stopped = None;
    for (j, var) in normalized.variables.iter().enumerate() {
        if let Err(reason) = budget.step_named("separable_quadratic") {
            stopped = Some(reason);
            break;
        }
        let q = diagonal[j];
        let c = linear[j];
        let mut x = 0.0;
        if q > 0.0 {
            x = -0.5 * c / q;
            if !x.is_finite() {
                match (c > 0.0, var.lower, var.upper) {
                    (true, Some(lo), _) => x = lo,
                    (false, _, Some(hi)) => x = hi,
                    _ => {
                        stopped = Some(Termination::NumericalFailure);
                        break;
                    }
                }
            }
        } else if c > 0.0 {
            if let Some(lo) = var.lower {
                x = lo;
            } else {
                unbounded = true;
            }
        } else if c < 0.0 {
            if let Some(hi) = var.upper {
                x = hi;
            } else {
                unbounded = true;
            }
        }
        if let Some(lo) = var.lower {
            x = x.max(lo);
        }
        if let Some(hi) = var.upper {
            x = x.min(hi);
        }
        values.push(x);
    }
    let mut result = if let Some(reason) = stopped {
        empty_result(reason, Feasibility::Unknown, Optimality::Unknown)
    } else if let Err(reason) = budget.evaluation() {
        empty_result(reason, Feasibility::Unknown, Optimality::Unknown)
    } else {
        verified_result(model, options, values, if unbounded { Optimality::Unbounded } else { Optimality::Optimal })?
    };
    result.engine = "solvercraft-separable-convex-qp-0.1".into();
    result.statistics = budget.statistics;
    result.warnings.retain(|warning| !warning.starts_with("Floating-point simplex"));
    result.warnings.push("Analytic continuous separable convex quadratic engine: variable bounds only; no cross terms, general linear constraints, or integer variables.".into());
    Ok(result)
}
