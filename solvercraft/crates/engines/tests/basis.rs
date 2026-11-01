#![allow(clippy::expect_used)]
use solvercraft_engines::*;
use solvercraft_types::*;
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "basis Wyndor".into(),
        variables: vec![
            Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: Some(4.0) },
            Variable { name: "y".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: Some(6.0) },
        ],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 5.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "resource".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 2.0 }],
            relation: Relation::LessEqual,
            rhs: 18.0,
        }],
    }
}
#[test]
fn shadow_prices_and_allowable_ranges_match_hand_derived_vertex() {
    // y=6 and3x+2y=18. RaisingresourceRHS bydelta gives x=2+delta/3,
    // so -6<=delta<=6 and objective derivative1. Raisingcx retainsbasis
    // preciselyfor -3<=delta_cx<=4.5.
    let m = model();
    let r = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, None).expect("basis solve");
    let report = r.sensitivity.expect("sensitivity");
    assert!((report.rhs_shadow_prices[0] - 1.0).abs() < 1e-12);
    assert!((report.rhs_changes[0].lower.expect("lower") + 6.0).abs() < 1e-12);
    assert!((report.rhs_changes[0].upper.expect("upper") - 6.0).abs() < 1e-12);
    assert!((report.objective_changes[0].lower.expect("lower") + 3.0).abs() < 1e-12);
    assert!((report.objective_changes[0].upper.expect("upper") - 4.5).abs() < 1e-12);
    assert_eq!(r.result.objective, Some(36.0));
    assert!(r.basis.is_some());
}
#[test]
fn rhs_change_reuses_the_actual_basis_and_matches_independent_solution() {
    let mut m = model();
    let first = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, None).expect("first");
    let basis = first.basis.expect("basis");
    m.constraints[0].rhs = 21.0;
    let warm = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, Some(&basis)).expect("warm");
    assert!(warm.warm_start_used, "{warm:?}");
    assert_eq!(warm.result.values, Some(vec![3.0, 6.0]));
    assert_eq!(warm.result.objective, Some(39.0));
    assert!(warm.result.best_bound.expect("bound") >= 39.0);
    let cold = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, None).expect("cold");
    assert_eq!(warm.result.values, cold.result.values);
}
#[test]
fn cost_change_outside_basis_range_runs_a_real_new_solve() {
    let mut m = model();
    let first = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, None).expect("first");
    let basis = first.basis.expect("basis");
    m.objective.linear[0].coefficient = 9.0;
    let r = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, Some(&basis)).expect("reoptimize");
    assert!(!r.warm_start_used);
    assert_eq!(r.result.values, Some(vec![4.0, 3.0]));
    assert_eq!(r.result.objective, Some(51.0));
}
#[test]
fn forged_or_different_structure_basis_is_rejected() {
    let mut m = model();
    let first = solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, None).expect("first");
    let mut basis = first.basis.expect("basis");
    basis.active_rows[0] = 9999;
    assert!(solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, Some(&basis)).is_err_and(|e| e.code == "invalid_warm_start"));
    basis.active_rows[0] = 0;
    m.constraints[0].terms[0].coefficient = 4.0;
    assert!(solve_lp_with_basis(&m, &SolveOptions::default(), &NoControl, Some(&basis)).is_err_and(|e| e.code == "invalid_warm_start"));
}
