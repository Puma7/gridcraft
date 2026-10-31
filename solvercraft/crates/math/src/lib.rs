//! Deterministic simulation primitives. No global random state or native dependencies.
#![forbid(unsafe_code)]
pub mod distributions;
pub mod random;
pub mod risk;

pub use distributions::{PreparedDistribution, cdf, quantile, sample, validate_distribution};
pub use random::{SAMPLING_VERSION, addressed_index, addressed_uniform};
pub use risk::{empirical_quantile, upper_tail_cvar};
