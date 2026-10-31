# Compiler model and automatic differentiation review

Date: 2026-10-31. Reviewer scope: the live Solvercraft types, model, compiler and reverse-mode differentiation sources in this workspace. This is a review of the current working files; no Git commit identity was inferred from the local snapshot.

The review reproduced six numerical defects before their fixes. It also identified late compiler metadata validation from source inspection. The authorized fixes are implemented, and the final targeted run passes **57 tests**, including **29 new independent regression/control tests**. Targeted Clippy with warnings denied and formatting checks pass.

## Reproduced defects and fixes

| ID | Executed pre-fix case | Observed defect | Fix and current source |
|---|---|---|---|
| CQA-01 | Canonicalize coefficients 1e16, 1, -1e16 on the same variable | The exact residual coefficient 1 disappeared; the resulting model had no linear term | Checked Neumaier accumulation and final compensated values in model normalization, [model lib](../../crates/model/src/lib.rs#L185) and [aggregate_linear](../../crates/model/src/lib.rs#L305) |
| CQA-02 | Compile (1e16*x + x) - 1e16*x | The compiler also erased coefficient 1 | Two-component coefficients are preserved through addition, scaling, multiplication and division before final emission, [compiler Coefficient](../../crates/compile/src/lib.rs#L196) |
| CQA-03 | Reverse derivative of (1e16*x + x) - 1e16*x at x=0 | Returned gradient 0 instead of the independently calculated operator-chain gradient 1 | Compensated per-variable reverse accumulation, [AD finalization](../../crates/compile/src/autodiff.rs#L167) |
| CQA-04 | Compile (ln(x))^0, then evaluate the original expression at x=-1 | Compilation produced constant 1 while the source evaluator retained a logarithm domain error | Zero powers of unsupported variable-dependent bases remain conservatively uncompiled, [power compilation](../../crates/compile/src/lib.rs#L374) |
| CQA-05 | Evaluate and differentiate abs(x)^0 at x=0 | Scalar value was 1, but AD failed on the irrelevant derivative of abs(0) | AD evaluates the base value/domain without differentiating that subtree for exponent zero, [AD zero power](../../crates/compile/src/autodiff.rs#L35) |
| CQA-06 | Compile finite constant 1e-320 / 1e-320 | Reciprocal-first implementation overflowed even though the quotient is exactly 1 | Polynomial coefficients divide directly by the finite nonzero divisor, [divide](../../crates/compile/src/lib.rs#L298) |

The initial compiler test run had 7 passes and 4 failures; the independent model run had 8 passes and 1 failure. After those five fixes passed, the added repeated-variable AD cancellation test reproduced CQA-03 separately. All are active regression tests, not ignored tests.

Neumaier sums also cover objective terms and the offset together, and row terms and the negative right-hand side together. Collapsing a dot product before adding an offset/RHS could otherwise lose the residual again. Tests include distinct-variable cancellation, a large offset, and a row whose residual is exactly 1.

## Source-supported allocation guard improvement

Previously, compile constructed and cloned output names/variables/constraint names before its final model::validate call checked their lengths and the memory estimate. No OOM experiment was performed.

Compilation now validates names and variable domains before polynomial work or output cloning. It estimates output metadata and subtracts that reservation from the polynomial working budget. New tests check early oversized-name rejection and an oversized-metadata/small-budget case. See [compile metadata checks](../../crates/compile/src/lib.rs#L447).

## Approved contract hardening

- Explicit integer bounds and integer candidates with magnitude at or above 2^53 return unsupported_integer_precision. This prevents declaring an integer result trustworthy beyond the range where f64 represents every integer. Large continuous values remain permitted.
- Expr, ExpressionConstraint and ExpressionModel reject unknown JSON fields. The root integrator handles strictness of the shared DTO types.
- Shared model helpers now provide compensated_sum, compensated_dot, CompensatedSum, validate_variable and INTEGER_PRECISION_LIMIT. They are available to engine code without duplicating the summation or domain policy.
- Existing function signatures remain available.

These are explicit policy changes approved by the integrator. They are distinguished from the six pre-fix numerical reproductions above.

## Executed coverage

The new files are [model deep review](../../crates/model/tests/deep_review.rs) and [compiler deep review](../../crates/compile/tests/deep_review.rs).

The review checked:

- Canonical decimal-string seed output and exact u64 roundtrips for 0, 1, 2^53+1 and u64::MAX; invalid negative, fractional, oversized and nonnumeric seeds.
- Nonfinite coefficients, offsets, bounds, RHS values and candidates; invalid handles even with zero coefficients; oversized variable counts, names and term lists.
- Binary intersections, integer-domain holes, fixed/free/negative variables, candidate integrality, all three constraint relations and original-unit violations.
- Exact zero removal without tolerance-based removal of a nonzero 1e-300 term; symmetric quadratic terms and independently computed cross-objective values.
- Structural affine/quadratic classification, negative powers represented by reciprocal expressions, unsupported-variable denominators and domain errors.
- Independently calculated multivariable values and gradients over sin, cos, exp, ln, sqrt, powers, division, abs and selected min/max branches.
- Explicit nondifferentiability errors for active abs(0), sqrt(0), and tied min/max; valid zero-power handling without erasing base-domain checks.
- The exact allowed depth boundary and one level beyond it; a shallow balanced 131071-node tree exceeding the node budget; no recursive drop stress on extreme depth.
- Strict expression JSON fields and finite tiny-constant division.
- Compensated coefficients retained through scaling, constant division and quadratic multiplication.

## Bounded high-exponent signoff

A final timeout-safe test covers exponents 2^31, 2^31+1 and u32::MAX through scalar evaluation, structural classification/compilation and reverse AD. It verifies constant 1, negative-one parity, variable values 1/0/-1, gradients, finite-overflow rejection for 2 raised to these powers, and deserialization rejection outside the u32 range.

The probe runs in a worker with a five-second receive deadline so a future exponent-sized loop cannot block the test indefinitely. The executed test completed in the harness-reported 0.00 seconds. Constant powers also compiled under a 4096-byte options budget.

Source inspection found f64::from(u32) conversions and powf calls, with no signed exponent cast or exponent-driven iteration. AD subtracts one only after exponent zero is excluded. No new production defect was found, and this signoff changed only the test and this report. Native powf execution was checked; no platform-independent library-complexity proof is claimed.

## Verification commands and results

Environment: Windows native, rustc 1.91.1, cargo 1.91.1. Build target directory: C:/workspace/solvercraft-targets/grid-review. CARGO_INCREMENTAL=0 was used for the final runs because this filesystem cannot hard-link incremental cache files.

Commands executed:

~~~text
cargo test -p solvercraft-model -p solvercraft-compile --locked --offline
cargo clippy -p solvercraft-model -p solvercraft-compile --all-targets --locked --offline -- -D warnings
cargo test -p solvercraft-compile --test deep_review high_u32 --locked --offline -- --nocapture
cargo clippy -p solvercraft-compile --test deep_review --locked --offline -- -D warnings
rustfmt --edition 2024 --check crates/model/src/lib.rs crates/model/tests/deep_review.rs crates/compile/src/lib.rs crates/compile/src/autodiff.rs crates/compile/tests/deep_review.rs
~~~

Final test totals: compiler unit tests 17; compiler independent review tests 17; model unit tests 11; model independent review tests 12. All 57 passed, with none ignored. Doc tests contained no executable cases. Targeted Clippy and formatting completed with exit code 0.

An initial Clippy run rejected a constant assertion in a new test; that redundant assertion was removed and Clippy rerun successfully.

## Limits of this review

This is not a proof that every expression, coefficient scale or platform is correct. It did not execute the full Gridcraft host, visual UI, WASM, a different Rust toolchain, an allocator fault campaign or all possible deserialization/deep-AST lifecycle paths.

Floating-point expression evaluation, compensated normalized arithmetic and different operation orders need not be bit-identical on ill-conditioned expressions. Products that overflow before a mathematically canceling sum remain structured numerical errors. There is no arbitrary-precision or exact-rational certificate.

Compiler work is bounded by depth, node, operation and term limits. Memory checks are conservative structural estimates, not a measured global allocator/RSS cap: several polynomial intermediates may coexist. Compiler/preflight does not take a RunControl callback, so cancellation latency includes this bounded phase. Gradient tape limits are fixed, rather than selected by an individual SolveOptions memory setting.

Min/max ties and other algebraically smooth composites can still be conservatively rejected by local operator-based differentiation. That produces an explicit error rather than an invented zero derivative.

The original model still needs independent candidate verification at each backend boundary. The host must preserve its snapshot/revision checks; this review does not replace them.

