//! Bounded reverse-mode differentiation for the supported expression operators.
use super::{Expr, MAX_EXPRESSION_NODES, checked, error, preflight};
use solvercraft_types::Result;

struct Node {
    value: f64,
    parents: [Option<(usize, f64)>; 2],
    variable: Option<usize>,
    active: bool,
}
fn node(tape: &[Node], index: usize) -> Result<&Node> {
    tape.get(index).ok_or_else(|| error("internal_error", "invalid differentiation tape index"))
}
fn push(tape: &mut Vec<Node>, value: f64, parents: [Option<(usize, f64)>; 2], variable: Option<usize>) -> Result<usize> {
    if tape.len() >= MAX_EXPRESSION_NODES {
        return Err(error("expression_limit", "differentiation tape budget exceeded"));
    }
    checked(value, "differentiation value")?;
    let mut active = variable.is_some();
    for (index, derivative) in parents.iter().flatten() {
        checked(*derivative, "local derivative")?;
        active |= node(tape, *index)?.active;
    }
    let index = tape.len();
    tape.push(Node { value, parents, variable, active });
    Ok(index)
}
fn record(expr: &Expr, values: &[f64], tape: &mut Vec<Node>) -> Result<usize> {
    match expr {
        Expr::Constant { value } => push(tape, *value, [None, None], None),
        Expr::Variable { index } => {
            let value = values.get(*index).copied().ok_or_else(|| error("invalid_variable", "missing differentiation variable"))?;
            push(tape, value, [None, None], Some(*index))
        }
        Expr::Power { base, exponent: 0 } => {
            // Keep the value/domain evaluation of the base, without requiring
            // a derivative for a subtree whose sensitivity is exactly zero.
            super::eval_inner(base, values)?;
            push(tape, 1.0, [None, None], None)
        }
        Expr::Add { left, right }
        | Expr::Subtract { left, right }
        | Expr::Multiply { left, right }
        | Expr::Divide { left, right }
        | Expr::Minimum { left, right }
        | Expr::Maximum { left, right } => {
            let li = record(left, values, tape)?;
            let ri = record(right, values, tape)?;
            let (l, r) = (node(tape, li)?.value, node(tape, ri)?.value);
            let active = node(tape, li)?.active || node(tape, ri)?.active;
            let (value, dl, dr) = match expr {
                Expr::Add { .. } => (l + r, 1.0, 1.0),
                Expr::Subtract { .. } => (l - r, 1.0, -1.0),
                Expr::Multiply { .. } => (l * r, r, l),
                Expr::Divide { .. } => {
                    if r == 0.0 {
                        return Err(error("domain_error", "division by zero"));
                    }
                    (l / r, 1.0 / r, -(l / r) / r)
                }
                Expr::Minimum { .. } | Expr::Maximum { .. } => {
                    if active && l == r {
                        return Err(error("nondifferentiable", "minimum/maximum is tied at this point"));
                    }
                    let left_wins = if matches!(expr, Expr::Minimum { .. }) { l <= r } else { l >= r };
                    if left_wins { (l, 1.0, 0.0) } else { (r, 0.0, 1.0) }
                }
                _ => {
                    return Err(error("internal_error", "invalid binary differentiation operator"));
                }
            };
            let dl = if node(tape, li)?.active { dl } else { 0.0 };
            let dr = if node(tape, ri)?.active { dr } else { 0.0 };
            push(tape, value, [Some((li, dl)), Some((ri, dr))], None)
        }
        _ => {
            let child = super::unary_child(expr).ok_or_else(|| error("internal_error", "invalid unary differentiation operator"))?;
            let ci = record(child, values, tape)?;
            let child = node(tape, ci)?;
            let v = child.value;
            let active = child.active;
            let (value, derivative) = match expr {
                Expr::Negate { .. } => (-v, -1.0),
                Expr::Power { exponent, .. } => {
                    let value = v.powf(f64::from(*exponent));
                    let derivative = if !active || *exponent == 0 { 0.0 } else { f64::from(*exponent) * v.powf(f64::from(*exponent - 1)) };
                    (value, derivative)
                }
                Expr::Sin { .. } => (v.sin(), v.cos()),
                Expr::Cos { .. } => (v.cos(), -v.sin()),
                Expr::Exp { .. } => (v.exp(), v.exp()),
                Expr::Ln { .. } => {
                    if v <= 0.0 {
                        return Err(error("domain_error", "logarithm requires a positive value"));
                    }
                    (v.ln(), if active { 1.0 / v } else { 0.0 })
                }
                Expr::Sqrt { .. } => {
                    if v < 0.0 {
                        return Err(error("domain_error", "square root requires a nonnegative value"));
                    }
                    if active && v == 0.0 {
                        return Err(error("nondifferentiable", "square root derivative is singular at zero"));
                    }
                    let value = v.sqrt();
                    (value, if active { 0.5 / value } else { 0.0 })
                }
                Expr::Abs { .. } => {
                    if active && v == 0.0 {
                        return Err(error("nondifferentiable", "absolute value is nondifferentiable at zero"));
                    }
                    (
                        v.abs(),
                        if v > 0.0 {
                            1.0
                        } else if v < 0.0 {
                            -1.0
                        } else {
                            0.0
                        },
                    )
                }
                _ => {
                    return Err(error("internal_error", "unsupported unary differentiation operator"));
                }
            };
            push(tape, value, [Some((ci, if active { derivative } else { 0.0 })), None], None)
        }
    }
}

/// Evaluate an expression and its exact operator-chain gradient (up to floating
/// point arithmetic), using reverse mode. No finite-difference step is used.
/// Nonsmooth ties, active abs(0), and active sqrt(0) return `nondifferentiable`;
/// this API never silently substitutes a zero gradient for a failed derivative.
pub fn evaluate_with_gradient(expr: &Expr, values: &[f64]) -> Result<(f64, Vec<f64>)> {
    if values.len() > solvercraft_model::MAX_VARIABLES {
        return Err(error("model_limit", "too many differentiation variables"));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(error("non_finite", "differentiation inputs must be finite"));
    }
    let mut remaining = MAX_EXPRESSION_NODES;
    preflight(expr, values.len(), &mut remaining)?;
    let mut tape = Vec::new();
    let root = record(expr, values, &mut tape)?;
    let value = node(&tape, root)?.value;
    let mut adjoints = vec![0.0; tape.len()];
    let seed = adjoints.get_mut(root).ok_or_else(|| error("internal_error", "missing differentiation root"))?;
    *seed = 1.0;
    let mut gradient = vec![solvercraft_model::CompensatedSum::default(); values.len()];
    for index in (0..tape.len()).rev() {
        let n = node(&tape, index)?;
        let adjoint = adjoints.get(index).copied().ok_or_else(|| error("internal_error", "missing adjoint"))?;
        if adjoint == 0.0 {
            continue;
        }
        if let Some(variable) = n.variable {
            let g = gradient.get_mut(variable).ok_or_else(|| error("internal_error", "missing gradient variable"))?;
            g.add(adjoint)?;
        }
        for (parent, coefficient) in n.parents.iter().flatten() {
            let target = adjoints.get_mut(*parent).ok_or_else(|| error("internal_error", "missing parent adjoint"))?;
            *target = checked(*target + adjoint * coefficient, "adjoint accumulation")?;
        }
    }
    Ok((value, gradient.into_iter().map(solvercraft_model::CompensatedSum::finish).collect::<Result<Vec<_>>>()?))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn x(i: usize) -> Expr {
        Expr::variable(i)
    }
    fn c(v: f64) -> Expr {
        Expr::constant(v)
    }
    fn add(a: Expr, b: Expr) -> Expr {
        Expr::Add { left: Box::new(a), right: Box::new(b) }
    }
    fn mul(a: Expr, b: Expr) -> Expr {
        Expr::Multiply { left: Box::new(a), right: Box::new(b) }
    }
    #[test]
    fn quadratic_cross_terms_have_analytic_gradients() {
        let expr = add(mul(x(0), x(0)), mul(c(3.0), mul(x(0), x(1))));
        let (value, g) = evaluate_with_gradient(&expr, &[2.0, -4.0]).unwrap();
        assert_eq!(value, -20.0);
        assert_eq!(g, vec![-8.0, 6.0]);
    }
    #[test]
    fn narrow_interval_example_has_the_nonzero_true_derivative() {
        let expr = Expr::Power { base: Box::new(add(mul(c(1e7), x(0)), c(-4.0))), exponent: 2 };
        let (value, g) = evaluate_with_gradient(&expr, &[0.0]).unwrap();
        assert_eq!(value, 16.0);
        assert_eq!(g, vec![-8e7]);
    }
    #[test]
    fn nonlinear_chain_matches_analytic_result_and_independent_differences() {
        let expr = add(Expr::Sin { value: Box::new(x(0)) }, Expr::Ln { value: Box::new(Expr::Exp { value: Box::new(mul(x(0), x(1))) }) });
        let values = [0.4, 1.5];
        let (_, g) = evaluate_with_gradient(&expr, &values).unwrap();
        assert!((g[0] - (values[0].cos() + values[1])).abs() < 1e-12);
        assert!((g[1] - values[0]).abs() < 1e-12);
        for (index, actual) in g.iter().enumerate() {
            let mut a = values;
            let mut b = values;
            a[index] -= 1e-5;
            b[index] += 1e-5;
            let expected = (super::super::evaluate(&expr, &b).unwrap() - super::super::evaluate(&expr, &a).unwrap()) / 2e-5;
            assert!((actual - expected).abs() < 1e-9);
        }
    }
    #[test]
    fn unavailable_derivatives_are_errors_not_zero() {
        for expr in [
            Expr::Abs { value: Box::new(x(0)) },
            Expr::Sqrt { value: Box::new(x(0)) },
            Expr::Minimum { left: Box::new(x(0)), right: Box::new(c(0.0)) },
        ] {
            assert_eq!(evaluate_with_gradient(&expr, &[0.0]).unwrap_err().code, "nondifferentiable");
        }
    }
    #[test]
    fn constants_and_inactive_zero_are_differentiable() {
        let expr = Expr::Sqrt { value: Box::new(c(0.0)) };
        assert_eq!(evaluate_with_gradient(&expr, &[2.0]).unwrap(), (0.0, vec![0.0]));
        let expr = Expr::Power { base: Box::new(x(0)), exponent: 0 };
        assert_eq!(evaluate_with_gradient(&expr, &[0.0]).unwrap(), (1.0, vec![0.0]));
    }
}
