# Numerical review, 2026-11-02

This review found silent wrong results in the published `dec991232f686930b52329d6caf8729b0c5e31fd` candidate. Earlier green integration and final7 reports did not establish numerical correctness. The repaired candidate adds independent mathematical witnesses and process/browser/workbook regression tests. Historical reports remain unchanged; this report supersedes their numerical confidence for the affected cases.

## Reproduced defects and repairs

| Area | Independent witness / previous behavior | Repair and qualification |
|---|---|---|
| Dense LP/MILP optimality | On `0<=x<=y<=z<=1`, maximize `1e16*x+y-1e16*z`. Exact optimum 1 at `(1,1,1)`; old CLI certified 0 with zero gap. | Reconstruct reduced costs from canonical original costs at every phase-II decision using exact product accumulation. The repaired engine must actually solve the unit-improvement variants. |
| Infeasibility | Bounds `[0,1e16]`, chain `x<=y<=z`, row `1e16*x+y-1e16*z>=1e16`. Exact feasible witness `(1e16,1e16,1e16)`; old CLI said infeasible. | A positive phase-I residual now requires a separately checked original-row/variable-box separation certificate. The ill-conditioned witness may be refused numerically; refusal is not counted as a computed solution. |
| Unboundedness | Nonnegative variables, cycle `x<=y<=z<=x`, row `1e16*x+y-1e16*z<=1`, maximize x. Cycle reduces the row to `x<=1`; old CLI said unbounded. | Validate the recession direction against exact original-row derivatives, original bound signs and a strictly improving original objective. An unchecked ray gives numerical failure. Ordinary valid unbounded controls remain tested. |
| Goal priorities | Maximize `1e16+x`, then minimize x with zero first-goal tolerance: old result could undo the first optimum. | Construct lexicographic preservation constraints from the variable part, excluding the constant offset. Test min/max directions, offsets and absolute tolerances. |
| Information value | One state, payoff `1e16`, uninformative signal costing 1: old net information value 0 instead of -1. | Linear-utility policy comparisons and information gains use anchored payoff differences; acquisition cost is subtracted once from the gain. Certainty-equivalent levels remain rounded binary64 values. |
| Simulation and SAA uncertainty | Nonconstant values near `1e-200` produced zero variance/SE; adjacent values near `1e10` produced positive SE but point confidence intervals. | Reject unsupported positive variance instead of inventing zero; scale variance computations, take square roots before sample-size division, and round positive-width interval endpoints outward. Constant holdouts retain point intervals. SAA retains its explicit rejection of subnormal variance. |
| Distribution tails/endpoints | Uniform(-1e100,1) upper quantile became 0; inverse-Gamma lower CDF/quantile tails disappeared; Cauchy/Pareto small CDFs became 0; log-logistic extreme log ratios became 0/1. | Exact bounded endpoints and stable direct/complementary-tail/log-ratio formulas. An independent broad SciPy run also caught an upper inverse-Gamma conditioning regression during the repair; the corrected branch passes the original tolerance. |
| Discrete mixture | A mixture of point masses 0 and 1 could return `1.000000000000254` as its quantile. | Search ordered binary64 values to the exact CDF jump. LHS samples must be exactly the declared atoms. |
| Summary/fitting extremes | Subnormal constant CVaR became 0; a tiny representable residual between cancelling large values disappeared in the fitted mean. | Exact binary upscaling for tiny CVaR and an independent compensated original-unit sum; refuse numerical ranges that cannot retain a required result. |

## Independent checks

The permanent `tests/e2e/numerical_process_oracles.py` script runs the real JSON CLI, records executable SHA-256 and seed `2026110207`, and compares:

- 72 bounded LP models through both dense and sparse entry points against SciPy/HiGHS, with independent original-domain residual checks.
- 48 integer models against exhaustive finite enumeration, excluding objective constants from comparisons so a large offset cannot conceal a wrong decision.
- 36 coupled convex QPs against constructed KKT solutions.
- Exact cancellation, false-infeasibility, false-unboundedness, tiny-variance and discrete-mixture witnesses.
- 20 MC/LHS cases across ten continuous/discrete families, 32,768 samples each, independently recomputing retained means, sample variances, quantiles and fractional empirical loss-tail CVaR, with SciPy CDF anchors at predeclared tolerances.
- Eight independently shifted Sobol product-integral cases and a bounded adaptive-MC interval whose simultaneous-checkpoint width is independently checked.

The separate distribution oracle checks 150 parameterizations: 1,349 quantiles, 1,361 CDF values and 1,361 log densities. One unavailable/extreme oracle quantile is explicitly excluded. The predeclared tolerances remain unchanged, including relative quantile `2e-6`, ordinary absolute CDF `2e-8`, disclosed legacy Normal/Lognormal absolute CDF `5e-7`, and log-density `2e-6 + 1e-10*abs(reference)`. Small analytic tail regressions use relative comparisons because absolute CDF tolerances alone can conceal a lost tail. Reference tools are test-only SciPy 1.18.1 and NumPy 2.5.4, with no product runtime dependency.

Rust oracle matrices cover dense/integer sign/row-order variants, valid infeasible/unbounded controls, coupled QP, MIQP and SOCP closed-form solutions, box/budget/ellipsoid robust geometry, chance probabilities, recourse/Benders breakpoints and local/global status distinctions. Sampling invariants cover 64-dimensional Sobol strata/prefix replay, streaming/retained moments and singular negative Gaussian dependence, in addition to the existing copula checks.

## Remaining numerical limits

No finite test suite proves correctness for all floating-point inputs. The very ill-conditioned feasible/bounded witnesses can still return a structured numerical refusal. One ordinary gamma-zero budget-robust case also remains an honest dense-engine numerical refusal; the explicitly chosen sparse route matches its independent geometric oracle. These are availability limitations, not successful solutions. Sparse results labelled `best_found` remain qualified, even when their point matches an independent optimum.

Local nonlinear stationarity and heuristic best points remain distinct from global certificates. MC finite-sample anchor checks are sanity checks, not proof of an entire CDF; LHS/Sobol points are not assumed IID. Adaptive intervals require true caller-supplied output bounds. Disclosed approximation assumptions remain part of interval contracts. Numerical support limits, 31 PSI import forms and incomplete legacy PSI/272-function compatibility remain explicit.

The dense engine identity changes to `solvercraft-dense-simplex-0.2`. The mathematical catalog changes to `solvercraft-baseline17-r4-catalog-v2`, appended to result sampling identities. The addressed RNG remains `solvercraft-addressed-mix64-v2`; this does not promise identical summaries across numerical revisions.

## Final executed gates and artifacts

All final native gates checked the same frozen code before and after each step.
Canonical code digest: `7353b7c2ecb447f8901b7130ad751d5bec9e778c65c2d5d08aaaf9aa142730bf` (838 Rust/TOML/lock/Python files, CRLF normalized to LF; not a complete binary-input manifest).

| Gate | Executed result |
|---|---|
| Complete independent SDK workspace | 686 passed, 0 failed/ignored; fmt and all-target Clippy passed |
| Complete Gridcraft CI | All ten stages; 2913 passed, 0 failed, 1 existing ignored timing test |
| Actual headless Chrome SDK | 16 passed, 0 failed/ignored; test WASM SHA `5ae86dc8dfa238561fb10b191c8ca6d42543b29e6367619dcc900d51e3cc5625` |
| Real CLI numerical process oracle | 259 numerical passes, 0 failures, 1 explicit rejection, 3 honest inconclusive cases |
| Immutable old CLI, same process oracle | 257 passes, 5 reproduced wrong-output failures, 1 honest inconclusive case |
| Distribution oracle | 150 parameterizations; 1,349 quantiles + 1,361 CDF + 1,361 log-density checks; 0 failures |
| Real MCP process subset, included in SDK total | 9 process tests, plus 8 protocol/resource tests; all 17 repeated against the optimized release build |
| Real Workbook/Calc commands, included in host total | Corrected chain preview/apply/one-step Undo; tiny-variance rejection preserves the original document |
| Release builds | GUI, CLI and MCP built successfully with locked dependencies |
| Portable archive | 702 entries; CRC/layout/all three binary hashes verified; 306 resolved dependency packages and 670 notice files |

The three inconclusive process cases are sparse exact cancellation, dense ill-conditioned phase I and dense bounded-recession cancellation. The explicit rejection is nonrepresentable positive simulation variance. They are deliberately not counted as numerical solutions. Ordinary sparse `best_found` points retain that label despite matching the independent objective within tolerance.

The [machine-readable summary](numerical-review-summary-2026-11-02.json), [before cases](numerical-process-before-2026-11-02.json) and [after cases](numerical-process-after-2026-11-02.json) expose fixed public fields and synthetic witnesses only. Full local diagnostics and intermediate failing repair attempts remain separate. Scoped owner test counts are included in the complete SDK total and are not added twice. Native tests required permission for ephemeral local test sockets; no persistent or hosted service was introduced.

The actual Chrome run executes the SDK math, including three new regressions. It does not establish a new production Worker run or native GUI click-through on this revision. The test bodies passed; the runner logged a window-close timeout warning before bounded ChromeDriver termination. The prior UI integration evidence remains historical.

A measured process benchmark (three repeats, one development machine under concurrent build load) changed the 100-variable/500-row dense LP median from239ms to574ms. The streaming 100,000-trial/20-input/10-output MC medians were501ms and432ms. These are workload observations with independent objective/moment checks, not isolated performance guarantees or a commercial comparison. The stronger dense cost reconstruction adds work; no performance speedup is claimed.

Updated local portable artifact: `solvercraft-0.14.2-windows-x64-portable.zip`, 47,133,564 bytes, SHA-256 `68c89513d38b0b947b0d6c136bc84729f9761e2ad5657ee62474364f8e45796b`. The older CLI-only download was also rebuilt, rather than left with the known buggy core. Application/SDK package versions remain alpha versions; distinguish numerical revisions by engine/catalog, source and executable hashes.

| Program | SHA-256 |
|---|---|
| `solvercraft.exe` | `b7561030d55291c2d0d091e6f20d8f42678f7742e20369c1c66f64e396c3346f` |
| `solvercraft-cli.exe` | `ef1034b5eff99a3545ad952ab9f911e5bc5a6876f1b231b4218221f64909f446` |
| `solvercraft-mcp.exe` | `aa6e3c6c222b0621977a2f88245a0d96468fc490f56f3f8d45cae6ae7b9323d6` |

Binary archives are local deliverables, not committed source or published GitHub release assets. The existing PR remains a draft; official ArtCraft catalog acceptance remains a separate maintainer decision.
