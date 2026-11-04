//! Multicut Benders for a checked complete-recourse linear subclass.
use crate::stochastic::{RecourseOutcome, RecourseScenario, TwoStageProblem};
use serde::{Deserialize, Serialize};
use solvercraft_model::CompensatedSum;
use solvercraft_types::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BendersResult {
    pub termination: Termination,
    pub converged: bool,
    pub iterations: u64,
    pub cuts: u64,
    pub first_stage_values: Option<Vec<f64>>,
    pub outcomes: Option<Vec<RecourseOutcome>>,
    pub objective: Option<f64>,
    pub lower_bound: Option<f64>,
    pub relative_gap: Option<f64>,
    pub note: String,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("unsupported_benders_structure", message)
}
fn term(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn model(
    name: &str,
    variables: Vec<Variable>,
    direction: Direction,
    offset: f64,
    linear: Vec<LinearTerm>,
    constraints: Vec<LinearConstraint>,
) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: name.into(),
        variables,
        objective: Objective { direction, offset, linear, quadratic: vec![] },
        constraints,
    }
}
fn spend(remaining: &mut SolveOptions, result: &SolveResult) {
    remaining.max_iterations = remaining.max_iterations.saturating_sub(result.statistics.iterations);
    remaining.max_evaluations = remaining.max_evaluations.saturating_sub(result.statistics.evaluations);
    remaining.max_nodes = remaining.max_nodes.saturating_sub(result.statistics.nodes);
}
fn rhs(scenario: &RecourseScenario, x: &[f64]) -> Result<Vec<f64>> {
    scenario
        .constraints
        .iter()
        .map(|row| {
            let mut total = CompensatedSum::default();
            total.add(row.rhs)?;
            for t in &row.first_stage_terms {
                total.add(-t.coefficient * x.get(t.variable).ok_or_else(|| invalid("Missing first-stage handle"))?)?;
            }
            total.finish()
        })
        .collect()
}
fn preflight(problem: &TwoStageProblem, options: &SolveOptions) -> Result<()> {
    crate::stochastic::preflight(problem, options)?;
    if problem.direction != Direction::Minimize {
        return Err(invalid("Benders supports cost minimization"));
    }
    for variable in &problem.first_stage_variables {
        let (lo, hi) = solvercraft_model::effective_bounds(variable)?;
        if lo.is_none() || hi.is_none() {
            return Err(invalid("Finite first-stage bounds are required"));
        }
    }
    let mut work = 0usize;
    if problem.first_stage_variables.len().saturating_add(problem.scenarios.len()) > solvercraft_model::MAX_VARIABLES {
        return Err(invalid("Benders master epigraph exceeds variable limit"));
    }
    for scenario in &problem.scenarios {
        let m = scenario.constraints.len();
        let n = scenario.variables.len();
        if m > solvercraft_model::MAX_VARIABLES || n > solvercraft_model::MAX_CONSTRAINTS {
            return Err(invalid("Dual subproblem exceeds model dimensions"));
        }
        let coefficient_count = scenario.constraints.iter().map(|r| r.recourse_terms.len()).sum::<usize>();
        work = work
            .checked_add(n.max(1).checked_mul(coefficient_count).ok_or_else(|| invalid("Dual assembly work overflow"))?)
            .ok_or_else(|| invalid("Work overflow"))?;
        if work > 10_000_000 {
            return Err(invalid("Complete-recourse structural check exceeds bounded work"));
        }
        if scenario.variables.iter().any(|v| v.kind != VariableKind::Continuous || v.lower != Some(0.0) || v.upper.is_some())
            || scenario.costs.iter().any(|c| *c < 0.0)
            || scenario.constraints.iter().any(|r| r.relation != Relation::GreaterEqual)
        {
            return Err(invalid("Recourse must be continuous nonnegative unbounded-above, with nonnegative costs and >= rows"));
        }
        let mut uses = vec![0usize; n];
        for row in &scenario.constraints {
            for t in &row.recourse_terms {
                uses[t.variable] = uses[t.variable].saturating_add(1);
            }
        }
        for row in &scenario.constraints {
            // A dedicated positive slack in every row proves feasibility for any x.
            let covered = row.recourse_terms.iter().any(|candidate| candidate.coefficient > 0.0 && uses[candidate.variable] == 1);
            if !covered {
                return Err(invalid("Each recourse row needs a dedicated positive slack to prove complete recourse"));
            }
        }
    }
    Ok(())
}
fn subproblem(scenario: &RecourseScenario, adjusted: &[f64]) -> OptimizationModel {
    model(
        &scenario.name,
        scenario.variables.clone(),
        Direction::Minimize,
        scenario.offset,
        scenario.costs.iter().enumerate().map(|(i, c)| term(i, *c)).collect(),
        scenario
            .constraints
            .iter()
            .zip(adjusted)
            .map(|(row, rhs)| LinearConstraint {
                name: row.name.clone(),
                terms: row.recourse_terms.clone(),
                relation: Relation::GreaterEqual,
                rhs: *rhs,
            })
            .collect(),
    )
}
fn dual(scenario: &RecourseScenario, adjusted: &[f64]) -> OptimizationModel {
    let variables = (0..scenario.constraints.len())
        .map(|_| Variable { name: "recourse dual".into(), lower: Some(0.0), upper: None, kind: VariableKind::Continuous })
        .collect();
    let constraints = scenario
        .costs
        .iter()
        .enumerate()
        .map(|(j, c)| LinearConstraint {
            name: "dual feasibility".into(),
            terms: scenario
                .constraints
                .iter()
                .enumerate()
                .flat_map(|(r, row)| row.recourse_terms.iter().filter(move |t| t.variable == j).map(move |t| term(r, t.coefficient)))
                .collect(),
            relation: Relation::LessEqual,
            rhs: *c,
        })
        .collect();
    model(
        "recourse dual",
        variables,
        Direction::Maximize,
        scenario.offset,
        adjusted.iter().enumerate().map(|(r, c)| term(r, *c)).collect(),
        constraints,
    )
}
fn certified_multiplier(scenario: &RecourseScenario, values: &[f64]) -> Result<Vec<f64>> {
    let mut multiplier = values.iter().map(|v| v.max(0.0)).collect::<Vec<_>>();
    let mut scale = 1.0_f64;
    for (j, cost) in scenario.costs.iter().enumerate() {
        let mut activity = CompensatedSum::default();
        for (r, row) in scenario.constraints.iter().enumerate() {
            for t in row.recourse_terms.iter().filter(|t| t.variable == j) {
                activity.add(t.coefficient * multiplier[r])?;
            }
        }
        let activity = activity.finish()?;
        if activity > *cost {
            scale = scale.min(if *cost == 0.0 { 0.0 } else { cost / activity });
        }
    }
    if scale < 1.0 {
        scale *= 1.0 - 1e-10;
        for p in &mut multiplier {
            *p *= scale;
        }
    }
    // Explicit check after clipping/scaling; a solver's enum is not a certificate.
    for (j, cost) in scenario.costs.iter().enumerate() {
        let mut activity = CompensatedSum::default();
        for (r, row) in scenario.constraints.iter().enumerate() {
            for t in row.recourse_terms.iter().filter(|t| t.variable == j) {
                activity.add(t.coefficient * multiplier[r])?;
            }
        }
        if activity.finish()? > *cost {
            return Err(SolverError::new("numerical_failure", "Cannot establish a dual-feasible Benders multiplier"));
        }
    }
    Ok(multiplier)
}
/// Benders multicut decomposition with explicit primal and dual recourse LPs.
/// Dedicated recourse slacks prove complete recourse; integer recourse is rejected.
pub fn solve(problem: &TwoStageProblem, options: &SolveOptions, control: &dyn RunControl) -> Result<BendersResult> {
    preflight(problem, options)?;
    let probabilities = crate::risk::probability_weights(problem.scenarios.iter().map(|s| s.weight))?;
    let weights: Vec<_> = problem.scenarios.iter().map(|s| s.weight).collect();
    let n = problem.first_stage_variables.len();
    let first_cut_terms = problem.scenarios.len().checked_mul(n.saturating_add(1)).ok_or_else(|| invalid("Cut size overflow"))?;
    if first_cut_terms > solvercraft_model::MAX_TERMS || first_cut_terms.checked_mul(256).is_none_or(|x| x as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "One Benders cut batch exceeds bounded storage"));
    }
    let offsets: Vec<_> = problem.scenarios.iter().map(|s| s.offset).collect();
    let mut master_offset = CompensatedSum::default();
    master_offset.add(problem.first_stage_offset)?;
    master_offset.add(crate::risk::weighted_mean(&offsets, &weights)?)?;
    let mut master = model(
        &problem.name,
        problem.first_stage_variables.clone(),
        Direction::Minimize,
        master_offset.finish()?,
        problem.first_stage_costs.iter().enumerate().map(|(i, c)| term(i, *c)).collect(),
        problem.first_stage_constraints.clone(),
    );
    for p in &probabilities {
        let index = master.variables.len();
        master.variables.push(Variable { name: "recourse epigraph".into(), lower: Some(0.0), upper: None, kind: VariableKind::Continuous });
        master.objective.linear.push(term(index, *p));
    }
    let mut remaining = options.clone();
    let mut result=BendersResult{termination:Termination::IterationLimit,converged:false,iterations:0,cuts:0,first_stage_values:None,outcomes:None,objective:None,lower_bound:None,relative_gap:None,note:"Complete linear recourse checked by dedicated slacks; every cut uses an explicitly verified nonnegative dual-feasible multiplier. Floating-point tolerances apply; no rational certificate.".into()};
    for _ in 0..10_000 {
        if control.is_cancelled() {
            result.termination = Termination::Cancelled;
            break;
        }
        if remaining.max_iterations == 0 {
            result.termination = Termination::IterationLimit;
            break;
        }
        let future_rows = master.constraints.len().saturating_add(problem.scenarios.len());
        let future_terms = master
            .constraints
            .iter()
            .map(|r| r.terms.len())
            .sum::<usize>()
            .saturating_add(first_cut_terms)
            .saturating_add(master.objective.linear.len());
        let future_names = master
            .name
            .len()
            .saturating_add(master.variables.iter().map(|v| v.name.len()).sum::<usize>())
            .saturating_add(master.constraints.iter().map(|r| r.name.len()).sum::<usize>());
        if future_rows > solvercraft_model::MAX_CONSTRAINTS
            || future_terms > solvercraft_model::MAX_TERMS
            || future_terms
                .checked_mul(384)
                .and_then(|x| x.checked_add(future_rows.checked_mul(384)?))
                .and_then(|x| x.checked_add(master.variables.len().checked_mul(384)?))
                .and_then(|x| x.checked_add(future_names.checked_mul(2)?))
                .is_none_or(|x| x as u64 > options.max_memory_bytes)
        {
            result.termination = Termination::MemoryLimit;
            break;
        }
        remaining.max_iterations -= 1;
        result.iterations += 1;
        let solved = crate::solve(&master, &remaining, control)?;
        spend(&mut remaining, &solved);
        if solved.feasibility == Feasibility::Infeasible {
            result.termination = solved.termination;
            result.note.push_str(" First-stage master is infeasible.");
            break;
        }
        let Some(master_values) = &solved.values else {
            result.termination = solved.termination;
            break;
        };
        if let Some(bound) = solved.best_bound.or(if solved.optimality == Optimality::Optimal { solved.objective } else { None }) {
            result.lower_bound = Some(result.lower_bound.map_or(bound, |old| old.max(bound)));
        }
        let x = master_values.get(..n).ok_or_else(|| invalid("Missing master decisions"))?.to_vec();
        let mut outcomes = Vec::new();
        let mut cuts = Vec::new();
        let mut interrupted = false;
        for (s, scenario) in problem.scenarios.iter().enumerate() {
            let adjusted = rhs(scenario, &x)?;
            let primal = crate::solve(&subproblem(scenario, &adjusted), &remaining, control)?;
            spend(&mut remaining, &primal);
            let Some(values) = primal.values else {
                result.termination = primal.termination;
                interrupted = true;
                break;
            };
            let cost = crate::stochastic::linear_cost(scenario.offset, &scenario.costs, &values)?;
            outcomes.push(RecourseOutcome { name: scenario.name.clone(), probability: probabilities[s], values, recourse_cost: cost });
            let solved_dual = crate::solve(&dual(scenario, &adjusted), &remaining, control)?;
            spend(&mut remaining, &solved_dual);
            if solved_dual.optimality != Optimality::Optimal {
                result.termination = solved_dual.termination;
                interrupted = true;
                break;
            }
            let multiplier = certified_multiplier(scenario, solved_dual.values.as_deref().ok_or_else(|| invalid("Dual has no solution"))?)?;
            let mut constant = CompensatedSum::default();
            let mut coefficients = vec![CompensatedSum::default(); n];
            for (r, row) in scenario.constraints.iter().enumerate() {
                constant.add(multiplier[r] * row.rhs)?;
                for t in &row.first_stage_terms {
                    coefficients[t.variable].add(multiplier[r] * t.coefficient)?;
                }
            }
            let mut terms = coefficients
                .into_iter()
                .enumerate()
                .map(|(variable, sum)| sum.finish().map(|coefficient| term(variable, coefficient)))
                .collect::<Result<Vec<_>>>()?;
            terms.push(term(n + s, 1.0));
            cuts.push(LinearConstraint {
                name: "verified Benders optimality cut".into(),
                terms,
                relation: Relation::GreaterEqual,
                rhs: constant.finish()?,
            });
        }
        if interrupted {
            break;
        }
        let costs: Vec<_> = outcomes.iter().map(|o| o.recourse_cost).collect();
        let mut total = CompensatedSum::default();
        total.add(crate::stochastic::linear_cost(problem.first_stage_offset, &problem.first_stage_costs, &x)?)?;
        total.add(crate::risk::weighted_mean(&costs, &weights)?)?;
        let total = total.finish()?;
        if result.objective.is_none_or(|old| total < old) {
            result.objective = Some(total);
            result.first_stage_values = Some(x);
            result.outcomes = Some(outcomes);
        }
        if let (Some(upper), Some(lower)) = (result.objective, result.lower_bound) {
            let absolute = (upper - lower).max(0.0);
            let scale = upper.abs().max(1.0);
            result.relative_gap = Some(absolute / scale);
            if lower > upper + options.feasibility_tolerance * scale * 16.0 {
                return Err(SolverError::new("numerical_failure", "Benders lower bound exceeded a feasible recourse objective"));
            }
            if absolute <= options.optimality_tolerance * scale || absolute / scale <= options.relative_gap {
                result.converged = true;
                result.termination = Termination::Completed;
                break;
            }
        }
        // Check all future cut rows/terms and budget before growing master storage.
        let rows = master.constraints.len().checked_add(cuts.len()).ok_or_else(|| invalid("Cut count overflow"))?;
        let terms = master
            .constraints
            .iter()
            .map(|r| r.terms.len())
            .sum::<usize>()
            .checked_add(cuts.iter().map(|r| r.terms.len()).sum())
            .and_then(|t| t.checked_add(master.objective.linear.len()))
            .ok_or_else(|| invalid("Cut term overflow"))?;
        if rows > solvercraft_model::MAX_CONSTRAINTS
            || terms > solvercraft_model::MAX_TERMS
            || terms.checked_mul(256).and_then(|v| v.checked_add(rows.checked_mul(256)?)).is_none_or(|v| v as u64 > options.max_memory_bytes)
        {
            result.termination = Termination::MemoryLimit;
            break;
        }
        result.cuts += cuts.len() as u64;
        master.constraints.extend(cuts);
    }
    Ok(result)
}
