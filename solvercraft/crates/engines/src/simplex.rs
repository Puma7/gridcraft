use crate::{Budget, Relaxation};
use solvercraft_types::{Direction, OptimizationModel, Relation, Termination};

const PIVOT_EPS: f64 = 1e-12;
type Bounds = (Option<f64>, Option<f64>);
struct Column {
    original: usize,
    sign: f64,
}
struct Row {
    coefficients: Vec<f64>,
    relation: Relation,
    rhs: f64,
    /// Original constraint and the sign/scale used to normalize its row.
    source: Option<(usize, f64)>,
}
struct DualLink {
    source: usize,
    column: usize,
    cost: f64,
    multiplier: f64,
}
struct Tableau {
    rows: Vec<Vec<f64>>,
    basis: Vec<usize>,
    width: usize,
    repaired_rhs: bool,
}
enum Step {
    Optimal,
    Certified(f64),
    Unbounded(usize),
    Stopped(Termination),
    Numerical(String),
}

fn allocated<T: Clone>(n: usize, value: T) -> std::result::Result<Vec<T>, Relaxation> {
    let mut out = Vec::new();
    if out.try_reserve_exact(n).is_err() {
        return Err(Relaxation::Stopped(Termination::MemoryLimit));
    }
    out.resize(n, value);
    Ok(out)
}
fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

pub(crate) fn relax(model: &OptimizationModel, bounds: &[(Option<f64>, Option<f64>)], budget: &mut Budget<'_>) -> Relaxation {
    match run(model, bounds, budget) {
        Ok(result) => result,
        Err(result) => result,
    }
}
fn run(model: &OptimizationModel, bounds: &[(Option<f64>, Option<f64>)], budget: &mut Budget<'_>) -> std::result::Result<Relaxation, Relaxation> {
    budget.evaluation().map_err(Relaxation::Stopped)?;
    // Use the shared compensated aggregation, rather than a different naive
    // coefficient sum than the original-model evaluator.
    let canonical = solvercraft_model::canonicalize(model, budget.options).map_err(|error| Relaxation::Numerical(error.to_string()))?;
    let model = &canonical;
    if bounds.iter().any(|(lo, hi)| matches!((lo,hi),(Some(l),Some(h)) if l>h)) {
        return Ok(Relaxation::Infeasible);
    }
    let n = model.variables.len();
    // Worst-case free-variable splits, bounds, slacks and artificial columns.
    // This estimate includes normalized rows and pivot scratch in addition to tableau.
    let max_rows = model.constraints.len().checked_add(n).ok_or(Relaxation::Stopped(Termination::MemoryLimit))?;
    let max_width = n
        .checked_mul(2)
        .and_then(|a| max_rows.checked_mul(2).and_then(|b| a.checked_add(b)))
        .and_then(|a| a.checked_add(1))
        .ok_or(Relaxation::Stopped(Termination::MemoryLimit))?;
    let bytes = (max_rows as u64)
        .checked_add(4)
        .and_then(|r| r.checked_mul(max_width as u64))
        .and_then(|v| v.checked_mul(24))
        .ok_or(Relaxation::Stopped(Termination::MemoryLimit))?;
    let certificate_bytes = (n as u64)
        .checked_mul(std::mem::size_of::<solvercraft_model::ExactProducts>() as u64)
        .and_then(|v| v.checked_add((max_rows as u64).checked_mul(64)?));
    if bytes.checked_add(certificate_bytes.unwrap_or(u64::MAX)).is_none_or(|v| v > budget.options.max_memory_bytes) {
        return Err(Relaxation::Stopped(Termination::MemoryLimit));
    }
    let mut columns = Vec::new();
    let mut offset = allocated(n, 0.0)?;
    let mut upper_bounds = Vec::new();
    for (j, (lo, hi)) in bounds.iter().enumerate() {
        if let (Some(l), Some(h)) = (lo, hi)
            && l == h
        {
            offset[j] = *l;
            continue;
        }
        if let Some(l) = lo {
            offset[j] = *l;
            columns.push(Column { original: j, sign: 1.0 });
            if let Some(h) = hi {
                let rhs = h - l;
                if !rhs.is_finite() {
                    return Err(Relaxation::Numerical("Bound translation overflowed.".into()));
                }
                upper_bounds.push((columns.len() - 1, rhs));
            }
        } else if let Some(h) = hi {
            offset[j] = *h;
            columns.push(Column { original: j, sign: -1.0 });
        } else {
            columns.push(Column { original: j, sign: 1.0 });
            columns.push(Column { original: j, sign: -1.0 });
        }
    }
    let ny = columns.len();
    let mut normalized = Vec::new();
    for (source, constraint) in model.constraints.iter().enumerate() {
        budget.check().map_err(Relaxation::Stopped)?;
        let mut original = allocated(n, 0.0)?;
        for term in &constraint.terms {
            let Some(value) = original.get_mut(term.variable) else {
                return Err(Relaxation::Numerical("Constraint has an invalid variable.".into()));
            };
            *value += term.coefficient;
        }
        if !finite(&original) {
            return Err(Relaxation::Numerical("Constraint coefficient aggregation overflowed.".into()));
        }
        let mut coefficients = allocated(ny, 0.0)?;
        for (a, c) in coefficients.iter_mut().zip(&columns) {
            *a = original[c.original] * c.sign;
        }
        // Keep rhs in the same compensated sum. Rounding a large shifted
        // constant first can lose a small residual before rhs is subtracted.
        let mut translated = solvercraft_model::ExactProducts::default();
        translated.add(constraint.rhs).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        for (a, b) in original.iter().zip(&offset) {
            translated.add_product(-a, *b).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        }
        let mut rhs = translated.clone().finish().map_err(|e| Relaxation::Numerical(format!("Constraint translation: {e}")))?;
        translated.add(-rhs).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        let lost = translated.finish().map_err(|e| Relaxation::Numerical(e.to_string()))?;
        if lost.abs() > budget.options.feasibility_tolerance {
            return Err(Relaxation::Numerical("Constraint translation loses a meaningful exact residual; rescale the model.".into()));
        }
        if !rhs.is_finite() {
            return Err(Relaxation::Numerical("Constraint translation overflowed.".into()));
        }
        let mut relation = constraint.relation;
        let scale = coefficients.iter().map(|a| a.abs()).fold(0.0, f64::max);
        if scale == 0.0 {
            let satisfied = match relation {
                Relation::LessEqual => 0.0 <= rhs + budget.options.feasibility_tolerance,
                Relation::Equal => rhs.abs() <= budget.options.feasibility_tolerance,
                Relation::GreaterEqual => 0.0 >= rhs - budget.options.feasibility_tolerance,
            };
            if !satisfied {
                return Ok(Relaxation::Infeasible);
            }
            continue;
        }
        for coefficient in &mut coefficients {
            *coefficient /= scale;
        }
        rhs /= scale;
        if !rhs.is_finite() {
            return Err(Relaxation::Numerical("Constraint row scaling overflowed.".into()));
        }
        let mut source_multiplier = 1.0 / scale;
        if rhs < 0.0 {
            source_multiplier = -source_multiplier;
            rhs = -rhs;
            for coefficient in &mut coefficients {
                *coefficient = -*coefficient;
            }
            relation = match relation {
                Relation::LessEqual => Relation::GreaterEqual,
                Relation::GreaterEqual => Relation::LessEqual,
                Relation::Equal => Relation::Equal,
            };
        }
        normalized.push(Row { coefficients, relation, rhs, source: Some((source, source_multiplier)) });
    }
    for (column, rhs) in upper_bounds {
        let mut coefficients = allocated(ny, 0.0)?;
        coefficients[column] = 1.0;
        normalized.push(Row { coefficients, relation: Relation::LessEqual, rhs, source: None });
    }
    let slack = normalized.iter().filter(|r| r.relation != Relation::Equal).count();
    let artificial = normalized.iter().filter(|r| r.relation != Relation::LessEqual).count();
    let width = ny + slack + artificial;
    let non_artificial = ny + slack;
    let mut rows = Vec::new();
    let mut basis = Vec::new();
    let mut next_slack = ny;
    let mut next_art = non_artificial;
    let mut dual_links = Vec::new();
    for row in &normalized {
        let mut values = allocated(width + 1, 0.0)?;
        for (v, a) in values.iter_mut().zip(&row.coefficients) {
            *v = *a;
        }
        let (dual_column, dual_cost, dual_sign) = match row.relation {
            Relation::LessEqual => (next_slack, 0.0, 1.0),
            Relation::GreaterEqual => (next_slack, 0.0, -1.0),
            Relation::Equal => (next_art, 1.0, 1.0),
        };
        if let Some((source, multiplier)) = row.source {
            dual_links.push(DualLink { source, column: dual_column, cost: dual_cost, multiplier: multiplier / dual_sign });
        }
        let basic = match row.relation {
            Relation::LessEqual => {
                values[next_slack] = 1.0;
                let b = next_slack;
                next_slack += 1;
                b
            }
            Relation::GreaterEqual => {
                values[next_slack] = -1.0;
                next_slack += 1;
                values[next_art] = 1.0;
                let b = next_art;
                next_art += 1;
                b
            }
            Relation::Equal => {
                values[next_art] = 1.0;
                let b = next_art;
                next_art += 1;
                b
            }
        };
        values[width] = row.rhs;
        rows.push(values);
        basis.push(basic);
    }
    let mut tableau = Tableau { rows, basis, width, repaired_rhs: false };
    if artificial > 0 {
        let mut phase = allocated(width + 1, 0.0)?;
        for value in phase.iter_mut().take(width).skip(non_artificial) {
            *value = 1.0;
        }
        for (column, value) in phase.iter_mut().enumerate() {
            let initial = *value;
            *value = solvercraft_model::compensated_sum(
                std::iter::once(initial)
                    .chain(tableau.rows.iter().zip(&tableau.basis).filter(|(_, basic)| **basic >= non_artificial).map(|(row, _)| -row[column])),
            )
            .map_err(|e| Relaxation::Numerical(format!("Phase-one cost aggregation: {e}")))?;
        }
        match simplex(&mut tableau, &mut phase, width, Some(non_artificial), None, budget) {
            Step::Stopped(reason) => return Err(Relaxation::Stopped(reason)),
            Step::Numerical(reason) => return Err(Relaxation::Numerical(reason)),
            Step::Unbounded(_) => {
                return Err(Relaxation::Numerical("Phase-one objective unexpectedly became unbounded.".into()));
            }
            Step::Optimal => {}
            Step::Certified(_) => return Err(Relaxation::Numerical("Unexpected phase-one certificate.".into())),
        }
        let mut artificial_value = solvercraft_model::ExactProducts::default();
        for (row, basic) in tableau.rows.iter().zip(&tableau.basis) {
            if *basic >= non_artificial {
                artificial_value.add(row[width]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
            }
        }
        let sum_artificial = artificial_value.finish().map_err(|e| Relaxation::Numerical(e.to_string()))?;
        if !sum_artificial.is_finite() {
            return Err(Relaxation::Numerical("Non-finite phase-one objective.".into()));
        }
        // Rounded phase-I pivots may erase a small coefficient that matters
        // over a wide variable interval. An artificial residual alone must not
        // label the original model infeasible. Independently verify a signed
        // original-row separation over the current variable box first.
        if sum_artificial > budget.options.feasibility_tolerance {
            return if infeasibility_certificate(model, bounds, &phase, &dual_links, budget)? {
                Ok(Relaxation::Infeasible)
            } else {
                Err(Relaxation::Numerical(
                    "Phase-one residual lacks an independently valid original-model infeasibility certificate; rescale the model.".into(),
                ))
            };
        }
        let mut redundant = Vec::new();
        for i in 0..tableau.rows.len() {
            if tableau.basis[i] < non_artificial {
                continue;
            }
            if let Some(j) = (0..non_artificial).find(|j| tableau.rows[i][*j].abs() > PIVOT_EPS) {
                if tableau.rows[i][width].abs() <= budget.options.feasibility_tolerance {
                    tableau.rows[i][width] = 0.0;
                }
                pivot(&mut tableau, &mut phase, i, j, budget)?;
            } else {
                redundant.push(i);
            }
        }
        for i in redundant.into_iter().rev() {
            tableau.rows.remove(i);
            tableau.basis.remove(i);
        }
    }
    let mut original_cost = allocated(n, 0.0)?;
    let direction = if model.objective.direction == Direction::Minimize { 1.0 } else { -1.0 };
    for term in &model.objective.linear {
        original_cost[term.variable] += direction * term.coefficient;
    }
    if !finite(&original_cost) {
        return Err(Relaxation::Numerical("Objective coefficient aggregation overflowed.".into()));
    }
    let mut cost = allocated(width + 1, 0.0)?;
    for (j, c) in columns.iter().enumerate() {
        cost[j] = original_cost[c.original] * c.sign;
    }
    let mut objective = allocated(width + 1, 0.0)?;
    let proof = CycleProof { model, bounds, columns: &columns, offset: &offset, links: &dual_links, costs: &cost, direction };
    let outcome = simplex(&mut tableau, &mut objective, non_artificial, None, Some(&proof), budget);
    let (unbounded, certified) = match outcome {
        Step::Optimal => (None, None),
        Step::Certified(bound) => (None, Some(bound)),
        Step::Unbounded(column) => (Some(column), None),
        Step::Stopped(reason) => return Err(Relaxation::Stopped(reason)),
        Step::Numerical(reason) => return Err(Relaxation::Numerical(reason)),
    };
    let values = original_point(&tableau, &columns, &offset, budget)?;
    // Continuous checks include tighter branch bounds and original linear rows.
    for (x, (lo, hi)) in values.iter().zip(bounds) {
        if lo.is_some_and(|l| *x < l - budget.options.feasibility_tolerance) || hi.is_some_and(|h| *x > h + budget.options.feasibility_tolerance) {
            return Err(Relaxation::Numerical("LP basis failed original bound verification.".into()));
        }
    }
    for row in &model.constraints {
        let residual =
            solvercraft_model::compensated_sum(row.terms.iter().map(|a| a.coefficient * values[a.variable]).chain(std::iter::once(-row.rhs)))
                .map_err(|e| Relaxation::Numerical(format!("Constraint verification: {e}")))?;
        let violation = match row.relation {
            Relation::LessEqual => residual.max(0.0),
            Relation::GreaterEqual => (-residual).max(0.0),
            Relation::Equal => residual.abs(),
        };
        if !residual.is_finite() || violation > budget.options.feasibility_tolerance {
            return Err(Relaxation::Numerical("LP basis failed original constraint verification; rescale the model.".into()));
        }
    }
    if let Some(entering) = unbounded {
        if !recession_certificate(model, bounds, &columns, &tableau, entering, budget)? {
            return Err(Relaxation::Numerical(
                "Tableau ray lacks an exact original-row/bound recession and improving-objective certificate; rescale the model.".into(),
            ));
        }
        Ok(Relaxation::Unbounded(values))
    } else {
        Ok(Relaxation::Optimal(values, certified))
    }
}

fn original_point(tableau: &Tableau, columns: &[Column], offset: &[f64], budget: &Budget<'_>) -> std::result::Result<Vec<f64>, Relaxation> {
    let mut transformed = allocated(tableau.width, 0.0)?;
    for (row, basic) in tableau.rows.iter().zip(&tableau.basis) {
        transformed[*basic] = row[tableau.width];
    }
    let mut values = allocated(offset.len(), 0.0)?;
    for (original, value) in values.iter_mut().enumerate() {
        if original % 64 == 0 {
            budget.check().map_err(Relaxation::Stopped)?;
        }
        let displacement =
            || columns.iter().enumerate().filter(|(_, column)| column.original == original).map(|(j, column)| column.sign * transformed[j]);
        *value = solvercraft_model::compensated_sum(std::iter::once(offset[original]).chain(displacement()))
            .map_err(|e| Relaxation::Numerical(format!("Original-coordinate reconstruction: {e}")))?;
        let lost = solvercraft_model::compensated_sum(std::iter::once(offset[original]).chain(displacement()).chain(std::iter::once(-*value)))
            .map_err(|e| Relaxation::Numerical(format!("Reconstruction precision check: {e}")))?;
        if lost.abs() > budget.options.feasibility_tolerance {
            return Err(Relaxation::Numerical(
                "Original-coordinate reconstruction loses a displacement larger than the feasibility tolerance; rescale the variable.".into(),
            ));
        }
    }
    if !finite(&values) {
        return Err(Relaxation::Numerical("Non-finite original-space LP solution.".into()));
    }
    Ok(values)
}

fn recession_certificate(
    model: &OptimizationModel,
    bounds: &[(Option<f64>, Option<f64>)],
    columns: &[Column],
    tableau: &Tableau,
    entering: usize,
    budget: &Budget<'_>,
) -> std::result::Result<bool, Relaxation> {
    use std::cmp::Ordering;
    let mut transformed = allocated(columns.len(), 0.0)?;
    if entering < columns.len() {
        transformed[entering] = 1.0;
    }
    for (row, basic) in tableau.rows.iter().zip(&tableau.basis) {
        if *basic < columns.len() {
            transformed[*basic] = -row[entering];
        }
    }
    let mut direction = allocated(model.variables.len(), 0.0)?;
    for (original, value) in direction.iter_mut().enumerate() {
        budget.check().map_err(Relaxation::Stopped)?;
        *value = solvercraft_model::compensated_sum(
            columns.iter().enumerate().filter(|(_, c)| c.original == original).map(|(i, c)| c.sign * transformed[i]),
        )
        .map_err(|e| Relaxation::Numerical(e.to_string()))?;
    }
    for (d, (low, high)) in direction.iter().zip(bounds) {
        if !d.is_finite() || low.is_some() && *d < 0.0 || high.is_some() && *d > 0.0 {
            return Ok(false);
        }
    }
    for row in &model.constraints {
        budget.check().map_err(Relaxation::Stopped)?;
        let mut derivative = solvercraft_model::ExactProducts::default();
        for term in &row.terms {
            derivative.add_product(term.coefficient, direction[term.variable]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        }
        let sign = derivative.exact_sign().map_err(|e| Relaxation::Numerical(e.to_string()))?;
        if match row.relation {
            Relation::LessEqual => sign == Ordering::Greater,
            Relation::GreaterEqual => sign == Ordering::Less,
            Relation::Equal => sign != Ordering::Equal,
        } {
            return Ok(false);
        }
    }
    let mut descent = solvercraft_model::ExactProducts::default();
    for term in &model.objective.linear {
        descent.add_product(term.coefficient, direction[term.variable]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
    }
    let sign = descent.exact_sign().map_err(|e| Relaxation::Numerical(e.to_string()))?;
    Ok(match model.objective.direction {
        Direction::Minimize => sign == Ordering::Less,
        Direction::Maximize => sign == Ordering::Greater,
    })
}

fn infeasibility_certificate(
    model: &OptimizationModel,
    bounds: &[(Option<f64>, Option<f64>)],
    phase: &[f64],
    links: &[DualLink],
    budget: &Budget<'_>,
) -> std::result::Result<bool, Relaxation> {
    let bounds = crate::certificate_box(model, bounds);
    let mut weights = allocated(model.constraints.len(), 0.0)?;
    for link in links {
        let value = (link.cost - phase[link.column]) * link.multiplier;
        if !value.is_finite() {
            return Ok(false);
        }
        weights[link.source] = match model.constraints[link.source].relation {
            Relation::LessEqual => value.min(0.0),
            Relation::GreaterEqual => value.max(0.0),
            Relation::Equal => value,
        };
    }
    if separates_box(model, &bounds, &weights, budget)? {
        return Ok(true);
    }
    // Use bounds implied by original rows only for certification. These
    // endpoints never enter the simplex tableau or change the search model.
    let bounds = propagate_certificate_box(model, bounds, budget)?;
    separates_box(model, &bounds, &weights, budget)
}

fn propagate_certificate_box(
    model: &OptimizationModel,
    mut bounds: Vec<Bounds>,
    budget: &Budget<'_>,
) -> std::result::Result<Vec<Bounds>, Relaxation> {
    for _ in 0..16 {
        let previous = bounds.clone();
        for row in &model.constraints {
            budget.check().map_err(Relaxation::Stopped)?;
            // Bound worst-case work for dense rows. Omitting a row can only
            // make the certificate box weaker.
            if row.terms.len() > 64 {
                continue;
            }
            for sign in [1.0, -1.0] {
                if (sign > 0.0 && row.relation == Relation::GreaterEqual) || (sign < 0.0 && row.relation == Relation::LessEqual) {
                    continue;
                }
                for (index, target) in row.terms.iter().enumerate() {
                    let coefficient = sign * target.coefficient;
                    if coefficient == 0.0 {
                        continue;
                    }
                    let Some(residual) = certificate_residual(row, &previous, index, sign) else { continue };
                    let ratio = residual / coefficient;
                    let (lower, upper) = &mut bounds[target.variable];
                    if coefficient > 0.0 && ratio.next_up().is_finite() {
                        let value = ratio.next_up();
                        *upper = Some(upper.map_or(value, |old| old.min(value)));
                    } else if coefficient < 0.0 && ratio.next_down().is_finite() {
                        let value = ratio.next_down();
                        *lower = Some(lower.map_or(value, |old| old.max(value)));
                    }
                }
            }
        }
        if bounds == previous {
            break;
        }
    }
    Ok(bounds)
}

fn certificate_residual(row: &solvercraft_types::LinearConstraint, bounds: &[(Option<f64>, Option<f64>)], omitted: usize, sign: f64) -> Option<f64> {
    let mut residual = solvercraft_model::ExactProducts::default();
    residual.add(sign * row.rhs).ok()?;
    for (index, term) in row.terms.iter().enumerate() {
        let coefficient = sign * term.coefficient;
        if index == omitted || coefficient == 0.0 {
            continue;
        }
        let (lower, upper) = bounds[term.variable];
        residual.add_product(-coefficient, if coefficient > 0.0 { lower? } else { upper? }).ok()?;
    }
    // Round the numerator outward before dividing. A negative divisor flips
    // the bound direction, which is handled by the caller.
    let outward = residual.finish().ok()?.next_up();
    outward.is_finite().then_some(outward)
}

fn separates_box(
    model: &OptimizationModel,
    bounds: &[(Option<f64>, Option<f64>)],
    weights: &[f64],
    budget: &Budget<'_>,
) -> std::result::Result<bool, Relaxation> {
    use std::cmp::Ordering;
    let mut gradients = allocated(model.variables.len(), solvercraft_model::ExactProducts::default())?;
    let mut separation = solvercraft_model::ExactProducts::default();
    for (row, weight) in model.constraints.iter().zip(weights) {
        budget.check().map_err(Relaxation::Stopped)?;
        separation.add_product(*weight, row.rhs).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        for term in &row.terms {
            gradients[term.variable].add_product(*weight, term.coefficient).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        }
    }
    let mut endpoints = allocated(model.variables.len(), 0.0)?;
    for ((gradient, (low, high)), endpoint) in gradients.iter().zip(bounds).zip(&mut endpoints) {
        *endpoint = match gradient.exact_sign().map_err(|e| Relaxation::Numerical(e.to_string()))? {
            Ordering::Greater => match high {
                Some(value) => *value,
                None => return Ok(false),
            },
            Ordering::Less => match low {
                Some(value) => *value,
                None => return Ok(false),
            },
            Ordering::Equal => 0.0,
        };
    }
    for (row, weight) in model.constraints.iter().zip(weights) {
        budget.check().map_err(Relaxation::Stopped)?;
        for term in &row.terms {
            separation.add_triple(-*weight, term.coefficient, endpoints[term.variable]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        }
    }
    // Signed row weights imply weighted(Ax)>=weighted(b) for every feasible
    // original x. Exact positivity of weighted(b)-max_box weighted(Ax) is a
    // contradiction; no rounded reduced cost or magnitude threshold is a proof.
    Ok(separation.exact_sign().map_err(|e| Relaxation::Numerical(e.to_string()))? == Ordering::Greater)
}

fn pivot(t: &mut Tableau, obj: &mut [f64], r: usize, j: usize, budget: &Budget<'_>) -> std::result::Result<(), Relaxation> {
    let pv = t.rows[r][j];
    if !pv.is_finite() || pv.abs() <= PIVOT_EPS {
        return Err(Relaxation::Numerical("Singular or non-finite simplex pivot.".into()));
    }
    for v in &mut t.rows[r] {
        *v /= pv;
    }
    if !finite(&t.rows[r]) {
        return Err(Relaxation::Numerical("Pivot normalization overflowed.".into()));
    }
    let pivot_row = t.rows[r].clone();
    for (i, row) in t.rows.iter_mut().enumerate() {
        budget.check().map_err(Relaxation::Stopped)?;
        if i == r {
            continue;
        }
        let coefficient = row[j];
        if coefficient != 0.0 {
            for (v, a) in row.iter_mut().zip(&pivot_row) {
                *v = (-coefficient).mul_add(*a, *v);
            }
            if !finite(row) {
                return Err(Relaxation::Numerical("Tableau elimination overflowed.".into()));
            }
        }
    }
    let coefficient = obj[j];
    for (v, a) in obj.iter_mut().zip(&pivot_row) {
        *v = (-coefficient).mul_add(*a, *v);
    }
    if !finite(obj) {
        return Err(Relaxation::Numerical("Reduced cost elimination overflowed.".into()));
    }
    t.basis[r] = j;
    Ok(())
}

fn refresh_objective(t: &Tableau, obj: &mut [f64], costs: &[f64], allowed: usize, budget: &Budget<'_>) -> std::result::Result<(), Relaxation> {
    // Pivoting a rounded/scaled objective row can erase a small cost and later
    // falsely cancel the remaining large terms. Reconstruct from the original
    // objective and the current basis before every phase-II entering decision.
    // In particular, 10^16*x+y-10^16*z on x<=y<=z must retain its unit residual
    // after the two zero-length pivots. Exact products recover it from this basis.
    for (column, value) in obj.iter_mut().enumerate().take(t.width) {
        if column % 32 == 0 {
            budget.check().map_err(Relaxation::Stopped)?;
        }
        let mut sum = solvercraft_model::ExactProducts::default();
        sum.add(costs[column]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        for (row, basic) in t.rows.iter().zip(&t.basis) {
            sum.add_product(-costs[*basic], row[column]).map_err(|e| Relaxation::Numerical(e.to_string()))?;
        }
        *value = sum.finish().map_err(|e| Relaxation::Numerical(format!("Reduced-cost aggregation: {e}")))?;
    }
    // Normalize only after eliminating the current basis, not the original
    // objective. A meaningful reduced cost is never deliberately rounded away.
    let scale = obj.iter().take(allowed).map(|v| v.abs()).fold(0.0_f64, f64::max);
    if scale > 0.0 {
        for value in obj.iter_mut().take(allowed) {
            let original = *value;
            *value /= scale;
            if !value.is_finite() || original != 0.0 && *value == 0.0 {
                return Err(Relaxation::Numerical("Objective scaling loses a nonzero reduced cost; rescale the model.".into()));
            }
        }
    }
    // Phase II never uses a translated objective constant to choose a pivot or
    // establish optimality. The original objective is evaluated independently.
    obj[t.width] = 0.0;
    Ok(())
}

struct CycleProof<'a> {
    model: &'a OptimizationModel,
    bounds: &'a [Bounds],
    columns: &'a [Column],
    offset: &'a [f64],
    links: &'a [DualLink],
    costs: &'a [f64],
    direction: f64,
}

impl CycleProof<'_> {
    fn verify(&self, tableau: &Tableau, budget: &Budget<'_>) -> std::result::Result<Option<f64>, Relaxation> {
        let weights = self.weights(tableau, budget)?;
        let bounds = propagate_certificate_box(self.model, crate::certificate_box(self.model, self.bounds), budget)?;
        let Some(lower) = self.lower_bound(&weights, &bounds, budget)? else { return Ok(None) };
        let values = original_point(tableau, self.columns, self.offset, budget)?;
        let mut gap = solvercraft_model::ExactProducts::default();
        for term in &self.model.objective.linear {
            gap.add_product(self.direction * term.coefficient, values[term.variable]).map_err(exact_error)?;
        }
        gap.add(-lower).map_err(exact_error)?;
        let tolerance = budget.options.optimality_tolerance;
        let mut above = gap.clone();
        above.add(-tolerance).map_err(exact_error)?;
        gap.add(tolerance).map_err(exact_error)?;
        use std::cmp::Ordering;
        let close = above.exact_sign().map_err(exact_error)? != Ordering::Greater && gap.exact_sign().map_err(exact_error)? != Ordering::Less;
        Ok(close.then_some(lower))
    }

    fn weights(&self, tableau: &Tableau, budget: &Budget<'_>) -> std::result::Result<Vec<f64>, Relaxation> {
        let mut weights = allocated(self.model.constraints.len(), 0.0)?;
        for link in self.links {
            budget.check().map_err(Relaxation::Stopped)?;
            let mut sum = solvercraft_model::ExactProducts::default();
            for (row, basic) in tableau.rows.iter().zip(&tableau.basis) {
                sum.add_product(self.costs[*basic], row[link.column]).map_err(exact_error)?;
            }
            let weight = sum.finish().map_err(exact_error)? * link.multiplier;
            if !weight.is_finite() {
                return Err(Relaxation::Numerical("Phase-two dual weight overflowed.".into()));
            }
            weights[link.source] = match self.model.constraints[link.source].relation {
                Relation::LessEqual => weight.min(0.0),
                Relation::GreaterEqual => weight.max(0.0),
                Relation::Equal => weight,
            };
        }
        Ok(weights)
    }

    /// For any feasible x, c*x >= y*b + (c-y*A)*x. Choose each
    /// coordinate's minimizing endpoint from a box containing the feasible set.
    /// Accumulate the resulting bound in the original coefficient space.
    fn lower_bound(&self, weights: &[f64], bounds: &[Bounds], budget: &Budget<'_>) -> std::result::Result<Option<f64>, Relaxation> {
        use std::cmp::Ordering;
        let mut reduced = allocated(bounds.len(), solvercraft_model::ExactProducts::default())?;
        let mut lower = solvercraft_model::ExactProducts::default();
        for term in &self.model.objective.linear {
            reduced[term.variable].add(self.direction * term.coefficient).map_err(exact_error)?;
        }
        for (row, weight) in self.model.constraints.iter().zip(weights) {
            budget.check().map_err(Relaxation::Stopped)?;
            lower.add_product(*weight, row.rhs).map_err(exact_error)?;
            for term in &row.terms {
                reduced[term.variable].add_product(-*weight, term.coefficient).map_err(exact_error)?;
            }
        }
        let mut endpoints = allocated(bounds.len(), 0.0)?;
        for ((reduced, (lo, hi)), endpoint) in reduced.iter().zip(bounds).zip(&mut endpoints) {
            let chosen = match reduced.exact_sign().map_err(exact_error)? {
                Ordering::Greater => *lo,
                Ordering::Less => *hi,
                Ordering::Equal => Some(0.0),
            };
            let Some(value) = chosen else { return Ok(None) };
            *endpoint = value;
        }
        for term in &self.model.objective.linear {
            lower.add_product(self.direction * term.coefficient, endpoints[term.variable]).map_err(exact_error)?;
        }
        for (row, weight) in self.model.constraints.iter().zip(weights) {
            budget.check().map_err(Relaxation::Stopped)?;
            for term in &row.terms {
                lower.add_triple(-*weight, term.coefficient, endpoints[term.variable]).map_err(exact_error)?;
            }
        }
        let outward = lower.finish().map_err(exact_error)?.next_down();
        Ok(outward.is_finite().then_some(outward))
    }
}

fn exact_error(error: solvercraft_types::SolverError) -> Relaxation {
    Relaxation::Numerical(error.to_string())
}

fn simplex(
    t: &mut Tableau,
    obj: &mut [f64],
    allowed: usize,
    artificial_start: Option<usize>,
    proof: Option<&CycleProof<'_>>,
    budget: &mut Budget<'_>,
) -> Step {
    let mut history = [None; 64];
    let mut cursor = 0;
    let mut scan_start = 0;
    'iteration: loop {
        if let Err(reason) = budget.step() {
            return Step::Stopped(reason);
        }
        // Phase I only needs a feasible basis. Artificial variables are
        // nonnegative, so exact zero for every basic artificial proves its
        // known global minimum0 without relying on rounded reduced costs.
        // This does not drop small reduced costs in the real phase-II objective.
        if artificial_start.is_some_and(|start| t.basis.iter().zip(&t.rows).all(|(basic, row)| *basic < start || row[t.width] == 0.0)) {
            obj[t.width] = 0.0;
            return Step::Optimal;
        }
        if let Some(proof) = proof
            && let Err(reason) = refresh_objective(t, obj, proof.costs, allowed, budget)
        {
            return match reason {
                Relaxation::Stopped(reason) => Step::Stopped(reason),
                Relaxation::Numerical(message) => Step::Numerical(message),
                _ => Step::Numerical("Unexpected reduced-cost reconstruction failure.".into()),
            };
        }
        // Bland's entering and tie-breaking rule avoids cycling on degenerate bases.
        // A small reduced cost can still improve the objective substantially over
        // a wide interval. Never turn it into a zero-gap optimality certificate
        // merely by comparing its magnitude with an absolute gradient threshold.
        // A basis may repeat in floating point even with Bland's rule.
        // Change only the search order on recurrence. No reduced cost is
        // discarded and each new pivot still undergoes the same checks.
        let signature = t.basis.iter().fold(0_u64, |hash, column| hash.wrapping_mul(1099511628211).wrapping_add(*column as u64));
        if history.contains(&Some(signature)) && allowed > 0 {
            if let Some(proof) = proof {
                match proof.verify(t, budget) {
                    Ok(Some(bound)) => return Step::Certified(bound),
                    Err(Relaxation::Stopped(reason)) => return Step::Stopped(reason),
                    _ => {}
                }
            }
            scan_start = (scan_start + 1) % allowed;
            history.fill(None);
        }
        history[cursor] = Some(signature);
        cursor = (cursor + 1) % history.len();
        let entering = (scan_start..allowed).chain(0..scan_start).find(|j| obj[*j] < 0.0 && !t.basis.contains(j));
        let Some(j) = entering else {
            if t.repaired_rhs
                && let Some(proof) = proof
            {
                return match proof.verify(t, budget) {
                    Ok(Some(bound)) => Step::Certified(bound),
                    Err(Relaxation::Stopped(reason)) => Step::Stopped(reason),
                    _ => Step::Numerical("Could not certify the original optimum after correcting a negative basic residual.".into()),
                };
            }
            return Step::Optimal;
        };
        let mut leaving: Option<(usize, f64)> = None;
        for (i, row) in t.rows.iter().enumerate() {
            let a = row[j];
            if a <= PIVOT_EPS {
                continue;
            }
            let ratio = row[t.width] / a;
            if !ratio.is_finite() {
                return Step::Numerical("Non-finite ratio test.".into());
            }
            if ratio < -budget.options.feasibility_tolerance {
                // A small negative basic value can look large after division
                // by a tiny pivot. Repair only in the basic value's own units.
                // A repaired phase two must certify its original-space bound.
                if row[t.width] >= -budget.options.feasibility_tolerance {
                    t.rows[i][t.width] = 0.0;
                    t.repaired_rhs = true;
                    history.fill(None);
                    continue 'iteration;
                }
                return Step::Numerical("Lost primal feasibility during simplex.".into());
            }
            let better = match leaving {
                None => true,
                Some((k, best)) => ratio < best - 1e-12 || ((ratio - best).abs() <= 1e-12 && t.basis[i] < t.basis[k]),
            };
            if better {
                leaving = Some((i, ratio));
            }
        }
        let Some((i, _)) = leaving else {
            // Tiny positive entries cannot justify a ray certificate.
            if t.rows.iter().any(|r| r[j] > 0.0 && r[j] <= PIVOT_EPS) {
                return Step::Numerical("Numerically tiny pivot prevents a reliable unboundedness decision.".into());
            }
            return Step::Unbounded(j);
        };
        if let Err(reason) = pivot(t, obj, i, j, budget) {
            return match reason {
                Relaxation::Stopped(r) => Step::Stopped(r),
                Relaxation::Numerical(s) => Step::Numerical(s),
                _ => Step::Numerical("Unexpected pivot failure.".into()),
            };
        }
    }
}

#[cfg(test)]
mod certificate_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use solvercraft_types::{LinearConstraint, LinearTerm, NoControl, Objective, SCHEMA_VERSION, SolveOptions, Variable, VariableKind};

    #[test]
    fn small_negative_basic_value_does_not_inherit_the_pivots_amplification() {
        let options = SolveOptions::default();
        let mut budget = Budget::new(&options, &NoControl);
        let mut tableau = Tableau { rows: vec![vec![2e-12, 1.0, -1e-13]], basis: vec![1], width: 2, repaired_rhs: false };
        let mut objective = [-1.0, 0.0, 0.0];
        assert!(matches!(simplex(&mut tableau, &mut objective, 2, Some(1), None, &mut budget), Step::Optimal));
        assert!(tableau.repaired_rhs);

        tableau.rows[0][2] = -1e-4;
        tableau.repaired_rhs = false;
        assert!(matches!(simplex(&mut tableau, &mut objective, 2, Some(1), None, &mut budget), Step::Numerical(_)));
        assert!(!tableau.repaired_rhs);
    }

    #[test]
    fn cycle_certificate_keeps_the_original_small_descent() {
        let variable = Variable { name: "v".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: None };
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "original lower bound".into(),
            variables: vec![variable; 2],
            objective: Objective {
                direction: Direction::Minimize,
                offset: 0.0,
                linear: vec![LinearTerm { variable: 0, coefficient: 2.0 }, LinearTerm { variable: 1, coefficient: 1.0_f64.next_down() }],
                quadratic: vec![],
            },
            constraints: vec![LinearConstraint {
                name: "demand".into(),
                relation: Relation::GreaterEqual,
                rhs: 1.0,
                terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            }],
        };
        let options = SolveOptions::default();
        let budget = Budget::new(&options, &NoControl);
        let bounds = [(Some(0.0), None); 2];
        let proof = CycleProof { model: &model, bounds: &bounds, columns: &[], offset: &[], links: &[], costs: &[], direction: 1.0 };
        // Rounding the reduced cost to zero would falsely certify a finite
        // lower bound from this dual weight on the unbounded variable.
        let unbounded = proof.lower_bound(&[1.0], &bounds, &budget).ok().unwrap();
        assert_eq!(unbounded, None);
        let bounded = proof.lower_bound(&[1.0], &[(Some(0.0), None), (Some(0.0), Some(2.0))], &budget).ok().unwrap().unwrap();
        assert!(bounded <= 1.0 - 2.0 * (1.0 - 1.0_f64.next_down()));
        assert!(bounded > 0.999_999_999_999);
    }

    #[test]
    fn implied_certificate_bounds_propagate_outward_across_original_rows() {
        let variable = Variable { name: "v".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: None };
        let row = |relation, rhs, terms: &[(usize, f64)]| LinearConstraint {
            name: "row".into(),
            relation,
            rhs,
            terms: terms.iter().map(|(variable, coefficient)| LinearTerm { variable: *variable, coefficient: *coefficient }).collect(),
        };
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "linked bounds".into(),
            variables: vec![variable; 3],
            objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
            constraints: vec![row(Relation::LessEqual, 3.0, &[(0, 1.0), (1, 1.0)]), row(Relation::GreaterEqual, 2.0, &[(1, 1.0), (2, -1.0)])],
        };
        let options = SolveOptions::default();
        let budget = Budget::new(&options, &NoControl);
        let result = propagate_certificate_box(&model, vec![(Some(0.0), None); 3], &budget);
        assert!(result.is_ok());
        let bounds = result.ok().unwrap();
        assert!((1.0..1.000_001).contains(&bounds[0].1.unwrap()));
        assert!((1.999_999..=2.0).contains(&bounds[1].0.unwrap()));
        assert!((1.0..1.000_001).contains(&bounds[2].1.unwrap()));
    }
}
