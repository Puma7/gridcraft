# Independent numerical validation

Validation baseline: Solvercraft alpha under development, 31 October 2026.
The cases here are synthetic mathematical references authored for this project. Expected optima are derived from equations or complete finite enumeration, not obtained by running the backend.

## Executed checks

The final independent integration run used:

    CARGO_TARGET_DIR=C:\workspace\solvercraft-targets\validation
    cargo test -p solvercraft --test independent --offline
    cargo clippy -p solvercraft --test independent --offline -- -D warnings

Environment: Rust 1.91.1, native x86_64 Windows. The final independent integration run passed **39 tests, zero failures, zero ignored**. Clippy completed successfully with no code diagnostics. The environment emitted incremental-cache hard-link warnings and used its copy fallback; no test failure resulted. Test source was formatted with rustfmt.

## Mathematical references

The source fixture tests/reference-cases.json records 16 linear/mixed-integer cases and three simulation/statistics references, together with their derivations. They include:

| Reference | Independent conclusion |
|---|---|
| Wyndor LP | Maximum 36 at (2, 6) |
| Alternative optima | Maximum 1; a unique decision vector must not be required |
| Binary triangle | Integer maximum 1; continuous relaxation maximum 1.5 |
| Contradictory rows | No feasible point |
| Positive unbounded ray | Continuous objective has no finite maximum |
| Free/negative and upper-only variables | No implicit non-negativity may be introduced |
| Fixed variable plus free variable | Fixed values and offsets survive transformation |
| Two equalities | Unique solution (2,1), objective 4 |
| Large finite objective | Magnitude alone is no unboundedness certificate |
| Integer equality x=0.5 | Integer infeasibility |
| Unbounded LP relaxation with x integer=0.5 | No original-MIP unboundedness proof; conservative unknown permitted |
| Small positive blocking coefficient | Finite problem must not be falsely certified unbounded |

A further 24 signed binary optimization models are checked against complete enumeration of all 64 assignments each. Both minimization and maximization occur. The returned values are separately checked against original bounds, integrality and constraint equations.

The narrow-bound quadratic regression minimizes 1e14*x^2-8e7*x+16 over [0,5e-7] and verifies x=4e-7 with objective zero. This exercises the explicit analytic separable-quadratic path; it does not establish general constrained-QP or nonlinear support.

Compiler checks include a cubic that is zero at 0, 1 and 1.7 but remains structurally nonlinear, exact expansion of (x+2y)^2 with no implicit half factor, analytic gradients, invalid variable references, excessive depth, domain failures and nondifferentiable active operators.

The simplex backend is floating point, not an exact rational proof system. Test success establishes the listed finite cases, not universal correctness or industrial MIP parity.

## Simulation and risk references

- Constant output has zero sample variance; a single observation has no unbiased sample variance.
- Large-offset data [1e12+1,1e12+2,1e12+3,1e12+4] preserve mean 1e12+2.5 and unbiased variance 5/3. Normal MLE fitting is separately checked to use population, not sample, variance.
- A seed exceeding JavaScript's exact integer range must serialize as a decimal string and roundtrip without losing bits.
- Latin Hypercube samples visit each of 128 strata exactly once per dimension.
- Identical seed and options reproduce the addressed sample stream.
- A fixed 8192-trial ensemble is checked against the analytic moments of Normal(0,1) and Uniform(2,8), using broad, predeclared validation bands. These bands do not constitute a general confidence-interval API.
- Cancellation before a run does not invoke the evaluator.
- Changes in evaluator output arity are errors.
- For losses [0,0,0,10], upper-tail CVaR at confidence 0.5 equals 5. Conditioning on every sample greater than or equal to VaR would incorrectly return 2.5.
- The same partial-mass convention holds for weighted losses [0,10] with weights [3,1].
- For equally likely scenario losses x and 4-3x on x in [0,2], expected loss is minimized at x=2, whereas CVaR at confidence 0.5 is minimized at x=1.
- Robustifying -x<=4 with independent symmetric coefficient radius 1 gives -x+abs(x)<=4. Over x in [-10,0], minimizing x yields -2, not -4.

## Findings addressed during implementation

1. The initial shared DTOs represented seed values as raw JSON integers despite a promised decimal-string interface. The integrator added a string codec, tested with u64::MAX and 9007199254740993.
2. An unbounded continuous relaxation cannot establish integer unboundedness when no integer-feasible point exists. The engine now keeps this case conservative.
3. A tiny positive coefficient that blocks a ray cannot be discarded by a pivot threshold and then used to claim unboundedness. The engine reports a numerical stop for the ill-conditioned case instead.

These were implementation review findings, not newly published GitHub issues.

## Unverified gates

This native core validation does not establish:
- complete Gridcraft build success (the checkout requests Rust 1.95);
- UI behavior or screenshot fidelity;
- workbook revision/undo and XLSX persistence in the full app;
- actual execution in a WebAssembly browser;
- large sparse or hard mixed-integer performance;
- general constrained-QP/SOCP/general-NLP/global nonlinear parity;
- external-engine integration;
- stochastic recourse, chance constraints, copula or Sobol support;
- statistical certification of RNG quality or general-purpose Monte Carlo confidence intervals.

No build, benchmark or platform result is implied where it was not run.

