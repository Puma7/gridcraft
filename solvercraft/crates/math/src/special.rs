//! Bounded real special functions used by probability distributions.
//! Formulas: NIST DLMF §§5, 8.2, 8.17. Original Rust implementation; no vendor code/tables.
use solvercraft_types::{Result, SolverError};
const EPS: f64 = 2e-14;
fn err() -> SolverError {
    SolverError::new("special_convergence", "Special function did not converge within its bounded work budget.")
}
pub fn log_gamma(z: f64) -> Result<f64> {
    if !z.is_finite() || z <= 0.0 {
        return Err(SolverError::new("distribution_domain", "Gamma argument must be finite and positive."));
    }
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if z < 0.5 {
        return Ok(std::f64::consts::PI.ln() - (std::f64::consts::PI * z).sin().ln() - log_gamma(1.0 - z)?);
    }
    let x = z - 1.0;
    let mut sum = C[0];
    for (i, c) in C.iter().enumerate().skip(1) {
        sum += c / (x + i as f64);
    }
    let t = x + 7.5;
    let value = 0.918_938_533_204_672_7 + (x + 0.5) * t.ln() - t + sum.ln();
    if value.is_finite() { Ok(value) } else { Err(err()) }
}
pub fn gamma_p(a: f64, x: f64) -> Result<f64> {
    if !a.is_finite() || a <= 0.0 || x.is_nan() || x < 0.0 {
        return Err(err());
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    if x == f64::INFINITY {
        return Ok(1.0);
    }
    let factor = (-x + a * x.ln() - log_gamma(a)?).exp();
    if x < a + 1.0 {
        let mut term = 1.0 / a;
        let mut sum = term;
        for n in 1..=20_000 {
            term *= x / (a + n as f64);
            sum += term;
            if term.abs() <= sum.abs() * EPS {
                return Ok((sum * factor).clamp(0.0, 1.0));
            }
        }
    } else {
        const TINY: f64 = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b;
        let mut h = d;
        for n in 1..=20_000 {
            let nf = n as f64;
            let an = -nf * (nf - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < TINY {
                d = TINY.copysign(d);
            }
            c = b + an / c;
            if c.abs() < TINY {
                c = TINY.copysign(c);
            }
            d = 1.0 / d;
            let delta = d * c;
            h *= delta;
            if (delta - 1.0).abs() < EPS {
                return Ok((1.0 - factor * h).clamp(0.0, 1.0));
            }
        }
    }
    Err(err())
}
/// Upper regularized gamma, evaluated directly in the upper tail to avoid 1-P cancellation.
pub fn gamma_q(a: f64, x: f64) -> Result<f64> {
    if !a.is_finite() || a <= 0.0 || x.is_nan() || x < 0.0 {
        return Err(err());
    }
    if x == f64::INFINITY {
        return Ok(0.0);
    }
    if x < a + 1.0 {
        return Ok(1.0 - gamma_p(a, x)?);
    }
    let factor = (-x + a * x.ln() - log_gamma(a)?).exp();
    const TINY: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for n in 1..=20_000 {
        let nf = n as f64;
        let an = -nf * (nf - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY.copysign(d);
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY.copysign(c);
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            return Ok((factor * h).clamp(0.0, 1.0));
        }
    }
    Err(err())
}
/// Gaussian CDF with a directly evaluated small tail. Addressed inverse-normal
/// draws use their separate unchanged quantile algorithm.
pub fn normal_cdf(x: f64) -> Result<f64> {
    if x.is_nan() {
        return Err(err());
    }
    if x == 0.0 {
        return Ok(0.5);
    }
    let tail = 0.5 * gamma_q(0.5, 0.5 * x * x)?;
    Ok(if x > 0.0 { 1.0 - tail } else { tail })
}
/// Gaussian interval probability without subtracting nearly equal tails.
/// The local8-node Gaussian rule integrates a smooth positive density over a
/// standardized width<=0.125/(1+|a|); its small-width limit is phi(a)*width.
pub(crate) fn normal_interval(mean: f64, std: f64, lower: f64, upper: f64) -> Result<f64> {
    if lower >= upper {
        return Ok(0.0);
    }
    if lower == f64::NEG_INFINITY {
        return normal_cdf(crate::extended::standardize(upper, mean, std));
    }
    let a = crate::extended::standardize(lower, mean, std);
    if upper == f64::INFINITY {
        return normal_cdf(-a);
    }
    let width = crate::extended::standardize(upper, lower, std);
    if width <= 0.125 / (1.0 + a.abs()) {
        let mut sum = 0.0;
        for (node, weight) in [
            (0.9602898564975363, 0.1012285362903763),
            (0.7966664774136267, 0.2223810344533745),
            (0.525532409916329, 0.3137066458778873),
            (0.1834346424956498, 0.362683783378362),
        ] {
            for sign in [-1.0, 1.0] {
                let z = a + 0.5 * width * (1.0 + sign * node);
                sum += weight * (-0.5 * z * z).exp();
            }
        }
        return Ok(0.5 * width * sum / std::f64::consts::TAU.sqrt());
    }
    let b = crate::extended::standardize(upper, mean, std);
    Ok(if a > 0.0 { normal_cdf(-a)? - normal_cdf(-b)? } else { normal_cdf(b)? - normal_cdf(a)? })
}
/// Log Phi(x), preserving finite log probabilities after the ordinary CDF underflows.
pub fn normal_log_cdf(x: f64) -> Result<f64> {
    if x.is_nan() {
        return Err(err());
    }
    if x < -10.0 {
        if x == f64::NEG_INFINITY {
            return Ok(f64::NEG_INFINITY);
        }
        let inverse = (1.0 / x) * (1.0 / x);
        let mut term = 1.0;
        let mut terms = vec![1.0];
        for n in 1..=40 {
            let next = term * (-f64::from(2 * n - 1)) * inverse;
            if next.abs() >= term.abs() {
                break;
            }
            terms.push(next);
            term = next;
            if next.abs() < 1e-17 {
                break;
            }
        }
        Ok(-0.5 * x * x - (-x).ln() - 0.918_938_533_204_672_7 + finite_sum(terms)?.ln())
    } else if x > 0.0 {
        Ok((-normal_cdf(-x)?).ln_1p())
    } else {
        Ok(normal_cdf(x)?.ln())
    }
}
fn beta_fraction(a: f64, b: f64, x: f64) -> Result<f64> {
    const TINY: f64 = 1e-300;
    let ab = a + b;
    let ap = a + 1.0;
    let am = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - ab * x / ap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=20_000 {
        let mf = m as f64;
        let m2 = 2.0 * mf;
        let aa = mf * (b - mf) * x / ((am + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY.copysign(d);
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY.copysign(c);
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + mf) * (ab + mf) * x / ((a + m2) * (ap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY.copysign(d);
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY.copysign(c);
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            return Ok(h);
        }
    }
    Err(err())
}
/// I_logistic(z)(a,b), retaining a small complementary coordinate in either tail.
pub fn beta_i_logit(a: f64, b: f64, z: f64) -> Result<f64> {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 || z.is_nan() {
        return Err(err());
    }
    if z > (a + 1.0).ln() - (b + 1.0).ln() {
        return Ok(1.0 - beta_i_logit(b, a, -z)?);
    }
    let log_x = -crate::reference_families::softplus(-z);
    if log_x < -700.0 {
        return Ok((a * log_x - log_gamma(a + 1.0)? - log_gamma(b)? + log_gamma(a + b)?).exp().clamp(0.0, 1.0));
    }
    let log_one_minus_x = -crate::reference_families::softplus(z);
    let fraction = beta_fraction(a, b, log_x.exp())?;
    Ok((a * log_x + b * log_one_minus_x + log_gamma(a + b)? - log_gamma(a)? - log_gamma(b)? + fraction.ln() - a.ln()).exp().clamp(0.0, 1.0))
}
/// Quantile on the logit axis, avoiding a rounded unit-Beta coordinate before scaling.
pub fn beta_logit_quantile(a: f64, b: f64, p: f64) -> Result<f64> {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 || !p.is_finite() || p <= 0.0 || p >= 1.0 {
        return Err(SolverError::new("quantile_domain", "Beta-logit quantiles require positive shapes and probability in(0,1)."));
    }
    if p > 0.5 {
        return Ok(-beta_logit_quantile(b, a, 1.0 - p)?);
    }
    invert(p, -1e8, 1e8, |z| z, |z| beta_i_logit(a, b, z))
}
pub fn beta_i(a: f64, b: f64, x: f64) -> Result<f64> {
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 || x.is_nan() {
        return Err(err());
    }
    if x <= 0.0 {
        return Ok(0.0);
    }
    if x >= 1.0 {
        return Ok(1.0);
    }
    let bt = (log_gamma(a + b)? - log_gamma(a)? - log_gamma(b)? + a * x.ln() + b * (-x).ln_1p()).exp();
    let value = if x < (a + 1.0) / (a + b + 2.0) { bt * beta_fraction(a, b, x)? / a } else { 1.0 - bt * beta_fraction(b, a, 1.0 - x)? / b };
    if value.is_finite() { Ok(value.clamp(0.0, 1.0)) } else { Err(err()) }
}
pub fn logistic(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
/// Bisection in a supplied monotonically increasing transform. Tail range is checked.
pub fn invert<F, G>(p: f64, mut low: f64, mut high: f64, transform: G, cdf: F) -> Result<f64>
where
    F: Fn(f64) -> Result<f64>,
    G: Fn(f64) -> f64,
{
    let lo = transform(low);
    let hi = transform(high);
    if cdf(lo)? > p || cdf(hi)? < p {
        return Err(SolverError::new("quantile_range", "Quantile is outside the finite numeric search range."));
    }
    for _ in 0..160 {
        let mid = low + 0.5 * (high - low);
        let value = transform(mid);
        let f = cdf(value)?;
        if f < p {
            low = mid
        } else {
            high = mid
        }
        if high - low <= 2e-13 * (1.0 + mid.abs()) {
            break;
        }
    }
    let q = transform(high);
    if !q.is_finite() {
        return Err(err());
    }
    Ok(q)
}
pub fn finite_sum(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut sum = 0.0_f64;
    let mut correction = 0.0;
    for value in values {
        if !value.is_finite() {
            return Err(SolverError::new("numerical_range", "Computed probability term is non-finite."));
        }
        let next = sum + value;
        correction += if sum.abs() >= value.abs() { (sum - next) + value } else { (value - next) + sum };
        sum = next;
    }
    let result = sum + correction;
    if result.is_finite() { Ok(result) } else { Err(err()) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gamma_beta_known_identities() -> Result<()> {
        assert!((log_gamma(5.0)? - 24.0_f64.ln()).abs() < 1e-12);
        for x in [0.01_f64, 0.2, 0.5, 0.9] {
            assert!((beta_i(1.0, 1.0, x)? - x).abs() < 1e-12);
            assert!((beta_i(2.0, 3.0, x)? - (6.0 * x * x - 8.0 * x.powi(3) + 3.0 * x.powi(4))).abs() < 1e-11);
            assert!((gamma_p(1.0, x)? - (-(-x).exp_m1())).abs() < 1e-12);
            assert!((gamma_p(2.0, x)? - (1.0 - (-x).exp() * (1.0 + x))).abs() < 1e-12);
        }
        Ok(())
    }
}
