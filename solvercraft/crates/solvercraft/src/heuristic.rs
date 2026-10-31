//! Deterministic bounded differential evolution for expression models.
//! This is a heuristic: no local/global optimality or infeasibility certificate is claimed.
use crate::{compile, math, model, types};
use compile::{Expr, ExpressionModel};
use std::cmp::Ordering;
use types::{
    Direction, Feasibility, Optimality, ProgressEvent, Relation, Result, RunControl, SolveMethod, SolveOptions, SolveResult, SolveStatistics,
    SolverError, Termination, VariableKind,
};

#[derive(Clone, Copy)]
struct Domain {
    low: f64,
    high: f64,
    integer: bool,
}
#[derive(Clone, Copy)]
struct Score {
    objective: f64,
    violation: f64,
    feasible: bool,
}
struct Member {
    values: Vec<f64>,
    score: Option<Score>,
}
fn error(code: &str, message: impl Into<String>) -> SolverError {
    SolverError::new(code, message)
}
fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<()> {
    values.try_reserve_exact(count).map_err(|_| error("allocation", "Cannot allocate differential-evolution storage."))
}
fn copy_values(values: &[f64]) -> Result<Vec<f64>> {
    let mut copy = Vec::new();
    reserve(&mut copy, values.len())?;
    copy.extend_from_slice(values);
    Ok(copy)
}
fn compare(a: Score, b: Score, direction: Direction) -> Ordering {
    match (a.feasible, b.feasible) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.violation.total_cmp(&b.violation).then_with(|| objective_order(a.objective, b.objective, direction)),
        (true, true) => objective_order(a.objective, b.objective, direction),
    }
}
fn objective_order(a: f64, b: f64, direction: Direction) -> Ordering {
    match direction {
        Direction::Minimize => a.total_cmp(&b),
        Direction::Maximize => b.total_cmp(&a),
    }
}
fn best_index(population: &[Member], direction: Direction) -> Option<usize> {
    population.iter().enumerate().filter_map(|(i, m)| m.score.map(|s| (i, s))).min_by(|(_, a), (_, b)| compare(*a, *b, direction)).map(|(i, _)| i)
}
fn check_expression(expr: &Expr, variables: usize, remaining: &mut usize) -> Result<()> {
    let mut stack = Vec::new();
    reserve(&mut stack, 128)?;
    stack.push((expr, 1usize));
    while let Some((node, depth)) = stack.pop() {
        if depth > compile::MAX_EXPRESSION_DEPTH || *remaining == 0 {
            return Err(error("expression_limit", "Expression model exceeds its depth or total node limit."));
        }
        *remaining -= 1;
        match node {
            Expr::Constant { value } if !value.is_finite() => return Err(error("non_finite", "Expression constants must be finite.")),
            Expr::Variable { index } if *index >= variables => return Err(error("invalid_variable", "Expression references an absent variable.")),
            Expr::Add { left, right }
            | Expr::Subtract { left, right }
            | Expr::Multiply { left, right }
            | Expr::Divide { left, right }
            | Expr::Minimum { left, right }
            | Expr::Maximum { left, right } => {
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
            Expr::Negate { value }
            | Expr::Sin { value }
            | Expr::Cos { value }
            | Expr::Exp { value }
            | Expr::Ln { value }
            | Expr::Sqrt { value }
            | Expr::Abs { value } => stack.push((value, depth + 1)),
            Expr::Power { base, .. } => stack.push((base, depth + 1)),
            _ => {}
        }
    }
    Ok(())
}
fn validate(input: &ExpressionModel, options: &SolveOptions) -> Result<(Vec<Domain>, usize)> {
    model::validate_options(options)?;
    if !matches!(options.method, SolveMethod::Auto | SolveMethod::DifferentialEvolution) {
        return Err(error("unsupported_method", "This expression engine implements differential_evolution only."));
    }
    if input.variables.len() > model::MAX_VARIABLES || input.constraints.len() > model::MAX_CONSTRAINTS {
        return Err(error("model_limit", "Expression model exceeds variable or constraint limits."));
    }
    if input.name.len() > model::MAX_NAME_BYTES {
        return Err(error("model_limit", "Model name is too long."));
    }
    let population_size = if input.variables.is_empty() { 1 } else { input.variables.len().saturating_mul(10).clamp(24, 128) };
    let estimated = (input.variables.len() as u64)
        .checked_mul((population_size as u64 + 5) * 8)
        .and_then(|v| v.checked_add(population_size as u64 * std::mem::size_of::<Member>() as u64))
        .and_then(|v| v.checked_add(input.variables.len() as u64 * std::mem::size_of::<Domain>() as u64))
        .and_then(|v| v.checked_add(4096))
        .ok_or_else(|| error("memory_limit", "Differential-evolution size overflow."))?;
    if estimated > options.max_memory_bytes {
        return Err(error("memory_limit", "Population exceeds its memory budget."));
    }
    let mut domains = Vec::new();
    reserve(&mut domains, input.variables.len())?;
    for variable in &input.variables {
        if variable.name.len() > model::MAX_NAME_BYTES {
            return Err(error("model_limit", "Variable name is too long."));
        }
        let (lower, upper) = model::effective_bounds(variable)?;
        let (Some(mut low), Some(mut high)) = (lower, upper) else {
            return Err(error("finite_bounds_required", "Differential evolution requires finite lower and upper bounds for every variable."));
        };
        let integer = variable.kind != VariableKind::Continuous;
        if integer {
            low = low.ceil();
            high = high.floor();
        }
        if !(high - low).is_finite() {
            return Err(error("numeric_range", "Variable interval is too wide; rescale the model."));
        }
        domains.push(Domain { low, high, integer });
    }
    let mut remaining = compile::MAX_EXPRESSION_NODES;
    check_expression(&input.objective, input.variables.len(), &mut remaining)?;
    for constraint in &input.constraints {
        if constraint.name.len() > model::MAX_NAME_BYTES {
            return Err(error("model_limit", "Constraint name is too long."));
        }
        check_expression(&constraint.left, input.variables.len(), &mut remaining)?;
        check_expression(&constraint.right, input.variables.len(), &mut remaining)?;
    }
    Ok((domains, population_size))
}
fn repair(value: f64, domain: Domain) -> f64 {
    let value = value.clamp(domain.low, domain.high);
    if domain.integer { value.round().clamp(domain.low, domain.high) } else { value }
}
fn initial(domains: &[Domain], seed: u64, index: usize) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    reserve(&mut values, domains.len())?;
    for (binding, domain) in domains.iter().enumerate() {
        let p = match index {
            0 => 0.5,
            1 => 0.0,
            2 => 1.0,
            _ => math::addressed_uniform(seed, binding as u64, index as u64, 0x4445),
        };
        values.push(repair(domain.low + (domain.high - domain.low) * p, *domain));
    }
    Ok(values)
}
struct Evaluation<'a> {
    input: &'a ExpressionModel,
    options: &'a SolveOptions,
    domains: &'a [Domain],
    control: &'a dyn RunControl,
    evaluations: u64,
    invalid: u64,
    cancelled: bool,
}
impl Evaluation<'_> {
    fn raw(&self, values: &[f64]) -> Result<Score> {
        let objective = compile::evaluate(&self.input.objective, values)?;
        let mut violation = 0.0_f64;
        let mut feasible = true;
        for (value, domain) in values.iter().zip(self.domains) {
            let amount = (domain.low - value).max(value - domain.high).max(0.0);
            violation = violation.max(amount);
            feasible &= amount <= self.options.feasibility_tolerance;
            if domain.integer {
                let integer = (value - value.round()).abs();
                violation = violation.max(integer);
                feasible &= integer <= self.options.integrality_tolerance;
            }
        }
        for constraint in &self.input.constraints {
            if self.control.is_cancelled() {
                return Err(error("cancelled", "User cancelled expression evaluation."));
            }
            let left = compile::evaluate(&constraint.left, values)?;
            let right = compile::evaluate(&constraint.right, values)?;
            let difference = left - right;
            if !difference.is_finite() {
                return Err(error("numeric_error", "Constraint difference exceeds finite arithmetic."));
            }
            let amount = match constraint.relation {
                Relation::LessEqual => difference.max(0.0),
                Relation::Equal => difference.abs(),
                Relation::GreaterEqual => (-difference).max(0.0),
            };
            violation = violation.max(amount);
            feasible &= amount <= self.options.feasibility_tolerance;
        }
        Ok(Score { objective, violation, feasible })
    }
    fn score(&mut self, values: &[f64]) -> Result<Option<Score>> {
        if self.evaluations >= self.options.max_evaluations {
            return Err(error("evaluation_limit", "Evaluation budget exhausted."));
        }
        self.evaluations += 1;
        match self.raw(values) {
            Ok(score) => Ok(Some(score)),
            Err(e) if e.code == "cancelled" => {
                self.cancelled = true;
                Ok(None)
            }
            Err(e) if matches!(e.code.as_str(), "domain_error" | "numeric_error" | "non_finite") => {
                self.invalid += 1;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}
fn random_distinct(seed: u64, generation: u64, target: usize, excluded: &[usize], count: usize, salt: u64) -> Result<usize> {
    let start = math::addressed_index(seed, generation, target as u64, salt, count)?;
    for offset in 0..count {
        let index = (start + offset) % count;
        if !excluded.contains(&index) {
            return Ok(index);
        }
    }
    Err(error("population", "Population has insufficient distinct members."))
}
fn trial_candidate(population: &[Member], domains: &[Domain], seed: u64, generation: u64, target: usize) -> Result<Vec<f64>> {
    let a = random_distinct(seed, generation, target, &[target], population.len(), 0x1000)?;
    let b = random_distinct(seed, generation, target, &[target, a], population.len(), 0x2000)?;
    let c = random_distinct(seed, generation, target, &[target, a, b], population.len(), 0x3000)?;
    let first = population.get(a).ok_or_else(|| error("population", "Missing mutation member."))?;
    let second = population.get(b).ok_or_else(|| error("population", "Missing mutation member."))?;
    let third = population.get(c).ok_or_else(|| error("population", "Missing mutation member."))?;
    let current = population.get(target).ok_or_else(|| error("population", "Missing target member."))?;
    let forced = math::addressed_index(seed, generation, target as u64, 0x4000, domains.len())?;
    let mut result = Vec::new();
    reserve(&mut result, domains.len())?;
    for (j, domain) in domains.iter().enumerate() {
        let old = *current.values.get(j).ok_or_else(|| error("population", "Missing target coordinate."))?;
        let a = *first.values.get(j).ok_or_else(|| error("population", "Missing mutation coordinate."))?;
        let b = *second.values.get(j).ok_or_else(|| error("population", "Missing mutation coordinate."))?;
        let c = *third.values.get(j).ok_or_else(|| error("population", "Missing mutation coordinate."))?;
        let p = math::addressed_uniform(seed, generation, target as u64, 0x5000 + j as u64);
        let mutation = a + 0.8 * (b - c);
        let value = if j == forced || p < 0.9 {
            if mutation.is_finite() {
                mutation
            } else {
                domain.low + (domain.high - domain.low) * math::addressed_uniform(seed, j as u64, generation, 0x6000 + target as u64)
            }
        } else {
            old
        };
        result.push(repair(value, *domain));
    }
    Ok(result)
}
fn no_solution(termination: Termination) -> SolveResult {
    SolveResult {
        termination,
        feasibility: Feasibility::Unknown,
        optimality: Optimality::Unknown,
        values: None,
        objective: None,
        best_bound: None,
        relative_gap: None,
        max_violation: None,
        statistics: SolveStatistics::default(),
        engine: "solvercraft-differential-evolution-v1".into(),
        warnings: vec!["Heuristic search supplies no optimality or infeasibility certificate.".into()],
    }
}
/// DE/rand/1/bin with F=0.8 and CR=0.9, finite bounds and feasibility-first ranking.
/// The first candidates are interval centers and endpoints. Search work uses strict model
/// evaluation / generation budgets. A validation evaluation is reserved for the chosen point.
pub fn solve(input: &ExpressionModel, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveResult> {
    let (domains, mut population_size) = validate(input, options)?;
    if control.is_cancelled() {
        return Ok(no_solution(Termination::Cancelled));
    }
    if options.max_evaluations == 0 {
        return Ok(no_solution(Termination::EvaluationLimit));
    }
    if options.max_iterations == 0 {
        return Ok(no_solution(Termination::IterationLimit));
    }
    if domains.iter().all(|d| d.low == d.high) {
        population_size = 1;
    }
    let mut evaluator = Evaluation { input, options, domains: &domains, control, evaluations: 0, invalid: 0, cancelled: false };
    let search_budget = options.max_evaluations.saturating_sub(1);
    let mut population = Vec::new();
    reserve(&mut population, population_size)?;
    let mut termination = Termination::Completed;
    let mut iterations = 0;
    for index in 0..population_size {
        if control.is_cancelled() {
            termination = Termination::Cancelled;
            break;
        }
        if evaluator.evaluations >= search_budget {
            termination = Termination::EvaluationLimit;
            break;
        }
        let values = initial(&domains, options.seed, index)?;
        let score = evaluator.score(&values)?;
        population.push(Member { values, score });
        if evaluator.cancelled {
            termination = Termination::Cancelled;
            break;
        }
    }
    let mut stalled = 0_u64;
    if termination == Termination::Completed && population.len() >= 4 && !domains.is_empty() {
        'generations: for generation in 0..options.max_iterations {
            if control.is_cancelled() {
                termination = Termination::Cancelled;
                break;
            }
            let prior = best_index(&population, input.direction).and_then(|i| population.get(i)).and_then(|m| m.score);
            iterations = generation + 1;
            for target in 0..population.len() {
                if control.is_cancelled() {
                    termination = Termination::Cancelled;
                    break 'generations;
                }
                if evaluator.evaluations >= search_budget {
                    termination = Termination::EvaluationLimit;
                    break 'generations;
                }
                let values = trial_candidate(&population, &domains, options.seed, generation, target)?;
                let score = evaluator.score(&values)?;
                let old = population.get(target).and_then(|m| m.score);
                let improves = match (score, old) {
                    (Some(new), Some(old)) => compare(new, old, input.direction) == Ordering::Less,
                    (Some(_), None) => true,
                    _ => false,
                };
                if improves {
                    let member = population.get_mut(target).ok_or_else(|| error("population", "Missing target member."))?;
                    *member = Member { values, score };
                }
                if evaluator.cancelled {
                    termination = Termination::Cancelled;
                    break 'generations;
                }
            }
            let best = best_index(&population, input.direction).and_then(|i| population.get(i)).and_then(|m| m.score);
            control.progress(&ProgressEvent {
                stage: "differential_evolution".into(),
                iterations,
                evaluations: evaluator.evaluations,
                nodes: 0,
                best_objective: best.filter(|s| s.feasible).map(|s| s.objective),
            });
            if let (Some(old), Some(new)) = (prior, best) {
                let improved = new.feasible
                    && (!old.feasible
                        || (old.objective - new.objective).abs()
                            > options.optimality_tolerance * (1.0 + old.objective.abs().max(new.objective.abs())));
                stalled = if improved { 0 } else { stalled + 1 };
                if new.feasible && stalled >= 50 {
                    break;
                }
            }
            if generation + 1 == options.max_iterations {
                termination = Termination::IterationLimit;
            }
        }
    }
    let member = best_index(&population, input.direction).and_then(|i| population.get(i));
    let chosen = match member {
        Some(member) => copy_values(&member.values)?,
        None => initial(&domains, options.seed, 0)?,
    };
    // A cancellation does not invalidate an already checked immutable-model incumbent.
    let score = if termination == Termination::Cancelled || control.is_cancelled() {
        termination = Termination::Cancelled;
        member.and_then(|m| m.score)
    } else {
        evaluator.score(&chosen)?
    };
    if evaluator.cancelled {
        termination = Termination::Cancelled;
    }
    let mut result = no_solution(termination);
    result.statistics = SolveStatistics { evaluations: evaluator.evaluations, iterations, nodes: 0 };
    if evaluator.invalid > 0 {
        result.warnings.push(format!(
            "{} candidates had invalid objective/constraint domains or numerical results and were not treated as zero.",
            evaluator.invalid
        ));
    }
    if stalled >= 50 {
        result.warnings.push("Search stopped after 50 generations without significant improvement; this is not an optimality test.".into());
    }
    if let Some(score) = score.filter(|s| s.feasible) {
        result.feasibility = Feasibility::Feasible;
        result.optimality = Optimality::BestFound;
        result.values = Some(chosen);
        result.objective = Some(score.objective);
        result.max_violation = Some(score.violation);
    } else {
        result.warnings.push("No feasible point was found; this is not a proof that the model is infeasible.".into());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "heuristic_tests.rs"]
mod tests;
