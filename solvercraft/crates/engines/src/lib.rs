//! Budgeted, workbook-independent floating-point LP and MILP engines.
//!
//! Independent dense simplex and proximal ADMM coexist with a genuine sparse
//! Clarabel LP/QP/SOCP path, finite-domain mixed-conic search, checked LP vertex
//! bases/sensitivity, and explicit bounded SOS/indicator formulations.
//! Guarantees and capability limits are documented per API; no exact rational
//! or unrestricted commercial-suite parity claim is made.
#![forbid(unsafe_code)]

mod basis;
mod conic;
mod discrete;
mod linear_algebra;
mod mixed_conic;
mod quadratic;
mod separable_qp;
mod simplex;
pub use basis::{AllowableChange, LpBasis, LpBasisResult, LpSensitivity, solve_lp_with_basis};
pub use conic::{
    AffineExpression, ConicCertificate, ConicDetailedResult, ConicModel, SecondOrderConeConstraint, solve_socp_detailed, solve_socp_owned,
    solve_sparse_owned, validate_conic_solution,
};
pub use discrete::{
    DiscreteCompilation, DiscreteConicModel, IndicatorConstraint, SosConstraint, SosKind, compile_discrete, solve_discrete_owned,
    validate_discrete_solution,
};
pub use mixed_conic::{MixedConicResult, solve_mixed_conic_owned};
pub use quadratic::{
    ConstraintDual, ConstraintSource, ConvexityAnalysis, QpCertificate, QpDetailedResult, QpWarmStart, analyze_quadratic_curvature, solve_qp_detailed,
};

use solvercraft_model::{ExactProducts, compensated_sum, effective_bounds, evaluate_working_objective, validate, validate_solution};
use solvercraft_types::*;

/// Capabilities describe implemented behavior, not aspirational backend names.
pub const ENGINE_NAME: &str = "solvercraft-dense-simplex-0.3";

/// A containing box implied by a canonical model. Single-coordinate rows are
/// genuine domain restrictions even when the variable metadata has an open
/// side. This is used only for independent certificates, never to round or
/// replace the user's constraints. Division is widened outwards by one ULP.
fn certificate_box(model: &OptimizationModel, initial: &[(Option<f64>, Option<f64>)]) -> Vec<(Option<f64>, Option<f64>)> {
    let mut bounds = initial.to_vec();
    for row in &model.constraints {
        let [term] = row.terms.as_slice() else { continue };
        if term.coefficient == 0.0 {
            continue;
        }
        let Some((lower, upper)) = bounds.get_mut(term.variable) else { continue };
        let ratio = row.rhs / term.coefficient;
        if !ratio.is_finite() {
            continue;
        }
        let at_least = match row.relation {
            Relation::GreaterEqual => term.coefficient > 0.0,
            Relation::LessEqual => term.coefficient < 0.0,
            Relation::Equal => true,
        };
        if at_least || row.relation == Relation::Equal {
            let value = ratio.next_down();
            if value.is_finite() {
                *lower = Some(lower.map_or(value, |old| old.max(value)));
            }
        }
        if !at_least || row.relation == Relation::Equal {
            let value = ratio.next_up();
            if value.is_finite() {
                *upper = Some(upper.map_or(value, |old| old.min(value)));
            }
        }
    }
    bounds
}

/// Owned-control auto route, including genuine finite-domain MIQP relaxations.
/// This is the portable entry point for job systems already owning cancellation.
pub fn solve_owned(model: &OptimizationModel, options: &SolveOptions, control: std::sync::Arc<dyn RunControl>) -> Result<SolveResult> {
    if !model.objective.quadratic.is_empty() && model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        // Admission must precede the owned adapter copy. Match the borrowed
        // entry point's domain and budget semantics rather than copying first.
        if let Err(error) = validate(model, options) {
            if error.code == "infeasible_bounds" {
                return Ok(empty_result(Termination::Completed, Feasibility::Infeasible, Optimality::Unknown));
            }
            return Err(error);
        }
        return solve_mixed_conic_owned(&ConicModel { base: model.clone(), second_order_cones: Vec::new() }, options, control).map(|r| r.result);
    }
    solve(model, options, control.as_ref())
}

/// Solve an LP, MILP, or continuous convex QP without mutating the supplied model.
///
/// General linearly constrained QP uses checked proximal ADMM. Explicit sparse,
/// conic, mixed-conic and basis APIs expose their additional result contracts.
/// Unsupported nonlinear-method requests are explicit errors.
/// An unbounded LP relaxation alone never establishes MILP unboundedness.
pub fn solve(model: &OptimizationModel, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    if let Err(error) = validate(model, options) {
        if error.code == "infeasible_bounds" {
            return Ok(empty_result(Termination::Completed, Feasibility::Infeasible, Optimality::Unknown));
        }
        return Err(error);
    }
    if !model.objective.quadratic.is_empty() {
        let separable = model.constraints.is_empty()
            && model.variables.iter().all(|v| v.kind == VariableKind::Continuous)
            && model.objective.quadratic.iter().all(|term| term.left == term.right);
        return if separable {
            separable_qp::solve(model, options, control)
        } else {
            solve_qp_detailed(model, options, control, None).map(|d| d.result)
        };
    }
    if matches!(options.method, SolveMethod::ProjectedGradient | SolveMethod::DifferentialEvolution) {
        return Err(SolverError::new("unsupported_method", "The selected nonlinear method is not implemented by this LP/MILP backend."));
    }
    let integer = model.variables.iter().any(|v| v.kind != VariableKind::Continuous);
    if integer && options.method == SolveMethod::Simplex {
        return Err(SolverError::new(
            "unsupported_method",
            "Simplex alone is a continuous relaxation. Use auto or branch_and_bound to honor integer domains.",
        ));
    }
    let mut bounds = Vec::with_capacity(model.variables.len());
    for v in &model.variables {
        let (lo, hi) = effective_bounds(v)?;
        bounds.push((lo, hi));
    }
    let mut budget = Budget::new(options, control);
    if integer { solve_mip(model, options, bounds, &mut budget) } else { solve_continuous(model, options, &bounds, &mut budget) }
}

fn empty_result(termination: Termination, feasibility: Feasibility, optimality: Optimality) -> SolveResult {
    SolveResult {
        termination,
        feasibility,
        optimality,
        values: None,
        objective: None,
        best_bound: None,
        relative_gap: None,
        max_violation: None,
        statistics: SolveStatistics::default(),
        engine: ENGINE_NAME.into(),
        warnings: vec![
            "Floating-point simplex uses documented feasibility and optimality tolerances; this is not an exact rational certificate.".into(),
        ],
    }
}

pub(crate) struct Budget<'a> {
    options: &'a SolveOptions,
    control: &'a dyn RunControl,
    statistics: SolveStatistics,
}
impl<'a> Budget<'a> {
    fn new(options: &'a SolveOptions, control: &'a dyn RunControl) -> Self {
        Self { options, control, statistics: SolveStatistics::default() }
    }
    fn check(&self) -> std::result::Result<(), Termination> {
        if self.control.is_cancelled() { Err(Termination::Cancelled) } else { Ok(()) }
    }
    pub(crate) fn step(&mut self) -> std::result::Result<(), Termination> {
        self.step_named("simplex")
    }
    pub(crate) fn step_named(&mut self, stage: &str) -> std::result::Result<(), Termination> {
        self.check()?;
        if self.statistics.iterations >= self.options.max_iterations {
            return Err(Termination::IterationLimit);
        }
        self.statistics.iterations += 1;
        if self.statistics.iterations % 128 == 1 {
            self.progress(stage, None);
        }
        Ok(())
    }
    fn evaluation(&mut self) -> std::result::Result<(), Termination> {
        self.check()?;
        if self.statistics.evaluations >= self.options.max_evaluations {
            return Err(Termination::EvaluationLimit);
        }
        self.statistics.evaluations += 1;
        Ok(())
    }
    fn progress(&self, stage: &str, best: Option<f64>) {
        self.control.progress(&ProgressEvent {
            stage: stage.into(),
            iterations: self.statistics.iterations,
            evaluations: self.statistics.evaluations,
            nodes: self.statistics.nodes,
            best_objective: best,
        });
    }
}

pub(crate) enum Relaxation {
    Optimal(Vec<f64>, Option<f64>),
    Infeasible,
    Unbounded(Vec<f64>),
    Stopped(Termination),
    Numerical(String),
}

fn solve_continuous(
    model: &OptimizationModel,
    options: &SolveOptions,
    bounds: &[(Option<f64>, Option<f64>)],
    budget: &mut Budget<'_>,
) -> Result<SolveResult> {
    let outcome = simplex::relax(model, bounds, budget);
    let mut result = match outcome {
        Relaxation::Optimal(values, certified) => {
            let mut result = verified_result(model, options, values, Optimality::Optimal)?;
            if let Some(bound) = certified
                && result.feasibility == Feasibility::Feasible
            {
                result.best_bound = reported_bound(model, bound).map(|(value, _)| value);
                result.relative_gap = result.values.as_ref().and_then(|values| {
                    let working = working_objective(model, values, options).ok()?;
                    let direction = model.objective.direction;
                    gap(score(direction, working), bound, score(direction, model.objective.offset))
                });
            }
            result
        }
        Relaxation::Unbounded(values) => {
            let mut r = verified_result(model, options, values, Optimality::Unbounded)?;
            r.best_bound = None;
            r.relative_gap = None;
            r
        }
        Relaxation::Infeasible => empty_result(Termination::Completed, Feasibility::Infeasible, Optimality::Unknown),
        Relaxation::Stopped(reason) => empty_result(reason, Feasibility::Unknown, Optimality::Unknown),
        Relaxation::Numerical(message) => {
            let mut r = empty_result(Termination::NumericalFailure, Feasibility::Unknown, Optimality::Unknown);
            r.warnings.push(message);
            r
        }
    };
    result.statistics = budget.statistics.clone();
    Ok(result)
}

fn verified_result(model: &OptimizationModel, options: &SolveOptions, values: Vec<f64>, optimality: Optimality) -> Result<SolveResult> {
    let validation = validate_solution(model, &values, options)?;
    let mut result = empty_result(Termination::Completed, Feasibility::Unknown, Optimality::Unknown);
    result.max_violation = Some(validation.max_violation);
    if !validation.feasible {
        result.termination = Termination::NumericalFailure;
        result.warnings.push("Backend candidate failed validation against the original model.".into());
        return Ok(result);
    }
    result.feasibility = Feasibility::Feasible;
    result.optimality = optimality;
    result.values = Some(values);
    result.objective = Some(validation.objective);
    if optimality == Optimality::Optimal {
        result.best_bound = Some(validation.objective);
        result.relative_gap = Some(0.0);
    }
    Ok(result)
}

struct Node {
    bounds: Vec<(Option<f64>, Option<f64>)>,
    bound: Option<f64>,
}
fn score(direction: Direction, value: f64) -> f64 {
    if direction == Direction::Minimize { value } else { -value }
}
fn unscore(direction: Direction, value: f64) -> f64 {
    score(direction, value)
}
fn gap(incumbent: f64, bound: f64, offset: f64) -> Option<f64> {
    // Keep the numerator constant-free so a large irrelevant objective offset
    // cannot round an unfinished search gap down to zero.
    let difference = (incumbent - bound).max(0.0);
    let a = incumbent + offset;
    let b = bound + offset;
    if !difference.is_finite() || !a.is_finite() || !b.is_finite() {
        return None;
    }
    Some(difference / a.abs().max(b.abs()).max(1.0))
}
fn working_objective(model: &OptimizationModel, values: &[f64], options: &SolveOptions) -> Result<f64> {
    let value = evaluate_working_objective(model, values)?;
    let mut remainder = ExactProducts::default();
    for term in &model.objective.linear {
        remainder.add_product(term.coefficient, values[term.variable])?;
    }
    for term in &model.objective.quadratic {
        remainder.add_triple(term.coefficient, values[term.left], values[term.right])?;
    }
    remainder.add(-value)?;
    let lost = remainder.finish()?;
    if lost.abs() > options.optimality_tolerance {
        return Err(SolverError::new(
            "numeric_range",
            "Constant-free objective loses a term larger than the optimality tolerance; rescale the objective.",
        ));
    }
    Ok(value)
}
fn restored_objective(model: &OptimizationModel, working_score: f64) -> Option<f64> {
    compensated_sum([model.objective.offset, unscore(model.objective.direction, working_score)]).ok()
}
fn reported_bound(model: &OptimizationModel, working_score: f64) -> Option<(f64, bool)> {
    let value = unscore(model.objective.direction, working_score);
    let rounded = compensated_sum([model.objective.offset, value]).ok()?;
    let lost = compensated_sum([model.objective.offset, value, -rounded]).ok()?;
    let outward = match model.objective.direction {
        Direction::Minimize if lost < 0.0 => rounded.next_down(),
        Direction::Maximize if lost > 0.0 => rounded.next_up(),
        _ => rounded,
    };
    outward.is_finite().then_some((outward, lost != 0.0))
}

fn frontier_bound(frontier: &[Node]) -> Option<f64> {
    let mut best = f64::INFINITY;
    for node in frontier {
        best = best.min(node.bound?);
    }
    if best.is_finite() { Some(best) } else { None }
}

fn solve_mip(
    model: &OptimizationModel,
    options: &SolveOptions,
    bounds: Vec<(Option<f64>, Option<f64>)>,
    budget: &mut Budget<'_>,
) -> Result<SolveResult> {
    let n = model.variables.len();
    let node_bytes =
        (n as u64).checked_mul(48).and_then(|x| x.checked_add(64)).ok_or_else(|| SolverError::new("resource_limit", "Branch state size overflow"))?;
    let mut frontier = vec![Node { bounds, bound: None }];
    let mut incumbent: Option<(Vec<f64>, f64)> = None;
    let mut termination = Termination::Completed;
    let mut warning = None;
    let mut active_bound = None;
    let mut stopped_for_gap = false;
    while !frontier.is_empty() {
        if let Err(reason) = budget.check() {
            termination = reason;
            break;
        }
        if budget.statistics.nodes >= options.max_nodes {
            termination = Termination::NodeLimit;
            break;
        }
        let Some(node) = frontier.pop() else {
            break;
        };
        active_bound = node.bound;
        if let (Some(b), Some((_, v))) = (node.bound, &incumbent)
            && b >= *v - options.optimality_tolerance
        {
            active_bound = None;
            continue;
        }
        budget.statistics.nodes += 1;
        let (values, certified) = match simplex::relax(model, &node.bounds, budget) {
            Relaxation::Optimal(values, bound) => (values, bound),
            Relaxation::Infeasible => {
                active_bound = None;
                continue;
            }
            Relaxation::Unbounded(_) => {
                termination = Termination::NumericalFailure;
                warning=Some("An LP relaxation is unbounded. This does not establish that the mixed-integer model is feasible or unbounded; its status remains unknown.".into());
                break;
            }
            Relaxation::Stopped(reason) => {
                termination = reason;
                break;
            }
            Relaxation::Numerical(message) => {
                termination = Termination::NumericalFailure;
                warning = Some(message);
                break;
            }
        };
        let relaxed_objective = match working_objective(model, &values, options) {
            Ok(value) => value,
            Err(error) => {
                termination = Termination::NumericalFailure;
                warning = Some(error.to_string());
                break;
            }
        };
        let bound = certified.unwrap_or_else(|| score(model.objective.direction, relaxed_objective));
        active_bound = Some(bound);
        if let Some((_, best)) = &incumbent
            && bound >= *best - options.optimality_tolerance
        {
            active_bound = None;
            continue;
        }
        let mut fractional = None;
        let mut fractional_size = 0.0;
        for (j, (v, var)) in values.iter().zip(&model.variables).enumerate() {
            if var.kind != VariableKind::Continuous {
                let delta = (v - v.round()).abs();
                if delta > options.integrality_tolerance && delta > fractional_size {
                    fractional = Some(j);
                    fractional_size = delta;
                }
            }
        }
        if fractional.is_none() {
            let mut rounded = values.clone();
            for (v, var) in rounded.iter_mut().zip(&model.variables) {
                if var.kind != VariableKind::Continuous {
                    *v = v.round();
                }
            }
            let check = validate_solution(model, &rounded, options)?;
            let mut close_node = false;
            if check.feasible {
                let candidate = match working_objective(model, &rounded, options) {
                    Ok(value) => score(model.objective.direction, value),
                    Err(error) => {
                        termination = Termination::NumericalFailure;
                        warning = Some(error.to_string());
                        break;
                    }
                };
                if incumbent.as_ref().is_none_or(|(_, value)| candidate < *value) {
                    incumbent = Some((rounded, candidate));
                }
                // Integer-distance tolerance alone cannot close a node: a tiny
                // fraction can carry a large objective coefficient. Keep the
                // rounded incumbent, but require its actual LP gap to be small.
                let mut difference = solvercraft_model::ExactProducts::default();
                difference.add(candidate)?;
                difference.add(-bound)?;
                difference.add(-options.optimality_tolerance)?;
                close_node = difference.exact_sign()? != std::cmp::Ordering::Greater;
            }
            if close_node {
                active_bound = None;
            } else {
                // Branch on any actual fractional coordinate when rounding
                // loses feasibility OR leaves a material objective gap.
                for (j, (v, var)) in values.iter().zip(&model.variables).enumerate() {
                    let delta = (v - v.round()).abs();
                    if var.kind != VariableKind::Continuous && delta > 0.0 && delta > fractional_size {
                        fractional = Some(j);
                        fractional_size = delta;
                    }
                }
                if fractional.is_none() {
                    termination = Termination::NumericalFailure;
                    warning = Some("An integral LP candidate could not close its original-model feasibility/objective certificate.".into());
                    break;
                }
            }
        }
        if let Some(j) = fractional {
            let Some(value) = values.get(j).copied() else {
                return Err(SolverError::new("internal_error", "Invalid branch variable"));
            };
            let pending = frontier.len().checked_add(2).and_then(|x| (x as u64).checked_mul(node_bytes));
            if pending.is_none_or(|bytes| bytes > options.max_memory_bytes / 2) {
                termination = Termination::MemoryLimit;
                break;
            }
            let mut down = node.bounds.clone();
            let mut up = node.bounds;
            if let Some((_, hi)) = down.get_mut(j) {
                *hi = Some(hi.map_or(value.floor(), |h| h.min(value.floor())));
            }
            if let Some((lo, _)) = up.get_mut(j) {
                *lo = Some(lo.map_or(value.ceil(), |l| l.max(value.ceil())));
            }
            let valid = |b: &[(Option<f64>, Option<f64>)]| b.iter().all(|(l, h)| !matches!((l,h),(Some(a),Some(z)) if a>z));
            if valid(&up) {
                frontier.push(Node { bounds: up, bound: Some(bound) });
            }
            if valid(&down) {
                frontier.push(Node { bounds: down, bound: Some(bound) });
            }
            active_bound = None;
        }
        budget.progress("branch_and_bound", incumbent.as_ref().and_then(|(_, s)| restored_objective(model, *s)));
        if let (Some((_, best)), Some(lower)) = (&incumbent, frontier_bound(&frontier))
            && options.relative_gap > 0.0
            && gap(*best, lower, score(model.objective.direction, model.objective.offset)).is_some_and(|gap| gap <= options.relative_gap)
        {
            stopped_for_gap = true;
            break;
        }
    }
    let frontier_lower = frontier_bound(&frontier);
    let lower = match (active_bound, frontier_lower) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) if frontier.is_empty() => Some(a),
        (None, b) => b,
        _ => None,
    };
    let mut result = if let Some((values, best)) = incumbent {
        let done = termination == Termination::Completed && frontier.is_empty() && !stopped_for_gap;
        let mut r = verified_result(model, options, values, if done { Optimality::Optimal } else { Optimality::BestFound })?;
        r.termination = termination;
        let bound = if done { Some(best) } else { lower };
        r.best_bound = bound.and_then(|b| reported_bound(model, b).map(|(value, _)| value));
        r.relative_gap = bound.and_then(|b| gap(best, b, score(model.objective.direction, model.objective.offset)));
        if let (Some(objective), Some(bound)) = (r.objective, r.best_bound) {
            // Public bounds are rounded outwards. Retain the more conservative
            // of the display-space and offset-free internal gap calculations.
            if let Some(display_gap) = gap(score(model.objective.direction, objective), score(model.objective.direction, bound), 0.0) {
                r.relative_gap = Some(r.relative_gap.map_or(display_gap, |g| g.max(display_gap)));
            }
        }
        if bound.and_then(|b| reported_bound(model, b)).is_some_and(|(_, rounded)| rounded) {
            r.warnings.push("Objective offset rounding affects displayed certificate values; the bound is rounded outwards and the reported gap includes that uncertainty.".into());
        }
        if bound.is_some() && r.best_bound.is_none() {
            r.relative_gap = None;
            r.warnings.push("The objective offset prevents a finite representable bound; bound and gap are unavailable.".into());
        }
        if stopped_for_gap {
            r.warnings.push("Stopped at the requested relative MIP gap; exact zero-gap optimality was not established.".into());
        }
        r
    } else {
        empty_result(
            termination,
            if termination == Termination::Completed && frontier.is_empty() { Feasibility::Infeasible } else { Feasibility::Unknown },
            Optimality::Unknown,
        )
    };
    result.statistics = budget.statistics.clone();
    if let Some(message) = warning {
        result.warnings.push(message);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
