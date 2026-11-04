# Original-model arithmetic

`evaluate_objective`, `evaluate_working_objective`, `evaluate_linear_residual`
and `compensated_dot` accumulate exact dyadic products before one binary64
nearest/ties-even rounding. They preserve product residuals that ordinary
compensated summation cannot recover after an already-rounded multiplication.

For example, `(2^53-1)*(1+2^-52)-2^53` returns `1-2^-52`, rather than 0.
Quadratic coefficient/value/value triples are accumulated before any
intermediate overflow or underflow. Final results that round outside finite binary64 range
return `numeric_range`; nonfinite inputs are rejected. Exact zero is +0;
a negative nonzero result that rounds to zero retains -0.

`ExactProducts` uses separate positive and negative arrays of 104 u64 limbs,
with a lowest bit exponent of -3222. A finite binary64 significand has at most 53
bits and exponent in [-1074,971]; three significands need at most 159 product
bits. The fixed arrays therefore cover all supported triple products and the
carry headroom for at most `MAX_TERMS+1` entries (one million terms plus an
offset/RHS). An additional call fails explicitly and poisons the accumulator.
No heap allocation, dependency, unsafe code or arbitrary-precision fallback is
used by the accumulator. It is reusable scratch, not storage per coefficient.
`exact_sign` compares the retained value with zero before underflow;
`enclosure` returns tight outward finite endpoints or refuses a nonfinite
endpoint. Certificate and recession checks therefore need not mistake an
underflowed rounded result for an exact zero.

The existing `CompensatedSum` remains exactly 16 bytes for compiler polynomial
maps. Its public streaming behavior remains checked Neumaier summation; it does
not promise exact products. Code needing product arithmetic must use the exact
APIs rather than multiply first and pass rounded products to a sum.

Canonicalization groups coefficients sequentially, accumulates their exact
sum and checks its residual after conversion to f64. A sum that cannot be
represented exactly is refused with `numeric_range`. This deliberately includes
duplicate coefficients 0.1+0.2 and 1e16+1; silently replacing them with a rounded
matrix could change feasibility, objective values and global bounds. Unique
coefficients and exact duplicate sums remain supported. Original input remains
available for final model validation. This representation limit is explicit;
it is not a claim of exact rational optimization or unrestricted numeric scale.

`tests/generate_exact_products.py` independently generates 1,619 bit-pattern
fixtures using Python's standard `fractions.Fraction` and rational-to-float
conversion. Rust tests compare every output bit in both orders, including
normal/subnormal ties, exponent carries, signed zeros, maximum finite values,
overflowing product cancellation and underflow products collecting to a
representable answer. A million-term triple-product cancellation stress also
checks carry headroom. No statistical acceptance or implementation-derived
oracle is used. Every finite exponent is also tested with both signs and five
significand boundary patterns (20,470 identity cases).

The independent design follows the exact fixed-point accumulation principle
described by [Kulisch and Snyder, The Exact Dot Product](https://grouper.ieee.org/groups/1788/email/pdfhsP_kVVcoq.pdf)
and [Neal, Fast exact summation using small and large superaccumulators](https://arxiv.org/abs/1505.05571).
It uses its own signed-bin layout, three-factor multiplication and rounding
code; no upstream source implementation was copied. The arithmetic foundation
does not make subsequent solver factorization, convergence or certificates
exact; their separate error checks and documented limitations still apply.
