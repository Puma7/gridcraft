use super::*;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

fn c(value: f64) -> Expr {
    Expr::constant(value)
}
fn x() -> Expr {
    Expr::variable(0)
}
fn sub(a: Expr, b: Expr) -> Expr {
    Expr::Subtract { left: Box::new(a), right: Box::new(b) }
}
fn mul(a: Expr, b: Expr) -> Expr {
    Expr::Multiply { left: Box::new(a), right: Box::new(b) }
}
fn add(a: Expr, b: Expr) -> Expr {
    Expr::Add { left: Box::new(a), right: Box::new(b) }
}
fn square(a: Expr) -> Expr {
    Expr::Power { base: Box::new(a), exponent: 2 }
}
fn variable(low: f64, high: f64) -> types::Variable {
    types::Variable { name: "x".into(), lower: Some(low), upper: Some(high), kind: VariableKind::Continuous }
}
fn problem(objective: Expr, low: f64, high: f64) -> ExpressionModel {
    ExpressionModel { name: "test".into(), variables: vec![variable(low, high)], direction: Direction::Minimize, objective, constraints: Vec::new() }
}
fn options() -> SolveOptions {
    SolveOptions { method: SolveMethod::DifferentialEvolution, max_evaluations: 20_000, max_iterations: 1000, ..SolveOptions::default() }
}
fn value(result: &SolveResult) -> Result<f64> {
    result.values.as_ref().and_then(|v| v.first()).copied().ok_or_else(|| error("test", "Expected a feasible point."))
}
#[test]
fn shifted_rugged_objective_is_actually_searched() -> Result<()> {
    let z = sub(x(), c(0.37));
    let objective = sub(add(c(10.0), square(z.clone())), mul(c(10.0), Expr::Cos { value: Box::new(mul(c(2.0 * std::f64::consts::PI), z)) }));
    let input = problem(objective, -5.0, 5.0);
    let result = solve(&input, &options(), &types::NoControl)?;
    assert_eq!(result.optimality, Optimality::BestFound);
    assert_eq!(result.feasibility, Feasibility::Feasible);
    assert!((value(&result)? - 0.37).abs() < 1e-3);
    assert!(result.objective.is_some_and(|v| v < 1e-6));
    assert_eq!(result.best_bound, None);
    assert_eq!(result.relative_gap, None);
    Ok(())
}
#[test]
fn non_smooth_cusp_and_nonlinear_constraints() -> Result<()> {
    let objective = add(Expr::Abs { value: Box::new(sub(x(), c(0.123))) }, mul(c(0.01), Expr::Sin { value: Box::new(mul(c(40.0), x())) }));
    let result = solve(&problem(objective, -1.0, 1.0), &options(), &types::NoControl)?;
    assert!((value(&result)? - 0.123).abs() < 1e-4);
    let mut constrained = problem(square(sub(x(), c(0.3))), -2.0, 2.0);
    constrained.constraints.push(compile::ExpressionConstraint {
        name: "outside unit interval".into(),
        left: square(x()),
        relation: Relation::GreaterEqual,
        right: c(1.0),
    });
    let result = solve(&constrained, &options(), &types::NoControl)?;
    let v = value(&result)?;
    assert!(v * v >= 1.0 - options().feasibility_tolerance);
    assert!((v - 1.0).abs() < 1e-3);
    Ok(())
}
#[test]
fn mixed_binary_values_are_repaired_and_constraints_checked() -> Result<()> {
    let input = ExpressionModel {
        name: "binary".into(),
        variables: vec![
            types::Variable { name: "x".into(), lower: None, upper: None, kind: VariableKind::Binary },
            types::Variable { name: "y".into(), lower: None, upper: None, kind: VariableKind::Binary },
        ],
        direction: Direction::Maximize,
        objective: add(mul(c(3.0), Expr::variable(0)), mul(c(2.0), Expr::variable(1))),
        constraints: vec![compile::ExpressionConstraint {
            name: "one".into(),
            left: add(Expr::variable(0), Expr::variable(1)),
            relation: Relation::LessEqual,
            right: c(1.0),
        }],
    };
    let result = solve(&input, &options(), &types::NoControl)?;
    assert_eq!(result.objective, Some(3.0));
    assert_eq!(result.values, Some(vec![1.0, 0.0]));
    assert_eq!(result.optimality, Optimality::BestFound);
    Ok(())
}
#[test]
fn narrow_bounds_and_large_objective_do_not_trigger_old_status_bugs() -> Result<()> {
    let narrow = problem(square(sub(mul(c(1e7), x()), c(4.0))), 0.0, 5e-7);
    let result = solve(&narrow, &options(), &types::NoControl)?;
    assert!(result.objective.is_some_and(|v| v < 1e-8));
    assert!((value(&result)? - 4e-7).abs() < 1e-10);
    let large = problem(mul(c(-1e16), x()), 0.0, 1.0);
    let result = solve(&large, &options(), &types::NoControl)?;
    assert_eq!(result.objective, Some(-1e16));
    assert_eq!(result.optimality, Optimality::BestFound);
    Ok(())
}
#[test]
fn repeatability_and_evaluation_budget_are_explicit() -> Result<()> {
    let input = problem(square(sub(x(), c(0.4))), 0.0, 1.0);
    let first = solve(&input, &options(), &types::NoControl)?;
    let second = solve(&input, &options(), &types::NoControl)?;
    assert_eq!(first.values, second.values);
    assert_eq!(first.objective, second.objective);
    assert_eq!(first.statistics.evaluations, second.statistics.evaluations);
    for budget in [0, 1, 2, 5, 25, 100] {
        let opts = SolveOptions { max_evaluations: budget, ..options() };
        let result = solve(&input, &opts, &types::NoControl)?;
        assert!(result.statistics.evaluations <= budget);
        if budget == 0 {
            assert!(result.values.is_none());
        }
    }
    Ok(())
}
struct Cancelled;
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}
struct CancelOnProgress(AtomicBool);
impl RunControl for CancelOnProgress {
    fn is_cancelled(&self) -> bool {
        self.0.load(AtomicOrdering::Relaxed)
    }
    fn progress(&self, _: &ProgressEvent) {
        self.0.store(true, AtomicOrdering::Relaxed);
    }
}
#[test]
fn cancellation_preserves_only_already_validated_points() -> Result<()> {
    let input = problem(square(sub(x(), c(0.4))), 0.0, 1.0);
    let result = solve(&input, &options(), &Cancelled)?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.statistics.evaluations, 0);
    assert!(result.values.is_none());
    let result = solve(&input, &options(), &CancelOnProgress(AtomicBool::new(false)))?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.feasibility, Feasibility::Feasible);
    let v = value(&result)?;
    assert!((0.0..=1.0).contains(&v));
    Ok(())
}
#[test]
fn failed_search_and_invalid_domains_are_not_infeasibility_proofs() -> Result<()> {
    let mut input = problem(square(x()), 0.0, 1.0);
    input.constraints.push(compile::ExpressionConstraint { name: "impossible".into(), left: x(), relation: Relation::GreaterEqual, right: c(2.0) });
    let opts = SolveOptions { max_evaluations: 100, ..options() };
    let result = solve(&input, &opts, &types::NoControl)?;
    assert_eq!(result.feasibility, Feasibility::Unknown);
    assert_eq!(result.optimality, Optimality::Unknown);
    assert!(result.values.is_none());
    let invalid = problem(Expr::Ln { value: Box::new(sub(x(), c(3.0))) }, 0.0, 1.0);
    let result = solve(&invalid, &opts, &types::NoControl)?;
    assert!(result.values.is_none());
    assert_eq!(result.objective, None);
    assert!(result.warnings.iter().any(|w| w.contains("invalid objective")));
    Ok(())
}
#[test]
fn input_memory_and_domain_limits_are_checked() {
    let mut input = problem(square(x()), 0.0, 1.0);
    input.variables[0].upper = None;
    assert!(solve(&input, &options(), &types::NoControl).is_err_and(|e| e.code == "finite_bounds_required"));
    input.variables[0].upper = Some(1.0);
    let opts = SolveOptions { max_memory_bytes: 1, ..options() };
    assert!(solve(&input, &opts, &types::NoControl).is_err_and(|e| e.code == "memory_limit"));
    input.objective = Expr::variable(1);
    assert!(solve(&input, &options(), &types::NoControl).is_err_and(|e| e.code == "invalid_variable"));
}
