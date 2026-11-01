//! Additional distribution families and compositional properties.
use crate::{PreparedDistribution, random, special};
use solvercraft_types::{Distribution as D, Result, SolverError};
fn domain() -> SolverError {
    SolverError::new("distribution_domain", "Invalid distribution parameters or unsupported numerical range.")
}
fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}
fn finite(v: f64) -> bool {
    v.is_finite()
}
fn shape(v: f64) -> bool {
    (1e-4..=1e4).contains(&v) && v.is_finite()
}
fn probability(v: f64) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
pub(crate) fn standardize(x: f64, location: f64, scale: f64) -> f64 {
    let difference = x - location;
    if difference.is_finite() {
        difference / scale
    } else if scale.abs() >= 1.0 {
        x / scale - location / scale
    } else {
        difference / scale
    }
}
pub(crate) fn log_abs_standardize(x: f64, location: f64, scale: f64) -> f64 {
    let difference = x - location;
    if difference.is_finite() {
        difference.abs().ln() - scale.abs().ln()
    } else {
        let magnitude = x.abs().max(location.abs());
        magnitude.ln() + (x / magnitude - location / magnitude).abs().ln() - scale.abs().ln()
    }
}
pub(crate) fn asinh_standardize(x: f64, location: f64, scale: f64) -> f64 {
    let z = standardize(x, location, scale);
    if z.is_finite() { z.asinh() } else { z.signum() * (log_abs_standardize(x, location, scale) + 2.0_f64.ln()) }
}
pub fn is_extended(d: &D) -> bool {
    !matches!(
        d,
        D::Constant { .. }
            | D::Uniform { .. }
            | D::Normal { .. }
            | D::Lognormal { .. }
            | D::Triangular { .. }
            | D::Bernoulli { .. }
            | D::Binomial { .. }
            | D::Poisson { .. }
            | D::Exponential { .. }
            | D::Weibull { .. }
            | D::Empirical { .. }
    )
}
pub fn validate_tree(d: &D) -> Result<()> {
    let mut stack = vec![(d, 0_usize, false)];
    let mut count = 0_usize;
    while let Some((d, depth, in_compound)) = stack.pop() {
        count += 1;
        if depth > 16 || count > 256 {
            return Err(SolverError::new("distribution_limit", "Distribution trees permit at most 16 levels and 256 nodes."));
        }
        match d {
            D::ShiftScale { base, shift, scale } => {
                if !finite(*shift) || !finite(*scale) || *scale == 0.0 {
                    return Err(domain());
                }
                stack.push((base, depth + 1, in_compound));
            }
            D::Truncated { base, lower, upper } => {
                if lower.is_some_and(|x| !finite(x)) || upper.is_some_and(|x| !finite(x)) || lower.zip(*upper).is_some_and(|(a, b)| a > b) {
                    return Err(domain());
                }
                stack.push((base, depth + 1, in_compound));
            }
            D::Mixture { components, weights } => {
                validate_weights(weights, components.len())?;
                if components.len() > 32 {
                    return Err(SolverError::new("distribution_limit", "At most 32 mixture components are supported."));
                }
                for child in components {
                    stack.push((child, depth + 1, in_compound));
                }
            }
            D::CompoundPoisson { rate, severity, max_events } => {
                if in_compound || !rate.is_finite() || !(0.0..=512.0).contains(rate) || *max_events == 0 || *max_events > 10_000 {
                    return Err(SolverError::new(
                        "distribution_limit",
                        "Compound Poisson permits rates 0..512, 1..10000 events, and no nested compounds.",
                    ));
                }
                stack.push((severity, depth + 1, true));
            }
            _ => crate::distributions::validate_leaf(d)?,
        }
    }
    Ok(())
}
pub fn validate_scalar(d: &D) -> Result<()> {
    if let D::Metalog { coefficients, lower, upper } = d {
        return prepare_metalog(coefficients, *lower, *upper).map(|_| ());
    }
    let valid = match d {
        D::DiscreteUniform { low, high } => {
            low <= high
                && low.unsigned_abs() < 9_007_199_254_740_992
                && high.unsigned_abs() < 9_007_199_254_740_992
                && i128::from(*high) - i128::from(*low) < 9_007_199_254_740_992
        }
        D::Beta { alpha, beta } => shape(*alpha) && shape(*beta),
        D::BetaPert { low, mode, high, weight } => {
            finite(*low)
                && finite(*mode)
                && finite(*high)
                && low <= mode
                && mode <= high
                && (high - low).is_finite()
                && positive(*weight)
                && *weight <= 100.0
        }
        D::Gamma { shape: a, scale } | D::InverseGamma { shape: a, scale } => shape(*a) && positive(*scale),
        D::NegativeBinomial { successes, probability: p } => *successes > 0 && *successes <= 10_000 && positive(*p) && *p <= 1.0,
        D::Geometric { probability: p } => positive(*p) && *p <= 1.0,
        D::EmpiricalContinuous { values } => !values.is_empty() && values.len() <= 1_000_000 && values.iter().all(|x| finite(*x)),
        D::StudentT { degrees_of_freedom, location, scale } => shape(*degrees_of_freedom) && finite(*location) && positive(*scale),
        D::ChiSquared { degrees_of_freedom } => shape(*degrees_of_freedom),
        D::F { numerator_df, denominator_df } => shape(*numerator_df) && shape(*denominator_df),
        D::Cauchy { location, scale } | D::Logistic { location, scale } | D::Laplace { location, scale } | D::Gumbel { location, scale } => {
            finite(*location) && positive(*scale)
        }
        D::Pareto { minimum, shape: a } => positive(*minimum) && shape(*a),
        D::Rayleigh { scale } => positive(*scale),
        D::Erlang { shape: a, scale } => *a > 0 && *a <= 10_000 && positive(*scale),
        D::InverseGaussian { mean, shape: a } => positive(*mean) && positive(*a) && (*a / *mean).is_finite(),
        D::LogLogistic { shape: a, scale } => shape(*a) && positive(*scale),
        D::SkewNormal { location, scale, shape: a } => finite(*location) && positive(*scale) && a.is_finite() && a.abs() <= 100.0,
        D::JohnsonSu { gamma, delta, location, scale }
        | D::JohnsonSb { gamma, delta, location, scale }
        | D::JohnsonSl { gamma, delta, location, scale } => finite(*gamma) && positive(*delta) && finite(*location) && positive(*scale),
        D::Hypergeometric { population, successes, draws } => {
            *population > 0 && *population <= 1_000_000 && successes <= population && draws <= population && *draws <= 10_000
        }
        D::Categorical { values, weights } => {
            values.len() <= 100_000 && values.iter().all(|x| finite(*x)) && validate_weights(weights, values.len()).is_ok()
        }
        _ => crate::reference_families::contains(d) && crate::reference_families::validate(d).is_ok(),
    };
    if valid { Ok(()) } else { Err(domain()) }
}
fn validate_weights(weights: &[f64], n: usize) -> Result<()> {
    if n == 0 || weights.len() != n || weights.iter().any(|x| !x.is_finite() || *x < 0.0) || !weights.iter().any(|x| *x > 0.0) {
        return Err(domain());
    }
    Ok(())
}
pub fn storage(d: &D) -> Result<u64> {
    let children = match d {
        D::ShiftScale { base, .. } | D::Truncated { base, .. } => crate::distributions::prepared_storage_bytes(base)?,
        D::Mixture { components, .. } => special::finite_sum(
            components.iter().map(|x| crate::distributions::prepared_storage_bytes(x).map(|n| n as f64)).collect::<Result<Vec<_>>>()?,
        )? as u64,
        D::CompoundPoisson { severity, max_events, .. } => {
            crate::distributions::prepared_storage_bytes(severity)?.saturating_add(32_000 + u64::from(*max_events) * 8)
        }
        D::EmpiricalContinuous { values } => values.len() as u64 * 8,
        D::Categorical { values, .. } => values.len() as u64 * 48,
        D::Hypergeometric { draws, .. } => (u64::from(*draws) + 1) * 48,
        D::Metalog { .. } => 1024,
        D::SkewNormal { .. } => 8192,
        _ => 0,
    };
    children.checked_add(512).ok_or_else(|| SolverError::new("memory_limit", "Distribution storage overflow."))
}
#[derive(Debug, Clone)]
pub enum PreparedExtra {
    Metalog(crate::metalog::PreparedMetalog),
    Scalar(D),
    Continuous(Vec<f64>),
    Points { values: Vec<f64>, cumulative: Vec<f64> },
    Shift { base: Box<PreparedDistribution>, shift: f64, scale: f64 },
    Truncated { base: Box<PreparedDistribution>, lower: Option<f64>, upper: Option<f64>, lo: f64, hi: f64 },
    Mixture { components: Vec<PreparedDistribution>, weights: Vec<f64> },
    Compound { frequency: Box<PreparedDistribution>, severity: Box<PreparedDistribution>, max_events: u32 },
}
pub(crate) fn prepare_metalog(coefficients: &[f64], lower: Option<f64>, upper: Option<f64>) -> Result<crate::metalog::PreparedMetalog> {
    if !(2..=crate::metalog::MAX_TERMS).contains(&coefficients.len()) {
        return Err(SolverError::new("metalog_domain", "Metalog requires2..30 coefficients."));
    }
    let mut copied = Vec::new();
    reserve(&mut copied, coefficients.len())?;
    copied.extend_from_slice(coefficients);
    crate::metalog::PreparedMetalog::new(&crate::metalog::Metalog { coefficients: copied, lower, upper })
}
fn reserve<T>(v: &mut Vec<T>, n: usize) -> Result<()> {
    v.try_reserve_exact(n).map_err(|_| SolverError::new("allocation", "Cannot allocate prepared distribution."))
}
fn point_table(values: &[f64], weights: &[f64]) -> Result<PreparedExtra> {
    validate_weights(weights, values.len())?;
    let maximum = weights.iter().copied().fold(0.0_f64, f64::max);
    let mut pairs = Vec::new();
    reserve(&mut pairs, values.len())?;
    for (value, weight) in values.iter().zip(weights) {
        if *weight > 0.0 {
            let scaled = weight / maximum;
            if scaled == 0.0 {
                return Err(SolverError::new("numerical_range", "Probability weights span an unrepresentable range."));
            }
            pairs.push((*value, scaled));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let sum = special::finite_sum(pairs.iter().map(|x| x.1))?;
    let mut v = Vec::new();
    let mut cumulative = Vec::new();
    reserve(&mut v, pairs.len())?;
    reserve(&mut cumulative, pairs.len())?;
    let mut current = 0.0;
    for (value, weight) in pairs {
        current += weight / sum;
        v.push(value);
        cumulative.push(current.min(1.0));
    }
    if let Some(last) = cumulative.last_mut() {
        *last = 1.0;
    }
    Ok(PreparedExtra::Points { values: v, cumulative })
}
pub fn prepare(d: &D) -> Result<PreparedExtra> {
    validate_tree(d)?;
    Ok(match d {
        D::Metalog { coefficients, lower, upper } => PreparedExtra::Metalog(prepare_metalog(coefficients, *lower, *upper)?),
        D::EmpiricalContinuous { values } => {
            let mut v = Vec::new();
            reserve(&mut v, values.len())?;
            v.extend_from_slice(values);
            v.sort_unstable_by(f64::total_cmp);
            PreparedExtra::Continuous(v)
        }
        D::Categorical { values, weights } => point_table(values, weights)?,
        D::Hypergeometric { population, successes, draws } => {
            let low = draws.saturating_sub(population - successes);
            let high = (*draws).min(*successes);
            let mut values = Vec::new();
            let mut logs = Vec::new();
            reserve(&mut values, (high - low + 1) as usize)?;
            reserve(&mut logs, (high - low + 1) as usize)?;
            let log_c = |n: u32, k: u32| -> Result<f64> {
                Ok(special::log_gamma(f64::from(n) + 1.0)? - special::log_gamma(f64::from(k) + 1.0)? - special::log_gamma(f64::from(n - k) + 1.0)?)
            };
            for k in low..=high {
                values.push(f64::from(k));
                logs.push(log_c(*successes, k)? + log_c(population - successes, draws - k)?);
            }
            let max = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            for w in &mut logs {
                *w = (*w - max).exp();
            }
            point_table(&values, &logs)?
        }
        D::ShiftScale { base, shift, scale } => {
            PreparedExtra::Shift { base: Box::new(PreparedDistribution::new(base)?), shift: *shift, scale: *scale }
        }
        D::Truncated { base, lower, upper } => {
            let prepared = PreparedDistribution::new(base)?;
            let lo = lower.map_or(Ok(0.0), |x| prepared.cdf(x.next_down()))?;
            let hi = upper.map_or(Ok(1.0), |x| prepared.cdf(x))?;
            if hi - lo < 1e-9 {
                return Err(SolverError::new("truncation_precision", "Truncation retains too little reliably resolved probability mass (<1e-9)."));
            }
            PreparedExtra::Truncated { base: Box::new(prepared), lower: *lower, upper: *upper, lo, hi }
        }
        D::Mixture { components, weights } => {
            let max = weights.iter().copied().fold(0.0_f64, f64::max);
            let total = special::finite_sum(weights.iter().map(|w| w / max))?;
            let mut bases = Vec::new();
            let mut probabilities = Vec::new();
            reserve(&mut bases, components.len())?;
            reserve(&mut probabilities, components.len())?;
            for (component, weight) in components.iter().zip(weights) {
                bases.push(PreparedDistribution::new(component)?);
                probabilities.push((weight / max) / total);
            }
            PreparedExtra::Mixture { components: bases, weights: probabilities }
        }
        D::CompoundPoisson { rate, severity, max_events } => PreparedExtra::Compound {
            frequency: Box::new(PreparedDistribution::new(&D::Poisson { rate: *rate })?),
            severity: Box::new(PreparedDistribution::new(severity)?),
            max_events: *max_events,
        },
        _ => PreparedExtra::Scalar(d.clone()),
    })
}
fn normal_cdf(x: f64) -> Result<f64> {
    special::normal_cdf(x)
}
fn normal_q(p: f64) -> Result<f64> {
    crate::distributions::inverse_normal(p)
}
fn endpoint() -> SolverError {
    SolverError::new("unbounded_quantile", "Distribution endpoint quantile is infinite.")
}
fn continuous_inverse<F: Fn(f64) -> Result<f64>>(p: f64, cdf: F, positive: bool) -> Result<f64> {
    if p <= 0.0 || p >= 1.0 {
        return Err(endpoint());
    }
    if positive { special::invert(p, -743.0, 709.0, f64::exp, cdf) } else { special::invert(p, -710.0, 710.0, f64::sinh, cdf) }
}
fn beta_quantile(a: f64, b: f64, p: f64) -> Result<f64> {
    if p == 0.0 {
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(1.0);
    }
    special::invert(p, -743.0, 40.0, special::logistic, |x| special::beta_i(a, b, x))
}
impl PreparedExtra {
    pub fn quantile(&self, p: f64) -> Result<f64> {
        if !probability(p) {
            return Err(domain());
        }
        let q = match self {
            Self::Metalog(prepared) => return prepared.quantile(p),
            Self::Continuous(v) => {
                let pos = p * (v.len() - 1) as f64;
                let i = (pos.floor() as usize).min(v.len() - 1);
                let j = (i + 1).min(v.len() - 1);
                let weight = pos - i as f64;
                (1.0 - weight) * v[i] + weight * v[j]
            }
            Self::Points { values, cumulative } => {
                let i = cumulative.partition_point(|x| *x < p).min(values.len() - 1);
                values[i]
            }
            Self::Shift { base, shift, scale } => {
                let target = if *scale > 0.0 {
                    p
                } else if p == 0.0 || p == 1.0 {
                    1.0 - p
                } else {
                    (1.0 - p).next_up().min(1.0)
                };
                scale.mul_add(base.quantile(target)?, *shift)
            }
            Self::Truncated { base, lower, upper, lo, hi } => {
                let target = (lo + p * (hi - lo)).max(lo.next_up()).min(*hi);
                let mut q = base.quantile(target)?;
                if let Some(v) = lower {
                    q = q.max(*v);
                }
                if let Some(v) = upper {
                    q = q.min(*v);
                }
                q
            }
            Self::Mixture { .. } => continuous_inverse(p, |x| self.cdf(x), false)?,
            Self::Compound { .. } => {
                return Err(SolverError::new(
                    "compound_quantile_unavailable",
                    "Generic compound sums support MC sampling, not a CDF/inverse-CDF or LHS.",
                ));
            }
            Self::Scalar(d) => match d {
                D::DiscreteUniform { low, high } => {
                    let span = (*high as i128 - *low as i128 + 1) as f64;
                    *low as f64 + (p * span).ceil().max(1.0).min(span) - 1.0
                }
                D::Beta { alpha, beta } => beta_quantile(*alpha, *beta, p)?,
                D::BetaPert { low, mode, high, weight } => {
                    if low == high || p == 0.0 {
                        *low
                    } else if p == 1.0 {
                        *high
                    } else {
                        (high - low)
                            .mul_add(beta_quantile(1.0 + weight * (mode - low) / (high - low), 1.0 + weight * (high - mode) / (high - low), p)?, *low)
                    }
                }
                D::Gamma { shape, scale } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        scale * continuous_inverse(p, |x| special::gamma_p(*shape, x), true)?
                    }
                }
                D::ChiSquared { degrees_of_freedom } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        2.0 * continuous_inverse(p, |x| special::gamma_p(0.5 * degrees_of_freedom, x), true)?
                    }
                }
                D::Erlang { shape, scale } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        scale * continuous_inverse(p, |x| special::gamma_p(f64::from(*shape), x), true)?
                    }
                }
                D::InverseGamma { shape, scale } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        scale / continuous_inverse(1.0 - p, |x| special::gamma_p(*shape, x), true)?
                    }
                }
                D::NegativeBinomial { successes, probability: q } => {
                    if *q == 1.0 || p == 0.0 {
                        0.0
                    } else {
                        if p == 1.0 {
                            return Err(endpoint());
                        }
                        let mut upper = 1_u64;
                        while special::beta_i(f64::from(*successes), upper as f64 + 1.0, *q)? < p {
                            upper = upper.checked_mul(2).ok_or_else(domain)?;
                            if upper > 1_000_000_000 {
                                return Err(SolverError::new("distribution_limit", "Negative binomial quantile exceeds 1e9 failures."));
                            }
                        }
                        let mut lower = 0_u64;
                        while lower < upper {
                            let mid = lower + (upper - lower) / 2;
                            if special::beta_i(f64::from(*successes), mid as f64 + 1.0, *q)? < p { lower = mid + 1 } else { upper = mid }
                        }
                        lower as f64
                    }
                }
                D::Geometric { probability: q } => {
                    if *q == 1.0 || p == 0.0 {
                        0.0
                    } else if p == 1.0 {
                        return Err(endpoint());
                    } else {
                        ((-p).ln_1p() / (-q).ln_1p()).ceil().max(1.0) - 1.0
                    }
                }
                D::StudentT { degrees_of_freedom, location, scale } => {
                    if p == 0.5 {
                        *location
                    } else if p > 0.5 {
                        (-scale).mul_add(continuous_inverse(1.0 - p, |x| student_cdf(x, *degrees_of_freedom), false)?, *location)
                    } else {
                        scale.mul_add(continuous_inverse(p, |x| student_cdf(x, *degrees_of_freedom), false)?, *location)
                    }
                }
                D::F { numerator_df, denominator_df } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        (denominator_df.ln() - numerator_df.ln() + special::beta_logit_quantile(numerator_df * 0.5, denominator_df * 0.5, p)?).exp()
                    }
                }
                D::Cauchy { location, scale } => {
                    if p == 0.0 || p == 1.0 {
                        return Err(endpoint());
                    }
                    if p == 0.5 {
                        *location
                    } else {
                        scale.mul_add(
                            if p < 0.5 { -1.0 / (std::f64::consts::PI * p).tan() } else { 1.0 / (std::f64::consts::PI * (1.0 - p)).tan() },
                            *location,
                        )
                    }
                }
                D::Logistic { location, scale } => {
                    if p == 0.0 || p == 1.0 {
                        return Err(endpoint());
                    }
                    scale.mul_add(p.ln() - (-p).ln_1p(), *location)
                }
                D::Laplace { location, scale } => {
                    if p == 0.0 || p == 1.0 {
                        return Err(endpoint());
                    }
                    if p < 0.5 { scale.mul_add((2.0 * p).ln(), *location) } else { (-scale).mul_add((2.0 * (1.0 - p)).ln(), *location) }
                }
                D::Gumbel { location, scale } => {
                    if p == 0.0 || p == 1.0 {
                        return Err(endpoint());
                    }
                    (-scale).mul_add((-p.ln()).ln(), *location)
                }
                D::Pareto { minimum, shape } => {
                    if p == 1.0 {
                        return Err(endpoint());
                    }
                    minimum * (-(-p).ln_1p() / shape).exp()
                }
                D::Rayleigh { scale } => {
                    if p == 1.0 {
                        return Err(endpoint());
                    }
                    scale * (-2.0 * (-p).ln_1p()).sqrt()
                }
                D::LogLogistic { shape, scale } => {
                    if p == 0.0 {
                        0.0
                    } else if p == 1.0 {
                        return Err(endpoint());
                    } else {
                        scale * ((p.ln() - (-p).ln_1p()) / shape).exp()
                    }
                }
                D::JohnsonSu { gamma, delta, location, scale } => scale.mul_add(((normal_q(p)? - gamma) / delta).sinh(), *location),
                D::JohnsonSl { gamma, delta, location, scale } => {
                    if p == 0.0 {
                        *location
                    } else {
                        location + scale * ((normal_q(p)? - gamma) / delta).exp()
                    }
                }
                D::JohnsonSb { gamma, delta, location, scale } => {
                    if p == 0.0 {
                        *location
                    } else if p == 1.0 {
                        location + scale
                    } else {
                        scale.mul_add(special::logistic((normal_q(p)? - gamma) / delta), *location)
                    }
                }
                D::SkewNormal { location, scale, shape } => scale.mul_add(continuous_inverse(p, |x| skew_cdf(x, *shape), false)?, *location),
                D::InverseGaussian { .. } => {
                    if p == 0.0 {
                        0.0
                    } else {
                        continuous_inverse(p, |x| self.cdf(x), true)?
                    }
                }
                _ => return crate::reference_families::quantile(d, p),
            },
        };
        if p > 0.0
            && p < 1.0
            && let Self::Scalar(d) = self
        {
            if q == 0.0
                && matches!(
                    d,
                    D::Gamma { .. }
                        | D::ChiSquared { .. }
                        | D::Erlang { .. }
                        | D::InverseGamma { .. }
                        | D::F { .. }
                        | D::Rayleigh { .. }
                        | D::LogLogistic { .. }
                        | D::InverseGaussian { .. }
                        | D::Beta { .. }
                )
            {
                return Err(SolverError::new("numerical_range", "Positive interior quantile underflows finite arithmetic."));
            }
            if q >= 9_007_199_254_740_992.0 && matches!(d, D::Geometric { .. }) {
                return Err(SolverError::new("integer_precision", "Geometric quantile exceeds exactly represented integer range."));
            }
        }
        if q.is_finite() { Ok(q) } else { Err(SolverError::new("numerical_range", "Distribution quantile exceeds finite arithmetic.")) }
    }
    pub fn cdf(&self, x: f64) -> Result<f64> {
        if x.is_nan() {
            return Err(domain());
        }
        let result = match self {
            Self::Metalog(prepared) => return prepared.cdf(x),
            Self::Continuous(v) => {
                if x < v[0] {
                    0.0
                } else if x >= v[v.len() - 1] {
                    1.0
                } else {
                    let upper = v.partition_point(|y| *y <= x);
                    let lower = upper - 1;
                    let scale = v[lower].abs().max(v[upper].abs());
                    let fraction = if (v[upper] - v[lower]).is_finite() {
                        (x - v[lower]) / (v[upper] - v[lower])
                    } else {
                        (x / scale - v[lower] / scale) / (v[upper] / scale - v[lower] / scale)
                    };
                    (lower as f64 + fraction) / (v.len() - 1) as f64
                }
            }
            Self::Points { values, cumulative } => {
                let upper = values.partition_point(|v| *v <= x);
                if upper == 0 { 0.0 } else { cumulative[upper - 1] }
            }
            Self::Shift { base, shift, scale } => {
                let z = standardize(x, *shift, *scale);
                if *scale > 0.0 { base.cdf(z)? } else { 1.0 - base.cdf(z.next_down())? }
            }
            Self::Truncated { base, lo, hi, lower, upper } => {
                if lower.is_some_and(|v| x < v) {
                    0.0
                } else if upper.is_some_and(|v| x >= v) {
                    1.0
                } else {
                    (base.cdf(x)? - lo) / (hi - lo)
                }
            }
            Self::Mixture { components, weights } => {
                special::finite_sum(components.iter().zip(weights).map(|(d, w)| d.cdf(x).map(|p| p * w)).collect::<Result<Vec<_>>>()?)?
            }
            Self::Compound { .. } => return Err(SolverError::new("compound_cdf_unavailable", "Generic compound-sum CDF is not available.")),
            Self::Scalar(d) => match d {
                D::DiscreteUniform { low, high } => {
                    if x < *low as f64 {
                        0.0
                    } else if x >= *high as f64 {
                        1.0
                    } else {
                        (x.floor() - *low as f64 + 1.0) / (*high as i128 - *low as i128 + 1) as f64
                    }
                }
                D::Beta { alpha, beta } => special::beta_i(*alpha, *beta, x)?,
                D::BetaPert { low, mode, high, weight } => {
                    if low == high {
                        if x < *low { 0.0 } else { 1.0 }
                    } else {
                        special::beta_i(
                            1.0 + weight * (mode - low) / (high - low),
                            1.0 + weight * (high - mode) / (high - low),
                            (x - low) / (high - low),
                        )?
                    }
                }
                D::Gamma { shape, scale } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        let ratio = x / scale;
                        if ratio == 0.0 {
                            return Err(SolverError::new("numerical_range", "Gamma standardization underflows."));
                        }
                        special::gamma_p(*shape, ratio)?
                    }
                }
                D::Erlang { shape, scale } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        special::gamma_p(f64::from(*shape), x / scale)?
                    }
                }
                D::ChiSquared { degrees_of_freedom } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        special::gamma_p(degrees_of_freedom * 0.5, x * 0.5)?
                    }
                }
                D::InverseGamma { shape, scale } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        1.0 - special::gamma_p(*shape, scale / x)?
                    }
                }
                D::NegativeBinomial { successes, probability: p } => {
                    if x < 0.0 {
                        0.0
                    } else if x == f64::INFINITY || *p == 1.0 {
                        1.0
                    } else {
                        special::beta_i(f64::from(*successes), x.floor() + 1.0, *p)?
                    }
                }
                D::Geometric { probability: p } => {
                    if x < 0.0 {
                        0.0
                    } else if x == f64::INFINITY || *p == 1.0 {
                        1.0
                    } else {
                        -((x.floor() + 1.0) * (-p).ln_1p()).exp_m1()
                    }
                }
                D::StudentT { degrees_of_freedom, location, scale } => student_cdf(standardize(x, *location, *scale), *degrees_of_freedom)?,
                D::F { numerator_df, denominator_df } => {
                    if x <= 0.0 {
                        0.0
                    } else if x == f64::INFINITY {
                        1.0
                    } else {
                        special::beta_i_logit(numerator_df * 0.5, denominator_df * 0.5, numerator_df.ln() + x.ln() - denominator_df.ln())?
                    }
                }
                D::Cauchy { location, scale } => 0.5 + standardize(x, *location, *scale).atan() / std::f64::consts::PI,
                D::Logistic { location, scale } => special::logistic(standardize(x, *location, *scale)),
                D::Laplace { location, scale } => {
                    if x < *location {
                        0.5 * standardize(x, *location, *scale).exp()
                    } else {
                        1.0 - 0.5 * (-standardize(x, *location, *scale)).exp()
                    }
                }
                D::Gumbel { location, scale } => {
                    let z = -standardize(x, *location, *scale);
                    (-z.exp()).exp()
                }
                D::Pareto { minimum, shape } => {
                    if x < *minimum {
                        0.0
                    } else {
                        1.0 - (minimum / x).powf(*shape)
                    }
                }
                D::Rayleigh { scale } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        -(-0.5 * (x / scale).powi(2)).exp_m1()
                    }
                }
                D::LogLogistic { shape, scale } => {
                    if x <= 0.0 {
                        0.0
                    } else {
                        special::logistic(shape * (x / scale).ln())
                    }
                }
                D::JohnsonSu { gamma, delta, location, scale } => normal_cdf(gamma + delta * asinh_standardize(x, *location, *scale))?,
                D::JohnsonSl { gamma, delta, location, scale } => {
                    if x <= *location {
                        0.0
                    } else {
                        normal_cdf(gamma + delta * log_abs_standardize(x, *location, *scale))?
                    }
                }
                D::JohnsonSb { gamma, delta, location, scale } => {
                    let z = standardize(x, *location, *scale);
                    if z <= 0.0 {
                        0.0
                    } else if z >= 1.0 {
                        1.0
                    } else {
                        normal_cdf(gamma + delta * (z.ln() - (-z).ln_1p()))?
                    }
                }
                D::SkewNormal { location, scale, shape } => skew_cdf(standardize(x, *location, *scale), *shape)?,
                D::InverseGaussian { mean, shape } => {
                    if x <= 0.0 {
                        0.0
                    } else if x == f64::INFINITY {
                        1.0
                    } else {
                        let root = (shape / x).sqrt();
                        let a = root * (x / mean - 1.0);
                        let b = -root * (x / mean + 1.0);
                        let second = if b < -8.0 {
                            let inv = 1.0 / (b * b);
                            let mills = 1.0 - inv + 3.0 * inv.powi(2) - 15.0 * inv.powi(3) + 105.0 * inv.powi(4);
                            (-0.5 * a * a - (-b).ln() - 0.918_938_533_204_672_7 + mills.ln()).exp()
                        } else {
                            (2.0 * shape / mean).exp() * normal_cdf(b)?
                        };
                        normal_cdf(a)? + second
                    }
                }
                _ => return crate::reference_families::cdf(d, x),
            },
        };
        if result.is_finite() { Ok(result.clamp(0.0, 1.0)) } else { Err(SolverError::new("numerical_range", "Distribution CDF is non-finite.")) }
    }
    pub fn sample(&self, seed: u64, binding: u64, trial: u64) -> Result<f64> {
        if let Self::Scalar(D::SkewNormal { location, scale, shape }) = self {
            // Azzalini construction; pure addresses keep both normals independent of scheduling.
            let u = normal_q(random::addressed_uniform(seed, binding, trial, 0))?;
            let v = normal_q(random::addressed_uniform(seed, binding, trial, 1))?;
            let z = shape.mul_add(u.abs(), v) / (1.0 + shape * shape).sqrt();
            let value = scale.mul_add(z, *location);
            return if value.is_finite() {
                Ok(value)
            } else {
                Err(SolverError::new("numerical_range", "Skew-normal sample exceeds finite arithmetic."))
            };
        }
        if let Self::Shift { base, shift, scale } = self {
            let value = scale.mul_add(base.sample(seed, binding, trial)?, *shift);
            return if value.is_finite() { Ok(value) } else { Err(SolverError::new("numerical_range", "Shifted sample is non-finite.")) };
        }
        if let Self::Mixture { components, weights } = self {
            let u = random::addressed_uniform(seed, binding, trial, 0);
            let mut total = 0.0;
            let mut index = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                total += w;
                if u < total {
                    index = i;
                    break;
                }
            }
            let child_seed = random::addressed_u64(seed, binding, trial, 0x4d49_5854);
            return components.get(index).ok_or_else(domain)?.sample(child_seed, binding, trial);
        }
        if let Self::Compound { frequency, severity, max_events } = self {
            let count = frequency.quantile(random::addressed_uniform(seed, binding, trial, 0))? as u32;
            if count > *max_events {
                return Err(SolverError::new("compound_event_limit", "Compound event-count budget exceeded."));
            }
            let child_seed = random::addressed_u64(seed, binding, trial, 0x434f_4d50);
            let mut values = Vec::new();
            reserve(&mut values, count as usize)?;
            for event in 0..count {
                values.push(severity.sample(child_seed, binding, u64::from(event))?);
            }
            special::finite_sum(values)
        } else {
            self.quantile(random::addressed_uniform(seed, binding, trial, 0))
        }
    }
}
pub fn student_cdf(x: f64, df: f64) -> Result<f64> {
    if !df.is_finite() || df <= 0.0 || x.is_nan() {
        return Err(domain());
    }
    if x == f64::NEG_INFINITY {
        return Ok(0.0);
    }
    if x == f64::INFINITY {
        return Ok(1.0);
    }
    if x == 0.0 {
        return Ok(0.5);
    }
    if x.abs() < 1.0 {
        let central = 0.5 * special::beta_i(0.5, df * 0.5, x * x / (df + x * x))?;
        return Ok(if x > 0.0 { 0.5 + central } else { 0.5 - central });
    }
    let tail = 0.5 * special::beta_i_logit(df * 0.5, 0.5, df.ln() - 2.0 * x.abs().ln())?;
    Ok(if x > 0.0 { 1.0 - tail } else { tail })
}
fn skew_cdf(x: f64, alpha: f64) -> Result<f64> {
    if !x.is_finite() {
        return Ok(if x < 0.0 { 0.0 } else { 1.0 });
    }
    if alpha == 0.0 {
        return normal_cdf(x);
    }
    if alpha < 0.0 {
        return Ok((if x <= 0.0 { 2.0 * special::normal_cdf(x)? - skew_cdf(x, -alpha)? } else { 1.0 - skew_cdf(-x, -alpha)? }).clamp(0.0, 1.0));
    }
    if x == 0.0 {
        return Ok(if alpha > 1.0 { (1.0 / alpha).atan() / std::f64::consts::PI } else { 0.5 - alpha.atan() / std::f64::consts::PI });
    }
    let h = x.abs();
    let tail = if alpha == 1.0 { (2.0 * special::normal_log_cdf(-h)?).exp() } else { skew_negative_tail(h, alpha)? };
    if x < 0.0 { Ok(tail) } else { Ok((special::gamma_p(0.5, 0.5 * x * x)? + tail).clamp(0.0, 1.0)) }
}
fn skew_negative_tail(h: f64, alpha: f64) -> Result<f64> {
    // Positive integral2*int_h^infinity phi(t)*Phi(-alpha*t)dt. Normalize by the
    // lower endpoint's log density, so tiny representable probabilities retain relative accuracy.
    let log_phi = special::normal_log_cdf(-alpha * h)?;
    let upper_log = 2.0_f64.ln() + special::normal_log_cdf(-h)? + log_phi;
    if upper_log < -746.0 {
        return Ok(0.0);
    }
    let width = 1.0 / ((1.0 + alpha * alpha).sqrt() + (1.0 + alpha * alpha) * h);
    let integrand = |s: f64| -> Result<f64> {
        let t = width.mul_add(s, h);
        Ok((-0.5 * (t - h) * (t + h) + special::normal_log_cdf(-alpha * t)? - log_phi).exp())
    };
    let integral = adaptive_positive_integral(integrand)?;
    Ok((-0.5 * h * h - 0.918_938_533_204_672_7 + log_phi + 2.0_f64.ln() + width.ln() + integral.ln()).exp().clamp(0.0, 1.0))
}
fn adaptive_positive_integral<F: Fn(f64) -> Result<f64>>(f: F) -> Result<f64> {
    struct Frame {
        a: f64,
        b: f64,
        fa: f64,
        fm: f64,
        fb: f64,
        estimate: f64,
        tolerance: f64,
        depth: u8,
    }
    let mut stack = Vec::new();
    reserve(&mut stack, 64)?;
    let fa = f(0.0)?;
    let fm = f(20.0)?;
    let fb = f(40.0)?;
    stack.push(Frame { a: 0.0, b: 40.0, fa, fm, fb, estimate: 40.0 / 6.0 * (fa + 4.0 * fm + fb), tolerance: 1e-11, depth: 0 });
    let mut evaluations = 3;
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    while let Some(frame) = stack.pop() {
        if evaluations + 2 > 65536 {
            return Err(SolverError::new("special_convergence", "Skew-normal quadrature exceeds its evaluation budget."));
        }
        let mid = 0.5 * (frame.a + frame.b);
        let left = f(0.5 * (frame.a + mid))?;
        let right = f(0.5 * (mid + frame.b))?;
        evaluations += 2;
        let l = (mid - frame.a) / 6.0 * (frame.fa + 4.0 * left + frame.fm);
        let r = (frame.b - mid) / 6.0 * (frame.fm + 4.0 * right + frame.fb);
        let error = l + r - frame.estimate;
        if error.abs() <= 15.0 * frame.tolerance {
            let value = l + r + error / 15.0;
            let next = sum + value;
            correction += if sum.abs() >= value.abs() { (sum - next) + value } else { (value - next) + sum };
            sum = next;
        } else {
            if frame.depth >= 20 {
                return Err(SolverError::new("special_convergence", "Skew-normal quadrature reached its refinement limit."));
            }
            stack.push(Frame {
                a: mid,
                b: frame.b,
                fa: frame.fm,
                fm: right,
                fb: frame.fb,
                estimate: r,
                tolerance: frame.tolerance * 0.5,
                depth: frame.depth + 1,
            });
            stack.push(Frame {
                a: frame.a,
                b: mid,
                fa: frame.fa,
                fm: left,
                fb: frame.fm,
                estimate: l,
                tolerance: frame.tolerance * 0.5,
                depth: frame.depth + 1,
            });
        }
    }
    special::finite_sum([sum, correction])
}
