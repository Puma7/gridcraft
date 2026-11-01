//! Canonical deterministic spreadsheet values: finite analytic mean where implemented,
//! otherwise explicitly labelled median fallback. Undefined means are never replaced by zero.
use crate::{quantile, special, validate_distribution};
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution as D, Result, SolverError};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NominalStatistic {
    Mean,
    MedianFallback,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NominalValue {
    pub value: f64,
    pub statistic: NominalStatistic,
}
fn average(values: &[f64]) -> Result<f64> {
    let scale = values.iter().copied().map(f64::abs).fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    Ok(scale * special::finite_sum(values.iter().map(|x| (x / scale) / values.len() as f64))?)
}
fn exp_gamma(scale: f64, terms: &[f64]) -> Result<f64> {
    Ok((scale.ln() + special::finite_sum(terms.iter().copied())?).exp())
}
/// Median fallback also covers implemented families whose analytic mean is unavailable,
/// e.g. bounded Johnson-SB or generic truncation. Inspect nominal_summary for the distinction.
pub fn nominal_value(d: &D) -> Result<f64> {
    Ok(nominal_summary(d)?.value)
}
pub fn nominal_summary(d: &D) -> Result<NominalValue> {
    validate_distribution(d)?;
    let gamma = 0.577_215_664_901_532_9;
    let mean = match d {
        D::Constant { value } => Some(*value),
        D::Uniform { low, high } => Some(0.5 * low + 0.5 * high),
        D::DiscreteUniform { low, high } => Some(0.5 * (*low as f64) + 0.5 * (*high as f64)),
        D::Normal { mean, .. } | D::InverseGaussian { mean, .. } => Some(*mean),
        D::Lognormal { log_mean, log_std_dev } => Some((log_mean + 0.5 * log_std_dev * log_std_dev).exp()),
        D::Triangular { low, mode, high } => Some(average(&[*low, *mode, *high])?),
        D::BetaPert { low, mode, high, weight } => {
            Some(special::finite_sum([low / (weight + 2.0), mode * (weight / (weight + 2.0)), high / (weight + 2.0)])?)
        }
        D::Beta { alpha, beta } => Some(alpha / (alpha + beta)),
        D::Gamma { shape, scale } => Some(shape * scale),
        D::Exponential { rate } => Some(1.0 / rate),
        D::Weibull { shape, scale } => Some(exp_gamma(*scale, &[special::log_gamma(1.0 + 1.0 / shape)?])?),
        D::Bernoulli { probability } => Some(*probability),
        D::Binomial { trials, probability } => Some(f64::from(*trials) * probability),
        D::Poisson { rate } => Some(*rate),
        D::NegativeBinomial { successes, probability } => Some(f64::from(*successes) * ((1.0 - probability) / probability)),
        D::Geometric { probability } => Some((1.0 - probability) / probability),
        D::Empirical { values } => Some(average(values)?),
        D::EmpiricalContinuous { values } => {
            if values.len() == 1 {
                Some(values[0])
            } else {
                let mut ordered = Vec::new();
                ordered.try_reserve_exact(values.len()).map_err(|_| SolverError::new("allocation", "Cannot allocate empirical nominal knots."))?;
                ordered.extend_from_slice(values);
                ordered.sort_unstable_by(f64::total_cmp);
                let scale = ordered.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
                if scale == 0.0 {
                    Some(0.0)
                } else {
                    Some(
                        scale
                            * special::finite_sum(
                                ordered
                                    .iter()
                                    .enumerate()
                                    .map(|(i, x)| x / scale / (ordered.len() - 1) as f64 * if i == 0 || i + 1 == ordered.len() { 0.5 } else { 1.0 }),
                            )?,
                    )
                }
            }
        }
        D::StudentT { degrees_of_freedom, location, .. } => {
            if *degrees_of_freedom > 1.0 {
                Some(*location)
            } else {
                None
            }
        }
        D::ChiSquared { degrees_of_freedom } => Some(*degrees_of_freedom),
        D::F { denominator_df, .. } => {
            if *denominator_df > 2.0 {
                Some(denominator_df / (denominator_df - 2.0))
            } else {
                None
            }
        }
        D::Cauchy { .. } | D::Levy { .. } => None,
        D::Logistic { location, .. } | D::Laplace { location, .. } | D::HyperbolicSecant { location, .. } => Some(*location),
        D::Gumbel { location, scale } => Some(scale.mul_add(gamma, *location)),
        D::MinExtreme { location, scale } => Some((-scale).mul_add(gamma, *location)),
        D::Pareto { minimum, shape } => {
            if *shape > 1.0 {
                Some(minimum * (shape / (shape - 1.0)))
            } else {
                None
            }
        }
        D::Pareto2 { shape, scale } => {
            if *shape > 1.0 {
                Some(scale / (shape - 1.0))
            } else {
                None
            }
        }
        D::Rayleigh { scale } => Some(scale * std::f64::consts::FRAC_PI_2.sqrt()),
        D::Erlang { shape, scale } => Some(f64::from(*shape) * scale),
        D::InverseGamma { shape, scale } => {
            if *shape > 1.0 {
                Some(scale / (shape - 1.0))
            } else {
                None
            }
        }
        D::LogLogistic { shape, scale } => {
            if *shape > 1.0 {
                let r = std::f64::consts::PI / shape;
                Some(scale * r / r.sin())
            } else {
                None
            }
        }
        D::SkewNormal { location, scale, shape } => {
            Some(scale.mul_add(shape / (1.0 + shape * shape).sqrt() * std::f64::consts::FRAC_2_PI.sqrt(), *location))
        }
        D::JohnsonSu { gamma, delta, location, scale } => Some(scale.mul_add((0.5 / (delta * delta)).exp() * (-gamma / delta).sinh(), *location)),
        D::JohnsonSl { gamma, delta, location, scale } => Some(location + (scale.ln() - gamma / delta + 0.5 / (delta * delta)).exp()),
        D::Hypergeometric { population, successes, draws } => Some(f64::from(*draws) * (f64::from(*successes) / f64::from(*population))),
        D::Categorical { values, weights } => {
            let max = weights.iter().copied().fold(0.0_f64, f64::max);
            let total = special::finite_sum(weights.iter().map(|x| x / max))?;
            let scale = values.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
            if scale == 0.0 {
                Some(0.0)
            } else {
                Some(scale * special::finite_sum(values.iter().zip(weights).map(|(v, w)| (v / scale) * (w / max) / total))?)
            }
        }
        D::DoubleTriangular { low, mode, high, left_probability: p } => {
            Some(special::finite_sum([p * low / 3.0, 2.0 * (mode / 3.0), (1.0 - p) * high / 3.0])?)
        }
        D::FatigueLife { shape, scale } => Some(scale * (1.0 + 0.5 * shape * shape)),
        D::Frechet { shape, scale } => {
            if *shape > 1.0 {
                Some(exp_gamma(*scale, &[special::log_gamma(1.0 - 1.0 / shape)?])?)
            } else {
                None
            }
        }
        D::Burr12 { shape_c: c, shape_k: k, scale } => {
            if k * c > 1.0 {
                Some(exp_gamma(*scale, &[special::log_gamma(1.0 + 1.0 / c)?, special::log_gamma(k - 1.0 / c)?, -special::log_gamma(*k)?])?)
            } else {
                None
            }
        }
        D::Dagum { shape: a, power: p, scale } => {
            if *a > 1.0 {
                Some(exp_gamma(*scale, &[special::log_gamma(p + 1.0 / a)?, special::log_gamma(1.0 - 1.0 / a)?, -special::log_gamma(*p)?])?)
            } else {
                None
            }
        }
        D::Kumaraswamy { alpha: a, beta: b } => {
            Some(exp_gamma(*b, &[special::log_gamma(1.0 + 1.0 / a)?, special::log_gamma(*b)?, -special::log_gamma(1.0 + 1.0 / a + b)?])?)
        }
        D::BetaPrime { alpha, beta, scale } => {
            if *beta > 1.0 {
                Some(scale * alpha / (beta - 1.0))
            } else {
                None
            }
        }
        D::Reciprocal { low, high } => Some((high - low) / crate::reference_families::log_ratio(*high, *low)),
        D::Logarithmic { probability: p } => Some(-p / ((1.0 - p) * (-p).ln_1p())),
        D::ShiftScale { base, shift, scale } => {
            let result = nominal_summary(base)?;
            let value = scale.mul_add(result.value, *shift);
            if !value.is_finite() {
                return Err(SolverError::new("numerical_range", "Nominal affine value exceeds finite arithmetic."));
            }
            return Ok(NominalValue { value, statistic: result.statistic });
        }
        D::Mixture { components, weights } => {
            let mut terms = Vec::new();
            let max = weights.iter().copied().fold(0.0_f64, f64::max);
            let sum = special::finite_sum(weights.iter().map(|w| w / max))?;
            let mut analytic = true;
            for (component, w) in components.iter().zip(weights) {
                if *w == 0.0 {
                    continue;
                }
                let nominal = nominal_summary(component)?;
                if nominal.statistic != NominalStatistic::Mean {
                    analytic = false;
                    break;
                }
                terms.push(nominal.value * ((w / max) / sum));
            }
            if analytic { Some(special::finite_sum(terms)?) } else { None }
        }
        D::CompoundPoisson { rate, severity, .. } => {
            if *rate == 0.0 {
                Some(0.0)
            } else {
                let nominal = nominal_summary(severity)?;
                if nominal.statistic == NominalStatistic::Mean {
                    Some(rate * nominal.value)
                } else {
                    return Err(SolverError::new(
                        "nominal_unavailable",
                        "Compound severity has no implemented finite analytic mean or compound median.",
                    ));
                }
            }
        }
        _ => None,
    };
    let (value, statistic) =
        if let Some(mean) = mean { (mean, NominalStatistic::Mean) } else { (quantile(d, 0.5)?, NominalStatistic::MedianFallback) };
    if !value.is_finite() {
        return Err(SolverError::new("numerical_range", "Nominal value exceeds finite arithmetic."));
    }
    Ok(NominalValue { value, statistic })
}
