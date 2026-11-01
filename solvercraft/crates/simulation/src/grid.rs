//! Parameter values stay constant within a run. Random-number coupling is explicit.
use serde::{Deserialize, Serialize};
use solvercraft_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterAxis {
    pub name: String,
    pub values: Vec<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridStreams {
    #[default]
    CommonRandomNumbers,
    DistinctSeeds,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridOptions {
    pub simulation: SimulationOptions,
    pub streams: GridStreams,
    pub max_runs: u64,
    pub max_total_trials: u64,
    pub max_memory_bytes: u64,
}
impl Default for GridOptions {
    fn default() -> Self {
        Self {
            simulation: SimulationOptions::default(),
            streams: GridStreams::CommonRandomNumbers,
            max_runs: 4096,
            max_total_trials: 100_000_000,
            max_memory_bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRun {
    pub index: u64,
    pub coordinates: Vec<usize>,
    pub parameters: Vec<f64>,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub effective_seed: u64,
    pub result: SimulationResult,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterGridResult {
    pub termination: Termination,
    pub requested_runs: u64,
    pub axes: Vec<String>,
    pub streams: GridStreams,
    pub runs: Vec<ParameterRun>,
}
fn bad(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
struct Control<'a> {
    parent: &'a dyn RunControl,
    prior_trials: u64,
    run: u64,
}
impl RunControl for Control<'_> {
    fn is_cancelled(&self) -> bool {
        self.parent.is_cancelled()
    }
    fn progress(&self, event: &ProgressEvent) {
        self.parent.progress(&ProgressEvent {
            stage: format!("parameter_grid/run/{}/{}", self.run, event.stage),
            iterations: event.iterations,
            evaluations: self.prior_trials.saturating_add(event.evaluations),
            nodes: self.run,
            best_objective: event.best_objective,
        });
    }
}
fn result_bytes(result: &SimulationResult) -> Result<u64> {
    let mut bytes = 2048u64
        .checked_add((result.summaries.len() as u64).checked_mul(512).ok_or_else(|| bad("memory_limit", "Grid summary estimate overflow"))?)
        .ok_or_else(|| bad("memory_limit", "Grid result estimate overflow"))?;
    if let Some(samples) = &result.samples {
        for row in samples {
            bytes = bytes
                .checked_add(24)
                .and_then(|n| n.checked_add((row.len() as u64).checked_mul(8)?))
                .ok_or_else(|| bad("memory_limit", "Grid retention estimate overflow"))?;
        }
    }
    for warning in &result.warnings {
        bytes = bytes.checked_add(warning.len() as u64).ok_or_else(|| bad("memory_limit", "Grid warnings estimate overflow"))?;
    }
    Ok(bytes)
}
pub fn simulate_grid<F>(
    axes: &[ParameterAxis],
    distributions: &[Distribution],
    options: &GridOptions,
    control: &dyn RunControl,
    mut evaluator: F,
) -> Result<ParameterGridResult>
where
    F: FnMut(&[f64], &[f64]) -> Result<Vec<f64>>,
{
    if axes.is_empty()
        || axes.len() > 16
        || options.max_runs == 0
        || options.max_runs > 4096
        || options.max_total_trials > 100_000_000
        || options.max_memory_bytes < 4096
    {
        return Err(bad("invalid_grid", "Expected 1..16 axes and bounded nonzero run/memory limits"));
    }
    let mut total = 1u64;
    let mut stored = 0u64;
    let mut names = std::collections::BTreeSet::new();
    for axis in axes {
        if axis.name.is_empty()
            || axis.name.len() > 256
            || !names.insert(axis.name.as_str())
            || axis.values.is_empty()
            || axis.values.len() > 4096
            || axis.values.iter().any(|v| !v.is_finite())
        {
            return Err(bad("invalid_grid", "Axis names must be unique and values finite, nonempty and bounded"));
        }
        total = total
            .checked_mul(axis.values.len() as u64)
            .filter(|n| *n <= options.max_runs)
            .ok_or_else(|| bad("grid_limit", "Parameter product exceeds max_runs"))?;
        stored = stored.checked_add(axis.name.len() as u64).ok_or_else(|| bad("memory_limit", "Grid metadata overflow"))?;
    }
    if total.checked_mul(options.simulation.trials).is_none_or(|n| n > options.max_total_trials) {
        return Err(bad("grid_limit", "Parameter trials exceed the cumulative budget"));
    }
    let metadata = total
        .checked_mul(256 + (axes.len() as u64) * 16)
        .and_then(|n| n.checked_add(stored))
        .ok_or_else(|| bad("memory_limit", "Grid metadata estimate overflow"))?;
    if metadata > options.max_memory_bytes {
        return Err(bad("memory_limit", "Grid run metadata exceeds its memory budget"));
    }
    stored = metadata;
    let mut result = ParameterGridResult {
        termination: Termination::Completed,
        requested_runs: total,
        axes: axes.iter().map(|a| a.name.clone()).collect(),
        streams: options.streams,
        runs: vec![],
    };
    result.runs.try_reserve(total as usize).map_err(|_| bad("memory_limit", "Cannot reserve grid result entries"))?;
    let mut prior_trials = 0u64;
    for index in 0..total {
        if control.is_cancelled() {
            result.termination = Termination::Cancelled;
            break;
        }
        let mut remainder = index as usize;
        let mut coordinates = vec![0; axes.len()];
        let mut parameters = vec![0.0; axes.len()];
        for dimension in (0..axes.len()).rev() {
            let axis = axes.get(dimension).ok_or_else(|| bad("invalid_grid", "Missing axis"))?;
            let coordinate = remainder % axis.values.len();
            remainder /= axis.values.len();
            coordinates[dimension] = coordinate;
            parameters[dimension] = *axis.values.get(coordinate).ok_or_else(|| bad("invalid_grid", "Missing axis value"))?;
        }
        let mut simulation = options.simulation.clone();
        simulation.max_memory_bytes = simulation
            .max_memory_bytes
            .min(options.max_memory_bytes.checked_sub(stored).ok_or_else(|| bad("memory_limit", "Grid storage exhausted"))?);
        simulation.seed = match options.streams {
            GridStreams::CommonRandomNumbers => simulation.seed,
            GridStreams::DistinctSeeds => simulation.seed.wrapping_add(0x9e3779b97f4a7c15u64.wrapping_mul(index + 1)),
        };
        let run_control = Control { parent: control, prior_trials, run: index };
        let simulation_result = crate::simulate(distributions, &simulation, &run_control, |inputs| evaluator(&parameters, inputs))?;
        let next = stored.checked_add(result_bytes(&simulation_result)?).ok_or_else(|| bad("memory_limit", "Grid result estimate overflow"))?;
        if next > options.max_memory_bytes {
            return Err(bad("memory_limit", "Grid result storage exceeds the cumulative memory budget"));
        }
        stored = next;
        prior_trials =
            prior_trials.checked_add(simulation_result.completed_trials).ok_or_else(|| bad("grid_limit", "Completed-trial counter overflow"))?;
        let termination = simulation_result.termination;
        result.runs.push(ParameterRun { index, coordinates, parameters, effective_seed: simulation.seed, result: simulation_result });
        if termination != Termination::Completed {
            result.termination = termination;
            break;
        }
    }
    Ok(result)
}
