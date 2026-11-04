//! Log density / probability mass with an explicit choice of dominating measure.
use crate::{
    extended::{asinh_standardize, log_abs_standardize, standardize},
    reference_families::{log_ratio, softplus},
};
use crate::{special, validate_distribution};
use solvercraft_types::{Distribution as D, Result, SolverError};
fn unsupported() -> SolverError {
    SolverError::new("density_unavailable", "This distribution has no implemented scalar density/mass function with a common dominating measure.")
}
pub fn is_discrete(d: &D) -> bool {
    match d {
        D::Constant { .. }
        | D::DiscreteUniform { .. }
        | D::Bernoulli { .. }
        | D::Binomial { .. }
        | D::Poisson { .. }
        | D::NegativeBinomial { .. }
        | D::Geometric { .. }
        | D::Empirical { .. }
        | D::Hypergeometric { .. }
        | D::Categorical { .. } => true,
        D::Logarithmic { .. } => true,
        D::Normal { std_dev, .. } => *std_dev == 0.0,
        D::Lognormal { log_std_dev, .. } => *log_std_dev == 0.0,
        D::Uniform { low, high } | D::Triangular { low, high, .. } | D::BetaPert { low, high, .. } => low == high,
        D::ShiftScale { base, .. } | D::Truncated { base, .. } => is_discrete(base),
        D::Mixture { components, .. } => components.iter().all(is_discrete),
        _ => false,
    }
}
fn log_power(x: f64, a: f64) -> f64 {
    if a == 0.0 { 0.0 } else { a * x.ln() }
}
fn log_choose(n: f64, k: f64) -> Result<f64> {
    Ok(special::log_gamma(n + 1.0)? - special::log_gamma(k + 1.0)? - special::log_gamma(n - k + 1.0)?)
}
fn log_beta(a: f64, b: f64) -> Result<f64> {
    Ok(special::log_gamma(a)? + special::log_gamma(b)? - special::log_gamma(a + b)?)
}
fn normal_log(z: f64) -> f64 {
    -0.5 * z * z - 0.918_938_533_204_672_7
}
/// Returns -infinity outside support. Positive infinity is possible at a singular density;
/// fitting rejects a non-finite total likelihood rather than inventing a finite score.
pub fn log_density(d: &D, x: f64) -> Result<f64> {
    validate_distribution(d)?;
    if !x.is_finite() {
        return Err(SolverError::new("density_domain", "Density requires a finite value."));
    }
    log_density_valid(d, x)
}
fn log_density_valid(d: &D, x: f64) -> Result<f64> {
    let neg = f64::NEG_INFINITY;
    let value = match d {
        D::Metalog { coefficients, lower, upper } => return crate::extended::prepare_metalog(coefficients, *lower, *upper)?.log_density(x),
        D::Constant { value } => {
            if x == *value {
                0.0
            } else {
                neg
            }
        }
        D::Uniform { low, high } => {
            if low == high {
                if x == *low { 0.0 } else { neg }
            } else if x >= *low && x <= *high {
                -(high - low).ln()
            } else {
                neg
            }
        }
        D::DiscreteUniform { low, high } => {
            if x.fract() == 0.0 && x >= *low as f64 && x <= *high as f64 {
                -((*high as i128 - *low as i128 + 1) as f64).ln()
            } else {
                neg
            }
        }
        D::Normal { mean, std_dev } => {
            if *std_dev == 0.0 {
                if x == *mean { 0.0 } else { neg }
            } else {
                normal_log(standardize(x, *mean, *std_dev)) - std_dev.ln()
            }
        }
        D::Lognormal { log_mean, log_std_dev } => {
            if x <= 0.0 {
                neg
            } else if *log_std_dev == 0.0 {
                if x == log_mean.exp() { 0.0 } else { neg }
            } else {
                normal_log(standardize(x.ln(), *log_mean, *log_std_dev)) - log_std_dev.ln() - x.ln()
            }
        }
        D::Triangular { low, mode, high } => {
            if low == high {
                if x == *low { 0.0 } else { neg }
            } else if x < *low || x > *high {
                neg
            } else if x == *mode {
                (2.0 / (high - low)).ln()
            } else if x < *mode {
                2.0_f64.ln() + (x - low).ln() - (high - low).ln() - (mode - low).ln()
            } else {
                2.0_f64.ln() + (high - x).ln() - (high - low).ln() - (high - mode).ln()
            }
        }
        D::Exponential { rate } => {
            if x < 0.0 {
                neg
            } else {
                rate.ln() - rate * x
            }
        }
        D::Weibull { shape, scale } => {
            if x < 0.0 {
                neg
            } else if x == 0.0 {
                if *shape == 1.0 {
                    -scale.ln()
                } else if *shape > 1.0 {
                    neg
                } else {
                    f64::INFINITY
                }
            } else {
                let z = shape * log_ratio(x, *scale);
                if z.is_infinite() { neg } else { shape.ln() - x.ln() + z - z.exp() }
            }
        }
        D::Bernoulli { probability: p } => {
            if x == 0.0 {
                (-p).ln_1p()
            } else if x == 1.0 {
                p.ln()
            } else {
                neg
            }
        }
        D::Binomial { trials, probability: p } => {
            if x.fract() != 0.0 || x < 0.0 || x > f64::from(*trials) {
                neg
            } else {
                log_choose(f64::from(*trials), x)? + log_power(*p, x) + log_power(1.0 - p, f64::from(*trials) - x)
            }
        }
        D::Poisson { rate } => {
            if x.fract() != 0.0 || x < 0.0 {
                neg
            } else {
                log_power(*rate, x) - rate - special::log_gamma(x + 1.0)?
            }
        }
        D::NegativeBinomial { successes, probability: p } => {
            if x.fract() != 0.0 || x < 0.0 {
                neg
            } else {
                let r = f64::from(*successes);
                log_choose(x + r - 1.0, x)? + log_power(*p, r) + log_power(1.0 - p, x)
            }
        }
        D::Geometric { probability: p } => {
            if x.fract() != 0.0 || x < 0.0 {
                neg
            } else {
                p.ln() + log_power(1.0 - p, x)
            }
        }
        D::Beta { alpha, beta } => {
            if !(0.0..=1.0).contains(&x) {
                neg
            } else {
                log_power(x, alpha - 1.0) + log_power(1.0 - x, beta - 1.0) - log_beta(*alpha, *beta)?
            }
        }
        D::BetaPert { low, mode, high, weight } => {
            if low == high {
                if x == *low { 0.0 } else { neg }
            } else {
                let width = high - low;
                let a = 1.0 + weight * (mode - low) / width;
                let b = 1.0 + weight * (high - mode) / width;
                log_density_valid(&D::Beta { alpha: a, beta: b }, (x - low) / width)? - width.ln()
            }
        }
        D::Gamma { shape, scale } | D::InverseGamma { shape, scale } => {
            if x < 0.0 || x == 0.0 && matches!(d, D::InverseGamma { .. }) {
                neg
            } else if matches!(d, D::Gamma { .. }) {
                log_power(x, shape - 1.0) - x / scale - shape * scale.ln() - special::log_gamma(*shape)?
            } else {
                shape * scale.ln() - special::log_gamma(*shape)? - (shape + 1.0) * x.ln() - scale / x
            }
        }
        D::Erlang { shape, scale } => log_density_valid(&D::Gamma { shape: f64::from(*shape), scale: *scale }, x)?,
        D::ChiSquared { degrees_of_freedom } => log_density_valid(&D::Gamma { shape: degrees_of_freedom / 2.0, scale: 2.0 }, x)?,
        D::StudentT { degrees_of_freedom: nu, location, scale } => {
            let log_z = log_abs_standardize(x, *location, *scale);
            special::log_gamma((nu + 1.0) / 2.0)?
                - special::log_gamma(nu / 2.0)?
                - 0.5 * (nu * std::f64::consts::PI).ln()
                - scale.ln()
                - 0.5 * (nu + 1.0) * softplus(2.0 * log_z - nu.ln())
        }
        D::F { numerator_df: a, denominator_df: b } => {
            if x < 0.0 {
                neg
            } else {
                0.5 * a * (a.ln() - b.ln()) + log_power(x, 0.5 * a - 1.0)
                    - 0.5 * (a + b) * softplus(a.ln() + x.ln() - b.ln())
                    - log_beta(a / 2.0, b / 2.0)?
            }
        }
        D::Cauchy { location, scale } => -std::f64::consts::PI.ln() - scale.ln() - softplus(2.0 * log_abs_standardize(x, *location, *scale)),
        D::Logistic { location, scale } => {
            let z = standardize(x, *location, *scale).abs();
            -scale.ln() - z - 2.0 * (-z).exp().ln_1p()
        }
        D::Laplace { location, scale } => -2.0_f64.ln() - scale.ln() - standardize(x, *location, *scale).abs(),
        D::Gumbel { location, scale } => {
            let z = standardize(x, *location, *scale);
            -scale.ln() - z - (-z).exp()
        }
        D::Pareto { minimum, shape } => {
            if x < *minimum {
                neg
            } else {
                shape.ln() + shape * minimum.ln() - (shape + 1.0) * x.ln()
            }
        }
        D::Rayleigh { scale } => {
            if x <= 0.0 {
                neg
            } else {
                x.ln() - 2.0 * scale.ln() - 0.5 * (x / scale).powi(2)
            }
        }
        D::LogLogistic { shape, scale } => {
            if x <= 0.0 {
                neg
            } else {
                let z = (shape * log_ratio(x, *scale)).abs();
                shape.ln() - x.ln() - z - 2.0 * (-z).exp().ln_1p()
            }
        }
        D::InverseGaussian { mean, shape } => {
            if x <= 0.0 {
                neg
            } else {
                0.5 * (shape.ln() - std::f64::consts::TAU.ln() - 3.0 * x.ln()) - 0.5 * shape / x * (x / mean - 1.0).powi(2)
            }
        }
        D::SkewNormal { location, scale, shape } => {
            let z = standardize(x, *location, *scale);
            2.0_f64.ln() - scale.ln() + normal_log(z) + special::normal_log_cdf(shape * z)?
        }
        D::JohnsonSu { gamma, delta, location, scale } => {
            let log_y = log_abs_standardize(x, *location, *scale);
            normal_log(gamma + delta * asinh_standardize(x, *location, *scale)) + delta.ln() - scale.ln() - 0.5 * softplus(2.0 * log_y)
        }
        D::JohnsonSl { gamma, delta, location, scale } => {
            if x <= *location {
                neg
            } else {
                let log_y = log_abs_standardize(x, *location, *scale);
                normal_log(gamma + delta * log_y) + delta.ln() - scale.ln() - log_y
            }
        }
        D::JohnsonSb { gamma, delta, location, scale } => {
            let y = standardize(x, *location, *scale);
            if y <= 0.0 || y >= 1.0 {
                neg
            } else {
                normal_log(gamma + delta * (y.ln() - (-y).ln_1p())) + delta.ln() - scale.ln() - y.ln() - (-y).ln_1p()
            }
        }
        D::Hypergeometric { population, successes, draws } => {
            if x.fract() != 0.0 || x < f64::from(draws.saturating_sub(population - successes)) || x > f64::from((*draws).min(*successes)) {
                neg
            } else {
                log_choose(f64::from(*successes), x)? + log_choose(f64::from(population - successes), f64::from(*draws) - x)?
                    - log_choose(f64::from(*population), f64::from(*draws))?
            }
        }
        D::Empirical { values } => {
            let n = values.iter().filter(|v| **v == x).count();
            if n == 0 { neg } else { (n as f64 / values.len() as f64).ln() }
        }
        D::Categorical { values, weights } => {
            let max = weights.iter().copied().fold(0.0_f64, f64::max);
            let denominator = special::finite_sum(weights.iter().map(|w| w / max))?;
            let numerator = special::finite_sum(values.iter().zip(weights).filter(|(v, _)| **v == x).map(|(_, w)| w / max))?;
            (numerator / denominator).ln()
        }
        D::ShiftScale { base, shift, scale } => {
            if is_discrete(base) {
                if let Some((lower, upper)) = crate::affine::preimage(x, x, *shift, *scale) {
                    crate::mass::interval(base, lower, upper)?
                } else {
                    neg
                }
            } else {
                log_density_valid(base, standardize(x, *shift, *scale))? - scale.abs().ln()
            }
        }
        D::Truncated { base, lower, upper } => {
            if lower.is_some_and(|v| x < v) || upper.is_some_and(|v| x > v) {
                neg
            } else {
                let prep = crate::PreparedDistribution::new(base)?;
                let mass = prep.interval_probability(lower.unwrap_or(f64::NEG_INFINITY), upper.unwrap_or(f64::INFINITY))?;
                log_density_valid(base, x)? - mass.ln()
            }
        }
        D::Mixture { components, weights } => {
            if components.iter().any(|c| is_discrete(c) != is_discrete(&components[0])) {
                return Err(unsupported());
            }
            let maximum = weights.iter().copied().fold(0.0_f64, f64::max);
            let denominator = special::finite_sum(weights.iter().map(|w| w / maximum))?;
            let logs = components
                .iter()
                .zip(weights)
                .map(|(c, w)| log_density_valid(c, x).map(|l| l + (w / maximum).ln() - denominator.ln()))
                .collect::<Result<Vec<_>>>()?;
            let max = logs.iter().copied().fold(neg, f64::max);
            if max == neg { neg } else { max + special::finite_sum(logs.iter().map(|l| (l - max).exp()))?.ln() }
        }
        _ => return if crate::reference_families::contains(d) { crate::reference_families::log_density(d, x) } else { Err(unsupported()) },
    };
    if value.is_nan() { Err(SolverError::new("density_numerical_range", "Density cannot be resolved in finite arithmetic.")) } else { Ok(value) }
}
