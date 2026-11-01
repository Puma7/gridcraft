//! Keelin's original 2016 Metalog basis, globally certified subset.
//! OLS is a quantile/CDF least-squares fit, never an MLE or a Johnson moment fit.
//! Primary reference: https://doi.org/10.1287/deca.2016.0338 .
use serde::{Deserialize, Serialize};
use solvercraft_types::{Result, SolverError};
pub const MAX_TERMS: usize = 30;
pub const MAX_FIT_CELLS: usize = 1_000_000;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metalog {
    pub coefficients: Vec<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonotonicityCertificate {
    pub derivative_in_logit_lower_bound: f64,
    pub method: String,
}
#[derive(Debug, Clone)]
pub struct PreparedMetalog {
    model: Metalog,
    certificate: MonotonicityCertificate,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetalogFit {
    pub model: Metalog,
    pub observations: usize,
    pub latent_rmse: f64,
    pub method: String,
    pub certificate: MonotonicityCertificate,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() { Ok(value) } else { Err(error("numerical_range", "Metalog arithmetic is not representable.")) }
}
fn validate_bounds(lower: Option<f64>, upper: Option<f64>) -> Result<()> {
    if lower.is_some_and(|v| !v.is_finite())
        || upper.is_some_and(|v| !v.is_finite())
        || lower.zip(upper).is_some_and(|(l, u)| l >= u || !(u - l).is_finite())
    {
        return Err(error("metalog_domain", "Metalog bounds must be finite, ordered and have a representable width."));
    }
    if lower.is_none() && upper.is_some() {
        return Err(error("unsupported_metalog_bounds", "Upper-only bounds are outside the frozen PSI Metalog convention."));
    }
    Ok(())
}
fn softplus(x: f64) -> f64 {
    if x > 0.0 { x + (-x).exp().ln_1p() } else { x.exp().ln_1p() }
}
fn logistic(z: f64) -> (f64, f64) {
    let tail = (-z.abs()).exp();
    let p = if z >= 0.0 { 1.0 / (1.0 + tail) } else { tail / (1.0 + tail) };
    (p, tail / (1.0 + tail).powi(2))
}
fn log_difference(a: f64, b: f64) -> Result<f64> {
    if !a.is_finite() || !b.is_finite() || a <= b {
        return Err(error("metalog_domain", "Value must lie strictly inside Metalog support."));
    }
    let difference = a - b;
    if difference.is_finite() {
        finite(difference.ln())
    } else {
        let scale = a.abs().max(b.abs());
        finite(scale.ln() + (a / scale - b / scale).ln())
    }
}
fn latent_target(x: f64, lower: Option<f64>, upper: Option<f64>) -> Result<f64> {
    match (lower, upper) {
        (None, None) => finite(x),
        (Some(l), None) => log_difference(x, l),
        (Some(l), Some(u)) => finite(log_difference(x, l)? - log_difference(u, x)?),
        _ => Err(error("unsupported_metalog_bounds", "Upper-only Metalog is unsupported.")),
    }
}
fn add(sum: &mut f64, correction: &mut f64, value: f64) -> Result<()> {
    let next = finite(*sum + value)?;
    *correction = finite(*correction + if sum.abs() >= value.abs() { (*sum - next) + value } else { (value - next) + *sum })?;
    *sum = next;
    Ok(())
}
fn basis(index: usize, z: f64, t: f64, v: f64) -> (f64, f64) {
    match index {
        0 => (1.0, 0.0),
        1 => (z, 1.0),
        2 => (t * z, t + v * z),
        3 => (t, v),
        _ => {
            let number = index + 1;
            let power = if number % 2 == 1 { (number - 1) / 2 } else { number / 2 - 1 };
            let tp = t.powi(power as i32);
            let derivative = power as f64 * t.powi(power as i32 - 1) * v;
            if number % 2 == 1 { (tp, derivative) } else { (tp * z, tp + derivative * z) }
        }
    }
}
fn latent(coefficients: &[f64], z: f64) -> Result<(f64, f64)> {
    latent_parts(coefficients, z, false)
}
fn latent_parts(coefficients: &[f64], z: f64, skip_location: bool) -> Result<(f64, f64)> {
    let (p, v) = logistic(z);
    let t = p - 0.5;
    let mut value = 0.0;
    let mut vc = 0.0;
    let mut derivative = 0.0;
    let mut dc = 0.0;
    for (index, coefficient) in coefficients.iter().enumerate() {
        if index == 0 && skip_location {
            continue;
        }
        let (b, d) = basis(index, z, t, v);
        let product = finite(coefficient * b)?;
        add(&mut value, &mut vc, product)?;
        add(&mut value, &mut vc, coefficient.mul_add(b, -product))?;
        let product = finite(coefficient * d)?;
        add(&mut derivative, &mut dc, product)?;
        add(&mut derivative, &mut dc, coefficient.mul_add(d, -product))?;
    }
    Ok((finite(value + vc)?, finite(derivative + dc)?))
}
impl PreparedMetalog {
    pub fn new(model: &Metalog) -> Result<Self> {
        if !(2..=MAX_TERMS).contains(&model.coefficients.len()) || model.coefficients.iter().any(|a| !a.is_finite()) {
            return Err(error("metalog_domain", "Metalog requires2..30 finite coefficients in original2016 order."));
        }
        validate_bounds(model.lower, model.upper)?;
        let a2 = model.coefficients.get(1).copied().ok_or_else(|| error("metalog_domain", "Missing logistic scale coefficient."))?;
        let mut penalty = 0.0_f64;
        for (index, a) in model.coefficients.iter().enumerate().skip(2) {
            let number = index + 1;
            let bound = match number {
                3 => 0.75,
                4 => 0.25,
                _ => {
                    let k = if number % 2 == 1 { (number - 1) / 2 } else { number / 2 - 1 };
                    let d = k as f64 * 0.5_f64.powi(k as i32 - 1) * 0.25;
                    if number % 2 == 1 { d } else { 0.5_f64.powi(k as i32) + d }
                }
            };
            // Positive a4 contributes a4*p*(1-p)>=0, so need not be subtracted.
            let magnitude = if number == 4 && *a >= 0.0 { 0.0 } else { a.abs() };
            penalty = finite(penalty + finite(magnitude * bound)?.next_up())?.next_up();
        }
        let margin = finite(a2 - penalty)?.next_down();
        if margin <= 0.0 {
            return Err(error(
                "metalog_uncertified",
                "Coefficients fail the sufficient global derivative certificate; a probability grid is not a proof.",
            ));
        }
        Ok(Self {
            model: model.clone(),
            certificate: MonotonicityCertificate {
                derivative_in_logit_lower_bound: margin,
                method: "global logit derivative dominance; |z|p(1-p)<=1/4; outward-rounded coefficient bounds".into(),
            },
        })
    }
    pub fn certificate(&self) -> &MonotonicityCertificate {
        &self.certificate
    }
    pub fn model(&self) -> &Metalog {
        &self.model
    }
    /// Exact finite support endpoints are returned at probabilities0/1;
    /// unbounded endpoints and unrepresentable interior values are errors.
    pub fn quantile(&self, p: f64) -> Result<f64> {
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return Err(error("metalog_domain", "Probability must lie in[0,1]."));
        }
        if p == 0.0 {
            return self.model.lower.ok_or_else(|| error("unbounded_quantile", "Metalog lower quantile is unbounded."));
        }
        if p == 1.0 {
            return self.model.upper.ok_or_else(|| error("unbounded_quantile", "Metalog upper quantile is unbounded."));
        }
        let z = p.ln() - (-p).ln_1p();
        let (m, _) = latent(&self.model.coefficients, z)?;
        let value = match (self.model.lower, self.model.upper) {
            (None, None) => m,
            (Some(l), None) => finite(l + m.exp())?,
            (Some(l), Some(u)) => {
                let s = logistic(m).0;
                finite(l + (u - l) * s)?
            }
            _ => return Err(error("unsupported_metalog_bounds", "Upper-only bounds unsupported.")),
        };
        if !value.is_finite() || self.model.lower.is_some_and(|l| value <= l) || self.model.upper.is_some_and(|u| value >= u) {
            return Err(error("numerical_range", "Interior Metalog quantile overflows or rounds to a support endpoint."));
        }
        Ok(value)
    }
    fn inverse_latent(&self, x: f64) -> Result<f64> {
        let target = latent_target(x, self.model.lower, self.model.upper)?;
        let median = *self.model.coefficients.first().ok_or_else(|| error("metalog_domain", "Missing location coefficient."))?;
        let target = finite(target - median)?;
        let (mut lo, mut hi) = if target >= 0.0 { (0.0, 1.0) } else { (-1.0, 0.0) };
        let mut bracketed = false;
        for _ in 0..1024 {
            let a = latent_parts(&self.model.coefficients, lo, true)?.0;
            let b = latent_parts(&self.model.coefficients, hi, true)?.0;
            if a <= target && target <= b {
                bracketed = true;
                break;
            }
            if target < a {
                lo = finite(lo * 2.0)?;
            } else {
                hi = finite(hi * 2.0)?;
            }
        }
        if !bracketed {
            return Err(error("numerical_range", "Could not bracket the Metalog inverse within bounded work."));
        }
        for _ in 0..192 {
            let middle = 0.5 * lo + 0.5 * hi;
            if middle == lo || middle == hi {
                return Ok(middle);
            }
            let value = latent_parts(&self.model.coefficients, middle, true)?.0;
            if value == target {
                return Ok(middle);
            }
            if value < target {
                lo = middle;
            } else {
                hi = middle;
            }
        }
        Ok(0.5 * lo + 0.5 * hi)
    }
    pub fn cdf(&self, x: f64) -> Result<f64> {
        if !x.is_finite() {
            return Err(error("metalog_domain", "CDF requires a finite argument."));
        }
        if self.model.lower.is_some_and(|l| x <= l) {
            return Ok(0.0);
        }
        if self.model.upper.is_some_and(|u| x >= u) {
            return Ok(1.0);
        }
        Ok(logistic(self.inverse_latent(x)?).0)
    }
    /// Like the other scalar log densities, returns -infinity outside support.
    pub fn log_density(&self, x: f64) -> Result<f64> {
        if !x.is_finite() {
            return Err(error("metalog_domain", "Density requires a finite argument."));
        }
        if self.model.lower.is_some_and(|l| x <= l) || self.model.upper.is_some_and(|u| x >= u) {
            return Ok(f64::NEG_INFINITY);
        }
        let z = self.inverse_latent(x)?;
        let (m, dz) = latent(&self.model.coefficients, z)?;
        if dz <= 0.0 {
            return Err(error("numerical_range", "Rounded Metalog derivative is not positive."));
        }
        let log_transform = match (self.model.lower, self.model.upper) {
            (None, None) => 0.0,
            (Some(_), None) => m,
            (Some(l), Some(u)) => (u - l).ln() - softplus(m) - softplus(-m),
            _ => return Err(error("unsupported_metalog_bounds", "Upper-only bounds unsupported.")),
        };
        finite(-softplus(z) - softplus(-z) - dz.ln() - log_transform)
    }
    pub fn density(&self, x: f64) -> Result<f64> {
        let log = self.log_density(x)?;
        if log == f64::NEG_INFINITY { Ok(0.0) } else { finite(log.exp()) }
    }
}

/// Householder QR OLS fit to CDF point pairs. The candidate must pass the same
/// global monotonicity certificate as a directly supplied coefficient vector.
pub fn fit_cdf(terms: usize, x_values: &[f64], y_values: &[f64], lower: Option<f64>, upper: Option<f64>) -> Result<MetalogFit> {
    validate_bounds(lower, upper)?;
    let m = x_values.len();
    let n = terms;
    if !(2..=MAX_TERMS).contains(&n) || m < n || m != y_values.len() || m.checked_mul(n).is_none_or(|v| v > MAX_FIT_CELLS) {
        return Err(error("metalog_fit_limit", "Fit requires2..30 terms, matching m>=terms pairs, at most1million matrix cells."));
    }
    if x_values.iter().any(|v| !v.is_finite())
        || x_values.windows(2).any(|p| p[0] > p[1])
        || y_values.iter().any(|p| !p.is_finite() || *p <= 0.0 || *p >= 1.0)
        || y_values.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(error("metalog_fit_domain", "CDF observations require ordered finite values and strictly increasing interior probabilities."));
    }
    let targets = x_values.iter().map(|x| latent_target(*x, lower, upper)).collect::<Result<Vec<_>>>()?;
    let center = targets.get(m / 2).copied().ok_or_else(|| error("metalog_fit_domain", "Missing CDF observations."))?;
    let centered = targets.iter().map(|x| finite(x - center)).collect::<Result<Vec<_>>>()?;
    let scale = centered.iter().fold(0.0_f64, |v, x| v.max(x.abs())).max(1.0);
    let mut matrix = vec![0.0; m * n];
    let mut rhs: Vec<_> = centered.iter().map(|x| x / scale).collect();
    for (row, p) in y_values.iter().enumerate() {
        let z = p.ln() - (-p).ln_1p();
        for col in 0..n {
            matrix[row * n + col] = basis(col, z, *p - 0.5, p * (1.0 - p)).0;
        }
    }
    // Matrix dimensions/product and loops above establish every flat index.
    let mut largest_diagonal = 0.0_f64;
    for k in 0..n {
        let norm = (k..m).fold(0.0_f64, |a, row| a.hypot(matrix[row * n + k]));
        if !norm.is_finite() || norm <= 64.0 * f64::EPSILON * (m as f64).sqrt() {
            return Err(error("metalog_fit_conditioning", "CDF design matrix is rank deficient or numerically unresolved."));
        }
        let alpha = if matrix[k * n + k] >= 0.0 { -norm } else { norm };
        let mut v: Vec<_> = (k..m).map(|row| matrix[row * n + k]).collect();
        if let Some(first) = v.first_mut() {
            *first -= alpha;
        }
        let vnorm = v.iter().fold(0.0_f64, |a, x| a.hypot(*x));
        for item in &mut v {
            *item /= vnorm;
        }
        for col in k..n {
            let dot = 2.0 * v.iter().enumerate().map(|(i, v)| v * matrix[(k + i) * n + col]).sum::<f64>();
            for (i, v) in v.iter().enumerate() {
                matrix[(k + i) * n + col] -= dot * v;
            }
        }
        let dot = 2.0 * v.iter().enumerate().map(|(i, v)| v * rhs[k + i]).sum::<f64>();
        for (i, v) in v.iter().enumerate() {
            rhs[k + i] -= dot * v;
        }
        matrix[k * n + k] = alpha;
        largest_diagonal = largest_diagonal.max(alpha.abs());
        if alpha.abs() <= largest_diagonal * 1e-12 {
            return Err(error("metalog_fit_conditioning", "CDF design matrix diagonal ratio is too small."));
        }
    }
    let mut coefficients = vec![0.0; n];
    for k in (0..n).rev() {
        let sum = ((k + 1)..n).map(|j| matrix[k * n + j] * coefficients[j]).sum::<f64>();
        coefficients[k] = finite((rhs[k] - sum) / matrix[k * n + k])?;
    }
    for coefficient in &mut coefficients {
        *coefficient = finite(*coefficient * scale)?;
    }
    if let Some(location) = coefficients.first_mut() {
        *location = finite(*location + center)?;
    }
    let model = Metalog { coefficients, lower, upper };
    let prepared = PreparedMetalog::new(&model)?;
    let mut residual_norm = 0.0_f64;
    for (p, target) in y_values.iter().zip(targets) {
        let z = p.ln() - (-p).ln_1p();
        let location = model.coefficients.first().copied().ok_or_else(|| error("metalog_fit_domain", "Missing fitted location."))?;
        residual_norm = residual_norm.hypot(finite(latent_parts(&model.coefficients, z, true)?.0 - finite(target - location)?)?);
    }
    Ok(MetalogFit {
        model,
        observations: m,
        latent_rmse: finite(residual_norm / (m as f64).sqrt())?,
        method: "ordinary_least_squares_cdf_householder_qr; not maximum likelihood".into(),
        certificate: prepared.certificate.clone(),
    })
}
/// Symmetric-percentile triplet at probabilities[p,0.5,1-p], source299.
pub fn fit_spt(lower: Option<f64>, upper: Option<f64>, quantiles: [f64; 3], probability: f64) -> Result<MetalogFit> {
    if !probability.is_finite() || probability <= 0.0 || probability >= 0.5 {
        return Err(error("metalog_fit_domain", "SPT tail probability must be in(0,0.5)."));
    }
    let mut fit = fit_cdf(3, &quantiles, &[probability, 0.5, 1.0 - probability], lower, upper)?;
    fit.method = "three_quantile_symmetric_percentile_triplet; not maximum likelihood".into();
    Ok(fit)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentFitResult {
    pub distribution: solvercraft_types::Distribution,
    pub family: String,
    pub matched_skewness: f64,
    pub matched_kurtosis: f64,
    pub scope: String,
}
/// Verified normal and shifted/reflected lognormal moment subfamilies of the
/// Johnson system. General SU/SB inversion remains explicitly unsupported.
/// This is separate from the Metalog OLS/SPT functions above.
pub fn moment_fit_supported(mean: f64, std_dev: f64, skewness: f64, kurtosis: f64) -> Result<MomentFitResult> {
    use solvercraft_types::Distribution as D;
    if !mean.is_finite() || !std_dev.is_finite() || std_dev <= 0.0 || !skewness.is_finite() || !kurtosis.is_finite() || kurtosis <= 1.0 {
        return Err(error("moment_domain", "Moment fitting requires finite mean/skewness, positive stddev and Pearson kurtosis>1."));
    }
    let (distribution, family, matched_skewness, matched_kurtosis) = if skewness == 0.0 && kurtosis == 3.0 {
        (D::Normal { mean, std_dev }, "normal", 0.0, 3.0)
    } else {
        // For lognormal, skew=v^3+3v, v=sqrt(exp(sigma^2)-1).
        let v = finite(2.0 * ((0.5 * skewness.abs()).asinh() / 3.0).sinh())?;
        if v == 0.0 {
            return Err(error("unsupported_moment_family", "Requested moments require a general Johnson SU/SB fit."));
        }
        let w = finite(v * v)?;
        let expected = finite(3.0 + w * (16.0 + w * (15.0 + w * (6.0 + w))))?;
        if (expected - kurtosis).abs() > 1e-8 * (1.0 + kurtosis.abs()) {
            return Err(error(
                "unsupported_moment_family",
                "Requested moments are outside the verified normal/lognormal subfamilies; general Johnson SU/SB inversion is unavailable.",
            ));
        }
        let base_mean = finite(std_dev / v)?;
        let log_variance = w.ln_1p();
        let sign = if skewness > 0.0 { 1.0 } else { -1.0 };
        let shift = finite(mean - sign * base_mean)?;
        let log_mean = finite(base_mean.ln() - 0.5 * log_variance)?;
        let log_std_dev = log_variance.sqrt();
        let recovered_base_mean = finite((log_mean + 0.5 * log_std_dev * log_std_dev).exp())?;
        let recovered_mean = finite(shift + sign * recovered_base_mean)?;
        let recovered_std = finite((log_mean + 0.5 * log_std_dev * log_std_dev + 0.5 * (log_std_dev * log_std_dev).exp_m1().ln()).exp())?;
        if (recovered_mean - mean).abs() > 1e-8 * (1.0 + mean.abs()) || (recovered_std / std_dev - 1.0).abs() > 1e-8 {
            return Err(error("numerical_range", "Rounded shifted lognormal parameters do not preserve the requested mean and standard deviation."));
        }
        let base = D::Lognormal { log_mean, log_std_dev };
        (D::ShiftScale { base: Box::new(base), shift, scale: sign }, "shifted_or_reflected_lognormal", sign * v * (w + 3.0), expected)
    };
    crate::validate_distribution(&distribution)?;
    Ok(MomentFitResult{distribution,family:family.into(),matched_skewness,matched_kurtosis,scope:"Only independently checked normal and shifted/reflected lognormal moment subfamilies; not a general PsiMomentFit/Johnson SU/SB implementation.".into()})
}
