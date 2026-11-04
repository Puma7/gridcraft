use solvercraft::decision::*;

#[test]
fn a_rounded_away_tree_cost_cannot_silently_choose_the_more_expensive_action() {
    let tree = DecisionTree {
        root: 0,
        nodes: vec![
            DecisionNode::Decision { alternatives: vec![TreeEdge { child: 1, cost: 1.0 }, TreeEdge { child: 1, cost: 0.0 }] },
            DecisionNode::Terminal { payoff: 1e16 },
        ],
        utility: Utility::Linear {},
    };
    match evaluate(&tree, 1024 * 1024) {
        Ok(result) => assert_eq!(result.choices[0], Some(1), "More expensive action was selected: {result:?}"),
        Err(error) => assert_eq!(error.code, "invalid_decision_tree"),
    }
}

#[test]
fn unrepresentable_exponential_utility_precision_cannot_erase_a_known_acquisition_cost() {
    for risk_tolerance in [1e16, 1e200] {
        for payoff in [0.0, 1e16] {
            let problem = InformationProblem {
                state_probabilities: vec![1.0],
                action_payoffs: vec![vec![payoff]],
                signal_likelihoods: vec![vec![1.0]],
                acquisition_cost: 1.0,
                utility: Utility::Exponential { risk_tolerance },
            };
            match information_value(&problem, 1024 * 1024) {
                Ok(result) => assert!((result.net_value_of_signal + 1.0).abs() < 1e-12, "payoff={payoff},risk={risk_tolerance}: {result:?}"),
                Err(error) => assert_eq!(error.code, "invalid_decision_tree"),
            }
        }
    }
}

fn digits(mut code: usize, radix: usize, center: f64) -> [[f64; 2]; 2] {
    let mut payoffs = [[0.0; 2]; 2];
    for action in &mut payoffs {
        for payoff in action {
            *payoff = (code % radix) as f64 - center;
            code /= radix;
        }
    }
    payoffs
}

fn first_best(values: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..values.len() {
        if values[i] > values[best] {
            best = i;
        }
    }
    best
}

fn arithmetic_rounding(actual: f64, expected: f64) {
    // Fixed forward-error budget for two-state conditioning/marginalization;
    // policies and exact tree arithmetic are checked separately without it.
    assert!((actual - expected).abs() <= 16.0 * f64::EPSILON * expected.abs().max(1.0), "actual={actual}, oracle={expected}");
}

#[test]
fn ninety_thousand_linear_information_cases_match_direct_policy_enumeration() {
    let mut checked = 0;
    for code in 0..625 {
        let p = digits(code, 5, 2.0);
        // Include a strictly dominated alternative and an exact policy duplicate.
        let payoffs = [p[0], p[1], [-3.0, -3.0], p[0]];
        for prior in [0.0, 0.25, 0.5, 1.0] {
            for signal in [[0.0, 0.0], [0.5, 0.5], [1.0, 0.0], [0.25, 0.75]] {
                let probabilities = [prior, 1.0 - prior];
                let law = [signal, [1.0 - signal[0], 1.0 - signal[1]]];
                let expected: Vec<_> = payoffs.iter().map(|v| prior * v[0] + (1.0 - prior) * v[1]).collect();
                let baseline = first_best(&expected);
                let perfect =
                    (0..2).map(|state| probabilities[state] * payoffs.iter().map(|v| v[state]).fold(f64::NEG_INFINITY, f64::max)).sum::<f64>();
                // Enumerate all sixteen observation-dependent action policies.
                let gross_signal = (0..16)
                    .map(|policy| {
                        let actions = [policy % 4, policy / 4];
                        (0..2).flat_map(|s| (0..2).map(move |state| probabilities[state] * law[s][state] * payoffs[actions[s]][state])).sum::<f64>()
                    })
                    .fold(f64::NEG_INFINITY, f64::max);
                for offset in [0.0, 1_099_511_627_776.0, -1_099_511_627_776.0] {
                    for cost in [0.0, 0.25, 2.0] {
                        let result = information_value(
                            &InformationProblem {
                                state_probabilities: probabilities.to_vec(),
                                action_payoffs: payoffs.iter().map(|p| p.iter().map(|v| v + offset).collect()).collect(),
                                signal_likelihoods: law.iter().map(|v| v.to_vec()).collect(),
                                acquisition_cost: cost,
                                utility: Utility::Linear {},
                            },
                            1024 * 1024,
                        )
                        .unwrap();
                        assert_eq!(result.baseline_action, baseline, "code={code}, prior={prior}, signal={signal:?}, offset={offset}");
                        arithmetic_rounding(result.baseline_certainty_equivalent, offset + expected[baseline]);
                        arithmetic_rounding(result.perfect_information_certainty_equivalent, offset + perfect);
                        arithmetic_rounding(result.signal_certainty_equivalent, offset + gross_signal - cost);
                        arithmetic_rounding(result.net_value_of_signal, gross_signal - cost - expected[baseline]);
                        arithmetic_rounding(result.value_of_perfect_information, perfect - expected[baseline]);
                        for s in 0..2 {
                            let mass: f64 = (0..2).map(|state| probabilities[state] * law[s][state]).sum();
                            if mass > 0.0 {
                                let scores: Vec<_> = payoffs
                                    .iter()
                                    .map(|p| (0..2).map(|state| probabilities[state] * law[s][state] * p[state]).sum::<f64>())
                                    .collect();
                                assert_eq!(scores[result.signal_actions[s]], scores[first_best(&scores)]);
                            } else {
                                assert_eq!(result.signal_actions[s], 0);
                            }
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 90_000);
    eprintln!("linear information: {checked} accepted computations, 0 refusals");
}

#[test]
fn thirty_four_thousand_exponential_information_cases_match_unconditional_policy_values() {
    let mut checked = 0;
    for code in 0..81 {
        let payoffs = digits(code, 3, 1.0);
        for prior in [0.0, 0.25, 0.5, 1.0] {
            for signal in [[0.0, 0.0], [0.5, 0.5], [1.0, 0.0], [0.25, 0.75]] {
                let probabilities = [prior, 1.0 - prior];
                let law = [signal, [1.0 - signal[0], 1.0 - signal[1]]];
                for tolerance in [0.5, 2.0, 10.0] {
                    let utility = |v: f64| -(-v / tolerance).exp();
                    let ce = |v: f64| -tolerance * (-v).ln();
                    let baseline = payoffs.iter().map(|p| prior * utility(p[0]) + (1.0 - prior) * utility(p[1])).fold(f64::NEG_INFINITY, f64::max);
                    let perfect = (0..2).map(|state| probabilities[state] * utility(payoffs[0][state].max(payoffs[1][state]))).sum::<f64>();
                    for offset in [-10.0, 0.0, 10.0] {
                        for cost in [0.0, 0.25, 1.0] {
                            let signal_utility = (0..4)
                                .map(|policy| {
                                    let actions = [policy % 2, policy / 2];
                                    (0..2)
                                        .flat_map(|s| {
                                            (0..2).map(move |state| probabilities[state] * law[s][state] * utility(payoffs[actions[s]][state] - cost))
                                        })
                                        .sum::<f64>()
                                })
                                .fold(f64::NEG_INFINITY, f64::max);
                            let result = information_value(
                                &InformationProblem {
                                    state_probabilities: probabilities.to_vec(),
                                    action_payoffs: payoffs.iter().map(|p| p.iter().map(|v| v + offset).collect()).collect(),
                                    signal_likelihoods: law.iter().map(|v| v.to_vec()).collect(),
                                    acquisition_cost: cost,
                                    utility: Utility::Exponential { risk_tolerance: tolerance },
                                },
                                1024 * 1024,
                            )
                            .unwrap();
                            assert!((result.baseline_certainty_equivalent - (offset + ce(baseline))).abs() < 2e-12);
                            assert!((result.perfect_information_certainty_equivalent - (offset + ce(perfect))).abs() < 2e-12);
                            assert!((result.signal_certainty_equivalent - (offset + ce(signal_utility))).abs() < 2e-12);
                            assert!((result.net_value_of_signal - (ce(signal_utility) - ce(baseline))).abs() < 2e-12);
                            assert!((result.value_of_perfect_information - (ce(perfect) - ce(baseline))).abs() < 2e-12);
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 34_992);
    eprintln!("exponential information: {checked} accepted computations, 0 refusals");
}

#[test]
fn thirty_thousand_tree_policies_and_values_match_direct_leaf_averages() {
    let mut checked = 0;
    for code in 0..625 {
        let p = digits(code, 5, 2.0);
        for prior in [0.0, 0.25, 0.5, 1.0] {
            for offset in [0.0, 1_099_511_627_776.0, -1_099_511_627_776.0] {
                for costs in [[0.0, 0.0], [0.25, 0.0], [0.0, 0.5], [-0.25, 0.25]] {
                    let scores = [prior * p[0][0] + (1.0 - prior) * p[0][1] - costs[0], prior * p[1][0] + (1.0 - prior) * p[1][1] - costs[1]];
                    let chosen = first_best(&scores);
                    let tree = DecisionTree {
                        root: 0,
                        nodes: vec![
                            DecisionNode::Decision {
                                alternatives: vec![TreeEdge { child: 1, cost: costs[0] }, TreeEdge { child: 2, cost: costs[1] }],
                            },
                            DecisionNode::Chance {
                                outcomes: vec![
                                    ChanceEdge { child: 3, cost: 0.0, probability: prior },
                                    ChanceEdge { child: 4, cost: 0.0, probability: 1.0 - prior },
                                ],
                            },
                            DecisionNode::Chance {
                                outcomes: vec![
                                    ChanceEdge { child: 5, cost: 0.0, probability: prior },
                                    ChanceEdge { child: 6, cost: 0.0, probability: 1.0 - prior },
                                ],
                            },
                            DecisionNode::Terminal { payoff: offset + p[0][0] },
                            DecisionNode::Terminal { payoff: offset + p[0][1] },
                            DecisionNode::Terminal { payoff: offset + p[1][0] },
                            DecisionNode::Terminal { payoff: offset + p[1][1] },
                        ],
                        utility: Utility::Linear {},
                    };
                    let result = evaluate(&tree, 1024 * 1024).unwrap();
                    assert_eq!(result.choices[0], Some(chosen), "code={code}, prior={prior}, offset={offset}, costs={costs:?}: {result:?}");
                    assert_eq!(result.expected_utility, offset + scores[chosen]);
                    assert_eq!(result.certainty_equivalent, offset + scores[chosen]);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 30_000);
    eprintln!("linear trees: {checked} accepted computations, 0 refusals");
}
