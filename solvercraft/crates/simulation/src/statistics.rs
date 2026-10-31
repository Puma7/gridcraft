//! Online moments and elementary, explicitly labelled distribution fits.
use solvercraft_types::{Distribution, Result, SampleSummary, SolverError};

#[derive(Debug, Clone, Default)]
pub struct OnlineMoments {
    count: u64,
    mean: f64,
    m2: f64,
    minimum: f64,
    maximum: f64,
}
impl OnlineMoments {
    pub fn push(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(SolverError::new("non_finite_sample", "Observation must be finite."));
        }
        let count = self.count.checked_add(1).ok_or_else(|| SolverError::new("sample_limit", "Observation count overflow."))?;
        if self.count == 0 {
            self.count = count;
            self.mean = value;
            self.minimum = value;
            self.maximum = value;
            return Ok(());
        }
        let delta = value - self.mean;
        let mean = self.mean + delta / count as f64;
        let m2 = self.m2 + delta * (value - mean);
        if !mean.is_finite() || !m2.is_finite() {
            return Err(SolverError::new("numerical_range", "Online moments exceed finite arithmetic; rescale the data."));
        }
        self.count = count;
        self.mean = mean;
        self.m2 = m2;
        self.minimum = self.minimum.min(value);
        self.maximum = self.maximum.max(value);
        Ok(())
    }
    pub fn summary(&self) -> Result<SampleSummary> {
        if self.count == 0 {
            return Err(SolverError::new("empty_samples", "At least one observation is required."));
        }
        Ok(SampleSummary {
            count: self.count,
            mean: self.mean,
            variance: if self.count > 1 { Some((self.m2 / (self.count - 1) as f64).max(0.0)) } else { None },
            minimum: self.minimum,
            maximum: self.maximum,
            quantiles: Vec::new(),
            value_at_risk_95: None,
            conditional_value_at_risk_95: None,
        })
    }
}
fn moments(values: &[f64]) -> Result<OnlineMoments> {
    if values.is_empty() {
        return Err(SolverError::new("empty_samples", "Fitting requires observations."));
    }
    let mut stats = OnlineMoments::default();
    for value in values {
        stats.push(*value)?;
    }
    Ok(stats)
}
/// Maximum-likelihood normal fit (population, not sample, variance).
/// This returns parameters, not a goodness-of-fit claim or a model-selection score.
pub fn fit_normal(values: &[f64]) -> Result<Distribution> {
    let stats = moments(values)?;
    Ok(Distribution::Normal { mean: stats.mean, std_dev: (stats.m2 / stats.count as f64).max(0.0).sqrt() })
}
/// Maximum-likelihood finite uniform endpoints; equal data produce a point mass interval.
pub fn fit_uniform(values: &[f64]) -> Result<Distribution> {
    let stats = moments(values)?;
    Ok(Distribution::Uniform { low: stats.minimum, high: stats.maximum })
}
/// Maximum-likelihood exponential rate for nonnegative, uncensored, unweighted observations.
pub fn fit_exponential(values: &[f64]) -> Result<Distribution> {
    if values.iter().any(|x| *x < 0.0) {
        return Err(SolverError::new("fit_domain", "Exponential fitting requires nonnegative observations."));
    }
    let stats = moments(values)?;
    let rate = 1.0 / stats.mean;
    if stats.mean <= 0.0 || !rate.is_finite() {
        return Err(SolverError::new("fit_domain", "Exponential fitting requires a finite positive sample mean."));
    }
    Ok(Distribution::Exponential { rate })
}
