#![allow(clippy::unwrap_used)]
use solvercraft_math::metalog::{Metalog, PreparedMetalog, fit_cdf, fit_spt};
fn close(a: f64, b: f64, tolerance: f64) {
    assert!((a - b).abs() <= tolerance * (1.0 + a.abs()), "{a} != {b}");
}
fn prepared(a: Vec<f64>, lower: Option<f64>, upper: Option<f64>) -> PreparedMetalog {
    PreparedMetalog::new(&Metalog { coefficients: a, lower, upper }).unwrap()
}
#[test]
fn two_term_logistic_matches_independent_quantile_cdf_and_density() {
    let d = prepared(vec![2.0, 3.0], None, None);
    for p in [1e-12_f64, 0.01, 0.2, 0.5, 0.9, 1.0 - 1e-12] {
        let x = 2.0 + 3.0 * (p / (1.0 - p)).ln();
        close(d.quantile(p).unwrap(), x, 1e-13);
        close(d.cdf(x).unwrap(), p, 1e-13);
        close(d.density(x).unwrap(), p * (1.0 - p) / 3.0, 1e-13);
    }
    assert!(d.quantile(0.0).is_err());
    assert!(d.quantile(1.0).is_err());
    assert!(d.certificate().derivative_in_logit_lower_bound > 0.0);
}
#[test]
fn inverse_cdf_removes_large_location_before_comparing_small_tail_changes() {
    let d = prepared(vec![1e16, 1.0], None, None);
    let x = 1e16 + 2.0;
    let p = 1.0 / (1.0 + (-2.0_f64).exp());
    close(d.cdf(x).unwrap(), p, 1e-14);
    close(d.density(x).unwrap(), p * (1.0 - p), 1e-14);
}
#[test]
fn bounded_and_lower_transforms_match_uniform_and_loglogistic_identities() {
    let uniform = prepared(vec![0.0, 1.0], Some(-2.0), Some(8.0));
    for p in [0.01, 0.2, 0.5, 0.9, 0.99] {
        let x = -2.0 + 10.0 * p;
        close(uniform.quantile(p).unwrap(), x, 1e-13);
        close(uniform.cdf(x).unwrap(), p, 1e-13);
        close(uniform.density(x).unwrap(), 0.1, 1e-13);
    }
    assert_eq!(uniform.quantile(0.0).unwrap(), -2.0);
    assert_eq!(uniform.quantile(1.0).unwrap(), 8.0);
    assert_eq!(uniform.cdf(-5.0).unwrap(), 0.0);
    assert_eq!(uniform.cdf(9.0).unwrap(), 1.0);
    let loglogistic = prepared(vec![0.0, 1.0], Some(0.0), None);
    for x in [0.01_f64, 0.2, 1.0, 5.0, 100.0] {
        close(loglogistic.cdf(x).unwrap(), x / (1.0 + x), 1e-13);
        close(loglogistic.density(x).unwrap(), 1.0 / (1.0 + x).powi(2), 1e-13);
    }
}
#[test]
fn original_ten_term_basis_order_has_an_independent_expanded_oracle() {
    let coefficients = vec![1.0, 4.0, 0.1, -0.2, 0.05, -0.04, 0.03, -0.02, 0.01, -0.005];
    let d = prepared(coefficients.clone(), None, None);
    for p in [0.1_f64, 0.23, 0.7, 0.93] {
        let t = p - 0.5;
        let z = (p / (1.0 - p)).ln();
        let expected = coefficients[0]
            + coefficients[1] * z
            + coefficients[2] * t * z
            + coefficients[3] * t
            + coefficients[4] * t * t
            + coefficients[5] * t * t * z
            + coefficients[6] * t * t * t
            + coefficients[7] * t * t * t * z
            + coefficients[8] * t.powi(4)
            + coefficients[9] * t.powi(4) * z;
        close(d.quantile(p).unwrap(), expected, 2e-14);
        close(d.cdf(expected).unwrap(), p, 2e-13);
    }
}
#[test]
fn ols_recovers_independent_coefficients_and_is_labeled_not_mle() {
    let ys: Vec<_> = (1..100).map(|i| f64::from(i) / 100.0).collect();
    let xs: Vec<_> = ys
        .iter()
        .map(|p| {
            let t = p - 0.5;
            let z = (p / (1.0 - p)).ln();
            2.0 + 3.0 * z + 0.2 * t * z + 0.1 * t
        })
        .collect();
    let fit = fit_cdf(4, &xs, &ys, None, None).unwrap();
    for (a, b) in fit.model.coefficients.iter().zip([2.0, 3.0, 0.2, 0.1]) {
        close(*a, b, 2e-12);
    }
    assert!(fit.latent_rmse < 1e-12);
    assert!(fit.method.contains("not maximum likelihood"));
}
#[test]
fn ols_centers_large_offsets_before_estimating_small_scale() {
    let z = [-4.0_f64, -2.0, 0.0, 2.0, 4.0];
    let ys: Vec<_> = z.iter().map(|z| 1.0 / (1.0 + (-z).exp())).collect();
    let xs: Vec<_> = z.iter().map(|z| 1e16 + z).collect();
    let fit = fit_cdf(2, &xs, &ys, None, None).unwrap();
    assert_eq!(fit.model.coefficients[0], 1e16);
    close(fit.model.coefficients[1], 1.0, 1e-12);
    assert!(fit.latent_rmse < 1e-12);
}
#[test]
fn spt_preserves_three_quantiles_in_each_supported_bound_mode() {
    for (lower, upper, qs) in [(None, None, [35.0, 50.0, 75.0]), (Some(0.0), None, [2.0, 4.0, 8.0]), (Some(0.0), Some(100.0), [10.0, 50.0, 90.0])] {
        let fit = fit_spt(lower, upper, qs, 0.1).unwrap();
        let d = PreparedMetalog::new(&fit.model).unwrap();
        for (p, q) in [0.1, 0.5, 0.9].into_iter().zip(qs) {
            close(d.quantile(p).unwrap(), q, 1e-12);
        }
    }
}
#[test]
fn global_certificate_is_required_even_when_center_samples_look_plausible() {
    assert_eq!(
        PreparedMetalog::new(&Metalog { coefficients: vec![0.0, 1.0, 2.0], lower: None, upper: None }).unwrap_err().code,
        "metalog_uncertified"
    );
    assert_eq!(PreparedMetalog::new(&Metalog { coefficients: vec![0.0, 0.0], lower: None, upper: None }).unwrap_err().code, "metalog_uncertified");
    assert!(PreparedMetalog::new(&Metalog { coefficients: vec![0.0, 1.0], lower: None, upper: Some(1.0) }).is_err());
    assert!(fit_cdf(5, &[1.0, 2.0], &[0.1, 0.9], None, None).is_err());
    assert!(fit_spt(None, None, [0.0, 1.0, 2.0], 0.5).is_err());
}
#[test]
fn numerical_endpoint_failures_are_not_silent_atoms() {
    let d = prepared(vec![0.0, 1.0], Some(1.0), Some(1.0_f64.next_up()));
    assert_eq!(d.quantile(0.5).unwrap_err().code, "numerical_range");
    let d = prepared(vec![0.0, 1.0], Some(0.0), None);
    assert_eq!(d.log_density(-1.0).unwrap(), f64::NEG_INFINITY);
    assert!(d.cdf(f64::NAN).is_err());
}
#[test]
fn density_integrates_to_the_corresponding_independent_cdf_mass() {
    let d = prepared(vec![0.0, 1.0, 0.2, 0.1], None, None);
    let low = d.quantile(0.01).unwrap();
    let high = d.quantile(0.99).unwrap();
    let n = 1000;
    let h = (high - low) / f64::from(n);
    let mut area = 0.0;
    for i in 0..=n {
        let x = low + h * f64::from(i);
        let weight = if i == 0 || i == n {
            1.0
        } else if i % 2 == 0 {
            2.0
        } else {
            4.0
        };
        area += weight * d.density(x).unwrap();
    }
    close(area * h / 3.0, 0.98, 1e-9);
}
#[test]
fn moment_fit_is_separate_from_ols_and_checks_the_requested_subfamily() {
    use solvercraft_math::metalog::moment_fit_supported;
    let normal = moment_fit_supported(10.0, 20.0, 0.0, 3.0).unwrap();
    assert_eq!(normal.family, "normal");
    let w = 0.25_f64.exp_m1();
    let skew = (w + 3.0) * w.sqrt();
    let kurt = 3.0 + w * (16.0 + w * (15.0 + w * (6.0 + w)));
    for sign in [-1.0, 1.0] {
        let fit = moment_fit_supported(10.0, 20.0, sign * skew, kurt).unwrap();
        close(fit.matched_skewness, sign * skew, 1e-12);
        close(fit.matched_kurtosis, kurt, 1e-12);
        assert!(fit.scope.contains("not a general"));
    }
    // Source's(10,20,4,41) is exactly a shifted lognormal: v=1 gives
    // skew=v^3+3v=4 and Pearson kurtosis=41.
    let reference = moment_fit_supported(10.0, 20.0, 4.0, 41.0).unwrap();
    close(reference.matched_skewness, 4.0, 1e-12);
    close(reference.matched_kurtosis, 41.0, 1e-12);
    assert_eq!(moment_fit_supported(10.0, 20.0, 4.0, 40.0).unwrap_err().code, "unsupported_moment_family");
    assert!(moment_fit_supported(0.0, 0.0, 0.0, 3.0).is_err());
    assert_eq!(moment_fit_supported(10.0, 1e300, 4.0, 41.0).unwrap_err().code, "numerical_range");
}
