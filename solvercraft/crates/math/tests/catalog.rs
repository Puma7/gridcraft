#![allow(clippy::unwrap_used)]
use solvercraft_math::{PreparedDistribution, cdf, quantile, sample, validate_distribution};
use solvercraft_types::Distribution as D;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "actual {a:?}, expected {b:?}, tolerance {tol}");
}

#[test]
fn baseline_added_families_match_independent_closed_forms() {
    for p in [0.001_f64, 0.1, 0.5, 0.9, 0.999] {
        close(quantile(&D::Beta { alpha: 1.0, beta: 1.0 }, p).unwrap(), p, 2e-12);
        close(quantile(&D::Gamma { shape: 1.0, scale: 3.0 }, p).unwrap(), -3.0 * (-p).ln_1p(), 2e-10);
        close(
            quantile(&D::BetaPert { low: -2.0, mode: 0.0, high: 2.0, weight: 4.0 }, p).unwrap(),
            -quantile(&D::BetaPert { low: -2.0, mode: 0.0, high: 2.0, weight: 4.0 }, 1.0 - p).unwrap(),
            1e-10,
        );
        close(quantile(&D::EmpiricalContinuous { values: vec![7.0, -3.0] }, p).unwrap(), -3.0 + 10.0 * p, 1e-13);
        let nb = quantile(&D::NegativeBinomial { successes: 1, probability: 0.25 }, p).unwrap();
        let geom = quantile(&D::Geometric { probability: 0.25 }, p).unwrap();
        assert_eq!(nb, geom);
    }
    for x in [0.0_f64, 0.1, 0.5, 0.9, 1.0] {
        close(cdf(&D::Beta { alpha: 2.0, beta: 3.0 }, x).unwrap(), 6.0 * x * x - 8.0 * x.powi(3) + 3.0 * x.powi(4), 2e-13);
        close(cdf(&D::Gamma { shape: 2.0, scale: 1.0 }, x).unwrap(), 1.0 - (-x).exp() * (1.0 + x), 2e-13);
    }
    let d = D::DiscreteUniform { low: -2, high: 2 };
    for (p, q) in [(0.0, -2.0), (0.2, -2.0), (0.2000001, -1.0), (1.0, 2.0)] {
        assert_eq!(quantile(&d, p).unwrap(), q);
    }
    close(cdf(&d, -1.1).unwrap(), 0.2, 1e-14);
}

#[test]
fn extended_families_match_independent_identities() {
    for p in [0.01_f64, 0.1, 0.5, 0.9, 0.99] {
        close(
            quantile(&D::StudentT { degrees_of_freedom: 1.0, location: 2.0, scale: 3.0 }, p).unwrap(),
            2.0 + 3.0 * (std::f64::consts::PI * (p - 0.5)).tan(),
            2e-9,
        );
        close(quantile(&D::ChiSquared { degrees_of_freedom: 2.0 }, p).unwrap(), -2.0 * (-p).ln_1p(), 2e-10);
        close(quantile(&D::F { numerator_df: 2.0, denominator_df: 2.0 }, p).unwrap(), p / (1.0 - p), 1e-8);
        close(quantile(&D::Erlang { shape: 1, scale: 2.0 }, p).unwrap(), -2.0 * (-p).ln_1p(), 2e-10);
        close(
            quantile(&D::SkewNormal { location: 0.0, scale: 1.0, shape: 0.0 }, p).unwrap(),
            quantile(&D::Normal { mean: 0.0, std_dev: 1.0 }, p).unwrap(),
            4e-6,
        );
        let logistic = D::Logistic { location: 2.0, scale: 3.0 };
        close(quantile(&logistic, p).unwrap(), 2.0 + 3.0 * (p / (1.0 - p)).ln(), 1e-12);
    }
    let hyper = D::Hypergeometric { population: 5, successes: 2, draws: 2 };
    close(cdf(&hyper, 0.0).unwrap(), 0.3, 1e-13);
    close(cdf(&hyper, 1.0).unwrap(), 0.9, 1e-13);
    close(cdf(&D::SkewNormal { location: 0.0, scale: 1.0, shape: 2.0 }, 0.0).unwrap(), 0.5 - 2.0_f64.atan() / std::f64::consts::PI, 1e-13);
}

#[test]
fn every_catalog_family_roundtrips_and_samples_reproducibly() {
    let families = vec![
        D::Beta { alpha: 2.0, beta: 5.0 },
        D::BetaPert { low: -5.0, mode: 0.0, high: 3.0, weight: 4.0 },
        D::Gamma { shape: 2.5, scale: 3.0 },
        D::StudentT { degrees_of_freedom: 4.0, location: 1.0, scale: 2.0 },
        D::ChiSquared { degrees_of_freedom: 3.0 },
        D::F { numerator_df: 3.0, denominator_df: 5.0 },
        D::Cauchy { location: 1.0, scale: 2.0 },
        D::Logistic { location: 1.0, scale: 2.0 },
        D::Laplace { location: 1.0, scale: 2.0 },
        D::Gumbel { location: 1.0, scale: 2.0 },
        D::Pareto { minimum: 1.0, shape: 3.0 },
        D::Rayleigh { scale: 2.0 },
        D::Erlang { shape: 4, scale: 2.0 },
        D::InverseGamma { shape: 3.0, scale: 2.0 },
        D::InverseGaussian { mean: 2.0, shape: 3.0 },
        D::LogLogistic { shape: 2.0, scale: 3.0 },
        D::SkewNormal { location: 0.0, scale: 2.0, shape: 3.0 },
        D::JohnsonSu { gamma: 0.3, delta: 1.2, location: 2.0, scale: 3.0 },
        D::JohnsonSb { gamma: 0.3, delta: 1.2, location: 2.0, scale: 3.0 },
        D::JohnsonSl { gamma: 0.3, delta: 1.2, location: 2.0, scale: 3.0 },
    ];
    for d in families {
        let prepared = PreparedDistribution::new(&d).unwrap();
        for p in [0.001, 0.05, 0.5, 0.95, 0.999] {
            let q = prepared.quantile(p).unwrap();
            close(prepared.cdf(q).unwrap(), p, 3e-6);
        }
        assert_eq!(sample(&d, 42, 7, 9).unwrap(), sample(&d, 42, 7, 9).unwrap());
    }
}

#[test]
fn modifiers_preserve_discrete_atoms_and_compound_identity() {
    let shifted = D::ShiftScale { base: Box::new(D::Bernoulli { probability: 0.25 }), shift: 4.0, scale: -2.0 };
    close(cdf(&shifted, 2.0).unwrap(), 0.25, 1e-15);
    close(cdf(&shifted, 3.0).unwrap(), 0.25, 1e-15);
    let truncated = D::Truncated { base: Box::new(D::DiscreteUniform { low: 0, high: 9 }), lower: Some(2.0), upper: Some(4.0) };
    close(cdf(&truncated, 2.0).unwrap(), 1.0 / 3.0, 1e-14);
    assert_eq!(quantile(&truncated, 0.0).unwrap(), 2.0);
    assert_eq!(quantile(&truncated, 1.0).unwrap(), 4.0);
    let mixture = D::Mixture { components: vec![D::Constant { value: -1.0 }, D::Constant { value: 3.0 }], weights: vec![1.0, 3.0] };
    close(cdf(&mixture, 0.0).unwrap(), 0.25, 1e-15);
    close(quantile(&mixture, 0.2).unwrap(), -1.0, 1e-11);
    close(quantile(&mixture, 0.3).unwrap(), 3.0, 1e-11);
    let compound = D::CompoundPoisson { rate: 3.0, severity: Box::new(D::Constant { value: 2.0 }), max_events: 100 };
    for trial in 0..100 {
        let x = sample(&compound, 17, 0, trial).unwrap();
        let n = sample(&D::Poisson { rate: 3.0 }, 17, 0, trial).unwrap();
        assert_eq!(x, 2.0 * n);
    }
    assert!(quantile(&compound, 0.5).is_err());
}

#[test]
fn public_domains_reject_malformed_or_excessive_inputs() {
    for d in [
        D::Gamma { shape: 0.0, scale: 1.0 },
        D::Beta { alpha: f64::NAN, beta: 1.0 },
        D::Geometric { probability: 0.0 },
        D::DiscreteUniform { low: 2, high: 1 },
        D::Hypergeometric { population: 4, successes: 5, draws: 1 },
        D::Categorical { values: vec![1.0], weights: vec![-1.0] },
        D::Mixture { components: vec![], weights: vec![] },
    ] {
        assert!(validate_distribution(&d).is_err(), "{d:?}");
    }
    let mut deep = D::Constant { value: 1.0 };
    for _ in 0..20 {
        deep = D::ShiftScale { base: Box::new(deep), shift: 0.0, scale: 1.0 };
    }
    assert!(PreparedDistribution::new(&deep).is_err());
    assert!(
        PreparedDistribution::new(&D::Truncated { base: Box::new(D::Uniform { low: 0.0, high: 1.0 }), lower: Some(3.0), upper: Some(4.0) }).is_err()
    );
}

#[test]
fn extended_extreme_ranges_are_correct_or_explicit_errors() {
    let empirical = D::EmpiricalContinuous { values: vec![-1e308, 1e308] };
    assert_eq!(quantile(&empirical, 0.5).unwrap(), 0.0);
    close(cdf(&empirical, 0.0).unwrap(), 0.5, 1e-15);
    let cauchy = D::Cauchy { location: -1e308, scale: 1e308 };
    close(cdf(&cauchy, 1e308).unwrap(), 0.5 + 2.0_f64.atan() / std::f64::consts::PI, 1e-15);
    assert_eq!(quantile(&cauchy, 0.5).unwrap(), -1e308);
    close(cdf(&D::Logistic { location: -1e308, scale: 1e308 }, 1e308).unwrap(), 1.0 / (1.0 + (-2.0_f64).exp()), 1e-15);
    assert!(quantile(&D::Gamma { shape: 1.0, scale: 1e-310 }, 1e-16).is_err());
    assert!(quantile(&D::Geometric { probability: 1e-100 }, 0.5).is_err());
    assert!(cdf(&D::Gamma { shape: 0.0001, scale: 1e308 }, 1e-308).is_err());
    let beta = D::Beta { alpha: 1.0, beta: 1.0 };
    let p = 1.0_f64.next_down();
    assert!(quantile(&beta, p).unwrap() >= p);
}

#[test]
fn half_integer_and_transformed_family_cdfs_match_independent_erf_references() {
    // Generated from Python's stdlib math.erf/erfc (platform libm), using analytic
    // half-integer Gamma and skew-normal(alpha=1) identities, not Solvercraft CDFs.
    for (x, p) in [(0.1, 0.022410702238350566), (1.0, 0.4275932955291201), (3.0, 0.8883897749052875), (10.0, 0.9998302575644472)] {
        close(cdf(&D::Gamma { shape: 1.5, scale: 1.0 }, x).unwrap(), p, 3e-13);
    }
    for (x, p) in [(-2.0, 0.0005175685036595646), (-0.3, 0.1459916812936688), (0.3, 0.381814525671574), (2.0, 0.9550173046073014)] {
        close(cdf(&D::SkewNormal { location: 0.0, scale: 1.0, shape: 1.0 }, x).unwrap(), p, 2e-10);
    }
    for (x, p) in
        [(0.5, 0.05518683599308054), (1.0, 0.28738674440477374), (2.0, 0.6436706247667283), (4.0, 0.900910513633541), (8.0, 0.9889942562708871)]
    {
        close(cdf(&D::InverseGaussian { mean: 2.0, shape: 3.0 }, x).unwrap(), p, 2e-12);
    }
    for x in [-10.0_f64, -1.0, -1e-5, 0.0, 1e-5, 1.0, 10.0] {
        close(cdf(&D::StudentT { degrees_of_freedom: 2.0, location: 0.0, scale: 1.0 }, x).unwrap(), 0.5 + x / (2.0 * (x * x + 2.0).sqrt()), 3e-13);
    }
    let tail = solvercraft_math::special::normal_cdf(-12.0).unwrap();
    assert!((tail / 1.776482112077702e-33 - 1.0).abs() < 1e-12);
}
#[test]
fn addressed_skew_normal_mc_matches_independent_population_moments() {
    let shape = 3.0_f64;
    let delta = shape / (1.0 + shape * shape).sqrt();
    let d = D::SkewNormal { location: 2.0, scale: 3.0, shape };
    let prepared = PreparedDistribution::new(&d).unwrap();
    let n = 40000_u64;
    let mut sum = 0.0;
    let mut squares = 0.0;
    for trial in 0..n {
        let x = prepared.sample(17, 0, trial).unwrap();
        sum += x;
        squares += x * x;
    }
    let mean = sum / n as f64;
    let variance = squares / n as f64 - mean * mean;
    close(mean, 2.0 + 3.0 * delta * std::f64::consts::FRAC_2_PI.sqrt(), 0.03);
    close(variance, 9.0 * (1.0 - std::f64::consts::FRAC_2_PI * delta * delta), 0.1);
}
