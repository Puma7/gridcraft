//! Interval branch-and-bound certificates for continuous polynomial objectives
//! on finite boxes. Natural interval arithmetic is deliberately conservative.
use serde::{Deserialize, Serialize};
use solvercraft_compile::{Expr, ExpressionModel, evaluate, validate_expression_model};
use solvercraft_types::{
    Direction, Feasibility, Optimality, ProgressEvent, Result, RunControl, SolveMethod, SolveOptions, SolveResult, SolveStatistics, SolverError,
    Termination, VariableKind,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxSplitWitness {
    pub parent: u64,
    pub axis: usize,
    pub midpoint: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxGlobalCertificate {
    pub domain: Vec<(f64, f64)>,
    /// Bounds on the signed minimization objective, with outward rounding.
    pub signed_lower_bound: Option<f64>,
    pub signed_incumbent_upper_bound: Option<f64>,
    pub absolute_gap: Option<f64>,
    pub remaining_boxes: usize,
    pub explored_boxes: u64,
    pub tolerance_certified: bool,
    /// Replay from root 0; split i creates child IDs 2*i+1 and 2*i+2.
    /// Unsplit leaves include pruned regions, preserving the entire domain.
    pub splits: Vec<BoxSplitWitness>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSolveResult {
    pub solution: SolveResult,
    pub certificate: BoxGlobalCertificate,
}
#[derive(Clone, Copy)]
struct Interval {
    lo: f64,
    hi: f64,
}
struct SearchBox {
    bounds: Vec<(f64, f64)>,
    lower: f64,
    proof_id: u64,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn absolute_gap(upper: f64, lower: f64) -> Result<f64> {
    let gap = (upper - lower).max(0.0);
    if gap.is_finite() { Ok(gap) } else { Err(error("numeric_error", "global gap is not representable")) }
}
fn enclosure(lo: f64, hi: f64) -> Result<Interval> {
    if !lo.is_finite() || !hi.is_finite() {
        return Err(error("numeric_error", "interval bound is not finite"));
    }
    let (lo, hi) = (lo.next_down(), hi.next_up());
    if !lo.is_finite() || !hi.is_finite() {
        return Err(error("numeric_error", "outward interval bound overflow"));
    }
    Ok(Interval { lo, hi })
}
fn product(a: Interval, b: Interval) -> Result<Interval> {
    let values = [a.lo * b.lo, a.lo * b.hi, a.hi * b.lo, a.hi * b.hi];
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("numeric_error", "interval product overflow"));
    }
    enclosure(values.iter().copied().fold(f64::INFINITY, f64::min), values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}
fn polynomial(expr: &Expr) -> bool {
    match expr {
        Expr::Constant { .. } | Expr::Variable { .. } => true,
        Expr::Add { left, right } | Expr::Subtract { left, right } | Expr::Multiply { left, right } => polynomial(left) && polynomial(right),
        Expr::Negate { value } => polynomial(value),
        Expr::Divide { left, right } => polynomial(left) && matches!(right.as_ref(),Expr::Constant{value}if *value!=0.0),
        Expr::Power { base, exponent } => *exponent <= 64 && polynomial(base),
        _ => false,
    }
}
fn interval(expr: &Expr, bounds: &[(f64, f64)]) -> Result<Interval> {
    match expr {
        Expr::Constant { value } => Ok(Interval { lo: *value, hi: *value }),
        Expr::Variable { index } => {
            bounds.get(*index).map(|(lo, hi)| Interval { lo: *lo, hi: *hi }).ok_or_else(|| error("invalid_variable", "interval variable is absent"))
        }
        Expr::Add { left, right } => {
            let a = interval(left, bounds)?;
            let b = interval(right, bounds)?;
            enclosure(a.lo + b.lo, a.hi + b.hi)
        }
        Expr::Subtract { left, right } => {
            let a = interval(left, bounds)?;
            let b = interval(right, bounds)?;
            enclosure(a.lo - b.hi, a.hi - b.lo)
        }
        Expr::Negate { value } => {
            let a = interval(value, bounds)?;
            Ok(Interval { lo: -a.hi, hi: -a.lo })
        }
        Expr::Multiply { left, right } => product(interval(left, bounds)?, interval(right, bounds)?),
        Expr::Divide { left, right } => {
            let a = interval(left, bounds)?;
            let b = interval(right, bounds)?;
            if b.lo != b.hi || b.lo == 0.0 {
                return Err(error("unsupported_problem", "global interval division requires a nonzero literal constant"));
            }
            if b.lo > 0.0 { enclosure(a.lo / b.lo, a.hi / b.lo) } else { enclosure(a.hi / b.lo, a.lo / b.lo) }
        }
        Expr::Power { base, exponent } => {
            let a = interval(base, bounds)?;
            if *exponent == 0 {
                return Ok(Interval { lo: 1.0, hi: 1.0 });
            }
            // Monotone endpoints avoid dependency loss from x*x. Endpoint powers
            // use repeated rounded interval products instead of platform powf.
            let endpoint = |x: f64| -> Result<Interval> {
                let mut r = Interval { lo: 1.0, hi: 1.0 };
                for _ in 0..*exponent {
                    r = product(r, Interval { lo: x, hi: x })?;
                }
                Ok(r)
            };
            let l = endpoint(a.lo)?;
            let h = endpoint(a.hi)?;
            if exponent % 2 == 0 {
                Ok(Interval { lo: if a.lo <= 0.0 && a.hi >= 0.0 { 0.0 } else { l.lo.min(h.lo).max(0.0) }, hi: l.hi.max(h.hi) })
            } else {
                Ok(Interval { lo: l.lo, hi: h.hi })
            }
        }
        _ => Err(error("unsupported_problem", "global certificate supports polynomial expressions only")),
    }
}
fn signed(expr: &Expr, bounds: &[(f64, f64)], direction: Direction) -> Result<Interval> {
    let a = interval(expr, bounds)?;
    Ok(if direction == Direction::Minimize { a } else { Interval { lo: -a.hi, hi: -a.lo } })
}
fn midpoint(bounds: &[(f64, f64)]) -> Vec<f64> {
    bounds.iter().map(|(l, h)| midpoint_coordinate(*l, *h)).collect()
}
fn midpoint_coordinate(lower: f64, upper: f64) -> f64 {
    if lower == upper {
        return lower;
    }
    let width = upper - lower;
    let proposed = if width.is_finite() { lower + 0.5 * width } else { 0.5 * lower + 0.5 * upper };
    proposed.max(lower).min(upper)
}
fn original_feasible(model: &ExpressionModel, values: &[f64]) -> Result<bool> {
    if values.len() != model.variables.len() {
        return Ok(false);
    }
    for (value, variable) in values.iter().zip(&model.variables) {
        if !value.is_finite() || variable.lower.is_some_and(|lo| *value < lo) || variable.upper.is_some_and(|hi| *value > hi) {
            return Ok(false);
        }
    }
    for row in &model.constraints {
        let left = evaluate(&row.left, values)?;
        let right = evaluate(&row.right, values)?;
        let valid = match row.relation {
            solvercraft_types::Relation::Equal => left == right,
            solvercraft_types::Relation::LessEqual => left <= right,
            solvercraft_types::Relation::GreaterEqual => left >= right,
        };
        if !valid {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Produce a checkable bound/gap for a finite continuous box. Unsupported
/// constraints, integer domains and transcendental functions are explicit errors.
/// Interrupted searches retain a valid bound for every unsearched box.
pub fn solve_box_polynomial(model: &ExpressionModel, options: &SolveOptions, control: &dyn RunControl) -> Result<GlobalSolveResult> {
    validate_expression_model(model, options)?;
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::BranchAndBound) {
        return Err(error("unsupported_method", "interval global search requires auto or branch_and_bound"));
    }
    if !model.constraints.is_empty() || model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(error("unsupported_problem", "box global search requires continuous variables and no general constraints"));
    }
    if model.variables.len() > 64 {
        return Err(error("model_limit", "interval global search supports at most 64 variables"));
    }
    if !polynomial(&model.objective) {
        return Err(error("unsupported_problem", "global certificate requires a polynomial AST with powers at most 64 and constant division"));
    }
    let mut domain = Vec::with_capacity(model.variables.len());
    for v in &model.variables {
        match (v.lower, v.upper) {
            (Some(l), Some(h)) => domain.push((l, h)),
            _ => return Err(error("unsupported_problem", "global certificate requires finite lower and upper bounds")),
        }
    }
    let root = signed(&model.objective, &domain, model.direction)?;
    let bytes_per_box =
        domain.len().checked_mul(16).and_then(|n| n.checked_add(64)).ok_or_else(|| error("memory_limit", "global box size overflow"))?;
    let capacity = usize::try_from(options.max_memory_bytes / 4).unwrap_or(usize::MAX) / bytes_per_box;
    if capacity == 0 {
        return Err(error("memory_limit", "global search cannot store its root box"));
    }
    let mut queue = vec![SearchBox { bounds: domain.clone(), lower: root.lo, proof_id: 0 }];
    let mut splits = Vec::new();
    let mut incumbent: Option<(Vec<f64>, f64, f64)> = None;
    let mut lower = root.lo;
    let mut explored = 0u64;
    let mut evaluations = 0u64;
    let mut iterations = 0u64;
    let mut termination = Termination::Completed;
    let mut certified = false;
    loop {
        if control.is_cancelled() {
            termination = Termination::Cancelled;
            break;
        }
        let queue_lower = queue.iter().map(|b| b.lower).fold(f64::INFINITY, f64::min);
        lower = if let Some((_, _, upper)) = &incumbent { queue_lower.min(*upper) } else { queue_lower };
        if let Some((_, _, upper)) = &incumbent {
            let gap = absolute_gap(*upper, lower)?;
            if gap <= options.optimality_tolerance || gap / (1.0 + upper.abs()) <= options.relative_gap {
                certified = true;
                break;
            }
        }
        if queue.is_empty() {
            certified = incumbent.is_some();
            break;
        }
        if evaluations >= options.max_evaluations {
            termination = Termination::EvaluationLimit;
            break;
        }
        if explored >= options.max_nodes {
            termination = Termination::NodeLimit;
            break;
        }
        if iterations >= options.max_iterations {
            termination = Termination::IterationLimit;
            break;
        }
        let index = queue
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.lower.total_cmp(&b.lower))
            .map(|(i, _)| i)
            .ok_or_else(|| error("internal_error", "global box queue is empty"))?;
        // Retain the parent until both children can be stored, so a budget stop
        // cannot discard the bound on an unsearched region.
        if queue.len().saturating_add(1) > capacity {
            termination = Termination::MemoryLimit;
            break;
        }
        if splits.len() >= 100_000
            || u64::try_from(splits.len().saturating_add(1)).map_or(true, |n| n.saturating_mul(64) > options.max_memory_bytes / 4)
        {
            termination = Termination::MemoryLimit;
            break;
        }
        let parent = queue.get(index).ok_or_else(|| error("internal_error", "global parent is absent"))?;
        let x = midpoint(&parent.bounds);
        if !original_feasible(model, &x)? {
            return Err(error("internal_error", "global candidate is outside the original feasible domain"));
        }
        evaluations += 1;
        let value = evaluate(&model.objective, &x)?;
        let point_bounds: Vec<_> = x.iter().map(|v| (*v, *v)).collect();
        let enclosure = signed(&model.objective, &point_bounds, model.direction)?;
        if incumbent.as_ref().is_none_or(|(_, _, u)| enclosure.hi < *u) {
            incumbent = Some((x, value, enclosure.hi));
        }
        explored += 1;
        iterations += 1;
        if let Some((_, _, upper)) = &incumbent {
            let lower = queue.iter().map(|b| b.lower).fold(*upper, f64::min);
            let gap = absolute_gap(*upper, lower)?;
            if gap <= options.optimality_tolerance || gap / (1.0 + upper.abs()) <= options.relative_gap {
                certified = true;
                break;
            }
        }
        let split = parent
            .bounds
            .iter()
            .enumerate()
            .filter_map(|(i, (l, h))| {
                let m = midpoint_coordinate(*l, *h);
                if m > *l && m < *h { Some((i, h - l, m)) } else { None }
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((axis, _, mid)) = split else {
            // A floating-point singleton can retain an enclosure wider than the
            // requested tolerance. Keep its bound and report numerical limits.
            termination = Termination::NumericalFailure;
            break;
        };
        let mut a = parent.bounds.clone();
        let mut b = parent.bounds.clone();
        if let Some(v) = a.get_mut(axis) {
            v.1 = mid;
        }
        if let Some(v) = b.get_mut(axis) {
            v.0 = mid;
        }
        let ia = signed(&model.objective, &a, model.direction)?;
        let ib = signed(&model.objective, &b, model.direction)?;
        let parent_id = parent.proof_id;
        let left_id = 2 * splits.len() as u64 + 1;
        let right_id = left_id + 1;
        splits.push(BoxSplitWitness { parent: parent_id, axis, midpoint: mid });
        queue.swap_remove(index);
        let upper = incumbent.as_ref().map_or(f64::INFINITY, |(_, _, u)| *u);
        if ia.lo <= upper {
            queue.push(SearchBox { bounds: a, lower: ia.lo, proof_id: left_id });
        }
        if ib.lo <= upper {
            queue.push(SearchBox { bounds: b, lower: ib.lo, proof_id: right_id });
        }
        queue.retain(|b| b.lower <= upper);
        control.progress(&ProgressEvent {
            stage: "interval_branch_and_bound".into(),
            iterations,
            evaluations,
            nodes: explored,
            best_objective: incumbent.as_ref().map(|(_, v, _)| *v),
        });
    }
    // Recompute the final lower bound after the last split/pruning operation.
    if let Some((_, _, upper)) = &incumbent {
        lower = queue.iter().map(|b| b.lower).fold(*upper, f64::min);
    }
    let upper = incumbent.as_ref().map(|(_, _, u)| *u);
    if let Some((values, value, _)) = &incumbent
        && (!original_feasible(model, values)? || evaluate(&model.objective, values)? != *value)
    {
        return Err(error("internal_error", "global incumbent failed original-model verification"));
    }
    let gap = upper.map(|u| absolute_gap(u, lower)).transpose()?;
    let original_bound = if model.direction == Direction::Minimize { lower } else { -lower };
    let relative_gap = gap.zip(upper).map(|(g, u)| g / (1.0 + u.abs()));
    let (values, objective) = incumbent.map_or((None, None), |(x, v, _)| (Some(x), Some(v)));
    let solution = SolveResult {
        termination,
        feasibility: if values.is_some() { Feasibility::Feasible } else { Feasibility::Unknown },
        optimality: if certified { Optimality::Optimal } else if values.is_some() { Optimality::BestFound } else { Optimality::Unknown },
        values,
        objective,
        best_bound: Some(original_bound),
        relative_gap,
        max_violation: upper.map(|_| 0.0),
        statistics: SolveStatistics { evaluations, iterations, nodes: explored },
        engine: "solvercraft-interval-box-polynomial".into(),
        warnings: vec!["Global certificate applies only to the supplied continuous finite box and supported polynomial AST; bounds use outward-rounded interval arithmetic.".into()],
    };
    let certificate = BoxGlobalCertificate {
        domain,
        signed_lower_bound: Some(lower),
        signed_incumbent_upper_bound: upper,
        absolute_gap: gap,
        remaining_boxes: queue.len(),
        explored_boxes: explored,
        tolerance_certified: certified,
        splits,
    };
    Ok(GlobalSolveResult { solution, certificate })
}

/// Independently replay a domain partition and recompute every leaf enclosure
/// and the incumbent enclosure. Tampered bounds, missing parents, nonpartitions
/// and unsupported expressions are rejected rather than trusted as proof.
pub fn verify_box_certificate(model: &ExpressionModel, report: &GlobalSolveResult, options: &SolveOptions) -> Result<bool> {
    validate_expression_model(model, options)?;
    if !model.constraints.is_empty()
        || !polynomial(&model.objective)
        || model.variables.len() > 64
        || model.variables.iter().any(|v| v.kind != VariableKind::Continuous)
    {
        return Ok(false);
    }
    let certificate = &report.certificate;
    if (report.solution.optimality == Optimality::Optimal) != certificate.tolerance_certified
        || certificate.tolerance_certified && report.solution.termination != Termination::Completed
    {
        return Ok(false);
    }
    if certificate.explored_boxes != report.solution.statistics.nodes {
        return Ok(false);
    }
    if certificate.splits.len() > 100_000 || certificate.domain.len() != model.variables.len() {
        return Ok(false);
    }
    for (domain, v) in certificate.domain.iter().zip(&model.variables) {
        if v.lower != Some(domain.0) || v.upper != Some(domain.1) {
            return Ok(false);
        }
    }
    let slots = certificate
        .splits
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(model.variables.len().saturating_mul(16).saturating_add(128)))
        .ok_or_else(|| error("memory_limit", "certificate replay size overflow"))?;
    if u64::try_from(slots).map_or(true, |n| n > options.max_memory_bytes) {
        return Err(error("memory_limit", "certificate replay exceeds memory budget"));
    }
    let mut leaves = BTreeMap::new();
    leaves.insert(0u64, certificate.domain.clone());
    for (index, split) in certificate.splits.iter().enumerate() {
        let Some(parent) = leaves.remove(&split.parent) else {
            return Ok(false);
        };
        let Some((l, h)) = parent.get(split.axis).copied() else {
            return Ok(false);
        };
        if !split.midpoint.is_finite() || split.midpoint <= l || split.midpoint >= h {
            return Ok(false);
        }
        let mut left = parent.clone();
        let mut right = parent;
        if let Some(v) = left.get_mut(split.axis) {
            v.1 = split.midpoint;
        }
        if let Some(v) = right.get_mut(split.axis) {
            v.0 = split.midpoint;
        }
        let id = 2 * index as u64 + 1;
        leaves.insert(id, left);
        leaves.insert(id + 1, right);
    }
    let mut lower = f64::INFINITY;
    for bounds in leaves.values() {
        lower = lower.min(signed(&model.objective, bounds, model.direction)?.lo);
    }
    let Some(claimed) = certificate.signed_lower_bound else {
        return Ok(false);
    };
    if !claimed.is_finite() || claimed > lower {
        return Ok(false);
    }
    if report.solution.best_bound != Some(if model.direction == Direction::Minimize { claimed } else { -claimed }) {
        return Ok(false);
    }
    let Some(values) = &report.solution.values else {
        return Ok(!certificate.tolerance_certified
            && certificate.signed_incumbent_upper_bound.is_none()
            && certificate.absolute_gap.is_none()
            && report.solution.feasibility == Feasibility::Unknown
            && report.solution.optimality == Optimality::Unknown
            && report.solution.objective.is_none()
            && report.solution.relative_gap.is_none()
            && report.solution.max_violation.is_none());
    };
    if report.solution.feasibility != Feasibility::Feasible || report.solution.max_violation != Some(0.0) {
        return Ok(false);
    }
    if values.len() != certificate.domain.len() || values.iter().zip(&certificate.domain).any(|(x, (l, h))| !x.is_finite() || x < l || x > h) {
        return Ok(false);
    }
    if !original_feasible(model, values)? {
        return Ok(false);
    }
    let point: Vec<_> = values.iter().map(|v| (*v, *v)).collect();
    let upper = signed(&model.objective, &point, model.direction)?.hi;
    let Some(claimed_upper) = certificate.signed_incumbent_upper_bound else {
        return Ok(false);
    };
    if !claimed_upper.is_finite() || claimed_upper < upper || report.solution.objective != Some(evaluate(&model.objective, values)?) {
        return Ok(false);
    }
    let gap = absolute_gap(claimed_upper, claimed)?;
    if certificate.absolute_gap != Some(gap) || report.solution.relative_gap != Some(gap / (1.0 + claimed_upper.abs())) {
        return Ok(false);
    }
    Ok(!certificate.tolerance_certified || gap <= options.optimality_tolerance || gap / (1.0 + claimed_upper.abs()) <= options.relative_gap)
}

#[cfg(test)]
#[path = "global_tests.rs"]
mod tests;
