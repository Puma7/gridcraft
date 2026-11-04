use solvercraft_simulation::{OnlineMoments, advanced::*, fitting::*};
use solvercraft_types::*;
#[test]
fn retained_streaming_design_matrix_has_independent_two_pass_statistics() {
    let mut runs = 0;
    let mut columns = 0;
    for n in [8_u64, 31, 64, 257] {
        for seed in [0_u64, 42, u64::MAX] {
            for dimension in [1_usize, 3, 16] {
                for method in 0..3 {
                    let mut options = AdvancedSimulationOptions {
                        base: SimulationOptions { trials: n, seed, retain_samples: true, ..Default::default() },
                        ..Default::default()
                    };
                    if method == 1 {
                        options.base.method = SamplingMethod::LatinHypercube;
                    } else if method == 2 {
                        options.sampler = SamplerSpec::RandomizedSobol { replicate: seed };
                    }
                    let distributions = vec![Distribution::Uniform { low: 0.0, high: 1.0 }; dimension];
                    let result = simulate_advanced(&distributions, &options, &NoControl, |x| Ok(x.to_vec())).unwrap();
                    options.base.retain_samples = false;
                    let streamed = simulate_advanced(&distributions, &options, &NoControl, |x| Ok(x.to_vec())).unwrap();
                    let samples = result.result.samples.unwrap();
                    for column in 0..dimension {
                        let values = samples.iter().map(|v| v[column]).collect::<Vec<_>>();
                        let mean = values.iter().sum::<f64>() / n as f64;
                        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
                        let summary = &result.result.summaries[column];
                        assert!((summary.mean - mean).abs() < 2e-14);
                        assert!((summary.variance.unwrap() - variance).abs() < 2e-14);
                        assert_eq!(summary.mean.to_bits(), streamed.result.summaries[column].mean.to_bits());
                        assert_eq!(summary.variance.map(f64::to_bits), streamed.result.summaries[column].variance.map(f64::to_bits));
                        if method == 1 {
                            let mut bins = values.iter().map(|x| (x * n as f64).floor() as usize).collect::<Vec<_>>();
                            bins.sort_unstable();
                            assert_eq!(bins, (0..n as usize).collect::<Vec<_>>());
                            assert!((mean - 0.5).abs() <= 0.5 / n as f64);
                        }
                        if method == 2 {
                            let size = 1_usize << (63 - n.leading_zeros());
                            let mut bins = values[..size].iter().map(|x| (x * size as f64).floor() as usize).collect::<Vec<_>>();
                            bins.sort_unstable();
                            assert_eq!(bins, (0..size).collect::<Vec<_>>());
                            assert!(result.mean_intervals.is_empty());
                        }
                        columns += 1;
                    }
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 108);
    assert_eq!(columns, 720);
    eprintln!("distinct sampler run specs={runs}, independently recomputed output columns={columns}, retained+streamed executions={}", runs * 2);
}
#[test]
fn scaled_summary_and_closed_form_fit_matrix() {
    let mut cases = 0;
    for n in [2_usize, 3, 4, 7, 16, 31, 64] {
        for scale in [1e-100_f64, 0.001, 1.0, 1e3, 1e100] {
            for offset in [-3.0_f64, 0.0, 3.0] {
                let values = (0..n).map(|i| scale * (offset + (i % 7) as f64 - 3.0)).collect::<Vec<_>>();
                let normalized = values.iter().map(|x| x / scale).collect::<Vec<_>>();
                let mean = normalized.iter().sum::<f64>() / n as f64;
                let variance = normalized.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
                let mut stats = OnlineMoments::default();
                for x in &values {
                    stats.push(*x).unwrap();
                }
                let summary = stats.summary().unwrap();
                assert!((summary.mean / scale - mean).abs() < 2e-12);
                assert!((summary.variance.unwrap() / scale / scale - variance).abs() < 2e-12);
                let fitted = solvercraft_simulation::fit_normal(&values).unwrap();
                let Distribution::Normal { mean: actual, std_dev } = fitted else {
                    panic!("normal");
                };
                assert!((actual / scale - mean).abs() < 2e-12);
                assert!((std_dev / scale - (variance * (n - 1) as f64 / n as f64).sqrt()).abs() < 2e-12);
                cases += 1;
            }
        }
    }
    // Distinct likelihood datasets with closed-form Bernoulli/Poisson/
    // geometric MLEs; likelihood is independently computed from probabilities.
    for n in [4_usize, 8, 16, 32] {
        for successes in [1_usize, 2, 3] {
            let values = (0..n).map(|i| if i < successes { 1.0 } else { 0.0 }).collect::<Vec<_>>();
            let report = fit_distribution(&values, &FitFamily::Bernoulli, &FitOptions::default(), &NoControl).unwrap();
            let p = successes as f64 / n as f64;
            let expected = successes as f64 * p.ln() + (n - successes) as f64 * (-p).ln_1p();
            assert!((report.log_likelihood - expected).abs() < 1e-12);
            assert!((report.aic - (2.0 - 2.0 * expected)).abs() < 1e-12);
            cases += 1;
        }
    }
    assert_eq!(cases, 117);
    eprintln!("distinct summary/fit datasets={cases}");
}
