//! Continuous smooth constrained local optimization by a safeguarded augmented
//! Lagrangian. Original constraints and a projected KKT residual are checked
//! independently of each inner bound-constrained subproblem.
use crate::nonlinear::{self, DerivativeMode, NlpOptions};
use solvercraft_compile::{
    Expr, ExpressionConstraint, ExpressionModel, evaluate, evaluate_with_gradient, finite_difference_gradient, validate_expression_model,
};
use solvercraft_types::{
    Direction, Feasibility, Optimality, ProgressEvent, Relation, Result, RunControl, SolveMethod, SolveOptions, SolveResult, SolveStatistics,
    SolverError, Termination, VariableKind,
};

fn error(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn c(value: f64) -> Expr {
    Expr::constant(value)
}
fn add(left: Expr, right: Expr) -> Expr {
    Expr::Add { left: Box::new(left), right: Box::new(right) }
}
fn sub(left: Expr, right: Expr) -> Expr {
    Expr::Subtract { left: Box::new(left), right: Box::new(right) }
}
fn mul(left: Expr, right: Expr) -> Expr {
    Expr::Multiply { left: Box::new(left), right: Box::new(right) }
}
fn square(base: Expr) -> Expr {
    Expr::Power { base: Box::new(base), exponent: 2 }
}
fn normalized(row: &ExpressionConstraint) -> Expr {
    if row.relation == Relation::GreaterEqual { sub(row.right.clone(), row.left.clone()) } else { sub(row.left.clone(), row.right.clone()) }
}
fn nodes(expr: &Expr) -> usize {
    match expr {
        Expr::Add { left, right }
        | Expr::Subtract { left, right }
        | Expr::Multiply { left, right }
        | Expr::Divide { left, right }
        | Expr::Minimum { left, right }
        | Expr::Maximum { left, right } => 1 + nodes(left) + nodes(right),
        Expr::Negate { value }
        | Expr::Sin { value }
        | Expr::Cos { value }
        | Expr::Exp { value }
        | Expr::Ln { value }
        | Expr::Sqrt { value }
        | Expr::Abs { value } => 1 + nodes(value),
        Expr::Power { base, .. } => 1 + nodes(base),
        Expr::Constant { .. } | Expr::Variable { .. } => 1,
    }
}
fn balanced_sum(mut terms: Vec<Expr>) -> Expr {
    if terms.is_empty() {
        return c(0.0);
    }
    while terms.len() > 1 {
        let mut next = Vec::with_capacity(terms.len().div_ceil(2));
        let mut iter = terms.into_iter();
        while let Some(a) = iter.next() {
            next.push(if let Some(b) = iter.next() { add(a, b) } else { a });
        }
        terms = next;
    }
    terms.pop().unwrap_or_else(|| c(0.0))
}
fn merit(model: &ExpressionModel, multipliers: &[f64], penalty: f64, options: &SolveOptions) -> Result<ExpressionModel> {
    let original_nodes = model.constraints.iter().fold(nodes(&model.objective), |n, r| n + nodes(&r.left) + nodes(&r.right));
    let merit_nodes = model.constraints.iter().fold(nodes(&model.objective) + 1, |n, r| n + 2 * (nodes(&r.left) + nodes(&r.right)) + 20);
    // AST cloning is bounded before construction. This conservative byte check
    // complements the global node/depth validation of the actual merit below.
    if merit_nodes > solvercraft_compile::MAX_EXPRESSION_NODES {
        return Err(error("expression_limit", "augmented merit exceeds expression node budget"));
    }
    let bytes = merit_nodes
        .checked_add(original_nodes)
        .and_then(|n| n.checked_mul(256))
        .ok_or_else(|| error("memory_limit", "nonlinear merit size overflow"))?;
    if u64::try_from(bytes).map_or(true, |n| n > options.max_memory_bytes / 2) {
        return Err(error("memory_limit", "nonlinear merit exceeds memory budget"));
    }
    let mut terms = Vec::with_capacity(model.constraints.len() + 1);
    terms.push(if model.direction == Direction::Minimize {
        model.objective.clone()
    } else {
        Expr::Negate { value: Box::new(model.objective.clone()) }
    });
    for (row, lambda) in model.constraints.iter().zip(multipliers) {
        let g = normalized(row);
        let term = if row.relation == Relation::Equal {
            add(mul(c(*lambda), g.clone()), mul(c(0.5 * penalty), square(g)))
        } else {
            let positive = Expr::Maximum { left: Box::new(c(0.0)), right: Box::new(add(c(*lambda), mul(c(penalty), g))) };
            mul(c(0.5 / penalty), square(positive))
        };
        terms.push(term);
    }
    let result = ExpressionModel {
        name: model.name.clone(),
        variables: model.variables.clone(),
        direction: Direction::Minimize,
        objective: balanced_sum(terms),
        constraints: vec![],
    };
    validate_expression_model(&result, options)?;
    Ok(result)
}
struct Audit {
    x: Vec<f64>,
    value: f64,
    rows: Vec<f64>,
    gradient: Vec<f64>,
    row_gradients: Vec<Vec<f64>>,
    violation: f64,
}
fn retain_feasible(a: &Audit, best: &mut Option<Audit>, model: &ExpressionModel, options: &SolveOptions) {
    if a.violation > options.feasibility_tolerance {
        return;
    }
    let better = best.as_ref().is_none_or(|b| if model.direction == Direction::Minimize { a.value < b.value } else { a.value > b.value });
    if better {
        *best = Some(Audit { x: a.x.clone(), value: a.value, rows: vec![], gradient: vec![], row_gradients: vec![], violation: a.violation });
    }
}
fn endpoint(latest: Audit, best: Option<Audit>) -> Audit {
    best.unwrap_or(latest)
}
struct Evaluator<'a> {
    options: &'a SolveOptions,
    nlp: &'a NlpOptions,
    control: &'a dyn RunControl,
    lower: Vec<Option<f64>>,
    upper: Vec<Option<f64>>,
    count: u64,
}
impl Evaluator<'_> {
    fn derivative(&mut self, expr: &Expr, x: &[f64]) -> Result<(f64, Vec<f64>)> {
        if self.control.is_cancelled() {
            return Err(error("cancelled", "nonlinear run cancelled"));
        }
        if self.count >= self.options.max_evaluations {
            return Err(error("evaluation_limit", "nonlinear evaluation budget exhausted"));
        }
        if self.nlp.derivative_mode == DerivativeMode::Automatic {
            self.count += 1;
            return evaluate_with_gradient(expr, x);
        }
        let fd = finite_difference_gradient(x, &self.lower, &self.upper, self.nlp.finite_difference_step, |sample| {
            if self.control.is_cancelled() {
                return Err(error("cancelled", "nonlinear run cancelled"));
            }
            if self.count >= self.options.max_evaluations {
                return Err(error("evaluation_limit", "nonlinear evaluation budget exhausted"));
            }
            self.count += 1;
            evaluate(expr, sample)
        })?;
        Ok((fd.value, fd.gradient))
    }
    fn audit(&mut self, model: &ExpressionModel, x: Vec<f64>) -> Result<Audit> {
        let (value, mut gradient) = self.derivative(&model.objective, &x)?;
        if model.direction == Direction::Maximize {
            for g in &mut gradient {
                *g = -*g;
            }
        }
        let mut rows = Vec::with_capacity(model.constraints.len());
        let mut row_gradients = Vec::with_capacity(model.constraints.len());
        let mut violation = 0.0_f64;
        for row in &model.constraints {
            let (left, gl) = self.derivative(&row.left, &x)?;
            let (right, gr) = self.derivative(&row.right, &x)?;
            let sign = if row.relation == Relation::GreaterEqual { -1.0 } else { 1.0 };
            let g = sign * (left - right);
            if !g.is_finite() {
                return Err(error("numeric_error", "nonlinear constraint residual overflow"));
            }
            violation = violation.max(if row.relation == Relation::Equal { g.abs() } else { g.max(0.0) });
            rows.push(g);
            let grad: Vec<_> = gl.iter().zip(gr).map(|(l, r)| sign * (l - r)).collect();
            if grad.iter().any(|v| !v.is_finite()) {
                return Err(error("numeric_error", "nonlinear constraint derivative overflow"));
            }
            row_gradients.push(grad);
        }
        for ((v, lo), hi) in x.iter().zip(&self.lower).zip(&self.upper) {
            violation = violation.max(lo.map_or(0.0, |lo| (lo - v).max(0.0))).max(hi.map_or(0.0, |hi| (v - hi).max(0.0)));
        }
        Ok(Audit { x, value, rows, gradient, row_gradients, violation })
    }
}
fn stop(e: &SolverError) -> Option<Termination> {
    match e.code.as_str() {
        "cancelled" => Some(Termination::Cancelled),
        "evaluation_limit" => Some(Termination::EvaluationLimit),
        "memory_limit" => Some(Termination::MemoryLimit),
        _ => None,
    }
}
fn result(
    audit: Option<Audit>,
    termination: Termination,
    stationary: bool,
    evaluations: u64,
    iterations: u64,
    options: &SolveOptions,
    mut warnings: Vec<String>,
) -> SolveResult {
    let feasible = audit.as_ref().is_some_and(|a| a.violation <= options.feasibility_tolerance);
    warnings.push("Augmented-Lagrangian local search; KKT stationarity does not prove local or global optimality.".into());
    let (values, objective, max_violation) = audit.map_or((None, None, None), |a| (Some(a.x), Some(a.value), Some(a.violation)));
    SolveResult {
        termination,
        feasibility: if feasible { Feasibility::Feasible } else { Feasibility::Unknown },
        optimality: if stationary && feasible {
            Optimality::Stationary
        } else if feasible {
            Optimality::BestFound
        } else {
            Optimality::Unknown
        },
        values,
        objective,
        best_bound: None,
        relative_gap: None,
        max_violation,
        statistics: SolveStatistics { evaluations, iterations, nodes: 0 },
        engine: "solvercraft-augmented-lagrangian".into(),
        warnings,
    }
}
/// Start at zero projected into the variable bounds.
pub fn solve(model: &ExpressionModel, options: &SolveOptions, nlp: &NlpOptions, control: &dyn RunControl) -> Result<SolveResult> {
    validate_expression_model(model, options)?;
    solve_from(model, &vec![0.0; model.variables.len()], options, nlp, control)
}
/// Solve continuous nonlinear equalities/inequalities and bounds. No general
/// infeasibility or optimality proof is inferred from local search failure.
pub fn solve_from(model: &ExpressionModel, start: &[f64], options: &SolveOptions, nlp: &NlpOptions, control: &dyn RunControl) -> Result<SolveResult> {
    validate_expression_model(model, options)?;
    nlp.validate()?;
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::ProjectedGradient) {
        return Err(error("unsupported_method", "constrained nonlinear search requires auto or projected_gradient"));
    }
    if model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(error("unsupported_problem", "constrained nonlinear search accepts continuous variables only"));
    }
    if start.len() != model.variables.len() || start.iter().any(|v| !v.is_finite()) {
        return Err(error("invalid_values", "starting values must be finite and match the model dimension"));
    }
    // The Jacobian and live merit clones are included before their allocations.
    let slots = model
        .constraints
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(model.variables.len()))
        .and_then(|n| n.checked_mul(16))
        .ok_or_else(|| error("memory_limit", "nonlinear Jacobian size overflow"))?;
    if u64::try_from(slots).map_or(true, |n| n > options.max_memory_bytes / 2) {
        return Err(error("memory_limit", "nonlinear Jacobian exceeds memory budget"));
    }
    let mut lower = Vec::with_capacity(start.len());
    let mut upper = Vec::with_capacity(start.len());
    let mut x = Vec::with_capacity(start.len());
    for (variable, value) in model.variables.iter().zip(start) {
        let (lo, hi) = solvercraft_model::effective_bounds(variable)?;
        lower.push(lo);
        upper.push(hi);
        x.push(hi.map_or(lo.map_or(*value, |l| value.max(l)), |h| lo.map_or(*value, |l| value.max(l)).min(h)));
    }
    let mut evaluator = Evaluator { options, nlp, control, lower, upper, count: 0 };
    let mut warnings = Vec::new();
    if nlp.derivative_mode == DerivativeMode::FiniteDifference {
        warnings.push("KKT residual uses approximate finite-difference derivatives.".into());
    }
    let mut latest = match evaluator.audit(model, x) {
        Ok(a) => a,
        Err(e) => return if let Some(reason) = stop(&e) { Ok(result(None, reason, false, evaluator.count, 0, options, warnings)) } else { Err(e) },
    };
    let mut best_feasible = None;
    retain_feasible(&latest, &mut best_feasible, model, options);
    let mut multipliers = vec![0.0; model.constraints.len()];
    let mut penalty = nlp.initial_penalty;
    let mut previous_violation = f64::INFINITY;
    let mut iterations = 0u64;
    for outer in 0..nlp.max_outer_iterations {
        if control.is_cancelled() {
            return Ok(result(Some(endpoint(latest, best_feasible)), Termination::Cancelled, false, evaluator.count, iterations, options, warnings));
        }
        if iterations >= options.max_iterations {
            return Ok(result(
                Some(endpoint(latest, best_feasible)),
                Termination::IterationLimit,
                false,
                evaluator.count,
                iterations,
                options,
                warnings,
            ));
        }
        let inner_model = merit(model, &multipliers, penalty, options)?;
        let inner_options = SolveOptions {
            max_evaluations: options.max_evaluations.saturating_sub(evaluator.count),
            max_iterations: nlp.max_inner_iterations.min(options.max_iterations - iterations),
            optimality_tolerance: (options.optimality_tolerance * 0.2).max(f64::from_bits(1)),
            ..options.clone()
        };
        let inner = nonlinear::solve_from_with_derivatives(
            &inner_model,
            &latest.x,
            &inner_options,
            control,
            nlp.derivative_mode,
            nlp.finite_difference_step,
        )?;
        evaluator.count = evaluator.count.saturating_add(inner.statistics.evaluations);
        iterations = iterations.saturating_add(inner.statistics.iterations).saturating_add(1).min(options.max_iterations);
        let Some(values) = inner.values else {
            return Ok(result(Some(endpoint(latest, best_feasible)), inner.termination, false, evaluator.count, iterations, options, warnings));
        };
        latest = match evaluator.audit(model, values) {
            Ok(a) => a,
            Err(e) => {
                return if let Some(reason) = stop(&e) {
                    Ok(result(Some(endpoint(latest, best_feasible)), reason, false, evaluator.count, iterations, options, warnings))
                } else {
                    Err(e)
                };
            }
        };
        retain_feasible(&latest, &mut best_feasible, model, options);
        for ((lambda, g), row) in multipliers.iter_mut().zip(&latest.rows).zip(&model.constraints) {
            let updated = *lambda + penalty * g;
            if !updated.is_finite() {
                return Err(error("numeric_error", "nonlinear multiplier update overflow"));
            }
            *lambda = if row.relation == Relation::Equal { updated.clamp(-1e12, 1e12) } else { updated.clamp(0.0, 1e12) };
        }
        let mut lagrangian = latest.gradient.clone();
        let mut complementarity = 0.0_f64;
        for (((row, lambda), g), grad) in model.constraints.iter().zip(&multipliers).zip(&latest.rows).zip(&latest.row_gradients) {
            for (target, d) in lagrangian.iter_mut().zip(grad) {
                *target += lambda * d;
            }
            if row.relation != Relation::Equal {
                complementarity = complementarity.max((lambda * g).abs());
            }
        }
        let mut residual = 0.0_f64;
        for (((value, g), lo), hi) in latest.x.iter().zip(lagrangian).zip(&evaluator.lower).zip(&evaluator.upper) {
            if !g.is_finite() {
                return Err(error("numeric_error", "KKT residual is not finite"));
            }
            let inactive =
                lo.zip(*hi).is_some_and(|(l, h)| l == h) || lo.is_some_and(|l| *value == l && g > 0.0) || hi.is_some_and(|h| *value == h && g < 0.0);
            if !inactive {
                residual = residual.max(g.abs());
            }
        }
        control.progress(&ProgressEvent {
            stage: "augmented_lagrangian".into(),
            iterations,
            evaluations: evaluator.count,
            nodes: outer + 1,
            best_objective: Some(latest.value),
        });
        if latest.violation <= options.feasibility_tolerance
            && residual <= options.optimality_tolerance
            && complementarity <= options.optimality_tolerance.max(options.feasibility_tolerance)
        {
            warnings.push(format!("Original-unit KKT residual {residual:e}; complementarity {complementarity:e}."));
            return Ok(result(Some(latest), Termination::Completed, true, evaluator.count, iterations, options, warnings));
        }
        if matches!(inner.termination, Termination::Cancelled | Termination::TimeLimit | Termination::EvaluationLimit | Termination::MemoryLimit) {
            return Ok(result(Some(endpoint(latest, best_feasible)), inner.termination, false, evaluator.count, iterations, options, warnings));
        }
        if latest.violation > 0.25 * previous_violation {
            penalty = (penalty * nlp.penalty_growth).min(1e12);
        }
        previous_violation = latest.violation;
    }
    warnings.push("Outer iteration limit reached without a verified KKT point.".into());
    Ok(result(Some(endpoint(latest, best_feasible)), Termination::IterationLimit, false, evaluator.count, iterations, options, warnings))
}

#[cfg(test)]
#[path = "constrained_tests.rs"]
mod tests;
