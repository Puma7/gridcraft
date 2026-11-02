# Public reference freeze and acceptance inventory

Frozen 2026-10-31 against **Frontline Solvers 2025 Q1 Reference Guide**, the complete 570-page public PDF. This pins a product/version scope. It is not a statement about every later Frontline release or a declaration that Solvercraft implements the catalog.

Source: [official 2025 Q1 reference](https://www.solver.com/sites/default/files/FrontlineSolversReferenceGuide2025Q1_1.pdf). Independently checked file size: **19,434,011 bytes**. SHA-256: `2a166ac37367f2db870808d590c41ee8310c1676aa3a73164193e999d3d333c6`. PDF page indexes are one based and match the printed page numbers used below. The PDF and copied manual prose are not committed. The factual inventory contains function names, symbolic call signatures, source-page locators and original implementation/test notes.

The machine-readable deliverable is [reference-catalog.json](reference-catalog.json). It includes **272 defined function entries**, **28 alternate parameter formulations**, five additional unresolved named mentions, and twenty-two recorded documentation conflicts. It was assembled from the full PSI chapter, table of contents, alternate-formulation headings, dimensional-modeling chapter and named-property audit. It was not filtered to functions currently implemented.

## Included scope

| Reference family | Entries | Pages |
|---|---:|---|
| Optimization modeling and result access | 22 | 261-267,421-423 |
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

The 272 full-vendor acceptance records retain their exact signature/property/
default limitations rather than being promoted to complete PSI parity. This is
separate from the original criterion assessment. The final6 [scope audit](../qa/original-scope-audit.md)
quotes the unchanged CSV, SC-14/SC-23 tasks and masterplan sections 10.1/12:
R046 asks for individually tested mathematical catalog functions; R081 asks
for separate verification of each supported signature/default, and SC-23
explicitly selects signatures. Neither adds a mandatory 272-name import suite.

All 111 frozen mathematical capabilities have individual positive associations
to executed tests in final6 SDK 630, across 73 selected test bodies/13 source files.
The two mathematical Pearson 6/Weibull Alt constructions are positively tested
with explicit identifying claims. Their literal vendor arities/defaults remain
open [source questions](reference-open-questions.md), not invented compatible
worksheet calls. Numerical support limits remain in [reference-gaps.md](reference-gaps.md).

The historical final6 converter had nineteen top-level required-argument PSI forms with
explicitly lossy `solvercraft_nominal` outside values and refusal under
`preserve_vendor`. Unsupported nesting/properties/extra arguments are preserved
whole. Final6 evidence contains 25 new native engine contract tests, 21 Calc tests
and one actual Chrome worker body over nineteen signatures, 2048 trials and
nineteen outputs. That body passed in both attempts. The initial overall worker
attempt was 7/1 at an extra 26 ms-versus-below 25 ms heartbeat assertion; one unchanged
repeat was 8/0. Both remain in the [composite report](browser-worker-final6-report.json),
with aggregate all-attempts-passed=false and an unproven first-failure cause.

Thus R046 and the selected R081 contract have bounded acceptance under the
original criteria; full reference/default/property compatibility remains
explicitly incomplete. The 111 mathematical entries, nineteen imported forms
and 272 inventory records describe different scopes. SDK math or inventory
checks are never relabelled as literal PSI or universal browser execution.

### Final7 implementation overlay: 31 qualified core forms

The external `reference-catalog.json` source inventory stays frozen. The
maintained `reference-implementation.json` overlay now records 31 active core
imports and 80 distribution-name worksheet gaps, from the same 111 mathematical
and 272 full-reference entries. This is not a change to the original 96 criteria
or a claim of full vendor compatibility. Actual final7 gate evidence is linked below.

The additional twelve are ChiSquare, Erlang, FDist, Logistic, MaxExtreme,
MinExtreme, Rayleigh, Reciprocal, Student, Levy, Kumaraswamy and HypSecant.
All are scalar required-argument cores. Cauchy/Laplace/HyperGeo remain excluded
for the explicit source/mapping discrepancies documented in
[reference-gaps.md](reference-gaps.md). No Alt, property or vendor display-default
compatibility is silently added. The [31-form scope](reference-gaps.md)
records PDF pages, domains, numeric limits and the shared registry.

The canonical immutable `gridcraft_calc::PSI_CORE_SIGNATURES` registry supplies
`legacy_name`, `worksheet_name` and `required_arguments` for all 31 forms. Calc
decoding, known-function recognition, host import and reported capabilities use
this shared contract rather than separate name/arity maps. Its counts are vendor
required core arguments; the optional numeric SC nominal override is not a
vendor property or an extra imported PSI argument.

Discovery now returns `nominal` and `nominal_basis` per input, with explicit
`analytic_mean`, `median_fallback` or `explicit_override` values. Simulation
requests and manifests retain `worksheet_nominal: {value, basis}` separately
from the effective sampling `distribution`. A caller can pass public
Discovery.inputs/outputs into simulation without removing these metadata fields.
An explicit simulation distribution does not silently relabel the worksheet's
outside value: a worksheet override 8 and sampled Constant 7 remain distinct.

Final7 now has executed source-bound qualification for the enlarged 31-form
contract. All ten host CI stages passed with 2904 tests passed, zero failed and
one pre-existing ignored; SDK 630 passed with zero failures/ignored. Native UI 6,
chart module 8 numerical+1 lifecycle and actual Chrome SDK 13 also passed. These
scoped counts are not added together to manufacture a total.

The actual product worker passed all 9 bodies on its first attempt. Both import
bodies passed: the existing 19-form body ran 2048 trials; the new 12-form body ran
4096 trials with seed 42. Its 24 empirical CDF anchors had maximum absolute error
0.020751953125 against the declared prefix tolerance 0.054792830314635735.
This is finite prefix-test evidence, not a universal distribution accuracy or
confidence guarantee. Observed admission was 0.765 ms, status 1 ms, heartbeat 12 ms,
active progress gap 58 ms and 29 progress updates; no hard realtime or universal
25 ms promise follows from these observations.

The [final7 summary](final7-verification-summary.json) and [actual worker report](browser-worker-final7-report.json) identify this run. Raw code/input SHA-256: `8d74eebf83ef7986d0dd5cb302ce159413b49d7e51c5327947a2d8444e505065`; LF-canonical SHA-256: `dc58726c5be86ddda8e5c6bfffdfe25507a29b66bedd51e50f376bcfbc3ad472`. Historical final6 evidence remains separately labelled below.

Current native PSI evidence comprises 31 separately named positive forms and 9 negative/policy groups (40 engine tests), plus 37 Calc tests. These selectors are present in the [final7 native test excerpt](gates-final7/host/host-ci.txt). The [CDF-prefix witness](psi-core12-cdf-final7-report.json) records the 24 comparisons and their outside-value basis. These public files are compact projections/excerpts, not complete private diagnostic dumps.


`crates/compile/tests/reference_catalog.rs` checks source identity, all 272 unique entries/test IDs, page ranges, retained unsupported families, visible conflicts and open acceptance semantics. These are inventory-integrity tests, not distribution or import parity tests. Actual functional acceptance must exercise each claimed signature and default, an independent numerical oracle where appropriate, and the platform being claimed. A cross-compilation check is not a browser execution test.

The catalog is pinned to 2025 Q1. Later public documentation, including an identified 2025 Q2 reference, must be a separately versioned delta, not silently merged into this freeze. Source updates and implementation evidence updates must preserve the earlier version and explain additions, corrections and changed guarantees.
