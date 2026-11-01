#![allow(clippy::unwrap_used)]
use solvercraft_compile::{Expr, validate_expressions};
use solvercraft_types::SolveOptions;
#[test]
fn shared_node_limit_rejects_many_individually_small_outputs() {
    let mut expression = Expr::variable(0);
    for _ in 0..24 {
        expression = Expr::Add { left: Box::new(expression), right: Box::new(Expr::constant(1.0)) };
    }
    validate_expressions(std::slice::from_ref(&expression), 1, &SolveOptions::default()).unwrap();
    let outputs = vec![expression; 4096];
    assert_eq!(validate_expressions(&outputs, 1, &SolveOptions::default()).unwrap_err().code, "expression_limit");
}
#[test]
fn caller_memory_budget_applies_to_all_nodes_and_variables() {
    let outputs = [Expr::variable(0), Expr::constant(3.0)];
    let exact = 2 * 160 + 256;
    validate_expressions(&outputs, 1, &SolveOptions { max_memory_bytes: exact, ..SolveOptions::default() }).unwrap();
    assert_eq!(
        validate_expressions(&outputs, 1, &SolveOptions { max_memory_bytes: exact - 1, ..SolveOptions::default() }).unwrap_err().code,
        "memory_limit"
    );
    let nested = [Expr::Add { left: Box::new(Expr::variable(0)), right: Box::new(Expr::constant(1.0)) }];
    assert_eq!(
        validate_expressions(&nested, 1, &SolveOptions { max_memory_bytes: 160 + 256, ..SolveOptions::default() }).unwrap_err().code,
        "memory_limit"
    );
}
#[test]
fn structural_preflight_does_not_require_a_polynomial_or_a_valid_point() {
    let output = [Expr::Ln { value: Box::new(Expr::constant(-1.0)) }];
    validate_expressions(&output, 0, &SolveOptions::default()).unwrap();
    assert_eq!(validate_expressions(&[Expr::variable(3)], 3, &SolveOptions::default()).unwrap_err().code, "invalid_variable");
    assert_eq!(validate_expressions(&[Expr::constant(f64::NAN)], 0, &SolveOptions::default()).unwrap_err().code, "non_finite");
}
