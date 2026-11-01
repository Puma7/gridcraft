#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::panic)]
use solvercraft::{jobs, types::*};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
static SERIAL: Mutex<()> = Mutex::new(());
fn problem() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "job-control".into(),
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
fn error(message: impl Into<String>) -> SolverError {
    SolverError::new("test", message)
}
#[test]
fn real_background_job_quota_cancel_shared_results_and_reuse() -> Result<()> {
    let _serial = SERIAL.lock().map_err(|e| error(e.to_string()))?;
    let (started_tx, started_rx) = mpsc::channel();
    let mut release = vec![];
    let mut handles = vec![];
    for id in 0..4 {
        let (tx, rx) = mpsc::channel();
        release.push(tx);
        let started = started_tx.clone();
        let mut first = true;
        handles.push(jobs::start_simulation(
            vec![Distribution::Constant { value: id as f64 }],
            SimulationOptions { trials: 10000, ..SimulationOptions::default() },
            move |x| {
                if first {
                    first = false;
                    started.send(()).map_err(|e| error(e.to_string()))?;
                    rx.recv_timeout(Duration::from_secs(5)).map_err(|e| error(e.to_string()))?;
                }
                Ok(x.to_vec())
            },
        )?);
    }
    for _ in 0..4 {
        started_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| error(e.to_string()))?;
    }
    let fifth = jobs::start_solve(problem(), SolveOptions::default());
    assert!(matches!(fifth,Err(SolverError{code,..}) if code=="job_limit"));
    for (h, tx) in handles.iter().zip(release) {
        h.cancel();
        tx.send(()).map_err(|e| error(e.to_string()))?;
    }
    for handle in handles {
        let start = solvercraft::runtime::monotonic_now();
        loop {
            let snapshot = handle.snapshot()?;
            if let Some(result) = snapshot.result {
                let result = result?;
                assert_eq!(result.termination, Termination::Cancelled);
                assert!(result.completed_trials < 10000);
                let again = handle.snapshot()?.result.ok_or_else(|| error("Missing stored result"))??;
                assert!(Arc::ptr_eq(&result, &again));
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let start = solvercraft::runtime::monotonic_now();
    let handle = loop {
        match jobs::start_solve(problem(), SolveOptions::default()) {
            Ok(h) => break h,
            Err(e) if e.code == "job_limit" && start.elapsed() < Duration::from_secs(2) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => return Err(e),
        }
    };
    loop {
        if let Some(result) = handle.snapshot()?.result {
            assert_eq!(result?.objective, Some(0.0));
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
#[test]
fn evaluator_panic_is_observable_failure_and_slot_is_recovered() -> Result<()> {
    let _serial = SERIAL.lock().map_err(|e| error(e.to_string()))?;
    let handle = jobs::start_simulation(vec![Distribution::Constant { value: 1.0 }], SimulationOptions::default(), |_| -> Result<Vec<f64>> {
        panic!("intentional evaluator regression")
    })?;
    let start = solvercraft::runtime::monotonic_now();
    loop {
        let snapshot = handle.snapshot()?;
        if let Some(result) = snapshot.result {
            assert_eq!(snapshot.status, jobs::JobStatus::Failed);
            assert!(matches!(result,Err(SolverError{code,..}) if code=="job_panic"));
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    let next = jobs::start_solve(problem(), SolveOptions::default())?;
    loop {
        if let Some(result) = next.snapshot()?.result {
            assert_eq!(result?.feasibility, Feasibility::Feasible);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
