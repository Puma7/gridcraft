//! Percentile elicitation with explicit claims, independent of vendor argument-order errata.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution as D, Result, SolverError};
fn domain(message: &str) -> SolverError {
    SolverError::new("elicitation_domain", message)
}
fn verify(d: D, claims: &[(f64, f64)], span: f64) -> Result<D> {
    crate::validate_distribution(&d)?;
    let prepared = crate::PreparedDistribution::new(&d)?;
    for (p, expected) in claims {
        let actual = prepared.quantile(*p)?;
        let tolerance = (span * 2e-8).max(expected.abs() * 16.0 * f64::EPSILON);
        if (actual - expected).abs() > tolerance {
            return Err(SolverError::new(
                "elicitation_precision",
                "Constructed distribution cannot preserve supplied percentiles within floating precision.",
            ));
        }
    }
    Ok(d)
}
/// Bottom/median/top are the quantiles at t/2, 1/2, 1-t/2. The frozen manual's
/// Myerson formula and prose disagree on positional argument labels; this API uses named semantics.
pub fn myerson_percentiles(bottom: f64, median: f64, top: f64, total_tail_probability: f64) -> Result<D> {
    if !bottom.is_finite()
        || !median.is_finite()
        || !top.is_finite()
        || bottom >= median
        || median >= top
        || !(top - bottom).is_finite()
        || !total_tail_probability.is_finite()
        || total_tail_probability <= 0.0
        || total_tail_probability >= 1.0
    {
        return Err(domain("Use increasing finite bottom/median/top and a total tail probability strictly between0 and1."));
    }
    let left = median - bottom;
    let right = top - median;
    let p = total_tail_probability * 0.5;
    let z = crate::distributions::inverse_normal(1.0 - p)?;
    let d = if left == right {
        D::Normal { mean: median, std_dev: right / z }
    } else {
        let log_h = crate::reference_families::log_ratio(right, left);
        let amplitude = if log_h > 0.0 { left / (-(-log_h).exp_m1()) } else { right / log_h.exp_m1() };
        if !amplitude.is_finite() || amplitude == 0.0 || !log_h.is_finite() {
            return Err(domain("Myerson affine/lognormal representation exceeds finite arithmetic."));
        }
        D::ShiftScale { base: Box::new(D::Lognormal { log_mean: 0.0, log_std_dev: log_h.abs() / z }), shift: median - amplitude, scale: amplitude }
    };
    verify(d, &[(p, bottom), (0.5, median), (1.0 - p, top)], top - bottom)
}
/// Unique triangular distribution with CDF(lower_value)=lower_probability,
/// CDF(upper_value)=upper_probability and the supplied interior mode. Endpoint0/1 claims are allowed.
pub fn triangular_from_percentiles(lower_value: f64, mode: f64, upper_value: f64, lower_probability: f64, upper_probability: f64) -> Result<D> {
    let p = lower_probability;
    let r = upper_probability;
    if !lower_value.is_finite()
        || !mode.is_finite()
        || !upper_value.is_finite()
        || lower_value >= mode
        || mode >= upper_value
        || !(upper_value - lower_value).is_finite()
        || !p.is_finite()
        || !r.is_finite()
        || p < 0.0
        || r > 1.0
        || p >= r
    {
        return Err(domain("Use finite lower<mode<upper and probabilities0<=p<r<=1."));
    }
    let left = mode - lower_value;
    let right = upper_value - mode;
    let magnitude = left.max(right);
    let l = left / magnitude;
    let rr = right / magnitude;
    let factors = |q: f64| -> (f64, f64) { ((q - p) / (q * (1.0 + (p / q).sqrt())), (r - q) / ((1.0 - q) * (1.0 + ((1.0 - r) / (1.0 - q)).sqrt()))) };
    let mut lo = p;
    let mut hi = r;
    for _ in 0..160 {
        let q = lo + 0.5 * (hi - lo);
        if q == lo || q == hi {
            break;
        }
        let (a, b) = factors(q);
        let residual = q * rr * a - (1.0 - q) * l * b;
        if residual < 0.0 { lo = q } else { hi = q }
    }
    let q = lo + 0.5 * (hi - lo);
    let (a, b) = factors(q);
    let low = if p == 0.0 { lower_value } else { mode - left / a };
    let high = if r == 1.0 { upper_value } else { mode + right / b };
    let d = D::Triangular { low, mode, high };
    crate::validate_distribution(&d)?;
    let prepared = crate::PreparedDistribution::new(&d)?;
    if (prepared.cdf(lower_value)? - p).abs() > 2e-10 || (prepared.cdf(upper_value)? - r).abs() > 2e-10 {
        return Err(SolverError::new("elicitation_precision", "Triangular claims are not resolved at finite precision."));
    }
    Ok(d)
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaryFamily {
    Uniform,
    Triangular,
    Pert,
}
/// Verified actual-bounds Vary convention (reference range_type2). Percentage/change
/// conventions are deliberately separate until their input-unit contract is unambiguous.
pub fn vary_actual_bounds(base: f64, low: f64, high: f64, family: VaryFamily) -> Result<D> {
    if !base.is_finite() || !low.is_finite() || !high.is_finite() || low > base || base > high {
        return Err(domain("Actual Vary bounds must contain the finite base value."));
    }
    let d = match family {
        VaryFamily::Uniform => D::Uniform { low, high },
        VaryFamily::Triangular => D::Triangular { low, mode: base, high },
        VaryFamily::Pert => D::BetaPert { low, mode: base, high, weight: 4.0 },
    };
    crate::validate_distribution(&d)?;
    Ok(d)
}
