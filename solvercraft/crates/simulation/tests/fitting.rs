#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_math::quantile;
use solvercraft_simulation::fitting::*;
use solvercraft_types::{Distribution as D, NoControl, RunControl};
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "actual {a}, expected {b}");
}
fn opts() -> FitOptions {
    FitOptions { max_evaluations: 2048, ..FitOptions::default() }
}
#[test]
fn closed_form_mles_and_information_criteria_match_direct_algebra() {
    let r = fit_distribution(&[-1.0, 0.0, 1.0], &FitFamily::Normal, &opts(), &NoControl).unwrap();
    let D::Normal { mean, std_dev } = r.distribution else { panic!() };
    close(mean, 0.0, 1e-14);
    close(std_dev, (2.0_f64 / 3.0).sqrt(), 1e-14);
    let ll = -3.0 * std_dev.ln() - 1.5 * std::f64::consts::TAU.ln() - 1.0 / (std_dev * std_dev);
    close(r.log_likelihood, ll, 1e-13);
    close(r.aic, 4.0 - 2.0 * ll, 1e-13);
    close(r.bic, 2.0 * 3.0_f64.ln() - 2.0 * ll, 1e-13);
    let r = fit_distribution(&[1.0, 0.0, 1.0, 1.0], &FitFamily::Bernoulli, &opts(), &NoControl).unwrap();
    assert!(matches!(r.distribution, D::Bernoulli { probability: 0.75 }));
    close(r.log_likelihood, 3.0 * 0.75_f64.ln() + 0.25_f64.ln(), 1e-13);
    let r = fit_distribution(&[1.0, 2.0, 3.0], &FitFamily::InverseGaussian, &opts(), &NoControl).unwrap();
    let D::InverseGaussian { mean, shape } = r.distribution else { panic!() };
    close(mean, 2.0, 1e-14);
    close(shape, 9.0, 1e-12);
}
#[test]
fn genuine_bounded_numerical_fit_recovers_quantile_grid_parameters() {
    for (truth, family) in [
        (D::Gamma { shape: 2.5, scale: 3.0 }, FitFamily::Gamma),
        (D::Beta { alpha: 2.0, beta: 5.0 }, FitFamily::Beta),
        (D::Weibull { shape: 2.5, scale: 3.0 }, FitFamily::Weibull),
    ] {
        let values = (0..400).map(|i| quantile(&truth, (f64::from(i) + 0.5) / 400.0).unwrap()).collect::<Vec<_>>();
        let r = fit_distribution(&values, &family, &opts(), &NoControl).unwrap();
        assert_eq!(r.method, "bounded_local_likelihood");
        assert!(r.evaluations > 1);
        assert!(r.converged);
        for p in [0.1, 0.5, 0.9] {
            close(quantile(&r.distribution, p).unwrap(), quantile(&truth, p).unwrap(), 0.08);
        }
    }
    let values = (0..100).map(|i| quantile(&D::Normal { mean: 2.0, std_dev: 1.0 }, (f64::from(i) + 0.5) / 100.0).unwrap()).collect::<Vec<_>>();
    let family = FitFamily::Parametric {
        initial: D::SkewNormal { location: 0.0, scale: 1.0, shape: 0.0 },
        parameters: vec![FitParameter { field: ParameterField::Location, lower: -5.0, upper: 5.0 }],
    };
    let r = fit_distribution(&values, &family, &opts(), &NoControl).unwrap();
    let D::SkewNormal { location, .. } = r.distribution else { panic!() };
    close(location, 2.0, 1e-4);
}
#[test]
fn bootstrap_refits_and_detects_known_misspecification_repeatably() {
    let values = [vec![-5.0; 32], vec![5.0; 32]].concat();
    let o = FitOptions { max_evaluations: 1, bootstrap_replicates: 39, seed: 17, ..opts() };
    let r = fit_distribution(&values, &FitFamily::Normal, &o, &NoControl).unwrap();
    let a = r.goodness_of_fit.unwrap();
    assert!(a.refitted_parameters);
    assert!(a.ks_bootstrap_p <= 0.05);
    assert!(a.ad_bootstrap_p <= 0.05);
    assert_eq!(a.bootstrap_replicates, 39);
    let b = fit_distribution(&values, &FitFamily::Normal, &o, &NoControl).unwrap().goodness_of_fit.unwrap();
    assert_eq!(a.ks_bootstrap_p, b.ks_bootstrap_p);
    assert_eq!(a.clipped_ad_statistic, b.clipped_ad_statistic);
    let counts = (0..64).map(|i| f64::from(i % 2)).collect::<Vec<_>>();
    let discrete = fit_distribution(&counts, &FitFamily::Bernoulli, &o, &NoControl).unwrap().goodness_of_fit.unwrap();
    assert!(discrete.method.contains("randomized_pit"));
    assert!(discrete.ks_bootstrap_p > 0.05);
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn ranking_reports_failures_and_resources_do_not_become_scores() {
    let values = [-2.0, -1.0, 0.0, 1.0, 2.0];
    let r = rank_fits(&values, &[FitFamily::Exponential, FitFamily::Normal, FitFamily::Uniform], RankingCriterion::Bic, &opts(), &NoControl).unwrap();
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].index, 0);
    assert_eq!(r.fits.len(), 2);
    assert!(r.fits[0].1.bic <= r.fits[1].1.bic);
    assert!(fit_distribution(&[1.0, 1.0], &FitFamily::Normal, &opts(), &NoControl).is_err());
    assert!(fit_distribution(&values, &FitFamily::Normal, &opts(), &Cancel).is_err());
    let mut limited = opts();
    limited.max_work = 1;
    assert!(fit_distribution(&values, &FitFamily::Normal, &limited, &NoControl).is_err());
    limited = opts();
    limited.max_memory_bytes = 1;
    assert!(fit_distribution(&values, &FitFamily::Normal, &limited, &NoControl).is_err());
}
