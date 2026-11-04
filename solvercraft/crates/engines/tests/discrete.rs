#![allow(clippy::expect_used)]
use solvercraft_engines::*;
use solvercraft_types::*;
use std::sync::Arc;
fn model(cost: &[f64]) -> DiscreteConicModel {
    DiscreteConicModel {
        model: ConicModel {
            base: OptimizationModel {
                schema_version: SCHEMA_VERSION,
                name: "discrete semantic oracle".into(),
                variables: cost
                    .iter()
                    .enumerate()
                    .map(|(i, _)| Variable { name: format!("x{i}"), kind: VariableKind::Continuous, lower: Some(0.0), upper: Some(1.0) })
                    .collect(),
                objective: Objective {
                    direction: Direction::Minimize,
                    offset: 0.0,
                    linear: cost.iter().enumerate().map(|(variable, c)| LinearTerm { variable, coefficient: *c }).collect(),
                    quadratic: vec![],
                },
                constraints: vec![],
            },
            second_order_cones: vec![],
        },
        indicators: vec![],
        special_ordered_sets: vec![],
    }
}
#[test]
fn sos1_allows_at_most_one_nonzero_variable() {
    let mut m = model(&[-3.0, -2.0, -4.0]);
    m.special_ordered_sets.push(SosConstraint {
        name: "one choice".into(),
        variables: vec![0, 1, 2],
        weights: vec![1.0, 2.0, 3.0],
        kind: SosKind::Sos1,
    });
    let r = solve_discrete_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("SOS1");
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    assert!((r.result.objective.expect("objective") + 4.0).abs() < 1e-7);
    let x = r.result.values.expect("point");
    assert_eq!(x.len(), 3);
    assert!(validate_discrete_solution(&m, &x, &SolveOptions::default()).expect("original semantics").0);
}
#[test]
fn sos2_adjacency_follows_weights_and_not_input_indices() {
    let mut m = model(&[-5.0, -4.0, -1.0]);
    m.special_ordered_sets.push(SosConstraint {
        name: "adjacent".into(),
        variables: vec![0, 1, 2],
        weights: vec![10.0, 1.0, 5.0],
        kind: SosKind::Sos2,
    });
    assert!(!validate_discrete_solution(&m, &[1.0, 1.0, 0.0], &SolveOptions::default()).expect("nonadjacent").0);
    let r = solve_discrete_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("SOS2");
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    assert!((r.result.objective.expect("objective") + 6.0).abs() < 1e-7);
}
#[test]
fn bounded_indicator_enforces_both_active_values_and_equality() {
    for active in [false, true] {
        let mut m = model(&[-1.0, if active { -20.0 } else { 20.0 }]);
        m.model.base.variables[0].upper = Some(10.0);
        m.model.base.variables[1].kind = VariableKind::Binary;
        m.indicators.push(IndicatorConstraint {
            binary: 1,
            active_value: active,
            constraint: LinearConstraint {
                name: "target".into(),
                terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
                relation: Relation::Equal,
                rhs: 2.0,
            },
        });
        let r = solve_discrete_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("indicator");
        assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
        let x = r.result.values.expect("point");
        assert_eq!(x[1], if active { 1.0 } else { 0.0 });
        assert!((x[0] - 2.0).abs() < 1e-7);
        assert!(validate_discrete_solution(&m, &x, &SolveOptions::default()).expect("independent check").0);
    }
}
#[test]
fn missing_bounds_or_ambiguous_sos_weights_are_explicit_errors() {
    let mut m = model(&[1.0, 0.0]);
    m.model.base.variables[1].kind = VariableKind::Binary;
    m.model.base.variables[0].upper = None;
    m.indicators.push(IndicatorConstraint {
        binary: 1,
        active_value: true,
        constraint: LinearConstraint {
            name: "row".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            relation: Relation::LessEqual,
            rhs: 1.0,
        },
    });
    assert!(compile_discrete(&m, &SolveOptions::default()).is_err_and(|e| e.code == "finite_indicator_bounds_required"));
    m.indicators.clear();
    m.special_ordered_sets.push(SosConstraint { name: "duplicate".into(), variables: vec![0, 1], weights: vec![1.0, 1.0], kind: SosKind::Sos1 });
    assert!(compile_discrete(&m, &SolveOptions::default()).is_err_and(|e| e.code == "invalid_discrete_constraint"));
}
