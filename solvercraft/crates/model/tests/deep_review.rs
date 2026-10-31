#![allow(clippy::unwrap_used)]
use solvercraft_model::{
    MAX_NAME_BYTES, MAX_TERMS, MAX_VARIABLES, ViolationKind, canonicalize, effective_bounds, evaluate_objective, validate, validate_solution,
};
use solvercraft_types::*;

fn variable(name: &str) -> Variable {
    Variable { name: name.into(), lower: None, upper: None, kind: VariableKind::Continuous }
}
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "independent review".into(),
        variables: vec![variable("x"), variable("y")],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 7.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: -2.0 }],
            quadratic: vec![QuadraticTerm { left: 0, right: 1, coefficient: 5.0 }],
        },
        constraints: vec![],
    }
}
#[test]
fn independently_computed_cross_objective_and_all_relation_residuals() {
    let mut m = model();
    // f(2,-3)=7+6+6-30=-11.
    assert_eq!(evaluate_objective(&m, &[2.0, -3.0]).unwrap(), -11.0);
    for (relation, rhs, expected) in [(Relation::LessEqual, 0.0, 5.0), (Relation::GreaterEqual, 9.0, 4.0), (Relation::Equal, -2.0, 7.0)] {
        m.constraints = vec![LinearConstraint {
            name: "row".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: -1.0 }],
            relation,
            rhs,
        }];
        let r = validate_solution(&m, &[2.0, -3.0], &SolveOptions::default()).unwrap();
        assert_eq!(r.max_violation, expected);
        assert!(r.violations.iter().any(|x| x.kind == ViolationKind::Constraint && x.amount == expected));
    }
}
#[test]
fn binary_intersections_and_integer_holes_are_not_relaxed() {
    let binary = Variable { kind: VariableKind::Binary, lower: Some(0.2), upper: Some(0.8), ..variable("b") };
    assert_eq!(effective_bounds(&binary).unwrap_err().code, "infeasible_bounds");
    let integer = Variable { kind: VariableKind::Integer, lower: Some(-1.9), upper: Some(-1.1), ..variable("z") };
    assert_eq!(effective_bounds(&integer).unwrap_err().code, "infeasible_bounds");
    let integer = Variable { lower: Some(-2.0), ..integer };
    assert_eq!(effective_bounds(&integer).unwrap(), (Some(-2.0), Some(-1.1)));
}
#[test]
fn every_nonfinite_numeric_position_is_rejected() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..6 {
            let mut m = model();
            match field {
                0 => m.variables[0].lower = Some(bad),
                1 => m.variables[0].upper = Some(bad),
                2 => m.objective.offset = bad,
                3 => m.objective.linear[0].coefficient = bad,
                4 => m.objective.quadratic[0].coefficient = bad,
                _ => m.constraints = vec![LinearConstraint { name: "bad".into(), terms: vec![], relation: Relation::Equal, rhs: bad }],
            }
            assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "non_finite");
        }
    }
}
#[test]
fn exact_zero_and_nonzero_tiny_coefficients_survive_the_right_way() {
    let mut m = model();
    m.objective.linear = vec![
        LinearTerm { variable: 0, coefficient: 1.0 },
        LinearTerm { variable: 0, coefficient: -1.0 },
        LinearTerm { variable: 1, coefficient: 1e-300 },
    ];
    m.objective.quadratic = vec![QuadraticTerm { left: 0, right: 1, coefficient: 2.0 }, QuadraticTerm { left: 1, right: 0, coefficient: -2.0 }];
    let c = canonicalize(&m, &SolveOptions::default()).unwrap();
    assert_eq!(c.objective.linear, vec![LinearTerm { variable: 1, coefficient: 1e-300 }]);
    assert!(c.objective.quadratic.is_empty());
}
#[test]
fn invalid_zero_coefficient_handles_are_still_invalid() {
    let mut m = model();
    m.objective.linear = vec![LinearTerm { variable: usize::MAX, coefficient: 0.0 }];
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "invalid_variable");
    assert_eq!(evaluate_objective(&m, &[0.0, 0.0]).unwrap_err().code, "invalid_variable");
}
#[test]
fn variable_and_name_limits_are_checked() {
    let mut m = model();
    m.variables = vec![variable(""); MAX_VARIABLES + 1];
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
    m = model();
    m.variables[0].name = "x".repeat(MAX_NAME_BYTES + 1);
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
    m = model();
    m.constraints = vec![LinearConstraint { name: "x".repeat(MAX_NAME_BYTES + 1), terms: vec![], relation: Relation::Equal, rhs: 0.0 }];
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
}
#[test]
fn excessive_term_count_is_rejected_before_normalization() {
    let mut m = model();
    m.objective.linear = vec![LinearTerm { variable: 0, coefficient: 0.0 }; MAX_TERMS + 1];
    assert_eq!(canonicalize(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
}
#[test]
fn option_domains_and_original_integer_validation_are_strict() {
    for tolerance in [-1.0, f64::INFINITY, f64::NAN] {
        let o = SolveOptions { feasibility_tolerance: tolerance, ..SolveOptions::default() };
        assert!(validate(&model(), &o).is_err());
    }
    let mut m = model();
    m.variables[0].kind = VariableKind::Integer;
    let result = validate_solution(&m, &[0.125, 0.0], &SolveOptions::default()).unwrap();
    assert!(!result.feasible);
    assert!(result.violations.iter().any(|v| v.kind == ViolationKind::Integrality));
}
#[test]
fn canonicalization_does_not_erase_a_representable_residual_coefficient() {
    let mut m = model();
    m.objective.offset = 0.0;
    m.objective.quadratic.clear();
    m.objective.linear = vec![
        LinearTerm { variable: 0, coefficient: 1e16 },
        LinearTerm { variable: 0, coefficient: 1.0 },
        LinearTerm { variable: 0, coefficient: -1e16 },
    ];
    let c = canonicalize(&m, &SolveOptions::default()).unwrap();
    // Sum of these exactly represented input coefficients is exactly 1.
    assert_eq!(c.objective.linear, vec![LinearTerm { variable: 0, coefficient: 1.0 }]);
}
#[test]
fn original_objective_dot_offset_and_rhs_share_compensation() {
    let mut m = model();
    m.variables = vec![variable("a"), variable("b"), variable("c")];
    m.objective = Objective {
        direction: Direction::Minimize,
        offset: 0.0,
        linear: vec![
            LinearTerm { variable: 0, coefficient: 1e16 },
            LinearTerm { variable: 1, coefficient: 1.0 },
            LinearTerm { variable: 2, coefficient: -1e16 },
        ],
        quadratic: vec![],
    };
    assert_eq!(evaluate_objective(&m, &[1.0, 1.0, 1.0]).unwrap(), 1.0);
    m.objective.offset = -1e16;
    m.objective.linear.pop();
    assert_eq!(evaluate_objective(&m, &[1.0, 1.0, 1.0]).unwrap(), 1.0);
    m.constraints =
        vec![LinearConstraint { name: "rhs cancellation".into(), terms: m.objective.linear.clone(), relation: Relation::LessEqual, rhs: 1e16 }];
    let r = validate_solution(&m, &[1.0, 1.0, 1.0], &SolveOptions::default()).unwrap();
    assert!(!r.feasible);
    assert_eq!(r.max_violation, 1.0);
}
#[test]
fn shared_compensated_helpers_are_checked_and_preserve_cancellation() {
    assert_eq!(solvercraft_model::compensated_sum([1e16, 1.0, -1e16]).unwrap(), 1.0);
    assert_eq!(solvercraft_model::compensated_dot(&[1e16, 1.0, -1e16], &[1.0, 1.0, 1.0]).unwrap(), 1.0);
    assert!(solvercraft_model::compensated_dot(&[1.0], &[]).is_err());
    assert!(solvercraft_model::compensated_sum([f64::INFINITY]).is_err());
}
#[test]
fn integer_precision_policy_does_not_restrict_large_continuous_values() {
    let limit = solvercraft_model::INTEGER_PRECISION_LIMIT;
    for value in [limit, -limit] {
        let mut m = model();
        m.variables[0].kind = VariableKind::Integer;
        m.variables[0].lower = Some(value);
        assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "unsupported_integer_precision");
        m.variables[0].lower = None;
        assert_eq!(validate_solution(&m, &[value, 0.0], &SolveOptions::default()).unwrap_err().code, "unsupported_integer_precision");
        m.variables[0].kind = VariableKind::Continuous;
        assert!(validate_solution(&m, &[value, 0.0], &SolveOptions::default()).unwrap().feasible);
    }
}
