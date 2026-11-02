//! Stable discrete interval masses used by rounded affine pushforwards.
use solvercraft_types::{Distribution as D, Result, SolverError};
pub(crate) fn log_sum(terms: impl IntoIterator<Item = f64>) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    let mut sum = 0.0;
    for term in terms {
        if term == f64::NEG_INFINITY {
            continue;
        }
        if !term.is_finite() {
            return Err(range());
        }
        if term > maximum {
            sum = sum * (maximum - term).exp() + 1.0;
            maximum = term;
        } else {
            sum += (term - maximum).exp();
        }
    }
    Ok(if maximum == f64::NEG_INFINITY { maximum } else { maximum + sum.ln() })
}
fn range() -> SolverError {
    SolverError::new("numerical_range", "Discrete interval mass is outside the bounded finite evaluation range.")
}
fn atom(value: f64, lower: f64, upper: f64) -> f64 {
    if value >= lower && value <= upper { 0.0 } else { f64::NEG_INFINITY }
}
pub(crate) fn interval(d: &D, lower: f64, upper: f64) -> Result<f64> {
    if lower > upper {
        return Ok(f64::NEG_INFINITY);
    }
    let zero = f64::NEG_INFINITY;
    let q = match d {
        D::Constant { value } => atom(*value, lower, upper),
        D::Normal { mean, std_dev: 0.0 } => atom(*mean, lower, upper),
        D::Lognormal { log_mean, log_std_dev: 0.0 } => atom(log_mean.exp(), lower, upper),
        D::Uniform { low, high } | D::Triangular { low, high, .. } | D::BetaPert { low, high, .. } if low == high => atom(*low, lower, upper),
        D::Empirical { values } => {
            let count = values.iter().filter(|v| **v >= lower && **v <= upper).count();
            if count == 0 { zero } else { (count as f64 / values.len() as f64).ln() }
        }
        D::Categorical { values, weights } => {
            let denominator = log_sum(weights.iter().filter(|w| **w > 0.0).map(|w| w.ln()))?;
            log_sum(values.iter().zip(weights).filter(|(v, w)| **v >= lower && **v <= upper && **w > 0.0).map(|(_, w)| w.ln()))? - denominator
        }
        D::DiscreteUniform { low, high } => {
            let left = lower.ceil().max(*low as f64);
            let right = upper.floor().min(*high as f64);
            if left > right { zero } else { ((right - left + 1.0) / (*high as i128 - *low as i128 + 1) as f64).ln() }
        }
        D::Bernoulli { probability } => log_sum([atom(0.0, lower, upper) + (-probability).ln_1p(), atom(1.0, lower, upper) + probability.ln()])?,
        D::Binomial { trials, probability } if *trials == 0 || *probability == 0.0 || *probability == 1.0 => {
            atom(if *probability == 1.0 { f64::from(*trials) } else { 0.0 }, lower, upper)
        }
        D::Poisson { rate: 0.0 } | D::NegativeBinomial { probability: 1.0, .. } | D::Geometric { probability: 1.0 } => atom(0.0, lower, upper),
        D::ShiftScale { base, shift, scale } => {
            if let Some((left, right)) = crate::affine::preimage(lower, upper, *shift, *scale) {
                interval(base, left, right)?
            } else {
                zero
            }
        }
        D::Truncated { base, lower: bound_low, upper: bound_high } => {
            let left = bound_low.unwrap_or(f64::NEG_INFINITY);
            let right = bound_high.unwrap_or(f64::INFINITY);
            interval(base, lower.max(left), upper.min(right))? - interval(base, left, right)?
        }
        D::Mixture { components, weights } => {
            let denominator = log_sum(weights.iter().filter(|w| **w > 0.0).map(|w| w.ln()))?;
            let terms = components
                .iter()
                .zip(weights)
                .filter(|(_, w)| **w > 0.0)
                .map(|(d, w)| interval(d, lower, upper).map(|mass| mass + w.ln()))
                .collect::<Result<Vec<_>>>()?;
            log_sum(terms)? - denominator
        }
        _ => count_interval(d, lower, upper)?,
    };
    if q.is_nan() || q == f64::INFINITY { Err(range()) } else { Ok(q.min(0.0)) }
}
fn count_interval(d: &D, lower: f64, upper: f64) -> Result<f64> {
    let (support_low, support_high) = match d {
        D::Binomial { trials, .. } => (0.0, f64::from(*trials)),
        D::Hypergeometric { population, successes, draws } => {
            (f64::from(draws.saturating_sub(population - successes)), f64::from((*draws).min(*successes)))
        }
        D::Logarithmic { .. } => (1.0, f64::INFINITY),
        D::Poisson { .. } | D::NegativeBinomial { .. } | D::Geometric { .. } => (0.0, f64::INFINITY),
        _ => return Err(range()),
    };
    let left = lower.ceil().max(support_low);
    let right = upper.floor().min(support_high);
    if left > right {
        return Ok(f64::NEG_INFINITY);
    }
    if left == support_low && right == support_high {
        return Ok(0.0);
    }
    if let D::Geometric { probability } = d {
        let log_tail = left * (-probability).ln_1p();
        return Ok(if right.is_infinite() { log_tail } else { log_tail + (-((right - left + 1.0) * (-probability).ln_1p()).exp_m1()).ln() });
    }
    // At most10001 integer atoms, independent of the requested floating range.
    if right.is_finite() && right < 9_007_199_254_740_992.0 && (right - left <= 16.0 || matches!(d, D::Hypergeometric { .. })) {
        let mut terms = Vec::new();
        terms.try_reserve_exact((right - left) as usize + 1).map_err(|_| range())?;
        for index in 0..=(right - left) as usize {
            terms.push(crate::density::log_density(d, left + index as f64)?);
        }
        return log_sum(terms);
    }
    // Resolve broad count intervals in whichever probability tail is smaller.
    let cdf = |k: f64| -> Result<f64> {
        if k < support_low {
            return Ok(0.0);
        }
        if k >= support_high || k.is_infinite() {
            return Ok(1.0);
        }
        match d {
            D::Binomial { trials, probability } => {
                crate::special::beta_i_logit(f64::from(*trials) - k, k + 1.0, (-probability).ln_1p() - probability.ln())
            }
            D::Poisson { rate } => crate::special::gamma_q(k + 1.0, *rate),
            _ => crate::cdf(d, k),
        }
    };
    let survival = |k: f64| -> Result<f64> {
        if k < support_low {
            return Ok(1.0);
        }
        if k.is_infinite() {
            return Ok(0.0);
        }
        match d {
            D::Binomial { trials, probability } => {
                if k >= f64::from(*trials) {
                    Ok(0.0)
                } else {
                    crate::special::beta_i_logit(k + 1.0, f64::from(*trials) - k, probability.ln() - (-probability).ln_1p())
                }
            }
            D::Poisson { rate } => crate::special::gamma_p(k + 1.0, *rate),
            D::NegativeBinomial { successes, probability } => {
                crate::special::beta_i_logit(k + 1.0, f64::from(*successes), (-probability).ln_1p() - probability.ln())
            }
            D::Logarithmic { probability } if k < 9_007_199_254_740_992.0 => crate::reference_families::logseries_tail(*probability, k as u64),
            _ => Ok(1.0 - cdf(k)?),
        }
    };
    let hi = cdf(right)?;
    let mass = if hi <= 0.5 { hi - cdf(left - 1.0)? } else { survival(left - 1.0)? - survival(right)? };
    if mass > 0.0 { Ok(mass.ln()) } else { Err(range()) }
}
