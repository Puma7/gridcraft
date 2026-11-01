//! Verified PSI parameter conversions onto workbook-independent distributions.
//! These constructors do not implement worksheet property/default semantics.
//! Public 2025 Q1 source pages and unresolved cases are in reference-catalog.json.
use solvercraft_types::{Distribution as D, Result, SolverError};
const MAX_SEGMENTS: usize = 32;
const MAX_KNOTS: usize = 100_000;
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn valid(d: D) -> Result<D> {
    crate::validate_distribution(&d)?;
    Ok(d)
}
fn interval(low: f64, high: f64) -> Result<f64> {
    let width = high - low;
    if !low.is_finite() || !high.is_finite() || low >= high || !width.is_finite() {
        return Err(error("psi_domain", "Require finite low<high with a representable width."));
    }
    Ok(width)
}
fn normalized(weights: &[f64]) -> Result<Vec<f64>> {
    if weights.is_empty() || weights.len() > MAX_KNOTS || weights.iter().any(|w| !w.is_finite() || *w < 0.0) {
        return Err(error("psi_domain", "Weights must be bounded in count, finite and nonnegative."));
    }
    let max = weights.iter().copied().fold(0.0, f64::max);
    if max == 0.0 {
        return Err(error("psi_domain", "At least one weight must be positive."));
    }
    let scaled: Vec<_> = weights.iter().map(|w| w / max).collect();
    if weights.iter().zip(&scaled).any(|(original, scaled)| *original > 0.0 && *scaled == 0.0) {
        return Err(error("numerical_range", "Positive probability weight underflows normalization."));
    }
    let mut sum = 0.0;
    let mut correction = 0.0;
    for w in &scaled {
        let y = *w - correction;
        let next = sum + y;
        correction = (next - sum) - y;
        sum = next;
    }
    let probabilities: Vec<_> = scaled.iter().map(|w| w / sum).collect();
    if scaled.iter().zip(&probabilities).any(|(original, p)| *original > 0.0 && *p == 0.0) {
        return Err(error("numerical_range", "Positive normalized probability is not representable."));
    }
    Ok(probabilities)
}
/// PsiLogNormal(mean,stddev), source293-294. Strict PSI stddev>0.
pub fn lognormal_arithmetic(mean: f64, std_dev: f64) -> Result<D> {
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(error("psi_domain", "PsiLogNormal requires a positive arithmetic standard deviation."));
    }
    crate::parameterization::lognormal_from_mean_std(mean, std_dev)
}
/// PsiLogNorm2(log_mean,log_stddev), distinct from PsiLogNormal.
pub fn lognormal_log(log_mean: f64, log_std_dev: f64) -> Result<D> {
    if !log_std_dev.is_finite() || log_std_dev <= 0.0 {
        return Err(error("psi_domain", "PsiLogNorm2 requires a positive log standard deviation."));
    }
    valid(D::Lognormal { log_mean, log_std_dev })
}
/// PsiExponential(beta) takes a mean/scale, whereas the DTO takes a rate.
pub fn exponential_mean(mean: f64) -> Result<D> {
    if !mean.is_finite() || mean <= 0.0 {
        return Err(error("psi_domain", "PsiExponential requires a positive finite mean."));
    }
    let rate = 1.0 / mean;
    if !rate.is_finite() || rate <= 0.0 {
        return Err(error("numerical_range", "Exponential reciprocal rate cannot be represented."));
    }
    valid(D::Exponential { rate })
}
/// PsiErf(h) is Normal(0,1/(sqrt(2)*h)), source278-279.
pub fn erf(h: f64) -> Result<D> {
    if !h.is_finite() || h <= 0.0 {
        return Err(error("psi_domain", "PsiErf requires h>0."));
    }
    let std_dev = std::f64::consts::FRAC_1_SQRT_2 / h;
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(error("numerical_range", "ERF scale is not representable."));
    }
    valid(D::Normal { mean: 0.0, std_dev })
}
/// PsiBetaGen(alpha1,alpha2,a,b), source271-272.
pub fn beta_general(alpha: f64, beta: f64, low: f64, high: f64) -> Result<D> {
    let width = interval(low, high)?;
    valid(D::ShiftScale { base: Box::new(D::Beta { alpha, beta }), shift: low, scale: width })
}
/// PsiBetaSubj(a,mode,mean,b), source272-273. Symmetric mean=mode at
/// the interval center does not uniquely identify Beta concentration and errors.
pub fn beta_subjective(low: f64, mode: f64, mean: f64, high: f64) -> Result<D> {
    let width = interval(low, high)?;
    if !mode.is_finite() || !mean.is_finite() || mode <= low || mode >= high || mean <= low || mean >= high {
        return Err(error("psi_domain", "Beta subjective mode and mean must lie strictly inside the interval."));
    }
    let t = (mean - low) / width;
    let z = (mode - low) / width;
    if t == z {
        return Err(error("psi_ambiguous_parameters", "Mean equals mode; Beta concentration is not uniquely determined or is inconsistent."));
    }
    let total = (2.0 * z - 1.0) / (z - t);
    let alpha = t * total;
    let beta = (1.0 - t) * total;
    if !alpha.is_finite() || !beta.is_finite() || alpha <= 1.0 || beta <= 1.0 {
        return Err(error("psi_domain", "Supplied mean and interior mode do not identify a unimodal Beta distribution."));
    }
    beta_general(alpha, beta, low, high)
}
/// PsiDblTriang(low,mode,high,p), source277-278. Strict p in (0,1).
pub fn double_triangular(low: f64, mode: f64, high: f64, left_probability: f64) -> Result<D> {
    interval(low, high)?;
    if !mode.is_finite() || mode <= low || mode >= high || !left_probability.is_finite() || left_probability <= 0.0 || left_probability >= 1.0 {
        return Err(error("psi_domain", "Double triangular requires low<mode<high and 0<p<1."));
    }
    valid(D::DoubleTriangular { low, mode, high, left_probability })
}
/// Minimum extreme-value reflection, PsiMinExtreme(m,s), source300.
pub fn minimum_extreme(location: f64, scale: f64) -> Result<D> {
    valid(D::ShiftScale { base: Box::new(D::Gumbel { location: 0.0, scale }), shift: location, scale: -1.0 })
}
fn piecewise(low: f64, high: f64, xs: &[f64], probabilities: &[f64], strict_probabilities: bool) -> Result<D> {
    interval(low, high)?;
    if xs.len() != probabilities.len() || xs.len() > MAX_KNOTS {
        return Err(error("psi_limit", "Cumulative values/probabilities must have matching bounded lengths."));
    }
    let mut previous_x = low;
    let mut previous_p = 0.0;
    let mut components = Vec::with_capacity(MAX_SEGMENTS);
    let mut weights = Vec::with_capacity(MAX_SEGMENTS);
    for (index, (&x, &p)) in xs.iter().zip(probabilities).enumerate() {
        if !x.is_finite()
            || !p.is_finite()
            || x < low
            || x > high
            || !(0.0..=1.0).contains(&p)
            || (index > 0 && x <= previous_x)
            || (index > 0 && (p < previous_p || strict_probabilities && p == previous_p))
        {
            return Err(error("psi_domain", "Cumulative knots must increase inside bounds, with increasing valid probabilities."));
        }
        if p < previous_p {
            return Err(error("psi_domain", "Cumulative probabilities cannot decrease."));
        }
        append_segment(previous_x, x, p - previous_p, &mut components, &mut weights)?;
        previous_x = x;
        previous_p = p;
    }
    append_segment(previous_x, high, 1.0 - previous_p, &mut components, &mut weights)?;
    valid(D::Mixture { components, weights })
}
fn append_segment(low: f64, high: f64, mass: f64, components: &mut Vec<D>, weights: &mut Vec<f64>) -> Result<()> {
    if mass == 0.0 {
        return Ok(());
    }
    if low >= high {
        return Err(error("psi_ambiguous_endpoint", "Positive probability on a zero-width boundary is not a verified PsiCumul signature case."));
    }
    if components.len() >= MAX_SEGMENTS {
        return Err(error("psi_limit", "Piecewise alias supports at most32 nonzero-mass intervals."));
    }
    components.push(D::Uniform { low, high });
    weights.push(mass);
    Ok(())
}
/// PsiCumul(a,b,xs,ps): piecewise-linear CDF with implicit(a,0),(b,1).
/// At most32 nonzero-mass intervals. Unresolved endpoint atoms are rejected.
pub fn cumulative(low: f64, high: f64, xs: &[f64], probabilities: &[f64]) -> Result<D> {
    piecewise(low, high, xs, probabilities, true)
}
/// Descending cumulative probabilities are converted explicitly, not resampled.
pub fn cumulative_descending(low: f64, high: f64, xs: &[f64], descending: &[f64]) -> Result<D> {
    if descending.len() > MAX_KNOTS {
        return Err(error("psi_limit", "Too many cumulative knots."));
    }
    let probabilities: Vec<_> = descending.iter().map(|p| 1.0 - p).collect();
    cumulative(low, high, xs, &probabilities)
}
/// PsiGeneral(a,b,xs,weights), source332: normalized cumulative weights at xs.
pub fn general(low: f64, high: f64, xs: &[f64], weights: &[f64]) -> Result<D> {
    if xs.len() != weights.len() {
        return Err(error("psi_domain", "General distribution values and weights must match."));
    }
    let weights = normalized(weights)?;
    let mut total = 0.0;
    let mut probabilities = Vec::with_capacity(weights.len());
    for weight in weights {
        let next = (total + weight).min(1.0);
        if weight > 0.0 && next == total {
            return Err(error("numerical_range", "A positive cumulative probability increment is not representable."));
        }
        total = next;
        probabilities.push(total);
    }
    if let Some(last) = probabilities.last_mut() {
        *last = 1.0;
    }
    piecewise(low, high, xs, &probabilities, false)
}
/// PsiHistogram(a,b,weights), source333: equal-width uniform bin densities.
pub fn histogram(low: f64, high: f64, weights: &[f64]) -> Result<D> {
    let width = interval(low, high)?;
    if weights.len() > MAX_SEGMENTS {
        return Err(error("psi_limit", "Histogram alias supports at most32 bins."));
    }
    let normalized = normalized(weights)?;
    let count = normalized.len();
    let mut components = Vec::with_capacity(count);
    let mut masses = Vec::with_capacity(count);
    for (i, mass) in normalized.into_iter().enumerate() {
        let a = if i == 0 { low } else { low + width * (i as f64 / count as f64) };
        let b = if i + 1 == count { high } else { low + width * ((i + 1) as f64 / count as f64) };
        if a >= b {
            return Err(error("numerical_range", "Histogram bin endpoints collapse at this scale."));
        }
        append_segment(a, b, mass, &mut components, &mut masses)?;
    }
    valid(D::Mixture { components, weights: masses })
}
/// PsiDiscrete(xs,weights), source330: normalized weighted finite support.
pub fn discrete(values: &[f64], weights: &[f64]) -> Result<D> {
    if values.len() != weights.len() || values.len() > MAX_KNOTS {
        return Err(error("psi_limit", "Discrete values/weights must match and remain within100000 entries."));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("psi_domain", "Discrete support must be finite."));
    }
    let normalized = normalized(weights)?;
    valid(D::Categorical { values: values.to_vec(), weights: normalized })
}
/// PsiDisUniform(xs), source331, distinct from an integer-range uniform.
pub fn discrete_uniform_values(values: &[f64]) -> Result<D> {
    if values.len() > 1_000_000 {
        return Err(error("psi_limit", "At most1million equal-weight values."));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("psi_domain", "Equal-weight support must be finite."));
    }
    valid(D::Empirical { values: values.to_vec() })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8 * (1.0 + a.abs()), "{a} != {b}");
    }
    #[test]
    fn lognormal_conventions_and_exponential_mean_are_distinct() {
        let arithmetic = lognormal_arithmetic(10.0, 2.0).unwrap();
        let log = lognormal_log(10.0, 2.0).unwrap();
        assert!((crate::quantile(&arithmetic, 0.5).unwrap() - crate::quantile(&log, 0.5).unwrap()).abs() > 1000.0);
        if let D::Lognormal { log_mean, log_std_dev } = arithmetic {
            close((log_mean + 0.5 * log_std_dev.powi(2)).exp(), 10.0);
            close(((log_std_dev.powi(2)).exp() - 1.0).sqrt() * 10.0, 2.0);
        } else {
            panic!("expected lognormal");
        }
        close(crate::quantile(&exponential_mean(3.0).unwrap(), 0.5).unwrap(), 3.0 * 2.0_f64.ln());
        assert!(lognormal_arithmetic(1.0, 0.0).is_err());
        assert!(exponential_mean(f64::from_bits(1)).is_err());
    }
    #[test]
    fn rescaled_beta_subjective_moments_and_erf_have_independent_oracles() {
        let d = beta_general(1.0, 1.0, -3.0, 7.0).unwrap();
        close(crate::quantile(&d, 0.25).unwrap(), -0.5);
        // Beta(2,5): mean2/7,mode1/5, then affine map[-2,8].
        let subjective = beta_subjective(-2.0, 0.0, -2.0 + 20.0 / 7.0, 8.0).unwrap();
        let expected = beta_general(2.0, 5.0, -2.0, 8.0).unwrap();
        for p in [0.1, 0.5, 0.9] {
            close(crate::quantile(&subjective, p).unwrap(), crate::quantile(&expected, p).unwrap());
        }
        assert_eq!(beta_subjective(0.0, 0.5, 0.5, 1.0).unwrap_err().code, "psi_ambiguous_parameters");
        let erf = erf(0.5).unwrap();
        close(crate::cdf(&erf, 0.0).unwrap(), 0.5);
        close(crate::quantile(&erf, 0.5).unwrap(), 0.0);
    }
    #[test]
    fn double_triangle_matches_its_two_independent_cdf_branches() {
        let d = double_triangular(0.0, 2.0, 5.0, 0.3).unwrap();
        close(crate::cdf(&d, 1.0).unwrap(), 0.3 * 0.25);
        close(crate::cdf(&d, 3.0).unwrap(), 1.0 - 0.7 * (2.0_f64 / 3.0).powi(2));
        assert!(double_triangular(0.0, 2.0, 5.0, 0.0).is_err());
    }
    #[test]
    fn custom_cumulative_is_not_equal_weight_empirical_interpolation() {
        let d = cumulative(0.0, 10.0, &[2.0, 8.0], &[0.1, 0.8]).unwrap();
        close(crate::cdf(&d, 1.0).unwrap(), 0.05);
        close(crate::cdf(&d, 5.0).unwrap(), 0.45);
        close(crate::quantile(&d, 0.1).unwrap(), 2.0);
        let descending = cumulative_descending(0.0, 10.0, &[2.0, 8.0], &[0.9, 0.2]).unwrap();
        close(crate::cdf(&descending, 5.0).unwrap(), 0.45);
        assert!(cumulative(0.0, 10.0, &[0.0], &[0.2]).is_err());
        assert!(cumulative(0.0, 10.0, &[2.0, 8.0], &[0.2, 0.2]).is_err());
        assert!(cumulative(0.0, 10.0, &[2.0, 2.0], &[0.2, 0.8]).is_err());
    }
    #[test]
    fn histogram_and_general_preserve_normalized_probability_mass() {
        let d = histogram(0.0, 4.0, &[1e308, 0.0, 1e308, 0.0]).unwrap();
        close(crate::cdf(&d, 1.0).unwrap(), 0.5);
        close(crate::cdf(&d, 2.5).unwrap(), 0.75);
        let d = general(0.0, 10.0, &[2.0, 8.0], &[1.0, 3.0]).unwrap();
        close(crate::cdf(&d, 2.0).unwrap(), 0.25);
        close(crate::cdf(&d, 5.0).unwrap(), 0.625);
        close(crate::cdf(&d, 9.0).unwrap(), 1.0);
        assert!(histogram(0.0, 1.0, &vec![1.0; 33]).is_err());
        assert!(histogram(0.0, 1.0, &[0.0, 0.0]).is_err());
    }
    #[test]
    fn weighted_and_equal_support_lists_have_correct_atoms() {
        let d = discrete(&[-2.0, 7.0], &[1.0, 3.0]).unwrap();
        close(crate::cdf(&d, 0.0).unwrap(), 0.25);
        let d = discrete_uniform_values(&[-2.0, 7.0]).unwrap();
        close(crate::cdf(&d, 0.0).unwrap(), 0.5);
    }
    #[test]
    fn minimum_extreme_is_the_correct_gumbel_reflection() {
        let d = minimum_extreme(3.0, 2.0).unwrap();
        for p in [0.1, 0.5, 0.9] {
            close(crate::quantile(&d, p).unwrap(), 3.0 + 2.0 * (-(-p).ln_1p()).ln());
        }
    }
}
