//! Small, checked vertex bases for LP reoptimization and local sensitivity.
//! These are original-coordinate active-row bases, not opaque tableau indices.
use serde::{Deserialize, Serialize};
use solvercraft_model::{canonicalize, compensated_sum, validate_solution};
use solvercraft_types::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LpBasis {
    pub active_rows: Vec<usize>,
    pub row_coefficients: Vec<Vec<f64>>,
    pub equalities: Vec<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowableChange {
    pub item: usize,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpSensitivity {
    /// Derivative of the original objective w.r.t. each normalized row RHS.
    pub rhs_shadow_prices: Vec<f64>,
    pub rhs_changes: Vec<AllowableChange>,
    pub objective_changes: Vec<AllowableChange>,
    pub row_descriptions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpBasisResult {
    pub result: SolveResult,
    pub basis: Option<LpBasis>,
    pub sensitivity: Option<LpSensitivity>,
    pub warm_start_used: bool,
}
struct Rows {
    a: Vec<Vec<f64>>,
    b: Vec<f64>,
    eq: Vec<bool>,
    names: Vec<String>,
    q: Vec<f64>,
    direction: f64,
}
fn err(message: &str) -> SolverError {
    SolverError::new("basis_unavailable", message)
}
fn dot(a: &[f64], b: &[f64]) -> Result<f64> {
    solvercraft_model::compensated_dot(a, b)
}
fn check_objective_products(model: &OptimizationModel, point: &[f64], options: &SolveOptions) -> Result<()> {
    crate::working_objective(model, point, options).map(|_| ())
}
fn row_residual(row: &[f64], point: &[f64], rhs: f64) -> Result<f64> {
    let mut sum = solvercraft_model::ExactProducts::default();
    sum.add(-rhs)?;
    for (a, x) in row.iter().zip(point) {
        sum.add_product(*a, *x)?;
    }
    sum.finish()
}
fn rows(model: &OptimizationModel, options: &SolveOptions) -> Result<Rows> {
    let model = canonicalize(model, options)?;
    let n = model.variables.len();
    if n > 64 || model.constraints.len() + 2 * n > 512 {
        return Err(err("Checked vertex-basis analysis currently supports 64 variables and 512 normalized rows."));
    }
    if !model.objective.quadratic.is_empty() || model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(SolverError::new("unsupported_problem", "LP basis analysis requires a continuous linear model."));
    }
    let estimate = (n as u64) * (model.constraints.len() as u64 + 2 * n as u64 + 4 * n as u64) * 128;
    if estimate > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "LP basis working-set estimate exceeds budget."));
    }
    let direction = if model.objective.direction == Direction::Minimize { 1.0 } else { -1.0 };
    let mut result = Rows { a: vec![], b: vec![], eq: vec![], names: vec![], q: vec![0.0; n], direction };
    for t in &model.objective.linear {
        result.q[t.variable] = direction * t.coefficient;
    }
    for (i, row) in model.constraints.iter().enumerate() {
        let sign = if row.relation == Relation::GreaterEqual { -1.0 } else { 1.0 };
        let mut a = vec![0.0; n];
        for t in &row.terms {
            a[t.variable] = sign * t.coefficient;
        }
        result.a.push(a);
        result.b.push(sign * row.rhs);
        result.eq.push(row.relation == Relation::Equal);
        result.names.push(format!("linear:{i}:normalized_rhs"));
    }
    for (i, var) in model.variables.iter().enumerate() {
        for (side, bound, sign) in [("lower", var.lower, -1.0), ("upper", var.upper, 1.0)] {
            if let Some(bound) = bound {
                let mut a = vec![0.0; n];
                a[i] = sign;
                result.a.push(a);
                result.b.push(sign * bound);
                result.eq.push(false);
                result.names.push(format!("bound:{i}:{side}:normalized_rhs"));
            }
        }
    }
    Ok(result)
}
#[allow(clippy::needless_range_loop)] // Elimination updates two indexed triangular rows.
fn linear_solve(matrix: &[Vec<f64>], rhs: &[f64], control: &dyn RunControl) -> Result<Vec<f64>> {
    let n = rhs.len();
    if matrix.len() != n || matrix.iter().any(|row| row.len() != n) {
        return Err(err("Basis dimension mismatch."));
    }
    let mut a = matrix.to_vec();
    let mut b = rhs.to_vec();
    for col in 0..n {
        if control.is_cancelled() {
            return Err(SolverError::new("cancelled", "Basis factorization cancelled."));
        }
        let pivot = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs())).ok_or_else(|| err("Empty pivot set."))?;
        let scale = a[pivot].iter().map(|v| v.abs()).fold(0.0, f64::max);
        if a[pivot][col].abs() <= 1e-12 * scale.max(f64::MIN_POSITIVE) {
            return Err(err("Singular or ill-conditioned active basis."));
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let multiplier = a[row][col] / a[col][col];
            a[row][col] = 0.0;
            for j in col + 1..n {
                a[row][j] = compensated_sum([a[row][j], -multiplier * a[col][j]])?;
            }
            b[row] = compensated_sum([b[row], -multiplier * b[col]])?;
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = compensated_sum(std::iter::once(b[i]).chain((i + 1..n).map(|j| -a[i][j] * x[j])))? / a[i][i];
        if !x[i].is_finite() {
            return Err(err("Basis solution overflow."));
        }
    }
    for (row, target) in matrix.iter().zip(rhs) {
        let residual = row_residual(row, &x, *target)?;
        if residual.abs() > 1e-9 {
            return Err(err("Basis linear solve failed absolute original-unit residual check."));
        }
    }
    Ok(x)
}
fn transpose(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    (0..a.len()).map(|i| a.iter().map(|row| row[i]).collect()).collect()
}
fn point_and_dual(data: &Rows, active: &[usize], control: &dyn RunControl) -> Result<(Vec<f64>, Vec<f64>)> {
    let matrix = active.iter().map(|i| data.a[*i].clone()).collect::<Vec<_>>();
    let rhs = active.iter().map(|i| data.b[*i]).collect::<Vec<_>>();
    let x = linear_solve(&matrix, &rhs, control)?;
    let dual = linear_solve(&transpose(&matrix), &data.q.iter().map(|q| -q).collect::<Vec<_>>(), control)?;
    Ok((x, dual))
}
fn dual_valid(data: &Rows, active: &[usize], dual: &[f64], tol: f64) -> bool {
    active.iter().zip(dual).all(|(i, d)| data.eq[*i] || *d >= -tol)
}
fn tighten(lower: &mut Option<f64>, upper: &mut Option<f64>, coefficient: f64, slack: f64) -> Result<()> {
    if coefficient == 0.0 {
        if slack < -1e-8 {
            return Err(err("Basis interval has inconsistent stationary row."));
        }
        return Ok(());
    }
    let limit = slack / coefficient;
    if !limit.is_finite() {
        return Err(err("Sensitivity interval arithmetic overflow."));
    }
    if coefficient > 0.0 {
        let limit = if limit == 0.0 { limit } else { limit.next_down() };
        *upper = Some(upper.map_or(limit, |old| old.min(limit)));
    } else {
        let limit = if limit == 0.0 { limit } else { limit.next_up() };
        *lower = Some(lower.map_or(limit, |old| old.max(limit)));
    }
    Ok(())
}
fn analyze(data: &Rows, active: &[usize], x: &[f64], dual: &[f64], control: &dyn RunControl) -> Result<LpSensitivity> {
    let n = x.len();
    let b = active.iter().map(|i| data.a[*i].clone()).collect::<Vec<_>>();
    let bt = transpose(&b);
    let mut shadow = vec![0.0; data.a.len()];
    for (i, d) in active.iter().zip(dual) {
        shadow[*i] = -data.direction * d;
    }
    let mut rhs_changes = vec![];
    for k in 0..data.a.len() {
        let mut lower = None;
        let mut upper = None;
        if let Some(position) = active.iter().position(|i| *i == k) {
            let mut unit = vec![0.0; n];
            unit[position] = 1.0;
            let d = linear_solve(&b, &unit, control)?;
            for (i, row) in data.a.iter().enumerate() {
                if active.contains(&i) {
                    continue;
                }
                let coefficient = dot(row, &d)?;
                let slack = -row_residual(row, x, data.b[i])?;
                tighten(&mut lower, &mut upper, coefficient, slack)?;
                if data.eq[i] {
                    tighten(&mut lower, &mut upper, -coefficient, -slack)?;
                }
            }
        } else {
            let slack = -row_residual(&data.a[k], x, data.b[k])?;
            lower = Some(-slack);
            if data.eq[k] {
                upper = lower;
            }
        }
        rhs_changes.push(AllowableChange { item: k, lower, upper });
    }
    let mut objective_changes = vec![];
    for k in 0..n {
        let mut unit = vec![0.0; n];
        unit[k] = -data.direction;
        let derivative = linear_solve(&bt, &unit, control)?;
        let mut lower = None;
        let mut upper = None;
        for ((i, d), change) in active.iter().zip(dual).zip(derivative) {
            if !data.eq[*i] {
                tighten(&mut lower, &mut upper, -change, *d)?;
            }
        }
        objective_changes.push(AllowableChange { item: k, lower, upper });
    }
    Ok(LpSensitivity { rhs_shadow_prices: shadow, rhs_changes, objective_changes, row_descriptions: data.names.clone() })
}
fn warm_result(
    model: &OptimizationModel,
    options: &SolveOptions,
    data: &Rows,
    active: &[usize],
    control: &dyn RunControl,
) -> Result<Option<SolveResult>> {
    let (x, mut dual) = point_and_dual(data, active, control)?;
    check_objective_products(model, &x, options)?;
    if !validate_solution(model, &x, options)?.feasible || !dual_valid(data, active, &dual, options.optimality_tolerance) {
        return Ok(None);
    }
    for (i, d) in active.iter().zip(&mut dual) {
        if !data.eq[*i] {
            *d = d.max(0.0);
        }
    }
    let mut gradient_intervals = Vec::with_capacity(x.len());
    for j in 0..x.len() {
        let mut sum = solvercraft_model::ExactProducts::default();
        sum.add(data.q[j])?;
        for (i, d) in active.iter().zip(&dual) {
            sum.add_product(data.a[*i][j], *d)?;
        }
        gradient_intervals.push(sum.enclosure()?);
    }
    let primal = dot(&data.q, &x)?;
    let mut exact_lagrangian = solvercraft_model::ExactProducts::default();
    for (q, x) in data.q.iter().zip(&x) {
        exact_lagrangian.add_product(*q, *x)?;
    }
    for (i, d) in active.iter().zip(&dual) {
        for (a, x) in data.a[*i].iter().zip(&x) {
            exact_lagrangian.add_triple(*d, *a, *x)?;
        }
        exact_lagrangian.add_product(-d, data.b[*i])?;
    }
    let lagrangian = exact_lagrangian.enclosure()?.0;
    let mut corrections = vec![];
    for (i, (g_low, g_high)) in gradient_intervals.iter().copied().enumerate() {
        let var = &model.variables[i];
        let endpoints = match (var.lower, var.upper) {
            (Some(low), Some(high)) => vec![low, high],
            (Some(low), None) if g_low >= 0.0 => vec![low],
            (None, Some(high)) if g_high <= 0.0 => vec![high],
            (None, None) if g_low == 0.0 && g_high == 0.0 => continue,
            _ => return Ok(None),
        };
        let mut minimum = f64::INFINITY;
        for endpoint in endpoints {
            for g in [g_low, g_high] {
                let mut product = solvercraft_model::ExactProducts::default();
                product.add_product(g, endpoint)?;
                product.add_product(-g, x[i])?;
                minimum = minimum.min(product.enclosure()?.0);
            }
        }
        corrections.push(minimum);
    }
    let magnitude =
        compensated_sum(data.q.iter().zip(&x).map(|(q, x)| (q * x).abs()).chain(active.iter().zip(&dual).map(|(i, d)| d.abs() * data.b[*i].abs())))?;
    let margin = 128.0 * f64::EPSILON * (1.0 + magnitude) * (1.0 + x.len() as f64 + active.len() as f64);
    let bound = compensated_sum(std::iter::once(lagrangian - margin).chain(corrections))?;
    if primal - bound < 0.0 || primal - bound > options.optimality_tolerance {
        return Ok(None);
    }
    let mut result = crate::verified_result(model, options, x, Optimality::Optimal)?;
    result.engine = "solvercraft-checked-lp-vertex-basis-0.2".into();
    result.best_bound = crate::reported_bound(model, bound).map(|p| p.0);
    result.relative_gap = Some((primal - bound) / primal.abs().max(bound.abs()).max(1.0));
    result.statistics.iterations = 1;
    result.statistics.evaluations = 1;
    Ok(Some(result))
}
/// Reuses a checked active basis when RHS/objective changes stay within its
/// validity region; otherwise performs a real LP solve and builds a new basis.
pub fn solve_lp_with_basis(
    model: &OptimizationModel,
    options: &SolveOptions,
    control: &dyn RunControl,
    warm: Option<&LpBasis>,
) -> Result<LpBasisResult> {
    if matches!(options.method, SolveMethod::ProjectedGradient | SolveMethod::DifferentialEvolution) {
        return Err(SolverError::new("unsupported_method", "LP vertex-basis reuse requires an LP method, just like a cold solve."));
    }
    let data = rows(model, options)?;
    if control.is_cancelled() || options.max_iterations == 0 || options.max_evaluations == 0 {
        return crate::solve(model, options, control).map(|result| LpBasisResult { result, basis: None, sensitivity: None, warm_start_used: false });
    }
    if let Some(warm) = warm {
        if warm.row_coefficients != data.a
            || warm.equalities != data.eq
            || warm.active_rows.len() != model.variables.len()
            || warm.active_rows.iter().any(|i| *i >= data.a.len())
            || warm.active_rows.iter().collect::<std::collections::BTreeSet<_>>().len() != warm.active_rows.len()
        {
            return Err(SolverError::new("invalid_warm_start", "LP basis structure or row indices do not match the model."));
        }
        let attempted = warm_result(model, options, &data, &warm.active_rows, control);
        if let Err(error) = &attempted
            && error.code == "numeric_range"
        {
            return Err(error.clone());
        }
        if let Ok(Some(result)) = attempted {
            let (x, dual) = point_and_dual(&data, &warm.active_rows, control)?;
            let sensitivity = analyze(&data, &warm.active_rows, &x, &dual, control).ok();
            return Ok(LpBasisResult { result, basis: Some(warm.clone()), sensitivity, warm_start_used: true });
        }
    }
    let result = crate::solve(model, options, control)?;
    let Some(x) = result.values.as_ref().filter(|_| result.optimality == Optimality::Optimal) else {
        return Ok(LpBasisResult { result, basis: None, sensitivity: None, warm_start_used: false });
    };
    check_objective_products(model, x, options)?;
    let active = data
        .a
        .iter()
        .enumerate()
        .filter_map(|(i, a)| dot(a, x).ok().filter(|v| (v - data.b[i]).abs() <= options.feasibility_tolerance).map(|_| i))
        .collect::<Vec<_>>();
    let n = x.len();
    if active.len() < n {
        return Ok(LpBasisResult { result, basis: None, sensitivity: None, warm_start_used: false });
    }
    let mut choices = (0..n).collect::<Vec<_>>();
    let mut trials = 0_u64;
    let maximum_trials =
        4096_u64.min(options.max_iterations.saturating_sub(result.statistics.iterations)).min(300_000_000 / (n as u64).pow(3).max(1));
    loop {
        if control.is_cancelled() || trials >= maximum_trials {
            break;
        }
        trials += 1;
        control.progress(&ProgressEvent {
            stage: "lp_vertex_basis".into(),
            iterations: result.statistics.iterations + trials,
            evaluations: result.statistics.evaluations,
            nodes: 0,
            best_objective: result.objective,
        });
        let selected = choices.iter().map(|i| active[*i]).collect::<Vec<_>>();
        if let Ok((point, dual)) = point_and_dual(&data, &selected, control)
            && validate_solution(model, &point, options)?.feasible
            && dual_valid(&data, &selected, &dual, options.optimality_tolerance)
        {
            let sensitivity = analyze(&data, &selected, &point, &dual, control).ok();
            return Ok(LpBasisResult {
                result,
                basis: Some(LpBasis { active_rows: selected, row_coefficients: data.a, equalities: data.eq }),
                sensitivity,
                warm_start_used: false,
            });
        }
        let Some(position) = (0..n).rev().find(|j| choices[*j] < active.len() - n + *j) else {
            break;
        };
        choices[position] += 1;
        for j in position + 1..n {
            choices[j] = choices[j - 1] + 1;
        }
    }
    Ok(LpBasisResult { result, basis: None, sensitivity: None, warm_start_used: false })
}
