//! Explicit copula semantics: Gaussian matrices are latent Pearson correlations.
use crate::{distributions::inverse_normal, extended::student_cdf, special};
use serde::{Deserialize, Serialize};
use solvercraft_types::{Distribution, Result, SolverError};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reflection {
    #[default]
    None,
    First,
    Second,
    Both,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DependenceSpec {
    #[default]
    Independent,
    Gaussian {
        correlation: Vec<Vec<f64>>,
    },
    GaussianSpearman {
        rank_correlation: Vec<Vec<f64>>,
    },
    StudentT {
        correlation: Vec<Vec<f64>>,
        degrees_of_freedom: f64,
    },
    Clayton {
        theta: f64,
        reflection: Reflection,
    },
    Frank {
        theta: f64,
        reflection: Reflection,
    },
    Gumbel {
        theta: f64,
        reflection: Reflection,
    },
}
impl<'de> Deserialize<'de> for DependenceSpec {
    fn deserialize<T: serde::Deserializer<'de>>(deserializer: T) -> std::result::Result<Self, T::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Strict {
            Independent {},
            Gaussian { correlation: Vec<Vec<f64>> },
            GaussianSpearman { rank_correlation: Vec<Vec<f64>> },
            StudentT { correlation: Vec<Vec<f64>>, degrees_of_freedom: f64 },
            Clayton { theta: f64, reflection: Reflection },
            Frank { theta: f64, reflection: Reflection },
            Gumbel { theta: f64, reflection: Reflection },
        }
        Ok(match Strict::deserialize(deserializer)? {
            Strict::Independent {} => Self::Independent,
            Strict::Gaussian { correlation } => Self::Gaussian { correlation },
            Strict::GaussianSpearman { rank_correlation } => Self::GaussianSpearman { rank_correlation },
            Strict::StudentT { correlation, degrees_of_freedom } => Self::StudentT { correlation, degrees_of_freedom },
            Strict::Clayton { theta, reflection } => Self::Clayton { theta, reflection },
            Strict::Frank { theta, reflection } => Self::Frank { theta, reflection },
            Strict::Gumbel { theta, reflection } => Self::Gumbel { theta, reflection },
        })
    }
}
#[derive(Debug, Clone)]
pub struct PreparedCopula {
    spec: DependenceSpec,
    dimensions: usize,
    lower: Vec<Vec<f64>>,
    chi: Option<crate::PreparedDistribution>,
    frailty: Option<crate::PreparedDistribution>,
}
fn domain() -> SolverError {
    SolverError::new("copula_domain", "Invalid copula parameters, dimensions, or non-PSD correlation matrix.")
}
fn open(x: f64) -> f64 {
    x.clamp(f64::from_bits(1), 1.0_f64.next_down())
}
pub(crate) fn cholesky(matrix: &[Vec<f64>], n: usize, spearman: bool) -> Result<Vec<Vec<f64>>> {
    if matrix.len() != n || matrix.iter().any(|row| row.len() != n || row.iter().any(|x| !x.is_finite())) {
        return Err(domain());
    }
    let mut lower = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let a = matrix[i][j];
            if !a.is_finite() || a.abs() > 1.0 || (a - matrix[j][i]).abs() > 1e-12 || (i == j && (a - 1.0).abs() > 1e-12) {
                return Err(domain());
            }
            let a = if spearman && i != j { 2.0 * (std::f64::consts::PI * a / 6.0).sin() } else { a };
            let value = special::finite_sum(std::iter::once(a).chain((0..j).map(|k| -lower[i][k] * lower[j][k])))?;
            if i == j {
                if value < -1e-12 {
                    return Err(domain());
                }
                lower[i][j] = value.max(0.0).sqrt();
            } else if lower[j][j] > 1e-12 {
                lower[i][j] = value / lower[j][j];
            } else if value.abs() > 1e-10 {
                return Err(domain());
            }
        }
    }
    Ok(lower)
}
impl PreparedCopula {
    pub fn new(spec: &DependenceSpec, dimensions: usize) -> Result<Self> {
        if dimensions > 4096 || (dimensions > 128 && !matches!(spec, DependenceSpec::Independent)) {
            return Err(SolverError::new("copula_dimension", "Dependence supports at most 128 dimensions; independent sampling supports 4096."));
        }
        let mut lower = Vec::new();
        let mut chi = None;
        let mut frailty = None;
        match spec {
            DependenceSpec::Independent => (),
            DependenceSpec::Gaussian { correlation } => lower = cholesky(correlation, dimensions, false)?,
            DependenceSpec::GaussianSpearman { rank_correlation } => lower = cholesky(rank_correlation, dimensions, true)?,
            DependenceSpec::StudentT { correlation, degrees_of_freedom } => {
                if !(1e-3..=1e4).contains(degrees_of_freedom) {
                    return Err(domain());
                }
                lower = cholesky(correlation, dimensions, false)?;
                chi = Some(crate::PreparedDistribution::new(&Distribution::ChiSquared { degrees_of_freedom: *degrees_of_freedom })?);
            }
            DependenceSpec::Clayton { theta, reflection } => {
                if dimensions < 2
                    || !theta.is_finite()
                    || !(-1.0..=20.0).contains(theta)
                    || (dimensions > 2 && (*theta < 0.0 || *reflection != Reflection::None || (*theta > 0.0 && *theta < 1e-4)))
                {
                    return Err(domain());
                }
                if dimensions > 2 && *theta > 0.0 {
                    frailty = Some(crate::PreparedDistribution::new(&Distribution::Gamma { shape: 1.0 / theta, scale: 1.0 })?);
                }
            }
            DependenceSpec::Frank { theta, reflection } => {
                if dimensions < 2
                    || !theta.is_finite()
                    || theta.abs() > 20.0
                    || (dimensions > 2 && (*theta < 0.0 || *theta > 4.0 || *reflection != Reflection::None))
                {
                    return Err(domain());
                }
                if dimensions > 2 && *theta > 0.0 {
                    frailty = Some(crate::PreparedDistribution::new(&Distribution::Logarithmic { probability: -(-theta).exp_m1() })?);
                }
            }
            DependenceSpec::Gumbel { theta, reflection } => {
                if dimensions < 2 || !theta.is_finite() || !(1.0..=20.0).contains(theta) || (dimensions > 2 && *reflection != Reflection::None) {
                    return Err(domain());
                }
            }
        }
        Ok(Self { spec: spec.clone(), dimensions, lower, chi, frailty })
    }
    pub fn latent_dimensions(&self) -> usize {
        self.dimensions
            + usize::from(self.chi.is_some() || self.frailty.is_some())
            + if self.dimensions > 2 && matches!(self.spec,DependenceSpec::Gumbel{theta,..} if theta>1.0) { 2 } else { 0 }
    }
    pub fn transform(&self, independent: &[f64], output: &mut [f64]) -> Result<()> {
        if independent.len() != self.latent_dimensions()
            || output.len() != self.dimensions
            || independent.iter().any(|p| !p.is_finite() || *p <= 0.0 || *p >= 1.0)
        {
            return Err(domain());
        }
        if self.dimensions > 2 {
            match self.spec {
                DependenceSpec::Clayton { theta, .. } if theta > 0.0 => {
                    let v = self.frailty.as_ref().ok_or_else(domain)?.quantile(independent[self.dimensions])?;
                    for (i, value) in output.iter_mut().enumerate() {
                        let log_ratio = (-independent[i].ln()).ln() - v.ln();
                        *value = open((-crate::reference_families::softplus(log_ratio) / theta).exp());
                    }
                    return Ok(());
                }
                DependenceSpec::Frank { theta, .. } if theta > 0.0 => {
                    let v = self.frailty.as_ref().ok_or_else(domain)?.quantile(independent[self.dimensions])?;
                    let p = -(-theta).exp_m1();
                    for (i, value) in output.iter_mut().enumerate() {
                        let e = -independent[i].ln() / v;
                        *value = open(-(-p * (-e).exp()).ln_1p() / theta);
                    }
                    return Ok(());
                }
                DependenceSpec::Gumbel { theta, .. } if theta > 1.0 => {
                    let alpha = 1.0 / theta;
                    let u = std::f64::consts::PI * independent[self.dimensions];
                    let w = -independent[self.dimensions + 1].ln();
                    let log_s = (alpha * u).sin().ln() - u.sin().ln() / alpha + (1.0 - alpha) / alpha * (((1.0 - alpha) * u).sin().ln() - w.ln());
                    if !log_s.is_finite() {
                        return Err(domain());
                    }
                    for (i, value) in output.iter_mut().enumerate() {
                        let log_e = (-independent[i].ln()).ln();
                        *value = open((-((alpha * (log_e - log_s)).exp())).exp());
                    }
                    return Ok(());
                }
                DependenceSpec::Clayton { theta: 0.0, .. } | DependenceSpec::Frank { theta: 0.0, .. } | DependenceSpec::Gumbel { theta: 1.0, .. } => {
                    output.copy_from_slice(independent);
                    return Ok(());
                }
                _ => (),
            }
        }
        match &self.spec {
            DependenceSpec::Independent => output.copy_from_slice(independent),
            DependenceSpec::Gaussian { .. } | DependenceSpec::GaussianSpearman { .. } | DependenceSpec::StudentT { .. } => {
                let normals = independent.iter().take(self.dimensions).map(|p| inverse_normal(*p)).collect::<Result<Vec<_>>>()?;
                let divisor = if let (Some(chi), DependenceSpec::StudentT { degrees_of_freedom, .. }) = (&self.chi, &self.spec) {
                    (chi.quantile(independent[self.dimensions])? / degrees_of_freedom).sqrt()
                } else {
                    1.0
                };
                for (i, value) in output.iter_mut().enumerate() {
                    let z = special::finite_sum((0..=i).map(|j| self.lower[i][j] * normals[j]))? / divisor;
                    *value = open(if let DependenceSpec::StudentT { degrees_of_freedom, .. } = self.spec {
                        student_cdf(z, degrees_of_freedom)?
                    } else {
                        special::normal_cdf(z)?
                    });
                }
            }
            DependenceSpec::Clayton { reflection, .. } | DependenceSpec::Frank { reflection, .. } | DependenceSpec::Gumbel { reflection, .. } => {
                let u = independent[0];
                let target = independent[1];
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..64 {
                    let mid = low + (high - low) * 0.5;
                    if self.conditional_base(u, mid)? < target { low = mid } else { high = mid }
                }
                output[0] = if matches!(reflection, Reflection::First | Reflection::Both) { open(1.0 - u) } else { u };
                output[1] = if matches!(reflection, Reflection::Second | Reflection::Both) { open(1.0 - high) } else { open(high) };
            }
        }
        Ok(())
    }
    /// dC(u,v)/du, the conditional CDF of the second coordinate given the first.
    pub fn conditional(&self, u: f64, v: f64) -> Result<f64> {
        if !u.is_finite() || !v.is_finite() || u <= 0.0 || u >= 1.0 || !(0.0..=1.0).contains(&v) {
            return Err(domain());
        }
        if v == 0.0 {
            return Ok(0.0);
        }
        if v == 1.0 {
            return Ok(1.0);
        }
        let reflection = match self.spec {
            DependenceSpec::Clayton { reflection, .. } | DependenceSpec::Frank { reflection, .. } | DependenceSpec::Gumbel { reflection, .. } => {
                reflection
            }
            _ => Reflection::None,
        };
        let base_u = if matches!(reflection, Reflection::First | Reflection::Both) { 1.0 - u } else { u };
        if matches!(reflection, Reflection::Second | Reflection::Both) {
            Ok(1.0 - self.conditional_base(base_u, (1.0 - v).next_down().max(0.0))?)
        } else {
            self.conditional_base(base_u, v)
        }
    }
    fn conditional_base(&self, u: f64, v: f64) -> Result<f64> {
        if !u.is_finite() || !v.is_finite() || u <= 0.0 || u >= 1.0 || !(0.0..=1.0).contains(&v) {
            return Err(domain());
        }
        if v == 0.0 {
            return Ok(0.0);
        }
        if v == 1.0 {
            return Ok(1.0);
        }
        let result = match self.spec {
            DependenceSpec::Clayton { theta, .. } => {
                if theta == -1.0 {
                    if v >= 1.0 - u { 1.0 } else { 0.0 }
                } else if theta.abs() < 1e-7 {
                    v
                } else if theta < 0.0 {
                    let sum = u.powf(-theta) + v.powf(-theta) - 1.0;
                    if sum <= 0.0 { 0.0 } else { u.powf(-theta - 1.0) * sum.powf(-1.0 / theta - 1.0) }
                } else {
                    let a = -theta * u.ln();
                    let b = -theta * v.ln();
                    let max = a.max(b);
                    let log_sum = max + ((a - max).exp() + (b - max).exp() - (-max).exp()).ln();
                    ((theta + 1.0) * (-u.ln()) - (1.0 + 1.0 / theta) * log_sum).exp()
                }
            }
            DependenceSpec::Frank { theta, .. } => {
                if theta.abs() < 1e-7 {
                    v
                } else {
                    let a = (-theta * u).exp();
                    let b = (-theta * v).exp_m1();
                    a * b / ((-theta).exp_m1() + (a - 1.0) * b)
                }
            }
            DependenceSpec::Gumbel { theta, .. } => {
                if theta == 1.0 {
                    v
                } else {
                    let a = -u.ln();
                    let b = -v.ln();
                    let s = a.powf(theta) + b.powf(theta);
                    (-s.powf(1.0 / theta) + (1.0 / theta - 1.0) * s.ln() + (theta - 1.0) * a.ln() - u.ln()).exp()
                }
            }
            _ => return Err(SolverError::new("copula_conditional", "Conditional helper is available for bivariate Archimedean families.")),
        };
        if result.is_finite() { Ok(result.clamp(0.0, 1.0)) } else { Err(domain()) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_declaration_rejects_unknown_configuration() {
        let extra = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(
            [("kind", "independent"), ("correlation", "silently ignored before")].into_iter(),
        );
        assert!(DependenceSpec::deserialize(extra).is_err());
        let valid = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new([("kind", "independent")].into_iter());
        assert!(matches!(DependenceSpec::deserialize(valid), Ok(DependenceSpec::Independent)));
    }
    #[test]
    fn matrices_and_singular_limits() {
        let p = PreparedCopula::new(&DependenceSpec::Gaussian { correlation: vec![vec![1.0, 1.0], vec![1.0, 1.0]] }, 2).unwrap();
        let mut out = [0.0; 2];
        p.transform(&[0.2, 0.8], &mut out).unwrap();
        assert_eq!(out[0], out[1]);
        assert!(PreparedCopula::new(&DependenceSpec::Gaussian { correlation: vec![vec![1.0, 1.1], vec![1.1, 1.0]] }, 2).is_err());
        assert!(
            PreparedCopula::new(&DependenceSpec::Gaussian { correlation: vec![vec![1.0, 0.9, 0.9], vec![0.9, 1.0, -0.9], vec![0.9, -0.9, 1.0]] }, 3)
                .is_err()
        );
    }
    #[test]
    fn archimedean_conditional_inversion_and_independence() {
        for spec in [
            DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::None },
            DependenceSpec::Frank { theta: -4.0, reflection: Reflection::None },
            DependenceSpec::Gumbel { theta: 2.0, reflection: Reflection::None },
        ] {
            let c = PreparedCopula::new(&spec, 2).unwrap();
            let mut out = [0.0; 2];
            for u in [0.01, 0.5, 0.99] {
                for v in [0.01, 0.5, 0.99] {
                    c.transform(&[u, v], &mut out).unwrap();
                    assert!((c.conditional(out[0], out[1]).unwrap() - v).abs() < 1e-10);
                }
            }
        }
        let c = PreparedCopula::new(&DependenceSpec::Gumbel { theta: 1.0, reflection: Reflection::None }, 2).unwrap();
        let mut out = [0.0; 2];
        c.transform(&[0.3, 0.8], &mut out).unwrap();
        assert!((out[1] - 0.8).abs() < 1e-15);
        let c = PreparedCopula::new(&DependenceSpec::Clayton { theta: 2.0, reflection: Reflection::None }, 2).unwrap();
        assert!((c.conditional(0.5, 0.5).unwrap() - 8.0 / 7.0_f64.powf(1.5)).abs() < 1e-14);
    }
}
