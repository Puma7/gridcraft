//! Preimages of the actual rounded affine map used by discrete samples.
const SIGN: u64 = 1 << 63;
use solvercraft_types::Distribution as D;
pub(crate) fn mirrored(d: &D) -> Option<D> {
    Some(match d {
        D::Constant { value } => D::Constant { value: -*value },
        D::Uniform { low, high } => D::Uniform { low: -*high, high: -*low },
        D::Triangular { low, mode, high } => D::Triangular { low: -*high, mode: -*mode, high: -*low },
        D::Normal { mean, std_dev } => D::Normal { mean: -*mean, std_dev: *std_dev },
        D::Cauchy { location, scale } => D::Cauchy { location: -*location, scale: *scale },
        D::Logistic { location, scale } => D::Logistic { location: -*location, scale: *scale },
        D::Laplace { location, scale } => D::Laplace { location: -*location, scale: *scale },
        D::HyperbolicSecant { location, scale } => D::HyperbolicSecant { location: -*location, scale: *scale },
        D::StudentT { degrees_of_freedom, location, scale } => {
            D::StudentT { degrees_of_freedom: *degrees_of_freedom, location: -*location, scale: *scale }
        }
        _ => return None,
    })
}
fn key(x: f64) -> u64 {
    let bits = x.to_bits();
    if bits & SIGN == 0 { bits ^ SIGN } else { !bits }
}
fn value(key: u64) -> f64 {
    f64::from_bits(if key & SIGN == 0 { !key } else { key ^ SIGN })
}
/// Inclusive finite-input interval mapped into [lower,upper]. Bisection on
/// representable inputs handles both inverse-rounding error and coalesced atoms.
pub(crate) fn preimage(lower: f64, upper: f64, shift: f64, scale: f64) -> Option<(f64, f64)> {
    if lower > upper {
        return None;
    }
    let map = |x: f64| scale.abs().mul_add(x, shift);
    let first = key(-f64::MAX);
    let last = key(f64::MAX);
    if map(value(last)) < lower || map(value(first)) > upper {
        return None;
    }
    let mut low = first;
    let mut high = last;
    while low < high {
        let mid = low + (high - low) / 2;
        if map(value(mid)) < lower {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    let left = value(low);
    low = first;
    high = last;
    while low < high {
        let mid = low + (high - low) / 2 + (high - low) % 2;
        if map(value(mid)) > upper {
            high = mid - 1;
        } else {
            low = mid;
        }
    }
    let right = value(low);
    let left = if left == -f64::MAX { f64::NEG_INFINITY } else { left };
    let right = if right == f64::MAX { f64::INFINITY } else { right };
    if left > right {
        None
    } else if scale > 0.0 {
        Some((left, right))
    } else {
        Some((-right, -left))
    }
}
