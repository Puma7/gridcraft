use solvercraft::{risk::*, types::*};
use std::sync::Arc;

fn term(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn model(objective: [f64; 2]) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "independent support geometry".into(),
        variables: (0..2).map(|i| Variable { name: format!("x{i}"), lower: Some(-2.0), upper: Some(3.0), kind: VariableKind::Continuous }).collect(),
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![term(0, objective[0]), term(1, objective[1])],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "uncertain capacity".into(),
            terms: vec![term(0, 1.0), term(1, 1.0)],
            relation: Relation::LessEqual,
            rhs: 2.0,
        }],
    }
}

// Independent uncertainty-polytope vertices, then independent primal vertices.
// No Solvercraft compiler/validator contributes to this numerical oracle.
fn uncertainty_vertices(gamma: f64) -> Vec<[f64; 2]> {
    if gamma == 0.0 {
        return vec![[0.0, 0.0]];
    }
    if gamma <= 1.0 {
        return vec![[gamma, 0.0], [-gamma, 0.0], [0.0, gamma], [0.0, -gamma]];
    }
    let mut out = vec![];
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            out.push([a, b * (gamma - 1.0)]);
            out.push([a * (gamma - 1.0), b]);
        }
    }
    out
}
fn vertex_oracle(objective: [f64; 2], gamma: f64) -> (f64, Vec<[f64; 3]>) {
    let mut rows = vec![[1.0, 0.0, 3.0], [-1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, -1.0, 2.0]];
    rows.extend(uncertainty_vertices(gamma).into_iter().map(|u| [1.0 + 0.5 * u[0], 1.0 + u[1], 2.0]));
    let mut best = f64::NEG_INFINITY;
    for (i, a) in rows.iter().enumerate() {
        for b in &rows[i + 1..] {
            let determinant = a[0] * b[1] - a[1] * b[0];
            if determinant.abs() < 1e-12 {
                continue;
            }
            let x = (a[2] * b[1] - a[1] * b[2]) / determinant;
            let y = (a[0] * b[2] - a[2] * b[0]) / determinant;
            if rows.iter().all(|r| r[0] * x + r[1] * y <= r[2] + 1e-10) {
                best = best.max(objective[0] * x + objective[1] * y);
            }
        }
    }
    assert!(best.is_finite());
    (best, rows)
}

#[test]
fn box_and_fractional_budget_solutions_match_uncertainty_and_primal_vertex_enumeration() {
    let options = SolveOptions::default();
    for objective in [[1.0, 0.5], [-1.0, 0.5], [1.0, -0.5], [-1.0, -0.5]] {
        for gamma in [0.0, 0.5, 1.0, 1.5, 2.0] {
            let (expected, rows) = vertex_oracle(objective, gamma);
            let mut solved = solve_robust_budget_owned(
                &model(objective),
                &[BudgetUncertainty { constraint: 0, radii: vec![0.5, 1.0], budget: gamma }],
                &options,
                Arc::new(NoControl),
            )
            .unwrap();
            if solved.result.values.is_none() {
                assert_eq!(solved.result.termination, Termination::NumericalFailure, "{solved:?}");
                eprintln!("Explicit sparse route after documented dense refusal: objective={objective:?}, budget={gamma}");
                solved.result = solvercraft::engines::solve_sparse_owned(&solved.counterpart, &options, Arc::new(NoControl)).unwrap().result;
            }
            let values = solved.result.values.as_ref().unwrap_or_else(|| panic!("objective={objective:?}, gamma={gamma}: {solved:?}"));
            assert!(rows.iter().all(|r| r[0] * values[0] + r[1] * values[1] <= r[2] + 2e-7));
            assert!(
                (solved.result.objective.unwrap() - expected).abs() < 1e-6,
                "objective={objective:?}, budget={gamma}: {solved:?}, oracle={expected}"
            );
        }
        let expected = vertex_oracle(objective, 2.0).0;
        let solved =
            solve_robust_box_owned(&model(objective), &[BoxUncertainty { constraint: 0, radii: vec![0.5, 1.0] }], &options, Arc::new(NoControl))
                .unwrap();
        assert!((solved.result.objective.unwrap() - expected).abs() < 1e-6);
    }
}

#[test]
fn a_degenerate_ordinary_counterpart_does_not_claim_false_unboundedness_or_optimality() {
    let result = solve_robust_budget_owned(
        &model([1.0, -0.7]),
        &[BudgetUncertainty { constraint: 0, radii: vec![0.5, 1.0], budget: 0.0 }],
        &SolveOptions::default(),
        Arc::new(NoControl),
    )
    .unwrap()
    .result;
    if let Some(values) = result.values {
        assert!((values[0] - 3.0).abs() < 1e-6 && (values[1] + 2.0).abs() < 1e-6);
        assert!((result.objective.unwrap() - 4.4).abs() < 1e-6);
    } else {
        // Current dense engine conservatively refuses a tiny degenerate pivot.
        // This explicitly records a numerical coverage limit, not a passed solve.
        assert_eq!(result.termination, Termination::NumericalFailure);
        assert_eq!(result.feasibility, Feasibility::Unknown);
        assert_eq!(result.optimality, Optimality::Unknown);
        assert!(result.best_bound.is_none() && result.objective.is_none());
    }
}

#[test]
fn ellipsoid_rows_match_closed_form_euclidean_ball_optima() {
    use solvercraft::ellipsoid::*;
    for (coefficient, radius) in [(0.0, 1.0), (0.5, 0.5), (1.0, 1.0), (2.0, 0.25)] {
        let mut base = model([1.0, 0.0]);
        base.variables[0].lower = Some(0.0);
        base.variables[1].lower = Some(0.0);
        base.constraints[0].terms = vec![term(0, coefficient)];
        base.constraints[0].rhs = 2.0;
        let uncertainty = EllipsoidUncertainty { constraint: 0, factor: vec![vec![1.0, 0.0], vec![0.0, 1.0]], radius };
        let result = solve_owned(&base, &[uncertainty], &SolveOptions::default(), Arc::new(NoControl)).unwrap();
        let expected = (2.0 / (coefficient + radius)).min(3.0);
        assert!((result.result.result.objective.unwrap() - expected).abs() < 2e-6, "{result:?}");
    }
}
