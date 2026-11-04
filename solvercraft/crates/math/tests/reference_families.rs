#![allow(clippy::unwrap_used)]
use solvercraft_math::{PreparedDistribution, cdf, density::log_density, quantile};
use solvercraft_types::Distribution as D;
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "actual{a}, expected{b}");
}
#[test]
fn every_added_reference_family_has_an_independent_cdf_anchor() {
    let cases = [
        (D::Burr12 { shape_c: 2.0, shape_k: 3.0, scale: 4.0 }, 4.0, 0.875),
        (D::Dagum { shape: 2.0, power: 3.0, scale: 4.0 }, 4.0, 0.125),
        (D::DoubleTriangular { low: 0.0, mode: 1.0, high: 3.0, left_probability: 0.7 }, 0.5, 0.175),
        (D::FatigueLife { shape: 0.5, scale: 2.0 }, 2.0, 0.5),
        (D::Frechet { shape: 2.0, scale: 3.0 }, 3.0, (-1.0_f64).exp()),
        (D::HyperbolicSecant { location: 2.0, scale: 3.0 }, 2.0, 0.5),
        (D::Kumaraswamy { alpha: 2.0, beta: 3.0 }, 0.5, 0.578125),
        (D::Levy { location: 2.0, scale: 3.0 }, 5.0, 0.3173105078629141),
        (D::Pareto2 { shape: 2.0, scale: 3.0 }, 3.0, 0.75),
        (D::BetaPrime { alpha: 2.0, beta: 3.0, scale: 4.0 }, 4.0, 0.6875),
        (D::Reciprocal { low: 1.0, high: 100.0 }, 10.0, 0.5),
        (D::MinExtreme { location: 2.0, scale: 3.0 }, 2.0, 1.0 - (-1.0_f64).exp()),
        (D::Logarithmic { probability: 0.5 }, 1.0, 1.0 / (2.0 * 2.0_f64.ln())),
    ];
    for (d, x, p) in cases {
        close(cdf(&d, x).unwrap(), p, 2e-12);
        if !matches!(d, D::Logarithmic { .. }) {
            close(quantile(&d, p).unwrap(), x, 2e-7);
        }
        let prep = PreparedDistribution::new(&d).unwrap();
        for p in [0.001, 0.1, 0.5, 0.9, 0.999] {
            let q = prep.quantile(p).unwrap();
            assert!(q.is_finite());
            let f = prep.cdf(q).unwrap();
            if matches!(d, D::Logarithmic { .. }) {
                assert!(f + 1e-12 >= p);
                assert!(prep.cdf(q - 1.0).unwrap() < p);
            } else {
                close(f, p, 2e-7);
            }
        }
    }
}
#[test]
fn new_density_and_tail_identities_are_independent() {
    close(log_density(&D::Burr12 { shape_c: 2.0, shape_k: 3.0, scale: 4.0 }, 4.0).unwrap().exp(), 0.09375, 1e-14);
    close(log_density(&D::Dagum { shape: 2.0, power: 3.0, scale: 4.0 }, 4.0).unwrap().exp(), 0.09375, 1e-14);
    close(log_density(&D::FatigueLife { shape: 0.5, scale: 2.0 }, 2.0).unwrap().exp(), 1.0 / std::f64::consts::TAU.sqrt(), 1e-14);
    close(quantile(&D::HyperbolicSecant { location: 0.0, scale: 1.0 }, 0.25).unwrap(), -1.0_f64.asinh(), 1e-14);
    close(cdf(&D::DoubleTriangular { low: 0.0, mode: 1.0, high: 3.0, left_probability: 0.7 }, 2.0).unwrap(), 0.925, 1e-14);
    let d = D::Logarithmic { probability: 0.99 };
    let q = quantile(&d, 1.0 - 1e-12).unwrap();
    assert!(q > 1000.0);
    assert!(cdf(&d, q).unwrap() >= 1.0 - 1.1e-12);
    assert!(quantile(&d, 1.0).is_err());
    for d in [
        D::Burr12 { shape_c: -1.0, shape_k: 2.0, scale: 1.0 },
        D::Reciprocal { low: 0.0, high: 1.0 },
        D::Logarithmic { probability: 0.9999 },
        D::DoubleTriangular { low: 0.0, mode: 1.0, high: 2.0, left_probability: 0.0 },
    ] {
        assert!(PreparedDistribution::new(&d).is_err());
    }
}
