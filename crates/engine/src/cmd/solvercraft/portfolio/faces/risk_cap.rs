//! A linear return over a simplex and one positive definite variance limit.
//! Polynomial coefficient intervals enclose the exact workbook expression.
use super::{Interval, enclose_linear_solution};
use ::solvercraft::compile::{Expr, ExpressionModel};
use ::solvercraft::types::{
    Direction, Feasibility, Optimality, Relation, RunControl, SolveMethod, SolveOptions, SolveResult, SolveStatistics, Termination, VariableKind,
};
use std::collections::BTreeMap;

const CONST: usize = usize::MAX;
type Polynomial = BTreeMap<[usize; 2], Interval>;

fn insert(out: &mut Polynomial, key: [usize; 2], value: Interval) -> Option<()> {
    if !value.is_zero() {
        let previous = out.get(&key).copied().unwrap_or(Interval::ZERO);
        out.insert(key, previous.add(value)?);
    }
    Some(())
}

fn combine(mut left: Polynomial, right: Polynomial, negate: bool) -> Option<Polynomial> {
    for (key, value) in right {
        insert(&mut left, key, if negate { value.neg() } else { value })?;
    }
    Some(left)
}

fn multiply(left: &Polynomial, right: &Polynomial) -> Option<Polynomial> {
    let mut out = Polynomial::new();
    for (&[a, b], &x) in left {
        for (&[c, d], &y) in right {
            let mut key = [a, b, c, d];
            key.sort_unstable();
            if key[2] != CONST {
                return None;
            }
            insert(&mut out, [key[0], key[1]], x.mul(y)?)?;
        }
    }
    Some(out)
}

fn polynomial(expression: &Expr, depth: usize, budget: &mut usize) -> Option<Polynomial> {
    *budget = budget.checked_sub(1)?;
    if depth > 64 {
        return None;
    }
    let d = depth + 1;
    match expression {
        Expr::Constant { value } if value.is_finite() => {
            Some(if *value == 0.0 { Polynomial::new() } else { Polynomial::from([([CONST, CONST], Interval::point(*value))]) })
        }
        Expr::Variable { index } if *index < 8 => Some(Polynomial::from([([*index, CONST], Interval::point(1.0))])),
        Expr::Add { left, right } | Expr::Subtract { left, right } => {
            combine(polynomial(left, d, budget)?, polynomial(right, d, budget)?, matches!(expression, Expr::Subtract { .. }))
        }
        Expr::Multiply { left, right } => multiply(&polynomial(left, d, budget)?, &polynomial(right, d, budget)?),
        Expr::Negate { value } => Some(polynomial(value, d, budget)?.into_iter().map(|(key, value)| (key, value.neg())).collect()),
        Expr::Power { base, exponent: 2 } => {
            let base = polynomial(base, d, budget)?;
            multiply(&base, &base)
        }
        _ => None,
    }
}

fn is_point(interval: Interval, value: f64) -> bool {
    interval.low == value && interval.high == value
}

fn is_budget_row(row: &Polynomial, n: usize) -> bool {
    row.len() == n + 1
        && row.get(&[CONST, CONST]).is_some_and(|v| is_point(*v, -1.0))
        && (0..n).all(|i| row.get(&[i, CONST]).is_some_and(|v| is_point(*v, 1.0)))
}

fn positive_definite(mut matrix: Vec<Vec<Interval>>) -> Option<()> {
    for col in 0..matrix.len() {
        let pivot = matrix[col][col];
        if pivot.low <= 0.0 {
            return None;
        }
        let row = matrix[col].clone();
        for other in &mut matrix[col + 1..] {
            let factor = other[col].div(pivot)?;
            for j in col + 1..row.len() {
                other[j] = other[j].add(factor.mul(row[j])?.neg())?;
            }
        }
    }
    Some(())
}

pub(in crate::cmd::solvercraft) struct RiskCap {
    objective: Polynomial,
    variance: Polynomial,
    cap: Interval,
    hessian: Vec<Vec<Interval>>,
    returns: Vec<Interval>,
}

pub(in crate::cmd::solvercraft) fn recognize_risk_cap(model: &ExpressionModel, options: &SolveOptions) -> Option<RiskCap> {
    let n = model.variables.len();
    if !(2..=8).contains(&n)
        || model.constraints.len() != 2
        || model.direction != Direction::Maximize
        || !matches!(options.method, SolveMethod::Auto | SolveMethod::ProjectedGradient)
        || options.max_memory_bytes < 256 * 1024
        || model.variables.iter().any(|v| v.kind != VariableKind::Continuous || v.lower != Some(0.0) || v.upper.is_some_and(|u| u < 1.0))
    {
        return None;
    }
    let mut budget = 4096;
    let objective = polynomial(&model.objective, 0, &mut budget)?;
    let mut returns = vec![Interval::ZERO; n];
    for (&[i, j], &value) in &objective {
        if j != CONST {
            return None;
        }
        if i != CONST {
            *returns.get_mut(i)? = value;
        }
    }
    let mut variance = None;
    let mut has_budget = false;
    for row in &model.constraints {
        let lhs = polynomial(&row.left, 0, &mut budget)?;
        let rhs = polynomial(&row.right, 0, &mut budget)?;
        let difference = if row.relation == Relation::GreaterEqual { combine(rhs, lhs, true)? } else { combine(lhs, rhs, true)? };
        match row.relation {
            Relation::Equal => has_budget = is_budget_row(&difference, n),
            _ => variance = Some(difference),
        }
    }
    if !has_budget {
        return None;
    }
    let mut variance = variance?;
    let cap = variance.remove(&[CONST, CONST])?.neg();
    if cap.low <= 0.0 {
        return None;
    }
    let mut hessian = vec![vec![Interval::ZERO; n]; n];
    for (&[i, j], &value) in &variance {
        if j == CONST || i >= n || j >= n {
            return None;
        }
        hessian[i][j] = hessian[i][j].add(value)?;
        hessian[j][i] = hessian[j][i].add(value)?;
    }
    positive_definite(hessian.clone())?;
    Some(RiskCap { objective, variance, cap, hessian, returns })
}

fn evaluate(polynomial: &Polynomial, values: &[Interval]) -> Option<Interval> {
    polynomial.iter().try_fold(Interval::ZERO, |sum, (&[i, j], &coefficient)| {
        let term = if i == CONST {
            coefficient
        } else {
            let term = coefficient.mul(*values.get(i)?)?;
            if j == CONST { term } else { term.mul(*values.get(j)?)? }
        };
        sum.add(term)
    })
}

fn embed(support: &[usize], values: Vec<Interval>, n: usize) -> Vec<Interval> {
    let mut point = vec![Interval::ZERO; n];
    for (&i, value) in support.iter().zip(values) {
        point[i] = value;
    }
    point
}

impl RiskCap {
    fn stationary(&self, support: &[usize], direction: bool) -> Option<Vec<Interval>> {
        let n = support.len();
        let mut matrix = vec![vec![Interval::ZERO; n + 2]; n + 1];
        if !direction {
            matrix[n][n + 1] = Interval::point(1.0);
        }
        for (i, &index) in support.iter().enumerate() {
            for (j, &other) in support.iter().enumerate() {
                matrix[i][j] = self.hessian[index][other];
            }
            matrix[i][n] = Interval::point(1.0);
            matrix[n][i] = Interval::point(1.0);
            if direction {
                matrix[i][n + 1] = self.returns[index];
            }
        }
        Some(embed(support, enclose_linear_solution(matrix)?, self.returns.len()))
    }

    /// The minimum-variance center is orthogonal (in the Q inner product)
    /// to the budget-preserving return direction. The variance allowance
    /// therefore fixes the maximum displacement along that direction.
    fn face(&self, support: &[usize]) -> Option<Option<Vec<Interval>>> {
        let center =
            if support.len() == 1 { embed(support, vec![Interval::point(1.0)], self.returns.len()) } else { self.stationary(support, false)? };
        let room = self.cap.add(evaluate(&self.variance, &center)?.neg())?;
        if room.high < 0.0 {
            return Some(None);
        }
        if room.low < 0.0 {
            return None;
        }
        if support.len() == 1 {
            return Some(Some(center));
        }
        let direction = self.stationary(support, true)?;
        let norm = evaluate(&self.variance, &direction)?;
        if norm.low <= 0.0 {
            return None;
        }
        let t = room.div(norm)?;
        let factor = Interval::outward(t.low.max(0.0).sqrt(), t.high.sqrt())?;
        let values = center.into_iter().zip(direction).map(|(x, d)| x.add(factor.mul(d)?)).collect::<Option<Vec<_>>>()?;
        Some(Some(values))
    }
}

fn original_candidate(model: &ExpressionModel, point: &[f64], options: &SolveOptions) -> Option<(f64, f64)> {
    let mut max_violation = 0.0_f64;
    for row in &model.constraints {
        let a = ::solvercraft::compile::evaluate(&row.left, point).ok()?;
        let b = ::solvercraft::compile::evaluate(&row.right, point).ok()?;
        let violation = match row.relation {
            Relation::Equal => (a - b).abs(),
            Relation::LessEqual => a - b,
            Relation::GreaterEqual => b - a,
        };
        max_violation = max_violation.max(violation);
    }
    let value = ::solvercraft::compile::evaluate(&model.objective, point).ok()?;
    (max_violation <= options.feasibility_tolerance).then_some((value, max_violation))
}

struct Maximum {
    upper: f64,
    witness: f64,
    unresolved: bool,
    candidate: Option<(Vec<f64>, f64, f64)>,
}

impl Maximum {
    fn observe(&mut self, model: &ExpressionModel, route: &RiskCap, options: &SolveOptions, support: &[usize]) {
        let values = match route.face(support) {
            Some(Some(values)) => values,
            Some(None) => return,
            None => {
                self.unresolved = true;
                return;
            }
        };
        if values.iter().any(|v| v.high < 0.0 || v.low > 1.0) {
            // An optimum in the relative interior must also be an optimum
            // on its affine hull. A negative affine maximizer forces the
            // constrained optimum onto a smaller face.
            return;
        }
        let Some(value) = evaluate(&route.objective, &values) else {
            self.unresolved = true;
            return;
        };
        self.upper = self.upper.max(value.high);
        if values.iter().all(|v| v.low >= 0.0) {
            self.witness = self.witness.max(value.low);
        }
        let point = values.into_iter().map(|v| v.midpoint().clamp(0.0, 1.0)).collect::<Vec<_>>();
        if let Some((value, violation)) = original_candidate(model, &point, options)
            && self.candidate.as_ref().is_none_or(|(_, best, _)| value > *best)
        {
            self.candidate = Some((point, value, violation));
        }
    }
}

impl RiskCap {
    pub(in crate::cmd::solvercraft) fn solve(&self, model: &ExpressionModel, options: &SolveOptions, control: &dyn RunControl) -> SolveResult {
        let n = self.returns.len();
        let mut search = Maximum { upper: f64::NEG_INFINITY, witness: f64::NEG_INFINITY, unresolved: false, candidate: None };
        let mut termination = Termination::Completed;
        let mut count = 0;
        for mask in 1..(1 << n) {
            if control.is_cancelled() {
                termination = Termination::Cancelled;
                break;
            }
            if count >= options.max_evaluations {
                termination = Termination::EvaluationLimit;
                break;
            }
            if count >= options.max_iterations {
                termination = Termination::IterationLimit;
                break;
            }
            let support = (0..n).filter(|i| mask & (1 << i) != 0).collect::<Vec<_>>();
            search.observe(model, self, options, &support);
            count += 1;
        }
        let (values, objective, max_violation) = match search.candidate {
            Some((point, value, violation)) => (Some(point), Some(value), Some(violation)),
            None => (None, None, None),
        };
        let complete = termination == Termination::Completed && !search.unresolved;
        let best_bound = (complete && search.upper.is_finite()).then_some(search.upper);
        let gap = best_bound.zip(objective).map(|(bound, value)| (bound - value.min(search.witness)).max(0.0)).filter(|gap| gap.is_finite());
        let optimality = match (objective, gap) {
            (None, _) => Optimality::Unknown,
            (_, Some(gap)) if gap.next_up() <= options.optimality_tolerance => Optimality::Optimal,
            _ => Optimality::BestFound,
        };
        SolveResult {
            termination,
            feasibility: if values.is_some() { Feasibility::Feasible } else { Feasibility::Unknown },
            optimality,
            values,
            objective,
            best_bound,
            relative_gap: gap.zip(objective).map(|(gap, value)| gap / value.abs().max(1.0)),
            max_violation,
            statistics: SolveStatistics { evaluations: count, iterations: count, nodes: 0 },
            engine: "solvercraft-bounded-risk-cap-faces-0.1".into(),
            warnings: if search.unresolved { vec!["An unresolved face prevented a global risk-cap certificate.".into()] } else { vec![] },
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::Session;
    use serde_json::json;

    #[test]
    fn risk_cap_uses_the_analytic_budget_face_maximum() {
        let mut session = Session::new();
        session.execute("file.new", json!({})).unwrap();
        for (cell, input) in [("A1", "0"), ("A2", "0"), ("B1", "=A1"), ("B2", "=A1^2+A2^2")] {
            session.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        let request = json!({"objective":"B1","goal":"max","variables":"A1:A2","constraints":["A1+A2=1","B2<=0.625"]});
        let solved = session.execute("solvercraft.solve", request).unwrap();
        assert_eq!(solved["result"]["optimality"], "optimal");
        assert_eq!(solved["result"]["engine"], "solvercraft-bounded-risk-cap-faces-0.1");
        assert!((solved["result"]["objective"].as_f64().unwrap() - 0.75).abs() < 1e-10);
        assert!(solved["result"]["best_bound"].as_f64().unwrap() >= 0.75);
        session.execute("solvercraft.result.apply", solved).unwrap();
    }
}
