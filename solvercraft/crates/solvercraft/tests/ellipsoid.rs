use solvercraft::{ellipsoid::*, types::*};
use std::sync::Arc;
fn base() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "ellipsoid".into(),
        variables: vec![
            Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous },
            Variable { name: "y".into(), lower: Some(0.6), upper: Some(0.6), kind: VariableKind::Continuous },
        ],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint { name: "uncertain coefficient row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 1.0 }],
    }
}
#[test]
fn ellipsoid_counterpart_is_a_true_norm_constraint_and_matches_circle_oracle() {
    let model = base();
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0, 0.0], vec![0.0, 1.0]], radius: 1.0 }];
    let conic = solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions::default()).expect("compile");
    assert_eq!(conic.second_order_cones.len(), 1);
    assert_eq!(conic.second_order_cones[0].components.len(), 2);
    let result = solvercraft::engines::solve_socp_owned(&conic, &SolveOptions::default(), Arc::new(NoControl)).expect("cone");
    let values = result.result.values.expect("values");
    assert!((values[0] - 0.8).abs() < 1e-5);
    assert!((values[0].hypot(values[1]) - 1.0).abs() < 1e-6);
    assert_eq!(result.result.optimality, Optimality::Optimal);
}
#[test]
fn ellipsoid_correlated_factor_matches_analytic_support() {
    let mut model = base();
    model.variables[0].lower = Some(2.0);
    model.variables[0].upper = Some(2.0);
    model.variables[1].lower = Some(-3.0);
    model.variables[1].upper = Some(-3.0);
    // Lx=[2x+y,y]=[1,-3], support radius2*sqrt(10).
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![2.0, 1.0], vec![0.0, 1.0]], radius: 2.0 }];
    model.constraints[0].rhs = 2.0 * 10.0_f64.sqrt();
    let conic = solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions::default()).expect("compile");
    let result = solvercraft::engines::solve_socp_owned(&conic, &SolveOptions::default(), Arc::new(NoControl)).expect("solve");
    assert_eq!(result.result.feasibility, Feasibility::Feasible);
    model.constraints[0].rhs -= 0.1;
    let conic = solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions::default()).expect("compile");
    let result = solvercraft::engines::solve_socp_owned(&conic, &SolveOptions::default(), Arc::new(NoControl)).expect("solve");
    assert_ne!(result.result.feasibility, Feasibility::Feasible);
}
#[test]
fn ellipsoid_rejects_bad_shape_radius_bounds_and_memory() {
    let model = base();
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0]], radius: 1.0 }];
    assert!(solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions::default()).is_err());
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0, 0.0]], radius: -1.0 }];
    assert!(solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions::default()).is_err());
    let uncertainty = [EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0, 0.0]], radius: 1.0 }];
    assert_eq!(
        solvercraft::ellipsoid::compile(&model, &uncertainty, &SolveOptions { max_memory_bytes: 1, ..Default::default() }).expect_err("memory").code,
        "memory_limit"
    );
}
