//! Checked dense linear algebra for the initial convex engine.
//! All matrices are internally sized and bounded before entry.
#![allow(clippy::needless_range_loop)] // Triangular algorithms address several matrix rows at once.
use solvercraft_types::{Result, RunControl, SolverError};
pub(crate) const MAX_CONVEX_VARIABLES: usize = 512;
const MAX_FACTOR_WORK: u64 = 300_000_000;

pub(crate) fn error(message: &str) -> SolverError {
    SolverError::new("convex_numerical", message)
}
pub(crate) fn check(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(SolverError::new("cancelled", "Convex solve cancelled.")) } else { Ok(()) }
}
pub(crate) fn norm(values: &[f64]) -> f64 {
    values.iter().map(|v| v.abs()).fold(0.0, f64::max)
}
pub(crate) fn dot(a: &[f64], b: &[f64]) -> Result<f64> {
    solvercraft_model::compensated_dot(a, b)
}
pub(crate) fn multiply(matrix: &[Vec<f64>], x: &[f64]) -> Result<Vec<f64>> {
    matrix.iter().map(|row| dot(row, x)).collect()
}
pub(crate) fn transpose_multiply(matrix: &[Vec<f64>], x: &[f64], n: usize) -> Result<Vec<f64>> {
    (0..n)
        .map(|j| {
            let mut sum = solvercraft_model::ExactProducts::default();
            for (row, value) in matrix.iter().zip(x) {
                sum.add_product(row[j], *value)?;
            }
            sum.finish()
        })
        .collect()
}
pub(crate) fn zero(rows: usize, cols: usize) -> Result<Vec<Vec<f64>>> {
    let mut out = Vec::new();
    out.try_reserve_exact(rows).map_err(|_| error("Cannot allocate matrix rows."))?;
    for _ in 0..rows {
        let mut row = Vec::new();
        row.try_reserve_exact(cols).map_err(|_| error("Cannot allocate matrix columns."))?;
        row.resize(cols, 0.0);
        out.push(row);
    }
    Ok(out)
}
#[derive(Clone)]
pub(crate) struct Ldlt {
    pub lower: Vec<Vec<f64>>,
    pub diagonal: Vec<f64>,
}
pub(crate) fn factor(matrix: &[Vec<f64>], allow_zero: bool, control: &dyn RunControl) -> Result<Ldlt> {
    let n = matrix.len();
    let mut lower = zero(n, n)?;
    let mut diagonal = vec![0.0; n];
    let mut work = 0_u64;
    for i in 0..n {
        check(control)?;
        lower[i][i] = 1.0;
        for j in 0..i {
            let mut residual = solvercraft_model::ExactProducts::default();
            residual.add(matrix[i][j])?;
            for k in 0..j {
                residual.add_triple(-lower[i][k], diagonal[k], lower[j][k])?;
            }
            work = work.saturating_add(j as u64 + 1);
            if work > MAX_FACTOR_WORK {
                return Err(SolverError::new("work_limit", "Dense factorization work budget exceeded."));
            }
            let value = residual.finish()?;
            if diagonal[j] > 0.0 {
                lower[i][j] = value / diagonal[j];
            } else if allow_zero && diagonal[j] == 0.0 && value == 0.0 {
                lower[i][j] = 0.0;
            } else {
                return Err(SolverError::new("convexity_not_proven", "A zero/negative LDL pivot has a nonzero coupling."));
            }
            if !lower[i][j].is_finite() {
                return Err(error("LDL elimination exceeds finite arithmetic."));
            }
        }
        let mut residual = solvercraft_model::ExactProducts::default();
        residual.add(matrix[i][i])?;
        for k in 0..i {
            residual.add_triple(-lower[i][k], lower[i][k], diagonal[k])?;
        }
        let d = residual.finish()?;
        if !d.is_finite() || d < 0.0 || (!allow_zero && d == 0.0) {
            return Err(SolverError::new("convexity_not_proven", "A nonnegative curvature/factor pivot could not be established."));
        }
        diagonal[i] = d;
    }
    Ok(Ldlt { lower, diagonal })
}
impl Ldlt {
    pub fn solve(&self, rhs: &[f64], control: &dyn RunControl) -> Result<Vec<f64>> {
        let n = rhs.len();
        let mut y = vec![0.0; n];
        for i in 0..n {
            if i % 32 == 0 {
                check(control)?;
            }
            let mut residual = solvercraft_model::ExactProducts::default();
            residual.add(rhs[i])?;
            for (a, b) in self.lower[i][..i].iter().zip(&y[..i]) {
                residual.add_product(-a, *b)?;
            }
            y[i] = residual.finish()?;
            if !y[i].is_finite() {
                return Err(error("Forward solve overflowed."));
            }
        }
        for (value, d) in y.iter_mut().zip(&self.diagonal) {
            if *d <= 0.0 {
                return Err(error("Singular factor cannot be inverted."));
            }
            *value /= d;
        }
        let mut result = vec![0.0; n];
        for i in (0..n).rev() {
            let mut residual = solvercraft_model::ExactProducts::default();
            residual.add(y[i])?;
            for (j, b) in result.iter().enumerate().skip(i + 1) {
                residual.add_product(-self.lower[j][i], *b)?;
            }
            result[i] = residual.finish()?;
            if !result[i].is_finite() {
                return Err(error("Backward solve overflowed."));
            }
        }
        Ok(result)
    }
}
fn upper_abs_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    for value in values {
        let term = value.abs();
        let next = sum + term;
        if !next.is_finite() {
            return f64::INFINITY;
        }
        let b = next - sum;
        let residual = (sum - (next - b)) + (term - b);
        sum = if residual > 0.0 { next.next_up() } else { next };
    }
    sum
}
/// A conservative lower eigenvalue estimate and a sufficient PSD check.
/// Diagonal dominance supplies a direct sufficient certificate, including
/// zero eigenvalues; otherwise a strictly positive checked LDL residual bound
/// is required. Unproven semidefinite cases are rejected rather than guessed.
pub(crate) fn curvature(matrix: &[Vec<f64>], control: &dyn RunControl) -> Result<(f64, String)> {
    let n = matrix.len();
    if n == 0 {
        return Ok((0.0, "empty objective".into()));
    }
    let mut dd = true;
    let mut gersh = f64::INFINITY;
    for (i, row) in matrix.iter().enumerate() {
        let off = upper_abs_sum(row.iter().enumerate().filter_map(|(j, v)| (i != j).then_some(*v)));
        if row[i] < off || !off.is_finite() {
            dd = false;
        }
        gersh = gersh.min((row[i] - off).next_down());
    }
    if dd {
        return Ok((gersh.max(0.0), "symmetric diagonal dominance with nonnegative diagonal".into()));
    }
    let factor = factor(matrix, true, control)?;
    if factor.diagonal.iter().any(|d| *d <= 0.0) {
        return Err(SolverError::new("convexity_not_proven", "Semidefinite curvature was not established by the available sufficient certificates."));
    }
    let mut inverse = zero(n, n)?;
    for column in 0..n {
        check(control)?;
        for i in 0..n {
            inverse[i][column] =
                if i == column { 1.0 } else { 0.0 } - solvercraft_model::compensated_sum((0..i).map(|j| factor.lower[i][j] * inverse[j][column]))?;
        }
    }
    let inverse_norm = inverse.iter().map(|row| upper_abs_sum(row.iter().copied())).fold(0.0, f64::max);
    let mut inverse_residual = 0.0_f64;
    let mut reconstruction_residual = 0.0_f64;
    let mut matrix_norm = 0.0_f64;
    for i in 0..n {
        check(control)?;
        let mut inverse_row = 0.0;
        let mut factor_row = 0.0;
        for j in 0..n {
            let value = solvercraft_model::compensated_sum((0..n).map(|k| factor.lower[i][k] * inverse[k][j]))?;
            inverse_row += (value - if i == j { 1.0 } else { 0.0 }).abs();
            let reconstructed = solvercraft_model::compensated_sum((0..n).map(|k| factor.lower[i][k] * factor.diagonal[k] * factor.lower[j][k]))?;
            factor_row += (matrix[i][j] - reconstructed).abs();
        }
        inverse_residual = inverse_residual.max(inverse_row);
        reconstruction_residual = reconstruction_residual.max(factor_row);
        matrix_norm = matrix_norm.max(upper_abs_sum(matrix[i].iter().copied()));
    }
    let dimension = n as f64;
    let rounding = 128.0 * f64::EPSILON * dimension;
    inverse_residual += rounding * (1.0 + inverse_norm);
    let error = reconstruction_residual + rounding * matrix_norm;
    if inverse_residual >= 1.0 || !error.is_finite() {
        return Err(error_fn());
    }
    let inverse_bound = (inverse_norm / (1.0 - inverse_residual)).next_up();
    let min_d = factor.diagonal.iter().copied().fold(f64::INFINITY, f64::min);
    let lower = (min_d / (dimension * inverse_bound * inverse_bound) - error).next_down();
    if !lower.is_finite() || lower <= 0.0 {
        return Err(error_fn());
    }
    Ok((lower, "checked positive LDL factor with residual/norm eigenvalue bound".into()))
}
fn error_fn() -> SolverError {
    SolverError::new("convexity_not_proven", "Factor residual bounds did not establish positive curvature.")
}
