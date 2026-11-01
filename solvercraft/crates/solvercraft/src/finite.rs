//! Exact finite-domain, indicator and SOS reformulations using bounded MILP data.
use serde::{Deserialize, Serialize};
use solvercraft_model::{CompensatedSum, MAX_CONSTRAINTS, MAX_TERMS, MAX_VARIABLES};
use solvercraft_types::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiniteDomain {
    pub variable: usize,
    pub values: Vec<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Indicator {
    pub binary: usize,
    pub active_when: bool,
    pub constraint: LinearConstraint,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SosKind {
    One,
    Two,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialOrderedSet {
    pub kind: SosKind,
    /// Order defines adjacency for SOS2; variable handles must be unique.
    pub variables: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReifiedInequality {
    pub binary: usize,
    pub constraint: LinearConstraint,
    /// Complement is separated from the threshold by this positive amount.
    /// Thus <= b is paired with >= b+separation, not an invented strict inequality.
    pub separation: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FiniteConstraints {
    pub domains: Vec<FiniteDomain>,
    pub all_different: Vec<Vec<usize>>,
    pub indicators: Vec<Indicator>,
    pub sos: Vec<SpecialOrderedSet>,
    pub reified: Vec<ReifiedInequality>,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_finite_constraint", message)
}
fn allocation() -> SolverError {
    SolverError::new("allocation", "Cannot allocate finite-domain counterpart")
}
fn append<T>(values: &mut Vec<T>, n: usize) -> Result<()> {
    values.try_reserve_exact(n).map_err(|_| allocation())
}
fn checked_add(total: &mut usize, n: usize) -> Result<()> {
    *total = total.checked_add(n).ok_or_else(|| invalid("Counterpart count overflow"))?;
    Ok(())
}
fn row(name: &str, terms: Vec<LinearTerm>, relation: Relation, rhs: f64) -> LinearConstraint {
    LinearConstraint { name: name.into(), terms, relation, rhs }
}
fn term(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn binary(model: &OptimizationModel, handle: usize) -> Result<()> {
    if model.variables.get(handle).is_none_or(|v| v.kind != VariableKind::Binary) {
        return Err(invalid("Indicator handle must refer to a binary variable"));
    }
    Ok(())
}
fn finite_bounds(model: &OptimizationModel, handle: usize) -> Result<(f64, f64)> {
    let variable = model.variables.get(handle).ok_or_else(|| invalid("Invalid variable handle"))?;
    let (lo, hi) = solvercraft_model::effective_bounds(variable)?;
    lo.zip(hi).ok_or_else(|| invalid("Finite valid bounds are required for a reformulation"))
}
fn upper_add(left: f64, right: f64) -> Result<f64> {
    let value = left + right;
    if !value.is_finite() {
        return Err(SolverError::new("indicator_precision", "Outward indicator sum exceeds finite arithmetic"));
    }
    let virtual_right = value - left;
    let residual = (left - (value - virtual_right)) + (right - virtual_right);
    let upper = if residual > 0.0 { value.next_up() } else { value };
    if upper.is_finite() { Ok(upper) } else { Err(SolverError::new("indicator_precision", "Outward indicator sum is not representable")) }
}
fn upper_product(left: f64, right: f64) -> Result<f64> {
    let value = left * right;
    if !value.is_finite() {
        return Err(SolverError::new("indicator_precision", "Outward indicator product exceeds finite arithmetic"));
    }
    let residual = left.mul_add(right, -value);
    let upper = if residual > 0.0 || (left != 0.0 && right != 0.0 && value.abs() < f64::MIN_POSITIVE) { value.next_up() } else { value };
    if upper.is_finite() { Ok(upper) } else { Err(SolverError::new("indicator_precision", "Outward indicator product is not representable")) }
}
/// Maximum of a linear row over the declared variable box. Duplicated terms
/// remain a conservative bound, including when their coefficients cancel.
pub fn derived_big_m(model: &OptimizationModel, constraint: &LinearConstraint, negate: bool) -> Result<f64> {
    if !constraint.rhs.is_finite() || constraint.name.len() > solvercraft_model::MAX_NAME_BYTES || constraint.terms.len() > MAX_TERMS {
        return Err(invalid("Invalid indicator row"));
    }
    let sign = if negate { -1.0 } else { 1.0 };
    let mut maximum = -sign * constraint.rhs;
    for item in &constraint.terms {
        if !item.coefficient.is_finite() {
            return Err(invalid("Nonfinite indicator coefficient"));
        }
        if item.coefficient == 0.0 {
            if item.variable >= model.variables.len() {
                return Err(invalid("Invalid variable handle"));
            }
            continue;
        }
        let (lo, hi) = finite_bounds(model, item.variable)?;
        let coefficient = sign * item.coefficient;
        maximum = upper_add(maximum, upper_product(coefficient, if coefficient > 0.0 { hi } else { lo })?)?;
    }
    Ok(maximum.max(0.0))
}
fn indicator_plan(model: &OptimizationModel, indicator: &Indicator) -> Result<Vec<(f64, f64)>> {
    binary(model, indicator.binary)?;
    let sides: &[bool] = match indicator.constraint.relation {
        Relation::LessEqual => &[false],
        Relation::GreaterEqual => &[true],
        Relation::Equal => &[false, true],
    };
    let mut output = Vec::new();
    append(&mut output, sides.len())?;
    for negate in sides {
        let big_m = derived_big_m(model, &indicator.constraint, *negate)?;
        let sign = if *negate { -1.0 } else { 1.0 };
        output.push((sign, big_m));
    }
    Ok(output)
}
fn append_indicator(output: &mut OptimizationModel, original: &OptimizationModel, indicator: &Indicator) -> Result<()> {
    for (sign, big_m) in indicator_plan(original, indicator)? {
        let slack = output.variables.len();
        output.variables.push(Variable { name: "indicator slack".into(), lower: Some(0.0), upper: Some(big_m), kind: VariableKind::Continuous });
        let mut terms = Vec::new();
        append(&mut terms, indicator.constraint.terms.len() + 1)?;
        terms.extend(indicator.constraint.terms.iter().map(|t| term(t.variable, sign * t.coefficient)));
        terms.push(term(slack, -1.0));
        output.constraints.push(row("indicator original row", terms, Relation::LessEqual, sign * indicator.constraint.rhs));
        output.constraints.push(row(
            "indicator exact slack gate",
            vec![term(slack, 1.0), term(indicator.binary, if indicator.active_when { big_m } else { -big_m })],
            Relation::LessEqual,
            if indicator.active_when { big_m } else { 0.0 },
        ));
    }
    Ok(())
}
fn bitkey(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}
/// Compile exact one-hot finite domains, AllDifferent, SOS1/SOS2, bound-derived
/// indicators and explicitly separated inequality reification into a MILP.
pub fn compile(model: &OptimizationModel, specification: &FiniteConstraints, options: &SolveOptions) -> Result<OptimizationModel> {
    solvercraft_model::validate(model, options)?;
    if specification.domains.len() > MAX_VARIABLES
        || specification.all_different.len() > MAX_CONSTRAINTS
        || specification.indicators.len() > MAX_CONSTRAINTS
        || specification.sos.len() > MAX_CONSTRAINTS
        || specification.reified.len() > MAX_CONSTRAINTS
    {
        return Err(invalid("Too many finite constraint specifications"));
    }
    let mut variables = model.variables.len();
    let mut rows = model.constraints.len();
    let mut terms = model.objective.linear.len() + model.objective.quadratic.len();
    for constraint in &model.constraints {
        checked_add(&mut terms, constraint.terms.len())?;
    }
    for domain in &specification.domains {
        checked_add(&mut variables, domain.values.len())?;
        checked_add(&mut rows, 2)?;
        checked_add(&mut terms, domain.values.len().checked_mul(2).and_then(|n| n.checked_add(1)).ok_or_else(|| invalid("Domain overflow"))?)?;
    }
    let group_handles = specification
        .all_different
        .iter()
        .try_fold(0usize, |total, group| total.checked_add(group.len()))
        .ok_or_else(|| invalid("AllDifferent work overflow"))?;
    if group_handles.checked_mul(specification.domains.len().max(1)).is_none_or(|work| work > 10_000_000) {
        return Err(SolverError::new("model_limit", "AllDifferent domain lookup exceeds bounded work"));
    }
    for group in &specification.all_different {
        if group.len() > MAX_VARIABLES {
            return Err(invalid("AllDifferent group too large"));
        }
        // Each selector occurs at most once in a group; at most one row per selector.
        let mut selectors = 0;
        for handle in group {
            let domain = specification
                .domains
                .iter()
                .find(|d| d.variable == *handle)
                .ok_or_else(|| invalid("AllDifferent requires explicit finite domains"))?;
            checked_add(&mut selectors, domain.values.len())?;
            if selectors > MAX_TERMS {
                return Err(SolverError::new("model_limit", "AllDifferent exceeds bounded selectors"));
            }
        }
        checked_add(&mut rows, selectors)?;
        checked_add(&mut terms, selectors)?;
    }
    for indicator in &specification.indicators {
        let factor = if indicator.constraint.relation == Relation::Equal { 2 } else { 1 };
        checked_add(&mut variables, factor)?;
        checked_add(&mut rows, factor * 2)?;
        checked_add(
            &mut terms,
            indicator.constraint.terms.len().checked_add(3).and_then(|n| n.checked_mul(factor)).ok_or_else(|| invalid("Indicator overflow"))?,
        )?;
    }
    for reified in &specification.reified {
        if reified.constraint.relation == Relation::Equal || !reified.separation.is_finite() || reified.separation <= 0.0 {
            return Err(invalid("Reification requires an inequality and positive finite separation"));
        }
        checked_add(&mut variables, 2)?;
        checked_add(&mut rows, 4)?;
        checked_add(
            &mut terms,
            reified.constraint.terms.len().checked_add(3).and_then(|n| n.checked_mul(2)).ok_or_else(|| invalid("Reification overflow"))?,
        )?;
    }
    for sos in &specification.sos {
        let n = sos.variables.len();
        if n > MAX_VARIABLES {
            return Err(invalid("SOS too large"));
        }
        checked_add(&mut variables, n)?;
        checked_add(&mut rows, n.checked_mul(2).and_then(|x| x.checked_add(1)).ok_or_else(|| invalid("SOS overflow"))?)?;
        checked_add(&mut terms, n.checked_mul(7).ok_or_else(|| invalid("SOS term overflow"))?)?;
    }
    if variables > MAX_VARIABLES || rows > MAX_CONSTRAINTS || terms > MAX_TERMS {
        return Err(SolverError::new("model_limit", "Finite counterpart exceeds model dimensions"));
    }
    let names = model.name.len()
        + model.variables.iter().map(|v| v.name.len()).sum::<usize>()
        + model.constraints.iter().map(|c| c.name.len()).sum::<usize>();
    let bytes = terms
        .checked_mul(256)
        .and_then(|n| n.checked_add(variables.checked_mul(256)?))
        .and_then(|n| n.checked_add(rows.checked_mul(256)?))
        .and_then(|n| n.checked_add(names));
    if bytes.is_none_or(|n| n as u64 > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Finite counterpart exceeds budget before allocation"));
    }
    // Validate and derive every Big M before cloning the input.
    for indicator in &specification.indicators {
        indicator_plan(model, indicator)?;
    }
    let mut reifications = Vec::new();
    append(&mut reifications, specification.reified.len() * 2)?;
    for reified in &specification.reified {
        let mut complement = reified.constraint.clone();
        match complement.relation {
            Relation::LessEqual => {
                complement.relation = Relation::GreaterEqual;
                complement.rhs += reified.separation;
            }
            Relation::GreaterEqual => {
                complement.relation = Relation::LessEqual;
                complement.rhs -= reified.separation;
            }
            Relation::Equal => return Err(invalid("Equality reification requires an explicit disjunction")),
        }
        if !complement.rhs.is_finite() || complement.rhs == reified.constraint.rhs {
            return Err(invalid("Reification separation is not representable at the threshold"));
        }
        for indicator in [
            Indicator { binary: reified.binary, active_when: true, constraint: reified.constraint.clone() },
            Indicator { binary: reified.binary, active_when: false, constraint: complement },
        ] {
            indicator_plan(model, &indicator)?;
            reifications.push(indicator);
        }
    }
    let mut seen = BTreeSet::new();
    for domain in &specification.domains {
        let variable = model.variables.get(domain.variable).ok_or_else(|| invalid("Invalid domain handle"))?;
        if !seen.insert(domain.variable) || domain.values.is_empty() {
            return Err(invalid("Domains must be nonempty and unique per variable"));
        }
        let (lo, hi) = solvercraft_model::effective_bounds(variable)?;
        let mut values = BTreeSet::new();
        for value in &domain.values {
            if !value.is_finite()
                || lo.is_some_and(|x| *value < x)
                || hi.is_some_and(|x| *value > x)
                || (variable.kind != VariableKind::Continuous && value.fract() != 0.0)
                || !values.insert(bitkey(*value))
            {
                return Err(invalid("Finite domain contains invalid, repeated or out-of-bound values"));
            }
        }
    }
    for group in &specification.all_different {
        let mut seen = BTreeSet::new();
        if group.iter().any(|v| !seen.insert(*v)) {
            return Err(invalid("AllDifferent repeats a variable"));
        }
        let mut ordered = Vec::new();
        for handle in group {
            let domain =
                specification.domains.iter().find(|d| d.variable == *handle).ok_or_else(|| invalid("AllDifferent requires explicit domains"))?;
            append(&mut ordered, domain.values.len())?;
            ordered.extend_from_slice(&domain.values);
        }
        ordered.sort_unstable_by(f64::total_cmp);
        for adjacent in ordered.windows(2) {
            if adjacent[0] != adjacent[1] && (adjacent[1] - adjacent[0]).abs() <= 2.0 * options.feasibility_tolerance {
                return Err(SolverError::new(
                    "unsupported_finite_precision",
                    "Distinct AllDifferent values must be separated by more than twice the feasibility tolerance",
                ));
            }
        }
    }
    for sos in &specification.sos {
        let mut seen = BTreeSet::new();
        for handle in &sos.variables {
            finite_bounds(model, *handle)?;
            if !seen.insert(*handle) {
                return Err(invalid("SOS repeats a variable"));
            }
        }
    }
    let mut output = model.clone();
    append(&mut output.variables, variables - model.variables.len())?;
    append(&mut output.constraints, rows - model.constraints.len())?;
    let mut domain_selectors: BTreeMap<usize, Vec<(u64, usize)>> = BTreeMap::new();
    for domain in &specification.domains {
        let mut selectors = Vec::new();
        append(&mut selectors, domain.values.len())?;
        let mut one = Vec::new();
        append(&mut one, domain.values.len())?;
        let mut link = Vec::new();
        append(&mut link, domain.values.len() + 1)?;
        link.push(term(domain.variable, 1.0));
        for value in &domain.values {
            let selector = output.variables.len();
            output.variables.push(Variable { name: "domain selector".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Binary });
            one.push(term(selector, 1.0));
            link.push(term(selector, -value));
            selectors.push((bitkey(*value), selector));
        }
        output.constraints.push(row("choose finite value", one, Relation::Equal, 1.0));
        output.constraints.push(row("finite value link", link, Relation::Equal, 0.0));
        domain_selectors.insert(domain.variable, selectors);
    }
    for group in &specification.all_different {
        let mut values: BTreeMap<u64, Vec<LinearTerm>> = BTreeMap::new();
        for handle in group {
            for (value, selector) in domain_selectors.get(handle).ok_or_else(|| invalid("Missing finite domain"))? {
                values.entry(*value).or_default().push(term(*selector, 1.0));
            }
        }
        for (_, selectors) in values {
            if selectors.len() > 1 {
                output.constraints.push(row("AllDifferent", selectors, Relation::LessEqual, 1.0));
            }
        }
    }
    for indicator in specification.indicators.iter().chain(&reifications) {
        append_indicator(&mut output, model, indicator)?;
    }
    for sos in &specification.sos {
        let n = sos.variables.len();
        if n <= 1 || (matches!(sos.kind, SosKind::Two) && n <= 2) {
            continue;
        }
        let selector_count = if matches!(sos.kind, SosKind::One) { n } else { n - 1 };
        let start = output.variables.len();
        for _ in 0..selector_count {
            output.variables.push(Variable { name: "SOS selector".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Binary });
        }
        output.constraints.push(row("SOS selection", (0..selector_count).map(|i| term(start + i, 1.0)).collect(), Relation::LessEqual, 1.0));
        for (i, handle) in sos.variables.iter().enumerate() {
            let (lo, hi) = finite_bounds(model, *handle)?;
            let adjacent: Vec<usize> = match sos.kind {
                SosKind::One => vec![start + i],
                SosKind::Two => (i.saturating_sub(1)..=i.min(n - 2)).map(|j| start + j).collect(),
            };
            let mut upper = vec![term(*handle, 1.0)];
            let mut lower = vec![term(*handle, -1.0)];
            upper.extend(adjacent.iter().map(|j| term(*j, -hi)));
            lower.extend(adjacent.iter().map(|j| term(*j, lo)));
            output.constraints.push(row("SOS upper", upper, Relation::LessEqual, 0.0));
            output.constraints.push(row("SOS lower", lower, Relation::LessEqual, 0.0));
        }
    }
    solvercraft_model::validate(&output, options)?;
    Ok(output)
}
fn row_holds(constraint: &LinearConstraint, values: &[f64], tolerance: f64) -> Result<bool> {
    let mut sum = CompensatedSum::default();
    sum.add(-constraint.rhs)?;
    for item in &constraint.terms {
        let value = values.get(item.variable).ok_or_else(|| invalid("Missing original variable"))?;
        sum.add(item.coefficient * value)?;
    }
    let residual = sum.finish()?;
    Ok(match constraint.relation {
        Relation::LessEqual => residual <= tolerance,
        Relation::GreaterEqual => residual >= -tolerance,
        Relation::Equal => residual.abs() <= tolerance,
    })
}
/// Verify the original discrete and logical semantics, independently of the
/// Big-M/selector rows, before returning an incumbent to a caller.
pub fn solve(model: &OptimizationModel, specification: &FiniteConstraints, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    let compiled = compile(model, specification, options)?;
    let mut solved = crate::solve(&compiled, options, control)?;
    if let Some(values) = &solved.values {
        let original = values.get(..model.variables.len()).ok_or_else(|| invalid("Missing original values"))?;
        if !solvercraft_model::validate_solution(model, original, options)?.feasible {
            return Err(SolverError::new("numerical_failure", "Finite incumbent failed original model"));
        }
        for domain in &specification.domains {
            if !domain.values.iter().any(|v| (original[domain.variable] - v).abs() <= options.feasibility_tolerance) {
                return Err(SolverError::new("numerical_failure", "Finite incumbent failed original domain"));
            }
        }
        for group in &specification.all_different {
            let mut values: BTreeSet<u64> = BTreeSet::new();
            for handle in group {
                let domain = specification.domains.iter().find(|d| d.variable == *handle).ok_or_else(|| invalid("Missing finite domain"))?;
                let nearest = domain
                    .values
                    .iter()
                    .min_by(|a, b| (original[*handle] - **a).abs().total_cmp(&(original[*handle] - **b).abs()))
                    .ok_or_else(|| invalid("Empty domain"))?;
                if !values.insert(bitkey(*nearest)) {
                    return Err(SolverError::new("numerical_failure", "Finite incumbent failed AllDifferent"));
                }
            }
        }
        for indicator in &specification.indicators {
            if (original[indicator.binary] >= 0.5) == indicator.active_when
                && !row_holds(&indicator.constraint, original, options.feasibility_tolerance)?
            {
                return Err(SolverError::new("numerical_failure", "Finite incumbent failed original indicator"));
            }
        }
        for reified in &specification.reified {
            let active = original[reified.binary] >= 0.5;
            let mut complement = reified.constraint.clone();
            match complement.relation {
                Relation::LessEqual => {
                    complement.relation = Relation::GreaterEqual;
                    complement.rhs += reified.separation;
                }
                Relation::GreaterEqual => {
                    complement.relation = Relation::LessEqual;
                    complement.rhs -= reified.separation;
                }
                Relation::Equal => return Err(invalid("Unsupported equality reification")),
            }
            if !row_holds(if active { &reified.constraint } else { &complement }, original, options.feasibility_tolerance)? {
                return Err(SolverError::new("numerical_failure", "Finite incumbent failed separated reification"));
            }
        }
        for sos in &specification.sos {
            let nonzero: Vec<_> =
                sos.variables.iter().enumerate().filter(|(_, v)| original[**v].abs() > options.feasibility_tolerance).map(|(i, _)| i).collect();
            let valid = match sos.kind {
                SosKind::One => nonzero.len() <= 1,
                SosKind::Two => nonzero.len() <= 1 || (nonzero.len() == 2 && nonzero[1] == nonzero[0] + 1),
            };
            if !valid {
                return Err(SolverError::new("numerical_failure", "Finite incumbent failed original SOS"));
            }
        }
        let objective = solvercraft_model::evaluate_objective(model, original)?;
        let original = original.to_vec();
        solved.values = Some(original);
        solved.objective = Some(objective);
    }
    Ok(solved)
}
