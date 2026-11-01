//! Deterministic simulation primitives. No global random state or native dependencies.
#![forbid(unsafe_code)]
pub mod catalog;
pub mod copula;
pub mod density;
pub mod distributions;
pub mod elicitation;
pub mod extended;
pub mod johnson_moments;
pub mod metalog;
pub mod multivariate;
pub mod nominal;
pub mod parameterization;
pub mod psi_aliases;
pub mod random;
pub mod reference_families;
pub mod reference_functions;
pub mod risk;
pub mod sobol;
mod sobol_directions;
pub mod special;
pub mod time_series;

pub use distributions::{PreparedDistribution, cdf, quantile, sample, validate_distribution};
pub use random::{SAMPLING_VERSION, addressed_index, addressed_uniform};
pub use risk::{empirical_quantile, upper_tail_cvar};
