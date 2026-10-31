use super::*;
use solvercraft_types::NoControl;
use std::sync::atomic::{AtomicU64, Ordering};

struct Cancelled;
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}
struct CancelAfterProgress {
    count: AtomicU64,
    limit: u64,
}
impl RunControl for CancelAfterProgress {
    fn is_cancelled(&self) -> bool {
        self.count.load(Ordering::Relaxed) >= self.limit
    }
    fn progress(&self, event: &ProgressEvent) {
        self.count.store(event.iterations, Ordering::Relaxed);
    }
}
fn options(trials: u64, retain_samples: bool, method: SamplingMethod) -> SimulationOptions {
    SimulationOptions { trials, retain_samples, method, ..SimulationOptions::default() }
}

#[test]
fn constant_and_single_observation_statistics() -> Result<()> {
    let result = simulate(&[], &options(100, false, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![17.0]))?;
    assert_eq!(result.completed_trials, 100);
    assert_eq!(result.summaries[0].mean, 17.0);
    assert_eq!(result.summaries[0].variance, Some(0.0));
    assert!(result.samples.is_none());
    assert!(result.summaries[0].quantiles.is_empty());
    assert_eq!(result.summaries[0].conditional_value_at_risk_95, None);
    let one = simulate(&[], &options(1, true, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![17.0]))?;
    assert_eq!(one.summaries[0].variance, None);
    assert_eq!(one.summaries[0].value_at_risk_95, Some(17.0));
    Ok(())
}
#[test]
fn lhs_occupies_each_stratum_once_in_each_dimension() -> Result<()> {
    let distributions = vec![Distribution::Uniform { low: 0.0, high: 1.0 }; 3];
    let result = simulate(&distributions, &options(128, true, SamplingMethod::LatinHypercube), &NoControl, |x| Ok(x.to_vec()))?;
    let samples = result.samples.ok_or_else(|| SolverError::new("test", "Missing retained samples."))?;
    for column in 0..3 {
        let mut strata: Vec<usize> = samples.iter().map(|row| (row[column] * 128.0).floor() as usize).collect();
        strata.sort_unstable();
        assert_eq!(strata, (0..128).collect::<Vec<_>>());
    }
    assert_ne!(samples.iter().map(|row| row[0]).collect::<Vec<_>>(), samples.iter().map(|row| row[1]).collect::<Vec<_>>());
    Ok(())
}
#[test]
fn seed_repeatability_and_same_trial_reads() -> Result<()> {
    let input = vec![Distribution::Normal { mean: 3.0, std_dev: 2.0 }];
    let opts = options(1000, true, SamplingMethod::MonteCarlo);
    let first = simulate(&input, &opts, &NoControl, |x| Ok(vec![x[0], x[0]]))?;
    let again = simulate(&input, &opts, &NoControl, |x| Ok(vec![x[0], x[0]]))?;
    assert_eq!(first.samples, again.samples);
    assert_eq!(first.summaries[1].mean, first.summaries[0].mean);
    assert_eq!(first.summaries[1].variance, first.summaries[0].variance);
    Ok(())
}
#[test]
fn fixed_seed_moments_match_analytic_references() -> Result<()> {
    let distributions = [
        Distribution::Uniform { low: 0.0, high: 1.0 },
        Distribution::Normal { mean: 3.0, std_dev: 2.0 },
        Distribution::Binomial { trials: 10, probability: 0.3 },
        Distribution::Poisson { rate: 4.0 },
    ];
    let result = simulate(&distributions, &options(50_000, false, SamplingMethod::MonteCarlo), &NoControl, |x| Ok(x.to_vec()))?;
    let expected = [(0.5, 1.0 / 12.0, 0.008, 0.003), (3.0, 4.0, 0.04, 0.12), (3.0, 2.1, 0.05, 0.10), (4.0, 4.0, 0.06, 0.18)];
    for (summary, (mean, variance, mean_tolerance, variance_tolerance)) in result.summaries.iter().zip(expected) {
        assert!((summary.mean - mean).abs() < mean_tolerance, "mean {}", summary.mean);
        let actual = summary.variance.ok_or_else(|| SolverError::new("test", "Missing variance."))?;
        assert!((actual - variance).abs() < variance_tolerance, "variance {actual}");
    }
    Ok(())
}
#[test]
fn streaming_large_offset_moments_do_not_use_difference_of_squares() -> Result<()> {
    let mut moments = OnlineMoments::default();
    for value in [1e12 + 1.0, 1e12 + 2.0, 1e12 + 3.0, 1e12 + 4.0] {
        moments.push(value)?;
    }
    let summary = moments.summary()?;
    assert_eq!(summary.mean, 1e12 + 2.5);
    assert!(summary.variance.is_some_and(|v| (v - 5.0 / 3.0).abs() < 1e-12));
    assert!(OnlineMoments::default().summary().is_err());
    Ok(())
}
#[test]
fn cancellation_has_no_fake_statistics_and_can_return_partial_data() -> Result<()> {
    let result = simulate(&[], &options(100, true, SamplingMethod::MonteCarlo), &Cancelled, |_| Err(SolverError::new("test", "Must not evaluate")))?;
    assert_eq!(result.termination, Termination::Cancelled);
    assert_eq!(result.completed_trials, 0);
    assert!(result.summaries.is_empty());
    let control = CancelAfterProgress { count: AtomicU64::new(0), limit: 256 };
    let partial = simulate(&[], &options(1000, true, SamplingMethod::MonteCarlo), &control, |_| Ok(vec![5.0]))?;
    assert_eq!(partial.termination, Termination::Cancelled);
    assert_eq!(partial.completed_trials, 256);
    assert_eq!(partial.summaries[0].count, 256);
    assert_eq!(partial.samples.as_ref().map(Vec::len), Some(256));
    Ok(())
}
#[test]
fn memory_estimates_bound_retention_and_lhs_before_allocating() -> Result<()> {
    let distribution = [Distribution::Uniform { low: 0.0, high: 1.0 }];
    let mut opts = options(10_000, false, SamplingMethod::MonteCarlo);
    opts.max_memory_bytes = 4096;
    assert!(simulate(&distribution, &opts, &NoControl, |x| Ok(vec![x[0]])).is_ok());
    opts.retain_samples = true;
    assert!(simulate(&distribution, &opts, &NoControl, |x| Ok(vec![x[0]])).is_err_and(|e| e.code == "memory_limit"));
    opts.retain_samples = false;
    opts.method = SamplingMethod::LatinHypercube;
    assert!(simulate(&distribution, &opts, &NoControl, |x| Ok(vec![x[0]])).is_err_and(|e| e.code == "memory_limit"));
    Ok(())
}
#[test]
fn invalid_trials_outputs_and_shape_are_errors() {
    assert!(simulate(&[], &options(0, false, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![1.0])).is_err());
    assert!(simulate(&[], &options(u64::MAX, false, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![1.0])).is_err());
    assert!(simulate(&[], &options(2, false, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![f64::NAN])).is_err());
    assert!(simulate(&[], &options(2, false, SamplingMethod::MonteCarlo), &NoControl, |_| Ok(vec![])).is_err());
    let mut calls = 0;
    let result = simulate(&[], &options(2, false, SamplingMethod::MonteCarlo), &NoControl, |_| {
        calls += 1;
        Ok(vec![0.0; calls])
    });
    assert!(result.is_err_and(|e| e.code == "output_dimension"));
    let result =
        simulate(&[], &options(2, false, SamplingMethod::MonteCarlo), &NoControl, |_| Err(SolverError::new("host_error", "No silent skip.")));
    assert!(result.is_err_and(|e| e.code == "host_error"));
}
#[test]
fn elementary_fits_report_mle_parameters_without_gof_claims() -> Result<()> {
    match fit_normal(&[0.0, 2.0])? {
        Distribution::Normal { mean, std_dev } => {
            assert_eq!(mean, 1.0);
            assert_eq!(std_dev, 1.0);
        }
        _ => return Err(SolverError::new("test", "Wrong fit family.")),
    }
    match fit_uniform(&[-1.0, 4.0])? {
        Distribution::Uniform { low, high } => {
            assert_eq!(low, -1.0);
            assert_eq!(high, 4.0);
        }
        _ => return Err(SolverError::new("test", "Wrong fit family.")),
    }
    match fit_exponential(&[1.0, 3.0])? {
        Distribution::Exponential { rate } => assert_eq!(rate, 0.5),
        _ => return Err(SolverError::new("test", "Wrong fit family.")),
    }
    assert!(fit_exponential(&[-1.0, 2.0]).is_err());
    assert!(fit_exponential(&[0.0, 0.0]).is_err());
    assert!(fit_normal(&[]).is_err());
    Ok(())
}
