# Optimization cross review

## Scope and baseline

Reviewed the numerical optimization implementation at PR 1 head **690a098a6c4c9d966d66b6c30c036e0e42534bfd**. The review covered the complete dense simplex, branch-and-bound, separable quadratic engine and their result/budget paths, plus source inspection of the facade projected-gradient and differential-evolution routines. Shared model arithmetic was traced because it validates and reports solver results.

The initial review was performed before the fixes described below. This report distinguishes reproduced baseline defects, executed post-fix regressions and explicit unsupported classes. It is not an exhaustive proof of numerical correctness and does not claim full legacy PSI parity.

## Reproduced baseline defects

### OPT-01 Small terms lost in compensated model arithmetic

At baseline, model/src/lib.rs lines 174–190 used classic Kahan accumulation. The sequence 1e16, 1, -1e16 still loses its lone unit. This affected distinct-variable objective evaluation, duplicate coefficient aggregation and original-constraint validation.

Executed examples:

- Objective 1e16*x + y - 1e16*z with x=y=z=1 was reported as **0**, although its exact value is **1**.
- Fixed values a=1e16, b=1, c=-1e16, and x in [0,1], constrained by a+b+c+x<=0, were reported **Feasible / Optimal at x=0**, although the exact constraint is 1+x<=0 and has no feasible point.
- The corresponding >=1 model rejected its valid x=0 reference point and returned x=1 with an incorrect optimality assertion.

The order-permuted fixed-value control (1e16,-1e16,1) was solved correctly. This established an arithmetic-order defect rather than an unsupported problem type.

**Fix ownership:** shared model/compile reviewer. Model arithmetic now uses checked Neumaier sums, including the RHS in a single constraint residual sum. The original-model checks and reported objective agree on the regression models.

### OPT-02 Constraint translation rounded away a meaningful residual

Baseline simplex.rs line 116 computed the fixed-offset dot product with an ordinary sum. A first compensated subtotal alone is also insufficient: rounding that subtotal before subtracting the RHS can still lose a unit.

Executed residual example: fixed a=1e16, b=1, x in [-2,0], maximize x subject to a+b+x<=1e16. The exact optimum is **x=-1**. The intermediate implementation returned **Optimal x=-2, bound=-2, gap=0**. Its returned point was feasible, so primal-only validation did not detect the false optimality assertion.

**Fix:** the RHS and all negative offset products are aggregated together in one compensated sum. Phase-one and initial reduced-cost aggregation also use the shared checked helper. Original row verification uses the same arithmetic policy.

### OPT-03 An absolute reduced-cost cutoff fabricated zero-gap optimality

Baseline simplex.rs lines 347 and 353 ignored a negative reduced cost smaller than the clamped absolute epsilon.

Executed example: minimize -1e-9*x over [0,1e12]. Baseline returned **Optimal x=0, objective=0, bound=0, gap=0**. The true optimum is approximately **-1000** at x=1e12.

**Fix:** normalize costs after eliminating the current basis, retaining a small cost on a remaining variable even if another constrained variable had a large original cost. A remaining negative reduced cost is not discarded by an absolute gradient cutoff. If normalization loses a nonzero cost or a pivot cannot be trusted, the engine reports a numerical failure instead of an unboundedness/optimality certificate.

### OPT-04 Integer domains outside exact f64 representability were falsely certified

Executed representable input: x fixed at 2^53+2, integer y in [2^53,2^53+2], x-y>=1, maximize y. The exact integer optimum is **2^53+1**, which f64 cannot encode. Baseline returned **Optimal y=2^53, bound equal to that value, gap=0**.

The same odd-integer optimum at a base of 2^52 was solved correctly.

**Fix ownership:** shared model reviewer. Integer bounds and candidates with magnitude at least 2^53 now produce explicit unsupported_integer_precision errors. This is a declared capability boundary, not an assertion that the mathematical model is infeasible.

### OPT-05 Original-coordinate reconstruction could silently lose a whole unit

The previous integer example with continuous variables bypasses an integer-domain guard. Its transformed optimum adds 1 to a lower offset of 2^53; rounding in reconstruction returned the unchanged lower bound. Baseline still claimed **Optimal / zero gap**, despite losing a displacement larger than the declared absolute feasibility tolerance.

**Fix:** original coordinates are reconstructed with checked compensated sums. An independently recomputed reconstruction residual larger than the feasibility tolerance produces **NumericalFailure / Unknown**, rather than a false optimum. Large continuous values are not blanket-rejected.

### OPT-06 A constant objective offset changed the MIP argmax

Executed example: integer x,y in [0,2], 2x+y<=3, maximize 3x+2y+offset.

- offset=0: baseline returned (1,1), variable-dependent objective **5**.
- offset=1e16: baseline returned (0,2), variable-dependent objective **4**, still claiming **Optimal / zero gap**.

The offset caused two branch scores to round to the same f64 value and a better branch to be pruned.

**Fix:** branch comparisons, incumbent selection and pruning use constant-free scores. Relative-gap numerators use these scores; the documented denominator still includes the objective offset. Public bounds are restored with outward rounding, and public gaps conservatively include display rounding. Offset-related certificate rounding is explicitly warned about. A constant-free working score that itself loses a term larger than the optimality tolerance stops with NumericalFailure rather than certifying an unreliable comparison.

Paired positive/negative-offset tests check both objective directions, node limits, identical incumbents/termination and nonzero unfinished gaps.

## Executed independent coverage

The new engines/tests/deep_review.rs contains **19 tests**, including:

- **300** deterministic bounded two-variable LP models compared with an exact i128 rational vertex-enumeration oracle. Constraint scaling spans 1e-6, 1 and 1e6.
- **64** mixed-integer/continuous models compared with an exact oracle that enumerates both integer variables and derives the continuous feasible interval rationally. Each model is checked at **6 node limits** (384 runs), including original-point feasibility, incumbent retention, bound direction and gap invariants.
- Objective-unit scaling controls from **1e-200 to 1e100**.
- Exact representability controls below and beyond 2^53.
- Offset-invariance and interrupted-search controls for minimization and maximization.
- Independent sum-of-squares identities for interior and bound-clipped separable QP solutions, genuine QP unboundedness, zero-evaluation/iteration limits and explicit unsupported cross terms.
- Every reproduced defect above has a permanent regression.

Existing engine tests also remain green: **21 unit tests**. The post-fix selected engine verification therefore passed **40 tests**.

Commands executed successfully:

    cargo test -p solvercraft-engines --locked --offline
    cargo clippy -p solvercraft-engines --all-targets --locked --offline -- -D warnings

Owned Rust files were formatted with rustfmt. Builds used Rust 1.91 and an isolated simulation target directory. No external issue, PR update, publication or repository write was performed by this review agent.

## Source review conclusions and limits

- The projected-gradient facade reports Stationary or BestFound rather than a local/global optimality proof. Its unsupported general constraints and integer domains are explicit errors; those are feature boundaries, not newly confirmed numerical bugs.
- Differential evolution reports BestFound, treats invalid candidates as invalid rather than zero and does not infer infeasibility from failed search. It is a heuristic, not a global certificate.
- The separable QP engine is restricted to continuous bounds-only diagonal convex minimization / concave maximization. General QP, SOCP, MIQP and arbitrary nonlinear constraints are not implemented by this engine.
- Dense-tableau memory limits and cancellation polls are present. Pivot elimination now polls cancellation between rows. No wall-clock cancellation SLA or peak-process-memory benchmark was established by this review.
- These tests concern the independent core. Full Gridcraft workbook/UI execution, platform-specific native integrations and actual WASM execution are separate gates.
- No exact rational certificate is produced by the engine. The reference oracles are deliberately small and exact; passing them does not establish correctness for all ill-conditioned or industrial-scale models.

