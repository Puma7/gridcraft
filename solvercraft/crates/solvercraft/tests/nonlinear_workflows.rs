//! Public SDK workflows with actual AST parsing, numerical solves, and proofs.
#![allow(clippy::unwrap_used)]
use solvercraft::{
    compile::{self, Expr, ExpressionModel},
    constrained, global, multistart,
    nonlinear::{DerivativeMode, NlpOptions},
    types::*,
};

fn model(objective: Expr) -> ExpressionModel {
    ExpressionModel {
        name: "external nonlinear workflow".into(),
        variables: vec![Variable { name: "decision".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective,
        constraints: vec![],
    }
}
#[test]
fn persisted_nonlinear_maximum_target_is_solved_and_checked_in_original_units() {
    let mut input = model(Expr::Exp { value: Box::new(Expr::variable(0)) });
    input.direction = Direction::Maximize;
    let encoded = serde_json::to_string(&input).unwrap();
    let parsed: ExpressionModel = serde_json::from_str(&encoded).unwrap();
    let o = SolveOptions::default();
    compile::validate_expression_model(&parsed, &o).unwrap();
    let targeted = compile::with_target(&parsed, 2.0).unwrap();
    let solved = constrained::solve(&targeted, &o, &NlpOptions::default(), &NoControl).unwrap();
    assert_eq!(solved.feasibility, Feasibility::Feasible, "{solved:?}");
    let x = solved.values.unwrap();
    let original = compile::evaluate(&parsed.objective, &x).unwrap();
    assert_eq!(solved.objective, Some(original));
    assert!((original - 2.0).abs() <= o.feasibility_tolerance);
    assert!(solved.best_bound.is_none());
    assert_ne!(solved.optimality, Optimality::Optimal);
}
#[test]
fn numerical_derivative_restart_configuration_is_seed_lossless_and_budgeted() {
    let m = model(Expr::Power {
        base: Box::new(Expr::Subtract { left: Box::new(Expr::variable(0)), right: Box::new(Expr::constant(0.375)) }),
        exponent: 2,
    });
    let n: NlpOptions = serde_json::from_str(r#"{"derivative_mode":"finite_difference","finite_difference_step":0.000001}"#).unwrap();
    assert_eq!(n.derivative_mode, DerivativeMode::FiniteDifference);
    let multi: multistart::MultiStartOptions = serde_json::from_str(r#"{"starts":4,"seed":"18446744073709551615"}"#).unwrap();
    let o = SolveOptions { max_evaluations: 1000, max_iterations: 200, ..SolveOptions::default() };
    let result = multistart::solve(&m, &o, &n, &multi, &NoControl).unwrap();
    assert_eq!(result.attempted_starts, 4);
    assert_eq!(result.solution.feasibility, Feasibility::Feasible);
    assert!(result.solution.objective.unwrap() < 1e-12);
    assert!(result.solution.statistics.evaluations <= 1000);
    assert!(result.solution.statistics.iterations <= 200);
    assert!(result.solution.best_bound.is_none());
}
#[test]
fn serialized_global_proof_replays_and_detects_edited_incumbents() {
    let m = model(Expr::Power {
        base: Box::new(Expr::Subtract {
            left: Box::new(Expr::Power { base: Box::new(Expr::variable(0)), exponent: 2 }),
            right: Box::new(Expr::constant(1.0)),
        }),
        exponent: 2,
    });
    let o = SolveOptions::default();
    let proof = global::solve_box_polynomial(&m, &o, &NoControl).unwrap();
    let encoded = serde_json::to_string(&proof).unwrap();
    let mut loaded: global::GlobalSolveResult = serde_json::from_str(&encoded).unwrap();
    assert_eq!(loaded.solution.optimality, Optimality::Optimal);
    assert!(global::verify_box_certificate(&m, &loaded, &o).unwrap());
    loaded.solution.objective = Some(-100.0);
    assert!(!global::verify_box_certificate(&m, &loaded, &o).unwrap());
}
