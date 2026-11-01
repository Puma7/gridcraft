//! Explicit bounded formulations for indicators and SOS1/SOS2.
use crate::{ConicModel, MixedConicResult, solve_mixed_conic_owned, validate_conic_solution};
use serde::{Deserialize, Serialize};
use solvercraft_model::{canonicalize, compensated_sum, effective_bounds};
use solvercraft_types::*;
use std::sync::Arc;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndicatorConstraint {
    pub binary: usize,
    pub active_value: bool,
    pub constraint: LinearConstraint,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SosConstraint {
    pub name: String,
    pub variables: Vec<usize>,
    pub weights: Vec<f64>,
    pub kind: SosKind,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SosKind {
    Sos1,
    Sos2,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscreteConicModel {
    pub model: ConicModel,
    pub indicators: Vec<IndicatorConstraint>,
    pub special_ordered_sets: Vec<SosConstraint>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscreteCompilation {
    pub model: ConicModel,
    pub original_variables: usize,
    pub auxiliary_binaries: usize,
    pub auxiliary_continuous: usize,
    pub formulations: Vec<String>,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_discrete_constraint", message)
}
fn binary(base: &mut OptimizationModel, name: String) -> usize {
    let i = base.variables.len();
    base.variables.push(Variable { name, kind: VariableKind::Binary, lower: Some(0.0), upper: Some(1.0) });
    i
}
fn add_row(base: &mut OptimizationModel, name: String, terms: Vec<LinearTerm>, rhs: f64) {
    base.constraints.push(LinearConstraint { name, terms, relation: Relation::LessEqual, rhs });
}
/// Produces an exact finite-bound MILP/MISOCP formulation, never an arbitrary M.
pub fn compile_discrete(input: &DiscreteConicModel, options: &SolveOptions) -> Result<DiscreteCompilation> {
    let normalized = canonicalize(&input.model.base, options)?;
    let n = normalized.variables.len();
    let additions = input
        .special_ordered_sets
        .iter()
        .try_fold(0usize, |s, set| s.checked_add(set.variables.len()).ok_or_else(|| invalid("SOS size overflow.")))?;
    let indicator_work = input.indicators.iter().try_fold(0_u64, |sum, i| {
        let directions = if i.constraint.relation == Relation::Equal { 2_u64 } else { 1 };
        (i.constraint.terms.len() as u64)
            .checked_add(16)
            .and_then(|v| v.checked_mul(directions))
            .and_then(|v| sum.checked_add(v))
            .ok_or_else(|| SolverError::new("memory_limit", "Indicator formulation dimensions overflow."))
    })?;
    let estimate = (n as u64)
        .checked_add(additions as u64)
        .and_then(|v| v.checked_mul(256))
        .and_then(|v| v.checked_add((additions as u64).checked_mul(1024)?))
        .and_then(|v| v.checked_add(indicator_work.checked_mul(128)?));
    if estimate.is_none_or(|v| v > options.max_memory_bytes / 4) {
        return Err(SolverError::new("memory_limit", "Discrete formulation estimate exceeds memory budget."));
    }
    let mut result = DiscreteCompilation {
        model: ConicModel { base: normalized, second_order_cones: input.model.second_order_cones.clone() },
        original_variables: n,
        auxiliary_binaries: 0,
        auxiliary_continuous: 0,
        formulations: vec![],
    };
    for indicator in &input.indicators {
        if indicator.binary >= n || result.model.base.variables[indicator.binary].kind != VariableKind::Binary {
            return Err(invalid("Indicator control must reference an original binary variable."));
        }
        if !indicator.constraint.rhs.is_finite() || indicator.constraint.terms.iter().any(|t| t.variable >= n || !t.coefficient.is_finite()) {
            return Err(invalid("Indicator row requires finite coefficients and original variable indices."));
        }
        let signs: &[f64] = match indicator.constraint.relation {
            Relation::LessEqual => &[1.0],
            Relation::GreaterEqual => &[-1.0],
            Relation::Equal => &[1.0, -1.0],
        };
        for sign in signs {
            let mut products = Vec::with_capacity(indicator.constraint.terms.len() + 1);
            for t in &indicator.constraint.terms {
                let c = sign * t.coefficient;
                let (low, high) = effective_bounds(&result.model.base.variables[t.variable])?;
                let bound = if c > 0.0 {
                    high
                } else if c < 0.0 {
                    low
                } else {
                    Some(0.0)
                };
                let Some(bound) = bound else {
                    return Err(SolverError::new("finite_indicator_bounds_required", "Indicator big-M must be derived from finite variable bounds."));
                };
                products.push(c * bound);
            }
            products.push(-sign * indicator.constraint.rhs);
            let raw = compensated_sum(products.iter().copied())?;
            let magnitude = compensated_sum(products.iter().map(|v| v.abs()))?;
            let m = (raw + 64.0 * f64::EPSILON * (1.0 + magnitude) * (1.0 + products.len() as f64)).max(0.0).next_up();
            if !m.is_finite() {
                return Err(invalid("Derived indicator big-M overflows finite arithmetic."));
            }
            // Keep the small original RHS separate from potentially huge M.
            // RHS+M can erase a feasible active boundary and invalidate bounds.
            let slack = result.model.base.variables.len();
            result.model.base.variables.push(Variable {
                name: format!("indicator:{}:{}:slack:{slack}", indicator.binary, indicator.constraint.name),
                kind: VariableKind::Continuous,
                lower: Some(0.0),
                upper: Some(m),
            });
            let mut terms =
                indicator.constraint.terms.iter().map(|t| LinearTerm { variable: t.variable, coefficient: sign * t.coefficient }).collect::<Vec<_>>();
            terms.push(LinearTerm { variable: slack, coefficient: -1.0 });
            add_row(
                &mut result.model.base,
                format!("indicator:{}:{}:row", indicator.binary, indicator.constraint.name),
                terms,
                sign * indicator.constraint.rhs,
            );
            add_row(
                &mut result.model.base,
                format!("indicator:{}:{}:gate", indicator.binary, indicator.constraint.name),
                vec![
                    LinearTerm { variable: slack, coefficient: 1.0 },
                    LinearTerm { variable: indicator.binary, coefficient: if indicator.active_value { m } else { -m } },
                ],
                if indicator.active_value { m } else { 0.0 },
            );
            result.auxiliary_continuous += 1;
            result.formulations.push(format!(
                "Indicator {} uses a nonnegative slack gate with outward finite-bound M={m}; original RHS is not added to M.",
                indicator.constraint.name
            ));
        }
    }
    for set in &input.special_ordered_sets {
        if set.variables.is_empty() || set.variables.len() != set.weights.len() || set.weights.iter().any(|v| !v.is_finite()) {
            return Err(invalid("SOS variables and finite weights must have equal nonzero length."));
        }
        let mut ordered = set.variables.iter().copied().zip(set.weights.iter().copied()).collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.1.total_cmp(&b.1));
        if ordered.windows(2).any(|w| w[0].1 == w[1].1) {
            return Err(invalid("SOS weights must be distinct."));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (i, _) in &ordered {
            if *i >= n || !seen.insert(*i) {
                return Err(invalid("SOS variables must be unique original indices."));
            }
        }
        let count = if set.kind == SosKind::Sos1 { ordered.len() } else { ordered.len().saturating_sub(1).max(1) };
        let selectors = (0..count).map(|j| binary(&mut result.model.base, format!("{}:selector:{j}", set.name))).collect::<Vec<_>>();
        add_row(
            &mut result.model.base,
            format!("{}:selection", set.name),
            selectors.iter().map(|i| LinearTerm { variable: *i, coefficient: 1.0 }).collect(),
            1.0,
        );
        for (j, (i, _)) in ordered.iter().enumerate() {
            let (low, high) = effective_bounds(&result.model.base.variables[*i])?;
            let (Some(low), Some(high)) = (low, high) else {
                return Err(SolverError::new("finite_sos_bounds_required", "SOS linkage requires finite original variable bounds."));
            };
            let incident = if set.kind == SosKind::Sos1 || ordered.len() == 1 {
                vec![selectors[j]]
            } else {
                (j.saturating_sub(1)..=j.min(selectors.len() - 1)).map(|k| selectors[k]).collect::<Vec<_>>()
            };
            let mut upper = vec![LinearTerm { variable: *i, coefficient: 1.0 }];
            let mut lower = vec![LinearTerm { variable: *i, coefficient: -1.0 }];
            for selector in incident {
                upper.push(LinearTerm { variable: selector, coefficient: -high });
                lower.push(LinearTerm { variable: selector, coefficient: low });
            }
            add_row(&mut result.model.base, format!("{}:upper:{i}", set.name), upper, 0.0);
            add_row(&mut result.model.base, format!("{}:lower:{i}", set.name), lower, 0.0);
        }
        result.auxiliary_binaries += count;
        result.formulations.push(format!("{} uses {:?} exact bounded selector formulation, ordered by weights.", set.name, set.kind));
    }
    // Aggregate rows including repeated indicator-control coefficients and enforce
    // the shared model's finite/structural checks before entering any backend.
    result.model.base = canonicalize(&result.model.base, options)?;
    Ok(result)
}
pub fn solve_discrete_owned(input: &DiscreteConicModel, options: &SolveOptions, control: Arc<dyn RunControl>) -> Result<MixedConicResult> {
    let compiled = compile_discrete(input, options)?;
    let mut result = solve_mixed_conic_owned(&compiled.model, options, control)?;
    if let Some(values) = &mut result.result.values {
        values.truncate(compiled.original_variables);
    }
    if let Some(values) = &result.result.values {
        let (valid, violation) = validate_discrete_solution(input, values, options)?;
        if !valid {
            result.result.termination = Termination::NumericalFailure;
            result.result.feasibility = Feasibility::Unknown;
            result.result.optimality = Optimality::Unknown;
            result.result.values = None;
            result.result.objective = None;
            result.result.relative_gap = None;
            result.result.max_violation = Some(violation);
            result.result.warnings.push("Compiled incumbent failed independent original indicator/SOS validation.".into());
        }
    }
    result.result.warnings.extend(compiled.formulations);
    Ok(result)
}
pub fn validate_discrete_solution(input: &DiscreteConicModel, values: &[f64], options: &SolveOptions) -> Result<(bool, f64)> {
    let (valid, mut violation) = validate_conic_solution(&input.model, values, options)?;
    for indicator in &input.indicators {
        if indicator.binary >= input.model.base.variables.len()
            || input.model.base.variables[indicator.binary].kind != VariableKind::Binary
            || !indicator.constraint.rhs.is_finite()
            || indicator.constraint.terms.iter().any(|t| t.variable >= values.len() || !t.coefficient.is_finite())
        {
            return Err(invalid("Invalid original indicator definition."));
        }
        let b = *values.get(indicator.binary).ok_or_else(|| invalid("Invalid indicator control index."))?;
        if (b - f64::from(u8::from(indicator.active_value))).abs() <= options.integrality_tolerance {
            let delta = solvercraft_model::evaluate_linear_residual(&indicator.constraint.terms, values, indicator.constraint.rhs)?;
            let v = match indicator.constraint.relation {
                Relation::LessEqual => delta.max(0.0),
                Relation::GreaterEqual => (-delta).max(0.0),
                Relation::Equal => delta.abs(),
            };
            violation = violation.max(v);
        }
    }
    for set in &input.special_ordered_sets {
        if set.variables.is_empty()
            || set.variables.len() != set.weights.len()
            || set.weights.iter().any(|v| !v.is_finite())
            || set.variables.iter().any(|i| *i >= values.len())
            || set.variables.iter().collect::<std::collections::BTreeSet<_>>().len() != set.variables.len()
        {
            return Err(invalid("SOS definition requires unique valid indices and finite matching weights."));
        }
        let mut order = set.variables.iter().zip(&set.weights).collect::<Vec<_>>();
        order.sort_by(|a, b| a.1.total_cmp(b.1));
        if order.windows(2).any(|w| w[0].1 == w[1].1) {
            return Err(invalid("SOS weights must be distinct."));
        }
        let active = order
            .iter()
            .enumerate()
            .filter_map(|(j, (i, _))| values.get(**i).is_some_and(|v| v.abs() > options.feasibility_tolerance).then_some(j))
            .collect::<Vec<_>>();
        let allowed =
            if set.kind == SosKind::Sos1 { active.len() <= 1 } else { active.len() <= 2 && (active.len() < 2 || active[1] == active[0] + 1) };
        if !allowed {
            let excess = active.iter().filter_map(|j| values.get(*order[*j].0)).map(|v| v.abs()).fold(0.0, f64::max);
            violation = violation.max(excess);
        }
    }
    Ok((valid && violation <= options.feasibility_tolerance, violation))
}
