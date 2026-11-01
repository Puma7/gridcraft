//! Two-percentile parameterizations. These solve the named model exactly in real arithmetic;
//! rounded supplied percentiles and floating arithmetic impose the usual numerical limits.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution as D, Result, SolverError};
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantileFamily {
    Normal,
    Lognormal,
    Uniform,
    Weibull,
    Logistic,
    Laplace,
    Gumbel,
    LogLogistic,
}
/// Converts arithmetic mean/std to the underlying normal parameters. PsiLogNormal uses
/// this convention; PsiLogNorm2 and Distribution::Lognormal use log parameters directly.
pub fn lognormal_from_mean_std(mean: f64, std_dev: f64) -> Result<D> {
    if !mean.is_finite() || mean <= 0.0 || !std_dev.is_finite() || std_dev < 0.0 {
        return Err(SolverError::new("distribution_domain", "Lognormal arithmetic mean must be positive and standard deviation nonnegative."));
    }
    let log_ratio = if std_dev == 0.0 { f64::NEG_INFINITY } else { std_dev.ln() - mean.ln() };
    let log_variance = if std_dev == 0.0 { 0.0 } else { crate::reference_families::softplus(2.0 * log_ratio) };
    let log_std_dev = if log_ratio < -350.0 { log_ratio.exp() } else { log_variance.sqrt() };
    if std_dev > 0.0 && log_std_dev == 0.0 {
        return Err(SolverError::new("numerical_range", "Lognormal log standard deviation underflows finite arithmetic."));
    }
    let d = D::Lognormal { log_mean: mean.ln() - 0.5 * log_variance, log_std_dev };
    crate::validate_distribution(&d)?;
    Ok(d)
}
pub fn from_two_quantiles(family: QuantileFamily, first: (f64, f64), second: (f64, f64)) -> Result<D> {
    let (p1, x1) = first;
    let (p2, x2) = second;
    if !p1.is_finite()
        || !p2.is_finite()
        || p1 <= 0.0
        || p1 >= p2
        || p2 >= 1.0
        || !x1.is_finite()
        || !x2.is_finite()
        || x1 >= x2
        || !(x2 - x1).is_finite()
    {
        return Err(SolverError::new("percentile_domain", "Use two increasing finite values at increasing probabilities strictly inside (0,1)."));
    }
    let z = |p: f64| -> Result<f64> {
        Ok(match family {
            QuantileFamily::Normal | QuantileFamily::Lognormal => crate::distributions::inverse_normal(p)?,
            QuantileFamily::Uniform => p,
            QuantileFamily::Weibull => (-(-p).ln_1p()).ln(),
            QuantileFamily::Logistic | QuantileFamily::LogLogistic => p.ln() - (-p).ln_1p(),
            QuantileFamily::Laplace => {
                if p < 0.5 {
                    (2.0 * p).ln()
                } else {
                    -(2.0 * (1.0 - p)).ln()
                }
            }
            QuantileFamily::Gumbel => -(-p.ln()).ln(),
        })
    };
    let z1 = z(p1)?;
    let z2 = z(p2)?;
    let logarithmic = matches!(family, QuantileFamily::Lognormal | QuantileFamily::Weibull | QuantileFamily::LogLogistic);
    if logarithmic && x1 <= 0.0 {
        return Err(SolverError::new("percentile_domain", "This percentile family requires positive values."));
    }
    let y1 = if logarithmic { x1.ln() } else { x1 };
    let y2 = if logarithmic { x2.ln() } else { x2 };
    let slope = (y2 - y1) / (z2 - z1);
    let location = (-slope).mul_add(z1, y1);
    let d = match family {
        QuantileFamily::Normal => D::Normal { mean: location, std_dev: slope },
        QuantileFamily::Lognormal => D::Lognormal { log_mean: location, log_std_dev: slope },
        QuantileFamily::Uniform => D::Uniform { low: location, high: location + slope },
        QuantileFamily::Weibull => D::Weibull { shape: 1.0 / slope, scale: location.exp() },
        QuantileFamily::Logistic => D::Logistic { location, scale: slope },
        QuantileFamily::Laplace => D::Laplace { location, scale: slope },
        QuantileFamily::Gumbel => D::Gumbel { location, scale: slope },
        QuantileFamily::LogLogistic => D::LogLogistic { shape: 1.0 / slope, scale: location.exp() },
    };
    crate::validate_distribution(&d)?;
    Ok(d)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconstructs_families_from_two_independent_percentiles() {
        for (family, d) in [
            (QuantileFamily::Normal, D::Normal { mean: 2.0, std_dev: 3.0 }),
            (QuantileFamily::Lognormal, D::Lognormal { log_mean: 1.0, log_std_dev: 0.5 }),
            (QuantileFamily::Uniform, D::Uniform { low: -2.0, high: 8.0 }),
            (QuantileFamily::Weibull, D::Weibull { shape: 2.0, scale: 3.0 }),
            (QuantileFamily::Logistic, D::Logistic { location: -2.0, scale: 3.0 }),
            (QuantileFamily::Laplace, D::Laplace { location: 2.0, scale: 3.0 }),
            (QuantileFamily::Gumbel, D::Gumbel { location: 2.0, scale: 3.0 }),
            (QuantileFamily::LogLogistic, D::LogLogistic { shape: 2.0, scale: 3.0 }),
        ] {
            let p1 = 0.1;
            let p2 = 0.9;
            let reconstructed = from_two_quantiles(family, (p1, crate::quantile(&d, p1).unwrap()), (p2, crate::quantile(&d, p2).unwrap())).unwrap();
            for p in [0.05, 0.3, 0.5, 0.8, 0.95] {
                assert!((crate::quantile(&d, p).unwrap() - crate::quantile(&reconstructed, p).unwrap()).abs() < 1e-10);
            }
        }
        assert!(from_two_quantiles(QuantileFamily::Normal, (0.9, 1.0), (0.1, 2.0)).is_err());
    }
}
