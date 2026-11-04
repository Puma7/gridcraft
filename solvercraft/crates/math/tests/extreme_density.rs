#![allow(clippy::unwrap_used)]
use solvercraft_math::{cdf, density::log_density, quantile};
use solvercraft_types::Distribution as D;
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "actual{a}, expected{b}");
}
#[test]
fn stable_log_densities_keep_finite_large_logarithms() {
    let t = D::StudentT { degrees_of_freedom: 1.0, location: 0.0, scale: 1.0 };
    let c = D::Cauchy { location: 0.0, scale: 1.0 };
    close(log_density(&t, 1e200).unwrap(), -922.1787670834677, 1e-10);
    close(log_density(&c, 1e200).unwrap(), -922.1787670834677, 1e-10);
    let scale = 1e308;
    close(log_density(&D::Laplace { location: 0.0, scale }, 0.0).unwrap(), -2.0_f64.ln() - scale.ln(), 1e-12);
    assert_eq!(log_density(&D::Logistic { location: 0.0, scale: 1.0 }, -1e308).unwrap(), -1e308);
    close(log_density(&D::Normal { mean: -1e308, std_dev: 1e308 }, 1e308).unwrap(), -2.0 - 0.5 * std::f64::consts::TAU.ln() - 1e308_f64.ln(), 1e-12);
    let f = D::F { numerator_df: 2.0, denominator_df: 2.0 };
    close(log_density(&f, 1e308).unwrap(), -2.0 * 1e308_f64.ln(), 1e-10);
    let johnson = D::JohnsonSu { gamma: 0.0, delta: 1.0, location: 0.0, scale: 1e-308 };
    assert!(log_density(&johnson, 1e308).unwrap().is_finite());
}
#[test]
fn reciprocal_narrow_large_bounds_do_not_round_into_outside_support() {
    let low = 1e308_f64;
    let high = low.next_up();
    let d = D::Reciprocal { low, high };
    for p in [0.0, 0.1, 0.5, 0.9, 1.0] {
        let q = quantile(&d, p).unwrap();
        assert!(q >= low && q <= high);
    }
    assert_eq!(cdf(&d, low).unwrap(), 0.0);
    assert_eq!(cdf(&d, high).unwrap(), 1.0);
    assert!(log_density(&d, low).unwrap().is_finite());
    let wide = D::Reciprocal { low: 1e-300, high: 1e300 };
    close(quantile(&wide, 0.5).unwrap(), 1.0, 1e-12);
    assert_eq!(quantile(&wide, 0.0).unwrap(), 1e-300);
    assert_eq!(quantile(&wide, 1.0).unwrap(), 1e300);
    assert_eq!(quantile(&D::BetaPert { low: -1e308, mode: 0.0, high: 1.0, weight: 4.0 }, 1.0).unwrap(), 1.0);
}
#[test]
fn beta_prime_and_f_invert_complementary_tails_before_rounding_unit_coordinates() {
    let value = 2.943586860630712e16;
    let bp = D::BetaPrime { alpha: 10.0, beta: 0.2, scale: 2.0 };
    let q = quantile(&bp, 0.999).unwrap();
    assert!((q / value - 1.0).abs() < 1e-10);
    close(cdf(&bp, value).unwrap(), 0.999, 1e-12);
    let f = D::F { numerator_df: 20.0, denominator_df: 0.4 };
    let q = quantile(&f, 0.999).unwrap();
    assert!((q / (value * 0.01) - 1.0).abs() < 1e-10);
    close(cdf(&f, value * 0.01).unwrap(), 0.999, 1e-12);
}
#[test]
fn heavy_student_t_tails_are_inverted_before_upper_cdf_rounding() {
    let d = D::StudentT { degrees_of_freedom: 0.1, location: 1.0, scale: 2.0 };
    let q = quantile(&d, 0.9999999999).unwrap();
    assert!((q / 3.2088487563187397e96 - 1.0).abs() < 2e-9);
    let base = D::StudentT { degrees_of_freedom: 0.1, location: 0.0, scale: 1.0 };
    let low = cdf(&base, -1e200).unwrap();
    assert!(low > 0.0 && low < 1e-19);
    let q = quantile(&base, 1.0_f64.next_down()).unwrap();
    assert!(q > 1e154 && q.is_finite());
}
