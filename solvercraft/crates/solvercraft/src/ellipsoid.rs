//! Ellipsoidal coefficient uncertainty as checked Lorentz-cone constraints.
use serde::{Deserialize, Serialize};
use solvercraft_engines::{AffineExpression, ConicDetailedResult, ConicModel, SecondOrderConeConstraint};
use solvercraft_model::ExactProducts;
use solvercraft_types::*;
use std::sync::Arc;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EllipsoidUncertainty {
    pub constraint: usize,
    /// Rows of L, defining coefficient deviation delta = radius * L^T u,
    /// with ||u||_2 <= 1. Each row has one entry per original variable.
    pub factor: Vec<Vec<f64>>,
    pub radius: f64,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_ellipsoid", message)
}
fn numerical(message: &str) -> SolverError {
    SolverError::new("numeric_range", message)
}
fn preflight(model: &OptimizationModel, uncertainties: &[EllipsoidUncertainty], options: &SolveOptions) -> Result<()> {
    solvercraft_model::validate(model, options)?;
    if uncertainties.len() > solvercraft_model::MAX_CONSTRAINTS {
        return Err(invalid("Too many uncertain rows"));
    }
    let mut terms = model.objective.linear.len() + model.objective.quadratic.len();
    let mut components = 0usize;
    let mut names = model.name.len();
    for variable in &model.variables {
        names = names.saturating_add(variable.name.len());
    }
    for row in &model.constraints {
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
        names = names.saturating_add(row.name.len());
    }
    for uncertainty in uncertainties {
        let row = model.constraints.get(uncertainty.constraint).ok_or_else(|| invalid("Invalid constraint handle"))?;
        if row.relation != Relation::LessEqual || !uncertainty.radius.is_finite() || uncertainty.radius < 0.0 || uncertainty.factor.is_empty() {
            return Err(invalid("Ellipsoids require <= rows, a nonnegative finite radius and a nonempty factor"));
        }
        components = components.checked_add(uncertainty.factor.len()).ok_or_else(|| invalid("Cone dimension overflow"))?;
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
        for factor in &uncertainty.factor {
            if factor.len() != model.variables.len() || factor.iter().any(|coefficient| !coefficient.is_finite()) {
                return Err(invalid("Factor columns must match variables and contain finite coefficients"));
            }
            terms = terms.checked_add(factor.len()).ok_or_else(|| invalid("Factor term overflow"))?;
        }
    }
    if terms > solvercraft_model::MAX_TERMS || components > solvercraft_model::MAX_CONSTRAINTS {
        return Err(SolverError::new("model_limit", "Ellipsoid counterpart exceeds bounded cone dimensions"));
    }
    let bytes = terms
        .checked_mul(256)
        .and_then(|x| x.checked_add(components.checked_mul(256)?))
        .and_then(|x| x.checked_add(names.checked_mul(2)?))
        .and_then(|x| x.checked_add(16_384));
    if bytes.is_none_or(|x| x as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Ellipsoid counterpart exceeds budget before allocation"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for uncertainty in uncertainties {
        if !seen.insert(uncertainty.constraint) {
            return Err(invalid("Duplicate uncertain constraint"));
        }
    }
    Ok(())
}
/// A uniform L1 bound also bounds the Euclidean norm of the coefficient-formation
/// perturbation. Products and residuals remain exact until directed enclosure.
fn formation_allowance(model: &OptimizationModel, uncertainty: &EllipsoidUncertainty, options: &SolveOptions) -> Result<f64> {
    let mut total = ExactProducts::default();
    for factor in &uncertainty.factor {
        let mut component = ExactProducts::default();
        for (coefficient, variable) in factor.iter().zip(&model.variables) {
            let rounded = *coefficient * uncertainty.radius;
            if !rounded.is_finite() || (*coefficient != 0.0 && uncertainty.radius != 0.0 && rounded.abs() < f64::MIN_POSITIVE) {
                return Err(numerical("Ellipsoid factor product exceeds supported finite precision"));
            }
            let mut error = ExactProducts::default();
            error.add_product(*coefficient, uncertainty.radius)?;
            error.add(-rounded)?;
            let sign = match error.exact_sign()? {
                std::cmp::Ordering::Equal => continue,
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Less => -1.0,
            };
            let (Some(lower), Some(upper)) = solvercraft_model::effective_bounds(variable)? else {
                return Err(numerical("Rounded ellipsoid coefficients require finite variable bounds for a uniform formation-error bound"));
            };
            let magnitude = lower.abs().max(upper.abs());
            component.add_triple(sign * *coefficient, uncertainty.radius, magnitude)?;
            component.add_product(-sign * rounded, magnitude)?;
        }
        total.add(component.enclosure()?.1)?;
    }
    let allowance = total.enclosure()?.1;
    if allowance > options.feasibility_tolerance * 0.125 {
        return Err(numerical("Ellipsoid coefficient formation error is material over the original variable box"));
    }
    Ok(allowance)
}
fn formed_offset(rhs: f64, allowance: f64, options: &SolveOptions) -> Result<f64> {
    let mut offset = ExactProducts::default();
    offset.add(rhs)?;
    offset.add(allowance)?;
    let offset = offset.enclosure()?.1;
    let mut widening = ExactProducts::default();
    widening.add(offset)?;
    widening.add(-rhs)?;
    if widening.enclosure()?.1 > options.feasibility_tolerance * 0.25 {
        return Err(numerical("Outward ellipsoid RHS formation exceeds the declared feasibility precision"));
    }
    Ok(offset)
}
/// a^T x <= b becomes ||radius L x||_2 <= b-a^T x. Directions are never
/// discretized. Exactly representable coefficient products preserve the exact
/// counterpart; accepted tiny formation errors receive an outward RHS allowance
/// so the conic model is a valid relaxation. Material/unbounded errors are refused.
/// Use [`solve_owned`] to check incumbents against the original robust rows.
pub fn compile(model: &OptimizationModel, uncertainties: &[EllipsoidUncertainty], options: &SolveOptions) -> Result<ConicModel> {
    preflight(model, uncertainties, options)?;
    // Validate every transform before copying/constructing any counterpart.
    for uncertainty in uncertainties {
        formed_offset(model.constraints[uncertainty.constraint].rhs, formation_allowance(model, uncertainty, options)?, options)?;
    }
    let mut cones = Vec::new();
    cones.try_reserve_exact(uncertainties.len()).map_err(|_| invalid("Cannot allocate cone constraints"))?;
    for uncertainty in uncertainties {
        let row = &model.constraints[uncertainty.constraint];
        let allowance = formation_allowance(model, uncertainty, options)?;
        let offset = formed_offset(row.rhs, allowance, options)?;
        let components = uncertainty
            .factor
            .iter()
            .map(|factor| AffineExpression {
                offset: 0.0,
                terms: factor
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c != 0.0 && uncertainty.radius != 0.0)
                    .map(|(variable, c)| LinearTerm { variable, coefficient: *c * uncertainty.radius })
                    .collect(),
            })
            .collect();
        let upper =
            AffineExpression { offset, terms: row.terms.iter().map(|t| LinearTerm { variable: t.variable, coefficient: -t.coefficient }).collect() };
        cones.push(SecondOrderConeConstraint { name: row.name.clone(), components, upper });
    }
    Ok(ConicModel { base: model.clone(), second_order_cones: cones })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EllipsoidValidation {
    pub feasible: bool,
    pub max_violation: f64,
    pub objective: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EllipsoidSolveResult {
    pub counterpart: ConicModel,
    pub result: ConicDetailedResult,
}

/// Directed norm upper bound from exact component enclosures. Power-independent
/// scaling prevents overflow in the sum of squares; each division is enclosed
/// outward, exact products sum their squares, and sqrt is rounded outward.
fn norm_upper(components: &[(f64, f64)]) -> Result<f64> {
    let maximum = components.iter().map(|(lower, upper)| lower.abs().max(upper.abs())).fold(0.0_f64, f64::max);
    if maximum == 0.0 {
        return Ok(0.0);
    }
    let mut squares = ExactProducts::default();
    for (lower, upper) in components {
        let magnitude = lower.abs().max(upper.abs());
        let ratio = if magnitude == 0.0 { 0.0 } else { (magnitude / maximum).next_up().min(1.0) };
        squares.add_product(ratio, ratio)?;
    }
    let root = squares.enclosure()?.1.sqrt().next_up();
    let mut value = ExactProducts::default();
    value.add_product(maximum, root)?;
    value.enclosure().map(|(_, upper)| upper)
}

/// Independently evaluates radius*L*x as exact triples before the norm, rather
/// than checking only the rounded counterpart coefficients. Conservative
/// directed norm/row enclosures can reject a numerically ambiguous boundary.
pub fn validate_original(
    model: &OptimizationModel,
    uncertainties: &[EllipsoidUncertainty],
    values: &[f64],
    options: &SolveOptions,
) -> Result<EllipsoidValidation> {
    preflight(model, uncertainties, options)?;
    let original = solvercraft_model::validate_solution(model, values, options)?;
    let mut violation = original.max_violation;
    for uncertainty in uncertainties {
        let mut components = Vec::new();
        components
            .try_reserve_exact(uncertainty.factor.len())
            .map_err(|_| SolverError::new("memory_limit", "Cannot allocate original ellipsoid validation scratch"))?;
        for factor in &uncertainty.factor {
            let mut component = ExactProducts::default();
            for (coefficient, value) in factor.iter().zip(values) {
                component.add_triple(uncertainty.radius, *coefficient, *value)?;
            }
            components.push(component.enclosure()?);
        }
        let norm = norm_upper(&components)?;
        let row = &model.constraints[uncertainty.constraint];
        let mut upper = ExactProducts::default();
        upper.add(row.rhs)?;
        for term in &row.terms {
            upper.add_product(-term.coefficient, values[term.variable])?;
        }
        let mut excess = ExactProducts::default();
        excess.add(norm)?;
        excess.add(-upper.enclosure()?.0)?;
        violation = violation.max(excess.enclosure()?.1.max(0.0));
    }
    Ok(EllipsoidValidation {
        feasible: original.feasible && violation <= options.feasibility_tolerance,
        max_violation: violation,
        objective: original.objective,
    })
}

/// Solve the outward conic relaxation and independently validate any returned
/// incumbent in the original ellipsoid model. Relaxation bounds remain valid
/// for the original feasible set; unchecked original candidates never escape.
pub fn solve_owned(
    model: &OptimizationModel,
    uncertainties: &[EllipsoidUncertainty],
    options: &SolveOptions,
    control: Arc<dyn RunControl>,
) -> Result<EllipsoidSolveResult> {
    let counterpart = compile(model, uncertainties, options)?;
    let mut result = solvercraft_engines::solve_socp_owned(&counterpart, options, control)?;
    if let Some(values) = &result.result.values {
        let original = validate_original(model, uncertainties, values, options)?;
        if !original.feasible {
            return Err(numerical("Conic incumbent violates the original ellipsoidal robust constraints"));
        }
        result.result.objective = Some(original.objective);
        result.result.max_violation = Some(original.max_violation);
    }
    result.result.warnings.push("Original ellipsoid rows independently checked with exact radius*factor*x triples; tiny accepted coefficient formation uses an outward bounded relaxation.".into());
    Ok(EllipsoidSolveResult { counterpart, result })
}
