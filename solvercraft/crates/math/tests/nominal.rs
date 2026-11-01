#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_math::{
    nominal::{NominalStatistic, nominal_summary, nominal_value},
    parameterization::lognormal_from_mean_std,
};
use solvercraft_types::Distribution as D;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1.0 + b.abs()), "actual {a}, expected {b}");
}
#[test]
fn baseline17_means_have_the_same_canonical_spreadsheet_semantics() {
    for (d, expected) in [
        (D::Uniform { low: 1.0, high: 5.0 }, 3.0),
        (D::DiscreteUniform { low: 1, high: 5 }, 3.0),
        (D::Normal { mean: 3.0, std_dev: 7.0 }, 3.0),
        (D::Lognormal { log_mean: 0.0, log_std_dev: 1.0 }, 0.5_f64.exp()),
        (D::Triangular { low: 0.0, mode: 1.0, high: 5.0 }, 2.0),
        (D::BetaPert { low: 0.0, mode: 1.0, high: 8.0, weight: 4.0 }, 2.0),
        (D::Beta { alpha: 2.0, beta: 3.0 }, 0.4),
        (D::Gamma { shape: 2.0, scale: 3.0 }, 6.0),
        (D::Exponential { rate: 0.5 }, 2.0),
        (D::Weibull { shape: 2.0, scale: 2.0 }, std::f64::consts::PI.sqrt()),
        (D::Bernoulli { probability: 0.2 }, 0.2),
        (D::Binomial { trials: 10, probability: 0.2 }, 2.0),
        (D::Poisson { rate: 3.0 }, 3.0),
        (D::NegativeBinomial { successes: 2, probability: 0.5 }, 2.0),
        (D::Geometric { probability: 0.25 }, 3.0),
        (D::Empirical { values: vec![0.0, 0.0, 9.0] }, 3.0),
        (D::EmpiricalContinuous { values: vec![0.0, 0.0, 8.0] }, 2.0),
    ] {
        let n = nominal_summary(&d).unwrap();
        assert_eq!(n.statistic, NominalStatistic::Mean);
        close(n.value, expected);
    }
}
#[test]
fn median_fallback_is_visible_and_large_offset_averages_remain_finite() {
    let c = nominal_summary(&D::Cauchy { location: 7.0, scale: 3.0 }).unwrap();
    assert_eq!(c.statistic, NominalStatistic::MedianFallback);
    assert_eq!(c.value, 7.0);
    assert_eq!(nominal_value(&D::Uniform { low: 1e308, high: 1e308 }).unwrap(), 1e308);
    close(nominal_value(&D::Empirical { values: vec![1e17, -1.0, -1e17] }).unwrap(), -1.0 / 3.0);
    assert_eq!(nominal_value(&D::Empirical { values: vec![f64::from_bits(1); 3] }).unwrap(), f64::from_bits(1));
    assert!(nominal_value(&D::Gamma { shape: 100.0, scale: 1e308 }).is_err());
}
#[test]
fn arithmetic_lognormal_conversion_preserves_mean_and_extreme_finite_inputs() {
    for (mean, sd) in [(3.0, 2.0), (1e-300, 1e300), (1e300, 1e150), (7.0, 0.0)] {
        let d = lognormal_from_mean_std(mean, sd).unwrap();
        let D::Lognormal { log_mean, log_std_dev } = d else { panic!() };
        assert!(log_mean.is_finite() && log_std_dev.is_finite());
        let reconstructed = (log_mean + 0.5 * log_std_dev * log_std_dev).exp();
        assert!((reconstructed / mean - 1.0).abs() < 1e-11);
    }
    assert!(lognormal_from_mean_std(1e300, 1e-300).is_err());
    assert!(lognormal_from_mean_std(0.0, 1.0).is_err());
    assert!(lognormal_from_mean_std(1.0, -1.0).is_err());
}
