//! Finite-scenario risk optimization and explicitly bounded robust counterparts.
use serde::{Deserialize, Serialize};
use solvercraft_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub weight: f64,
    pub offset: f64,
    pub coefficients: Vec<f64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RiskMeasure {
    ExpectedLoss,
    ConditionalValueAtRisk { confidence: f64 },
}
impl<'de> Deserialize<'de> for RiskMeasure {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Input {
            ExpectedLoss {},
            ConditionalValueAtRisk { confidence: f64 },
        }
        match Input::deserialize(deserializer)? {
            Input::ExpectedLoss {} => Ok(Self::ExpectedLoss),
            Input::ConditionalValueAtRisk { confidence } => Ok(Self::ConditionalValueAtRisk { confidence }),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
// Power-of-two scaling preserves input significands within the supported range.
fn binary_scale(maximum: f64) -> f64 {
    if maximum == 0.0 {
        return 1.0;
    }
    let bits = maximum.to_bits();
    let exponent = bits & 0x7ff0_0000_0000_0000;
    if exponent != 0 { f64::from_bits(exponent) } else { f64::from_bits(1_u64 << (63 - bits.leading_zeros())) }
}
fn numerical(message: &str) -> SolverError {
    SolverError::new("numerical_failure", message)
}
impl Sum {
    fn product(&mut self, a: f64, b: f64) -> Result<()> {
        let value = a * b;
        if !value.is_finite() || (a != 0.0 && b != 0.0 && value.abs() < f64::MIN_POSITIVE) {
            return Err(numerical("Risk product exceeds the supported finite precision range"));
        }
        self.add(value)?;
        // FMA recovers the multiplication residual before compensated accumulation.
        self.add(a.mul_add(b, -value))
    }
}
struct WeightPlan {
    scaled: Vec<f64>,
    total: Sum,
}
impl WeightPlan {
    fn new<I: ExactSizeIterator<Item = f64> + Clone>(weights: I) -> Result<Self> {
        if weights.len() == 0 || weights.len() > 1_000_000 {
            return Err(invalid("Invalid weight count"));
        }
        let mut maximum = 0.0_f64;
        for w in weights.clone() {
            if !w.is_finite() || w < 0.0 {
                return Err(invalid("Weights must be finite and nonnegative"));
            }
            maximum = maximum.max(w);
        }
        if maximum == 0.0 {
            return Err(invalid("Weights need positive total mass"));
        }
        let scale = binary_scale(maximum);
        let mut scaled = Vec::new();
        scaled.try_reserve_exact(weights.len()).map_err(|_| SolverError::new("allocation", "Cannot allocate risk weights"))?;
        let mut total = Sum::default();
        for w in weights {
            let value = w / scale;
            if w > 0.0 && value < f64::MIN_POSITIVE {
                return Err(numerical("Weight dynamic range exceeds supported precision"));
            }
            scaled.push(value);
            total.add(value)?;
        }
        Ok(Self { scaled, total })
    }
}
fn scaled_loss(value: f64, scale: f64) -> Result<f64> {
    let result = value / scale;
    if value != 0.0 && result.abs() < f64::MIN_POSITIVE {
        return Err(numerical("Loss dynamic range exceeds supported precision"));
    }
    Ok(result)
}
fn finish_average(area: &Sum, mass: &Sum, scale: f64, absolute_area: f64, terms: usize, minimum: f64, maximum: f64) -> Result<f64> {
    let denominator = mass.total()?;
    if denominator <= 0.0 {
        return Err(numerical("Risk average has no positive tail mass"));
    }
    let mut result = (area.total()? / denominator) * scale;
    if !result.is_finite() {
        return Err(numerical("Risk average exceeds finite arithmetic"));
    }
    // Twofold products and compensated sums still have finite precision. Reject a
    // severely cancelled result rather than silently return an unreliable risk number.
    let error_bound =
        ((192.0 * (terms as f64 + 1.0) * f64::EPSILON * f64::EPSILON) * absolute_area / denominator) * scale + 8.0 * f64::EPSILON * result.abs();
    let tolerance = 1e-10 * result.abs().max(1.0);
    if !error_bound.is_finite() || error_bound > tolerance {
        return Err(numerical("Risk average is too ill-conditioned for a reliable f64 result"));
    }
    if result < minimum {
        if minimum - result > tolerance {
            return Err(numerical("Risk average fell below its loss support"));
        }
        result = minimum;
    }
    if result > maximum {
        if result - maximum > tolerance {
            return Err(numerical("Risk average exceeded its loss support"));
        }
        result = maximum;
    }
    Ok(result)
}
fn weighted_average<I: ExactSizeIterator<Item = f64> + Clone>(values: I, weights: &WeightPlan) -> Result<f64> {
    if values.len() != weights.scaled.len() {
        return Err(invalid("Loss and weight counts differ"));
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (value, weight) in values.clone().zip(&weights.scaled) {
        if !value.is_finite() {
            return Err(invalid("Losses must be finite"));
        }
        if *weight > 0.0 {
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    if minimum == maximum {
        return Ok(minimum);
    }
    let scale = binary_scale(minimum.abs().max(maximum.abs()));
    let mut area = Sum::default();
    let mut absolute = Sum::default();
    let mut terms = 0usize;
    for (value, weight) in values.zip(&weights.scaled) {
        if *weight == 0.0 {
            continue;
        }
        let value = scaled_loss(value, scale)?;
        area.product(value, *weight)?;
        absolute.product(value.abs(), *weight)?;
        terms += 1;
    }
    finish_average(&area, &weights.total, scale, absolute.total()?, terms, minimum, maximum)
}
fn normalized_weights(problem: &ScenarioProblem, options: &SolveOptions) -> Result<WeightPlan> {
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
    for s in &problem.scenarios {
        if !s.weight.is_finite()
            || s.weight < 0.0
            || !s.offset.is_finite()
            || s.coefficients.len() != problem.variables.len()
            || s.coefficients.iter().any(|x| !x.is_finite())
        {
            return Err(invalid("Scenario weights, dimensions and coefficients must be finite and valid"));
        }
    }
    WeightPlan::new(problem.scenarios.iter().map(|s| s.weight))
}

fn preflight(problem: &ScenarioProblem, options: &SolveOptions) -> Result<()> {
    use solvercraft_model::{MAX_CONSTRAINTS, MAX_NAME_BYTES, MAX_TERMS, MAX_VARIABLES};
    solvercraft_model::validate_options(options)?;
    if let RiskMeasure::ConditionalValueAtRisk { confidence } = problem.measure {
        if !confidence.is_finite() || !(0.0..1.0).contains(&confidence) {
            return Err(invalid("CVaR confidence must be in [0,1)"));
        }
    }
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
        if terms > MAX_TERMS {
            return Err(SolverError::new("model_limit", "Risk counterpart exceeds term limit before coefficient traversal"));
        }
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
            model.objective.offset = weighted_average(problem.scenarios.iter().map(|s| s.offset), &weights)?;
            for variable in 0..problem.variables.len() {
                let coefficient =
                    weighted_average(problem.scenarios.iter().map(|s| s.coefficients.get(variable).copied().unwrap_or(f64::NAN)), &weights)?;
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
            for (i, (s, w)) in problem.scenarios.iter().zip(&weights.scaled).enumerate() {
                let excess = model.variables.len();
                model.variables.push(Variable {
                    name: format!("__solvercraft_tail_{i}"),
                    lower: Some(0.0),
                    upper: None,
                    kind: VariableKind::Continuous,
                });
                model.objective.linear.push(LinearTerm { variable: excess, coefficient: (w / weights.total.total()?) / (1.0 - confidence) });
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
    {
        return Err(invalid("Invalid weighted tail data"));
    }
    let plan = WeightPlan::new(weights.iter().copied())?;
    if confidence == 0.0 {
        return weighted_average(losses.iter().copied(), &plan);
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (value, weight) in losses.iter().zip(&plan.scaled) {
        if *weight > 0.0 {
            minimum = minimum.min(*value);
            maximum = maximum.max(*value);
        }
    }
    if minimum == maximum {
        return Ok(minimum);
    }
    let scale = binary_scale(minimum.abs().max(maximum.abs()));
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(losses.len()).map_err(|_| SolverError::new("allocation", "Cannot allocate ordered risk data"))?;
    for (value, weight) in losses.iter().zip(&plan.scaled) {
        if *weight > 0.0 {
            ordered.push((scaled_loss(*value, scale)?, *weight));
        }
    }
    ordered.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
    // Preserve the residual of 1-alpha and the total-weight expansion. Repeated
    // subtraction of rounded normalized probabilities can clip the last atom.
    let tail = 1.0 - confidence;
    let tail_low = (1.0 - tail) - confidence;
    let mut mass = Sum::default();
    for a in [tail, tail_low] {
        for b in [plan.total.value, plan.total.correction] {
            mass.product(a, b)?;
        }
    }
    let mut remaining = mass.clone();
    let mut area = Sum::default();
    let mut absolute = Sum::default();
    let mut terms = 0usize;
    let mut selected_minimum = f64::INFINITY;
    for (value, weight) in ordered {
        let mut difference = remaining.clone();
        difference.add(-weight)?;
        selected_minimum = selected_minimum.min(value);
        terms += 1;
        if difference.total()? >= 0.0 {
            area.product(value, weight)?;
            absolute.product(value.abs(), weight)?;
            remaining = difference;
            if remaining.total()? == 0.0 {
                break;
            }
        } else {
            for part in [remaining.value, remaining.correction] {
                area.product(value, part)?;
                absolute.product(value.abs(), part.abs())?;
            }
            remaining = Sum::default();
            break;
        }
    }
    if remaining.total()? > 0.0 {
        return Err(numerical("Available scenario mass did not cover the requested tail"));
    }
    if selected_minimum * scale == maximum {
        return Ok(maximum);
    }
    finish_average(&area, &mass, scale, absolute.total()?, terms, minimum, maximum)
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
        let mean = weighted_average(losses.iter().copied(), &weights)?;
        expected_loss = Some(mean);
        if let Some(alpha) = confidence {
            let raw_weights: Vec<f64> = problem.scenarios.iter().map(|s| s.weight).collect();
            cvar = Some(weighted_upper_tail_cvar(&losses, &raw_weights, alpha)?);
        }
        let original_measure = cvar.unwrap_or(mean);
        if let Some(reported) = solve.objective {
            let scale = reported.abs().max(original_measure.abs()).max(1.0);
            let tolerance = options.optimality_tolerance.max(options.feasibility_tolerance);
            if (reported / scale - original_measure / scale).abs() > tolerance + tolerance / scale {
                return Err(numerical("Transformed objective disagrees with the original scenario risk measure"));
            }
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
#[serde(deny_unknown_fields)]
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
