use solvercraft::{risk::*, saa::*, types::*};
fn variable() -> Variable {
    Variable { name: "x".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous }
}
fn sampling() -> SampleAverageOptions {
    SampleAverageOptions { training_trials: 100, validation_trials: 200, training_seed: 42, validation_seed: 43 }
}
fn problem() -> SampleAverageProblem {
    SampleAverageProblem {
        name: "sampled loss".into(),
        variables: vec![variable()],
        constraints: vec![],
        loss: SampledAffineLoss {
            offset: Distribution::Normal { mean: 3.0, std_dev: 1.0 },
            coefficients: vec![Distribution::Uniform { low: -3.0, high: -1.0 }],
        },
        measure: RiskMeasure::ExpectedLoss,
    }
}
#[test]
fn saa_training_is_fixed_across_candidates_and_order() {
    let sampling = sampling();
    let prepared = training_problem(&problem(), &sampling, &SolveOptions::default()).expect("prepare");
    let x = vec![vec![0.0], vec![1.0], vec![0.5]];
    let results = compare_candidates(&prepared, &x, &SolveOptions::default()).expect("compare");
    let reversed = compare_candidates(&prepared, &x.iter().rev().cloned().collect::<Vec<_>>(), &SolveOptions::default()).expect("reversed");
    assert_eq!(results, reversed.into_iter().rev().collect::<Vec<_>>());
    let repeated = training_problem(&problem(), &sampling, &SolveOptions::default()).expect("repeat");
    assert_eq!(serde_json::to_value(prepared).expect("serialize"), serde_json::to_value(repeated).expect("serialize"));
}
#[test]
fn independent_holdout_uses_separate_seed_and_uncertainty() {
    let problem = problem();
    let sampling = sampling();
    let result = train_and_validate(&problem, &sampling, &SolveOptions::default(), &NoControl).expect("SAA");
    assert_eq!(result.training.decision_values, Some(vec![1.0]));
    let holdout = result.validation.expect("holdout");
    assert_eq!(holdout.trials, 200);
    assert!(holdout.standard_error > 0.0);
    assert!(holdout.approximate_mean_interval_95.0 < holdout.mean && holdout.mean < holdout.approximate_mean_interval_95.1);
    let mut reference = 0.0;
    for trial in 0..200 {
        reference += solvercraft::math::sample(&problem.loss.offset, 43, 0, trial).expect("draw")
            + solvercraft::math::sample(&problem.loss.coefficients[0], 43, 1, trial).expect("draw");
    }
    reference /= 200.0;
    assert!((holdout.mean - reference).abs() < 1e-12);
    assert_ne!(result.training.expected_loss, Some(holdout.mean));
    let bad = SampleAverageOptions { validation_seed: 42, ..sampling };
    assert!(train_and_validate(&problem, &bad, &SolveOptions::default(), &NoControl).is_err());
}
#[test]
fn mean_variance_objective_matches_analytic_quadratic_and_original_losses() {
    // Losses x and 2-3x with equal probability: mean 1-x, variance (2x-1)^2.
    // Mean + variance has derivative -1 + 8x-4; optimum x=5/8.
    let p = ScenarioProblem {
        name: "mean variance".into(),
        variables: vec![variable()],
        constraints: vec![],
        scenarios: vec![
            Scenario { weight: 1.0, offset: 0.0, coefficients: vec![1.0] },
            Scenario { weight: 1.0, offset: 2.0, coefficients: vec![-3.0] },
        ],
        measure: RiskMeasure::MeanVariance { penalty: 1.0 },
    };
    let result = optimize(&p, &SolveOptions::default(), &NoControl).expect("mean variance");
    assert_eq!(result.solve.optimality, Optimality::Optimal);
    assert!((result.decision_values.expect("x")[0] - 0.625).abs() < 1e-8);
    assert!((result.variance.expect("variance") - 0.0625).abs() < 1e-8);
    assert!((result.solve.objective.expect("objective") - 0.4375).abs() < 1e-8);
    assert_eq!(weighted_variance(&[2.0, 4.0], &[f64::MAX, f64::MAX]).expect("variance"), 1.0);
}
#[test]
fn budget_robust_support_matches_independent_fractional_knapsack_oracle() {
    // x fixed at [2,-3], radii [1,2]. Support Γ=1.5: 6 + 0.5*2 = 7.
    let mut m = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "budget support".into(),
        variables: vec![
            Variable { name: "x".into(), lower: Some(2.0), upper: Some(2.0), kind: VariableKind::Continuous },
            Variable { name: "y".into(), lower: Some(-3.0), upper: Some(-3.0), kind: VariableKind::Continuous },
        ],
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: vec![LinearConstraint { name: "uncertain".into(), terms: vec![], relation: Relation::LessEqual, rhs: 7.0 }],
    };
    let u = [BudgetUncertainty { constraint: 0, radii: vec![1.0, 2.0], budget: 1.5 }];
    let compiled = robust_budget(&m, &u, &SolveOptions::default()).expect("robust");
    let solved = solvercraft::solve(&compiled, &SolveOptions::default(), &NoControl).expect("solve");
    assert_eq!(solved.feasibility, Feasibility::Feasible);
    m.constraints[0].rhs = 6.9;
    let compiled = robust_budget(&m, &u, &SolveOptions::default()).expect("robust");
    let solved = solvercraft::solve(&compiled, &SolveOptions::default(), &NoControl).expect("solve");
    assert_eq!(solved.feasibility, Feasibility::Infeasible);
}
#[test]
fn mean_variance_keeps_ordinary_well_conditioned_decimal_models_solvable() {
    let p = ScenarioProblem {
        name: "decimal mean variance".into(),
        variables: vec![variable()],
        constraints: vec![],
        scenarios: vec![
            Scenario { weight: 1.0, offset: 0.2, coefficients: vec![0.1] },
            Scenario { weight: 1.0, offset: 0.3, coefficients: vec![0.2] },
        ],
        measure: RiskMeasure::MeanVariance { penalty: 0.5 },
    };
    // Both affine losses and their nonnegative slope increase on[0,1]. Atx=0,
    // mean=.25 and populationvariance=.0025, so objective=.25125.
    let result = optimize(&p, &SolveOptions::default(), &NoControl).expect("ordinary decimal risk model");
    assert!((result.decision_values.expect("x")[0]).abs() < 1e-7);
    assert!((result.solve.objective.expect("obj") - 0.25125).abs() < 1e-8);
    assert!(result.coefficient_roundoff_bound > 0.0);
    assert!(result.solve.best_bound.expect("original lower bound") <= 0.25125);
    let tight = optimize(&p, &SolveOptions { optimality_tolerance: 1e-30, ..Default::default() }, &NoControl).expect("truthful tight precision");
    assert_ne!(tight.solve.optimality, Optimality::Optimal, "roundoff cannot be certified smaller than the requested gap");
    let mut unbounded = p;
    unbounded.variables[0].upper = None;
    assert_eq!(
        compile(&unbounded, &SolveOptions::default()).expect_err("unbounded nonzero coefficient rounding impact").code,
        "unsupported_risk_precision"
    );
}
