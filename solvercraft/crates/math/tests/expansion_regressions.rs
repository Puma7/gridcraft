use solvercraft_math::{cdf, density::log_density, quantile};
use solvercraft_types::Distribution as D;

#[test]
fn affine_point_masses_keep_their_atoms_after_float_rounding() {
    let mut cases = 0;
    for atom in [-0.1_f64, 0.1, 0.3, 1.0, 1e-100, 1e100] {
        for shift in [-0.3_f64, 0.0, 0.3] {
            for scale in [-0.2_f64, -3.0, 0.2, 3.0] {
                let mapped = scale.mul_add(atom, shift);
                let d = D::ShiftScale { base: Box::new(D::Constant { value: atom }), shift, scale };
                assert_eq!(quantile(&d, 0.5).unwrap(), mapped);
                assert_eq!(cdf(&d, mapped).unwrap(), 1.0, "atom={atom}, shift={shift}, scale={scale}, mapped={mapped}");
                assert_eq!(cdf(&d, mapped.next_down()).unwrap(), 0.0, "atom={atom}, shift={shift}, scale={scale}");
                assert_eq!(log_density(&d, mapped).unwrap(), 0.0, "atom={atom}, shift={shift}, scale={scale}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 72);
}

#[test]
fn weighted_point_endpoints_do_not_lose_small_positive_last_mass() {
    for tiny_weight in [1e-20, 1e-100, 1e-300] {
        let d = D::Categorical { values: vec![-2.0, 3.0], weights: vec![1.0, tiny_weight] };
        assert_eq!(quantile(&d, 0.0).unwrap(), -2.0);
        assert_eq!(quantile(&d, 1.0).unwrap(), 3.0, "weight={tiny_weight}");
    }
}

#[test]
fn accepted_normal_tail_truncation_has_accurate_conditional_mass() {
    // Independent libc erfc Gaussian probabilities, also checked by root's
    // external SciPy matrix; retained mass2.856649842e-7 exceeds the1e-9 gate.
    let lo = 9.865_876_450_376_946e-10;
    let hi = 2.866_515_718_791_933e-7;
    let at = 1.898_956_246_588_768e-8;
    let d = D::Truncated { base: Box::new(D::Normal { mean: 0.0, std_dev: 1.0 }), lower: Some(-6.0), upper: Some(-5.0) };
    let expected = (at - lo) / (hi - lo);
    let actual = cdf(&d, -5.5).unwrap();
    assert!((actual - expected).abs() < 2e-10, "actual={actual},expected={expected}");
    let expected_log = -0.5 * 5.5 * 5.5 - 0.5 * std::f64::consts::TAU.ln() - (hi - lo).ln();
    let actual = log_density(&d, -5.5).unwrap();
    assert!((actual - expected_log).abs() < 2e-10, "actual={actual},expected={expected_log}");
}
#[test]
fn direct_upper_gamma_tails_and_symmetric_medians_do_not_cancel() {
    for beta in [0.1_f64, 1.0, 2.0, 123.0, 1000.0] {
        for p in [0.9_f64, 0.999, 1.0 - 1e-12] {
            let expected = -((-p).ln_1p() / beta).exp_m1();
            let actual = quantile(&D::Beta { alpha: 1.0, beta }, p).unwrap();
            assert!((actual - expected).abs() < 1e-10, "beta={beta},p={p},actual={actual},expected={expected}");
        }
    }
    for scale in [1e-50_f64, 0.3, 2.0, 7.0, 1e50] {
        for p in [0.9_f64, 0.999, 1.0 - 1e-12, 1.0_f64.next_down()] {
            let expected = -scale * (-p).ln_1p();
            let actual = quantile(&D::Gamma { shape: 1.0, scale }, p).unwrap();
            assert!((actual / expected - 1.0).abs() < 1e-10, "scale={scale},p={p},actual={actual},expected={expected}");
        }
        assert_eq!(quantile(&D::HyperbolicSecant { location: -3.0, scale }, 0.5).unwrap(), -3.0);
    }
}
#[test]
fn reflected_symmetric_quantiles_keep_the_requested_small_tail() {
    for scale in [-10.0_f64, -1.0, -0.1] {
        let normal = D::ShiftScale { base: Box::new(D::Normal { mean: 0.0, std_dev: 1.0 }), shift: -2.0, scale };
        let expected = (-scale).mul_add(-7.034_483_825_301_132, -2.0);
        let actual = quantile(&normal, 1e-12).unwrap();
        assert!((actual - expected).abs() < 1e-7, "actual={actual},expected={expected}");
        let cauchy = D::ShiftScale { base: Box::new(D::Cauchy { location: 0.0, scale: 1.0 }), shift: -2.0, scale };
        let expected = scale.mul_add(1.0 / (std::f64::consts::PI * 1e-12).tan(), -2.0);
        let actual = quantile(&cauchy, 1e-12).unwrap();
        assert!((actual / expected - 1.0).abs() < 1e-12, "actual={actual},expected={expected}");
    }
}
#[test]
fn inverse_gaussian_upper_quantile_inverts_the_small_survival_tail() {
    // Independent SciPy inverse-Gaussian tail reference from the external matrix.
    let d = D::InverseGaussian { mean: 2.0, shape: 1.0 };
    let actual = quantile(&d, 1.0 - 1e-12).unwrap();
    let expected = 172.020_286_964_213_06;
    assert!((actual / expected - 1.0).abs() < 2e-7, "actual={actual},expected={expected}");
}
#[test]
fn continuous_truncation_does_not_invent_an_atom_at_its_lower_bound() {
    for lower in [-1.0_f64, 4.0, 5.0] {
        let upper = if lower < 0.0 { 1.0 } else { lower + 1.0 };
        let d = D::Truncated { base: Box::new(D::Normal { mean: 0.0, std_dev: 1.0 }), lower: Some(lower), upper: Some(upper) };
        assert_eq!(cdf(&d, lower).unwrap(), 0.0, "lower={lower}");
        let width = lower.next_up() - lower;
        let phi = (-0.5 * lower * lower).exp() / std::f64::consts::TAU.sqrt();
        let full = if lower < 0.0 {
            0.6826894921370859
        } else if lower == 4.0 {
            3.138459026124067e-5
        } else {
            2.856649842341562e-7
        };
        let expected = width * phi / full;
        let actual = cdf(&d, lower.next_up()).unwrap();
        assert!((actual / expected - 1.0).abs() < 2e-12, "lower={lower},actual={actual},expected={expected}");
    }
}
