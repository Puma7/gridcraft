//! Finite-scenario risk optimization and explicitly bounded robust counterparts.
use serde::{Deserialize, Serialize};
use solvercraft_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub weight: f64,
    pub offset: f64,
    pub coefficients: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RiskMeasure {
    ExpectedLoss,
    ConditionalValueAtRisk { confidence: f64 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioProblem {
    pub name: String,
    pub variables: Vec<Variable>,
    pub constraints: Vec<LinearConstraint>,
    pub scenarios: Vec<Scenario>,
    pub measure: RiskMeasure,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskResult {
    pub solve: SolveResult,
    pub decision_values: Option<Vec<f64>>,
    pub scenario_losses: Option<Vec<f64>>,
    pub expected_loss: Option<f64>,
    pub cvar: Option<f64>,
    pub confidence: Option<f64>,
    pub validation_note: String,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_risk_model", message)
}
#[derive(Clone, Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, x: f64) -> Result<()> {
        let next = self.value + x;
        let correction = if self.value.abs() >= x.abs() { (self.value - next) + x } else { (x - next) + self.value };
        self.correction += correction;
        self.value = next;
        if !self.value.is_finite() || !self.correction.is_finite() {
            return Err(SolverError::new("numerical_failure", "Risk summation overflow"));
        }
        Ok(())
    }
    fn total(&self) -> Result<f64> {
        let v = self.value + self.correction;
        if v.is_finite() { Ok(v) } else { Err(SolverError::new("numerical_failure", "Risk sum is not finite")) }
    }
}
fn normalized_weights(problem: &ScenarioProblem, options: &SolveOptions) -> Result<Vec<f64>> {
    preflight(problem, options)?;
    if problem.scenarios.is_empty() || problem.scenarios.len() > 100_000 {
        return Err(invalid("Provide between 1 and 100000 scenarios"));
    }
    let size = problem
        .scenarios
        .len()
        .checked_mul(problem.variables.len().saturating_add(4))
        .and_then(|x| x.checked_mul(16))
        .ok_or_else(|| invalid("Scenario allocation overflow"))?;
    if size as u64 > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "Scenario model exceeds memory budget"));
    }
    let mut total = Sum::default();
    for s in &problem.scenarios {
        if !s.weight.is_finite()
            || s.weight < 0.0
            || !s.offset.is_finite()
            || s.coefficients.len() != problem.variables.len()
            || s.coefficients.iter().any(|x| !x.is_finite())
        {
            return Err(invalid("Scenario weights, dimensions and coefficients must be finite and valid"));
        }
        total.add(s.weight)?;
    }
    let total = total.total()?;
    if total <= 0.0 {
        return Err(invalid("Scenario weights must have a positive finite total"));
    }
    Ok(problem.scenarios.iter().map(|x| x.weight / total).collect())
}

fn preflight(problem: &ScenarioProblem, options: &SolveOptions) -> Result<()> {
    use solvercraft_model::{MAX_CONSTRAINTS, MAX_NAME_BYTES, MAX_TERMS, MAX_VARIABLES};
    solvercraft_model::validate_options(options)?;
    let n = problem.variables.len();
    let s = problem.scenarios.len();
    let extra = if matches!(problem.measure, RiskMeasure::ConditionalValueAtRisk { .. }) { s.saturating_add(1) } else { 0 };
    let future_variables = n.checked_add(extra).ok_or_else(|| invalid("Risk variable overflow"))?;
    let future_rows = problem.constraints.len().checked_add(if extra > 0 { s } else { 0 }).ok_or_else(|| invalid("Risk row overflow"))?;
    if future_variables > MAX_VARIABLES || future_rows > MAX_CONSTRAINTS || problem.name.len() > MAX_NAME_BYTES {
        return Err(SolverError::new("model_limit", "Risk counterpart exceeds model limits"));
    }
    let mut names = problem.name.len();
    let mut terms = 0usize;
    for v in &problem.variables {
        if v.name.len() > MAX_NAME_BYTES {
            return Err(invalid("Variable name too long"));
        }
        names = names.checked_add(v.name.len()).ok_or_else(|| invalid("Name overflow"))?;
        solvercraft_model::effective_bounds(v)?;
    }
    for row in &problem.constraints {
        if row.name.len() > MAX_NAME_BYTES || !row.rhs.is_finite() {
            return Err(invalid("Invalid base constraint"));
        }
        names = names.checked_add(row.name.len()).ok_or_else(|| invalid("Name overflow"))?;
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
        if row.terms.iter().any(|t| t.variable >= n || !t.coefficient.is_finite()) {
            return Err(invalid("Invalid base constraint coefficient"));
        }
    }
    let transformed = if extra > 0 { s.checked_mul(n.saturating_add(3)).and_then(|v| v.checked_add(1)) } else { Some(n) };
    terms = terms.checked_add(transformed.ok_or_else(|| invalid("Counterpart term overflow"))?).ok_or_else(|| invalid("Term overflow"))?;
    if terms > MAX_TERMS {
        return Err(SolverError::new("model_limit", "Risk counterpart exceeds term limit"));
    }
    let size = terms
        .checked_mul(128)
        .and_then(|v| v.checked_add(names))
        .and_then(|v| v.checked_add(future_variables.saturating_mul(128)))
        .and_then(|v| v.checked_add(future_rows.saturating_mul(128)));
    if size.is_none_or(|v| v as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Risk counterpart exceeds memory budget before allocation"));
    }
    Ok(())
}
pub fn compile(problem: &ScenarioProblem, options: &SolveOptions) -> Result<OptimizationModel> {
    let weights = normalized_weights(problem, options)?;
    let mut model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: problem.name.clone(),
        variables: problem.variables.clone(),
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: problem.constraints.clone(),
    };
    match problem.measure {
        RiskMeasure::ExpectedLoss => {
            let mut coefficients = vec![Sum::default(); problem.variables.len()];
            let mut offset = Sum::default();
            for (s, w) in problem.scenarios.iter().zip(&weights) {
                offset.add(s.offset * w)?;
                for (c, v) in coefficients.iter_mut().zip(&s.coefficients) {
                    c.add(v * w)?;
                }
            }
            model.objective.offset = offset.total()?;
            for (variable, sum) in coefficients.into_iter().enumerate() {
                let coefficient = sum.total()?;
                if coefficient != 0.0 {
                    model.objective.linear.push(LinearTerm { variable, coefficient });
                }
            }
        }
        RiskMeasure::ConditionalValueAtRisk { confidence } => {
            if !confidence.is_finite() || !(0.0..1.0).contains(&confidence) {
                return Err(invalid("CVaR confidence must be in [0,1)"));
            }
            let eta = model.variables.len();
            model.variables.push(Variable { name: "__solvercraft_var".into(), lower: None, upper: None, kind: VariableKind::Continuous });
            model.objective.linear.push(LinearTerm { variable: eta, coefficient: 1.0 });
            for (i, (s, w)) in problem.scenarios.iter().zip(weights).enumerate() {
                let excess = model.variables.len();
                model.variables.push(Variable {
                    name: format!("__solvercraft_tail_{i}"),
                    lower: Some(0.0),
                    upper: None,
                    kind: VariableKind::Continuous,
                });
                model.objective.linear.push(LinearTerm { variable: excess, coefficient: w / (1.0 - confidence) });
                let mut terms: Vec<_> = s
                    .coefficients
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v != 0.0)
                    .map(|(variable, v)| LinearTerm { variable, coefficient: *v })
                    .collect();
                terms.push(LinearTerm { variable: eta, coefficient: -1.0 });
                terms.push(LinearTerm { variable: excess, coefficient: -1.0 });
                model.constraints.push(LinearConstraint { name: format!("tail_scenario_{i}"), terms, relation: Relation::LessEqual, rhs: -s.offset });
            }
        }
    }
    solvercraft_model::validate(&model, options)?;
    Ok(model)
}
pub fn weighted_upper_tail_cvar(losses: &[f64], weights: &[f64], confidence: f64) -> Result<f64> {
    if losses.len() > 1_000_000 {
        return Err(SolverError::new("sample_limit", "Weighted tail helper is limited to one million observations"));
    }
    if losses.is_empty()
        || losses.len() != weights.len()
        || !confidence.is_finite()
        || !(0.0..1.0).contains(&confidence)
        || losses.iter().any(|x| !x.is_finite())
        || weights.iter().any(|x| !x.is_finite() || *x < 0.0)
    {
        return Err(invalid("Invalid weighted tail data"));
    }
    let mut weight_sum = Sum::default();
    for weight in weights {
        weight_sum.add(*weight)?;
    }
    let total = weight_sum.total()?;
    if total <= 0.0 {
        return Err(invalid("Invalid weight total"));
    }
    let mut ordered: Vec<_> = losses.iter().copied().zip(weights.iter().map(|w| w / total)).collect();
    ordered.sort_by(|a, b| b.0.total_cmp(&a.0));
    let tail = 1.0 - confidence;
    let mut remaining = tail;
    let mut sum = Sum::default();
    for (value, weight) in ordered {
        let take = remaining.min(weight);
        sum.add(value * (take / tail))?;
        remaining = (remaining - take).max(0.0);
        if remaining == 0.0 {
            break;
        }
    }
    let result = sum.total()?;
    if result.is_finite() { Ok(result) } else { Err(SolverError::new("numerical_failure", "Tail mean overflow")) }
}
pub fn optimize(problem: &ScenarioProblem, options: &SolveOptions, control: &dyn RunControl) -> Result<RiskResult> {
    let model = compile(problem, options)?;
    let solve = solvercraft_engines::solve(&model, options, control)?;
    let decision_values = solve.values.as_ref().map(|v| v.iter().take(problem.variables.len()).copied().collect::<Vec<_>>());
    let weights = normalized_weights(problem, options)?;
    let mut scenario_losses = None;
    let mut expected_loss = None;
    let mut cvar = None;
    let confidence = match problem.measure {
        RiskMeasure::ExpectedLoss => None,
        RiskMeasure::ConditionalValueAtRisk { confidence } => Some(confidence),
    };
    if let Some(values) = &decision_values {
        let losses: Vec<f64> = problem
            .scenarios
            .iter()
            .map(|s| {
                let mut sum = Sum::default();
                sum.add(s.offset)?;
                for (c, v) in s.coefficients.iter().zip(values) {
                    sum.add(c * v)?;
                }
                sum.total()
            })
            .collect::<Result<_>>()?;
        if losses.iter().any(|x| !x.is_finite()) {
            return Err(SolverError::new("numerical_failure", "Scenario loss overflow"));
        }
        let mut mean = Sum::default();
        for (l, w) in losses.iter().zip(&weights) {
            mean.add(l * w)?;
        }
        let mean = mean.total()?;
        if !mean.is_finite() {
            return Err(SolverError::new("numerical_failure", "Expected loss overflow"));
        }
        expected_loss = Some(mean);
        if let Some(alpha) = confidence {
            cvar = Some(weighted_upper_tail_cvar(&losses, &weights, alpha)?);
        }
        scenario_losses = Some(losses);
    }
    Ok(RiskResult {
        solve,
        decision_values,
        scenario_losses,
        expected_loss,
        cvar,
        confidence,
        validation_note: "Exact finite-scenario formulation; scenario performance is in-sample, not an out-of-sample probability guarantee.".into(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxUncertainty {
    pub constraint: usize,
    pub radii: Vec<f64>,
}
/// Robustifies selected <= rows against independent, symmetric coefficient boxes.
pub fn robust_box(model: &OptimizationModel, uncertainties: &[BoxUncertainty], options: &SolveOptions) -> Result<OptimizationModel> {
    solvercraft_model::validate(model, options)?;
    let n = model.variables.len();
    let requested = uncertainties.len().checked_mul(n).ok_or_else(|| invalid("Robust size overflow"))?;
    if requested > 100_000 || (requested as u64).saturating_mul(256) > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "Robust counterpart exceeds budget"));
    }
    let mut checked = std::collections::BTreeSet::new();
    let mut auxiliaries = 0usize;
    for u in uncertainties {
        if !checked.insert(u.constraint)
            || u.constraint >= model.constraints.len()
            || u.radii.len() != n
            || u.radii.iter().any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(invalid("Invalid or duplicate box constraint"));
        }
        if model.constraints.get(u.constraint).is_none_or(|r| r.relation != Relation::LessEqual) {
            return Err(SolverError::new("unsupported_robust_relation", "Box counterpart supports <= constraints only"));
        }
        auxiliaries = auxiliaries.checked_add(u.radii.iter().filter(|r| **r > 0.0).count()).ok_or_else(|| invalid("Robust dimension overflow"))?;
    }
    let rows = model.constraints.len().saturating_add(auxiliaries.saturating_mul(2));
    let variables = n.saturating_add(auxiliaries);
    let terms = model
        .objective
        .linear
        .len()
        .saturating_add(model.objective.quadratic.len())
        .saturating_add(model.constraints.iter().map(|r| r.terms.len()).sum::<usize>())
        .saturating_add(auxiliaries.saturating_mul(5));
    if rows > solvercraft_model::MAX_CONSTRAINTS || variables > solvercraft_model::MAX_VARIABLES || terms > solvercraft_model::MAX_TERMS {
        return Err(SolverError::new("model_limit", "Robust counterpart exceeds structural limits"));
    }
    let names = model
        .name
        .len()
        .saturating_add(model.variables.iter().map(|v| v.name.len()).sum::<usize>())
        .saturating_add(model.constraints.iter().map(|r| r.name.len()).sum::<usize>());
    let size = terms
        .saturating_mul(128)
        .saturating_add(variables.saturating_mul(128))
        .saturating_add(rows.saturating_mul(128))
        .saturating_add(names)
        .saturating_add(auxiliaries.saturating_mul(160));
    if size as u64 > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "Robust allocation exceeds budget before cloning"));
    }
    let mut out = model.clone();
    let mut seen = std::collections::BTreeSet::new();
    for u in uncertainties {
        if !seen.insert(u.constraint)
            || u.constraint >= model.constraints.len()
            || u.radii.len() != n
            || u.radii.iter().any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(invalid("Invalid or duplicate box constraint"));
        }
        let row = model.constraints.get(u.constraint).ok_or_else(|| invalid("Missing constraint"))?;
        if row.relation != Relation::LessEqual {
            return Err(SolverError::new("unsupported_robust_relation", "Box counterpart supports <= constraints only"));
        }
        let mut extra = vec![];
        for (i, radius) in u.radii.iter().copied().enumerate().filter(|(_, r)| *r > 0.0) {
            let a = out.variables.len();
            out.variables.push(Variable {
                name: format!("__abs_{}_{}", u.constraint, i),
                lower: Some(0.0),
                upper: None,
                kind: VariableKind::Continuous,
            });
            for sign in [-1.0, 1.0] {
                out.constraints.push(LinearConstraint {
                    name: format!("abs_{}_{}_{}", u.constraint, i, sign),
                    terms: vec![LinearTerm { variable: i, coefficient: sign }, LinearTerm { variable: a, coefficient: -1.0 }],
                    relation: Relation::LessEqual,
                    rhs: 0.0,
                });
            }
            extra.push(LinearTerm { variable: a, coefficient: radius });
        }
        let target = out.constraints.get_mut(u.constraint).ok_or_else(|| invalid("Missing counterpart row"))?;
        target.terms.extend(extra);
    }
    solvercraft_model::validate(&out, options)?;
    Ok(out)
}
