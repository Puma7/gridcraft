//! Validation and original-model arithmetic for Solvercraft.
//!
//! All coefficients and supplied values must be finite. Bounds use `None` for an
//! open side. Feasibility uses absolute tolerances in the original model units.
#![forbid(unsafe_code)]

use solvercraft_types::{
    LinearTerm, OptimizationModel, QuadraticTerm, Relation, Result, SCHEMA_VERSION, SolveOptions, SolverError, Variable, VariableKind,
};
use std::collections::BTreeMap;

pub const MAX_VARIABLES: usize = 10_000;
pub const MAX_CONSTRAINTS: usize = 50_000;
pub const MAX_TERMS: usize = 1_000_000;
pub const MAX_NAME_BYTES: usize = 4096;
/// At and above this magnitude f64 no longer represents every integer.
pub const INTEGER_PRECISION_LIMIT: f64 = 9_007_199_254_740_992.0;

/// A failed bound, integrality or row check in the untransformed model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    LowerBound,
    UpperBound,
    Integrality,
    Constraint,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub kind: ViolationKind,
    /// Variable index or constraint index, according to kind.
    pub index: usize,
    /// Absolute violation in original units.
    pub amount: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SolutionValidation {
    pub feasible: bool,
    pub max_violation: f64,
    pub objective: f64,
    pub violations: Vec<Violation>,
}

fn invalid(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn finite(value: f64, label: &str) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid("non_finite", format!("{label} must be finite")));
    }
    Ok(())
}
fn check_name(name: &str, label: &str) -> Result<()> {
    if name.len() > MAX_NAME_BYTES {
        return Err(invalid("model_limit", format!("{label} exceeds {MAX_NAME_BYTES} bytes")));
    }
    Ok(())
}
fn checked_bytes(count: usize, each: usize) -> Result<usize> {
    count.checked_mul(each).ok_or_else(|| invalid("model_limit", "model size overflow"))
}
fn add_size(total: &mut usize, value: usize) -> Result<()> {
    *total = total.checked_add(value).ok_or_else(|| invalid("model_limit", "model size overflow"))?;
    Ok(())
}

/// Validate numeric options. Zero work budgets are permitted so that callers can
/// request an immediate interrupted result rather than special-case this API.
pub fn validate_options(options: &SolveOptions) -> Result<()> {
    for (name, value) in [
        ("feasibility_tolerance", options.feasibility_tolerance),
        ("integrality_tolerance", options.integrality_tolerance),
        ("optimality_tolerance", options.optimality_tolerance),
        ("relative_gap", options.relative_gap),
    ] {
        finite(value, name)?;
    }
    if options.feasibility_tolerance <= 0.0 || options.optimality_tolerance <= 0.0 {
        return Err(invalid("invalid_options", "feasibility and optimality tolerances must be positive"));
    }
    if !(0.0..0.5).contains(&options.integrality_tolerance) {
        return Err(invalid("invalid_options", "integrality tolerance must be in [0, 0.5)"));
    }
    if !(0.0..=1.0).contains(&options.relative_gap) {
        return Err(invalid("invalid_options", "relative gap must be in [0, 1]"));
    }
    Ok(())
}

/// Return bounds after applying the binary domain. Fractional bounds on integer
/// variables are valid; rounding them inward is a solver transformation.
pub fn effective_bounds(variable: &Variable) -> Result<(Option<f64>, Option<f64>)> {
    if let Some(value) = variable.lower {
        finite(value, "lower bound")?;
    }
    if let Some(value) = variable.upper {
        finite(value, "upper bound")?;
    }
    if variable.kind != VariableKind::Continuous && variable.lower.into_iter().chain(variable.upper).any(|x| x.abs() >= INTEGER_PRECISION_LIMIT) {
        return Err(invalid("unsupported_integer_precision", "integer bounds must have magnitude below 2^53"));
    }
    let (lower, upper) = if variable.kind == VariableKind::Binary {
        (Some(variable.lower.map_or(0.0, |v| v.max(0.0))), Some(variable.upper.map_or(1.0, |v| v.min(1.0))))
    } else {
        (variable.lower, variable.upper)
    };
    if lower.zip(upper).is_some_and(|(lo, hi)| lo > hi) {
        return Err(invalid("infeasible_bounds", format!("empty domain for variable '{}'", variable.name)));
    }
    if variable.kind != VariableKind::Continuous && lower.zip(upper).is_some_and(|(lo, hi)| lo.ceil() > hi.floor()) {
        return Err(invalid("infeasible_bounds", format!("domain contains no integer for variable '{}'", variable.name)));
    }
    Ok((lower, upper))
}

/// Validate a variable's name and representable nonempty domain.
pub fn validate_variable(variable: &Variable) -> Result<()> {
    check_name(&variable.name, "variable name")?;
    effective_bounds(variable)?;
    Ok(())
}

fn check_linear(terms: &[LinearTerm], variables: usize, label: &str) -> Result<()> {
    for term in terms {
        if term.variable >= variables {
            return Err(invalid("invalid_variable", format!("{label} refers to variable {}", term.variable)));
        }
        finite(term.coefficient, label)?;
    }
    Ok(())
}

/// Validate schema, finite data, handles, domains, options and bounded input size.
pub fn validate(model: &OptimizationModel, options: &SolveOptions) -> Result<()> {
    validate_options(options)?;
    if model.schema_version != SCHEMA_VERSION {
        return Err(invalid("unsupported_schema", format!("unsupported model schema {}", model.schema_version)));
    }
    if model.variables.len() > MAX_VARIABLES || model.constraints.len() > MAX_CONSTRAINTS {
        return Err(invalid("model_limit", "too many variables or constraints"));
    }
    check_name(&model.name, "model name")?;
    let mut size = model.name.len();
    add_size(&mut size, checked_bytes(model.variables.len(), std::mem::size_of::<Variable>())?)?;
    add_size(&mut size, checked_bytes(model.constraints.len(), std::mem::size_of::<solvercraft_types::LinearConstraint>())?)?;
    let mut terms =
        model.objective.linear.len().checked_add(model.objective.quadratic.len()).ok_or_else(|| invalid("model_limit", "term count overflow"))?;
    if terms > MAX_TERMS {
        return Err(invalid("model_limit", "too many objective terms"));
    }
    for variable in &model.variables {
        validate_variable(variable)?;
        add_size(&mut size, variable.name.len())?;
    }
    finite(model.objective.offset, "objective offset")?;
    check_linear(&model.objective.linear, model.variables.len(), "objective coefficient")?;
    for term in &model.objective.quadratic {
        if term.left >= model.variables.len() || term.right >= model.variables.len() {
            return Err(invalid("invalid_variable", "quadratic objective has an invalid variable handle"));
        }
        finite(term.coefficient, "quadratic coefficient")?;
    }
    for row in &model.constraints {
        check_name(&row.name, "constraint name")?;
        add_size(&mut size, row.name.len())?;
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("model_limit", "term count overflow"))?;
        if terms > MAX_TERMS {
            return Err(invalid("model_limit", "too many model terms"));
        }
        finite(row.rhs, "constraint right hand side")?;
        check_linear(&row.terms, model.variables.len(), "constraint coefficient")?;
    }
    if terms > MAX_TERMS {
        return Err(invalid("model_limit", "too many model terms"));
    }
    // Includes conservative space for normalization maps as well as raw terms.
    add_size(&mut size, checked_bytes(terms, 128)?)?;
    let size_u64 = u64::try_from(size).map_err(|_| invalid("model_limit", "model size cannot be represented"))?;
    if size_u64 > options.max_memory_bytes {
        return Err(invalid("memory_limit", "model validation exceeds the memory budget"));
    }
    Ok(())
}

#[derive(Default, Clone, Copy)]
pub struct CompensatedSum {
    value: f64,
    correction: f64,
}
impl CompensatedSum {
    /// Neumaier compensation preserves a small addend even when a later term
    /// cancels the large running sum. Every intermediate remains checked.
    pub fn add(&mut self, term: f64) -> Result<()> {
        finite(term, "computed term")?;
        let next = self.value + term;
        finite(next, "computed sum")?;
        let residual = if self.value.abs() >= term.abs() { (self.value - next) + term } else { (term - next) + self.value };
        let correction = self.correction + residual;
        finite(correction, "summation correction")?;
        self.value = next;
        self.correction = correction;
        Ok(())
    }
    pub fn finish(self) -> Result<f64> {
        let value = self.value + self.correction;
        finite(value, "compensated sum")?;
        Ok(value)
    }
}
type Sum = CompensatedSum;

/// Checked Neumaier summation for finite modeled terms. Products that overflow
/// must be reported rather than silently canceling infinities.
pub fn compensated_sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut sum = Sum::default();
    for value in values {
        sum.add(value)?;
    }
    sum.finish()
}

/// Checked compensated dot product, with a strict length contract.
pub fn compensated_dot(left: &[f64], right: &[f64]) -> Result<f64> {
    if left.len() != right.len() {
        return Err(invalid("invalid_values", "dot product lengths differ"));
    }
    compensated_sum(left.iter().zip(right).map(|(a, b)| a * b))
}
fn check_values(model: &OptimizationModel, values: &[f64]) -> Result<()> {
    if values.len() != model.variables.len() {
        return Err(invalid("invalid_values", "solution length does not match variable count"));
    }
    for value in values {
        finite(*value, "solution value")?;
    }
    Ok(())
}
/// Evaluate the objective exactly as represented, before any presolve or scaling.
pub fn evaluate_objective(model: &OptimizationModel, values: &[f64]) -> Result<f64> {
    check_values(model, values)?;
    let mut sum = Sum::default();
    sum.add(model.objective.offset)?;
    for term in &model.objective.linear {
        let value = values.get(term.variable).ok_or_else(|| invalid("invalid_variable", "invalid variable handle during evaluation"))?;
        sum.add(term.coefficient * value)?;
    }
    for term in &model.objective.quadratic {
        let left = values.get(term.left).ok_or_else(|| invalid("invalid_variable", "invalid quadratic variable"))?;
        let right = values.get(term.right).ok_or_else(|| invalid("invalid_variable", "invalid quadratic variable"))?;
        sum.add(term.coefficient * left * right)?;
    }
    sum.finish()
}

/// Check every original bound, integer domain and row, without changing values.
/// A rounded integer assignment must call this function again.
pub fn validate_solution(model: &OptimizationModel, values: &[f64], options: &SolveOptions) -> Result<SolutionValidation> {
    validate(model, options)?;
    check_values(model, values)?;
    let mut violations = Vec::new();
    let mut max_violation: f64 = 0.0;
    for (index, (variable, value)) in model.variables.iter().zip(values).enumerate() {
        let (lower, upper) = effective_bounds(variable)?;
        if variable.kind != VariableKind::Continuous && value.abs() >= INTEGER_PRECISION_LIMIT {
            return Err(invalid("unsupported_integer_precision", "integer candidate magnitude must be below 2^53"));
        }
        let checks = [
            (ViolationKind::LowerBound, lower.map_or(0.0, |lo| (lo - value).max(0.0)), options.feasibility_tolerance),
            (ViolationKind::UpperBound, upper.map_or(0.0, |hi| (value - hi).max(0.0)), options.feasibility_tolerance),
            (
                ViolationKind::Integrality,
                if variable.kind == VariableKind::Continuous { 0.0 } else { (value - value.round()).abs() },
                options.integrality_tolerance,
            ),
        ];
        for (kind, amount, tolerance) in checks {
            finite(amount, "bound violation")?;
            max_violation = max_violation.max(amount);
            if amount > tolerance {
                violations.push(Violation { kind, index, amount });
            }
        }
    }
    for (index, row) in model.constraints.iter().enumerate() {
        let mut sum = Sum::default();
        for term in &row.terms {
            let value = values.get(term.variable).ok_or_else(|| invalid("invalid_variable", "invalid variable handle during evaluation"))?;
            sum.add(term.coefficient * value)?;
        }
        sum.add(-row.rhs)?;
        let residual = sum.finish()?;
        finite(residual, "constraint residual")?;
        let amount = match row.relation {
            Relation::LessEqual => residual.max(0.0),
            Relation::Equal => residual.abs(),
            Relation::GreaterEqual => (-residual).max(0.0),
        };
        max_violation = max_violation.max(amount);
        if amount > options.feasibility_tolerance {
            violations.push(Violation { kind: ViolationKind::Constraint, index, amount });
        }
    }
    Ok(SolutionValidation { feasible: violations.is_empty(), max_violation, objective: evaluate_objective(model, values)?, violations })
}

fn aggregate_linear(terms: &[LinearTerm]) -> Result<Vec<LinearTerm>> {
    let mut map: BTreeMap<usize, Sum> = BTreeMap::new();
    for term in terms {
        map.entry(term.variable).or_default().add(term.coefficient)?;
    }
    let mut result = Vec::with_capacity(map.len());
    for (variable, sum) in map {
        let coefficient = sum.finish()?;
        if coefficient != 0.0 {
            result.push(LinearTerm { variable, coefficient });
        }
    }
    Ok(result)
}
/// Aggregate duplicates, sort variable handles and discard exact zero terms.
/// This is algebraic normalization, never tolerance-based term dropping.
pub fn canonicalize(model: &OptimizationModel, options: &SolveOptions) -> Result<OptimizationModel> {
    validate(model, options)?;
    let mut result = model.clone();
    result.objective.linear = aggregate_linear(&model.objective.linear)?;
    let mut quadratic: BTreeMap<(usize, usize), Sum> = BTreeMap::new();
    for term in &model.objective.quadratic {
        let pair = (term.left.min(term.right), term.left.max(term.right));
        quadratic.entry(pair).or_default().add(term.coefficient)?;
    }
    result.objective.quadratic.clear();
    for ((left, right), sum) in quadratic {
        let coefficient = sum.finish()?;
        if coefficient != 0.0 {
            result.objective.quadratic.push(QuadraticTerm { left, right, coefficient });
        }
    }
    for row in &mut result.constraints {
        row.terms = aggregate_linear(&row.terms)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
