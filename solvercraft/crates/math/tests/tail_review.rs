use solvercraft_math::{cdf, quantile};
use solvercraft_types::Distribution;

#[test]
fn bounded_uniform_endpoint_preserves_the_declared_upper_bound() {
    for (low, high) in [(-1e100, 1.0), (-1e308, 1e-308), (-1e50, 1e-50)] {
        let distribution = Distribution::Uniform { low, high };
        assert_eq!(quantile(&distribution, 0.0).unwrap(), low);
        assert_eq!(quantile(&distribution, 1.0).unwrap(), high);
    }
}

#[test]
fn inverse_gamma_unit_shape_has_its_exact_exponential_reciprocal_tail() {
    // If X is inverse-Gamma(shape=1, scale=1), F(x)=exp(-1/x).
    // This anchor has no special-function or SciPy dependency.
    let distribution = Distribution::InverseGamma { shape: 1.0, scale: 1.0 };
    for x in [0.01_f64, 0.02, 0.1, 1.0, 10.0] {
        let expected = (-1.0 / x).exp();
        let actual = cdf(&distribution, x).unwrap();
        assert!((actual / expected - 1.0).abs() < 2e-12, "x={x}, actual={actual}, expected={expected}");
    }
    for p in [1e-100_f64, 1e-20, 1e-8, 0.25, 0.5, 0.9] {
        let expected = -1.0 / p.ln();
        let actual = quantile(&distribution, p).unwrap();
        assert!((actual / expected - 1.0).abs() < 2e-11, "p={p}, actual={actual}, expected={expected}");
    }
}

#[test]
fn inverse_gamma_small_shape_upper_quantile_preserves_small_survival_mass() {
    // Independent SciPy1.18.1 invgamma.ppf(0.9999999999,0.1,scale=2).
    // The small shape amplifies loss of precision if a near-one Q CDF is
    // inverted instead of its small regularized-gamma-P complement.
    let d = Distribution::InverseGamma { shape: 0.1, scale: 2.0 };
    let expected = 3.293_236_321_932_283e100;
    let actual = quantile(&d, 0.999_999_999_9).unwrap();
    assert!((actual / expected - 1.0).abs() < 2e-6, "actual={actual}, expected={expected}");
}

#[test]
fn cauchy_cdf_preserves_its_representable_negative_tail() {
    let d = Distribution::Cauchy { location: 0.0, scale: 1.0 };
    for x in [-1e20_f64, -1e100, -1e200] {
        let expected = (-x.recip()).atan() / std::f64::consts::PI;
        let actual = cdf(&d, x).unwrap();
        assert!((actual / expected - 1.0).abs() < 1e-14, "x={x}, actual={actual}, expected={expected}");
    }
}

#[test]
fn log_logistic_cdf_keeps_log_ratios_before_small_shape_scaling() {
    for (x, scale) in [(1e-308_f64, 1e308_f64), (1e308, 1e-308)] {
        let d = Distribution::LogLogistic { shape: 1e-4, scale };
        let expected = 1.0 / (1.0 + (-1e-4 * (x.ln() - scale.ln())).exp());
        let actual = cdf(&d, x).unwrap();
        assert!((actual - expected).abs() < 1e-14, "x={x}, scale={scale}, actual={actual}, expected={expected}");
    }
}

#[test]
fn pareto_cdf_preserves_tiny_mass_next_to_its_minimum() {
    let d = Distribution::Pareto { minimum: 1.0, shape: 1e-4 };
    let x = 1.0_f64.next_up();
    let expected = -(-1e-4 * (x - 1.0).ln_1p()).exp_m1();
    let actual = cdf(&d, x).unwrap();
    assert!((actual / expected - 1.0).abs() < 1e-14, "actual={actual}, expected={expected}");
}

#[test]
fn a_mixture_of_point_masses_has_only_its_declared_atom_quantiles() {
    let d = Distribution::Mixture {
        components: vec![Distribution::Constant { value: 0.0 }, Distribution::Constant { value: 1.0 }],
        weights: vec![0.5, 0.5],
    };
    for (p, expected) in [(0.1, 0.0), (0.5, 0.0), (0.75, 1.0), (0.999, 1.0)] {
        assert_eq!(quantile(&d, p).unwrap(), expected, "p={p}");
    }
}

#[test]
fn cvar_retains_subnormal_point_mass_and_rounds_subnormal_averages_once() {
    let tiny = f64::from_bits(1);
    assert_eq!(solvercraft_math::risk::upper_tail_cvar(&[tiny, tiny], 0.0).unwrap(), tiny);
    assert_eq!(solvercraft_math::risk::upper_tail_cvar(&[tiny, 2.0 * tiny], 0.0).unwrap(), 2.0 * tiny);
}
