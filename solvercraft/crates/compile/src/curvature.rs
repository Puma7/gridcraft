//! Sufficient structural curvature rules. `Unknown` is never a nonconvexity
//! proof. Classifications apply on the expression's natural domain; domain and
//! feasibility checks remain mandatory before optimization.
use super::{Expr, MAX_EXPRESSION_NODES, preflight};
use serde::{Deserialize, Serialize};
use solvercraft_types::Result;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curvature {
    Affine,
    Convex,
    Concave,
    Unknown,
}
fn flip(c: Curvature) -> Curvature {
    match c {
        Curvature::Convex => Curvature::Concave,
        Curvature::Concave => Curvature::Convex,
        other => other,
    }
}
fn plus(a: Curvature, b: Curvature) -> Curvature {
    match (a, b) {
        (Curvature::Affine, v) | (v, Curvature::Affine) => v,
        (a, b) if a == b => a,
        _ => Curvature::Unknown,
    }
}
fn scale(c: Curvature, k: f64) -> Curvature {
    if c == Curvature::Unknown {
        return Curvature::Unknown;
    }
    if k == 0.0 {
        Curvature::Affine
    } else if k > 0.0 {
        c
    } else {
        flip(c)
    }
}
fn constant(expr: &Expr) -> Option<f64> {
    // Literal constants introduce no hidden domain guards and need no probes.
    if let Expr::Constant { value } = expr { Some(*value) } else { None }
}
fn infer(expr: &Expr) -> Result<Curvature> {
    use Curvature::*;
    Ok(match expr {
        Expr::Constant { .. } | Expr::Variable { .. } => Affine,
        Expr::Add { left, right } => plus(infer(left)?, infer(right)?),
        Expr::Subtract { left, right } => plus(infer(left)?, flip(infer(right)?)),
        Expr::Negate { value } => flip(infer(value)?),
        Expr::Multiply { left, right } => {
            if let Some(k) = constant(left) {
                scale(infer(right)?, k)
            } else if let Some(k) = constant(right) {
                scale(infer(left)?, k)
            } else {
                Unknown
            }
        }
        Expr::Divide { left, right } => {
            if let Some(k) = constant(right) {
                if k == 0.0 { Unknown } else { scale(infer(left)?, 1.0 / k) }
            } else {
                Unknown
            }
        }
        Expr::Power { base, exponent: 0 } => {
            if infer(base)? == Unknown {
                Unknown
            } else {
                Affine
            }
        }
        Expr::Power { base, exponent: 1 } => infer(base)?,
        Expr::Power { base, exponent } => {
            if exponent % 2 == 0 && infer(base)? == Affine {
                Convex
            } else {
                Unknown
            }
        }
        Expr::Exp { value } => {
            if matches!(infer(value)?, Affine | Convex) {
                Convex
            } else {
                Unknown
            }
        }
        Expr::Ln { value } | Expr::Sqrt { value } => {
            if matches!(infer(value)?, Affine | Concave) {
                Concave
            } else {
                Unknown
            }
        }
        Expr::Abs { value } => {
            if infer(value)? == Affine {
                Convex
            } else {
                Unknown
            }
        }
        Expr::Maximum { left, right } => {
            if matches!(infer(left)?, Affine | Convex) && matches!(infer(right)?, Affine | Convex) {
                Convex
            } else {
                Unknown
            }
        }
        Expr::Minimum { left, right } => {
            if matches!(infer(left)?, Affine | Concave) && matches!(infer(right)?, Affine | Concave) {
                Concave
            } else {
                Unknown
            }
        }
        Expr::Sin { .. } | Expr::Cos { .. } => Unknown,
    })
}
/// Conservative composition-based classification, without numerical probing.
/// It may return Unknown for a convex algebraically equivalent expression.
pub fn curvature(expr: &Expr, variables: usize) -> Result<Curvature> {
    if variables > solvercraft_model::MAX_VARIABLES {
        return Err(super::error("model_limit", "too many curvature variables"));
    }
    let mut remaining = MAX_EXPRESSION_NODES;
    preflight(expr, variables, &mut remaining)?;
    infer(expr)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn x() -> Expr {
        Expr::variable(0)
    }
    #[test]
    fn smooth_compositions_have_sufficient_proofs() {
        assert_eq!(curvature(&Expr::Exp { value: Box::new(Expr::Power { base: Box::new(x()), exponent: 2 }) }, 1).unwrap(), Curvature::Convex);
        assert_eq!(curvature(&Expr::Ln { value: Box::new(x()) }, 1).unwrap(), Curvature::Concave);
        assert_eq!(curvature(&Expr::Negate { value: Box::new(Expr::Power { base: Box::new(x()), exponent: 2 }) }, 1).unwrap(), Curvature::Concave);
    }
    #[test]
    fn ambiguous_products_and_sampling_disguises_stay_unknown() {
        assert_eq!(curvature(&Expr::Sin { value: Box::new(x()) }, 1).unwrap(), Curvature::Unknown);
        assert_eq!(curvature(&Expr::Multiply { left: Box::new(x()), right: Box::new(x()) }, 1).unwrap(), Curvature::Unknown);
        assert_eq!(curvature(&Expr::Power { base: Box::new(x()), exponent: 3 }, 1).unwrap(), Curvature::Unknown);
    }
    #[test]
    fn bad_handles_are_not_hidden_by_curvature_rules() {
        assert!(curvature(&Expr::Power { base: Box::new(Expr::variable(1)), exponent: 0 }, 1).is_err());
    }
}
