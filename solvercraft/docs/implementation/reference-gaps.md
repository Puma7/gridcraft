# Exact remaining reference-function work

This report preserves the original CSV criteria. **R046** is the111 frozen
continuous/Alt/discrete/custom/special/time-series function entries, with each
mathematical core tested individually. **R081** concerns supported PSI worksheet
signatures/defaults. **R091** is the complete272-row test/platform/guarantee
matrix, with gaps explicit. These are distinct acceptance questions.

## Executable R046 cores

| Current evidence | Function count |
|---|---:|
| Ordinary/custom/special/time-series cores with executed tests | 82 |
| Typed parameter cores with executed tests (26 Alt plus NormalSkew) | 27 |
| Own determined mathematical Alt variants with unresolved legacy arity | 2 |
| Unambiguous function names without an executable SDK core | 0 |
| Frozen distribution/time-series inventory | 111 |

The two unresolved legacy functions are **PsiPearson6Alt** and
**PsiWeibullAlt**. The exact source contradictions/questions and tested own
3-/2-claim policies are in [reference-open-questions.md](reference-open-questions.md).
The existing ordinary Pearson6/Weibull distributions are implemented and tested;
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
are below; all272 full PSI acceptance records remain open rather than being
converted into a claim of vendor parity.

## Explicit core/domain limitations

- Metalog: sufficient global derivative certificate only. Valid coefficients
  outside that sufficient class and upper-only bounded transforms are rejected.
  The published reference's full coefficient validity domain is not reproduced.
- Alt numerical fits: supported positive shape[1e-4,1e4], bounded normalized
  location/width/interior-mode ranges and configured work limits. Convergence
  failures and non-identifiability are explicit. A matching candidate is not a
  proof of unique vendor branch choice. Each advertised constructor's tests and
  exact policy are in [reference-functions.md](reference-functions.md).
- Custom Cumul/CumulD/General/Histogram constructors: at most32 nonzero mixture
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

## Exact R081 worksheet-import gap

Nineteen strict required-argument PSI imports are executed in the actual host:
Uniform, IntUniform, Normal, LogNormal, LogNorm2, Triangular, Pert, Beta, Gamma,
Exponential, Weibull, Bernoulli, Binomial, Poisson, NegBinomial, Geometric,
DisUniform, Discrete and Cumul. They preserve declared parameter conventions and
strict domains; properties and a vendor-identical outside-value policy are not
claimed. All are prefixed `Psi` in the frozen catalog.

The remaining **92 R046 names** have executable SDK cores or the two explicit
mathematical alternatives, but no verified PSI worksheet importer. This is host
compatibility work, not a missing scalar math implementation:

- continuous_distribution: PsiBetaGen, PsiBetaGenAlt, PsiBetaSubj, PsiBurr12, PsiCauchy, PsiCauchyAlt, PsiChiSquare, PsiChiSquareAlt, PsiDagum, PsiDblTriang, PsiErf, PsiErfAlt, PsiErlang, PsiExponentialAlt, PsiFatigueLife, PsiFatigueLifeAlt, PsiFDist, PsiFrechet, PsiFrechetAlt, PsiGammaAlt, PsiHypSecant, PsiHypSecantAlt, PsiInvNormal, PsiInvNormalAlt, PsiJohnsonSB, PsiJohnsonSU, PsiKumaraswamy, PsiLaplace, PsiLaplaceAlt, PsiLevy, PsiLevyAlt, PsiLogistic, PsiLogisticAlt, PsiLogLogistic, PsiLogLogisticAlt, PsiLogNormalAlt, PsiMakeInput, PsiMaxExtreme, PsiMaxExtremeAlt, PsiMetalog, PsiMetalogFit, PsiMetalogSPT, PsiMinExtreme, PsiMinExtremeAlt, PsiMomentFit, PsiMyerson, PsiNormalAlt, PsiNormalSkew, PsiPareto, PsiParetoAlt, PsiPareto2, PsiPareto2Alt, PsiPearson5, PsiPearson5Alt, PsiPearson6, PsiPearson6Alt, PsiPertAlt, PsiRayleigh, PsiRayleighAlt, PsiReciprocal, PsiStudent, PsiStudentAlt, PsiTriangGen, PsiTriangularAlt, PsiUniformAlt, PsiVary, PsiWeibullAlt.
- discrete_distribution: PsiHyperGeo, PsiLogarithmic.
- custom_distribution: PsiCumulD, PsiGeneral, PsiHistogram.
- special_distribution: PsiCertified, PsiDistInfo, PsiFit, PsiMVLogNormal, PsiMVNormal, PsiMVResample, PsiResample, PsiMVShuffle, PsiSip, PsiSlurp.
- simulation_time_series: PsiAR1, PsiAR2, PsiMA1, PsiMA2, PsiARCH1, PsiARMA11, PsiEGARCH11, PsiGARCH11, PsiAPARCH11, PsiTSSip.

The other161 frozen reference functions (optimization/property/statistics/
control/dimensional/etc.) remain separately visible in the272-row R081/R091
matrix. A generic SDK optimization, statistics or copula capability is not
relabelled as execution of those literal worksheet signatures/defaults.

## Platforms and reproducible audit

Recorded numerical tests are Windows/MSVC Rust1.95; peer special-function
WASM32 checks are compile evidence only. No blanket per-function browser,
Linux/macOS runtime or vendor binary comparison is inferred. The project's
separate actual worker/browser controls retain their own narrower scope.

Run `python tests/e2e/reference_matrix.py` for an inventory/source/test-symbol
integrity audit. `--list-gaps` reports the two unresolved R046 legacy arities.
`--run PsiBetaGenAlt` or another name executes an exact real mapped Cargo test;
a planned-only entry is refused, and a zero-test Cargo match is an error.
Inventory success alone is not implementation acceptance. The final audit
executed the BetaGenAlt and automatic PsiFit mapped tests successfully; each
matched and passed one actual test.
