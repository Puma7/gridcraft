use solvercraft::{multiobjective::*, types::*};
fn t(coefficient: f64) -> LinearTerm {
    LinearTerm { variable: 0, coefficient }
}
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "objective oracle matrix".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-1.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: vec![],
    }
}
fn signed(direction: Direction) -> f64 {
    if direction == Direction::Minimize { 1.0 } else { -1.0 }
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-7, "actual={actual}, oracle={expected}");
}

#[test]
fn weighted_goal_matrix_matches_independently_signed_linear_endpoints() {
    let mut checked = 0;
    for a in [-2.0, -1.0, 0.0, 1.0, 2.0] {
        for b in [-2.0, -1.0, 0.0, 1.0, 2.0] {
            for da in [Direction::Minimize, Direction::Maximize] {
                for db in [Direction::Minimize, Direction::Maximize] {
                    for code in 1..9 {
                        let weights = [(code % 3) as f64, (code / 3) as f64];
                        let total = weights[0] + weights[1];
                        let coefficient = (weights[0] * signed(da) * a + weights[1] * signed(db) * b) / total;
                        for scale in [1e-200, 1.0, 1e200] {
                            for offset in [0.0, 1_099_511_627_776.0, -1_099_511_627_776.0] {
                                let goals = [
                                    LinearGoal { name: "a".into(), direction: da, offset, terms: vec![t(a)] },
                                    LinearGoal { name: "b".into(), direction: db, offset: -offset, terms: vec![t(b)] },
                                ];
                                let result = solve(
                                    &model(),
                                    &goals,
                                    &MultiobjectiveMethod::Weighted { weights: weights.iter().map(|v| v * scale).collect() },
                                    &SolveOptions::default(),
                                    &NoControl,
                                )
                                .unwrap();
                                let x = result.values.as_ref().unwrap()[0];
                                if coefficient > 0.0 {
                                    close(x, -1.0);
                                } else if coefficient < 0.0 {
                                    close(x, 2.0);
                                } else {
                                    assert!((-1.0..=2.0).contains(&x));
                                }
                                assert_eq!(result.stages[0].optimality, Optimality::Optimal);
                                for (actual, weight) in result.scalarization_weights.as_ref().unwrap().iter().zip(weights) {
                                    assert!((*actual - weight / total).abs() < 2e-16);
                                }
                                assert_eq!(result.goal_values.as_ref().unwrap(), &vec![offset + a * x, -offset + b * x]);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 7200);
    eprintln!("weighted goals: {checked} endpoint optima, 0 refusals");
}

fn restricted_interval(a: f64, direction: Direction, threshold: f64) -> (f64, f64) {
    if (a > 0.0) == (direction == Direction::Minimize) { (-1.0, threshold.min(2.0)) } else { (threshold.max(-1.0), 2.0) }
}

#[test]
fn lexicographic_and_epsilon_matrices_preserve_independent_original_unit_bounds() {
    let mut lex = 0;
    let mut epsilon = 0;
    for a in [-2.0, -1.0, 1.0, 2.0] {
        for b in [-2.0, -1.0, 1.0, 2.0] {
            for da in [Direction::Minimize, Direction::Maximize] {
                for db in [Direction::Minimize, Direction::Maximize] {
                    for offset in [0.0, 1_099_511_627_776.0, -1_099_511_627_776.0] {
                        let goals = [
                            LinearGoal { name: "a".into(), direction: da, offset, terms: vec![t(a)] },
                            LinearGoal { name: "b".into(), direction: db, offset: -offset, terms: vec![t(b)] },
                        ];
                        let first = if signed(da) * a > 0.0 { -1.0 } else { 2.0 };
                        for tolerance in [0.0, 0.25, 1.0] {
                            let threshold = first + signed(da) * tolerance / a;
                            let (lo, hi) = restricted_interval(a, da, threshold);
                            let expected = if signed(db) * b > 0.0 { lo } else { hi };
                            let result = solve(
                                &model(),
                                &goals,
                                &MultiobjectiveMethod::Lexicographic { absolute_tolerances: vec![tolerance, 0.0] },
                                &SolveOptions::default(),
                                &NoControl,
                            )
                            .unwrap();
                            close(result.values.as_ref().unwrap()[0], expected);
                            assert_eq!(result.stages.len(), 2);
                            assert!(result.stages.iter().all(|s| s.optimality == Optimality::Optimal));
                            close(result.imposed_bounds[0].rhs, a * first + signed(da) * tolerance);
                            lex += 1;
                        }
                        for threshold in [-1.0, 0.0, 1.0, 2.0] {
                            let (lo, hi) = restricted_interval(a, da, threshold);
                            let expected = if signed(db) * b > 0.0 { lo } else { hi };
                            let result = solve(
                                &model(),
                                &goals,
                                &MultiobjectiveMethod::EpsilonConstraint {
                                    primary: 1,
                                    bounds: vec![EpsilonBound { goal: 0, value: offset + a * threshold }],
                                },
                                &SolveOptions::default(),
                                &NoControl,
                            )
                            .unwrap();
                            close(result.values.as_ref().unwrap()[0], expected);
                            assert_eq!(result.stages[0].optimality, Optimality::Optimal);
                            epsilon += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!((lex, epsilon), (576, 768));
    eprintln!("priorities: {lex} lexicographic + {epsilon} epsilon optima, 0 refusals");
}

#[test]
fn sensitivity_prices_and_warm_rhs_changes_match_analytic_reoptimization() {
    let mut checked = 0;
    for a in [0.25, 0.5, 1.0, 2.0, 4.0] {
        for c in [0.25, 0.5, 1.0, 2.0, 4.0] {
            for rhs in [0.5, 1.0, 2.0] {
                for direction in [Direction::Minimize, Direction::Maximize] {
                    for offset in [0.0, 1_099_511_627_776.0] {
                        let coefficient = if direction == Direction::Maximize { c } else { -c };
                        let mut original = model();
                        original.variables[0].lower = Some(0.0);
                        original.variables[0].upper = Some(10.0);
                        original.objective = Objective { direction, offset, linear: vec![t(coefficient)], quadratic: vec![] };
                        original.constraints.push(LinearConstraint {
                            name: "active capacity".into(),
                            terms: vec![t(a)],
                            relation: Relation::LessEqual,
                            rhs,
                        });
                        let initial = solvercraft::engines::solve_lp_with_basis(&original, &SolveOptions::default(), &NoControl, None).unwrap();
                        let sensitivity = initial.sensitivity.as_ref().unwrap();
                        close(sensitivity.rhs_shadow_prices[0], coefficient / a);
                        for delta in [-0.25, 0.0, 0.25] {
                            let mut changed = original.clone();
                            changed.constraints[0].rhs += delta;
                            let next =
                                solvercraft::engines::solve_lp_with_basis(&changed, &SolveOptions::default(), &NoControl, initial.basis.as_ref())
                                    .unwrap();
                            close(next.result.values.as_ref().unwrap()[0], (rhs + delta) / a);
                            assert_eq!(next.result.objective.unwrap(), offset + coefficient * ((rhs + delta) / a));
                            assert_eq!(next.result.optimality, Optimality::Optimal);
                            assert!(next.warm_start_used);
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 900);
    eprintln!("sensitivity: 300 cold bases + {checked} analytic warm RHS reoptimizations, 0 refusals");
}
