//! Empirical lower-inverse-CDF quantiles and upper loss-tail expected shortfall.
use solvercraft_types::{Result, SolverError};

fn sorted(values: &[f64]) -> Result<Vec<f64>> {
    if values.is_empty() {
        return Err(SolverError::new("empty_samples", "At least one observation is required."));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(SolverError::new("non_finite_sample", "Observations must be finite."));
    }
    let mut result = Vec::new();
    result.try_reserve_exact(values.len()).map_err(|_| SolverError::new("allocation", "Cannot allocate sample ordering."))?;
    result.extend_from_slice(values);
    result.sort_unstable_by(f64::total_cmp);
    Ok(result)
}
fn probability(value: f64, include_one: bool) -> Result<()> {
    if !value.is_finite() || value < 0.0 || if include_one { value > 1.0 } else { value >= 1.0 } {
        return Err(SolverError::new("probability", if include_one { "Probability must lie in [0,1]." } else { "Confidence must lie in [0,1)." }));
    }
    Ok(())
}
/// Quantile of the equally-weighted empirical measure, using its lower inverse CDF.
/// It is an observed value, with no linear interpolation between observations.
pub fn empirical_quantile(values: &[f64], p: f64) -> Result<f64> {
    probability(p, true)?;
    let values = sorted(values)?;
    quantile_sorted(&values, p)
}
/// Input must be sorted and finite. Checked independently for safe public use.
pub fn quantile_sorted(values: &[f64], p: f64) -> Result<f64> {
    probability(p, true)?;
    if values.is_empty() {
        return Err(SolverError::new("empty_samples", "At least one observation is required."));
    }
    if values.iter().any(|v| !v.is_finite()) || values.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(SolverError::new("sample_order", "Observations must be finite and sorted."));
    }
    let index = ((p * values.len() as f64).ceil() as usize).saturating_sub(1).min(values.len() - 1);
    values.get(index).copied().ok_or_else(|| SolverError::new("sample_index", "Invalid quantile index."))
}
/// CVaR / Expected Shortfall for the upper tail of LOSS values.
/// Integrates the quantile function over [alpha,1], including fractional boundary mass.
/// [0,0,0,10] at alpha=0.5 returns 5, rather than averaging all values >= VaR.
pub fn upper_tail_cvar(values: &[f64], alpha: f64) -> Result<f64> {
    probability(alpha, false)?;
    let values = sorted(values)?;
    cvar_sorted(&values, alpha)
}
/// Checked sorted form avoiding an additional copy.
pub fn cvar_sorted(values: &[f64], alpha: f64) -> Result<f64> {
    probability(alpha, false)?;
    if values.is_empty() {
        return Err(SolverError::new("empty_samples", "At least one observation is required."));
    }
    if values.iter().any(|v| !v.is_finite()) || values.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(SolverError::new("sample_order", "Observations must be finite and sorted."));
    }
    let tail_mass = (1.0 - alpha) * values.len() as f64;
    // Exact binary upscaling keeps normalization from rounding every term of
    // a subnormal tail to zero. The weighted sum stays below 2^1022 here.
    let magnitude = values[0].abs().max(values[values.len() - 1].abs());
    let scale = if magnitude < 1.0 { f64::from_bits(2045_u64 << 52) } else { 1.0 };
    let mut remaining = tail_mass;
    let mut mean = 0.0;
    let mut correction = 0.0;
    // Normalize before multiplying, retaining subnormal constants in tiny tails.
    // Neumaier summation also retains small terms between cancelling large losses.
    for value in values.iter().rev() {
        if remaining <= 0.0 {
            break;
        }
        let weight = remaining.min(1.0);
        let term = (*value * scale) * (weight / tail_mass);
        let next = mean + term;
        correction += if mean.abs() >= term.abs() { (mean - next) + term } else { (term - next) + mean };
        mean = next;
        remaining -= weight;
    }
    let mean = (mean + correction) / scale;
    if !mean.is_finite() {
        return Err(SolverError::new("numerical_range", "Expected shortfall exceeds finite arithmetic."));
    }
    Ok(mean)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discrete_boundary_mass_is_fractional() -> Result<()> {
        let losses = [0.0, 0.0, 0.0, 10.0];
        assert_eq!(empirical_quantile(&losses, 0.5)?, 0.0);
        assert_eq!(upper_tail_cvar(&losses, 0.5)?, 5.0);
        assert_eq!(upper_tail_cvar(&losses, 0.75)?, 10.0);
        assert_eq!(upper_tail_cvar(&losses, 0.0)?, 2.5);
        assert_eq!(upper_tail_cvar(&[-10.0, -4.0], 0.5)?, -4.0);
        Ok(())
    }
    #[test]
    fn empirical_convention_and_domains() -> Result<()> {
        assert_eq!(empirical_quantile(&[4.0, 1.0, 3.0, 2.0], 0.0)?, 1.0);
        assert_eq!(empirical_quantile(&[4.0, 1.0, 3.0, 2.0], 0.5)?, 2.0);
        assert_eq!(empirical_quantile(&[4.0, 1.0, 3.0, 2.0], 1.0)?, 4.0);
        assert!(empirical_quantile(&[], 0.5).is_err());
        assert!(empirical_quantile(&[f64::NAN], 0.5).is_err());
        assert!(upper_tail_cvar(&[1.0], 1.0).is_err());
        assert!(quantile_sorted(&[2.0, 1.0], 0.5).is_err());
        Ok(())
    }
    #[test]
    fn cancellation_and_subnormal_tails_keep_small_terms() -> Result<()> {
        let result = upper_tail_cvar(&[1e17, -1.0, -1e17], 0.0)?;
        assert!((result + 1.0 / 3.0).abs() < 1e-15);
        assert_eq!(upper_tail_cvar(&[1e-310], 1.0 - f64::EPSILON)?, 1e-310);
        Ok(())
    }
}
