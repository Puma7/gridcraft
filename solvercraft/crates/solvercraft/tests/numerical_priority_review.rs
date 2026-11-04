use solvercraft::{decision::*, multiobjective::*, types::*};

fn variable() -> Variable {
    Variable { name: "x".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous }
}
fn term(coefficient: f64) -> LinearTerm {
    LinearTerm { variable: 0, coefficient }
}
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "priority offset invariant".into(),
        variables: vec![variable()],
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: vec![],
    }
}

#[test]
fn lexicographic_priorities_do_not_change_when_a_goal_has_a_large_constant() {
    for offset in [0.0, 1e16, -1e16] {
        let goals = vec![
            LinearGoal { name: "first maximize x".into(), direction: Direction::Maximize, offset, terms: vec![term(1.0)] },
            LinearGoal { name: "then minimize x".into(), direction: Direction::Minimize, offset: 0.0, terms: vec![term(1.0)] },
        ];
        let result = solvercraft::multiobjective::solve(
            &model(),
            &goals,
            &MultiobjectiveMethod::Lexicographic { absolute_tolerances: vec![0.0, 0.0] },
            &SolveOptions::default(),
            &NoControl,
        )
        .unwrap();
        assert_eq!(result.values.as_ref().unwrap()[0], 1.0, "offset {offset}: {result:?}");
    }
}

#[test]
fn acquiring_an_uninformative_signal_keeps_its_one_unit_cost_at_large_payoff_offset() {
    for offset in [0.0, 1e16, -1e16] {
        let p = InformationProblem {
            state_probabilities: vec![1.0],
            action_payoffs: vec![vec![offset]],
            signal_likelihoods: vec![vec![1.0]],
            acquisition_cost: 1.0,
            utility: Utility::Linear {},
        };
        let result = information_value(&p, 16 * 1024 * 1024).unwrap();
        assert_eq!(result.net_value_of_signal, -1.0, "offset {offset}: {result:?}");
    }
}

#[test]
fn saa_does_not_report_exact_zero_uncertainty_when_nonconstant_variance_is_unrepresentable() {
    use solvercraft::{risk::RiskMeasure, saa::*};
    let problem = SampleAverageProblem {
        name: "underflow holdout".into(),
        variables: vec![Variable { upper: Some(0.0), ..variable() }],
        constraints: vec![],
        loss: SampledAffineLoss {
            offset: Distribution::Uniform { low: -1e-200, high: 1e-200 },
            coefficients: vec![Distribution::Constant { value: 0.0 }],
        },
        measure: RiskMeasure::ExpectedLoss,
    };
    let result = train_and_validate(
        &problem,
        &SampleAverageOptions { training_trials: 16, validation_trials: 32, training_seed: 42, validation_seed: 43 },
        &SolveOptions::default(),
        &NoControl,
    );
    assert!(result.is_err(), "Must reject unrepresentable nonzero variance, not display SE=0: {result:?}");
}

fn fixed_loss_holdout(offset: Distribution, validation_trials: u64) -> solvercraft::types::Result<solvercraft::saa::SampleAverageResult> {
    use solvercraft::{risk::RiskMeasure, saa::*};
    train_and_validate(
        &SampleAverageProblem {
            name: "holdout interval precision".into(),
            variables: vec![Variable { upper: Some(0.0), ..variable() }],
            constraints: vec![],
            loss: SampledAffineLoss { offset, coefficients: vec![Distribution::Constant { value: 0.0 }] },
            measure: RiskMeasure::ExpectedLoss,
        },
        &SampleAverageOptions { training_trials: 16, validation_trials, training_seed: 42, validation_seed: 43 },
        &SolveOptions::default(),
        &NoControl,
    )
}

#[test]
fn saa_nonzero_interval_outwardly_encloses_the_mean_between_adjacent_large_losses() {
    let low = 1e10_f64;
    for trials in [32, 1024] {
        let result = fixed_loss_holdout(Distribution::Categorical { values: vec![low, low.next_up()], weights: vec![1.0, 1.0] }, trials).unwrap();
        let validation = result.validation.unwrap();
        assert!(validation.sample_variance > 0.0 && validation.standard_error > 0.0);
        let (lower, upper) = validation.approximate_mean_interval_95;
        assert!(lower < validation.mean && upper > validation.mean, "trials={trials}: {validation:?}");
    }
}

#[test]
fn saa_subnormal_variance_is_an_explicit_supported_range_rejection() {
    let result = fixed_loss_holdout(Distribution::Categorical { values: vec![-1e-160, 1e-160], weights: vec![1.0, 1.0] }, 1024);
    assert_eq!(result.unwrap_err().code, "numerical_failure");
}

#[test]
fn saa_constant_holdout_stays_an_exact_point_and_records_catalog_identity() {
    for trials in [32, 1024] {
        let result = fixed_loss_holdout(Distribution::Constant { value: 1e10 }, trials).unwrap();
        assert_eq!(result.sampling_version, format!("{}/{}", solvercraft::math::SAMPLING_VERSION, solvercraft::math::catalog::CATALOG_VERSION));
        let validation = result.validation.unwrap();
        assert_eq!(validation.sample_variance, 0.0);
        assert_eq!(validation.standard_error, 0.0);
        assert_eq!(validation.approximate_mean_interval_95, (1e10, 1e10));
    }
}

#[test]
fn information_advantages_survive_rounded_certainty_equivalent_levels() {
    for offset in [0.0, 1e16, -1e16] {
        let problem = InformationProblem {
            state_probabilities: vec![0.5, 0.5],
            action_payoffs: vec![vec![offset + 2.0, offset], vec![offset, offset + 2.0]],
            signal_likelihoods: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            acquisition_cost: 0.25,
            utility: Utility::Linear {},
        };
        let actual = information_value(&problem, 16 * 1024 * 1024).unwrap();
        assert_eq!(actual.value_of_perfect_information, 1.0, "offset={offset}");
        assert_eq!(actual.net_value_of_signal, 0.75, "offset={offset}");
        assert_eq!(actual.signal_actions, vec![0, 1]);
    }
}

#[test]
fn lexicographic_absolute_tolerance_is_offset_invariant_in_both_directions() {
    for offset in [0.0, 1e16, -1e16] {
        for first_direction in [Direction::Minimize, Direction::Maximize] {
            for tolerance in [0.0, 0.25, 0.5] {
                let second_direction = if first_direction == Direction::Minimize { Direction::Maximize } else { Direction::Minimize };
                let goals = vec![
                    LinearGoal { name: "first priority".into(), direction: first_direction, offset, terms: vec![term(1.0)] },
                    LinearGoal { name: "second priority".into(), direction: second_direction, offset: 0.0, terms: vec![term(1.0)] },
                ];
                let result = solvercraft::multiobjective::solve(
                    &model(),
                    &goals,
                    &MultiobjectiveMethod::Lexicographic { absolute_tolerances: vec![tolerance, 0.0] },
                    &SolveOptions::default(),
                    &NoControl,
                )
                .unwrap();
                let expected = if first_direction == Direction::Minimize { tolerance } else { 1.0 - tolerance };
                assert!(
                    (result.values.as_ref().unwrap()[0] - expected).abs() < 1e-10,
                    "offset={offset}, direction={first_direction:?}, tolerance={tolerance}: {result:?}"
                );
            }
        }
    }
}
