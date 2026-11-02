use solvercraft_math::{empirical_quantile, upper_tail_cvar};
#[test]
fn empirical_quantiles_and_cvar_match_the_excess_loss_identity() {
    let mut cases = 0;
    for n in [2_usize, 3, 4, 7, 16, 31, 64, 127] {
        for seed in 0..5 {
            for scale in [1e-100_f64, 0.001, 1.0, 1e3, 1e100] {
                let normalized = (0..n).map(|i| ((i * 7 + seed * 11) % 41) as f64 - 20.0).collect::<Vec<_>>();
                let values = normalized.iter().map(|x| x * scale).collect::<Vec<_>>();
                let mut ordered = normalized.clone();
                ordered.sort_by(f64::total_cmp);
                for alpha in [0.0_f64, 0.125, 0.25, 0.5, 0.75, 0.875, 0.95, 0.99, 1.0_f64.next_down()] {
                    // Derive the lower inverse from empirical CDF counts, rather
                    // than reuse the implementation's ceil(alpha*n) index formula.
                    let threshold = *ordered.iter().find(|x| normalized.iter().filter(|v| **v <= **x).count() as f64 / n as f64 >= alpha).unwrap();
                    let actual = empirical_quantile(&values, alpha).unwrap();
                    assert_eq!(actual, threshold * scale);
                    // Independent expected-shortfall identity at an alpha quantile:
                    // ES=eta+E[(L-eta)+]/(1-alpha), including all fractional atoms.
                    let expected = threshold + normalized.iter().map(|x| (x - threshold).max(0.0)).sum::<f64>() / (n as f64 * (1.0 - alpha));
                    let actual = upper_tail_cvar(&values, alpha).unwrap() / scale;
                    assert!((actual - expected).abs() < 3e-12, "n={n},seed={seed},scale={scale},alpha={alpha},actual={actual},expected={expected}");
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 1800);
    eprintln!("distinct empirical-risk dataset/alpha tuples={cases}");
}
