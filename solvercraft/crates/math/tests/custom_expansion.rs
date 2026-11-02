use solvercraft_math::{density::log_density, psi_aliases, quantile};
use solvercraft_types::Distribution as D;
fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "actual={a},expected={b},tolerance={t}");
}
#[test]
fn independent_custom_histogram_cumulative_mixture_and_truncation_laws() {
    let mut cases = 0;
    let mut seen = std::collections::BTreeSet::new();
    let mut cdf = |d: &D, x: f64| {
        seen.insert(format!("{d:?}/{}", x.to_bits()));
        solvercraft_math::cdf(d, x)
    };
    for s in [1e-100_f64, 0.001, 1.0, 3.0, 1e100] {
        for m in [-3.0_f64, 0.0, 2.0] {
            let low = (m - 2.0) * s;
            let high = (m + 2.0) * s;
            let histogram = psi_aliases::histogram(low, high, &[1.0, 2.0, 0.0, 1.0]).unwrap();
            let knots = [low + s, low + 2.0 * s, low + 3.0 * s];
            let ps = [0.1, 0.4, 0.9];
            let cumulative = psi_aliases::cumulative(low, high, &knots, &ps).unwrap();
            let descending = psi_aliases::cumulative_descending(low, high, &knots, &[0.9, 0.6, 0.1]).unwrap();
            for bin in 0..4 {
                for fraction in [0.1_f64, 0.25, 0.5, 0.75, 0.9] {
                    let x = low + s * (bin as f64 + fraction);
                    let weights = [0.25_f64, 0.5, 0.0, 0.25];
                    let prior = weights[..bin].iter().sum::<f64>();
                    near(cdf(&histogram, x).unwrap(), prior + weights[bin] * fraction, 2e-12);
                    cases += 1;
                    let mass = [0.1_f64, 0.3, 0.5, 0.1];
                    let prior = mass[..bin].iter().sum::<f64>();
                    near(cdf(&cumulative, x).unwrap(), prior + mass[bin] * fraction, 2e-12);
                    near(cdf(&descending, x).unwrap(), prior + mass[bin] * fraction, 2e-12);
                    cases += 2;
                }
            }
            let uniform = D::EmpiricalContinuous { values: vec![high, low] };
            let truncated = D::Truncated { base: Box::new(D::Uniform { low, high }), lower: Some(low + s), upper: Some(high - s) };
            for i in 1..20 {
                let p = i as f64 / 20.0;
                let x = low + (high - low) * p;
                near(cdf(&uniform, x).unwrap(), p, 2e-12);
                near(quantile(&uniform, p).unwrap(), x, 2e-12 * s);
                cases += 1;
                let x = low + s + 2.0 * s * p;
                near(cdf(&truncated, x).unwrap(), p, 2e-12);
                near(quantile(&truncated, p).unwrap(), x, 2e-11 * s);
                cases += 1;
            }
        }
    }
    // Original two-term Metalog reductions: logistic, lower-loglogistic,
    // and doubly bounded uniform, independent of Metalog's own inverse.
    for s in [0.1_f64, 1.0, 3.0] {
        for m in [-3.0_f64, 0.0, 2.0] {
            for z in [-10.0_f64, -3.0, -1.0, 0.0, 1.0, 3.0, 10.0] {
                let p = 1.0 / (1.0 + (-z).exp());
                let x = m + s * z;
                let d = D::Metalog { coefficients: vec![m, s], lower: None, upper: None };
                near(cdf(&d, x).unwrap(), p, 2e-10);
                near(log_density(&d, x).unwrap(), -s.ln() - z.abs() - 2.0 * (-z.abs()).exp().ln_1p(), 2e-10);
                cases += 1;
                let d = D::Metalog { coefficients: vec![m, s], lower: Some(-2.0), upper: None };
                let x = -2.0 + (m + s * z).exp();
                let expected = 1.0 / (1.0 + (-((x + 2.0).ln() - m) / s).exp());
                near(cdf(&d, x).unwrap(), expected, 2e-9);
                cases += 1;
                let d = D::Metalog { coefficients: vec![0.0, 1.0], lower: Some(m), upper: Some(m + s) };
                let x = m + s * p;
                near(cdf(&d, x).unwrap(), (x - m) / s, 2e-9);
                cases += 1;
            }
        }
    }
    let duplicate = D::EmpiricalContinuous { values: vec![0.0, 0.0, 2.0, 2.0] };
    for i in -2..=22 {
        let x = i as f64 / 10.0;
        let p = if x < 0.0 {
            0.0
        } else if x >= 2.0 {
            1.0
        } else {
            1.0 / 3.0 + x / 6.0
        };
        near(cdf(&duplicate, x).unwrap(), p, 2e-14);
        cases += 1;
    }
    assert_eq!(log_density(&duplicate, 1.0).unwrap_err().code, "density_unavailable");
    assert!(quantile(&D::Metalog { coefficients: vec![0.0, 1.0], lower: None, upper: Some(1.0) }, 0.5).is_err());
    eprintln!("distinct custom-law CDF tuples={}, evaluated feature probes={cases}; explicit capability refusals=2", seen.len());
}
