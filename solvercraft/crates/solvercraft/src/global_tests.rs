#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_types::{NoControl, Variable};
fn x() -> Expr {
    Expr::variable(0)
}
fn c(v: f64) -> Expr {
    Expr::constant(v)
}
fn sub(l: Expr, r: Expr) -> Expr {
    Expr::Subtract { left: Box::new(l), right: Box::new(r) }
}
fn sq(base: Expr) -> Expr {
    Expr::Power { base: Box::new(base), exponent: 2 }
}
fn model(objective: Expr) -> ExpressionModel {
    ExpressionModel {
        name: "global quartic".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective,
        constraints: vec![],
    }
}
#[test]
fn quartic_multiple_minima_gets_an_outward_global_gap() {
    let m = model(sq(sub(sq(x()), c(1.0))));
    let r = solve_box_polynomial(&m, &SolveOptions::default(), &NoControl).unwrap();
    assert_eq!(r.solution.optimality, Optimality::Optimal, "{r:?}");
    assert!(r.solution.objective.unwrap() < 1e-8);
    assert!(r.certificate.signed_lower_bound.unwrap() <= 0.0);
    assert!(r.certificate.absolute_gap.unwrap() <= 1e-8);
    assert!(r.certificate.tolerance_certified);
    assert!(verify_box_certificate(&m, &r, &SolveOptions::default()).unwrap());
}
#[test]
fn maximize_sign_and_original_bound_are_consistent() {
    let mut m = model(Expr::Negate { value: Box::new(sq(sub(x(), c(0.3)))) });
    m.direction = Direction::Maximize;
    let r = solve_box_polynomial(&m, &SolveOptions::default(), &NoControl).unwrap();
    assert_eq!(r.solution.optimality, Optimality::Optimal, "{r:?}");
    assert!(r.solution.best_bound.unwrap() >= 0.0);
    assert!(r.solution.objective.unwrap() > -1e-8);
}
#[test]
fn every_interruption_retains_a_bound_on_unsearched_space() {
    let m = model(sq(sub(x(), c(0.3))));
    for (eval, nodes) in [(0, 100), (1, 100), (100, 0), (100, 1)] {
        let options = SolveOptions { max_evaluations: eval, max_nodes: nodes, ..SolveOptions::default() };
        let r = solve_box_polynomial(&m, &options, &NoControl).unwrap();
        assert!(r.solution.best_bound.unwrap() <= 0.0, "{r:?}");
        assert!(r.solution.statistics.evaluations <= eval);
        assert!(r.solution.statistics.nodes <= nodes);
        assert!(!r.certificate.tolerance_certified);
        assert!(r.certificate.remaining_boxes > 0);
        assert!(verify_box_certificate(&m, &r, &options).unwrap());
    }
}
#[test]
fn interval_enclosure_contains_dense_independent_grid() {
    let expr = Expr::Multiply { left: Box::new(sub(sq(x()), c(0.7))), right: Box::new(sub(x(), c(-0.4))) };
    let b = [(-1.7, 1.4)];
    let bound = interval(&expr, &b).unwrap();
    for i in 0..1001 {
        let x = -1.7 + 3.1 * f64::from(i) / 1000.0;
        let v = (x * x - 0.7) * (x + 0.4);
        assert!(bound.lo <= v && v <= bound.hi);
    }
}
#[test]
fn unsupported_and_overflow_are_explicit() {
    let m = model(Expr::Sin { value: Box::new(x()) });
    assert_eq!(solve_box_polynomial(&m, &SolveOptions::default(), &NoControl).unwrap_err().code, "unsupported_problem");
    let mut m = model(x());
    m.variables[0].upper = None;
    assert_eq!(solve_box_polynomial(&m, &SolveOptions::default(), &NoControl).unwrap_err().code, "unsupported_problem");
    let m = model(Expr::Multiply { left: Box::new(c(1e308)), right: Box::new(c(1e308)) });
    assert_eq!(solve_box_polynomial(&m, &SolveOptions::default(), &NoControl).unwrap_err().code, "numeric_error");
}
#[test]
fn independent_replay_rejects_tampered_certificates() {
    let m = model(sq(sub(x(), c(0.31))));
    let o = SolveOptions::default();
    let r = solve_box_polynomial(&m, &o, &NoControl).unwrap();
    assert!(!r.certificate.splits.is_empty());
    assert!(verify_box_certificate(&m, &r, &o).unwrap());
    let mut bad = r.clone();
    bad.certificate.signed_lower_bound = Some(1.0);
    assert!(!verify_box_certificate(&m, &bad, &o).unwrap());
    let mut bad = r.clone();
    bad.certificate.splits[0].midpoint = 3.0;
    assert!(!verify_box_certificate(&m, &bad, &o).unwrap());
    let mut bad = r.clone();
    bad.certificate.splits[0].parent = 99;
    assert!(!verify_box_certificate(&m, &bad, &o).unwrap());
    let mut bad = r.clone();
    bad.solution.values = Some(vec![9.0]);
    assert!(!verify_box_certificate(&m, &bad, &o).unwrap());
    let mut bad = r.clone();
    bad.certificate.domain[0].0 = -1.0;
    assert!(!verify_box_certificate(&m, &bad, &o).unwrap());
}
