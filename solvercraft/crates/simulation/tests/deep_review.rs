//! Independent deterministic simulation and fitting review cases.

#[test]
fn very_wide_streaming_output_is_budgeted_before_the_second_trial() {
    let mut opts = options(10_000_000, SamplingMethod::MonteCarlo);
    opts.retain_samples = false;
    let mut calls = 0;
    let result = simulate(&[], &opts, &NoControl, |_| {
        calls += 1;
        if calls > 1 {
            return Err(SolverError::new("review_stop", "Output work guard must run on first shape discovery"));
        }
        Ok(vec![0.0; 4096])
    });
    assert!(result.is_err_and(|error| error.code == "work_limit"));
    assert_eq!(calls, 1);
}

use solvercraft_simulation::{OnlineMoments, fit_exponential, fit_normal, fit_uniform, simulate};
use solvercraft_types::{Distribution, NoControl, Result, RunControl, SamplingMethod, SimulationOptions, SolverError, Termination};
use std::sync::atomic::{AtomicBool, Ordering};

fn options(trials: u64, method: SamplingMethod) -> SimulationOptions {
    SimulationOptions { trials, seed: u64::MAX, method, retain_samples: true, max_memory_bytes: 8 * 1024 * 1024 }
}

#[test]
fn variance_is_translation_and_order_invariant_for_representable_two_point_data() -> Result<()> {
    // Both inputs are exactly representable. The exact unbiased variance is 2.
    // The exact mean is not representable at this offset; that must not erase M2.
    for values in [[1e16, 1e16 + 2.0], [1e16 + 2.0, 1e16], [0.0, 2.0], [2.0, 0.0]] {
        let mut moments = OnlineMoments::default();
        for value in values {
            moments.push(value)?;
        }
        assert_eq!(moments.summary()?.variance, Some(2.0), "values={values:?}");
    }
    Ok(())
}

#[test]
fn fitting_uniform_does_not_require_representable_sample_variance() -> Result<()> {
    match fit_uniform(&[1e200, 2e200])? {
        Distribution::Uniform { low, high } => {
            assert_eq!(low, 1e200);
            assert_eq!(high, 2e200);
        }
        _ => return Err(SolverError::new("review", "Wrong distribution family")),
    }
    Ok(())
}

#[test]
fn fitting_exponential_does_not_require_representable_sample_variance() -> Result<()> {
    match fit_exponential(&[1e200, 2e200])? {
        Distribution::Exponential { rate } => assert!((rate / (1.0 / 1.5e200) - 1.0).abs() < 1e-14),
        _ => return Err(SolverError::new("review", "Wrong distribution family")),
    }
    Ok(())
}

#[test]
fn fits_obey_mle_and_degenerate_domains() -> Result<()> {
    match fit_normal(&[-3.0, -1.0, 1.0, 3.0])? {
        Distribution::Normal { mean, std_dev } => {
            assert_eq!(mean, 0.0);
            assert!((std_dev - 5.0_f64.sqrt()).abs() < 1e-15);
        }
        _ => return Err(SolverError::new("review", "Wrong normal fit family")),
    }
    match fit_uniform(&[17.0])? {
        Distribution::Uniform { low, high } => {
            assert_eq!(low, 17.0);
            assert_eq!(high, 17.0);
        }
        _ => return Err(SolverError::new("review", "Wrong uniform fit family")),
    }
    assert!(fit_normal(&[f64::NAN]).is_err());
    assert!(fit_exponential(&[0.0]).is_err());
    assert!(fit_exponential(&[-1.0]).is_err());
    Ok(())
}

#[test]
fn lhs_exactly_covers_prime_sized_strata_with_maximum_seed() -> Result<()> {
    let inputs = vec![Distribution::Uniform { low: 0.0, high: 1.0 }; 3];
    let first = simulate(&inputs, &options(257, SamplingMethod::LatinHypercube), &NoControl, |values| Ok(values.to_vec()))?;
    let repeated = simulate(&inputs, &options(257, SamplingMethod::LatinHypercube), &NoControl, |values| Ok(values.to_vec()))?;
    assert_eq!(first.samples, repeated.samples);
    let samples = first.samples.ok_or_else(|| SolverError::new("review", "Missing samples"))?;
    for column in 0..3 {
        let mut strata = samples.iter().map(|row| (row[column] * 257.0).floor() as usize).collect::<Vec<_>>();
        strata.sort_unstable();
        assert_eq!(strata, (0..257).collect::<Vec<_>>());
    }
    Ok(())
}

#[test]
fn streaming_mode_handles_large_trial_count_under_small_memory_budget() -> Result<()> {
    let mut opts = options(50_000, SamplingMethod::MonteCarlo);
    opts.retain_samples = false;
    opts.max_memory_bytes = 4096;
    let result = simulate(&[], &opts, &NoControl, |_| Ok(vec![5.0]))?;
    assert_eq!(result.completed_trials, 50_000);
    assert_eq!(result.summaries[0].mean, 5.0);
    assert_eq!(result.summaries[0].variance, Some(0.0));
    assert!(result.samples.is_none());
    opts.retain_samples = true;
    assert!(simulate(&[], &opts, &NoControl, |_| Ok(vec![5.0])).is_err_and(|error| error.code == "memory_limit"));
    Ok(())
}

#[test]
fn failed_observation_does_not_corrupt_prior_online_summary() -> Result<()> {
    let mut moments = OnlineMoments::default();
    moments.push(1.0)?;
    moments.push(3.0)?;
    assert!(moments.push(f64::INFINITY).is_err());
    let prior = moments.summary()?;
    assert_eq!(prior.count, 2);
    assert_eq!(prior.mean, 2.0);
    assert_eq!(prior.variance, Some(2.0));
    Ok(())
}

struct Cancelled(AtomicBool);
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
#[test]
fn cancelled_setup_does_not_evaluate_or_return_fake_summary() -> Result<()> {
    let result = simulate(&[], &options(50_000, SamplingMethod::LatinHypercube), &Cancelled(AtomicBool::new(true)), |_| {
        Err(SolverError::new("review", "Evaluator must not be called"))
    })?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.completed_trials, 0);
    assert!(result.summaries.is_empty());
    Ok(())
}

#[test]
fn normal_mle_can_return_finite_standard_deviation_when_variance_overflows() -> Result<()> {
    match fit_normal(&[1e200, 2e200])? {
        Distribution::Normal { mean, std_dev } => {
            assert!((mean / 1.5e200 - 1.0).abs() < 1e-14);
            assert!((std_dev / 5e199 - 1.0).abs() < 1e-14);
        }
        _ => return Err(SolverError::new("review", "Wrong normal fit family")),
    }
    match fit_normal(&[-1e308, 1e308])? {
        Distribution::Normal { mean, std_dev } => {
            assert_eq!(mean, 0.0);
            assert_eq!(std_dev, 1e308);
        }
        _ => return Err(SolverError::new("review", "Wrong normal fit family")),
    }
    Ok(())
}

#[test]
fn compensated_online_mean_retains_small_terms_between_cancelling_large_values() -> Result<()> {
    let mut moments = OnlineMoments::default();
    for value in [1e17, -1.0, -1e17] {
        moments.push(value)?;
    }
    assert!((moments.summary()?.mean + 1.0 / 3.0).abs() < 1e-15);
    Ok(())
}
