# Public reference freeze and acceptance inventory

Frozen 2026-10-31 against **Frontline Solvers 2025 Q1 Reference Guide**, the complete 570-page public PDF. This pins a product/version scope. It is not a statement about every later Frontline release or a declaration that Solvercraft implements the catalog.

Source: [official 2025 Q1 reference](https://www.solver.com/sites/default/files/FrontlineSolversReferenceGuide2025Q1_1.pdf). Independently checked file size: **19,434,011 bytes**. SHA-256: `2a166ac37367f2db870808d590c41ee8310c1676aa3a73164193e999d3d333c6`. PDF page indexes are one based and match the printed page numbers used below. The PDF and copied manual prose are not committed. The factual inventory contains function names, symbolic call signatures, source-page locators and original implementation/test notes.

The machine-readable deliverable is [reference-catalog.json](reference-catalog.json). It includes **272 defined function entries**, **28 alternate parameter formulations**, five additional unresolved named mentions, and twenty-two recorded documentation conflicts. It was assembled from the full PSI chapter, table of contents, alternate-formulation headings, dimensional-modeling chapter and named-property audit. It was not filtered to functions currently implemented.

## Included scope

| Reference family | Entries | Pages |
|---|---:|---|
| Optimization modeling and result access | 22 | 261-267, 421-423 |
| Continuous distributions and alternate formulations | 77 | 270-320 |
| Discrete analytic distributions | 8 | 321-328 |
| Custom distributions | 6 | 329-333 |
| Special/multivariate/resampling/SIP distributions | 10 | 334-338 |
| Simulation time-series generators | 10 | 340-344 |
| Distribution, correlation and time-series properties | 31 | 345-362 |
| Sample statistics | 47 | 363-382 |
| Theoretical distribution statistics and aliases | 19 | 379-381 |
| Six Sigma functions/properties | 21 | 382-388 |
| Multiple-simulation controls | 3 | 389 |
| Dimensional-modeling functions/properties | 12 | 410-438 |
| RASON conversion references | 2 | 439-440 |
| Additional decision-table functions | 2 | 441-442 |
| Custom modeling functions | 2 | 443-446 |

`PsiCalcValue` is counted once despite definitions in both optimization and decision-table sections; `PsiOptValue` is likewise one function with scalar and dimensional signatures. Aliases with distinct public names are separate entries. `PsiDimActive`, `PsiDimLock` and `PsiTableCube` are retained even though the table of contents does not list each one separately. Simulation ARMA/ARCH functions generate a time vector on each Monte Carlo trial; they are included as simulation features, rather than excluded with forecasting.

The original MASTERPLAN excludes the separate Data Science / classification / prediction / forecasting program. The corresponding pages 390-399 and names (`PsiForecast`, `PsiPosteriors`/`PsiPosterior`, `PsiPredict`, `PsiTransform`, `PsiForecastETS`, `PsiForecastLinear`) are explicitly recorded as excluded. This exception does not remove the simulation path generators. VBA/object-model binary compatibility is not claimed by the PSI function inventory. The neighboring matrix helper chapter, pages 401-408, and VBA APIs are separate interface inventories, not hidden implementations of these PSI signatures.

## Product features behind the function inventory

The reference also describes model/UI operations, sampling and convergence controls, correlation/copulas, fitting, transformation, engine selection/options, result statuses, reports, stored models and programming interfaces. Source anchors are: UI/modeling 18-160; result semantics 162-186; platform settings 190-216; engine settings 218-253; saved models 253-259; PSI functions 260-399; dimensional modeling 410-446; reports 447-469; object model 471-510; traditional VBA 511 onward. Function coverage alone does not demonstrate that all these workflows or proprietary engine options are implemented.

## Signature and semantic rules

The catalog preserves required-argument order and source optional notation, normalizing Greek symbols and typography to ASCII parameter names. A source ellipsis denotes additional PSI properties; it does not authorize interpreting any extra positional number as an SDK parameter. Multiple properties can follow required distribution arguments (page 268), so a three-argument import test cannot establish property composition coverage.

Twenty-eight alternate formulations are explicitly cataloged. Most are described through allowed selector/parameter counts rather than a literal complete call signature. They carry `reference_describes_selector_arity_only`, remain open and require authoritative per-selector and inverse-parameter tests before compatibility is declared. Concrete example calls are not presented as exhaustive overload definitions. No function has been removed because its exact semantics remain unresolved.

Important parameter distinctions have been sent to both host and math implementers:

- `PsiLogNormal(mean,stddev)` uses arithmetic moments. `PsiLogNorm2(log_mean,log_stddev)` uses moments of the underlying normal. A direct alias between the former and a log-parameter DTO would change the distribution.
- `PsiExponential(beta)` uses mean/scale beta; the SDK rate representation requires `rate=1/beta`.
- `PsiGamma(alpha,beta)` and `PsiWeibull(alpha,beta)` use shape/scale.
- `PsiGeometric(p)` and `PsiNegBinomial(s,p)` count failures and start at zero.
- `PsiIntUniform(a,b)` is an inclusive integer interval. `PsiDisUniform(values)` assigns equal probability to a supplied list. These are different distributions.
- `PsiCumul(a,b,xs,ps)` interpolates a specified cumulative probability function including its bounds. It is not a direct synonym for equal-weight interpolation of arbitrary observations.
- `PsiPert(a,mode,b)` has the canonical mean `(a+4*mode+b)/6`. Its fourth positional argument is not documented as a configurable PERT weight.
- The outside-simulation value depends on display/base-case/lock/static settings and context. A deterministic SC nominal value is an explicit Solvercraft policy, not an assumed match for every PSI default.

## Source conflicts and unresolved names

The machine-readable errata list retains source locations and affected functions. Examples include swapped Myerson parameter roles in prose versus formulas, `PsiPerson6Alt` versus `PsiPearson6Alt`, `PsiCompund` versus `PsiCompound`, the `PsiCumulD` declaration printed as `PsiCumul`, incompatible dimensional-statistic exclusion lists, unmatched dimension-property parentheses, and endpoint-atom ambiguity in custom cumulative distributions. Beta's printed support and the PERT/Weibull alternative descriptions also require independent mathematical checks. The time-series AR(2) printed variance omits covariance terms, and the APARCH printed left side conflicts with the published power-scale recurrence. MomentFit uses Pearson kurtosis while some other reference tables report excess kurtosis; those conventions remain source-specific. These errors are reasons to preserve uncertainty, not to reproduce an incorrect formula.

`PsiCorrelate`, `PsiLen`, `PsiTruncateD`, `PsiXtoP` and `PsiXtoQ` appear in the frozen chapters without complete corresponding signature definitions. They have a separate unresolved-mentions inventory rather than invented support. Placeholder names and obvious prose typos are not silently promoted to executable aliases.

## Implementation/test mapping and open acceptance

Each function has a unique planned signature/default test ID, source pages, signature evidence quality, implementation mapping, native/browser evidence and guarantee statement. Related SDK math is recorded separately from PSI worksheet import. A numerically similar family does not prove argument conversion, optional defaults, property order, result selection, or outside-simulation parity. Existing broad math tests are related evidence only; they are not relabeled as 272 individual PSI acceptance tests.

At this freeze, **all 272 full-reference acceptance records remain open**. Executed host tests cover nineteen strict core PSI signatures and ten domain cases through SC.PSI shims, including arithmetic/log parameter conversions and the canonical PERT weight. The catalog records this subset separately from full-reference coverage: properties are unsupported, ambiguous custom-CDF endpoint atoms are rejected, and the deterministic outside value remains an explicit difference. Seven executed SDK constructor tests add independently checked parameter conversions and custom distribution subsets. Eleven Metalog/moment-fit tests cover a globally certified coefficient subset, CDF-OLS, SPT and normal/shifted-lognormal moment subfamilies. Five further Johnson tests exercise bounded genuine SN/SL/SU/SB population inversion against nine independent numeric fixtures and actual returned-moment postchecks. Ten time-series tests cover all nine Gaussian recurrence families, explicit startup/postprocessing, addressed prefixes and SIP replay. These are executable SDK subsets with documented numerical limits; unknown PSI properties/defaults and remaining Metalog/Johnson domains stay explicit. See [Johnson moment inversion](johnson-moments.md) and [time-series simulation](time-series.md). Forty typed reference-parameter tests now cover all28 Alt mathematical cores, native order/location conversions and corrected NormalSkew geometry; the Pearson6/Weibull vendor arities remain explicitly unresolved. Peer multivariate/resampling tests add covariance-vector and sequential SIP/SLURP cores. The executable272-row [implementation matrix](reference-implementation.json) separates111 R046 distribution/time-series functions from R081 host compatibility and retains all source hashes, test commands, platform scope and gaps. [Source-backed questions](reference-open-questions.md) preserve unresolved vendor behavior. Further imports and math additions must update individual entries only after their actual tests run. The inventory deliberately does not mark R046 or complete R081/R091 fulfilled merely because the source is now available.

`crates/compile/tests/reference_catalog.rs` checks source identity, all 272 unique entries/test IDs, page ranges, retained unsupported families, visible conflicts and open acceptance semantics. These are inventory-integrity tests, not distribution or import parity tests. Actual functional acceptance must exercise each claimed signature and default, an independent numerical oracle where appropriate, and the platform being claimed. A cross-compilation check is not a browser execution test.

The catalog is pinned to 2025 Q1. Later public documentation, including an identified 2025 Q2 reference, must be a separately versioned delta, not silently merged into this freeze. Source updates and implementation evidence updates must preserve the earlier version and explain additions, corrections and changed guarantees.
