//! Independent finite-scenario and native-runtime review cases.
use solvercraft::{jobs, model, provenance, risk, solve, types::*};
use std::sync::{Arc, mpsc};
use std::time::Duration;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn coefficient_box_feasibility_does_not_amplify_auxiliary_tolerance() -> TestResult {
    for radius in [1.0, 1e12] {
        let mut original = linear_model();
        original.variables[0].lower = Some(1e-9);
        original.variables[0].upper = Some(1e-9);
        original.objective.linear.clear();
        original.constraints.push(LinearConstraint { name: "uncertain row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 0.0 });
        let options = SolveOptions::default();
        let counterpart = risk::robust_box(&original, &[risk::BoxUncertainty { constraint: 0, radii: vec![radius] }], &options)?;
        let report = solve(&counterpart, &options, &NoControl)?;
        if report.feasibility == Feasibility::Feasible {
            let x = report.values.as_ref().ok_or("Feasible without values")?[0];
            let original_worst_case = radius * x.abs();
            assert!(
                original_worst_case <= options.feasibility_tolerance,
                "False robust feasibility after an auxiliary tolerance was multiplied by the uncertainty radius: radius={radius}, support={original_worst_case}, report={report:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn checked_robust_wrappers_validate_original_summed_support() -> TestResult {
    let mut original = linear_model();
    original.objective.linear.clear();
    original.variables = vec![
        Variable { name: "x".into(), lower: Some(8e-9), upper: Some(8e-9), kind: VariableKind::Continuous },
        Variable { name: "y".into(), lower: Some(8e-9), upper: Some(8e-9), kind: VariableKind::Continuous },
    ];
    original.constraints.push(LinearConstraint { name: "uncertain row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 0.0 });
    let options = SolveOptions { feasibility_tolerance: 1e-8, ..Default::default() };
    let boxes = [risk::BoxUncertainty { constraint: 0, radii: vec![1.0, 1.0] }];
    let budgets = [risk::BudgetUncertainty { constraint: 0, radii: vec![1.0, 1.0], budget: f64::MAX }];
    assert!(!risk::validate_robust_box_solution(&original, &boxes, &[8e-9, 8e-9], &options)?.feasible);
    assert!(!risk::validate_robust_budget_solution(&original, &budgets, &[8e-9, 8e-9], &options)?.feasible);
    for output in [
        risk::solve_robust_box_owned(&original, &boxes, &options, Arc::new(NoControl)),
        risk::solve_robust_budget_owned(&original, &budgets, &options, Arc::new(NoControl)),
    ] {
        match output {
            Ok(output) => assert_ne!(output.result.feasibility, Feasibility::Feasible, "{output:?}"),
            Err(error) => assert!(matches!(error.code.as_str(), "numerical_failure" | "numeric_range"), "{error}"),
        }
    }
    original.constraints[0].rhs = 1.0;
    for output in [
        risk::solve_robust_box_owned(&original, &boxes, &options, Arc::new(NoControl))?,
        risk::solve_robust_budget_owned(&original, &budgets, &options, Arc::new(NoControl))?,
    ] {
        assert_eq!(output.result.feasibility, Feasibility::Feasible);
        assert_eq!(output.result.objective, Some(0.0));
        assert_eq!(output.result.max_violation, Some(0.0));
    }
    Ok(())
}

#[test]
fn budget_support_orders_exact_products_and_retains_collective_underflow() -> TestResult {
    let mut original = linear_model();
    original.objective.linear.clear();
    original.variables =
        (0..3).map(|index| Variable { name: format!("x{index}"), lower: None, upper: None, kind: VariableKind::Continuous }).collect();
    let delta = 2.0_f64.powi(-106);
    original.constraints.push(LinearConstraint {
        name: "nearly tied weights".into(),
        terms: vec![LinearTerm { variable: 2, coefficient: delta }],
        relation: Relation::LessEqual,
        rhs: 0.5,
    });
    let options = SolveOptions { feasibility_tolerance: 1e-40, ..Default::default() };
    let epsilon = f64::EPSILON;
    // Both first products round to1, but (1+eps)*(1-eps)=1-eps^2
    // is strictly smaller than1. Choosing the wrong fractional atom would
    // change the residual from+2^-106 to-2^-106.
    for (values, radii) in
        [(vec![1.0 - epsilon, 1.0, 1.0], vec![1.0 + epsilon, 1.0, 0.0]), (vec![1.0, 1.0 - epsilon, 1.0], vec![1.0, 1.0 + epsilon, 0.0])]
    {
        let checked =
            risk::validate_robust_budget_solution(&original, &[risk::BudgetUncertainty { constraint: 0, radii, budget: 0.5 }], &values, &options)?;
        assert!(!checked.feasible);
        assert_eq!(checked.max_violation, delta);
    }
    original.variables.truncate(2);
    original.constraints[0].terms.clear();
    original.constraints[0].rhs = 0.0;
    let options = SolveOptions { feasibility_tolerance: f64::from_bits(1), ..Default::default() };
    let uncertainties = [risk::BudgetUncertainty { constraint: 0, radii: vec![f64::MIN_POSITIVE; 2], budget: 1.5 }];
    let checked = risk::validate_robust_budget_solution(&original, &uncertainties, &[1e-16, 2e-16], &options)?;
    assert!(!checked.feasible, "Individually rounded products must not erase positive combined support");
    assert!(checked.max_violation > options.feasibility_tolerance);
    Ok(())
}

fn linear_model() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "runtime review".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: -1.0 }],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}

#[test]
fn confidence_zero_cvar_is_weighted_mean_with_unequal_exact_weights() -> TestResult {
    // Exact raw-weight numerator: 3*10^17 - 2 - 5*6*10^16 = -2.
    // Neither a finite-tail subtraction nor normalized-probability rounding may turn this into +3.8.
    let actual = risk::weighted_upper_tail_cvar(&[1e17, -1.0, -6e16], &[3.0, 2.0, 5.0], 0.0)?;
    assert!((actual + 0.2).abs() < 1e-12, "weighted mean identity failed: {actual}");
    Ok(())
}

#[test]
fn finite_tail_controls_cover_atoms_tiny_constants_and_zero_weight_outliers() -> TestResult {
    let atom = risk::weighted_upper_tail_cvar(&[0.0, 10.0], &[3.0, 1.0], 0.5)?;
    assert!((atom - 5.0).abs() < 1e-12);
    let tiny = risk::weighted_upper_tail_cvar(&[1e-310], &[1.0], 1.0 - f64::EPSILON)?;
    assert_eq!(tiny, 1e-310);
    let zero_weight = risk::weighted_upper_tail_cvar(&[1.0, 1e300], &[1.0, 0.0], 0.95)?;
    assert_eq!(zero_weight, 1.0);
    Ok(())
}

#[test]
fn coefficient_box_counterpart_preserves_negative_coordinate_semantics() -> TestResult {
    let mut input = linear_model();
    input.variables[0].lower = Some(-10.0);
    input.variables[0].upper = Some(10.0);
    input.objective.linear[0].coefficient = 1.0;
    input.constraints.push(LinearConstraint {
        name: "uncertain row".into(),
        terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
        relation: Relation::LessEqual,
        rhs: 3.0,
    });
    // x + 2|x| <= 3 allows -3 <= x <= 1.
    let counterpart = risk::robust_box(&input, &[risk::BoxUncertainty { constraint: 0, radii: vec![2.0] }], &SolveOptions::default())?;
    let answer = solve(&counterpart, &SolveOptions::default(), &NoControl)?;
    assert_eq!(answer.feasibility, Feasibility::Feasible);
    assert_eq!(answer.optimality, Optimality::Optimal);
    assert!((answer.objective.ok_or("missing objective")? + 3.0).abs() < 1e-10);
    let values = answer.values.ok_or("missing decision values")?;
    let x = *values.first().ok_or("missing x")?;
    assert!(x + 2.0 * x.abs() <= 3.0 + 1e-10);
    assert_eq!(input.variables.len(), 1, "input model was modified");
    Ok(())
}

#[test]
fn transformed_dimension_and_manifest_escape_limits_are_explicit() -> TestResult {
    let mut input = linear_model();
    input.variables = (0..model::MAX_VARIABLES)
        .map(|i| Variable { name: format!("x{i}"), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous })
        .collect();
    input.constraints.push(LinearConstraint { name: "row".into(), terms: vec![], relation: Relation::LessEqual, rhs: 1.0 });
    let mut radii = vec![0.0; model::MAX_VARIABLES];
    radii[0] = 1.0;
    let err = risk::robust_box(&input, &[risk::BoxUncertainty { constraint: 0, radii }], &SolveOptions::default())
        .err()
        .ok_or("oversized counterpart unexpectedly accepted")?;
    assert_eq!(err.code, "model_limit");

    let mut escaped = linear_model();
    escaped.name = "\0".repeat(4096);
    let options = SolveOptions { max_memory_bytes: 8192, ..SolveOptions::default() };
    model::validate(&escaped, &options)?;
    let err = provenance::manifest(&escaped, &options).err().ok_or("oversized serialized manifest unexpectedly accepted")?;
    assert_eq!(err.code, "memory_limit");
    Ok(())
}

fn wait_terminal<T: Clone>(handle: &jobs::JobHandle<T>) -> std::result::Result<jobs::JobSnapshot<T>, Box<dyn std::error::Error>> {
    let until = solvercraft::runtime::monotonic_now() + Duration::from_secs(5);
    loop {
        let state = handle.snapshot()?;
        if matches!(state.status, jobs::JobStatus::Completed | jobs::JobStatus::Failed) {
            return Ok(state);
        }
        if solvercraft::runtime::monotonic_now() >= until {
            return Err("background job failed to terminate in review timeout".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[allow(clippy::panic)]
fn intentional_evaluator_panic() -> ! {
    panic!("intentional isolated QA evaluator failure")
}

#[test]
fn native_job_quota_panic_cancel_and_shared_results_have_real_lifecycle() -> TestResult {
    let (started_tx, started_rx) = mpsc::channel();
    let mut gates = Vec::new();
    let mut handles = Vec::new();
    for index in 0..4 {
        let (gate_tx, gate_rx) = mpsc::channel();
        gates.push(gate_tx);
        let started = started_tx.clone();
        let mut first = true;
        handles.push(jobs::start_simulation(
            vec![Distribution::Constant { value: 1.0 }],
            SimulationOptions { trials: 3, ..SimulationOptions::default() },
            move |values| {
                if first {
                    first = false;
                    started.send(index).map_err(|_| SolverError::new("qa_channel", "start signal receiver closed"))?;
                    gate_rx.recv().map_err(|_| SolverError::new("qa_channel", "test gate closed"))?;
                    if index == 0 {
                        intentional_evaluator_panic();
                    }
                }
                Ok(values.to_vec())
            },
        )?);
    }
    for _ in 0..4 {
        let _ = started_rx.recv_timeout(Duration::from_secs(5))?;
    }
    let rejected = jobs::start_solve(linear_model(), SolveOptions::default()).err().ok_or("fifth active job unexpectedly accepted")?;
    assert_eq!(rejected.code, "job_limit");
    handles.get(1).ok_or("missing cancellable job")?.cancel();
    for gate in gates {
        gate.send(())?;
    }

    let failed = wait_terminal(handles.first().ok_or("missing panic job")?)?;
    assert_eq!(failed.status, jobs::JobStatus::Failed);
    match failed.result.ok_or("missing panic result")? {
        Err(e) => assert_eq!(e.code, "job_panic"),
        Ok(_) => return Err("panic became a successful job result".into()),
    }
    let cancelled = wait_terminal(handles.get(1).ok_or("missing cancelled job")?)?;
    let cancelled_result = cancelled.result.ok_or("missing cancelled result")??;
    assert_eq!(cancelled_result.termination, Termination::Cancelled);
    assert!(cancelled_result.completed_trials < 3);

    for handle in handles.iter().skip(2) {
        let first = wait_terminal(handle)?;
        let second = handle.snapshot()?;
        let first_result = first.result.ok_or("missing completed result")??;
        let second_result = second.result.ok_or("missing repeated result")??;
        assert!(Arc::ptr_eq(&first_result, &second_result), "snapshot cloned rather than shared its completed result");
        assert_eq!(first_result.completed_trials, 3);
    }
    // Terminal state may be published just before the worker's slot guard drops.
    let until = solvercraft::runtime::monotonic_now() + Duration::from_secs(5);
    let recovered = loop {
        match jobs::start_solve(linear_model(), SolveOptions::default()) {
            Ok(handle) => break handle,
            Err(e) if e.code == "job_limit" && solvercraft::runtime::monotonic_now() < until => std::thread::yield_now(),
            Err(e) => return Err(e.into()),
        }
    };
    let result = wait_terminal(&recovered)?.result.ok_or("missing recovered solve")??;
    assert_eq!(result.optimality, Optimality::Optimal);
    Ok(())
}

#[test]
fn fractional_tail_mass_matches_high_precision_binary_input_reference_and_permutations() -> TestResult {
    // Independent Decimal(prec=100) integration uses Decimal.from_float(0.05),
    // so the oracle includes the actual binary confidence value.
    let losses = [3e16, -1.0, -2e16];
    let weights = [3.0, 2.0, 5.0];
    let expected = -0.152_093_525_019_728_61;
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let values = order.map(|i| losses[i]);
        let masses = order.map(|i| weights[i]);
        let actual = risk::weighted_upper_tail_cvar(&values, &masses, 0.05)?;
        assert!((actual - expected).abs() < 1e-11, "fractional mass or ordering changed risk: {actual}");
    }
    Ok(())
}

#[test]
fn finite_weights_are_scale_invariant_at_normal_and_subnormal_extremes() -> TestResult {
    for weights in [[1.0, 1.0], [f64::MAX, f64::MAX], [f64::from_bits(1), f64::from_bits(1)]] {
        let mean = risk::weighted_upper_tail_cvar(&[2.0, 4.0], &weights, 0.0)?;
        let tail = risk::weighted_upper_tail_cvar(&[2.0, 4.0], &weights, 0.5)?;
        assert!((mean - 3.0).abs() < 1e-12);
        assert_eq!(tail, 4.0);
    }
    let tiny_active = risk::weighted_upper_tail_cvar(&[1e-310, f64::MAX], &[1.0, 0.0], 0.95)?;
    assert_eq!(tiny_active, 1e-310);
    Ok(())
}

#[test]
fn small_integer_weight_tail_means_match_exact_rational_mass_accounting() -> TestResult {
    for seed in 0_i64..32 {
        let values = [seed % 7 - 3, (seed * 3) % 11 - 5, (seed * 5) % 13 - 6];
        let weights = [seed % 3 + 1, (seed * 2) % 5 + 1, (seed * 3) % 7 + 1];
        let mut ordered = values.into_iter().zip(weights).collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
        for (numerator, denominator) in [(0, 1), (1, 2), (3, 4), (19, 20)] {
            let total = weights.into_iter().sum::<i64>();
            let tail_units = total * (denominator - numerator);
            let mut remaining = tail_units;
            let mut area = 0_i64;
            for (value, weight) in &ordered {
                let take = remaining.min(weight * denominator);
                area += value * take;
                remaining -= take;
                if remaining == 0 {
                    break;
                }
            }
            let expected = area as f64 / tail_units as f64;
            let actual =
                risk::weighted_upper_tail_cvar(&values.map(|v| v as f64), &weights.map(|v| v as f64), numerator as f64 / denominator as f64)?;
            assert!((actual - expected).abs() < 1e-11, "seed={seed}, alpha={numerator}/{denominator}: {actual} vs {expected}");
        }
    }
    Ok(())
}

#[test]
fn risk_request_fields_are_strict_and_unreliable_extremes_fail_explicitly() -> TestResult {
    let bad_scenario = serde_json::json!({"weight":1,"offset":0,"coefficients":[1],"weigth":2});
    assert!(serde_json::from_value::<risk::Scenario>(bad_scenario).is_err());
    assert!(
        serde_json::from_value::<risk::RiskMeasure>(serde_json::json!({"kind":"conditional_value_at_risk","confidence":0.95,"confidnce":0.9}))
            .is_err()
    );
    assert!(serde_json::from_value::<risk::RiskMeasure>(serde_json::json!({"kind":"expected_loss","confidence":0.95})).is_err());
    let uncertain = risk::weighted_upper_tail_cvar(&[1e300, -1.0, -1e300], &[1.0, 1.0, 1.0], 0.0)
        .err()
        .ok_or("ill-conditioned data produced an unqualified risk number")?;
    assert_eq!(uncertain.code, "numerical_failure");
    Ok(())
}

#[test]
fn scenario_optimization_normalizes_large_finite_weights_without_changing_decision() -> TestResult {
    let input = risk::ScenarioProblem {
        name: "large finite weights".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        constraints: vec![],
        scenarios: vec![
            risk::Scenario { weight: f64::MAX, offset: 0.0, coefficients: vec![1.0] },
            risk::Scenario { weight: f64::MAX, offset: 4.0, coefficients: vec![-3.0] },
        ],
        measure: risk::RiskMeasure::ExpectedLoss,
    };
    let result = risk::optimize(&input, &SolveOptions::default(), &NoControl)?;
    assert_eq!(result.solve.optimality, Optimality::Optimal);
    assert!((result.decision_values.ok_or("missing scenario decision")?[0] - 2.0).abs() < 1e-12);
    assert!(result.expected_loss.ok_or("missing expected loss")?.abs() < 1e-12);
    let mut unknown = serde_json::to_value(input)?;
    unknown["scenarioz"] = serde_json::json!([]);
    assert!(serde_json::from_value::<risk::ScenarioProblem>(unknown).is_err());
    assert!(serde_json::from_value::<risk::BoxUncertainty>(serde_json::json!({"constraint":0,"radii":[1.0],"radius":2.0})).is_err());
    Ok(())
}
