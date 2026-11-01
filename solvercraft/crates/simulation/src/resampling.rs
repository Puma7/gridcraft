//! Immutable numeric resampling, intact-vector bootstrap, cell shuffle and trial replay.
//! SIP uses zero-based internal trial coordinates; SLURP's column selector is explicitly1-based.
use serde::{Deserialize, Serialize};
pub use solvercraft_math::multivariate::VectorAddress;
use solvercraft_types::{Result, RunControl, SolverError};
pub const STREAM_VERSION: &str = "solvercraft-resampling-v1/addressed-mix64-v2";
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorOrientation {
    Row,
    Column,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResamplingLimits {
    pub max_cells: usize,
    pub max_memory_bytes: u64,
}
impl Default for ResamplingLimits {
    fn default() -> Self {
        Self { max_cells: 1_000_000, max_memory_bytes: 64 * 1024 * 1024 }
    }
}
#[derive(Debug, Clone)]
pub struct PreparedValues {
    values: Vec<f64>,
    max_memory: u64,
}
#[derive(Debug, Clone)]
pub struct PreparedTable {
    rows: Vec<Vec<f64>>,
    columns: usize,
    cells: usize,
    bytes: u64,
    max_memory: u64,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn reserve<T>(v: &mut Vec<T>, n: usize) -> Result<()> {
    v.try_reserve_exact(n).map_err(|_| error("allocation", "Cannot allocate resampling storage."))
}
fn cancelled(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(error("cancelled", "Resampling cancelled.")) } else { Ok(()) }
}
fn memory(bytes: u64, limit: u64) -> Result<()> {
    if bytes > limit { Err(error("memory_limit", "Resampling storage exceeds configured memory.")) } else { Ok(()) }
}
fn count(n: usize, limits: &ResamplingLimits) -> Result<()> {
    if n == 0 || n > 1_000_000 || n > limits.max_cells {
        Err(error("resampling_limit", "Use1..1000000 finite numeric cells within configured limits."))
    } else {
        Ok(())
    }
}
fn copy(values: &[f64]) -> Result<Vec<f64>> {
    let mut result = Vec::new();
    reserve(&mut result, values.len())?;
    result.extend_from_slice(values);
    Ok(result)
}
impl PreparedValues {
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn new(values: &[f64], limits: &ResamplingLimits, control: &dyn RunControl) -> Result<Self> {
        count(values.len(), limits)?;
        memory(values.len() as u64 * 8 + 256, limits.max_memory_bytes)?;
        cancelled(control)?;
        if values.iter().any(|x| !x.is_finite()) {
            return Err(error("non_finite_sample", "Resampling cells must be finite."));
        }
        Ok(Self { values: copy(values)?, max_memory: limits.max_memory_bytes })
    }
    pub fn resample(&self, address: VectorAddress, control: &dyn RunControl) -> Result<f64> {
        cancelled(control)?;
        let index = solvercraft_math::addressed_index(address.seed, address.binding, address.trial, 0x5256_0000_0000_0000, self.values.len())?;
        self.values.get(index).copied().ok_or_else(|| error("resampling_index", "Missing resampled cell."))
    }
    /// No wraparound, extrapolation, or extra random draw is implied by trial replay.
    pub fn sip(&self, trial: u64, control: &dyn RunControl) -> Result<f64> {
        cancelled(control)?;
        let index = usize::try_from(trial).map_err(|_| error("replay_index", "SIP trial is out of range."))?;
        self.values.get(index).copied().ok_or_else(|| error("replay_index", "SIP trial is out of range."))
    }
    pub fn memory_limit(&self) -> u64 {
        self.max_memory
    }
}
impl PreparedTable {
    pub fn shape(&self) -> (usize, usize) {
        (self.rows.len(), self.columns)
    }
    pub fn new(rows: &[Vec<f64>], limits: &ResamplingLimits, control: &dyn RunControl) -> Result<Self> {
        let columns = rows.first().map_or(0, Vec::len);
        let cells = rows.len().checked_mul(columns).ok_or_else(|| error("resampling_limit", "Table cell count overflow."))?;
        count(cells, limits)?;
        if rows.iter().any(|r| r.len() != columns) {
            return Err(error("table_shape", "Resampling tables must be rectangular and nonempty."));
        }
        let bytes = rows.len() as u64 * 24 + cells as u64 * 8 + 256;
        memory(bytes, limits.max_memory_bytes)?;
        cancelled(control)?;
        for row in rows {
            cancelled(control)?;
            if row.iter().any(|x| !x.is_finite()) {
                return Err(error("non_finite_sample", "Resampling cells must be finite."));
            }
        }
        let mut copied = Vec::new();
        reserve(&mut copied, rows.len())?;
        for row in rows {
            cancelled(control)?;
            copied.push(copy(row)?);
        }
        Ok(Self { rows: copied, columns, cells, bytes, max_memory: limits.max_memory_bytes })
    }
    /// With replacement across trial coordinates. The selected source vector remains intact.
    pub fn mv_resample(&self, orientation: VectorOrientation, address: VectorAddress, control: &dyn RunControl) -> Result<Vec<f64>> {
        cancelled(control)?;
        let (choices, width, domain) = match orientation {
            VectorOrientation::Row => (self.rows.len(), self.columns, 0x5252_0000_0000_0000),
            VectorOrientation::Column => (self.columns, self.rows.len(), 0x5243_0000_0000_0000),
        };
        memory(self.bytes + width as u64 * 8 + 24, self.max_memory)?;
        let index = solvercraft_math::addressed_index(address.seed, address.binding, address.trial, domain, choices)?;
        match orientation {
            VectorOrientation::Row => copy(self.rows.get(index).ok_or_else(|| error("resampling_index", "Missing source row."))?),
            VectorOrientation::Column => {
                let mut result = Vec::new();
                reserve(&mut result, width)?;
                for row in &self.rows {
                    result.push(*row.get(index).ok_or_else(|| error("resampling_index", "Missing source column."))?);
                }
                Ok(result)
            }
        }
    }
    /// A full-cell permutation without replacement within this trial; duplicate values are preserved.
    pub fn mv_shuffle(&self, address: VectorAddress, control: &dyn RunControl) -> Result<Vec<Vec<f64>>> {
        cancelled(control)?;
        memory(self.bytes.checked_mul(2).ok_or_else(|| error("memory_limit", "Shuffle memory overflow."))?, self.max_memory)?;
        let mut result = Vec::new();
        reserve(&mut result, self.rows.len())?;
        for row in &self.rows {
            cancelled(control)?;
            result.push(copy(row)?);
        }
        for i in (1..self.cells).rev() {
            if i.is_multiple_of(1024) {
                cancelled(control)?;
            }
            // Each index reserves128 subaddresses, so bounded rejection retries never overlap neighbors.
            let j = solvercraft_math::addressed_index(address.seed, address.binding, address.trial, 0x5348_0000_0000_0000 + i as u64 * 128, i + 1)?;
            let a = result
                .get(i / self.columns)
                .and_then(|r| r.get(i % self.columns))
                .copied()
                .ok_or_else(|| error("resampling_index", "Missing shuffle cell."))?;
            let b = result
                .get(j / self.columns)
                .and_then(|r| r.get(j % self.columns))
                .copied()
                .ok_or_else(|| error("resampling_index", "Missing shuffle cell."))?;
            *result
                .get_mut(i / self.columns)
                .and_then(|r| r.get_mut(i % self.columns))
                .ok_or_else(|| error("resampling_index", "Missing shuffle cell."))? = b;
            *result
                .get_mut(j / self.columns)
                .and_then(|r| r.get_mut(j % self.columns))
                .ok_or_else(|| error("resampling_index", "Missing shuffle cell."))? = a;
        }
        Ok(result)
    }
    pub fn slurp(&self, trial: u64, one_based_column: usize, control: &dyn RunControl) -> Result<f64> {
        cancelled(control)?;
        let column = one_based_column.checked_sub(1).ok_or_else(|| error("replay_index", "SLURP columns are1-based."))?;
        let row = usize::try_from(trial).map_err(|_| error("replay_index", "SLURP trial is out of range."))?;
        self.rows.get(row).and_then(|r| r.get(column)).copied().ok_or_else(|| error("replay_index", "SLURP trial/column is out of range."))
    }
}
