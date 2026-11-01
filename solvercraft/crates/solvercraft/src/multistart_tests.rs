#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_compile::Expr;
use solvercraft_types::{NoControl, Variable};
fn model() -> ExpressionModel {
    let x = Expr::variable(0);
    let square = Expr::Power { base: Box::new(x.clone()), exponent: 2 };
    ExpressionModel {
        name: "double well".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective: Expr::Add {
            left: Box::new(Expr::Power {
                base: Box::new(Expr::Subtract { left: Box::new(square), right: Box::new(Expr::constant(1.0)) }),
                exponent: 2,
            }),
            right: Box::new(Expr::Multiply { left: Box::new(Expr::constant(0.2)), right: Box::new(x) }),
        },
        constraints: vec![],
    }
}
#[test]
fn seeded_restarts_escape_a_worse_local_basin_and_repeat() {
    let m = model();
    let o = SolveOptions { optimality_tolerance: 1e-6, ..SolveOptions::default() };
    let n = NlpOptions::default();
    let multi = MultiStartOptions { starts: 12, seed: 42 };
    let mut positive = 1.0_f64;
    for _ in 0..12 {
        positive -= (4.0 * positive.powi(3) - 4.0 * positive + 0.2) / (12.0 * positive * positive - 4.0);
    }
    let single = nonlinear::solve_from(&m, &[positive], &o, &solvercraft_types::NoControl).unwrap();
    let a = solve_from(&m, &[positive], &o, &n, &multi, &NoControl).unwrap();
    let b = solve_from(&m, &[positive], &o, &n, &multi, &NoControl).unwrap();
    assert_eq!(a.attempted_starts, 12);
    assert_eq!(a.solution.values, b.solution.values);
    assert!(a.solution.objective.unwrap() < single.objective.unwrap() - 0.3);
    assert!(a.solution.best_bound.is_none());
    assert_ne!(a.solution.optimality, Optimality::Optimal);
}
#[test]
fn restart_budget_is_not_reset_per_run() {
    let m = model();
    let o = SolveOptions { max_evaluations: 40, max_iterations: 25, ..SolveOptions::default() };
    let r = solve(&m, &o, &NlpOptions::default(), &MultiStartOptions { starts: 20, seed: 0 }, &NoControl).unwrap();
    assert!(r.solution.statistics.evaluations <= 40);
    assert!(r.solution.statistics.iterations <= 25);
    assert!(r.attempted_starts <= 20);
}
#[test]
fn unsupported_bounds_and_invalid_start_are_explicit() {
    let mut m = model();
    m.variables[0].upper = None;
    assert_eq!(
        solve(&m, &SolveOptions::default(), &NlpOptions::default(), &MultiStartOptions::default(), &NoControl).unwrap_err().code,
        "unsupported_problem"
    );
    assert_eq!(
        solve_from(&m, &[f64::NAN], &SolveOptions::default(), &NlpOptions::default(), &MultiStartOptions { starts: 1, seed: 0 }, &NoControl)
            .unwrap_err()
            .code,
        "invalid_values"
    );
}
#[test]
fn full_u64_seed_is_string_serialized() {
    let m = MultiStartOptions { starts: 2, seed: u64::MAX };
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(json["seed"], u64::MAX.to_string());
}
