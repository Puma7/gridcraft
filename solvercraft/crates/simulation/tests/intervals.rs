#![allow(clippy::unwrap_used)]
use solvercraft_simulation::intervals::*;
#[test]
fn large_mc_sample_uses_an_explicit_large_df_critical_approximation() {
    let values: Vec<f64> = (0..10002).map(|i| if i % 2 == 0 { -1.0 } else { 1.0 }).collect();
    let interval = iid_mean_interval(&values, 0.95).unwrap();
    let critical = interval.upper / interval.standard_error;
    assert!(critical > 1.9601 && critical < 1.9603);
    assert!(interval.critical_value_method.contains("cornish_fisher"));
    assert_eq!(interval.independent_units, 10002);
}
#[test]
fn student_t_mean_interval_matches_an_independent_reference() {
    let i = iid_mean_interval(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.95).unwrap();
    let critical = 2.7764451051977987;
    let half = critical * (2.5_f64 / 5.0).sqrt();
    assert!((i.mean - 3.0).abs() < 1e-14);
    assert!((i.lower - (3.0 - half)).abs() < 1e-9);
    assert!((i.upper - (3.0 + half)).abs() < 1e-9);
    assert_eq!(i.independent_units, 5);
    assert!(i.fixed_sample_only);
}
#[test]
fn qmc_and_lhs_intervals_use_replicate_means_not_pointwise_variance() {
    let estimates: Vec<_> = [0.4, 0.5, 0.6]
        .into_iter()
        .enumerate()
        .map(|(j, mean)| ReplicateEstimate { seed: u64::MAX, replicate: j as u64, mean, trials: 1024 })
        .collect();
    let qmc = replicate_mean_interval(&estimates, ReplicateDesign::RandomizedSobol, 0.95).unwrap();
    let lhs_estimates: Vec<_> = estimates.iter().enumerate().map(|(i, e)| ReplicateEstimate { seed: i as u64, replicate: 0, ..e.clone() }).collect();
    let lhs = replicate_mean_interval(&lhs_estimates, ReplicateDesign::LatinHypercube, 0.95).unwrap();
    assert_eq!(qmc.independent_units, 3);
    assert_eq!(qmc.standard_error, lhs.standard_error);
    assert!((qmc.standard_error - (0.01_f64 / 3.0).sqrt()).abs() < 1e-14);
    assert!(qmc.assumptions.contains("approximation"));
    let mut repeated = estimates.clone();
    repeated[1].replicate = 0;
    assert!(replicate_mean_interval(&repeated, ReplicateDesign::RandomizedSobol, 0.95).is_err());
    let mut unequal = lhs_estimates;
    unequal[1].trials = 2;
    assert!(replicate_mean_interval(&unequal, ReplicateDesign::LatinHypercube, 0.95).is_err());
}
#[test]
fn interval_degeneracy_and_invalid_inputs_are_explicit() {
    let c = iid_mean_interval(&[7.0; 4], 0.95).unwrap();
    assert_eq!(c.lower, 7.0);
    assert_eq!(c.upper, 7.0);
    for values in [&[][..], &[1.0][..], &[1.0, f64::NAN][..]] {
        assert!(iid_mean_interval(values, 0.95).is_err());
    }
    assert!(iid_mean_interval(&[1.0, 2.0], 1.0).is_err());
    assert!(iid_mean_interval(&[1.0, 2.0], f64::NAN).is_err());
}
