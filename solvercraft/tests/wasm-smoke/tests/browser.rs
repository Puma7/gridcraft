//! Actual browser-executed smoke tests. These are not native tests or a WASM compile-only check.
#![cfg(target_arch = "wasm32")]
use solvercraft::types::*;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

fn example_model() -> OptimizationModel {
    let request: serde_json::Value = serde_json::from_str(include_str!("../../../examples/lp.json")).expect("synthetic fixture JSON");
    serde_json::from_value(request["model"].clone()).expect("synthetic fixture model")
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-7 * expected.abs().max(1.0), "{actual} != {expected}");
}

#[wasm_bindgen_test]
fn browser_solves_wyndor_and_validates_original_constraints() {
    let model = example_model();
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl).expect("solve Wyndor");
    assert_eq!(result.optimality, Optimality::Optimal);
    assert_eq!(result.feasibility, Feasibility::Feasible);
    close(result.objective.expect("objective"), 36.0);
    let values = result.values.expect("values");
    close(values[0], 2.0);
    close(values[1], 6.0);
    assert!(solvercraft::model::validate_solution(&model, &values, &SolveOptions::default()).expect("validate").feasible);
}

#[wasm_bindgen_test]
fn browser_honors_binary_domains() {
    let mut model = example_model();
    for v in &mut model.variables {
        v.kind = VariableKind::Binary;
        v.lower = Some(0.0);
        v.upper = Some(1.0);
    }
    model.objective.linear = vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: 1.0 }];
    model.constraints = vec![LinearConstraint {
        name: "capacity".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 2.0 }, LinearTerm { variable: 1, coefficient: 2.0 }],
        relation: Relation::LessEqual,
        rhs: 3.0,
    }];
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl).expect("binary solve");
    assert_eq!(result.optimality, Optimality::Optimal);
    close(result.objective.expect("objective"), 1.0);
    // Same original-space integer oracle as native: x=1 is impossible;
    // feasible points are (0,0) and (0,1), so the unique optimum is (0,1).
    model.constraints[0].terms = vec![LinearTerm { variable: 0, coefficient: 1e9 }, LinearTerm { variable: 1, coefficient: 1.0 }];
    model.constraints[0].rhs = 1.0;
    for (direction, sign, offset) in [(Direction::Maximize, 1.0, 0.0), (Direction::Maximize, 1.0, 1e16), (Direction::Minimize, -1.0, 0.0)] {
        model.objective.direction = direction;
        model.objective.offset = offset;
        model.objective.linear = vec![LinearTerm { variable: 0, coefficient: sign * 1e12 }, LinearTerm { variable: 1, coefficient: sign * 500.0 }];
        let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl).expect("near-integer binary solve");
        assert_eq!(result.feasibility, Feasibility::Feasible);
        assert_eq!(result.optimality, Optimality::Optimal);
        assert_eq!(result.values, Some(vec![0.0, 1.0]));
        // Relative tolerance would hide the complete improvement at offset1e16.
        assert_eq!(result.objective, Some(offset + sign * 500.0));
        assert_eq!(result.best_bound, Some(offset + sign * 500.0));
        assert_eq!(result.relative_gap, Some(0.0));
    }
}

#[wasm_bindgen_test]
fn browser_samples_deterministically_with_loss_tail_statistics() {
    let options =
        SimulationOptions { trials: 4, seed: u64::MAX, method: SamplingMethod::LatinHypercube, retain_samples: true, ..SimulationOptions::default() };
    let inputs = [Distribution::Empirical { values: vec![0.0, 0.0, 0.0, 10.0] }];
    let first = solvercraft::simulation::simulate(&inputs, &options, &NoControl, |values| Ok(values.to_vec())).expect("simulate");
    let second = solvercraft::simulation::simulate(&inputs, &options, &NoControl, |values| Ok(values.to_vec())).expect("repeat simulation");
    assert_eq!(first.samples, second.samples);
    assert_eq!(first.completed_trials, 4);
    close(first.summaries[0].mean, 2.5);
    close(first.summaries[0].conditional_value_at_risk_95.expect("loss tail"), 10.0);
}

#[wasm_bindgen_test]
fn browser_latin_hypercube_preserves_all_strata() {
    let options =
        SimulationOptions { trials: 64, seed: 42, method: SamplingMethod::LatinHypercube, retain_samples: true, ..SimulationOptions::default() };
    let result =
        solvercraft::simulation::simulate(&[Distribution::Uniform { low: 0.0, high: 1.0 }], &options, &NoControl, |x| Ok(x.to_vec())).expect("LHS");
    let mut seen = [false; 64];
    for row in result.samples.expect("retained rows") {
        assert!((0.0..1.0).contains(&row[0]));
        let stratum = (row[0] * 64.0).floor() as usize;
        assert!(!seen[stratum]);
        seen[stratum] = true;
    }
    assert!(seen.into_iter().all(|x| x));
}

#[wasm_bindgen_test]
fn browser_handles_large_seed_json_without_javascript_precision_loss() {
    let options = SolveOptions { seed: u64::MAX, ..SolveOptions::default() };
    let json = serde_json::to_value(&options).expect("serialize seed");
    assert_eq!(json["seed"], "18446744073709551615");
    assert_eq!(serde_json::from_value::<SolveOptions>(json).expect("deserialize seed").seed, u64::MAX);
}

#[wasm_bindgen_test]
fn browser_retains_a_unit_improvement_after_large_objective_cancellation() {
    let model: OptimizationModel = serde_json::from_value(serde_json::json!({
        "schema_version":1,"name":"browser cancellation oracle",
        "variables":[{"name":"x","lower":0,"upper":1,"kind":"continuous"},
                     {"name":"y","lower":0,"upper":1,"kind":"continuous"},
                     {"name":"z","lower":0,"upper":1,"kind":"continuous"}],
        "objective":{"direction":"maximize","offset":0,"linear":[{"variable":0,"coefficient":1e16},
            {"variable":1,"coefficient":1},{"variable":2,"coefficient":-1e16}],"quadratic":[]},
        "constraints":[{"name":"xy","terms":[{"variable":0,"coefficient":1},{"variable":1,"coefficient":-1}],"relation":"less_equal","rhs":0},
            {"name":"yz","terms":[{"variable":1,"coefficient":1},{"variable":2,"coefficient":-1}],"relation":"less_equal","rhs":0}]
    }))
    .expect("synthetic model");
    // 1e16*(x-z)+y <= y <= 1; equality is attained by x=y=z=1.
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl).expect("cancellation solve");
    assert_eq!(result.optimality, Optimality::Optimal);
    assert_eq!(result.values, Some(vec![1.0, 1.0, 1.0]));
    assert_eq!(result.objective, Some(1.0));
    assert_eq!(result.best_bound, Some(1.0));
}

#[wasm_bindgen_test]
fn browser_does_not_round_nonrepresentable_positive_variance_to_zero() {
    let options = SimulationOptions { trials: 32, seed: 42, retain_samples: true, ..SimulationOptions::default() };
    let result = solvercraft::simulation::simulate(&[Distribution::Uniform { low: -1e-200, high: 1e-200 }], &options, &NoControl, |x| Ok(x.to_vec()));
    let error = result.expect_err("nonconstant tiny samples have no representable binary64 variance");
    assert_eq!(error.code, "numerical_range");
}

#[wasm_bindgen_test]
fn browser_lhs_keeps_a_discrete_mixture_on_its_exact_atoms() {
    let options =
        SimulationOptions { trials: 64, seed: 42, method: SamplingMethod::LatinHypercube, retain_samples: true, ..SimulationOptions::default() };
    let distribution = Distribution::Mixture {
        components: vec![Distribution::Constant { value: 0.0 }, Distribution::Constant { value: 1.0 }],
        weights: vec![0.5, 0.5],
    };
    let result = solvercraft::simulation::simulate(&[distribution], &options, &NoControl, |x| Ok(x.to_vec())).expect("discrete mixture LHS");
    let rows = result.samples.expect("samples");
    assert!(rows.iter().all(|row| row == &[0.0] || row == &[1.0]));
    assert_eq!(rows.iter().filter(|row| row[0] == 0.0).count(), 32);
}

#[wasm_bindgen_test]
fn browser_autodiff_rejects_a_nondifferentiable_point() {
    let abs = solvercraft::compile::Expr::Abs { value: Box::new(solvercraft::compile::Expr::variable(0)) };
    assert!(solvercraft::compile::evaluate_with_gradient(&abs, &[0.0]).is_err());
    let expression = solvercraft::compile::Expr::Power { base: Box::new(solvercraft::compile::Expr::variable(0)), exponent: 2 };
    let (value, gradient) = solvercraft::compile::evaluate_with_gradient(&expression, &[3.0]).expect("AD");
    close(value, 9.0);
    close(gradient[0], 6.0);
}

#[wasm_bindgen_test]
fn browser_optimizes_finite_scenario_tail_risk() {
    use solvercraft::risk::{RiskMeasure, Scenario, ScenarioProblem};
    let problem = ScenarioProblem {
        name: "browser risk".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        constraints: vec![],
        scenarios: vec![
            Scenario { weight: 1.0, offset: 0.0, coefficients: vec![1.0] },
            Scenario { weight: 1.0, offset: 4.0, coefficients: vec![-3.0] },
        ],
        measure: RiskMeasure::ConditionalValueAtRisk { confidence: 0.5 },
    };
    let result = solvercraft::risk::optimize(&problem, &SolveOptions::default(), &NoControl).expect("risk solve");
    close(result.decision_values.expect("decisions")[0], 1.0);
    close(result.cvar.expect("CVaR"), 1.0);
}

#[wasm_bindgen_test]
fn browser_rejects_native_thread_jobs_without_panicking() {
    let result = solvercraft::jobs::start_solve(example_model(), SolveOptions::default());
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn browser_solves_a_coupled_convex_quadratic_with_original_residuals() {
    let mut model = example_model();
    model.objective = Objective {
        direction: Direction::Minimize,
        offset: 0.0,
        linear: vec![],
        quadratic: vec![
            QuadraticTerm { left: 0, right: 0, coefficient: 1.0 },
            QuadraticTerm { left: 0, right: 1, coefficient: 1.0 },
            QuadraticTerm { left: 1, right: 1, coefficient: 1.0 },
        ],
    };
    model.constraints = vec![LinearConstraint {
        name: "required total".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: 1.0 }],
        relation: Relation::GreaterEqual,
        rhs: 2.0,
    }];
    let result = solvercraft::solve(&model, &SolveOptions::default(), &NoControl).expect("coupled QP");
    assert_eq!(result.optimality, Optimality::Optimal, "{result:?}");
    let values = result.values.expect("QP point");
    assert!((values[0] - 1.0).abs() < 1e-5 && (values[1] - 1.0).abs() < 1e-5);
    // x²+xy+y² = 3/4(x+y)²+1/4(x-y)² >= 3 under x+y>=2.
    assert!((values[0] * values[0] + values[0] * values[1] + values[1] * values[1] - 3.0).abs() < 1e-6);
    assert!(solvercraft::model::validate_solution(&model, &values, &SolveOptions::default()).expect("original rows").feasible);
}

#[wasm_bindgen_test]
fn browser_executes_clarabel_socp_with_a_cauchy_schwarz_oracle() {
    use solvercraft::engines::{AffineExpression, ConicModel, SecondOrderConeConstraint, solve_socp_owned};
    let mut base = example_model();
    base.objective = Objective {
        direction: Direction::Maximize,
        offset: 0.0,
        linear: vec![LinearTerm { variable: 0, coefficient: 3.0 }, LinearTerm { variable: 1, coefficient: 4.0 }],
        quadratic: vec![],
    };
    base.constraints.clear();
    let model = ConicModel {
        base,
        second_order_cones: vec![SecondOrderConeConstraint {
            name: "radius two".into(),
            components: (0..2).map(|variable| AffineExpression { offset: 0.0, terms: vec![LinearTerm { variable, coefficient: 1.0 }] }).collect(),
            upper: AffineExpression { offset: 2.0, terms: vec![] },
        }],
    };
    let solved = solve_socp_owned(&model, &SolveOptions::default(), std::sync::Arc::new(NoControl)).expect("SOCP");
    assert_eq!(solved.result.optimality, Optimality::Optimal, "{solved:?}");
    let x = solved.result.values.expect("conic point");
    assert!((x[0] - 1.2).abs() < 1e-5 && (x[1] - 1.6).abs() < 1e-5);
    assert!(x[0].hypot(x[1]) <= 2.0 + 1e-7);
    assert!((3.0 * x[0] + 4.0 * x[1] - 10.0).abs() < 1e-6);
    assert!(solved.result.best_bound.expect("global upper bound") >= 10.0 - 1e-8);
}

#[wasm_bindgen_test]
fn browser_nonlinear_method_reports_stationarity_separately_from_global_optimality() {
    use solvercraft::compile::{Expr, ExpressionModel};
    let model = ExpressionModel {
        name: "monotone exponential".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective: Expr::Exp { value: Box::new(Expr::variable(0)) },
        constraints: vec![],
    };
    let options = SolveOptions { method: SolveMethod::ProjectedGradient, ..SolveOptions::default() };
    let result = solvercraft::nonlinear::solve(&model, &options, &NoControl).expect("projected gradient");
    assert_eq!(result.optimality, Optimality::Stationary, "{result:?}");
    close(result.values.expect("point")[0], -2.0);
    close(result.objective.expect("objective"), (-2.0_f64).exp());
    assert_eq!(result.best_bound, None);
}

#[wasm_bindgen_test]
fn browser_randomized_sobol_integrates_a_smooth_product() {
    use solvercraft::simulation::advanced::{AdvancedSimulationOptions, SamplerSpec, simulate_advanced};
    let options = AdvancedSimulationOptions {
        base: SimulationOptions { trials: 1024, seed: 42, retain_samples: false, ..SimulationOptions::default() },
        sampler: SamplerSpec::RandomizedSobol { replicate: 7 },
        ..AdvancedSimulationOptions::default()
    };
    let result = simulate_advanced(
        &[Distribution::Uniform { low: 0.0, high: 1.0 }, Distribution::Uniform { low: 0.0, high: 1.0 }],
        &options,
        &NoControl,
        |x| Ok(vec![x[0] * x[1]]),
    )
    .expect("Sobol integration");
    assert_eq!(result.result.completed_trials, 1024);
    assert!((result.result.summaries[0].mean - 0.25).abs() < 0.005);
    assert!(result.result.sampling_version.contains("sobol"));
}

#[wasm_bindgen_test]
fn browser_constrained_nonlinear_rechecks_the_original_exponential_inequality() {
    use solvercraft::compile::{Expr, ExpressionConstraint, ExpressionModel};
    let model = ExpressionModel {
        name: "nonlinear exponential constraint".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective: Expr::Power { base: Box::new(Expr::variable(0)), exponent: 2 },
        constraints: vec![ExpressionConstraint {
            name: "minimum exponential".into(),
            left: Expr::Exp { value: Box::new(Expr::variable(0)) },
            relation: Relation::GreaterEqual,
            right: Expr::constant(2.0),
        }],
    };
    let result =
        solvercraft::constrained::solve_from(&model, &[1.0], &SolveOptions::default(), &solvercraft::nonlinear::NlpOptions::default(), &NoControl)
            .expect("constrained NLP");
    assert_eq!(result.feasibility, Feasibility::Feasible, "{result:?}");
    assert_eq!(result.optimality, Optimality::Stationary, "{result:?}");
    let x = result.values.expect("NLP point")[0];
    assert!(x.exp() >= 2.0 - 1e-7, "original nonlinear feasibility");
    assert!((x - 2.0_f64.ln()).abs() < 1e-5);
    assert!((result.objective.expect("objective") - x * x).abs() < 1e-10);
    assert_eq!(result.best_bound, None);
}
