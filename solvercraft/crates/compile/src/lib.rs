//! Structural compilation of a bounded expression AST.
//!
//! Compilation accepts affine constraints and an affine or quadratic objective.
//! Unsupported variable-dependent operators are explicitly rejected. No sampled
//! values are used to infer linearity, degree or convexity.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use solvercraft_types::{
    Direction, LinearConstraint, LinearTerm, Objective, OptimizationModel, QuadraticTerm, Relation, Result, SCHEMA_VERSION, SolveOptions,
    SolverError, Variable,
};
use std::collections::BTreeMap;

mod autodiff;
pub use autodiff::evaluate_with_gradient;

pub const MAX_EXPRESSION_DEPTH: usize = 64;
pub const MAX_EXPRESSION_NODES: usize = 100_000;
const MAX_POLYNOMIAL_TERMS: usize = 100_000;
const MAX_COMPILER_OPERATIONS: usize = 2_000_000;

/// Explicit operators are serialized as tagged JSON, for example
/// `{"op":"variable","index":0}`. Power has a nonnegative integer exponent;
/// exponent zero is defined as one, including at a zero base.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Constant { value: f64 },
    Variable { index: usize },
    Add { left: Box<Expr>, right: Box<Expr> },
    Subtract { left: Box<Expr>, right: Box<Expr> },
    Multiply { left: Box<Expr>, right: Box<Expr> },
    Divide { left: Box<Expr>, right: Box<Expr> },
    Negate { value: Box<Expr> },
    Power { base: Box<Expr>, exponent: u32 },
    Sin { value: Box<Expr> },
    Cos { value: Box<Expr> },
    Exp { value: Box<Expr> },
    Ln { value: Box<Expr> },
    Sqrt { value: Box<Expr> },
    Abs { value: Box<Expr> },
    Minimum { left: Box<Expr>, right: Box<Expr> },
    Maximum { left: Box<Expr>, right: Box<Expr> },
}
impl Expr {
    pub fn constant(value: f64) -> Self {
        Self::Constant { value }
    }
    pub fn variable(index: usize) -> Self {
        Self::Variable { index }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionConstraint {
    pub name: String,
    pub left: Expr,
    pub relation: Relation,
    pub right: Expr,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionModel {
    pub name: String,
    pub variables: Vec<Variable>,
    pub direction: Direction,
    pub objective: Expr,
    pub constraints: Vec<ExpressionConstraint>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionClass {
    Constant,
    Affine,
    Quadratic,
    Nonlinear,
}

fn error(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn checked(value: f64, operation: &str) -> Result<f64> {
    if value.is_finite() { Ok(value) } else { Err(error("numeric_error", format!("non-finite result in {operation}"))) }
}
fn unary_child(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Negate { value }
        | Expr::Sin { value }
        | Expr::Cos { value }
        | Expr::Exp { value }
        | Expr::Ln { value }
        | Expr::Sqrt { value }
        | Expr::Abs { value } => Some(value),
        Expr::Power { base, .. } => Some(base),
        _ => None,
    }
}
fn binary_children(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::Add { left, right }
        | Expr::Subtract { left, right }
        | Expr::Multiply { left, right }
        | Expr::Divide { left, right }
        | Expr::Minimum { left, right }
        | Expr::Maximum { left, right } => Some((left, right)),
        _ => None,
    }
}

/// Nonrecursive validation bounds work and recursion before arithmetic begins.
fn preflight(expr: &Expr, variables: usize, remaining_nodes: &mut usize) -> Result<()> {
    let mut stack = vec![(expr, 1usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH || *remaining_nodes == 0 {
            return Err(error("expression_limit", "expression depth or node budget exceeded"));
        }
        *remaining_nodes -= 1;
        match node {
            Expr::Constant { value } if !value.is_finite() => {
                return Err(error("non_finite", "expression constants must be finite"));
            }
            Expr::Variable { index } if *index >= variables => {
                return Err(error("invalid_variable", format!("expression references variable {index}")));
            }
            _ => {}
        }
        if let Some(child) = unary_child(node) {
            stack.push((child, depth + 1));
        }
        if let Some((left, right)) = binary_children(node) {
            stack.push((right, depth + 1));
            stack.push((left, depth + 1));
        }
    }
    Ok(())
}

fn eval_inner(expr: &Expr, values: &[f64]) -> Result<f64> {
    let result = match expr {
        Expr::Constant { value } => *value,
        Expr::Variable { index } => *values.get(*index).ok_or_else(|| error("invalid_variable", "expression variable is absent"))?,
        Expr::Add { left, right } => eval_inner(left, values)? + eval_inner(right, values)?,
        Expr::Subtract { left, right } => eval_inner(left, values)? - eval_inner(right, values)?,
        Expr::Multiply { left, right } => eval_inner(left, values)? * eval_inner(right, values)?,
        Expr::Divide { left, right } => {
            let denominator = eval_inner(right, values)?;
            if denominator == 0.0 {
                return Err(error("domain_error", "division by zero"));
            }
            eval_inner(left, values)? / denominator
        }
        Expr::Negate { value } => -eval_inner(value, values)?,
        Expr::Power { base, exponent } => eval_inner(base, values)?.powf(f64::from(*exponent)),
        Expr::Sin { value } => eval_inner(value, values)?.sin(),
        Expr::Cos { value } => eval_inner(value, values)?.cos(),
        Expr::Exp { value } => eval_inner(value, values)?.exp(),
        Expr::Ln { value } => {
            let value = eval_inner(value, values)?;
            if value <= 0.0 {
                return Err(error("domain_error", "logarithm requires a positive value"));
            }
            value.ln()
        }
        Expr::Sqrt { value } => {
            let value = eval_inner(value, values)?;
            if value < 0.0 {
                return Err(error("domain_error", "square root requires a nonnegative value"));
            }
            value.sqrt()
        }
        Expr::Abs { value } => eval_inner(value, values)?.abs(),
        Expr::Minimum { left, right } => eval_inner(left, values)?.min(eval_inner(right, values)?),
        Expr::Maximum { left, right } => eval_inner(left, values)?.max(eval_inner(right, values)?),
    };
    checked(result, "expression evaluation")
}

/// Evaluate supported nonlinear operators without pretending they are affine.
/// Values must be finite, and node/depth limits apply before recursion.
pub fn evaluate(expr: &Expr, values: &[f64]) -> Result<f64> {
    if values.len() > solvercraft_model::MAX_VARIABLES {
        return Err(error("model_limit", "too many expression variables"));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("non_finite", "expression inputs must be finite"));
    }
    let mut remaining_nodes = MAX_EXPRESSION_NODES;
    preflight(expr, values.len(), &mut remaining_nodes)?;
    eval_inner(expr, values)
}

/// Two-component coefficients keep cancellation residuals through polynomial
/// transformations instead of rounding away small terms before later sums.
#[derive(Clone, Copy)]
struct Coefficient {
    main: f64,
    correction: f64,
}
impl Coefficient {
    fn new(value: f64) -> Self {
        Self { main: value, correction: 0.0 }
    }
    fn add(&mut self, value: f64) -> Result<()> {
        checked(value, "polynomial coefficient")?;
        let next = checked(self.main + value, "polynomial coefficient aggregation")?;
        let residual = if self.main.abs() >= value.abs() { (self.main - next) + value } else { (value - next) + self.main };
        let correction = checked(self.correction + residual, "polynomial coefficient correction")?;
        self.main = next;
        self.correction = correction;
        checked(self.value(), "polynomial coefficient total")?;
        Ok(())
    }
    fn value(self) -> f64 {
        self.main + self.correction
    }
    fn parts(self) -> [f64; 2] {
        [self.main, self.correction]
    }
}
/// A monomial has zero, one or two sorted variable handles. Higher degrees
/// are never truncated, and zero terms are removed only after compensation.
#[derive(Default)]
struct Polynomial {
    terms: BTreeMap<Vec<usize>, Coefficient>,
}
impl Polynomial {
    fn constant(value: f64) -> Self {
        let mut terms = BTreeMap::new();
        if value != 0.0 {
            terms.insert(Vec::new(), Coefficient::new(value));
        }
        Self { terms }
    }
    fn degree(&self) -> usize {
        self.terms.keys().map(Vec::len).max().unwrap_or(0)
    }
    fn as_constant(&self) -> Option<f64> {
        if self.degree() == 0 { Some(self.terms.get(&Vec::new()).copied().map(Coefficient::value).unwrap_or(0.0)) } else { None }
    }
}
struct Budget {
    remaining_operations: usize,
    max_terms: usize,
}
impl Budget {
    fn for_options(options: &SolveOptions) -> Result<Self> {
        let bytes = usize::try_from(options.max_memory_bytes).unwrap_or(usize::MAX);
        let max_terms = MAX_POLYNOMIAL_TERMS.min(bytes / 128);
        if max_terms == 0 {
            return Err(error("memory_limit", "compiler memory budget is too small"));
        }
        Ok(Self { remaining_operations: MAX_COMPILER_OPERATIONS.min(bytes / 16), max_terms })
    }
    fn charge(&mut self) -> Result<()> {
        self.remaining_operations =
            self.remaining_operations.checked_sub(1).ok_or_else(|| error("expression_limit", "compiler operation budget exceeded"))?;
        Ok(())
    }
    fn insert(&mut self, result: &mut Polynomial, monomial: Vec<usize>, value: f64) -> Result<()> {
        self.charge()?;
        checked(value, "polynomial coefficient")?;
        let mut coefficient = result.terms.get(&monomial).copied().unwrap_or_else(|| Coefficient::new(0.0));
        coefficient.add(value)?;
        if coefficient.value() == 0.0 {
            result.terms.remove(&monomial);
        } else {
            if !result.terms.contains_key(&monomial) && result.terms.len() >= self.max_terms {
                return Err(error("expression_limit", "polynomial term budget exceeded"));
            }
            result.terms.insert(monomial, coefficient);
        }
        Ok(())
    }
}
fn add(left: Polynomial, right: Polynomial, right_scale: f64, budget: &mut Budget) -> Result<Polynomial> {
    let mut result = left;
    for (monomial, coefficient) in right.terms {
        for part in coefficient.parts() {
            if part != 0.0 {
                budget.insert(&mut result, monomial.clone(), part * right_scale)?;
            }
        }
    }
    Ok(result)
}
fn scale(poly: Polynomial, factor: f64, budget: &mut Budget) -> Result<Polynomial> {
    let mut result = Polynomial::default();
    for (monomial, coefficient) in poly.terms {
        for part in coefficient.parts() {
            if part != 0.0 {
                budget.insert(&mut result, monomial.clone(), part * factor)?;
            }
        }
    }
    Ok(result)
}
fn divide(poly: Polynomial, divisor: f64, budget: &mut Budget) -> Result<Polynomial> {
    let mut result = Polynomial::default();
    for (monomial, coefficient) in poly.terms {
        for part in coefficient.parts() {
            if part != 0.0 {
                budget.insert(&mut result, monomial.clone(), checked(part / divisor, "division")?)?;
            }
        }
    }
    Ok(result)
}
fn multiply(left: &Polynomial, right: &Polynomial, budget: &mut Budget) -> Result<Option<Polynomial>> {
    if left.degree().saturating_add(right.degree()) > 2 {
        return Ok(None);
    }
    let mut result = Polynomial::default();
    for (left_monomial, left_coefficient) in &left.terms {
        for (right_monomial, right_coefficient) in &right.terms {
            let mut monomial = left_monomial.clone();
            monomial.extend(right_monomial.iter().copied());
            monomial.sort_unstable();
            for left in left_coefficient.parts() {
                for right in right_coefficient.parts() {
                    if left != 0.0 && right != 0.0 {
                        budget.insert(&mut result, monomial.clone(), left * right)?;
                    }
                }
            }
        }
    }
    Ok(Some(result))
}
fn fold_unary(poly: Option<Polynomial>, op: fn(f64) -> f64) -> Result<Option<Polynomial>> {
    match poly.and_then(|p| p.as_constant()) {
        Some(value) => Ok(Some(Polynomial::constant(checked(op(value), "constant folding")?))),
        None => Ok(None),
    }
}
fn polynomial(expr: &Expr, budget: &mut Budget) -> Result<Option<Polynomial>> {
    budget.charge()?;
    match expr {
        Expr::Constant { value } => Ok(Some(Polynomial::constant(*value))),
        Expr::Variable { index } => {
            let mut p = Polynomial::default();
            budget.insert(&mut p, vec![*index], 1.0)?;
            Ok(Some(p))
        }
        Expr::Add { left, right } | Expr::Subtract { left, right } => {
            let l = polynomial(left, budget)?;
            let r = polynomial(right, budget)?;
            match (l, r) {
                (Some(l), Some(r)) => Ok(Some(add(l, r, if matches!(expr, Expr::Subtract { .. }) { -1.0 } else { 1.0 }, budget)?)),
                _ => Ok(None),
            }
        }
        Expr::Multiply { left, right } => {
            let l = polynomial(left, budget)?;
            let r = polynomial(right, budget)?;
            match (l, r) {
                (Some(l), Some(r)) => multiply(&l, &r, budget),
                _ => Ok(None),
            }
        }
        Expr::Divide { left, right } => {
            let l = polynomial(left, budget)?;
            let r = polynomial(right, budget)?;
            match (l, r.and_then(|p| p.as_constant())) {
                (_, Some(0.0)) => Err(error("domain_error", "division by zero")),
                (Some(l), Some(r)) => Ok(Some(divide(l, r, budget)?)),
                _ => Ok(None),
            }
        }
        Expr::Negate { value } => match polynomial(value, budget)? {
            Some(p) => Ok(Some(scale(p, -1.0, budget)?)),
            None => Ok(None),
        },
        Expr::Power { base, exponent } => {
            let p = polynomial(base, budget)?;
            if *exponent == 0 {
                // A variable-dependent unsupported base may restrict the domain.
                // Preserve/reject that source behavior rather than erasing it.
                return Ok(p.map(|_| Polynomial::constant(1.0)));
            }
            match p {
                None => Ok(None),
                Some(p) if *exponent == 1 => Ok(Some(p)),
                Some(p) if p.as_constant().is_some() => {
                    Ok(Some(Polynomial::constant(checked(p.as_constant().unwrap_or(0.0).powf(f64::from(*exponent)), "power")?)))
                }
                Some(p) if *exponent == 2 => multiply(&p, &p, budget),
                Some(_) => Ok(None),
            }
        }
        Expr::Sin { value } => fold_unary(polynomial(value, budget)?, f64::sin),
        Expr::Cos { value } => fold_unary(polynomial(value, budget)?, f64::cos),
        Expr::Exp { value } => fold_unary(polynomial(value, budget)?, f64::exp),
        Expr::Ln { value } => fold_unary(polynomial(value, budget)?, f64::ln),
        Expr::Sqrt { value } => fold_unary(polynomial(value, budget)?, f64::sqrt),
        Expr::Abs { value } => fold_unary(polynomial(value, budget)?, f64::abs),
        Expr::Minimum { left, right } | Expr::Maximum { left, right } => {
            let l = polynomial(left, budget)?.and_then(|p| p.as_constant());
            let r = polynomial(right, budget)?.and_then(|p| p.as_constant());
            match (l, r) {
                (Some(l), Some(r)) => Ok(Some(Polynomial::constant(if matches!(expr, Expr::Minimum { .. }) { l.min(r) } else { l.max(r) }))),
                _ => Ok(None),
            }
        }
    }
}

fn classify_poly(poly: Option<Polynomial>) -> ExpressionClass {
    match poly.map(|p| p.degree()) {
        Some(0) => ExpressionClass::Constant,
        Some(1) => ExpressionClass::Affine,
        Some(_) => ExpressionClass::Quadratic,
        None => ExpressionClass::Nonlinear,
    }
}
/// Structural classification. Algebraic cancellation of polynomial terms is
/// honored; unsupported operators remain conservatively nonlinear.
pub fn classify(expr: &Expr, variable_count: usize) -> Result<ExpressionClass> {
    if variable_count > solvercraft_model::MAX_VARIABLES {
        return Err(error("model_limit", "too many variables"));
    }
    let mut remaining_nodes = MAX_EXPRESSION_NODES;
    preflight(expr, variable_count, &mut remaining_nodes)?;
    let mut budget = Budget::for_options(&SolveOptions::default())?;
    Ok(classify_poly(polynomial(expr, &mut budget)?))
}
fn require_polynomial(expr: &Expr, budget: &mut Budget, location: &str) -> Result<Polynomial> {
    polynomial(expr, budget)?
        .ok_or_else(|| error("unsupported_expression", format!("{location}: variable-dependent expression is not representable as degree <= 2")))
}
fn objective(poly: Polynomial, direction: Direction) -> Result<Objective> {
    let mut objective = Objective { direction, offset: 0.0, linear: Vec::new(), quadratic: Vec::new() };
    for (monomial, coefficient) in poly.terms {
        let coefficient = checked(coefficient.value(), "objective coefficient")?;
        match monomial.as_slice() {
            [] => objective.offset = coefficient,
            [variable] => objective.linear.push(LinearTerm { variable: *variable, coefficient }),
            [left, right] => objective.quadratic.push(QuadraticTerm { left: *left, right: *right, coefficient }),
            _ => return Err(error("internal_error", "unsupported polynomial degree")),
        }
    }
    Ok(objective)
}

/// Compile a model with an affine/quadratic objective and affine rows. Convexity
/// is not inferred merely because an expression is quadratic.
pub fn compile(model: &ExpressionModel, options: &SolveOptions) -> Result<OptimizationModel> {
    solvercraft_model::validate_options(options)?;
    if model.variables.len() > solvercraft_model::MAX_VARIABLES || model.constraints.len() > solvercraft_model::MAX_CONSTRAINTS {
        return Err(error("model_limit", "too many expression variables or constraints"));
    }
    // Validate output metadata before cloning it or doing polynomial work.
    if model.name.len() > solvercraft_model::MAX_NAME_BYTES {
        return Err(error("model_limit", "expression model name is too long"));
    }
    let mut metadata = std::mem::size_of::<OptimizationModel>();
    let mut charge = |bytes: usize| -> Result<()> {
        metadata = metadata.checked_add(bytes).ok_or_else(|| error("model_limit", "compiler metadata size overflow"))?;
        Ok(())
    };
    charge(model.name.len())?;
    charge(
        model.variables.len().checked_mul(std::mem::size_of::<Variable>()).ok_or_else(|| error("model_limit", "variable metadata size overflow"))?,
    )?;
    charge(
        model
            .constraints
            .len()
            .checked_mul(std::mem::size_of::<LinearConstraint>())
            .ok_or_else(|| error("model_limit", "constraint metadata size overflow"))?,
    )?;
    for variable in &model.variables {
        solvercraft_model::validate_variable(variable)?;
        charge(variable.name.len())?;
    }
    for row in &model.constraints {
        if row.name.len() > solvercraft_model::MAX_NAME_BYTES {
            return Err(error("model_limit", "expression constraint name is too long"));
        }
        charge(row.name.len())?;
    }
    let metadata = u64::try_from(metadata).map_err(|_| error("model_limit", "compiler metadata cannot be represented"))?;
    let remaining_memory =
        options.max_memory_bytes.checked_sub(metadata).ok_or_else(|| error("memory_limit", "compiler metadata exceeds memory budget"))?;
    let compiler_options = SolveOptions { max_memory_bytes: remaining_memory, ..options.clone() };
    let mut remaining_nodes = MAX_EXPRESSION_NODES;
    preflight(&model.objective, model.variables.len(), &mut remaining_nodes)?;
    for constraint in &model.constraints {
        preflight(&constraint.left, model.variables.len(), &mut remaining_nodes)?;
        preflight(&constraint.right, model.variables.len(), &mut remaining_nodes)?;
    }
    let mut budget = Budget::for_options(&compiler_options)?;
    let compiled_objective = objective(require_polynomial(&model.objective, &mut budget, "objective")?, model.direction)?;
    let mut constraints = Vec::with_capacity(model.constraints.len());
    for (index, row) in model.constraints.iter().enumerate() {
        let left = require_polynomial(&row.left, &mut budget, &format!("constraint {index} left"))?;
        let right = require_polynomial(&row.right, &mut budget, &format!("constraint {index} right"))?;
        let combined = add(left, right, -1.0, &mut budget)?;
        if combined.degree() > 1 {
            return Err(error("unsupported_constraint", format!("constraint {index} is quadratic; only affine rows are supported")));
        }
        let mut terms = Vec::new();
        let mut rhs = 0.0;
        for (monomial, coefficient) in combined.terms {
            let coefficient = checked(coefficient.value(), "constraint coefficient")?;
            match monomial.as_slice() {
                [] => rhs = checked(-coefficient, "constraint rhs")?,
                [variable] => terms.push(LinearTerm { variable: *variable, coefficient }),
                _ => return Err(error("internal_error", "non-affine constraint")),
            }
        }
        constraints.push(LinearConstraint { name: row.name.clone(), terms, relation: row.relation, rhs });
    }
    let compiled = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: model.name.clone(),
        variables: model.variables.clone(),
        objective: compiled_objective,
        constraints,
    };
    solvercraft_model::validate(&compiled, options)?;
    Ok(compiled)
}

#[cfg(test)]
mod tests;
