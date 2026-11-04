#![allow(clippy::unwrap_used)]
use solvercraft_math::{
    johnson_moments::{JohnsonFamily, JohnsonOptions, fit, fit_with_options, population_moments},
    quantile,
};
use solvercraft_types::{Distribution as D, NoControl, RunControl};
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t * (1.0 + b.abs()), "actual{a}, expected{b}");
}
// SU: SciPy1.18.1stats. SB: independent SciPy.integrate.quad of the
// Gaussian translation (generic scipy.stats SB moments were less accurate).
// Last column is EXCESS kurtosis; API under test uses Pearson(+3).
const FIXTURES: &[(&str, f64, f64, f64, f64, f64, f64)] = &[
    ("su", -0.8, 1.2, 5.044619863159599, 22.84053837456438, 2.7464142065000887, 23.128611065772063),
    ("su", 1.4, 0.8, -16.284475964005523, 1358.5588178467456, -12.919176856868395, 776.9631604773264),
    ("su", 0.0, 2.0, 2.0, 2.9192457181505764, 0.0, 1.5078621849296505),
    ("su", -2.0, 2.5, 4.886221175685858, 3.142467806946422, 0.9097575584558765, 1.9703368908476788),
    ("sb", 0.0, 1.0, 3.5, 0.39041132272283674, 0.0, -0.8606196018182803),
    ("sb", 1.0, 1.0, 2.9097959895689502, 0.30016880458731604, 0.7266702876645502, -0.09222089088243912),
    ("sb", -2.0, 0.7, 4.692152385088546, 0.14608196763132852, -2.3706844917274017, 6.665400678117397),
    ("sb", 3.0, 2.0, 2.581072161731277, 0.052895602198887615, 0.8287190939132436, 0.8531565134289449),
    ("sb", 0.5, 0.3, 2.987749237520561, 1.1247110715121968, 0.7240758707828397, -1.0414696599850004),
];
fn distribution(name: &str, gamma: f64, delta: f64) -> D {
    if name == "su" { D::JohnsonSu { gamma, delta, location: 2.0, scale: 3.0 } } else { D::JohnsonSb { gamma, delta, location: 2.0, scale: 3.0 } }
}
#[test]
fn deterministic_population_moments_match_independent_scipy_fixtures() {
    for &(name, gamma, delta, mean, var, skew, excess) in FIXTURES {
        let actual = population_moments(&distribution(name, gamma, delta)).unwrap();
        close(actual.mean, mean, 2e-8);
        close(actual.std_dev, var.sqrt(), 2e-8);
        close(actual.skewness, skew, 2e-8);
        close(actual.kurtosis, excess + 3.0, 2e-8);
    }
}
#[test]
fn full_su_inverse_matches_population_moments_and_independent_source_quantiles() {
    for &(name, gamma, delta, mean, var, skew, excess) in FIXTURES.iter().filter(|f| f.0 == "su") {
        let fitted = fit(mean, var.sqrt(), skew, excess + 3.0).unwrap();
        assert_eq!(fitted.family, JohnsonFamily::Su);
        assert!(fitted.max_relative_residual <= 1e-6);
        let original = distribution(name, gamma, delta);
        for p in [0.05, 0.5, 0.95] {
            close(quantile(&fitted.distribution, p).unwrap(), quantile(&original, p).unwrap(), 2e-6);
        }
    }
}
#[test]
fn full_sb_inverse_matches_population_moments_and_original_quantiles() {
    for &(name, gamma, delta, mean, var, skew, excess) in FIXTURES.iter().filter(|f| f.0 == "sb") {
        let fitted = fit(mean, var.sqrt(), skew, excess + 3.0).unwrap();
        assert_eq!(fitted.family, JohnsonFamily::Sb);
        assert!(fitted.max_relative_residual <= 1e-6);
        let original = distribution(name, gamma, delta);
        for p in [0.05, 0.5, 0.95] {
            close(quantile(&fitted.distribution, p).unwrap(), quantile(&original, p).unwrap(), 2e-6);
        }
    }
}
#[test]
fn normal_lognormal_boundary_and_invalid_moments_are_distinct() {
    assert_eq!(fit(10.0, 2.0, 0.0, 3.0).unwrap().family, JohnsonFamily::Normal);
    let a = fit(10.0, 20.0, 4.0, 41.0).unwrap();
    assert_eq!(a.family, JohnsonFamily::Lognormal);
    close(a.achieved.skewness, 4.0, 1e-10);
    close(a.achieved.kurtosis, 41.0, 1e-10);
    for (skew, kurt) in [(0.0, 1.0), (2.0, 5.0), (2.0, 4.0)] {
        assert_eq!(fit(0.0, 1.0, skew, kurt).unwrap_err().code, "infeasible_moments");
    }
    assert!(fit(f64::NAN, 1.0, 0.0, 3.0).is_err());
    assert!(fit(0.0, 0.0, 0.0, 3.0).is_err());
}
struct Cancelled;
impl RunControl for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn evaluation_and_cancel_budgets_do_not_manufacture_fits() {
    let o = JohnsonOptions { max_shape_evaluations: 0, ..JohnsonOptions::default() };
    assert_eq!(fit_with_options(0.0, 1.0, 0.0, 4.0, &o, &NoControl).unwrap_err().code, "evaluation_limit");
    assert_eq!(fit_with_options(0.0, 1.0, 0.0, 4.0, &JohnsonOptions::default(), &Cancelled).unwrap_err().code, "cancelled");
    assert!(fit(10.0, 1e300, 4.0, 41.0).is_err());
}
