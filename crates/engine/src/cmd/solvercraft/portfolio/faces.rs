//! Exhaustive face certificate for small quadratic objectives on the simplex.
use super::super::sdk_error;
use super::Result;
use ::solvercraft::types::{
    Direction, Feasibility, LinearConstraint, Optimality, OptimizationModel, Relation, RunControl, SolveMethod, SolveOptions, SolveResult,
    SolveStatistics, Termination, VariableKind,
};

mod risk_cap;
pub(in crate::cmd::solvercraft) use risk_cap::{RiskCap, recognize_risk_cap};

#[derive(Clone, Copy)]
struct Interval {
    low: f64,
    high: f64,
}

impl Interval {
    const ZERO: Self = Self::point(0.0);

    const fn point(value: f64) -> Self {
        Self { low: value, high: value }
    }

    fn outward(low: f64, high: f64) -> Option<Self> {
        let (low, high) = (low.next_down(), high.next_up());
        (low.is_finite() && high.is_finite()).then_some(Self { low, high })
    }

    fn is_zero(self) -> bool {
        self.low == 0.0 && self.high == 0.0
    }

    fn neg(self) -> Self {
        Self { low: -self.high, high: -self.low }
    }

    fn add(self, other: Self) -> Option<Self> {
        if self.is_zero() {
            Some(other)
        } else if other.is_zero() {
            Some(self)
        } else {
            Self::outward(self.low + other.low, self.high + other.high)
        }
    }

    fn mul(self, other: Self) -> Option<Self> {
        if self.is_zero() || other.is_zero() {
            return Some(Self::ZERO);
        }
        let values = [self.low * other.low, self.low * other.high, self.high * other.low, self.high * other.high];
        Self::outward(values.into_iter().fold(f64::INFINITY, f64::min), values.into_iter().fold(f64::NEG_INFINITY, f64::max))
    }

    fn min_abs(self) -> f64 {
        if self.low > 0.0 { self.low } else { (-self.high).max(0.0) }
    }

    fn div(self, other: Self) -> Option<Self> {
        if other.min_abs() == 0.0 {
            return None;
        }
        self.mul(Self::outward(1.0 / other.high, 1.0 / other.low)?)
    }

    fn midpoint(self) -> f64 {
        self.low.midpoint(self.high)
    }
}

/// Verified elimination. A pivot interval containing zero is inconclusive.
fn enclose_linear_solution(mut rows: Vec<Vec<Interval>>) -> Option<Vec<Interval>> {
    let n = rows.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| rows[i][col].min_abs().total_cmp(&rows[j][col].min_abs()))?;
        if rows[pivot][col].min_abs() == 0.0 {
            return None;
        }
        rows.swap(col, pivot);
        let pivot_row = rows[col].clone();
        for row in &mut rows[col + 1..] {
            let factor = row[col].div(pivot_row[col])?;
            row[col] = Interval::ZERO;
            for j in col + 1..=n {
                row[j] = row[j].add(factor.mul(pivot_row[j])?.neg())?;
            }
        }
    }
    let mut values = vec![Interval::ZERO; n];
    for i in (0..n).rev() {
        let mut rhs = rows[i][n];
        for (j, value) in values.iter().enumerate().skip(i + 1) {
            rhs = rhs.add(value.mul(rows[i][j])?.neg())?;
        }
        values[i] = rhs.div(rows[i][i])?;
    }
    Some(values)
}

fn is_simplex_row(row: &LinearConstraint, n: usize) -> bool {
    row.relation == Relation::Equal && row.rhs == 1.0 && row.terms.len() == n && row.terms.iter().all(|term| term.coefficient == 1.0)
}

struct Faces {
    hessian: Vec<Vec<Interval>>,
    linear: Vec<f64>,
    inequality: Option<(Vec<f64>, f64)>,
}

impl Faces {
    fn new(model: &OptimizationModel) -> Option<Self> {
        let n = model.variables.len();
        let simplex = model.constraints.iter().position(|row| is_simplex_row(row, n))?;
        let inequality = model.constraints.iter().enumerate().find(|(i, _)| *i != simplex).map(|(_, row)| row);
        let inequality = match inequality {
            None => None,
            Some(row) => {
                let sign = match row.relation {
                    Relation::LessEqual => 1.0,
                    Relation::GreaterEqual => -1.0,
                    Relation::Equal => return None,
                };
                let mut dense = vec![0.0; n];
                for term in &row.terms {
                    *dense.get_mut(term.variable)? = term.coefficient * sign;
                }
                Some((dense, row.rhs * sign))
            }
        };
        let mut hessian = vec![vec![Interval::ZERO; n]; n];
        let mut linear = vec![0.0; n];
        for term in &model.objective.linear {
            *linear.get_mut(term.variable)? = term.coefficient;
        }
        for term in &model.objective.quadratic {
            let (i, j) = (term.left, term.right);
            let coefficient = Interval::point(term.coefficient);
            hessian[i][j] = hessian[i][j].add(coefficient)?;
            hessian[j][i] = hessian[j][i].add(coefficient)?;
        }
        Some(Self { hessian, linear, inequality })
    }

    fn stationary(&self, support: &[usize], active: bool) -> Option<Vec<Interval>> {
        let m = support.len();
        if m == 1 && !active {
            let mut values = vec![Interval::ZERO; self.linear.len()];
            values[support[0]] = Interval::point(1.0);
            return Some(values);
        }
        let n = m + 1 + usize::from(active);
        let mut matrix = vec![vec![Interval::ZERO; n + 1]; n];
        matrix[m][n] = Interval::point(1.0);
        for (i, &index) in support.iter().enumerate() {
            for (j, &other) in support.iter().enumerate() {
                matrix[i][j] = self.hessian[index][other];
            }
            matrix[i][m] = Interval::point(1.0);
            matrix[m][i] = Interval::point(1.0);
            matrix[i][n] = Interval::point(-self.linear[index]);
            if active {
                let (row, rhs) = self.inequality.as_ref()?;
                matrix[i][m + 1] = Interval::point(row[index]);
                matrix[m + 1][i] = Interval::point(row[index]);
                matrix[m + 1][n] = Interval::point(*rhs);
            }
        }
        let result = enclose_linear_solution(matrix)?;
        let mut values = vec![Interval::ZERO; self.linear.len()];
        for (&i, v) in support.iter().zip(result) {
            values[i] = v;
        }
        Some(values)
    }

    fn feasible(&self, values: &[Interval], active: bool) -> Option<(bool, bool)> {
        if values.iter().any(|value| value.high < 0.0 || value.low > 1.0) {
            return Some((false, false));
        }
        let nonnegative = values.iter().all(|value| value.low >= 0.0);
        if !active && let Some((row, rhs)) = &self.inequality {
            let sum =
                values.iter().zip(row).try_fold(Interval::ZERO, |sum, (value, coefficient)| sum.add(value.mul(Interval::point(*coefficient))?))?;
            return Some((sum.low <= *rhs, nonnegative && sum.high <= *rhs));
        }
        // The exact stationary point satisfies the budget equality and, on
        // an active face, the additional row equality by construction.
        Some((true, nonnegative))
    }
}

fn objective_enclosure(model: &OptimizationModel, values: &[Interval]) -> Option<Interval> {
    let mut sum = Interval::point(model.objective.offset);
    for term in &model.objective.linear {
        sum = sum.add(values[term.variable].mul(Interval::point(term.coefficient))?)?;
    }
    for term in &model.objective.quadratic {
        sum = sum.add(values[term.left].mul(values[term.right])?.mul(Interval::point(term.coefficient))?)?;
    }
    Some(sum)
}

struct Search {
    bound: f64,
    witness_upper: f64,
    unresolved: bool,
    best: Option<(Vec<f64>, f64, f64)>,
}

impl Search {
    fn observe(
        &mut self,
        model: &OptimizationModel,
        faces: &Faces,
        values: Option<Vec<Interval>>,
        active: bool,
        options: &SolveOptions,
    ) -> Result<()> {
        let Some(values) = values else {
            self.unresolved = true;
            return Ok(());
        };
        let proven_feasible = match faces.feasible(&values, active) {
            Some((false, _)) => return Ok(()),
            Some((true, proven)) => proven,
            None => {
                self.unresolved = true;
                false
            }
        };
        if let Some(enclosure) = objective_enclosure(model, &values) {
            self.bound = self.bound.min(enclosure.low);
            if proven_feasible {
                self.witness_upper = self.witness_upper.min(enclosure.high);
            }
        } else {
            self.unresolved = true;
        }
        let point = values.into_iter().map(|value| value.midpoint().clamp(0.0, 1.0)).collect::<Vec<_>>();
        let verification = ::solvercraft::model::validate_solution(model, &point, options).map_err(sdk_error)?;
        if verification.feasible {
            let value = ::solvercraft::model::evaluate_objective(model, &point).map_err(sdk_error)?;
            if self.best.as_ref().is_none_or(|(_, old, _)| value < *old) {
                self.best = Some((point, value, verification.max_violation));
            }
        }
        Ok(())
    }
}

/// At a global minimum the objective is stationary relative to the smallest
/// face containing the point. Enumerate every nonempty support and both states
/// of the optional affine limit. Singular unresolved faces prevent a proof.
pub(in crate::cmd::solvercraft) fn solve_simplex_quadratic(
    model: &OptimizationModel,
    options: &SolveOptions,
    control: &dyn RunControl,
) -> Result<Option<SolveResult>> {
    let n = model.variables.len();
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::Simplex)
        || !(1..=8).contains(&n)
        || !(1..=2).contains(&model.constraints.len())
        || model.objective.direction != Direction::Minimize
        || model.objective.quadratic.is_empty()
        || model.variables.iter().any(|v| v.kind != VariableKind::Continuous || v.lower != Some(0.0) || v.upper.is_some_and(|u| u < 1.0))
        || options.max_memory_bytes < 64 * 1024
    {
        return Ok(None);
    }
    let model = ::solvercraft::model::canonicalize(model, options).map_err(sdk_error)?;
    let Some(faces) = Faces::new(&model) else { return Ok(None) };
    let mut search = Search { bound: f64::INFINITY, witness_upper: f64::INFINITY, unresolved: false, best: None };
    let mut count = 0;
    let mut termination = Termination::Completed;
    'search: for mask in 1..(1 << n) {
        let support = (0..n).filter(|i| mask & (1 << i) != 0).collect::<Vec<_>>();
        for active in [false, true] {
            if active && faces.inequality.as_ref().is_none_or(|(row, _)| support.iter().all(|&i| row[i] == row[support[0]])) {
                // A constant row on this face either forbids it or adds no
                // independent equation. The inactive test handles both.
                continue;
            }
            if control.is_cancelled() {
                termination = Termination::Cancelled;
                break 'search;
            }
            if count >= options.max_evaluations {
                termination = Termination::EvaluationLimit;
                break 'search;
            }
            if count >= options.max_iterations {
                termination = Termination::IterationLimit;
                break 'search;
            }
            count += 1;
            search.observe(&model, &faces, faces.stationary(&support, active), active, options)?;
        }
    }
    let (values, objective, max_violation) = match search.best {
        Some((point, value, violation)) => (Some(point), Some(value), Some(violation)),
        None => (None, None, None),
    };
    let complete = termination == Termination::Completed && !search.unresolved;
    let bound = (complete && search.bound.is_finite()).then_some(search.bound);
    let gap = bound.zip(objective).map(|(lower, value)| (value.max(search.witness_upper) - lower).max(0.0)).filter(|gap| gap.is_finite());
    let optimality = match (objective, gap) {
        (None, _) => Optimality::Unknown,
        (_, Some(gap)) if gap.next_up() <= options.optimality_tolerance => Optimality::Optimal,
        _ => Optimality::BestFound,
    };
    Ok(Some(SolveResult {
        termination,
        feasibility: if values.is_some() { Feasibility::Feasible } else { Feasibility::Unknown },
        optimality,
        values,
        objective,
        best_bound: bound,
        relative_gap: gap.zip(objective).map(|(gap, value)| gap / value.abs().max(1.0)),
        max_violation,
        statistics: SolveStatistics { evaluations: count, iterations: count, nodes: 0 },
        engine: "solvercraft-bounded-quadratic-faces-0.1".into(),
        warnings: if search.unresolved { vec!["A singular or numerically unresolved face prevented a global certificate.".into()] } else { vec![] },
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::Session;
    use serde_json::json;

    #[test]
    fn indefinite_quadratic_minimum_is_certified_on_an_active_face() {
        let mut session = Session::new();
        session.execute("file.new", json!({})).unwrap();
        for (cell, input) in [("A1", "0"), ("A2", "0"), ("B1", "=-(A1^2)-(A2^2)")] {
            session.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        let request = json!({
            "objective":"B1","goal":"min","variables":"A1:A2",
            "constraints":["A1+A2=1","A1+2*A2>=1.25","A1>=0","A2>=0"]
        });
        let solved = session.execute("solvercraft.solve", request).unwrap();
        assert_eq!(solved["result"]["optimality"], "optimal");
        assert_eq!(solved["result"]["engine"], "solvercraft-bounded-quadratic-faces-0.1");
        assert!((solved["result"]["objective"].as_f64().unwrap() + 1.0).abs() < 1e-10);
        assert!(solved["result"]["best_bound"].as_f64().unwrap() <= -1.0);
        session.execute("solvercraft.result.apply", solved).unwrap();
    }
}
