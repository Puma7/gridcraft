#![allow(clippy::unwrap_used)]
//! Independent Gaussian probability constants and transformation Jacobians.
use solvercraft_math::{cdf, density::log_density};
use solvercraft_types::Distribution as D;
const ANCHORS: [(f64, f64); 7] = [
    (-6.0, 9.865876450376946e-10),
    (-3.0, 0.0013498980316300945),
    (-1.0, 0.15865525393145707),
    (0.0, 0.5),
    (1.0, 0.8413447460685429),
    (3.0, 0.9986501019683699),
    (6.0, 0.9999999990134123),
];
fn check(d: &D, x: f64, p: f64, log: f64) {
    let actual = cdf(d, x).unwrap();
    assert!((actual - p).abs() < 2e-10, "CDF {d:?},x={x},actual={actual},expected={p}");
    let actual = log_density(d, x).unwrap();
    assert!((actual - log).abs() < 2e-8 * (1.0 + log.abs()), "logPDF {d:?},x={x},actual={actual},expected={log}");
}
#[test]
fn gaussian_transformation_and_jacobian_matrix() {
    let mut cases = 0;
    let mut seen = std::collections::BTreeSet::new();
    let mut check = |d: &D, x: f64, p: f64, log: f64| {
        seen.insert(format!("{d:?}/{}", x.to_bits()));
        check(d, x, p, log);
    };
    for s in [1e-100_f64, 0.001, 1.0, 3.0, 1e100] {
        for m in [-3.0_f64, 0.0, 2.0] {
            let location = m * s;
            for (z, p) in ANCHORS {
                let log = -0.5 * z * z - 0.5 * std::f64::consts::TAU.ln();
                check(&D::Normal { mean: location, std_dev: s }, s.mul_add(z, location), p, log - s.ln());
                cases += 1;
                check(&D::SkewNormal { location, scale: s, shape: 0.0 }, s.mul_add(z, location), p, log - s.ln());
                cases += 1;
                for gamma in [-1.0_f64, 0.0, 1.0] {
                    for delta in [1.0_f64, 3.0] {
                        let t = (z - gamma) / delta;
                        check(
                            &D::JohnsonSu { gamma, delta, location, scale: s },
                            s.mul_add(t.sinh(), location),
                            p,
                            log + delta.ln() - s.ln() - t.cosh().ln(),
                        );
                        cases += 1;
                        let y = t.exp();
                        check(&D::JohnsonSl { gamma, delta, location, scale: s }, s.mul_add(y, location), p, log + delta.ln() - s.ln() - t);
                        cases += 1;
                        let y = 1.0 / (1.0 + (-t).exp());
                        check(
                            &D::JohnsonSb { gamma, delta, location, scale: s },
                            s.mul_add(y, location),
                            p,
                            log + delta.ln() - s.ln() - y.ln() - (-y).ln_1p(),
                        );
                        cases += 1;
                    }
                }
                for shape in [0.1_f64, 1.0, 3.0] {
                    let a = shape * z / 2.0;
                    let y = (a.hypot(1.0) + a).powi(2);
                    check(&D::FatigueLife { shape, scale: s }, s * y, p, log + (1.0 + y).ln() - 2.0_f64.ln() - shape.ln() - s.ln() - 1.5 * y.ln());
                    cases += 1;
                }
                if z < 0.0 {
                    let y = 1.0 / (z * z);
                    check(
                        &D::Levy { location, scale: s },
                        s.mul_add(y, location),
                        2.0 * p,
                        -0.5 * std::f64::consts::TAU.ln() - s.ln() - 1.5 * y.ln() - 0.5 / y,
                    );
                    cases += 1;
                }
            }
            for shape in [-100.0_f64, -10.0, -2.0, -0.1, 0.0, 0.1, 2.0, 10.0, 100.0] {
                check(
                    &D::SkewNormal { location, scale: s, shape },
                    location,
                    0.5 - shape.atan() / std::f64::consts::PI,
                    -0.5 * std::f64::consts::TAU.ln() - s.ln(),
                );
                cases += 1;
            }
        }
    }
    // Lognormal parameter units differ from the physical scale matrix above.
    for mean in [-10.0_f64, 0.0, 10.0] {
        for std in [0.1_f64, 1.0, 3.0] {
            for (z, p) in ANCHORS {
                let x = (mean + std * z).exp();
                check(&D::Lognormal { log_mean: mean, log_std_dev: std }, x, p, -0.5 * z * z - 0.5 * std::f64::consts::TAU.ln() - std.ln() - x.ln());
                cases += 1;
            }
        }
    }
    eprintln!("distinct Gaussian transformation CDF+log-density tuples={}, evaluated probes={cases}", seen.len());
    assert_eq!(cases, 2658);
    assert_eq!(seen.len(), 2433);
}
