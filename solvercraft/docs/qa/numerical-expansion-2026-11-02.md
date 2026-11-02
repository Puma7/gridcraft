# Expanded numerical qualification, 2026-11-02

This expands the first [numerical review](numerical-review-2026-11-02.md) on published commit `e50da565f44f5f9e9f68a1281033e98e6ef312fc`. The new tests use a finite, declared matrix across every advertised method group, together with the existing workspace tests. They found further calculation errors and a cross-operator regression before publication. The tested cases support the documented numerical contracts; they do not prove correctness for all floating-point inputs or all possible models.

## Repairs found by expansion

| Area | Independent counterexample | Result |
|---|---|---|
| Polynomial multiplication | With `a=2^27+1` and `B=2^54+2^28`, `a*a-B` is exactly1. Compiling `(a*x)*a-B*x` or `(a*x)*(a*x)-B*x^2` previously erased that unit coefficient. | Product residuals propagate through exact accumulation. A third expansion component or unrepresentable final coefficient is explicitly refused. |
| Division/multiplication interaction | During the expanded review, the first unpublished product-residual repair compiled `(x/3)*3-x=0` into a tiny nonzero row and falsely classified the original model with `1<=x<=2` as infeasible. | A coefficient quotient must have zero exact division residual before polynomial emission. The final CLI, MCP, workbook and browser tests refuse this case with `numeric_range`; exact `(3*x)/3` and dyadic controls still compute. The earlier candidate was not pushed. |
| Finite differences near bounds | `1024+0.25*(x+2)^2` on `[-3,3]` could report stationarity close to-3 with an almost-zero estimated derivative, despite true derivative about-0.5. | Extremely unbalanced central probes use a resolved larger-side second-order formula. Independent derivative and constrained-solution matrices check the original units. Numerical derivatives still have approximation limits. |
| Risk variance | Equal losses `1e10` and `next_up(1e10)` produced twice the true population variance. Centering at an already-rounded large mean lost the half-ULP displacement. | Center on an actual positive-mass observation, then compute the centered mean and deviations at their own scale. The corrected variance is exactly `9.094947017729282e-13`. |
| Zero-confidence CVaR | Three equal-mass affine losses `[-x-1,-1,x-1]` on `[-2,2]` have constant mean/CVaR-1 at confidence0. Three rounded one-third tail coefficients instead allowed a spurious free-eta recession direction. | Use the exact expected-loss identity at confidence0. Tiny positive confidences whose tail coefficients cannot represent the required strict margin are explicitly refused. |
| Decision costs / exponential utility | Payoff `1e16` with edge costs1 and0 could select the costly action. A one-state CARA problem with payoff0, cost1, tolerance`1e16` could report net information cost0 rather than-1. | Invisible edge/acquisition costs and unresolved utility round trips produce structured precision refusals. Moderate CARA and linear policy matrices remain positive computations. |
| Discrete endpoints and transformed atoms | A categorical law with weights`[1,1e-20]` could return its lower value at quantile1. Rounded affine transforms could lose atom CDF jumps or mass, including coalesced atoms. | Preserve the last positive endpoint; use exact binary64 preimages and stable original interval masses for discrete pushforwards. |
| Distribution tails | Gamma/Chi-squared/Beta/inverse-Gaussian upper inverse tails lost accuracy near1. Reflecting a symmetric law through `1-p` lost lower-tail precision. A huge-scale hyperbolic-secant median moved far from its exact location. | Direct complementary-tail inversions, stable inverse-Gaussian survival/Mills evaluation, symmetric reflection and exact symmetric medians. |
| Gaussian CDF/truncation | The documented legacy Normal CDF approximation was amplified by accepted tail truncations. Inclusive `next_down(lower)` also invented mass for atom-free continuous bounds; tiny Gaussian interval probabilities disappeared. | Stable direct Gaussian tails, exact atom-free lower endpoints and a bounded positive small-interval Gaussian quadrature. Discrete/hybrid inclusive atoms remain distinct. |
| Proof availability | Initially many exact small LP/MILP models with free/one-sided coordinates could not obtain reliable certificates even though simple original rows bounded those coordinates. | Certificate-only outward bounds are derived from canonical one-variable rows. Conic internal stopping is tighter to leave room for independent original-tolerance checks. Public tolerances are unchanged. |

## Independent coverage and counting rules

The complete SDK test count is a count of Rust test bodies. Matrix rows below count declared model/run/query configurations inside those bodies. Multiple scalar checks of the same law, two backend routes for one model and retained/streaming executions are identified separately. These numbers must not be added together and called a count of independent unit tests.

- **6,784 engine/compiler mathematical cases:** 4,608 actual solver route cases, 1,024 algebra/gradient cases, 512 exact product-cancellation cases, 256 bound-neighbor finite-difference cases and 384 exact quotient controls. LP vertex oracles, integer slices and complete binary enumeration, KKT quadratics, rank-deficient QP, MIQP, SOCP support functions, valid infeasibility/recession controls and variable/row/scale transformations are included.
- **216,978 high-level matrix configurations/route checks:** 216,906 computed/feasible entries and72 correctly infeasible entries. They cover linear/CARA information policies, decision trees, weighted mean/variance/CVaR, individual/joint chance constraints, recourse and Benders, robust box/budget/ellipsoid counterparts, multiple objectives, basis sensitivity, addressed SAA holdout counts, constrained local optima, multistart and polynomial-box certificates. The coverage JSON gives every row. Certificate forgeries and precision refusals are separate.
- **16,979 distribution/modifier/custom/risk query tuples:** independent elementary CDF/density reductions, combinatorial PMF/CDF sums, coalesced affine atoms, Gaussian transformations/Jacobians, histogram/cumulative/Metalog reductions and fractional empirical expected-shortfall identities.
- **576 time-series specifications /9,792 step tuples:** independently supplied shocks replay every advertised recurrence; this checks recurrence algebra rather than universal stochastic stationarity.
- **108 sampler run specifications,216 retained/streaming executions,720 recomputed output columns**, plus117 summary/fitting datasets. Exact LHS/Sobol strata, prefix identity, independent two-pass statistics, finite-sample intervals and simple likelihood/MLE identities are checked.
- **1,171 external distribution parameterizations:** SciPy references are independently qualified; elementary inverses/CDFs use90-digit Decimal where near-one reference cancellation matters, symmetric medians are exact, and positive normal/skew-normal quadrature checks small tails. Unresolved reference quantiles and numerical refusals are excluded from successful comparisons.
- **1,523 real CLI cases:** 512 LP models through dense+sparse routes,256 exhaustively enumerated integer models,160 KKT QPs,60 MC/LHS runs across ten families/three seeds (including `u64::MAX`),16 shifted Sobol integrals and seven exact/precision/adaptive controls. Original decisions, bounds and constraints are recomputed independently; objective constants cannot conceal a wrong decision.

Existing positive MISOCP, SOS/indicator/finite-domain, bootstrap fitting, generic modifiers, model/schema, cancellation/deadline/resource and host-revision tests continue to run in the complete workspace. Their existence does not imply every parameter value has been exhaustively exercised. Unsupported GRG, unrestricted nonconvex proofs, incomplete/integer recourse and complete vendor compatibility remain unavailable.

Primary reference parameter conventions: [SciPy distributions](https://docs.scipy.org/doc/scipy/reference/stats.html), [inverse Gaussian](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.invgauss.html), [truncated Normal](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.truncnorm.html) and [skew Normal](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.skewnorm.html). SciPy is test-only. Some raw SciPy extreme PPF/CDF values were demonstrably ill-conditioned; they were qualified using independent mathematics, never accepted merely because they came from another library. Product tolerances were not loosened to erase mismatches.

## Numerical limits and changed support policy

The compiler now refuses inexact coefficient products/divisions instead of emitting a changed algebraic model. Of64 characterized decimal products,43 remain exact accepted computations and21 are explicit `numeric_range` refusals. An ordinary expression such as `0.1*x*0.1` can therefore be refused; `x/3` can also be refused, while exact `(3*x)/3` and dyadic operations remain supported. Ninety-eight additional division rejection controls, including subnormal residuals, are not counted as mathematical solutions. Two-component intermediate expansion capacity remains bounded. Constant transcendental evaluations retain their documented floating semantics; original-expression/Calc checks remain necessary.

Finite differences cannot prove a derivative from finitely many black-box probes. Local stationarity, heuristic best points and globally certified results remain different statuses. SAA and statistical intervals retain their sampling assumptions; LHS/Sobol points are not IID observations. A structured numerical refusal remains a coverage limit, not a solved problem. Highly conditioned cancellation LPs and the known dense robust refusal remain documented.

This alpha has finite domain/resource limits and finite executed platform evidence. Neither these matrices nor bounded tracking of the original96 criteria establish complete Frontline/272-function parity, all-f64 correctness or commercial certification. The numerical identities are dense-simplex0.3, Clarabel0.11.1-adapter0.2 and catalogv3; the addressed RNG remainsv2.

## Reproducing the external matrices

Use a test-only Python environment with NumPy and SciPy. From the Solvercraft directory, build the locked release CLI, then run:

~~~sh
python tests/e2e/numerical_process_oracles.py --executable ../target/release/solvercraft-cli --report /tmp/solvercraft-process.json --lp-cases 512 --integer-cases 256 --qp-cases 160 --simulation-seeds 3 --simulation-trials 16384 --sobol-replicates 16 --qualify-refusals
python tests/e2e/systematic_distribution_oracle.py --scratch /tmp/solvercraft-distribution-probe --report /tmp/solvercraft-distributions.json
~~~

Adapt executable/output paths to the platform; Windows uses `.exe`. The second command builds an isolated JSON-lines mathematical probe from the current source. The recorded external environment was Python 3.12.6, NumPy 2.5.4 and SciPy 1.18.1. These are finite deterministic grids and selected fixed sampling seeds, not exhaustive all-seed stochastic validation. The full workspace tests also include declared Rust matrix seeds and statistical bounds.

## Final executed gates, source identity and deliveries

Canonical code SHA-256: `fdebe74b82d87d10d247e90222d998c3cde87ca19dbac23750ba71d44ca996ba` over 857 Rust/TOML/lock/Python files. The final native gate runner checked this unchanged code before and after every step. This is a normalized code digest, not a claim of a complete reproducible-binary input manifest.

| Final check | Measured outcome |
|---|---|
| Complete SDK workspace | 752 passed,0 failed/ignored; fmt and strict all-target Clippy passed |
| Complete Gridcraft CI | All ten stages; 2915 passed,0 failed,1 existing ignored timing test |
| Qualified external distribution oracle | 1,171 parameterizations;16,112 quantile+16,538 CDF+16,519 log-density comparisons passed;0 failures,0 stronger tail-target misses |
| Noncomputed external operations | 68 explicit numerical-range refusals;1,404 unavailable/ill-conditioned reference operations, excluded from numerical passes |
| Same oracle against immutable e50da565 probe | 328 comparison failures and520 stricter tail-target misses (overlapping categories, not a count of distinct bugs) |
| Real release CLI matrix | 1519 numerical passes,0 failures,1 explicit rejection,3 honest inconclusive cases across1,523 requests |
| SDK in actual Chrome | 20 passed,0 failed/ignored |
| Actual final production app+Worker in Chrome | 9 passed,0 failed,0 ignored; fresh accepted Trunk assets and exact emitted Worker-test WASM |
| Real MCP process/protocol tests | 20 tests repeated successfully against the optimized release build |
| Actual application flow | Preview/apply/recalculate/Undo, stale/cross-document rejection, cancellation/deadlines, Calc equivalence and31 selected PSI imports exercised |
| Native Windows delivery | GUI,CLI,MCP optimized builds passed; x64 PE/version/archive CRC/layout/executable hashes verified |

These matrices are not millions of separate Rust test functions. Counts of routes, queries, laws, recurrence steps and statistical checks remain separate in the [coverage projection](numerical-expansion-coverage-2026-11-02.json). The [verification summary](numerical-expansion-summary-2026-11-02.json) records actual identities/counts; [synthetic witnesses](numerical-expansion-witnesses-2026-11-02.json) retain before/after evidence. The permanent external generators are `tests/e2e/systematic_distribution_oracle.py` and `tests/e2e/numerical_process_oracles.py`.

### Worker timing qualification and all attempts

The first expanded candidate passed eight Worker bodies and failed one additional25ms window-heartbeat guard at31ms while native builds were also running. One unchanged full repeat after builds passed9/9 with17ms maximum heartbeat and1ms maximum status response. The later division interaction was then found and fixed before publication, requiring fresh assets and another full final-source Worker run. Its outcome is the table above. The first failure is retained; no claim is made that all attempts passed. Original R023's250ms normal-model status-access requirement and the stricter25ms diagnostic heartbeat guard are distinct. The observed load/timing sequence is not proof of the cause or a universal performance guarantee.

The first local Trunk launch also rejected an inherited boolean environment value before compilation. The runner configuration was corrected; the accepted final build is source-stable. Native/SDK/transport outcomes, tool setup failures and optional timing observations are not conflated. Production browser execution here is fresh; prior stages and intermediate repaired candidates are retained separately.

The actual final source builds and tests are local executed evidence, not a claim that GitHub-hosted CI, signed installers or untested operating systems were verified. No hosted or persistent solver service was introduced. The existing PR stays a draft; ArtCraft launcher acceptance remains a separate maintainer decision.

### Updated portable artifact

`solvercraft-0.14.2-windows-x64-portable.zip`: 47,203,913 bytes; SHA-256 `8cbb03bd68cce129c2ab7b3333101b1128c55d48f7858ab7de74e0f7c3a945b3`. The CLI-only ZIP was also rebuilt. Binary archives are local user deliverables, not committed source or newly published GitHub release assets. Package licensing still includes306 resolved packages/670 notice files plus fonts and numerical-data notices.

| Program | SHA-256 |
|---|---|
| `solvercraft.exe` | `cfa3e4942eee94eea1b0fad1239b6f60d29e2117d47eebb0427ba083bdbb5d03` |
| `solvercraft-cli.exe` | `34a04ddc7e8b12eb06d2160c6a88d95446fe8fc8c730f14d023bceb078808129` |
| `solvercraft-mcp.exe` | `2a5b3e0a49d61cb203913a30dc450d350504106e294e28df9487bdbdedd0c6bb` |
