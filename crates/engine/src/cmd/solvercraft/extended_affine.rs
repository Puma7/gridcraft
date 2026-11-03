//! An opt-in, exact affine extended formulation for workbook expressions whose
//! coefficients cannot be stored as a single binary64 number.
use super::{
    Expr, ExpressionModel, Objective, OptimizationModel, Relation, Result, SCHEMA_VERSION, SOLVE, SolveOptions, Variable, VariableKind, bad,
    sdk_error,
};
use ::solvercraft::model::ExactProducts;
use ::solvercraft::types::{LinearConstraint, LinearTerm};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
struct Form {
    terms: BTreeMap<usize, f64>,
    constant: f64,
}
impl Form {
    fn variable(variable: usize, coefficient: f64) -> Self {
        Self { terms: BTreeMap::from([(variable, coefficient)]), constant: 0.0 }
    }
    fn terms(self) -> Vec<LinearTerm> {
        self.terms
            .into_iter()
            .filter(|(_, coefficient)| *coefficient != 0.0)
            .map(|(variable, coefficient)| LinearTerm { variable, coefficient })
            .collect()
    }
}

fn exact(value: f64, a: f64, b: f64, c: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let mut sum = ExactProducts::default();
    sum.add_product(a, b).ok()?;
    sum.add(c).ok()?;
    sum.add(-value).ok()?;
    (sum.exact_sign().ok()? == std::cmp::Ordering::Equal).then_some(value)
}

type FormKey = (Vec<(usize, u64)>, u64);
struct Lowering {
    model: OptimizationModel,
    definitions: BTreeMap<FormKey, usize>,
    aliases: BTreeMap<usize, usize>,
    nodes: usize,
    terms: usize,
    memory: u64,
}
impl Lowering {
    fn charge(&mut self, bytes: u64) -> Result<()> {
        self.memory = self.memory.checked_sub(bytes).ok_or_else(|| bad(SOLVE, "Affine formulation exceeds its memory budget"))?;
        Ok(())
    }
    fn push(&mut self, form: Form, relation: Relation, name: String) -> Result<()> {
        self.terms = self.terms.saturating_add(form.terms.len());
        if self.model.constraints.len() >= 4096 || self.terms > 100_000 {
            return Err(bad(SOLVE, "Affine formulation exceeds its row or term budget"));
        }
        self.charge(256 + (form.terms.len() as u64) * 128)?;
        let rhs = -form.constant;
        self.model.constraints.push(LinearConstraint { name, relation, rhs, terms: form.terms() });
        Ok(())
    }
    fn bind(&mut self, form: Form) -> Result<usize> {
        if form.constant == 0.0
            && let Some((&index, &1.0)) = form.terms.first_key_value().filter(|_| form.terms.len() == 1)
        {
            return Ok(index);
        }
        let key = (form.terms.iter().map(|(index, value)| (*index, value.to_bits())).collect(), form.constant.to_bits());
        if let Some(&index) = self.definitions.get(&key) {
            return Ok(index);
        }
        let index = self.auxiliary(self.endpoint(&form, false), self.endpoint(&form, true))?;
        let mut row = form;
        row.terms.insert(index, -1.0);
        self.push(row, Relation::Equal, format!("Affine definition {}", index + 1))?;
        self.definitions.insert(key, index);
        Ok(index)
    }
    fn endpoint(&self, form: &Form, upper: bool) -> Option<f64> {
        let mut value = ExactProducts::default();
        value.add(form.constant).ok()?;
        for (&index, &coefficient) in &form.terms {
            if coefficient == 0.0 {
                continue;
            }
            let variable = self.model.variables.get(index)?;
            let (low, high) = ::solvercraft::model::effective_bounds(variable).ok()?;
            let endpoint = if upper == (coefficient > 0.0) { high? } else { low? };
            value.add_product(coefficient, endpoint).ok()?;
        }
        if value.exact_sign().ok()? == std::cmp::Ordering::Equal {
            return Some(0.0);
        }
        outward(value.finish().ok()?, upper)
    }
    fn auxiliary(&mut self, lower: Option<f64>, upper: Option<f64>) -> Result<usize> {
        let index = self.model.variables.len();
        if index >= 1024 {
            return Err(bad(SOLVE, "Affine formulation exceeds the auxiliary variable budget"));
        }
        self.charge(256)?;
        self.model.variables.push(Variable { name: format!("Affine auxiliary {}", index + 1), lower, upper, kind: VariableKind::Continuous });
        Ok(index)
    }
    fn alias(&mut self, original: usize) -> Result<usize> {
        if let Some(&index) = self.aliases.get(&original) {
            return Ok(index);
        }
        let variable = self.model.variables.get(original).ok_or_else(|| bad(SOLVE, "Missing affine variable"))?;
        let (lower, upper) = ::solvercraft::model::effective_bounds(variable).map_err(sdk_error)?;
        let alias = self.auxiliary(lower, upper)?;
        let mut equality = Form::variable(original, 1.0);
        equality.terms.insert(alias, -1.0);
        self.push(equality, Relation::Equal, format!("Affine alias {}", alias + 1))?;
        self.aliases.insert(original, alias);
        Ok(alias)
    }
    fn add(&mut self, left: Form, right: Form, sign: f64) -> Result<Form> {
        let mut merged = left.clone();
        let offset = left.constant + sign * right.constant;
        let mut success = exact(offset, sign, right.constant, left.constant).is_some();
        if success {
            merged.constant = offset;
            for (&index, &coefficient) in &right.terms {
                let prior = merged.terms.get(&index).copied().unwrap_or(0.0);
                let value = prior + sign * coefficient;
                if exact(value, sign, coefficient, prior).is_none() {
                    success = false;
                    break;
                }
                if value == 0.0 {
                    merged.terms.remove(&index);
                } else {
                    merged.terms.insert(index, value);
                }
            }
        }
        if success {
            return Ok(merged);
        }
        let l = self.bind(left)?;
        let r = self.bind(right)?;
        let mut merged = Form::variable(l, 1.0);
        *merged.terms.entry(r).or_default() += sign;
        merged.terms.retain(|_, coefficient| *coefficient != 0.0);
        Ok(merged)
    }
    fn scale(&mut self, form: Form, factor: f64, divide: bool) -> Result<Form> {
        if divide && factor == 0.0 {
            return Err(bad(SOLVE, "Affine division requires a nonzero fixed denominator"));
        }
        if factor == 0.0 {
            return Ok(Form::default());
        }
        let transform = |v: f64| {
            if divide {
                let quotient = v / factor;
                exact(0.0, quotient, factor, -v).map(|_| quotient)
            } else {
                exact(v * factor, v, factor, 0.0)
            }
        };
        let terms = form.terms.iter().map(|(&index, &value)| Some((index, transform(value)?))).collect::<Option<BTreeMap<_, _>>>();
        if let Some((constant, terms)) = transform(form.constant).zip(terms) {
            return Ok(Form { constant, terms });
        }
        if !(1e-10..=1e10).contains(&factor.abs()) {
            return Err(bad(SOLVE, "Affine lifting requires a well-scaled fixed factor"));
        }
        if !divide {
            if form.constant == 0.0
                && let Some((&variable, &coefficient)) = form.terms.first_key_value().filter(|_| form.terms.len() == 1)
                && let Some((main, residual)) = split_product(coefficient, factor)
            {
                let mut scaled = Form::variable(variable, main);
                scaled.terms.insert(self.alias(variable)?, residual);
                return Ok(scaled);
            }
            return Ok(Form::variable(self.bind(form)?, factor));
        }
        let endpoint = |upper| {
            let numerator = self.endpoint(&form, upper == (factor > 0.0))?;
            if numerator == 0.0 { Some(0.0) } else { outward(numerator / factor, upper) }
        };
        let result = self.auxiliary(endpoint(false), endpoint(true))?;
        let mut equation = form;
        equation.terms.insert(result, -factor);
        self.push(equation, Relation::Equal, format!("Affine quotient {}", result + 1))?;
        Ok(Form::variable(result, 1.0))
    }
    fn scale_sum(&mut self, form: Form, factors: (f64, f64)) -> Result<Form> {
        self.charge(128 * form.terms.len() as u64)?;
        let left = self.scale(form.clone(), factors.0, false)?;
        let right = self.scale(form, factors.1, false)?;
        self.add(left, right, 1.0)
    }
    fn lower(&mut self, expression: &Expr, depth: usize) -> Result<Form> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > 100_000 || depth > 64 {
            return Err(bad(SOLVE, "Affine formulation exceeds its expression budget"));
        }
        let d = depth + 1;
        match expression {
            Expr::Constant { value } => Ok(Form { constant: *value, ..Form::default() }),
            Expr::Variable { index } => Ok(Form::variable(*index, 1.0)),
            Expr::Add { left, right } | Expr::Subtract { left, right } => {
                let l = self.lower(left, d)?;
                let r = self.lower(right, d)?;
                self.charge(64 * (l.terms.len().saturating_add(r.terms.len()) as u64))?;
                self.add(l, r, if matches!(expression, Expr::Add { .. }) { 1.0 } else { -1.0 })
            }
            Expr::Negate { value } => {
                let form = self.lower(value, d)?;
                self.scale(form, -1.0, false)
            }
            Expr::Multiply { left, right } if fixed_sum(left).is_some() || fixed_sum(right).is_some() => {
                let (variable, factors) = if let Some(factors) = fixed_sum(left) {
                    (right, factors)
                } else if let Some(factors) = fixed_sum(right) {
                    (left, factors)
                } else {
                    return Err(bad(SOLVE, "Missing fixed affine scale"));
                };
                let form = self.lower(variable, d)?;
                self.scale_sum(form, factors)
            }
            Expr::Multiply { left, right } | Expr::Divide { left, right } => {
                let l = self.lower(left, d)?;
                let r = self.lower(right, d)?;
                let divide = matches!(expression, Expr::Divide { .. });
                if r.terms.is_empty() {
                    self.scale(l, r.constant, divide)
                } else if !divide && l.terms.is_empty() {
                    self.scale(r, l.constant, false)
                } else {
                    Err(bad(SOLVE, "Affine lifting requires a fixed multiplier or denominator"))
                }
            }
            _ => Err(bad(SOLVE, "Expression is outside the exact affine formulation")),
        }
    }
}

fn outward(value: f64, upper: bool) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let value = if upper { value.next_up() } else { value.next_down() };
    value.is_finite().then_some(value)
}

/// Keep an exact product as two coefficients on equal, same-scale columns.
/// The equality avoids using a large intermediate variable just to multiply
/// it by a small rate. The exact check also covers subnormal FMA residuals.
fn split_product(a: f64, b: f64) -> Option<(f64, f64)> {
    let main = a * b;
    let residual = a.mul_add(b, -main);
    if !main.is_finite() || !residual.is_finite() || residual == 0.0 {
        return None;
    }
    let mut exact = ExactProducts::default();
    exact.add_product(a, b).ok()?;
    exact.add(-main).ok()?;
    exact.add(-residual).ok()?;
    (exact.exact_sign().ok()? == std::cmp::Ordering::Equal).then_some((main, residual))
}

fn fixed_sum(expr: &Expr) -> Option<(f64, f64)> {
    let (left, right, sign) = match expr {
        Expr::Add { left, right } => (left, right, 1.0),
        Expr::Subtract { left, right } => (left, right, -1.0),
        _ => return None,
    };
    match (left.as_ref(), right.as_ref()) {
        (Expr::Constant { value: a }, Expr::Constant { value: b }) => Some((*a, sign * b)),
        _ => None,
    }
}

pub(super) fn compile(source: &ExpressionModel, options: &SolveOptions) -> Result<OptimizationModel> {
    ::solvercraft::compile::validate_expression_model(source, options).map_err(sdk_error)?;
    let mut lower = Lowering {
        model: super::bounds_model(source),
        definitions: BTreeMap::new(),
        aliases: BTreeMap::new(),
        nodes: 0,
        terms: 0,
        memory: options.max_memory_bytes / 2,
    };
    let objective = lower.lower(&source.objective, 0)?;
    lower.model.objective = Objective { direction: source.direction, offset: objective.constant, linear: objective.terms(), quadratic: Vec::new() };
    for row in &source.constraints {
        let left = lower.lower(&row.left, 0)?;
        let right = lower.lower(&row.right, 0)?;
        let form = lower.add(left, right, -1.0)?;
        lower.push(form, row.relation, row.name.clone())?;
    }
    lower.model.schema_version = SCHEMA_VERSION;
    ::solvercraft::model::validate(&lower.model, options).map_err(sdk_error)?;
    Ok(lower.model)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use serde_json::json;

    #[test]
    fn affine_extension_keeps_exact_divisions_and_projects_to_original_cells() {
        let mut session = crate::Session::new();
        session.execute("file.new", json!({})).unwrap();
        for (cell, input) in [("A1", "0"), ("A2", "0"), ("B1", "=A1/3+A2*0.1*0.1")] {
            session.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        let mut request = json!({"objective":"B1","goal":"max","variables":"A1:A2","constraints":["A1<=6","A2<=10"]});
        assert!(session.execute("solvercraft.validate", request.clone()).is_err());
        request["extended_affine"] = json!(true);
        let translated = session.execute("solvercraft.validate", request.clone()).unwrap();
        assert_eq!(translated["class"], "extended_affine");
        let solved = session.execute("solvercraft.solve", request).unwrap();
        assert_eq!(solved["result"]["optimality"], "optimal");
        assert!((solved["result"]["objective"].as_f64().unwrap() - 2.1).abs() < 1e-12);
        assert_eq!(solved["result"]["values"], json!([6.0, 10.0]));
        let report = session.execute("solvercraft.report.answer", solved.clone()).unwrap();
        assert_eq!(report["variables"].as_array().unwrap().len(), 2);
        session.execute("solvercraft.result.apply", solved).unwrap();
        let wb = &session.doc().unwrap().wb;
        assert_eq!(wb.active().unwrap().value(gridcraft_core::CellRef::new(0, 0)).as_f64(), Some(6.0));
    }
}
