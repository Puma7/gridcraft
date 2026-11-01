//! Convex linearly constrained quadratic programming by proximal ADMM.
//! Algorithm reference: https://osqp.org/docs/solver/index.html
//! This is an independent Rust implementation, not an OSQP binding.
use crate::linear_algebra::{self as la, Ldlt};
use crate::{Budget, empty_result, verified_result};
use serde::{Deserialize, Serialize};
use solvercraft_model::{canonicalize, compensated_sum, validate, validate_solution};
use solvercraft_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexityAnalysis {
    pub minimum_eigenvalue_lower_bound: f64,
    pub certificate: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintSource {
    Linear { constraint: usize },
    Bounds { variable: usize },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintDual {
    pub source: ConstraintSource,
    /// Signed multiplier for the minimization-normalized problem.
    /// Upper inequalities are nonnegative; lower inequalities nonpositive.
    pub multiplier: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QpWarmStart {
    pub primal: Vec<f64>,
    /// Original-row units, linear constraints first, then nonfree variable bounds.
    pub dual: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QpCertificate {
    /// Objective values without the irrelevant additive constant, in original direction.
    pub working_primal_objective: f64,
    pub working_dual_objective: Option<f64>,
    pub convexity: ConvexityAnalysis,
    pub primal_residual: f64,
    pub stationarity_residual: f64,
    pub complementarity_residual: f64,
    pub dual_objective: Option<f64>,
    pub absolute_gap: Option<f64>,
    pub relative_gap: Option<f64>,
    pub bound_method: Option<String>,
    pub duals: Vec<ConstraintDual>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QpDetailedResult {
    pub result: SolveResult,
    pub certificate: Option<QpCertificate>,
    pub warm_start: Option<QpWarmStart>,
    pub warm_start_used: bool,
}
pub(crate) struct Row {
    pub coefficients: Vec<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub scale: f64,
    pub source: ConstraintSource,
}
pub(crate) struct Problem {
    pub h: Vec<Vec<f64>>,
    pub q: Vec<f64>,
    pub a: Vec<Vec<f64>>,
    pub rows: Vec<Row>,
    pub curvature: ConvexityAnalysis,
    pub direction: f64,
}
fn finite(values: &[f64]) -> bool {
    values.iter().all(|x| x.is_finite())
}
fn stop(reason: Termination, statistics: SolveStatistics, used: bool) -> QpDetailedResult {
    let mut result = empty_result(reason, Feasibility::Unknown, Optimality::Unknown);
    result.engine = "solvercraft-proximal-admm-qp-0.2".into();
    result.statistics = statistics;
    QpDetailedResult { result, certificate: None, warm_start: None, warm_start_used: used }
}
pub(crate) fn dimensions(n: usize, m: usize, options: &SolveOptions) -> Result<()> {
    if n > la::MAX_CONVEX_VARIABLES {
        return Err(SolverError::new("dimension_limit", "Initial checked dense convex factorization supports at most 512 variables."));
    }
    let entries = n
        .checked_mul(n)
        .and_then(|x| x.checked_mul(12))
        .and_then(|x| n.checked_mul(m).and_then(|a| a.checked_mul(4)).and_then(|a| x.checked_add(a)))
        .and_then(|x| m.checked_mul(32).and_then(|a| x.checked_add(a)))
        .ok_or_else(|| SolverError::new("memory_limit", "Convex matrix size overflow."))?;
    let exact_scratch = (n as u64).checked_add(4).and_then(|count| count.checked_mul(std::mem::size_of::<solvercraft_model::ExactProducts>() as u64));
    if (entries as u64).checked_mul(8).zip(exact_scratch).and_then(|(a, b)| a.checked_add(b)).is_none_or(|bytes| bytes > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Convex matrix/factor working set exceeds memory budget."));
    }
    Ok(())
}
pub(crate) fn build(model: &OptimizationModel, options: &SolveOptions, control: &dyn RunControl) -> Result<Problem> {
    validate(model, options)?;
    if model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(SolverError::new("unsupported_problem", "Continuous convex QP cannot silently relax integer domains."));
    }
    dimensions(model.variables.len(), model.constraints.len() + model.variables.len(), options)?;
    la::check(control)?;
    let normalized = canonicalize(model, options)?;
    let n = model.variables.len();
    let direction = if model.objective.direction == Direction::Minimize { 1.0 } else { -1.0 };
    let mut h = la::zero(n, n)?;
    let mut q = vec![0.0; n];
    for term in &normalized.objective.linear {
        q[term.variable] = direction * term.coefficient;
    }
    for term in &normalized.objective.quadratic {
        let value = direction * term.coefficient;
        if term.left == term.right {
            h[term.left][term.left] = 2.0 * value;
        } else {
            h[term.left][term.right] = value;
            h[term.right][term.left] = value;
        }
    }
    if h.iter().any(|row| !finite(row)) {
        return Err(la::error("Quadratic Hessian exceeds finite arithmetic."));
    }
    let (lower, certificate) = la::curvature(&h, control)?;
    let curvature = ConvexityAnalysis { minimum_eigenvalue_lower_bound: lower, certificate };
    let mut rows = Vec::new();
    for (i, row) in normalized.constraints.iter().enumerate() {
        let mut coefficients = vec![0.0; n];
        for term in &row.terms {
            coefficients[term.variable] = term.coefficient;
        }
        let scale = coefficients.iter().map(|v| v.abs()).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
        let scale = if scale == f64::MIN_POSITIVE { 1.0 } else { scale };
        for coefficient in &mut coefficients {
            *coefficient /= scale;
        }
        let rhs = row.rhs / scale;
        if !rhs.is_finite() {
            return Err(la::error("Constraint scaling overflowed."));
        }
        let (lower, upper) = match row.relation {
            Relation::LessEqual => (None, Some(rhs)),
            Relation::Equal => (Some(rhs), Some(rhs)),
            Relation::GreaterEqual => (Some(rhs), None),
        };
        rows.push(Row { coefficients, lower, upper, scale, source: ConstraintSource::Linear { constraint: i } });
    }
    for (i, var) in model.variables.iter().enumerate() {
        if var.lower.is_some() || var.upper.is_some() {
            let mut coefficients = vec![0.0; n];
            coefficients[i] = 1.0;
            rows.push(Row { coefficients, lower: var.lower, upper: var.upper, scale: 1.0, source: ConstraintSource::Bounds { variable: i } });
        }
    }
    let a = rows.iter().map(|r| r.coefficients.clone()).collect();
    Ok(Problem { h, q, a, rows, curvature, direction })
}
fn clip(value: f64, lower: Option<f64>, upper: Option<f64>) -> f64 {
    let value = lower.map_or(value, |low| value.max(low));
    upper.map_or(value, |high| value.min(high))
}
fn normal_dual(row: &Row, value: f64) -> f64 {
    match (row.lower, row.upper) {
        (None, None) => 0.0,
        (None, Some(_)) => value.max(0.0),
        (Some(_), None) => value.min(0.0),
        _ => value,
    }
}
pub(crate) fn system(problem: &Problem, rho: f64, sigma: f64, control: &dyn RunControl) -> Result<Ldlt> {
    let mut matrix = problem.h.clone();
    for (i, row) in matrix.iter_mut().enumerate() {
        row[i] += sigma;
    }
    for (i, row) in matrix.iter_mut().enumerate() {
        la::check(control)?;
        for (j, value) in row.iter_mut().enumerate() {
            let mut sum = solvercraft_model::ExactProducts::default();
            sum.add(*value)?;
            for a in &problem.a {
                sum.add_triple(rho, a[i], a[j])?;
            }
            *value = sum.finish()?;
            if !value.is_finite() {
                return Err(la::error("ADMM linear system overflowed."));
            }
        }
    }
    la::factor(&matrix, false, control)
}
fn add_stats(budget: &mut Budget<'_>, other: &SolveStatistics) {
    budget.statistics.iterations = budget.statistics.iterations.saturating_add(other.iterations);
    budget.statistics.evaluations = budget.statistics.evaluations.saturating_add(other.evaluations);
    budget.statistics.nodes = budget.statistics.nodes.saturating_add(other.nodes);
}
fn remaining_options(options: &SolveOptions, budget: &Budget<'_>) -> SolveOptions {
    let mut remaining = options.clone();
    remaining.method = SolveMethod::Simplex;
    remaining.max_iterations = options.max_iterations.saturating_sub(budget.statistics.iterations);
    remaining.max_evaluations = options.max_evaluations.saturating_sub(budget.statistics.evaluations);
    remaining
}
fn initial_primal(
    model: &OptimizationModel,
    options: &SolveOptions,
    control: &dyn RunControl,
    warm: Option<&QpWarmStart>,
    budget: &mut Budget<'_>,
) -> Result<std::result::Result<Vec<f64>, SolveResult>> {
    let point = warm.map_or_else(|| model.variables.iter().map(|var| clip(0.0, var.lower, var.upper)).collect(), |w| w.primal.clone());
    if validate_solution(model, &point, options)?.feasible {
        return Ok(Ok(point));
    }
    let mut feasibility = model.clone();
    feasibility.objective.linear.clear();
    feasibility.objective.quadratic.clear();
    feasibility.objective.offset = 0.0;
    feasibility.objective.direction = Direction::Minimize;
    let result = crate::solve(&feasibility, &remaining_options(options, budget), control)?;
    add_stats(budget, &result.statistics);
    if result.feasibility == Feasibility::Feasible
        && let Some(values) = result.values
    {
        return Ok(Ok(values));
    }
    Ok(Err(result))
}
fn certificate(model: &OptimizationModel, problem: &Problem, x: &[f64], y: &[f64], options: &SolveOptions) -> Result<QpCertificate> {
    let validation = validate_solution(model, x, options)?;
    let y = y.iter().zip(&problem.rows).map(|(value, row)| normal_dual(row, *value)).collect::<Vec<_>>();
    let mut gradients = vec![solvercraft_model::ExactProducts::default(); x.len()];
    let mut lagrangian = solvercraft_model::ExactProducts::default();
    for term in &model.objective.linear {
        let coefficient = problem.direction * term.coefficient;
        lagrangian.add_product(coefficient, x[term.variable])?;
        gradients[term.variable].add(coefficient)?;
    }
    for term in &model.objective.quadratic {
        let coefficient = problem.direction * term.coefficient;
        lagrangian.add_triple(coefficient, x[term.left], x[term.right])?;
        gradients[term.left].add_product(coefficient, x[term.right])?;
        gradients[term.right].add_product(coefficient, x[term.left])?;
    }
    let mut complementarity = 0.0_f64;
    let mut duals = Vec::with_capacity(y.len());
    for (row, value) in problem.rows.iter().zip(&y) {
        let multiplier = value / row.scale;
        if !multiplier.is_finite() {
            return Err(la::error("Original-unit dual multiplier overflow."));
        }
        duals.push(ConstraintDual { source: row.source.clone(), multiplier });
        if multiplier == 0.0 {
            continue;
        }
        let residual = match row.source {
            ConstraintSource::Linear { constraint } => {
                let original = &model.constraints[constraint];
                for term in &original.terms {
                    gradients[term.variable].add_product(multiplier, term.coefficient)?;
                    lagrangian.add_triple(multiplier, term.coefficient, x[term.variable])?;
                }
                lagrangian.add_product(-multiplier, original.rhs)?;
                solvercraft_model::evaluate_linear_residual(&original.terms, x, original.rhs)?
            }
            ConstraintSource::Bounds { variable } => {
                let original = &model.variables[variable];
                let endpoint = if multiplier > 0.0 { original.upper } else { original.lower }
                    .ok_or_else(|| la::error("Dual points into an unbounded original support side."))?;
                gradients[variable].add(multiplier)?;
                lagrangian.add_product(multiplier, x[variable])?;
                lagrangian.add_product(-multiplier, endpoint)?;
                x[variable] - endpoint
            }
        };
        complementarity = complementarity.max((multiplier * residual).abs());
    }
    let working_primal = working_value(model, x, options)?;
    let score = problem.direction * working_primal;
    let lagrangian = lagrangian.enclosure()?.0;
    let mut gradient = Vec::with_capacity(x.len());
    let mut gradient_errors = Vec::with_capacity(x.len());
    for accumulator in gradients {
        let (low, high) = accumulator.enclosure()?;
        let value = accumulator.finish()?;
        let error = (value - low).abs().max((high - value).abs());
        gradient.push(value);
        gradient_errors.push(if error == 0.0 { 0.0 } else { error.next_up() });
    }
    let objective_magnitude = compensated_sum(
        model
            .objective
            .linear
            .iter()
            .map(|t| (t.coefficient * x[t.variable]).abs())
            .chain(model.objective.quadratic.iter().map(|t| (t.coefficient * x[t.left] * x[t.right]).abs())),
    )?;
    let dual_magnitude = compensated_sum(
        problem
            .rows
            .iter()
            .zip(&y)
            .map(|(row, y)| {
                let scale = compensated_sum(row.coefficients.iter().zip(x).map(|(a, x)| (a * x).abs()))?;
                let bound = if *y >= 0.0 { row.upper } else { row.lower };
                Ok(y.abs() * (scale + bound.unwrap_or(0.0).abs()))
            })
            .collect::<Result<Vec<_>>>()?,
    )?;
    let mut lower = None;
    let mut method = None;
    if problem.curvature.minimum_eigenvalue_lower_bound > 0.0 {
        let residual_upper = gradient.iter().zip(&gradient_errors).map(|(g, e)| g.abs() + e).collect::<Vec<_>>();
        let residual_norm_sq = la::dot(&residual_upper, &residual_upper)?;
        lower = Some(lagrangian - residual_norm_sq / (2.0 * problem.curvature.minimum_eigenvalue_lower_bound));
        method = Some("strong-convexity Lagrangian residual bound".into());
    } else {
        let mut corrections = Some(Vec::new());
        for (((var, g), e), point) in model.variables.iter().zip(&gradient).zip(&gradient_errors).zip(x) {
            let delta = match (var.lower, var.upper) {
                (Some(low), Some(high)) => [(*g - e) * (low - point), (*g + e) * (low - point), (*g - e) * (high - point), (*g + e) * (high - point)]
                    .into_iter()
                    .reduce(f64::min),
                (Some(low), None) if *g - e >= 0.0 => Some(((*g - e) * (low - point)).min((*g + e) * (low - point))),
                (None, Some(high)) if *g + e <= 0.0 => Some(((*g - e) * (high - point)).min((*g + e) * (high - point))),
                (None, None) if *g == 0.0 && *e == 0.0 => Some(0.0),
                _ => None,
            };
            if let (Some(corrections), Some(delta)) = (&mut corrections, delta) {
                corrections.push(delta);
            } else {
                corrections = None;
                break;
            }
        }
        if let Some(corrections) = corrections {
            lower = Some(compensated_sum(std::iter::once(lagrangian).chain(corrections))?);
            method = Some("convex Lagrangian affine bound over original variable box".into());
        }
    }
    if let Some(bound) = &mut lower {
        let scale = objective_magnitude + dual_magnitude + lagrangian.abs() + bound.abs() + la::norm(&gradient) * (1.0 + la::norm(x));
        let margin = 256.0 * f64::EPSILON * (1 + x.len() + y.len()) as f64 * scale.max(f64::MIN_POSITIVE);
        *bound = (*bound - margin).next_down();
        if !bound.is_finite() || *bound > score {
            lower = None;
            method = None;
        }
    }
    let absolute_gap = lower.map(|bound| score - bound);
    let relative_gap = lower.zip(absolute_gap).map(|(bound, g)| g.max(0.0) / score.abs().max(bound.abs()).max(1.0));
    Ok(QpCertificate {
        working_primal_objective: working_primal,
        working_dual_objective: lower.map(|v| problem.direction * v),
        convexity: problem.curvature.clone(),
        primal_residual: validation.max_violation,
        stationarity_residual: la::norm(&gradient),
        complementarity_residual: complementarity,
        dual_objective: lower.and_then(|v| crate::reported_bound(model, v).map(|(value, _)| value)),
        absolute_gap,
        relative_gap,
        bound_method: method,
        duals,
    })
}
fn certified(c: &QpCertificate, objective: f64, options: &SolveOptions) -> bool {
    c.primal_residual <= options.feasibility_tolerance
        && c.stationarity_residual <= options.optimality_tolerance
        && c.complementarity_residual <= options.optimality_tolerance * (1.0 + objective.abs())
        && c.absolute_gap.is_some_and(|gap| gap >= 0.0 && gap <= options.optimality_tolerance * (1.0 + objective.abs()))
}
#[allow(clippy::too_many_arguments)] // Private immutable solve context plus current primal/dual state.
fn details(
    model: &OptimizationModel,
    problem: &Problem,
    x: Vec<f64>,
    y: Vec<f64>,
    options: &SolveOptions,
    termination: Termination,
    budget: &Budget<'_>,
    warm_used: bool,
) -> Result<QpDetailedResult> {
    let mut result = verified_result(model, options, x.clone(), Optimality::BestFound)?;
    result.termination = termination;
    result.engine = "solvercraft-proximal-admm-qp-0.2".into();
    result.statistics = budget.statistics.clone();
    result.best_bound = None;
    result.relative_gap = None;
    result.warnings.retain(|w| !w.starts_with("Floating-point simplex"));
    let dual_state = y.iter().zip(&problem.rows).map(|(value, row)| value / row.scale).collect::<Vec<_>>();
    let state = finite(&dual_state).then(|| QpWarmStart { primal: x.clone(), dual: dual_state });
    if state.is_none() {
        result.warnings.push("Original-unit dual warm state exceeds finite arithmetic and is omitted.".into());
    }
    let c = match certificate(model, problem, &x, &y, options) {
        Ok(c) => c,
        Err(error) => {
            if result.termination == Termination::Completed {
                result.termination = Termination::NumericalFailure;
            }
            result.warnings.push(format!("Verified QP candidate retained but dual-certificate arithmetic failed: {error}"));
            return Ok(QpDetailedResult { result, certificate: None, warm_start: state, warm_start_used: warm_used });
        }
    };
    if result.feasibility == Feasibility::Feasible {
        if certified(&c, c.working_primal_objective, options) {
            result.optimality = Optimality::Optimal;
            if result.termination != Termination::Cancelled {
                result.termination = Termination::Completed;
            }
        } else if c.stationarity_residual <= options.optimality_tolerance && c.primal_residual <= options.feasibility_tolerance {
            result.optimality = Optimality::Stationary;
        }
        if c.absolute_gap.is_some_and(|g| g >= 0.0) {
            result.best_bound = c.dual_objective;
            result.relative_gap = c.relative_gap;
        }
    }
    result.warnings.push("Convex QP uses checked sufficient curvature conditions and floating-point primal/dual certificates; no exact rational certificate is claimed.".into());
    Ok(QpDetailedResult { result, certificate: Some(c), warm_start: state, warm_start_used: warm_used })
}
/// Detailed continuous convex QP solve, including a real primal/dual warm start.
/// A general stationary point is not labeled optimal without a checked finite gap.
fn solve_qp_impl(
    model: &OptimizationModel,
    options: &SolveOptions,
    control: &dyn RunControl,
    warm: Option<&QpWarmStart>,
) -> Result<QpDetailedResult> {
    if options.method != SolveMethod::Auto {
        return Err(SolverError::new(
            "unsupported_method",
            "The convex QP engine uses proximal ADMM; select auto or call this explicit engine API with auto options.",
        ));
    }
    let used = warm.is_some();
    let mut budget = Budget::new(options, control);
    if control.is_cancelled() {
        return Ok(stop(Termination::Cancelled, budget.statistics, used));
    }
    let problem = build(model, options, control)?;
    if let Some(state) = warm
        && (state.primal.len() != model.variables.len() || state.dual.len() != problem.rows.len() || !finite(&state.primal) || !finite(&state.dual))
    {
        return Err(SolverError::new("warm_start_shape", "QP warm start must have finite primal/model and dual/constraint dimensions."));
    }
    if options.max_iterations == 0 {
        return Ok(stop(Termination::IterationLimit, budget.statistics, used));
    }
    if options.max_evaluations == 0 {
        return Ok(stop(Termination::EvaluationLimit, budget.statistics, used));
    }
    let mut x = match initial_primal(model, options, control, warm, &mut budget)? {
        Ok(point) => point,
        Err(mut result) => {
            result.engine = "solvercraft-proximal-admm-qp-0.2".into();
            return Ok(QpDetailedResult { result, certificate: None, warm_start: None, warm_start_used: used });
        }
    };
    let mut y = warm.map_or_else(|| vec![0.0; problem.rows.len()], |w| w.dual.iter().zip(&problem.rows).map(|(v, row)| v * row.scale).collect());
    if !finite(&y) {
        return Err(SolverError::new("warm_start_scale", "Scaling finite original-unit warm duals overflowed; rescale or omit the warm start."));
    }
    let mut z = la::multiply(&problem.a, &x)?.iter().zip(&problem.rows).map(|(v, row)| clip(*v, row.lower, row.upper)).collect::<Vec<_>>();
    let mut rho = 1.0;
    let sigma = 1e-6;
    let mut last = details(model, &problem, x.clone(), y.clone(), options, Termination::IterationLimit, &budget, used)?;
    if last.result.optimality == Optimality::Optimal {
        return Ok(last);
    }
    let mut factor = match system(&problem, rho, sigma, control) {
        Ok(value) => value,
        Err(error) if error.code == "cancelled" => {
            last.result.termination = Termination::Cancelled;
            return Ok(last);
        }
        Err(error) => return Err(error),
    };
    let mut arithmetic_work = 0_u64;
    let per_iteration_work =
        (x.len() as u64).saturating_mul(x.len() as u64).saturating_add((problem.a.len() as u64).saturating_mul(x.len() as u64).saturating_mul(4));
    let termination = loop {
        if let Err(reason) = budget.step_named("convex_qp") {
            break reason;
        }
        if let Err(reason) = budget.evaluation() {
            break reason;
        }
        arithmetic_work = arithmetic_work.saturating_add(per_iteration_work);
        if arithmetic_work > 300_000_000 {
            last.result.warnings.push("Dense convex arithmetic work limit reached.".into());
            break Termination::IterationLimit;
        }
        let at = la::transpose_multiply(&problem.a, &z.iter().zip(&y).map(|(v, d)| rho * v - d).collect::<Vec<_>>(), x.len())?;
        let rhs = x.iter().zip(&problem.q).zip(at).map(|((old, q), a)| sigma * old - q + a).collect::<Vec<_>>();
        let next = match factor.solve(&rhs, control) {
            Ok(value) => value,
            Err(error) if error.code == "cancelled" => break Termination::Cancelled,
            Err(error) => return Err(error),
        };
        let ax = la::multiply(&problem.a, &next)?;
        for i in 0..z.len() {
            let relaxed = 1.6 * ax[i] - 0.6 * z[i];
            z[i] = clip(relaxed + y[i] / rho, problem.rows[i].lower, problem.rows[i].upper);
            y[i] += rho * (relaxed - z[i]);
        }
        x = next;
        if !finite(&x) || !finite(&y) {
            break Termination::NumericalFailure;
        }
        if budget.statistics.iterations.is_multiple_of(10) || budget.statistics.iterations == 1 {
            let candidate = details(model, &problem, x.clone(), y.clone(), options, Termination::IterationLimit, &budget, used)?;
            if candidate.result.feasibility == Feasibility::Feasible {
                let cost = |result: &QpDetailedResult| {
                    result
                        .certificate
                        .as_ref()
                        .map(|c| c.working_primal_objective)
                        .or_else(|| result.result.values.as_ref().and_then(|x| working_value(model, x, options).ok()))
                };
                let improve = last.result.feasibility != Feasibility::Feasible
                    || cost(&candidate).zip(cost(&last)).is_some_and(|(new, old)| problem.direction * new < problem.direction * old);
                if improve || candidate.result.optimality == Optimality::Optimal {
                    last = candidate;
                }
                if last.result.optimality == Optimality::Optimal {
                    return Ok(last);
                }
            }
        }
        if budget.statistics.iterations.is_multiple_of(50) {
            let primal = la::norm(&ax.iter().zip(&z).map(|(a, b)| a - b).collect::<Vec<_>>());
            let hx = la::multiply(&problem.h, &x)?;
            let aty = la::transpose_multiply(&problem.a, &y, x.len())?;
            let dual = la::norm(&hx.iter().zip(&problem.q).zip(aty).map(|((h, q), a)| h + q + a).collect::<Vec<_>>());
            let next_rho = if primal > 10.0 * dual {
                rho * 2.0
            } else if dual > 10.0 * primal {
                rho / 2.0
            } else {
                rho
            };
            let next_rho = next_rho.clamp(1e-6, 1e6);
            if next_rho != rho {
                rho = next_rho;
                factor = match system(&problem, rho, sigma, control) {
                    Ok(value) => value,
                    Err(error) if error.code == "cancelled" => break Termination::Cancelled,
                    Err(error) => return Err(error),
                };
            }
            budget.progress("convex_qp", last.result.objective);
        }
    };
    if last.result.feasibility == Feasibility::Feasible {
        last.result.termination = termination;
        last.result.statistics = budget.statistics;
        Ok(last)
    } else {
        Ok(stop(termination, budget.statistics, used))
    }
}
pub fn analyze_quadratic_curvature(model: &OptimizationModel, options: &SolveOptions, control: &dyn RunControl) -> Result<ConvexityAnalysis> {
    Ok(build(model, options, control)?.curvature)
}

pub(crate) fn working_value(model: &OptimizationModel, x: &[f64], options: &SolveOptions) -> Result<f64> {
    crate::working_objective(model, x, options)
}
/// Solve a continuous convex QP with explicit primal/dual certificate and genuine warm start.
pub fn solve_qp_detailed(
    model: &OptimizationModel,
    options: &SolveOptions,
    control: &dyn RunControl,
    warm: Option<&QpWarmStart>,
) -> Result<QpDetailedResult> {
    match solve_qp_impl(model, options, control, warm) {
        Err(error) if error.code == "cancelled" => Ok(stop(Termination::Cancelled, SolveStatistics::default(), warm.is_some())),
        other => other,
    }
}
