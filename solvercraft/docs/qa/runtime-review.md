# Solvercraft runtime and guarantee review

Review date: 2026-10-31.

The review started from PR predecessor **690a098a6c4c9d966d66b6c30c036e0e42534bfd**, based on Gridcraft **981fad14e0b90a7906a02f29785a6d0eb214fee6**. The local checkout is a materialized source tree without Git metadata. The source identities below identify the reviewed local fixes; the integrator associates the final commit with the consolidated verification report.

Scope: facade risk.rs, jobs.rs, provenance.rs, seed codecs, advertised README guarantees and the 96 masterplan requirements. This was functional/numerical QA, not a security certification or a claim of complete solver parity.

## Conclusion and release boundary

Two additional confirmed defects were found and fixed during this review:

1. Weighted CVaR could clip the final scenario's probability mass and return the wrong sign at confidence zero.
2. The unit expected_loss risk variant silently accepted additional fields despite a deny-unknown-fields annotation.

The examined finite counterexamples now pass. No unresolved executed defect remains in these reviewed facade paths at the recorded source identities. This is not a proof of absence of other defects.

The integrator's final frozen standalone run recorded **265 tests passed, zero failed, zero ignored**, plus formatting, all-targets Clippy with -D warnings, release build/examples and a Linux cross-target compile check. The current evidence is [verification-report.json](verification-report.json). The original 136-test report remains historical and must not be mistaken for the expanded QA run.

The complete Gridcraft build, **17 host integration tests (4 existing + 13 new)**, visual UI validation and actual WASM/browser execution remain unexecuted. Their source was inspected, not behaviorally proven. Full R1/R2/R3 and Frontline parity remain open; see [coverage-matrix.md](coverage-matrix.md).

## Confirmed defect 1: weighted tail mass rounding

The previous helper repeatedly subtracted normalized probabilities from a scalar remaining tail. With losses [10^17,-1,-6*10^16], raw weights [3,2,5] and confidence 0, it returned **3.8**. Confidence-zero CVaR must equal the weighted mean:

    (3*10^17 - 2 - 5*6*10^16) / 10 = -0.2

After subtracting 0.3 and 0.2, the scalar remaining mass became slightly smaller than 0.5. The helper clipped the final scenario's 0.5 mass; the tiny mass error multiplied a large negative loss. Compensating only the loss sum did not repair the incorrect mass.

Executed controls included [0,10] with weights [3,1], confidence 0.5 (expected 5), and a tiny constant loss 10^-310 near confidence one (expected the same constant). Both controls already passed; they did not expose the unequal-weight clipping defect.

### Correction

The risk implementation now:

- scales weights and losses by powers of two within its supported numerical range;
- accumulates products with multiplication residuals and compensated sums;
- computes confidence-zero CVaR through weighted-mean arithmetic, not a clipped tail;
- retains a compensated tail-mass remainder and the residual of 1-alpha;
- preserves low mass components at a fractional boundary;
- rejects unsupported dynamic range or severe numerical conditioning rather than returning an unqualified number;
- checks the transformed optimizer objective against the independently recomputed original scenario risk measure.

The conditioning check is an engineering guard, not a rigorous interval/error certificate. No exact-arithmetic or all-finite-input guarantee is claimed.

### Executed independent checks

| Case | Independent expected result | Current result |
| --- | --- | --- |
| Unequal raw weights [3,2,5], losses [10^17,-1,-6*10^16], confidence 0 | -0.2 | -0.2 |
| Losses [3*10^16,-1,-2*10^16], same weights, actual binary f64 confidence 0.05 | -0.15209352501972861 | Matches; all six input permutations agree |
| 128 small integer-weight tail cases | Exact integer mass accounting for rational confidence levels | Within predeclared tolerance |
| Equal weights at 1, f64::MAX and minimum subnormal | Mean of [2,4] is 3; upper half-tail is 4 | Matches |
| Zero-weight maximum-f64 outlier with tiny active constant | Active constant is unchanged | Matches |
| Expected-loss optimization with two maximum-f64 weights | Same normalized decision as equal weights; x=2, expected loss 0 | Matches |
| Extreme signed losses whose conditioning exceeds the supported range | Explicit numerical failure | No fabricated successful risk value |

The fractional-confidence oracle used Python Decimal at precision 100 with Decimal.from_float for the confidence. Using decimal 0.05 as though it were exactly the binary input would create a misleading reference on such highly conditioned data.

## Confirmed defect 2: strict unit-variant decoding

Serde's internally tagged unit expected_loss variant accepted a confidence field even when the enum had deny_unknown_fields. A caller could therefore provide a configuration intended for a tail-risk model and receive expected-loss behavior.

The public API/serialization remains compatible, but deserialization now uses a strict helper with an empty struct variant. Extra fields on expected_loss, conditional_value_at_risk, Scenario, ScenarioProblem and BoxUncertainty are rejected. The review test first reproduced the ignored unit-variant field, then passed after the correction.

## Mathematical assumptions checked

### Finite-scenario optimization

The implemented loss in each scenario is affine in the decision values, and the supplied base constraints are deterministic linear constraints. Weights are finite and nonnegative with positive mass. CVaR uses the upper tail of losses and a confidence in [0,1). The formulation introduces a free threshold and nonnegative excess variables, minimizing threshold plus weighted excess divided by tail probability.

This is a finite supplied-scenario formulation. It does not supply sample generation, independent out-of-sample testing, a confidence guarantee, stage/recourse semantics or chance constraints. Equal or unequal scenario weights are not evidence that a user's scenarios represent a population distribution.

Returned decision_values excludes the formulation's threshold/excess auxiliaries; solve.values describes the transformed optimization model. Callers must not treat that longer transformed vector as an original workbook decision range.

### Independent coefficient boxes

For a selected <= row, the implemented counterpart is:

    nominal_coefficients * x + sum(radius_i * abs(x_i)) <= rhs

It is valid for independent symmetric coefficient intervals and either sign of x. It does not model an uncertain right-hand side, correlated/ellipsoidal/budgeted sets, or arbitrary robust nonlinear models. Nonnegative continuous absolute-value auxiliaries enforce both sign inequalities.

The negative-coordinate control minimizes x under x+2*abs(x)<=3 and [-10,10], yielding x=-3 and absolute auxiliary 3. A second independent pre-existing control robustifies -x<=4 on [-10,0] and yields x=-2. Inputs are not modified.

## Resource, cancellation and validity review

| Area | Checked behavior | Remaining boundary |
| --- | --- | --- |
| Scenario compilation | Checks future variables/rows/terms, base indices/names/coefficients, confidence and memory estimate before copying/expansion | Estimates are not measured process-RSS limits; very large permitted input processing remains work |
| Robust counterpart | Counts +k variables, +2k rows and +5k terms for k positive radii; validates selected rows, dimensions and final limits before clone | Only <= coefficient boxes supported |
| Weighted tail helper | Fixed one-million-observation cap, bounded reserves and in-place unstable ordering | No caller-specific memory option for this standalone helper |
| Original risk measure | Recomputed losses/mean/CVaR are finite and checked against the optimizer objective | Error/range checks are floating point, not an exact rational certificate |
| Manifest | JSON streams into a byte-limited diagnostic hash writer | Byte bound is not a total application-memory quota |
| Native jobs | Four active jobs, structured failure/cancellation, panic containment and slot recovery | Arbitrary user evaluator work/allocations are the evaluator's responsibility |
| Completed results | Shared Arc result objects; repeated snapshots share the same allocation | Callers retaining handles/results retain their memory |
| Seeds | Maximum u64 and 2^53+1 serialize as decimal strings and round-trip exactly | Integer JSON input is accepted for compatibility; upstream client rounding cannot be repaired |

Pre-existing fixes were rechecked: escaped 4096-byte model names can expand beyond the manifest budget without creating a complete oversized JSON buffer; oversized CVaR/robust transformations fail their preflight; invalid base coefficients do not reach model copying; tiny constant tails and original opposing-coefficient cancellation controls remain valid.

## Native job lifecycle evidence

The added test starts four actual simulation workers and uses channels to hold all four inside their first evaluator call. It confirms that a fifth job is refused while those workers are active, rather than relying on a race with a very short solve.

After release, one evaluator deliberately panics, one job is cancelled, and two complete. The test checks:

- the panic becomes job_panic/Failed;
- cancelled work returns a cancelled result rather than fabricated completion;
- completed snapshots share their Arc allocation;
- a new real solve can start after slot guards release.

The SDK can catch unwinding evaluator panics on the tested native build. It cannot promise to recover from process abort, allocation abort, a panic-abort build or a user evaluator that never returns. Cancellation is cooperative at implemented polling points; hard real-time interruption of arbitrary evaluator code is not provided.

OS thread-creation failure, ID exhaustion and poisoned-lock branches were source-inspected but not forced in runtime tests. Actual WASM job behavior was not run; the native thread API explicitly rejects that host instead of claiming an asynchronous browser implementation.

## Reproducibility manifest assessment

The manifest stores model fingerprint, schema/package version, options/seed and platform. The FNV-1a-64 fingerprint is explicitly diagnostic. It is not authentication, a collision-resistant authorization token, a workbook revision guard or an algebraically canonical model identity. Reordering equivalent terms can legitimately change it.

The manifest does not yet provide a complete reproducibility bundle: compiler/build identity, host workbook revision, run identity and a corresponding simulation manifest remain separate work. Those omissions are classified as partial coverage, not silently satisfied by a hash.

## Executed review commands

Using CARGO_TARGET_DIR=C:\workspace\solvercraft-targets\reference-review:

    cargo test --manifest-path solvercraft/Cargo.toml -p solvercraft --test runtime_review --locked --offline
    cargo test --manifest-path solvercraft/Cargo.toml -p solvercraft --locked --offline
    cargo clippy --manifest-path solvercraft/Cargo.toml -p solvercraft --all-targets --locked --offline -- -D warnings

The final additive runtime_review suite passed **10 tests**, zero failures/ignored. The full facade run immediately before its last additive control passed 80 tests (30 unit, 39 independent, 2 separate runtime lifecycle, 9 review). The consolidated final 265-test workspace run includes the final 10-test review suite. The nine-test intermediate run had 128 exact rational subcases inside one test function; those subcases are not added to Cargo's test count.

Formatting was applied to the two owned Rust files. Clippy passed. Incremental-cache hard-link warnings used the toolchain's copying fallback; they were environment warnings, not test or code-lint failures. No full Gridcraft/UI/WASM result is inferred from these commands.

## Source identities

The hashes below identify the locally reviewed source state rather than inventing a Git HEAD for a directory without Git metadata.

| File | SHA-256 |
| --- | --- |
| crates/solvercraft/src/risk.rs | dbb77ccce215af177bba14a37ccf4cdfe092bb67856757d5950d755c9d2f592f |
| crates/solvercraft/src/jobs.rs | 063dbf65c6f7e32fd4f83124ab4bc02e1da9cc7b2b36281adb84b42e57bda6e3 |
| crates/solvercraft/src/provenance.rs | 226760ca0908e7c09b85422990b2bc0517914655bf40854973eb7a9e9e0faff3 |
| crates/types/src/lib.rs | 133512f6ea8fd4f9e3740242c572eeac73153972a4d58ed4dbe186df687ae6a7 |
| crates/solvercraft/tests/runtime_review.rs | b8baada142d7979889fd5fb2983fa1a37406a4de1fc36c1aafb845498da46314 |

No external issue, pull-request or other publication was performed by this QA worker.

