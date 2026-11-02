# Exact remaining reference-function work

The [expanded numerical qualification](../qa/numerical-expansion-2026-11-02.md)
is the latest evidence for calculation accuracy and precision limits. Its
finite matrices and explicit refusals do not establish full parameter coverage.

The [2026-11-02 numerical review](../qa/numerical-review-2026-11-02.md) records
subsequent wrong-result repairs, fresh independent checks and remaining numeric
refusals. Historical positive function-name coverage below is not proof that
every parameter combination was numerically correct.

This report preserves the original CSV criteria. R046 covers the 111 frozen
distribution/custom/special/time-series mathematical capabilities; R081 checks
each actually supported worksheet signature/default policy. R091 records all
272 reference functions with tests, platforms, guarantees and gaps. It does not
turn the entire inventory into a mandatory 272-name import contract.

The historical final6 [scope audit](../qa/original-scope-audit.md) and [current acceptance
review](../qa/acceptance-current.md) accept R046 and R081 within their original
bounded contracts. The broader compatibility limitations below remain real;
criterion acceptance is not full Frontline or 272-PSI parity.

## Executable R046 cores

| Current evidence | Function count |
|---|---:|
| Ordinary/custom/special/time-series cores with executed tests | 82 |
| Typed parameter cores with executed tests (26 Alt plus NormalSkew) | 27 |
| Own determined mathematical Alt variants with unresolved legacy arity | 2 |
| Unambiguous function names without an executable SDK core | 0 |
| Frozen distribution/time-series inventory | 111 |

All 111 mathematical capability names have individual positive test associations
in the actual final6 SDK run: 630 passed, zero failed/ignored, with 73 selected
test bodies across 13 source files. Exact log lines and source hashes are in the
[scope audit](../qa/original-scope-audit.md); no rejecting guard is counted as a
positive implementation.

The two unresolved legacy functions are **PsiPearson6Alt** and
**PsiWeibullAlt**. The exact source contradictions/questions and tested own
3-/2-claim policies are in [reference-open-questions.md](reference-open-questions.md).
The existing ordinary Pearson 6/Weibull distributions are implemented and tested;
these gaps concern their undocumented alternative calling contract. No arbitrary
parameter is silently fixed to manufacture a compatible call.

The newly added special APIs are executable: covariance normal/lognormal
vectors, row/column bootstrap, full-matrix shuffle, sequential SIP/SLURP,
whole-event compounding, a caller-published analytic/SIP library, exact declared
name/formula metadata, and automatic fitting with changed-snapshot caching.
Their positive tests exercise real computations; rejecting unsupported calls is
not counted as their implementation. Individual paths, exact Cargo selectors,
source hashes, owner execution evidence and limits appear in
[reference-implementation.json](reference-implementation.json).

This does not mean every mathematical parameter value, every vendor engine
choice, or every worksheet property is supported. Concrete remaining differences
are below; all 272 full PSI acceptance records remain open rather than being
converted into a claim of vendor parity.

## Explicit core/domain limitations

- Metalog: sufficient global derivative certificate only. Valid coefficients
  outside that sufficient class and upper-only bounded transforms are rejected.
  The published reference's full coefficient validity domain is not reproduced.
- Alt numerical fits: supported positive shape[1e-4, 1e4], bounded normalized
  location/width/interior-mode ranges and configured work limits. Convergence
  failures and non-identifiability are explicit. A matching candidate is not a
  proof of unique vendor branch choice. Each advertised constructor's tests and
  exact policy are in [reference-functions.md](reference-functions.md).
- Custom Cumul/CumulD/General/Histogram constructors: at most 32 nonzero mixture
  intervals; ambiguous positive endpoint atoms are not inferred from the source.
  Subjective Beta with identical symmetric mean/mode does not identify a unique
  concentration and is rejected rather than assigned an invented default.
- Scalar family limits remain inspectable in `math::catalog`: e.g. Poisson
  rate<=512, Logarithmic p<=0.99, capped integer counts and special-function shape
  ranges. These are stated numerical capacities, not full-reference domain parity.
- PsiFit: `solvercraft-continuous-six-bic-v1` chooses among Normal, Uniform,
  Exponential, Lognormal, Laplace and Cauchy, exposes ranking/failures, and
  invalidates an exact-data-bit cache. The proprietary vendor candidate/default
  policy and live worksheet watching are not claimed. Explicit-family fitting
  and the broader SDK ranker are separate APIs.
- PsiCertified: callers can publish and resolve their own versioned analytic,
  SIP and SLURP records. Frontline's proprietary published service/catalog is
  not accessed or represented as Solvercraft certification.
- PsiDistInfo: declared original names/formulas are preserved without guessing
  them from transformed DTOs. Live worksheet-cell discovery/capture is separate.
- PsiMakeInput: the whole event expression runs afresh with stable repeated
  reads within that event, bounded resources and explicit frequency kinds.
  Source-ambiguous deductible/limit ordering is an explicit selectable policy.
- MVResample/MVShuffle: row/column orientation is explicit and full source matrix
  shuffling is tested. Ambiguous oversized worksheet output mapping is omitted.
- Time-series: explicit startup states and fixed SDK postprocessing order;
  source AR2/APARCH formula discrepancies, omitted startup/property defaults and
  vendor RNG sequence remain distinct compatibility questions.

## Final7: 31-form converter with source-bound qualification

The active branch extends the historical nineteen-form final6 converter to
**31 selected top-level required-argument forms**. This is a separate extension;
the original 96 requirement rows, external source freeze and 111 mathematical
catalog entries are unchanged. Actual final7 native, SDK and browser gates now
qualify this extension. The final6 evidence below continues to describe its own
nineteen-form source snapshot.

The existing nineteen forms remain Uniform, IntUniform, Normal, LogNormal,
LogNorm2, Triangular, Pert, Beta, Gamma, Exponential, Weibull, Bernoulli,
Binomial, Poisson, NegBinomial, Geometric, DisUniform, Discrete and Cumul.
All names have the `Psi` prefix. The twelve additions are:

| Added core | Required arguments, in order | Domain / mapping distinction | Frozen PDF page |
|---|---|---|---|
| PsiChiSquare | `df` (1) | Positive integer df; `ChiSquared` | 275 |
| PsiErlang | `k, beta` (2) | Positive integer k; beta is positive scale, not rate; `Erlang` | 279 |
| PsiFDist | `df1, df2` (2) | Positive integer numerator/denominator df; `F` | 282 |
| PsiLogistic | `mu, s` (2) | Finite location; positive scale; `Logistic` | 291 |
| PsiMaxExtreme | `m, s` (2) | Finite location/mode; positive scale; `Gumbel` | 296 |
| PsiMinExtreme | `m, s` (2) | Finite location/mode; positive scale; `MinExtreme` | 300 |
| PsiRayleigh | `beta` (1) | Positive Rayleigh sigma, not Weibull scale; `Rayleigh` | 312-313 |
| PsiReciprocal | `min, max` (2) | `0 < min < max`; `Reciprocal` | 313-314 |
| PsiStudent | `df` (1) | Positive integer df; `StudentT` with location 0 and scale 1 | 314-315 |
| PsiLevy | `loc, c` (2) | Finite real location; positive scale; `Levy` | 290-291 |
| PsiKumaraswamy | `shape1, shape2, min, max` (4) | Positive shapes; finite min<max; affine bounded constructor | 289 |
| PsiHypSecant | `loc, beta` (2) | beta is native standard deviation; SDK scale=`beta * FRAC_2_PI` | 285-286 |

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

Student df=1 is a labelled median fallback, while df=2 has analytic mean 0 even
though its variance is undefined. FDist denominator df<=2 uses median fallback;
a larger denominator uses its finite mean. Levy always uses a labelled median
fallback. All are explicit SC outside-value policies, not vendor display-state
compatibility. Basis labels must be retained even when a mean and median happen
to have the same numerical value.

HypSecant requires a genuine parameter conversion: the frozen guide's native
beta is the standard deviation, while the SDK HyperbolicSecant has variance
`pi^2 * scale^2 / 4`. Use `beta * FRAC_2_PI`, not `(2 * beta) / pi`, to avoid an
unnecessary intermediate overflow. Kumaraswamy requires all four arguments:
`X = min + (max-min)*Y`, with Y on [0,1]. This is affine in the sampled Y, not a
claim of affine dependence on its shape parameters. Shared SDK validation still
bounds df/k at 10000, Kumaraswamy shapes to [1e-4,1e4] and requires finite positive
width. These are numerical capacities, not additional vendor domain claims.

Cauchy, Laplace and HyperGeo remain explicitly unsupported imports. For
Cauchy/Laplace the header's location argument conflicts with scale/positive-loc
prose and unshifted formulas; HyperGeo calls D failures in prose but treats D as
counted successes in its support/PMF and binomial approximation. Current official
help repeats those discrepancies. Existing mathematical SDK families remain
available, but this expansion does not invent their literal PSI mapping.

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


Historical final6 evidence included 25 new engine tests, 21 Calc tests and one
actual Chrome body over 19 signatures/2048 trials/19 outputs, passed in both
attempts. Its [worker composite](browser-worker-final6-report.json) retains the
initial additional timing-guard failure and one unchanged successful repeat;
none of those outcomes is reassigned to the new 31-form branch.

The default `outside_policy=solvercraft_nominal` remains explicitly lossy and
reports `vendor_display_context_preserved=false`. `preserve_vendor` refuses
conversion when that context is unavailable. Unsupported nesting, properties,
Alt/extra calls remain preserved whole; the original nineteen cases/policies
stay regressions. The optional SC numeric override is not imported as a vendor
property. No new vendor display lifecycle behavior is claimed.

## Remaining worksheet compatibility backlog

The maintained [implementation matrix](reference-implementation.json) now lists
**80 R046 names without a supported PSI worksheet importer** (111 mathematical
names minus 31 active core imports). They retain executable SDK cores or explicit
mathematical alternatives; this is future host compatibility work, not missing
scalar mathematics or an extra mandatory 272-name R081 requirement:

- continuous_distribution: PsiBetaGen, PsiBetaGenAlt, PsiBetaSubj, PsiBurr12, PsiCauchy, PsiCauchyAlt, PsiChiSquareAlt, PsiDagum, PsiDblTriang, PsiErf, PsiErfAlt, PsiExponentialAlt, PsiFatigueLife, PsiFatigueLifeAlt, PsiFrechet, PsiFrechetAlt, PsiGammaAlt, PsiHypSecantAlt, PsiInvNormal, PsiInvNormalAlt, PsiJohnsonSB, PsiJohnsonSU, PsiLaplace, PsiLaplaceAlt, PsiLevyAlt, PsiLogisticAlt, PsiLogLogistic, PsiLogLogisticAlt, PsiLogNormalAlt, PsiMakeInput, PsiMaxExtremeAlt, PsiMetalog, PsiMetalogFit, PsiMetalogSPT, PsiMinExtremeAlt, PsiMomentFit, PsiMyerson, PsiNormalAlt, PsiNormalSkew, PsiPareto, PsiParetoAlt, PsiPareto2, PsiPareto2Alt, PsiPearson5, PsiPearson5Alt, PsiPearson6, PsiPearson6Alt, PsiPertAlt, PsiRayleighAlt, PsiStudentAlt, PsiTriangGen, PsiTriangularAlt, PsiUniformAlt, PsiVary, PsiWeibullAlt.
- discrete_distribution: PsiHyperGeo, PsiLogarithmic.
- custom_distribution: PsiCumulD, PsiGeneral, PsiHistogram.
- special_distribution: PsiCertified, PsiDistInfo, PsiFit, PsiMVLogNormal, PsiMVNormal, PsiMVResample, PsiResample, PsiMVShuffle, PsiSip, PsiSlurp.
- simulation_time_series: PsiAR1, PsiAR2, PsiMA1, PsiMA2, PsiARCH1, PsiARMA11, PsiEGARCH11, PsiGARCH11, PsiAPARCH11, PsiTSSip.

The other 161 frozen reference entries (optimization/property/statistics/control/
dimensional/etc.) remain in the 272-row full-reference/R091 matrix. A generic
SDK capability is not execution of a literal worksheet signature or its defaults.

## Platforms and reproducible audit

Recorded numerical tests are Windows/MSVC Rust 1.95; peer special-function
WASM32 checks are compile evidence only. No blanket per-function browser,
Linux/macOS runtime or vendor binary comparison is inferred. The project's
separate actual worker/browser controls retain their own narrower scope.

Run `python tests/e2e/reference_matrix.py` for an inventory/source/test-symbol
integrity audit. `--list-gaps` reports the two unresolved literal vendor Alt arities, not
two missing mathematical-core tests or two open original criteria.
`--run PsiBetaGenAlt` or another name executes an exact real mapped Cargo test;
a planned-only entry is refused, and a zero-test Cargo match is an error.
Inventory success alone is not implementation acceptance. The final audit
executed the BetaGenAlt and automatic PsiFit mapped tests successfully; each
matched and passed one actual test.
