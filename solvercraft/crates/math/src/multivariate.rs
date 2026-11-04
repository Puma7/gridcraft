//! Covariance-parameterized multivariate normal and latent-lognormal draws.
//! Mean/covariance here are not a rank-correlation request.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Result, RunControl, SolverError};
pub const STREAM_VERSION: &str = "solvercraft-multivariate-normal-v1/addressed-mix64-v2";
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorAddress {
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub binding: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub trial: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MultivariateLimits {
    pub max_dimensions: usize,
    pub max_memory_bytes: u64,
}
impl Default for MultivariateLimits {
    fn default() -> Self {
        Self { max_dimensions: 128, max_memory_bytes: 16 * 1024 * 1024 }
    }
}
#[derive(Debug, Clone)]
pub struct PreparedMultivariateNormal {
    means: Vec<f64>,
    stds: Vec<f64>,
    lower: Vec<Vec<f64>>,
}
fn domain(message: &str) -> SolverError {
    SolverError::new("multivariate_domain", message)
}
fn reserve<T>(v: &mut Vec<T>, count: usize) -> Result<()> {
    v.try_reserve_exact(count).map_err(|_| SolverError::new("allocation", "Cannot allocate multivariate storage."))
}
fn cancelled(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(SolverError::new("cancelled", "Multivariate draw cancelled.")) } else { Ok(()) }
}
impl PreparedMultivariateNormal {
    /// Singular PSD covariance is permitted explicitly; a zero variance requires a zero covariance row.
    pub fn new(means: &[f64], covariance: &[Vec<f64>], limits: &MultivariateLimits, control: &dyn RunControl) -> Result<Self> {
        let n = means.len();
        if n == 0
            || n > 128
            || n > limits.max_dimensions
            || means.iter().any(|x| !x.is_finite())
            || covariance.len() != n
            || covariance.iter().any(|r| r.len() != n || r.iter().any(|x| !x.is_finite()))
        {
            return Err(domain("Use1..128 finite means with a square finite covariance matrix."));
        }
        if covariance.iter().enumerate().any(|(i, row)| row[i] < 0.0) {
            return Err(domain("Variances must be nonnegative."));
        }
        let memory = (n as u64)
            .checked_mul(n as u64)
            .and_then(|v| v.checked_mul(16))
            .and_then(|v| v.checked_add(n as u64 * 128 + 4096))
            .ok_or_else(|| domain("Covariance memory overflow."))?;
        if memory > limits.max_memory_bytes {
            return Err(SolverError::new("memory_limit", "Multivariate preparation exceeds memory budget."));
        }
        cancelled(control)?;
        let mut stds = Vec::new();
        reserve(&mut stds, n)?;
        for (i, row) in covariance.iter().enumerate() {
            stds.push(row[i].sqrt());
        }
        let mut correlation = Vec::new();
        reserve(&mut correlation, n)?;
        for i in 0..n {
            cancelled(control)?;
            let mut row = Vec::new();
            reserve(&mut row, n)?;
            for j in 0..n {
                let value = if i == j {
                    1.0
                } else if stds[i] == 0.0 || stds[j] == 0.0 {
                    if covariance[i][j] != 0.0 || covariance[j][i] != 0.0 {
                        return Err(domain("Zero-variance covariance rows must be exactly zero."));
                    }
                    0.0
                } else {
                    if covariance[i][i] == covariance[j][j] && covariance[i][j].abs() == covariance[i][i] {
                        covariance[i][j].signum()
                    } else {
                        covariance[i][j] / stds[i] / stds[j]
                    }
                };
                row.push(value);
            }
            correlation.push(row);
        }
        let lower = crate::copula::cholesky(&correlation, n, false)?;
        cancelled(control)?;
        let mut copied = Vec::new();
        reserve(&mut copied, n)?;
        copied.extend_from_slice(means);
        Ok(Self { means: copied, stds, lower })
    }
    pub fn dimensions(&self) -> usize {
        self.means.len()
    }
    /// Linear map of explicitly supplied independent standard-normal coordinates.
    pub fn transform(&self, independent_normals: &[f64], logarithmic: bool) -> Result<Vec<f64>> {
        if independent_normals.len() != self.dimensions() || independent_normals.iter().any(|x| !x.is_finite()) {
            return Err(domain("One finite standard-normal coordinate per dimension is required."));
        }
        let mut output = Vec::new();
        reserve(&mut output, self.dimensions())?;
        for i in 0..self.dimensions() {
            let mut value = if self.stds[i] == 0.0 {
                self.means[i]
            } else {
                let z = crate::special::finite_sum((0..=i).map(|j| self.lower[i][j] * independent_normals[j]))?;
                self.stds[i].mul_add(z, self.means[i])
            };
            if logarithmic {
                value = value.exp();
                if value == 0.0 {
                    return Err(SolverError::new("numerical_range", "Lognormal vector coordinate underflows."));
                }
            }
            if !value.is_finite() {
                return Err(SolverError::new("numerical_range", "Multivariate coordinate exceeds finite arithmetic."));
            }
            output.push(value);
        }
        Ok(output)
    }
    pub fn sample(&self, address: VectorAddress, logarithmic: bool, control: &dyn RunControl) -> Result<Vec<f64>> {
        cancelled(control)?;
        let mut normals = Vec::new();
        reserve(&mut normals, self.dimensions())?;
        for i in 0..self.dimensions() {
            normals.push(crate::distributions::inverse_normal(crate::addressed_uniform(
                address.seed,
                address.binding,
                address.trial,
                0x4d56_0000 + i as u64,
            ))?);
        }
        self.transform(&normals, logarithmic)
    }
}
