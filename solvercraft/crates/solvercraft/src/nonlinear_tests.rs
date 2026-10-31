#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_compile::ExpressionConstraint;
use solvercraft_types::{NoControl, Relation, Variable};
use std::sync::atomic::{AtomicBool, Ordering};

fn c(value: f64) -> Expr {
    Expr::constant(value)
}
fn x(index: usize) -> Expr {
    Expr::variable(index)
}
fn add(left: Expr, right: Expr) -> Expr {
    Expr::Add { left: Box::new(left), right: Box::new(right) }
}
fn sub(left: Expr, right: Expr) -> Expr {
    Expr::Subtract { left: Box::new(left), right: Box::new(right) }
}
fn mul(left: Expr, right: Expr) -> Expr {
    Expr::Multiply { left: Box::new(left), right: Box::new(right) }
}
fn square(base: Expr) -> Expr {
    Expr::Power { base: Box::new(base), exponent: 2 }
}
fn variable(name: &str, lower: Option<f64>, upper: Option<f64>) -> Variable {
    Variable { name: name.into(), lower, upper, kind: VariableKind::Continuous }
}
fn model(objective: Expr, variables: Vec<Variable>) -> ExpressionModel {
    ExpressionModel { name: "nonlinear regression".into(), variables, direction: Direction::Minimize, objective, constraints: vec![] }
}
fn options() -> SolveOptions {
    SolveOptions { method: SolveMethod::ProjectedGradient, ..SolveOptions::default() }
}
#[test]
fn narrow_bounds_do_not_become_a_zero_gradient() {
    let m = model(square(sub(mul(c(1e7), x(0)), c(4.0))), vec![variable("x", Some(0.0), Some(5e-7))]);
    let r = solve(&m, &options(), &NoControl).unwrap();
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert_eq!(r.optimality, Optimality::Stationary);
    assert!(r.objective.unwrap() < 1e-20);
    assert!((r.values.unwrap()[0] - 4e-7).abs() < 1e-15);
    assert!(r.statistics.evaluations > 1);
}
#[test]
fn bounded_large_objective_never_implies_unboundedness() {
    let m = model(mul(c(-1e16), x(0)), vec![variable("x", Some(0.0), Some(1.0))]);
    let r = solve(&m, &options(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary);
    assert_eq!(r.values, Some(vec![1.0]));
    assert_eq!(r.objective, Some(-1e16));
    assert!(r.best_bound.is_none() && r.relative_gap.is_none());
}
#[test]
fn rosenbrock_converges_from_a_nontrivial_start() {
    let expression = add(square(sub(c(1.0), x(0))), mul(c(100.0), square(sub(x(1), square(x(0))))));
    let m = model(expression, vec![variable("x", Some(-3.0), Some(3.0)), variable("y", Some(-3.0), Some(3.0))]);
    let o = SolveOptions { max_evaluations: 1_000_000, max_iterations: 100_000, optimality_tolerance: 1e-6, ..options() };
    let r = solve_from(&m, &[-1.2, 1.0], &o, &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
    assert!(r.objective.unwrap() < 1e-10, "{r:?}");
    let v = r.values.unwrap();
    assert!((v[0] - 1.0).abs() < 1e-4 && (v[1] - 1.0).abs() < 1e-4);
}
#[test]
fn exponential_has_a_smooth_nonpolynomial_stationary_point() {
    let expression = sub(Expr::Exp { value: Box::new(x(0)) }, mul(c(2.0), x(0)));
    let m = model(expression, vec![variable("x", Some(-2.0), Some(2.0))]);
    let r = solve(&m, &options(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary, "{r:?}");
    assert!((r.values.unwrap()[0] - 2.0_f64.ln()).abs() < 1e-7);
}
#[test]
fn maximization_sign_and_exact_active_bound_are_correct() {
    let mut m = model(x(0), vec![variable("x", Some(-2.0), Some(3.0))]);
    m.direction = Direction::Maximize;
    let r = solve(&m, &options(), &NoControl).unwrap();
    assert_eq!(r.values, Some(vec![3.0]));
    assert_eq!(r.objective, Some(3.0));
    assert_eq!(r.optimality, Optimality::Stationary);
}
#[test]
fn being_close_to_a_bound_does_not_zero_an_inward_gradient() {
    let m = model(x(0), vec![variable("x", Some(0.0), Some(1.0))]);
    let o = SolveOptions { max_iterations: 0, ..options() };
    let r = solve_from(&m, &[1e-10], &o, &NoControl).unwrap();
    assert_eq!(r.termination, Termination::IterationLimit);
    assert_eq!(r.optimality, Optimality::BestFound);
    let r = solve_from(&m, &[1e-10], &options(), &NoControl).unwrap();
    assert_eq!(r.values, Some(vec![0.0]));
    assert_eq!(r.optimality, Optimality::Stationary);
}
#[test]
fn start_projection_is_visible_and_feasible() {
    let m = model(square(sub(x(0), c(2.0))), vec![variable("x", Some(1.0), Some(4.0))]);
    let r = solve_from(&m, &[-3.0], &options(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary);
    assert!(r.warnings.iter().any(|s| s.contains("projected")));
    assert!((r.values.unwrap()[0] - 2.0).abs() < 1e-7);
}
#[test]
fn derivative_and_domain_failures_are_not_convergence() {
    let m = model(Expr::Ln { value: Box::new(x(0)) }, vec![variable("x", Some(0.0), Some(2.0))]);
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "domain_error");
    let m = model(Expr::Abs { value: Box::new(x(0)) }, vec![variable("x", Some(-2.0), Some(2.0))]);
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "nondifferentiable");
}
#[test]
fn line_search_domain_rejections_keep_a_valid_best_point() {
    let m = model(Expr::Ln { value: Box::new(x(0)) }, vec![variable("x", Some(-2.0), Some(2.0))]);
    let o = SolveOptions { max_evaluations: 50, ..options() };
    let r = solve_from(&m, &[1.0], &o, &NoControl).unwrap();
    assert_ne!(r.optimality, Optimality::Unbounded);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert!(r.values.unwrap()[0] > 0.0);
    assert!(matches!(r.termination, Termination::EvaluationLimit | Termination::NumericalFailure));
}
#[test]
fn fixed_singleton_does_not_require_a_derivative() {
    let m = model(Expr::Abs { value: Box::new(x(0)) }, vec![variable("x", Some(0.0), Some(0.0))]);
    let r = solve(&m, &options(), &NoControl).unwrap();
    assert_eq!(r.optimality, Optimality::Stationary);
    assert_eq!(r.objective, Some(0.0));
}
#[test]
fn unsupported_domains_constraints_and_methods_are_rejected() {
    let mut m = model(square(x(0)), vec![variable("x", Some(0.0), Some(1.0))]);
    m.variables[0].kind = VariableKind::Integer;
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "unsupported_problem");
    m.variables[0].kind = VariableKind::Continuous;
    m.constraints.push(ExpressionConstraint { name: "not bounds".into(), left: x(0), relation: Relation::LessEqual, right: c(1.0) });
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "unsupported_problem");
    m.constraints.clear();
    let o = SolveOptions { method: SolveMethod::Simplex, ..options() };
    assert_eq!(solve(&m, &o, &NoControl).unwrap_err().code, "unsupported_method");
}
#[test]
fn finite_bounds_and_finite_starts_are_required() {
    let mut m = model(square(x(0)), vec![variable("x", Some(0.0), Some(1.0))]);
    assert_eq!(solve_from(&m, &[f64::NAN], &options(), &NoControl).unwrap_err().code, "non_finite");
    m.variables[0].upper = Some(f64::INFINITY);
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "non_finite");
    m.variables[0].upper = Some(-1.0);
    assert_eq!(solve(&m, &options(), &NoControl).unwrap_err().code, "infeasible_bounds");
}
#[test]
fn budgets_have_truthful_partial_results() {
    let m = model(square(sub(x(0), c(2.0))), vec![variable("x", Some(0.0), Some(4.0))]);
    let o = SolveOptions { max_evaluations: 0, ..options() };
    let r = solve(&m, &o, &NoControl).unwrap();
    assert_eq!(r.termination, Termination::EvaluationLimit);
    assert_eq!(r.feasibility, Feasibility::Unknown);
    assert_eq!(r.statistics.evaluations, 0);
    let o = SolveOptions { max_iterations: 0, ..options() };
    let r = solve(&m, &o, &NoControl).unwrap();
    assert_eq!(r.termination, Termination::IterationLimit);
    assert_eq!(r.optimality, Optimality::BestFound);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    let o = SolveOptions { max_memory_bytes: 1, ..options() };
    assert_eq!(solve(&m, &o, &NoControl).unwrap().termination, Termination::MemoryLimit);
}
struct CancelOnProgress(AtomicBool);
impl RunControl for CancelOnProgress {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    fn progress(&self, _event: &ProgressEvent) {
        self.0.store(true, Ordering::Relaxed);
    }
}
#[test]
fn cancellation_is_checked_before_the_next_evaluation() {
    let m = model(square(sub(x(0), c(2.0))), vec![variable("x", Some(0.0), Some(4.0))]);
    let c = CancelOnProgress(AtomicBool::new(false));
    let r = solve(&m, &options(), &c).unwrap();
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.optimality, Optimality::BestFound);
    assert_eq!(r.statistics.evaluations, 1);
}
