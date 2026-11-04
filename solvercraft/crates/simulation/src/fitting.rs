//! Likelihood fitting, transparent AIC/BIC ranking, and refit parametric-bootstrap GOF.
//! Numerical fits are bounded local likelihood searches, not global MLE certificates.
use crate::{fit_exponential, fit_normal, fit_uniform, reserve};
use serde::{Deserialize, Serialize};
use solvercraft_math::{
    PreparedDistribution, addressed_uniform,
    density::{is_discrete, log_density},
    special::finite_sum,
    validate_distribution,
};
use solvercraft_types::{Distribution as D, Result, RunControl, SolverError};
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterField {
    Mean,
    StdDev,
    LogMean,
    LogStdDev,
    Low,
    High,
    Mode,
    Weight,
    Alpha,
    Beta,
    Shape,
    ShapeC,
    ShapeK,
    Power,
    LeftProbability,
    Scale,
    Location,
    Rate,
    Probability,
    DegreesOfFreedom,
    NumeratorDf,
    DenominatorDf,
    Minimum,
    Gamma,
    Delta,
    Shift,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitParameter {
    pub field: ParameterField,
    pub lower: f64,
    pub upper: f64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FitFamily {
    Normal,
    Uniform,
    Exponential,
    Lognormal,
    Bernoulli,
    Poisson,
    Geometric,
    Gamma,
    Beta,
    Weibull,
    Laplace,
    Cauchy,
    Pareto,
    Rayleigh,
    InverseGaussian,
    Parametric { initial: D, parameters: Vec<FitParameter> },
}
impl<'de> Deserialize<'de> for FitFamily {
    fn deserialize<T: serde::Deserializer<'de>>(deserializer: T) -> std::result::Result<Self, T::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Strict {
            Normal {},
            Uniform {},
            Exponential {},
            Lognormal {},
            Bernoulli {},
            Poisson {},
            Geometric {},
            Gamma {},
            Beta {},
            Weibull {},
            Laplace {},
            Cauchy {},
            Pareto {},
            Rayleigh {},
            InverseGaussian {},
            Parametric { initial: D, parameters: Vec<FitParameter> },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Normal {} => Self::Normal,
            Strict::Uniform {} => Self::Uniform,
            Strict::Exponential {} => Self::Exponential,
            Strict::Lognormal {} => Self::Lognormal,
            Strict::Bernoulli {} => Self::Bernoulli,
            Strict::Poisson {} => Self::Poisson,
            Strict::Geometric {} => Self::Geometric,
            Strict::Gamma {} => Self::Gamma,
            Strict::Beta {} => Self::Beta,
            Strict::Weibull {} => Self::Weibull,
            Strict::Laplace {} => Self::Laplace,
            Strict::Cauchy {} => Self::Cauchy,
            Strict::Pareto {} => Self::Pareto,
            Strict::Rayleigh {} => Self::Rayleigh,
            Strict::InverseGaussian {} => Self::InverseGaussian,
            Strict::Parametric { initial, parameters } => Self::Parametric { initial, parameters },
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FitOptions {
    pub max_evaluations: u64,
    pub bootstrap_replicates: u32,
    #[serde(with = "solvercraft_types::decimal_seed")]
    pub seed: u64,
    pub max_memory_bytes: u64,
    pub max_work: u64,
}
impl Default for FitOptions {
    fn default() -> Self {
        Self { max_evaluations: 2048, bootstrap_replicates: 0, seed: 0, max_memory_bytes: 64 * 1024 * 1024, max_work: 100_000_000 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodnessOfFit {
    pub ks_statistic: f64,
    pub clipped_ad_statistic: f64,
    pub ks_bootstrap_p: f64,
    pub ad_bootstrap_p: f64,
    pub bootstrap_replicates: u32,
    pub refitted_parameters: bool,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitReport {
    pub distribution: D,
    pub estimated_parameters: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub bic: f64,
    pub method: String,
    pub converged: bool,
    pub evaluations: u64,
    pub goodness_of_fit: Option<GoodnessOfFit>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingCriterion {
    Aic,
    Bic,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateFailure {
    pub index: usize,
    pub error: SolverError,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitRanking {
    pub criterion: RankingCriterion,
    pub fits: Vec<(usize, FitReport)>,
    pub failures: Vec<CandidateFailure>,
}
fn domain(message: &str) -> SolverError {
    SolverError::new("fit_domain", message)
}
fn check(values: &[f64], options: &FitOptions) -> Result<()> {
    if values.len() < 2 || values.len() > 100_000 || values.iter().any(|v| !v.is_finite()) {
        return Err(domain("Fitting requires 2..100000 finite observations."));
    }
    if options.max_evaluations == 0 || options.max_evaluations > 100_000 || options.bootstrap_replicates > 500 {
        return Err(domain("Fitting evaluation limit is 1..100000, bootstrap limit is 500."));
    }
    let work = (values.len() as u64)
        .checked_mul(options.max_evaluations)
        .and_then(|n| n.checked_mul(u64::from(options.bootstrap_replicates) + 1))
        .ok_or_else(|| domain("Fitting work overflow."))?;
    if work > options.max_work {
        return Err(SolverError::new("work_limit", "Fitting worst-case work exceeds budget; reduce observations/evaluations/bootstrap."));
    }
    if (values.len() as u64).saturating_mul(64).saturating_add(65536) > options.max_memory_bytes {
        return Err(SolverError::new("memory_limit", "Fitting scratch exceeds configured memory."));
    }
    Ok(())
}
fn cancelled(control: &dyn RunControl) -> Result<()> {
    if control.is_cancelled() { Err(SolverError::new("cancelled", "Fitting was cancelled.")) } else { Ok(()) }
}
fn parameter(d: &mut D, field: ParameterField) -> Result<&mut f64> {
    use ParameterField as P;
    let value = match (field, d) {
        (P::Mean, D::Normal { mean, .. } | D::InverseGaussian { mean, .. }) => mean,
        (P::StdDev, D::Normal { std_dev, .. }) => std_dev,
        (P::LogMean, D::Lognormal { log_mean, .. }) => log_mean,
        (P::LogStdDev, D::Lognormal { log_std_dev, .. }) => log_std_dev,
        (
            P::Low,
            D::Uniform { low, .. }
            | D::Triangular { low, .. }
            | D::BetaPert { low, .. }
            | D::Reciprocal { low, .. }
            | D::DoubleTriangular { low, .. },
        ) => low,
        (
            P::High,
            D::Uniform { high, .. }
            | D::Triangular { high, .. }
            | D::BetaPert { high, .. }
            | D::Reciprocal { high, .. }
            | D::DoubleTriangular { high, .. },
        ) => high,
        (P::Mode, D::Triangular { mode, .. } | D::BetaPert { mode, .. } | D::DoubleTriangular { mode, .. }) => mode,
        (P::Weight, D::BetaPert { weight, .. }) => weight,
        (P::Alpha, D::Beta { alpha, .. } | D::Kumaraswamy { alpha, .. } | D::BetaPrime { alpha, .. }) => alpha,
        (P::Beta, D::Beta { beta, .. } | D::Kumaraswamy { beta, .. } | D::BetaPrime { beta, .. }) => beta,
        (P::ShapeC, D::Burr12 { shape_c, .. }) => shape_c,
        (P::ShapeK, D::Burr12 { shape_k, .. }) => shape_k,
        (P::Power, D::Dagum { power, .. }) => power,
        (P::LeftProbability, D::DoubleTriangular { left_probability, .. }) => left_probability,
        (
            P::Shape,
            D::Gamma { shape, .. }
            | D::Weibull { shape, .. }
            | D::InverseGamma { shape, .. }
            | D::InverseGaussian { shape, .. }
            | D::LogLogistic { shape, .. }
            | D::Pareto { shape, .. }
            | D::SkewNormal { shape, .. },
        ) => shape,
        (P::Shape, D::Dagum { shape, .. } | D::FatigueLife { shape, .. } | D::Frechet { shape, .. } | D::Pareto2 { shape, .. }) => shape,
        (
            P::Scale,
            D::Gamma { scale, .. }
            | D::Weibull { scale, .. }
            | D::StudentT { scale, .. }
            | D::Cauchy { scale, .. }
            | D::Logistic { scale, .. }
            | D::Laplace { scale, .. }
            | D::Gumbel { scale, .. }
            | D::Rayleigh { scale, .. }
            | D::Erlang { scale, .. }
            | D::InverseGamma { scale, .. }
            | D::LogLogistic { scale, .. }
            | D::SkewNormal { scale, .. }
            | D::JohnsonSu { scale, .. }
            | D::JohnsonSl { scale, .. }
            | D::JohnsonSb { scale, .. }
            | D::ShiftScale { scale, .. },
        ) => scale,
        (
            P::Scale,
            D::Burr12 { scale, .. }
            | D::Dagum { scale, .. }
            | D::FatigueLife { scale, .. }
            | D::Frechet { scale, .. }
            | D::HyperbolicSecant { scale, .. }
            | D::Levy { scale, .. }
            | D::Pareto2 { scale, .. }
            | D::BetaPrime { scale, .. }
            | D::MinExtreme { scale, .. },
        ) => scale,
        (
            P::Location,
            D::StudentT { location, .. }
            | D::Cauchy { location, .. }
            | D::Logistic { location, .. }
            | D::Laplace { location, .. }
            | D::Gumbel { location, .. }
            | D::SkewNormal { location, .. }
            | D::JohnsonSu { location, .. }
            | D::JohnsonSl { location, .. }
            | D::JohnsonSb { location, .. },
        ) => location,
        (P::Location, D::HyperbolicSecant { location, .. } | D::Levy { location, .. } | D::MinExtreme { location, .. }) => location,
        (P::Rate, D::Poisson { rate } | D::Exponential { rate } | D::CompoundPoisson { rate, .. }) => rate,
        (
            P::Probability,
            D::Bernoulli { probability }
            | D::Binomial { probability, .. }
            | D::NegativeBinomial { probability, .. }
            | D::Geometric { probability }
            | D::Logarithmic { probability },
        ) => probability,
        (P::DegreesOfFreedom, D::StudentT { degrees_of_freedom, .. } | D::ChiSquared { degrees_of_freedom }) => degrees_of_freedom,
        (P::NumeratorDf, D::F { numerator_df, .. }) => numerator_df,
        (P::DenominatorDf, D::F { denominator_df, .. }) => denominator_df,
        (P::Minimum, D::Pareto { minimum, .. }) => minimum,
        (P::Gamma, D::JohnsonSu { gamma, .. } | D::JohnsonSb { gamma, .. } | D::JohnsonSl { gamma, .. }) => gamma,
        (P::Delta, D::JohnsonSu { delta, .. } | D::JohnsonSb { delta, .. } | D::JohnsonSl { delta, .. }) => delta,
        (P::Shift, D::ShiftScale { shift, .. }) => shift,
        _ => return Err(domain("Fit field is not a floating parameter of the selected distribution.")),
    };
    Ok(value)
}
fn p(field: ParameterField, lower: f64, upper: f64) -> FitParameter {
    FitParameter { field, lower, upper }
}
fn initial(values: &[f64], family: &FitFamily) -> Result<(D, Vec<FitParameter>, usize)> {
    use ParameterField as P;
    if let FitFamily::Parametric { initial, parameters } = family {
        if matches!(
            initial,
            D::Empirical { .. }
                | D::EmpiricalContinuous { .. }
                | D::Categorical { .. }
                | D::ShiftScale { .. }
                | D::Truncated { .. }
                | D::Mixture { .. }
                | D::CompoundPoisson { .. }
                | D::Metalog { .. }
        ) {
            return Err(domain(
                "Generic likelihood fitting currently accepts scalar analytic families; composite/empirical fits need a dedicated estimator.",
            ));
        }
        return Ok((initial.clone(), parameters.clone(), parameters.len()));
    }
    let normal = fit_normal(values)?;
    let (mean, sd) = if let D::Normal { mean, std_dev } = normal {
        (mean, std_dev)
    } else {
        return Err(domain("Normal initializer failed."));
    };
    let mut sorted = Vec::new();
    reserve(&mut sorted, values.len())?;
    sorted.extend_from_slice(values);
    sorted.sort_unstable_by(f64::total_cmp);
    let low = sorted[0];
    let high = sorted[sorted.len() - 1];
    let median = sorted[sorted.len() / 2];
    let width = high - low;
    if !width.is_finite() {
        return Err(domain("Fit data span exceeds finite arithmetic."));
    }
    let positive = || if low > 0.0 { Ok(()) } else { Err(domain("This fit family requires strictly positive observations.")) };
    let pair = match family {
        FitFamily::Normal => (D::Normal { mean, std_dev: sd }, vec![], 2),
        FitFamily::Uniform => (fit_uniform(values)?, vec![], 2),
        FitFamily::Exponential => (fit_exponential(values)?, vec![], 1),
        FitFamily::Lognormal => {
            positive()?;
            let logs = values.iter().map(|x| x.ln()).collect::<Vec<_>>();
            let d = fit_normal(&logs)?;
            let D::Normal { mean, std_dev } = d else {
                return Err(domain("Lognormal initializer failed."));
            };
            (D::Lognormal { log_mean: mean, log_std_dev: std_dev }, vec![], 2)
        }
        FitFamily::Bernoulli => {
            if values.iter().any(|x| *x != 0.0 && *x != 1.0) {
                return Err(domain("Bernoulli data must be 0 or 1."));
            }
            (D::Bernoulli { probability: mean }, vec![], 1)
        }
        FitFamily::Poisson => {
            if low < 0.0 || values.iter().any(|x| x.fract() != 0.0) {
                return Err(domain("Count data must be nonnegative integers."));
            }
            (D::Poisson { rate: mean }, vec![], 1)
        }
        FitFamily::Geometric => {
            if low < 0.0 || values.iter().any(|x| x.fract() != 0.0) {
                return Err(domain("Count data must be nonnegative integers."));
            }
            (D::Geometric { probability: 1.0 / (1.0 + mean) }, vec![], 1)
        }
        FitFamily::Gamma => {
            positive()?;
            if sd <= 0.0 {
                return Err(domain("Gamma fit requires varying data."));
            }
            let shape = (mean / sd).powi(2).clamp(0.01, 1000.0);
            (
                D::Gamma { shape, scale: mean / shape },
                vec![p(P::Shape, 0.001, 10000.0), p(P::Scale, (mean / 10000.0).max(f64::MIN_POSITIVE), mean * 1000.0)],
                2,
            )
        }
        FitFamily::Beta => {
            if low <= 0.0 || high >= 1.0 || sd <= 0.0 {
                return Err(domain("Beta fitting requires varying observations in (0,1)."));
            }
            let k = (mean * (1.0 - mean) / (sd * sd) - 1.0).max(0.1);
            (
                D::Beta { alpha: (mean * k).clamp(0.001, 1000.0), beta: ((1.0 - mean) * k).clamp(0.001, 1000.0) },
                vec![p(P::Alpha, 0.0001, 10000.0), p(P::Beta, 0.0001, 10000.0)],
                2,
            )
        }
        FitFamily::Weibull => {
            positive()?;
            (D::Weibull { shape: 1.0, scale: mean }, vec![p(P::Shape, 0.01, 100.0), p(P::Scale, low * 0.01, high * 100.0)], 2)
        }
        FitFamily::Laplace => {
            let scale = finite_sum(values.iter().map(|x| (x - median).abs() / values.len() as f64))?;
            (D::Laplace { location: median, scale }, vec![], 2)
        }
        FitFamily::Cauchy => {
            let scale = ((sorted[3 * sorted.len() / 4] - sorted[sorted.len() / 4]) * 0.5).max(sd * 0.1);
            (D::Cauchy { location: median, scale }, vec![p(P::Location, low - width, high + width), p(P::Scale, width * 1e-6, width * 10.0)], 2)
        }
        FitFamily::Pareto => {
            positive()?;
            let denominator = finite_sum(values.iter().map(|x| (x / low).ln()))?;
            (D::Pareto { minimum: low, shape: values.len() as f64 / denominator }, vec![], 2)
        }
        FitFamily::Rayleigh => {
            positive()?;
            let scale = high * (finite_sum(values.iter().map(|x| (x / high).powi(2)))? / (2.0 * values.len() as f64)).sqrt();
            (D::Rayleigh { scale }, vec![], 1)
        }
        FitFamily::InverseGaussian => {
            positive()?;
            let denominator = finite_sum(values.iter().map(|x| 1.0 / x - 1.0 / mean))?;
            (D::InverseGaussian { mean, shape: values.len() as f64 / denominator }, vec![], 2)
        }
        FitFamily::Parametric { .. } => return Err(domain("Parametric initializer is inconsistent.")),
    };
    Ok(pair)
}
fn likelihood(d: &D, values: &[f64], control: &dyn RunControl) -> Result<Option<f64>> {
    if validate_distribution(d).is_err() {
        return Ok(None);
    }
    let mut terms = Vec::new();
    reserve(&mut terms, values.len())?;
    for (index, x) in values.iter().enumerate() {
        if index % 256 == 0 {
            cancelled(control)?;
        }
        let value = log_density(d, *x)?;
        if !value.is_finite() {
            return Ok(None);
        }
        terms.push(value);
    }
    Ok(Some(finite_sum(terms)?))
}
fn fit_once(values: &[f64], family: &FitFamily, options: &FitOptions, control: &dyn RunControl) -> Result<FitReport> {
    cancelled(control)?;
    let (mut best, parameters, k) = initial(values, family)?;
    if parameters.len() > 16 {
        return Err(domain("At most 16 fit parameters are supported."));
    }
    for (index, param) in parameters.iter().enumerate() {
        if !param.lower.is_finite() || !param.upper.is_finite() || param.lower >= param.upper || !(param.upper - param.lower).is_finite() {
            return Err(domain("Fit bounds must be finite, distinct, and have finite width."));
        }
        if parameters[..index].iter().any(|p| std::mem::discriminant(&p.field) == std::mem::discriminant(&param.field)) {
            return Err(domain("Fit parameters must be unique."));
        }
        let value = parameter(&mut best, param.field)?;
        if *value < param.lower || *value > param.upper {
            return Err(domain("Initial fit parameter is outside its bounds."));
        }
    }
    if matches!(best, D::Normal { std_dev: 0.0, .. } | D::Lognormal { log_std_dev: 0.0, .. })
        || matches!(&best,D::Uniform{low,high}|D::Triangular{low,high,..}|D::BetaPert{low,high,..} if low==high)
    {
        return Err(domain("Degenerate continuous fits have no finite continuous likelihood for information-criterion ranking."));
    }
    let mut score = likelihood(&best, values, control)?.ok_or_else(|| domain("Initial fit has a non-finite likelihood or invalid domain."))?;
    let mut evaluations = 1;
    let mut steps = parameters.iter().map(|p| (p.upper - p.lower) * 0.25).collect::<Vec<_>>();
    let mut converged = parameters.is_empty();
    while !converged && evaluations < options.max_evaluations {
        let mut improved = false;
        for (index, param) in parameters.iter().enumerate() {
            for direction in [-1.0, 1.0] {
                if evaluations >= options.max_evaluations {
                    break;
                }
                let mut candidate = best.clone();
                let field = parameter(&mut candidate, param.field)?;
                *field = (*field + direction * steps[index]).clamp(param.lower, param.upper);
                evaluations += 1;
                if let Some(value) = likelihood(&candidate, values, control)?
                    && value > score
                {
                    score = value;
                    best = candidate;
                    improved = true;
                }
            }
        }
        if !improved {
            for step in &mut steps {
                *step *= 0.5;
            }
        }
        converged = parameters.iter().zip(&steps).all(|(p, s)| *s <= 1e-7 * (1.0 + p.lower.abs().max(p.upper.abs())));
    }
    let aic = 2.0 * k as f64 - 2.0 * score;
    let bic = (values.len() as f64).ln() * k as f64 - 2.0 * score;
    if !aic.is_finite() || !bic.is_finite() {
        return Err(domain("Information criterion exceeds finite arithmetic."));
    }
    Ok(FitReport {
        distribution: best,
        estimated_parameters: k,
        log_likelihood: score,
        aic,
        bic,
        method: if parameters.is_empty() { "closed_form_likelihood" } else { "bounded_local_likelihood" }.into(),
        converged,
        evaluations,
        goodness_of_fit: None,
    })
}
fn gof(values: &[f64], d: &D, seed: u64, replicate: u64) -> Result<(f64, f64)> {
    let prepared = PreparedDistribution::new(d)?;
    let discrete = is_discrete(d);
    let mut probabilities = Vec::new();
    reserve(&mut probabilities, values.len())?;
    for (i, x) in values.iter().enumerate() {
        let hi = prepared.cdf(*x)?;
        let lo = if discrete { prepared.cdf(x.next_down())? } else { hi };
        probabilities.push(if discrete { lo + (hi - lo) * addressed_uniform(seed, i as u64, replicate, 0x5049_5431) } else { hi });
    }
    probabilities.sort_unstable_by(f64::total_cmp);
    let n = values.len() as f64;
    let clip = 1.0 / (4.0 * n * n);
    let mut ks = 0.0_f64;
    let mut terms = Vec::new();
    reserve(&mut terms, values.len())?;
    for (i, p) in probabilities.iter().enumerate() {
        ks = ks.max(((i + 1) as f64 / n - p).abs()).max((p - i as f64 / n).abs());
        let q = probabilities[probabilities.len() - 1 - i];
        terms.push((2 * i + 1) as f64 * (p.clamp(clip, 1.0 - clip).ln() + (1.0 - q).clamp(clip, 1.0 - clip).ln()));
    }
    Ok((ks, (-n - finite_sum(terms)? / n).max(0.0)))
}
pub fn fit_distribution(values: &[f64], family: &FitFamily, options: &FitOptions, control: &dyn RunControl) -> Result<FitReport> {
    check(values, options)?;
    let mut report = fit_once(values, family, options, control)?;
    if options.bootstrap_replicates > 0 {
        let (ks, ad) = gof(values, &report.distribution, options.seed, u64::MAX)?;
        let prepared = PreparedDistribution::new(&report.distribution)?;
        let mut simulated = Vec::new();
        reserve(&mut simulated, values.len())?;
        let mut ks_greater = 0;
        let mut ad_greater = 0;
        for replicate in 0..options.bootstrap_replicates {
            cancelled(control)?;
            simulated.clear();
            let seed = solvercraft_math::random::addressed_u64(options.seed, u64::from(replicate), 0, 0x4649_5431);
            for i in 0..values.len() {
                if i % 256 == 0 {
                    cancelled(control)?;
                }
                simulated.push(prepared.sample(seed, 0, i as u64)?);
            }
            let fitted = fit_once(&simulated, family, options, control)?;
            let (bks, bad) = gof(&simulated, &fitted.distribution, options.seed, u64::from(replicate))?;
            ks_greater += u32::from(bks >= ks);
            ad_greater += u32::from(bad >= ad);
        }
        report.goodness_of_fit = Some(GoodnessOfFit {
            ks_statistic: ks,
            clipped_ad_statistic: ad,
            ks_bootstrap_p: f64::from(ks_greater + 1) / f64::from(options.bootstrap_replicates + 1),
            ad_bootstrap_p: f64::from(ad_greater + 1) / f64::from(options.bootstrap_replicates + 1),
            bootstrap_replicates: options.bootstrap_replicates,
            refitted_parameters: report.estimated_parameters > 0,
            method: if is_discrete(&report.distribution) {
                "refit_parametric_bootstrap_randomized_pit_ks_clipped_ad"
            } else {
                "refit_parametric_bootstrap_ks_clipped_ad"
            }
            .into(),
        });
    }
    Ok(report)
}
pub fn rank_fits(
    values: &[f64],
    families: &[FitFamily],
    criterion: RankingCriterion,
    options: &FitOptions,
    control: &dyn RunControl,
) -> Result<FitRanking> {
    if families.is_empty() || families.len() > 32 {
        return Err(domain("Ranking requires 1..32 candidate families."));
    }
    check(values, options)?;
    let work = (values.len() as u64)
        .saturating_mul(options.max_evaluations)
        .saturating_mul(u64::from(options.bootstrap_replicates) + 1)
        .saturating_mul(families.len() as u64);
    if work > options.max_work {
        return Err(SolverError::new("work_limit", "All candidate fits exceed aggregate work budget."));
    }
    let mut fits = Vec::new();
    let mut failures = Vec::new();
    reserve(&mut fits, families.len())?;
    reserve(&mut failures, families.len())?;
    for (index, family) in families.iter().enumerate() {
        cancelled(control)?;
        match fit_distribution(values, family, options, control) {
            Ok(report) => fits.push((index, report)),
            Err(error) => {
                if error.code == "cancelled" {
                    return Err(error);
                }
                failures.push(CandidateFailure { index, error });
            }
        }
    }
    if fits.first().is_some_and(|first| fits.iter().any(|fit| is_discrete(&fit.1.distribution) != is_discrete(&first.1.distribution))) {
        return Err(SolverError::new(
            "incomparable_likelihood_measures",
            "AIC/BIC cannot rank counting-measure PMFs together with continuous PDFs. Select one observation measure or supply an explicit binning/measurement model.",
        ));
    }
    fits.sort_by(|a, b| {
        let score = |f: &FitReport| match criterion {
            RankingCriterion::Aic => f.aic,
            RankingCriterion::Bic => f.bic,
        };
        score(&a.1).total_cmp(&score(&b.1)).then(a.0.cmp(&b.0))
    });
    Ok(FitRanking { criterion, fits, failures })
}

#[cfg(test)]
mod strict_input_tests {
    use super::*;
    #[test]
    fn unit_fit_families_reject_unknown_parameters() {
        for kind in [
            "normal",
            "uniform",
            "exponential",
            "lognormal",
            "bernoulli",
            "poisson",
            "geometric",
            "gamma",
            "beta",
            "weibull",
            "laplace",
            "cauchy",
            "pareto",
            "rayleigh",
            "inverse_gaussian",
        ] {
            let extra = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new([("kind", kind), ("std_dev", "1")].into_iter());
            assert!(FitFamily::deserialize(extra).is_err(), "kind{kind}");
            let valid = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new([("kind", kind)].into_iter());
            assert!(FitFamily::deserialize(valid).is_ok());
        }
    }
}
