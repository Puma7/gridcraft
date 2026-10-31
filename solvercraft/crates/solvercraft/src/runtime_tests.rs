use super::*;
use types::*;
fn small() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "runtime".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(0.0), upper: Some(1.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}
#[test]
fn manifest_limits_json_expansion_without_an_unbounded_buffer() {
    let mut model = small();
    model.name = "\u{1}".repeat(4096);
    let options = SolveOptions { max_memory_bytes: 6000, ..SolveOptions::default() };
    assert!(model::validate(&model, &options).is_ok());
    let result = provenance::manifest(&model, &options);
    assert!(matches!(result,Err(SolverError{code,..}) if code=="memory_limit"));
}
#[test]
fn manifest_identity_is_stable_and_seed_lossless() -> Result<()> {
    let model = small();
    let options = SolveOptions { seed: u64::MAX, ..SolveOptions::default() };
    let a = provenance::manifest(&model, &options)?;
    let b = provenance::manifest(&model, &options)?;
    assert_eq!(a.model_fingerprint, b.model_fingerprint);
    let text = serde_json::to_string(&a).map_err(|e| SolverError::new("test_json", e.to_string()))?;
    assert!(text.contains("\"18446744073709551615\""));
    Ok(())
}
#[test]
fn cvar_size_limits_precede_counterpart_allocation() {
    let problem = risk::ScenarioProblem {
        name: "large".into(),
        variables: small().variables,
        constraints: vec![],
        scenarios: vec![risk::Scenario { weight: 1.0, offset: 0.0, coefficients: vec![1.0] }; 10001],
        measure: risk::RiskMeasure::ConditionalValueAtRisk { confidence: 0.95 },
    };
    assert!(matches!(risk::compile(&problem,&SolveOptions::default()),Err(SolverError{code,..}) if code=="model_limit"));
}
#[test]
fn invalid_base_constraints_are_rejected_before_clone() {
    let problem = risk::ScenarioProblem {
        name: "invalid".into(),
        variables: small().variables,
        constraints: vec![LinearConstraint {
            name: "bad".into(),
            terms: vec![LinearTerm { variable: 99, coefficient: 1.0 }],
            relation: Relation::LessEqual,
            rhs: 2.0,
        }],
        scenarios: vec![risk::Scenario { weight: 1.0, offset: 0.0, coefficients: vec![1.0] }],
        measure: risk::RiskMeasure::ExpectedLoss,
    };
    assert!(risk::compile(&problem, &SolveOptions::default()).is_err());
}

#[test]
fn scenario_aggregation_does_not_lose_small_opposing_terms() -> Result<()> {
    let p = risk::ScenarioProblem {
        name: "cancellation".into(),
        variables: small().variables,
        constraints: vec![],
        scenarios: vec![1e17, -1.0, -1e17].into_iter().map(|v| risk::Scenario { weight: 1.0, offset: 0.0, coefficients: vec![v] }).collect(),
        measure: risk::RiskMeasure::ExpectedLoss,
    };
    let result = risk::optimize(&p, &SolveOptions::default(), &NoControl)?;
    let x = result.decision_values.ok_or_else(|| SolverError::new("test", "Missing solution"))?;
    assert_eq!(x, vec![1.0]);
    assert!((result.expected_loss.ok_or_else(|| SolverError::new("test", "Missing mean"))? + 1.0 / 3.0).abs() < 1e-12);
    let tail = risk::weighted_upper_tail_cvar(&[1e17, -1.0, -1e17], &[1.0; 3], 0.0)?;
    assert!((tail + 1.0 / 3.0).abs() < 1e-12);
    Ok(())
}

#[test]
fn a_tiny_constant_tail_mean_stays_constant() -> Result<()> {
    let x = 1e-310;
    let result = risk::weighted_upper_tail_cvar(&[x], &[1.0], 1.0 - f64::EPSILON)?;
    assert_eq!(result, x);
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn background_solve_is_observable_and_returns_a_real_solution() -> Result<()> {
    let handle = jobs::start_solve(small(), SolveOptions::default())?;
    let start = std::time::Instant::now();
    loop {
        let snapshot = handle.snapshot()?;
        if let Some(result) = snapshot.result {
            let result = result?;
            assert_eq!(result.feasibility, Feasibility::Feasible);
            assert_eq!(result.objective, Some(0.0));
            break;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn background_simulation_can_cancel_after_a_trial_has_started() -> Result<()> {
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut first = true;
    let handle = jobs::start_simulation(
        vec![Distribution::Constant { value: 1.0 }],
        SimulationOptions { trials: 1000, ..SimulationOptions::default() },
        move |x| {
            if first {
                first = false;
                started_tx.send(()).map_err(|e| SolverError::new("test_channel", e.to_string()))?;
                release_rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|e| SolverError::new("test_channel", e.to_string()))?;
            }
            Ok(x.to_vec())
        },
    )?;
    started_rx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|e| SolverError::new("test_channel", e.to_string()))?;
    handle.cancel();
    release_tx.send(()).map_err(|e| SolverError::new("test_channel", e.to_string()))?;
    let start = std::time::Instant::now();
    loop {
        let snapshot = handle.snapshot()?;
        if let Some(result) = snapshot.result {
            let result = result?;
            assert_eq!(result.termination, Termination::Cancelled);
            assert!(result.completed_trials < 1000);
            break;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}
