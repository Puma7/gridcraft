//! Random-access Sobol digital nets with64 licensed Joe–Kuo direction rows.
//! See sobol_directions.rs and third_party/BOOST_LICENSE_1_0.txt for provenance.
use crate::random::addressed_u64;
use solvercraft_types::{Result, SolverError};
pub const SOBOL_VERSION: &str = "solvercraft-sobol-joe-kuo-boost64-digital-shift-v2";
pub const MAX_SOBOL_DIMENSIONS: usize = 64;
#[derive(Debug, Clone)]
pub struct RandomizedSobol {
    directions: Vec<[u64; 64]>,
    seed: u64,
    replicate: u64,
}
#[cfg(test)]
fn mul_mod(mut a: u32, mut b: u32, poly: u32, degree: u32) -> u32 {
    let mut r = 0;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        b >>= 1;
        a <<= 1;
        if a & (1 << degree) != 0 {
            a ^= poly;
        }
    }
    r
}
#[cfg(test)]
fn power(mut a: u32, mut n: u32, p: u32, d: u32) -> u32 {
    let mut r = 1;
    while n > 0 {
        if n & 1 != 0 {
            r = mul_mod(r, a, p, d);
        }
        a = mul_mod(a, a, p, d);
        n >>= 1;
    }
    r
}
#[cfg(test)]
fn primitive(poly: u32, d: u32) -> bool {
    let order = (1 << d) - 1;
    if power(2, order, poly, d) != 1 {
        return false;
    }
    let mut n = order;
    let mut factor = 2;
    while factor * factor <= n {
        if n.is_multiple_of(factor) {
            if power(2, order / factor, poly, d) == 1 {
                return false;
            }
            while n.is_multiple_of(factor) {
                n /= factor;
            }
        }
        factor += 1;
    }
    n <= 1 || power(2, order / n, poly, d) != 1
}
impl RandomizedSobol {
    pub fn new(dimensions: usize, seed: u64, replicate: u64) -> Result<Self> {
        if dimensions == 0 || dimensions > MAX_SOBOL_DIMENSIONS {
            return Err(SolverError::new("sobol_dimension", "Sobol supports 1..64 dimensions."));
        }
        let mut directions = Vec::new();
        directions.try_reserve_exact(dimensions).map_err(|_| SolverError::new("allocation", "Cannot allocate Sobol directions."))?;
        let mut first = [0_u64; 64];
        for (i, v) in first.iter_mut().enumerate() {
            *v = 1_u64 << (63 - i);
        }
        directions.push(first);
        for (polynomial, initial) in crate::sobol_directions::DIRECTIONS.iter().take(dimensions - 1) {
            let s = (u16::BITS - 1 - polynomial.leading_zeros()) as usize;
            let mut v = [0_u64; 64];
            for (i, m) in initial.iter().enumerate().take(s) {
                v[i] = u64::from(*m) << (63 - i);
            }
            for i in s..64 {
                let mut value = v[i - s] ^ (v[i - s] >> s);
                for k in 1..s {
                    if polynomial & (1 << (s - k)) != 0 {
                        value ^= v[i - k];
                    }
                }
                v[i] = value;
            }
            directions.push(v);
        }
        if directions.len() != dimensions {
            return Err(SolverError::new("sobol_dimension", "Primitive polynomial limit exhausted."));
        }
        Ok(Self { directions, seed, replicate })
    }
    pub fn dimensions(&self) -> usize {
        self.directions.len()
    }
    fn point_bits(&self, index: u64, dimension: usize) -> Result<u64> {
        let directions = self.directions.get(dimension).ok_or_else(|| SolverError::new("sobol_dimension", "Sobol coordinate outside dimensions."))?;
        let gray = index ^ (index >> 1);
        let mut bits = 0;
        for (i, v) in directions.iter().enumerate() {
            if gray & (1_u64 << i) != 0 {
                bits ^= v;
            }
        }
        Ok(bits)
    }
    /// Unrandomized Gray-order net point in [0,1), for interoperability and reference checks.
    /// Use coordinate() for independently randomized simulation, which keeps samples open(0,1).
    pub fn unshifted_coordinate(&self, index: u64, dimension: usize) -> Result<f64> {
        Ok((self.point_bits(index, dimension)? >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0))
    }
    /// Same point regardless of access order. The 52-bit midpoint keeps the result open (0,1).
    pub fn coordinate(&self, index: u64, dimension: usize) -> Result<f64> {
        let bits = self.point_bits(index, dimension)? ^ addressed_u64(self.seed, dimension as u64, self.replicate, 0x534f_424f_4c32);
        Ok(((bits >> 12) as f64 + 0.5) * (1.0 / 4_503_599_627_370_496.0))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_primitive_polynomials_and_first_net() {
        assert!(primitive(0b11, 1));
        assert!(primitive(0b111, 2));
        assert!(primitive(0b1011, 3));
        assert!(!primitive(0b1001, 3));
        for (poly, initial) in crate::sobol_directions::DIRECTIONS {
            let degree = u16::BITS - 1 - poly.leading_zeros();
            assert!(primitive(u32::from(*poly), degree));
            for (i, m) in initial.iter().take(degree as usize).enumerate() {
                assert!(*m > 0 && m % 2 == 1 && u32::from(*m) < (1_u32 << (i + 1)));
            }
        }
        let net = RandomizedSobol::new(64, 42, 0).unwrap();
        for dimension in 0..64 {
            let mut occupied = [false; 256];
            for index in 0..256 {
                let x = net.coordinate(index, dimension).unwrap();
                assert!(x > 0.0 && x < 1.0);
                let bin = (x * 256.0).floor() as usize;
                assert!(!occupied[bin], "dim {dimension}, bin {bin}");
                occupied[bin] = true;
            }
        }
        assert_eq!(net.coordinate(17, 5).unwrap(), RandomizedSobol::new(64, 42, 0).unwrap().coordinate(17, 5).unwrap());
        assert_ne!(net.coordinate(17, 5).unwrap(), RandomizedSobol::new(64, 42, 1).unwrap().coordinate(17, 5).unwrap());
    }
    #[test]
    fn published_three_dimensional_net_and_shifted_v2_manifest() {
        let expected = [
            [0.0, 0.0, 0.0],
            [0.5, 0.5, 0.5],
            [0.75, 0.25, 0.25],
            [0.25, 0.75, 0.75],
            [0.375, 0.375, 0.625],
            [0.875, 0.875, 0.125],
            [0.625, 0.125, 0.875],
            [0.125, 0.625, 0.375],
        ];
        let net = RandomizedSobol::new(64, 0, 0).unwrap();
        for (i, row) in expected.iter().enumerate() {
            for (dim, value) in row.iter().enumerate() {
                assert_eq!(net.unshifted_coordinate(i as u64, dim).unwrap(), *value);
            }
        }
        for (seed, replicate, index, dimension, expected) in
            [(0, 0, 0, 0, 0x3febe4f48abff3e9_u64), (42, 0, 17, 5, 0x3fef1b163060394f), (u64::MAX, 9, 1023, 63, 0x3fc8a0322a434b6c)]
        {
            assert_eq!(RandomizedSobol::new(64, seed, replicate).unwrap().coordinate(index, dimension).unwrap().to_bits(), expected);
        }
        let mut cells = std::collections::BTreeSet::new();
        for i in 0..256 {
            cells.insert(((net.unshifted_coordinate(i, 50).unwrap() * 16.0) as u8, (net.unshifted_coordinate(i, 51).unwrap() * 16.0) as u8));
        }
        assert_eq!(cells.len(), 128);
        assert_eq!(SOBOL_VERSION, "solvercraft-sobol-joe-kuo-boost64-digital-shift-v2");
    }
}
