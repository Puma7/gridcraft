//! Validated distributions with inverse-CDF sampling.
//! Normal inverse CDF uses the piecewise rational approximation of P. J. Acklam.
//! The normal CDF is a classical erf approximation (absolute error about 1.5e-7).
//! Poisson rate is deliberately limited to 512; binomial trials to 10,000 for bounded work.
//! Unsupported parameter sizes fail explicitly rather than selecting a normal approximation.
use crate::random::addressed_uniform;
use solvercraft_types::{Distribution, Result, SolverError};

const MAX_BINOMIAL_TRIALS: u32 = 10_000;
const MAX_POISSON_RATE: f64 = 512.0;
const MAX_EMPIRICAL_SAMPLES: usize = 1_000_000;

fn error(message: &str) -> SolverError {
    SolverError::new("distribution_domain", message)
}
fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}
fn is_probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub fn validate_distribution(distribution: &Distribution) -> Result<()> {
    crate::extended::validate_tree(distribution)
}
pub(crate) fn validate_leaf(distribution: &Distribution) -> Result<()> {
    match distribution {
        Distribution::Constant { value } if value.is_finite() => Ok(()),
        Distribution::Uniform { low, high } if finite(&[*low, *high]) && low <= high && (high - low).is_finite() => Ok(()),
        Distribution::Normal { mean, std_dev } if finite(&[*mean, *std_dev]) && *std_dev >= 0.0 => Ok(()),
        Distribution::Lognormal { log_mean, log_std_dev }
            if finite(&[*log_mean, *log_std_dev])
                && *log_std_dev >= 0.0
                && (*log_std_dev > 0.0 || (log_mean.exp().is_finite() && log_mean.exp() > 0.0)) =>
        {
            Ok(())
        }
        Distribution::Triangular { low, mode, high } if finite(&[*low, *mode, *high]) && low <= mode && mode <= high && (high - low).is_finite() => {
            Ok(())
        }
        Distribution::Bernoulli { probability } if is_probability(*probability) => Ok(()),
        Distribution::Binomial { trials, probability } if is_probability(*probability) => {
            if *trials > MAX_BINOMIAL_TRIALS && *probability > 0.0 && *probability < 1.0 {
                Err(SolverError::new("distribution_limit", "Binomial supports at most 10,000 trials per variate."))
            } else {
                Ok(())
            }
        }
        Distribution::Poisson { rate } if rate.is_finite() && *rate >= 0.0 => {
            if *rate > MAX_POISSON_RATE {
                Err(SolverError::new("distribution_limit", "Poisson supports rates up to 512 in this engine."))
            } else {
                Ok(())
            }
        }
        Distribution::Exponential { rate } if rate.is_finite() && *rate > 0.0 => Ok(()),
        Distribution::Weibull { shape, scale } if finite(&[*shape, *scale]) && *shape > 0.0 && *scale > 0.0 => Ok(()),
        Distribution::Empirical { values } if !values.is_empty() && values.len() <= MAX_EMPIRICAL_SAMPLES && values.iter().all(|v| v.is_finite()) => {
            Ok(())
        }
        _ => crate::extended::validate_scalar(distribution),
    }
}

/// Extra heap storage required by a prepared distribution, excluding the caller's original data.
pub fn prepared_storage_bytes(distribution: &Distribution) -> Result<u64> {
    validate_distribution(distribution)?;
    if crate::extended::is_extended(distribution) {
        return crate::extended::storage(distribution);
    }
    let elements = match distribution {
        Distribution::Empirical { values } => values.len() as u64,
        Distribution::Binomial { trials, probability } if *probability > 0.0 && *probability < 1.0 => u64::from(*trials) + 1,
        Distribution::Poisson { rate } if *rate > 0.0 => poisson_last(*rate) as u64 + 1,
        _ => 0,
    };
    elements
        .checked_mul(8)
        .and_then(|x| x.checked_add(std::mem::size_of::<PreparedDistribution>() as u64))
        .ok_or_else(|| SolverError::new("memory_limit", "Prepared distribution size overflow."))
}

#[derive(Debug, Clone)]
pub struct PreparedDistribution {
    distribution: Distribution,
    cumulative: Vec<f64>,
    extra: Option<Box<crate::extended::PreparedExtra>>,
}
impl PreparedDistribution {
    pub fn new(distribution: &Distribution) -> Result<Self> {
        validate_distribution(distribution)?;
        if crate::extended::is_extended(distribution) {
            return Ok(Self {
                distribution: Distribution::Constant { value: 0.0 },
                cumulative: Vec::new(),
                extra: Some(Box::new(crate::extended::prepare(distribution)?)),
            });
        }
        let distribution = match distribution {
            Distribution::Empirical { values } => {
                let mut copy = allocate(values.len())?;
                copy.copy_from_slice(values);
                copy.sort_unstable_by(f64::total_cmp);
                Distribution::Empirical { values: copy }
            }
            other => other.clone(),
        };
        let cumulative = match &distribution {
            Distribution::Binomial { trials, probability } if *probability > 0.0 && *probability < 1.0 && *trials > 0 => {
                binomial_table(*trials, *probability)?
            }
            Distribution::Poisson { rate } if *rate > 0.0 => poisson_table(*rate)?,
            _ => Vec::new(),
        };
        Ok(Self { distribution, cumulative, extra: None })
    }
    pub fn quantile(&self, p: f64) -> Result<f64> {
        if let Some(extra) = &self.extra {
            return extra.quantile(p);
        }
        if !is_probability(p) {
            return Err(error("Quantile probability must lie in [0,1]."));
        }
        let value = match &self.distribution {
            Distribution::Constant { value } => *value,
            Distribution::Uniform { low, high } => {
                // Preserve the declared endpoints even if subtracting a large
                // negative lower bound rounds away the upper bound.
                if p == 1.0 { *high } else { low + (high - low) * p }
            }
            Distribution::Normal { mean, std_dev } => {
                if *std_dev == 0.0 {
                    *mean
                } else {
                    std_dev.mul_add(inverse_normal(p)?, *mean)
                }
            }
            Distribution::Lognormal { log_mean, log_std_dev } => {
                if *log_std_dev == 0.0 {
                    log_mean.exp()
                } else if p == 0.0 {
                    0.0
                } else {
                    (log_mean + log_std_dev * inverse_normal(p)?).exp()
                }
            }
            Distribution::Triangular { low, mode, high } => {
                let span = high - low;
                if span == 0.0 || p == 0.0 {
                    *low
                } else if p == 1.0 {
                    *high
                } else {
                    let split = (mode - low) / span;
                    if p <= split {
                        let left_width = mode - low;
                        let root = (p / split).sqrt();
                        if p >= 0.5 {
                            // Approach the mode without subtracting two
                            // rounded, almost equal square roots.
                            mode - (left_width * (split - p)) / (split * (1.0 + root))
                        } else {
                            left_width.mul_add(root, *low)
                        }
                    } else if p <= 0.5 {
                        // Rationalize 1-sqrt((1-p)*(1-split)). Forming 1-p
                        // first would erase tiny positive probabilities.
                        let root = (1.0 - p).sqrt() * (1.0 - split).sqrt();
                        let numerator = p.mul_add(1.0 - split, split);
                        low + (span * numerator) / (1.0 + root)
                    } else {
                        let right_width = high - mode;
                        (-right_width).mul_add(((1.0 - p) / (1.0 - split)).sqrt(), *high)
                    }
                }
            }
            Distribution::Bernoulli { probability } => {
                if *probability == 1.0 {
                    1.0
                } else if p <= 1.0 - probability {
                    0.0
                } else {
                    1.0
                }
            }
            Distribution::Binomial { trials, probability } => {
                if *trials == 0 || *probability == 0.0 {
                    0.0
                } else if *probability == 1.0 || p == 1.0 {
                    f64::from(*trials)
                } else {
                    self.discrete_quantile(p)?
                }
            }
            Distribution::Poisson { rate } => {
                if *rate == 0.0 {
                    0.0
                } else if p == 1.0 {
                    return Err(unbounded_quantile());
                } else {
                    self.discrete_quantile(p)?
                }
            }
            Distribution::Exponential { rate } => {
                if p == 1.0 {
                    return Err(unbounded_quantile());
                } else {
                    -(-p).ln_1p() / rate
                }
            }
            Distribution::Weibull { shape, scale } => {
                if p == 1.0 {
                    return Err(unbounded_quantile());
                } else if p == 0.0 {
                    0.0
                } else {
                    (scale.ln() + (-(-p).ln_1p()).ln() / shape).exp()
                }
            }
            Distribution::Empirical { values } => {
                let index = ((p * values.len() as f64).ceil() as usize).saturating_sub(1).min(values.len().saturating_sub(1));
                *values.get(index).ok_or_else(|| error("Empty empirical distribution."))?
            }
            _ => return Err(error("Distribution preparation is inconsistent.")),
        };
        if !value.is_finite()
            || (value == 0.0
                && p > 0.0
                && matches!(&self.distribution, Distribution::Lognormal { .. } | Distribution::Exponential { .. } | Distribution::Weibull { .. }))
        {
            return Err(SolverError::new("numerical_range", "Distribution quantile exceeds finite arithmetic."));
        }
        Ok(value)
    }
    fn discrete_quantile(&self, p: f64) -> Result<f64> {
        let index = self.cumulative.partition_point(|x| *x < p).min(self.cumulative.len().saturating_sub(1));
        if self.cumulative.is_empty() {
            return Err(error("Discrete distribution table is empty."));
        }
        Ok(index as f64)
    }
    pub fn cdf(&self, x: f64) -> Result<f64> {
        if let Some(extra) = &self.extra {
            return extra.cdf(x);
        }
        if x.is_nan() {
            return Err(error("CDF value must not be NaN."));
        }
        let value = match &self.distribution {
            Distribution::Constant { value } => {
                if x < *value {
                    0.0
                } else {
                    1.0
                }
            }
            Distribution::Uniform { low, high } => {
                if low == high {
                    if x < *low { 0.0 } else { 1.0 }
                } else {
                    ((x - low) / (high - low)).clamp(0.0, 1.0)
                }
            }
            Distribution::Normal { mean, std_dev } => {
                if *std_dev == 0.0 {
                    if x < *mean { 0.0 } else { 1.0 }
                } else {
                    let difference = x - mean;
                    let standardized = if difference.is_finite() || !x.is_finite() { difference / std_dev } else { x / std_dev - mean / std_dev };
                    normal_cdf(standardized)
                }
            }
            Distribution::Lognormal { log_mean, log_std_dev } => {
                if x <= 0.0 {
                    0.0
                } else if *log_std_dev == 0.0 {
                    if x < log_mean.exp() { 0.0 } else { 1.0 }
                } else {
                    normal_cdf((x.ln() - log_mean) / log_std_dev)
                }
            }
            Distribution::Triangular { low, mode, high } => {
                if low == high {
                    if x < *low { 0.0 } else { 1.0 }
                } else if x <= *low {
                    0.0
                } else if x >= *high {
                    1.0
                } else if x <= *mode {
                    let distance = x - low;
                    (distance / (high - low)) * (distance / (mode - low))
                } else {
                    let distance = high - x;
                    1.0 - (distance / (high - low)) * (distance / (high - mode))
                }
            }
            Distribution::Bernoulli { probability } => {
                if x < 0.0 {
                    0.0
                } else if x < 1.0 {
                    1.0 - probability
                } else {
                    1.0
                }
            }
            Distribution::Binomial { trials, probability } => {
                if x < 0.0 {
                    0.0
                } else if x >= f64::from(*trials) || *probability == 0.0 {
                    1.0
                } else if *probability == 1.0 {
                    0.0
                } else {
                    self.discrete_cdf(x)
                }
            }
            Distribution::Poisson { rate } => {
                if x < 0.0 {
                    0.0
                } else if *rate == 0.0 {
                    1.0
                } else {
                    self.discrete_cdf(x)
                }
            }
            Distribution::Exponential { rate } => {
                if x <= 0.0 {
                    0.0
                } else {
                    -(-rate * x).exp_m1()
                }
            }
            Distribution::Weibull { shape, scale } => {
                if x <= 0.0 {
                    0.0
                } else {
                    // Avoid overflow/underflow in x/scale before a small shape
                    // can bring the exponent back into a representable range.
                    let ratio = x / scale;
                    let log_ratio = if (0.5..=2.0).contains(&ratio) {
                        // Sterbenz-safe subtraction retains adjacent finite
                        // values even when ln(x) and ln(scale) would be equal.
                        ((x - scale) / scale).ln_1p()
                    } else if ratio > 0.0 && ratio.is_finite() {
                        ratio.ln()
                    } else {
                        x.ln() - scale.ln()
                    };
                    let log_power = shape * log_ratio;
                    -(-log_power.exp()).exp_m1()
                }
            }
            Distribution::Empirical { values } => values.partition_point(|v| *v <= x) as f64 / values.len() as f64,
            _ => return Err(error("Distribution preparation is inconsistent.")),
        };
        Ok(value.clamp(0.0, 1.0))
    }
    fn discrete_cdf(&self, x: f64) -> f64 {
        if x < 0.0 { 0.0 } else { self.cumulative.get(x.floor() as usize).copied().unwrap_or(1.0) }
    }
    pub fn sample(&self, seed: u64, binding: u64, trial: u64) -> Result<f64> {
        if let Some(extra) = &self.extra { extra.sample(seed, binding, trial) } else { self.quantile(addressed_uniform(seed, binding, trial, 0)) }
    }
}
pub fn quantile(distribution: &Distribution, p: f64) -> Result<f64> {
    PreparedDistribution::new(distribution)?.quantile(p)
}
pub fn cdf(distribution: &Distribution, x: f64) -> Result<f64> {
    PreparedDistribution::new(distribution)?.cdf(x)
}
pub fn sample(distribution: &Distribution, seed: u64, binding: u64, trial: u64) -> Result<f64> {
    PreparedDistribution::new(distribution)?.sample(seed, binding, trial)
}
fn unbounded_quantile() -> SolverError {
    SolverError::new("unbounded_quantile", "Endpoint quantile is infinite; request a probability strictly inside (0,1).")
}
fn allocate(size: usize) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values.try_reserve_exact(size).map_err(|_| SolverError::new("allocation", "Cannot allocate distribution table."))?;
    values.resize(size, 0.0);
    Ok(values)
}
fn cumulative_table(mut values: Vec<f64>) -> Result<Vec<f64>> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in &values {
        let y = *value - correction;
        let t = sum + y;
        correction = (t - sum) - y;
        sum = t;
    }
    if !sum.is_finite() || sum <= 0.0 {
        return Err(SolverError::new("numerical_range", "Discrete probability normalization failed."));
    }
    let mut cumulative = 0.0;
    let mut correction = 0.0;
    for value in &mut values {
        let y = *value / sum - correction;
        let t = cumulative + y;
        correction = (t - cumulative) - y;
        cumulative = t;
        *value = cumulative.clamp(0.0, 1.0);
    }
    if let Some(last) = values.last_mut() {
        *last = 1.0;
    }
    Ok(values)
}
fn binomial_table(trials: u32, p: f64) -> Result<Vec<f64>> {
    let count = trials as usize;
    let mode = (((f64::from(trials) + 1.0) * p).floor() as usize).min(count);
    let mut values = allocate(count + 1)?;
    // Start at the mode, with an arbitrary positive weight. Normalize only once at the end.
    // This avoids factorials and underflow of P(X=0) for large n or p.
    if let Some(value) = values.get_mut(mode) {
        *value = 1.0;
    }
    let mut weight = 1.0;
    for k in (1..=mode).rev() {
        weight *= k as f64 / (count - k + 1) as f64 * (1.0 - p) / p;
        if let Some(value) = values.get_mut(k - 1) {
            *value = weight;
        }
    }
    weight = 1.0;
    for k in mode..count {
        weight *= (count - k) as f64 / (k + 1) as f64 * p / (1.0 - p);
        if let Some(value) = values.get_mut(k + 1) {
            *value = weight;
        }
    }
    cumulative_table(values)
}
fn poisson_last(rate: f64) -> usize {
    (rate + 40.0 * rate.sqrt() + 100.0).ceil() as usize
}
fn poisson_table(rate: f64) -> Result<Vec<f64>> {
    let last = poisson_last(rate);
    let mode = rate.floor() as usize;
    let mut values = allocate(last + 1)?;
    if let Some(value) = values.get_mut(mode) {
        *value = 1.0;
    }
    let mut weight = 1.0;
    for k in (1..=mode).rev() {
        weight *= k as f64 / rate;
        if let Some(value) = values.get_mut(k - 1) {
            *value = weight;
        }
    }
    weight = 1.0;
    for k in mode..last {
        weight *= rate / (k + 1) as f64;
        if let Some(value) = values.get_mut(k + 1) {
            *value = weight;
        }
    }
    cumulative_table(values)
}
pub(crate) fn inverse_normal(p: f64) -> Result<f64> {
    if p <= 0.0 || p >= 1.0 {
        return Err(unbounded_quantile());
    }
    let a = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    let b = [-5.447_609_879_822_406e1, 1.615_858_368_580_409e2, -1.556_989_798_598_866e2, 6.680_131_188_771_972e1, -1.328_068_155_288_572e1];
    let c = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    let d = [7.784_695_709_041_462e-3, 3.224_671_290_700_398e-1, 2.445_134_137_142_996, 3.754_408_661_907_416];
    let poly = |coefficients: &[f64], x: f64| coefficients.iter().fold(0.0, |acc, v| acc * x + v);
    if p < 0.024_25 {
        let q = (-2.0 * p.ln()).sqrt();
        Ok(poly(&c, q) / (poly(&d, q) * q + 1.0))
    } else if p > 0.975_75 {
        let q = (-2.0 * (-p).ln_1p()).sqrt();
        Ok(-poly(&c, q) / (poly(&d, q) * q + 1.0))
    } else {
        let q = p - 0.5;
        let r = q * q;
        Ok(poly(&a, r) * q / (poly(&b, r) * r + 1.0))
    }
}
pub(crate) fn normal_cdf(x: f64) -> f64 {
    if x == f64::NEG_INFINITY {
        return 0.0;
    }
    if x == f64::INFINITY {
        return 1.0;
    }
    if x == 0.0 {
        return 0.5;
    }
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let tail = (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t + 0.254_829_592) * t * (-z * z).exp();
    if x >= 0.0 { 1.0 - 0.5 * tail } else { 0.5 * tail }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_quantiles_match_independent_constants() -> Result<()> {
        let d = Distribution::Normal { mean: 0.0, std_dev: 1.0 };
        for (p, q) in [(0.5, 0.0), (0.975, 1.959_963_984_540_054), (0.001, -3.090_232_306_167_813), (1e-8, -5.612_001_244_174_789)] {
            assert!((quantile(&d, p)? - q).abs() < 1e-7);
            assert!((cdf(&d, q)? - p).abs() < 2e-7);
        }
        assert!(quantile(&d, 0.0).is_err());
        assert!(quantile(&d, 1.0).is_err());
        Ok(())
    }
    #[test]
    fn degenerate_and_parameter_cases() -> Result<()> {
        assert_eq!(quantile(&Distribution::Normal { mean: 3.0, std_dev: 0.0 }, 0.0)?, 3.0);
        assert_eq!(quantile(&Distribution::Triangular { low: 2.0, mode: 2.0, high: 2.0 }, 0.9)?, 2.0);
        assert!(validate_distribution(&Distribution::Uniform { low: 2.0, high: 1.0 }).is_err());
        assert!(validate_distribution(&Distribution::Normal { mean: 0.0, std_dev: -1.0 }).is_err());
        assert!(validate_distribution(&Distribution::Poisson { rate: 513.0 }).is_err());
        assert!(validate_distribution(&Distribution::Empirical { values: vec![] }).is_err());
        assert!(validate_distribution(&Distribution::Constant { value: f64::INFINITY }).is_err());
        Ok(())
    }
    #[test]
    fn inverse_cdf_semantics_and_support() -> Result<()> {
        assert_eq!(quantile(&Distribution::Bernoulli { probability: 0.75 }, 0.25)?, 0.0);
        assert_eq!(quantile(&Distribution::Binomial { trials: 1, probability: 0.75 }, 0.25)?, 0.0);
        assert_eq!(quantile(&Distribution::Empirical { values: vec![4.0, 1.0, 3.0, 2.0] }, 0.5)?, 2.0);
        assert_eq!(quantile(&Distribution::Poisson { rate: 1.0 }, 0.5)?, 1.0);
        assert_eq!(quantile(&Distribution::Binomial { trials: 10, probability: 0.5 }, 0.5)?, 5.0);
        for (p, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 2.0), (1.0, 4.0)] {
            assert!((quantile(&Distribution::Triangular { low: 0.0, mode: 2.0, high: 4.0 }, p)? - expected).abs() < 0.5);
        }
        for seed in 0..1000 {
            let x = sample(&Distribution::Uniform { low: -2.0, high: 3.0 }, seed, 7, 1)?;
            assert!((-2.0..=3.0).contains(&x));
        }
        Ok(())
    }
    #[test]
    fn discrete_tables_are_stable_near_endpoints() -> Result<()> {
        for (n, p) in [(10_000, 0.5), (10_000, 0.999_99), (10_000, 0.000_01)] {
            let dist = PreparedDistribution::new(&Distribution::Binomial { trials: n, probability: p })?;
            let q = dist.quantile(0.5)?;
            assert!((q - f64::from(n) * p).abs() < 2.0);
            assert!(dist.cdf(q)? >= 0.5);
            if q > 0.0 {
                assert!(dist.cdf(q - 1.0)? < 0.5);
            }
        }
        let p = PreparedDistribution::new(&Distribution::Poisson { rate: 512.0 })?;
        assert!((p.quantile(0.5)? - 512.0).abs() <= 1.0);
        Ok(())
    }
}
