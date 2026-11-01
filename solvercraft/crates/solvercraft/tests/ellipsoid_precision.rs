#![allow(clippy::expect_used)]
use solvercraft::{ellipsoid::*, types::*};

fn cancellation_fixture() -> (OptimizationModel, Vec<EllipsoidUncertainty>, Vec<f64>) {
    let epsilon = 2.0_f64.powi(-52);
    let large = 2.0_f64.powi(104);
    let values = vec![large, -(large + 2.0_f64.powi(52)), -1.0];
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "ellipsoid formation cancellation".into(),
        variables: values
            .iter()
            .enumerate()
            .map(|(i, value)| Variable { name: format!("x{i}"), kind: VariableKind::Continuous, lower: Some(*value), upper: Some(*value) })
            .collect(),
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: vec![LinearConstraint { name: "robust row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 0.0 }],
    };
    let uncertainty = vec![EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0 + epsilon, 1.0, 1.0]], radius: 1.0 - epsilon }];
    (model, uncertainty, values)
}

#[test]
fn ellipsoid_coefficient_formation_never_accepts_a_material_original_violation() {
    let (model, uncertainty, values) = cancellation_fixture();
    let mut original = solvercraft::model::ExactProducts::default();
    for (coefficient, value) in uncertainty[0].factor[0].iter().zip(&values) {
        original.add_triple(uncertainty[0].radius, *coefficient, *value).expect("exact original triple");
    }
    assert_eq!(original.finish().expect("original support").abs(), 1.0 - 2.0_f64.powi(-52));
    let original = validate_original(&model, &uncertainty, &values, &SolveOptions::default()).expect("original-domain validation");
    assert!(!original.feasible);
    assert!(original.max_violation > 0.99);
    match compile(&model, &uncertainty, &SolveOptions::default()) {
        Err(error) => assert_eq!(error.code, "numeric_range"),
        Ok(counterpart) => {
            let checked = solvercraft::engines::validate_conic_solution(&counterpart, &values, &SolveOptions::default()).expect("counterpart check");
            assert!(!checked.0, "Rounded radius*factor erased a robust violation of almost one: {checked:?}");
        }
    }
}

#[test]
fn ordinary_decimal_ellipsoid_products_keep_an_outward_relaxation_and_checked_solution() -> Result<()> {
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "ordinary decimal ellipsoid".into(),
        variables: vec![Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: Some(2.0) }],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint { name: "robust row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 0.03 }],
    };
    let uncertainty = vec![EllipsoidUncertainty { constraint: 0, factor: vec![vec![0.1]], radius: 0.3 }];
    let options = SolveOptions::default();
    let counterpart = compile(&model, &uncertainty, &options)?;
    assert!(counterpart.second_order_cones[0].upper.offset > model.constraints[0].rhs);
    let solved = solve_owned(&model, &uncertainty, &options, std::sync::Arc::new(NoControl))?;
    assert_eq!(solved.result.result.feasibility, Feasibility::Feasible);
    let values = solved.result.result.values.as_ref().expect("checked values");
    assert!((values[0] - 1.0).abs() < 1e-5);
    assert!(validate_original(&model, &uncertainty, values, &options)?.feasible);
    if let Some(bound) = solved.result.result.best_bound {
        assert!(bound >= 1.0 - 1e-10, "A maximization relaxation must retain an upper bound");
    }
    let mut free = model;
    free.variables[0].upper = None;
    assert!(compile(&free, &uncertainty, &options).is_err_and(|e| e.code == "numeric_range"));
    Ok(())
}

#[test]
fn checked_ellipsoid_solution_keeps_two_component_support_and_original_validation() -> Result<()> {
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "checked circle".into(),
        variables: vec![
            Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(-2.0), upper: Some(2.0) },
            Variable { name: "y".into(), kind: VariableKind::Continuous, lower: Some(0.6), upper: Some(0.6) },
        ],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint { name: "circle".into(), terms: vec![], relation: Relation::LessEqual, rhs: 1.0 }],
    };
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0, 0.0], vec![0.0, 1.0]], radius: 1.0 }];
    let options = SolveOptions::default();
    assert!(validate_original(&model, &uncertainty, &[0.8, 0.6], &options)?.feasible);
    let outside = validate_original(&model, &uncertainty, &[0.9, 0.6], &options)?;
    assert!(!outside.feasible);
    assert!(outside.max_violation > 0.08);
    let checked = solve_owned(&model, &uncertainty, &options, std::sync::Arc::new(NoControl))?;
    assert_eq!(checked.result.result.feasibility, Feasibility::Feasible);
    assert_eq!(checked.result.result.optimality, Optimality::Optimal);
    let values = checked.result.result.values.as_ref().expect("original feasible point");
    assert!((values[0] - 0.8).abs() < 1e-5);
    assert!(validate_original(&model, &uncertainty, values, &options)?.feasible);
    assert!(checked.result.result.best_bound.is_some_and(|upper| upper >= 0.8 - 1e-10));
    Ok(())
}
