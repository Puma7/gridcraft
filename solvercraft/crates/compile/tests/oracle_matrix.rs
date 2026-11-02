//! Closed-form dyadic polynomial and differential oracles.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use solvercraft_compile::{Expr, ExpressionConstraint, ExpressionModel, compile, evaluate, evaluate_with_gradient, finite_difference_gradient};
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
fn square(a: Expr) -> Expr {
    Expr::Power { base: Box::new(a), exponent: 2 }
}
fn model(objective: Expr) -> ExpressionModel {
    ExpressionModel {
        name: "independent dyadic polynomial".into(),
        variables: (0..2).map(|i| Variable { name: format!("x{i}"), lower: Some(-4.0), upper: Some(4.0), kind: VariableKind::Continuous }).collect(),
        direction: Direction::Minimize,
        objective,
        constraints: vec![],
    }
}
#[test]
fn scalar_and_quadratic_multiplication_preserve_exact_product_residuals() {
    // (2^27+1)^2 = 2^54+2^28+1 exactly; binary64 multiplication
    // rounds away the final unit. Cancellation must restore that unit.
    let a = 134_217_729.0;
    let rounded = 18_014_398_777_917_440.0;
    for quadratic in [false, true] {
        let (left, right) = if quadratic {
            (mul(mul(c(a), x(0)), mul(c(a), x(0))), mul(c(rounded), square(x(0))))
        } else {
            (mul(mul(c(a), x(0)), c(a)), mul(c(rounded), x(0)))
        };
        let compiled = compile(&model(sub(left, right)), &SolveOptions::default()).unwrap();
        if quadratic {
            assert_eq!(compiled.objective.quadratic, vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }]);
        } else {
            assert_eq!(compiled.objective.linear, vec![LinearTerm { variable: 0, coefficient: 1.0 }]);
        }
    }
}
#[test]
fn dyadic_factoring_and_affine_row_translation_matrix() {
    let mut count = 0;
    for i in 0..1024 {
        let a = (i % 17) as f64 / 4.0 - 2.0;
        let b = (i % 13) as f64 / 4.0 - 1.5;
        let q = 1.0 + (i % 7) as f64 / 4.0;
        let r = 1.0 + (i % 5) as f64 / 2.0;
        let d = (i % 11) as f64 / 4.0 - 1.0;
        // f=q*(x-a)^2+r*(x-y-b)^2+d. Expected coefficients are
        // obtained directly from binomial identities, not the compiler.
        let objective = add(add(mul(c(q), square(sub(x(0), c(a)))), mul(c(r), square(sub(sub(x(0), x(1)), c(b))))), c(d));
        let objective = if i % 2 == 0 { div(mul(objective, c(8.0)), c(8.0)) } else { objective };
        let mut source = model(objective);
        source.constraints.push(ExpressionConstraint {
            name: "translated".into(),
            left: add(mul(c(q), add(x(0), c(a))), mul(c(r), sub(x(1), c(b)))),
            relation: Relation::LessEqual,
            right: add(c(d), mul(c(0.5), x(0))),
        });
        let compiled = compile(&source, &SolveOptions::default()).unwrap();
        let expected_linear = [-2.0 * q * a - 2.0 * r * b, 2.0 * r * b];
        let mut actual_linear = [0.0; 2];
        for t in &compiled.objective.linear {
            actual_linear[t.variable] += t.coefficient;
        }
        assert_eq!(actual_linear, expected_linear, "case={i}");
        let expected_quadratic = [q + r, -2.0 * r, r];
        let mut actual_quadratic = [0.0; 3];
        for t in &compiled.objective.quadratic {
            actual_quadratic[if t.left == t.right { if t.left == 0 { 0 } else { 2 } } else { 1 }] += t.coefficient;
        }
        assert_eq!(actual_quadratic, expected_quadratic, "case={i}");
        assert_eq!(compiled.objective.offset, q * a * a + r * b * b + d, "case={i}");
        let mut row = [0.0; 2];
        for t in &compiled.constraints[0].terms {
            row[t.variable] += t.coefficient;
        }
        assert_eq!(row, [q - 0.5, r], "case={i}");
        assert_eq!(compiled.constraints[0].rhs, d - q * a + r * b, "case={i}");
        let point = [(i % 19) as f64 / 8.0 - 1.0, (i % 23) as f64 / 8.0 - 1.5];
        let dx = point[0] - a;
        let dy = point[0] - point[1] - b;
        let expected = q * dx * dx + r * dy * dy + d;
        assert_eq!(evaluate(&source.objective, &point).unwrap(), expected, "case={i}");
        let (actual, gradient) = evaluate_with_gradient(&source.objective, &point).unwrap();
        assert_eq!(actual, expected, "case={i}");
        assert_eq!(gradient, vec![2.0 * q * dx + 2.0 * r * dy, -2.0 * r * dy], "case={i}");
        count += 1;
    }
    eprintln!("MATRIX compiler dyadic polynomial: cases={count}, failures=0");
}

#[test]
fn large_product_cancellation_matches_integer_arithmetic_matrix() {
    for i in 0..256 {
        let exponent = 27 + i % 19;
        let a = (1_i128 << exponent) + (2 * i + 1) as i128;
        let b = (1_i128 << exponent) + (2 * (i % 37) + 1) as i128;
        let rounded = a as f64 * b as f64;
        let expected = (a * b - rounded as i128) as f64;
        assert_ne!(expected, 0.0);
        for quadratic in [false, true] {
            let left = if quadratic { mul(mul(c(a as f64), x(0)), mul(c(b as f64), x(0))) } else { mul(mul(c(a as f64), x(0)), c(b as f64)) };
            let right = mul(c(rounded), if quadratic { square(x(0)) } else { x(0) });
            let compiled = compile(&model(sub(left, right)), &SolveOptions::default()).unwrap();
            if quadratic {
                assert_eq!(compiled.objective.quadratic, vec![QuadraticTerm { left: 0, right: 0, coefficient: expected }], "case={i}");
            } else {
                assert_eq!(compiled.objective.linear, vec![LinearTerm { variable: 0, coefficient: expected }], "case={i}");
            }
        }
    }
    eprintln!("MATRIX exact integer product cancellation: cases=512, failures=0");
}
#[test]
fn fixed_coefficient_capacity_fails_explicitly_instead_of_erasing_a_third_component() {
    // 2^120 + 2^60 + 1 needs three binary64 components. The bounded compiler
    // must reject this intermediate representation, even if later cancellation
    // would leave a representable unit. This is a rejection contract, not a
    // successfully optimized mathematical case.
    let a = 2.0_f64.powi(120);
    let b = 2.0_f64.powi(60);
    let expr = sub(sub(add(add(mul(c(a), x(0)), mul(c(b), x(0))), x(0)), mul(c(a), x(0))), mul(c(b), x(0)));
    assert_eq!(compile(&model(expr), &SolveOptions::default()).unwrap_err().code, "numeric_range");
}
#[test]
fn ordinary_decimal_product_compatibility_is_explicitly_measured() {
    // An independent integer-significand oracle decides whether the exact
    // product of the two input dyadics fits in binary64. No approximate
    // coefficient is silently substituted for an unrepresentable final term.
    let operands = [0.1_f64, 0.2, 0.3, 0.5, 1.0, 1.5, 2.0, 10.0];
    let mut exact = 0;
    let mut refused = 0;
    for a in operands {
        for b in operands {
            let mantissa = |v: f64| u128::from((v.to_bits() & ((1_u64 << 52) - 1)) | (1_u64 << 52));
            let product = mantissa(a) * mantissa(b);
            let significant = 128 - product.leading_zeros() - product.trailing_zeros();
            let result = compile(&model(mul(mul(c(a), x(0)), c(b))), &SolveOptions::default());
            if significant <= 53 {
                assert_eq!(result.unwrap().objective.linear, vec![LinearTerm { variable: 0, coefficient: a * b }], "{a} * {b}");
                exact += 1;
            } else {
                assert_eq!(result.unwrap_err().code, "numeric_range", "{a} * {b}");
                refused += 1;
            }
        }
    }
    eprintln!(
        "MATRIX decimal compatibility characterization: cases=64, exact_accepted={exact}, explicit_numeric_range={refused}; refusals are not mathematical successes"
    );
}

#[test]
fn finite_differences_at_bound_neighbors_keep_the_original_unit_derivative() {
    for i in 0..256 {
        let lower = -3.0_f64;
        let upper = 3.0_f64;
        let mut point = if i % 2 == 0 { lower } else { upper };
        for _ in 0..i % 4 {
            point = if i % 2 == 0 { point.next_up() } else { point.next_down() };
        }
        let center = if i % 2 == 0 { -2.0 } else { 2.0 };
        let q = [0.25, 0.5, 1.0, 2.0][i % 4] * (1.0 + (i / 16) as f64 / 16.0);
        let offset = [0.0, 16.0, 1024.0, 65536.0][(i / 4) % 4];
        let sign = if i % 7 == 0 { -1.0 } else { 1.0 };
        let result =
            finite_difference_gradient(&[point], &[Some(lower)], &[Some(upper)], 1e-6, |x| Ok(offset + sign * q * (x[0] - center).powi(2))).unwrap();
        let expected = sign * 2.0 * q * (point - center);
        assert!(
            (result.gradient[0] - expected).abs() < 2e-5 * (1.0 + expected.abs()),
            "case={i}, x={point}, offset={offset}, expected={expected}: {result:?}"
        );
    }
    eprintln!("MATRIX bound-neighbor finite differences: cases=256, failures=0");
}

fn quotient_identity(numerator: f64, divisor: f64) -> Expr {
    sub(mul(div(mul(c(numerator), x(0)), c(divisor)), c(divisor)), mul(c(numerator), x(0)))
}

#[test]
fn rounded_division_cannot_turn_an_identity_into_a_false_constraint() {
    let mut refusals = 0;
    for divisor in [-13.0, -11.0, -9.0, -7.0, -5.0, -3.0, 3.0, 5.0, 7.0, 9.0, 11.0, 13.0] {
        for numerator in [-1.0, 1.0] {
            let identity = quotient_identity(numerator, divisor);
            let mut source = model(c(0.0));
            source.variables[0].lower = Some(1.0);
            source.variables[0].upper = Some(2.0);
            for relation in [Relation::Equal, Relation::LessEqual, Relation::GreaterEqual] {
                source.constraints =
                    vec![ExpressionConstraint { name: "exact quotient identity".into(), left: identity.clone(), relation, right: c(0.0) }];
                let error = compile(&source, &SolveOptions::default()).unwrap_err();
                assert_eq!(error.code, "numeric_range", "numerator={numerator}, divisor={divisor}, relation={relation:?}");
                refusals += 1;
            }
            source.constraints.clear();
            source.objective = identity;
            assert_eq!(compile(&source, &SolveOptions::default()).unwrap_err().code, "numeric_range");
            refusals += 1;
        }
    }
    eprintln!(
        "MATRIX inexact division rejection contracts: cases={refusals}, explicit_numeric_range={refusals}; refusals are not mathematical successes"
    );
}

#[test]
fn exact_quotient_identities_keep_zero_objectives_and_rows() {
    let mut count = 0;
    for divisor in [-13.0, -11.0, -9.0, -7.0, -5.0, -3.0, 3.0, 5.0, 7.0, 9.0, 11.0, 13.0] {
        for factor in [-3.0, -1.0, 1.0, 4.0] {
            count += check_exact_identity(divisor * factor, divisor);
        }
    }
    for divisor in [-16.0, -8.0, -4.0, -2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
        for numerator in [-3.0, -1.0, 1.0, 3.0] {
            count += check_exact_identity(numerator, divisor);
        }
    }
    eprintln!("MATRIX exactly representable division identities: cases={count}, failures=0");
}

#[test]
fn subnormal_division_remainder_is_not_mistaken_for_exact_zero() {
    let numerator = f64::from_bits(2);
    let quotient = numerator / 1.5;
    assert_eq!((-quotient).mul_add(1.5, numerator), 0.0);
    // The true remainder is half the minimum subnormal: its rounded zero is
    // insufficient evidence of exact division. These are explicit refusals.
    for sign in [-1.0, 1.0] {
        let source = model(div(mul(c(sign * numerator), x(0)), c(1.5)));
        assert_eq!(compile(&source, &SolveOptions::default()).unwrap_err().code, "numeric_range");
    }
}

fn check_exact_identity(numerator: f64, divisor: f64) -> usize {
    let identity = quotient_identity(numerator, divisor);
    let mut source = model(c(0.0));
    source.variables[0].lower = Some(1.0);
    source.variables[0].upper = Some(2.0);
    for relation in [Relation::Equal, Relation::LessEqual, Relation::GreaterEqual] {
        source.constraints = vec![ExpressionConstraint { name: "exact quotient identity".into(), left: identity.clone(), relation, right: c(0.0) }];
        let compiled = compile(&source, &SolveOptions::default()).unwrap();
        assert!(compiled.constraints[0].terms.is_empty(), "numerator={numerator}, divisor={divisor}");
        assert_eq!(compiled.constraints[0].rhs, 0.0);
        assert_eq!(evaluate(&identity, &[1.0, 0.0]).unwrap(), 0.0);
        assert_eq!(evaluate(&identity, &[2.0, 0.0]).unwrap(), 0.0);
    }
    source.constraints.clear();
    source.objective = identity;
    let compiled = compile(&source, &SolveOptions::default()).unwrap();
    assert_eq!(compiled.objective.offset, 0.0);
    assert!(compiled.objective.linear.is_empty() && compiled.objective.quadratic.is_empty());
    4
}
