#![allow(clippy::expect_used)]
use solvercraft_engines::*;
use solvercraft_types::*;
use std::sync::Arc;
fn base(variables: Vec<Variable>, linear: &[f64], quadratic: Vec<QuadraticTerm>) -> ConicModel {
    ConicModel {
        base: OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "mixed convex oracle".into(),
            variables,
            objective: Objective {
                direction: Direction::Minimize,
                offset: 0.0,
                linear: linear.iter().enumerate().map(|(variable, c)| LinearTerm { variable, coefficient: *c }).collect(),
                quadratic,
            },
            constraints: vec![],
        },
        second_order_cones: vec![],
    }
}
fn var(name: &str, kind: VariableKind, lo: f64, hi: f64) -> Variable {
    Variable { name: name.into(), kind, lower: Some(lo), upper: Some(hi) }
}
fn expr(terms: &[(usize, f64)]) -> AffineExpression {
    AffineExpression { offset: 0.0, terms: terms.iter().map(|(i, c)| LinearTerm { variable: *i, coefficient: *c }).collect() }
}
#[test]
fn mixed_integer_quadratic_matches_independent_square_enumeration() {
    // (x-1.3)^2+(y+0.2)^2 plus a constant omitted: optimum x=1,y=-.2.
    let m = base(
        vec![var("integer", VariableKind::Integer, -3.0, 4.0), var("continuous", VariableKind::Continuous, -2.0, 2.0)],
        &[-2.6, 0.4],
        vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }, QuadraticTerm { left: 1, right: 1, coefficient: 1.0 }],
    );
    let r = solve_mixed_conic_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("MIQP");
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    let x = r.result.values.expect("incumbent");
    assert_eq!(x[0], 1.0);
    assert!((x[1] + 0.2).abs() < 1e-6);
    assert!((r.result.objective.expect("objective") + 1.64).abs() < 1e-7);
    assert!(r.result.best_bound.expect("bound") <= -1.64);
    assert!(r.relaxation_solves >= 3);
}
#[test]
fn binary_conic_epigraph_honors_integrality_and_exact_norm() {
    // min t-.8b, sqrt(b^2+1)<=t. b=0 gives1; b=1 gives sqrt2-.8.
    let mut m = base(vec![var("binary", VariableKind::Binary, 0.0, 1.0), var("t", VariableKind::Continuous, 0.0, 3.0)], &[-0.8, 1.0], vec![]);
    m.second_order_cones.push(SecondOrderConeConstraint {
        name: "norm".into(),
        components: vec![expr(&[(0, 1.0)]), AffineExpression { offset: 1.0, terms: vec![] }],
        upper: expr(&[(1, 1.0)]),
    });
    let r = solve_mixed_conic_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("MISOCP");
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    let x = r.result.values.expect("incumbent");
    assert_eq!(x[0], 1.0);
    assert!((x[1] - std::f64::consts::SQRT_2).abs() < 1e-7);
    assert!((r.result.objective.expect("objective") - (std::f64::consts::SQRT_2 - 0.8)).abs() < 1e-7);
}
#[test]
fn integer_infeasibility_is_separate_from_unbounded_relaxation() {
    let mut m =
        base(vec![var("integer", VariableKind::Integer, 0.0, 1.0), var("continuous", VariableKind::Continuous, 0.0, 5.0)], &[0.0, -1.0], vec![]);
    m.base.constraints.push(LinearConstraint {
        name: "half".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
        relation: Relation::Equal,
        rhs: 0.5,
    });
    let r = solve_mixed_conic_owned(&m, &SolveOptions::default(), Arc::new(NoControl)).expect("infeasible MIP");
    assert_eq!(r.result.feasibility, Feasibility::Infeasible, "{r:?}");
    assert_ne!(r.result.optimality, Optimality::Unbounded);
}
#[test]
fn node_limit_preserves_integer_incumbent_and_valid_relaxation_bound() {
    let m = base(vec![var("integer", VariableKind::Integer, 0.0, 4.0)], &[-2.6], vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }]);
    let r = solve_mixed_conic_owned(&m, &SolveOptions { max_nodes: 1, ..SolveOptions::default() }, Arc::new(NoControl)).expect("limited MIQP");
    assert_eq!(r.result.termination, Termination::NodeLimit, "{r:?}");
    assert_eq!(r.result.values.expect("rounded verified incumbent"), vec![1.0]);
    assert!(r.result.best_bound.expect("valid relaxation bound") <= -1.69);
    assert_ne!(r.result.optimality, Optimality::Optimal);
}
#[test]
fn owned_auto_route_solves_integer_quadratic_without_relaxing_integrality() {
    let m = base(vec![var("integer", VariableKind::Integer, 0.0, 4.0)], &[-2.6], vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }]);
    let r = solve_owned(&m.base, &SolveOptions::default(), Arc::new(NoControl)).expect("owned auto MIQP");
    assert_eq!(r.values, Some(vec![1.0]));
    assert_eq!(r.optimality, Optimality::Optimal);
    assert!(
        solve_owned(&m.base, &SolveOptions { method: SolveMethod::Simplex, ..SolveOptions::default() }, Arc::new(NoControl))
            .is_err_and(|e| e.code == "unsupported_method")
    );
}
