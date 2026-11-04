#![allow(clippy::unwrap_used)]
use solvercraft_math::density::log_density;
use solvercraft_types::Distribution as D;
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-11, "actual {a}, expected {b}");
}
#[test]
fn independent_density_identities_and_discrete_normalization() {
    for x in [0.0_f64, 0.5, 1.0, 2.0, 10.0] {
        close(log_density(&D::Gamma { shape: 1.0, scale: 2.0 }, x).unwrap(), log_density(&D::Exponential { rate: 0.5 }, x).unwrap());
        close(
            log_density(&D::StudentT { degrees_of_freedom: 1.0, location: 2.0, scale: 3.0 }, x).unwrap(),
            log_density(&D::Cauchy { location: 2.0, scale: 3.0 }, x).unwrap(),
        );
    }
    for d in [
        D::Binomial { trials: 8, probability: 0.3 },
        D::Hypergeometric { population: 10, successes: 4, draws: 3 },
        D::DiscreteUniform { low: 0, high: 9 },
        D::Categorical { values: vec![0.0, 1.0, 1.0], weights: vec![1.0, 2.0, 3.0] },
    ] {
        let total = (0..=10).map(|x| log_density(&d, f64::from(x)).unwrap().exp()).sum::<f64>();
        close(total, 1.0);
    }
    close(log_density(&D::Beta { alpha: 2.0, beta: 3.0 }, 0.5).unwrap().exp(), 1.5);
    close(log_density(&D::Normal { mean: 0.0, std_dev: 1.0 }, 0.0).unwrap().exp(), 1.0 / std::f64::consts::TAU.sqrt());
    assert_eq!(log_density(&D::Exponential { rate: 1.0 }, -1.0).unwrap(), f64::NEG_INFINITY);
    assert!(
        log_density(
            &D::Mixture { components: vec![D::Constant { value: 0.0 }, D::Normal { mean: 0.0, std_dev: 1.0 }], weights: vec![1.0, 1.0] },
            0.0
        )
        .is_err()
    );
}
#[test]
fn negative_scale_and_truncation_use_correct_density_measure() {
    let continuous = D::ShiftScale { base: Box::new(D::Uniform { low: 0.0, high: 1.0 }), shift: 3.0, scale: -2.0 };
    close(log_density(&continuous, 2.0).unwrap().exp(), 0.5);
    let discrete = D::ShiftScale { base: Box::new(D::Bernoulli { probability: 0.3 }), shift: 3.0, scale: -2.0 };
    close(log_density(&discrete, 1.0).unwrap().exp(), 0.3);
    let trunc = D::Truncated { base: Box::new(D::Uniform { low: 0.0, high: 10.0 }), lower: Some(2.0), upper: Some(4.0) };
    close(log_density(&trunc, 3.0).unwrap().exp(), 0.5);
}
