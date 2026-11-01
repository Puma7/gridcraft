#![allow(clippy::unwrap_used)]
use super::*;
use solvercraft_types::{Direction, LinearConstraint, Objective, VariableKind};

fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "original model".into(),
        variables: vec![
            Variable { name: "x".into(), lower: None, upper: Some(2.0), kind: VariableKind::Continuous },
            Variable { name: "y".into(), lower: Some(-3.0), upper: Some(-3.0), kind: VariableKind::Integer },
        ],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 2.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 3.0 }],
            quadratic: vec![QuadraticTerm { left: 0, right: 1, coefficient: 2.0 }],
        },
        constraints: vec![LinearConstraint {
            name: "row".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            relation: Relation::Equal,
            rhs: -4.0,
        }],
    }
}

#[test]
fn negative_free_and_fixed_variables_are_valid() {
    let m = model();
    assert!(validate(&m, &SolveOptions::default()).is_ok());
    let result = validate_solution(&m, &[-1.0, -3.0], &SolveOptions::default()).unwrap();
    assert!(result.feasible);
    assert_eq!(result.max_violation, 0.0);
    assert_eq!(result.objective, 5.0);
}

#[test]
fn original_rows_bounds_and_integrality_are_checked_separately() {
    let result = validate_solution(&model(), &[3.0, -2.5], &SolveOptions::default()).unwrap();
    assert!(!result.feasible);
    assert!(result.violations.iter().any(|v| v.kind == ViolationKind::UpperBound));
    assert!(result.violations.iter().any(|v| v.kind == ViolationKind::Integrality));
    assert!(result.violations.iter().any(|v| v.kind == ViolationKind::Constraint));
}
#[test]
fn binary_defaults_are_real_domain_bounds() {
    let v = Variable { name: "b".into(), lower: None, upper: None, kind: VariableKind::Binary };
    assert_eq!(effective_bounds(&v).unwrap(), (Some(0.0), Some(1.0)));
    let v = Variable { lower: Some(-2.0), upper: Some(10.0), ..v };
    assert_eq!(effective_bounds(&v).unwrap(), (Some(0.0), Some(1.0)));
}
#[test]
fn empty_and_integer_empty_domains_are_detected() {
    let mut m = model();
    m.variables[0].lower = Some(3.0);
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "infeasible_bounds");
    m.variables[0].lower = None;
    m.variables[1].lower = Some(0.2);
    m.variables[1].upper = Some(0.8);
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "infeasible_bounds");
}
#[test]
fn schema_handles_and_nonfinite_input_are_rejected() {
    let mut m = model();
    m.schema_version += 1;
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "unsupported_schema");
    m.schema_version = SCHEMA_VERSION;
    m.constraints[0].terms[0].variable = 2;
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "invalid_variable");
    m.constraints[0].terms[0].variable = 0;
    m.objective.linear[0].coefficient = f64::NAN;
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "non_finite");
}
#[test]
fn numeric_options_are_validated_but_zero_work_budgets_are_allowed() {
    let mut o = SolveOptions { max_evaluations: 0, max_iterations: 0, max_nodes: 0, ..SolveOptions::default() };
    assert!(validate_options(&o).is_ok());
    o.integrality_tolerance = 0.5;
    assert!(validate_options(&o).is_err());
    o.integrality_tolerance = 0.0;
    o.relative_gap = f64::INFINITY;
    assert!(validate_options(&o).is_err());
}
#[test]
fn insufficient_memory_and_oversized_names_are_rejected() {
    let o = SolveOptions { max_memory_bytes: 1, ..SolveOptions::default() };
    assert_eq!(validate(&model(), &o).unwrap_err().code, "memory_limit");
    let mut m = model();
    m.name = "x".repeat(MAX_NAME_BYTES + 1);
    assert_eq!(validate(&m, &SolveOptions::default()).unwrap_err().code, "model_limit");
}
#[test]
fn duplicate_and_symmetric_quadratic_coefficients_aggregate() {
    let mut m = model();
    m.objective.linear.extend([LinearTerm { variable: 0, coefficient: 2.0 }, LinearTerm { variable: 0, coefficient: -5.0 }]);
    m.objective.quadratic.push(QuadraticTerm { left: 1, right: 0, coefficient: 4.0 });
    m.constraints[0].terms.push(LinearTerm { variable: 0, coefficient: -1.0 });
    let c = canonicalize(&m, &SolveOptions::default()).unwrap();
    assert!(c.objective.linear.is_empty());
    assert_eq!(c.objective.quadratic, vec![QuadraticTerm { left: 0, right: 1, coefficient: 6.0 }]);
    assert_eq!(c.constraints[0].terms, vec![LinearTerm { variable: 1, coefficient: 1.0 }]);
    assert_eq!(evaluate_objective(&m, &[-1.0, -3.0]).unwrap(), evaluate_objective(&c, &[-1.0, -3.0]).unwrap());
}
#[test]
fn candidate_rounding_requires_revalidation() {
    let mut m = model();
    m.variables = vec![
        Variable { name: "x".into(), lower: Some(0.0), upper: Some(10.0), kind: VariableKind::Continuous },
        Variable { name: "y".into(), lower: None, upper: None, kind: VariableKind::Binary },
    ];
    m.objective = Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] };
    m.constraints = vec![LinearConstraint {
        name: "big M".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: -1e7 }],
        relation: Relation::LessEqual,
        rhs: 0.0,
    }];
    let o = SolveOptions { integrality_tolerance: 1e-6, ..SolveOptions::default() };
    assert!(validate_solution(&m, &[1.0, 1e-7], &o).unwrap().feasible);
    assert!(!validate_solution(&m, &[1.0, 0.0], &o).unwrap().feasible);
}
#[test]
fn lengths_nonfinite_values_and_overflow_are_structured_errors() {
    assert_eq!(evaluate_objective(&model(), &[]).unwrap_err().code, "invalid_values");
    assert_eq!(evaluate_objective(&model(), &[f64::NAN, 0.0]).unwrap_err().code, "non_finite");
    let mut m = model();
    m.objective.linear[0].coefficient = 1e308;
    assert_eq!(evaluate_objective(&m, &[10.0, 0.0]).unwrap_err().code, "numeric_range");
}
#[test]
fn large_finite_objective_is_valid_arithmetic() {
    let mut m = model();
    m.objective =
        Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![LinearTerm { variable: 0, coefficient: -1e16 }], quadratic: vec![] };
    assert_eq!(evaluate_objective(&m, &[1.0, -3.0]).unwrap(), -1e16);
}
