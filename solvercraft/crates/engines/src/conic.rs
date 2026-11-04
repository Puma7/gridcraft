//! Sparse continuous LP/QP/SOCP adapter. The backend is Clarabel 0.11.1.
//! Cone membership and candidates are checked again in the original model.
use clarabel::{
    algebra::CscMatrix,
    solver::{DefaultSettings, DefaultSolver, IPSolver, SolverStatus, SupportedConeT},
};
use serde::{Deserialize, Serialize};
use solvercraft_model::{
    ExactProducts, canonicalize, compensated_sum, evaluate_linear_residual, evaluate_objective, evaluate_working_objective, validate,
    validate_solution,
};
use solvercraft_types::*;
use std::sync::Arc;

const ENGINE: &str = "solvercraft-clarabel-0.11.1-adapter-0.2";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffineExpression {
    pub offset: f64,
    pub terms: Vec<LinearTerm>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondOrderConeConstraint {
    pub name: String,
    pub components: Vec<AffineExpression>,
    pub upper: AffineExpression,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConicModel {
    pub base: OptimizationModel,
    pub second_order_cones: Vec<SecondOrderConeConstraint>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConicCertificate {
    /// Multipliers in standard form Ax+s=b. Zero-cone rows are unrestricted.
    pub dual: Vec<f64>,
    pub primal_residual: f64,
    pub stationarity_residual: f64,
    pub complementarity_residual: f64,
    pub working_primal_objective: f64,
    pub working_dual_bound: Option<f64>,
    pub absolute_gap: Option<f64>,
    pub row_descriptions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConicDetailedResult {
    pub result: SolveResult,
    pub certificate: Option<ConicCertificate>,
    pub backend_status: String,
}
#[derive(Clone, Copy)]
enum Kind {
    Zero,
    Nonnegative,
    Lorentz,
}
struct Block {
    start: usize,
    len: usize,
    kind: Kind,
}
struct Data {
    p: CscMatrix<f64>,
    q: Vec<f64>,
    a: CscMatrix<f64>,
    b: Vec<f64>,
    cones: Vec<SupportedConeT<f64>>,
    blocks: Vec<Block>,
    descriptions: Vec<String>,
    direction: f64,
    minimum_eigenvalue: f64,
    certificate_bounds: Vec<(Option<f64>, Option<f64>)>,
}
fn error(message: &str) -> SolverError {
    SolverError::new("invalid_conic_model", message)
}
fn cancelled(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(SolverError::new("cancelled", "Conic solve cancelled.")) } else { Ok(()) }
}
fn stopped(termination: Termination) -> ConicDetailedResult {
    let mut result = crate::empty_result(termination, Feasibility::Unknown, Optimality::Unknown);
    result.engine = ENGINE.into();
    result.warnings = vec!["Floating-point conic solver; no exact rational certificate is claimed.".into()];
    ConicDetailedResult { result, certificate: None, backend_status: "not_started".into() }
}
fn check_affine(expression: &AffineExpression, n: usize) -> Result<()> {
    if !expression.offset.is_finite() || expression.terms.iter().any(|t| t.variable >= n || !t.coefficient.is_finite()) {
        return Err(error("Affine cone expressions require finite coefficients and valid variable indices."));
    }
    Ok(())
}
fn csc(rows: usize, cols: usize, mut entries: Vec<(usize, usize, f64)>) -> Result<CscMatrix<f64>> {
    entries.sort_unstable_by_key(|(r, c, _)| (*c, *r));
    let mut ptr = vec![0; cols + 1];
    let mut indices = Vec::with_capacity(entries.len());
    let mut values = Vec::with_capacity(entries.len());
    let mut i = 0;
    while i < entries.len() {
        let (row, col, _) = entries[i];
        let start = i;
        while i < entries.len() && entries[i].0 == row && entries[i].1 == col {
            i += 1;
        }
        let mut exact = ExactProducts::default();
        for entry in &entries[start..i] {
            exact.add(entry.2)?;
        }
        let value = exact.clone().finish()?;
        exact.add(-value)?;
        if exact.exact_sign()? != std::cmp::Ordering::Equal {
            return Err(SolverError::new(
                "numeric_range",
                "Duplicate CSC coefficients are not exactly representable; refusing a changed conic model.",
            ));
        }
        if value != 0.0 {
            indices.push(row);
            values.push(value);
            ptr[col + 1] += 1;
        }
    }
    for col in 0..cols {
        ptr[col + 1] += ptr[col];
    }
    let matrix = CscMatrix::new(rows, cols, ptr, indices, values);
    matrix.check_format().map_err(|_| error("Constructed CSC matrix failed format validation."))?;
    Ok(matrix)
}
pub(crate) fn preflight(model: &ConicModel, options: &SolveOptions, control: &dyn RunControl) -> Result<(usize, usize, usize)> {
    validate(&model.base, options)?;
    let n = model.base.variables.len();
    let mut m = model.base.constraints.len();
    let mut nnz =
        model.base.constraints.iter().try_fold(0usize, |s, r| s.checked_add(r.terms.len()).ok_or_else(|| error("Nonzero count overflow.")))?;
    for v in &model.base.variables {
        let (lower, upper) = solvercraft_model::effective_bounds(v)?;
        m = m.checked_add(usize::from(lower.is_some()) + usize::from(upper.is_some())).ok_or_else(|| error("Bound row count overflow."))?;
        nnz = nnz.checked_add(2).ok_or_else(|| error("Bound nonzero count overflow."))?;
    }
    for cone in &model.second_order_cones {
        if cone.components.is_empty() {
            return Err(error("Lorentz constraints require at least one component."));
        }
        if cone.components.len() > 100_000 {
            return Err(error("Cone dimension exceeds configured adapter limit."));
        }
        m = m.checked_add(cone.components.len() + 1).ok_or_else(|| error("Cone row count overflow."))?;
        for expr in std::iter::once(&cone.upper).chain(&cone.components) {
            check_affine(expr, n)?;
            nnz = nnz.checked_add(expr.terms.len()).ok_or_else(|| error("Cone nonzero count overflow."))?;
        }
    }
    // The sparse factor can acquire fill. Reserve a conservative dense upper estimate,
    // including several copies of data, symbolic/factor indices and cone work vectors.
    let dim = n
        .checked_add(m)
        .and_then(|v| model.second_order_cones.len().checked_mul(2).and_then(|extra| v.checked_add(extra)))
        .ok_or_else(|| error("KKT dimension overflow."))?;
    let bytes = (dim as u64)
        .checked_mul(dim as u64)
        .and_then(|v| v.checked_mul(96))
        .and_then(|v| (dim as u64).checked_mul(2048).and_then(|scratch| v.checked_add(scratch)))
        .and_then(|v| (nnz as u64).checked_mul(96).and_then(|s| v.checked_add(s)))
        .and_then(|v| (n as u64).checked_mul(64).and_then(|s| v.checked_add(s)))
        .ok_or_else(|| SolverError::new("memory_limit", "Sparse conic working-set estimate overflow."))?;
    if dim > 20_000 || bytes > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "Conservative sparse KKT fill estimate exceeds memory budget."));
    }
    cancelled(control)?;
    Ok((n, m, nnz))
}
fn build(model: &ConicModel, options: &SolveOptions, control: &dyn RunControl) -> Result<Data> {
    let (n, m, nnz) = preflight(model, options, control)?;
    if options.method != SolveMethod::Auto {
        return Err(SolverError::new("unsupported_method", "Conic adapter requires auto; it does not impersonate another algorithm."));
    }
    if model.base.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
        return Err(SolverError::new("unsupported_problem", "Continuous conic backend does not relax integer domains silently."));
    }
    let base = canonicalize(&model.base, options)?;
    let original_bounds = base.variables.iter().map(|v| (v.lower, v.upper)).collect::<Vec<_>>();
    let certificate_bounds = crate::certificate_box(&base, &original_bounds);
    let direction = if base.objective.direction == Direction::Minimize { 1.0 } else { -1.0 };
    let minimum_eigenvalue = if base.objective.quadratic.is_empty() {
        0.0
    } else {
        crate::analyze_quadratic_curvature(&base, options, control)?.minimum_eigenvalue_lower_bound
    };
    let mut q = vec![0.0; n];
    for t in &base.objective.linear {
        q[t.variable] = direction * t.coefficient;
    }
    let mut p = Vec::with_capacity(base.objective.quadratic.len());
    for t in &base.objective.quadratic {
        let v = direction * t.coefficient * if t.left == t.right { 2.0 } else { 1.0 };
        if !v.is_finite() {
            return Err(error("Hessian coefficient overflow."));
        }
        p.push((t.left.min(t.right), t.left.max(t.right), v));
    }
    let mut a = Vec::with_capacity(nnz);
    let mut b = Vec::with_capacity(m);
    let mut blocks = Vec::new();
    let mut cones = Vec::new();
    let mut descriptions = Vec::with_capacity(m);
    for (i, row) in base.constraints.iter().enumerate() {
        if i % 1024 == 0 {
            cancelled(control)?;
        }
        let sign = if row.relation == Relation::GreaterEqual { -1.0 } else { 1.0 };
        let r = b.len();
        for t in &row.terms {
            a.push((r, t.variable, sign * t.coefficient));
        }
        b.push(sign * row.rhs);
        let kind = if row.relation == Relation::Equal { Kind::Zero } else { Kind::Nonnegative };
        blocks.push(Block { start: r, len: 1, kind });
        cones.push(if row.relation == Relation::Equal { SupportedConeT::ZeroConeT(1) } else { SupportedConeT::NonnegativeConeT(1) });
        descriptions.push(format!("linear:{i}"));
    }
    for (i, var) in base.variables.iter().enumerate() {
        for (side, bound, sign) in [("lower", var.lower, -1.0), ("upper", var.upper, 1.0)] {
            if let Some(bound) = bound {
                let r = b.len();
                a.push((r, i, sign));
                b.push(sign * bound);
                blocks.push(Block { start: r, len: 1, kind: Kind::Nonnegative });
                cones.push(SupportedConeT::NonnegativeConeT(1));
                descriptions.push(format!("bound:{i}:{side}"));
            }
        }
    }
    for (i, cone) in model.second_order_cones.iter().enumerate() {
        cancelled(control)?;
        let start = b.len();
        for (j, expr) in std::iter::once(&cone.upper).chain(&cone.components).enumerate() {
            let r = b.len();
            for t in &expr.terms {
                a.push((r, t.variable, -t.coefficient));
            }
            b.push(expr.offset);
            descriptions.push(format!("soc:{i}:{j}"));
        }
        let len = b.len() - start;
        blocks.push(Block { start, len, kind: Kind::Lorentz });
        cones.push(SupportedConeT::SecondOrderConeT(len));
    }
    Ok(Data { p: csc(n, n, p)?, q, a: csc(b.len(), n, a)?, b, cones, blocks, descriptions, direction, minimum_eigenvalue, certificate_bounds })
}
fn accumulate_multiply(matrix: &CscMatrix<f64>, x: &[f64], transpose: bool, symmetric: bool) -> Result<Vec<ExactProducts>> {
    let mut sums = vec![ExactProducts::default(); if transpose { matrix.n } else { matrix.m }];
    for col in 0..matrix.n {
        for idx in matrix.colptr[col]..matrix.colptr[col + 1] {
            let row = matrix.rowval[idx];
            let v = matrix.nzval[idx];
            if transpose {
                sums[col].add_product(v, x[row])?;
            } else {
                sums[row].add_product(v, x[col])?;
            }
            if symmetric && row != col {
                sums[col].add_product(v, x[row])?;
            }
        }
    }
    Ok(sums)
}
fn multiply(matrix: &CscMatrix<f64>, x: &[f64], transpose: bool, symmetric: bool) -> Result<Vec<f64>> {
    accumulate_multiply(matrix, x, transpose, symmetric)?.into_iter().map(ExactProducts::finish).collect()
}
fn norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0, |s, v| s.hypot(*v))
}
fn dot(a: &[f64], b: &[f64]) -> Result<f64> {
    solvercraft_model::compensated_dot(a, b)
}
// Absolute product sums bound arithmetic sensitivity even when large terms
// cancel to a small objective or stationarity residual.
fn arithmetic_scales(data: &Data, x: &[f64], z: &[f64]) -> Option<(Vec<f64>, f64)> {
    let mut gradient = vec![0.0; x.len()];
    let mut counts = vec![2_u64; x.len()];
    for col in 0..data.p.n {
        for k in data.p.colptr[col]..data.p.colptr[col + 1] {
            let row = data.p.rowval[k];
            let p = data.p.nzval[k].abs();
            gradient[row] += p * x[col].abs();
            counts[row] += 1;
            if row != col {
                gradient[col] += p * x[row].abs();
                counts[col] += 1;
            }
        }
    }
    let mut magnitude = 0.0;
    for i in 0..x.len() {
        magnitude += (0.5 * gradient[i] + data.q[i].abs()) * x[i].abs();
        gradient[i] += data.q[i].abs();
    }
    let mut row_scale = data.b.iter().map(|v| v.abs()).collect::<Vec<_>>();
    for col in 0..data.a.n {
        for k in data.a.colptr[col]..data.a.colptr[col + 1] {
            let row = data.a.rowval[k];
            let a = data.a.nzval[k].abs();
            gradient[col] += a * z[row].abs();
            counts[col] += 1;
            row_scale[row] += a * x[col].abs();
        }
    }
    for (row, multiplier) in row_scale.iter().zip(z) {
        magnitude += row * multiplier.abs();
    }
    let errors = gradient.iter().zip(counts).map(|(v, count)| 16.0 * f64::EPSILON * count as f64 * v).collect::<Vec<_>>();
    (magnitude.is_finite() && errors.iter().all(|e| e.is_finite())).then_some((errors, magnitude))
}
fn project_dual(z: &[f64], data: &Data) -> Vec<f64> {
    let mut z = z.to_vec();
    for block in &data.blocks {
        let range = block.start..block.start + block.len;
        match block.kind {
            Kind::Zero => {}
            Kind::Nonnegative => {
                for value in &mut z[range] {
                    *value = value.max(0.0);
                }
            }
            Kind::Lorentz => {
                let t = z[block.start];
                let radius = norm(&z[block.start + 1..range.end]);
                if radius > t {
                    if radius <= -t {
                        z[range].fill(0.0);
                    } else {
                        let head = 0.5 * radius + 0.5 * t;
                        for v in &mut z[block.start + 1..range.end] {
                            *v *= head / radius;
                        }
                        z[block.start] = norm(&z[block.start + 1..range.end]);
                    }
                }
                z[block.start] *= 1.0 + 16.0 * f64::EPSILON;
            }
        }
    }
    z
}
fn cone_violation(values: &[f64], data: &Data, dual: bool) -> f64 {
    data.blocks
        .iter()
        .map(|block| {
            let slice = &values[block.start..block.start + block.len];
            match block.kind {
                Kind::Zero => {
                    if dual {
                        0.0
                    } else {
                        slice.iter().map(|v| v.abs()).fold(0.0, f64::max)
                    }
                }
                Kind::Nonnegative => slice.iter().map(|v| (-v).max(0.0)).fold(0.0, f64::max),
                Kind::Lorentz => (norm(&slice[1..]) - slice[0]).max(0.0),
            }
        })
        .fold(0.0, f64::max)
}
fn expression(expr: &AffineExpression, x: &[f64]) -> Result<f64> {
    evaluate_linear_residual(&expr.terms, x, -expr.offset)
}
/// Recheck cone constraints in user units (no backend scaling or relaxation).
pub fn validate_conic_solution(model: &ConicModel, values: &[f64], options: &SolveOptions) -> Result<(bool, f64)> {
    let component_count = model.second_order_cones.iter().try_fold(0_u64, |sum, c| {
        sum.checked_add(c.components.len() as u64).ok_or_else(|| SolverError::new("memory_limit", "Conic validation dimension overflow."))
    })?;
    let estimate = component_count
        .checked_mul(16)
        .and_then(|v| v.checked_add((model.base.variables.len() as u64).checked_mul(64)?))
        .and_then(|v| v.checked_add((model.base.constraints.len() as u64).checked_mul(48)?));
    if estimate.is_none_or(|bytes| bytes > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "Conic original-domain validation exceeds resource budget."));
    }
    validate(&model.base, options)?;
    for cone in &model.second_order_cones {
        if cone.components.is_empty() {
            return Err(error("Lorentz constraints require at least one component."));
        }
        for expr in std::iter::once(&cone.upper).chain(&cone.components) {
            check_affine(expr, model.base.variables.len())?;
        }
    }
    let checked = validate_solution(&model.base, values, options)?;
    let mut violation = checked.max_violation;
    for cone in &model.second_order_cones {
        let radius = cone.components.iter().try_fold(0.0_f64, |r, e| Ok::<f64, SolverError>(r.hypot(expression(e, values)?)))?;
        let upper = expression(&cone.upper, values)?;
        let v = radius - upper;
        if !v.is_finite() {
            return Err(error("Cone evaluation overflow."));
        }
        violation = violation.max(v.max(0.0));
    }
    Ok((checked.feasible && violation <= options.feasibility_tolerance, violation))
}
fn certificate(model: &ConicModel, data: &Data, x: &[f64], z: &[f64]) -> Result<ConicCertificate> {
    let z = project_dual(z, data);
    let ax = multiply(&data.a, x, false, false)?;
    let slack = data.b.iter().zip(&ax).map(|(b, a)| b - a).collect::<Vec<_>>();
    let mut gradients = accumulate_multiply(&data.p, x, false, true)?;
    for (sum, q) in gradients.iter_mut().zip(&data.q) {
        sum.add(*q)?;
    }
    let mut exact_lagrangian = ExactProducts::default();
    for term in &model.base.objective.linear {
        exact_lagrangian.add_product(data.direction * term.coefficient, x[term.variable])?;
    }
    for term in &model.base.objective.quadratic {
        exact_lagrangian.add_triple(data.direction * term.coefficient, x[term.left], x[term.right])?;
    }
    for (col, gradient) in gradients.iter_mut().enumerate() {
        for k in data.a.colptr[col]..data.a.colptr[col + 1] {
            let row = data.a.rowval[k];
            let coefficient = data.a.nzval[k];
            gradient.add_product(coefficient, z[row])?;
            exact_lagrangian.add_triple(z[row], coefficient, x[col])?;
        }
    }
    for (z, b) in z.iter().zip(&data.b) {
        exact_lagrangian.add_product(-z, *b)?;
    }
    let mut residual = Vec::with_capacity(x.len());
    let mut exact_errors = Vec::with_capacity(x.len());
    for gradient in gradients {
        let (low, high) = gradient.enclosure()?;
        let value = gradient.finish()?;
        let error = (value - low).abs().max((high - value).abs());
        residual.push(value);
        exact_errors.push(if error == 0.0 { 0.0 } else { error.next_up() });
    }
    let stationarity = residual.iter().map(|v| v.abs()).fold(0.0, f64::max);
    let primal = data.direction * evaluate_working_objective(&model.base, x)?;
    let lagrangian = exact_lagrangian.enclosure()?.0;
    let scales = arithmetic_scales(data, x, &z).map(|(mut errors, scale)| {
        for (error, exact) in errors.iter_mut().zip(exact_errors) {
            *error = error.max(exact);
        }
        (errors, scale)
    });
    let mut correction = scales.as_ref().map(|_| 0.0);
    for (i, r) in residual.iter().enumerate() {
        let Some((errors, _)) = &scales else { break };
        let error = errors[i];
        let contribution = match data.certificate_bounds[i] {
            (Some(low), Some(high)) => {
                [(*r - error) * (low - x[i]), (*r + error) * (low - x[i]), (*r - error) * (high - x[i]), (*r + error) * (high - x[i])]
                    .into_iter()
                    .reduce(f64::min)
            }
            (Some(low), None) if *r - error >= 0.0 => Some(((*r - error) * (low - x[i])).min((*r + error) * (low - x[i]))),
            (None, Some(high)) if *r + error <= 0.0 => Some(((*r - error) * (high - x[i])).min((*r + error) * (high - x[i]))),
            (None, None) if *r == 0.0 && error == 0.0 => Some(0.0),
            _ => None,
        };
        correction = correction.zip(contribution).and_then(|(a, b)| compensated_sum([a, b]).ok());
    }
    let strong = if data.minimum_eigenvalue > 0.0 && scales.is_some() {
        let rnorm = norm(&residual.iter().zip(scales.as_ref().map(|s| s.0.as_slice()).unwrap_or(&[])).map(|(r, e)| r.abs() + e).collect::<Vec<_>>());
        Some(-0.5 * rnorm * (rnorm / data.minimum_eigenvalue))
    } else {
        None
    };
    let correction = match (correction, strong) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    let bound = correction.and_then(|c| compensated_sum([lagrangian, c]).ok()).and_then(|v| {
        let magnitude = scales.as_ref()?.1 + v.abs();
        let safety = 128.0 * f64::EPSILON * (1.0 + magnitude) * (1.0 + x.len() as f64 + z.len() as f64);
        let value = v - safety;
        (value.is_finite() && value <= primal).then_some(value)
    });
    Ok(ConicCertificate {
        dual: z.clone(),
        primal_residual: cone_violation(&slack, data, false),
        stationarity_residual: stationarity,
        complementarity_residual: dot(&z, &slack)?.abs(),
        working_primal_objective: data.direction * primal,
        working_dual_bound: bound.map(|b| data.direction * b),
        absolute_gap: bound.map(|b| primal - b),
        row_descriptions: data.descriptions.clone(),
    })
}
fn infeasibility_certificate(_model: &ConicModel, data: &Data, z: &[f64]) -> Result<bool> {
    if z.len() != data.b.len() || z.iter().any(|v| !v.is_finite()) {
        return Ok(false);
    }
    let z = project_dual(z, data);
    let residual = multiply(&data.a, &z, true, false)?;
    let mut errors = Vec::with_capacity(residual.len());
    for col in 0..data.a.n {
        let range = data.a.colptr[col]..data.a.colptr[col + 1];
        let scale = compensated_sum(range.clone().map(|k| (data.a.nzval[k] * z[data.a.rowval[k]]).abs()))?;
        errors.push(16.0 * f64::EPSILON * (2 + range.len()) as f64 * scale);
    }
    let mut minimum = Vec::with_capacity(residual.len());
    for ((r, e), bounds) in residual.iter().zip(errors).zip(&data.certificate_bounds) {
        if *r == 0.0 && e == 0.0 {
            minimum.push(0.0);
        } else {
            let value = match *bounds {
                (Some(low), Some(high)) => [(*r - e) * low, (*r + e) * low, (*r - e) * high, (*r + e) * high].into_iter().reduce(f64::min),
                (Some(low), None) if *r - e >= 0.0 => Some(((*r - e) * low).min((*r + e) * low)),
                (None, Some(high)) if *r + e <= 0.0 => Some(((*r - e) * high).min((*r + e) * high)),
                _ => None,
            };
            let Some(value) = value else { return Ok(false) };
            minimum.push(value);
        }
    }
    let left_scale = compensated_sum(minimum.iter().map(|v| v.abs()))?;
    let left = compensated_sum(minimum)?;
    let right = dot(&z, &data.b)?;
    // For every feasible x, z'Ax<=z'b. A strict lower bound over the original
    // variable box contradicts this. Unlike a small A'z residual, this remains
    // valid for huge finite bounds. Free nonzero residuals are refused above.
    let right_scale = compensated_sum(z.iter().zip(&data.b).map(|(z, b)| (z * b).abs()))?;
    let safety = 128.0 * f64::EPSILON * (1.0 + left_scale + right_scale) * (1.0 + z.len() as f64);
    Ok(left - right > safety)
}
fn unbounded_certificate(model: &ConicModel, options: &SolveOptions, data: &Data, direction: &[f64]) -> Result<Option<Vec<f64>>> {
    if direction.len() != data.q.len() || direction.iter().any(|v| !v.is_finite()) {
        return Ok(None);
    }
    // A feasible anchor plus a genuine recession direction proves unboundedness.
    // Exact-zero tests deliberately refuse approximate equality/Hessian rays;
    // in particular a tiny nonzero movement against a finite bound is refused.
    let hd = accumulate_multiply(&data.p, direction, false, true)?;
    if hd.iter().any(|v| !v.exact_sign().is_ok_and(|s| s == std::cmp::Ordering::Equal)) {
        return Ok(None);
    }
    let ad = accumulate_multiply(&data.a, direction, false, false)?;
    for block in &data.blocks {
        let rows = &ad[block.start..block.start + block.len];
        match block.kind {
            Kind::Zero => {
                if rows.iter().any(|r| !r.exact_sign().is_ok_and(|s| s == std::cmp::Ordering::Equal)) {
                    return Ok(None);
                }
            }
            Kind::Nonnegative => {
                if rows.iter().any(|r| !r.exact_sign().is_ok_and(|s| s != std::cmp::Ordering::Greater)) {
                    return Ok(None);
                }
            }
            Kind::Lorentz => {
                if rows.iter().all(|r| r.exact_sign().is_ok_and(|s| s == std::cmp::Ordering::Equal)) {
                    continue;
                }
                let head = -rows[0].enclosure()?.1;
                let tail = rows[1..].iter().map(|r| r.enclosure().map(|(lo, hi)| lo.abs().max(hi.abs()))).collect::<Result<Vec<_>>>()?;
                let radius = (norm(&tail) * (1.0 + 16.0 * f64::EPSILON * tail.len() as f64)).next_up();
                if head < radius {
                    return Ok(None);
                }
            }
        }
    }
    let descent = dot(&data.q, direction)?;
    let scale = compensated_sum(data.q.iter().zip(direction).map(|(q, d)| (q * d).abs()))?;
    if descent >= -128.0 * f64::EPSILON * scale {
        return Ok(None);
    }
    let anchor = model
        .base
        .variables
        .iter()
        .map(|v| {
            let low = v.lower.unwrap_or(f64::NEG_INFINITY);
            let high = v.upper.unwrap_or(f64::INFINITY);
            0.0_f64.max(low).min(high)
        })
        .collect::<Vec<_>>();
    if !validate_conic_solution(model, &anchor, options)?.0 {
        return Ok(None);
    }
    Ok(Some(anchor))
}
fn finish(
    model: &ConicModel,
    options: &SolveOptions,
    data: &Data,
    solution: &clarabel::solver::DefaultSolution<f64>,
    cancelled: bool,
) -> Result<ConicDetailedResult> {
    let status = solution.status;
    let termination = if cancelled {
        Termination::Cancelled
    } else {
        match status {
            SolverStatus::MaxIterations => Termination::IterationLimit,
            SolverStatus::MaxTime => Termination::TimeLimit,
            SolverStatus::NumericalError | SolverStatus::InsufficientProgress => Termination::NumericalFailure,
            SolverStatus::CallbackTerminated => Termination::EvaluationLimit,
            _ => Termination::Completed,
        }
    };
    let mut output = stopped(termination);
    output.backend_status = status.to_string();
    output.result.statistics.iterations = u64::from(solution.iterations);
    output.result.statistics.evaluations = u64::from(solution.iterations);
    if matches!(
        status,
        SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible | SolverStatus::DualInfeasible | SolverStatus::AlmostDualInfeasible
    ) {
        if matches!(status, SolverStatus::PrimalInfeasible | SolverStatus::AlmostPrimalInfeasible)
            && infeasibility_certificate(model, data, &solution.z)?
        {
            output.result.feasibility = Feasibility::Infeasible;
            output.result.warnings.push("Infeasibility ray independently separated from the original variable box.".into());
            return Ok(output);
        }
        if matches!(status, SolverStatus::DualInfeasible | SolverStatus::AlmostDualInfeasible)
            && let Some(anchor) = unbounded_certificate(model, options, data, &solution.x)?
        {
            output.result = crate::verified_result(&model.base, options, anchor, Optimality::Unbounded)?;
            output.result.termination = termination;
            output.result.engine = ENGINE.into();
            output.result.statistics.iterations = u64::from(solution.iterations);
            output.result.statistics.evaluations = u64::from(solution.iterations);
            output.result.warnings=vec!["Unboundedness independently checked from an original-domain feasible anchor and an exact-zero recession direction, not numeric magnitude.".into()];
            return Ok(output);
        }
        // Approximate rays need additional domain-aware proof before deciding original feasibility.
        output.result.warnings.push(
            "Backend returned an infeasibility ray. Original-domain infeasibility/unboundedness is not inferred from an unchecked approximate ray."
                .into(),
        );
        return Ok(output);
    }
    if solution.x.iter().any(|v| !v.is_finite()) || solution.z.iter().any(|v| !v.is_finite()) {
        return Ok(output);
    }
    let (feasible, violation) = validate_conic_solution(model, &solution.x, options)?;
    output.result.max_violation = Some(violation);
    if !feasible {
        return Ok(output);
    }
    output.result.values = Some(solution.x.clone());
    output.result.objective = Some(evaluate_objective(&model.base, &solution.x)?);
    output.result.feasibility = Feasibility::Feasible;
    output.result.optimality = Optimality::BestFound;
    let cert = match certificate(model, data, &solution.x, &solution.z) {
        Ok(certificate) => certificate,
        Err(error) => {
            if output.result.termination == Termination::Completed {
                output.result.termination = Termination::NumericalFailure;
            }
            output.result.warnings.push(format!("Verified primal point retained, but dual certificate arithmetic failed: {error}"));
            return Ok(output);
        }
    };
    if let Some(bound) = cert.working_dual_bound {
        output.result.best_bound = crate::reported_bound(&model.base, data.direction * bound).map(|p| p.0);
        let gap = cert.absolute_gap.unwrap_or(f64::INFINITY) / (1.0_f64.max(cert.working_primal_objective.abs()));
        output.result.relative_gap = Some(gap);
        if cert.primal_residual <= options.feasibility_tolerance
            && cert.stationarity_residual <= options.optimality_tolerance
            && cert.absolute_gap.is_some_and(|g| g <= options.optimality_tolerance || gap <= options.relative_gap)
        {
            output.result.optimality = Optimality::Optimal;
        }
    } else if cert.stationarity_residual <= options.optimality_tolerance && cert.complementarity_residual <= options.optimality_tolerance {
        output.result.optimality = Optimality::Stationary;
        output
            .result
            .warnings
            .push("A finite global dual bound could not be independently restored for free coordinates; stationary status is retained.".into());
    }
    output.certificate = Some(cert);
    Ok(output)
}
/// Owned callbacks avoid hidden lifetime casts and work on native Rust and WASM.
/// The backend has no public warm-start API; none is claimed here.
pub fn solve_socp_owned(model: &ConicModel, options: &SolveOptions, control: Arc<dyn RunControl>) -> Result<ConicDetailedResult> {
    if control.is_cancelled() {
        return Ok(stopped(Termination::Cancelled));
    }
    let data = match build(model, options, control.as_ref()) {
        Err(e) if e.code == "cancelled" => return Ok(stopped(Termination::Cancelled)),
        other => other?,
    };
    if options.max_iterations == 0 {
        return Ok(stopped(Termination::IterationLimit));
    }
    if options.max_evaluations == 0 {
        return Ok(stopped(Termination::EvaluationLimit));
    }
    let settings = DefaultSettings {
        verbose: false,
        max_iter: options.max_iterations.min(u64::from(u32::MAX)) as u32,
        // The public result also includes an independently rounded dual bound.
        // Leave numerical room for that proof instead of stopping as soon as
        // the backend's differently scaled internal gap reaches the user limit.
        tol_gap_abs: (options.optimality_tolerance * 0.01).max(f64::MIN_POSITIVE).min(options.optimality_tolerance),
        tol_gap_rel: (options.relative_gap * 0.01).max(1e-14),
        tol_feas: (options.feasibility_tolerance.min(options.optimality_tolerance) * 0.01)
            .max(f64::MIN_POSITIVE)
            .min(options.feasibility_tolerance.min(options.optimality_tolerance)),
        presolve_enable: false,
        max_threads: 1,
        ..DefaultSettings::default()
    };
    let mut solver = DefaultSolver::new(&data.p, &data.q, &data.a, &data.b, &data.cones, settings)
        .map_err(|e| SolverError::new("conic_setup", format!("Sparse conic backend rejected setup: {e}")))?;
    let callback = Arc::clone(&control);
    let max_evaluations = options.max_evaluations;
    solver.set_termination_callback(move |info| {
        callback.progress(&ProgressEvent {
            stage: "conic_interior_point".into(),
            iterations: u64::from(info.iterations),
            nodes: 0,
            evaluations: u64::from(info.iterations),
            best_objective: None,
        });
        callback.is_cancelled() || u64::from(info.iterations) >= max_evaluations
    });
    solver.solve();
    finish(model, options, &data, &solver.solution, control.is_cancelled())
}
/// Explicit sparse LP/QP route; it uses actual CSC matrices and a sparse KKT factor.
pub fn solve_sparse_owned(model: &OptimizationModel, options: &SolveOptions, control: Arc<dyn RunControl>) -> Result<ConicDetailedResult> {
    if control.is_cancelled() {
        return Ok(stopped(Termination::Cancelled));
    }
    validate(model, options)?;
    solve_socp_owned(&ConicModel { base: model.clone(), second_order_cones: Vec::new() }, options, control)
}

/// Borrowed callback compatibility on native targets. A scoped worker executes the
/// backend; this caller polls cancellation and forwards progress without unsafe code.
#[cfg(not(target_arch = "wasm32"))]
pub fn solve_socp_detailed(model: &ConicModel, options: &SolveOptions, control: &dyn RunControl) -> Result<ConicDetailedResult> {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    struct Bridge {
        cancel: AtomicBool,
        progress: Mutex<Option<ProgressEvent>>,
    }
    impl RunControl for Bridge {
        fn is_cancelled(&self) -> bool {
            self.cancel.load(Ordering::Relaxed)
        }
        fn progress(&self, p: &ProgressEvent) {
            if let Ok(mut pending) = self.progress.lock() {
                *pending = Some(p.clone());
            }
        }
    }
    let bridge = Arc::new(Bridge { cancel: AtomicBool::new(control.is_cancelled()), progress: Mutex::new(None) });
    std::thread::scope(|scope| {
        let worker_bridge = Arc::clone(&bridge);
        let worker = scope.spawn(move || solve_socp_owned(model, options, worker_bridge));
        while !worker.is_finished() {
            if control.is_cancelled() {
                bridge.cancel.store(true, Ordering::Relaxed);
            }
            let pending = bridge.progress.lock().ok().and_then(|mut p| p.take());
            if let Some(p) = pending {
                control.progress(&p);
            }
            std::thread::park_timeout(std::time::Duration::from_millis(1));
        }
        worker.join().map_err(|_| SolverError::new("backend_panic", "Sparse conic backend worker panicked."))?
    })
}
#[cfg(target_arch = "wasm32")]
pub fn solve_socp_detailed(_model: &ConicModel, _options: &SolveOptions, _control: &dyn RunControl) -> Result<ConicDetailedResult> {
    Err(SolverError::new("owned_control_required", "WASM conic solves require solve_socp_owned with Arc<dyn RunControl>."))
}

#[cfg(test)]
mod arithmetic_proof_tests {
    use super::*;
    #[test]
    fn underflowed_positive_blocker_is_not_an_exact_recession_zero() -> Result<()> {
        let base = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: "underflow recession blocker".into(),
            variables: vec![Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(0.0), upper: None }],
            objective: Objective {
                direction: Direction::Minimize,
                offset: 0.0,
                linear: vec![LinearTerm { variable: 0, coefficient: -1e308 }],
                quadratic: Vec::new(),
            },
            constraints: vec![LinearConstraint {
                name: "finite mathematical upper bound".into(),
                terms: vec![LinearTerm { variable: 0, coefficient: 1e-320 }],
                relation: Relation::LessEqual,
                rhs: 1.0,
            }],
        };
        let model = ConicModel { base, second_order_cones: Vec::new() };
        let options = SolveOptions::default();
        let data = build(&model, &options, &NoControl)?;
        // This positive ray gives positive lhs growth even though its product
        // underflows if multiplied in binary64 before the proof checks it.
        assert_eq!(1e-320_f64 * 1e-308_f64, 0.0);
        assert!(unbounded_certificate(&model, &options, &data, &[1e-308])?.is_none());
        Ok(())
    }
}
