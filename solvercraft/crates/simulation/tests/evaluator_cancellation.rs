#![allow(clippy::unwrap_used)]
use solvercraft_simulation::{
    advanced::{AdvancedSimulationOptions, simulate_advanced},
    simulate,
};
use solvercraft_types::{NoControl, RunControl, SimulationOptions, SolverError, Termination};
use std::sync::atomic::{AtomicBool, Ordering};
struct Control(AtomicBool);
impl RunControl for Control {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
#[test]
fn an_interrupted_evaluator_never_admits_its_current_row() {
    for returned_error in [false, true] {
        let control = Control(AtomicBool::new(false));
        let mut calls = 0;
        let options = SimulationOptions { trials: 5, retain_samples: true, ..Default::default() };
        let report = simulate(&[], &options, &control, |_| {
            calls += 1;
            if calls == 2 {
                control.0.store(true, Ordering::SeqCst);
                if returned_error {
                    return Err(SolverError::new("cancelled", "interrupted formula"));
                }
            }
            Ok(vec![calls as f64])
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(report.termination, Termination::Cancelled);
        assert_eq!(report.completed_trials, 1);
        assert_eq!(report.samples.unwrap(), vec![vec![1.0]]);
        assert_eq!(report.summaries[0].mean, 1.0);
        let control = Control(AtomicBool::new(false));
        let mut calls = 0;
        let report = simulate_advanced(&[], &AdvancedSimulationOptions { base: options, ..Default::default() }, &control, |_| {
            calls += 1;
            if calls == 2 {
                control.0.store(true, Ordering::SeqCst);
                if returned_error {
                    return Err(SolverError::new("cancelled", "interrupted formula"));
                }
            }
            Ok(vec![calls as f64])
        })
        .unwrap();
        assert_eq!(report.result.termination, Termination::Cancelled);
        assert_eq!(report.attempted_trials, 1);
        assert_eq!(report.invalid_trials, 0);
        assert_eq!(report.result.completed_trials, 1);
        assert_eq!(report.result.samples.unwrap(), vec![vec![1.0]]);
    }
}
#[test]
fn cancellation_before_the_first_return_has_no_fake_statistics() {
    for error in [None, Some("cancelled"), Some("genuine_error_raced_with_cancellation")] {
        let control = Control(AtomicBool::new(false));
        let options = SimulationOptions { trials: 2, retain_samples: true, ..Default::default() };
        let report = simulate(&[], &options, &control, |_| {
            control.0.store(true, Ordering::SeqCst);
            if let Some(code) = error { Err(SolverError::new(code, "race")) } else { Ok(vec![f64::NAN]) }
        })
        .unwrap();
        assert_eq!(report.termination, Termination::Cancelled);
        assert_eq!(report.completed_trials, 0);
        assert!(report.summaries.is_empty());
        assert!(report.samples.unwrap().is_empty());
        let control = Control(AtomicBool::new(false));
        let report = simulate_advanced(&[], &AdvancedSimulationOptions { base: options, ..Default::default() }, &control, |_| {
            control.0.store(true, Ordering::SeqCst);
            if let Some(code) = error { Err(SolverError::new(code, "race")) } else { Ok(vec![f64::NAN]) }
        })
        .unwrap();
        assert_eq!(report.result.termination, Termination::Cancelled);
        assert_eq!(report.attempted_trials, 0);
        assert_eq!(report.invalid_trials, 0);
        assert!(report.result.summaries.is_empty());
        assert!(report.result.samples.unwrap().is_empty());
    }
    let error =
        simulate(&[], &SimulationOptions { trials: 2, ..Default::default() }, &NoControl, |_| Err(SolverError::new("genuine", "formula failed")))
            .unwrap_err();
    assert_eq!(error.code, "genuine");
    let error = simulate_advanced(&[], &AdvancedSimulationOptions::default(), &NoControl, |_| Err(SolverError::new("genuine", "formula failed")))
        .unwrap_err();
    assert_eq!(error.code, "genuine");
}
