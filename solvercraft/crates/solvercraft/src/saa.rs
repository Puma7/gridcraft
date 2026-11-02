//! Sample-average optimization with a frozen training set and a separate final holdout.
use crate::risk::{RiskMeasure, RiskResult, Scenario, ScenarioProblem};
use serde::{Deserialize, Serialize};
use solvercraft_types::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampledAffineLoss {
    pub offset: Distribution,
    /// Independent marginals; shared uncertain factors should use an explicit scenario source.
    pub coefficients: Vec<Distribution>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleAverageProblem {
    pub name: String,
    pub variables: Vec<Variable>,
    pub constraints: Vec<LinearConstraint>,
    pub loss: SampledAffineLoss,
    pub measure: RiskMeasure,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleAverageOptions {
    pub training_trials: u64,
    pub validation_trials: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub training_seed: u64,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub validation_seed: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldoutEvaluation {
    pub trials: u64,
    pub mean: f64,
    pub sample_variance: f64,
    pub standard_error: f64,
    /// Normal-approximation interval for the mean of this fixed final decision.
    pub approximate_mean_interval_95: (f64, f64),
    pub cvar: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleAverageResult {
    pub training: RiskResult,
    pub validation: Option<HoldoutEvaluation>,
    pub training_trials: u64,
    pub validation_trials: u64,
    pub sampling_version: String,
    pub note: String,
}
fn invalid(message: &str) -> SolverError {
    SolverError::new("invalid_saa", message)
}
fn preflight(problem: &SampleAverageProblem, sampling: &SampleAverageOptions, options: &SolveOptions) -> Result<()> {
    solvercraft_model::validate_options(options)?;
    if sampling.training_seed == sampling.validation_seed {
        return Err(invalid("Training and validation seeds must differ"));
    }
    if sampling.training_trials == 0 || sampling.training_trials > 100_000 || sampling.validation_trials < 2 || sampling.validation_trials > 1_000_000
    {
        return Err(invalid("Training requires 1..100000 draws and validation 2..1000000 draws"));
    }
    if problem.loss.coefficients.len() != problem.variables.len()
        || problem.variables.len() > solvercraft_model::MAX_VARIABLES
        || problem.constraints.len() > solvercraft_model::MAX_CONSTRAINTS
        || problem.name.len() > solvercraft_model::MAX_NAME_BYTES
    {
        return Err(invalid("Invalid affine loss dimensions or name"));
    }
    if sampling
        .training_trials
        .checked_add(sampling.validation_trials)
        .and_then(|n| n.checked_mul(problem.variables.len() as u64 + 1))
        .is_none_or(|n| n > 50_000_000)
    {
        return Err(SolverError::new("model_limit", "SAA sampling exceeds bounded quantile evaluations"));
    }
    let mut terms = 0usize;
    for row in &problem.constraints {
        terms = terms.checked_add(row.terms.len()).ok_or_else(|| invalid("Term overflow"))?;
    }
    if terms > solvercraft_model::MAX_TERMS {
        return Err(invalid("Too many deterministic terms"));
    }
    let mut distribution_storage = solvercraft_math::distributions::prepared_storage_bytes(&problem.loss.offset)?;
    for d in &problem.loss.coefficients {
        distribution_storage = distribution_storage
            .checked_add(solvercraft_math::distributions::prepared_storage_bytes(d)?)
            .ok_or_else(|| invalid("Distribution storage overflow"))?;
    }
    let draw_cells = sampling.training_trials.checked_mul(problem.variables.len() as u64 + 4).and_then(|x| x.checked_mul(64));
    let names = problem
        .name
        .len()
        .saturating_add(problem.variables.iter().map(|v| v.name.len()).sum::<usize>())
        .saturating_add(problem.constraints.iter().map(|v| v.name.len()).sum::<usize>());
    let bytes = draw_cells
        .and_then(|x| x.checked_add(distribution_storage))
        .and_then(|x| x.checked_add(sampling.validation_trials.checked_mul(32)?))
        .and_then(|x| x.checked_add((terms as u64).checked_mul(256)?))
        .and_then(|x| x.checked_add((names as u64).checked_mul(2)?));
    if bytes.is_none_or(|x| x > options.max_memory_bytes) {
        return Err(SolverError::new("memory_limit", "SAA draws exceed budget before allocation"));
    }
    // Validate deterministic metadata without a temporary cloned model.
    for variable in &problem.variables {
        solvercraft_model::validate_variable(variable)?;
    }
    for row in &problem.constraints {
        if row.name.len() > solvercraft_model::MAX_NAME_BYTES
            || !row.rhs.is_finite()
            || row.terms.iter().any(|t| t.variable >= problem.variables.len() || !t.coefficient.is_finite())
        {
            return Err(invalid("Invalid deterministic constraint"));
        }
    }
    if let RiskMeasure::ConditionalValueAtRisk { confidence } = problem.measure {
        if !confidence.is_finite() || !(0.0..1.0).contains(&confidence) {
            return Err(invalid("CVaR confidence must lie in [0,1)"));
        }
        if problem.variables.len().saturating_add(sampling.training_trials as usize).saturating_add(1) > solvercraft_model::MAX_VARIABLES
            || problem.constraints.len().saturating_add(sampling.training_trials as usize) > solvercraft_model::MAX_CONSTRAINTS
            || (sampling.training_trials as usize)
                .checked_mul(problem.variables.len().saturating_add(3))
                .and_then(|n| n.checked_add(terms))
                .is_none_or(|n| n > solvercraft_model::MAX_TERMS)
        {
            return Err(SolverError::new("model_limit", "Sampled CVaR expansion exceeds model dimensions before drawing"));
        }
    }
    if let RiskMeasure::MeanVariance { penalty } = problem.measure {
        if !penalty.is_finite() || penalty < 0.0 {
            return Err(invalid("Variance penalty must be finite and nonnegative"));
        }
        let n = problem.variables.len();
        let covariance = n.checked_mul(n.saturating_add(1)).map(|v| v / 2).and_then(|v| v.checked_add(n.saturating_mul(2)));
        if penalty > 0.0
            && (covariance.is_none_or(|v| v.saturating_add(terms) > solvercraft_model::MAX_TERMS)
                || covariance.and_then(|v| v.checked_mul(sampling.training_trials as usize)).is_none_or(|v| v > 50_000_000))
        {
            return Err(SolverError::new("model_limit", "Sampled variance assembly exceeds bounded work before drawing"));
        }
    }
    Ok(())
}
fn prepare(loss: &SampledAffineLoss) -> Result<Vec<solvercraft_math::PreparedDistribution>> {
    let mut prepared = Vec::new();
    prepared.try_reserve_exact(loss.coefficients.len() + 1).map_err(|_| invalid("Cannot allocate loss marginals"))?;
    prepared.push(solvercraft_math::PreparedDistribution::new(&loss.offset)?);
    for distribution in &loss.coefficients {
        prepared.push(solvercraft_math::PreparedDistribution::new(distribution)?);
    }
    Ok(prepared)
}
fn draw(prepared: &[solvercraft_math::PreparedDistribution], seed: u64, trial: u64) -> Result<Scenario> {
    let mut values = Vec::new();
    values.try_reserve_exact(prepared.len()).map_err(|_| invalid("Cannot allocate scenario"))?;
    for (binding, distribution) in prepared.iter().enumerate() {
        values.push(distribution.quantile(solvercraft_math::addressed_uniform(seed, binding as u64, trial, 0))?);
    }
    let offset = *values.first().ok_or_else(|| invalid("Missing affine offset"))?;
    Ok(Scenario { weight: 1.0, offset, coefficients: values.into_iter().skip(1).collect() })
}
/// Materialize the training scenarios once. Reuse this object for all candidate
/// comparisons; no candidate evaluation consumes or changes a random stream.
pub fn training_problem(problem: &SampleAverageProblem, sampling: &SampleAverageOptions, options: &SolveOptions) -> Result<ScenarioProblem> {
    controlled_training_problem(problem, sampling, options, &NoControl)
}
fn controlled_training_problem(
    problem: &SampleAverageProblem,
    sampling: &SampleAverageOptions,
    options: &SolveOptions,
    control: &dyn RunControl,
) -> Result<ScenarioProblem> {
    preflight(problem, sampling, options)?;
    if control.is_cancelled() {
        return Err(SolverError::new("cancelled", "SAA training preparation was cancelled"));
    }
    let prepared = prepare(&problem.loss)?;
    let mut scenarios = Vec::new();
    scenarios.try_reserve_exact(sampling.training_trials as usize).map_err(|_| invalid("Cannot allocate training scenarios"))?;
    for trial in 0..sampling.training_trials {
        if control.is_cancelled() {
            return Err(SolverError::new("cancelled", "SAA training preparation was cancelled"));
        }
        if trial % 128 == 0 {
            control.progress(&ProgressEvent {
                stage: "saa_training_draws".into(),
                iterations: 0,
                evaluations: trial,
                nodes: 0,
                best_objective: None,
            });
        }
        scenarios.push(draw(&prepared, sampling.training_seed, trial)?);
    }
    let training = ScenarioProblem {
        name: problem.name.clone(),
        variables: problem.variables.clone(),
        constraints: problem.constraints.clone(),
        scenarios,
        measure: problem.measure.clone(),
    };
    crate::risk::compile(&training, options)?;
    Ok(training)
}
/// Original affine losses on the identical frozen training scenarios.
pub fn compare_candidates(training: &ScenarioProblem, candidates: &[Vec<f64>], options: &SolveOptions) -> Result<Vec<f64>> {
    crate::risk::compile(training, options)?;
    if candidates.len() > 10_000
        || candidates
            .len()
            .checked_mul(training.scenarios.len())
            .and_then(|x| x.checked_mul(training.variables.len().max(1)))
            .is_none_or(|x| x > 10_000_000)
    {
        return Err(invalid("Candidate comparison exceeds bounded work"));
    }
    let base = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: training.name.clone(),
        variables: training.variables.clone(),
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: training.constraints.clone(),
    };
    let weights: Vec<_> = training.scenarios.iter().map(|s| s.weight).collect();
    let mut results = Vec::new();
    results.try_reserve_exact(candidates.len()).map_err(|_| invalid("Cannot allocate comparison results"))?;
    for values in candidates {
        if !solvercraft_model::validate_solution(&base, values, options)?.feasible {
            return Err(invalid("Candidate violates deterministic constraints"));
        }
        let losses = training.scenarios.iter().map(|s| loss(s, values)).collect::<Result<Vec<_>>>()?;
        results.push(match training.measure {
            RiskMeasure::ExpectedLoss => crate::risk::weighted_mean(&losses, &weights)?,
            RiskMeasure::MeanVariance { penalty } => {
                let mean = crate::risk::weighted_mean(&losses, &weights)?;
                let variance = crate::risk::weighted_variance(&losses, &weights)?;
                let value = mean + penalty * variance;
                if !value.is_finite() {
                    return Err(invalid("Candidate mean-variance objective overflow"));
                }
                value
            }
            RiskMeasure::ConditionalValueAtRisk { confidence } => crate::risk::weighted_upper_tail_cvar(&losses, &weights, confidence)?,
        });
    }
    Ok(results)
}
fn loss(scenario: &Scenario, values: &[f64]) -> Result<f64> {
    let mut sum = solvercraft_model::CompensatedSum::default();
    sum.add(scenario.offset)?;
    for (coefficient, value) in scenario.coefficients.iter().zip(values) {
        sum.add(coefficient * value)?;
    }
    sum.finish()
}
pub fn train_and_validate(
    problem: &SampleAverageProblem,
    sampling: &SampleAverageOptions,
    options: &SolveOptions,
    control: &dyn RunControl,
) -> Result<SampleAverageResult> {
    let frozen = controlled_training_problem(problem, sampling, options, control)?;
    let training = crate::risk::optimize(&frozen, options, control)?;
    let mut validation = None;
    if let Some(decision) = &training.decision_values {
        let prepared = prepare(&problem.loss)?;
        let mut losses = Vec::new();
        losses.try_reserve_exact(sampling.validation_trials as usize).map_err(|_| invalid("Cannot allocate validation losses"))?;
        for trial in 0..sampling.validation_trials {
            if control.is_cancelled() {
                return Err(SolverError::new("cancelled", "Final holdout evaluation was cancelled; no incomplete interval is reported"));
            }
            if trial % 128 == 0 {
                control.progress(&ProgressEvent {
                    stage: "saa_final_holdout".into(),
                    iterations: 0,
                    evaluations: trial,
                    nodes: 0,
                    best_objective: training.expected_loss,
                });
            }
            losses.push(loss(&draw(&prepared, sampling.validation_seed, trial)?, decision)?);
        }
        let weights = vec![1.0; losses.len()];
        let mean = crate::risk::weighted_mean(&losses, &weights)?;
        // The shared risk statistic scales before squaring and rejects a
        // nonzero variance outside f64's supported range. Direct tiny squares
        // otherwise underflow to zero and manufacture a zero-width interval.
        let population_variance = crate::risk::weighted_variance(&losses, &weights)?;
        let sample_variance = population_variance * (losses.len() as f64 / (losses.len() - 1) as f64);
        if !sample_variance.is_finite() {
            return Err(SolverError::new("numerical_failure", "Holdout variance exceeds finite precision"));
        }
        let standard_error = sample_variance.sqrt() / (losses.len() as f64).sqrt();
        let half_width = 1.959_963_984_540_054 * standard_error;
        // A positive width can be smaller than one ULP of a large mean. Round
        // the endpoints outward so it cannot become a false point interval.
        // Truly constant holdouts retain their exact zero-width interval.
        let interval = if half_width > 0.0 { ((mean - half_width).next_down(), (mean + half_width).next_up()) } else { (mean, mean) };
        if !interval.0.is_finite() || !interval.1.is_finite() {
            return Err(SolverError::new("numerical_failure", "Holdout interval exceeds finite precision"));
        }
        let cvar = match problem.measure {
            RiskMeasure::ExpectedLoss | RiskMeasure::MeanVariance { .. } => None,
            RiskMeasure::ConditionalValueAtRisk { confidence } => Some(crate::risk::weighted_upper_tail_cvar(&losses, &weights, confidence)?),
        };
        validation = Some(HoldoutEvaluation {
            trials: sampling.validation_trials,
            mean,
            sample_variance,
            standard_error,
            approximate_mean_interval_95: interval,
            cvar,
        });
    }
    Ok(SampleAverageResult{training,validation,training_trials:sampling.training_trials,validation_trials:sampling.validation_trials,sampling_version:format!("{}/{}", solvercraft_math::SAMPLING_VERSION, solvercraft_math::catalog::CATALOG_VERSION),note:"Frozen addressed training draws and distinct-seed final holdout. The displayed mean interval uses a normal approximation for a fixed final decision; it is not an optimality gap, distribution-free confidence guarantee, or a CVaR interval. Independent marginals only.".into()})
}
