//! Scaled online moments and elementary, explicitly labelled distribution fits.
use solvercraft_types::{Distribution, Result, SampleSummary, SolverError};

fn numerical_range() -> SolverError {
    SolverError::new("numerical_range", "Requested statistic exceeds finite arithmetic; rescale the data.")
}

/// An online accumulator with two independent numerical roles:
///
/// - Origin-centered, adaptively scaled Welford moments preserve small variance
///   around a large offset and avoid forming an overflowing squared deviation.
/// - Compensated sums in scaled and original units compute the reported mean
///   without cancelling an origin and an almost equal reconstructed offset,
///   or normalizing away a small residue between huge cancelling values.
///
/// A finite standard deviation does not imply a representable variance. The
/// latter remains an explicit error in summary(), rather than an infinite field
/// or a rounded zero for nonconstant observations.
#[derive(Debug, Clone, Default)]
pub struct OnlineMoments {
    count: u64,
    origin: f64,
    deviation_scale: f64,
    mean_delta: f64,
    mean_delta_correction: f64,
    m2_scaled: f64,
    m2_correction: f64,
    sum_scale: f64,
    sum_scaled: f64,
    sum_correction: f64,
    raw_sum: f64,
    raw_correction: f64,
    raw_sum_available: bool,
    scaled_sum_lost_term: bool,
    minimum: f64,
    maximum: f64,
}
fn compensated_add(sum: f64, correction: f64, term: f64) -> (f64, f64) {
    let adjusted = term - correction;
    let next = sum + adjusted;
    (next, (next - sum) - adjusted)
}
fn scaled_sum_add(scale: f64, sum: f64, correction: f64, value: f64) -> (f64, f64, f64) {
    let new_scale = scale.max(value.abs());
    if new_scale == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let ratio = if scale == 0.0 { 0.0 } else { scale / new_scale };
    let previous = sum * ratio;
    let previous_correction = correction * ratio;
    let term = value / new_scale;
    let next = previous + term;
    let correction = previous_correction + if previous.abs() >= term.abs() { (previous - next) + term } else { (term - next) + previous };
    (new_scale, next, correction)
}
impl OnlineMoments {
    pub fn push(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(SolverError::new("non_finite_sample", "Observation must be finite."));
        }
        let count = self.count.checked_add(1).ok_or_else(|| SolverError::new("sample_limit", "Observation count overflow."))?;
        if self.count == 0 {
            self.count = count;
            self.origin = value;
            self.sum_scale = value.abs();
            self.sum_scaled = if value == 0.0 { 0.0 } else { value / self.sum_scale };
            self.raw_sum = value;
            self.raw_sum_available = true;
            self.minimum = value;
            self.maximum = value;
            return Ok(());
        }
        let difference = value - self.origin;
        let new_scale = if difference.is_finite() {
            self.deviation_scale.max(difference.abs())
        } else {
            self.deviation_scale.max(value.abs()).max(self.origin.abs())
        };
        let mut mean_delta = self.mean_delta;
        let mut mean_correction = self.mean_delta_correction;
        let mut m2 = self.m2_scaled;
        let mut m2_correction = self.m2_correction;
        if new_scale > 0.0 {
            let ratio = if self.deviation_scale == 0.0 { 0.0 } else { self.deviation_scale / new_scale };
            mean_delta *= ratio;
            mean_correction *= ratio;
            m2 *= ratio * ratio;
            m2_correction *= ratio * ratio;
            let centered = if difference.is_finite() { difference / new_scale } else { value / new_scale - self.origin / new_scale };
            let delta = centered - mean_delta;
            (mean_delta, mean_correction) = compensated_add(mean_delta, mean_correction, delta / count as f64);
            let increment = delta * (centered - mean_delta);
            (m2, m2_correction) = compensated_add(m2, m2_correction, increment);
        }
        let (sum_scale, sum_scaled, sum_correction) = scaled_sum_add(self.sum_scale, self.sum_scaled, self.sum_correction, value);
        // Normalizing vastly different finite magnitudes can erase a term
        // which becomes significant after cancellation. Retain an independent
        // Neumaier sum in original units whenever its partial sums fit.
        let raw_sum = self.raw_sum + value;
        let raw_correction =
            self.raw_correction + if self.raw_sum.abs() >= value.abs() { (self.raw_sum - raw_sum) + value } else { (value - raw_sum) + self.raw_sum };
        let raw_sum_available = self.raw_sum_available && raw_sum.is_finite() && raw_correction.is_finite();
        let ratio = if self.sum_scale == 0.0 { 0.0 } else { self.sum_scale / sum_scale };
        let scaled_sum_lost_term = self.scaled_sum_lost_term
            || (value != 0.0 && value / sum_scale == 0.0)
            || (self.sum_scaled != 0.0 && self.sum_scaled * ratio == 0.0)
            || (self.sum_correction != 0.0 && self.sum_correction * ratio == 0.0);
        if !mean_delta.is_finite() || !m2.is_finite() || m2 < 0.0 || !sum_scaled.is_finite() || !sum_correction.is_finite() {
            return Err(numerical_range());
        }
        // Commit only after all arithmetic checks, so a rejected push is atomic.
        self.count = count;
        self.deviation_scale = new_scale;
        self.mean_delta = mean_delta;
        self.mean_delta_correction = mean_correction;
        self.m2_scaled = m2;
        self.m2_correction = m2_correction;
        self.sum_scale = sum_scale;
        self.sum_scaled = sum_scaled;
        self.sum_correction = sum_correction;
        self.raw_sum = if raw_sum_available { raw_sum } else { 0.0 };
        self.raw_correction = if raw_sum_available { raw_correction } else { 0.0 };
        self.raw_sum_available = raw_sum_available;
        self.scaled_sum_lost_term = scaled_sum_lost_term;
        self.minimum = self.minimum.min(value);
        self.maximum = self.maximum.max(value);
        Ok(())
    }
    fn mean(&self) -> Result<f64> {
        if self.count == 0 {
            return Err(SolverError::new("empty_samples", "At least one observation is required."));
        }
        let value = if self.deviation_scale <= 0.5 * self.origin.abs() {
            // For a tightly clustered large offset, reconstructing from the
            // centered mean avoids normalizing away the low-order location.
            // This branch cannot suffer large opposite-sign cancellation.
            self.deviation_scale.mul_add(self.mean_delta, self.origin)
        } else if self.raw_sum_available {
            // Divide each finite expansion component before their final sum;
            // the sum itself could overflow although the mean is finite.
            self.raw_sum / self.count as f64 + self.raw_correction / self.count as f64
        } else {
            // For data crossing zero, prefer the compensated raw sum rather
            // than subtracting a large origin from a reconstructed offset.
            if self.scaled_sum_lost_term {
                return Err(numerical_range());
            }
            self.sum_scale * ((self.sum_scaled + self.sum_correction) / self.count as f64)
        };
        if value.is_finite() { Ok(value) } else { Err(numerical_range()) }
    }
    fn population_std_dev(&self) -> Result<f64> {
        let value = self.deviation_scale * (self.m2_scaled / self.count as f64).sqrt();
        if value.is_finite() && !(value == 0.0 && self.m2_scaled > 0.0) { Ok(value) } else { Err(numerical_range()) }
    }
    pub fn summary(&self) -> Result<SampleSummary> {
        let mean = self.mean()?;
        let variance = if self.count > 1 {
            let value = ((self.m2_scaled / (self.count - 1) as f64) * self.deviation_scale) * self.deviation_scale;
            if !value.is_finite() || (value == 0.0 && self.m2_scaled > 0.0) {
                return Err(numerical_range());
            }
            Some(value)
        } else {
            None
        };
        Ok(SampleSummary {
            count: self.count,
            mean,
            variance,
            minimum: self.minimum,
            maximum: self.maximum,
            quantiles: Vec::new(),
            value_at_risk_95: None,
            conditional_value_at_risk_95: None,
        })
    }
}
fn checked_observations(values: &[f64]) -> Result<()> {
    if values.is_empty() {
        return Err(SolverError::new("empty_samples", "Fitting requires observations."));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(SolverError::new("non_finite_sample", "Observation must be finite."));
    }
    Ok(())
}
fn moments(values: &[f64]) -> Result<OnlineMoments> {
    checked_observations(values)?;
    let mut stats = OnlineMoments::default();
    for value in values {
        stats.push(*value)?;
    }
    Ok(stats)
}
/// Maximum-likelihood normal fit (population, not sample, standard deviation).
/// This returns parameters, not a goodness-of-fit claim or a model-selection score.
pub fn fit_normal(values: &[f64]) -> Result<Distribution> {
    let stats = moments(values)?;
    Ok(Distribution::Normal { mean: stats.mean()?, std_dev: stats.population_std_dev()? })
}
/// Maximum-likelihood finite uniform endpoints; equal data produce a point mass interval.
/// Endpoint fitting does not require a representable sample variance.
pub fn fit_uniform(values: &[f64]) -> Result<Distribution> {
    checked_observations(values)?;
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !(high - low).is_finite() {
        return Err(numerical_range());
    }
    Ok(Distribution::Uniform { low, high })
}
/// Maximum-likelihood exponential rate for nonnegative, uncensored, unweighted observations.
/// The mean is computed without the irrelevant second moment.
pub fn fit_exponential(values: &[f64]) -> Result<Distribution> {
    checked_observations(values)?;
    if values.iter().any(|value| *value < 0.0) {
        return Err(SolverError::new("fit_domain", "Exponential fitting requires nonnegative observations."));
    }
    let mut scale = 0.0;
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        (scale, sum, correction) = scaled_sum_add(scale, sum, correction, *value);
    }
    let mean = scale * ((sum + correction) / values.len() as f64);
    let rate = 1.0 / mean;
    if mean <= 0.0 || !rate.is_finite() || rate <= 0.0 {
        return Err(SolverError::new("fit_domain", "Exponential fitting requires a representable positive sample mean and rate."));
    }
    Ok(Distribution::Exponential { rate })
}
