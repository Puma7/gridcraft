//! Deterministic worksheet semantics. Trial sampling belongs to the host's
//! immutable simulation snapshot, never to a mutable global function registry.
use super::{CellError, Ev, Evaluator, Expr, Value, err};
use solvercraft_types::Distribution as D;

/// A worksheet declaration is metadata plus a deterministic ordinary-cell value.
/// Sampling is performed once per declared cell/trial by the host.
pub struct WorksheetDistribution {
    pub distribution: D,
    pub nominal: f64,
}
pub fn distribution_arity(name: &str) -> Option<usize> {
    if let Some(name) = name.strip_prefix("SC.PSI.") {
        return Some(match name {
            "UNIFORM" | "INTUNIFORM" | "NORMAL" | "LOGNORMAL" | "LOGNORM2" | "BETA" | "GAMMA" | "WEIBULL" | "BINOMIAL" | "NEGBINOMIAL"
            | "DISCRETE" => 2,
            "TRIANGULAR" | "PERT" => 3,
            "EXPONENTIAL" | "BERNOULLI" | "POISSON" | "GEOMETRIC" | "DISUNIFORM" => 1,
            "CUMUL" => 4,
            _ => return None,
        });
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
/// validation. No state or RNG is consulted here.
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
    solvercraft_math::validate_distribution(&distribution).map_err(|_| CellError::Num)?;
    let nominal = if let Some(value) = args.get(count) {
        scalar(value)?
    } else {
        solvercraft_math::nominal::nominal_value(&distribution).map_err(|_| CellError::Num)?
    };
    if let D::Uniform { low, high } | D::Triangular { low, high, .. } | D::BetaPert { low, high, .. } = &distribution
        && (nominal < *low || nominal > *high)
    {
        return Err(CellError::Num);
    }
    Ok(Some(WorksheetDistribution { distribution, nominal }))
}
fn psi_distribution(name: &str, args: &[Value]) -> Result<Option<WorksheetDistribution>, CellError> {
    let Some(count) = distribution_arity(&format!("SC.PSI.{name}")) else { return Ok(None) };
    if args.len() != count && args.len() != count + 1 {
        return Err(CellError::Value);
    }
    let number = |i: usize| args.get(i).ok_or(CellError::Value).and_then(scalar);
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
