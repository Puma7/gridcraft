//! Exact signed binary accumulation for one, two, or three finite f64 factors.
//!
//! Each input is an exact dyadic rational. Integer significand products are
//! accumulated without floating multiplication, then rounded once to binary64.
//! This is an independent fixed-bin implementation, not copied solver code.
use solvercraft_types::{Result, SolverError};

const MIN_EXPONENT: i32 = -3222; // Three minimum subnormal factors.
const LIMBS: usize = 104; // Covers max finite^3 plus >20 carry bits for a million terms.
const MAX_ENTRIES: usize = super::MAX_TERMS + 1;
#[derive(Clone)]
pub struct ExactProducts {
    positive: [u64; LIMBS],
    negative: [u64; LIMBS],
    entries: usize,
    failed: bool,
}
impl Default for ExactProducts {
    fn default() -> Self {
        Self { positive: [0; LIMBS], negative: [0; LIMBS], entries: 0, failed: false }
    }
}
fn range(message: &str) -> SolverError {
    SolverError::new("numeric_range", message)
}
fn decode(value: f64) -> Result<(bool, u64, i32)> {
    if !value.is_finite() {
        return Err(SolverError::new("non_finite", "Exact product operands must be finite."));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    Ok((bits >> 63 != 0, if exponent == 0 { fraction } else { (1_u64 << 52) | fraction }, if exponent == 0 { -1074 } else { exponent - 1075 }))
}
fn add_word(bins: &mut [u64; LIMBS], mut index: usize, mut word: u64) -> Result<()> {
    while word != 0 {
        let slot = bins.get_mut(index).ok_or_else(|| range("Exact accumulator exceeded fixed carry headroom."))?;
        let (value, carry) = slot.overflowing_add(word);
        *slot = value;
        word = u64::from(carry);
        index += 1;
    }
    Ok(())
}
impl ExactProducts {
    /// Compare the retained exact sum with zero, before any underflow rounding.
    pub fn exact_sign(&self) -> Result<std::cmp::Ordering> {
        if self.failed {
            return Err(range("Exact accumulator is invalid after an earlier error."));
        }
        Ok(self.positive.iter().rev().cmp(self.negative.iter().rev()))
    }
    /// Tight finite binary64 enclosure of the exact sum, rounded outwards.
    /// Unlike finish()==0, this preserves nonzero values below minsubnormal.
    /// If an outward endpoint is infinite, no finite enclosure is claimed.
    pub fn enclosure(&self) -> Result<(f64, f64)> {
        let nearest = self.clone().finish()?;
        let mut remainder = self.clone();
        // This is one internal comparison, not another caller-supplied modeled
        // term. Fixed carry headroom covers it independently of the public cap.
        remainder.entries = 0;
        remainder.add(-nearest)?;
        let (lower, upper) = match remainder.exact_sign()? {
            std::cmp::Ordering::Equal => (nearest, nearest),
            std::cmp::Ordering::Greater => (nearest, nearest.next_up()),
            std::cmp::Ordering::Less => (nearest.next_down(), nearest),
        };
        if !lower.is_finite() || !upper.is_finite() {
            return Err(range("Exact sum has no finite outward binary64 enclosure."));
        }
        Ok((lower, upper))
    }
    fn add_parts(&mut self, negative: bool, mantissa: [u64; 3], exponent: i32) -> Result<()> {
        if self.failed {
            return Err(range("Exact accumulator is invalid after an earlier error."));
        }
        if self.entries >= MAX_ENTRIES {
            self.failed = true;
            return Err(SolverError::new("model_limit", "Exact accumulation exceeds the bounded term count."));
        }
        self.entries += 1;
        let shift = usize::try_from(exponent - MIN_EXPONENT).map_err(|_| range("Exact product exponent is outside supported range."))?;
        let (start, bits) = (shift / 64, shift % 64);
        let bins = if negative { &mut self.negative } else { &mut self.positive };
        for (i, word) in mantissa.into_iter().enumerate() {
            if word == 0 {
                continue;
            }
            let added = add_word(bins, start + i, word << bits)
                .and_then(|()| if bits == 0 { Ok(()) } else { add_word(bins, start + i + 1, word >> (64 - bits)) });
            if let Err(error) = added {
                self.failed = true;
                return Err(error);
            }
        }
        Ok(())
    }
    fn operand(&mut self, value: f64) -> Result<(bool, u64, i32)> {
        match decode(value) {
            Ok(parts) => Ok(parts),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
    /// Add a finite f64 exactly, including subnormal values.
    pub fn add(&mut self, value: f64) -> Result<()> {
        let (negative, mantissa, exponent) = self.operand(value)?;
        self.add_parts(negative, [mantissa, 0, 0], exponent)
    }
    /// Add the exact real product of two finite binary64 operands.
    /// Intermediate overflow and underflow are retained in the bins.
    pub fn add_product(&mut self, left: f64, right: f64) -> Result<()> {
        let (ls, lm, le) = self.operand(left)?;
        let (rs, rm, re) = self.operand(right)?;
        let product = u128::from(lm) * u128::from(rm);
        self.add_parts(ls ^ rs, [product as u64, (product >> 64) as u64, 0], le + re)
    }
    /// Add the exact real product of three finite binary64 operands.
    pub fn add_triple(&mut self, first: f64, second: f64, third: f64) -> Result<()> {
        let (asign, a, ae) = self.operand(first)?;
        let (bsign, b, be) = self.operand(second)?;
        let (csign, c, ce) = self.operand(third)?;
        let ab = u128::from(a) * u128::from(b);
        let low = u128::from(ab as u64) * u128::from(c);
        let high = (ab >> 64) * u128::from(c) + (low >> 64);
        self.add_parts(asign ^ bsign ^ csign, [low as u64, high as u64, (high >> 64) as u64], ae + be + ce)
    }
    /// Correctly round the exact accumulated dyadic rational once, nearest/ties-even.
    /// A nonrepresentable finite result returns numeric_range rather than infinity.
    pub fn finish(self) -> Result<f64> {
        if self.failed {
            return Err(range("Exact accumulator is invalid after an earlier error."));
        }
        let ordering = self.exact_sign()?;
        let (mut magnitude, subtract, negative) = match ordering {
            std::cmp::Ordering::Equal => return Ok(0.0),
            std::cmp::Ordering::Greater => (self.positive, self.negative, false),
            std::cmp::Ordering::Less => (self.negative, self.positive, true),
        };
        let mut borrow = false;
        for (word, sub) in magnitude.iter_mut().zip(subtract) {
            let (difference, b1) = word.overflowing_sub(sub);
            let (difference, b2) = difference.overflowing_sub(u64::from(borrow));
            *word = difference;
            borrow = b1 || b2;
        }
        if borrow {
            return Err(range("Exact accumulator signed subtraction failed."));
        }
        round(&magnitude, negative)
    }
}
fn word_at(words: &[u64; LIMBS], shift: usize) -> u64 {
    let index = shift / 64;
    let bits = shift % 64;
    let low = words.get(index).copied().unwrap_or(0) >> bits;
    if bits == 0 { low } else { low | (words.get(index + 1).copied().unwrap_or(0) << (64 - bits)) }
}
fn bit_at(words: &[u64; LIMBS], index: usize) -> bool {
    words.get(index / 64).is_some_and(|w| w & (1_u64 << (index % 64)) != 0)
}
fn any_below(words: &[u64; LIMBS], exclusive: usize) -> bool {
    let full = exclusive / 64;
    let bits = exclusive % 64;
    words.iter().take(full).any(|w| *w != 0) || (bits != 0 && words.get(full).is_some_and(|w| w & ((1_u64 << bits) - 1) != 0))
}
fn round(words: &[u64; LIMBS], negative: bool) -> Result<f64> {
    let Some(last) = words.iter().rposition(|w| *w != 0) else { return Ok(0.0) };
    let high = last * 64 + (63 - words[last].leading_zeros() as usize);
    let exponent = high as i32 + MIN_EXPONENT;
    let normal = exponent >= -1022;
    let shift = if normal { high - 52 } else { (-1074 - MIN_EXPONENT) as usize };
    let mut significand = word_at(words, shift);
    let guard = shift != 0 && bit_at(words, shift - 1);
    let sticky = shift > 1 && any_below(words, shift - 1);
    if guard && (sticky || significand & 1 != 0) {
        significand += 1;
    }
    let sign = u64::from(negative) << 63;
    if !normal {
        return Ok(f64::from_bits(sign | significand));
    }
    let mut exponent = exponent;
    if significand == 1_u64 << 53 {
        significand >>= 1;
        exponent += 1;
    }
    if exponent > 1023 {
        return Err(range("Exact product sum rounds outside finite binary64 range."));
    }
    let bits = sign | (((exponent + 1023) as u64) << 52) | (significand & ((1_u64 << 52) - 1));
    Ok(f64::from_bits(bits))
}
