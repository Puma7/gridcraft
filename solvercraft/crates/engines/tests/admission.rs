use solvercraft_engines::{AffineExpression, ConicModel, SecondOrderConeConstraint, solve_mixed_conic_owned, solve_owned};
use solvercraft_types::*;
use std::sync::Arc;

fn integer_quadratic() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "admission".into(),
        variables: vec![Variable { name: "x".into(), kind: VariableKind::Binary, lower: None, upper: None }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![],
            quadratic: vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }],
        },
        constraints: vec![],
    }
}

#[test]
fn owned_integer_quadratic_rejects_invalid_options_and_preserves_infeasible_domain_status() -> Result<()> {
    let mut model = integer_quadratic();
    let options = SolveOptions { max_memory_bytes: 1, ..Default::default() };
    assert!(solve_owned(&model, &options, Arc::new(NoControl)).is_err_and(|e| e.code == "memory_limit"));
    let options = SolveOptions { optimality_tolerance: f64::NAN, ..Default::default() };
    assert!(solve_owned(&model, &options, Arc::new(NoControl)).is_err());
    model.variables[0].lower = Some(0.25);
    model.variables[0].upper = Some(0.75);
    let result = solve_owned(&model, &SolveOptions::default(), Arc::new(NoControl))?;
    assert_eq!(result.termination, Termination::Completed);
    assert_eq!(result.feasibility, Feasibility::Infeasible);
    assert_eq!(result.values, None);
    Ok(())
}

#[test]
fn mixed_conic_admits_all_cone_dimensions_and_indices_before_search() {
    let model = ConicModel {
        base: integer_quadratic(),
        second_order_cones: vec![SecondOrderConeConstraint {
            name: "large original cone".into(),
            components: vec![AffineExpression { offset: 0.0, terms: vec![] }; 20_000],
            upper: AffineExpression { offset: 1.0, terms: vec![] },
        }],
    };
    let options = SolveOptions { max_memory_bytes: 65_536, ..Default::default() };
    assert!(solve_mixed_conic_owned(&model, &options, Arc::new(NoControl)).is_err_and(|e| e.code == "memory_limit"));
    let mut malformed = ConicModel { base: integer_quadratic(), second_order_cones: vec![] };
    malformed.second_order_cones.push(SecondOrderConeConstraint {
        name: "bad original handle".into(),
        components: vec![AffineExpression { offset: 0.0, terms: vec![LinearTerm { variable: 1, coefficient: 1.0 }] }],
        upper: AffineExpression { offset: 1.0, terms: vec![] },
    });
    assert!(solve_mixed_conic_owned(&malformed, &SolveOptions::default(), Arc::new(NoControl)).is_err());
}
