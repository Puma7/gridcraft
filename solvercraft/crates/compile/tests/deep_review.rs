#![allow(clippy::unwrap_used)]
use solvercraft_compile::{
    Expr, ExpressionClass, ExpressionConstraint, ExpressionModel, MAX_EXPRESSION_DEPTH, classify, compile, evaluate, evaluate_with_gradient,
};
use solvercraft_types::*;
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
fn div(a: Expr, b: Expr) -> Expr {
    Expr::Divide { left: Box::new(a), right: Box::new(b) }
}
fn pow(a: Expr, exponent: u32) -> Expr {
    Expr::Power { base: Box::new(a), exponent }
}
fn model(objective: Expr) -> ExpressionModel {
    ExpressionModel {
        name: "deep review".into(),
        variables: (0..2).map(|i| Variable { name: format!("v{i}"), lower: None, upper: None, kind: VariableKind::Continuous }).collect(),
        direction: Direction::Minimize,
        objective,
        constraints: vec![],
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-11 * (1.0 + a.abs().max(b.abs())), "{a} != {b}");
}
#[test]
fn seed_contract_roundtrips_all_64_bits_and_rejects_invalid_inputs() {
    for seed in [0, 1, (1u64 << 53) + 1, u64::MAX] {
        let options = SolveOptions { seed, ..SolveOptions::default() };
        let encoded = serde_json::to_value(&options).unwrap();
        assert_eq!(encoded["seed"], seed.to_string());
        let decoded: SolveOptions = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.seed, seed);
        let simulation = SimulationOptions { seed, ..SimulationOptions::default() };
        let encoded = serde_json::to_value(&simulation).unwrap();
        assert_eq!(encoded["seed"], seed.to_string());
    }
    for seed in [serde_json::json!(-1), serde_json::json!(1.25), serde_json::json!("18446744073709551616"), serde_json::json!("not-a-seed")] {
        assert!(serde_json::from_value::<SolveOptions>(serde_json::json!({"seed":seed})).is_err());
    }
    let default: SolveOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(default.seed, 0);
}
#[test]
fn independent_multivariable_gradient_covers_all_smooth_operators() {
    // f=sin(x)+cos(y)+exp(x*y)+ln(x+3)+sqrt(y+4)+(x-y)^3.
    let expr = add(
        add(
            add(Expr::Sin { value: Box::new(x(0)) }, Expr::Cos { value: Box::new(x(1)) }),
            add(Expr::Exp { value: Box::new(mul(x(0), x(1))) }, Expr::Ln { value: Box::new(add(x(0), c(3.0))) }),
        ),
        add(Expr::Sqrt { value: Box::new(add(x(1), c(4.0))) }, pow(sub(x(0), x(1)), 3)),
    );
    for [a, b] in [[0.3, 0.7], [-0.2, 1.5], [1.1, -0.5]] {
        let (value, gradient) = evaluate_with_gradient(&expr, &[a, b]).unwrap();
        let residual = (a - b).powi(2);
        close(value, a.sin() + b.cos() + (a * b).exp() + (a + 3.0).ln() + (b + 4.0).sqrt() + (a - b).powi(3));
        close(gradient[0], a.cos() + b * (a * b).exp() + 1.0 / (a + 3.0) + 3.0 * residual);
        close(gradient[1], -b.sin() + a * (a * b).exp() + 0.5 / (b + 4.0).sqrt() - 3.0 * residual);
    }
}
#[test]
fn reciprocal_negative_powers_and_selected_extrema_have_correct_gradients() {
    let expr = add(div(x(0), x(1)), pow(x(0), 5));
    let (value, g) = evaluate_with_gradient(&expr, &[-2.0, 4.0]).unwrap();
    close(value, -32.5);
    close(g[0], 80.25);
    close(g[1], 0.125);
    for expr in [
        Expr::Abs { value: Box::new(x(0)) },
        Expr::Minimum { left: Box::new(x(0)), right: Box::new(c(10.0)) },
        Expr::Maximum { left: Box::new(x(0)), right: Box::new(c(-10.0)) },
    ] {
        let (_, g) = evaluate_with_gradient(&expr, &[-2.0]).unwrap();
        assert_eq!(g[0], if matches!(expr, Expr::Abs { .. }) { -1.0 } else { 1.0 });
    }
}
#[test]
fn zero_power_has_consistent_value_and_zero_gradient_through_abs() {
    let expr = pow(Expr::Abs { value: Box::new(x(0)) }, 0);
    assert_eq!(evaluate(&expr, &[0.0]).unwrap(), 1.0);
    assert_eq!(evaluate_with_gradient(&expr, &[0.0]).unwrap(), (1.0, vec![0.0]));
}
#[test]
fn compiler_must_not_discard_a_domain_guard_that_evaluation_keeps() {
    let expr = pow(Expr::Ln { value: Box::new(x(0)) }, 0);
    let source = evaluate(&expr, &[-1.0, 0.0]);
    let compiled = compile(&model(expr), &SolveOptions::default());
    // Either all APIs implement a lazy constant power, or compilation must
    // preserve/reject the logarithm's domain. It cannot make an invalid point valid.
    assert!(source.is_ok() || compiled.is_err(), "compiled model lost the logarithm domain");
}
#[test]
fn finite_tiny_constant_division_remains_compilable() {
    let expr = div(c(1e-320), c(1e-320));
    assert_eq!(evaluate(&expr, &[]).unwrap(), 1.0);
    assert_eq!(classify(&expr, 0).unwrap(), ExpressionClass::Constant);
    let compiled = compile(&model(expr), &SolveOptions::default()).unwrap();
    assert_eq!(compiled.objective.offset, 1.0);
}
#[test]
fn affine_constraints_and_quadratic_cross_terms_match_independent_arithmetic() {
    let expr = mul(add(x(0), mul(c(2.0), x(1))), sub(x(0), mul(c(3.0), x(1))));
    let mut m = model(expr.clone());
    m.constraints.push(ExpressionConstraint {
        name: "residual".into(),
        left: add(mul(c(3.0), x(0)), c(5.0)),
        relation: Relation::LessEqual,
        right: sub(mul(c(2.0), x(1)), c(1.0)),
    });
    let compiled = compile(&m, &SolveOptions::default()).unwrap();
    for [a, b] in [[2.0, 3.0], [-1.0, 4.0], [0.0, 0.0]] {
        let expected = a * a - a * b - 6.0 * b * b;
        close(evaluate(&expr, &[a, b]).unwrap(), expected);
        close(solvercraft_model::evaluate_objective(&compiled, &[a, b]).unwrap(), expected);
    }
    assert_eq!(compiled.constraints[0].rhs, -6.0);
    assert_eq!(compiled.constraints[0].terms, vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: -2.0 }]);
}
#[test]
fn zero_power_still_validates_handles_and_finite_constants() {
    for expr in [pow(x(2), 0), pow(c(f64::NAN), 0)] {
        assert!(classify(&expr, 1).is_err());
        assert!(evaluate(&expr, &[1.0]).is_err());
        assert!(evaluate_with_gradient(&expr, &[1.0]).is_err());
    }
}
#[test]
fn depth_boundary_node_budget_and_invalid_inputs_fail_before_arithmetic() {
    let mut expr = x(0);
    for _ in 1..MAX_EXPRESSION_DEPTH {
        expr = Expr::Negate { value: Box::new(expr) };
    }
    assert!(evaluate(&expr, &[2.0]).is_ok());
    expr = Expr::Negate { value: Box::new(expr) };
    assert_eq!(evaluate(&expr, &[2.0]).unwrap_err().code, "expression_limit");
    assert_eq!(evaluate_with_gradient(&expr, &[2.0]).unwrap_err().code, "expression_limit");
    fn tree(depth: usize) -> Expr {
        if depth == 0 { c(0.0) } else { add(tree(depth - 1), tree(depth - 1)) }
    }
    // Balanced depth sixteen has 131071 nodes and remains shallow to drop.
    let wide = tree(16);
    assert_eq!(classify(&wide, 0).unwrap_err().code, "expression_limit");
    for values in [vec![f64::INFINITY], vec![f64::NAN]] {
        assert!(evaluate_with_gradient(&x(0), &values).is_err());
    }
}
#[test]
fn coefficient_sum_keeps_a_small_exact_residual() {
    let expr = sub(add(mul(c(1e16), x(0)), x(0)), mul(c(1e16), x(0)));
    let compiled = compile(&model(expr), &SolveOptions::default()).unwrap();
    assert_eq!(compiled.objective.linear, vec![LinearTerm { variable: 0, coefficient: 1.0 }]);
}
#[test]
fn min_max_ties_abs_zero_and_sqrt_zero_are_not_silently_differentiated() {
    for expr in
        [Expr::Abs { value: Box::new(x(0)) }, Expr::Sqrt { value: Box::new(x(0)) }, Expr::Maximum { left: Box::new(x(0)), right: Box::new(c(0.0)) }]
    {
        assert_eq!(evaluate_with_gradient(&expr, &[0.0]).unwrap_err().code, "nondifferentiable");
    }
}
#[test]
fn compensated_coefficients_survive_scale_division_and_quadratic_products() {
    for (expr, expected) in [
        (sub(mul(add(mul(c(1e16), x(0)), x(0)), c(2.0)), mul(c(2e16), x(0))), 2.0),
        (sub(div(add(mul(c(1e16), x(0)), x(0)), c(2.0)), mul(c(5e15), x(0))), 0.5),
    ] {
        let m = compile(&model(expr), &SolveOptions::default()).unwrap();
        assert_eq!(m.objective.linear, vec![LinearTerm { variable: 0, coefficient: expected }]);
    }
    let expr = sub(add(mul(c(1e16), mul(x(0), x(1))), mul(x(0), x(1))), mul(c(1e16), mul(x(0), x(1))));
    let m = compile(&model(expr), &SolveOptions::default()).unwrap();
    assert_eq!(m.objective.quadratic, vec![QuadraticTerm { left: 0, right: 1, coefficient: 1.0 }]);
}
#[test]
fn unknown_fields_are_rejected_at_each_expression_contract_boundary() {
    assert!(serde_json::from_value::<Expr>(serde_json::json!({"op":"variable","index":0,"typo":1})).is_err());
    let mut expression = serde_json::to_value(model(x(0))).unwrap();
    expression["typo"] = serde_json::json!(1);
    assert!(serde_json::from_value::<ExpressionModel>(expression).is_err());
    let row = serde_json::json!({"name":"r","left":{"op":"constant","value":0},"relation":"equal","right":{"op":"constant","value":0},"typo":1});
    assert!(serde_json::from_value::<ExpressionConstraint>(row).is_err());
}
#[test]
fn zero_power_preserves_base_value_domain_errors() {
    for expr in [pow(Expr::Ln { value: Box::new(x(0)) }, 0), pow(div(c(1.0), x(0)), 0)] {
        assert!(evaluate(&expr, &[0.0]).is_err());
        assert!(evaluate_with_gradient(&expr, &[0.0]).is_err());
    }
    let expr = pow(Expr::Ln { value: Box::new(x(0)) }, 0);
    assert_eq!(evaluate(&expr, &[2.0]).unwrap(), 1.0);
    assert_eq!(evaluate_with_gradient(&expr, &[2.0]).unwrap(), (1.0, vec![0.0]));
}
#[test]
fn reverse_gradient_does_not_erase_a_small_repeated_variable_contribution() {
    let expr = sub(add(mul(c(1e16), x(0)), x(0)), mul(c(1e16), x(0)));
    let (value, g) = evaluate_with_gradient(&expr, &[0.0]).unwrap();
    assert_eq!(value, 0.0);
    assert_eq!(g, vec![1.0]);
}
#[test]
fn invalid_metadata_is_rejected_before_domain_arithmetic_or_output_cloning() {
    let mut m = model(Expr::Ln { value: Box::new(c(-1.0)) });
    m.name = "x".repeat(solvercraft_model::MAX_NAME_BYTES + 1);
    assert_eq!(compile(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
    m.name = "valid".into();
    m.variables[0].name = "x".repeat(solvercraft_model::MAX_NAME_BYTES + 1);
    assert_eq!(compile(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
    let mut m = model(c(0.0));
    m.variables = (0..128).map(|i| Variable { name: format!("{i:0128}"), lower: None, upper: None, kind: VariableKind::Continuous }).collect();
    assert_eq!(compile(&m, &SolveOptions { max_memory_bytes: 4096, ..SolveOptions::default() }).unwrap_err().code, "memory_limit");
}
#[test]
fn high_u32_powers_are_bounded_and_do_not_wrap_signed_exponents() {
    // The worker deadline bounds the probe even if a future implementation
    // accidentally introduces exponent-sized repeated multiplication.
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let options = SolveOptions { max_memory_bytes: 4096, ..SolveOptions::default() };
        for exponent in [1_u32 << 31, (1_u32 << 31) + 1, u32::MAX] {
            let one = pow(c(1.0), exponent);
            assert_eq!(evaluate(&one, &[]).unwrap(), 1.0);
            assert_eq!(classify(&one, 0).unwrap(), ExpressionClass::Constant);
            assert_eq!(compile(&model(one.clone()), &options).unwrap().objective.offset, 1.0);
            assert_eq!(evaluate_with_gradient(&one, &[]).unwrap(), (1.0, vec![]));
            let negative = pow(c(-1.0), exponent);
            let parity = if exponent % 2 == 0 { 1.0 } else { -1.0 };
            assert_eq!(evaluate(&negative, &[]).unwrap(), parity);
            assert_eq!(compile(&model(negative), &options).unwrap().objective.offset, parity);
            let variable = pow(x(0), exponent);
            assert_eq!(classify(&variable, 1).unwrap(), ExpressionClass::Nonlinear);
            assert_eq!(compile(&model(variable.clone()), &options).unwrap_err().code, "unsupported_expression");
            assert_eq!(evaluate_with_gradient(&variable, &[1.0]).unwrap(), (1.0, vec![f64::from(exponent)]));
            assert_eq!(evaluate_with_gradient(&variable, &[0.0]).unwrap(), (0.0, vec![0.0]));
            let (value, g) = evaluate_with_gradient(&variable, &[-1.0]).unwrap();
            assert_eq!(value, parity);
            assert_eq!(g, vec![-parity * f64::from(exponent)]);
            let overflow = pow(c(2.0), exponent);
            assert_eq!(evaluate(&overflow, &[]).unwrap_err().code, "numeric_error");
            assert_eq!(classify(&overflow, 0).unwrap_err().code, "numeric_error");
            assert_eq!(evaluate_with_gradient(&overflow, &[]).unwrap_err().code, "numeric_error");
        }
        assert!(
            serde_json::from_value::<Expr>(serde_json::json!({"op":"power","base":{"op":"constant","value":1.0},"exponent":4294967296_u64})).is_err()
        );
        assert!(serde_json::from_value::<Expr>(serde_json::json!({"op":"power","base":{"op":"constant","value":1.0},"exponent":-1})).is_err());
        sender.send(()).unwrap();
    });
    let completed = receiver.recv_timeout(std::time::Duration::from_secs(5));
    assert!(completed.is_ok(), "bounded high-exponent worker failed or exceeded its five-second deadline");
    worker.join().unwrap();
}
