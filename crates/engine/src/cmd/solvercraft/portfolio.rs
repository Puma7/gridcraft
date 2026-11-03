//! Structural spreadsheet bindings used by portfolio objectives.
use super::{EngineError, FormulaExpr, Result, SOLVE, Translator, Value, bad, expression_shape, parse, reference_sheet, sum};
use ::solvercraft::compile::Expr;

mod faces;
pub(super) use faces::{RiskCap, recognize_risk_cap, solve_simplex_quadratic};

impl Translator<'_> {
    pub(super) fn defined_range(&self, base: usize, name: &str) -> Result<(usize, FormulaExpr)> {
        let (sheet, bare) = gridcraft_core::split_sheet(name);
        let sheet =
            sheet.map(|sheet| self.wb.sheet_index(&sheet).ok_or_else(|| bad(SOLVE, "Unknown sheet in range name"))).transpose()?.unwrap_or(base);
        let definition = self.wb.name(bare, sheet).ok_or_else(|| bad(SOLVE, format!("Unknown range name: {name}")))?;
        let expression = parse(&definition.formula)?;
        if gridcraft_formula::adjust::has_relative_refs(&expression) {
            return Err(bad(SOLVE, "Named Solvercraft ranges must use absolute references"));
        }
        Ok((definition.scope.unwrap_or(sheet), expression))
    }

    pub(super) fn aggregate_shape(&mut self, base: usize, expression: &FormulaExpr, depth: usize) -> Result<(usize, usize)> {
        self.enter(depth)?;
        match expression {
            FormulaExpr::Name(name) => {
                let (scope, expression) = self.defined_range(base, name)?;
                self.aggregate_shape(scope, &expression, depth + 1)
            }
            FormulaExpr::Paren(inner) => self.aggregate_shape(base, inner, depth + 1),
            _ => expression_shape(expression),
        }
    }

    pub(super) fn quadratic_product(&mut self, base: usize, args: &[FormulaExpr], depth: usize) -> Result<Expr> {
        let [variables, linear, quadratic] = args else {
            return Err(bad(SOLVE, "QUADPRODUCT requires three arguments"));
        };
        let variables = self.expanded(base, variables, depth + 1)?;
        let n = variables.len();
        if n == 0 || n > 128 {
            return Err(bad(SOLVE, "QUADPRODUCT requires 1 to 128 entries"));
        }
        let linear = self.expanded(base, linear, depth + 1)?;
        let quadratic = self.expanded(base, quadratic, depth + 1)?;
        if linear.len() != n || quadratic.len() != n * n {
            return Err(bad(SOLVE, "QUADPRODUCT requires n linear and n² quadratic coefficients"));
        }
        let multiply = |left, right| Expr::Multiply { left: Box::new(left), right: Box::new(right) };
        let mut terms = Vec::with_capacity(n + n * n);
        for ((x, a), row) in variables.iter().zip(linear).zip(quadratic.chunks_exact(n)) {
            terms.push(multiply(x.clone(), a));
            for (y, coefficient) in variables.iter().zip(row) {
                terms.push(multiply(multiply(x.clone(), y.clone()), coefficient.clone()));
            }
        }
        Ok(sum(&terms))
    }

    /// Evaluate fixed samples through Calc only after checking all inputs.
    /// No volatile calls, references to formulas or decision-dependent values
    /// are admitted, even if the workbook currently has a numeric cache.
    pub(super) fn fixed_statistics(&mut self, base: usize, name: &str, args: &[FormulaExpr], depth: usize) -> Result<Expr> {
        let expression = FormulaExpr::Call(name.into(), args.to_vec());
        if !self.static_sample(base, &expression, depth + 1)? {
            return Err(bad(SOLVE, "Portfolio statistics require fixed numeric samples in optimization models"));
        }
        self.evaluate_fixed(base, &expression)
    }

    fn static_sample(&mut self, base: usize, expression: &FormulaExpr, depth: usize) -> Result<bool> {
        self.enter(depth)?;
        Ok(match expression {
            FormulaExpr::Number(value) => value.is_finite(),
            FormulaExpr::Bool(_) | FormulaExpr::Missing => true,
            FormulaExpr::Ref(reference) => {
                let sheet = reference_sheet(self.wb, base, reference)?;
                let range = reference.range();
                if range.count() > 1024 {
                    return Err(bad(SOLVE, "Portfolio sample exceeds 1024 cells"));
                }
                let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
                for cell in range.iter() {
                    self.enter(depth + 1)?;
                    if self.bindings.contains_key(&(sheet, cell))
                        || sh.spill_anchor(cell).is_some()
                        || !sh.cell(cell).is_some_and(|value| value.formula.is_none() && matches!(value.value, Value::Number(n) if n.is_finite()))
                    {
                        return Ok(false);
                    }
                }
                true
            }
            FormulaExpr::Paren(inner) => self.static_sample(base, inner, depth + 1)?,
            FormulaExpr::Call(name, args)
                if matches!(name.as_str(), "AVERAGE" | "VARP" | "VAR.P" | "VAR" | "VAR.S" | "COVAR" | "COVARIANCE.P" | "INDEX" | "LINEST") =>
            {
                if name == "LINEST"
                    && !args.iter().take(2).all(|arg| matches!(arg, FormulaExpr::Ref(r) if r.range().width() == 1 || r.range().height() == 1))
                {
                    return Ok(false);
                }
                for arg in args {
                    if !self.static_sample(base, arg, depth + 1)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::Session;
    use serde_json::json;

    #[test]
    fn portfolio_statistics_require_fixed_inputs_and_quadproduct_is_verified_on_apply() {
        let mut session = Session::new();
        session.execute("file.new", json!({})).unwrap();
        for (cell, input) in [
            ("A1", "0"),
            ("A2", "0"),
            ("C1", "1"),
            ("C2", "3"),
            ("D1", "0"),
            ("D2", "0"),
            ("E1", "=VARP(C1:C2)"),
            ("F1", "0"),
            ("E2", "0"),
            ("F2", "3"),
            ("B1", "=_xll.QUADPRODUCT(A1:A2,D1:D2,E1:F2)"),
        ] {
            session.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        let request = json!({"objective":"B1","goal":"min","variables":"A1:A2","constraints":["A1+A2=1"]});
        let solved = session.execute("solvercraft.solve", request.clone()).unwrap();
        assert_eq!(solved["result"]["optimality"], "optimal");
        assert!((solved["result"]["objective"].as_f64().unwrap() - 0.75).abs() < 1e-10);
        session.execute("solvercraft.result.apply", solved).unwrap();
        session.execute("cell.set", json!({"cell":"E1","input":"=VARP(A1:A2)"})).unwrap();
        let error = session.execute("solvercraft.solve", request).unwrap_err();
        assert!(error.to_string().contains("fixed numeric samples"), "{error}");
    }
}
