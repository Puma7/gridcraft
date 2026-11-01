//! Additional frozen-reference families, with independent public parameter semantics.
use crate::{distributions::inverse_normal, extended::standardize, special};
use solvercraft_types::{Distribution as D, Result, SolverError};
fn domain() -> SolverError {
    SolverError::new("distribution_domain", "Invalid frozen-reference distribution parameters or unsupported numerical range.")
}
fn shape(x: f64) -> bool {
    x.is_finite() && (1e-4..=1e4).contains(&x)
}
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}
pub fn contains(d: &D) -> bool {
    matches!(
        d,
        D::Burr12 { .. }
            | D::Dagum { .. }
            | D::DoubleTriangular { .. }
            | D::FatigueLife { .. }
            | D::Frechet { .. }
            | D::HyperbolicSecant { .. }
            | D::Kumaraswamy { .. }
            | D::Levy { .. }
            | D::Pareto2 { .. }
            | D::BetaPrime { .. }
            | D::Reciprocal { .. }
            | D::MinExtreme { .. }
            | D::Logarithmic { .. }
    )
}
pub fn validate(d: &D) -> Result<()> {
    let valid = match d {
        D::Burr12 { shape_c, shape_k, scale } => shape(*shape_c) && shape(*shape_k) && positive(*scale),
        D::Dagum { shape: a, power, scale } => shape(*a) && shape(*power) && positive(*scale),
        D::DoubleTriangular { low, mode, high, left_probability: p } => {
            low.is_finite()
                && mode.is_finite()
                && high.is_finite()
                && low < mode
                && mode < high
                && (high - low).is_finite()
                && p.is_finite()
                && *p > 0.0
                && *p < 1.0
        }
        D::FatigueLife { shape: a, scale } | D::Frechet { shape: a, scale } | D::Pareto2 { shape: a, scale } => shape(*a) && positive(*scale),
        D::HyperbolicSecant { location, scale } | D::Levy { location, scale } | D::MinExtreme { location, scale } => {
            location.is_finite() && positive(*scale)
        }
        D::Kumaraswamy { alpha, beta } => shape(*alpha) && shape(*beta),
        D::BetaPrime { alpha, beta, scale } => shape(*alpha) && shape(*beta) && positive(*scale),
        D::Reciprocal { low, high } => positive(*low) && positive(*high) && low < high,
        D::Logarithmic { probability: p } => positive(*p) && *p <= 0.99,
        _ => false,
    };
    if valid { Ok(()) } else { Err(domain()) }
}
fn range() -> SolverError {
    SolverError::new("numerical_range", "Frozen-reference distribution tail is outside supported finite arithmetic.")
}
pub(crate) fn softplus(x: f64) -> f64 {
    if x > 0.0 { x + (-x).exp().ln_1p() } else { x.exp().ln_1p() }
}
pub(crate) fn log_ratio(x: f64, scale: f64) -> f64 {
    let delta = x - scale;
    if delta.is_finite() && delta.abs() <= 0.5 * scale {
        return (delta / scale).ln_1p();
    }
    let ratio = x / scale;
    if ratio > 0.0 && ratio.is_finite() { ratio.ln() } else { x.ln() - scale.ln() }
}
fn ln_expm1(x: f64) -> f64 {
    if x > 40.0 { x + (-(-x).exp()).ln_1p() } else { x.exp_m1().ln() }
}
fn logseries_tail(p: f64, k: u64) -> Result<f64> {
    if k == 0 {
        return Ok(1.0);
    }
    let first = k + 1;
    let mut term = ((first as f64) * p.ln() - (first as f64).ln() - (-(-p).ln_1p()).ln()).exp();
    let mut sum = term;
    for i in first..first + 20000 {
        term *= p * (i as f64) / (i + 1) as f64;
        sum += term;
        if term <= sum * 2e-14 || term == 0.0 {
            return Ok(sum.clamp(0.0, 1.0));
        }
    }
    Err(SolverError::new("special_convergence", "Log-series tail did not converge within20000 terms."))
}
fn logseries_cdf(p: f64, k: u64) -> Result<f64> {
    if k == 0 {
        return Ok(0.0);
    }
    if k > 1000 {
        return Ok(1.0 - logseries_tail(p, k)?);
    }
    let mut term = -p / (-p).ln_1p();
    let mut terms = Vec::new();
    terms.try_reserve_exact(k as usize).map_err(|_| range())?;
    terms.push(term);
    for i in 1..k {
        term *= p * (i as f64) / (i + 1) as f64;
        terms.push(term);
    }
    Ok(special::finite_sum(terms)?.clamp(0.0, 1.0))
}
pub fn quantile(d: &D, p: f64) -> Result<f64> {
    validate(d)?;
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(domain());
    }
    let q = match d {
        D::Burr12 { shape_c, shape_k, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() + ln_expm1(-(-p).ln_1p() / shape_k) / shape_c).exp()
            }
        }
        D::Dagum { shape, power, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() - ln_expm1(-p.ln() / power) / shape).exp()
            }
        }
        D::DoubleTriangular { low, mode, high, left_probability: w } => {
            if p <= *w {
                low + (mode - low) * (p / w).sqrt()
            } else {
                high - (high - mode) * ((1.0 - p) / (1.0 - w)).sqrt()
            }
        }
        D::FatigueLife { shape, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() + 2.0 * (shape * inverse_normal(p)? / 2.0).asinh()).exp()
            }
        }
        D::Frechet { shape, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() - (-p.ln()).ln() / shape).exp()
            }
        }
        D::HyperbolicSecant { location, scale } => {
            if p == 0.0 || p == 1.0 {
                return Err(range());
            }
            let z = if p < 0.5 { (std::f64::consts::FRAC_PI_2 * p).tan().ln() } else { -((std::f64::consts::FRAC_PI_2 * (1.0 - p)).tan()).ln() };
            scale.mul_add(z, *location)
        }
        D::Kumaraswamy { alpha, beta } => {
            if p == 0.0 {
                0.0
            } else if p == 1.0 {
                1.0
            } else {
                let mass = -(((-p).ln_1p()) / beta).exp_m1();
                (mass.ln() / alpha).exp()
            }
        }
        D::Levy { location, scale } => {
            if p == 0.0 {
                *location
            } else {
                let z = inverse_normal(p * 0.5)?;
                scale.mul_add(1.0 / (z * z), *location)
            }
        }
        D::Pareto2 { shape, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() + ln_expm1(-(-p).ln_1p() / shape)).exp()
            }
        }
        D::BetaPrime { alpha, beta, scale } => {
            if p == 0.0 {
                0.0
            } else {
                (scale.ln() + special::beta_logit_quantile(*alpha, *beta, p)?).exp()
            }
        }
        D::Reciprocal { low, high } => {
            if p == 0.0 {
                return Ok(*low);
            }
            if p == 1.0 {
                return Ok(*high);
            }
            let span = log_ratio(*high, *low);
            let t = p * span;
            let q = if t < 700.0 { low + low * t.exp_m1() } else { p.mul_add(span, low.ln()).exp() };
            q.clamp(*low, *high)
        }
        D::MinExtreme { location, scale } => {
            if p == 0.0 || p == 1.0 {
                return Err(range());
            }
            scale.mul_add((-(-p).ln_1p()).ln(), *location)
        }
        D::Logarithmic { probability: q } => {
            if p == 1.0 {
                return Err(range());
            }
            let predicate = |k| -> Result<bool> { if p <= 0.5 { Ok(logseries_cdf(*q, k)? < p) } else { Ok(logseries_tail(*q, k)? > 1.0 - p) } };
            let mut upper = 1;
            while predicate(upper)? {
                upper *= 2;
                if upper > 100000 {
                    return Err(range());
                }
            }
            let mut lower = 1;
            while lower < upper {
                let mid = lower + (upper - lower) / 2;
                if predicate(mid)? { lower = mid + 1 } else { upper = mid }
            }
            lower as f64
        }
        _ => return Err(domain()),
    };
    if !q.is_finite()
        || (p > 0.0
            && p < 1.0
            && q == 0.0
            && !matches!(d, D::DoubleTriangular { .. } | D::HyperbolicSecant { .. } | D::Levy { .. } | D::MinExtreme { .. }))
    {
        Err(range())
    } else {
        Ok(q)
    }
}
pub fn cdf(d: &D, x: f64) -> Result<f64> {
    validate(d)?;
    if x.is_nan() {
        return Err(domain());
    }
    if x == f64::NEG_INFINITY {
        return Ok(0.0);
    }
    if x == f64::INFINITY {
        return Ok(1.0);
    }
    let p = match d {
        D::Burr12 { shape_c, shape_k, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                -(-shape_k * softplus(shape_c * log_ratio(x, *scale))).exp_m1()
            }
        }
        D::Dagum { shape, power, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                (-power * softplus(-shape * log_ratio(x, *scale))).exp()
            }
        }
        D::DoubleTriangular { low, mode, high, left_probability: w } => {
            if x <= *low {
                0.0
            } else if x >= *high {
                1.0
            } else if x <= *mode {
                w * ((x - low) / (mode - low)).powi(2)
            } else {
                1.0 - (1.0 - w) * ((high - x) / (high - mode)).powi(2)
            }
        }
        D::FatigueLife { shape, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                special::normal_cdf(2.0 * (0.5 * log_ratio(x, *scale)).sinh() / shape)?
            }
        }
        D::Frechet { shape, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                (-(-shape * log_ratio(x, *scale)).exp()).exp()
            }
        }
        D::HyperbolicSecant { location, scale } => {
            let z = standardize(x, *location, *scale);
            if z < 0.0 { std::f64::consts::FRAC_2_PI * z.exp().atan() } else { 1.0 - std::f64::consts::FRAC_2_PI * (-z).exp().atan() }
        }
        D::Kumaraswamy { alpha, beta } => {
            if x <= 0.0 {
                0.0
            } else if x >= 1.0 {
                1.0
            } else {
                -((-(x.powf(*alpha))).ln_1p() * beta).exp_m1()
            }
        }
        D::Levy { location, scale } => {
            if x <= *location {
                0.0
            } else {
                2.0 * special::normal_cdf(-1.0 / standardize(x, *location, *scale).sqrt())?
            }
        }
        D::Pareto2 { shape, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                -(-shape * softplus(log_ratio(x, *scale))).exp_m1()
            }
        }
        D::BetaPrime { alpha, beta, scale } => {
            if x <= 0.0 {
                0.0
            } else {
                special::beta_i_logit(*alpha, *beta, log_ratio(x, *scale))?
            }
        }
        D::Reciprocal { low, high } => {
            if x <= *low {
                0.0
            } else if x >= *high {
                1.0
            } else {
                log_ratio(x, *low) / log_ratio(*high, *low)
            }
        }
        D::MinExtreme { location, scale } => -(-standardize(x, *location, *scale).exp()).exp_m1(),
        D::Logarithmic { probability: p } => {
            if x < 1.0 {
                0.0
            } else if x >= 100000.0 {
                1.0
            } else {
                logseries_cdf(*p, x.floor() as u64)?
            }
        }
        _ => return Err(domain()),
    };
    if p.is_finite() { Ok(p.clamp(0.0, 1.0)) } else { Err(range()) }
}
pub fn log_density(d: &D, x: f64) -> Result<f64> {
    validate(d)?;
    let neg = f64::NEG_INFINITY;
    let log_beta = |a, b| -> Result<f64> { Ok(special::log_gamma(a)? + special::log_gamma(b)? - special::log_gamma(a + b)?) };
    let ln_cosh = |z: f64| z.abs() + (-2.0 * z.abs()).exp().ln_1p() - 2.0_f64.ln();
    let value = match d {
        D::Burr12 { shape_c: c, shape_k: k, scale: s } => {
            if x <= 0.0 {
                neg
            } else {
                let z = log_ratio(x, *s);
                c.ln() + k.ln() - s.ln() + (c - 1.0) * z - (k + 1.0) * softplus(c * z)
            }
        }
        D::Dagum { shape: a, power: p, scale: s } => {
            if x <= 0.0 {
                neg
            } else {
                let z = log_ratio(x, *s);
                a.ln() + p.ln() - s.ln() - (a + 1.0) * z - (p + 1.0) * softplus(-a * z)
            }
        }
        D::DoubleTriangular { low, mode, high, left_probability: p } => {
            if x <= *low || x >= *high {
                neg
            } else if x <= *mode {
                (2.0 * p).ln() + (x - low).ln() - 2.0 * (mode - low).ln()
            } else {
                (2.0 * (1.0 - p)).ln() + (high - x).ln() - 2.0 * (high - mode).ln()
            }
        }
        D::FatigueLife { shape: a, scale: s } => {
            if x <= 0.0 {
                neg
            } else {
                let z = log_ratio(x, *s) * 0.5;
                let t = 2.0 * z.sinh() / a;
                -0.5 * t * t - 0.918_938_533_204_672_7 - a.ln() - x.ln() + ln_cosh(z)
            }
        }
        D::Frechet { shape: a, scale: s } => {
            if x <= 0.0 {
                neg
            } else {
                let z = log_ratio(x, *s);
                a.ln() - s.ln() - (a + 1.0) * z - (-a * z).exp()
            }
        }
        D::HyperbolicSecant { location, scale } => -std::f64::consts::PI.ln() - scale.ln() - ln_cosh(standardize(x, *location, *scale)),
        D::Kumaraswamy { alpha: a, beta: b } => {
            if !(0.0..=1.0).contains(&x) {
                neg
            } else {
                let pow = |value: f64, exponent: f64| if exponent == 0.0 { 0.0 } else { exponent * value.ln() };
                a.ln() + b.ln() + pow(x, a - 1.0) + pow(1.0 - x.powf(*a), b - 1.0)
            }
        }
        D::Levy { location, scale } => {
            if x <= *location {
                neg
            } else {
                let z = standardize(x, *location, *scale);
                -scale.ln() - 0.918_938_533_204_672_7 - 1.5 * z.ln() - 0.5 / z
            }
        }
        D::Pareto2 { shape: a, scale: s } => {
            if x < 0.0 {
                neg
            } else {
                a.ln() - s.ln() - (a + 1.0) * softplus(log_ratio(x, *s))
            }
        }
        D::BetaPrime { alpha: a, beta: b, scale: s } => {
            if x <= 0.0 {
                neg
            } else {
                let z = log_ratio(x, *s);
                (a - 1.0) * z - s.ln() - (a + b) * softplus(z) - log_beta(*a, *b)?
            }
        }
        D::Reciprocal { low, high } => {
            if x < *low || x > *high {
                neg
            } else {
                -x.ln() - log_ratio(*high, *low).ln()
            }
        }
        D::MinExtreme { location, scale } => {
            let z = standardize(x, *location, *scale);
            -scale.ln() + z - z.exp()
        }
        D::Logarithmic { probability: p } => {
            if x < 1.0 || x.fract() != 0.0 {
                neg
            } else {
                x * p.ln() - x.ln() - (-(-p).ln_1p()).ln()
            }
        }
        _ => return Err(domain()),
    };
    if value.is_nan() { Err(range()) } else { Ok(value) }
}
