//! Fixed-sample mean intervals with explicit sampling units and assumptions.
use crate::OnlineMoments;
use serde::{Deserialize, Serialize};
use solvercraft_types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicateDesign {
    LatinHypercube,
    RandomizedSobol,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicateEstimate {
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    /// Sobol's randomization nonce. Legacy LHS has no nonce and requires zero.
    pub replicate: u64,
    pub mean: f64,
    pub trials: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeanInterval {
    pub mean: f64,
    pub lower: f64,
    pub upper: f64,
    pub standard_error: f64,
    pub confidence: f64,
    pub independent_units: u64,
    pub sampling_design: String,
    pub assumptions: String,
    pub critical_value_method: String,
    /// This API neither chooses a stopping time nor claims sequential coverage.
    pub fixed_sample_only: bool,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_interval", message)
}
fn calculate(values: &[f64], confidence: f64, design: &str, assumptions: &str) -> Result<MeanInterval> {
    if !(2..=1_000_000).contains(&values.len())
        || !confidence.is_finite()
        || confidence <= 0.0
        || confidence >= 1.0
        || values.iter().any(|x| !x.is_finite())
    {
        return Err(invalid("A fixed interval requires 2..1000000 finite independent units and confidence in (0,1)"));
    }
    let mut stats = OnlineMoments::default();
    for value in values {
        stats.push(*value)?;
    }
    let summary = stats.summary()?;
    let variance = summary.variance.ok_or_else(|| invalid("Sample variance is not available"))?;
    let standard_error = variance.sqrt() / (values.len() as f64).sqrt();
    let probability = 0.5 + confidence * 0.5;
    if probability >= 1.0 {
        return Err(invalid("Confidence is too close to one for finite tail evaluation"));
    }
    let degrees = (values.len() - 1) as f64;
    let (critical, critical_value_method) = if degrees <= 10000.0 {
        (
            solvercraft_math::quantile(&Distribution::StudentT { degrees_of_freedom: degrees, location: 0.0, scale: 1.0 }, probability)?,
            "student_t_inverse_cdf",
        )
    } else {
        // Explicit large-df Cornish-Fisher approximation; the incomplete-beta
        // implementation deliberately restricts its degrees-of-freedom range.
        let z = solvercraft_math::quantile(&Distribution::Normal { mean: 0.0, std_dev: 1.0 }, probability)?;
        let z2 = z * z;
        let inv = 1.0 / degrees;
        let value = z
            + z * (z2 + 1.0) * inv / 4.0
            + z * (5.0 * z2 * z2 + 16.0 * z2 + 3.0) * inv * inv / 96.0
            + z * (3.0 * z2 * z2 * z2 + 19.0 * z2 * z2 + 17.0 * z2 - 15.0) * inv * inv * inv / 384.0;
        (value, "student_t_cornish_fisher_order_3_df_above_10000")
    };
    let half_width = critical * standard_error;
    let lower = summary.mean - half_width;
    let upper = summary.mean + half_width;
    // Preserve nonzero uncertainty at the scale of a large location. Without
    // outward rounding, both endpoints can round back to the sample mean.
    let lower = if half_width > 0.0 { lower.next_down() } else { lower };
    let upper = if half_width > 0.0 { upper.next_up() } else { upper };
    if !standard_error.is_finite() || !half_width.is_finite() || !lower.is_finite() || !upper.is_finite() {
        return Err(SolverError::new("numerical_range", "Mean interval exceeds finite arithmetic"));
    }
    Ok(MeanInterval {
        mean: summary.mean,
        lower,
        upper,
        standard_error,
        confidence,
        independent_units: values.len() as u64,
        sampling_design: design.into(),
        assumptions: assumptions.into(),
        critical_value_method: critical_value_method.into(),
        fixed_sample_only: true,
    })
}
/// The usual Student-t interval is exact for IID Gaussian observations and an
/// approximation under a suitable finite-variance CLT for other distributions.
/// Above 10000 degrees of freedom the critical value uses a disclosed
/// third-order Cornish-Fisher approximation, including for Gaussian output.
/// It is not valid merely because an arbitrary simulation returned these values.
pub fn iid_mean_interval(values: &[f64], confidence: f64) -> Result<MeanInterval> {
    calculate(
        values,
        confidence,
        "iid_monte_carlo",
        "IID units; Gaussian Student-t model or finite-variance CLT approximation; critical_value_method records numerical approximation; no optional stopping",
    )
}
/// Treat each independently randomized full simulation as one statistical unit.
/// Pointwise LHS/Sobol samples are never treated as IID observations here.
pub fn replicate_mean_interval(estimates: &[ReplicateEstimate], design: ReplicateDesign, confidence: f64) -> Result<MeanInterval> {
    if !(2..=4096).contains(&estimates.len()) || estimates.iter().any(|e| e.trials == 0 || !e.mean.is_finite()) {
        return Err(invalid("Need 2..4096 valid independently randomized replicate means"));
    }
    let trials = estimates.first().ok_or_else(|| invalid("No replicates"))?.trials;
    if estimates.iter().any(|e| e.trials != trials) {
        return Err(invalid("Replicates must have the same preselected sample size"));
    }
    let mut identities = std::collections::BTreeSet::new();
    for estimate in estimates {
        if design == ReplicateDesign::LatinHypercube && estimate.replicate != 0 {
            return Err(invalid("LHS randomization uses the actual seed only; replicate nonce must be zero"));
        }
        if !identities.insert((estimate.seed, estimate.replicate)) {
            return Err(invalid("Duplicate randomization identity is not an independent replicate"));
        }
    }
    let means: Vec<f64> = estimates.iter().map(|e| e.mean).collect();
    let design = match design {
        ReplicateDesign::LatinHypercube => "independent_latin_hypercube_replicates",
        ReplicateDesign::RandomizedSobol => "independent_randomized_sobol_replicates",
    };
    calculate(
        &means,
        confidence,
        design,
        "Independent randomizations with equal fixed sample sizes; Student-t approximation across replicate means; no pointwise IID or optional-stopping claim",
    )
}
