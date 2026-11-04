//! Bound-aware finite differences for evaluators without an analytic derivative.
use solvercraft_types::{Result, SolverError};

#[derive(Debug, Clone)]
pub struct FiniteDifferenceResult {
    pub value: f64,
    pub gradient: Vec<f64>,
    pub evaluations: u64,
    pub one_sided_components: usize,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn call<F: FnMut(&[f64]) -> Result<f64>>(f: &mut F, x: &[f64], count: &mut u64) -> Result<f64> {
    *count = count.saturating_add(1);
    let value = f(x)?;
    if !value.is_finite() {
        return Err(error("non_finite", "finite-difference evaluation is not finite"));
    }
    Ok(value)
}
/// Estimate derivatives with unequal central steps, adapting to finite bounds.
/// A failed side may use a valid one-sided slope; two failed sides are an error.
/// Fixed variables have zero feasible sensitivity. No failed derivative becomes zero.
pub fn finite_difference_gradient<F>(
    values: &[f64],
    lower: &[Option<f64>],
    upper: &[Option<f64>],
    relative_step: f64,
    mut f: F,
) -> Result<FiniteDifferenceResult>
where
    F: FnMut(&[f64]) -> Result<f64>,
{
    if values.len() > solvercraft_model::MAX_VARIABLES || values.len() != lower.len() || values.len() != upper.len() {
        return Err(error("invalid_values", "finite-difference bounds and values must have matching bounded dimensions"));
    }
    if !relative_step.is_finite() || relative_step <= 0.0 || relative_step > 0.25 {
        return Err(error("invalid_options", "finite-difference relative step must be in (0,0.25]"));
    }
    for ((x, lo), hi) in values.iter().zip(lower).zip(upper) {
        if !x.is_finite() || lo.is_some_and(|v| !v.is_finite()) || hi.is_some_and(|v| !v.is_finite()) {
            return Err(error("non_finite", "finite-difference values and supplied bounds must be finite"));
        }
        if lo.is_some_and(|v| *x < v) || hi.is_some_and(|v| *x > v) || lo.zip(*hi).is_some_and(|(l, h)| l > h) {
            return Err(error("invalid_bounds", "finite-difference point must lie within consistent bounds"));
        }
    }
    let mut evaluations = 0;
    let value = call(&mut f, values, &mut evaluations)?;
    let mut gradient = Vec::with_capacity(values.len());
    let mut one_sided_components = 0;
    for (index, ((x, lo), hi)) in values.iter().zip(lower).zip(upper).enumerate() {
        if lo.zip(*hi).is_some_and(|(l, h)| l == h) {
            gradient.push(0.0);
            continue;
        }
        let mut step = relative_step * x.abs().max(1.0);
        if let Some((lo, hi)) = lo.zip(*hi) {
            let width = hi - lo;
            if width.is_finite() && width > 0.0 {
                step = step.min(width * 0.001);
            }
        }
        let a = lo.map_or(*x - step, |v| (*x - step).max(v));
        let b = hi.map_or(*x + step, |v| (*x + step).min(v));
        let a = if a >= *x { x.next_down() } else { a };
        let b = if b <= *x { x.next_up() } else { b };
        let mut sample = values.to_vec();
        let left = if a.is_finite() && a < *x && lo.is_none_or(|v| a >= v) {
            if let Some(v) = sample.get_mut(index) {
                *v = a;
            }
            match call(&mut f, &sample, &mut evaluations) {
                Ok(v) => Some(((*x - a), (value - v) / (*x - a))),
                Err(e) if matches!(e.code.as_str(), "cancelled" | "evaluation_limit") => return Err(e),
                Err(_) => None,
            }
        } else {
            None
        };
        if let Some(v) = sample.get_mut(index) {
            *v = *x;
        }
        let right = if b.is_finite() && b > *x && hi.is_none_or(|v| b <= v) {
            if let Some(v) = sample.get_mut(index) {
                *v = b;
            }
            match call(&mut f, &sample, &mut evaluations) {
                Ok(v) => Some(((b - *x), (v - value) / (b - *x))),
                Err(e) if matches!(e.code.as_str(), "cancelled" | "evaluation_limit") => return Err(e),
                Err(_) => None,
            }
        } else {
            None
        };
        // A point one ULP inside a finite bound must not give nearly all
        // central weight to that microscopic side: f(x)-f(neighbor) can round
        // to zero even for a large true slope, especially with an offset.
        // A second-order estimate on the resolved side avoids that artificial
        // stationary point. Moderately unequal steps retain the central rule.
        let (left, right) = match (left, right) {
            (Some((ha, _)), Some((hb, _))) if ha < hb * 0.125 => (None, right),
            (Some((ha, _)), Some((hb, _))) if hb < ha * 0.125 => (left, None),
            pair => pair,
        };
        let derivative = match (left, right) {
            (Some((ha, da)), Some((hb, db))) => {
                let scale = ha.max(hb);
                let weight = (hb / scale) / (ha / scale + hb / scale);
                weight * da + (1.0 - weight) * db
            }
            (Some((h, d)), None) => {
                one_sided_components += 1;
                one_sided(&mut f, values, Side { index, h, slope: d, sign: -1.0, bound: *lo, base: value }, &mut evaluations)?
            }
            (None, Some((h, d))) => {
                one_sided_components += 1;
                one_sided(&mut f, values, Side { index, h, slope: d, sign: 1.0, bound: *hi, base: value }, &mut evaluations)?
            }
            _ => return Err(error("derivative_unavailable", "no valid feasible perturbation for a nonfixed variable")),
        };
        if !derivative.is_finite() {
            return Err(error("derivative_unavailable", "finite-difference slope is not representable"));
        }
        gradient.push(derivative);
    }
    Ok(FiniteDifferenceResult { value, gradient, evaluations, one_sided_components })
}
struct Side {
    index: usize,
    h: f64,
    slope: f64,
    sign: f64,
    bound: Option<f64>,
    base: f64,
}
fn one_sided<F: FnMut(&[f64]) -> Result<f64>>(f: &mut F, values: &[f64], side: Side, count: &mut u64) -> Result<f64> {
    let x = values.get(side.index).copied().ok_or_else(|| error("invalid_values", "missing difference coordinate"))?;
    let proposed = x + side.sign * (2.0 * side.h);
    let p = side.bound.map_or(proposed, |v| if side.sign > 0.0 { proposed.min(v) } else { proposed.max(v) });
    let h2 = (p - x).abs();
    if !p.is_finite() || h2 <= side.h {
        return Ok(side.slope);
    }
    let mut sample = values.to_vec();
    if let Some(v) = sample.get_mut(side.index) {
        *v = p;
    }
    let second = match call(f, &sample, count) {
        Ok(v) => v,
        Err(e) if matches!(e.code.as_str(), "cancelled" | "evaluation_limit") => return Err(e),
        Err(_) => return Ok(side.slope),
    };
    let d2 = side.sign * (second - side.base) / h2;
    Ok((h2 / (h2 - side.h)) * side.slope - (side.h / (h2 - side.h)) * d2)
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn narrow_box_has_a_nonzero_feasible_derivative() {
        let d = finite_difference_gradient(&[0.0], &[Some(0.0)], &[Some(5e-7)], 1e-6, |x| Ok((1e7 * x[0] - 4.0).powi(2))).unwrap();
        assert!(d.gradient[0] < -1e7);
        assert_eq!(d.one_sided_components, 1);
    }
    #[test]
    fn unequal_central_steps_differentiate_quadratic() {
        let d = finite_difference_gradient(&[0.9999999], &[Some(0.0)], &[Some(1.0)], 1e-6, |x| Ok(x[0] * x[0])).unwrap();
        assert!((d.gradient[0] - 1.9999998).abs() < 1e-8);
    }
    #[test]
    fn domain_failure_uses_valid_side_and_two_failures_are_errors() {
        let d =
            finite_difference_gradient(
                &[1e-8],
                &[None],
                &[None],
                1e-6,
                |x| if x[0] > 0.0 { Ok(x[0].ln()) } else { Err(error("domain_error", "log")) },
            )
            .unwrap();
        assert!(d.gradient[0] > 0.0);
        assert_eq!(d.one_sided_components, 1);
        assert_eq!(
            finite_difference_gradient(&[0.0], &[None], &[None], 1e-6, |x| if x[0] == 0.0 {
                Ok(1.0)
            } else {
                Err(error("domain_error", "isolated"))
            })
            .unwrap_err()
            .code,
            "derivative_unavailable"
        );
    }
    #[test]
    fn fixed_variable_and_cancellation_are_distinct() {
        let d = finite_difference_gradient(&[2.0], &[Some(2.0)], &[Some(2.0)], 1e-6, |_| Ok(4.0)).unwrap();
        assert_eq!(d.gradient, vec![0.0]);
        assert_eq!(d.evaluations, 1);
        assert_eq!(finite_difference_gradient(&[2.0], &[None], &[None], 1e-6, |_| Err(error("cancelled", "stop"))).unwrap_err().code, "cancelled");
    }
    #[test]
    fn invalid_bounds_and_nonfinite_evaluations_fail() {
        assert!(finite_difference_gradient(&[0.0], &[Some(1.0)], &[None], 1e-6, |_| Ok(0.0)).is_err());
        assert!(finite_difference_gradient(&[0.0], &[None], &[None], 1e-6, |_| Ok(f64::NAN)).is_err());
    }
}
