//! Independent exact witnesses for degenerate scaled LP/MILP pivots.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use solvercraft_engines::{AffineExpression, ConicModel, SecondOrderConeConstraint, solve, solve_owned, solve_socp_owned, solve_sparse_owned};
use solvercraft_types::*;
use std::sync::Arc;

fn ordered_chain(kind: VariableKind, direction: Direction, count: usize, reverse_rows: bool) -> OptimizationModel {
    let sign = if direction == Direction::Maximize { 1.0 } else { -1.0 };
    let mut constraints: Vec<LinearConstraint> = (0..count - 1)
        .map(|i| LinearConstraint {
            name: format!("order-{i}"),
            terms: vec![LinearTerm { variable: i, coefficient: 1.0 }, LinearTerm { variable: i + 1, coefficient: -1.0 }],
            relation: Relation::LessEqual,
            rhs: 0.0,
        })
        .collect();
    if reverse_rows {
        constraints.reverse();
    }
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "exact ordered-chain witness".into(),
        variables: (0..count).map(|i| Variable { name: format!("x{i}"), lower: Some(0.0), upper: Some(1.0), kind }).collect(),
        objective: Objective {
            direction,
            offset: 0.0,
            linear: vec![
                LinearTerm { variable: 0, coefficient: sign * 1e16 },
                LinearTerm { variable: count / 2, coefficient: sign },
                LinearTerm { variable: count - 1, coefficient: -sign * 1e16 },
            ],
            quadratic: vec![],
        },
        constraints,
    }
}

#[test]
fn degenerate_inequality_pivots_preserve_the_unit_objective_residual() {
    // For every 0<=x0<=...<=xn<=1:
    // 10^16*(x0-xn)+xmid <= xmid <=1, achieved by all ones.
    // Reversing the objective gives the independent minimum -1 witness.
    // The source equality-chain test does not exercise these phase-II pivots.
    for kind in [VariableKind::Continuous, VariableKind::Integer, VariableKind::Binary] {
        for direction in [Direction::Maximize, Direction::Minimize] {
            for reverse in [false, true] {
                for count in [3, 5] {
                    let model = ordered_chain(kind, direction, count, reverse);
                    let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
                    let expected = if direction == Direction::Maximize { 1.0 } else { -1.0 };
                    assert_eq!(result.optimality, Optimality::Optimal, "{kind:?}/{direction:?}/{reverse}/{count}: {result:?}");
                    assert_eq!(result.objective, Some(expected), "{kind:?}/{direction:?}/{reverse}/{count}: {result:?}");
                    assert_eq!(result.values, Some(vec![1.0; count]));
                    assert_eq!(result.best_bound, Some(expected));
                    assert_eq!(result.relative_gap, Some(0.0));
                }
            }
        }
    }
}

#[test]
fn a_feasible_phase_one_cancellation_is_never_reported_infeasible() {
    for bound in [1e16, 1e20] {
        for flipped in [false, true] {
            for reverse in [false, true] {
                let mut model = ordered_chain(VariableKind::Continuous, Direction::Minimize, 3, reverse);
                model.objective.linear.clear();
                for variable in &mut model.variables {
                    variable.upper = Some(bound);
                }
                let sign = if flipped { -1.0 } else { 1.0 };
                model.constraints.push(LinearConstraint {
                    name: "exact feasible large cancellation".into(),
                    terms: vec![
                        LinearTerm { variable: 0, coefficient: sign * bound },
                        LinearTerm { variable: 1, coefficient: sign },
                        LinearTerm { variable: 2, coefficient: -sign * bound },
                    ],
                    relation: if flipped { Relation::LessEqual } else { Relation::GreaterEqual },
                    rhs: sign * bound,
                });
                // All coordinates equal bound is an exact feasible dyadic point:
                // bound*bound + bound - bound*bound = bound.
                let exact = solvercraft_model::validate_solution(&model, &[bound; 3], &SolveOptions::default()).unwrap();
                assert!(exact.feasible);
                let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
                assert_ne!(result.feasibility, Feasibility::Infeasible, "{bound}/{flipped}/{reverse}: {result:?}");
                if result.feasibility == Feasibility::Feasible {
                    assert!(
                        solvercraft_model::validate_solution(&model, result.values.as_ref().unwrap(), &SolveOptions::default()).unwrap().feasible
                    );
                } else {
                    assert_eq!(result.termination, Termination::NumericalFailure);
                    assert_eq!(result.optimality, Optimality::Unknown);
                }
            }
        }
    }
}

#[test]
fn original_row_certificates_keep_provable_infeasibility_for_open_boxes_and_equalities() {
    let mut base = ordered_chain(VariableKind::Continuous, Direction::Minimize, 1, false);
    base.objective.linear.clear();
    for (lower, upper, rows) in [
        (Some(0.0), Some(1.0), vec![(1.0, Relation::GreaterEqual, 2.0)]),
        (Some(0.0), None, vec![(1.0, Relation::LessEqual, 1.0), (1.0, Relation::GreaterEqual, 2.0)]),
        (None, None, vec![(1.0, Relation::Equal, 0.0), (1.0, Relation::Equal, 1.0)]),
        (None, Some(1.0), vec![(-1.0, Relation::LessEqual, -2.0)]),
        (None, None, vec![(1.0, Relation::GreaterEqual, 1.0), (1.0, Relation::LessEqual, 0.0)]),
    ] {
        let mut model = base.clone();
        model.variables[0].lower = lower;
        model.variables[0].upper = upper;
        model.constraints = rows
            .into_iter()
            .map(|(coefficient, relation, rhs)| LinearConstraint {
                name: "contradiction".into(),
                terms: vec![LinearTerm { variable: 0, coefficient }],
                relation,
                rhs,
            })
            .collect();
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        assert_eq!(result.feasibility, Feasibility::Infeasible, "{result:?}");
        assert_eq!(result.termination, Termination::Completed);
    }
}

#[test]
fn a_bounded_inequality_cycle_is_never_reported_unbounded_after_roundoff() {
    for reverse in [false, true] {
        let mut model = ordered_chain(VariableKind::Continuous, Direction::Maximize, 3, reverse);
        for variable in &mut model.variables {
            variable.upper = None;
        }
        model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 1.0 }];
        model.constraints.push(LinearConstraint {
            name: "close equality cycle".into(),
            terms: vec![LinearTerm { variable: 2, coefficient: 1.0 }, LinearTerm { variable: 0, coefficient: -1.0 }],
            relation: Relation::LessEqual,
            rhs: 0.0,
        });
        model.constraints.push(LinearConstraint {
            name: "unit effective upper bound".into(),
            terms: vec![
                LinearTerm { variable: 0, coefficient: 1e16 },
                LinearTerm { variable: 1, coefficient: 1.0 },
                LinearTerm { variable: 2, coefficient: -1e16 },
            ],
            relation: Relation::LessEqual,
            rhs: 1.0,
        });
        // The three order inequalities force x=y=z; the final exact dyadic
        // row is therefore x<=1. This is bounded, attained at all ones.
        assert!(solvercraft_model::validate_solution(&model, &[1.0; 3], &SolveOptions::default()).unwrap().feasible);
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        assert_ne!(result.optimality, Optimality::Unbounded, "{result:?}");
        if result.optimality == Optimality::Optimal {
            assert_eq!(result.objective, Some(1.0));
        } else {
            assert_eq!(result.termination, Termination::NumericalFailure);
        }
    }
}

#[test]
fn exact_original_recession_checks_preserve_genuine_unbounded_models() {
    let mut model = ordered_chain(VariableKind::Continuous, Direction::Maximize, 1, false);
    model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 1.0 }];
    model.variables[0].upper = None;
    assert_eq!(solve(&model, &SolveOptions::default(), &NoControl).unwrap().optimality, Optimality::Unbounded);
    model.objective.direction = Direction::Minimize;
    model.objective.linear[0].coefficient = -1.0;
    assert_eq!(solve(&model, &SolveOptions::default(), &NoControl).unwrap().optimality, Optimality::Unbounded);
    model.variables[0].lower = None;
    model.variables[0].upper = Some(0.0);
    model.objective.linear[0].coefficient = 1.0;
    assert_eq!(solve(&model, &SolveOptions::default(), &NoControl).unwrap().optimality, Optimality::Unbounded);
    model.variables[0].upper = None;
    assert_eq!(solve(&model, &SolveOptions::default(), &NoControl).unwrap().optimality, Optimality::Unbounded);
    let mut model = ordered_chain(VariableKind::Continuous, Direction::Maximize, 2, false);
    model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 1.0 }];
    for variable in &mut model.variables {
        variable.upper = None;
    }
    model.constraints[0].relation = Relation::Equal;
    assert_eq!(solve(&model, &SolveOptions::default(), &NoControl).unwrap().optimality, Optimality::Unbounded);
}

fn squares_model(a: f64, b: f64, k: f64, kind: VariableKind) -> OptimizationModel {
    // Independently specified sum of nonnegative squares:
    // (x-a)^2 + 2(y-b)^2 + k((x-a)-(y-b))^2.
    // Its unique global minimizer is exactly (a,b), with value zero.
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "independent square-sum KKT witness".into(),
        variables: vec![
            Variable { name: "x".into(), kind, lower: Some(a - 3.0), upper: Some(a + 3.0) },
            Variable { name: "y".into(), kind, lower: Some(b - 3.0), upper: Some(b + 3.0) },
        ],
        objective: Objective {
            direction: Direction::Minimize,
            offset: a * a + 2.0 * b * b + k * (a - b) * (a - b),
            linear: vec![
                LinearTerm { variable: 0, coefficient: -2.0 * a - 2.0 * k * (a - b) },
                LinearTerm { variable: 1, coefficient: -4.0 * b + 2.0 * k * (a - b) },
            ],
            quadratic: vec![
                QuadraticTerm { left: 0, right: 0, coefficient: 1.0 + k },
                QuadraticTerm { left: 0, right: 1, coefficient: -2.0 * k },
                QuadraticTerm { left: 1, right: 1, coefficient: 2.0 + k },
            ],
        },
        constraints: vec![],
    }
}
fn check_square_minimum(model: &OptimizationModel, a: f64, b: f64, result: &SolveResult) {
    assert_eq!(result.feasibility, Feasibility::Feasible, "{result:?}");
    let values = result.values.as_ref().unwrap();
    assert!((values[0] - a).abs() < 2e-5 && (values[1] - b).abs() < 2e-5, "{result:?}");
    let direct = (values[0] - a).powi(2)
        + 2.0 * (values[1] - b).powi(2)
        + model.objective.quadratic[1].coefficient.abs() / 2.0 * ((values[0] - a) - (values[1] - b)).powi(2);
    assert!(direct < 1e-8, "direct square-sum={direct}; {result:?}");
    assert!(result.objective.unwrap().abs() < 1e-7, "{result:?}");
    if let Some(bound) = result.best_bound {
        assert!(bound <= 1e-7, "Invalid lower bound {bound}");
    }
}

#[test]
fn dense_and_sparse_coupled_qp_match_independent_square_sum_minima() {
    for i in 0..24 {
        let a = (i % 7) as f64 / 4.0 - 0.75;
        let b = (i % 5) as f64 / 4.0 - 0.5;
        let k = [0.25, 1.0, 4.0][i % 3];
        let model = squares_model(a, b, k, VariableKind::Continuous);
        let dense = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        check_square_minimum(&model, a, b, &dense);
        assert_eq!(dense.optimality, Optimality::Optimal, "{dense:?}");
        let sparse = solve_sparse_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).unwrap();
        check_square_minimum(&model, a, b, &sparse.result);
        assert_eq!(sparse.result.optimality, Optimality::Optimal, "{sparse:?}");
    }
}

#[test]
fn integer_quadratic_route_matches_exact_integer_square_sum_minima() {
    for i in 0..12 {
        let a = (i % 5) as f64 - 2.0;
        let b = (i % 3) as f64 - 1.0;
        let model = squares_model(a, b, [0.25, 1.0, 4.0][i % 3], VariableKind::Integer);
        let result = solve_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).unwrap();
        check_square_minimum(&model, a, b, &result);
        assert_eq!(result.values, Some(vec![a, b]));
        assert_eq!(result.optimality, Optimality::Optimal, "{result:?}");
    }
}

#[test]
fn shifted_scaled_socp_balls_match_cauchy_schwarz_support_oracles() {
    for i in 0..16 {
        let n = 2 + i % 4;
        let radius = 0.5 + (i % 5) as f64 / 4.0;
        let scale = [0.25, 2.0, 128.0][i % 3];
        let center: Vec<f64> = (0..n).map(|j| (j as f64 - 1.0) / 4.0).collect();
        let direction: Vec<f64> = (0..n).map(|j| 1.0 + ((i + j) % 4) as f64).collect();
        let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut base = ordered_chain(VariableKind::Continuous, Direction::Minimize, n, false);
        base.constraints.clear();
        for (var, c) in base.variables.iter_mut().zip(&center) {
            var.lower = Some(c - 2.0 * radius);
            var.upper = Some(c + 2.0 * radius);
        }
        base.objective.linear = direction.iter().enumerate().map(|(variable, c)| LinearTerm { variable, coefficient: -c }).collect();
        let model = ConicModel {
            base,
            second_order_cones: vec![SecondOrderConeConstraint {
                name: "shifted ball".into(),
                components: center
                    .iter()
                    .enumerate()
                    .map(|(variable, c)| AffineExpression { offset: -scale * c, terms: vec![LinearTerm { variable, coefficient: scale }] })
                    .collect(),
                upper: AffineExpression { offset: scale * radius, terms: vec![] },
            }],
        };
        let result = solve_socp_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).unwrap();
        let expected = -center.iter().zip(&direction).map(|(c, d)| c * d).sum::<f64>() - radius * length;
        assert_eq!(result.result.feasibility, Feasibility::Feasible, "{result:?}");
        let values = result.result.values.as_ref().unwrap();
        for ((value, c), d) in values.iter().zip(&center).zip(&direction) {
            assert!((value - (c + radius * d / length)).abs() < 2e-5, "{result:?}");
        }
        assert!((result.result.objective.unwrap() - expected).abs() < 2e-6, "expected {expected}; {result:?}");
        if let Some(bound) = result.result.best_bound {
            assert!(bound <= expected + 1e-7, "Invalid SOCP lower bound {bound} > {expected}");
        }
        assert_eq!(result.result.optimality, Optimality::Optimal, "{result:?}");
    }
}
