//! Certified separable minimization for positive quantities.
//!
//! The admitted objective is a sum of a*x + b/x, with positive, exactly
//! represented coefficients. The domain excludes zero because b/x is undefined
//! there. Resource rows must be affine, nonnegative, and upper bounded.
use super::{Direction, Expr, ExpressionModel, Feasibility, Relation, SolveMethod, SolveOptions, SolveResult, VariableKind};
use ::solvercraft::model::{ExactProducts, effective_bounds};
use ::solvercraft::types::{LinearConstraint, Optimality, RunControl, SolveStatistics, Termination};

const ENGINE: &str = "solvercraft-positive-reciprocal-1";
const MAX_VARIABLES: usize = 12;
const MAX_ROWS: usize = 4;
const MAX_QUANTITY: f64 = 4096.0;

#[derive(Clone, Copy, Default)]
struct Cost {
    linear: f64,
    reciprocal: f64,
}

pub(super) struct LotSizing {
    costs: Vec<Cost>,
    rows: Vec<LinearConstraint>,
    domains: Option<Vec<(u32, u32)>>,
}

fn multiply(a: f64, b: f64) -> Option<f64> {
    let mut product = ExactProducts::default();
    product.add_product(a, b).ok()?;
    let value = a * b;
    product.add(-value).ok()?;
    product.exact_sign().ok()?.is_eq().then_some(value)
}

fn divide(a: f64, b: f64) -> Option<f64> {
    let value = a / b;
    (multiply(value, b)? == a).then_some(value)
}

fn constant(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Constant { value } => Some(*value),
        Expr::Multiply { left, right } => multiply(constant(left)?, constant(right)?),
        Expr::Divide { left, right } => divide(constant(left)?, constant(right)?),
        _ => None,
    }
}

fn linear(expr: &Expr) -> Option<(usize, f64)> {
    match expr {
        Expr::Variable { index } => Some((*index, 1.0)),
        Expr::Multiply { left, right } => {
            let ((index, coefficient), multiplier) = linear(left).zip(constant(right)).or_else(|| linear(right).zip(constant(left)))?;
            Some((index, multiply(coefficient, multiplier)?))
        }
        Expr::Divide { left, right } => {
            let (index, coefficient) = linear(left)?;
            Some((index, divide(coefficient, constant(right)?)?))
        }
        _ => None,
    }
}

fn collect_costs(expr: &Expr, costs: &mut [Cost]) -> Option<()> {
    if let Expr::Add { left, right } = expr {
        collect_costs(left, costs)?;
        return collect_costs(right, costs);
    }
    let (coefficient, slot) = match expr {
        Expr::Divide { left, right } if matches!(right.as_ref(), Expr::Variable { .. }) => {
            let Expr::Variable { index } = right.as_ref() else { return None };
            (constant(left)?, &mut costs.get_mut(*index)?.reciprocal)
        }
        _ => {
            let (index, value) = linear(expr)?;
            (value, &mut costs.get_mut(index)?.linear)
        }
    };
    if *slot != 0.0 || !coefficient.is_finite() || coefficient <= 0.0 {
        return None;
    }
    *slot = coefficient;
    Some(())
}

fn integer_domains(expr: &ExpressionModel, rows: &[LinearConstraint], options: &SolveOptions) -> Option<Vec<(u32, u32)>> {
    let mut domains = Vec::with_capacity(expr.variables.len());
    let mut points = 0_u64;
    for (index, variable) in expr.variables.iter().enumerate() {
        let (lower, upper) = effective_bounds(variable).ok()?;
        let lower = lower?.ceil().max(1.0);
        let mut upper = upper.unwrap_or(f64::INFINITY);
        for row in rows {
            if let Some(term) = row.terms.iter().find(|t| t.variable == index && t.coefficient > 0.0) {
                // Dropping all other nonnegative usages gives a safe finite
                // upper limit. Round outward before restricting to integers.
                upper = upper.min((row.rhs / term.coefficient).next_up());
            }
        }
        let upper = upper.floor();
        if !upper.is_finite() || upper > MAX_QUANTITY || upper < lower {
            return None;
        }
        let (lower, upper) = (lower as u32, upper as u32);
        points += u64::from(upper - lower + 1);
        domains.push((lower, upper));
    }
    (points * 128 + 16_384 <= options.max_memory_bytes).then_some(domains)
}

pub(super) fn recognize(expr: &ExpressionModel, options: &SolveOptions) -> Option<LotSizing> {
    if expr.direction != Direction::Minimize
        || !matches!(options.method, SolveMethod::Auto | SolveMethod::BranchAndBound)
        || !(1..=MAX_VARIABLES).contains(&expr.variables.len())
        || !(1..=MAX_ROWS).contains(&expr.constraints.len())
    {
        return None;
    }
    ::solvercraft::compile::validate_expression_model(expr, options).ok()?;
    let mut costs = vec![Cost::default(); expr.variables.len()];
    collect_costs(&expr.objective, &mut costs)?;
    if costs.iter().any(|cost| cost.linear == 0.0 || cost.reciprocal == 0.0) {
        return None;
    }
    let affine = ExpressionModel {
        name: expr.name.clone(),
        variables: expr.variables.clone(),
        direction: expr.direction,
        objective: Expr::constant(0.0),
        constraints: expr.constraints.clone(),
    };
    let rows = ::solvercraft::compile::compile(&affine, options).ok()?.constraints;
    if rows.iter().any(|row| {
        row.relation != Relation::LessEqual || row.rhs <= 0.0 || row.terms.is_empty() || row.terms.iter().any(|term| term.coefficient < 0.0)
    }) {
        return None;
    }
    let domains = if expr.variables.iter().all(|v| v.kind == VariableKind::Integer && v.lower.is_some_and(|lo| lo >= 0.0)) {
        Some(integer_domains(expr, &rows, options)?)
    } else if options.method == SolveMethod::Auto
        && rows.len() == 1
        && options.max_memory_bytes >= 16_384
        && expr.variables.iter().all(|v| v.kind == VariableKind::Continuous && v.lower == Some(0.0) && v.upper.is_none())
    {
        None
    } else {
        return None;
    };
    Some(LotSizing { costs, rows, domains })
}

#[derive(Clone, Copy, Default)]
struct Interval {
    lo: f64,
    hi: f64,
}
impl Interval {
    fn plus(self, other: Self) -> Self {
        Self { lo: (self.lo + other.lo).next_down(), hi: (self.hi + other.hi).next_up() }
    }

    fn product(a: f64, b: f64) -> Self {
        Self { lo: (a * b).next_down(), hi: (a * b).next_up() }
    }
}

impl Cost {
    fn at(self, quantity: f64) -> Interval {
        let reciprocal = self.reciprocal / quantity;
        Interval::product(self.linear, quantity).plus(Interval { lo: reciprocal.next_down(), hi: reciprocal.next_up() })
    }
}

fn feasible(row: &LinearConstraint, point: &[f64]) -> Option<bool> {
    let mut residual = ExactProducts::default();
    residual.add(-row.rhs).ok()?;
    for term in &row.terms {
        residual.add_product(term.coefficient, *point.get(term.variable)?).ok()?;
    }
    Some(!residual.exact_sign().ok()?.is_gt())
}

struct Search<'a> {
    options: &'a SolveOptions,
    control: &'a dyn RunControl,
    statistics: SolveStatistics,
    termination: Termination,
}

impl Search<'_> {
    fn active(&mut self) -> bool {
        if self.termination != Termination::Completed {
            return false;
        }
        if self.control.is_cancelled() {
            self.termination = Termination::Cancelled;
            return false;
        }
        true
    }

    fn evaluation(&mut self) -> bool {
        if !self.active() {
            return false;
        }
        if self.statistics.evaluations >= self.options.max_evaluations {
            self.termination = Termination::EvaluationLimit;
            return false;
        }
        self.statistics.evaluations += 1;
        true
    }

    fn iteration(&mut self) -> bool {
        if !self.active() {
            return false;
        }
        if self.statistics.iterations >= self.options.max_iterations {
            self.termination = Termination::IterationLimit;
            return false;
        }
        self.statistics.iterations += 1;
        true
    }

    fn node(&mut self) -> bool {
        if !self.active() {
            return false;
        }
        if self.statistics.nodes >= self.options.max_nodes {
            self.termination = Termination::NodeLimit;
            return false;
        }
        self.statistics.nodes += 1;
        self.iteration()
    }
}

fn weights(row: &LinearConstraint, n: usize) -> Vec<f64> {
    let mut values = vec![0.0; n];
    for term in &row.terms {
        values[term.variable] = term.coefficient;
    }
    values
}

fn multiplier_point(costs: &[Cost], weights: &[f64], multiplier: f64) -> Option<Vec<f64>> {
    costs
        .iter()
        .zip(weights)
        .map(|(cost, weight)| {
            let linear = multiplier.mul_add(*weight, cost.linear);
            let x = (cost.reciprocal / linear).sqrt();
            (x.is_finite() && x > 0.0).then_some(x)
        })
        .collect()
}

fn evaluate_multiplier(
    costs: &[Cost],
    weights: &[f64],
    row: &LinearConstraint,
    multiplier: f64,
    search: &mut Search<'_>,
) -> Option<(Vec<f64>, bool)> {
    if !search.iteration() || !search.evaluation() {
        return None;
    }
    let point = multiplier_point(costs, weights, multiplier)?;
    let fits = feasible(row, &point)?;
    Some((point, fits))
}

fn find_multiplier(costs: &[Cost], weights: &[f64], row: &LinearConstraint, search: &mut Search<'_>) -> Option<(f64, Vec<f64>)> {
    let (point, fits) = evaluate_multiplier(costs, weights, row, 0.0, search)?;
    if fits {
        return Some((0.0, point));
    }
    let mut low = 0.0;
    let mut high = 1.0;
    let mut best = loop {
        let (point, fits) = evaluate_multiplier(costs, weights, row, high, search)?;
        if fits {
            break point;
        }
        low = high;
        high *= 2.0;
        if !high.is_finite() {
            return None;
        }
    };
    for _ in 0..80 {
        let middle = f64::midpoint(low, high);
        if middle == low || middle == high {
            break;
        }
        let Some((point, fits)) = evaluate_multiplier(costs, weights, row, middle, search) else { break };
        if fits {
            high = middle;
            best = point;
        } else {
            low = middle;
        }
    }
    Some((high, best))
}

// For every x > 0, (a + multiplier*w)*x + b/x is at least
// 2*sqrt((a + multiplier*w)*b). Outward rounding gives a dual lower bound
// independent of whether the bisection found the best multiplier.
fn continuous_lower(costs: &[Cost], weights: &[f64], rhs: f64, multiplier: f64) -> f64 {
    let mut sum = -Interval::product(multiplier, rhs).hi;
    for (cost, weight) in costs.iter().zip(weights) {
        let linear = (cost.linear + Interval::product(multiplier, *weight).lo).next_down().max(0.0);
        let product = Interval::product(linear, cost.reciprocal).lo.max(0.0);
        sum = (sum + (2.0 * product.sqrt().next_down()).next_down()).next_down();
    }
    sum
}

struct Outcome {
    values: Option<Vec<f64>>,
    lower: Option<f64>,
    upper: Option<f64>,
    infeasible: bool,
}
impl Outcome {
    fn unknown() -> Self {
        Self { values: None, lower: None, upper: None, infeasible: false }
    }
}

fn continuous(model: &LotSizing, search: &mut Search<'_>) -> Outcome {
    let Some(row) = model.rows.first() else { return Outcome::unknown() };
    let weights = weights(row, model.costs.len());
    let Some((multiplier, point)) = find_multiplier(&model.costs, &weights, row, search) else { return Outcome::unknown() };
    let lower = continuous_lower(&model.costs, &weights, row.rhs, multiplier);
    let upper = model.costs.iter().zip(&point).fold(Interval::default(), |sum, (cost, x)| sum.plus(cost.at(*x))).hi;
    Outcome { values: Some(point), lower: Some(lower), upper: Some(upper), infeasible: false }
}

#[derive(Clone, Copy)]
struct Point {
    quantity: f64,
    cost: Interval,
    shifted_lower: f64,
}

struct IntegerSearch<'a> {
    rows: &'a [LinearConstraint],
    domains: Vec<Vec<Point>>,
    weights: Vec<Vec<f64>>,
    suffix: Vec<f64>,
    multiplier: f64,
    dual_row: usize,
    best: Option<(Vec<f64>, f64)>,
    best_lower: f64,
}

impl IntegerSearch<'_> {
    fn visit(&mut self, index: usize, costs: Interval, resources: [Interval; MAX_ROWS], path: &mut [f64], search: &mut Search<'_>) {
        if index == self.domains.len() {
            self.candidate(costs, path, search);
            return;
        }
        let row = &self.rows[self.dual_row];
        let remaining = (row.rhs - resources[self.dual_row].lo).next_up();
        let penalty = Interval::product(self.multiplier, remaining).hi;
        for i in 0..self.domains[index].len() {
            let point = self.domains[index][i];
            let bound = (((costs.lo + point.shifted_lower).next_down() + self.suffix[index + 1]).next_down() - penalty).next_down();
            if self.best.as_ref().is_some_and(|(_, incumbent)| bound >= *incumbent) {
                break;
            }
            if !search.node() {
                return;
            }
            let mut next = resources;
            for (k, usage) in next.iter_mut().enumerate().take(self.rows.len()) {
                *usage = usage.plus(Interval::product(self.weights[k][index], point.quantity));
            }
            if next.iter().zip(self.rows).any(|(usage, row)| usage.lo > row.rhs) {
                continue;
            }
            path[index] = point.quantity;
            self.visit(index + 1, costs.plus(point.cost), next, path, search);
            if !search.active() {
                return;
            }
        }
    }

    fn candidate(&mut self, costs: Interval, path: &[f64], search: &mut Search<'_>) {
        if !search.evaluation() {
            return;
        }
        for row in self.rows {
            match feasible(row, path) {
                Some(true) => {}
                Some(false) => return,
                None => {
                    search.termination = Termination::NumericalFailure;
                    return;
                }
            }
        }
        self.best_lower = self.best_lower.min(costs.lo);
        if self.best.as_ref().is_none_or(|(_, incumbent)| costs.hi < *incumbent) {
            self.best = Some((path.to_vec(), costs.hi));
        }
    }
}

fn choose_dual(model: &LotSizing, weights: &[Vec<f64>], search: &mut Search<'_>) -> (usize, f64) {
    let mut dual = (0, 0.0);
    let mut lower = f64::NEG_INFINITY;
    for (index, (row, weights)) in model.rows.iter().zip(weights).enumerate() {
        let multiplier = find_multiplier(&model.costs, weights, row, search).map_or(0.0, |(multiplier, _)| multiplier);
        let candidate = continuous_lower(&model.costs, weights, row.rhs, multiplier);
        if candidate > lower {
            dual = (index, multiplier);
            lower = candidate;
        }
    }
    dual
}

fn integer(model: &LotSizing, domains: &[(u32, u32)], search: &mut Search<'_>) -> Outcome {
    let weights = model.rows.iter().map(|row| weights(row, model.costs.len())).collect::<Vec<_>>();
    let (dual_row, multiplier) = choose_dual(model, &weights, search);
    let mut points = Vec::with_capacity(domains.len());
    for (index, (lo, hi)) in domains.iter().enumerate() {
        let mut domain = Vec::with_capacity((hi - lo + 1) as usize);
        for value in *lo..=*hi {
            if !search.evaluation() {
                return Outcome::unknown();
            }
            let quantity = f64::from(value);
            let cost = model.costs[index].at(quantity);
            let weighted_quantity = Interval::product(weights[dual_row][index], quantity).lo;
            let shift = Interval::product(multiplier, weighted_quantity).lo;
            let shifted_lower = (cost.lo + shift).next_down();
            if !shifted_lower.is_finite() || !cost.hi.is_finite() {
                return Outcome::unknown();
            }
            domain.push(Point { quantity, cost, shifted_lower });
        }
        domain.sort_by(|a, b| a.shifted_lower.total_cmp(&b.shifted_lower));
        points.push(domain);
    }
    let mut suffix = vec![0.0; domains.len() + 1];
    for (index, domain) in points.iter().enumerate().rev() {
        let Some(first) = domain.first() else { return Outcome::unknown() };
        suffix[index] = (suffix[index + 1] + first.shifted_lower).next_down();
    }
    let initial_lower = (suffix[0] - Interval::product(multiplier, model.rows[dual_row].rhs).hi).next_down();
    let mut state =
        IntegerSearch { rows: &model.rows, domains: points, weights, suffix, multiplier, dual_row, best: None, best_lower: f64::INFINITY };
    state.visit(0, Interval::default(), [Interval::default(); MAX_ROWS], &mut vec![0.0; domains.len()], search);
    let completed = search.termination == Termination::Completed;
    let lower = if completed { state.best_lower } else { initial_lower };
    let (values, upper) = state.best.map_or((None, None), |(point, cost)| (Some(point), Some(cost)));
    Outcome { infeasible: completed && values.is_none(), values, upper, lower: lower.is_finite().then_some(lower) }
}

impl LotSizing {
    pub(super) fn solve(&self, expr: &ExpressionModel, options: &SolveOptions, control: &dyn RunControl) -> SolveResult {
        let mut search = Search { options, control, statistics: SolveStatistics::default(), termination: Termination::Completed };
        let mut result = match &self.domains {
            Some(domains) => integer(self, domains, &mut search),
            None => continuous(self, &mut search),
        };
        let objective = result.values.as_ref().and_then(|point| ::solvercraft::compile::evaluate(&expr.objective, point).ok());
        if objective.is_none() {
            result.values = None;
            if search.termination == Termination::Completed && !result.infeasible {
                search.termination = Termination::NumericalFailure;
            }
        }
        let optimal = search.termination == Termination::Completed
            && result.lower.zip(result.upper).zip(objective).is_some_and(|((lo, hi), objective)| {
                lo.is_finite() && hi.is_finite() && (hi.max(objective) - lo.min(objective)).next_up() <= options.optimality_tolerance
            });
        let bound = result.lower.filter(|x| x.is_finite()).map(|lo| objective.map_or(lo, |value| lo.min(value)));
        SolveResult {
            termination: search.termination,
            feasibility: if objective.is_some() {
                Feasibility::Feasible
            } else if result.infeasible {
                Feasibility::Infeasible
            } else {
                Feasibility::Unknown
            },
            optimality: if optimal {
                Optimality::Optimal
            } else if objective.is_some() {
                Optimality::BestFound
            } else {
                Optimality::Unknown
            },
            values: result.values,
            objective,
            best_bound: bound,
            relative_gap: objective.zip(bound).map(|(value, bound)| ((value - bound) / value.abs().max(1.0)).max(0.0)),
            max_violation: objective.map(|_| 0.0),
            statistics: search.statistics,
            engine: ENGINE.into(),
            warnings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use ::solvercraft::compile::ExpressionConstraint;
    use ::solvercraft::types::{NoControl, Variable};

    fn add(left: Expr, right: Expr) -> Expr {
        Expr::Add { left: Box::new(left), right: Box::new(right) }
    }

    fn product(coefficient: f64, index: usize) -> Expr {
        Expr::Multiply { left: Box::new(Expr::constant(coefficient)), right: Box::new(Expr::variable(index)) }
    }

    fn cost(a: f64, b: f64, index: usize) -> Expr {
        add(product(a, index), Expr::Divide { left: Box::new(Expr::constant(b)), right: Box::new(Expr::variable(index)) })
    }

    fn row(left: Expr, right: f64) -> ExpressionConstraint {
        ExpressionConstraint { name: "capacity".into(), left, relation: Relation::LessEqual, right: Expr::constant(right) }
    }

    fn variable(kind: VariableKind) -> Variable {
        Variable { name: "quantity".into(), kind, lower: Some(0.0), upper: None }
    }

    #[test]
    fn continuous_reciprocal_minimum_has_a_feasible_point_and_a_dual_bound() {
        let expression = ExpressionModel {
            name: "continuous quantity".into(),
            variables: vec![variable(VariableKind::Continuous)],
            direction: Direction::Minimize,
            objective: cost(2.0, 8.0, 0),
            constraints: vec![row(Expr::variable(0), 1.5)],
        };
        let options = SolveOptions::default();
        let route = recognize(&expression, &options).unwrap();
        let solved = route.solve(&expression, &options, &NoControl);
        assert_eq!(solved.optimality, Optimality::Optimal);
        assert!((solved.values.unwrap()[0] - 1.5).abs() < 1e-12);
        let expected = 3.0 + 8.0 / 1.5;
        assert!(solved.best_bound.unwrap() <= expected);
        assert!((solved.objective.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn integer_reciprocal_minimum_respects_both_resources_and_the_node_budget() {
        let expression = ExpressionModel {
            name: "integer quantities".into(),
            variables: vec![variable(VariableKind::Integer), variable(VariableKind::Integer)],
            direction: Direction::Minimize,
            objective: add(cost(1.0, 9.0, 0), cost(2.0, 32.0, 1)),
            constraints: vec![row(add(Expr::variable(0), Expr::variable(1)), 5.0), row(add(product(2.0, 0), Expr::variable(1)), 7.0)],
        };
        let options = SolveOptions::default();
        let route = recognize(&expression, &options).unwrap();
        let solved = route.solve(&expression, &options, &NoControl);
        assert_eq!(solved.optimality, Optimality::Optimal);
        assert_eq!(solved.values.unwrap(), vec![2.0, 3.0]);
        assert!(solved.best_bound.unwrap() <= 12.5 + 32.0 / 3.0);
        let stopped = route.solve(&expression, &SolveOptions { max_nodes: 1, ..options }, &NoControl);
        assert_eq!(stopped.termination, Termination::NodeLimit);
        assert_ne!(stopped.optimality, Optimality::Optimal);
    }

    #[test]
    fn negative_quantities_and_explicit_other_methods_are_outside_the_route() {
        let mut expression = ExpressionModel {
            name: "unsupported domain".into(),
            variables: vec![variable(VariableKind::Continuous)],
            direction: Direction::Minimize,
            objective: cost(2.0, 8.0, 0),
            constraints: vec![row(Expr::variable(0), 1.5)],
        };
        let options = SolveOptions { method: SolveMethod::ProjectedGradient, ..Default::default() };
        assert!(recognize(&expression, &options).is_none());
        expression.variables[0].lower = Some(-1.0);
        assert!(recognize(&expression, &SolveOptions::default()).is_none());
    }
}
