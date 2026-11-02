//! Explicit advanced execution without changing the legacy v2 sampling contract.
use crate::{MAX_DIMENSIONS, MAX_SAMPLE_WORK, OnlineMoments, checked_options, memory_error, reserve, size_add, size_mul};
use serde::{Deserialize, Serialize};
pub use solvercraft_math::copula::{DependenceSpec, Reflection};
use solvercraft_math::{PreparedDistribution, addressed_index, addressed_uniform};
use solvercraft_math::{
    copula::PreparedCopula,
    risk::{cvar_sorted, quantile_sorted},
    sobol::{RandomizedSobol, SOBOL_VERSION},
};
use solvercraft_types::{
    Distribution, ProgressEvent, Result, RunControl, SampleSummary, SamplingMethod, SimulationOptions, SimulationResult, SolverError, Termination,
};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SamplerSpec {
    #[default]
    Legacy,
    RandomizedSobol {
        #[serde(with = "solvercraft_types::decimal_seed")]
        replicate: u64,
    },
}
impl<'de> Deserialize<'de> for SamplerSpec {
    fn deserialize<T: serde::Deserializer<'de>>(deserializer: T) -> std::result::Result<Self, T::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Strict {
            Legacy {},
            RandomizedSobol {
                #[serde(with = "solvercraft_types::decimal_seed")]
                replicate: u64,
            },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Legacy {} => Self::Legacy,
            Strict::RandomizedSobol { replicate } => Self::RandomizedSobol { replicate },
        })
    }
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InvalidTrialPolicy {
    #[default]
    Fail,
    Skip {
        maximum_invalid: u64,
    },
}
impl<'de> Deserialize<'de> for InvalidTrialPolicy {
    fn deserialize<T: serde::Deserializer<'de>>(deserializer: T) -> std::result::Result<Self, T::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Strict {
            Fail {},
            Skip { maximum_invalid: u64 },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Fail {} => Self::Fail,
            Strict::Skip { maximum_invalid } => Self::Skip { maximum_invalid },
        })
    }
}
/// Hoeffding intervals are simultaneous over all configured checkpoints and outputs.
/// The caller must provide true deterministic output bounds; observed violations are errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoppingRule {
    pub minimum_trials: u64,
    pub check_every: u64,
    pub absolute_half_width: f64,
    pub confidence: f64,
    pub output_bounds: Vec<(f64, f64)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdvancedSimulationOptions {
    pub base: SimulationOptions,
    pub sampler: SamplerSpec,
    pub dependence: DependenceSpec,
    pub invalid_trials: InvalidTrialPolicy,
    pub stopping: Option<StoppingRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeanInterval {
    pub lower: f64,
    pub upper: f64,
    pub confidence: f64,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidTrial {
    pub trial: u64,
    pub code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSimulationResult {
    pub result: SimulationResult,
    pub attempted_trials: u64,
    pub invalid_trials: u64,
    pub first_invalid_trials: Vec<InvalidTrial>,
    pub mean_intervals: Vec<MeanInterval>,
    pub stopped_early: bool,
}

fn invalid(message: &str) -> SolverError {
    SolverError::new("advanced_simulation_options", message)
}
fn validate(options: &AdvancedSimulationOptions) -> Result<()> {
    if let InvalidTrialPolicy::Skip { maximum_invalid } = options.invalid_trials
        && maximum_invalid > options.base.trials
    {
        return Err(invalid("Invalid-trial limit exceeds attempted trials."));
    }
    if let Some(rule) = &options.stopping {
        if !matches!(options.sampler, SamplerSpec::Legacy)
            || options.base.method != SamplingMethod::MonteCarlo
            || !matches!(options.invalid_trials, InvalidTrialPolicy::Fail)
        {
            return Err(invalid("Bounded mean stopping requires ordinary MC with fail-on-invalid policy."));
        }
        if rule.minimum_trials < 2
            || rule.minimum_trials > options.base.trials
            || rule.check_every == 0
            || !rule.absolute_half_width.is_finite()
            || rule.absolute_half_width <= 0.0
            || !rule.confidence.is_finite()
            || rule.confidence <= 0.0
            || rule.confidence >= 1.0
            || rule.output_bounds.is_empty()
            || rule.output_bounds.len() > MAX_DIMENSIONS
            || rule.output_bounds.iter().any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b || !(b - a).is_finite())
        {
            return Err(invalid("Invalid bounded mean stopping rule."));
        }
    }
    Ok(())
}
fn bounded_intervals(stats: &[OnlineMoments], rule: &StoppingRule, total_trials: u64) -> Result<Vec<MeanInterval>> {
    let checkpoints = total_trials.div_ceil(rule.check_every).saturating_add(1) as f64;
    let log = (2.0 * checkpoints * stats.len() as f64 / (1.0 - rule.confidence)).ln();
    let mut intervals = Vec::new();
    reserve(&mut intervals, stats.len())?;
    for (stat, (low, high)) in stats.iter().zip(&rule.output_bounds) {
        let summary = stat.summary()?;
        let half = (high - low) * (log / (2.0 * summary.count as f64)).sqrt();
        // A rounded sample mean and endpoint arithmetic can otherwise turn
        // positive uncertainty into a point between adjacent support values.
        // Reserve one outward representable endpoint before clipping to the
        // caller's true deterministic bounds; exact zero-width bounds stay exact.
        let lower = summary.mean - half;
        let upper = summary.mean + half;
        intervals.push(MeanInterval {
            lower: if low < high { lower.next_down().max(*low) } else { *low },
            upper: if low < high { upper.next_up().min(*high) } else { *high },
            confidence: rule.confidence,
            method: "simultaneous_bounded_hoeffding".into(),
        });
    }
    Ok(intervals)
}
fn summarize(statistics: &[OnlineMoments], samples: &[Vec<f64>], retain: bool) -> Result<Vec<SampleSummary>> {
    let mut summaries = Vec::new();
    reserve(&mut summaries, statistics.len())?;
    for (column, stats) in statistics.iter().enumerate() {
        let mut summary = stats.summary()?;
        if retain {
            let mut ordered = Vec::new();
            reserve(&mut ordered, samples.len())?;
            for row in samples {
                ordered.push(*row.get(column).ok_or_else(|| invalid("Stored output shape changed."))?);
            }
            ordered.sort_unstable_by(f64::total_cmp);
            if !ordered.is_empty() {
                for p in [0.05, 0.25, 0.5, 0.75, 0.95] {
                    summary.quantiles.push((p, quantile_sorted(&ordered, p)?));
                }
                summary.value_at_risk_95 = Some(quantile_sorted(&ordered, 0.95)?);
                summary.conditional_value_at_risk_95 = Some(cvar_sorted(&ordered, 0.95)?);
            }
        }
        summaries.push(summary);
    }
    Ok(summaries)
}

/// A skip policy conditions the reported sample on evaluator success. Invalid counts are explicit.
/// The `trials` budget counts attempts, so skipping does not change later stream addresses.
pub fn simulate_advanced<F>(
    distributions: &[Distribution],
    options: &AdvancedSimulationOptions,
    control: &dyn RunControl,
    mut evaluator: F,
) -> Result<AdvancedSimulationResult>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>>,
{
    validate(options)?;
    let base = &options.base;
    let mut setup = checked_options(distributions, base)?;
    // Correlation and direction storage, scratch, and bounded diagnostic records.
    let matrix_bytes = if matches!(
        options.dependence,
        DependenceSpec::Gaussian { .. } | DependenceSpec::GaussianSpearman { .. } | DependenceSpec::StudentT { .. }
    ) {
        size_mul(distributions.len() as u64, distributions.len() as u64 * 16)?
    } else {
        0
    };
    setup = size_add(setup, size_add(size_add(matrix_bytes, size_mul(distributions.len() as u64, 1024)?)?, 65536)?)?;
    if setup > base.max_memory_bytes {
        return Err(memory_error());
    }
    let version = if matches!(options.sampler, SamplerSpec::RandomizedSobol { .. }) { SOBOL_VERSION } else { solvercraft_math::SAMPLING_VERSION };
    if control.is_cancelled() {
        return Ok(AdvancedSimulationResult {
            result: crate::empty_result(Termination::Cancelled),
            attempted_trials: 0,
            invalid_trials: 0,
            first_invalid_trials: Vec::new(),
            mean_intervals: Vec::new(),
            stopped_early: false,
        });
    }
    let copula = PreparedCopula::new(&options.dependence, distributions.len())?;
    let dimensions = copula.latent_dimensions();
    if size_mul(base.trials, dimensions as u64)? > MAX_SAMPLE_WORK {
        return Err(invalid("Advanced input sampling exceeds work budget."));
    }
    let sobol = if let SamplerSpec::RandomizedSobol { replicate } = options.sampler {
        if base.method != SamplingMethod::MonteCarlo {
            return Err(invalid("Sobol cannot be combined with Latin Hypercube."));
        }
        Some(RandomizedSobol::new(dimensions, base.seed, replicate)?)
    } else {
        None
    };
    let mut prepared = Vec::new();
    reserve(&mut prepared, distributions.len())?;
    for d in distributions {
        if control.is_cancelled() {
            return Ok(AdvancedSimulationResult {
                result: crate::empty_result(Termination::Cancelled),
                attempted_trials: 0,
                invalid_trials: 0,
                first_invalid_trials: Vec::new(),
                mean_intervals: Vec::new(),
                stopped_early: false,
            });
        }
        prepared.push(PreparedDistribution::new(d)?);
    }
    let count = usize::try_from(base.trials).map_err(|_| memory_error())?;
    let mut permutations = Vec::new();
    if base.method == SamplingMethod::LatinHypercube {
        setup = size_add(setup, size_mul(base.trials, dimensions.saturating_sub(distributions.len()) as u64 * 8)?)?;
        if setup > base.max_memory_bytes {
            return Err(memory_error());
        }
        reserve(&mut permutations, dimensions)?;
        for binding in 0..dimensions {
            let mut permutation = Vec::new();
            reserve(&mut permutation, count)?;
            for i in 0..count {
                permutation.push(i);
            }
            for i in (1..count).rev() {
                if i % 1024 == 0 && control.is_cancelled() {
                    return Ok(AdvancedSimulationResult {
                        result: crate::empty_result(Termination::Cancelled),
                        attempted_trials: 0,
                        invalid_trials: 0,
                        first_invalid_trials: Vec::new(),
                        mean_intervals: Vec::new(),
                        stopped_early: false,
                    });
                }
                let j = addressed_index(base.seed, binding as u64, i as u64, 0x1_0000_0000, i + 1)?;
                permutation.swap(i, j);
            }
            permutations.push(permutation);
        }
    }
    let mut latent = Vec::new();
    reserve(&mut latent, dimensions)?;
    latent.resize(dimensions, 0.5);
    let mut uniforms = Vec::new();
    reserve(&mut uniforms, distributions.len())?;
    uniforms.resize(distributions.len(), 0.5);
    let mut inputs = Vec::new();
    reserve(&mut inputs, distributions.len())?;
    inputs.resize(distributions.len(), 0.0);
    let mut statistics: Vec<OnlineMoments> = Vec::new();
    let mut samples = Vec::new();
    let mut attempted = 0;
    let mut invalid_count = 0;
    let mut invalid_records = Vec::new();
    let mut intervals = Vec::new();
    let mut stopped = false;
    let mut termination = Termination::Completed;
    for trial in 0..base.trials {
        if control.is_cancelled() {
            termination = Termination::Cancelled;
            break;
        }
        attempted += 1;
        let candidate = (|| -> Result<Vec<f64>> {
            for (binding, value) in latent.iter_mut().enumerate() {
                *value = if let Some(net) = &sobol {
                    net.coordinate(trial, binding)?
                } else {
                    let jitter = addressed_uniform(base.seed, binding as u64, trial, 0);
                    if base.method == SamplingMethod::LatinHypercube {
                        ((permutations[binding][trial as usize] as f64 + jitter) / base.trials as f64).clamp(f64::MIN_POSITIVE, 1.0_f64.next_down())
                    } else {
                        jitter
                    }
                };
            }
            copula.transform(&latent, &mut uniforms)?;
            for (binding, d) in prepared.iter().enumerate() {
                inputs[binding] =
                    if sobol.is_none() && base.method == SamplingMethod::MonteCarlo && matches!(options.dependence, DependenceSpec::Independent) {
                        d.sample(base.seed, binding as u64, trial)?
                    } else {
                        d.quantile(uniforms[binding])?
                    };
            }
            evaluator(&inputs)
        })();
        if control.is_cancelled() {
            attempted -= 1;
            termination = Termination::Cancelled;
            break;
        }
        let candidate = candidate.and_then(|outputs| {
            if outputs.iter().any(|v| !v.is_finite()) {
                return Err(SolverError::new("non_finite_output", "Evaluator produced a non-finite output."));
            }
            Ok(outputs)
        });
        let outputs = match candidate {
            Ok(v) => v,
            Err(e) => match options.invalid_trials {
                InvalidTrialPolicy::Fail => return Err(e),
                InvalidTrialPolicy::Skip { maximum_invalid } => {
                    invalid_count += 1;
                    if invalid_count > maximum_invalid {
                        return Err(SolverError::new(
                            "invalid_trial_limit",
                            format!("Exceeded {maximum_invalid} invalid attempts at trial {trial}: {}", e.code.chars().take(128).collect::<String>()),
                        ));
                    }
                    if invalid_records.len() < 100 {
                        invalid_records.push(InvalidTrial { trial, code: e.code.chars().take(128).collect() });
                    }
                    continue;
                }
            },
        };
        if outputs.is_empty() || outputs.len() > MAX_DIMENSIONS {
            return Err(invalid("Evaluator must return 1..4096 outputs."));
        }
        if statistics.is_empty() {
            if size_mul(base.trials, outputs.len() as u64)? > MAX_SAMPLE_WORK {
                return Err(invalid("Advanced output sampling exceeds work budget."));
            }
            let mut memory = size_add(
                setup,
                size_mul(outputs.len() as u64, (std::mem::size_of::<OnlineMoments>() + std::mem::size_of::<SampleSummary>() + 128) as u64)?,
            )?;
            if base.retain_samples {
                memory = size_add(memory, size_add(size_mul(base.trials, 24 + outputs.len() as u64 * 8)?, size_mul(base.trials, 8)?)?)?;
            }
            if memory > base.max_memory_bytes {
                return Err(memory_error());
            }
            reserve(&mut statistics, outputs.len())?;
            statistics.resize_with(outputs.len(), OnlineMoments::default);
            if base.retain_samples {
                reserve(&mut samples, count)?;
            }
            if options.stopping.as_ref().is_some_and(|r| r.output_bounds.len() != outputs.len()) {
                return Err(invalid("Stopping bounds must match output count."));
            }
        } else if outputs.len() != statistics.len() {
            return Err(invalid("Evaluator output count changed."));
        }
        if let Some(rule) = &options.stopping
            && outputs.iter().zip(&rule.output_bounds).any(|(x, (lo, hi))| x < lo || x > hi)
        {
            return Err(SolverError::new("output_bound_violation", "Evaluator violated the declared stopping bounds."));
        }
        for (stats, value) in statistics.iter_mut().zip(&outputs) {
            stats.push(*value)?;
        }
        if base.retain_samples {
            samples.push(outputs);
        }
        let completed = attempted - invalid_count;
        if completed == 1 || completed % 256 == 0 {
            control.progress(&ProgressEvent {
                stage: "advanced_simulation".into(),
                iterations: attempted,
                evaluations: attempted,
                nodes: 0,
                best_objective: None,
            });
        }
        if let Some(rule) = &options.stopping
            && completed >= rule.minimum_trials
            && (completed.is_multiple_of(rule.check_every) || trial + 1 == base.trials)
        {
            intervals = bounded_intervals(&statistics, rule, base.trials)?;
            if intervals.iter().all(|i| (i.upper - i.lower) * 0.5 <= rule.absolute_half_width) {
                stopped = trial + 1 < base.trials;
                break;
            }
        }
    }
    if !statistics.is_empty()
        && let Some(rule) = &options.stopping
    {
        let completed = attempted - invalid_count;
        if completed >= rule.minimum_trials && (completed.is_multiple_of(rule.check_every) || completed == base.trials) {
            intervals = bounded_intervals(&statistics, rule, base.trials)?;
        } else {
            intervals.clear();
        }
    }
    let completed = attempted - invalid_count;
    let mut warnings = Vec::new();
    if invalid_count > 0 {
        warnings.push("Results are conditioned on successful trials; invalid attempts were skipped explicitly and can bias the unconditional model distribution.".into());
    }
    if matches!(options.dependence, DependenceSpec::GaussianSpearman { .. }) {
        warnings.push("The requested Spearman matrix describes continuous copula ranks. Discrete or tied marginal values can have different observed rank correlations.".into());
    } else if matches!(options.dependence, DependenceSpec::Gaussian { .. } | DependenceSpec::StudentT { .. }) {
        warnings.push("The correlation matrix describes latent elliptical variables; nonlinear marginal transforms do not generally preserve output Pearson correlation.".into());
    }
    if !base.retain_samples {
        warnings.push("Streaming mode: exact quantiles and loss-tail risk were not computed.".into());
    }
    if sobol.is_some() {
        warnings.push("Sobol uses the64-dimensional licensed Joe-Kuo direction subset with independent digital shifts. Digital shifts are not Owen nested scrambles; no pointwise IID confidence interval is claimed.".into());
    }
    let summaries = summarize(&statistics, &samples, base.retain_samples)?;
    Ok(AdvancedSimulationResult {
        result: SimulationResult {
            termination,
            completed_trials: completed,
            summaries,
            samples: if base.retain_samples { Some(samples) } else { None },
            sampling_version: format!("{version}/advanced-v1/{}", solvercraft_math::catalog::CATALOG_VERSION),
            warnings,
        },
        attempted_trials: attempted,
        invalid_trials: invalid_count,
        first_invalid_trials: invalid_records,
        mean_intervals: intervals,
        stopped_early: stopped,
    })
}

#[cfg(test)]
mod strict_input_tests {
    use super::*;
    #[test]
    fn unit_declarations_do_not_discard_unknown_fields() {
        let extra = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new([("kind", "legacy"), ("replicate", "17")].into_iter());
        assert!(SamplerSpec::deserialize(extra).is_err());
        let extra = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new([("kind", "fail"), ("maximum_invalid", "17")].into_iter());
        assert!(InvalidTrialPolicy::deserialize(extra).is_err());
        let valid = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(
            [("kind", "randomized_sobol"), ("replicate", "18446744073709551615")].into_iter(),
        );
        assert!(matches!(SamplerSpec::deserialize(valid), Ok(SamplerSpec::RandomizedSobol { replicate: u64::MAX })));
    }
}
