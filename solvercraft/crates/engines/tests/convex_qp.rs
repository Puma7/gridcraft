use solvercraft_engines::{QpWarmStart, analyze_quadratic_curvature, solve_qp_detailed};
use solvercraft_types::*;

fn var(name: &str, low: Option<f64>, high: Option<f64>) -> Variable {
    Variable { name: name.into(), lower: low, upper: high, kind: VariableKind::Continuous }
}
fn model(variables: Vec<Variable>, linear: &[f64], quadratic: &[(usize, usize, f64)], constraints: Vec<LinearConstraint>) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "convex QP reference".into(),
        variables,
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: linear.iter().enumerate().map(|(variable, c)| LinearTerm { variable, coefficient: *c }).collect(),
            quadratic: quadratic.iter().map(|(left, right, c)| QuadraticTerm { left: *left, right: *right, coefficient: *c }).collect(),
        },
        constraints,
    }
}
fn row(terms: &[f64], relation: Relation, rhs: f64) -> LinearConstraint {
    LinearConstraint {
        name: "row".into(),
        terms: terms.iter().enumerate().map(|(variable, c)| LinearTerm { variable, coefficient: *c }).collect(),
        relation,
        rhs,
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-6, "{a} != {b}");
}
#[test]
fn cross_term_equality_qp_has_independent_primal_dual_solution() -> Result<()> {
    let m = model(
        vec![var("x", Some(0.0), None), var("y", Some(0.0), None)],
        &[-1.0, -1.0],
        &[(0, 0, 2.0), (0, 1, 1.0), (1, 1, 1.0)],
        vec![row(&[1.0, 1.0], Relation::Equal, 1.0)],
    );
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    assert_eq!(solved.result.optimality, Optimality::Optimal, "{solved:?}");
    let point = solved.result.values.as_ref().ok_or_else(|| SolverError::new("test", "No solution"))?;
    near(point[0], 0.25);
    near(point[1], 0.75);
    near(solved.result.objective.unwrap_or(f64::NAN), -0.125);
    let cert = solved.certificate.as_ref().ok_or_else(|| SolverError::new("test", "No certificate"))?;
    near(cert.duals[0].multiplier, -0.75);
    assert!(cert.absolute_gap.is_some_and(|gap| (0.0..1e-7).contains(&gap)));
    assert!(cert.stationarity_residual < 1e-8);
    Ok(())
}
#[test]
fn active_inequality_qp_finds_feasible_boundary_and_multiplier() -> Result<()> {
    let m = model(
        vec![var("x", None, None), var("y", None, None)],
        &[0.0, 0.0],
        &[(0, 0, 1.0), (1, 1, 1.0)],
        vec![row(&[1.0, 1.0], Relation::GreaterEqual, 3.0)],
    );
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    assert_eq!(solved.result.optimality, Optimality::Optimal, "{solved:?}");
    let point = solved.result.values.as_ref().ok_or_else(|| SolverError::new("test", "No solution"))?;
    near(point[0], 1.5);
    near(point[1], 1.5);
    near(solved.result.objective.unwrap_or(f64::NAN), 4.5);
    near(solved.certificate.as_ref().ok_or_else(|| SolverError::new("test", "No dual"))?.duals[0].multiplier, -3.0);
    Ok(())
}
#[test]
fn positive_definite_non_diagonally_dominant_matrix_is_supported() -> Result<()> {
    let m = model(vec![var("x", None, None), var("y", None, None)], &[-6.0, -14.0], &[(0, 0, 1.0), (0, 1, 4.0), (1, 1, 5.0)], vec![]);
    let curvature = analyze_quadratic_curvature(&m, &SolveOptions::default(), &NoControl)?;
    assert!(curvature.minimum_eigenvalue_lower_bound > 0.0);
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    assert_eq!(solved.result.optimality, Optimality::Optimal, "{solved:?}");
    let point = solved.result.values.as_ref().ok_or_else(|| SolverError::new("test", "No solution"))?;
    near(point[0], 1.0);
    near(point[1], 1.0);
    near(solved.result.objective.unwrap_or(f64::NAN), -10.0);
    Ok(())
}
#[test]
fn semidefinite_coupled_objective_with_box_and_equality() -> Result<()> {
    let m = model(
        vec![var("x", Some(0.0), Some(2.0)), var("y", Some(0.0), Some(2.0))],
        &[0.0, 0.0],
        &[(0, 0, 1.0), (0, 1, -2.0), (1, 1, 1.0)],
        vec![row(&[1.0, 1.0], Relation::Equal, 2.0)],
    );
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    assert_eq!(solved.result.optimality, Optimality::Optimal, "{solved:?}");
    let point = solved.result.values.as_ref().ok_or_else(|| SolverError::new("test", "No solution"))?;
    near(point[0], 1.0);
    near(point[1], 1.0);
    near(solved.result.objective.unwrap_or(f64::NAN), 0.0);
    Ok(())
}
#[test]
fn primal_and_dual_warm_start_is_used_and_reduces_iterations() -> Result<()> {
    let m = model(vec![var("x", None, None), var("y", None, None)], &[-6.0, -14.0], &[(0, 0, 1.0), (0, 1, 4.0), (1, 1, 5.0)], vec![]);
    let cold = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    let warm = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, cold.warm_start.as_ref())?;
    assert!(warm.warm_start_used);
    assert_eq!(warm.result.optimality, Optimality::Optimal);
    assert!(warm.result.statistics.iterations < cold.result.statistics.iterations);
    let invalid = QpWarmStart { primal: vec![f64::NAN, 0.0], dual: vec![] };
    assert!(solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, Some(&invalid)).is_err_and(|e| e.code == "warm_start_shape"));
    Ok(())
}
#[test]
fn indefinite_quadratic_is_not_mislabeled_convex() {
    let m = model(vec![var("x", Some(-1.0), Some(1.0)), var("y", Some(-1.0), Some(1.0))], &[0.0, 0.0], &[(0, 1, 2.0)], vec![]);
    assert!(solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None).is_err_and(|e| e.code == "convexity_not_proven"));
}
#[test]
fn infeasible_linear_constraints_remain_distinct_from_qp_nonconvergence() -> Result<()> {
    let m = model(
        vec![var("x", None, None)],
        &[0.0],
        &[(0, 0, 1.0)],
        vec![row(&[1.0], Relation::LessEqual, 0.0), row(&[1.0], Relation::GreaterEqual, 1.0)],
    );
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, None)?;
    assert_eq!(solved.result.feasibility, Feasibility::Infeasible);
    assert_eq!(solved.result.termination, Termination::Completed);
    Ok(())
}
#[test]
fn cancellation_preserves_a_verified_qp_point_and_the_termination_reason() -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Cancel(AtomicBool);
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
        fn progress(&self, _: &ProgressEvent) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let m = model(
        vec![var("x", Some(-2.0), Some(2.0)), var("y", Some(-2.0), Some(2.0))],
        &[-6.0, -14.0],
        &[(0, 0, 1.0), (0, 1, 4.0), (1, 1, 5.0)],
        vec![],
    );
    let solved = solve_qp_detailed(&m, &SolveOptions::default(), &Cancel(AtomicBool::new(false)), None)?;
    assert_eq!(solved.result.termination, Termination::Cancelled, "{solved:?}");
    assert_eq!(solved.result.feasibility, Feasibility::Feasible);
    assert!(
        solved
            .result
            .values
            .as_ref()
            .is_some_and(|x| solvercraft_model::validate_solution(&m, x, &SolveOptions::default()).is_ok_and(|v| v.feasible))
    );
    Ok(())
}
#[test]
fn interrupted_qp_incumbent_and_gap_are_invariant_to_objective_offset() -> Result<()> {
    let mut m = model(
        vec![var("x", Some(-2.0), Some(2.0)), var("y", Some(-2.0), Some(2.0))],
        &[-6.0, -14.0],
        &[(0, 0, 1.0), (0, 1, 4.0), (1, 1, 5.0)],
        vec![],
    );
    let options = SolveOptions { max_iterations: 11, ..SolveOptions::default() };
    let baseline = solve_qp_detailed(&m, &options, &NoControl, None)?;
    for offset in [1e16, -1e16] {
        m.objective.offset = offset;
        let shifted = solve_qp_detailed(&m, &options, &NoControl, None)?;
        assert_eq!(shifted.result.values, baseline.result.values);
        assert_eq!(shifted.result.termination, baseline.result.termination);
        assert_eq!(shifted.certificate.as_ref().and_then(|c| c.absolute_gap), baseline.certificate.as_ref().and_then(|c| c.absolute_gap));
    }
    Ok(())
}
#[test]
fn qp_zero_budgets_do_not_fabricate_optimality() -> Result<()> {
    let m =
        model(vec![var("x", Some(0.0), Some(2.0)), var("y", Some(0.0), Some(2.0))], &[-1.0, -1.0], &[(0, 0, 1.0), (0, 1, 0.25), (1, 1, 1.0)], vec![]);
    for (options, termination) in [
        (SolveOptions { max_iterations: 0, ..SolveOptions::default() }, Termination::IterationLimit),
        (SolveOptions { max_evaluations: 0, ..SolveOptions::default() }, Termination::EvaluationLimit),
    ] {
        let r = solve_qp_detailed(&m, &options, &NoControl, None)?;
        assert_eq!(r.result.termination, termination);
        assert_eq!(r.result.optimality, Optimality::Unknown);
        assert_eq!(r.result.values, None);
    }
    Ok(())
}
#[test]
fn finite_but_unscalable_warm_duals_are_rejected_explicitly() {
    let m = model(vec![var("x", Some(0.0), Some(2.0))], &[0.0], &[(0, 0, 1.0)], vec![row(&[1e308], Relation::Equal, 1e308)]);
    let warm = QpWarmStart { primal: vec![1.0], dual: vec![1e308, 0.0] };
    assert!(solve_qp_detailed(&m, &SolveOptions::default(), &NoControl, Some(&warm)).is_err_and(|e| e.code == "warm_start_scale"));
}
