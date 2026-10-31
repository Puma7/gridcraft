#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_types::VariableKind;
fn c(v: f64) -> Expr {
    Expr::constant(v)
}
fn x(i: usize) -> Expr {
    Expr::variable(i)
}
fn add(a: Expr, b: Expr) -> Expr {
    Expr::Add { left: Box::new(a), right: Box::new(b) }
}
fn sub(a: Expr, b: Expr) -> Expr {
    Expr::Subtract { left: Box::new(a), right: Box::new(b) }
}
fn mul(a: Expr, b: Expr) -> Expr {
    Expr::Multiply { left: Box::new(a), right: Box::new(b) }
}
fn pow(a: Expr, n: u32) -> Expr {
    Expr::Power { base: Box::new(a), exponent: n }
}
fn model(expr: Expr) -> ExpressionModel {
    ExpressionModel {
        name: "compiled test".into(),
        variables: (0..2).map(|i| Variable { name: format!("v{i}"), lower: None, upper: None, kind: VariableKind::Continuous }).collect(),
        direction: Direction::Minimize,
        objective: expr,
        constraints: vec![],
    }
}
#[test]
fn expanded_quadratic_has_explicit_cross_coefficient() {
    let expr = pow(add(x(0), mul(c(2.0), x(1))), 2);
    let m = compile(&model(expr.clone()), &SolveOptions::default()).unwrap();
    assert_eq!(
        m.objective.quadratic,
        vec![
            QuadraticTerm { left: 0, right: 0, coefficient: 1.0 },
            QuadraticTerm { left: 0, right: 1, coefficient: 4.0 },
            QuadraticTerm { left: 1, right: 1, coefficient: 4.0 }
        ]
    );
    for values in [[-3.0, 2.0], [0.0, 5.0], [1.5, -2.0]] {
        assert_eq!(evaluate(&expr, &values).unwrap(), solvercraft_model::evaluate_objective(&m, &values).unwrap());
    }
}
#[test]
fn polynomial_cancellation_and_constant_folding_are_structural() {
    let expr = add(sub(mul(x(0), x(1)), mul(x(1), x(0))), Expr::Sin { value: Box::new(c(0.0)) });
    assert_eq!(classify(&expr, 2).unwrap(), ExpressionClass::Constant);
    let m = compile(&model(expr), &SolveOptions::default()).unwrap();
    assert!(m.objective.quadratic.is_empty());
    assert_eq!(m.objective.offset, 0.0);
}
#[test]
fn pointwise_disguised_nonlinearity_is_not_classified_as_linear() {
    // This polynomial is zero at x=0,1,2,3. A finite sample test could miss it.
    let expr = mul(mul(x(0), sub(x(0), c(1.0))), mul(sub(x(0), c(2.0)), sub(x(0), c(3.0))));
    for v in 0..4 {
        assert_eq!(evaluate(&expr, &[f64::from(v)]).unwrap(), 0.0);
    }
    assert_eq!(classify(&expr, 1).unwrap(), ExpressionClass::Nonlinear);
    assert_eq!(compile(&model(expr), &SolveOptions::default()).unwrap_err().code, "unsupported_expression");
}
#[test]
fn variable_denominator_and_nonsmooth_operators_are_not_affine() {
    let expressions = [
        Expr::Divide { left: Box::new(c(1.0)), right: Box::new(x(0)) },
        Expr::Abs { value: Box::new(x(0)) },
        Expr::Maximum { left: Box::new(x(0)), right: Box::new(c(0.0)) },
    ];
    for expr in expressions {
        assert_eq!(classify(&expr, 1).unwrap(), ExpressionClass::Nonlinear);
        assert!(compile(&model(expr), &SolveOptions::default()).is_err());
    }
}
#[test]
fn rhs_expressions_move_to_the_original_row_correctly() {
    let mut m = model(x(0));
    m.constraints.push(ExpressionConstraint {
        name: "moving terms".into(),
        left: add(mul(c(3.0), x(0)), c(4.0)),
        relation: Relation::GreaterEqual,
        right: add(mul(c(2.0), x(1)), c(9.0)),
    });
    let compiled = compile(&m, &SolveOptions::default()).unwrap();
    assert_eq!(compiled.constraints[0].rhs, 5.0);
    assert_eq!(compiled.constraints[0].relation, Relation::GreaterEqual);
    assert_eq!(compiled.constraints[0].terms, vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: -2.0 }]);
}
#[test]
fn quadratic_rows_are_rejected_unless_they_cancel() {
    let mut m = model(x(0));
    m.constraints.push(ExpressionConstraint { name: "nonlinear row".into(), left: pow(x(0), 2), relation: Relation::LessEqual, right: c(4.0) });
    assert_eq!(compile(&m, &SolveOptions::default()).unwrap_err().code, "unsupported_constraint");
    m.constraints[0].right = add(pow(x(0), 2), c(4.0));
    assert!(compile(&m, &SolveOptions::default()).is_ok());
}
#[test]
fn invalid_handles_and_nonfinite_constants_are_rejected_before_classification() {
    assert_eq!(classify(&mul(x(0), x(10)), 1).unwrap_err().code, "invalid_variable");
    assert_eq!(classify(&c(f64::INFINITY), 0).unwrap_err().code, "non_finite");
    assert_eq!(evaluate(&x(0), &[f64::NAN]).unwrap_err().code, "non_finite");
}
#[test]
fn domain_errors_are_not_silently_zero() {
    for expr in [
        Expr::Divide { left: Box::new(c(1.0)), right: Box::new(c(0.0)) },
        Expr::Ln { value: Box::new(c(-1.0)) },
        Expr::Sqrt { value: Box::new(c(-1.0)) },
    ] {
        assert!(evaluate(&expr, &[]).is_err());
        assert!(classify(&expr, 0).is_err());
    }
}
#[test]
fn expression_depth_is_bounded() {
    let mut expr = x(0);
    for _ in 0..MAX_EXPRESSION_DEPTH {
        expr = Expr::Negate { value: Box::new(expr) };
    }
    assert_eq!(evaluate(&expr, &[1.0]).unwrap_err().code, "expression_limit");
    assert_eq!(classify(&expr, 1).unwrap_err().code, "expression_limit");
}
#[test]
fn compilation_rejects_too_small_memory_budget() {
    let options = SolveOptions { max_memory_bytes: 1, ..SolveOptions::default() };
    assert_eq!(compile(&model(x(0)), &options).unwrap_err().code, "memory_limit");
}
#[test]
fn tagged_json_roundtrip_evaluates_the_same_expression() {
    let expr = add(x(0), c(4.5));
    let text = serde_json::to_string(&expr).unwrap();
    assert!(text.contains("\"op\":\"add\""));
    let roundtrip: Expr = serde_json::from_str(&text).unwrap();
    assert_eq!(expr, roundtrip);
    assert_eq!(evaluate(&roundtrip, &[-2.0]).unwrap(), 2.5);
}
#[test]
fn constant_division_and_negative_coefficients_preserve_affine_structure() {
    let expr = Expr::Divide { left: Box::new(sub(c(5.0), mul(c(6.0), x(0)))), right: Box::new(c(2.0)) };
    assert_eq!(classify(&expr, 1).unwrap(), ExpressionClass::Affine);
    let m = compile(&model(expr), &SolveOptions::default()).unwrap();
    assert_eq!(m.objective.offset, 2.5);
    assert_eq!(m.objective.linear, vec![LinearTerm { variable: 0, coefficient: -3.0 }]);
}
