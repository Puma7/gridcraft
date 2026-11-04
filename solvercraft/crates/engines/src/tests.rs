#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

fn variable(kind: VariableKind, lower: Option<f64>, upper: Option<f64>) -> Variable {
    Variable { name: "x".into(), lower, upper, kind }
}
fn model(variables: Vec<Variable>, direction: Direction, coefficients: &[f64], rows: Vec<LinearConstraint>) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "reference".into(),
        variables,
        objective: Objective {
            direction,
            offset: 0.0,
            linear: coefficients.iter().enumerate().map(|(variable, coefficient)| LinearTerm { variable, coefficient: *coefficient }).collect(),
            quadratic: vec![],
        },
        constraints: rows,
    }
}
fn row(a: &[f64], relation: Relation, rhs: f64) -> LinearConstraint {
    LinearConstraint {
        name: "row".into(),
        terms: a.iter().enumerate().map(|(variable, coefficient)| LinearTerm { variable, coefficient: *coefficient }).collect(),
        relation,
        rhs,
    }
}
fn run(m: &OptimizationModel) -> SolveResult {
    solve(m, &SolveOptions::default(), &NoControl).expect("valid reference")
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn wyndor_glass_objective_and_original_residuals() {
    let m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), Some(4.0)), variable(VariableKind::Continuous, Some(0.0), Some(6.0))],
        Direction::Maximize,
        &[3.0, 5.0],
        vec![row(&[3.0, 2.0], Relation::LessEqual, 18.0)],
    );
    let r = run(&m);
    assert_eq!(r.optimality, Optimality::Optimal);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    let x = r.values.as_ref().expect("solution");
    near(x[0], 2.0);
    near(x[1], 6.0);
    near(r.objective.expect("objective"), 36.0);
    assert!(r.max_violation.expect("residual") <= 1e-7);
}
#[test]
fn alternative_optima_do_not_require_a_specific_vector() {
    let m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), None); 2],
        Direction::Maximize,
        &[1.0, 1.0],
        vec![row(&[1.0, 1.0], Relation::LessEqual, 1.0)],
    );
    let r = run(&m);
    near(r.objective.expect("objective"), 1.0);
    assert_eq!(r.feasibility, Feasibility::Feasible);
}
#[test]
fn binary_problem_honors_domains_instead_of_relaxation() {
    let mut m = model(
        vec![variable(VariableKind::Binary, None, None); 2],
        Direction::Maximize,
        &[1.0, 1.0],
        vec![row(&[2.0, 2.0], Relation::LessEqual, 3.0)],
    );
    let r = run(&m);
    near(r.objective.expect("objective"), 1.0);
    assert_eq!(r.optimality, Optimality::Optimal);
    for x in r.values.expect("values") {
        assert_eq!(x, x.round());
    }
    for v in &mut m.variables {
        v.kind = VariableKind::Continuous;
        v.lower = Some(0.0);
        v.upper = Some(1.0);
    }
    near(run(&m).objective.expect("relaxation objective"), 1.5);
}
#[test]
fn free_negative_fixed_and_equality_variables() {
    let m = model(
        vec![variable(VariableKind::Continuous, None, None), variable(VariableKind::Continuous, Some(-3.0), Some(-3.0))],
        Direction::Minimize,
        &[1.0, 2.0],
        vec![row(&[1.0, 1.0], Relation::Equal, -5.0)],
    );
    let r = run(&m);
    let x = r.values.expect("solution");
    near(x[0], -2.0);
    near(x[1], -3.0);
    near(r.objective.expect("objective"), -8.0);
}
#[test]
fn greater_equal_rows_and_redundant_equalities() {
    let m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), None)],
        Direction::Minimize,
        &[1.0],
        vec![row(&[1.0], Relation::GreaterEqual, 3.0), row(&[0.0], Relation::Equal, 0.0), row(&[2.0], Relation::GreaterEqual, 6.0)],
    );
    let r = run(&m);
    near(r.objective.expect("objective"), 3.0);
    assert_eq!(r.optimality, Optimality::Optimal);
}
#[test]
fn infeasible_constraints_are_distinct_from_limits() {
    let m = model(
        vec![variable(VariableKind::Continuous, None, None)],
        Direction::Minimize,
        &[1.0],
        vec![row(&[1.0], Relation::LessEqual, 0.0), row(&[1.0], Relation::GreaterEqual, 1.0)],
    );
    let r = run(&m);
    assert_eq!(r.feasibility, Feasibility::Infeasible);
    assert_eq!(r.termination, Termination::Completed);
}
#[test]
fn real_unbounded_lp_has_a_valid_feasible_point() {
    let m = model(vec![variable(VariableKind::Continuous, Some(0.0), None)], Direction::Minimize, &[-1.0], vec![]);
    let r = run(&m);
    assert_eq!(r.optimality, Optimality::Unbounded);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert!(r.values.is_some());
}
#[test]
fn unbounded_relaxation_does_not_prove_unbounded_mip() {
    let m = model(
        vec![variable(VariableKind::Integer, Some(0.0), Some(1.0)), variable(VariableKind::Continuous, Some(0.0), None)],
        Direction::Maximize,
        &[0.0, 1.0],
        vec![row(&[1.0, 0.0], Relation::Equal, 0.5)],
    );
    let r = run(&m);
    assert_eq!(r.optimality, Optimality::Unknown);
    assert_eq!(r.feasibility, Feasibility::Unknown);
    assert_eq!(r.termination, Termination::NumericalFailure);
}
#[test]
fn large_finite_objective_does_not_mean_unbounded() {
    let m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0))], Direction::Minimize, &[-1e16], vec![]);
    let r = run(&m);
    assert_eq!(r.optimality, Optimality::Optimal);
    assert_eq!(r.objective, Some(-1e16));
    assert_eq!(r.values, Some(vec![1.0]));
}
#[test]
fn integer_incumbent_survives_a_node_budget() {
    let m =
        model(vec![variable(VariableKind::Integer, Some(0.0), Some(3.0))], Direction::Maximize, &[1.0], vec![row(&[1.0], Relation::LessEqual, 1.5)]);
    let opts = SolveOptions { max_nodes: 2, ..SolveOptions::default() };
    let r = solve(&m, &opts, &NoControl).expect("budgeted solve");
    assert_eq!(r.termination, Termination::NodeLimit);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert_eq!(r.optimality, Optimality::BestFound);
    near(r.objective.expect("incumbent"), 1.0);
    near(r.best_bound.expect("bound"), 1.5);
}
struct Cancelled(AtomicBool);
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
#[test]
fn pre_cancelled_jobs_have_no_fabricated_feasibility() {
    let m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0))], Direction::Maximize, &[1.0], vec![]);
    let r = solve(&m, &SolveOptions::default(), &Cancelled(AtomicBool::new(true))).expect("cancel");
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.feasibility, Feasibility::Unknown);
    assert!(r.values.is_none());
}
#[test]
fn iteration_limit_is_not_infeasibility() {
    let m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0))], Direction::Maximize, &[1.0], vec![]);
    let r = solve(&m, &SolveOptions { max_iterations: 1, ..SolveOptions::default() }, &NoControl).expect("limit");
    assert_eq!(r.termination, Termination::IterationLimit);
    assert_eq!(r.feasibility, Feasibility::Unknown);
}
#[test]
fn small_integer_models_match_exhaustive_independent_oracle() {
    for a in 1..5 {
        for b in 1..5 {
            let m = model(
                vec![variable(VariableKind::Integer, Some(0.0), Some(4.0)); 2],
                Direction::Maximize,
                &[f64::from(a), f64::from(b)],
                vec![row(&[2.0, 3.0], Relation::LessEqual, 9.0)],
            );
            let mut oracle = 0;
            for x in 0..=4 {
                for y in 0..=4 {
                    if 2 * x + 3 * y <= 9 {
                        oracle = oracle.max(a * x + b * y);
                    }
                }
            }
            let r = run(&m);
            assert_eq!(r.optimality, Optimality::Optimal);
            near(r.objective.expect("objective"), f64::from(oracle));
        }
    }
}
#[test]
fn unsupported_nonconvex_quadratic_is_explicit() {
    let mut m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0))], Direction::Minimize, &[0.0], vec![]);
    m.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: -1.0 });
    assert_eq!(solve(&m, &SolveOptions::default(), &NoControl).expect_err("unsupported").code, "unsupported_problem");
}

#[test]
fn separable_quadratic_solves_narrow_interval_without_finite_difference_failure() {
    let mut m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(5e-7))], Direction::Minimize, &[-8e7], vec![]);
    m.objective.offset = 16.0;
    m.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: 1e14 });
    let r = run(&m);
    assert_eq!(r.optimality, Optimality::Optimal);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert!((r.values.expect("values")[0] - 4e-7).abs() < 1e-20);
    assert!(r.objective.expect("objective").abs() < 1e-12);
    assert_eq!(r.engine, "solvercraft-separable-convex-qp-0.1");
}
#[test]
fn separable_concave_maximization_and_bound_clamping() {
    let mut m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0)), variable(VariableKind::Continuous, Some(-5.0), Some(5.0))],
        Direction::Maximize,
        &[4.0, -6.0],
        vec![],
    );
    m.objective.quadratic = vec![QuadraticTerm { left: 0, right: 0, coefficient: -1.0 }, QuadraticTerm { left: 1, right: 1, coefficient: -1.0 }];
    let r = run(&m);
    assert_eq!(r.values, Some(vec![1.0, -3.0]));
    near(r.objective.expect("objective"), 12.0);
}
#[test]
fn large_unrelated_cost_does_not_hide_a_meaningful_small_cost() {
    let m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0)); 2],
        Direction::Maximize,
        &[1e16, 1.0],
        vec![row(&[1.0, 0.0], Relation::Equal, 0.0)],
    );
    let r = run(&m);
    near(r.objective.expect("objective"), 1.0);
    assert_eq!(r.values, Some(vec![0.0, 1.0]));
}
#[test]
fn tiny_positive_blocker_is_not_an_unbounded_ray() {
    let m = model(
        vec![variable(VariableKind::Continuous, Some(0.0), None), variable(VariableKind::Continuous, Some(0.0), Some(1.0))],
        Direction::Maximize,
        &[1.0, 0.0],
        vec![row(&[1e-15, 1.0], Relation::LessEqual, 1.0)],
    );
    let r = run(&m);
    assert_ne!(r.optimality, Optimality::Unbounded);
    assert_eq!(r.termination, Termination::NumericalFailure);
}

#[test]
fn signed_integer_models_match_exhaustive_three_variable_oracle() {
    let mut state = 901_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as i32 & 0x7fff_ffff
    };
    for _case in 0..128 {
        let cost = [next() % 9 - 4, next() % 9 - 4, next() % 9 - 4];
        let a = [next() % 7 - 3, next() % 7 - 3, next() % 7 - 3];
        let b = [next() % 7 - 3, next() % 7 - 3, next() % 7 - 3];
        let rhs = [next() % 13 - 6, next() % 13 - 6];
        let direction = if next() % 2 == 0 { Direction::Minimize } else { Direction::Maximize };
        let m = model(
            vec![variable(VariableKind::Integer, Some(-2.0), Some(3.0)); 3],
            direction,
            &cost.map(f64::from),
            vec![row(&a.map(f64::from), Relation::LessEqual, f64::from(rhs[0])), row(&b.map(f64::from), Relation::GreaterEqual, f64::from(rhs[1]))],
        );
        let mut oracle: Option<i32> = None;
        for x in -2..=3 {
            for y in -2..=3 {
                for z in -2..=3 {
                    let point = [x, y, z];
                    let dot = |terms: &[i32; 3]| terms.iter().zip(point).map(|(t, v)| t * v).sum::<i32>();
                    if dot(&a) <= rhs[0] && dot(&b) >= rhs[1] {
                        let value = dot(&cost);
                        oracle = Some(oracle.map_or(value, |old| if direction == Direction::Minimize { old.min(value) } else { old.max(value) }));
                    }
                }
            }
        }
        let r = run(&m);
        if let Some(best) = oracle {
            assert_eq!(r.optimality, Optimality::Optimal, "case {m:?}, result {r:?}");
            near(r.objective.expect("objective"), f64::from(best));
        } else {
            assert_eq!(r.feasibility, Feasibility::Infeasible, "case {m:?}, result {r:?}");
        }
    }
}
struct CancelAfterIncumbent(AtomicBool);
impl RunControl for CancelAfterIncumbent {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    fn progress(&self, event: &ProgressEvent) {
        if event.stage == "branch_and_bound" && event.best_objective.is_some() {
            self.0.store(true, Ordering::Relaxed);
        }
    }
}
#[test]
fn cancel_preserves_a_verified_integer_incumbent_and_valid_bound() {
    let m =
        model(vec![variable(VariableKind::Integer, Some(0.0), Some(3.0))], Direction::Maximize, &[1.0], vec![row(&[1.0], Relation::LessEqual, 1.5)]);
    let r = solve(&m, &SolveOptions::default(), &CancelAfterIncumbent(AtomicBool::new(false))).expect("cancelled search");
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.feasibility, Feasibility::Feasible);
    assert_eq!(r.optimality, Optimality::BestFound);
    assert_eq!(r.objective, Some(1.0));
    assert_eq!(r.best_bound, Some(1.5));
}

#[test]
fn duplicate_objective_terms_use_shared_compensated_aggregation() {
    let mut m = model(vec![variable(VariableKind::Continuous, Some(0.0), Some(1.0))], Direction::Maximize, &[], vec![]);
    m.objective.linear = [1e16, 1.0, 1.0, -1e16].into_iter().map(|coefficient| LinearTerm { variable: 0, coefficient }).collect();
    let r = run(&m);
    assert_eq!(r.values, Some(vec![1.0]));
    assert_eq!(r.objective, Some(2.0));
}
