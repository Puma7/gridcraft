//! Exact finite-scenario chance constraints, with bounds-derived Big M only.
use crate::finite::{FiniteConstraints, Indicator};
use serde::{Deserialize, Serialize};
use solvercraft_types::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChanceScenario {
    pub weight: f64,
    pub constraints: Vec<LinearConstraint>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChanceRule {
    Joint { maximum_violation_probability: f64 },
    Individual { maximum_violation_probabilities: Vec<f64> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChanceProblem {
    pub base: OptimizationModel,
    pub scenarios: Vec<ChanceScenario>,
    pub rule: ChanceRule,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanceResult {
    pub solve: SolveResult,
    pub individual_violation_probabilities: Option<Vec<f64>>,
    pub joint_violation_probability: Option<f64>,
    pub validation_note: String,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_chance_model", message)
}
fn limit(value: f64) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) { Ok(()) } else { Err(invalid("Violation probability limits must lie in [0,1]")) }
}
fn prepare(problem: &ChanceProblem, options: &SolveOptions) -> Result<(OptimizationModel, FiniteConstraints)> {
    solvercraft_model::validate(&problem.base, options)?;
    if problem.scenarios.is_empty() || problem.scenarios.len() > 100_000 {
        return Err(invalid("Provide 1 to 100000 scenarios"));
    }
    let conditions = problem.scenarios.first().map_or(0, |s| s.constraints.len());
    if conditions == 0 || conditions > solvercraft_model::MAX_CONSTRAINTS || problem.scenarios.iter().any(|s| s.constraints.len() != conditions) {
        return Err(invalid("All scenarios need the same nonzero number of conditions"));
    }
    let limits = match &problem.rule {
        ChanceRule::Joint { maximum_violation_probability } => {
            limit(*maximum_violation_probability)?;
            1
        }
        ChanceRule::Individual { maximum_violation_probabilities } => {
            if maximum_violation_probabilities.len() != conditions {
                return Err(invalid("One probability limit is required per condition"));
            }
            for p in maximum_violation_probabilities {
                limit(*p)?;
            }
            conditions
        }
    };
    let binaries = problem.scenarios.len().checked_mul(limits).ok_or_else(|| invalid("Chance dimensions overflow"))?;
    let mut rows = problem.base.constraints.len().checked_add(limits).ok_or_else(|| invalid("Row overflow"))?;
    let mut terms = problem.base.objective.linear.len() + problem.base.objective.quadratic.len();
    for row in &problem.base.constraints {
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
    }
    terms = terms.checked_add(binaries).ok_or_else(|| invalid("Term overflow"))?;
    let mut slacks = 0usize;
    for scenario in &problem.scenarios {
        for row in &scenario.constraints {
            let factor = if row.relation == Relation::Equal { 2 } else { 1 };
            slacks = slacks.checked_add(factor).ok_or_else(|| invalid("Slack overflow"))?;
            rows = rows.checked_add(factor * 2).ok_or_else(|| invalid("Row overflow"))?;
            terms = terms
                .checked_add(row.terms.len().checked_add(3).and_then(|n| n.checked_mul(factor)).ok_or_else(|| invalid("Term overflow"))?)
                .ok_or_else(|| invalid("Term overflow"))?;
        }
    }
    let variables =
        problem.base.variables.len().checked_add(binaries).and_then(|n| n.checked_add(slacks)).ok_or_else(|| invalid("Variable overflow"))?;
    if variables > solvercraft_model::MAX_VARIABLES || rows > solvercraft_model::MAX_CONSTRAINTS || terms > solvercraft_model::MAX_TERMS {
        return Err(SolverError::new("model_limit", "Chance counterpart exceeds dimensions"));
    }
    let mut names = problem
        .base
        .name
        .len()
        .saturating_add(problem.base.variables.iter().map(|v| v.name.len()).sum::<usize>())
        .saturating_add(problem.base.constraints.iter().map(|v| v.name.len()).sum::<usize>());
    for scenario in &problem.scenarios {
        for row in &scenario.constraints {
            names =
                names.checked_add(row.name.len().checked_mul(4).ok_or_else(|| invalid("Name overflow"))?).ok_or_else(|| invalid("Name overflow"))?;
        }
    }
    let bytes = terms
        .checked_mul(384)
        .and_then(|n| n.checked_add(variables.checked_mul(384)?))
        .and_then(|n| n.checked_add(rows.checked_mul(384)?))
        .and_then(|n| n.checked_add(names));
    if bytes.is_none_or(|n| n as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Chance counterpart exceeds budget before allocation"));
    }
    for scenario in &problem.scenarios {
        for row in &scenario.constraints {
            if row.relation != Relation::GreaterEqual {
                crate::finite::derived_big_m(&problem.base, row, false)?;
            }
            if row.relation != Relation::LessEqual {
                crate::finite::derived_big_m(&problem.base, row, true)?;
            }
        }
    }
    let probabilities = crate::risk::probability_weights(problem.scenarios.iter().map(|s| s.weight))?;
    let mut model = problem.base.clone();
    let mut finite = FiniteConstraints::default();
    finite.indicators.try_reserve_exact(problem.scenarios.len() * conditions).map_err(|_| invalid("Cannot allocate chance indicators"))?;
    model.variables.try_reserve_exact(binaries).map_err(|_| invalid("Cannot allocate chance selectors"))?;
    let start = model.variables.len();
    for _ in 0..binaries {
        model.variables.push(Variable { name: "scenario violation".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Binary });
    }
    for index in 0..limits {
        let terms = probabilities.iter().enumerate().map(|(s, p)| LinearTerm { variable: start + s * limits + index, coefficient: *p }).collect();
        let rhs = match &problem.rule {
            ChanceRule::Joint { maximum_violation_probability } => *maximum_violation_probability,
            ChanceRule::Individual { maximum_violation_probabilities } => maximum_violation_probabilities[index],
        };
        model.constraints.push(LinearConstraint { name: "chance probability budget".into(), terms, relation: Relation::LessEqual, rhs });
    }
    for (s, scenario) in problem.scenarios.iter().enumerate() {
        for (condition, row) in scenario.constraints.iter().enumerate() {
            finite.indicators.push(Indicator {
                binary: start + s * limits + if limits == 1 { 0 } else { condition },
                active_when: false,
                constraint: row.clone(),
            });
        }
    }
    Ok((model, finite))
}
pub fn compile(problem: &ChanceProblem, options: &SolveOptions) -> Result<OptimizationModel> {
    let (model, finite) = prepare(problem, options)?;
    crate::finite::compile(&model, &finite, options)
}
fn violation(row: &LinearConstraint, values: &[f64], tolerance: f64) -> Result<bool> {
    let mut sum = solvercraft_model::CompensatedSum::default();
    sum.add(-row.rhs)?;
    for term in &row.terms {
        sum.add(term.coefficient * values.get(term.variable).ok_or_else(|| invalid("Invalid condition handle"))?)?;
    }
    let r = sum.finish()?;
    Ok(match row.relation {
        Relation::LessEqual => r > tolerance,
        Relation::GreaterEqual => r < -tolerance,
        Relation::Equal => r.abs() > tolerance,
    })
}
pub fn solve(problem: &ChanceProblem, options: &SolveOptions, control: &dyn RunControl) -> Result<ChanceResult> {
    let (model, finite) = prepare(problem, options)?;
    let mut solve = crate::finite::solve(&model, &finite, options, control)?;
    let mut individual = None;
    let mut joint = None;
    if let Some(values) = &solve.values {
        let original = values.get(..problem.base.variables.len()).ok_or_else(|| invalid("Missing original variables"))?;
        let weights: Vec<_> = problem.scenarios.iter().map(|s| s.weight).collect();
        let count = problem.scenarios[0].constraints.len();
        let mut indicators = vec![vec![0.0; problem.scenarios.len()]; count];
        let mut union = vec![0.0; problem.scenarios.len()];
        for (s, scenario) in problem.scenarios.iter().enumerate() {
            for (i, row) in scenario.constraints.iter().enumerate() {
                if violation(row, original, options.feasibility_tolerance)? {
                    indicators[i][s] = 1.0;
                    union[s] = 1.0;
                }
            }
        }
        let probabilities = indicators.iter().map(|v| crate::risk::weighted_mean(v, &weights)).collect::<Result<Vec<_>>>()?;
        let joint_probability = crate::risk::weighted_mean(&union, &weights)?;
        let valid = match &problem.rule {
            ChanceRule::Joint { maximum_violation_probability } => joint_probability <= maximum_violation_probability + options.feasibility_tolerance,
            ChanceRule::Individual { maximum_violation_probabilities } => {
                probabilities.iter().zip(maximum_violation_probabilities).all(|(p, l)| *p <= l + options.feasibility_tolerance)
            }
        };
        if !valid {
            return Err(SolverError::new("numerical_failure", "Original finite-scenario violation mass exceeds chance budget"));
        }
        let original = original.to_vec();
        solve.values = Some(original);
        individual = Some(probabilities);
        joint = Some(joint_probability);
    }
    Ok(ChanceResult {
        solve,
        individual_violation_probabilities: individual,
        joint_violation_probability: joint,
        validation_note:
            "Exact probability only for the supplied finite discrete distribution. No population or out-of-sample guarantee is inferred.".into(),
    })
}
