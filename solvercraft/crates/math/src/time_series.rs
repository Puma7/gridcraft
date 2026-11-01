//! Reproducible Gaussian-innovation ARMA and conditional-volatility paths.
//! Startup states are explicit; no stationary warmup or worksheet context is implied.
use serde::{Deserialize, Serialize};
use solvercraft_types::{ProgressEvent, Result, RunControl, SolverError};
pub const STREAM_VERSION: &str = "solvercraft-time-series-normal-v1/addressed-mix64-v2";
const STREAM_BASE: u64 = 0x5453_0000_0000_0000;
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesAddress {
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub binding: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub trial: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Process {
    Ar1 {
        innovation_std: f64,
        coefficient: f64,
        initial_value: f64,
    },
    Ar2 {
        innovation_std: f64,
        first: f64,
        second: f64,
        initial_latest: f64,
        initial_previous: f64,
    },
    Ma1 {
        innovation_std: f64,
        coefficient: f64,
        initial_error: f64,
    },
    Ma2 {
        innovation_std: f64,
        first: f64,
        second: f64,
        initial_latest_error: f64,
        initial_previous_error: f64,
    },
    Arma11 {
        innovation_std: f64,
        ar: f64,
        ma: f64,
        initial_value: f64,
        initial_error: f64,
    },
    Arch1 {
        variance_intercept: f64,
        error_coefficient: f64,
        initial_value: f64,
    },
    Garch11 {
        variance_intercept: f64,
        error_coefficient: f64,
        variance_coefficient: f64,
        initial_value: f64,
        initial_std: f64,
    },
    Egarch11 {
        log_variance_intercept: f64,
        error_coefficient: f64,
        log_variance_coefficient: f64,
        theta: f64,
        gamma: f64,
        initial_value: f64,
        initial_std: f64,
    },
    Aparch11 {
        power_intercept: f64,
        error_coefficient: f64,
        persistence: f64,
        power: f64,
        asymmetry: f64,
        initial_value: f64,
        initial_std: f64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Integration {
    None {},
    First { initial_level: f64 },
    Second { initial_level: f64, initial_difference: f64 },
}
impl Default for Integration {
    fn default() -> Self {
        Self::None {}
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Seasonality {
    None {},
    Additive { terms: Vec<f64>, starting_phase: usize },
    First { previous_season: Vec<f64> },
    Second { previous_season: Vec<f64>, older_season: Vec<f64> },
}
impl Default for Seasonality {
    fn default() -> Self {
        Self::None {}
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transformation {
    Identity { shift: f64 },
    Exponential { shift: f64 },
    Square { shift: f64 },
}
impl Default for Transformation {
    fn default() -> Self {
        Self::Identity { shift: 0.0 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Postprocess {
    pub integration: Integration,
    pub seasonality: Seasonality,
    pub transformation: Transformation,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSeriesSpec {
    pub mean: f64,
    pub process: Process,
    #[serde(default)]
    pub postprocess: Postprocess,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimeSeriesLimits {
    pub max_length: usize,
    pub max_steps: u64,
    pub max_memory_bytes: u64,
    pub require_stationary: bool,
}
impl Default for TimeSeriesLimits {
    fn default() -> Self {
        Self { max_length: 1_000_000, max_steps: 1_000_000, max_memory_bytes: 32 * 1024 * 1024, require_stationary: true }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSeriesPath {
    pub values: Vec<f64>,
    pub conditional_std: Vec<f64>,
    pub standard_normal_shocks: Vec<f64>,
    pub steps: u64,
    pub sampling_version: String,
    pub startup: String,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn finite(v: f64) -> Result<f64> {
    if v.is_finite() { Ok(v) } else { Err(error("numerical_range", "Time-series arithmetic is not representable.")) }
}
fn all(values: &[f64]) -> Result<()> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(error("time_series_domain", "Time-series parameters and startup states must be finite."))
    }
}
fn nonnegative(value: f64) -> Result<()> {
    if value >= 0.0 { Ok(()) } else { Err(error("time_series_domain", "Innovation scale and variance coefficients must be nonnegative.")) }
}
fn positive(value: f64) -> Result<()> {
    if value > 0.0 { Ok(()) } else { Err(error("time_series_domain", "Variance/power intercept and initial conditional scale must be positive.")) }
}
fn stationary(valid: bool, required: bool) -> Result<()> {
    if valid || !required {
        Ok(())
    } else {
        Err(error("nonstationary_time_series", "Parameters fail the configured sufficient stationarity/moment condition."))
    }
}
fn seasonal_period(season: &Seasonality) -> usize {
    match season {
        Seasonality::None {} => 0,
        Seasonality::Additive { terms, .. } => terms.len(),
        Seasonality::First { previous_season } | Seasonality::Second { previous_season, .. } => previous_season.len(),
    }
}
/// Validate recurrence conventions, finite startup state and output/step memory
/// budgets before generating or allocating a path.
pub fn validate(spec: &TimeSeriesSpec, length: usize, limits: &TimeSeriesLimits) -> Result<()> {
    if limits.max_length > 1_000_000 {
        return Err(error("time_series_limit", "Configured path cap may not exceed1million steps."));
    }
    if length > limits.max_length || u64::try_from(length).map_or(true, |n| n > limits.max_steps) {
        return Err(error("time_series_limit", "Path exceeds its length or recurrence-step budget."));
    }
    all(&[spec.mean])?;
    let require = limits.require_stationary;
    match &spec.process {
        Process::Ar1 { innovation_std, coefficient, initial_value } => {
            all(&[*innovation_std, *coefficient, *initial_value])?;
            nonnegative(*innovation_std)?;
            stationary(coefficient.abs() < 1.0, require)?;
        }
        Process::Ar2 { innovation_std, first, second, initial_latest, initial_previous } => {
            all(&[*innovation_std, *first, *second, *initial_latest, *initial_previous])?;
            nonnegative(*innovation_std)?;
            stationary(first + second < 1.0 && second - first < 1.0 && second.abs() < 1.0, require)?;
        }
        Process::Ma1 { innovation_std, coefficient, initial_error } => {
            all(&[*innovation_std, *coefficient, *initial_error])?;
            nonnegative(*innovation_std)?;
        }
        Process::Ma2 { innovation_std, first, second, initial_latest_error, initial_previous_error } => {
            all(&[*innovation_std, *first, *second, *initial_latest_error, *initial_previous_error])?;
            nonnegative(*innovation_std)?;
        }
        Process::Arma11 { innovation_std, ar, ma, initial_value, initial_error } => {
            all(&[*innovation_std, *ar, *ma, *initial_value, *initial_error])?;
            nonnegative(*innovation_std)?;
            stationary(ar.abs() < 1.0, require)?;
        }
        Process::Arch1 { variance_intercept, error_coefficient, initial_value } => {
            all(&[*variance_intercept, *error_coefficient, *initial_value])?;
            positive(*variance_intercept)?;
            nonnegative(*error_coefficient)?;
            stationary(*error_coefficient < 1.0, require)?;
        }
        Process::Garch11 { variance_intercept, error_coefficient, variance_coefficient, initial_value, initial_std } => {
            all(&[*variance_intercept, *error_coefficient, *variance_coefficient, *initial_value, *initial_std])?;
            positive(*variance_intercept)?;
            positive(*initial_std)?;
            nonnegative(*error_coefficient)?;
            nonnegative(*variance_coefficient)?;
            stationary(error_coefficient + variance_coefficient < 1.0, require)?;
        }
        Process::Egarch11 { log_variance_intercept, error_coefficient, log_variance_coefficient, theta, gamma, initial_value, initial_std } => {
            all(&[*log_variance_intercept, *error_coefficient, *log_variance_coefficient, *theta, *gamma, *initial_value, *initial_std])?;
            positive(*initial_std)?;
            stationary(log_variance_coefficient.abs() < 1.0, require)?;
        }
        Process::Aparch11 { power_intercept, error_coefficient, persistence, power, asymmetry, initial_value, initial_std } => {
            all(&[*power_intercept, *error_coefficient, *persistence, *power, *asymmetry, *initial_value, *initial_std])?;
            positive(*power_intercept)?;
            positive(*initial_std)?;
            nonnegative(*error_coefficient)?;
            nonnegative(*persistence)?;
            if !(0.1..=8.0).contains(power) || asymmetry.abs() >= 1.0 {
                return Err(error("time_series_domain", "APARCH requires power[0.1,8] and asymmetry strictly in(-1,1)."));
            }
            let normal_moment =
                (0.5 * power * 2.0_f64.ln() + crate::special::log_gamma(0.5 * (power + 1.0))? - 0.5 * std::f64::consts::PI.ln()).exp();
            let factor = 0.5 * ((1.0 - asymmetry).powf(*power) + (1.0 + asymmetry).powf(*power)) * normal_moment;
            stationary(error_coefficient * factor + persistence < 1.0, require)?;
        }
    }
    match &spec.postprocess.integration {
        Integration::None {} => {}
        Integration::First { initial_level } => all(&[*initial_level])?,
        Integration::Second { initial_level, initial_difference } => all(&[*initial_level, *initial_difference])?,
    }
    let period = seasonal_period(&spec.postprocess.seasonality);
    if period > 100_000 {
        return Err(error("time_series_limit", "Seasonal period exceeds100000."));
    }
    match &spec.postprocess.seasonality {
        Seasonality::None {} => {}
        Seasonality::Additive { terms, starting_phase } => {
            if terms.is_empty() || *starting_phase >= terms.len() {
                return Err(error("time_series_domain", "Additive seasonality needs terms and a valid zero-based phase."));
            }
            all(terms)?;
        }
        Seasonality::First { previous_season } => {
            if previous_season.is_empty() {
                return Err(error("time_series_domain", "Seasonal integration requires a nonempty initial season."));
            }
            all(previous_season)?;
        }
        Seasonality::Second { previous_season, older_season } => {
            if previous_season.is_empty() || previous_season.len() != older_season.len() {
                return Err(error("time_series_domain", "Second seasonal integration requires two matching nonempty initial seasons."));
            }
            all(previous_season)?;
            all(older_season)?;
        }
    }
    match &spec.postprocess.transformation {
        Transformation::Identity { shift } | Transformation::Exponential { shift } | Transformation::Square { shift } => all(&[*shift])?,
    }
    let bytes = length
        .checked_mul(24)
        .and_then(|b| period.checked_mul(16).and_then(|p| b.checked_add(p)))
        .and_then(|b| b.checked_add(1024))
        .ok_or_else(|| error("memory_limit", "Path memory size overflow."))?;
    if u64::try_from(bytes).map_or(true, |b| b > limits.max_memory_bytes) {
        return Err(error("memory_limit", "Path exceeds its memory budget."));
    }
    Ok(())
}
struct State {
    center: f64,
    previous_center: f64,
    error: f64,
    previous_error: f64,
    std: f64,
    normal: f64,
    log_variance: f64,
}
fn startup(spec: &TimeSeriesSpec) -> Result<State> {
    let mut state = State { center: 0.0, previous_center: 0.0, error: 0.0, previous_error: 0.0, std: 0.0, normal: 0.0, log_variance: 0.0 };
    match &spec.process {
        Process::Ar1 { initial_value, .. } | Process::Arch1 { initial_value, .. } => {
            state.center = finite(initial_value - spec.mean)?;
            state.error = state.center;
        }
        Process::Ar2 { initial_latest, initial_previous, .. } => {
            state.center = finite(initial_latest - spec.mean)?;
            state.previous_center = finite(initial_previous - spec.mean)?;
        }
        Process::Ma1 { initial_error, .. } => state.error = *initial_error,
        Process::Ma2 { initial_latest_error, initial_previous_error, .. } => {
            state.error = *initial_latest_error;
            state.previous_error = *initial_previous_error;
        }
        Process::Arma11 { initial_value, initial_error, .. } => {
            state.center = finite(initial_value - spec.mean)?;
            state.error = *initial_error;
        }
        Process::Garch11 { initial_value, initial_std, .. }
        | Process::Egarch11 { initial_value, initial_std, .. }
        | Process::Aparch11 { initial_value, initial_std, .. } => {
            state.center = finite(initial_value - spec.mean)?;
            state.error = state.center;
            state.std = *initial_std;
            state.normal = finite(state.error / state.std)?;
            state.log_variance = 2.0 * state.std.ln();
        }
    }
    Ok(state)
}
fn log_sum(values: &[f64]) -> Result<f64> {
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    finite(max + values.iter().map(|v| (v - max).exp()).sum::<f64>().ln())
}
fn step(spec: &TimeSeriesSpec, state: &mut State, z: f64) -> Result<(f64, f64)> {
    let std = match &spec.process {
        Process::Ar1 { innovation_std, .. }
        | Process::Ar2 { innovation_std, .. }
        | Process::Ma1 { innovation_std, .. }
        | Process::Ma2 { innovation_std, .. }
        | Process::Arma11 { innovation_std, .. } => *innovation_std,
        Process::Arch1 { variance_intercept, error_coefficient, .. } => variance_intercept.sqrt().hypot(error_coefficient.sqrt() * state.error.abs()),
        Process::Garch11 { variance_intercept, error_coefficient, variance_coefficient, .. } => {
            variance_intercept.sqrt().hypot(error_coefficient.sqrt() * state.error.abs()).hypot(variance_coefficient.sqrt() * state.std)
        }
        Process::Egarch11 { log_variance_intercept, error_coefficient, log_variance_coefficient, theta, gamma, .. } => {
            let g = theta * state.normal + gamma * (state.normal.abs() - (2.0 / std::f64::consts::PI).sqrt());
            let log_var = finite(log_variance_coefficient.mul_add(state.log_variance, error_coefficient.mul_add(g, *log_variance_intercept)))?;
            let std = finite((0.5 * log_var).exp())?;
            if std == 0.0 {
                return Err(error("numerical_range", "EGARCH conditional scale underflows."));
            }
            std
        }
        Process::Aparch11 { power_intercept, error_coefficient, persistence, power, asymmetry, .. } => {
            let mut terms = vec![power_intercept.ln()];
            if *error_coefficient > 0.0 && state.error != 0.0 {
                terms.push(error_coefficient.ln() + power * (state.error.abs().ln() + (1.0 - asymmetry * state.error.signum()).ln()));
            }
            if *persistence > 0.0 {
                terms.push(persistence.ln() + power * state.std.ln());
            }
            let std = finite((log_sum(&terms)? / power).exp())?;
            if std == 0.0 {
                return Err(error("numerical_range", "APARCH conditional scale underflows."));
            }
            std
        }
    };
    finite(std)?;
    let innovation = finite(std * z)?;
    let center = finite(match &spec.process {
        Process::Ar1 { coefficient, .. } => coefficient.mul_add(state.center, innovation),
        Process::Ar2 { first, second, .. } => first.mul_add(state.center, second.mul_add(state.previous_center, innovation)),
        Process::Ma1 { coefficient, .. } => coefficient.mul_add(state.error, innovation),
        Process::Ma2 { first, second, .. } => first.mul_add(state.error, second.mul_add(state.previous_error, innovation)),
        Process::Arma11 { ar, ma, .. } => ar.mul_add(state.center, ma.mul_add(state.error, innovation)),
        _ => innovation,
    })?;
    state.previous_center = state.center;
    state.center = center;
    state.previous_error = state.error;
    state.error = innovation;
    state.std = std;
    state.normal = z;
    state.log_variance = if std > 0.0 { 2.0 * std.ln() } else { 0.0 };
    Ok((finite(center + spec.mean)?, std))
}
struct PostState {
    level: f64,
    difference: f64,
    recent: Vec<f64>,
    older: Vec<f64>,
}
impl PostState {
    fn new(spec: &Postprocess) -> Self {
        let (level, difference) = match spec.integration {
            Integration::None {} => (0.0, 0.0),
            Integration::First { initial_level } => (initial_level, 0.0),
            Integration::Second { initial_level, initial_difference } => (initial_level, initial_difference),
        };
        let (recent, older) = match &spec.seasonality {
            Seasonality::First { previous_season } => (previous_season.clone(), vec![]),
            Seasonality::Second { previous_season, older_season } => (previous_season.clone(), older_season.clone()),
            _ => (vec![], vec![]),
        };
        Self { level, difference, recent, older }
    }
    fn apply(&mut self, config: &Postprocess, raw: f64, index: usize) -> Result<f64> {
        let mut value = match config.integration {
            Integration::None {} => raw,
            Integration::First { .. } => {
                self.level = finite(self.level + raw)?;
                self.level
            }
            Integration::Second { .. } => {
                self.difference = finite(self.difference + raw)?;
                self.level = finite(self.level + self.difference)?;
                self.level
            }
        };
        value = match &config.seasonality {
            Seasonality::None {} => value,
            Seasonality::Additive { terms, starting_phase } => {
                let pos = (index + starting_phase) % terms.len();
                value + terms.get(pos).copied().ok_or_else(|| error("internal_error", "Missing seasonal term."))?
            }
            Seasonality::First { .. } => {
                let pos = index % self.recent.len();
                let previous = self.recent.get_mut(pos).ok_or_else(|| error("internal_error", "Missing seasonal state."))?;
                let next = finite(value + *previous)?;
                *previous = next;
                next
            }
            Seasonality::Second { .. } => {
                let pos = index % self.recent.len();
                let recent = self.recent.get_mut(pos).ok_or_else(|| error("internal_error", "Missing recent seasonal state."))?;
                let older = self.older.get_mut(pos).ok_or_else(|| error("internal_error", "Missing older seasonal state."))?;
                let next = finite(value + 2.0 * *recent - *older)?;
                *older = *recent;
                *recent = next;
                next
            }
        };
        finite(match config.transformation {
            Transformation::Identity { shift } => value + shift,
            Transformation::Exponential { shift } => value.exp() + shift,
            Transformation::Square { shift } => value.mul_add(value, shift),
        })
    }
}
fn run(spec: &TimeSeriesSpec, shocks: Vec<f64>, limits: &TimeSeriesLimits, control: &dyn RunControl, version: &str) -> Result<TimeSeriesPath> {
    validate(spec, shocks.len(), limits)?;
    all(&shocks)?;
    if control.is_cancelled() {
        return Err(error("cancelled", "Time-series path cancelled."));
    }
    let mut state = startup(spec)?;
    let mut post = PostState::new(&spec.postprocess);
    let mut values = Vec::with_capacity(shocks.len());
    let mut conditional_std = Vec::with_capacity(shocks.len());
    for (index, z) in shocks.iter().enumerate() {
        if control.is_cancelled() {
            return Err(error("cancelled", "Time-series path cancelled."));
        }
        let (raw, std) = step(spec, &mut state, *z)?;
        values.push(post.apply(&spec.postprocess, raw, index)?);
        conditional_std.push(std);
        if index % 256 == 0 {
            control.progress(&ProgressEvent {
                stage: "time_series".into(),
                iterations: index as u64 + 1,
                evaluations: index as u64 + 1,
                nodes: 0,
                best_objective: None,
            });
        }
    }
    Ok(TimeSeriesPath {
        steps: shocks.len() as u64,
        values,
        conditional_std,
        standard_normal_shocks: shocks,
        sampling_version: version.into(),
        startup: "Output index0 is the first generated step after caller-supplied time0 state; no random warmup or stationary initialization.".into(),
    })
}
/// Each shock is a pure function of(seed,binding,trial,time_index). Calls do not
/// mutate a global generator; length extension preserves an existing prefix.
pub fn sample_path(
    spec: &TimeSeriesSpec,
    length: usize,
    address: SeriesAddress,
    limits: &TimeSeriesLimits,
    control: &dyn RunControl,
) -> Result<TimeSeriesPath> {
    validate(spec, length, limits)?;
    let mut shocks = Vec::with_capacity(length);
    for index in 0..length {
        if control.is_cancelled() {
            return Err(error("cancelled", "Time-series path cancelled."));
        }
        let counter = STREAM_BASE.checked_add(index as u64).ok_or_else(|| error("time_series_limit", "Time address overflow."))?;
        let u = crate::addressed_uniform(address.seed, address.binding, address.trial, counter);
        shocks.push(crate::distributions::inverse_normal(u)?);
    }
    run(spec, shocks, limits, control, STREAM_VERSION)
}
/// Replay the immutable prefix to a zero-based output index. Use sample_path
/// for bulk paths to avoid repeated prefix work.
pub fn value_at(
    spec: &TimeSeriesSpec,
    time_index: usize,
    address: SeriesAddress,
    limits: &TimeSeriesLimits,
    control: &dyn RunControl,
) -> Result<f64> {
    let length = time_index.checked_add(1).ok_or_else(|| error("time_series_limit", "Time index overflow."))?;
    sample_path(spec, length, address, limits, control)?
        .values
        .get(time_index)
        .copied()
        .ok_or_else(|| error("internal_error", "Requested path value is absent."))
}
/// Condition the recurrence on explicitly supplied finite standard-normal
/// innovations. This API does not certify that external shocks are random.
pub fn simulate_with_shocks(spec: &TimeSeriesSpec, shocks: &[f64], limits: &TimeSeriesLimits, control: &dyn RunControl) -> Result<TimeSeriesPath> {
    validate(spec, shocks.len(), limits)?;
    all(shocks)?;
    run(spec, shocks.to_vec(), limits, control, "external-standard-normal-innovations-v1")
}
/// PsiTSSip-style matrix replay: rows=time, columns=trial, no resampling.
pub fn replay_tssip(matrix: &[Vec<f64>], trial_index: usize, limits: &TimeSeriesLimits, control: &dyn RunControl) -> Result<Vec<f64>> {
    if matrix.len() > limits.max_length || matrix.len() as u64 > limits.max_steps || matrix.len() > 1_000_000 {
        return Err(error("time_series_limit", "Replay path exceeds its step/length cap."));
    }
    let columns = matrix.first().map_or(0, Vec::len);
    if columns == 0 || trial_index >= columns || matrix.len().checked_mul(columns).is_none_or(|n| n > 1_000_000) {
        return Err(error("time_series_domain", "Replay needs a bounded rectangular time-by-trial matrix and valid trial index."));
    }
    if matrix.len().checked_mul(8).and_then(|n| u64::try_from(n).ok()).is_none_or(|b| b > limits.max_memory_bytes) {
        return Err(error("memory_limit", "Replay output exceeds memory budget."));
    }
    for row in matrix {
        if row.len() != columns {
            return Err(error("time_series_domain", "Replay rows must have equal trial counts."));
        }
        all(row)?;
    }
    let mut values = Vec::with_capacity(matrix.len());
    for row in matrix {
        if control.is_cancelled() {
            return Err(error("cancelled", "Replay cancelled."));
        }
        values.push(row.get(trial_index).copied().ok_or_else(|| error("time_series_domain", "Replay trial is missing."))?);
    }
    Ok(values)
}
