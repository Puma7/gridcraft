use solvercraft::{chance::*, decision::*, finite::*, multiobjective::*, types::*};
fn v(name: &str, lower: f64, upper: f64, kind: VariableKind) -> Variable {
    Variable { name: name.into(), lower: Some(lower), upper: Some(upper), kind }
}
fn t(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn row(terms: Vec<LinearTerm>, relation: Relation, rhs: f64) -> LinearConstraint {
    LinearConstraint { name: "row".into(), terms, relation, rhs }
}
fn model(variables: Vec<Variable>, linear: Vec<LinearTerm>) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "test".into(),
        variables,
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear, quadratic: vec![] },
        constraints: vec![],
    }
}
#[test]
fn finite_domains_all_different_match_exhaustive_assignment() {
    let base = model(vec![v("x", 0.0, 3.0, VariableKind::Continuous), v("y", 0.0, 3.0, VariableKind::Continuous)], vec![t(0, 1.0), t(1, 2.0)]);
    let spec = FiniteConstraints {
        domains: vec![FiniteDomain { variable: 0, values: vec![1.0, 3.0] }, FiniteDomain { variable: 1, values: vec![1.0, 2.0] }],
        all_different: vec![vec![0, 1]],
        ..Default::default()
    };
    let result = solvercraft::finite::solve(&base, &spec, &SolveOptions::default(), &NoControl).expect("finite");
    let oracle =
        [(1.0, 1.0), (1.0, 2.0), (3.0, 1.0), (3.0, 2.0)].into_iter().filter(|(x, y)| x != y).map(|(x, y)| x + 2.0 * y).fold(f64::INFINITY, f64::min);
    assert!((result.objective.expect("obj") - oracle).abs() < 1e-7);
    assert_eq!(result.values.expect("values").len(), 2);
}
#[test]
fn indicators_derive_bounds_and_reject_open_domains() {
    let mut base = model(vec![v("x", -2.0, 10.0, VariableKind::Continuous), v("z", 1.0, 1.0, VariableKind::Binary)], vec![t(0, -1.0)]);
    let spec = FiniteConstraints {
        indicators: vec![Indicator { binary: 1, active_when: true, constraint: row(vec![t(0, 1.0)], Relation::LessEqual, 3.0) }],
        ..Default::default()
    };
    let result = solvercraft::finite::solve(&base, &spec, &SolveOptions::default(), &NoControl).expect("indicator");
    assert!((result.values.expect("x")[0] - 3.0).abs() < 1e-7);
    base.variables[0].upper = None;
    assert_eq!(solvercraft::finite::compile(&base, &spec, &SolveOptions::default()).expect_err("bound").code, "invalid_finite_constraint");
}
#[test]
fn sos2_prohibits_nonadjacent_nonzero_values() {
    let base = model((0..3).map(|i| v(&format!("x{i}"), 0.0, 1.0, VariableKind::Continuous)).collect(), vec![t(0, -1.0), t(2, -1.0)]);
    for (kind, expected) in [(SosKind::One, -1.0), (SosKind::Two, -1.0)] {
        let spec = FiniteConstraints { sos: vec![SpecialOrderedSet { kind, variables: vec![0, 1, 2] }], ..Default::default() };
        let solved = solvercraft::finite::solve(&base, &spec, &SolveOptions::default(), &NoControl).expect("SOS");
        assert!((solved.objective.expect("cost") - expected).abs() < 1e-7);
    }
}
#[test]
fn reification_observes_explicit_separated_complement() {
    let base = model(vec![v("x", 0.0, 4.0, VariableKind::Continuous), v("z", 0.0, 0.0, VariableKind::Binary)], vec![t(0, 1.0)]);
    let spec = FiniteConstraints {
        reified: vec![ReifiedInequality { binary: 1, constraint: row(vec![t(0, 1.0)], Relation::LessEqual, 2.0), separation: 0.25 }],
        ..Default::default()
    };
    let solved = solvercraft::finite::solve(&base, &spec, &SolveOptions::default(), &NoControl).expect("reified");
    assert!((solved.values.expect("x")[0] - 2.25).abs() < 1e-7);
}
#[test]
fn joint_and_individual_chance_constraints_have_different_semantics() {
    let base = model(vec![v("x", 0.0, 2.0, VariableKind::Continuous), v("y", 0.0, 2.0, VariableKind::Continuous)], vec![t(0, 1.0), t(1, 1.0)]);
    let scenarios = vec![
        ChanceScenario {
            weight: 1.0,
            constraints: vec![row(vec![t(0, 1.0)], Relation::GreaterEqual, 2.0), row(vec![t(1, 1.0)], Relation::GreaterEqual, 0.0)],
        },
        ChanceScenario {
            weight: 1.0,
            constraints: vec![row(vec![t(0, 1.0)], Relation::GreaterEqual, 0.0), row(vec![t(1, 1.0)], Relation::GreaterEqual, 2.0)],
        },
    ];
    let individual = ChanceProblem {
        base: base.clone(),
        scenarios: scenarios.clone(),
        rule: ChanceRule::Individual { maximum_violation_probabilities: vec![0.5, 0.5] },
    };
    let solved = solvercraft::chance::solve(&individual, &SolveOptions::default(), &NoControl).expect("individual");
    assert!(solved.solve.objective.expect("obj").abs() < 1e-7);
    assert_eq!(solved.joint_violation_probability, Some(1.0));
    let joint = ChanceProblem { base, scenarios, rule: ChanceRule::Joint { maximum_violation_probability: 0.5 } };
    let solved = solvercraft::chance::solve(&joint, &SolveOptions::default(), &NoControl).expect("joint");
    assert!((solved.solve.objective.expect("obj") - 2.0).abs() < 1e-7);
    assert_eq!(solved.joint_violation_probability, Some(0.5));
}
#[test]
fn decision_backward_evaluation_utility_and_cycle_guard() {
    let tree = DecisionTree {
        root: 0,
        utility: Utility::Linear {},
        nodes: vec![
            DecisionNode::Decision { alternatives: vec![TreeEdge { child: 1, cost: 1.0 }, TreeEdge { child: 2, cost: 0.0 }] },
            DecisionNode::Chance {
                outcomes: vec![ChanceEdge { child: 3, cost: 0.0, probability: 0.5 }, ChanceEdge { child: 4, cost: 0.0, probability: 0.5 }],
            },
            DecisionNode::Terminal { payoff: 3.0 },
            DecisionNode::Terminal { payoff: 0.0 },
            DecisionNode::Terminal { payoff: 10.0 },
        ],
    };
    let result = evaluate(&tree, 1_000_000).expect("tree");
    assert_eq!(result.certainty_equivalent, 4.0);
    assert_eq!(result.choices[0], Some(0));
    let mut risk_averse = tree.clone();
    risk_averse.utility = Utility::Exponential { risk_tolerance: 1.0 };
    let result = evaluate(&risk_averse, 1_000_000).expect("utility");
    assert_eq!(result.choices[0], Some(1));
    assert!((result.certainty_equivalent - 3.0).abs() < 1e-12);
    let cyclic = DecisionTree {
        root: 0,
        utility: Utility::Linear {},
        nodes: vec![DecisionNode::Decision { alternatives: vec![TreeEdge { child: 0, cost: 0.0 }] }],
    };
    assert!(evaluate(&cyclic, 1_000_000).is_err());
}
#[test]
fn information_is_observed_before_action_and_cost_is_paid_once() {
    let problem = InformationProblem {
        state_probabilities: vec![0.5, 0.5],
        action_payoffs: vec![vec![10.0, 0.0], vec![0.0, 10.0]],
        signal_likelihoods: vec![vec![0.8, 0.2], vec![0.2, 0.8]],
        acquisition_cost: 1.0,
        utility: Utility::Linear {},
    };
    let result = information_value(&problem, 1_000_000).expect("VOI");
    assert_eq!(result.baseline_certainty_equivalent, 5.0);
    assert_eq!(result.perfect_information_certainty_equivalent, 10.0);
    assert!((result.signal_certainty_equivalent - 7.0).abs() < 1e-12);
    assert!((result.net_value_of_signal - 2.0).abs() < 1e-12);
    assert_eq!(result.signal_actions, vec![0, 1]);
}
#[test]
fn information_value_rejects_unrepresentable_finite_payoff_spread() {
    let p = InformationProblem {
        state_probabilities: vec![0.2; 5],
        action_payoffs: (0..5).map(|a| (0..5).map(|s| if a == s { f64::MAX } else { -f64::MAX }).collect()).collect(),
        signal_likelihoods: vec![vec![1.0; 5]],
        acquisition_cost: 0.0,
        utility: Utility::Linear {},
    };
    assert_eq!(information_value(&p, 1_000_000).expect_err("overflow must be explicit").code, "numerical_failure");
}
#[test]
fn finite_domain_precision_and_saa_cancellation_are_explicit() {
    let base = model(vec![v("x", 0.0, 2.0, VariableKind::Continuous), v("y", 0.0, 2.0, VariableKind::Continuous)], vec![]);
    let spec = FiniteConstraints {
        domains: vec![FiniteDomain { variable: 0, values: vec![1.0] }, FiniteDomain { variable: 1, values: vec![1.0 + 1e-10] }],
        all_different: vec![vec![0, 1]],
        ..Default::default()
    };
    assert_eq!(
        solvercraft::finite::compile(&base, &spec, &SolveOptions::default()).expect_err("indistinguishable domains").code,
        "unsupported_finite_precision"
    );
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let p = solvercraft::saa::SampleAverageProblem {
        name: "cancel".into(),
        variables: vec![v("x", 0.0, 1.0, VariableKind::Continuous)],
        constraints: vec![],
        loss: solvercraft::saa::SampledAffineLoss {
            offset: Distribution::Constant { value: 0.0 },
            coefficients: vec![Distribution::Constant { value: 1.0 }],
        },
        measure: solvercraft::risk::RiskMeasure::ExpectedLoss,
    };
    let sampling = solvercraft::saa::SampleAverageOptions { training_trials: 1000, validation_trials: 1000, training_seed: 1, validation_seed: 2 };
    assert_eq!(solvercraft::saa::train_and_validate(&p, &sampling, &SolveOptions::default(), &Cancel).expect_err("cancel").code, "cancelled");
}
#[test]
fn indicator_exact_slack_gate_preserves_tiny_active_rhs_beside_huge_m() {
    for upper in [10.0, 1e20] {
        let base = model(vec![v("x", 0.0, upper, VariableKind::Continuous), v("b", 0.0, 1.0, VariableKind::Binary)], vec![t(0, -1.0)]);
        for relation in [Relation::LessEqual, Relation::Equal] {
            let spec = FiniteConstraints {
                indicators: vec![Indicator { binary: 1, active_when: true, constraint: row(vec![t(0, 1.0)], relation, 3.0) }],
                ..Default::default()
            };
            let compiled = solvercraft::finite::compile(&base, &spec, &SolveOptions::default()).expect("gate compile");
            let mut point = vec![3.0, 1.0];
            point.resize(compiled.variables.len(), 0.0);
            assert!(
                solvercraft::model::validate_solution(&compiled, &point, &SolveOptions::default()).expect("original feasible active point").feasible
            );
            assert_eq!(compiled.variables.len(), 2 + if relation == Relation::Equal { 2 } else { 1 });
        }
    }
}
#[test]
fn uninformative_free_signal_has_zero_value_even_with_accepted_prior_roundoff() {
    let p = InformationProblem {
        state_probabilities: vec![0.5, 0.500_000_000_000_5],
        action_payoffs: vec![vec![1e17, 1e17]],
        signal_likelihoods: vec![vec![1.0, 1.0]],
        acquisition_cost: 0.0,
        utility: Utility::Linear {},
    };
    let result = information_value(&p, 1_000_000).expect("accepted near-unit probabilities");
    assert_eq!(result.net_value_of_signal, 0.0, "An uninformative signal cannot create value out of probability normalizing error: {result:?}");
    assert_eq!(result.value_of_perfect_information, 0.0);
}
#[test]
fn normalized_signal_law_respects_information_bounds() {
    for utility in [Utility::Linear {}, Utility::Exponential { risk_tolerance: 100.0 }] {
        for acquisition_cost in [0.0, 3.0] {
            let p = InformationProblem {
                state_probabilities: vec![0.4, 0.600_000_000_000_4],
                action_payoffs: vec![vec![100.0, 0.0], vec![0.0, 100.0]],
                signal_likelihoods: vec![vec![0.5, 0.5], vec![0.500_000_000_000_4, 0.500_000_000_000_4]],
                acquisition_cost,
                utility,
            };
            let result = information_value(&p, 1_000_000).expect("uninformative near-unit law");
            assert!((result.net_value_of_signal + acquisition_cost).abs() < 1e-10, "{result:?}");
            assert!(result.net_value_of_signal <= result.value_of_perfect_information + 1e-10);
            let mut informative = p;
            informative.signal_likelihoods = vec![vec![1.0, 0.0], vec![0.0, 1.000_000_000_000_4]];
            let perfect = information_value(&informative, 1_000_000).expect("revealing signal");
            assert!((perfect.net_value_of_signal + acquisition_cost - perfect.value_of_perfect_information).abs() < 1e-10, "{perfect:?}");
        }
    }
}
#[test]
fn weighted_lexicographic_and_epsilon_methods_are_traceable() {
    let mut base = model(vec![v("x", 0.0, 1.0, VariableKind::Continuous), v("y", 0.0, 1.0, VariableKind::Continuous)], vec![]);
    base.constraints.push(row(vec![t(0, 1.0), t(1, 1.0)], Relation::GreaterEqual, 1.0));
    let goals = vec![
        LinearGoal { name: "x cost".into(), direction: Direction::Minimize, offset: 0.0, terms: vec![t(0, 1.0)] },
        LinearGoal { name: "y cost".into(), direction: Direction::Minimize, offset: 0.0, terms: vec![t(1, 1.0)] },
    ];
    let weighted = solvercraft::multiobjective::solve(
        &base,
        &goals,
        &MultiobjectiveMethod::Weighted { weights: vec![2.0, 1.0] },
        &SolveOptions::default(),
        &NoControl,
    )
    .expect("weighted");
    assert_eq!(weighted.goal_values.expect("goals"), vec![0.0, 1.0]);
    let lex = solvercraft::multiobjective::solve(
        &base,
        &goals,
        &MultiobjectiveMethod::Lexicographic { absolute_tolerances: vec![0.0, 0.0] },
        &SolveOptions::default(),
        &NoControl,
    )
    .expect("lex");
    assert_eq!(lex.stages.len(), 2);
    assert_eq!(lex.imposed_bounds.len(), 1);
    assert_eq!(lex.goal_values.expect("goals"), vec![0.0, 1.0]);
    let epsilon = solvercraft::multiobjective::solve(
        &base,
        &goals,
        &MultiobjectiveMethod::EpsilonConstraint { primary: 1, bounds: vec![EpsilonBound { goal: 0, value: 0.25 }] },
        &SolveOptions::default(),
        &NoControl,
    )
    .expect("epsilon");
    let values = epsilon.goal_values.expect("goals");
    assert!((values[0] - 0.25).abs() < 1e-7);
    assert!((values[1] - 0.75).abs() < 1e-7);
}
