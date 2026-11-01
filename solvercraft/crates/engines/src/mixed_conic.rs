//! Finite-domain branch-and-bound using genuine convex conic relaxations.
use crate::{ConicModel, solve_socp_owned, validate_conic_solution};
use serde::{Deserialize, Serialize};
use solvercraft_model::effective_bounds;
use solvercraft_types::*;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedConicResult {
    pub result: SolveResult,
    pub relaxation_solves: u64,
    pub unresolved_leaves: u64,
}
struct Node {
    bounds: Vec<(f64, f64)>,
    bound: Option<f64>,
}
fn working(model: &OptimizationModel, x: &[f64], options: &SolveOptions) -> Result<f64> {
    let value = crate::working_objective(model, x, options)?;
    Ok(if model.objective.direction == Direction::Minimize { value } else { -value })
}
fn best_bound(nodes: &[Node]) -> Option<f64> {
    nodes.iter().try_fold(f64::INFINITY, |bound, node| Some(bound.min(node.bound?))).filter(|v| v.is_finite())
}
struct NodeControl {
    parent: Arc<dyn RunControl>,
    iterations: u64,
    evaluations: u64,
    nodes: u64,
}
impl RunControl for NodeControl {
    fn is_cancelled(&self) -> bool {
        self.parent.is_cancelled()
    }
    fn progress(&self, event: &ProgressEvent) {
        self.parent.progress(&ProgressEvent {
            stage: "conic_branch_and_bound".into(),
            iterations: self.iterations + event.iterations,
            evaluations: self.evaluations + event.evaluations,
            nodes: self.nodes,
            best_objective: None,
        });
    }
}
/// Bounded integer/binary variables, with continuous LP/QP/SOCP subproblems.
/// No unbounded continuous relaxation is promoted to integer unboundedness.
pub fn solve_mixed_conic_owned(model: &ConicModel, options: &SolveOptions, control: Arc<dyn RunControl>) -> Result<MixedConicResult> {
    // Check the full cone model before integer-index storage or relaxation
    // copies; validating only the algebraic base cannot bound cone memory.
    if let Err(error) = crate::conic::preflight(model, options, control.as_ref()) {
        if error.code == "cancelled" {
            let mut result = crate::empty_result(Termination::Cancelled, Feasibility::Unknown, Optimality::Unknown);
            result.engine = "solvercraft-conic-branch-and-bound-0.2".into();
            return Ok(MixedConicResult { result, relaxation_solves: 0, unresolved_leaves: 0 });
        }
        return Err(error);
    }
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::BranchAndBound) {
        return Err(SolverError::new("unsupported_method", "Mixed conic search requires auto or branch_and_bound."));
    }
    let integer = model.base.variables.iter().enumerate().filter_map(|(i, v)| (v.kind != VariableKind::Continuous).then_some(i)).collect::<Vec<_>>();
    if integer.is_empty() {
        let continuous_options = SolveOptions { method: SolveMethod::Auto, ..options.clone() };
        let r = solve_socp_owned(model, &continuous_options, control)?;
        return Ok(MixedConicResult { result: r.result, relaxation_solves: 1, unresolved_leaves: 0 });
    }
    let mut bounds = Vec::with_capacity(integer.len());
    for i in &integer {
        let (low, high) = effective_bounds(&model.base.variables[*i])?;
        let (Some(low), Some(high)) = (low, high) else {
            return Err(SolverError::new(
                "finite_integer_domain_required",
                "Mixed conic branch-and-bound currently requires finite integer domains.",
            ));
        };
        if low.abs() > 9_007_199_254_740_991.0 || high.abs() > 9_007_199_254_740_991.0 {
            return Err(SolverError::new("integer_range", "Integer bounds exceed exact f64 adjacent-integer range."));
        }
        bounds.push((low.ceil(), high.floor()));
    }
    let mut pending = vec![Node { bounds, bound: None }];
    let mut result = crate::empty_result(Termination::Completed, Feasibility::Unknown, Optimality::Unknown);
    result.engine = "solvercraft-conic-branch-and-bound-0.2".into();
    result.warnings = vec![
        "Finite-domain conic branch-and-bound with independently checked floating-point relaxation bounds; no exact rational certificate.".into(),
    ];
    let mut incumbent: Option<f64> = None;
    let mut unresolved = 0;
    let mut unresolved_bound = f64::INFINITY;
    let mut unresolved_missing = false;
    let mut closed_bound = f64::INFINITY;
    while let Some(node) = pending.pop() {
        let stop = if control.is_cancelled() {
            Some(Termination::Cancelled)
        } else if result.statistics.nodes >= options.max_nodes {
            Some(Termination::NodeLimit)
        } else if result.statistics.iterations >= options.max_iterations {
            Some(Termination::IterationLimit)
        } else if result.statistics.evaluations >= options.max_evaluations {
            Some(Termination::EvaluationLimit)
        } else {
            None
        };
        if let Some(stop) = stop {
            result.termination = stop;
            pending.push(node);
            break;
        }
        if incumbent.zip(node.bound).is_some_and(|(upper, lower)| lower >= upper) {
            closed_bound = closed_bound.min(node.bound.unwrap_or(f64::INFINITY));
            continue;
        }
        let mut relaxation = model.clone();
        for (j, i) in integer.iter().enumerate() {
            relaxation.base.variables[*i].kind = VariableKind::Continuous;
            relaxation.base.variables[*i].lower = Some(node.bounds[j].0);
            relaxation.base.variables[*i].upper = Some(node.bounds[j].1);
        }
        let suboptions = SolveOptions {
            method: SolveMethod::Auto,
            max_iterations: options.max_iterations - result.statistics.iterations,
            max_evaluations: options.max_evaluations - result.statistics.evaluations,
            ..options.clone()
        };
        result.statistics.nodes += 1;
        let callback = Arc::new(NodeControl {
            parent: Arc::clone(&control),
            iterations: result.statistics.iterations,
            evaluations: result.statistics.evaluations,
            nodes: result.statistics.nodes,
        });
        let relaxed = solve_socp_owned(&relaxation, &suboptions, callback)?;
        result.statistics.iterations += relaxed.result.statistics.iterations;
        result.statistics.evaluations += relaxed.result.statistics.evaluations;
        if relaxed.result.feasibility == Feasibility::Infeasible {
            continue;
        }
        let bound = relaxed
            .certificate
            .as_ref()
            .and_then(|c| c.working_dual_bound)
            .map(|v| if model.base.objective.direction == Direction::Minimize { v } else { -v })
            .or(node.bound);
        if incumbent.zip(bound).is_some_and(|(upper, lower)| lower >= upper) {
            closed_bound = closed_bound.min(bound.unwrap_or(f64::INFINITY));
            continue;
        }
        let mut fractional = None;
        if let Some(point) = &relaxed.result.values {
            let mut candidate = point.clone();
            let mut deviation = 0.0_f64;
            for (j, i) in integer.iter().enumerate() {
                let rounded = point[*i].round();
                let distance = (point[*i] - rounded).abs();
                candidate[*i] = rounded;
                if distance > options.integrality_tolerance && distance > deviation && node.bounds[j].0 < node.bounds[j].1 {
                    fractional = Some((j, point[*i].floor()));
                    deviation = distance;
                }
            }
            if validate_conic_solution(model, &candidate, options)?.0 {
                let cost = working(&model.base, &candidate, options)?;
                if incumbent.is_none_or(|old| cost < old) {
                    let verified = crate::verified_result(&model.base, options, candidate, Optimality::BestFound)?;
                    result.values = verified.values;
                    result.objective = verified.objective;
                    result.max_violation = verified.max_violation;
                    result.feasibility = Feasibility::Feasible;
                    result.optimality = Optimality::BestFound;
                    incumbent = Some(cost);
                }
            }
        }
        if relaxed.result.termination != Termination::Completed {
            result.termination = relaxed.result.termination;
            pending.push(Node { bounds: node.bounds, bound });
            break;
        }
        if incumbent.zip(bound).is_some_and(|(upper, lower)| upper - lower <= options.optimality_tolerance) {
            closed_bound = closed_bound.min(bound.unwrap_or(f64::INFINITY));
            continue;
        }
        let branch =
            fractional.or_else(|| node.bounds.iter().enumerate().find(|(_, b)| b.0 < b.1).map(|(j, b)| (j, (0.5 * b.0 + 0.5 * b.1).floor())));
        if let Some((j, split)) = branch {
            let split = split.max(node.bounds[j].0).min(node.bounds[j].1 - 1.0);
            let projected = (pending.len() as u64 + 2).checked_mul(integer.len() as u64).and_then(|n| n.checked_mul(32));
            if projected.is_none_or(|bytes| bytes > options.max_memory_bytes / 4) {
                result.termination = Termination::MemoryLimit;
                pending.push(Node { bounds: node.bounds, bound });
                break;
            }
            let mut low = node.bounds.clone();
            let mut high = node.bounds;
            low[j].1 = split;
            high[j].0 = split + 1.0;
            pending.push(Node { bounds: high, bound });
            pending.push(Node { bounds: low, bound });
        } else {
            // Every independently justified closure already passed the
            // incumbent/bound test above. A backend Optimal label alone does
            // not close a fixed-domain leaf after rounding changed its value
            // or when its certified bound is absent/weak.
            unresolved += 1;
            if let Some(b) = bound {
                unresolved_bound = unresolved_bound.min(b);
            } else {
                unresolved_missing = true;
            }
        }
    }
    let frontier = if pending.is_empty() { Some(f64::INFINITY) } else { best_bound(&pending) };
    let global = if unresolved_missing { None } else { frontier.map(|b| b.min(closed_bound).min(unresolved_bound)).filter(|b| b.is_finite()) };
    if result.termination == Termination::Completed && pending.is_empty() && unresolved == 0 {
        if let Some(cost) = incumbent {
            if global.is_some_and(|bound| cost - bound <= options.optimality_tolerance) {
                result.optimality = Optimality::Optimal;
            } else if options.relative_gap > 0.0 {
                result.warnings.push(
                    "Requested positive mixed-integer relative gap does not establish numerical zero-gap optimality; BestFound is retained.".into(),
                );
            }
        } else {
            result.feasibility = Feasibility::Infeasible;
        }
    }
    if let Some(bound) = global {
        result.best_bound = crate::reported_bound(&model.base, bound).map(|v| v.0);
        result.relative_gap = incumbent.map(|cost| (cost - bound).max(0.0) / cost.abs().max(bound.abs()).max(1.0));
    }
    if unresolved > 0 {
        result.warnings.push(format!(
            "{unresolved} fixed-integer leaves lack a checked continuous optimality/infeasibility certificate; global optimality remains unknown."
        ));
    }
    Ok(MixedConicResult { relaxation_solves: result.statistics.nodes, result, unresolved_leaves: unresolved })
}
