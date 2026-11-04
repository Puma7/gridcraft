#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_types::{NoControl, Variable};
fn x(i: usize) -> Expr {
    Expr::variable(i)
}
fn model(n: usize, objective: Expr, rows: Vec<(Expr, Relation, Expr)>) -> ExpressionModel {
    ExpressionModel {
        name: "analytic NLP".into(),
        variables: (0..n).map(|i| Variable { name: format!("x{i}"), lower: Some(-3.0), upper: Some(3.0), kind: VariableKind::Continuous }).collect(),
        direction: Direction::Minimize,
        objective,
        constraints: rows.into_iter().map(|(left, relation, right)| ExpressionConstraint { name: "row".into(), left, relation, right }).collect(),
    }
}
fn options() -> SolveOptions {
    SolveOptions { optimality_tolerance: 2e-6, feasibility_tolerance: 1e-6, ..SolveOptions::default() }
}
#[test]
fn equality_constrained_quadratic_checks_original_kkt() {
    let m = model(2, add(square(x(0)), square(x(1))), vec![(add(x(0), x(1)), Relation::Equal, c(1.0))]);
    let r = solve(&m, &options(), &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
    let v = r.values.unwrap();
    assert!((v[0] - 0.5).abs() < 2e-6);
    assert!((v[1] - 0.5).abs() < 2e-6);
    assert!((r.objective.unwrap() - 0.5).abs() < 2e-6);
    assert!(r.best_bound.is_none());
}
#[test]
fn active_and_inactive_inequalities() {
    for relation in [Relation::LessEqual, Relation::GreaterEqual] {
        let (left, right) = if relation == Relation::LessEqual { (x(0), c(1.0)) } else { (c(1.0), x(0)) };
        let m = model(1, square(sub(x(0), c(2.0))), vec![(left, relation, right)]);
        let r = solve(&m, &options(), &NlpOptions::default(), &NoControl).unwrap();
        assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
        assert!((r.values.unwrap()[0] - 1.0).abs() < 2e-6);
    }
    let m = model(1, square(x(0)), vec![(x(0), Relation::LessEqual, c(1.0))]);
    let r = solve(&m, &options(), &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
}
#[test]
fn nonlinear_inequality_is_not_silently_linearized() {
    let m = model(1, square(sub(x(0), c(2.0))), vec![(square(x(0)), Relation::LessEqual, c(1.0))]);
    let r = solve_from(&m, &[0.8], &options(), &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
    assert!((r.values.unwrap()[0] - 1.0).abs() < 2e-6);
}
#[test]
fn impossible_constraint_does_not_manufacture_infeasibility_proof() {
    let m = model(1, square(x(0)), vec![(square(x(0)), Relation::LessEqual, c(-1.0))]);
    let n = NlpOptions { max_outer_iterations: 3, ..NlpOptions::default() };
    let r = solve(&m, &options(), &n, &NoControl).unwrap();
    assert_eq!(r.feasibility, Feasibility::Unknown);
    assert_eq!(r.optimality, Optimality::Unknown);
    assert!(r.best_bound.is_none());
}
#[test]
fn evaluation_budget_is_shared_by_inner_and_original_validation() {
    let m = model(1, square(x(0)), vec![(x(0), Relation::Equal, c(1.0))]);
    for budget in [0, 1, 3, 8, 20] {
        let o = SolveOptions { max_evaluations: budget, ..options() };
        let r = solve(&m, &o, &NlpOptions::default(), &NoControl).unwrap();
        assert!(r.statistics.evaluations <= budget);
        assert_eq!(r.termination, Termination::EvaluationLimit, "{r:?}");
    }
}
#[test]
fn explicit_finite_differences_solve_a_smooth_constrained_case() {
    let m = model(1, square(sub(x(0), c(2.0))), vec![(x(0), Relation::LessEqual, c(1.0))]);
    let n = NlpOptions { derivative_mode: DerivativeMode::FiniteDifference, ..NlpOptions::default() };
    let r = solve(&m, &options(), &n, &NoControl).unwrap();
    assert_eq!(r.feasibility, Feasibility::Feasible, "{r:?}");
    assert!((r.values.unwrap()[0] - 1.0).abs() < 1e-5);
    assert!(r.warnings.iter().any(|w| w.contains("approximate")));
}
#[test]
fn target_goal_preserves_original_objective_value() {
    let m = model(1, x(0), vec![]);
    let targeted = solvercraft_compile::with_target(&m, 1.25).unwrap();
    let r = solve(&targeted, &options(), &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(r.feasibility, Feasibility::Feasible, "{r:?}");
    assert!((r.objective.unwrap() - 1.25).abs() < 1e-6);
}
#[test]
fn invalid_options_domain_and_integer_are_errors() {
    let m = model(1, Expr::Ln { value: Box::new(x(0)) }, vec![]);
    assert_eq!(solve(&m, &options(), &NlpOptions::default(), &NoControl).unwrap_err().code, "domain_error");
    let mut m = model(1, square(x(0)), vec![]);
    m.variables[0].kind = VariableKind::Integer;
    assert_eq!(solve(&m, &options(), &NlpOptions::default(), &NoControl).unwrap_err().code, "unsupported_problem");
    let n = NlpOptions { initial_penalty: f64::NAN, ..NlpOptions::default() };
    assert_eq!(solve(&m, &options(), &n, &NoControl).unwrap_err().code, "invalid_options");
}
#[test]
fn default_tolerances_remain_original_unit_kkt_thresholds() {
    let m = model(2, add(square(x(0)), square(x(1))), vec![(add(x(0), x(1)), Relation::Equal, c(1.0))]);
    let o = SolveOptions::default();
    let r = solve(&m, &o, &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
    assert!(r.max_violation.unwrap() <= o.feasibility_tolerance);
}
#[test]
fn interrupted_constraint_search_retains_a_previously_verified_feasible_point() {
    let m = model(1, square(sub(x(0), c(2.0))), vec![(x(0), Relation::LessEqual, c(1.0))]);
    let n = NlpOptions { max_outer_iterations: 1, ..NlpOptions::default() };
    let r = solve(&m, &options(), &n, &NoControl).unwrap();
    assert_eq!(r.feasibility, Feasibility::Feasible, "{r:?}");
    assert!(r.values.unwrap()[0] <= 1.0 + options().feasibility_tolerance);
    assert_ne!(r.optimality, Optimality::Stationary);
}
