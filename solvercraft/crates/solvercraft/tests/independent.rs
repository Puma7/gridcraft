//! Synthetic acceptance tests derived from analytic answers, not backend output.
use solvercraft::types::*;
use std::error::Error;
type TestResult = std::result::Result<(), Box<dyn Error>>;

fn cases() -> std::result::Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(include_str!("../../../tests/reference-cases.json"))
}

fn close(actual: f64, expected: f64, label: &str) {
    let tolerance = 1e-7 * expected.abs().max(1.0);
    assert!(actual.is_finite() && (actual - expected).abs() <= tolerance, "{label}: actual={actual}, expected={expected}, tolerance={tolerance}");
}

fn assert_original_feasibility(model: &OptimizationModel, values: &[f64]) {
    assert_eq!(model.variables.len(), values.len());
    for (index, (var, value)) in model.variables.iter().zip(values).enumerate() {
        assert!(value.is_finite(), "variable {index} must be finite");
        if let Some(low) = var.lower {
            assert!(*value >= low - 1e-6, "{} lower bound", var.name);
        }
        if let Some(high) = var.upper {
            assert!(*value <= high + 1e-6, "{} upper bound", var.name);
        }
        if var.kind != VariableKind::Continuous {
            assert!((value - value.round()).abs() <= 1e-6, "{} integrality", var.name);
        }
        if var.kind == VariableKind::Binary {
            assert!(*value >= -1e-6 && *value <= 1.0 + 1e-6);
        }
    }
    for row in &model.constraints {
        let lhs: f64 = row.terms.iter().map(|t| t.coefficient * values[t.variable]).sum();
        assert!(lhs.is_finite(), "row {} evaluation must be finite", row.name);
        let ok = match row.relation {
            Relation::LessEqual => lhs <= row.rhs + 1e-6,
            Relation::Equal => (lhs - row.rhs).abs() <= 1e-6,
            Relation::GreaterEqual => lhs >= row.rhs - 1e-6,
        };
        assert!(ok, "row {} is violated: lhs={lhs}, rhs={}", row.name, row.rhs);
    }
}

fn run_case(id: &str) -> TestResult {
    let all = cases()?;
    let c = all["cases"].as_array().ok_or("cases must be an array")?.iter().find(|c| c["id"] == id).ok_or("missing case")?;
    let model: OptimizationModel = serde_json::from_value(c["model"].clone())?;
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl)?;
    let result_json = serde_json::to_value(&result)?;
    let expected = &c["expected"];
    for field in ["feasibility", "optimality"] {
        if let Some(value) = expected.get(field) {
            assert_eq!(&result_json[field], value, "{id}: {field}: {result:?}");
        }
    }
    if let Some(forbidden) = expected["forbidden_optimality"].as_array() {
        assert!(!forbidden.contains(&result_json["optimality"]), "{id}: invalid optimality claim: {result:?}");
    }
    if let Some(allowed) = expected["permitted_feasibility"].as_array() {
        assert!(allowed.contains(&result_json["feasibility"]), "{id}: invalid feasibility claim: {result:?}");
    }
    if let Some(objective) = expected["objective"].as_f64() {
        close(result.objective.ok_or("missing objective")?, objective, id);
        let values = result.values.as_ref().ok_or("missing values")?;
        assert_original_feasibility(&model, values);
        let objective_recomputed = model.objective.offset
            + model.objective.linear.iter().map(|t| t.coefficient * values[t.variable]).sum::<f64>()
            + model.objective.quadratic.iter().map(|t| t.coefficient * values[t.left] * values[t.right]).sum::<f64>();
        close(objective_recomputed, objective, id);
        if let Some(known_values) = expected["values"].as_array() {
            for (actual, expected) in values.iter().zip(known_values) {
                close(*actual, expected.as_f64().ok_or("bad expected value")?, id);
            }
        }
    }
    if result.feasibility == Feasibility::Infeasible {
        assert!(result.values.is_none(), "{id}: an infeasible certificate cannot also publish a feasible candidate");
    }
    Ok(())
}

macro_rules! case {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() -> TestResult {
            run_case($id)
        }
    };
}
case!(wyndor_analytic_optimum, "wyndor");
case!(alternate_optima_do_not_require_unique_values, "alternative-optima");
case!(binary_enumeration_reference, "binary-triangle");
case!(continuous_relaxation_reference, "continuous-triangle");
case!(contradictory_constraints, "inconsistent-rows");
case!(genuine_continuous_unboundedness, "unbounded-continuous");
case!(free_variable_can_be_negative, "free-negative");
case!(upper_only_negative_variable, "upper-only-negative");
case!(fixed_and_free_variables, "fixed-and-free");
case!(two_equalities_determine_unique_point, "equality-pair");
case!(objective_offset_is_preserved, "objective-offset");
case!(large_finite_objective_is_not_unbounded, "bounded-large-objective");
case!(integer_equality_has_no_feasible_integer_point, "integer-equality-infeasible");
case!(lp_unboundedness_is_not_mip_unboundedness_proof, "unbounded-relaxation-infeasible-mip");
case!(zero_objective_finds_a_feasible_point, "zero-objective-feasibility");

#[test]
fn small_pivot_scaling_challenge() -> TestResult {
    run_case("small-pivot-challenge")
}

struct Cancelled;
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

fn wyndor_model() -> std::result::Result<OptimizationModel, Box<dyn Error>> {
    let all = cases()?;
    let c = all["cases"].as_array().ok_or("cases array")?.first().ok_or("first case")?;
    Ok(serde_json::from_value(c["model"].clone())?)
}

#[test]
fn cancelled_solve_is_not_optimal() -> TestResult {
    let result = solvercraft::solve(&wyndor_model()?, &SolveOptions::default(), &Cancelled)?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_ne!(result.optimality, Optimality::Optimal);
    Ok(())
}

#[test]
fn insufficient_memory_is_a_limit_or_error_never_fake_success() -> TestResult {
    let options = SolveOptions { max_memory_bytes: 1, ..SolveOptions::default() };
    match solvercraft::solve(&wyndor_model()?, &options, &NoControl) {
        Ok(result) => {
            assert_eq!(result.termination, Termination::MemoryLimit);
            assert_ne!(result.optimality, Optimality::Optimal);
        }
        Err(error) => assert!(!error.code.is_empty() && !error.message.is_empty()),
    }
    Ok(())
}

#[test]
fn external_seed_encoding_preserves_u64_max() -> TestResult {
    let solve = SolveOptions { seed: u64::MAX, ..SolveOptions::default() };
    let solve_json = serde_json::to_value(&solve)?;
    assert_eq!(solve_json["seed"], u64::MAX.to_string());
    let roundtrip: SolveOptions = serde_json::from_value(solve_json)?;
    assert_eq!(roundtrip.seed, u64::MAX);
    let sim = SimulationOptions { seed: 9_007_199_254_740_993, ..SimulationOptions::default() };
    let sim_json = serde_json::to_value(&sim)?;
    assert_eq!(sim_json["seed"], "9007199254740993");
    assert_eq!(serde_json::from_value::<SimulationOptions>(sim_json)?.seed, sim.seed);
    Ok(())
}

#[test]
fn constant_simulation_has_zero_variance_without_storing_trials() -> TestResult {
    let options = SimulationOptions { trials: 64, retain_samples: false, ..SimulationOptions::default() };
    let result = solvercraft::simulation::simulate(&[Distribution::Constant { value: 7.0 }], &options, &NoControl, |inputs| Ok(vec![inputs[0]]))?;
    assert_eq!(result.completed_trials, 64);
    assert!(result.samples.is_none());
    assert_eq!(result.summaries.len(), 1);
    close(result.summaries[0].mean, 7.0, "constant mean");
    close(result.summaries[0].variance.ok_or("missing variance")?, 0.0, "constant variance");
    Ok(())
}

#[test]
fn single_observation_does_not_claim_unbiased_variance() -> TestResult {
    let options = SimulationOptions { trials: 1, ..SimulationOptions::default() };
    let result = solvercraft::simulation::simulate(&[Distribution::Constant { value: 7.0 }], &options, &NoControl, |inputs| Ok(vec![inputs[0]]))?;
    assert!(result.summaries[0].variance.is_none());
    Ok(())
}

#[test]
fn latin_hypercube_visits_each_uniform_stratum_once_per_dimension() -> TestResult {
    let n = 128;
    let options =
        SimulationOptions { trials: n, seed: 42, method: SamplingMethod::LatinHypercube, retain_samples: true, ..SimulationOptions::default() };
    let result = solvercraft::simulation::simulate(
        &[Distribution::Uniform { low: 0.0, high: 1.0 }, Distribution::Uniform { low: 0.0, high: 1.0 }],
        &options,
        &NoControl,
        |inputs| Ok(inputs.to_vec()),
    )?;
    let samples = result.samples.as_ref().ok_or("retained samples missing")?;
    assert_eq!(samples.len(), n as usize);
    for dimension in 0..2 {
        let mut visited = vec![false; n as usize];
        for sample in samples {
            assert_eq!(sample.len(), 2);
            let x = sample[dimension];
            assert!((0.0..1.0).contains(&x), "uniform sample outside support");
            let stratum = (x * n as f64).floor() as usize;
            assert!(!visited[stratum], "stratum repeated in dimension {dimension}");
            visited[stratum] = true;
        }
        assert!(visited.iter().all(|x| *x));
    }
    Ok(())
}

#[test]
fn simulation_seed_repeats_exact_input_stream() -> TestResult {
    let options = SimulationOptions { trials: 64, seed: u64::MAX, retain_samples: true, ..SimulationOptions::default() };
    let distributions = [Distribution::Normal { mean: 0.0, std_dev: 1.0 }, Distribution::Uniform { low: 2.0, high: 8.0 }];
    let first = solvercraft::simulation::simulate(&distributions, &options, &NoControl, |x| Ok(x.to_vec()))?;
    let second = solvercraft::simulation::simulate(&distributions, &options, &NoControl, |x| Ok(x.to_vec()))?;
    assert_eq!(first.samples, second.samples);
    Ok(())
}

#[test]
fn cancelled_simulation_does_not_execute_trials() -> TestResult {
    let mut called = false;
    let result = solvercraft::simulation::simulate(
        &[Distribution::Constant { value: 7.0 }],
        &SimulationOptions::default(),
        &Cancelled,
        |_inputs| -> solvercraft::types::Result<Vec<f64>> {
            called = true;
            Ok(vec![7.0])
        },
    )?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.completed_trials, 0);
    assert!(!called, "cancelled job must not evaluate");
    Ok(())
}

#[test]
fn changing_output_arity_is_rejected() -> TestResult {
    let options = SimulationOptions { trials: 4, ..SimulationOptions::default() };
    let mut trial = 0;
    let result = solvercraft::simulation::simulate(&[Distribution::Constant { value: 0.0 }], &options, &NoControl, |_inputs| {
        trial += 1;
        if trial == 1 { Ok(vec![1.0]) } else { Ok(vec![1.0, 2.0]) }
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn empirical_cvar_fractionally_includes_mass_at_quantile_boundary() -> TestResult {
    let losses = [0.0, 0.0, 0.0, 10.0];
    close(solvercraft::math::empirical_quantile(&losses, 0.5)?, 0.0, "VaR");
    close(solvercraft::math::upper_tail_cvar(&losses, 0.5)?, 5.0, "CVaR");
    close(solvercraft::math::upper_tail_cvar(&losses, 0.75)?, 10.0, "tail quarter");
    Ok(())
}

#[test]
fn empty_empirical_sample_is_not_zero_risk() {
    assert!(solvercraft::math::empirical_quantile(&[], 0.95).is_err());
    assert!(solvercraft::math::upper_tail_cvar(&[], 0.95).is_err());
}

#[test]
fn small_signed_binary_models_match_complete_enumeration() -> TestResult {
    let n = 6usize;
    let mut random = 0x5EED_u64;
    let mut next = || {
        random = random.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (random >> 32) as i32 & 0x7fff_ffff
    };
    for case in 0..24 {
        let variables = (0..n).map(|i| Variable { name: format!("b{i}"), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Binary }).collect();
        let linear = (0..n).map(|i| LinearTerm { variable: i, coefficient: f64::from(next() % 15 - 7) }).collect();
        let constraints: Vec<_> = (0..3)
            .map(|r| LinearConstraint {
                name: format!("r{r}"),
                relation: Relation::LessEqual,
                rhs: f64::from(next() % 9),
                terms: (0..n).map(|i| LinearTerm { variable: i, coefficient: f64::from(next() % 11 - 5) }).collect(),
            })
            .collect();
        let direction = if case % 2 == 0 { Direction::Maximize } else { Direction::Minimize };
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("exhaustive-{case}"),
            variables,
            objective: Objective { direction, offset: -3.0, linear, quadratic: vec![] },
            constraints,
        };
        let mut exact = if direction == Direction::Maximize { f64::NEG_INFINITY } else { f64::INFINITY };
        for mask in 0..(1usize << n) {
            let values: Vec<f64> = (0..n).map(|i| ((mask >> i) & 1) as f64).collect();
            let feasible = model.constraints.iter().all(|row| row.terms.iter().map(|t| t.coefficient * values[t.variable]).sum::<f64>() <= row.rhs);
            if feasible {
                let value = model.objective.offset + model.objective.linear.iter().map(|t| t.coefficient * values[t.variable]).sum::<f64>();
                exact = if direction == Direction::Maximize { exact.max(value) } else { exact.min(value) };
            }
        }
        assert!(exact.is_finite(), "zero assignment should be feasible");
        let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl)?;
        assert_eq!(result.feasibility, Feasibility::Feasible, "{case}: {result:?}");
        assert_eq!(result.optimality, Optimality::Optimal, "{case}: {result:?}");
        close(result.objective.ok_or("missing enumerated objective")?, exact, "exhaustive binary objective");
        assert_original_feasibility(&model, result.values.as_ref().ok_or("missing binary values")?);
    }
    Ok(())
}

#[test]
fn weighted_tail_mean_respects_partial_quantile_mass() -> TestResult {
    close(solvercraft::risk::weighted_upper_tail_cvar(&[0.0, 10.0], &[3.0, 1.0], 0.5)?, 5.0, "weighted CVaR");
    close(solvercraft::risk::weighted_upper_tail_cvar(&[0.0, 10.0], &[3.0, 1.0], 0.8)?, 10.0, "top tail entirely at atom");
    assert!(solvercraft::risk::weighted_upper_tail_cvar(&[0.0, 10.0], &[0.0, 0.0], 0.5).is_err());
    assert!(solvercraft::risk::weighted_upper_tail_cvar(&[0.0, 10.0], &[-1.0, 2.0], 0.5).is_err());
    Ok(())
}

#[test]
fn expected_loss_and_tail_loss_choose_different_analytic_decisions() -> TestResult {
    use solvercraft::risk::{RiskMeasure, Scenario, ScenarioProblem};
    // E[L]=2-x, so minimizing over [0,2] selects x=2.
    // CVaR_0.5=max(x,4-3x), so its minimum is their intersection x=1.
    let mut problem = ScenarioProblem {
        name: "analytic-risk-tradeoff".to_string(),
        variables: vec![Variable { name: "x".to_string(), lower: Some(0.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        constraints: vec![],
        scenarios: vec![
            Scenario { weight: 1.0, offset: 0.0, coefficients: vec![1.0] },
            Scenario { weight: 1.0, offset: 4.0, coefficients: vec![-3.0] },
        ],
        measure: RiskMeasure::ExpectedLoss,
    };
    let expected = solvercraft::risk::optimize(&problem, &SolveOptions::default(), &NoControl)?;
    assert_eq!(expected.solve.optimality, Optimality::Optimal);
    close(expected.decision_values.ok_or("expected decision missing")?[0], 2.0, "mean-optimal x");
    close(expected.expected_loss.ok_or("mean missing")?, 0.0, "optimal mean");
    problem.measure = RiskMeasure::ConditionalValueAtRisk { confidence: 0.5 };
    let risk = solvercraft::risk::optimize(&problem, &SolveOptions::default(), &NoControl)?;
    assert_eq!(risk.solve.optimality, Optimality::Optimal);
    close(risk.decision_values.ok_or("tail decision missing")?[0], 1.0, "tail-optimal x");
    close(risk.cvar.ok_or("CVaR missing")?, 1.0, "optimal CVaR");
    Ok(())
}

#[test]
fn robust_box_counterpart_handles_negative_variables() -> TestResult {
    let nominal = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "negative robust variable".to_string(),
        variables: vec![Variable { name: "x".to_string(), lower: Some(-10.0), upper: Some(0.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "uncertain row".to_string(),
            terms: vec![LinearTerm { variable: 0, coefficient: -1.0 }],
            relation: Relation::LessEqual,
            rhs: 4.0,
        }],
    };
    let uncertainty = [solvercraft::risk::BoxUncertainty { constraint: 0, radii: vec![1.0] }];
    let counterpart = solvercraft::risk::robust_box(&nominal, &uncertainty, &SolveOptions::default())?;
    let result = solvercraft::solve(&counterpart, &SolveOptions::default(), &NoControl)?;
    assert_eq!(result.optimality, Optimality::Optimal);
    let x = result.values.ok_or("robust decision missing")?[0];
    close(x, -2.0, "robust negative x");
    assert!(-x + x.abs() <= 4.0 + 1e-6);
    Ok(())
}

#[test]
fn tiny_positive_blocking_coefficient_is_not_an_unboundedness_certificate() -> TestResult {
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "ill-conditioned bounded LP".to_string(),
        variables: vec![
            Variable { name: "x".to_string(), lower: Some(0.0), upper: None, kind: VariableKind::Continuous },
            Variable { name: "y".to_string(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous },
        ],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "small blocker".to_string(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1e-15 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            relation: Relation::LessEqual,
            rhs: 1.0,
        }],
    };
    match solvercraft::solve(&model, &SolveOptions::default(), &NoControl) {
        Ok(result) => {
            assert_ne!(result.optimality, Optimality::Unbounded, "Tiny positive coefficients still bound x");
            if result.optimality == Optimality::Optimal {
                close(result.objective.ok_or("finite optimum missing")?, 1e15, "scaled finite optimum");
            } else {
                assert_eq!(result.termination, Termination::NumericalFailure, "unresolved numerics must be explicit");
            }
        }
        Err(error) => assert_eq!(error.code, "numerical_failure"),
    }
    Ok(())
}

#[test]
fn cubic_zero_at_probe_points_is_structurally_nonlinear() -> TestResult {
    use solvercraft::compile::{Expr, ExpressionClass};
    let x = Expr::variable(0);
    let expression = Expr::Multiply {
        left: Box::new(Expr::Multiply {
            left: Box::new(x.clone()),
            right: Box::new(Expr::Subtract { left: Box::new(x.clone()), right: Box::new(Expr::constant(1.0)) }),
        }),
        right: Box::new(Expr::Subtract { left: Box::new(x), right: Box::new(Expr::constant(1.7)) }),
    };
    for x in [0.0, 1.0, 1.7] {
        close(solvercraft::compile::evaluate(&expression, &[x])?, 0.0, "probe-zero");
    }
    assert_eq!(solvercraft::compile::classify(&expression, 1)?, ExpressionClass::Nonlinear);
    Ok(())
}

#[test]
fn expanded_square_has_no_hidden_half_factor_and_correct_gradient() -> TestResult {
    use solvercraft::compile::{Expr, ExpressionModel};
    let expression = Expr::Power {
        base: Box::new(Expr::Add {
            left: Box::new(Expr::variable(0)),
            right: Box::new(Expr::Multiply { left: Box::new(Expr::constant(2.0)), right: Box::new(Expr::variable(1)) }),
        }),
        exponent: 2,
    };
    let input = ExpressionModel {
        name: "square".to_string(),
        variables: vec![
            Variable { name: "x".to_string(), lower: None, upper: None, kind: VariableKind::Continuous },
            Variable { name: "y".to_string(), lower: None, upper: None, kind: VariableKind::Continuous },
        ],
        direction: Direction::Minimize,
        objective: expression.clone(),
        constraints: vec![],
    };
    let compiled = solvercraft::compile::compile(&input, &SolveOptions::default())?;
    close(solvercraft::model::evaluate_objective(&compiled, &[3.0, 5.0])?, 169.0, "expanded square");
    let (value, gradient) = solvercraft::compile::evaluate_with_gradient(&expression, &[3.0, 5.0])?;
    close(value, 169.0, "AD square value");
    close(gradient[0], 26.0, "AD x");
    close(gradient[1], 52.0, "AD y");
    Ok(())
}

#[test]
fn autodiff_reports_domain_and_nonsmooth_failures_instead_of_zero_gradient() {
    use solvercraft::compile::Expr;
    let abs = Expr::Abs { value: Box::new(Expr::variable(0)) };
    let ln = Expr::Ln { value: Box::new(Expr::variable(0)) };
    let exp = Expr::Exp { value: Box::new(Expr::variable(0)) };
    assert!(solvercraft::compile::evaluate_with_gradient(&abs, &[0.0]).is_err());
    assert!(solvercraft::compile::evaluate_with_gradient(&ln, &[-1.0]).is_err());
    assert!(solvercraft::compile::evaluate_with_gradient(&exp, &[1000.0]).is_err());
    assert!(solvercraft::compile::evaluate_with_gradient(&Expr::variable(1), &[1.0]).is_err());
}

#[test]
fn compiler_rejects_excessive_depth_before_recursive_arithmetic() {
    use solvercraft::compile::Expr;
    let mut expression = Expr::variable(0);
    for _ in 0..65 {
        expression = Expr::Negate { value: Box::new(expression) };
    }
    assert!(solvercraft::compile::classify(&expression, 1).is_err());
    assert!(solvercraft::compile::evaluate_with_gradient(&expression, &[0.0]).is_err());
}

#[test]
fn bounds_only_quadratic_solves_the_previous_narrow_interval_counterexample() -> TestResult {
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "narrow QP".to_string(),
        variables: vec![Variable { name: "x".to_string(), lower: Some(0.0), upper: Some(5e-7), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 16.0,
            linear: vec![LinearTerm { variable: 0, coefficient: -8e7 }],
            quadratic: vec![QuadraticTerm { left: 0, right: 0, coefficient: 1e14 }],
        },
        constraints: vec![],
    };
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl)?;
    assert_eq!(result.optimality, Optimality::Optimal);
    close(result.objective.ok_or("quadratic objective missing")?, 0.0, "narrow quadratic optimum");
    let x = result.values.ok_or("quadratic value missing")?[0];
    assert!((x - 4e-7).abs() <= 1e-14, "wrong narrow QP decision: {x}");
    Ok(())
}

#[test]
fn online_moments_avoid_large_offset_cancellation() -> TestResult {
    let mut moments = solvercraft::simulation::OnlineMoments::default();
    for value in [1e12 + 1.0, 1e12 + 2.0, 1e12 + 3.0, 1e12 + 4.0] {
        moments.push(value)?;
    }
    let summary = moments.summary()?;
    assert_eq!(summary.mean, 1e12 + 2.5);
    close(summary.variance.ok_or("variance missing")?, 5.0 / 3.0, "large-offset variance");
    match solvercraft::simulation::fit_normal(&[1.0, 2.0, 3.0, 4.0])? {
        Distribution::Normal { mean, std_dev } => {
            close(mean, 2.5, "normal MLE mean");
            close(std_dev, 1.25_f64.sqrt(), "normal MLE stddev");
        }
        other => return Err(format!("wrong fitted family {other:?}").into()),
    }
    Ok(())
}

#[test]
fn fixed_seed_monte_carlo_matches_independent_analytic_moments() -> TestResult {
    // Broad, declared-before-execution bands for this fixed deterministic ensemble.
    // They are validation tolerances, not a claimed confidence interval API.
    let options = SimulationOptions { trials: 8192, seed: 20261031, retain_samples: false, ..SimulationOptions::default() };
    let result = solvercraft::simulation::simulate(
        &[Distribution::Normal { mean: 0.0, std_dev: 1.0 }, Distribution::Uniform { low: 2.0, high: 8.0 }],
        &options,
        &NoControl,
        |inputs| Ok(inputs.to_vec()),
    )?;
    assert_eq!(result.completed_trials, 8192);
    assert_eq!(result.summaries.len(), 2);
    let normal = &result.summaries[0];
    let uniform = &result.summaries[1];
    assert!(normal.mean.abs() < 0.07, "normal mean {}", normal.mean);
    assert!((normal.variance.ok_or("normal variance missing")? - 1.0).abs() < 0.12);
    assert!((uniform.mean - 5.0).abs() < 0.1, "uniform mean {}", uniform.mean);
    assert!((uniform.variance.ok_or("uniform variance missing")? - 3.0).abs() < 0.25);
    assert!(uniform.minimum >= 2.0 && uniform.maximum <= 8.0);
    Ok(())
}
