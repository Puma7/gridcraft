# Solvercraft acceptance cases

Last updated: 2026-10-31. Status: acceptance specification.

These cases define independent checks for the initial core and later host integration. A row below is not a passing-test claim. Executed test names, results, code revision, toolchain and target belong in the integration test report. Features outside the implemented capability set must return an explicit unsupported error rather than a placeholder answer.

The current scope and unresolved gates are described in [reference.md](reference.md).

## Numerical comparison rules

Compare feasibility and objective values, not identical solution vectors when several optima exist. Recompute the objective from the reported variable values and check the original, unscaled constraints and variable domains. State absolute/relative tolerances in the report.

For a constraint residual, a suitable policy is an explicitly specified combination of absolute and relative tolerance based on the original row scale. Use the same policy in validation and reporting. Neither tolerance nor scaling should silently widen an integer domain. If an integer value is rounded for presentation or application, validate the rounded assignment again.

For minimization, a valid best bound must not exceed the verified incumbent beyond tolerance. For maximization the ordering reverses. A relative gap is absent unless a feasible incumbent and a valid bound both exist.

## Contract and status cases

| ID | Requirement | Input or trigger | Expected result |
| --- | --- | --- | --- |
| SC-A01 | R001-R003, R096 | JSON model round-trip with free, one-sided, two-sided and fixed variables | All values and domain semantics preserved; no NaN/Infinity bound encoding |
| SC-A02 | R028-R029 | Budget stop before a solution, and stop after a verified incumbent | Termination is a limit; feasibility and optional solution reflect actual evidence; no invented optimality or gap |
| SC-A03 | R028, R090 | Local search stagnates or encounters an invalid expression | Appropriate error/stop or unknown status; no model-infeasibility or optimality claim |
| SC-A04 | R037, R095 | Seed 9007199254740993 and maximum supported seed through external JSON | Exact seed survives; external decimal-string convention is enforced |
| SC-A05 | R021 | Request unsupported class or method | Specific unsupported-capability error; no silent algorithm substitution |
| SC-A06 | R044, R089-R090 | Nonfinite coefficient/result, invalid index, unsupported schema version or oversized request | Structured validation error before unsafe access or large allocation |
| SC-A07 | R027-R029 | Returned values, objective and bound disagree | Reject or downgrade result; do not apply an inconsistent solution |

Serialization tests must cover semantic validation. Serde deriving alone is not a proof that public numeric inputs or outputs are valid.

## Linear and integer optimization cases

| ID | Requirement | Model | Independent reference |
| --- | --- | --- | --- |
| SC-L01 | R012 | Maximize 3x+5y; x<=4; y<=6; 3x+2y<=18; x,y>=0 | Objective 36 at x=2,y=6 |
| SC-L02 | R012 | Maximize x+y; x+y<=1; x,y>=0 | Objective 1 at any feasible point on x+y=1 |
| SC-L03 | R012 | Minimize x with x>=1 and x<=0 | Infeasible, justified by conflicting bounds |
| SC-L04 | R012 | Minimize -x with x>=0 and no upper bound | Unbounded only if supported by the LP algorithm's valid detection path |
| SC-L05 | R003, R012 | Equality constraints, a free negative variable and fixed variables | Compare with an analytically eliminated system |
| SC-L06 | R013 | Maximize x+y; 2x+2y<=3; x,y binary | Integer optimum 1; continuous relaxation is 1.5 |
| SC-L07 | R013 | Small integer domains including negative values | Exhaustive enumeration supplies the optimum independently of branch and bound |
| SC-L08 | R027 | Integer-rounding/big-M trap | A value within integrality tolerance is not blindly rounded into a violated constraint |
| SC-L09 | R024 | Limited search with and without a feasible incumbent | Honest partial result; no proof created by budget exhaustion |
| SC-L10 | R089 | Large proposed tableau/nonzero expansion and checked arithmetic boundaries | Reject before allocation when the configured budget is exceeded |
| SC-L11 | R012-R013 | Permuted constraints/variables and duplicate linear terms | Equivalent objective/feasibility; stable origin mapping and no foreign-index aliasing |

For SC-L08, a simple validation trap is x<=10^7*b with a reported tiny positive binary value. Rounding b to zero changes the allowed x. Validate exactly the assignment that would be displayed/applied. The test need not rely on a particular backend actually producing that assignment.

Sparse model input does not prove sparse internal storage. Record internal representation and peak memory separately before claiming large-model capability.

## Compiler and nonlinear regression cases

| ID | Requirement | Case | Expected result |
| --- | --- | --- | --- |
| SC-C01 | R006, R009 | Supported affine expression | Structural extraction agrees with direct evaluation over independent points |
| SC-C02 | R007 | Products of two decision variables | Classified as quadratic, not affine; coefficient convention has no implicit half |
| SC-C03 | R009 | Nonlinear expression constructed to match finitely many sampled points | Never accepted as a proved affine model from those point tests |
| SC-C04 | R043-R044 | Unknown operator, variable-dependent branch/reference, cycle or invalid domain | Conservative classification or explicit rejection with source provenance |
| SC-N01 | R017-R018 | Minimize (10^7*x-4)^2 on [0,5e-7] from zero | Minimum 0 at 4e-7; do not report success at x=0/objective 16 |
| SC-N02 | R017, R028 | Minimize -10^16*x on [0,1] from zero | Finite minimum -10^16 at x=1; never infer unboundedness from magnitude alone |
| SC-N03 | R017 | Minimize (x-2)^2 on [0,10] from zero | Objective 0 at x=2 |
| SC-N04 | R010-R011 | Supported smooth expressions at interior, boundary and narrow intervals | Derivatives agree with analytic references; unavailable differences are not zero derivatives |
| SC-N05 | R017, R079 | Nonconvex stationary point or failed local solve | No unjustified local-minimum, infeasibility or global-bound claim |
| SC-N06 | R017, R021 | Bound-only AD solver receives integer variables or a general constraint | Explicit unsupported error; no silent constraint dropping |
| SC-N07 | R019-R020 | Finite-bound differential evolution on a nonsmooth objective with expression constraints | Verified feasible BestFound point only; no global bound/optimality or infeasibility certificate |
| SC-N08 | R019, R024 | DE receives an open bound, empty integer interval, invalid objective domain or cancellation | Domain-aware error/stop and no fabricated successful member |

SC-N01 and SC-N02 are regression cases previously reproduced in the original Gridcraft nonlinear routine using an isolated Rust harness. Their full Gridcraft workbook/command paths were not executed in that investigation. They must be re-run against whichever Solvercraft implementation is adopted.

For the current structured quadratic model, SC-N01 can be expressed as quadratic coefficient 10^14, linear coefficient -8*10^7 and offset 16. The reference evaluation should also use the direct square expression to detect cancellation or convention mistakes.

## Sampling, distributions and statistics cases

| ID | Requirement | Case | Expected result |
| --- | --- | --- | --- |
| SC-S01 | R049-R050, R064 | Same seed/binding/trial/replicate in serial, reordered and differently batched evaluation | Same random inputs on the same tested platform; no dependence on scheduling |
| SC-S02 | R049, R064 | Read one stochastic binding twice in a trial; use two independent bindings with identical formulas | First two reads match; separate bindings have distinct streams unless explicitly linked |
| SC-S03 | R045, R047 | Every implemented distribution at valid and invalid parameter edges | Correct support, parameter meaning and structured errors |
| SC-S04 | R050 | Uniform [a,b] and fixed-seed normal ensembles | Compare support and analytic moments under predeclared nonflaky thresholds |
| SC-S05 | R051 | Latin Hypercube in each latent uniform dimension | Exactly one sample in each of n strata, before any discrete inverse-CDF plateaus |
| SC-S06 | R056 | Constant output and one valid output | Constant variance is zero; unbiased sample variance is absent for n=1 |
| SC-S07 | R044, R056 | Zero trials, no outputs, changing output arity or invalid output | Explicit documented behavior; no fabricated mean or hidden invalid-trial deletion |
| SC-S08 | R056 | Large-offset samples such as 10^12+1, +2, +3 | Stable mean/variance, checked against centered arithmetic |
| SC-S09 | R057 | Known ordered corpus, repeated values, p=0 and p=1 | Declared empirical quantile method; exact and approximate methods identified |
| SC-S10 | R058 | Forty equally weighted losses: 39 zeroes and one value 10 | At confidence 0.95, left-inverse empirical VaR=0 and upper-tail CVaR=5 |
| SC-S11 | R060 | IID MC, LHS and later randomized-QMC outputs | Only uncertainty estimates justified for the actual sampling method |
| SC-S12 | R024, R089 | Cancel or memory limit partway through simulation | Counts and summaries describe completed valid trials; bounded retained storage |
| SC-S13 | R053-R055 | Invalid and valid correlation matrices/copula parameters | Check symmetry, diagonal and PSD/domain conditions; never repair silently |
| SC-S14 | R059 | Fitting parameters from the same sample used for goodness-of-fit | Use an appropriate adjusted/bootstrap procedure; no naive fixed-parameter p-value |
| SC-S15 | R047-R048 | Tails, degenerate distributions, undefined theoretical moments | Domain-aware result; no fabricated finite moment or silently clipped sample |

SC-S10 exercises the discrete quantile boundary: the worst 5% consists of the one high loss (2.5% mass) and 2.5% zero mass. Averaging all samples at or above VaR would incorrectly produce 0.25. Use upper-tail expected-shortfall semantics with an explicit loss convention.

For fixed seeded statistical tests, select an ensemble of seeds and thresholds from analytic error bounds or separate reference calculations. Do not repeatedly tune the test threshold to the observed implementation. A single random goodness-of-fit rejection is not a robust regression gate.

Version the generator, stream addressing, distribution transformations, LHS permutation algorithm and quantile convention. Portable integer RNG output does not automatically guarantee bit-identical transcendental transforms on every CPU/browser. State tested reproducibility separately from cross-platform numerical equivalence.

## Host integration cases

| ID | Requirement | Case | Expected result |
| --- | --- | --- | --- |
| SC-H01 | R004, R041-R044 | Multi-sheet snapshot vs existing Calc/Trial on supported formulas | Equal results; live workbook and history unchanged |
| SC-H02 | R022-R024, R033 | Job running while UI/CLI/MCP polls, edits or cancels | Status remains reachable; explicit budgets and measured cancellation latency |
| SC-H03 | R025-R026 | Apply a result to unchanged workbook, then undo/redo | One atomic history operation; originals restored by one undo |
| SC-H04 | R025-R026 | Edit a bound cell, rename/delete a sheet, alter protection or create a spill after job starts | Reject stale/invalid application; do not overwrite current state |
| SC-H05 | R005, R037-R040 | Multiple stored models through Gridcraft XLSX/JSON round-trip | Model fields and structural references survive; legacy subset migration is tested |
| SC-H06 | R030-R032 | Solvercraft button beside existing Solver and panel workflow | Original Solver remains usable; keyboard/focus/theme/label behavior is visually checked |
| SC-H07 | R034-R036 | Request answer, dual or sensitivity report | Only mathematically valid available data are shown; optional reports disclose unavailable capabilities |
| SC-H08 | R088, R093 | Native core, full Gridcraft build and actual WASM/browser run | Each receives a distinct executed result; one does not substitute for another |
| SC-H09 | R083-R084 | Independent host using the core with no workbook or egui dependency | Same supported model solves through the reusable SDK |
| SC-H10 | R089-R090 | Empty commands, hostile ranges, huge nesting and malformed persisted model | Structured errors with bounded allocation/recursion; no panic or partial mutation |

The suggested UI goals in the masterplan (status within 250 ms and cancellation within 1 s at defined polling points) require measurement on stated hardware and representative models. Existence of a callback or an asynchronous wrapper is not that measurement.

## Finite-scenario and robust-counterpart cases

| ID | Requirement | Case | Expected result |
| --- | --- | --- | --- |
| SC-R01 | R065-R067 | One x in [0,1], equal scenario weights and affine loss slopes [10^17,-1,-10^17] | Weighted expected loss is -x/3; optimum x=1, with compensated mean -1/3 rather than zero |
| SC-R02 | R058, R067 | Weighted CVaR of [10^17,-1,-10^17] with equal weights at confidence 0 | Equals the weighted mean -1/3 despite cancellation |
| SC-R03 | R058 | Constant loss 10^-310 and confidence 1 minus machine epsilon | CVaR equals the constant; avoid underflow from multiplying by tail mass before normalizing |
| SC-R04 | R070 | Nominal row x<=3 with coefficient radius 2, x in [-10,10], minimizing x | Counterpart is x+2*abs(x)<=3; optimum x=-3, absolute auxiliary 3 |
| SC-R05 | R089 | CVaR/robust expansion exceeds final variable, row, term or byte cap | Reject before model cloning or auxiliary allocation, including base names/terms and generated content |
| SC-R06 | R070 | Duplicate/invalid row index, non-<= robust row, wrong radius dimension or negative/nonfinite radius | Structured error; do not alter the supplied model |
| SC-R07 | R095, R089 | Model name with JSON-escaped control characters expands beyond manifest byte budget | Bounded streaming fingerprint fails without first creating the oversized serialized buffer |

These are alpha finite-scenario tests. They do not imply that the wider R3 risk-optimization gate below is complete.

## Later risk-optimization gates

Use a small analytic newsvendor or finite scenario model for independent reference. Training and validation scenarios must have separate seeds. A candidate comparison should use fixed training scenarios/Common Random Numbers. A chance constraint needs a clear distinction between individual and joint probability and an independent violation interval.

For two-stage models, first-stage values are identical across scenarios. Recourse may depend only on information available at its decision time. A robust uncertainty set is not automatically a calibrated probability statement. Big-M values require derived finite bounds. Local nonlinear values cannot supply global MINLP relaxation bounds.

## Required integration evidence

Before changing a feature's status from planned/experimental to supported, record:

1. Implementation entry point and exact public capability.
2. Relevant acceptance IDs and actual test names/results.
3. Original-model feasibility, objective and any bound verification.
4. Native target/toolchain and any separately tested WASM/browser target.
5. Resource limits, cancellation behavior and measured exceptions.
6. Dependency versions, enabled features and provenance.
7. Known unsupported classes and remaining differences.

The standalone core should run its offline workspace tests, formatting and clippy gates. Gridcraft's `cargo xtask ci`, asset/layer checks and actual host/UI/WASM verification remain additional required gates before claiming a complete integrated release.

## Revision history

| Date | Change |
| --- | --- |
| 2026-10-31 | Initial independent numerical, sampling, status and integration acceptance cases |
| 2026-10-31 | Add restricted AD/DE behavior and seven finite-scenario, robust and manifest regressions |

