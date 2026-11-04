//! Typed reference-parameter constructors. These implement mathematical claims,
//! not worksheet defaults, optional property ordering or a vendor seed stream.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution as D, NoControl, Result, RunControl, SolverError};
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Claim {
    Percentile { probability: f64, value: f64 },
    Mean { value: f64 },
    Variance { value: f64 },
    StdDev { value: f64 },
    Location { value: f64 },
    Scale { value: f64 },
    Shape { value: f64 },
    Shape1 { value: f64 },
    Shape2 { value: f64 },
    Lower { value: f64 },
    Upper { value: f64 },
    Mode { value: f64 },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationScaleFamily {
    Normal,
    Uniform,
    Cauchy,
    Logistic,
    Laplace,
    HyperbolicSecant,
    Levy,
    MaxExtreme,
    MinExtreme,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleFamily {
    Erf,
    Exponential,
    Rayleigh,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeFamily {
    ChiSquared,
    StudentT,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeScaleFamily {
    Gamma,
    InverseGamma,
    Pareto,
    Pareto2,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationShapeScaleFamily {
    FatigueLife,
    Frechet,
    LogLogistic,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundedFamily {
    BetaGeneral,
    Triangular,
    Pert,
    BetaPrime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AltOptions {
    pub max_evaluations: u64,
    pub max_iterations: u32,
    pub cdf_tolerance: f64,
}
impl Default for AltOptions {
    fn default() -> Self {
        Self { max_evaluations: 512, max_iterations: 128, cdf_tolerance: 2e-8 }
    }
}
fn err(code: &str, text: &str) -> SolverError {
    SolverError::new(code, text)
}
fn checked(d: D) -> Result<D> {
    crate::validate_distribution(&d)?;
    Ok(d)
}
fn claim_valid(c: Claim) -> Result<()> {
    let value = match c {
        Claim::Percentile { probability, value } => {
            if !probability.is_finite() || probability <= 0.0 || probability >= 1.0 {
                return Err(err("alt_domain", "Percentile probability must lie strictly inside (0,1)."));
            }
            value
        }
        Claim::Mean { value } | Claim::Location { value } | Claim::Lower { value } | Claim::Upper { value } | Claim::Mode { value } => value,
        Claim::Variance { value }
        | Claim::StdDev { value }
        | Claim::Scale { value }
        | Claim::Shape { value }
        | Claim::Shape1 { value }
        | Claim::Shape2 { value } => {
            if value <= 0.0 {
                return Err(err("alt_domain", "Variance and scale claims must be positive."));
            }
            value
        }
    };
    if !value.is_finite() {
        return Err(err("alt_domain", "Parameter claims must be finite."));
    }
    Ok(())
}
fn unit(f: LocationScaleFamily) -> D {
    match f {
        LocationScaleFamily::Normal => D::Normal { mean: 0.0, std_dev: 1.0 },
        LocationScaleFamily::Uniform => D::Uniform { low: 0.0, high: 1.0 },
        LocationScaleFamily::Cauchy => D::Cauchy { location: 0.0, scale: 1.0 },
        LocationScaleFamily::Logistic => D::Logistic { location: 0.0, scale: 1.0 },
        LocationScaleFamily::Laplace => D::Laplace { location: 0.0, scale: 1.0 },
        LocationScaleFamily::HyperbolicSecant => D::HyperbolicSecant { location: 0.0, scale: 1.0 },
        LocationScaleFamily::Levy => D::Levy { location: 0.0, scale: 1.0 },
        LocationScaleFamily::MaxExtreme => D::Gumbel { location: 0.0, scale: 1.0 },
        LocationScaleFamily::MinExtreme => D::MinExtreme { location: 0.0, scale: 1.0 },
    }
}
fn build(f: LocationScaleFamily, location: f64, scale: f64) -> Result<D> {
    checked(match f {
        LocationScaleFamily::Normal => D::Normal { mean: location, std_dev: scale },
        LocationScaleFamily::Uniform => D::Uniform { low: location, high: location + scale },
        LocationScaleFamily::Cauchy => D::Cauchy { location, scale },
        LocationScaleFamily::Logistic => D::Logistic { location, scale },
        LocationScaleFamily::Laplace => D::Laplace { location, scale },
        LocationScaleFamily::HyperbolicSecant => D::HyperbolicSecant { location, scale },
        LocationScaleFamily::Levy => D::Levy { location, scale },
        LocationScaleFamily::MaxExtreme => D::Gumbel { location, scale },
        LocationScaleFamily::MinExtreme => D::MinExtreme { location, scale },
    })
}
fn variance_factor(f: LocationScaleFamily) -> Result<f64> {
    Ok(match f {
        LocationScaleFamily::Normal => 1.0,
        LocationScaleFamily::Uniform => 1.0 / 12.0,
        LocationScaleFamily::Logistic => std::f64::consts::PI.powi(2) / 3.0,
        LocationScaleFamily::Laplace => 2.0,
        LocationScaleFamily::HyperbolicSecant => std::f64::consts::PI.powi(2) / 4.0,
        LocationScaleFamily::MaxExtreme | LocationScaleFamily::MinExtreme => std::f64::consts::PI.powi(2) / 6.0,
        _ => return Err(err("alt_selector", "This family has no finite variance.")),
    })
}
fn affine_claim(f: LocationScaleFamily, c: Claim) -> Result<(f64, f64, f64)> {
    // a*location + b*scale = value; variance/std claims determine scale first.
    Ok(match c {
        Claim::Percentile { probability, value } => (1.0, crate::quantile(&unit(f), probability)?, value),
        Claim::Mean { value } => (
            1.0,
            match f {
                LocationScaleFamily::Uniform => 0.5,
                LocationScaleFamily::MaxExtreme => 0.577_215_664_901_532_9,
                LocationScaleFamily::MinExtreme => -0.577_215_664_901_532_9,
                LocationScaleFamily::Cauchy | LocationScaleFamily::Levy => return Err(err("alt_selector", "This family has no finite mean.")),
                _ => 0.0,
            },
            value,
        ),
        Claim::Mode { value } => (1.0, if matches!(f, LocationScaleFamily::Levy) { 1.0 / 3.0 } else { 0.0 }, value),
        Claim::Location { value } | Claim::Lower { value } => (1.0, 0.0, value),
        Claim::Upper { value } => (1.0, 1.0, value),
        Claim::Scale { value } => (0.0, 1.0, value),
        Claim::Variance { value } => (0.0, 1.0, (value / variance_factor(f)?).sqrt()),
        Claim::StdDev { value } => (0.0, 1.0, value / variance_factor(f)?.sqrt()),
        Claim::Shape { .. } | Claim::Shape1 { .. } | Claim::Shape2 { .. } => {
            return Err(err("alt_selector", "A location-scale family has no shape selector."));
        }
    })
}
fn allowed(f: LocationScaleFamily, c: Claim) -> bool {
    match f {
        LocationScaleFamily::Normal => matches!(c, Claim::Percentile { .. } | Claim::Mean { .. } | Claim::StdDev { .. }),
        LocationScaleFamily::Uniform => matches!(c, Claim::Percentile { .. } | Claim::Lower { .. } | Claim::Upper { .. }),
        LocationScaleFamily::Cauchy => matches!(c, Claim::Percentile { .. }),
        LocationScaleFamily::HyperbolicSecant | LocationScaleFamily::Levy => {
            matches!(c, Claim::Percentile { .. } | Claim::Location { .. } | Claim::Scale { .. })
        }
        LocationScaleFamily::Logistic => matches!(c, Claim::Percentile { .. } | Claim::Mean { .. } | Claim::Variance { .. } | Claim::Scale { .. }),
        LocationScaleFamily::Laplace => matches!(c, Claim::Percentile { .. } | Claim::StdDev { .. }),
        LocationScaleFamily::MaxExtreme | LocationScaleFamily::MinExtreme => {
            matches!(c, Claim::Percentile { .. } | Claim::Mean { .. } | Claim::Variance { .. } | Claim::Mode { .. } | Claim::Scale { .. })
        }
    }
}
/// Solve documented location/scale Alt claims by a 2x2 affine system, with
/// independent original-percentile CDF postchecks. Repeated/dependent claims fail.
pub fn location_scale(f: LocationScaleFamily, first: Claim, second: Claim) -> Result<D> {
    claim_valid(first)?;
    claim_valid(second)?;
    if !allowed(f, first) || !allowed(f, second) {
        return Err(err("alt_selector", "Selector is not documented for this Alt family."));
    }
    if matches!(f, LocationScaleFamily::Normal | LocationScaleFamily::HyperbolicSecant | LocationScaleFamily::Levy | LocationScaleFamily::Laplace)
        && !matches!(first, Claim::Percentile { .. })
        && !matches!(second, Claim::Percentile { .. })
    {
        return Err(err("alt_selector", "This Alt selector combination requires a percentile."));
    }
    let (a, b, y) = affine_claim(f, first)?;
    let (c, d, z) = affine_claim(f, second)?;
    let determinant = a * d - b * c;
    if determinant == 0.0 || !determinant.is_finite() {
        return Err(err("alt_ambiguous", "Claims do not identify a location and scale."));
    }
    let location = (y * d - b * z) / determinant;
    let scale = (a * z - y * c) / determinant;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(err("alt_domain", "Claims imply a nonpositive or unrepresentable scale."));
    }
    let distribution = build(f, location, scale)?;
    check_percentiles(&distribution, &[first, second], 3e-7)?;
    Ok(distribution)
}
fn check_percentiles(d: &D, claims: &[Claim], tolerance: f64) -> Result<()> {
    for c in claims {
        if let Claim::Percentile { probability, value } = *c
            && (crate::cdf(d, value)? - probability).abs() > tolerance
        {
            return Err(err("alt_unverified", "Returned parameters do not preserve the original CDF anchor."));
        }
    }
    Ok(())
}
/// One-parameter Erf/Exponential/Rayleigh Alt claims. Erf's variance is that
/// of Normal(0,1/(sqrt(2)*h)); exponential uses its mean, not a rate argument.
pub fn scale_only(family: ScaleFamily, claim: Claim) -> Result<D> {
    claim_valid(claim)?;
    let base = match family {
        ScaleFamily::Erf => D::Normal { mean: 0.0, std_dev: 1.0 },
        ScaleFamily::Exponential => D::Exponential { rate: 1.0 },
        ScaleFamily::Rayleigh => D::Rayleigh { scale: 1.0 },
    };
    let scale = match claim {
        Claim::Percentile { probability, value } => {
            let q = crate::quantile(&base, probability)?;
            if q == 0.0 {
                return Err(err("alt_ambiguous", "The median zero does not determine a symmetric zero-location scale."));
            }
            value / q
        }
        Claim::Variance { value } => (value
            / match family {
                ScaleFamily::Erf | ScaleFamily::Exponential => 1.0,
                ScaleFamily::Rayleigh => (4.0 - std::f64::consts::PI) / 2.0,
            })
        .sqrt(),
        Claim::Mean { value } if matches!(family, ScaleFamily::Rayleigh) => value / (std::f64::consts::PI / 2.0).sqrt(),
        _ => return Err(err("alt_selector", "Selector is not documented for this one-parameter Alt family.")),
    };
    if !scale.is_finite() || scale <= 0.0 {
        return Err(err("alt_domain", "Claims imply a nonpositive or unrepresentable scale."));
    }
    let d = checked(match family {
        ScaleFamily::Erf => D::Normal { mean: 0.0, std_dev: scale },
        ScaleFamily::Exponential => D::Exponential { rate: 1.0 / scale },
        ScaleFamily::Rayleigh => D::Rayleigh { scale },
    })?;
    check_percentiles(&d, &[claim], 3e-7)?;
    Ok(d)
}
fn options_valid(options: &AltOptions) -> Result<()> {
    if options.max_iterations > 256
        || options.max_evaluations > 100_000
        || !options.cdf_tolerance.is_finite()
        || !(1e-10..=1e-4).contains(&options.cdf_tolerance)
    {
        return Err(err("invalid_options", "Invalid Alt accuracy or bounded-work configuration."));
    }
    Ok(())
}
fn shape_distribution(family: ShapeFamily, shape: f64) -> D {
    match family {
        ShapeFamily::ChiSquared => D::ChiSquared { degrees_of_freedom: shape },
        ShapeFamily::StudentT => D::StudentT { degrees_of_freedom: shape, location: 0.0, scale: 1.0 },
    }
}
/// Bounded one-dimensional shape inversion for PsiChiSquareAlt/PsiStudentAlt.
/// Quantile claims are checked as original CDF equations, never by sampled fits.
pub fn shape_only(family: ShapeFamily, claim: Claim, options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    claim_valid(claim)?;
    options_valid(options)?;
    if control.is_cancelled() {
        return Err(err("cancelled", "Alt inversion cancelled."));
    }
    if let (ShapeFamily::ChiSquared, Claim::Variance { value }) = (family, claim) {
        return checked(shape_distribution(family, value / 2.0));
    }
    let Claim::Percentile { probability, value } = claim else {
        return Err(err("alt_selector", "This shape Alt requires a percentile (or ChiSquare variance)."));
    };
    if matches!(family, ShapeFamily::StudentT) && (value == 0.0 || probability == 0.5) {
        return Err(err("alt_ambiguous", "The standard t median cannot determine degrees of freedom."));
    }
    let mut evaluations = 0;
    let mut residual = |log_shape: f64| -> Result<f64> {
        if control.is_cancelled() {
            return Err(err("cancelled", "Alt inversion cancelled."));
        }
        if evaluations >= options.max_evaluations {
            return Err(err("evaluation_limit", "Alt CDF evaluation budget exhausted."));
        }
        evaluations += 1;
        Ok(crate::cdf(&shape_distribution(family, log_shape.exp().clamp(1e-4, 1e4)), value)? - probability)
    };
    let (mut low, mut high) = (1e-4_f64.ln(), 1e4_f64.ln());
    let mut fl = residual(low)?;
    let fh = residual(high)?;
    if fl * fh > 0.0 {
        return Err(err("alt_shape_range", "No shape root is bracketed within supported degrees of freedom[1e-4,1e4]."));
    }
    for _ in 0..options.max_iterations {
        let mid = 0.5 * low + 0.5 * high;
        let fm = residual(mid)?;
        if fm.abs() <= options.cdf_tolerance {
            let d = checked(shape_distribution(family, mid.exp().clamp(1e-4, 1e4)))?;
            check_percentiles(&d, &[claim], options.cdf_tolerance)?;
            return Ok(d);
        }
        if fm.signum() == fl.signum() {
            low = mid;
            fl = fm;
        } else {
            high = mid;
        }
    }
    Err(err("iteration_limit", "Alt bounded shape inversion did not satisfy its original CDF anchor."))
}
pub fn shape_only_default(family: ShapeFamily, claim: Claim) -> Result<D> {
    shape_only(family, claim, &AltOptions::default(), &NoControl)
}
fn shifted(base: D, location: f64) -> Result<D> {
    checked(D::ShiftScale { base: Box::new(base), shift: location, scale: 1.0 })
}
/// PsiBurr12(loc,scale,shape1,shape2), including its explicit location.
pub fn burr12(location: f64, scale: f64, shape1: f64, shape2: f64) -> Result<D> {
    shifted(D::Burr12 { shape_c: shape1, shape_k: shape2, scale }, location)
}
pub fn dagum(location: f64, scale: f64, shape1: f64, shape2: f64) -> Result<D> {
    shifted(D::Dagum { shape: shape1, power: shape2, scale }, location)
}
pub fn fatigue_life(location: f64, scale: f64, shape: f64) -> Result<D> {
    shifted(D::FatigueLife { shape, scale }, location)
}
pub fn frechet(location: f64, scale: f64, shape: f64) -> Result<D> {
    shifted(D::Frechet { shape, scale }, location)
}
pub fn johnson_sb(shape1: f64, shape2: f64, min: f64, max: f64) -> Result<D> {
    checked(D::JohnsonSb { gamma: shape1, delta: shape2, location: min, scale: max - min })
}
pub fn kumaraswamy(shape1: f64, shape2: f64, min: f64, max: f64) -> Result<D> {
    if !min.is_finite() || !max.is_finite() || min >= max {
        return Err(err("alt_domain", "Kumaraswamy requires finite min<max."));
    }
    checked(D::ShiftScale { base: Box::new(D::Kumaraswamy { alpha: shape1, beta: shape2 }), shift: min, scale: max - min })
}
/// PsiLogLogistic(gamma,beta,alpha): location, scale, shape, in that order.
pub fn log_logistic(location: f64, scale: f64, shape: f64) -> Result<D> {
    shifted(D::LogLogistic { shape, scale }, location)
}
/// PsiPareto(theta,a): shape then minimum, distinct from the DTO field order.
pub fn pareto(shape: f64, minimum: f64) -> Result<D> {
    checked(D::Pareto { minimum, shape })
}
/// NormalSkew's c selects the median's relative position between the stated
/// +/-3-normal-score percentiles. This uses the translation-consistent b-a
/// width, correcting the frozen manual's b-1 typo; no Azzalini alias is used.
pub fn normal_skew(low: f64, high: f64, skew: f64) -> Result<D> {
    if !low.is_finite() || !high.is_finite() || low >= high || !(high - low).is_finite() || !skew.is_finite() || skew.abs() >= 1.0 {
        return Err(err("alt_domain", "NormalSkew requires finite low<high and -1<skew<1."));
    }
    let median = (0.5 * (skew + 1.0)).mul_add(high - low, low);
    crate::elicitation::myerson_percentiles(low, median, high, 0.002_699_796_146_511)
}
fn positive_root<F: FnMut(f64) -> Result<f64>>(mut function: F, options: &AltOptions, control: &dyn RunControl) -> Result<f64> {
    options_valid(options)?;
    let (lo, hi) = (1e-4_f64.ln(), 1e4_f64.ln());
    let mut evaluations = 0;
    let mut evaluate = |x: f64| -> Result<f64> {
        if control.is_cancelled() {
            return Err(err("cancelled", "Alt parameter inversion cancelled."));
        }
        if evaluations >= options.max_evaluations {
            return Err(err("evaluation_limit", "Alt parameter evaluation budget exhausted."));
        }
        evaluations += 1;
        function(x.exp().clamp(1e-4, 1e4))
    };
    let mut previous: Option<(f64, f64)> = None;
    for i in 0..=64 {
        let x = lo + (hi - lo) * (i as f64 / 64.0);
        let fx = match evaluate(x) {
            Ok(value) if value.is_finite() => value,
            Err(e) if e.code == "cancelled" || e.code == "evaluation_limit" => return Err(e),
            _ => {
                previous = None;
                continue;
            }
        };
        if fx.abs() <= options.cdf_tolerance {
            return Ok(x.exp().clamp(1e-4, 1e4));
        }
        if let Some((mut left, mut fl)) = previous
            && fx.signum() != fl.signum()
        {
            let mut right = x;
            for _ in 0..options.max_iterations {
                let mid = 0.5 * left + 0.5 * right;
                let fm = evaluate(mid)?;
                if fm.abs() <= options.cdf_tolerance {
                    return Ok(mid.exp().clamp(1e-4, 1e4));
                }
                if fm.signum() == fl.signum() {
                    left = mid;
                    fl = fm;
                } else {
                    right = mid;
                }
            }
            return Err(err("iteration_limit", "Alt bounded root did not satisfy its original parameter equation."));
        }
        previous = Some((x, fx));
    }
    Err(err("alt_shape_range", "No valid parameter root was bracketed in the supported shape range[1e-4,1e4]."))
}
fn scale_shape_distribution(family: ShapeScaleFamily, shape: f64, scale: f64) -> D {
    match family {
        ShapeScaleFamily::Gamma => D::Gamma { shape, scale },
        ShapeScaleFamily::InverseGamma => D::InverseGamma { shape, scale },
        ShapeScaleFamily::Pareto => D::Pareto { minimum: scale, shape },
        ShapeScaleFamily::Pareto2 => D::Pareto2 { shape, scale },
    }
}
/// Gamma/Pearson5/Pareto/Pareto2 inverse parameterization. Supports two positive
/// percentiles, shape+percentile, scale+percentile, native shape+scale, and the
/// Gamma mean/variance/native combinations. Bounded numerical roots return a
/// matching parameterization, not a proof of globally unique vendor selection.
pub fn shape_scale(family: ShapeScaleFamily, first: Claim, second: Claim, options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    claim_valid(first)?;
    claim_valid(second)?;
    options_valid(options)?;
    if control.is_cancelled() {
        return Err(err("cancelled", "Alt inversion cancelled."));
    }
    let (shape, scale) = match (first, second) {
        (Claim::Shape { value: shape }, Claim::Scale { value: scale }) | (Claim::Scale { value: scale }, Claim::Shape { value: shape }) => {
            (shape, scale)
        }
        (Claim::Shape { value: shape }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Shape { value: shape }) => {
            let q = crate::quantile(&scale_shape_distribution(family, shape, 1.0), probability)?;
            (shape, value / q)
        }
        (Claim::Scale { value: scale }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Scale { value: scale }) => {
            let shape =
                positive_root(|shape| Ok(crate::cdf(&scale_shape_distribution(family, shape, scale), value)? - probability), options, control)?;
            (shape, scale)
        }
        (Claim::Percentile { probability: p1, value: x1 }, Claim::Percentile { probability: p2, value: x2 }) => {
            if p1 == p2 || x1 <= 0.0 || x2 <= 0.0 || (p1 - p2).signum() != (x1 - x2).signum() {
                return Err(err("alt_domain", "Two percentiles must be distinct and increasing positive values."));
            }
            let ratio = x2.ln() - x1.ln();
            let shape = positive_root(
                |shape| {
                    let d = scale_shape_distribution(family, shape, 1.0);
                    let q1 = crate::quantile(&d, p1)?;
                    let q2 = crate::quantile(&d, p2)?;
                    Ok(q2.ln() - q1.ln() - ratio)
                },
                options,
                control,
            )?;
            let q1 = crate::quantile(&scale_shape_distribution(family, shape, 1.0), p1)?;
            (shape, x1 / q1)
        }
        (Claim::Mean { value: mean }, Claim::Variance { value: variance }) | (Claim::Variance { value: variance }, Claim::Mean { value: mean })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            (mean * mean / variance, variance / mean)
        }
        (Claim::Mean { value: mean }, Claim::Shape { value: shape }) | (Claim::Shape { value: shape }, Claim::Mean { value: mean })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            (shape, mean / shape)
        }
        (Claim::Variance { value: variance }, Claim::Shape { value: shape })
        | (Claim::Shape { value: shape }, Claim::Variance { value: variance })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            (shape, (variance / shape).sqrt())
        }
        (Claim::Mean { value: mean }, Claim::Scale { value: scale }) | (Claim::Scale { value: scale }, Claim::Mean { value: mean })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            (mean / scale, scale)
        }
        (Claim::Variance { value: variance }, Claim::Scale { value: scale })
        | (Claim::Scale { value: scale }, Claim::Variance { value: variance })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            (variance / (scale * scale), scale)
        }
        (Claim::Mean { value: mean }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Mean { value: mean })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            let shape = positive_root(|shape| Ok(crate::cdf(&D::Gamma { shape, scale: mean / shape }, value)? - probability), options, control)?;
            (shape, mean / shape)
        }
        (Claim::Variance { value: variance }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Variance { value: variance })
            if matches!(family, ShapeScaleFamily::Gamma) =>
        {
            let shape =
                positive_root(|shape| Ok(crate::cdf(&D::Gamma { shape, scale: (variance / shape).sqrt() }, value)? - probability), options, control)?;
            (shape, (variance / shape).sqrt())
        }
        _ => return Err(err("alt_selector", "Unsupported or dependent shape/scale selectors.")),
    };
    let d = checked(scale_shape_distribution(family, shape, scale))?;
    check_percentiles(&d, &[first, second], 3e-7)?;
    Ok(d)
}
pub fn shape_scale_default(family: ShapeScaleFamily, first: Claim, second: Claim) -> Result<D> {
    shape_scale(family, first, second, &AltOptions::default(), &NoControl)
}
fn three_distribution(family: LocationShapeScaleFamily, location: f64, scale: f64, shape: f64) -> Result<D> {
    match family {
        LocationShapeScaleFamily::FatigueLife => fatigue_life(location, scale, shape),
        LocationShapeScaleFamily::Frechet => frechet(location, scale, shape),
        LocationShapeScaleFamily::LogLogistic => log_logistic(location, scale, shape),
    }
}
/// Three native/percentile claims for FatigueLifeAlt, FrechetAlt and
/// LogLogisticAlt. Supports three percentiles, two percentiles plus one native
/// parameter, one percentile plus two native parameters and native triples.
pub fn location_shape_scale(family: LocationShapeScaleFamily, claims: &[Claim], options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    options_valid(options)?;
    if claims.len() != 3 {
        return Err(err("alt_selector", "This Alt family needs exactly3 parameter claims."));
    }
    let (mut location, mut scale, mut shape) = (None, None, None);
    let mut percentiles = Vec::with_capacity(3);
    for &claim in claims {
        claim_valid(claim)?;
        let target = match claim {
            Claim::Location { value } => {
                if location.replace(value).is_some() {
                    return Err(err("alt_ambiguous", "Repeated location claim."));
                }
                None
            }
            Claim::Scale { value } => {
                if scale.replace(value).is_some() {
                    return Err(err("alt_ambiguous", "Repeated scale claim."));
                }
                None
            }
            Claim::Shape { value } => {
                if shape.replace(value).is_some() {
                    return Err(err("alt_ambiguous", "Repeated shape claim."));
                }
                None
            }
            Claim::Percentile { probability, value } => Some((probability, value)),
            _ => return Err(err("alt_selector", "Use percentile/location/scale/shape claims for this family.")),
        };
        if let Some(point) = target {
            percentiles.push(point);
        }
    }
    percentiles.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    for pair in percentiles.windows(2) {
        if pair[0].0 >= pair[1].0 || pair[0].1 >= pair[1].1 {
            return Err(err("alt_domain", "Percentile claims must be distinct and increasing."));
        }
    }
    if control.is_cancelled() {
        return Err(err("cancelled", "Alt inversion cancelled."));
    }
    let unit_quantile = |a: f64, p: f64| -> Result<f64> { crate::quantile(&three_distribution(family, 0.0, 1.0, a)?, p) };
    let (loc, scl, shp) = match (percentiles.as_slice(), location, scale, shape) {
        ([], Some(l), Some(s), Some(a)) => (l, s, a),
        ([(p, x)], Some(l), Some(s), None) => {
            let a = positive_root(|a| Ok(crate::cdf(&three_distribution(family, l, s, a)?, *x)? - *p), options, control)?;
            (l, s, a)
        }
        ([(p, x)], Some(l), None, Some(a)) => (l, (x - l) / unit_quantile(a, *p)?, a),
        ([(p, x)], None, Some(s), Some(a)) => (x - s * unit_quantile(a, *p)?, s, a),
        ([(p1, x1), (p2, x2)], None, None, Some(a)) => {
            let q1 = unit_quantile(a, *p1)?;
            let q2 = unit_quantile(a, *p2)?;
            let s = (x2 - x1) / (q2 - q1);
            (x1 - s * q1, s, a)
        }
        ([(p1, x1), (p2, x2)], Some(l), None, None) => {
            if *x1 <= l {
                return Err(err("alt_domain", "Positive-support percentile must exceed location."));
            }
            let ratio = (x2 - l).ln() - (x1 - l).ln();
            let a = positive_root(|a| Ok(unit_quantile(a, *p2)?.ln() - unit_quantile(a, *p1)?.ln() - ratio), options, control)?;
            (l, (x1 - l) / unit_quantile(a, *p1)?, a)
        }
        ([(p1, x1), (p2, x2)], None, Some(s), None) => {
            let a = positive_root(|a| Ok((unit_quantile(a, *p2)? - unit_quantile(a, *p1)?) * s / (x2 - x1) - 1.0), options, control)?;
            (x1 - s * unit_quantile(a, *p1)?, s, a)
        }
        ([(p1, x1), (p2, x2), (p3, x3)], None, None, None) => {
            let ratio = (x2 - x1) / (x3 - x1);
            let a = positive_root(
                |a| {
                    let q1 = unit_quantile(a, *p1)?;
                    Ok((unit_quantile(a, *p2)? - q1) / (unit_quantile(a, *p3)? - q1) - ratio)
                },
                options,
                control,
            )?;
            let q1 = unit_quantile(a, *p1)?;
            let s = (x2 - x1) / (unit_quantile(a, *p2)? - q1);
            (x1 - s * q1, s, a)
        }
        _ => return Err(err("alt_ambiguous", "Claims do not identify all three native parameters.")),
    };
    let d = three_distribution(family, loc, scl, shp)?;
    check_percentiles(&d, claims, 3e-7)?;
    Ok(d)
}
pub fn location_shape_scale_default(family: LocationShapeScaleFamily, claims: &[Claim]) -> Result<D> {
    location_shape_scale(family, claims, &AltOptions::default(), &NoControl)
}
/// Inverse-Gaussian Alt: `Scale` names the native lambda parameter. The SDK
/// DTO calls it `shape`; the positive-mean and variance identities are explicit.
pub fn inverse_gaussian(first: Claim, second: Claim, options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    claim_valid(first)?;
    claim_valid(second)?;
    options_valid(options)?;
    if control.is_cancelled() {
        return Err(err("cancelled", "Alt inversion cancelled."));
    }
    let (mean, lambda) = match (first, second) {
        (Claim::Mean { value: m }, Claim::Variance { value: v }) | (Claim::Variance { value: v }, Claim::Mean { value: m }) => (m, m * m * m / v),
        (Claim::Mean { value: m }, Claim::Scale { value: l }) | (Claim::Scale { value: l }, Claim::Mean { value: m }) => (m, l),
        (Claim::Variance { value: v }, Claim::Scale { value: l }) | (Claim::Scale { value: l }, Claim::Variance { value: v }) => ((v * l).cbrt(), l),
        (Claim::Mean { value: m }, Claim::Percentile { probability: p, value: x })
        | (Claim::Percentile { probability: p, value: x }, Claim::Mean { value: m }) => {
            let ratio = positive_root(|r| Ok(crate::cdf(&D::InverseGaussian { mean: m, shape: r * m }, x)? - p), options, control)?;
            (m, ratio * m)
        }
        (Claim::Variance { value: v }, Claim::Percentile { probability: p, value: x })
        | (Claim::Percentile { probability: p, value: x }, Claim::Variance { value: v }) => {
            let ratio = positive_root(
                |r| {
                    let m = (v * r).sqrt();
                    Ok(crate::cdf(&D::InverseGaussian { mean: m, shape: r * m }, x)? - p)
                },
                options,
                control,
            )?;
            let m = (v * ratio).sqrt();
            (m, ratio * m)
        }
        (Claim::Scale { value: l }, Claim::Percentile { probability: p, value: x })
        | (Claim::Percentile { probability: p, value: x }, Claim::Scale { value: l }) => {
            let ratio = positive_root(|r| Ok(crate::cdf(&D::InverseGaussian { mean: l / r, shape: l }, x)? - p), options, control)?;
            (l / ratio, l)
        }
        (Claim::Percentile { probability: p1, value: x1 }, Claim::Percentile { probability: p2, value: x2 }) => {
            if p1 == p2 || x1 <= 0.0 || x2 <= 0.0 || (p1 - p2).signum() != (x1 - x2).signum() {
                return Err(err("alt_domain", "Use distinct increasing positive percentile claims."));
            }
            let target = x2.ln() - x1.ln();
            let ratio = positive_root(
                |r| {
                    let d = D::InverseGaussian { mean: 1.0, shape: r };
                    Ok(crate::quantile(&d, p2)?.ln() - crate::quantile(&d, p1)?.ln() - target)
                },
                options,
                control,
            )?;
            let m = x1 / crate::quantile(&D::InverseGaussian { mean: 1.0, shape: ratio }, p1)?;
            (m, ratio * m)
        }
        _ => return Err(err("alt_selector", "Inverse-Gaussian Alt supports percentile/mean/variance/native lambda(scale).")),
    };
    let d = checked(D::InverseGaussian { mean, shape: lambda })?;
    check_percentiles(&d, &[first, second], 3e-7)?;
    Ok(d)
}
/// Arithmetic-moment LogNormalAlt claims. A percentile plus an arithmetic mean
/// can identify two positive log scales; that ambiguity is explicitly rejected.
/// Percentile+stddev uses a bounded first-bracketed numerical root and reports
/// failure if original CDF or actual arithmetic stddev cannot be preserved.
pub fn lognormal(first: Claim, second: Claim, options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    claim_valid(first)?;
    claim_valid(second)?;
    options_valid(options)?;
    if control.is_cancelled() {
        return Err(err("cancelled", "Alt inversion cancelled."));
    }
    let (mu, sigma) = match (first, second) {
        (Claim::Percentile { probability: p1, value: x1 }, Claim::Percentile { probability: p2, value: x2 }) => {
            let d = crate::parameterization::from_two_quantiles(crate::parameterization::QuantileFamily::Lognormal, (p1, x1), (p2, x2))?;
            if let D::Lognormal { log_mean, log_std_dev } = d {
                (log_mean, log_std_dev)
            } else {
                return Err(err("internal_error", "Lognormal parameter factory returned another family."));
            }
        }
        (Claim::Mean { value: mean }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Mean { value: mean }) => {
            if mean <= 0.0 || value <= 0.0 {
                return Err(err("alt_domain", "Lognormal means/percentiles must be positive."));
            }
            let z = crate::distributions::inverse_normal(probability)?;
            let discr = z * z - 2.0 * (value.ln() - mean.ln());
            if discr < 0.0 || !discr.is_finite() {
                return Err(err("alt_domain", "Lognormal percentile and arithmetic mean are incompatible."));
            }
            let roots = [z - discr.sqrt(), z + discr.sqrt()];
            let mut positive: Vec<_> = roots.into_iter().filter(|s| *s > 0.0).collect();
            positive.dedup();
            if positive.len() != 1 {
                return Err(err("alt_ambiguous", "Percentile and arithmetic mean do not identify one positive log standard deviation."));
            }
            let s = positive[0];
            (value.ln() - s * z, s)
        }
        (Claim::StdDev { value: std }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::StdDev { value: std }) => {
            if value <= 0.0 {
                return Err(err("alt_domain", "Lognormal percentile values must be positive."));
            }
            let z = crate::distributions::inverse_normal(probability)?;
            let s = positive_root(
                |s| {
                    let t = s * s;
                    let log_expm1 = if t > 40.0 { t + (-(-t).exp()).ln_1p() } else { t.exp_m1().ln() };
                    Ok(value.ln() - s * z + 0.5 * t + 0.5 * log_expm1 - std.ln())
                },
                options,
                control,
            )?;
            (value.ln() - s * z, s)
        }
        _ => return Err(err("alt_selector", "Lognormal Alt needs two percentiles or a percentile plus arithmetic mean/stddev.")),
    };
    let d = checked(D::Lognormal { log_mean: mu, log_std_dev: sigma })?;
    check_percentiles(&d, &[first, second], 3e-7)?;
    let t = sigma * sigma;
    let log_expm1 = if t > 40.0 { t + (-(-t).exp()).ln_1p() } else { t.exp_m1().ln() };
    let actual_mean = (mu + 0.5 * t).exp();
    let actual_std = (mu + 0.5 * t + 0.5 * log_expm1).exp();
    for c in [first, second] {
        let pair = match c {
            Claim::Mean { value } => Some((actual_mean, value)),
            Claim::StdDev { value } => Some((actual_std, value)),
            _ => None,
        };
        if let Some((actual, wanted)) = pair
            && (actual / wanted - 1.0).abs() > 3e-7
        {
            return Err(err("alt_unverified", "Actual arithmetic lognormal moment did not preserve its original claim."));
        }
    }
    Ok(d)
}
fn sigmoid(v: f64) -> f64 {
    if v >= 0.0 {
        1.0 / (1.0 + (-v).exp())
    } else {
        let e = v.exp();
        e / (1.0 + e)
    }
}
fn bounded_parameters(f: BoundedFamily, q: &[f64; 4]) -> (f64, f64, f64, f64) {
    if matches!(f, BoundedFamily::BetaPrime) {
        return (0.0, q[2].exp(), q[0].exp(), q[1].exp());
    }
    let (low, width) = (q[0], q[1].exp());
    if matches!(f, BoundedFamily::BetaGeneral) { (low, width, q[2].exp(), q[3].exp()) } else { (low, width, sigmoid(q[2]), 0.0) }
}
fn bounded_distribution(f: BoundedFamily, q: &[f64; 4], origin: f64, span: f64) -> Result<D> {
    let (low, width, a, b) = bounded_parameters(f, q);
    let min = span.mul_add(low, origin);
    let max = span.mul_add(low + width, origin);
    match f {
        BoundedFamily::BetaGeneral => crate::psi_aliases::beta_general(a, b, min, max),
        BoundedFamily::Triangular => checked(D::Triangular { low: min, mode: (a * span).mul_add(width, min), high: max }),
        BoundedFamily::Pert => checked(D::BetaPert { low: min, mode: (a * span).mul_add(width, min), high: max, weight: 4.0 }),
        BoundedFamily::BetaPrime => checked(D::BetaPrime { alpha: a, beta: b, scale: span * width }),
    }
}
fn solve_small(mut matrix: [[f64; 4]; 4], mut rhs: [f64; 4], n: usize) -> Option<[f64; 4]> {
    for col in 0..n {
        let mut pivot = col;
        for row in col + 1..n {
            if matrix[row][col].abs() > matrix[pivot][col].abs() {
                pivot = row;
            }
        }
        if !matrix[pivot][col].is_finite() || matrix[pivot][col].abs() < 1e-14 {
            return None;
        }
        matrix.swap(col, pivot);
        rhs.swap(col, pivot);
        let d = matrix[col][col];
        for value in matrix[col].iter_mut().take(n).skip(col) {
            *value /= d;
        }
        rhs[col] /= d;
        let pivot_values = matrix[col];
        for (row, values) in matrix.iter_mut().enumerate().take(n) {
            if row != col {
                let factor = values[col];
                for (value, pivot) in values.iter_mut().zip(pivot_values).take(n).skip(col) {
                    *value -= factor * pivot;
                }
                rhs[row] -= factor * rhs[col];
            }
        }
    }
    if rhs.iter().take(n).all(|x| x.is_finite()) { Some(rhs) } else { None }
}
fn claim_value(c: Claim) -> f64 {
    match c {
        Claim::Percentile { value, .. }
        | Claim::Mean { value }
        | Claim::Variance { value }
        | Claim::StdDev { value }
        | Claim::Location { value }
        | Claim::Scale { value }
        | Claim::Shape { value }
        | Claim::Shape1 { value }
        | Claim::Shape2 { value }
        | Claim::Lower { value }
        | Claim::Upper { value }
        | Claim::Mode { value } => value,
    }
}
fn verify_native_claims(family: BoundedFamily, d: &D, claims: &[Claim]) -> Result<()> {
    let (low, high, mode, mean, std, shape1, shape2, scale) = match (family, d) {
        (BoundedFamily::BetaGeneral, D::ShiftScale { base, shift, scale }) => {
            let D::Beta { alpha, beta } = base.as_ref() else {
                return Err(err("internal_error", "Beta constructor returned another family."));
            };
            let total = alpha + beta;
            (
                *shift,
                *shift + *scale,
                0.0,
                scale.mul_add(alpha / total, *shift),
                *scale * (alpha * beta / (total * total * (total + 1.0))).sqrt(),
                *alpha,
                *beta,
                *scale,
            )
        }
        (BoundedFamily::Triangular, D::Triangular { low, mode, high }) => {
            let w = high - low;
            let q = if w > 0.0 { (mode - low) / w } else { 0.0 };
            (*low, *high, *mode, w.mul_add((1.0 + q) / 3.0, *low), w * ((1.0 - q + q * q) / 18.0).sqrt(), 0.0, 0.0, w)
        }
        (BoundedFamily::Pert, D::BetaPert { low, mode, high, .. }) => {
            let w = high - low;
            let q = if w > 0.0 { (mode - low) / w } else { 0.0 };
            let a = 1.0 + 4.0 * q;
            let b = 5.0 - 4.0 * q;
            (*low, *high, *mode, w.mul_add((1.0 + 4.0 * q) / 6.0, *low), w * (a * b / 252.0).sqrt(), a, b, w)
        }
        (BoundedFamily::BetaPrime, D::BetaPrime { alpha, beta, scale }) => (0.0, 0.0, 0.0, 0.0, 0.0, *alpha, *beta, *scale),
        _ => return Err(err("internal_error", "Numerical inverse fit returned another family.")),
    };
    for &claim in claims {
        let pair = match claim {
            Claim::Lower { value } => Some((low, value)),
            Claim::Upper { value } => Some((high, value)),
            Claim::Mode { value } => Some((mode, value)),
            Claim::Mean { value } => Some((mean, value)),
            Claim::Variance { value } => {
                if !std.is_finite() || (std / value.sqrt() - 1.0).abs() > 3e-7 {
                    return Err(err("alt_unverified", "Actual returned distribution variance does not preserve its original claim."));
                }
                None
            }
            Claim::Shape1 { value } => Some((shape1, value)),
            Claim::Shape2 { value } => Some((shape2, value)),
            Claim::Scale { value } => Some((scale, value)),
            _ => None,
        };
        if let Some((actual, wanted)) = pair
            && (!actual.is_finite() || (actual - wanted).abs() > 3e-7 * (1.0 + wanted.abs()))
        {
            return Err(err("alt_unverified", "Actual rounded native distribution parameter/moment does not preserve its original claim."));
        }
    }
    Ok(())
}
/// Bounded Levenberg-Marquardt inverse parameter solver for BetaGenAlt (four
/// claims), TriangularAlt and PertAlt (three claims). All original CDF/native
/// claims are postchecked and locally rank-deficient parameterizations rejected.
/// This is a bounded numerical parameter fit, not a global uniqueness proof.
pub fn bounded(family: BoundedFamily, claims: &[Claim], options: &AltOptions, control: &dyn RunControl) -> Result<D> {
    options_valid(options)?;
    let n = if matches!(family, BoundedFamily::BetaGeneral) { 4 } else { 3 };
    if claims.len() != n {
        return Err(err("alt_selector", "BetaGen Alt needs4 claims; Triangular/Pert Alt need3."));
    }
    if matches!(family, BoundedFamily::Triangular) {
        // A single branch has inverse CDF a+A*sqrt(p) or b-B*sqrt(1-p).
        // Three points on one branch leave the other bound and mode
        // unidentified. Prove this from the claims rather than trusting a
        // finite-difference Jacobian near the moving branch boundary.
        let mut points: Vec<_> = claims
            .iter()
            .filter_map(|c| if let Claim::Percentile { probability, value } = c { Some((*probability, *value)) } else { None })
            .collect();
        for &c in claims {
            claim_valid(c)?;
        }
        points.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        if let [(p1, x1), (p2, x2), (p3, x3)] = points.as_slice() {
            if p1 >= p2 || p2 >= p3 || x1 >= x2 || x2 >= x3 {
                return Err(err("alt_domain", "Percentile claims must increase."));
            }
            let ratio = (x2 - x1) / (x3 - x1);
            let left = (p2.sqrt() - p1.sqrt()) / (p3.sqrt() - p1.sqrt());
            let right = ((1.0 - p2).sqrt() - (1.0 - p1).sqrt()) / ((1.0 - p3).sqrt() - (1.0 - p1).sqrt());
            if (ratio - left).abs() <= 1e-10 || (ratio - right).abs() <= 1e-10 {
                return Err(err("alt_ambiguous", "Three triangular percentiles lie on one inverse-CDF branch and do not identify all parameters."));
            }
        }
    }
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for (index, &c) in claims.iter().enumerate() {
        claim_valid(c)?;
        let valid = match family {
            BoundedFamily::BetaGeneral => {
                matches!(c, Claim::Percentile { .. } | Claim::Shape1 { .. } | Claim::Shape2 { .. } | Claim::Lower { .. } | Claim::Upper { .. })
            }
            BoundedFamily::Triangular => matches!(c, Claim::Percentile { .. } | Claim::Lower { .. } | Claim::Mode { .. } | Claim::Upper { .. }),
            BoundedFamily::Pert => matches!(
                c,
                Claim::Percentile { .. }
                    | Claim::Mean { .. }
                    | Claim::Variance { .. }
                    | Claim::Lower { .. }
                    | Claim::Mode { .. }
                    | Claim::Upper { .. }
            ),
            BoundedFamily::BetaPrime => matches!(c, Claim::Percentile { .. } | Claim::Shape1 { .. } | Claim::Shape2 { .. } | Claim::Scale { .. }),
        };
        if !valid {
            return Err(err("alt_selector", "Selector is not documented for this bounded Alt family."));
        }
        for &prior in &claims[..index] {
            if let (Claim::Percentile { probability: p, .. }, Claim::Percentile { probability: r, .. }) = (c, prior) {
                if p == r {
                    return Err(err("alt_ambiguous", "Repeated percentile selector."));
                }
            } else if std::mem::discriminant(&c) == std::mem::discriminant(&prior) {
                return Err(err("alt_ambiguous", "Repeated native selector."));
            }
        }
        if matches!(
            c,
            Claim::Percentile { .. } | Claim::Lower { .. } | Claim::Upper { .. } | Claim::Mean { .. } | Claim::Mode { .. } | Claim::Scale { .. }
        ) {
            low = low.min(claim_value(c));
            high = high.max(claim_value(c));
        }
    }
    if !low.is_finite() || !high.is_finite() {
        return Err(err("alt_ambiguous", "At least one original-value anchor is needed."));
    }
    let span = if matches!(family, BoundedFamily::BetaPrime) {
        high.abs().max(1e-100)
    } else if high > low {
        high - low
    } else {
        1.0_f64.max(low.abs() * 0.1)
    };
    if !span.is_finite() || span <= 0.0 {
        return Err(err("numerical_range", "Bounded Alt value span is unrepresentable."));
    }
    let origin = if matches!(family, BoundedFamily::BetaPrime) { 0.0 } else { low };
    let mut evaluations = 0_u64;
    let mut residual = |q: &[f64; 4]| -> Result<[f64; 4]> {
        if control.is_cancelled() {
            return Err(err("cancelled", "Bounded Alt parameter fit cancelled."));
        }
        if evaluations >= options.max_evaluations {
            return Err(err("evaluation_limit", "Bounded Alt residual evaluation budget exhausted."));
        }
        evaluations += 1;
        let d = bounded_distribution(family, q, 0.0, 1.0)?;
        let (l, w, a, b) = bounded_parameters(family, q);
        let mut result = [0.0; 4];
        for (i, &c) in claims.iter().enumerate() {
            result[i] = match c {
                Claim::Percentile { probability, value } => crate::cdf(&d, (value - origin) / span)? - probability,
                Claim::Lower { value } => l - (value - origin) / span,
                Claim::Upper { value } => l + w - (value - origin) / span,
                Claim::Mode { value } => l + w * a - (value - origin) / span,
                Claim::Mean { value } => l + w * (1.0 + 4.0 * a) / 6.0 - (value - origin) / span,
                Claim::Variance { value } => {
                    let alpha = 1.0 + 4.0 * a;
                    let beta = 5.0 - 4.0 * a;
                    w * (alpha * beta / 252.0).sqrt() - value.sqrt() / span
                }
                Claim::Shape1 { value } => a.ln() - value.ln(),
                Claim::Shape2 { value } => b.ln() - value.ln(),
                Claim::Scale { value } if matches!(family, BoundedFamily::BetaPrime) => w.ln() - (value / span).ln(),
                _ => return Err(err("alt_selector", "Unsupported bounded parameter claim.")),
            };
        }
        if result.iter().all(|x| x.is_finite()) { Ok(result) } else { Err(err("numerical_range", "Bounded Alt residual is not finite.")) }
    };
    let norm = |r: &[f64; 4]| -> f64 { r.iter().take(n).map(|x| x * x).sum() };
    for (start_a, start_b) in [(2.0_f64, 2.0_f64), (2.0, 5.0), (5.0, 2.0), (1.0, 1.0), (5.0, 5.0)] {
        let mut q = [-0.25, 1.5_f64.ln(), if n == 4 { start_a.ln() } else { (start_a / start_b).ln() }, if n == 4 { start_b.ln() } else { 0.0 }];
        if matches!(family, BoundedFamily::BetaPrime) {
            q = [start_a.ln(), start_b.ln(), 0.0, 0.0];
        }
        for &c in claims {
            match c {
                Claim::Lower { value } => q[0] = (value - origin) / span,
                Claim::Shape1 { value } => q[if matches!(family, BoundedFamily::BetaPrime) { 0 } else { 2 }] = value.ln(),
                Claim::Shape2 { value } => q[if matches!(family, BoundedFamily::BetaPrime) { 1 } else { 3 }] = value.ln(),
                Claim::Scale { value } if matches!(family, BoundedFamily::BetaPrime) => q[2] = (value / span).ln(),
                _ => {}
            }
        }
        let mut damping = 1e-5;
        for _ in 0..options.max_iterations {
            let r = residual(&q)?;
            let mut jac = [[0.0; 4]; 4];
            for col in 0..n {
                let h = 1e-5 * (1.0 + q[col].abs());
                let mut left = q;
                let mut right = q;
                left[col] -= h;
                right[col] += h;
                let rl = residual(&left)?;
                let rr = residual(&right)?;
                for row in 0..n {
                    jac[row][col] = (rr[row] - rl[row]) / (2.0 * h);
                }
            }
            let mut normal = [[0.0; 4]; 4];
            let mut gradient = [0.0; 4];
            for j in 0..n {
                for k in 0..n {
                    normal[j][k] = jac.iter().take(n).map(|row| row[j] * row[k]).sum();
                }
                for row in 0..n {
                    gradient[j] -= jac[row][j] * r[row];
                }
            }
            if r.iter().take(n).all(|x| x.abs() <= options.cdf_tolerance) {
                if solve_small(normal, [1.0; 4], n).is_none() {
                    return Err(err("alt_ambiguous", "Original claims are locally rank deficient; no uniquely identified parameters are returned."));
                }
                let d = bounded_distribution(family, &q, origin, span)?;
                check_percentiles(&d, claims, 3e-7)?;
                verify_native_claims(family, &d, claims)?;
                return Ok(d);
            }
            let mut accepted = false;
            for _ in 0..8 {
                let mut matrix = normal;
                for (j, row) in matrix.iter_mut().enumerate().take(n) {
                    row[j] += damping * (1.0 + normal[j][j]);
                }
                let Some(step) = solve_small(matrix, gradient, n) else {
                    damping *= 10.0;
                    continue;
                };
                let mut next = q;
                for j in 0..n {
                    let (lo, hi) = if matches!(family, BoundedFamily::BetaPrime) {
                        if j < 2 { (1e-4_f64.ln(), 1e4_f64.ln()) } else { (-20.0, 20.0) }
                    } else if j == 0 {
                        (-100.0, 100.0)
                    } else if j == 1 {
                        (-20.0, 20.0)
                    } else if n == 4 {
                        (1e-4_f64.ln(), 1e4_f64.ln())
                    } else {
                        (-18.0, 18.0)
                    };
                    next[j] = (q[j] + step[j]).clamp(lo, hi);
                }
                let candidate = match residual(&next) {
                    Ok(r) => r,
                    Err(e) if e.code == "cancelled" || e.code == "evaluation_limit" => return Err(e),
                    Err(_) => {
                        damping *= 10.0;
                        continue;
                    }
                };
                if norm(&candidate) < norm(&r) {
                    q = next;
                    damping = (damping * 0.3).max(1e-12);
                    accepted = true;
                    break;
                }
                damping *= 10.0;
            }
            if !accepted {
                break;
            }
        }
    }
    Err(err("alt_not_converged", "Bounded parameter fit did not satisfy all original claims within its supported numerical domain."))
}
/// Mathematically identified two-claim Weibull inverse parameterization. The
/// frozen/current PSI help incorrectly describes a one-parameter BetaGen; this
/// API does not invent that omitted vendor parameter or expose a PSI alias.
pub fn weibull(first: Claim, second: Claim) -> Result<D> {
    claim_valid(first)?;
    claim_valid(second)?;
    let d = match (first, second) {
        (Claim::Percentile { probability: p1, value: x1 }, Claim::Percentile { probability: p2, value: x2 }) => {
            crate::parameterization::from_two_quantiles(crate::parameterization::QuantileFamily::Weibull, (p1, x1), (p2, x2))?
        }
        (Claim::Shape { value: shape }, Claim::Scale { value: scale }) | (Claim::Scale { value: scale }, Claim::Shape { value: shape }) => {
            checked(D::Weibull { shape, scale })?
        }
        (Claim::Shape { value: shape }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Shape { value: shape }) => {
            checked(D::Weibull { shape, scale: value / (-(-probability).ln_1p()).powf(1.0 / shape) })?
        }
        (Claim::Scale { value: scale }, Claim::Percentile { probability, value })
        | (Claim::Percentile { probability, value }, Claim::Scale { value: scale }) => {
            if value <= 0.0 {
                return Err(err("alt_domain", "Weibull percentile must be positive."));
            }
            let denominator = value.ln() - scale.ln();
            if denominator == 0.0 {
                return Err(err("alt_ambiguous", "This percentile at the native Weibull scale does not identify shape."));
            }
            checked(D::Weibull { shape: (-(-probability).ln_1p()).ln() / denominator, scale })?
        }
        _ => return Err(err("alt_selector", "Use two percentiles or native shape/scale plus a percentile.")),
    };
    check_percentiles(&d, &[first, second], 3e-7)?;
    Ok(d)
}
