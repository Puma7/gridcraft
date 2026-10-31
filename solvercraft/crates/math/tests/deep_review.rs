//! Independent deterministic numerical review cases, not tests of implementation details.

#[test]
fn triangular_scale_invariance_and_conditional_tail_probabilities() -> Result<()> {
    for width in [1e-200, 1.0, 1e200] {
        let distribution = Distribution::Triangular { low: 0.0, mode: width, high: 2.0 * width };
        assert!((quantile(&distribution, 0.5)? / width - 1.0).abs() < 1e-14);
        assert!((cdf(&distribution, width)? - 0.5).abs() < 1e-15);
    }
    let tiny_mode = Distribution::Triangular { low: 0.0, mode: 1e-200, high: 1.0 };
    assert!((cdf(&tiny_mode, 1e-200)? / 1e-200 - 1.0).abs() < 1e-14);
    assert!((quantile(&tiny_mode, 1e-200)? / 1e-200 - 1.0).abs() < 1e-14);
    Ok(())
}

#[test]
fn triangular_extreme_branch_ratios_remain_finite_and_correct() -> Result<()> {
    let left = Distribution::Triangular { low: 0.0, mode: 1e-200, high: 1e308 };
    assert_eq!(cdf(&left, 1e-200)?, 0.0); // Actual mass is below representable range.
    let right = Distribution::Triangular { low: -1e308, mode: -1e-200, high: 0.0 };
    assert_eq!(cdf(&right, -5e-201)?, 1.0);
    let right_triangle = Distribution::Triangular { low: 0.0, mode: 0.0, high: 1e308 };
    // For p=1e-300 the exact quantile is approximately width*p/2=5e7.
    assert!((quantile(&right_triangle, 1e-300)? / 5e7 - 1.0).abs() < 1e-14);
    Ok(())
}

#[test]
fn rng_v2_has_explicit_reproducibility_vectors() {
    for (seed, binding, trial, subcounter, expected_bits) in [
        (0, 0, 0, 0, 0x3fe8_ca02_87f8_714b_u64),
        (7, 2, 3, 0, 0x3feb_8ccd_b361_3f83_u64),
        (u64::MAX, u64::MAX, u64::MAX, u64::MAX, 0x3fe2_997a_2303_f54b_u64),
    ] {
        assert_eq!(addressed_uniform(seed, binding, trial, subcounter).to_bits(), expected_bits);
    }
}

#[test]
fn rng_v2_removes_alpha_v1_coordinate_cancellation_family() {
    let domain_delta = 0x1319_8a2e_0370_7344_u64 ^ 0xa409_3822_299f_31d0_u64;
    let values = (0_u64..32).map(|trial| addressed_uniform(7, domain_delta ^ trial, trial, 0).to_bits()).collect::<std::collections::BTreeSet<_>>();
    assert!(values.len() > 16, "Distinct addresses must not collapse to the alpha-v1 constant family");
    assert_eq!(solvercraft_math::SAMPLING_VERSION, "solvercraft-addressed-mix64-v2");
}

#[test]
fn positive_continuous_interior_quantile_underflow_is_explicit() -> Result<()> {
    for distribution in [Distribution::Exponential { rate: f64::MAX }, Distribution::Weibull { shape: 0.001, scale: f64::MIN_POSITIVE }] {
        assert!(quantile(&distribution, f64::MIN_POSITIVE).is_err_and(|error| error.code == "numerical_range"));
        assert_eq!(quantile(&distribution, 0.0)?, 0.0);
    }
    Ok(())
}

#[test]
fn normal_quantile_keeps_finite_answer_after_cancelling_large_terms() -> Result<()> {
    let distribution = Distribution::Normal { mean: -1e308, std_dev: 1e308 };
    let actual = quantile(&distribution, 0.977_249_868_051_820_8)?;
    assert!(actual.is_finite());
    assert!((actual / 1e308 - 1.0).abs() < 1e-7);
    Ok(())
}

use solvercraft_math::{PreparedDistribution, addressed_index, addressed_uniform, cdf, empirical_quantile, quantile, upper_tail_cvar};
use solvercraft_types::{Distribution, Result};

#[test]
fn normal_cdf_standardization_does_not_overflow_before_division() -> Result<()> {
    // The mathematical standardized argument is exactly 2, despite x-mean
    // overflowing in ordinary f64 arithmetic. Phi(2) is an independent reference.
    let distribution = Distribution::Normal { mean: -1e308, std_dev: 1e308 };
    let probability = cdf(&distribution, 1e308)?;
    assert!((probability - 0.977_249_868_051_820_8).abs() < 2e-7, "CDF={probability}");
    let mirrored = Distribution::Normal { mean: 1e308, std_dev: 1e308 };
    assert!((cdf(&mirrored, -1e308)? - 0.022_750_131_948_179_21).abs() < 2e-7);
    Ok(())
}

#[test]
fn weibull_cdf_preserves_finite_log_ratio_when_division_overflows_or_underflows() -> Result<()> {
    let expected = 1.0 - (-1.0_f64).exp();
    // shape*ln(x/scale) rounds to zero, so (x/scale)^shape rounds to one.
    // Forming x/scale first incorrectly produces infinity or zero.
    for (x, scale) in [(1e308, 1e-308), (1e-308, 1e308)] {
        let actual = cdf(&Distribution::Weibull { shape: 1e-308, scale }, x)?;
        assert!((actual - expected).abs() < 1e-12, "CDF={actual}, scale={scale}");
    }
    Ok(())
}

#[test]
fn lognormal_interior_quantile_does_not_silently_return_zero() {
    let distribution = Distribution::Lognormal { log_mean: -1000.0, log_std_dev: 1.0 };
    assert!(quantile(&distribution, 0.5).is_err_and(|error| error.code == "numerical_range"));
}

#[test]
fn maximum_seed_and_address_extremes_are_open_interval_and_repeatable() -> Result<()> {
    let addresses = [(0, 0, 0, 0), (u64::MAX, 0, 0, 0), (u64::MAX, u64::MAX, u64::MAX, u64::MAX), (17, 33, 1 << 63, 1 << 62)];
    for (seed, binding, trial, subcounter) in addresses {
        let value = addressed_uniform(seed, binding, trial, subcounter);
        assert!(value > 0.0 && value < 1.0);
        assert_eq!(value, addressed_uniform(seed, binding, trial, subcounter));
    }
    for upper in [1, 2, 3, 17, 257, usize::MAX] {
        assert!(addressed_index(u64::MAX, 9, 8, 0, upper)? < upper);
    }
    assert_eq!(addressed_index(u64::MAX, 1, 2, 0, 1)?, 0);
    assert!(addressed_index(1, 2, 3, 0, 0).is_err());
    Ok(())
}

#[test]
fn discrete_limits_are_explicit_and_degenerate_large_counts_remain_exact() -> Result<()> {
    assert!(
        PreparedDistribution::new(&Distribution::Binomial { trials: 10_001, probability: 0.5 })
            .is_err_and(|error| error.code == "distribution_limit")
    );
    assert!(PreparedDistribution::new(&Distribution::Poisson { rate: 512.000_001 }).is_err_and(|error| error.code == "distribution_limit"));
    assert_eq!(quantile(&Distribution::Binomial { trials: u32::MAX, probability: 1.0 }, 0.4)?, f64::from(u32::MAX));
    assert_eq!(quantile(&Distribution::Binomial { trials: u32::MAX, probability: 0.0 }, 1.0)?, 0.0);
    assert_eq!(quantile(&Distribution::Binomial { trials: 0, probability: 0.5 }, 1.0)?, 0.0);
    assert_eq!(quantile(&Distribution::Poisson { rate: 0.0 }, 1.0)?, 0.0);
    assert!(quantile(&Distribution::Poisson { rate: 1.0 }, 1.0).is_err());
    Ok(())
}

#[test]
fn binomial_small_exact_distribution_and_discrete_inverse_boundary() -> Result<()> {
    // Four independent Bernoulli(1/2): exact masses [1,4,6,4,1]/16.
    let distribution = PreparedDistribution::new(&Distribution::Binomial { trials: 4, probability: 0.5 })?;
    for (x, p) in [(0.0, 1.0 / 16.0), (1.0, 5.0 / 16.0), (2.0, 11.0 / 16.0), (3.0, 15.0 / 16.0), (4.0, 1.0)] {
        assert!((distribution.cdf(x)? - p).abs() < 1e-15);
        assert_eq!(distribution.quantile(p)?, x);
    }
    assert_eq!(distribution.quantile(0.0)?, 0.0);
    Ok(())
}

#[test]
fn discrete_quantiles_bracket_probabilities_at_large_allowed_parameters() -> Result<()> {
    for source in [
        Distribution::Binomial { trials: 10_000, probability: 0.001 },
        Distribution::Binomial { trials: 10_000, probability: 0.999 },
        Distribution::Poisson { rate: 512.0 },
    ] {
        let distribution = PreparedDistribution::new(&source)?;
        let mut previous = -1.0;
        for probability in [1e-12, 0.01, 0.5, 0.99, 1.0 - 1e-12] {
            let point = distribution.quantile(probability)?;
            assert_eq!(point, point.floor());
            assert!(point >= previous);
            assert!(distribution.cdf(point)? >= probability);
            if point > 0.0 {
                assert!(distribution.cdf(point - 1.0)? < probability);
            }
            previous = point;
        }
    }
    Ok(())
}

#[test]
fn degenerate_supports_and_boundary_cdfs_are_consistent() -> Result<()> {
    for distribution in [
        Distribution::Uniform { low: 7.0, high: 7.0 },
        Distribution::Normal { mean: 7.0, std_dev: 0.0 },
        Distribution::Triangular { low: 7.0, mode: 7.0, high: 7.0 },
        Distribution::Constant { value: 7.0 },
    ] {
        assert_eq!(quantile(&distribution, 0.0)?, 7.0);
        assert_eq!(quantile(&distribution, 1.0)?, 7.0);
        assert_eq!(cdf(&distribution, 6.0)?, 0.0);
        assert_eq!(cdf(&distribution, 7.0)?, 1.0);
        assert_eq!(cdf(&distribution, f64::NEG_INFINITY)?, 0.0);
        assert_eq!(cdf(&distribution, f64::INFINITY)?, 1.0);
    }
    Ok(())
}

#[test]
fn normal_tail_quantiles_match_independent_reference_constants() -> Result<()> {
    let distribution = Distribution::Normal { mean: 0.0, std_dev: 1.0 };
    for (p, reference) in [(1e-12, -7.034_483_825_301_131), (1e-100, -21.273_453_560_965_322), (1e-300, -37.047_096_299_361_2)] {
        let actual = quantile(&distribution, p)?;
        assert!((actual - reference).abs() < 1e-6, "p={p}, q={actual}");
    }
    assert!(quantile(&distribution, 0.0).is_err());
    assert!(quantile(&distribution, 1.0).is_err());
    Ok(())
}

#[test]
fn empirical_quantiles_do_not_interpolate_and_cvar_integrates_boundary_mass() -> Result<()> {
    let losses = [-7.0, 2.0, 2.0, 2.0, 11.0];
    assert_eq!(empirical_quantile(&losses, 0.0)?, -7.0);
    assert_eq!(empirical_quantile(&losses, 0.8)?, 2.0);
    assert_eq!(empirical_quantile(&losses, 1.0)?, 11.0);
    // Upper half contains 11, 2 and half of another 2.
    assert!((upper_tail_cvar(&losses, 0.5)? - 5.6).abs() < 1e-14);
    assert!((upper_tail_cvar(&[0.0, 0.0, 0.0, 10.0], 0.6)? - 6.25).abs() < 1e-14);
    assert_eq!(upper_tail_cvar(&[-3.0, -3.0, -3.0], 0.999)?, -3.0);
    assert_eq!(upper_tail_cvar(&[1e-310], 1.0 - f64::EPSILON)?, 1e-310);
    assert!((upper_tail_cvar(&[1e17, -1.0, -1e17], 0.0)? + 1.0 / 3.0).abs() < 1e-15);
    Ok(())
}

#[test]
fn weibull_extreme_shape_distinguishes_adjacent_values_near_scale() -> Result<()> {
    let scale = 1e308_f64;
    let distribution = Distribution::Weibull { shape: 1e308, scale };
    assert_eq!(cdf(&distribution, f64::from_bits(scale.to_bits() + 1))?, 1.0);
    assert_eq!(cdf(&distribution, f64::from_bits(scale.to_bits() - 1))?, 0.0);
    assert!((cdf(&distribution, scale)? - (1.0 - (-1.0_f64).exp())).abs() < 1e-15);
    Ok(())
}

#[test]
fn weibull_quantile_avoids_overflow_before_scale_restores_finite_answer() -> Result<()> {
    let distribution = Distribution::Weibull { shape: 0.001, scale: 1e-308 };
    let p = -(-2.1_f64).exp_m1();
    let result = quantile(&distribution, p)?;
    let expected = 165_689_403_301_917.47_f64;
    assert!(result.is_finite() && result > 0.0);
    assert!((result / expected - 1.0).abs() < 1e-12);
    assert!((cdf(&distribution, result)? - p).abs() < 1e-12);
    Ok(())
}
