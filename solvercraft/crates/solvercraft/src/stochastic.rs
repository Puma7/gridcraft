//! Two-stage finite linear recourse with structurally shared first-stage decisions.
use serde::{Deserialize, Serialize};
use solvercraft_model::{CompensatedSum, MAX_CONSTRAINTS, MAX_NAME_BYTES, MAX_TERMS, MAX_VARIABLES};
use solvercraft_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecourseConstraint {
    pub name: String,
    pub first_stage_terms: Vec<LinearTerm>,
    pub recourse_terms: Vec<LinearTerm>,
    pub relation: Relation,
    pub rhs: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecourseScenario {
    pub name: String,
    pub weight: f64,
    pub variables: Vec<Variable>,
    pub costs: Vec<f64>,
    pub offset: f64,
    pub constraints: Vec<RecourseConstraint>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoStageProblem {
    pub name: String,
    pub direction: Direction,
    pub first_stage_variables: Vec<Variable>,
    pub first_stage_costs: Vec<f64>,
    pub first_stage_offset: f64,
    pub first_stage_constraints: Vec<LinearConstraint>,
    pub scenarios: Vec<RecourseScenario>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMapping {
    pub start: usize,
    pub length: usize,
    pub probability: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterministicEquivalent {
    pub model: OptimizationModel,
    pub first_stage_length: usize,
    pub scenarios: Vec<ScenarioMapping>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecourseOutcome {
    pub name: String,
    pub probability: f64,
    pub values: Vec<f64>,
    /// Includes the scenario offset, excludes the first-stage cost.
    pub recourse_cost: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoStageResult {
    pub solve: SolveResult,
    pub first_stage_values: Option<Vec<f64>>,
    pub outcomes: Option<Vec<RecourseOutcome>>,
    pub expected_total_cost: Option<f64>,
}
fn invalid(message: impl Into<String>) -> SolverError {
    SolverError::new("invalid_two_stage", message)
}
fn name(value: &str) -> Result<()> {
    if value.len() > MAX_NAME_BYTES { Err(invalid("Name exceeds model limit")) } else { Ok(()) }
}
fn finite(value: f64) -> Result<()> {
    if value.is_finite() { Ok(()) } else { Err(invalid("All numeric data must be finite")) }
}
fn add(total: &mut usize, value: usize) -> Result<()> {
    *total = total.checked_add(value).ok_or_else(|| invalid("Counterpart size overflow"))?;
    Ok(())
}
fn terms(values: &[LinearTerm], variables: usize) -> Result<()> {
    if values.iter().any(|v| v.variable >= variables || !v.coefficient.is_finite()) {
        return Err(invalid("Invalid coefficient or variable handle"));
    }
    Ok(())
}
pub(crate) fn preflight(problem: &TwoStageProblem, options: &SolveOptions) -> Result<()> {
    solvercraft_model::validate_options(options)?;
    name(&problem.name)?;
    if problem.scenarios.is_empty() || problem.scenarios.len() > 100_000 {
        return Err(invalid("Provide 1 to 100000 scenarios"));
    }
    let n = problem.first_stage_variables.len();
    if problem.first_stage_costs.len() != n {
        return Err(invalid("First-stage cost dimension differs"));
    }
    let mut variables = n;
    let mut rows = problem.first_stage_constraints.len();
    let mut coefficients = n;
    let mut names = problem.name.len();
    // Count the entire expansion before traversing term vectors or cloning inputs.
    for scenario in &problem.scenarios {
        add(&mut variables, scenario.variables.len())?;
        add(&mut rows, scenario.constraints.len())?;
        add(&mut coefficients, scenario.costs.len())?;
        for row in &scenario.constraints {
            add(&mut coefficients, row.first_stage_terms.len())?;
            add(&mut coefficients, row.recourse_terms.len())?;
        }
    }
    for row in &problem.first_stage_constraints {
        add(&mut coefficients, row.terms.len())?;
    }
    if variables > MAX_VARIABLES || rows > MAX_CONSTRAINTS || coefficients > MAX_TERMS {
        return Err(SolverError::new("model_limit", "Two-stage counterpart exceeds model dimensions"));
    }
    finite(problem.first_stage_offset)?;
    for (variable, cost) in problem.first_stage_variables.iter().zip(&problem.first_stage_costs) {
        solvercraft_model::validate_variable(variable)?;
        finite(*cost)?;
        add(&mut names, variable.name.len())?;
    }
    for row in &problem.first_stage_constraints {
        name(&row.name)?;
        finite(row.rhs)?;
        terms(&row.terms, n)?;
        add(&mut names, row.name.len())?;
    }
    let mut positive = false;
    for scenario in &problem.scenarios {
        name(&scenario.name)?;
        finite(scenario.weight)?;
        finite(scenario.offset)?;
        if scenario.weight < 0.0 || scenario.costs.len() != scenario.variables.len() {
            return Err(invalid("Invalid scenario weight or cost dimension"));
        }
        positive |= scenario.weight > 0.0;
        add(&mut names, scenario.name.len())?;
        for (variable, cost) in scenario.variables.iter().zip(&scenario.costs) {
            solvercraft_model::validate_variable(variable)?;
            finite(*cost)?;
            add(&mut names, variable.name.len())?;
        }
        for row in &scenario.constraints {
            name(&row.name)?;
            finite(row.rhs)?;
            terms(&row.first_stage_terms, n)?;
            terms(&row.recourse_terms, scenario.variables.len())?;
            add(&mut names, row.name.len())?;
        }
    }
    if !positive {
        return Err(invalid("Scenario weights need positive total mass"));
    }
    let bytes = coefficients
        .checked_mul(256)
        .and_then(|x| x.checked_add(variables.checked_mul(256)?))
        .and_then(|x| x.checked_add(rows.checked_mul(256)?))
        .and_then(|x| x.checked_add(problem.scenarios.len().checked_mul(256)?))
        .and_then(|x| x.checked_add(names.checked_mul(2)?));
    if bytes.is_none_or(|x| x as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Two-stage expansion exceeds memory budget before allocation"));
    }
    Ok(())
}
fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values.try_reserve_exact(additional).map_err(|_| SolverError::new("allocation", "Cannot allocate two-stage counterpart"))
}
fn checked_product(a: f64, b: f64) -> Result<f64> {
    let result = a * b;
    if !result.is_finite() || (a != 0.0 && b != 0.0 && result.abs() < f64::MIN_POSITIVE) {
        return Err(SolverError::new("numerical_failure", "Weighted recourse coefficient exceeds supported precision"));
    }
    Ok(result)
}
/// Build one common first-stage block and one local recourse block per scenario.
/// Even zero-weight scenarios retain their recourse feasibility requirements.
pub fn compile_two_stage(problem: &TwoStageProblem, options: &SolveOptions) -> Result<DeterministicEquivalent> {
    preflight(problem, options)?;
    let probabilities = crate::risk::probability_weights(problem.scenarios.iter().map(|s| s.weight))?;
    let offsets: Vec<_> = problem.scenarios.iter().map(|s| s.offset).collect();
    let weights: Vec<_> = problem.scenarios.iter().map(|s| s.weight).collect();
    let mut offset = CompensatedSum::default();
    offset.add(problem.first_stage_offset)?;
    offset.add(crate::risk::weighted_mean(&offsets, &weights)?)?;
    // Validate every weighted coefficient before the model is cloned.
    for (scenario, probability) in problem.scenarios.iter().zip(&probabilities) {
        for cost in &scenario.costs {
            checked_product(*cost, *probability)?;
        }
    }
    let mut model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: problem.name.clone(),
        variables: problem.first_stage_variables.clone(),
        objective: Objective { direction: problem.direction, offset: offset.finish()?, linear: Vec::new(), quadratic: Vec::new() },
        constraints: problem.first_stage_constraints.clone(),
    };
    let mut mappings = Vec::new();
    reserve(&mut mappings, problem.scenarios.len())?;
    reserve(&mut model.objective.linear, problem.first_stage_costs.len())?;
    for (variable, coefficient) in problem.first_stage_costs.iter().copied().enumerate() {
        if coefficient != 0.0 {
            model.objective.linear.push(LinearTerm { variable, coefficient });
        }
    }
    for (scenario, probability) in problem.scenarios.iter().zip(probabilities) {
        let start = model.variables.len();
        reserve(&mut model.variables, scenario.variables.len())?;
        reserve(&mut model.objective.linear, scenario.costs.len())?;
        reserve(&mut model.constraints, scenario.constraints.len())?;
        model.variables.extend(scenario.variables.iter().cloned());
        for (local, cost) in scenario.costs.iter().enumerate() {
            let coefficient = checked_product(*cost, probability)?;
            if coefficient != 0.0 {
                model.objective.linear.push(LinearTerm { variable: start + local, coefficient });
            }
        }
        for row in &scenario.constraints {
            let mut coefficients = Vec::new();
            reserve(&mut coefficients, row.first_stage_terms.len() + row.recourse_terms.len())?;
            coefficients.extend_from_slice(&row.first_stage_terms);
            coefficients.extend(row.recourse_terms.iter().map(|t| LinearTerm { variable: start + t.variable, coefficient: t.coefficient }));
            model.constraints.push(LinearConstraint { name: row.name.clone(), terms: coefficients, relation: row.relation, rhs: row.rhs });
        }
        mappings.push(ScenarioMapping { start, length: scenario.variables.len(), probability });
    }
    solvercraft_model::validate(&model, options)?;
    Ok(DeterministicEquivalent { model, first_stage_length: problem.first_stage_variables.len(), scenarios: mappings })
}
pub(crate) fn linear_cost(offset: f64, coefficients: &[f64], values: &[f64]) -> Result<f64> {
    let mut sum = CompensatedSum::default();
    sum.add(offset)?;
    for (coefficient, value) in coefficients.iter().zip(values) {
        sum.add(checked_product(*coefficient, *value)?)?;
    }
    sum.finish()
}
pub fn solve_two_stage(problem: &TwoStageProblem, options: &SolveOptions, control: &dyn RunControl) -> Result<TwoStageResult> {
    let equivalent = compile_two_stage(problem, options)?;
    let solve = crate::solve(&equivalent.model, options, control)?;
    let mut result = TwoStageResult { solve, first_stage_values: None, outcomes: None, expected_total_cost: None };
    if let Some(values) = &result.solve.values {
        let checked = solvercraft_model::validate_solution(&equivalent.model, values, options)?;
        if !checked.feasible {
            return Err(SolverError::new("numerical_failure", "Two-stage incumbent failed original constraints"));
        }
        let first = values.get(..equivalent.first_stage_length).ok_or_else(|| invalid("Missing first-stage values"))?.to_vec();
        let first_cost = linear_cost(problem.first_stage_offset, &problem.first_stage_costs, &first)?;
        let mut outcomes = Vec::new();
        reserve(&mut outcomes, problem.scenarios.len())?;
        for (scenario, mapping) in problem.scenarios.iter().zip(&equivalent.scenarios) {
            let local = values.get(mapping.start..mapping.start + mapping.length).ok_or_else(|| invalid("Missing recourse values"))?.to_vec();
            let cost = linear_cost(scenario.offset, &scenario.costs, &local)?;
            outcomes.push(RecourseOutcome { name: scenario.name.clone(), probability: mapping.probability, values: local, recourse_cost: cost });
        }
        let costs: Vec<_> = outcomes.iter().map(|o| o.recourse_cost).collect();
        let weights: Vec<_> = problem.scenarios.iter().map(|s| s.weight).collect();
        let mut total = CompensatedSum::default();
        total.add(first_cost)?;
        total.add(crate::risk::weighted_mean(&costs, &weights)?)?;
        let total = total.finish()?;
        if result.solve.objective.is_some_and(|value| (value - total).abs() > options.feasibility_tolerance * total.abs().max(1.0) * 16.0) {
            return Err(SolverError::new("numerical_failure", "Original two-stage cost differs from compiled objective"));
        }
        result.first_stage_values = Some(first);
        result.outcomes = Some(outcomes);
        result.expected_total_cost = Some(total);
    }
    Ok(result)
}
