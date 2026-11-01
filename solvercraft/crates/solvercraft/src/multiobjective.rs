//! Auditable scalarization, lexicographic priorities and epsilon constraints.
use serde::{Deserialize, Serialize};
use solvercraft_model::{CompensatedSum, MAX_CONSTRAINTS, MAX_NAME_BYTES, MAX_TERMS};
use solvercraft_types::*;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearGoal {
    pub name: String,
    pub direction: Direction,
    pub offset: f64,
    pub terms: Vec<LinearTerm>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpsilonBound {
    pub goal: usize,
    pub value: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultiobjectiveMethod {
    Weighted {
        weights: Vec<f64>,
    },
    /// Goal order defines priorities; each tolerance is absolute in that goal's units.
    Lexicographic {
        absolute_tolerances: Vec<f64>,
    },
    /// A minimized goal receives an upper bound, a maximized goal a lower bound.
    EpsilonConstraint {
        primary: usize,
        bounds: Vec<EpsilonBound>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiobjectiveResult {
    pub stages: Vec<SolveResult>,
    pub goal_values: Option<Vec<f64>>,
    pub values: Option<Vec<f64>>,
    pub imposed_bounds: Vec<LinearConstraint>,
    /// Positive common rescaling preserves the weighted optimum. These are the
    /// actual sum-one weights used by stages[0], with directions signed to minimize.
    pub scalarization_weights: Option<Vec<f64>>,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_multiobjective", message)
}
fn sign(direction: Direction) -> f64 {
    if direction == Direction::Minimize { 1.0 } else { -1.0 }
}
fn objective(goal: &LinearGoal) -> Objective {
    Objective { direction: goal.direction, offset: goal.offset, linear: goal.terms.clone(), quadratic: vec![] }
}
fn bound(goal: &LinearGoal, value: f64) -> Result<LinearConstraint> {
    let rhs = value - goal.offset;
    if !rhs.is_finite() {
        return Err(invalid("Goal bound overflows"));
    }
    Ok(LinearConstraint {
        name: goal.name.clone(),
        terms: goal.terms.clone(),
        relation: if goal.direction == Direction::Minimize { Relation::LessEqual } else { Relation::GreaterEqual },
        rhs,
    })
}
fn evaluate(goal: &LinearGoal, values: &[f64]) -> Result<f64> {
    let mut total = CompensatedSum::default();
    total.add(goal.offset)?;
    for term in &goal.terms {
        total.add(term.coefficient * values.get(term.variable).ok_or_else(|| invalid("Missing solution handle"))?)?;
    }
    total.finish()
}
fn scalarize(goals: &[LinearGoal], weights: &[f64]) -> Result<Objective> {
    let normalized = crate::risk::probability_weights(weights.iter().copied())?;
    let mut coefficients: BTreeMap<usize, CompensatedSum> = BTreeMap::new();
    let mut offset = CompensatedSum::default();
    for (goal, weight) in goals.iter().zip(normalized) {
        let coefficient = weight * sign(goal.direction);
        let product = coefficient * goal.offset;
        if !product.is_finite() || (coefficient != 0.0 && goal.offset != 0.0 && product.abs() < f64::MIN_POSITIVE) {
            return Err(SolverError::new("numerical_failure", "Weighted objective offset exceeds supported precision"));
        }
        offset.add(product)?;
        for term in &goal.terms {
            let value = coefficient * term.coefficient;
            if !value.is_finite() || (coefficient != 0.0 && term.coefficient != 0.0 && value.abs() < f64::MIN_POSITIVE) {
                return Err(SolverError::new("numerical_failure", "Weighted objective exceeds supported precision"));
            }
            coefficients.entry(term.variable).or_default().add(value)?;
        }
    }
    let linear = coefficients
        .into_iter()
        .map(|(variable, sum)| sum.finish().map(|coefficient| LinearTerm { variable, coefficient }))
        .collect::<Result<Vec<_>>>()?;
    Ok(Objective { direction: Direction::Minimize, offset: offset.finish()?, linear, quadratic: vec![] })
}
pub fn solve(
    model: &OptimizationModel,
    goals: &[LinearGoal],
    method: &MultiobjectiveMethod,
    options: &SolveOptions,
    control: &dyn RunControl,
) -> Result<MultiobjectiveResult> {
    solvercraft_model::validate(model, options)?;
    if goals.is_empty() || goals.len() > 1000 {
        return Err(invalid("Provide 1 to 1000 goals"));
    }
    let mut count = model.objective.linear.len() + model.objective.quadratic.len();
    for row in &model.constraints {
        count = count.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
    }
    let mut names = model
        .name
        .len()
        .saturating_add(model.variables.iter().map(|v| v.name.len()).sum::<usize>())
        .saturating_add(model.constraints.iter().map(|v| v.name.len()).sum::<usize>());
    for goal in goals {
        count = count.checked_add(goal.terms.len().saturating_mul(2)).ok_or_else(|| invalid("Goal term overflow"))?;
        names = names.checked_add(goal.name.len()).ok_or_else(|| invalid("Name overflow"))?;
        if count > MAX_TERMS
            || goal.name.len() > MAX_NAME_BYTES
            || !goal.offset.is_finite()
            || goal.terms.iter().any(|t| t.variable >= model.variables.len() || !t.coefficient.is_finite())
        {
            return Err(invalid("Invalid goal handles, dimensions or coefficients"));
        }
    }
    let maximum_rows = model.constraints.len().checked_add(goals.len()).ok_or_else(|| invalid("Row overflow"))?;
    let bytes = count
        .checked_mul(256)
        .and_then(|n| n.checked_add(model.variables.len().checked_mul(256)?))
        .and_then(|n| n.checked_add(maximum_rows.checked_mul(256)?))
        .and_then(|n| n.checked_add(names));
    if maximum_rows > MAX_CONSTRAINTS || bytes.is_none_or(|n| n as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Multiobjective expansion exceeds budget before allocation"));
    }
    match method {
        MultiobjectiveMethod::Weighted { weights } => {
            if weights.len() != goals.len() || weights.iter().any(|w| !w.is_finite() || *w < 0.0) || !weights.iter().any(|w| *w > 0.0) {
                return Err(invalid("Provide one nonnegative finite weight per goal and positive total weight"));
            }
        }
        MultiobjectiveMethod::Lexicographic { absolute_tolerances } => {
            if absolute_tolerances.len() != goals.len() || absolute_tolerances.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(invalid("Provide one nonnegative finite absolute tolerance per goal"));
            }
        }
        MultiobjectiveMethod::EpsilonConstraint { primary, bounds } => {
            if *primary >= goals.len() || bounds.len() > goals.len() || bounds.iter().any(|b| b.goal >= goals.len() || !b.value.is_finite()) {
                return Err(invalid("Invalid epsilon bounds or primary goal"));
            }
        }
    }
    let scalarized = if let MultiobjectiveMethod::Weighted { weights } = method { Some(scalarize(goals, weights)?) } else { None };
    let mut working = model.clone();
    let mut imposed = Vec::new();
    let mut stages = Vec::new();
    let mut remaining = options.clone();
    let stage_count = if matches!(method, MultiobjectiveMethod::Lexicographic { .. }) { goals.len() } else { 1 };
    stages.try_reserve_exact(stage_count).map_err(|_| invalid("Cannot allocate solve trace"))?;
    for stage in 0..stage_count {
        working.objective = match method {
            MultiobjectiveMethod::Weighted { .. } => scalarized.clone().ok_or_else(|| invalid("Missing scalarized objective"))?,
            MultiobjectiveMethod::Lexicographic { .. } => objective(&goals[stage]),
            MultiobjectiveMethod::EpsilonConstraint { primary, bounds } => {
                for item in bounds {
                    let constraint = bound(&goals[item.goal], item.value)?;
                    working.constraints.push(constraint.clone());
                    imposed.push(constraint);
                }
                objective(&goals[*primary])
            }
        };
        let solved = crate::solve(&working, &remaining, control)?;
        remaining.max_evaluations = remaining.max_evaluations.saturating_sub(solved.statistics.evaluations);
        remaining.max_iterations = remaining.max_iterations.saturating_sub(solved.statistics.iterations);
        remaining.max_nodes = remaining.max_nodes.saturating_sub(solved.statistics.nodes);
        let optimal = solved.optimality == Optimality::Optimal;
        if let (MultiobjectiveMethod::Lexicographic { absolute_tolerances }, Some(values)) = (method, &solved.values)
            && optimal
            && stage + 1 < goals.len()
        {
            let achieved = evaluate(&goals[stage], values)?;
            let limit = achieved + sign(goals[stage].direction) * absolute_tolerances[stage];
            if !limit.is_finite() {
                return Err(invalid("Lexicographic bound overflow"));
            }
            let constraint = bound(&goals[stage], limit)?;
            working.constraints.push(constraint.clone());
            imposed.push(constraint);
        }
        stages.push(solved);
        if !optimal {
            break;
        }
    }
    let values = stages.last().and_then(|s| s.values.clone());
    let goal_values = values.as_ref().map(|values| goals.iter().map(|goal| evaluate(goal, values)).collect::<Result<Vec<_>>>()).transpose()?;
    let scalarization_weights =
        if let MultiobjectiveMethod::Weighted { weights } = method { Some(crate::risk::probability_weights(weights.iter().copied())?) } else { None };
    Ok(MultiobjectiveResult { stages, goal_values, values, imposed_bounds: imposed, scalarization_weights })
}
