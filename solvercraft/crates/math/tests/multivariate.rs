#![allow(clippy::unwrap_used)]
use solvercraft_math::multivariate::*;
use solvercraft_types::{NoControl, RunControl};
fn close(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "actual{a}, expected{b}");
}
#[test]
fn covariance_map_has_an_independent_linear_algebra_oracle() {
    let limits = MultivariateLimits::default();
    let prepared = PreparedMultivariateNormal::new(&[1.0, -2.0], &[vec![4.0, 2.0], vec![2.0, 9.0]], &limits, &NoControl).unwrap();
    let first = prepared.transform(&[1.0, 0.0], false).unwrap();
    close(first[0], 3.0, 1e-14);
    close(first[1], -1.0, 1e-14);
    let second = prepared.transform(&[0.0, 1.0], false).unwrap();
    close(second[0], 1.0, 1e-14);
    close(second[1], -2.0 + 8.0_f64.sqrt(), 1e-14);
    let singular = PreparedMultivariateNormal::new(&[0.0, 0.0], &[vec![1e-320, 1e-320], vec![1e-320, 1e-320]], &limits, &NoControl).unwrap();
    let a = singular.sample(VectorAddress { seed: 17, binding: 2, trial: 9 }, false, &NoControl).unwrap();
    assert_eq!(a[0].to_bits(), a[1].to_bits());
    let zero = PreparedMultivariateNormal::new(&[-0.0, 2.0], &[vec![0.0, 0.0], vec![0.0, 1.0]], &limits, &NoControl).unwrap();
    assert_eq!(zero.transform(&[7.0, 0.0], false).unwrap()[0].to_bits(), (-0.0_f64).to_bits());
}
#[test]
fn normal_and_latent_lognormal_population_moments_match_reproducible_draws() {
    let limits = MultivariateLimits::default();
    let normal = PreparedMultivariateNormal::new(&[1.0, -2.0], &[vec![4.0, 2.0], vec![2.0, 9.0]], &limits, &NoControl).unwrap();
    let n = 40000;
    let (mut x, mut y, mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for trial in 0..n {
        let a = normal.sample(VectorAddress { seed: 7, binding: 2, trial }, false, &NoControl).unwrap();
        x += a[0];
        y += a[1];
        xx += a[0] * a[0];
        yy += a[1] * a[1];
        xy += a[0] * a[1];
    }
    let n = n as f64;
    let (mx, my) = (x / n, y / n);
    close(mx, 1.0, 0.04);
    close(my, -2.0, 0.05);
    close(xx / n - mx * mx, 4.0, 0.15);
    close(yy / n - my * my, 9.0, 0.2);
    close(xy / n - mx * my, 2.0, 0.12);
    let lognormal = PreparedMultivariateNormal::new(&[0.2, -0.1], &[vec![0.2, 0.05], vec![0.05, 0.3]], &limits, &NoControl).unwrap();
    let (mut x, mut y, mut xy) = (0.0, 0.0, 0.0);
    for trial in 0..40000 {
        let a = lognormal.sample(VectorAddress { seed: 19, binding: 0, trial }, true, &NoControl).unwrap();
        assert!(a.iter().all(|x| *x > 0.0));
        x += a[0];
        y += a[1];
        xy += a[0] * a[1];
    }
    let (mx, my) = (x / 40000.0, y / 40000.0);
    close(mx, 0.3_f64.exp(), 0.025);
    close(my, 0.05_f64.exp(), 0.025);
    close(xy / 40000.0 - mx * my, 0.35_f64.exp() * 0.05_f64.exp_m1(), 0.02);
    let address = VectorAddress { seed: u64::MAX, binding: u64::MAX, trial: u64::MAX };
    assert_eq!(normal.sample(address, false, &NoControl).unwrap(), normal.sample(address, false, &NoControl).unwrap());
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn invalid_covariances_resources_and_lognormal_endpoints_are_explicit() {
    let limits = MultivariateLimits::default();
    for matrix in [
        vec![vec![-1.0, 0.0], vec![0.0, 1.0]],
        vec![vec![1.0, 2.0], vec![2.0, 1.0]],
        vec![vec![1.0, 0.1], vec![0.2, 1.0]],
        vec![vec![0.0, 1e-310], vec![1e-310, 1.0]],
    ] {
        assert!(PreparedMultivariateNormal::new(&[0.0, 0.0], &matrix, &limits, &NoControl).is_err());
    }
    assert!(
        PreparedMultivariateNormal::new(&[0.0], &[vec![1.0]], &MultivariateLimits { max_memory_bytes: 1, ..limits.clone() }, &NoControl).is_err()
    );
    assert!(PreparedMultivariateNormal::new(&[0.0], &[vec![1.0]], &limits, &Cancel).is_err());
    for mean in [-1000.0, 1000.0] {
        let d = PreparedMultivariateNormal::new(&[mean], &[vec![0.0]], &limits, &NoControl).unwrap();
        assert!(d.transform(&[0.0], true).is_err());
    }
}
