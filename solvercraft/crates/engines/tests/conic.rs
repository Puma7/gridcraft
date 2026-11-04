#![allow(clippy::expect_used)] // Test oracle failures deliberately stop the test.
use solvercraft_engines::*;
use solvercraft_types::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn variable(name: &str, low: f64, high: f64) -> Variable {
    Variable { name: name.into(), kind: VariableKind::Continuous, lower: Some(low), upper: Some(high) }
}
fn expr(offset: f64, coefficients: &[(usize, f64)]) -> AffineExpression {
    AffineExpression {
        offset,
        terms: coefficients.iter().map(|(variable, coefficient)| LinearTerm { variable: *variable, coefficient: *coefficient }).collect(),
    }
}
fn model(variables: Vec<Variable>, coefficients: &[f64], cones: Vec<SecondOrderConeConstraint>) -> ConicModel {
    ConicModel {
        base: OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "conic analytic oracle".into(),
            variables,
            objective: Objective {
                direction: Direction::Minimize,
                offset: 0.0,
                linear: coefficients.iter().enumerate().map(|(variable, coefficient)| LinearTerm { variable, coefficient: *coefficient }).collect(),
                quadratic: vec![],
            },
            constraints: vec![],
        },
        second_order_cones: cones,
    }
}
fn solve(m: &ConicModel) -> ConicDetailedResult {
    solve_socp_owned(m, &SolveOptions::default(), Arc::new(NoControl)).expect("conic solve")
}
#[test]
fn ball_support_matches_cauchy_schwarz_oracle() {
    let m = model(
        vec![variable("x", -2.0, 2.0), variable("y", -2.0, 2.0)],
        &[-1.0, -1.0],
        vec![SecondOrderConeConstraint {
            name: "unit disk".into(),
            components: vec![expr(0.0, &[(0, 1.0)]), expr(0.0, &[(1, 1.0)])],
            upper: expr(1.0, &[]),
        }],
    );
    let r = solve(&m);
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    let x = r.result.values.as_ref().expect("point");
    assert!((x[0] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert!((x[1] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert!((r.result.objective.expect("objective") + std::f64::consts::SQRT_2).abs() < 1e-7);
    assert!(r.result.best_bound.expect("bound") <= -std::f64::consts::SQRT_2);
    assert!(validate_conic_solution(&m, x, &SolveOptions::default()).expect("validation").0);
}
#[test]
fn affine_upper_and_constant_components_match_three_four_five_triangle() {
    let m = model(
        vec![variable("t", 0.0, 10.0)],
        &[1.0],
        vec![SecondOrderConeConstraint { name: "epigraph".into(), components: vec![expr(3.0, &[]), expr(4.0, &[])], upper: expr(0.0, &[(0, 1.0)]) }],
    );
    let r = solve(&m);
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    assert!((r.result.objective.expect("objective") - 5.0).abs() < 1e-7);
    assert!(r.result.best_bound.expect("lower bound") <= 5.0);
}
#[test]
fn original_scale_cone_violation_is_rejected() {
    let m = model(
        vec![variable("x", -10.0, 10.0)],
        &[0.0],
        vec![SecondOrderConeConstraint { name: "interval".into(), components: vec![expr(0.0, &[(0, 2.0)])], upper: expr(1.0, &[]) }],
    );
    assert!(!validate_conic_solution(&m, &[0.6], &SolveOptions::default()).expect("validation").0);
    assert!(validate_conic_solution(&m, &[-0.5], &SolveOptions::default()).expect("validation").0);
}
#[test]
fn contradictory_constant_cone_has_independently_checked_infeasibility_ray() {
    let m = model(
        vec![variable("dummy", 0.0, 1.0)],
        &[0.0],
        vec![SecondOrderConeConstraint { name: "impossible".into(), components: vec![expr(2.0, &[])], upper: expr(1.0, &[]) }],
    );
    let r = solve(&m);
    assert_eq!(r.result.feasibility, Feasibility::Infeasible, "{r:?}");
    assert_eq!(r.result.optimality, Optimality::Unknown);
}
#[test]
fn objective_offset_does_not_erase_a_conic_gap_or_change_argmin() {
    let mut m = model(
        vec![variable("x", -2.0, 2.0)],
        &[-1.0],
        vec![SecondOrderConeConstraint { name: "absolute".into(), components: vec![expr(0.0, &[(0, 1.0)])], upper: expr(1.0, &[]) }],
    );
    let baseline = solve(&m);
    m.base.objective.offset = 1e16;
    let shifted = solve(&m);
    assert_eq!(baseline.result.values, shifted.result.values);
    assert_eq!(baseline.certificate.as_ref().expect("certificate").absolute_gap, shifted.certificate.as_ref().expect("certificate").absolute_gap);
    assert!(shifted.result.best_bound.expect("outward bound") <= 1e16 - 1.0);
}
struct CancelOnProgress(AtomicBool);
impl RunControl for CancelOnProgress {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn progress(&self, _: &ProgressEvent) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[test]
fn owned_callback_cancels_during_actual_backend_iterations() {
    let m = model(
        vec![variable("x", -2.0, 2.0)],
        &[-1.0],
        vec![SecondOrderConeConstraint { name: "ball".into(), components: vec![expr(0.0, &[(0, 1.0)])], upper: expr(1.0, &[]) }],
    );
    let c = Arc::new(CancelOnProgress(AtomicBool::new(false)));
    let r = solve_socp_owned(&m, &SolveOptions::default(), c.clone()).expect("cancel result");
    assert!(c.is_cancelled());
    assert_eq!(r.result.termination, Termination::Cancelled, "{r:?}");
    assert_ne!(r.result.optimality, Optimality::Optimal);
}
#[test]
fn memory_and_iteration_budgets_are_explicit() {
    let m = model(vec![variable("x", 0.0, 1.0)], &[-1.0], vec![]);
    assert!(
        solve_socp_owned(&m, &SolveOptions { max_memory_bytes: 1, ..SolveOptions::default() }, Arc::new(NoControl))
            .is_err_and(|e| e.code == "memory_limit")
    );
    let r = solve_socp_owned(&m, &SolveOptions { max_iterations: 0, ..SolveOptions::default() }, Arc::new(NoControl)).expect("zero budget");
    assert_eq!(r.result.termination, Termination::IterationLimit);
    assert_eq!(r.result.feasibility, Feasibility::Unknown);
}
#[test]
fn sparse_lp_route_solves_wyndor_with_global_bound_and_duals() {
    let mut m = model(vec![variable("x", 0.0, 2.0), variable("y", 0.0, 6.0)], &[-3.0, -5.0], vec![]);
    m.base.constraints.push(LinearConstraint {
        name: "resource".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 2.0 }],
        relation: Relation::LessEqual,
        rhs: 18.0,
    });
    let r = solve_sparse_owned(&m.base, &SolveOptions::default(), Arc::new(NoControl)).expect("sparse LP");
    assert_eq!(r.result.optimality, Optimality::Optimal, "{r:?}");
    assert!((r.result.objective.expect("objective") + 36.0).abs() < 1e-7);
    assert!(r.certificate.expect("duals").dual.len() >= 5);
}
#[test]
fn genuine_sparse_unbounded_ray_requires_a_feasible_anchor_and_no_finite_blocker() {
    let mut m = model(vec![variable("x", 0.0, 1.0)], &[-1.0], vec![]);
    m.base.variables[0].upper = None;
    let r = solve(&m);
    assert_eq!(r.result.optimality, Optimality::Unbounded, "{r:?}");
    assert_eq!(r.result.feasibility, Feasibility::Feasible);
    m.base.variables[0].upper = Some(1e16);
    let finite = solve(&m);
    assert_ne!(finite.result.optimality, Optimality::Unbounded, "{finite:?}");
}
