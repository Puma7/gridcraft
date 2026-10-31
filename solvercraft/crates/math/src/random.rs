//! Versioned, counter-addressed non-cryptographic random numbers.
//! Each draw is a function of its complete address, independent of evaluation order.
use solvercraft_types::{Result, SolverError};

pub const SAMPLING_VERSION: &str = "solvercraft-addressed-mix64-v1";

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
fn addressed_bits(seed: u64, binding: u64, trial: u64, subcounter: u64) -> u64 {
    mix(mix(seed ^ 0x243f_6a88_85a3_08d3)
        ^ mix(binding ^ 0x1319_8a2e_0370_7344)
        ^ mix(trial ^ 0xa409_3822_299f_31d0)
        ^ mix(subcounter ^ 0x082e_fa98_ec4e_6c89))
}
/// An open-interval uniform value. Its 52-bit midpoint construction cannot round to 0 or 1.
pub fn addressed_uniform(seed: u64, binding: u64, trial: u64, subcounter: u64) -> f64 {
    let mantissa = addressed_bits(seed, binding, trial, subcounter) >> 12;
    (mantissa as f64 + 0.5) * (1.0 / 4_503_599_627_370_496.0)
}
/// An unbiased index in 0..upper, using rejection and a bounded deterministic substream.
pub fn addressed_index(seed: u64, binding: u64, trial: u64, subcounter: u64, upper: usize) -> Result<usize> {
    let bound = u64::try_from(upper).map_err(|_| SolverError::new("index_range", "Index range is too large."))?;
    if bound == 0 {
        return Err(SolverError::new("index_range", "Index range must be positive."));
    }
    let threshold = bound.wrapping_neg() % bound;
    for retry in 0..64 {
        let counter = subcounter.checked_add(retry).ok_or_else(|| SolverError::new("rng_address", "Random subcounter overflow."))?;
        let value = addressed_bits(seed, binding, trial, counter);
        if value >= threshold {
            return usize::try_from(value % bound).map_err(|_| SolverError::new("index_range", "Index does not fit this platform."));
        }
    }
    Err(SolverError::new("rng_rejection", "Random index rejection budget exceeded."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_are_repeatable_and_order_independent() {
        let expected: Vec<f64> = (0..100).map(|i| addressed_uniform(7, 2, i, 0)).collect();
        for i in (0..100).rev() {
            assert_eq!(expected[i], addressed_uniform(7, 2, i as u64, 0));
        }
        assert_ne!(addressed_uniform(7, 2, 3, 0), addressed_uniform(7, 3, 2, 0));
        assert_ne!(addressed_uniform(7, 2, 3, 0), addressed_uniform(7, 2, 3, 1));
        assert!(expected.iter().all(|v| *v > 0.0 && *v < 1.0));
    }
    #[test]
    fn index_domains_are_checked() {
        assert!(addressed_index(0, 0, 0, 0, 0).is_err());
        for i in 0..1000 {
            assert!(addressed_index(2, 0, i, 0, 17).is_ok_and(|v| v < 17));
        }
    }
}
