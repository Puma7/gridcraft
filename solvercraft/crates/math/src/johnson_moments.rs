//! Population-moment inversion for Johnson SN/SL/SU/SB distributions.
//! Clean-room translation/moment methods; no AS99 implementation is copied.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution as D, NoControl, Result, RunControl, SolverError};
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationMoments {
    pub mean: f64,
    pub std_dev: f64,
    pub skewness: f64,
    pub kurtosis: f64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JohnsonFamily {
    Normal,
    Lognormal,
    Su,
    Sb,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JohnsonMomentFit {
    pub distribution: D,
    pub family: JohnsonFamily,
    pub achieved: PopulationMoments,
    pub iterations: u64,
    pub shape_evaluations: u64,
    pub max_relative_residual: f64,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JohnsonOptions {
    pub relative_tolerance: f64,
    pub max_bisection_steps: u32,
    pub max_shape_evaluations: u64,
    pub max_quadrature_nodes: usize,
}
impl Default for JohnsonOptions {
    fn default() -> Self {
        Self { relative_tolerance: 1e-6, max_bisection_steps: 80, max_shape_evaluations: 20_000, max_quadrature_nodes: 50_000 }
    }
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn finite(v: f64) -> Result<f64> {
    if v.is_finite() { Ok(v) } else { Err(error("numerical_range", "Johnson moments are not representable.")) }
}
fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
fn sigmoid_difference(a: f64, b: f64) -> f64 {
    if a <= 0.0 && b <= 0.0 {
        let eb = b.exp();
        eb * (a - b).exp_m1() / ((1.0 + a.exp()) * (1.0 + eb))
    } else if a >= 0.0 && b >= 0.0 {
        let eb = (-b).exp();
        -eb * (b - a).exp_m1() / ((1.0 + (-a).exp()) * (1.0 + eb))
    } else {
        sigmoid(a) - sigmoid(b)
    }
}
#[derive(Clone, Copy)]
struct Shape {
    mean: f64,
    std: f64,
    skew: f64,
    kurt: f64,
}
fn su_shape(w: f64, r: f64) -> Result<Shape> {
    if !w.is_finite() || w <= 1.0 || w > 1e6 || !r.is_finite() || r.abs() >= 1.0 {
        return Err(error("johnson_shape_range", "SU shape is outside the supported finite range."));
    }
    let q = r * r;
    let b = w + 1.0 + (w - 1.0) * q;
    let c = 0.75 * (w + 1.0).powi(2) * (1.0 - q) + w * (w + 2.0) * q;
    let skew = (w * (w - 1.0)).sqrt() * r * c / (0.5 * b).powf(1.5);
    let a0 = (((((w + 2.0) * w + 3.0) * w + 4.0) * w + 5.0) * w + 6.0) * w + 3.0;
    let a2 = (((w + 2.0) * w + 3.0) * w + 1.0) * w - 1.0;
    let a4 = (((w + 2.0) * w + 3.0) * w) * w - 3.0;
    let kurt = (0.5 * a0 * (1.0 - q).powi(2) + 4.0 * w * w * a2 * q * (1.0 - q) + 4.0 * w * w * a4 * q * q) / (b * b);
    let h = r / (1.0 - q).sqrt();
    let mean = w.sqrt() * h;
    let variance = 0.5 * (w - 1.0) * (w + 1.0 + 2.0 * w * h * h);
    Ok(Shape { mean: finite(mean)?, std: finite(variance.sqrt())?, skew: finite(skew)?, kurt: finite(kurt)? })
}
struct Segment {
    a: f64,
    b: f64,
    fa: [f64; 5],
    fm: [f64; 5],
    fb: [f64; 5],
    whole: [f64; 5],
    depth: u32,
}
fn simpson(a: f64, b: f64, fa: [f64; 5], fm: [f64; 5], fb: [f64; 5]) -> [f64; 5] {
    let mut out = [0.0; 5];
    for i in 0..5 {
        out[i] = (b - a) / 6.0 * (fa[i] + 4.0 * fm[i] + fb[i]);
    }
    out
}
fn sb_integrand(z: f64, m: f64, s: f64, scale: f64) -> Result<[f64; 5]> {
    let density = (-0.5 * z * z - 0.918_938_533_204_672_7).exp();
    let d = sigmoid_difference(m + s * z, m) / scale;
    let mut out = [0.0; 5];
    out[0] = density;
    for k in 1..5 {
        out[k] = finite(out[k - 1] * d)?;
    }
    Ok(out)
}
fn sb_shape(m: f64, s: f64, tolerance: f64, node_cap: usize, control: &dyn RunControl) -> Result<Shape> {
    if !m.is_finite() || m.abs() > 40.0 || !s.is_finite() || !(1e-5..=128.0).contains(&s) {
        return Err(error("johnson_shape_range", "SB logit-normal shape is outside m magnitude40 and s[1e-5,128]."));
    }
    // Reflect internally, so the reference median never rounds to one.
    if m > 0.0 {
        let mut a = sb_shape(-m, s, tolerance, node_cap, control)?;
        a.mean = 1.0 - a.mean;
        a.skew = -a.skew;
        return Ok(a);
    }
    let origin = sigmoid(m);
    let scale = origin * (1.0 - origin) * s.min(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(error("numerical_range", "SB central-moment scale is unrepresentable."));
    }
    // Gaussian Mills bound times max|centered/scaled Y|^4 makes omitted
    // population tails <exp(-40), independent of empirical draws.
    let limit = (2.0 * (4.0 * (-scale.ln()).max(0.0) + 40.0)).sqrt().max(12.0) + 1.0;
    if limit > 40.0 {
        return Err(error("johnson_shape_range", "SB quadrature tail bound exceeds its supported range."));
    }
    let mut edges = Vec::new();
    let bins = (2.0 * limit / 0.5).ceil() as usize;
    for i in 0..=bins {
        edges.push(-limit + 2.0 * limit * (i as f64 / bins as f64));
    }
    edges.push(0.0);
    for shift in [-8.0, -4.0, -1.0, 0.0, 1.0, 4.0, 8.0] {
        let x = (-m + shift) / s;
        if x > -limit && x < limit {
            edges.push(x);
        }
    }
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let mut total = [0.0; 5];
    let mut correction = [0.0; 5];
    let mut nodes = 0usize;
    for edge in edges.windows(2) {
        if control.is_cancelled() {
            return Err(error("cancelled", "Johnson moment fitting cancelled."));
        }
        let (a, b) = (edge[0], edge[1]);
        let middle = 0.5 * a + 0.5 * b;
        let fa = sb_integrand(a, m, s, scale)?;
        let fm = sb_integrand(middle, m, s, scale)?;
        let fb = sb_integrand(b, m, s, scale)?;
        nodes += 3;
        let mut stack = vec![Segment { a, b, fa, fm, fb, whole: simpson(a, b, fa, fm, fb), depth: 0 }];
        while let Some(part) = stack.pop() {
            if nodes % 256 < 2 && control.is_cancelled() {
                return Err(error("cancelled", "Johnson population integration cancelled."));
            }
            if nodes.saturating_add(2) > node_cap {
                return Err(error("quadrature_limit", "SB population quadrature node budget exhausted."));
            }
            let mid = 0.5 * part.a + 0.5 * part.b;
            let lm = 0.5 * part.a + 0.5 * mid;
            let rm = 0.5 * mid + 0.5 * part.b;
            let fl = sb_integrand(lm, m, s, scale)?;
            let fr = sb_integrand(rm, m, s, scale)?;
            nodes += 2;
            let left = simpson(part.a, mid, part.fa, fl, part.fm);
            let right = simpson(mid, part.b, part.fm, fr, part.fb);
            let local = tolerance * (part.b - part.a) / (2.0 * limit);
            let ok = (0..5).all(|i| (left[i] + right[i] - part.whole[i]).abs() <= 15.0 * (local + tolerance * (left[i] + right[i]).abs()));
            if ok {
                for i in 0..5 {
                    let value = left[i] + right[i] + (left[i] + right[i] - part.whole[i]) / 15.0;
                    let y = value - correction[i];
                    let next = finite(total[i] + y)?;
                    correction[i] = (next - total[i]) - y;
                    total[i] = next;
                }
            } else {
                if part.depth >= 24 {
                    return Err(error("quadrature_limit", "SB quadrature did not converge at bounded recursion depth."));
                }
                stack.push(Segment { a: mid, b: part.b, fa: part.fm, fm: fr, fb: part.fb, whole: right, depth: part.depth + 1 });
                stack.push(Segment { a: part.a, b: mid, fa: part.fa, fm: fl, fb: part.fm, whole: left, depth: part.depth + 1 });
            }
        }
    }
    if total[0] <= 0.0 {
        return Err(error("numerical_range", "SB quadrature has no finite Gaussian mass."));
    }
    let mean = total[1] / total[0];
    let r2 = total[2] / total[0];
    let r3 = total[3] / total[0];
    let r4 = total[4] / total[0];
    let variance = r2 - mean * mean;
    if variance <= 0.0 {
        return Err(error("numerical_range", "SB quadrature variance is numerically unresolved."));
    }
    let skew = (r3 - 3.0 * mean * r2 + 2.0 * mean.powi(3)) / variance.powf(1.5);
    let kurt = (r4 - 4.0 * mean * r3 + 6.0 * mean * mean * r2 - 3.0 * mean.powi(4)) / (variance * variance);
    Ok(Shape { mean: finite(origin + scale * mean)?, std: finite(scale * variance.sqrt())?, skew: finite(skew)?, kurt: finite(kurt)? })
}
fn transform(shape: Shape, location: f64, scale: f64) -> Result<PopulationMoments> {
    Ok(PopulationMoments {
        mean: finite(scale.mul_add(shape.mean, location))?,
        std_dev: finite(scale.abs() * shape.std)?,
        skewness: shape.skew * scale.signum(),
        kurtosis: shape.kurt,
    })
}
fn moments_inner(d: &D, depth: usize, node_cap: usize, control: &dyn RunControl) -> Result<PopulationMoments> {
    if depth > 16 {
        return Err(error("distribution_limit", "Moment composition depth exceeded."));
    }
    match d {
        D::Normal { mean, std_dev } if *std_dev > 0.0 => Ok(PopulationMoments { mean: *mean, std_dev: *std_dev, skewness: 0.0, kurtosis: 3.0 }),
        D::Lognormal { log_mean, log_std_dev } if *log_std_dev > 0.0 => {
            let w = finite((log_std_dev * log_std_dev).exp_m1())?;
            let mean = finite((log_mean + 0.5 * log_std_dev * log_std_dev).exp())?;
            Ok(PopulationMoments {
                mean,
                std_dev: finite(mean * w.sqrt())?,
                skewness: finite(w.sqrt() * (w + 3.0))?,
                kurtosis: finite(3.0 + w * (16.0 + w * (15.0 + w * (6.0 + w))))?,
            })
        }
        D::JohnsonSu { gamma, delta, location, scale } => {
            let s = 1.0 / delta;
            let shape = su_shape((s * s).exp(), (-gamma / delta).tanh())?;
            transform(shape, *location, *scale)
        }
        D::JohnsonSb { gamma, delta, location, scale } => {
            let shape = sb_shape(-gamma / delta, 1.0 / delta, 2e-12, node_cap, control)?;
            transform(shape, *location, *scale)
        }
        D::JohnsonSl { gamma, delta, location, scale } => {
            let base = D::Lognormal { log_mean: -gamma / delta, log_std_dev: 1.0 / delta };
            let a = moments_inner(&base, depth + 1, node_cap, control)?;
            transform(Shape { mean: a.mean, std: a.std_dev, skew: a.skewness, kurt: a.kurtosis }, *location, *scale)
        }
        D::ShiftScale { base, shift, scale } => {
            let a = moments_inner(base, depth + 1, node_cap, control)?;
            transform(Shape { mean: a.mean, std: a.std_dev, skew: a.skewness, kurt: a.kurtosis }, *shift, *scale)
        }
        _ => Err(error("moment_family", "Population moments are implemented for normal/lognormal and Johnson SU/SB/SL with affine compositions.")),
    }
}
/// Deterministic population moments, using Gaussian integration for SB, never
/// random empirical moments. Pearson kurtosis is reported, not excess kurtosis.
pub fn population_moments(d: &D) -> Result<PopulationMoments> {
    crate::validate_distribution(d)?;
    moments_inner(d, 0, 100_000, &NoControl)
}

struct Work<'a> {
    options: &'a JohnsonOptions,
    control: &'a dyn RunControl,
    evaluations: u64,
    iterations: u64,
}
impl Work<'_> {
    fn charge(&mut self) -> Result<()> {
        if self.control.is_cancelled() {
            return Err(error("cancelled", "Johnson fitting cancelled."));
        }
        if self.evaluations >= self.options.max_shape_evaluations {
            return Err(error("evaluation_limit", "Johnson shape evaluation budget exhausted."));
        }
        self.evaluations += 1;
        Ok(())
    }
    fn su_at(&mut self, w: f64, skew: f64) -> Result<(f64, Shape)> {
        if skew == 0.0 {
            self.charge()?;
            return Ok((0.0, su_shape(w, 0.0)?));
        }
        let (mut lo, mut hi) = (0.0, 1.0_f64.next_down());
        for _ in 0..self.options.max_bisection_steps {
            let r = 0.5 * lo + 0.5 * hi;
            self.charge()?;
            let a = su_shape(w, r)?;
            self.iterations += 1;
            if (a.skew - skew).abs() <= self.options.relative_tolerance * 0.001 * (1.0 + skew) {
                return Ok((r, a));
            }
            if a.skew < skew {
                lo = r;
            } else {
                hi = r;
            }
        }
        let r = 0.5 * lo + 0.5 * hi;
        self.charge()?;
        Ok((r, su_shape(w, r)?))
    }
    fn sb_at(&mut self, s: f64, skew: f64) -> Result<(f64, Shape)> {
        if skew == 0.0 {
            self.charge()?;
            return Ok((0.0, sb_shape(0.0, s, 2e-10, self.options.max_quadrature_nodes, self.control)?));
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        loop {
            self.charge()?;
            let a = sb_shape(-hi, s, 2e-10, self.options.max_quadrature_nodes, self.control)?;
            if a.skew >= skew {
                break;
            }
            if hi == 40.0 {
                return Err(error("johnson_shape_range", "SB asymmetry cannot be bracketed within the supported range."));
            }
            lo = hi;
            hi = (hi * 2.0).min(40.0);
        }
        for _ in 0..self.options.max_bisection_steps {
            let u = 0.5 * lo + 0.5 * hi;
            self.charge()?;
            let a = sb_shape(-u, s, 2e-10, self.options.max_quadrature_nodes, self.control)?;
            self.iterations += 1;
            if (a.skew - skew).abs() <= self.options.relative_tolerance * 0.001 * (1.0 + skew) {
                return Ok((u, a));
            }
            if a.skew < skew {
                lo = u;
            } else {
                hi = u;
            }
        }
        let u = 0.5 * lo + 0.5 * hi;
        self.charge()?;
        Ok((u, sb_shape(-u, s, 2e-11, self.options.max_quadrature_nodes, self.control)?))
    }
}
/// Fit all four Johnson families by population moments within explicit numeric
/// domains and postcheck the actual rounded returned distribution.
pub fn fit(mean: f64, std_dev: f64, skewness: f64, kurtosis: f64) -> Result<JohnsonMomentFit> {
    fit_with_options(mean, std_dev, skewness, kurtosis, &JohnsonOptions::default(), &NoControl)
}
pub fn fit_with_options(
    mean: f64,
    std_dev: f64,
    skewness: f64,
    kurtosis: f64,
    options: &JohnsonOptions,
    control: &dyn RunControl,
) -> Result<JohnsonMomentFit> {
    if !options.relative_tolerance.is_finite()
        || !(1e-9..=1e-3).contains(&options.relative_tolerance)
        || options.max_bisection_steps > 200
        || options.max_quadrature_nodes > 500_000
    {
        return Err(error("invalid_options", "Invalid Johnson accuracy or resource configuration."));
    }
    if !mean.is_finite()
        || !std_dev.is_finite()
        || std_dev <= 0.0
        || !skewness.is_finite()
        || skewness.abs() > 1e4
        || !kurtosis.is_finite()
        || kurtosis > 1e10
    {
        return Err(error("moment_domain", "Require finite mean, positive stddev, skew magnitude<=10000 and Pearson kurtosis<=1e10."));
    }
    if kurtosis <= 1.0 + skewness * skewness {
        return Err(error("infeasible_moments", "Continuous population moments require Pearson kurtosis>1+skewness^2."));
    }
    let mut work = Work { options, control, evaluations: 0, iterations: 0 };
    work.charge()?;
    let requested = PopulationMoments { mean, std_dev, skewness, kurtosis };
    let v = 2.0 * ((0.5 * skewness.abs()).asinh() / 3.0).sinh();
    let q = v * v;
    let boundary = 3.0 + q * (16.0 + q * (15.0 + q * (6.0 + q)));
    let boundary_tolerance = options.relative_tolerance * 0.05 * (1.0 + kurtosis);
    let (distribution, family) = if skewness == 0.0 && (kurtosis - 3.0).abs() <= boundary_tolerance {
        (D::Normal { mean, std_dev }, JohnsonFamily::Normal)
    } else if (kurtosis - boundary).abs() <= boundary_tolerance.min(5e-9 * (1.0 + kurtosis)) {
        (crate::metalog::moment_fit_supported(mean, std_dev, skewness, kurtosis)?.distribution, JohnsonFamily::Lognormal)
    } else if kurtosis > boundary {
        let (mut lo, mut hi) = ((1.0 + q).max(1.0 + 1e-12), (1.0 + q).max(1.0 + 1e-12) * 1.2 + 0.1);
        loop {
            let (_, a) = work.su_at(hi, skewness.abs())?;
            if a.kurt >= kurtosis {
                break;
            }
            lo = hi;
            hi *= 2.0;
            if hi > 1e6 {
                return Err(error("johnson_shape_range", "SU shape cannot be bracketed."));
            }
        }
        for _ in 0..options.max_bisection_steps {
            let w = 0.5 * lo + 0.5 * hi;
            let (_, a) = work.su_at(w, skewness.abs())?;
            work.iterations += 1;
            if (a.kurt - kurtosis).abs() <= options.relative_tolerance * 0.005 * (1.0 + kurtosis) {
                lo = w;
                hi = w;
                break;
            }
            if a.kurt < kurtosis {
                lo = w;
            } else {
                hi = w;
            }
        }
        let w = 0.5 * lo + 0.5 * hi;
        let (r, mut a) = work.su_at(w, skewness.abs())?;
        let sign = if skewness < 0.0 { -1.0 } else { 1.0 };
        a.mean *= sign;
        a.skew *= sign;
        let s = w.ln().sqrt();
        let m = sign * r.atanh();
        let scale = finite(std_dev / a.std)?;
        let location = finite(mean - scale * a.mean)?;
        (D::JohnsonSu { gamma: -m / s, delta: 1.0 / s, location, scale }, JohnsonFamily::Su)
    } else {
        let (mut lo, mut hi) = ((1.0 + q).ln().sqrt().max(1e-5), (2.0 * (1.0 + q).ln().sqrt()).max(1.0));
        loop {
            let (_, a) = work.sb_at(hi, skewness.abs())?;
            if a.kurt <= kurtosis {
                break;
            }
            lo = hi;
            hi *= 2.0;
            if hi > 128.0 {
                return Err(error("johnson_shape_range", "SB shape cannot be bracketed before the bounded solver limit."));
            }
        }
        for _ in 0..options.max_bisection_steps {
            let s = 0.5 * lo + 0.5 * hi;
            let (_, a) = work.sb_at(s, skewness.abs())?;
            work.iterations += 1;
            if (a.kurt - kurtosis).abs() <= options.relative_tolerance * 0.005 * (1.0 + kurtosis) {
                lo = s;
                hi = s;
                break;
            }
            if a.kurt > kurtosis {
                lo = s;
            } else {
                hi = s;
            }
        }
        let s = 0.5 * lo + 0.5 * hi;
        let (u, a) = work.sb_at(s, skewness.abs())?;
        let scale = finite(std_dev / a.std)?;
        if skewness >= 0.0 {
            (D::JohnsonSb { gamma: u / s, delta: 1.0 / s, location: finite(mean - scale * a.mean)?, scale }, JohnsonFamily::Sb)
        } else {
            (
                D::ShiftScale {
                    base: Box::new(D::JohnsonSb { gamma: u / s, delta: 1.0 / s, location: 0.0, scale: 1.0 }),
                    shift: finite(mean + scale * a.mean)?,
                    scale: -scale,
                },
                JohnsonFamily::Sb,
            )
        }
    };
    crate::validate_distribution(&distribution)?;
    work.charge()?;
    let achieved = moments_inner(&distribution, 0, options.max_quadrature_nodes, control)?;
    let residual = ((achieved.mean - requested.mean).abs() / (1.0 + requested.mean.abs()))
        .max((achieved.std_dev / requested.std_dev - 1.0).abs())
        .max((achieved.skewness - requested.skewness).abs() / (1.0 + requested.skewness.abs()))
        .max((achieved.kurtosis - requested.kurtosis).abs() / (1.0 + requested.kurtosis));
    if !residual.is_finite() || residual > options.relative_tolerance {
        return Err(error("moment_fit_unverified", "Returned rounded Johnson parameters did not pass requested population-moment checks."));
    }
    Ok(JohnsonMomentFit {
        distribution,
        family,
        achieved,
        iterations: work.iterations,
        shape_evaluations: work.evaluations,
        max_relative_residual: residual,
        method:
            "clean-room Johnson population moment inversion; nested bisection, analytic SU and adaptive Gaussian SB integration; no empirical samples"
                .into(),
    })
}
