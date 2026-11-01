#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_simulation::{
    advanced::{AdvancedSimulationOptions, DependenceSpec, InvalidTrialPolicy, Reflection, SamplerSpec, StoppingRule, simulate_advanced},
    simulate,
};
use solvercraft_types::{Distribution as D, NoControl, RunControl, SimulationOptions, SolverError, Termination};
fn options() -> AdvancedSimulationOptions {
    AdvancedSimulationOptions {
        base: SimulationOptions { trials: 256, retain_samples: true, ..SimulationOptions::default() },
        ..AdvancedSimulationOptions::default()
    }
}
#[test]
fn advanced_legacy_preserves_v2_and_sobol_is_addressed() {
    let distributions = [D::Normal { mean: 2.0, std_dev: 3.0 }, D::Uniform { low: -1.0, high: 1.0 }];
    let mut o = options();
    let old = simulate(&distributions, &o.base, &NoControl, |x| Ok(x.to_vec())).unwrap();
    let new = simulate_advanced(&distributions, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    assert_eq!(old.samples, new.result.samples);
    o.sampler = SamplerSpec::RandomizedSobol { replicate: 4 };
    let a = simulate_advanced(&[D::Uniform { low: 0.0, high: 1.0 }], &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    let b = simulate_advanced(&[D::Uniform { low: 0.0, high: 1.0 }], &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    assert_eq!(a.result.samples, b.result.samples);
    let mut bins = [false; 256];
    for row in a.result.samples.unwrap() {
        let index = (row[0] * 256.0).floor() as usize;
        assert!(!bins[index]);
        bins[index] = true;
    }
}
#[test]
fn skipped_invalid_attempts_are_reported_without_readdressing() {
    let mut o = options();
    o.base.trials = 10;
    o.invalid_trials = InvalidTrialPolicy::Skip { maximum_invalid: 5 };
    let mut call = 0;
    let r = simulate_advanced(&[D::Uniform { low: 0.0, high: 1.0 }], &o, &NoControl, |x| {
        call += 1;
        if call % 2 == 0 { Err(SolverError::new("synthetic", "failure")) } else { Ok(x.to_vec()) }
    })
    .unwrap();
    assert_eq!(r.attempted_trials, 10);
    assert_eq!(r.invalid_trials, 5);
    assert_eq!(r.result.completed_trials, 5);
    assert_eq!(r.first_invalid_trials[0].trial, 1);
    let full = simulate(&[D::Uniform { low: 0.0, high: 1.0 }], &o.base, &NoControl, |x| Ok(x.to_vec())).unwrap();
    assert_eq!(r.result.samples.unwrap(), full.samples.unwrap().into_iter().step_by(2).collect::<Vec<_>>());
    assert!(simulate_advanced(&[], &o, &NoControl, |_| Ok(vec![f64::NAN])).is_err());
}
#[test]
fn gaussian_spearman_and_archimedean_have_expected_dependence() {
    let mut o = options();
    o.base.trials = 4096;
    o.dependence = DependenceSpec::GaussianSpearman { rank_correlation: vec![vec![1.0, 0.6], vec![0.6, 1.0]] };
    let d = [D::Uniform { low: 0.0, high: 1.0 }, D::Uniform { low: 0.0, high: 1.0 }];
    let r = simulate_advanced(&d, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    let rows = r.result.samples.unwrap();
    let mut covariance = 0.0;
    for row in &rows {
        covariance += (row[0] - 0.5) * (row[1] - 0.5);
    }
    assert!((12.0 * covariance / rows.len() as f64 - 0.6).abs() < 0.06);
    o.base.trials = 512;
    for spec in
        [DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::None }, DependenceSpec::Gumbel { theta: 2.0, reflection: Reflection::None }]
    {
        o.dependence = spec;
        let r = simulate_advanced(&d, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
        let rows = r.result.samples.unwrap();
        let mut concordance = 0.0;
        for (i, a) in rows.iter().enumerate() {
            for b in rows.iter().skip(i + 1) {
                concordance += ((a[0] - b[0]) * (a[1] - b[1])).signum();
            }
        }
        let tau = concordance / (rows.len() * (rows.len() - 1) / 2) as f64;
        assert!((tau - 0.5).abs() < 0.09, "tau={tau}");
    }
}
#[test]
fn bounded_mc_stopping_is_simultaneous_and_validates_claims() {
    let mut o = options();
    o.base.trials = 10000;
    o.stopping =
        Some(StoppingRule { minimum_trials: 100, check_every: 100, absolute_half_width: 0.1, confidence: 0.95, output_bounds: vec![(0.0, 1.0)] });
    let r = simulate_advanced(&[D::Bernoulli { probability: 0.4 }], &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    assert!(r.stopped_early);
    assert!(r.result.completed_trials >= 100);
    assert!(r.mean_intervals[0].lower < 0.4 && r.mean_intervals[0].upper > 0.4);
    assert!(simulate_advanced(&[], &o, &NoControl, |_| Ok(vec![2.0])).is_err());
    o.sampler = SamplerSpec::RandomizedSobol { replicate: 0 };
    assert!(simulate_advanced(&[D::Uniform { low: 0.0, high: 1.0 }], &o, &NoControl, |x| Ok(x.to_vec())).is_err());
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn budgets_cancellation_and_arity_do_not_produce_claims() {
    let o = options();
    let r = simulate_advanced(&[], &o, &Cancel, |_| panic!("must not evaluate")).unwrap();
    assert_eq!(r.result.termination, Termination::Cancelled);
    assert_eq!(r.attempted_trials, 0);
    assert!(r.result.summaries.is_empty());
    let mut tiny = o.clone();
    tiny.base.max_memory_bytes = 1024;
    assert!(simulate_advanced(&[], &tiny, &NoControl, |_| Ok(vec![0.0])).is_err());
    let mut call = 0;
    assert!(
        simulate_advanced(&[], &o, &NoControl, |_| {
            call += 1;
            Ok(vec![0.0; call])
        })
        .is_err()
    );
}

#[test]
fn multivariate_archimedean_frailties_match_pair_cdfs_and_margins() {
    let mut o = options();
    o.base.trials = 4096;
    let d = [D::Uniform { low: 0.0, high: 1.0 }, D::Uniform { low: 0.0, high: 1.0 }, D::Uniform { low: 0.0, high: 1.0 }];
    let frank = -0.5 * (1.0 + ((-1.0_f64).exp() - 1.0).powi(2) / ((-2.0_f64).exp() - 1.0)).ln();
    for (spec, expected) in [
        (DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::None }, 1.0 / 7.0_f64.sqrt()),
        (DependenceSpec::Gumbel { theta: 2.0, reflection: Reflection::None }, (-2.0_f64.sqrt() * 2.0_f64.ln()).exp()),
        (DependenceSpec::Frank { theta: 2.0, reflection: Reflection::None }, frank),
    ] {
        o.dependence = spec;
        let r = simulate_advanced(&d, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
        for summary in &r.result.summaries {
            assert!((summary.mean - 0.5).abs() < 0.025, "mean{}", summary.mean);
        }
        let rows = r.result.samples.unwrap();
        let joint = rows.iter().filter(|r| r[0] <= 0.5 && r[1] <= 0.5).count() as f64 / rows.len() as f64;
        assert!((joint - expected).abs() < 0.025, "joint{joint}, expected{expected}");
    }
}
#[test]
fn negative_clayton_countermonotonic_boundary_and_student_singular_limit() {
    let mut o = options();
    o.dependence = DependenceSpec::Clayton { theta: -1.0, reflection: Reflection::None };
    let d = [D::Uniform { low: 0.0, high: 1.0 }, D::Uniform { low: 0.0, high: 1.0 }];
    let r = simulate_advanced(&d, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    for row in r.result.samples.unwrap() {
        assert!((row[0] + row[1] - 1.0).abs() < 1e-14);
    }
    o.dependence = DependenceSpec::StudentT { correlation: vec![vec![1.0, 1.0], vec![1.0, 1.0]], degrees_of_freedom: 4.0 };
    let r = simulate_advanced(&d, &o, &NoControl, |x| Ok(x.to_vec())).unwrap();
    for row in r.result.samples.unwrap() {
        assert_eq!(row[0], row[1]);
    }
}
