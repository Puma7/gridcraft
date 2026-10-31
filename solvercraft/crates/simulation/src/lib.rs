//! Budgeted deterministic Monte Carlo / Latin Hypercube execution and online summaries.
//! Exact empirical quantiles and loss-tail risk require retain_samples=true.
#![forbid(unsafe_code)]
use solvercraft_math::distributions::prepared_storage_bytes;
use solvercraft_math::risk::{cvar_sorted, quantile_sorted};
use solvercraft_math::{PreparedDistribution, SAMPLING_VERSION, addressed_index, addressed_uniform};
use solvercraft_types::{
    Distribution, ProgressEvent, Result, RunControl, SampleSummary, SamplingMethod, SimulationOptions, SimulationResult, SolverError, Termination,
};
mod statistics;
pub use statistics::{OnlineMoments, fit_exponential, fit_normal, fit_uniform};

const MAX_TRIALS: u64 = 10_000_000;
const MAX_DIMENSIONS: usize = 4096;
const MAX_SAMPLE_WORK: u64 = 100_000_000;
fn memory_error() -> SolverError {
    SolverError::new("memory_limit", "Simulation exceeds its configured memory budget.")
}
fn size_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(memory_error)
}
fn size_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(memory_error)
}
fn reserve<T>(items: &mut Vec<T>, count: usize) -> Result<()> {
    items.try_reserve_exact(count).map_err(|_| SolverError::new("allocation", "Cannot allocate simulation storage."))
}
fn empty_result(termination: Termination) -> SimulationResult {
    SimulationResult {
        termination,
        completed_trials: 0,
        summaries: Vec::new(),
        samples: None,
        sampling_version: SAMPLING_VERSION.into(),
        warnings: Vec::new(),
    }
}
fn checked_options(distributions: &[Distribution], options: &SimulationOptions) -> Result<u64> {
    if options.trials == 0 || options.trials > MAX_TRIALS {
        return Err(SolverError::new("trial_limit", "Trial count must be between 1 and 10,000,000."));
    }
    if distributions.len() > MAX_DIMENSIONS {
        return Err(SolverError::new("dimension_limit", "At most 4096 inputs are supported."));
    }
    if options.max_memory_bytes == 0 {
        return Err(memory_error());
    }
    if size_mul(options.trials, distributions.len() as u64)? > MAX_SAMPLE_WORK {
        return Err(SolverError::new("work_limit", "Simulation exceeds 100,000,000 input samples."));
    }
    let mut bytes = size_mul(distributions.len() as u64, 32)?;
    for distribution in distributions {
        bytes = size_add(bytes, prepared_storage_bytes(distribution)?)?;
    }
    if options.method == SamplingMethod::LatinHypercube {
        bytes = size_add(bytes, size_mul(size_mul(options.trials, distributions.len() as u64)?, std::mem::size_of::<usize>() as u64)?)?;
    }
    if bytes > options.max_memory_bytes {
        return Err(memory_error());
    }
    Ok(bytes)
}

/// Runs the evaluator once per trial, keeping identical addressed input samples regardless of
/// batching. Repeated input reads within an evaluator call see the same slice value.
/// The evaluator must perform its own allocation / computation budgeting.
pub fn simulate<F>(
    distributions: &[Distribution],
    options: &SimulationOptions,
    control: &dyn RunControl,
    mut evaluator: F,
) -> Result<SimulationResult>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>>,
{
    let setup_bytes = checked_options(distributions, options)?;
    if control.is_cancelled() {
        return Ok(empty_result(Termination::Cancelled));
    }
    let count = usize::try_from(options.trials).map_err(|_| memory_error())?;
    let mut prepared = Vec::new();
    reserve(&mut prepared, distributions.len())?;
    for distribution in distributions {
        if control.is_cancelled() {
            return Ok(empty_result(Termination::Cancelled));
        }
        prepared.push(PreparedDistribution::new(distribution)?);
    }
    let mut permutations: Vec<Vec<usize>> = Vec::new();
    if options.method == SamplingMethod::LatinHypercube {
        reserve(&mut permutations, distributions.len())?;
        for binding in 0..distributions.len() {
            let mut permutation = Vec::new();
            reserve(&mut permutation, count)?;
            for i in 0..count {
                permutation.push(i);
            }
            for i in (1..count).rev() {
                if i % 1024 == 0 && control.is_cancelled() {
                    return Ok(empty_result(Termination::Cancelled));
                }
                let j = addressed_index(options.seed, binding as u64, i as u64, 0x1_0000_0000, i + 1)?;
                permutation.swap(i, j);
            }
            permutations.push(permutation);
        }
    }
    let mut inputs = Vec::new();
    reserve(&mut inputs, distributions.len())?;
    inputs.resize(distributions.len(), 0.0);
    let mut statistics: Vec<OnlineMoments> = Vec::new();
    let mut samples: Vec<Vec<f64>> = Vec::new();
    let mut completed = 0_u64;
    let mut termination = Termination::Completed;
    for trial in 0..options.trials {
        if control.is_cancelled() {
            termination = Termination::Cancelled;
            break;
        }
        for (binding, distribution) in prepared.iter().enumerate() {
            let jitter = addressed_uniform(options.seed, binding as u64, trial, 0);
            let p = if options.method == SamplingMethod::LatinHypercube {
                let stratum = permutations
                    .get(binding)
                    .and_then(|v| v.get(trial as usize))
                    .copied()
                    .ok_or_else(|| SolverError::new("sampling_index", "Missing Latin Hypercube stratum."))?;
                let x = (stratum as f64 + jitter) / options.trials as f64;
                // Rounding at the top stratum can produce exactly 1 on large trial counts.
                x.clamp(f64::MIN_POSITIVE, f64::from_bits(1.0_f64.to_bits() - 1))
            } else {
                jitter
            };
            let value = distribution.quantile(p)?;
            let destination = inputs.get_mut(binding).ok_or_else(|| SolverError::new("sampling_index", "Missing input binding."))?;
            *destination = value;
        }
        let outputs = evaluator(&inputs)?;
        if outputs.is_empty() || outputs.len() > MAX_DIMENSIONS {
            return Err(SolverError::new("output_dimension", "Evaluator must return between 1 and 4096 outputs."));
        }
        if outputs.iter().any(|x| !x.is_finite()) {
            return Err(SolverError::new("non_finite_output", format!("Evaluator returned a non-finite value at trial {trial}.")));
        }
        if trial == 0 {
            let mut bytes = size_add(setup_bytes, size_mul(outputs.len() as u64, std::mem::size_of::<OnlineMoments>() as u64)?)?;
            // Include current output row and final summaries, even in streaming mode.
            bytes = size_add(bytes, size_mul(outputs.len() as u64, (std::mem::size_of::<SampleSummary>() + std::mem::size_of::<f64>()) as u64)?)?;
            if options.retain_samples {
                bytes = size_add(bytes, size_mul(outputs.len() as u64, 5 * 16)?)?;
                let row_size = size_add(std::mem::size_of::<Vec<f64>>() as u64, size_mul(outputs.len() as u64, 8)?)?;
                bytes = size_add(bytes, size_mul(options.trials, row_size)?)?;
                // One temporary ordered column for exact empirical tail/quantiles.
                bytes = size_add(bytes, size_mul(options.trials, 8)?)?;
            }
            if bytes > options.max_memory_bytes {
                return Err(memory_error());
            }
            reserve(&mut statistics, outputs.len())?;
            statistics.resize_with(outputs.len(), OnlineMoments::default);
            if options.retain_samples {
                reserve(&mut samples, count)?;
            }
        } else if outputs.len() != statistics.len() {
            return Err(SolverError::new("output_dimension", format!("Evaluator output count changed at trial {trial}.")));
        }
        for (stats, value) in statistics.iter_mut().zip(&outputs) {
            stats.push(*value)?;
        }
        if options.retain_samples {
            samples.push(outputs);
        }
        completed += 1;
        if completed == 1 || completed % 256 == 0 || completed == options.trials {
            control.progress(&ProgressEvent {
                stage: "simulation".into(),
                iterations: completed,
                evaluations: completed,
                nodes: 0,
                best_objective: None,
            });
        }
    }
    let mut summaries = Vec::new();
    reserve(&mut summaries, statistics.len())?;
    for (column, stats) in statistics.iter().enumerate() {
        let mut summary = stats.summary()?;
        if options.retain_samples {
            let mut ordered = Vec::new();
            reserve(&mut ordered, samples.len())?;
            for row in &samples {
                let value = row.get(column).copied().ok_or_else(|| SolverError::new("output_dimension", "Stored output shape changed."))?;
                ordered.push(value);
            }
            ordered.sort_by(f64::total_cmp);
            if !ordered.is_empty() {
                reserve(&mut summary.quantiles, 5)?;
                for p in [0.05, 0.25, 0.5, 0.75, 0.95] {
                    summary.quantiles.push((p, quantile_sorted(&ordered, p)?));
                }
                summary.value_at_risk_95 = Some(quantile_sorted(&ordered, 0.95)?);
                summary.conditional_value_at_risk_95 = Some(cvar_sorted(&ordered, 0.95)?);
            }
        }
        summaries.push(summary);
    }
    let mut warnings = Vec::new();
    if !options.retain_samples && completed > 0 {
        warnings
            .push("Streaming mode: exact empirical quantiles and tail-risk measures were not computed. Enable retain_samples to obtain them.".into());
    }
    Ok(SimulationResult {
        termination,
        completed_trials: completed,
        summaries,
        samples: if options.retain_samples { Some(samples) } else { None },
        sampling_version: SAMPLING_VERSION.into(),
        warnings,
    })
}

#[cfg(test)]
mod tests;
