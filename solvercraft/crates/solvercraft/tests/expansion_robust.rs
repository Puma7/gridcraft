use solvercraft::{risk::*, types::*};
use std::sync::Arc;

fn scalar_oracle(a: f64, radius: f64, rhs: f64, c: f64) -> f64 {
    let mut candidates = vec![-2.0, 0.0, 3.0];
    for slope in [a - radius, a + radius] {
        if slope != 0.0 {
            let point = rhs / slope;
            if (-2.0..=3.0).contains(&point) {
                candidates.push(point);
            }
        }
    }
    candidates.into_iter().filter(|x| a * x + radius * x.abs() <= rhs + 1e-12).map(|x| c * x).fold(f64::NEG_INFINITY, f64::max)
}

#[test]
fn four_thousand_box_budget_and_ellipsoid_counterparts_match_scalar_support_breakpoints() {
    let mut proven = 0;
    let mut best_found = 0;
    let mut refused = 0;
    let options = SolveOptions::default();
    for a in [-2.0, -1.0, 0.0, 1.0, 2.0] {
        for radius in [0.0, 0.25, 1.0, 2.0] {
            for rhs in [0.0, 0.5, 2.0] {
                for c in [-2.0, -1.0, 1.0, 2.0] {
                    for gamma in [0.0_f64, 0.5, 1.0, 2.0] {
                        for offset in [0.0, 1024.0] {
                            let model = OptimizationModel {
                                schema_version: SCHEMA_VERSION,
                                name: "scalar robust support".into(),
                                variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(3.0), kind: VariableKind::Continuous }],
                                objective: Objective {
                                    direction: Direction::Maximize,
                                    offset,
                                    linear: vec![LinearTerm { variable: 0, coefficient: c }],
                                    quadratic: vec![],
                                },
                                constraints: vec![LinearConstraint {
                                    name: "uncertain scalar cap".into(),
                                    terms: vec![LinearTerm { variable: 0, coefficient: a }],
                                    relation: Relation::LessEqual,
                                    rhs,
                                }],
                            };
                            let mut cases = vec![
                                (
                                    "budget",
                                    radius * gamma.min(1.0),
                                    solve_robust_budget_owned(
                                        &model,
                                        &[BudgetUncertainty { constraint: 0, radii: vec![radius], budget: gamma }],
                                        &options,
                                        Arc::new(NoControl),
                                    )
                                    .map(|v| v.result),
                                ),
                                (
                                    "ellipsoid",
                                    radius * gamma,
                                    solvercraft::ellipsoid::solve_owned(
                                        &model,
                                        &[solvercraft::ellipsoid::EllipsoidUncertainty { constraint: 0, factor: vec![vec![radius]], radius: gamma }],
                                        &options,
                                        Arc::new(NoControl),
                                    )
                                    .map(|v| v.result.result),
                                ),
                            ];
                            if gamma == 1.0 {
                                cases.push((
                                    "box",
                                    radius,
                                    solve_robust_box_owned(
                                        &model,
                                        &[BoxUncertainty { constraint: 0, radii: vec![radius] }],
                                        &options,
                                        Arc::new(NoControl),
                                    )
                                    .map(|v| v.result),
                                ));
                            }
                            for (route, support, answer) in cases {
                                let expected = offset + scalar_oracle(a, support, rhs, c);
                                let result = match answer {
                                    Ok(v) => v,
                                    Err(error) => {
                                        assert!(matches!(error.code.as_str(), "numeric_range" | "numerical_failure"), "{error:?}");
                                        refused += 1;
                                        continue;
                                    }
                                };
                                if let Some(values) = &result.values {
                                    let x = values[0];
                                    assert!(x >= -2.0 - options.feasibility_tolerance && x <= 3.0 + options.feasibility_tolerance);
                                    assert!(
                                        a * x + support * x.abs() <= rhs + options.feasibility_tolerance,
                                        "{route},a={a},r={radius},b={rhs},c={c},gamma={gamma}: {result:?}"
                                    );
                                    assert!(
                                        (result.objective.unwrap() - expected).abs() < 2e-6,
                                        "{route},a={a},r={radius},b={rhs},c={c},gamma={gamma}: {result:?}, oracle={expected}"
                                    );
                                    assert!((result.objective.unwrap() - (offset + c * x)).abs() < 1e-7);
                                    if let Some(bound) = result.best_bound {
                                        assert!(bound >= expected - 2e-6, "{result:?}, oracle={expected}");
                                    }
                                    assert_eq!(result.feasibility, Feasibility::Feasible);
                                    match result.optimality {
                                        Optimality::Optimal => proven += 1,
                                        Optimality::BestFound => best_found += 1,
                                        _ => panic!("{result:?}"),
                                    }
                                } else {
                                    assert_eq!(result.termination, Termination::NumericalFailure, "{result:?}");
                                    assert_eq!(result.feasibility, Feasibility::Unknown);
                                    assert_eq!(result.optimality, Optimality::Unknown);
                                    assert!(result.objective.is_none() && result.best_bound.is_none());
                                    refused += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(proven + best_found + refused, 4320);
    eprintln!("robust scalar support: {proven} Optimal + {best_found} independently optimal BestFound points + {refused} honest numerical refusals");
}
