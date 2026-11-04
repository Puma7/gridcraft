//! Independent analytic identities and SciPy1.18.1 reference anchors.
use solvercraft_math::{cdf, density::log_density, quantile};
use solvercraft_types::Distribution as D;
#[test]
fn multivariate_transform_reconstructs_covariance_and_handles_zero_rank() {
    use solvercraft_math::multivariate::{MultivariateLimits, PreparedMultivariateNormal};
    use solvercraft_types::NoControl;
    let covariance = vec![vec![4.0, 3.0], vec![3.0, 9.0]];
    let prepared = PreparedMultivariateNormal::new(&[0.0, 0.0], &covariance, &MultivariateLimits::default(), &NoControl).expect("covariance");
    let columns = [prepared.transform(&[1.0, 0.0], false).expect("first basis"), prepared.transform(&[0.0, 1.0], false).expect("second basis")];
    for i in 0..2 {
        for j in 0..2 {
            let reconstructed: f64 = columns.iter().map(|column| column[i] * column[j]).sum();
            assert!((reconstructed - covariance[i][j]).abs() < 1e-12, "({i},{j}) covariance {reconstructed}");
        }
    }
    let zero = vec![vec![0.0; 2]; 2];
    let prepared = PreparedMultivariateNormal::new(&[0.0, 2.0_f64.ln()], &zero, &MultivariateLimits::default(), &NoControl).expect("zero PSD");
    assert_eq!(prepared.transform(&[1e300, -1e300], false).unwrap(), vec![0.0, 2.0_f64.ln()]);
    assert_eq!(prepared.transform(&[1e300, -1e300], true).unwrap(), vec![1.0, 2.0]);
    let singular = vec![vec![4.0, -6.0], vec![-6.0, 9.0]];
    let prepared = PreparedMultivariateNormal::new(&[0.0, 0.0], &singular, &MultivariateLimits::default(), &NoControl).expect("rank-one PSD");
    let values = prepared.transform(&[2.0, -123.0], false).unwrap();
    assert_eq!(values, vec![4.0, -6.0]);
    assert!(PreparedMultivariateNormal::new(&[0.0, 0.0], &[vec![0.0, 1.0], vec![1.0, 1.0]], &MultivariateLimits::default(), &NoControl).is_err());
}
#[test]
fn skew_normal_log_density_does_not_turn_a_finite_tail_into_zero_mass() {
    let control = log_density(&D::SkewNormal { location: 0.0, scale: 1.0, shape: 0.0 }, -10.0).expect("symmetric control");
    assert!((control - (-50.918_938_533_204_674)).abs() < 1e-10);
    // Independent scipy.stats.skewnorm.logpdf(-10,100), SciPy1.18.1.
    let actual = log_density(&D::SkewNormal { location: 0.0, scale: 1.0, shape: 100.0 }, -10.0).expect("finite supported observation");
    assert!(actual.is_finite() && (actual - (-500_058.052_486_164_84)).abs() < 1e-6, "finite log density was lost: {actual}");
}
#[test]
fn cauchy_log_density_uses_log_scale_before_squaring_huge_finite_values() {
    // log(1+x²)=2log|x|+log1p(1/x²), avoiding the overflowing square.
    let x = 1e200_f64;
    let truth = -std::f64::consts::PI.ln() - 2.0 * x.ln();
    let actual = log_density(&D::Cauchy { location: 0.0, scale: 1.0 }, x).expect("Cauchy tail");
    assert!(actual.is_finite() && (actual - truth).abs() < 1e-10, "finite Cauchy log density lost: {actual}, reference{truth}");
}
#[test]
fn inverse_gaussian_and_small_shape_beta_match_independent_anchors() {
    let cdf_value = cdf(&D::InverseGaussian { mean: 1.0, shape: 1000.0 }, 1.0).expect("IG CDF");
    assert!((cdf_value - 0.506_306_255_528_460_9).abs() < 1e-7);
    let q = quantile(&D::Beta { alpha: 0.1, beta: 0.1 }, 1e-10).expect("beta tail");
    let reference = 8.869_280_655_550_325e-98;
    assert!(((q - reference) / reference).abs() < 1e-6, "beta tail {q}, reference{reference}");
}
#[test]
fn reflected_discrete_quantile_respects_probability_mass_at_the_boundary() {
    let reflected = D::ShiftScale { base: Box::new(D::Bernoulli { probability: 0.3 }), shift: 0.0, scale: -1.0 };
    assert_eq!(quantile(&reflected, 0.25).expect("below mass boundary"), -1.0);
    assert_eq!(quantile(&reflected, 0.3).expect("at mass boundary"), -1.0);
    assert_eq!(quantile(&reflected, 0.35).expect("above mass boundary"), 0.0);
    assert!((cdf(&reflected, -1.0).expect("reflected atom") - 0.3).abs() < 1e-15);
}
#[test]
fn reflected_copula_conditional_cdf_includes_the_selected_reflection() {
    use solvercraft_math::copula::{DependenceSpec, PreparedCopula, Reflection};
    let plain = 8.0 / 7.0_f64.powf(1.5);
    let c = PreparedCopula::new(&DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::Second }, 2).expect("copula");
    // C_ref(u,v)=u-C(u,1-v), hence dC_ref/du=1-dC(u,1-v)/du.
    assert!((c.conditional(0.5, 0.5).expect("reflected conditional") - (1.0 - plain)).abs() < 1e-12);
    let lower = PreparedCopula::new(&DependenceSpec::Clayton { theta: -1.0, reflection: Reflection::None }, 2).expect("countermonotonic limit");
    assert_eq!(lower.conditional(0.25, 0.74).expect("below deterministic atom"), 0.0);
    assert_eq!(lower.conditional(0.25, 0.75).expect("at deterministic atom"), 1.0);
}
#[test]
fn beta_prime_cdf_does_not_erase_representable_tail_mass() {
    let beta_prime = D::BetaPrime { alpha: 10.0, beta: 0.2, scale: 2.0 };
    let actual = cdf(&beta_prime, 2.943_586_860_630_712e16).expect("finite beta-prime point");
    assert!((actual - 0.999).abs() < 2e-8, "BetaPrime CDF erased0.001 tail probability: {actual}");
}
#[test]
fn skew_cdf_matches_an_independent_small_tail_anchor() {
    let skew = D::SkewNormal { location: 0.0, scale: 1.0, shape: 100.0 };
    let x = -0.007_761_517_742_369_156;
    let actual = cdf(&skew, x).expect("skew CDF");
    assert!((actual - 0.001).abs() < 2e-8, "Skew CDF{actual} disagrees with independent0.001 anchor");
}
#[test]
fn sobol_v2_matches_independent_scipy_64_dimensional_digital_net() {
    use solvercraft_math::sobol::{RandomizedSobol, SOBOL_VERSION};
    let net = RandomizedSobol::new(64, 42, 0).expect("Sobol net");
    let mut fingerprint = 0xcbf29ce484222325_u64;
    // SciPy1.18.1 qmc.Sobol(d=64,scramble=False,bits=64).random_base2(8),
    // row-major bytes obtained by exact scaling by256; 16384 independent anchors.
    for point in 0..256 {
        for dimension in 0..64 {
            let coordinate = net.unshifted_coordinate(point, dimension).expect("net coordinate");
            let integer = coordinate * 256.0;
            assert_eq!(integer.fract(), 0.0);
            fingerprint ^= integer as u64;
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(fingerprint, 0xd6218aa1c6599625, "Unexpected Joe-Kuo net under{SOBOL_VERSION}");
}
