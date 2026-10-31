//! Bound-constrained smooth nonlinear search using reverse-mode derivatives.
//!
//! This is a projected-gradient method with scaled coordinates, Armijo search
//! and Barzilai-Borwein step estimates. It is not GRG. Its strongest successful
//! status is first-order `Stationary`, not a local or global optimality proof.
//! Cancellation is polled at iteration and expression-evaluation boundaries.
use solvercraft_compile::{Expr, ExpressionModel, evaluate, evaluate_with_gradient};
use solvercraft_types::{
    Direction, Feasibility, Optimality, ProgressEvent, Result, RunControl, SolveMethod, SolveOptions, SolveResult, SolveStatistics, SolverError,
    Termination, VariableKind,
};

const ENGINE: &str = "solvercraft-projected-gradient-ad";
const MAX_LINE_STEPS: usize = 80;

struct Bounds {
    lower: Vec<Option<f64>>,
    upper: Vec<Option<f64>>,
    scales: Vec<f64>,
}
struct Point {
    x: Vec<f64>,
    value: f64,
    phi: f64,
    gradient: Vec<f64>,
}
struct Evaluation<'a> {
    model: &'a ExpressionModel,
    options: &'a SolveOptions,
    control: &'a dyn RunControl,
    count: u64,
}
enum EvalError {
    Stop(Termination),
    Formula(SolverError),
}

fn error(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn direction(model: &ExpressionModel) -> f64 {
    if model.direction == Direction::Minimize { 1.0 } else { -1.0 }
}
fn project(value: f64, lower: Option<f64>, upper: Option<f64>) -> f64 {
    let value = lower.map_or(value, |lo| value.max(lo));
    upper.map_or(value, |hi| value.min(hi))
}
fn expression_nodes(expr: &Expr) -> Result<usize> {
    let mut stack = vec![(expr, 1usize)];
    let mut count = 0usize;
    while let Some((expr, depth)) = stack.pop() {
        count = count.checked_add(1).ok_or_else(|| error("expression_limit", "expression size overflow"))?;
        if depth > solvercraft_compile::MAX_EXPRESSION_DEPTH || count > solvercraft_compile::MAX_EXPRESSION_NODES {
            return Err(error("expression_limit", "nonlinear expression budget exceeded"));
        }
        match expr {
            Expr::Add { left, right }
            | Expr::Subtract { left, right }
            | Expr::Multiply { left, right }
            | Expr::Divide { left, right }
            | Expr::Minimum { left, right }
            | Expr::Maximum { left, right } => {
                stack.push((left, depth + 1));
                stack.push((right, depth + 1));
            }
            Expr::Negate { value }
            | Expr::Sin { value }
            | Expr::Cos { value }
            | Expr::Exp { value }
            | Expr::Ln { value }
            | Expr::Sqrt { value }
            | Expr::Abs { value } => stack.push((value, depth + 1)),
            Expr::Power { base, .. } => stack.push((base, depth + 1)),
            Expr::Constant { .. } | Expr::Variable { .. } => {}
        }
    }
    Ok(count)
}
fn validate(model: &ExpressionModel, start: &[f64], options: &SolveOptions) -> Result<(Bounds, Vec<f64>, bool, usize)> {
    solvercraft_model::validate_options(options)?;
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::ProjectedGradient) {
        return Err(error("unsupported_method", "bound nonlinear search requires auto or projected_gradient"));
    }
    if !model.constraints.is_empty() {
        return Err(error("unsupported_problem", "bound nonlinear search does not support general constraints"));
    }
    if model.variables.len() > solvercraft_model::MAX_VARIABLES {
        return Err(error("model_limit", "too many nonlinear variables"));
    }
    if start.len() != model.variables.len() {
        return Err(error("invalid_values", "nonlinear starting point has the wrong dimension"));
    }
    if model.name.len() > solvercraft_model::MAX_NAME_BYTES {
        return Err(error("model_limit", "nonlinear model name is too long"));
    }
    for (variable, value) in model.variables.iter().zip(start) {
        if variable.kind != VariableKind::Continuous {
            return Err(error("unsupported_problem", "bound nonlinear search accepts continuous variables only"));
        }
        if variable.name.len() > solvercraft_model::MAX_NAME_BYTES {
            return Err(error("model_limit", "nonlinear variable name is too long"));
        }
        if !value.is_finite() {
            return Err(error("non_finite", "starting values must be finite"));
        }
        solvercraft_model::effective_bounds(variable)?;
    }
    let nodes = expression_nodes(&model.objective)?;
    // Covers the AD tape, adjoints, AST scan, and live candidate/gradient vectors.
    let bytes = nodes
        .checked_mul(160)
        .and_then(|v| start.len().checked_mul(256).and_then(|n| v.checked_add(n)))
        .and_then(|v| v.checked_add(4096))
        .ok_or_else(|| error("memory_limit", "nonlinear memory estimate overflow"))?;
    // Return before allocating candidate, bounds, gradient or tape storage.
    if u64::try_from(bytes).map_or(true, |v| v > options.max_memory_bytes) {
        return Ok((Bounds { lower: Vec::new(), upper: Vec::new(), scales: Vec::new() }, Vec::new(), false, bytes));
    }
    let mut lower = Vec::with_capacity(start.len());
    let mut upper = Vec::with_capacity(start.len());
    let mut scales = Vec::with_capacity(start.len());
    let mut values = Vec::with_capacity(start.len());
    let mut clipped = false;
    for (variable, value) in model.variables.iter().zip(start) {
        let (lo, hi) = solvercraft_model::effective_bounds(variable)?;
        let projected = project(*value, lo, hi);
        clipped |= projected != *value;
        let scale = match (lo, hi) {
            (Some(lo), Some(hi)) if lo < hi => {
                let width = hi - lo;
                if width.is_finite() && width > 0.0 { width } else { lo.abs().max(hi.abs()).max(1.0) }
            }
            _ => projected.abs().max(1.0),
        };
        lower.push(lo);
        upper.push(hi);
        scales.push(scale);
        values.push(projected);
    }
    Ok((Bounds { lower, upper, scales }, values, clipped, bytes))
}
impl Evaluation<'_> {
    fn point(&mut self, x: Vec<f64>) -> std::result::Result<Point, EvalError> {
        if self.control.is_cancelled() {
            return Err(EvalError::Stop(Termination::Cancelled));
        }
        if self.count >= self.options.max_evaluations {
            return Err(EvalError::Stop(Termination::EvaluationLimit));
        }
        self.count += 1;
        let (value, gradient) = evaluate_with_gradient(&self.model.objective, &x).map_err(EvalError::Formula)?;
        let sign = direction(self.model);
        let gradient: Vec<f64> = gradient.into_iter().map(|g| sign * g).collect();
        if !value.is_finite() || gradient.iter().any(|v| !v.is_finite()) {
            return Err(EvalError::Formula(error("numeric_error", "nonlinear value or derivative is not finite")));
        }
        Ok(Point { x, value, phi: sign * value, gradient })
    }
}
fn projected_gradient(point: &Point, bounds: &Bounds) -> Vec<f64> {
    point
        .x
        .iter()
        .zip(&point.gradient)
        .zip(bounds.lower.iter().zip(&bounds.upper))
        .map(|((x, g), (lo, hi))| {
            let fixed = lo.zip(*hi).is_some_and(|(lo, hi)| lo == hi);
            let outward = lo.is_some_and(|lo| *x == lo && *g > 0.0) || hi.is_some_and(|hi| *x == hi && *g < 0.0);
            if fixed || outward { 0.0 } else { *g }
        })
        .collect()
}
fn norm(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |a, v| a.max(v.abs()))
}
fn finish(
    point: Option<Point>,
    termination: Termination,
    stationary: bool,
    iterations: u64,
    evaluations: u64,
    mut warnings: Vec<String>,
    bounds: &Bounds,
) -> SolveResult {
    let (values, objective, max_violation, feasibility) = if let Some(p) = point {
        let mut violation = 0.0_f64;
        for ((value, lo), hi) in p.x.iter().zip(&bounds.lower).zip(&bounds.upper) {
            if let Some(lo) = lo {
                violation = violation.max((lo - value).max(0.0));
            }
            if let Some(hi) = hi {
                violation = violation.max((value - hi).max(0.0));
            }
        }
        (Some(p.x), Some(p.value), Some(violation), Feasibility::Feasible)
    } else {
        (None, None, None, Feasibility::Unknown)
    };
    warnings.push("Bound-constrained local search; stationarity is not an optimality guarantee.".into());
    let optimality = if stationary {
        Optimality::Stationary
    } else if values.is_some() {
        Optimality::BestFound
    } else {
        Optimality::Unknown
    };
    SolveResult {
        termination,
        feasibility,
        optimality,
        values,
        objective,
        best_bound: None,
        relative_gap: None,
        max_violation,
        statistics: SolveStatistics { evaluations, iterations, nodes: 0 },
        engine: ENGINE.into(),
        warnings,
    }
}

/// Solve from zero projected into every variable's bounds.
pub fn solve(model: &ExpressionModel, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    if model.variables.len() > solvercraft_model::MAX_VARIABLES {
        return Err(error("model_limit", "too many nonlinear variables"));
    }
    solve_from(model, &vec![0.0; model.variables.len()], options, control)
}

/// Solve from a finite caller-provided point, projecting out-of-bound starts
/// explicitly and recording a warning. General constraints and integer domains
/// are rejected. Domain/derivative failure at the initial point is an error.
pub fn solve_from(model: &ExpressionModel, start: &[f64], options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    let (bounds, x, clipped, memory) = validate(model, start, options)?;
    let mut warnings = Vec::new();
    if clipped {
        warnings.push("Starting point was projected into variable bounds.".into());
    }
    if options.relative_gap != 0.0 {
        warnings.push("Relative gap is unavailable without a valid global bound.".into());
    }
    if u64::try_from(memory).map_or(true, |m| m > options.max_memory_bytes) {
        return Ok(finish(None, Termination::MemoryLimit, false, 0, 0, warnings, &bounds));
    }
    let mut ev = Evaluation { model, options, control, count: 0 };
    if control.is_cancelled() {
        return Ok(finish(None, Termination::Cancelled, false, 0, 0, warnings, &bounds));
    }
    if options.max_evaluations == 0 {
        return Ok(finish(None, Termination::EvaluationLimit, false, 0, 0, warnings, &bounds));
    }
    // If all variables are fixed there is no feasible search direction, and no
    // derivative is required to evaluate this singleton domain.
    if bounds.lower.iter().zip(&bounds.upper).all(|(lo, hi)| lo.zip(*hi).is_some_and(|(lo, hi)| lo == hi)) {
        let value = evaluate(&model.objective, &x)?;
        let point = Point { x, value, phi: direction(model) * value, gradient: vec![] };
        return Ok(finish(Some(point), Termination::Completed, true, 0, 1, warnings, &bounds));
    }
    let mut current = match ev.point(x) {
        Ok(point) => point,
        Err(EvalError::Stop(reason)) => {
            return Ok(finish(None, reason, false, 0, ev.count, warnings, &bounds));
        }
        Err(EvalError::Formula(e)) => return Err(e),
    };
    let mut alpha: Option<f64> = None;
    for iteration in 0..=options.max_iterations {
        if control.is_cancelled() {
            return Ok(finish(Some(current), Termination::Cancelled, false, iteration, ev.count, warnings, &bounds));
        }
        let pg = projected_gradient(&current, &bounds);
        let residual = norm(&pg);
        if residual <= options.optimality_tolerance {
            warnings.push(format!("Original-unit projected-gradient infinity norm: {residual:e}."));
            return Ok(finish(Some(current), Termination::Completed, true, iteration, ev.count, warnings, &bounds));
        }
        if iteration == options.max_iterations {
            return Ok(finish(Some(current), Termination::IterationLimit, false, iteration, ev.count, warnings, &bounds));
        }
        control.progress(&ProgressEvent {
            stage: "nonlinear".into(),
            iterations: iteration,
            evaluations: ev.count,
            nodes: 0,
            best_objective: Some(current.value),
        });
        let scaled: Vec<f64> = pg.iter().zip(&bounds.scales).map(|(g, s)| g * s).collect();
        if scaled.iter().any(|g| !g.is_finite()) || norm(&scaled) == 0.0 {
            warnings.push("Gradient scaling is not numerically representable.".into());
            return Ok(finish(Some(current), Termination::NumericalFailure, false, iteration, ev.count, warnings, &bounds));
        }
        let initial_alpha = 1.0 / norm(&scaled);
        let mut step_alpha = alpha.filter(|a| a.is_finite() && *a > 0.0).unwrap_or(initial_alpha);
        if !step_alpha.is_finite() || step_alpha <= 0.0 {
            warnings.push("Search step cannot be represented numerically.".into());
            return Ok(finish(Some(current), Termination::NumericalFailure, false, iteration, ev.count, warnings, &bounds));
        }
        let mut accepted = None;
        let mut last_error = None;
        for _ in 0..MAX_LINE_STEPS {
            if control.is_cancelled() {
                return Ok(finish(Some(current), Termination::Cancelled, false, iteration, ev.count, warnings, &bounds));
            }
            let candidate: Vec<f64> = current
                .x
                .iter()
                .zip(&scaled)
                .zip(bounds.scales.iter().zip(bounds.lower.iter().zip(&bounds.upper)))
                .map(|((x, g), (scale, (lo, hi)))| project(x + scale * (-step_alpha * g), *lo, *hi))
                .collect();
            if candidate.iter().any(|x| !x.is_finite()) {
                step_alpha *= 0.5;
                continue;
            }
            let decrease: f64 = current.gradient.iter().zip(current.x.iter().zip(&candidate)).map(|(g, (old, new))| g * (old - new)).sum();
            if !decrease.is_finite() || decrease <= 0.0 {
                step_alpha *= 0.5;
                continue;
            }
            let next = match ev.point(candidate) {
                Ok(point) => point,
                Err(EvalError::Stop(reason)) => {
                    return Ok(finish(Some(current), reason, false, iteration, ev.count, warnings, &bounds));
                }
                Err(EvalError::Formula(e)) => {
                    last_error = Some(e);
                    step_alpha *= 0.5;
                    continue;
                }
            };
            if next.phi <= current.phi - 1e-4 * decrease {
                accepted = Some(next);
                break;
            }
            step_alpha *= 0.5;
        }
        let Some(next) = accepted else {
            warnings.push(last_error.map_or_else(
                || "Line search stagnated before satisfying the projected-gradient criterion.".into(),
                |e| format!("Line search could not find a valid improving evaluation: {e}"),
            ));
            return Ok(finish(Some(current), Termination::NumericalFailure, false, iteration, ev.count, warnings, &bounds));
        };
        let next_pg = projected_gradient(&next, &bounds);
        let mut ss = 0.0;
        let mut sy = 0.0;
        for ((((new, old), scale), new_g), old_g) in next.x.iter().zip(&current.x).zip(&bounds.scales).zip(&next_pg).zip(&pg) {
            let s = (new - old) / scale;
            let y = (new_g - old_g) * scale;
            ss += s * s;
            sy += s * y;
        }
        let bb = ss / sy;
        alpha = Some(if sy > 0.0 && bb.is_finite() && bb > 0.0 { bb } else { (step_alpha * 2.0).min(1e300) });
        current = next;
    }
    // The inclusive bounded loop always returns at max_iterations. This fallback
    // remains an ordinary structured result rather than relying on a panic.
    Ok(finish(Some(current), Termination::IterationLimit, false, options.max_iterations, ev.count, warnings, &bounds))
}

#[cfg(test)]
#[path = "nonlinear_tests.rs"]
mod tests;
