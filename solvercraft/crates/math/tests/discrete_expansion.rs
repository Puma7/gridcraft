#![allow(clippy::unwrap_used)]
//! Direct combinatorial/recurrence oracles, never the engine's quantile/CDF.
use solvercraft_math::{PreparedDistribution, cdf, density::log_density, quantile};
use solvercraft_types::Distribution as D;
fn choose(n: u32, k: u32) -> f64 {
    if k > n { 0.0 } else { (1..=k.min(n - k)).fold(1.0, |v, i| v * f64::from(n + 1 - i) / f64::from(i)) }
}
fn checked(d: &D, k: f64, mass: f64, total: f64) {
    let actual = log_density(d, k).unwrap().exp();
    assert!((actual - mass).abs() <= 2e-11 * mass.max(1e-300), "PMF {d:?},k={k},actual={actual},expected={mass}");
    let actual = cdf(d, k).unwrap();
    let tolerance = if total <= 0.25 { 2e-10 * total.max(1e-300) } else { 2e-12 };
    assert!((actual - total).abs() <= tolerance, "CDF {d:?},k={k},actual={actual},expected={total}");
}
#[test]
fn exhaustive_small_binomial_hypergeometric_and_unbounded_count_prefixes() {
    let mut cases = 0;
    for n in [0_u32, 1, 2, 4, 8, 16, 32] {
        for p in [0.001_f64, 0.1, 0.25, 0.5, 0.9, 0.999] {
            let d = D::Binomial { trials: n, probability: p };
            let mut total = 0.0;
            for k in 0..=n {
                let mass = choose(n, k) * p.powi(k as i32) * (1.0 - p).powi((n - k) as i32);
                total += mass;
                checked(&d, f64::from(k), mass, total.min(1.0));
                cases += 1;
            }
            assert_eq!(quantile(&d, 0.0).unwrap(), 0.0);
            assert_eq!(quantile(&d, 1.0).unwrap(), f64::from(n));
        }
    }
    for n in [1_u32, 2, 4, 7, 10, 20] {
        for success in 0..=n {
            for draws in 0..=n.min(5) {
                let d = D::Hypergeometric { population: n, successes: success, draws };
                let mut total = 0.0;
                for k in 0..=draws {
                    let mass = if k <= success && draws - k <= n - success {
                        choose(success, k) * choose(n - success, draws - k) / choose(n, draws)
                    } else {
                        0.0
                    };
                    total += mass;
                    checked(&d, f64::from(k), mass, total.min(1.0));
                    cases += 1;
                }
            }
        }
    }
    for r in [1_u32, 2, 3, 8] {
        for p in [0.001_f64, 0.1, 0.25, 0.5, 0.9, 0.999] {
            let d = D::NegativeBinomial { successes: r, probability: p };
            let mut total = 0.0;
            for k in 0..64 {
                let mass = choose(k + r - 1, k) * p.powi(r as i32) * (1.0 - p).powi(k as i32);
                total += mass;
                checked(&d, f64::from(k), mass, total.min(1.0));
                cases += 1;
            }
        }
    }
    for rate in [0.001_f64, 0.1, 1.0, 4.0, 20.0, 128.0, 512.0] {
        let d = D::Poisson { rate };
        let mut mass = (-rate).exp();
        let mut total = 0.0;
        for k in 0..64 {
            if k > 0 {
                mass *= rate / f64::from(k);
            }
            total += mass;
            checked(&d, f64::from(k), mass, total.min(1.0));
            cases += 1;
        }
    }
    for p in [1e-8_f64, 0.001, 0.1, 0.5, 0.99] {
        let d = D::Geometric { probability: p };
        for k in 0..64 {
            checked(&d, f64::from(k), p * (1.0 - p).powi(k), -((f64::from(k) + 1.0) * (-p).ln_1p()).exp_m1());
            cases += 1;
        }
    }
    for p in [0.001_f64, 0.1, 0.5, 0.9, 0.99] {
        let d = D::Logarithmic { probability: p };
        let mut total = 0.0;
        for k in 1..=64 {
            let mass = -p.powi(k) / (f64::from(k) * (-p).ln_1p());
            total += mass;
            checked(&d, f64::from(k), mass, total.min(1.0));
            cases += 1;
        }
    }
    eprintln!("distinct combinatorial/count CDF+PMF tuples={cases}");
    assert_eq!(cases, 3983);
}
#[test]
fn rounded_affine_discrete_pushforward_matches_enumerated_atoms() {
    let sources = [
        (D::Categorical { values: vec![-2.0, 0.1, 0.1, 3.0], weights: vec![1.0, 1.0, 1.0, 1.0] }, vec![(-2.0, 0.25), (0.1, 0.5), (3.0, 0.25)]),
        (D::Empirical { values: vec![-2.0, 0.1, 0.1, 3.0] }, vec![(-2.0, 0.25), (0.1, 0.5), (3.0, 0.25)]),
        (D::Bernoulli { probability: 0.25 }, vec![(0.0, 0.75), (1.0, 0.25)]),
        (D::Binomial { trials: 3, probability: 0.5 }, vec![(0.0, 0.125), (1.0, 0.375), (2.0, 0.375), (3.0, 0.125)]),
        (D::DiscreteUniform { low: -2, high: 2 }, vec![(-2.0, 0.2), (-1.0, 0.2), (0.0, 0.2), (1.0, 0.2), (2.0, 0.2)]),
    ];
    let mut cases = 0;
    for (base, atoms) in sources {
        for shift in [-1e100_f64, -0.3, 0.0, 0.3, 1e100] {
            for scale in [-1e100_f64, -3.0, -0.2, -1e-100, 1e-100, 0.2, 3.0, 1e100] {
                let d = D::ShiftScale { base: Box::new(base.clone()), shift, scale };
                let prepared = PreparedDistribution::new(&d).unwrap();
                let mapped = atoms.iter().map(|(value, p)| (scale.mul_add(*value, shift), *p)).collect::<Vec<_>>();
                let mut visited = std::collections::BTreeSet::new();
                for (value, _) in &mapped {
                    for x in [value.next_down(), *value, value.next_up()] {
                        if !visited.insert(if x == 0.0 { 0 } else { x.to_bits() }) {
                            continue;
                        }
                        let expected = mapped.iter().filter(|(v, _)| *v <= x).map(|(_, p)| p).sum::<f64>();
                        let mass = mapped.iter().filter(|(v, _)| *v == x).map(|(_, p)| p).sum::<f64>();
                        let actual = prepared.cdf(x).unwrap();
                        assert!((actual - expected).abs() < 2e-12, "CDF {d:?},x={x},actual={actual},expected={expected}");
                        let actual = log_density(&d, x).unwrap().exp();
                        assert!((actual - mass).abs() < 2e-12, "PMF {d:?},x={x},actual={actual},expected={mass}");
                        cases += 1;
                    }
                }
            }
        }
    }
    eprintln!("distinct enumerated affine-atom CDF+PMF tuples={cases}");
}
