# Simulation and math cross review

Baseline reviewed: PR head 690a098a6c4c9d966d66b6c30c036e0e42534bfd. Review date: 31 October 2026.

This review covered math distributions, addressed RNG, empirical risk, online statistics, elementary MLE fits, and Monte Carlo/LHS execution. It cross-reviewed code authored by another implementation agent. Production changes described below were authorized by the integrator.

## Evidence and execution status

The reviewer's local execution transport disconnected before its first command. Source was initially retrieved from the pinned GitHub commit and host writes used apply_patch. The execution transport was restored before final signoff, allowing the full selected Rust suites and lint checks to run against the final working tree.

Initial arithmetic reproductions used deterministic IEEE-754 JavaScript experiments and exact source reasoning. Those initial experiments are not presented as baseline Rust execution. The final corrected implementation and regression cases were subsequently compiled and executed with Rust 1.91.1 on Windows.

Two integration test files are added:

- crates/math/tests/deep_review.rs: 18 deterministic review tests.
- crates/simulation/tests/deep_review.rs: 11 deterministic review tests.

These tests assert independent mathematical invariants, exact small discrete distributions, known quantile references, and declared resource/seed contracts. They contain no random p-value acceptance test.

Run the full math and simulation test suites after these changes, not only the new tests:

    cargo test -p solvercraft-math -p solvercraft-simulation --offline
    cargo clippy -p solvercraft-math -p solvercraft-simulation --all-targets --offline -- -D warnings

Final selected-crate verification passed: **47 tests, including all 29 new deep-review cases**, with no failures. The selected-crate Clippy command above passed for all targets with warnings denied. The selected-package cargo fmt check also passed. Existing nominal-moment, repeatability and LHS tests were rerun successfully with RNG v2. Full-workspace/facade job verification remains the integrator's separate gate.

## Correctness findings and fixes

| ID | Original finite input and effect | Mathematical reference | Change |
|---|---|---|---|
| MATH-01 | Normal CDF: mean=-1e308, std=1e308, x=1e308; subtracting before dividing overflows and produces CDF=1 | Standardized x=2; CDF approximately 0.977249868 | Use a scaled-difference fallback only when finite x/mean subtraction overflows |
| MATH-02 | Normal quantile: mean=-1e308, std=1e308, p=Phi(2); the product can overflow before the mean cancels it | A finite quantile near 1e308 | Fused multiply-add avoids the unrepresentable intermediate product |
| MATH-03 | Weibull CDF: x=1e308, scale=1e-308, shape=1e-308; ratio overflows, yielding 1; reversed ratio underflows, yielding 0 | Both CDFs round to 1-exp(-1), approximately 0.6321205588 | Compute the power in log space; preserve close x/scale differences with ln1p rather than subtracting equal rounded logarithms |
| MATH-04 | Positive lognormal interior quantiles can underflow to zero | A positive quantile cannot be represented, so a range error is appropriate | Return numerical_range for underflowed positive interior lognormal, exponential and Weibull quantiles; valid zero endpoints remain supported |
| MATH-05 | Weibull inverse: shape=.001, scale=1e-308, p=1-exp(-2.1); the power overflows before multiplication by scale | Finite quantile approximately 1.6568940330191747e14 | Evaluate the inverse in log space |
| MATH-06 | Triangular: low=0, mode=1e-200, high=1; squaring a normalized tail or multiplying p by split underflows | CDF(mode)=1e-200 and inverse at p=1e-200 is mode | CDF uses a product of two bounded conditional ratios; inverse uses conditional ratios without tiny probability products |
| MATH-07 | Extreme triangular split can underflow to zero and create 0/0 in CDF; a right triangle with width=1e308 and p=1e-300 can lose p in 1-p | Finite CDF 0/1 as appropriate; inverse approximately 5e7 | Avoid zero denominators in the branch formula and rationalize the lower-end right-branch inverse |
| STAT-01 | OnlineMoments([1e16,1e16+2]) gives variance 4; reversed data gives 0 | Exact unbiased sample variance is 2 in both orders | Origin-centered, adaptively scaled Welford moments preserve small variance around a large location |
| STAT-02 | Large-offset mean reconstruction and opposite-sign cancellation need different numerical treatment | Mean of [1e12+1,...,+4] is exactly representable as 1e12+2.5; mean of [1e17,-1,-1e17] is -1/3 | Use centered fused reconstruction for tight large-offset clusters and an independently compensated scaled raw sum when data span zero |
| FIT-01 | Uniform and exponential fitting compute an irrelevant M2 and reject [1e200,2e200] | Uniform endpoints and exponential rate 1/1.5e200 are representable | Uniform scans finite extrema only; exponential computes a stable scaled mean without a second moment |
| FIT-02 | Normal fitting forms a variance that overflows although the standard deviation is finite | For [1e200,2e200], population std is 5e199; for [-1e308,1e308], std is 1e308 | Compute population standard deviation directly from scaled moments |
| SIM-01 | A zero-input evaluator can request 10 million trials with 4096 output columns and evade the input-sample work cap | Engine-owned output work must also be bounded | Check the 100-million-output-sample limit at first output-shape discovery |

The variance accumulator retains a representability boundary: if the requested sample variance itself exceeds f64 range, summary returns an explicit range error. A finite normal-fit standard deviation can still be obtained directly from scaled moments. No infinite variance field or silent zero replacement is returned.

Rejected observations do not partially commit accumulator state. The simulation's allocation estimate uses size_of<OnlineMoments>, so the expanded accumulator's storage is included automatically.

## Addressed RNG strengthening

The alpha-v1 address combiner XORed independently mixed tuple coordinates. For fixed seed=7 and subcounter=0, define:

    D = 0x13198a2e03707344 XOR 0xa4093822299f31d0
      = 13191239073897857684

All addresses with binding=D XOR trial cancel the binding/trial terms. The examples trial=0,1,2,3,123456 all produce identical old 64-bit output 13694608328658067128. This is a constructed algebraic family, not an ordinary birthday collision.

Those bindings exceed the current simulator's index range of 0..4095. This review therefore **does not claim an observed failure of the current bounded Monte Carlo workflow**. It identifies a structural weakness in the public u64-address API and strengthens the design before future binding-ID reuse.

With integrator approval, v2 folds seed, binding, trial and subcounter in tuple order with position-specific domain constants. It removes this coordinate-cancellation pattern. The sampling manifest now reports:

    solvercraft-addressed-mix64-v2

**Seeded outputs intentionally change from alpha v1.** Reusing a numeric seed across v1/v2 does not mean reusing the same sample set. Existing repeatability, traversal-order, LHS occupancy and nominal-moment tests must be rerun. A new regression rejects collapse of the old coordinate family, and three v2 address/seed vectors record exact uniform output bits, including maximum u64 values.

This remains a non-cryptographic generator without a statistical-certification claim. The change is not a proof of absence of all finite-state hash collisions.

## Boundary and integration checks included

- Exact Binomial(4,.5) CDF masses [1,4,6,4,1]/16 and lower-inverse boundaries.
- Large supported Binomial and Poisson quantile/CDF bracketing.
- Explicit rejection above Binomial nondegenerate count 10,000 and Poisson rate 512.
- Exact degenerate count/rate behavior, including maximum u32 count at p=0/1.
- Degenerate normal, constant, uniform and triangular support endpoints.
- Normal lower-tail quantiles at probabilities 1e-12, 1e-100 and 1e-300 against reference constants.
- Maximum seed/address values, positive index ranges, deterministic repeatability and v2 reference vectors.
- Prime-sized 257-stratum Latin Hypercube occupancy and repeated maximum-seed runs.
- Exact empirical lower-inverse quantiles, negative losses, fractional CVaR boundary mass, cancellation and subnormal loss tails.
- Streaming 50,000 constant outputs under 4096 bytes versus rejected retained storage.
- Work-limit rejection on the first shape discovery for very wide outputs.
- Pre-cancelled execution without evaluator calls or fabricated summaries.

## Declared limits and remaining gaps

These are distinct from defects fixed above:

- Normal CDF uses a documented absolute-error approximation of about 1.5e-7. It is not a high-relative-accuracy extreme-tail probability routine.
- Binomial nondegenerate trials and Poisson rate have explicit work caps; unsupported sizes do not silently select normal approximations.
- Streaming mode deliberately omits exact quantiles/CVaR unless samples are retained.
- Only elementary normal/uniform/exponential unweighted, uncensored MLE fits are implemented. Fitting scores, goodness-of-fit, censoring, weighting and broader families remain product-scope gaps.
- Correlation/copulas, randomized Sobol, certified RNG statistics and broader Frontline parity are not supplied by this patch.
- The public sampler documents that evaluator allocation/computation budgeting is the host's responsibility. Engine-owned input/output/storage work is separately checked.
- Cancellation is checked during preparation and trials. Exact retained-data sorting/final summary construction is not yet a separately resumable cancellation phase; no one-second end-to-end cancellation guarantee is established here.
- Full facade simulation-job, GUI and WASM execution remain integrator-level checks; they were not executed by this reviewer.

## Files changed by this review

Production: math/src/distributions.rs, math/src/random.rs, simulation/src/statistics.rs, simulation/src/lib.rs.

Tests: math/tests/deep_review.rs, simulation/tests/deep_review.rs.

Review: docs/qa/simulation-review.md.

No changes were made to optimization engines by this cross-review task.
