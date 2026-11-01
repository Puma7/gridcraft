//! Stable data contracts shared by the workbook-independent Solvercraft components.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use std::fmt;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SolverError {
    pub code: String,
    pub message: String,
}
impl SolverError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}
impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for SolverError {}
pub type Result<T> = std::result::Result<T, SolverError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Minimize,
    Maximize,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableKind {
    Continuous,
    Integer,
    Binary,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    pub name: String,
    /// None is an unbounded side. Infinity and NaN are invalid.
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub kind: VariableKind,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinearTerm {
    pub variable: usize,
    pub coefficient: f64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuadraticTerm {
    pub left: usize,
    pub right: usize,
    pub coefficient: f64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    LessEqual,
    Equal,
    GreaterEqual,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinearConstraint {
    pub name: String,
    pub terms: Vec<LinearTerm>,
    pub relation: Relation,
    pub rhs: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Objective {
    pub direction: Direction,
    pub offset: f64,
    pub linear: Vec<LinearTerm>,
    /// Each term is coefficient*x[left]*x[right], with no implicit half factor.
    #[serde(default)]
    pub quadratic: Vec<QuadraticTerm>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OptimizationModel {
    pub schema_version: u32,
    pub name: String,
    pub variables: Vec<Variable>,
    pub objective: Objective,
    pub constraints: Vec<LinearConstraint>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SolveMethod {
    Auto,
    Simplex,
    BranchAndBound,
    ProjectedGradient,
    DifferentialEvolution,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SolveOptions {
    pub method: SolveMethod,
    pub max_evaluations: u64,
    pub max_iterations: u64,
    pub max_nodes: u64,
    pub max_memory_bytes: u64,
    pub feasibility_tolerance: f64,
    pub integrality_tolerance: f64,
    pub optimality_tolerance: f64,
    pub relative_gap: f64,
    /// Serialize seeds as decimal strings at external JSON boundaries.
    #[serde(with = "decimal_seed")]
    pub seed: u64,
}
impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            method: SolveMethod::Auto,
            max_evaluations: 100_000,
            max_iterations: 100_000,
            max_nodes: 10_000,
            max_memory_bytes: 256 * 1024 * 1024,
            feasibility_tolerance: 1e-7,
            integrality_tolerance: 1e-7,
            optimality_tolerance: 1e-8,
            relative_gap: 0.0,
            seed: 0,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    Completed,
    Cancelled,
    TimeLimit,
    EvaluationLimit,
    IterationLimit,
    NodeLimit,
    MemoryLimit,
    NumericalFailure,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Feasibility {
    Feasible,
    Infeasible,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Optimality {
    Optimal,
    Stationary,
    BestFound,
    Unbounded,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SolveStatistics {
    pub evaluations: u64,
    pub iterations: u64,
    pub nodes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResult {
    pub termination: Termination,
    pub feasibility: Feasibility,
    pub optimality: Optimality,
    pub values: Option<Vec<f64>>,
    pub objective: Option<f64>,
    pub best_bound: Option<f64>,
    pub relative_gap: Option<f64>,
    pub max_violation: Option<f64>,
    pub statistics: SolveStatistics,
    pub engine: String,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub stage: String,
    pub iterations: u64,
    pub evaluations: u64,
    pub nodes: u64,
    pub best_objective: Option<f64>,
}
pub trait RunControl: Send + Sync {
    fn is_cancelled(&self) -> bool;
    fn progress(&self, _event: &ProgressEvent) {}
}
pub struct NoControl;
impl RunControl for NoControl {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Distribution {
    Constant {
        value: f64,
    },
    Uniform {
        low: f64,
        high: f64,
    },
    Normal {
        mean: f64,
        std_dev: f64,
    },
    Lognormal {
        log_mean: f64,
        log_std_dev: f64,
    },
    Triangular {
        low: f64,
        mode: f64,
        high: f64,
    },
    Bernoulli {
        probability: f64,
    },
    Binomial {
        trials: u32,
        probability: f64,
    },
    Poisson {
        rate: f64,
    },
    Exponential {
        rate: f64,
    },
    Weibull {
        shape: f64,
        scale: f64,
    },
    Empirical {
        values: Vec<f64>,
    },
    DiscreteUniform {
        low: i64,
        high: i64,
    },
    Beta {
        alpha: f64,
        beta: f64,
    },
    BetaPert {
        low: f64,
        mode: f64,
        high: f64,
        weight: f64,
    },
    Gamma {
        shape: f64,
        scale: f64,
    },
    NegativeBinomial {
        successes: u32,
        probability: f64,
    },
    /// Number of failures before the first success (support 0,1,...).
    Geometric {
        probability: f64,
    },
    /// Piecewise-linear quantile interpolation between sorted observations.
    EmpiricalContinuous {
        values: Vec<f64>,
    },
    StudentT {
        degrees_of_freedom: f64,
        location: f64,
        scale: f64,
    },
    ChiSquared {
        degrees_of_freedom: f64,
    },
    F {
        numerator_df: f64,
        denominator_df: f64,
    },
    Cauchy {
        location: f64,
        scale: f64,
    },
    Logistic {
        location: f64,
        scale: f64,
    },
    Laplace {
        location: f64,
        scale: f64,
    },
    Gumbel {
        location: f64,
        scale: f64,
    },
    Pareto {
        minimum: f64,
        shape: f64,
    },
    Rayleigh {
        scale: f64,
    },
    Erlang {
        shape: u32,
        scale: f64,
    },
    InverseGamma {
        shape: f64,
        scale: f64,
    },
    InverseGaussian {
        mean: f64,
        shape: f64,
    },
    LogLogistic {
        shape: f64,
        scale: f64,
    },
    SkewNormal {
        location: f64,
        scale: f64,
        shape: f64,
    },
    JohnsonSu {
        gamma: f64,
        delta: f64,
        location: f64,
        scale: f64,
    },
    JohnsonSb {
        gamma: f64,
        delta: f64,
        location: f64,
        scale: f64,
    },
    /// Johnson SL branch (shifted lognormal); the SN branch is Normal.
    JohnsonSl {
        gamma: f64,
        delta: f64,
        location: f64,
        scale: f64,
    },
    Burr12 {
        shape_c: f64,
        shape_k: f64,
        scale: f64,
    },
    Dagum {
        shape: f64,
        power: f64,
        scale: f64,
    },
    DoubleTriangular {
        low: f64,
        mode: f64,
        high: f64,
        left_probability: f64,
    },
    FatigueLife {
        shape: f64,
        scale: f64,
    },
    Frechet {
        shape: f64,
        scale: f64,
    },
    HyperbolicSecant {
        location: f64,
        scale: f64,
    },
    Kumaraswamy {
        alpha: f64,
        beta: f64,
    },
    Levy {
        location: f64,
        scale: f64,
    },
    Pareto2 {
        shape: f64,
        scale: f64,
    },
    BetaPrime {
        alpha: f64,
        beta: f64,
        scale: f64,
    },
    Reciprocal {
        low: f64,
        high: f64,
    },
    MinExtreme {
        location: f64,
        scale: f64,
    },
    /// Log-series count distribution; numerical implementation limits p to0.99.
    Logarithmic {
        probability: f64,
    },
    /// Original2016 Metalog; only globally certified coefficient sets are accepted.
    Metalog {
        coefficients: Vec<f64>,
        lower: Option<f64>,
        upper: Option<f64>,
    },
    Hypergeometric {
        population: u32,
        successes: u32,
        draws: u32,
    },
    Categorical {
        values: Vec<f64>,
        weights: Vec<f64>,
    },
    ShiftScale {
        base: Box<Distribution>,
        shift: f64,
        scale: f64,
    },
    Truncated {
        base: Box<Distribution>,
        lower: Option<f64>,
        upper: Option<f64>,
    },
    Mixture {
        components: Vec<Distribution>,
        weights: Vec<f64>,
    },
    /// Sampling-only compound sum; generic compound CDF/quantile is not implied.
    CompoundPoisson {
        rate: f64,
        severity: Box<Distribution>,
        max_events: u32,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SamplingMethod {
    MonteCarlo,
    LatinHypercube,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SimulationOptions {
    pub trials: u64,
    #[serde(with = "decimal_seed")]
    pub seed: u64,
    pub method: SamplingMethod,
    pub retain_samples: bool,
    pub max_memory_bytes: u64,
}
impl Default for SimulationOptions {
    fn default() -> Self {
        Self { trials: 10_000, seed: 0, method: SamplingMethod::MonteCarlo, retain_samples: false, max_memory_bytes: 128 * 1024 * 1024 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleSummary {
    pub count: u64,
    pub mean: f64,
    pub variance: Option<f64>,
    pub minimum: f64,
    pub maximum: f64,
    pub quantiles: Vec<(f64, f64)>,
    pub value_at_risk_95: Option<f64>,
    pub conditional_value_at_risk_95: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub termination: Termination,
    pub completed_trials: u64,
    pub summaries: Vec<SampleSummary>,
    pub samples: Option<Vec<Vec<f64>>>,
    pub sampling_version: String,
    pub warnings: Vec<String>,
}

/// Decimal strings are canonical across clients with 53-bit JSON number precision.
pub mod decimal_seed {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            Text(String),
            Number(u64),
        }
        match Input::deserialize(deserializer)? {
            Input::Text(value) => value.parse::<u64>().map_err(serde::de::Error::custom),
            Input::Number(value) => Ok(value),
        }
    }
}
