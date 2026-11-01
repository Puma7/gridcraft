//! Independent fitting likelihood identities and declared stopping assumptions.
use solvercraft_simulation::{advanced::*, fitting::*};
use solvercraft_types::*;
#[test]
fn resampling_preserves_intact_vectors_and_shuffle_preserves_all_cells() {
    use solvercraft_simulation::resampling::{PreparedTable, PreparedValues, ResamplingLimits, VectorAddress, VectorOrientation};
    let rows = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
    let prepared = PreparedTable::new(&rows, &ResamplingLimits::default(), &NoControl).expect("table");
    let columns = [vec![1.0, 4.0], vec![2.0, 5.0], vec![3.0, 6.0]];
    let mut crossed_source_rows = false;
    for trial in 0..32 {
        let address = VectorAddress { seed: 17, binding: 9007199254740993, trial };
        assert!(rows.contains(&prepared.mv_resample(VectorOrientation::Row, address, &NoControl).unwrap()));
        assert!(columns.contains(&prepared.mv_resample(VectorOrientation::Column, address, &NoControl).unwrap()));
        let shuffled = prepared.mv_shuffle(address, &NoControl).unwrap();
        assert_eq!(shuffled.len(), 2);
        assert!(shuffled.iter().all(|row| row.len() == 3));
        let mut flattened: Vec<_> = shuffled.iter().flatten().map(|value| value.to_bits()).collect();
        flattened.sort_unstable();
        let mut expected: Vec<_> = rows.iter().flatten().map(|value| value.to_bits()).collect();
        expected.sort_unstable();
        assert_eq!(flattened, expected);
        crossed_source_rows |= shuffled.iter().any(|row| row.iter().any(|value| *value <= 3.0) && row.iter().any(|value| *value >= 4.0));
        assert_eq!(shuffled, prepared.mv_shuffle(address, &NoControl).unwrap());
    }
    assert!(crossed_source_rows, "Whole-array shuffle must not be restricted to intact row permutations");
    assert_eq!(prepared.slurp(1, 3, &NoControl).unwrap(), 6.0);
    assert!(prepared.slurp(0, 0, &NoControl).is_err());
    assert!(prepared.slurp(2, 1, &NoControl).is_err());
    let replay = PreparedValues::new(&[-0.0, 5.0], &ResamplingLimits::default(), &NoControl).unwrap();
    assert_eq!(replay.sip(0, &NoControl).unwrap().to_bits(), (-0.0_f64).to_bits());
    assert!(replay.sip(2, &NoControl).is_err());
}
#[test]
fn normal_likelihood_aic_bic_use_population_mle_and_declared_parameter_count() {
    let values = [-1.0, 0.0, 1.0];
    let report = fit_distribution(&values, &FitFamily::Normal, &FitOptions::default(), &NoControl).expect("normal fit");
    let variance = 2.0 / 3.0;
    let ll = -1.5 * ((std::f64::consts::TAU * variance).ln() + 1.0);
    if let Distribution::Normal { mean, std_dev } = report.distribution {
        assert!(mean.abs() < 1e-15);
        assert!((std_dev * std_dev - variance).abs() < 1e-14);
    } else {
        panic!("wrong fitted family");
    }
    assert!((report.log_likelihood - ll).abs() < 1e-12);
    assert!((report.aic - (4.0 - 2.0 * ll)).abs() < 1e-12);
    assert!((report.bic - (2.0 * 3.0_f64.ln() - 2.0 * ll)).abs() < 1e-12);
    assert_eq!(report.estimated_parameters, 2);
}
#[test]
fn information_criteria_do_not_rank_incompatible_mass_and_density_likelihoods() {
    let values = [0.0, 1.0, 0.0, 1.0, 1.0, 0.0];
    let result = rank_fits(&values, &[FitFamily::Bernoulli, FitFamily::Normal], RankingCriterion::Aic, &FitOptions::default(), &NoControl);
    assert!(
        result.is_err(),
        "Cannot compare discrete PMF and continuous PDF information criteria without an explicit common observation measure: {result:?}"
    );
}
#[test]
fn bootstrap_gof_refits_parameters_and_is_reproducible_with_lossless_seed() {
    let values = [0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0];
    let options = FitOptions { bootstrap_replicates: 7, seed: u64::MAX, ..Default::default() };
    let first = fit_distribution(&values, &FitFamily::Bernoulli, &options, &NoControl).expect("bootstrap");
    let second = fit_distribution(&values, &FitFamily::Bernoulli, &options, &NoControl).expect("repeat");
    let first = first.goodness_of_fit.expect("GOF");
    let second = second.goodness_of_fit.expect("GOF");
    assert!(first.refitted_parameters);
    assert_eq!(first.bootstrap_replicates, 7);
    assert_eq!(first.ks_bootstrap_p, second.ks_bootstrap_p);
    assert_eq!(first.ad_bootstrap_p, second.ad_bootstrap_p);
    for p in [first.ks_bootstrap_p, first.ad_bootstrap_p] {
        assert!((0.125..=1.0).contains(&p));
        assert_eq!((p * 8.0).fract(), 0.0);
    }
}
fn stopping_options() -> AdvancedSimulationOptions {
    AdvancedSimulationOptions {
        base: SimulationOptions { trials: 10_000, seed: 7, ..Default::default() },
        stopping: Some(StoppingRule {
            minimum_trials: 100,
            check_every: 100,
            absolute_half_width: 0.05,
            confidence: 0.95,
            output_bounds: vec![(0.0, 1.0)],
        }),
        ..Default::default()
    }
}
#[test]
fn bounded_stopping_uses_a_verified_mc_checkpoint_and_refuses_incompatible_samplers() {
    let options = stopping_options();
    let result = simulate_advanced(&[Distribution::Constant { value: 0.5 }], &options, &NoControl, |x| Ok(vec![x[0]])).expect("bounded MC");
    assert!(result.stopped_early);
    assert!(result.result.completed_trials >= 100 && result.result.completed_trials.is_multiple_of(100));
    assert!(result.mean_intervals[0].lower <= 0.5 && result.mean_intervals[0].upper >= 0.5);
    assert!((result.mean_intervals[0].upper - result.mean_intervals[0].lower) / 2.0 <= 0.05);
    let mut invalid = stopping_options();
    invalid.base.method = SamplingMethod::LatinHypercube;
    assert!(simulate_advanced(&[Distribution::Constant { value: 0.5 }], &invalid, &NoControl, |x| Ok(vec![x[0]])).is_err());
    invalid = stopping_options();
    invalid.sampler = SamplerSpec::RandomizedSobol { replicate: 0 };
    assert!(simulate_advanced(&[Distribution::Constant { value: 0.5 }], &invalid, &NoControl, |x| Ok(vec![x[0]])).is_err());
    invalid = stopping_options();
    invalid.invalid_trials = InvalidTrialPolicy::Skip { maximum_invalid: 1 };
    assert!(simulate_advanced(&[Distribution::Constant { value: 0.5 }], &invalid, &NoControl, |x| Ok(vec![x[0]])).is_err());
}
#[test]
fn checkpoint_only_intervals_are_not_published_at_data_adaptive_cancellation_times() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Control(AtomicBool);
    impl RunControl for Control {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }
    let mut exposed = Vec::new();
    let log = (4.0_f64 / 0.05).ln();
    // Seed3 is a deterministic replay from a prior64-seed adversarial probe.
    let seed = 3;
    let control = Control(AtomicBool::new(false));
    let mut n = 0_u64;
    let mut successes = 0.0;
    let options = AdvancedSimulationOptions {
        base: SimulationOptions { trials: 100_000, seed, ..Default::default() },
        stopping: Some(StoppingRule {
            minimum_trials: 100_000,
            check_every: 100_000,
            absolute_half_width: 1e-6,
            confidence: 0.95,
            output_bounds: vec![(0.0, 1.0)],
        }),
        ..Default::default()
    };
    let report = simulate_advanced(&[Distribution::Bernoulli { probability: 0.5 }], &options, &control, |x| {
        n += 1;
        successes += x[0];
        let half = (log / (2.0 * n as f64)).sqrt();
        if n >= 50 && (successes / n as f64 - 0.5).abs() > half {
            control.0.store(true, Ordering::Relaxed);
        }
        Ok(vec![x[0]])
    })
    .expect("adaptive cancellation");
    if report.result.termination == Termination::Cancelled && !report.mean_intervals.is_empty() {
        let i = &report.mean_intervals[0];
        if i.method == "simultaneous_bounded_hoeffding" && !(i.lower <= 0.5 && i.upper >= 0.5) {
            exposed.push((seed, report.result.completed_trials, i.lower, i.upper));
        }
    }
    assert!(
        exposed.is_empty(),
        "Checkpoint-only confidence was applied to data-adaptive unconfigured cancellation times; {} replay exposed, first{:?}",
        exposed.len(),
        &exposed[..exposed.len().min(4)]
    );
}
#[test]
fn copula_simulation_matches_analytic_margins_quadrants_and_kendall_tau() {
    let rho = 0.7_f64;
    let ellipse_quadrant = 0.25 + rho.asin() / std::f64::consts::TAU;
    let ellipse_tau = 2.0 * rho.asin() / std::f64::consts::PI;
    let clay = 1.0 / 7.0_f64.sqrt();
    let gumbel = (-2.0_f64.sqrt() * 2.0_f64.ln()).exp();
    let frank = -(((-2.0_f64).exp_m1().powi(2) / (-4.0_f64).exp_m1()).ln_1p()) / 4.0;
    let cases = [
        (DependenceSpec::Gaussian { correlation: vec![vec![1.0, rho], vec![rho, 1.0]] }, ellipse_quadrant, ellipse_tau),
        (DependenceSpec::StudentT { correlation: vec![vec![1.0, rho], vec![rho, 1.0]], degrees_of_freedom: 4.0 }, ellipse_quadrant, ellipse_tau),
        (DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::None }, clay, 0.5),
        (DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::Second }, 0.5 - clay, -0.5),
        (DependenceSpec::Gumbel { theta: 2.0, reflection: Reflection::None }, gumbel, 0.5),
        (DependenceSpec::Frank { theta: 4.0, reflection: Reflection::None }, frank, 0.388_148_021_297_937_85),
    ];
    for (dependence, true_quadrant, true_tau) in cases {
        let options = AdvancedSimulationOptions {
            base: SimulationOptions { trials: 2048, seed: 42, retain_samples: true, ..Default::default() },
            dependence,
            ..Default::default()
        };
        let report = simulate_advanced(
            &[Distribution::Uniform { low: 0.0, high: 1.0 }, Distribution::Uniform { low: 0.0, high: 1.0 }],
            &options,
            &NoControl,
            |x| Ok(x.to_vec()),
        )
        .expect("copula simulation");
        let points = report.result.samples.expect("samples");
        for column in 0..2 {
            let mean = points.iter().map(|p| p[column]).sum::<f64>() / points.len() as f64;
            assert!((mean - 0.5).abs() < 0.03, "nonuniform margin {mean}, {:?}", options.dependence);
        }
        let observed = points.iter().filter(|p| p[0] <= 0.5 && p[1] <= 0.5).count() as f64 / points.len() as f64;
        assert!((observed - true_quadrant).abs() < 0.05, "quadrant{observed}, true{true_quadrant}, {:?}", options.dependence);
        let mut signed = 0_i64;
        for i in 0..points.len() {
            for j in 0..i {
                let product = (points[i][0] - points[j][0]) * (points[i][1] - points[j][1]);
                signed += if product > 0.0 {
                    1
                } else if product < 0.0 {
                    -1
                } else {
                    0
                };
            }
        }
        let tau = signed as f64 / (points.len() * (points.len() - 1) / 2) as f64;
        assert!((tau - true_tau).abs() < 0.07, "tau{tau}, true{true_tau}, {:?}", options.dependence);
    }
}
#[test]
fn lhs_replicate_intervals_reject_reused_seed_even_if_metadata_replicate_differs() {
    use solvercraft_simulation::intervals::{ReplicateDesign, ReplicateEstimate, replicate_mean_interval};
    let base = SimulationOptions { trials: 32, seed: 7, method: SamplingMethod::LatinHypercube, ..Default::default() };
    let run = || {
        solvercraft_simulation::simulate(&[Distribution::Uniform { low: 0.0, high: 1.0 }], &base, &NoControl, |x| Ok(vec![x[0] * x[0]]))
            .expect("real LHS run")
            .summaries[0]
            .mean
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "same seed really reuses the exact LHS randomization");
    let estimates =
        [ReplicateEstimate { seed: 7, replicate: 0, mean: a, trials: 32 }, ReplicateEstimate { seed: 7, replicate: 1, mean: b, trials: 32 }];
    assert!(
        replicate_mean_interval(&estimates, ReplicateDesign::LatinHypercube, 0.95).is_err(),
        "LHS sampler has no replicate nonce; same seed is not an independent randomization"
    );
}
#[test]
fn fixed_sample_intervals_match_independent_student_critical_values() {
    use solvercraft_simulation::intervals::iid_mean_interval;
    let short = iid_mean_interval(&[-1.0, 0.0, 1.0], 0.95).expect("short IID interval");
    assert!((short.upper * 3.0_f64.sqrt() - 4.302_652_729_749_462).abs() < 1e-7);
    assert!(short.fixed_sample_only);
    let values = (0..20_000).map(|i| if i % 2 == 0 { -1.0 } else { 1.0 }).collect::<Vec<_>>();
    let large = iid_mean_interval(&values, 0.95).expect("large df interval");
    assert!((large.upper * (19_999.0_f64).sqrt() - 1.960_082_611_089_815).abs() < 1e-7);
    assert!(large.critical_value_method.contains("cornish_fisher"));
}
