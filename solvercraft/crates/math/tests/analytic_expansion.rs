#![allow(clippy::panic)]
//! Independent elementary probability laws; each counter is one distinct
//! (distribution parameters, x) tuple checked against both CDF and log density.
use solvercraft_math::{cdf, density::log_density};
use solvercraft_types::Distribution as D;
const SCALES: [f64; 5] = [1e-100, 0.001, 1.0, 3.0, 1e100];
const X: [f64; 16] = [1e-12, 1e-6, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 2.0, 4.0, 8.0, 16.0, 100.0, 1e6];
fn check(d: &D, x: f64, p: f64, log: f64) {
    let actual = cdf(d, x).unwrap_or_else(|e| panic!("CDF {d:?},x={x}: {e:?}"));
    let tail = p.min(1.0 - p);
    let tolerance = 2e-12_f64.max(2e-8 * tail);
    assert!((actual - p).abs() <= tolerance, "CDF {d:?},x={x}: actual={actual},expected={p}");
    let actual = log_density(d, x).unwrap_or_else(|e| panic!("density {d:?},x={x}: {e:?}"));
    if log.is_infinite() {
        assert_eq!(actual, log, "{d:?},x={x}");
    } else {
        assert!((actual - log).abs() <= 2e-10 * (1.0 + log.abs()), "logPDF {d:?},x={x}: actual={actual},expected={log}");
    }
}
#[test]
fn positive_scale_shape_closed_form_matrix() {
    let mut cases = 0;
    for s in SCALES {
        for k in [0.25_f64, 1.0, 2.0, 10.0] {
            for z in X {
                let x = s * z;
                let power = z.powf(k);
                check(&D::Weibull { shape: k, scale: s }, x, -(-power).exp_m1(), k.ln() - s.ln() + (k - 1.0) * z.ln() - power);
                cases += 1;
                check(
                    &D::Pareto { minimum: s, shape: k },
                    x,
                    if z < 1.0 { 0.0 } else { -(-k * z.ln()).exp_m1() },
                    if z < 1.0 { f64::NEG_INFINITY } else { k.ln() - s.ln() - (k + 1.0) * z.ln() },
                );
                cases += 1;
                let lomax = -(-k * z.ln_1p()).exp_m1();
                let lomax_log = k.ln() - s.ln() - (k + 1.0) * z.ln_1p();
                check(&D::Pareto2 { shape: k, scale: s }, x, lomax, lomax_log);
                cases += 1;
                check(&D::BetaPrime { alpha: 1.0, beta: k, scale: s }, x, lomax, lomax_log);
                cases += 1;
                let inverse = z.powf(-k);
                check(&D::Frechet { shape: k, scale: s }, x, (-inverse).exp(), k.ln() - s.ln() - (k + 1.0) * z.ln() - inverse);
                cases += 1;
                check(&D::LogLogistic { shape: k, scale: s }, x, power / (1.0 + power), k.ln() - s.ln() + (k - 1.0) * z.ln() - 2.0 * power.ln_1p());
                cases += 1;
                check(
                    &D::Burr12 { shape_c: k, shape_k: 2.0, scale: s },
                    x,
                    -(-2.0 * power.ln_1p()).exp_m1(),
                    (2.0 * k).ln() - s.ln() + (k - 1.0) * z.ln() - 3.0 * power.ln_1p(),
                );
                cases += 1;
                check(
                    &D::Dagum { shape: k, power: 2.0, scale: s },
                    x,
                    (-2.0 * inverse.ln_1p()).exp(),
                    (2.0 * k).ln() - s.ln() + (2.0 * k - 1.0) * z.ln() - 3.0 * power.ln_1p(),
                );
                cases += 1;
            }
        }
        for z in X {
            let x = s * z;
            let exponential = -(-z).exp_m1();
            let log = -s.ln() - z;
            check(&D::Exponential { rate: 1.0 / s }, x, exponential, log);
            cases += 1;
            check(&D::Gamma { shape: 1.0, scale: s }, x, exponential, log);
            cases += 1;
            check(&D::Erlang { shape: 1, scale: s }, x, exponential, log);
            cases += 1;
            check(&D::InverseGamma { shape: 1.0, scale: s }, x, (-1.0 / z).exp(), -s.ln() - 2.0 * z.ln() - 1.0 / z);
            cases += 1;
            check(&D::Rayleigh { scale: s }, x, -(-0.5 * z * z).exp_m1(), z.ln() - s.ln() - 0.5 * z * z);
            cases += 1;
        }
    }
    assert_eq!(cases, 2960);
    eprintln!("distinct positive CDF/log-density tuples={cases}");
}
#[test]
fn location_scale_closed_form_matrix() {
    let mut cases = 0;
    for s in SCALES {
        // Location is proportional to scale so the tested standardized input
        // remains representable even at extreme units.
        for m in [-3.0_f64, 0.0, 2.0] {
            let location = s * m;
            for z in [-12.0_f64, -8.0, -4.0, -2.0, -1.0, -0.1, 0.0, 0.1, 1.0, 2.0, 4.0, 8.0, 12.0] {
                let x = s.mul_add(z, location);
                // Use the actual representable displacement as the analytic
                // argument; this separates conversion rounding from the law.
                let t = (x - location) / s;
                let sig = if t < 0.0 { t.exp() / (1.0 + t.exp()) } else { 1.0 / (1.0 + (-t).exp()) };
                check(&D::Logistic { location, scale: s }, x, sig, -s.ln() - t.abs() - 2.0 * (-t.abs()).exp().ln_1p());
                cases += 1;
                check(
                    &D::Laplace { location, scale: s },
                    x,
                    if t < 0.0 { 0.5 * t.exp() } else { 1.0 - 0.5 * (-t).exp() },
                    -2.0_f64.ln() - s.ln() - t.abs(),
                );
                cases += 1;
                check(
                    &D::Cauchy { location, scale: s },
                    x,
                    if t < 0.0 {
                        (-t.recip()).atan() / std::f64::consts::PI
                    } else if t > 0.0 {
                        1.0 - t.recip().atan() / std::f64::consts::PI
                    } else {
                        0.5
                    },
                    -std::f64::consts::PI.ln() - s.ln() - (t * t).ln_1p(),
                );
                cases += 1;
                check(&D::Gumbel { location, scale: s }, x, (-(-t).exp()).exp(), -s.ln() - t - (-t).exp());
                cases += 1;
                check(&D::MinExtreme { location, scale: s }, x, -(-t.exp()).exp_m1(), -s.ln() + t - t.exp());
                cases += 1;
                check(
                    &D::HyperbolicSecant { location, scale: s },
                    x,
                    2.0 * t.exp().atan() / std::f64::consts::PI,
                    (2.0 / std::f64::consts::PI).ln() - s.ln() + t - (2.0 * t).exp().ln_1p(),
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 1170);
    eprintln!("distinct location CDF/log-density tuples={cases}");
}
#[test]
fn bounded_polynomial_and_power_laws() {
    let mut cases = 0;
    let mut seen = std::collections::BTreeSet::new();
    let mut check = |d: &D, x: f64, p: f64, log: f64| {
        seen.insert(format!("{d:?}/{}", x.to_bits()));
        check(d, x, p, log);
    };
    for i in 1..100 {
        let x = i as f64 / 100.0;
        for k in [0.25_f64, 1.0, 2.0, 10.0] {
            check(&D::Beta { alpha: k, beta: 1.0 }, x, x.powf(k), k.ln() + (k - 1.0) * x.ln());
            cases += 1;
            check(&D::Beta { alpha: 1.0, beta: k }, x, -(k * (-x).ln_1p()).exp_m1(), k.ln() + (k - 1.0) * (-x).ln_1p());
            cases += 1;
            let power = x.powf(k);
            check(&D::Kumaraswamy { alpha: k, beta: 2.0 }, x, 2.0 * power - power * power, (2.0 * k).ln() + (k - 1.0) * x.ln() + (-power).ln_1p());
            cases += 1;
        }
        check(&D::Beta { alpha: 2.0, beta: 3.0 }, x, 6.0 * x * x - 8.0 * x.powi(3) + 3.0 * x.powi(4), 12.0_f64.ln() + x.ln() + 2.0 * (-x).ln_1p());
        cases += 1;
        let pdf = 30.0 * x * x * (1.0 - x) * (1.0 - x);
        check(&D::BetaPert { low: 0.0, mode: 0.5, high: 1.0, weight: 4.0 }, x, 10.0 * x.powi(3) - 15.0 * x.powi(4) + 6.0 * x.powi(5), pdf.ln());
        cases += 1;
        check(&D::F { numerator_df: 2.0, denominator_df: 2.0 }, x, x / (1.0 + x), -2.0 * x.ln_1p());
        cases += 1;
        check(&D::ChiSquared { degrees_of_freedom: 2.0 }, x, -(-x / 2.0).exp_m1(), -2.0_f64.ln() - x / 2.0);
        cases += 1;
    }
    assert_eq!(cases, 1584);
    eprintln!("distinct bounded/reduction CDF/log-density tuples={}, evaluated probes={cases}", seen.len());
    assert_eq!(seen.len(), 1485);
}
