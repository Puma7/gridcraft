//! Analytic design invariants, independent of a fitted sampling distribution.
use solvercraft_simulation::advanced::{AdvancedSimulationOptions, DependenceSpec, SamplerSpec, simulate_advanced};
use solvercraft_types::{Distribution, NoControl, SimulationOptions};

#[test]
fn sobol_simulation_preserves_all_64_strata_prefixes_and_streaming_moments() {
    let inputs = vec![Distribution::Uniform { low: 0.0, high: 1.0 }; 64];
    let mut options = AdvancedSimulationOptions {
        base: SimulationOptions { trials: 256, seed: u64::MAX, retain_samples: true, ..Default::default() },
        sampler: SamplerSpec::RandomizedSobol { replicate: u64::MAX },
        ..Default::default()
    };
    let run = |options: &AdvancedSimulationOptions| simulate_advanced(&inputs, options, &NoControl, |x| Ok(x.to_vec())).unwrap();
    let retained = run(&options);
    let samples = retained.result.samples.as_ref().unwrap();
    for dimension in 0..64 {
        let mut bins = samples.iter().map(|row| (row[dimension] * 256.0).floor() as usize).collect::<Vec<_>>();
        bins.sort_unstable();
        assert_eq!(bins, (0..256).collect::<Vec<_>>(), "dimension={dimension}");
        // One value in every equal bin implies this deterministic mean bound.
        assert!((retained.result.summaries[dimension].mean - 0.5).abs() <= 0.5 / 256.0);
    }
    options.base.trials = 128;
    let prefix = run(&options);
    assert_eq!(prefix.result.samples.as_ref().unwrap(), &samples[..128]);
    options.base.trials = 256;
    options.base.retain_samples = false;
    let streamed = run(&options);
    for (a, b) in retained.result.summaries.iter().zip(&streamed.result.summaries) {
        assert_eq!(a.mean.to_bits(), b.mean.to_bits());
        assert_eq!(a.variance.map(f64::to_bits), b.variance.map(f64::to_bits));
    }
    assert!(streamed.result.samples.is_none());
    assert!(retained.mean_intervals.is_empty());
    assert!(retained.result.warnings.iter().any(|w| w.contains("no pointwise IID")));
}

#[test]
fn baseline_simulation_reports_the_numerical_catalog_revision_without_changing_rng_identity() {
    let result = solvercraft_simulation::simulate(
        &[Distribution::Constant { value: 1.0 }],
        &SimulationOptions { trials: 2, ..Default::default() },
        &NoControl,
        |x| Ok(x.to_vec()),
    )
    .unwrap();
    assert_eq!(solvercraft_math::SAMPLING_VERSION, "solvercraft-addressed-mix64-v2");
    assert_eq!(result.sampling_version, "solvercraft-addressed-mix64-v2/solvercraft-baseline17-r4-catalog-v3");
}

#[test]
fn singular_negative_gaussian_correlation_preserves_its_linear_constraint() {
    // This PSD matrix has null vector (1,1,1), hence X1+X2+X3=0.
    // The 1e-7 bound is fixed in advance for normal CDF/inverse approximation,
    // and does not depend on an observed Monte Carlo standard error.
    let correlation = vec![vec![1.0, -0.5, -0.5], vec![-0.5, 1.0, -0.5], vec![-0.5, -0.5, 1.0]];
    let options = AdvancedSimulationOptions {
        base: SimulationOptions { trials: 2048, seed: 73, retain_samples: true, ..Default::default() },
        dependence: DependenceSpec::Gaussian { correlation },
        ..Default::default()
    };
    let report = simulate_advanced(&vec![Distribution::Normal { mean: 0.0, std_dev: 1.0 }; 3], &options, &NoControl, |x| Ok(x.to_vec())).unwrap();
    for row in report.result.samples.unwrap() {
        assert!(row.iter().sum::<f64>().abs() < 1e-7, "row={row:?}");
    }
}
