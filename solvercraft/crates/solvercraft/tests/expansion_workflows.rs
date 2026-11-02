use solvercraft::{risk::*, types::*};

fn t(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn variable(lower: f64, upper: f64) -> Variable {
    Variable { name: "x".into(), lower: Some(lower), upper: Some(upper), kind: VariableKind::Continuous }
}
fn base(lower: f64, upper: f64) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "finite oracle matrix".into(),
        variables: vec![variable(lower, upper)],
        objective: Objective { direction: Direction::Maximize, offset: 0.0, linear: vec![t(0, 1.0)], quadratic: vec![] },
        constraints: vec![],
    }
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 2e-6, "actual={actual}, oracle={expected}");
}

#[test]
fn chance_boundaries_zero_mass_signs_and_weight_scales_match_scalar_enumeration() {
    use solvercraft::chance::*;
    let options = SolveOptions::default();
    let mut computed = 0;
    let mut refused = 0;
    for code in 1..27 {
        let weights = [(code % 3) as f64, ((code / 3) % 3) as f64, (code / 9) as f64];
        let total = weights.iter().sum::<f64>();
        for scale in [1e-200, 1.0, 1e200] {
            for boundary in [0.0, weights[0] / total, (weights[0] + weights[1]) / total, 1.0] {
                for delta in [-1e-4, 0.0, 1e-4] {
                    let limit = (boundary + delta).clamp(0.0, 1.0);
                    let mass = |x: f64| (0..3).filter(|i| x > *i as f64 + options.feasibility_tolerance).map(|i| weights[i]).sum::<f64>() / total;
                    let expected =
                        [0.0, 1.0, 2.0, 3.0].into_iter().filter(|x| mass(*x) <= limit + options.feasibility_tolerance).fold(0.0_f64, f64::max);
                    for direction in [Direction::Minimize, Direction::Maximize] {
                        for offset in [0.0, 1_048_576.0] {
                            let sign = if direction == Direction::Minimize { -1.0 } else { 1.0 };
                            let mut model = base(0.0, 3.0);
                            model.objective.direction = direction;
                            model.objective.offset = offset;
                            model.objective.linear[0].coefficient = sign;
                            let result = solve(
                                &ChanceProblem {
                                    base: model,
                                    scenarios: (0..3)
                                        .map(|i| ChanceScenario {
                                            weight: weights[i] * scale,
                                            constraints: vec![LinearConstraint {
                                                name: "scenario cap".into(),
                                                terms: vec![t(0, 1.0)],
                                                relation: Relation::LessEqual,
                                                rhs: i as f64,
                                            }],
                                        })
                                        .collect(),
                                    rule: ChanceRule::Joint { maximum_violation_probability: limit },
                                },
                                &options,
                                &NoControl,
                            )
                            .unwrap();
                            if let Some(values) = &result.solve.values {
                                close(values[0], expected);
                                close(result.solve.objective.unwrap(), offset + sign * expected);
                                assert_eq!(result.solve.feasibility, Feasibility::Feasible);
                                assert_eq!(result.solve.optimality, Optimality::Optimal, "{result:?}");
                                close(result.joint_violation_probability.unwrap(), mass(values[0]));
                                close(result.individual_violation_probabilities.unwrap()[0], mass(values[0]));
                                computed += 1;
                            } else {
                                assert_eq!(result.solve.termination, Termination::NumericalFailure, "{result:?}");
                                assert_eq!(result.solve.feasibility, Feasibility::Unknown);
                                assert_eq!(result.solve.optimality, Optimality::Unknown);
                                assert!(result.solve.objective.is_none() && result.solve.best_bound.is_none());
                                refused += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(computed + refused, 3744);
    eprintln!("chance: {computed} proven scalar oracle optima, {refused} explicit numerical refusals");
}

#[test]
fn recourse_and_benders_matrix_matches_inventory_breakpoints_with_offsets_and_zero_mass() {
    use solvercraft::stochastic::*;
    let options = SolveOptions::default();
    let mut checked = 0;
    for cost in [0.5, 1.5, 3.0, 6.0] {
        for shortage in [2.0, 4.0] {
            for d0 in [0.5, 1.0, 2.0] {
                for d1 in [0.5, 2.0, 3.0] {
                    for weights in [[1.0, 1.0], [1.0, 3.0], [0.0, 1.0], [1.0, 0.0]] {
                        let total = weights[0] + weights[1];
                        let value = |x: f64| cost * x + shortage * (weights[0] * (d0 - x).max(0.0) + weights[1] * (d1 - x).max(0.0)) / total;
                        let expected = [0.0, d0, d1, 3.0].into_iter().map(value).fold(f64::INFINITY, f64::min);
                        for scale in [1e-200, 1.0, 1e200] {
                            for offset in [0.0, 1024.0] {
                                let problem = TwoStageProblem {
                                    name: "inventory oracle grid".into(),
                                    direction: Direction::Minimize,
                                    first_stage_variables: vec![variable(0.0, 3.0)],
                                    first_stage_costs: vec![cost],
                                    first_stage_offset: offset,
                                    first_stage_constraints: vec![],
                                    scenarios: [d0, d1]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, demand)| RecourseScenario {
                                            name: format!("demand{i}"),
                                            weight: weights[i] * scale,
                                            offset: 0.0,
                                            variables: vec![Variable {
                                                name: "shortage".into(),
                                                lower: Some(0.0),
                                                upper: None,
                                                kind: VariableKind::Continuous,
                                            }],
                                            costs: vec![shortage],
                                            constraints: vec![RecourseConstraint {
                                                name: "balance".into(),
                                                first_stage_terms: vec![t(0, 1.0)],
                                                recourse_terms: vec![t(0, 1.0)],
                                                relation: Relation::GreaterEqual,
                                                rhs: demand,
                                            }],
                                        })
                                        .collect(),
                                };
                                let result = solve_two_stage(&problem, &options, &NoControl).unwrap();
                                close(result.expected_total_cost.unwrap(), offset + expected);
                                close(value(result.first_stage_values.as_ref().unwrap()[0]), expected);
                                assert_eq!(result.solve.optimality, Optimality::Optimal);
                                let benders = solvercraft::benders::solve(&problem, &options, &NoControl).unwrap();
                                assert!(benders.converged, "{problem:?}: {benders:?}");
                                close(benders.objective.unwrap(), offset + expected);
                                close(value(benders.first_stage_values.as_ref().unwrap()[0]), expected);
                                assert!(benders.lower_bound.unwrap() <= offset + expected + 2e-6);
                                // Sign reversal is a distinct maximization route.
                                let mut maximization = problem.clone();
                                maximization.direction = Direction::Maximize;
                                maximization.first_stage_costs[0] = -cost;
                                maximization.first_stage_offset = -offset;
                                for scenario in &mut maximization.scenarios {
                                    scenario.costs[0] = -shortage;
                                }
                                let maximized = solve_two_stage(&maximization, &options, &NoControl).unwrap();
                                close(maximized.expected_total_cost.unwrap(), -offset - expected);
                                assert_eq!(maximized.solve.optimality, Optimality::Optimal);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 1728);
    eprintln!("recourse: {checked} laws x3 solve routes (extensive min/max and Benders), 0 refusals");
}

#[test]
fn mean_variance_quadratic_grid_matches_closed_form_clipped_vertices() {
    let mut checked = 0;
    for code in 0..27 {
        let offsets = [(code % 3) as f64 - 1.0, ((code / 3) % 3) as f64 - 1.0, (code / 9) as f64 - 1.0];
        let slopes = [-1.0, 0.0, 1.0];
        for weights in [[1.0, 1.0, 1.0], [1.0, 2.0, 1.0], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0]] {
            let total = weights.iter().sum::<f64>();
            let mean_a = (0..3).map(|i| weights[i] * slopes[i]).sum::<f64>() / total;
            let mean_b = (0..3).map(|i| weights[i] * offsets[i]).sum::<f64>() / total;
            let var_a = (0..3).map(|i| weights[i] * (slopes[i] - mean_a).powi(2)).sum::<f64>() / total;
            let covariance = (0..3).map(|i| weights[i] * (slopes[i] - mean_a) * (offsets[i] - mean_b)).sum::<f64>() / total;
            for penalty in [0.0, 0.25, 1.0, 4.0] {
                let linear = mean_a + 2.0 * penalty * covariance;
                let quadratic = penalty * var_a;
                let optimum = if quadratic > 0.0 {
                    (-linear / (2.0 * quadratic)).clamp(-2.0, 2.0)
                } else if linear < 0.0 {
                    2.0
                } else {
                    -2.0
                };
                let objective = |x: f64| {
                    let losses = offsets.map(|v| v);
                    let mean = mean_a * x + mean_b;
                    let variance = (0..3).map(|i| weights[i] * (slopes[i] * x + losses[i] - mean).powi(2)).sum::<f64>() / total;
                    mean + penalty * variance
                };
                for scale in [1e-200, 1.0, 1e200] {
                    for offset in [0.0, 1_048_576.0] {
                        let problem = ScenarioProblem {
                            name: "analytic mean variance".into(),
                            variables: vec![variable(-2.0, 2.0)],
                            constraints: vec![],
                            scenarios: (0..3)
                                .map(|i| Scenario { weight: weights[i] * scale, offset: offset + offsets[i], coefficients: vec![slopes[i]] })
                                .collect(),
                            measure: RiskMeasure::MeanVariance { penalty },
                        };
                        let result = optimize(&problem, &SolveOptions::default(), &NoControl).unwrap();
                        let x = result.decision_values.as_ref().unwrap()[0];
                        close(objective(x), objective(optimum));
                        close(result.solve.objective.unwrap(), offset + objective(optimum));
                        assert_eq!(result.solve.feasibility, Feasibility::Feasible);
                        if let Some(bound) = result.solve.best_bound {
                            assert!(bound <= offset + objective(optimum) + 2e-6);
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 2592);
    eprintln!("mean variance: {checked} analytic scalar optima, 0 refusals");
}

#[test]
fn saa_holdouts_match_independently_counted_addressed_two_atom_samples() {
    use solvercraft::saa::*;
    let mut checked = 0;
    for seed in 0..64 {
        for trials in [2, 32, 1024] {
            let positives = (0..trials).filter(|trial| solvercraft::math::addressed_uniform(seed * 2 + 1, 0, *trial, 0) >= 0.5).count() as f64;
            let probability = positives / trials as f64;
            let mean = 2.0 * probability - 1.0;
            let variance = 4.0 * probability * (1.0 - probability) * (trials as f64 / (trials - 1) as f64);
            for offset in [0.0, 1_099_511_627_776.0] {
                let result = train_and_validate(
                    &SampleAverageProblem {
                        name: "independent addressed holdout count".into(),
                        variables: vec![variable(0.0, 0.0)],
                        constraints: vec![],
                        loss: SampledAffineLoss {
                            offset: Distribution::Categorical { values: vec![offset - 1.0, offset + 1.0], weights: vec![1.0, 1.0] },
                            coefficients: vec![Distribution::Constant { value: 0.0 }],
                        },
                        measure: RiskMeasure::ExpectedLoss,
                    },
                    &SampleAverageOptions { training_trials: 16, validation_trials: trials, training_seed: seed * 2, validation_seed: seed * 2 + 1 },
                    &SolveOptions::default(),
                    &NoControl,
                )
                .unwrap();
                let validation = result.validation.unwrap();
                close(validation.mean, offset + mean);
                assert!((validation.sample_variance - variance).abs() < 1e-13);
                assert!((validation.standard_error - variance.sqrt() / (trials as f64).sqrt()).abs() < 1e-14);
                let (low, high) = validation.approximate_mean_interval_95;
                if variance > 0.0 {
                    assert!(low < validation.mean && high > validation.mean);
                } else {
                    assert_eq!((low, high), (validation.mean, validation.mean));
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 384);
    eprintln!("SAA: {checked} independently counted holdouts, 0 refusals");
}

#[test]
fn cvar_optimization_matches_all_affine_intersections_and_fractional_tail_mass() {
    let mut computed = 0;
    let mut refused = 0;
    for code in 0..27 {
        let offsets = [(code % 3) as f64 - 1.0, ((code / 3) % 3) as f64 - 1.0, (code / 9) as f64 - 1.0];
        let slopes = [-1.0, 0.0, 1.0];
        let mut candidates = vec![-2.0, 2.0];
        for i in 0..3 {
            for j in (i + 1)..3 {
                let x = (offsets[j] - offsets[i]) / (slopes[i] - slopes[j]);
                if (-2.0..=2.0).contains(&x) {
                    candidates.push(x);
                }
            }
        }
        for weights in [[1.0, 1.0, 1.0], [1.0, 2.0, 1.0], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0]] {
            for confidence in [0.0, 0.25, 0.5, 0.75, 0.9] {
                let objective = |x: f64| {
                    let losses = [slopes[0] * x + offsets[0], slopes[1] * x + offsets[1], slopes[2] * x + offsets[2]];
                    let mut indices = [0, 1, 2];
                    indices.sort_by(|i, j| losses[*j].total_cmp(&losses[*i]));
                    let tail = (1.0 - confidence) * weights.iter().sum::<f64>();
                    let mut remaining = tail;
                    let mut area = 0.0;
                    for i in indices {
                        let consumed = remaining.min(weights[i]);
                        area += consumed * losses[i];
                        remaining -= consumed;
                    }
                    area / tail
                };
                let expected = candidates.iter().copied().map(objective).fold(f64::INFINITY, f64::min);
                for scale in [1e-200, 1.0, 1e200] {
                    for offset in [0.0, 1024.0] {
                        let problem = ScenarioProblem {
                            name: "CVaR affine intersection oracle".into(),
                            variables: vec![variable(-2.0, 2.0)],
                            constraints: vec![],
                            scenarios: (0..3)
                                .map(|i| Scenario { weight: weights[i] * scale, offset: offset + offsets[i], coefficients: vec![slopes[i]] })
                                .collect(),
                            measure: RiskMeasure::ConditionalValueAtRisk { confidence },
                        };
                        let result = optimize(&problem, &SolveOptions::default(), &NoControl).unwrap();
                        if let Some(x) = result.decision_values.as_ref().map(|v| v[0]) {
                            close(objective(x), expected);
                            close(result.cvar.unwrap(), offset + expected);
                            close(result.solve.objective.unwrap(), offset + expected);
                            assert_eq!(result.solve.optimality, Optimality::Optimal, "{result:?}");
                            if let Some(bound) = result.solve.best_bound {
                                assert!(bound <= offset + expected + 2e-6);
                            }
                            computed += 1;
                        } else {
                            assert_eq!(result.solve.termination, Termination::NumericalFailure);
                            assert_eq!(result.solve.feasibility, Feasibility::Unknown);
                            assert_eq!(result.solve.optimality, Optimality::Unknown);
                            refused += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(computed + refused, 3240);
    eprintln!("CVaR: {computed} intersection-oracle optima + {refused} honest numerical refusals");
}

#[test]
fn joint_and_individual_chance_policies_match_original_two_condition_enumeration() {
    use solvercraft::chance::*;
    let options = SolveOptions::default();
    let mut feasible = 0;
    let mut infeasible = 0;
    let mut refused = 0;
    for intervals in [
        [[-1.0, 0.0], [0.0, 1.0], [1.0, 2.0]],
        [[-1.0, 1.0], [0.0, 2.0], [-1.0, 2.0]],
        [[0.0, 0.0], [-1.0, 2.0], [1.0, 1.0]],
        [[-1.0, 2.0], [-1.0, 2.0], [-1.0, 2.0]],
    ] {
        for weights in [[1.0, 1.0, 1.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0], [1.0, 2.0, 1.0], [0.0, 0.0, 1.0]] {
            let total = weights.iter().sum::<f64>();
            let masses = |x: f64| {
                let mut upper = 0.0;
                let mut lower = 0.0;
                let mut joint = 0.0;
                for i in 0..3 {
                    let hi = x > intervals[i][1] + options.feasibility_tolerance;
                    let lo = x < intervals[i][0] - options.feasibility_tolerance;
                    if hi {
                        upper += weights[i];
                    }
                    if lo {
                        lower += weights[i];
                    }
                    if hi || lo {
                        joint += weights[i];
                    }
                }
                [upper / total, lower / total, joint / total]
            };
            let mut rules = vec![];
            for a in [0.0, 0.5, 1.0] {
                rules.push(ChanceRule::Joint { maximum_violation_probability: a });
                for b in [0.0, 0.5, 1.0] {
                    rules.push(ChanceRule::Individual { maximum_violation_probabilities: vec![a, b] });
                }
            }
            for scale in [1e-200, 1.0, 1e200] {
                for rule in &rules {
                    for direction in [Direction::Minimize, Direction::Maximize] {
                        let allowed = |x: f64| {
                            let mass = masses(x);
                            match rule {
                                ChanceRule::Joint { maximum_violation_probability } => {
                                    mass[2] <= maximum_violation_probability + options.feasibility_tolerance
                                }
                                ChanceRule::Individual { maximum_violation_probabilities } => {
                                    mass[0] <= maximum_violation_probabilities[0] + options.feasibility_tolerance
                                        && mass[1] <= maximum_violation_probabilities[1] + options.feasibility_tolerance
                                }
                            }
                        };
                        let expected = [-1.0, 0.0, 1.0, 2.0]
                            .into_iter()
                            .filter(|x| allowed(*x))
                            .reduce(|a, b| if direction == Direction::Maximize { a.max(b) } else { a.min(b) });
                        let mut model = base(-1.0, 2.0);
                        model.objective.direction = direction;
                        let problem = ChanceProblem {
                            base: model,
                            scenarios: (0..3)
                                .map(|i| ChanceScenario {
                                    weight: weights[i] * scale,
                                    constraints: vec![
                                        LinearConstraint {
                                            name: "upper".into(),
                                            terms: vec![t(0, 1.0)],
                                            relation: Relation::LessEqual,
                                            rhs: intervals[i][1],
                                        },
                                        LinearConstraint {
                                            name: "lower".into(),
                                            terms: vec![t(0, 1.0)],
                                            relation: Relation::GreaterEqual,
                                            rhs: intervals[i][0],
                                        },
                                    ],
                                })
                                .collect(),
                            rule: rule.clone(),
                        };
                        let result = solve(&problem, &options, &NoControl).unwrap();
                        if result.solve.termination == Termination::NumericalFailure && result.solve.values.is_none() {
                            assert_eq!(result.solve.feasibility, Feasibility::Unknown);
                            refused += 1;
                            continue;
                        }
                        if let Some(expected) = expected {
                            let x = result.solve.values.as_ref().unwrap()[0];
                            close(x, expected);
                            close(result.solve.objective.unwrap(), expected);
                            assert_eq!(result.solve.optimality, Optimality::Optimal);
                            let mass = masses(x);
                            let reported = result.individual_violation_probabilities.as_ref().unwrap();
                            close(reported[0], mass[0]);
                            close(reported[1], mass[1]);
                            close(result.joint_violation_probability.unwrap(), mass[2]);
                            feasible += 1;
                        } else {
                            assert_eq!(result.solve.feasibility, Feasibility::Infeasible, "{result:?}");
                            assert!(result.solve.values.is_none() && result.solve.objective.is_none());
                            infeasible += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(feasible + infeasible + refused, 1728);
    eprintln!("two-condition chance: {feasible} feasible optima + {infeasible} correctly infeasible + {refused} honest refusals");
}
