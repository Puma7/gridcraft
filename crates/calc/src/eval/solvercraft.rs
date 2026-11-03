//! Deterministic worksheet semantics. Trial sampling belongs to the host's
//! immutable simulation snapshot, never to a mutable global function registry.
use super::{CellError, Ev, Evaluator, Expr, Value, err};
use solvercraft_types::Distribution as D;

/// Canonical, immutable core-signature list shared by decoder and host import.
/// The count is vendor required arguments only. The optional numeric SC nominal
/// override is our extension and is deliberately absent from this catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiCoreSignature {
    pub legacy_name: &'static str,
    pub worksheet_name: &'static str,
    pub required_arguments: usize,
}
pub const PSI_CORE_SIGNATURES: &[PsiCoreSignature] = &[
    PsiCoreSignature { legacy_name: "PsiUniform", worksheet_name: "SC.PSI.UNIFORM", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiIntUniform", worksheet_name: "SC.PSI.INTUNIFORM", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiNormal", worksheet_name: "SC.PSI.NORMAL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiLogNormal", worksheet_name: "SC.PSI.LOGNORMAL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiLogNorm2", worksheet_name: "SC.PSI.LOGNORM2", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiTriangular", worksheet_name: "SC.PSI.TRIANGULAR", required_arguments: 3 },
    PsiCoreSignature { legacy_name: "PsiPert", worksheet_name: "SC.PSI.PERT", required_arguments: 3 },
    PsiCoreSignature { legacy_name: "PsiBeta", worksheet_name: "SC.PSI.BETA", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiGamma", worksheet_name: "SC.PSI.GAMMA", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiExponential", worksheet_name: "SC.PSI.EXPONENTIAL", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiWeibull", worksheet_name: "SC.PSI.WEIBULL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiBernoulli", worksheet_name: "SC.PSI.BERNOULLI", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiBinomial", worksheet_name: "SC.PSI.BINOMIAL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiPoisson", worksheet_name: "SC.PSI.POISSON", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiNegBinomial", worksheet_name: "SC.PSI.NEGBINOMIAL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiGeometric", worksheet_name: "SC.PSI.GEOMETRIC", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiDisUniform", worksheet_name: "SC.PSI.DISUNIFORM", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiDiscrete", worksheet_name: "SC.PSI.DISCRETE", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiCumul", worksheet_name: "SC.PSI.CUMUL", required_arguments: 4 },
    PsiCoreSignature { legacy_name: "PsiLevy", worksheet_name: "SC.PSI.LEVY", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiChiSquare", worksheet_name: "SC.PSI.CHISQUARE", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiErlang", worksheet_name: "SC.PSI.ERLANG", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiFDist", worksheet_name: "SC.PSI.FDIST", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiKumaraswamy", worksheet_name: "SC.PSI.KUMARASWAMY", required_arguments: 4 },
    PsiCoreSignature { legacy_name: "PsiLogistic", worksheet_name: "SC.PSI.LOGISTIC", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiMaxExtreme", worksheet_name: "SC.PSI.MAXEXTREME", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiMinExtreme", worksheet_name: "SC.PSI.MINEXTREME", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiRayleigh", worksheet_name: "SC.PSI.RAYLEIGH", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiReciprocal", worksheet_name: "SC.PSI.RECIPROCAL", required_arguments: 2 },
    PsiCoreSignature { legacy_name: "PsiStudent", worksheet_name: "SC.PSI.STUDENT", required_arguments: 1 },
    PsiCoreSignature { legacy_name: "PsiHypSecant", worksheet_name: "SC.PSI.HYPSECANT", required_arguments: 2 },
];

/// Provenance of an ordinary-cell value; none of these imply vendor display state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorksheetNominalBasis {
    AnalyticMean,
    MedianFallback,
    ExplicitOverride,
}
impl WorksheetNominalBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnalyticMean => "analytic_mean",
            Self::MedianFallback => "median_fallback",
            Self::ExplicitOverride => "explicit_override",
        }
    }
}

/// A worksheet declaration is metadata plus a deterministic ordinary-cell value.
/// Sampling is performed once per declared cell/trial by the host. `nominal` is
/// Solvercraft's explicit ordinary-cell policy: a supplied trailing numeric
/// override, otherwise the SDK's finite analytic mean/labelled median fallback.
/// It is neither a stored simulation sample mean nor legacy PSI ValueToDisplay,
/// PsiStatic, PsiBaseCase or a pre-run random sample.
pub struct WorksheetDistribution {
    pub distribution: D,
    pub nominal: f64,
    pub nominal_basis: WorksheetNominalBasis,
}
pub fn distribution_arity(name: &str) -> Option<usize> {
    if name.starts_with("SC.PSI.") {
        return PSI_CORE_SIGNATURES.iter().find(|s| s.worksheet_name == name).map(|s| s.required_arguments);
    }
    Some(match name {
        "SC.UNIFORM"
        | "SC.DISCRETEUNIFORM"
        | "SC.NORMAL"
        | "SC.LOGNORMAL"
        | "SC.LOGNORMALMEAN"
        | "SC.BETA"
        | "SC.GAMMA"
        | "SC.WEIBULL"
        | "SC.BINOMIAL"
        | "SC.NEGATIVEBINOMIAL"
        | "SC.CATEGORICAL" => 2,
        "SC.TRIANGULAR" => 3,
        "SC.BETAPERT" | "SC.CUMUL" => 4,
        "SC.EXPONENTIAL" | "SC.BERNOULLI" | "SC.POISSON" | "SC.GEOMETRIC" | "SC.EMPIRICAL" | "SC.EMPIRICALCONTINUOUS" => 1,
        _ => return None,
    })
}
fn scalar(value: &Value) -> Result<f64, CellError> {
    if matches!(value,Value::Array(a) if a.data.len()!=1) {
        return Err(CellError::Value);
    }
    let number = value.scalar().to_number()?;
    if number.is_finite() { Ok(number) } else { Err(CellError::Num) }
}
fn vector(value: &Value) -> Result<Vec<f64>, CellError> {
    match value {
        Value::Array(array) if array.data.len() <= 100_000 => array.data.iter().map(scalar).collect(),
        Value::Array(_) => Err(CellError::Calc),
        value => Ok(vec![scalar(value)?]),
    }
}
fn uint(value: f64) -> Result<u32, CellError> {
    if value >= 0.0 && value.fract() == 0.0 && value <= u32::MAX as f64 { Ok(value as u32) } else { Err(CellError::Num) }
}
fn integer(value: f64) -> Result<i64, CellError> {
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 { Ok(value as i64) } else { Err(CellError::Num) }
}
/// Decode the documented SC scalar/range signatures through canonical SDK
/// validation. No state or RNG is consulted here. The required arguments may be
/// followed by one numeric Solvercraft nominal override, including for internal
/// SC.PSI shims. That extra argument is not a legacy PSI property/default;
/// legacy import must validate its original signature separately.
pub fn worksheet_distribution(name: &str, args: &[Value]) -> Result<Option<WorksheetDistribution>, CellError> {
    if let Some(name) = name.strip_prefix("SC.PSI.") {
        return psi_distribution(name, args);
    }
    let Some(count) = distribution_arity(name) else { return Ok(None) };
    if args.len() != count && args.len() != count + 1 {
        return Err(CellError::Value);
    }
    let number = |i: usize| args.get(i).ok_or(CellError::Value).and_then(scalar);
    let list = |i: usize| args.get(i).ok_or(CellError::Value).and_then(vector);
    let distribution = match name {
        "SC.UNIFORM" => D::Uniform { low: number(0)?, high: number(1)? },
        "SC.DISCRETEUNIFORM" => D::DiscreteUniform { low: integer(number(0)?)?, high: integer(number(1)?)? },
        "SC.NORMAL" => D::Normal { mean: number(0)?, std_dev: number(1)? },
        "SC.LOGNORMAL" => D::Lognormal { log_mean: number(0)?, log_std_dev: number(1)? },
        "SC.LOGNORMALMEAN" => solvercraft_math::parameterization::lognormal_from_mean_std(number(0)?, number(1)?).map_err(|_| CellError::Num)?,
        "SC.TRIANGULAR" => D::Triangular { low: number(0)?, mode: number(1)?, high: number(2)? },
        "SC.BETAPERT" => D::BetaPert { low: number(0)?, mode: number(1)?, high: number(2)?, weight: number(3)? },
        "SC.BETA" => D::Beta { alpha: number(0)?, beta: number(1)? },
        "SC.GAMMA" => D::Gamma { shape: number(0)?, scale: number(1)? },
        "SC.EXPONENTIAL" => D::Exponential { rate: number(0)? },
        "SC.WEIBULL" => D::Weibull { shape: number(0)?, scale: number(1)? },
        "SC.BERNOULLI" => D::Bernoulli { probability: number(0)? },
        "SC.BINOMIAL" => D::Binomial { trials: uint(number(0)?)?, probability: number(1)? },
        "SC.POISSON" => D::Poisson { rate: number(0)? },
        "SC.NEGATIVEBINOMIAL" => D::NegativeBinomial { successes: uint(number(0)?)?, probability: number(1)? },
        "SC.GEOMETRIC" => D::Geometric { probability: number(0)? },
        "SC.EMPIRICAL" => D::Empirical { values: list(0)? },
        "SC.EMPIRICALCONTINUOUS" => D::EmpiricalContinuous { values: list(0)? },
        "SC.CATEGORICAL" => D::Categorical { values: list(0)?, weights: list(1)? },
        "SC.CUMUL" => {
            let low = number(0)?;
            let high = number(1)?;
            let knots = list(2)?;
            let probabilities = list(3)?;
            if low >= high
                || knots.len() != probabilities.len()
                || knots.windows(2).any(|p| p[0] >= p[1])
                || probabilities.windows(2).any(|p| p[0] >= p[1])
            {
                return Err(CellError::Num);
            }
            let mut prior_x = low;
            let mut prior_p = 0.0;
            let mut components = Vec::new();
            let mut weights = Vec::new();
            for (x, p) in knots.into_iter().zip(probabilities).chain(std::iter::once((high, 1.0))) {
                if x < low || x > high || !(0.0..=1.0).contains(&p) || x < prior_x || p < prior_p {
                    return Err(CellError::Num);
                }
                if x == prior_x {
                    if p != prior_p {
                        return Err(CellError::Num);
                    }
                    continue;
                }
                if p > prior_p {
                    components.push(D::Uniform { low: prior_x, high: x });
                    weights.push(p - prior_p);
                }
                prior_x = x;
                prior_p = p;
            }
            D::Mixture { components, weights }
        }
        _ => return Err(CellError::Name),
    };
    finish_distribution(distribution, args.get(count))
}
fn finish_distribution(distribution: D, supplied_nominal: Option<&Value>) -> Result<Option<WorksheetDistribution>, CellError> {
    solvercraft_math::validate_distribution(&distribution).map_err(|_| CellError::Num)?;
    let (nominal, nominal_basis) = if let Some(value) = supplied_nominal {
        (scalar(value)?, WorksheetNominalBasis::ExplicitOverride)
    } else {
        let summary = solvercraft_math::nominal::nominal_summary(&distribution).map_err(|_| CellError::Num)?;
        (
            summary.value,
            match summary.statistic {
                solvercraft_math::nominal::NominalStatistic::Mean => WorksheetNominalBasis::AnalyticMean,
                solvercraft_math::nominal::NominalStatistic::MedianFallback => WorksheetNominalBasis::MedianFallback,
            },
        )
    };
    if let D::Uniform { low, high } | D::Triangular { low, high, .. } | D::BetaPert { low, high, .. } = &distribution
        && (nominal < *low || nominal > *high)
    {
        return Err(CellError::Num);
    }
    Ok(Some(WorksheetDistribution { distribution, nominal, nominal_basis }))
}

fn psi_distribution(name: &str, args: &[Value]) -> Result<Option<WorksheetDistribution>, CellError> {
    let Some(count) = distribution_arity(&format!("SC.PSI.{name}")) else { return Ok(None) };
    if args.len() != count && args.len() != count + 1 {
        return Err(CellError::Value);
    }
    let number = |i: usize| args.get(i).ok_or(CellError::Value).and_then(scalar);
    let direct = match name {
        "LEVY" => Some(D::Levy { location: number(0)?, scale: number(1)? }),
        "CHISQUARE" => Some(D::ChiSquared { degrees_of_freedom: f64::from(uint(number(0)?)?) }),
        "ERLANG" => Some(D::Erlang { shape: uint(number(0)?)?, scale: number(1)? }),
        "FDIST" => Some(D::F { numerator_df: f64::from(uint(number(0)?)?), denominator_df: f64::from(uint(number(1)?)?) }),
        "KUMARASWAMY" => {
            let low = number(2)?;
            let high = number(3)?;
            if low >= high {
                return Err(CellError::Num);
            }
            Some(D::ShiftScale { base: Box::new(D::Kumaraswamy { alpha: number(0)?, beta: number(1)? }), shift: low, scale: high - low })
        }
        "LOGISTIC" => Some(D::Logistic { location: number(0)?, scale: number(1)? }),
        "MAXEXTREME" => Some(D::Gumbel { location: number(0)?, scale: number(1)? }),
        "MINEXTREME" => Some(D::MinExtreme { location: number(0)?, scale: number(1)? }),
        "RAYLEIGH" => Some(D::Rayleigh { scale: number(0)? }),
        "RECIPROCAL" => Some(D::Reciprocal { low: number(0)?, high: number(1)? }),
        "STUDENT" => Some(D::StudentT { degrees_of_freedom: f64::from(uint(number(0)?)?), location: 0.0, scale: 1.0 }),
        // Frozen reference285–286 uses beta=standard deviation, whereas the
        // SDK parameter scales the exponent directly: sdk_scale=2*beta/pi.
        "HYPSECANT" => Some(D::HyperbolicSecant { location: number(0)?, scale: number(1)? * std::f64::consts::FRAC_2_PI }),
        _ => None,
    };
    if let Some(distribution) = direct {
        return finish_distribution(distribution, args.get(count));
    }
    let target = match name {
        "UNIFORM" | "INTUNIFORM" => {
            if number(0)? >= number(1)? {
                return Err(CellError::Num);
            }
            if name == "UNIFORM" { "SC.UNIFORM" } else { "SC.DISCRETEUNIFORM" }
        }
        "NORMAL" => {
            if number(1)? <= 0.0 {
                return Err(CellError::Num);
            }
            "SC.NORMAL"
        }
        "LOGNORMAL" => {
            if number(0)? <= 0.0 || number(1)? <= 0.0 {
                return Err(CellError::Num);
            }
            "SC.LOGNORMALMEAN"
        }
        "LOGNORM2" => {
            if number(1)? <= 0.0 {
                return Err(CellError::Num);
            }
            "SC.LOGNORMAL"
        }
        "TRIANGULAR" => {
            if number(0)? >= number(2)? {
                return Err(CellError::Num);
            }
            "SC.TRIANGULAR"
        }
        "PERT" => {
            if number(0)? >= number(1)? || number(1)? >= number(2)? {
                return Err(CellError::Num);
            }
            "SC.BETAPERT"
        }
        "BETA" => "SC.BETA",
        "GAMMA" => "SC.GAMMA",
        "WEIBULL" => "SC.WEIBULL",
        "BERNOULLI" => "SC.BERNOULLI",
        "BINOMIAL" => {
            if number(0)? <= 0.0 {
                return Err(CellError::Num);
            }
            "SC.BINOMIAL"
        }
        "POISSON" => {
            if number(0)? <= 0.0 {
                return Err(CellError::Num);
            }
            "SC.POISSON"
        }
        "NEGBINOMIAL" => "SC.NEGATIVEBINOMIAL",
        "GEOMETRIC" => "SC.GEOMETRIC",
        "DISUNIFORM" => "SC.EMPIRICAL",
        "DISCRETE" => "SC.CATEGORICAL",
        "CUMUL" => "SC.CUMUL",
        "EXPONENTIAL" => "SC.EXPONENTIAL",
        _ => return Ok(None),
    };
    let mut converted = args.to_vec();
    if name == "PERT" {
        converted.insert(3, Value::number(4.0));
    }
    if name == "EXPONENTIAL" {
        let mean = number(0)?;
        if mean <= 0.0 {
            return Err(CellError::Num);
        }
        if let Some(rate) = converted.first_mut() {
            *rate = Value::number(1.0 / mean);
        }
    }
    worksheet_distribution(target, &converted)
}

impl Evaluator<'_> {
    pub(super) fn solvercraft(&mut self, name: &str, args: &[Expr]) -> Ev {
        if name == "SC.OUTPUT" {
            if !(1..=2).contains(&args.len()) {
                return err(CellError::Value);
            }
            return args.first().map(|value| self.eval(value)).unwrap_or_else(|| err(CellError::Value));
        }
        if matches!(name, "SC.MEAN" | "SC.QUANTILE" | "SC.CVAR") {
            return self.solvercraft_stat(name, args);
        }
        let values = args.iter().map(|arg| self.value(arg)).collect::<Vec<_>>();
        match worksheet_distribution(name, &values) {
            Ok(Some(declaration)) => Ev::V(Value::number(declaration.nominal)),
            Ok(None) => err(CellError::Name),
            Err(e) => err(e),
        }
    }
    fn solvercraft_stat(&mut self, name: &str, args: &[Expr]) -> Ev {
        if args.len() != if name == "SC.MEAN" { 1 } else { 2 } {
            return err(CellError::Value);
        }
        let Some(input) = args.first() else { return err(CellError::Value) };
        let value = self.value(input);
        let mut values = Vec::new();
        let mut append = |v: &Value| -> Result<(), CellError> {
            match v {
                Value::Number(n) if n.is_finite() => {
                    values.push(*n);
                    Ok(())
                }
                Value::Empty => Ok(()),
                Value::Error(e) => Err(*e),
                _ => Err(CellError::Value),
            }
        };
        let checked = match &value {
            Value::Array(array) if array.data.len() <= 1_000_000 => array.data.iter().try_for_each(&mut append),
            Value::Array(_) => Err(CellError::Calc),
            value => append(value),
        };
        if let Err(e) = checked {
            return err(e);
        }
        if values.is_empty() {
            return err(CellError::Div0);
        }
        let probability = if name == "SC.MEAN" {
            0.0
        } else {
            match self.arg_num(args, 1, 0.0) {
                Ok(p) if p.is_finite() && (0.0..=1.0).contains(&p) => p,
                Ok(_) => return err(CellError::Num),
                Err(e) => return err(e),
            }
        };
        values.sort_by(f64::total_cmp);
        if name == "SC.QUANTILE" {
            let index = ((probability * values.len() as f64).ceil() as usize).saturating_sub(1).min(values.len() - 1);
            return values.get(index).copied().map(|x| Ev::V(Value::number(x))).unwrap_or_else(|| err(CellError::Calc));
        }
        if probability >= 1.0 {
            return err(CellError::Num);
        }
        let mass = (1.0 - probability) * values.len() as f64;
        let mut remaining = mass;
        let mut sum = 0.0_f64;
        let mut correction = 0.0;
        for value in values.iter().rev() {
            if remaining <= 0.0 {
                break;
            }
            let weight = remaining.min(1.0);
            let term = *value * (weight / mass);
            let next = sum + term;
            correction += if sum.abs() >= term.abs() { (sum - next) + term } else { (term - next) + sum };
            sum = next;
            remaining -= weight;
        }
        let result = sum + correction;
        if result.is_finite() { Ev::V(Value::number(result)) } else { err(CellError::Num) }
    }
}

#[cfg(test)]
mod tests;
