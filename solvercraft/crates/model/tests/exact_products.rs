#![allow(clippy::expect_used)]
use solvercraft_model::{
    CompensatedSum, ExactProducts, MAX_TERMS, canonicalize, compensated_dot, evaluate_linear_residual, evaluate_objective,
    evaluate_working_objective, validate_solution,
};
use solvercraft_types::*;
fn add_factors(sum: &mut ExactProducts, factors: &[f64]) -> Result<()> {
    match factors {
        [a] => sum.add(*a),
        [a, b] => sum.add_product(*a, *b),
        [a, b, c] => sum.add_triple(*a, *b, *c),
        _ => Err(SolverError::new("test", "invalid fixture arity")),
    }
}
#[test]
fn python_fraction_oracle_fixtures_match_all_result_bits_and_permutations() {
    let mut count = 0;
    for line in include_str!("data/exact_products.txt").lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
        let (header, input) = line.split_once(':').expect("fixture header");
        let mut fields = header.split_whitespace();
        let name = fields.next().expect("name");
        let expected = fields.next().expect("oracle");
        let terms = input
            .trim()
            .split(';')
            .map(|term| term.split(',').map(|raw| f64::from_bits(u64::from_str_radix(raw, 16).expect("bits"))).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        for reversed in [false, true] {
            let mut sum = ExactProducts::default();
            if reversed {
                for term in terms.iter().rev() {
                    add_factors(&mut sum, term).expect("bounded finite inputs");
                }
            } else {
                for term in &terms {
                    add_factors(&mut sum, term).expect("bounded finite inputs");
                }
            }
            let result = sum.finish();
            if expected == "overflow" {
                assert!(result.is_err_and(|e| e.code == "numeric_range"), "{name}, reversed{reversed}");
            } else {
                assert_eq!(
                    result.expect("finite answer").to_bits(),
                    u64::from_str_radix(expected, 16).expect("oracle bits"),
                    "{name}, reversed{reversed}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 1619);
}
fn fixed_model() -> OptimizationModel {
    let x = 1.0 + f64::EPSILON;
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "exact dyadic input oracle".into(),
        variables: vec![
            Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(x), upper: Some(x) },
            Variable { name: "y".into(), kind: VariableKind::Continuous, lower: Some(1.0), upper: Some(1.0) },
        ],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![
                LinearTerm { variable: 0, coefficient: 9_007_199_254_740_991.0 },
                LinearTerm { variable: 1, coefficient: -9_007_199_254_740_992.0 },
            ],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}
#[test]
fn original_objective_and_row_keep_product_residual_before_cancellation() -> Result<()> {
    let mut model = fixed_model();
    let point = [1.0 + f64::EPSILON, 1.0];
    let expected = 1.0 - f64::EPSILON;
    assert_eq!(evaluate_objective(&model, &point)?, expected);
    assert_eq!(evaluate_working_objective(&model, &point)?, expected);
    assert_eq!(compensated_dot(&[9_007_199_254_740_991.0, -9_007_199_254_740_992.0], &point)?, expected);
    assert_eq!(evaluate_linear_residual(&model.objective.linear, &point, 0.0)?, expected);
    model.constraints.push(LinearConstraint {
        name: "dyadic violation".into(),
        terms: model.objective.linear.clone(),
        relation: Relation::LessEqual,
        rhs: 0.0,
    });
    let checked = validate_solution(&model, &point, &SolveOptions::default())?;
    assert!(!checked.feasible);
    assert_eq!(checked.max_violation, expected);
    Ok(())
}
#[test]
fn quadratic_products_cancel_before_intermediate_overflow_or_underflow() -> Result<()> {
    let mut model = fixed_model();
    model.variables[0].lower = None;
    model.variables[0].upper = None;
    model.variables[1].lower = None;
    model.variables[1].upper = None;
    model.objective.linear.clear();
    model.objective.offset = 1.0;
    model.objective.quadratic =
        vec![QuadraticTerm { left: 0, right: 1, coefficient: f64::MAX }, QuadraticTerm { left: 1, right: 0, coefficient: -f64::MAX }];
    assert_eq!(evaluate_objective(&model, &[f64::MAX, f64::MAX])?, 1.0);
    let mut sum = ExactProducts::default();
    for _ in 0..1024 {
        sum.add_product(f64::from_bits(1), 2.0_f64.powi(-10))?;
    }
    assert_eq!(sum.finish()?.to_bits(), 1);
    Ok(())
}
#[test]
fn zero_sign_rounding_limits_and_error_poison_are_explicit() -> Result<()> {
    let mut negative_tiny = ExactProducts::default();
    negative_tiny.add_product(-f64::from_bits(1), 0.5)?;
    assert_eq!(negative_tiny.finish()?.to_bits(), (-0.0_f64).to_bits());
    let mut cancellation = ExactProducts::default();
    cancellation.add(f64::MAX)?;
    cancellation.add(-f64::MAX)?;
    assert_eq!(cancellation.finish()?.to_bits(), 0);
    let mut bad = ExactProducts::default();
    assert!(bad.add_product(0.0, f64::INFINITY).is_err());
    assert!(bad.finish().is_err());
    let mut count = ExactProducts::default();
    for _ in 0..=MAX_TERMS {
        count.add(0.0)?;
    }
    assert!(count.add(0.0).is_err());
    assert!(count.finish().is_err());
    assert_eq!(std::mem::size_of::<CompensatedSum>(), 16);
    assert!(std::mem::size_of::<ExactProducts>() < 1800);
    Ok(())
}
#[test]
fn maximum_supported_term_carry_and_full_exponent_span_are_exact() -> Result<()> {
    let mut sum = ExactProducts::default();
    for _ in 0..MAX_TERMS / 2 {
        sum.add_triple(f64::MAX, f64::MAX, f64::MAX)?;
        sum.add_triple(-f64::MAX, f64::MAX, f64::MAX)?;
    }
    sum.add(f64::from_bits(1))?;
    assert_eq!(sum.finish()?.to_bits(), 1);
    Ok(())
}
#[test]
fn duplicate_coefficient_rounding_is_never_silently_a_different_model() -> Result<()> {
    let mut model = fixed_model();
    model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 1e16 }, LinearTerm { variable: 0, coefficient: 1.0 }];
    assert!(canonicalize(&model, &SolveOptions::default()).is_err_and(|e| e.code == "numeric_range"));
    model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 0.1 }, LinearTerm { variable: 0, coefficient: 0.2 }];
    assert!(canonicalize(&model, &SolveOptions::default()).is_err_and(|e| e.code == "numeric_range"));
    model.objective.linear = vec![
        LinearTerm { variable: 0, coefficient: f64::MAX },
        LinearTerm { variable: 0, coefficient: f64::MAX },
        LinearTerm { variable: 0, coefficient: -f64::MAX },
    ];
    assert_eq!(canonicalize(&model, &SolveOptions::default())?.objective.linear[0].coefficient, f64::MAX);
    model.objective.linear.clear();
    model.objective.quadratic = vec![QuadraticTerm { left: 0, right: 1, coefficient: 0.1 }, QuadraticTerm { left: 1, right: 0, coefficient: 0.2 }];
    assert!(canonicalize(&model, &SolveOptions::default()).is_err_and(|e| e.code == "numeric_range"));
    model.objective.quadratic.clear();
    model.constraints = vec![LinearConstraint {
        name: "row".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 0.1 }, LinearTerm { variable: 0, coefficient: 0.2 }],
        relation: Relation::Equal,
        rhs: 1.0,
    }];
    assert!(canonicalize(&model, &SolveOptions::default()).is_err_and(|e| e.code == "numeric_range"));
    Ok(())
}
#[test]
fn outward_enclosures_and_exact_sign_do_not_invent_underflowed_zero() -> Result<()> {
    let tiny = f64::from_bits(1);
    let mut sum = ExactProducts::default();
    sum.add_triple(tiny, tiny, tiny)?;
    assert_eq!(sum.exact_sign()?, std::cmp::Ordering::Greater);
    assert_eq!(sum.clone().finish()?.to_bits(), 0);
    assert_eq!(sum.enclosure()?, (0.0, tiny));
    let mut negative = ExactProducts::default();
    negative.add_triple(-tiny, tiny, tiny)?;
    assert_eq!(negative.exact_sign()?, std::cmp::Ordering::Less);
    assert_eq!(negative.enclosure()?, (-tiny, -0.0));
    let mut midway = ExactProducts::default();
    midway.add(1.0)?;
    midway.add(2.0_f64.powi(-53))?;
    assert_eq!(midway.enclosure()?, (1.0, 1.0 + f64::EPSILON));
    let mut maximum = ExactProducts::default();
    maximum.add(f64::MAX)?;
    maximum.add(1.0)?;
    assert_eq!(maximum.clone().finish()?, f64::MAX);
    assert!(maximum.enclosure().is_err());
    Ok(())
}
#[test]
fn every_finite_exponent_identity_and_sign_roundtrip_is_preserved() -> Result<()> {
    for exponent in 0_u64..2047 {
        for fraction in [0, 1, (1_u64 << 51) - 1, 1_u64 << 51, (1_u64 << 52) - 1] {
            for sign in [0_u64, 1_u64 << 63] {
                let bits = sign | (exponent << 52) | fraction;
                let value = f64::from_bits(bits);
                let mut sum = ExactProducts::default();
                sum.add(value)?;
                assert_eq!(sum.finish()?.to_bits(), if value == 0.0 { 0 } else { bits });
            }
        }
    }
    Ok(())
}
