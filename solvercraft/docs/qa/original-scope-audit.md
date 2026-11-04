# Original scope audit: R046 and R081

Audit date: 2026-11-01. This documentation-only audit retains the historical final6 original-criterion proof and the source-bound final7 extension. It does not rerun gates, change production files, alter the frozen reference/catalog or update global acceptance counts. Final5 remains separately preserved historical evidence. The final6 assessment below uses its independent reports and retains both worker attempts; the active final7 extension is separately qualified below.

## Final7: source-bound qualification of 31 core imports

This extension adds 12 scalar core forms to the previously accepted 19, without
changing any original 96 CSV field or the frozen external reference. The current
branch's shared registry and maintained implementation matrix list 31 forms;
80 distribution-name worksheet adapters remain unsupported. The final6
mathematical and 19-form log associations below remain historical evidence for
that snapshot, not acceptance evidence for the new code.

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

The [final7 summary](../implementation/final7-verification-summary.json) and [actual worker report](../implementation/browser-worker-final7-report.json) identify this run. Raw code/input SHA-256: `8d74eebf83ef7986d0dd5cb302ce159413b49d7e51c5327947a2d8444e505065`; LF-canonical SHA-256: `dc58726c5be86ddda8e5c6bfffdfe25507a29b66bedd51e50f376bcfbc3ad472`. Historical final6 evidence remains separately labelled below.

Current native PSI evidence comprises 31 separately named positive forms and 9 negative/policy groups (40 engine tests), plus 37 Calc tests. These selectors are present in the [final7 native test excerpt](../implementation/gates-final7/host/host-ci.txt). The [CDF-prefix witness](../implementation/psi-core12-cdf-final7-report.json) records the 24 comparisons and their outside-value basis. These public files are compact projections/excerpts, not complete private diagnostic dumps.


The new twelve are ChiSquare, Erlang, FDist, Logistic, MaxExtreme, MinExtreme,
Rayleigh, Reciprocal, Student, Levy, Kumaraswamy and HypSecant. Their
[source-page/domain table](../implementation/reference-gaps.md) includes the
four-argument affine Kumaraswamy constructor and HypSecant native-SD conversion
`SDKscale = beta * FRAC_2_PI`. Cauchy/Laplace/HyperGeo, optional properties, Alt
calls and nested PSI randomness remain unsupported, not silently completed.
The final7 reports/excerpts have their own source identities;
shared current acceptance/verification pages are maintained by the integrator.

## Unchanged original contract

Original [requirements.csv](../masterplan/requirements.csv), byte-identical to the original planning artifact: SHA-256 `699e87ae50a62ff5605b0d40dee38f4b2f8b7200a4c92f825d2972a57e24de55`. The eight original fields are retained below, including historical `planned` status and planned test IDs:

```csv
"R046","R4","SC-14","Verteilungskatalog","Jede weitere eingefrorene Referenzfunktion einzeln testen","O10","planned","TEST-R046"
"R081","R4","SC-23","PSI Import","Jede unterstützte Signatur und Defaultsemantik separat geprüft","O10","planned","TEST-R081"
"R091","R5","SC-25","Erfüllungsmatrix","Jede Referenzfunktion mit Test, Plattform und Garantie; Lücken offen","O4","planned","TEST-R091"
```

The original requirement is interpreted together with its assigned task and masterplan section; later catalog annotations are evidence records, not amendments to the original acceptance wording.

### SC-14 and masterplan section 10.1

Original [SC-14 task](../masterplan/tasks.json), deliverables:

> Versionierte counter-basierte Streams und R2-Verteilungen implementieren.  
> CDF/Quantile/Momente/Sampling, Trunkierung und Parametersemantik festlegen.  
> Quelldaten und Tabellenlizenzen prüfen.  

Original SC-14 acceptance:

> Samples unabhängig von Workerzahl und Auswertungsreihenfolge.  
> Tail-, Domain-, Degenerations- und undefinierte-Momente-Tests.  
> RNG ist Simulationsbaustein, kein unbegründetes kryptografisches Versprechen.  

Original [MASTERPLAN section 10.1](../masterplan/MASTERPLAN.md):

> R4 ergänzt nach eingefrorenem Referenzkatalog unter anderem Student-t, Chi-Quadrat, F, Cauchy, Logistic, Laplace, Gumbel, Pareto, Rayleigh, Erlang, inverse Gamma, inverse Gaussian, Loglogistic, Johnson-Familien, Skew-Normal, Hypergeometrisch, kategoriale und weitere erforderliche Familien. Trunkierung, Verschiebung, Skalierung, Mischungen, zusammengesetzte Verteilungen und Parameter aus Perzentilen werden als eigene Fähigkeiten gezählt, nicht zum Aufblähen einer Verteilungszahl.

> Zieldimension ist die vollständige einzeln getestete Referenzliste. Für numerisch schwierige Tails oder undefinierte Momente gibt es eine explizite Rückgabe statt irreführender Zahlen.

### SC-23 and masterplan section 12

Original [SC-23 task](../masterplan/tasks.json), selected-PSI deliverable and its related acceptance:

> Ausgewählte Psi*-Signaturen mit echten Semantiktests implementieren.

> Dokumentierte PSI-Parameterreihenfolge und Fehlersemantik prüfen.

Original [MASTERPLAN section 12](../masterplan/MASTERPLAN.md):

> Psi*-Kompatibilität wird funktionsweise eingeführt: dokumentierte Signatur, Defaultparameter, Reihenfolge der Argumente, Verhalten außerhalb der Simulation und Fehlerfälle erhalten eigene Tests. Beispiel: PsiTriangular verwendet laut Online-Hilfe die Reihenfolge Minimum, Modus, Maximum. O11. Ein pauschaler Namensalias genügt nicht. Unbekannte Funktionen bleiben erhalten und werden verständlich als nicht unterstützt gemeldet.

## R046: accepted bounded mathematical catalog

The original wording is assigned to SC-14 and section 10.1: mathematical distributions, parameter conventions and parameter construction from percentiles. Literal worksheet overloads, omitted vendor defaults, properties and outside-simulation context are separately assigned to SC-23/section 12. A documented, tested mathematical constructor is not an assertion that an undocumented PSI calling grammar is compatible.

**Abnahme: accepted, bounded.** All 111 frozen mathematical capability names are individually associated with positive tests in the actual final6 SDK run of 630 passed/0 failed/0 ignored. This is execution matching, not inventory presence or rejection-only evidence. Several named capability cases share table-driven test functions: 111 names correspond here to 73 distinct test bodies across 13 source files, not 111 independent Cargo functions.

The two disputed Alt constructions are positive mathematical implementations:

- Pearson 6/BetaPrime: `reference_functions::bounded(BoundedFamily::BetaPrime, ...)` fits three percentile claims, checks two unused CDF anchors against an independently expanded BetaPrime polynomial, and separately fits explicit shape1/shape2/scale. Rejecting the underdetermined two-claim input is additional evidence, not the implementation.
- Weibull: `reference_functions::weibull` computes two-percentile, shape+percentile and scale+percentile forms and checks an independent power-exponential CDF. No undocumented one-claim default is invented.

The mathematical owner confirmed both positive implementations and no known rejection-only R046 entry. Numerical domains, convergence/budget limitations and unsupported moment/tail cases remain explicit in [reference-gaps.md](../implementation/reference-gaps.md) and [reference-functions.md](../implementation/reference-functions.md). There is no full-domain or literal 111-vendor-contract guarantee.

### Matrix and actual final6 SDK log match

- 104 entries already have an exact SDK locator in the frozen matrix and an `... ok` line in the final6 log. Every selected source-file hash matches its matrix record.
- Seven baseline entries have only a host-test locator in that matrix. Their positive, already executed SDK tests are supplied explicitly in AppendixA under `Audit supplement`; no compiled JSON is edited.
- 111 entries matched, 73 distinct positive test bodies, 13 source files; no selected failed/ignored/unmatched test. AppendixA gives exact source/test/log-line associations; AppendixC gives source hashes.
- Public test-result excerpt [SDK log](../implementation/gates-final6/sdk/sdk-tests.txt) SHA-256: `dec9422d0180c8e3960e97e8df072584c7cad0291f0e783f3ad6f197ecb1158b`.
- Historical final6 implementation-matrix SHA-256: `8b27561b6955c05b7e9c946d8ef8ad231395ba2c6a1705c82496e1eb00f5c3d8`. The [maintained matrix](../implementation/reference-implementation.json) now describes the active final7 overlay; that current file is not claimed to retain the historical whole-file hash. Mathematical source/test associations below retain their stated final6 scope.
- Accepted final6 raw code/input digest: `c4a349613fcb95e65a56429acd34c08a7f129102edb346c545854026e11795a7`; canonical digest: `0ed5c92fd31d137b68115df67e910fd50ff8ef7a145896da86d1578b2eb43d76`. [Primary summary](../implementation/final6-verification-summary.json).

The matrix status `partial_parameterization` on the two literal Alt records describes unresolved vendor grammar. It is not silently changed to full PSI compatibility, and it does not add an undocumented-overload condition to the original mathematical criterion.

## R081: accepted selected 19-form conversion contract

Original SC-23 explicitly selects supported PSI signatures; section 12 introduces them function by function and preserves unknown calls. Consequently R081 is not an independent requirement to import all 272 names. It does require separate semantic checks for every actually supported form/policy, rather than accepting a blind name alias or SDK family resemblance.

**Abnahme: accepted, bounded.** The actual advertised contract consists of 19 top-level numeric distribution forms with required arguments, default preview/no-apply behavior and explicit outside-value conversion policies. Native execution now includes 19 individually named import-to-Calc-to-discovery-to-IR-to-simulation cases plus six negative/policy groups (25 new engine tests), and 21 Calc tests. The one actual Chrome worker body covers all 19 signatures, 2048 trials and 19 outputs; it passed in both recorded worker attempts. These are not 19 separate browser test bodies.

The default `outside_policy="solvercraft_nominal"` explicitly converts outside-run values to SC theoretical nominal values and reports `vendor_display_context_preserved=false` in the response and proposed changes. `preserve_vendor` refuses conversion because the vendor display context is unavailable. This is a tested, disclosed lossy converter; it does not claim random pre-run values, selected-trial/sample-mean/base-case display lifecycle equivalence or a new ValueToDisplay UI.

The independent integration owner identified the earlier mean-only oracle, missing end-to-end cases and recursive-import/top-level-discovery mismatch. The final6 changes address those concrete gaps:

- Asymmetric CDF/descriptor checks now distinguish Gamma shape/scale, Normal sigma, LogNorm2 log-standard-deviation and negative-binomial p versus 1-p. Retained rows exercise actual trial evaluation and repeated reads; previews leave the live workbook unchanged and the import is undoable.
- Negative groups cover omitted/blank/extra arguments, domain and parameter-formula errors, mixed supported/unsupported formulas, policy refusal, strict range/apply types and revision/undo/redo invariance.
- Nested PSI expressions and property-bearing calls are preserved as whole unsupported formulas. They are not rewritten into undiscoverable random expressions, and unsupported properties never become the SC-only numeric nominal override. Parentheses around one supported top-level declaration remain discoverable.

That correction is an admission/refusal guarantee, not implementation of nested PSI randomness or PSI properties. The remaining 92 distribution-name worksheet adapters and other unsupported names may be useful future work, but their absence is not an added mandatory 272-import condition under R081. Literal Alt grammar, full vendor default/property/display context and proprietary data remain open compatibility gaps.

### R081 native and browser proof

- Public test-result excerpt [host CI log](../implementation/gates-final6/host/host-ci.txt) SHA-256: `65b3376b2b69b40f25fdb40d92d5f892ed48181fe8e2db150f20ac1b017cb41c` contains all 25 `cmd::solvercraft::psi_import_tests` and all 21 `eval::solvercraft::tests` as passed. AppendixB maps the 19 selected signatures separately.
- [Engine cases](../../../crates/engine/src/cmd/solvercraft/psi_import_tests.rs), [Calc cases](../../../crates/calc/src/eval/solvercraft/tests.rs), [worker case](../../tests/worker-browser/tests/worker.rs). The browser selector is `actual_browser_imports_nineteen_psi_signatures_and_preserves_vendor_context_refusals`.
- [Worker composite](../implementation/browser-worker-final6-report.json), [initial attempt](../implementation/browser-worker-final6-initial-report.json), [identical controlled repeat](../implementation/browser-worker-final6-rerun-report.json). The R081 body passed in both. The full worker suite initially passed 7/failed 1 at an additional 26 ms-versus-below 25 ms heartbeat assertion; exactly one unchanged repeat passed 8/0. Aggregate all-attempts-passed remains false and the initial cause is unproven.

The original R023 target is 250 ms normal-model status access. Repeat measurements are admission 0.81 ms, status 1 ms, active progress gap 57 ms and heartbeat 12 ms. The additional 25 ms heartbeat check is not converted into a product promise, hidden, or used to manufacture a mandatory new R081 contract. Initial full status/progress maxima were not logged after the failing assertion.

## Full-reference gaps and R091 remain distinct

The 272-entry inventory is the unfiltered frozen reference/matrix universe. R091 explicitly requires each reference function to retain test/platform/guarantee records with gaps open. R046 covers 111 mathematical catalog entries; R081 checks the 19 actually supported calls and disclosed conversion policies. Those scoped acceptances do not close all full-vendor PSI records.

The unresolved exact Pearson 6Alt/WeibullAlt arity/defaults, selected NormalSkew/AR2/APARCH formula discrepancies, oversized MVResample mapping and source-specific kurtosis conventions remain in [reference-open-questions.md](../implementation/reference-open-questions.md). Proprietary Certified libraries and proprietary Fit defaults are not inferred from mathematical replacements. Five mentioned names still lack complete frozen signatures: PsiCorrelate, PsiLen, PsiTruncateD, PsiXtoP, PsiXtoQ. Unsupported records remain visible rather than counted as implemented.

The public reference already specifies several useful optional extensions (for example no-op metadata properties, Static, Lock, Shift, numeric censoring/truncation, Vary defaults and selected result/control adapters). Their future implementation needs no blanket user clarification, but they are not silently added to the presently supported import contract. In particular PDFp358 calls Truncate type 1 the default while the frozen catalog lists 0; Censor type 0 genuinely inherits a platform setting. That source/catalog discrepancy must be resolved before expanding those adapters, rather than inventing a default.

## AppendixA: all 111 positive capability-to-final6-SDK-test associations

Every selected locator below occurs as passed in the current final6 SDK log. Positive test cases, not just malformed-input refusals, underpin the mathematical acceptance. Additional domain/tail tests remain in the suites and frozen matrix.

| Capability | SDK source / exact test name | Final6 SDK log | Locator origin |
|---|---|---|---|
| PsiBeta | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiBetaGen | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::rescaled_beta_subjective_moments_and_erf_have_independent_oracles` | [line 887](../implementation/gates-final6/sdk/sdk-tests.txt#L887): ok | Frozen matrix |
| PsiBetaGenAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_beta_gen_alt_four_percentiles_recover_independent_beta_polynomial` | [line 1044](../implementation/gates-final6/sdk/sdk-tests.txt#L1044): ok | Frozen matrix |
| PsiBetaSubj | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::rescaled_beta_subjective_moments_and_erf_have_independent_oracles` | [line 887](../implementation/gates-final6/sdk/sdk-tests.txt#L887): ok | Frozen matrix |
| PsiBurr12 | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiCauchy | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_cauchy_native_parameter_convention_has_independent_quartiles` | [line 1042](../implementation/gates-final6/sdk/sdk-tests.txt#L1042): ok | Frozen matrix |
| PsiCauchyAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_cauchy_alt_two_percentiles_preserve_independent_tangent_anchors` | [line 1038](../implementation/gates-final6/sdk/sdk-tests.txt#L1038): ok | Frozen matrix |
| PsiChiSquare | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiChiSquareAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_chi_square_alt_variance_and_percentile_match_df_two_exponential` | [line 1039](../implementation/gates-final6/sdk/sdk-tests.txt#L1039): ok | Frozen matrix |
| PsiDagum | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiDblTriang | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::double_triangle_matches_its_two_independent_cdf_branches` | [line 881](../implementation/gates-final6/sdk/sdk-tests.txt#L881): ok | Frozen matrix |
| PsiErf | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::rescaled_beta_subjective_moments_and_erf_have_independent_oracles` | [line 887](../implementation/gates-final6/sdk/sdk-tests.txt#L887): ok | Frozen matrix |
| PsiErfAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_erf_alt_variance_and_percentile_are_not_exponential_scale` | [line 1046](../implementation/gates-final6/sdk/sdk-tests.txt#L1046): ok | Frozen matrix |
| PsiErlang | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiExponential | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::lognormal_conventions_and_exponential_mean_are_distinct` | [line 868](../implementation/gates-final6/sdk/sdk-tests.txt#L868): ok | Frozen matrix |
| PsiExponentialAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_exponential_alt_variance_and_percentile_preserve_mean_convention` | [line 1043](../implementation/gates-final6/sdk/sdk-tests.txt#L1043): ok | Frozen matrix |
| PsiFatigueLife | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiFatigueLifeAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_fatigue_life_alt_all_percentile_native_combinations_preserve_gaussian_geometry` | [line 1047](../implementation/gates-final6/sdk/sdk-tests.txt#L1047): ok | Frozen matrix |
| PsiFDist | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiFrechet | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiFrechetAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_frechet_alt_all_percentile_native_combinations_preserve_power_exponential_geometry` | [line 1061](../implementation/gates-final6/sdk/sdk-tests.txt#L1061): ok | Frozen matrix |
| PsiGamma | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiGammaAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_gamma_alt_supports_two_quantiles_and_all_native_moment_pairs` | [line 1050](../implementation/gates-final6/sdk/sdk-tests.txt#L1050): ok | Frozen matrix |
| PsiHypSecant | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiHypSecantAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_hyp_secant_alt_uses_the_documented_location_scale_convention` | [line 1037](../implementation/gates-final6/sdk/sdk-tests.txt#L1037): ok | Frozen matrix |
| PsiInvNormal | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_inv_normal_native_mean_shape_have_independent_gaussian_anchor` | [line 1048](../implementation/gates-final6/sdk/sdk-tests.txt#L1048): ok | Frozen matrix |
| PsiInvNormalAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_inv_normal_alt_mean_variance_and_native_lambda_have_independent_cdf_anchors` | [line 1045](../implementation/gates-final6/sdk/sdk-tests.txt#L1045): ok | Frozen matrix |
| PsiJohnsonSB | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiJohnsonSU | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_johnson_su_native_shapes_location_scale_have_independent_normal_anchor` | [line 1040](../implementation/gates-final6/sdk/sdk-tests.txt#L1040): ok | Frozen matrix |
| PsiKumaraswamy | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiLaplace | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_laplace_native_location_scale_have_independent_tail_anchors` | [line 1049](../implementation/gates-final6/sdk/sdk-tests.txt#L1049): ok | Frozen matrix |
| PsiLaplaceAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_laplace_alt_percentile_and_stddev_match_exponential_tails` | [line 1041](../implementation/gates-final6/sdk/sdk-tests.txt#L1041): ok | Frozen matrix |
| PsiLevy | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiLevyAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_levy_alt_preserves_independent_normal_tail_anchor` | [line 1051](../implementation/gates-final6/sdk/sdk-tests.txt#L1051): ok | Frozen matrix |
| PsiLogistic | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiLogisticAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_logistic_alt_mean_and_variance_determine_independent_quartiles` | [line 1054](../implementation/gates-final6/sdk/sdk-tests.txt#L1054): ok | Frozen matrix |
| PsiLogLogistic | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiLogLogisticAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_log_logistic_alt_three_parameters_preserve_nonzero_location` | [line 1057](../implementation/gates-final6/sdk/sdk-tests.txt#L1057): ok | Frozen matrix |
| PsiLogNormal | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::lognormal_conventions_and_exponential_mean_are_distinct` | [line 868](../implementation/gates-final6/sdk/sdk-tests.txt#L868): ok | Frozen matrix |
| PsiLogNormalAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_log_normal_alt_mixed_arithmetic_claims_do_not_confuse_log_parameters` | [line 1052](../implementation/gates-final6/sdk/sdk-tests.txt#L1052): ok | Frozen matrix |
| PsiLogNorm2 | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::lognormal_conventions_and_exponential_mean_are_distinct` | [line 868](../implementation/gates-final6/sdk/sdk-tests.txt#L868): ok | Frozen matrix |
| PsiMakeInput | [crates/solvercraft/tests/catalog_inputs.rs](../../crates/solvercraft/tests/catalog_inputs.rs) / `psimakeinput_compounds_the_whole_expression_and_preserves_event_reads` | [line 326](../implementation/gates-final6/sdk/sdk-tests.txt#L326): ok | Frozen matrix |
| PsiMaxExtreme | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_max_extreme_native_location_scale_have_independent_exponential_anchor` | [line 1053](../implementation/gates-final6/sdk/sdk-tests.txt#L1053): ok | Frozen matrix |
| PsiMaxExtremeAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_max_extreme_alt_mean_mode_and_variance_use_gumbel_constants` | [line 1056](../implementation/gates-final6/sdk/sdk-tests.txt#L1056): ok | Frozen matrix |
| PsiMetalog | [crates/math/tests/metalog.rs](../../crates/math/tests/metalog.rs) / `original_ten_term_basis_order_has_an_independent_expanded_oracle` | [line 988](../implementation/gates-final6/sdk/sdk-tests.txt#L988): ok | Frozen matrix |
| PsiMetalogFit | [crates/math/tests/metalog.rs](../../crates/math/tests/metalog.rs) / `ols_recovers_independent_coefficients_and_is_labeled_not_mle` | [line 986](../implementation/gates-final6/sdk/sdk-tests.txt#L986): ok | Frozen matrix |
| PsiMetalogSPT | [crates/math/tests/metalog.rs](../../crates/math/tests/metalog.rs) / `spt_preserves_three_quantiles_in_each_supported_bound_mode` | [line 985](../implementation/gates-final6/sdk/sdk-tests.txt#L985): ok | Frozen matrix |
| PsiMinExtreme | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::minimum_extreme_is_the_correct_gumbel_reflection` | [line 870](../implementation/gates-final6/sdk/sdk-tests.txt#L870): ok | Frozen matrix |
| PsiMinExtremeAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_min_extreme_alt_mean_and_scale_have_the_correct_reflection` | [line 1058](../implementation/gates-final6/sdk/sdk-tests.txt#L1058): ok | Frozen matrix |
| PsiMomentFit | [crates/math/tests/metalog.rs](../../crates/math/tests/metalog.rs) / `moment_fit_is_separate_from_ols_and_checks_the_requested_subfamily` | [line 982](../implementation/gates-final6/sdk/sdk-tests.txt#L982): ok | Frozen matrix |
| PsiMyerson | [crates/math/tests/elicitation.rs](../../crates/math/tests/elicitation.rs) / `myerson_preserves_named_percentiles_for_both_skew_directions` | [line 943](../implementation/gates-final6/sdk/sdk-tests.txt#L943): ok | Frozen matrix |
| PsiNormal | [crates/math/src/distributions.rs](../../crates/math/src/distributions.rs) / `distributions::tests::normal_quantiles_match_independent_constants` | [line 879](../implementation/gates-final6/sdk/sdk-tests.txt#L879): ok | Audit supplement |
| PsiNormalAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_normal_alt_supports_both_mixed_parameter_forms` | [line 1059](../implementation/gates-final6/sdk/sdk-tests.txt#L1059): ok | Frozen matrix |
| PsiNormalSkew | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_normal_skew_is_myerson_percentile_geometry_not_azzalini_shape` | [line 1060](../implementation/gates-final6/sdk/sdk-tests.txt#L1060): ok | Frozen matrix |
| PsiPareto | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `shifted_reference_constructors_preserve_native_order_and_bounds` | [line 1075](../implementation/gates-final6/sdk/sdk-tests.txt#L1075): ok | Frozen matrix |
| PsiParetoAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pareto_alt_recovers_independent_power_quantiles_and_native_scale` | [line 1063](../implementation/gates-final6/sdk/sdk-tests.txt#L1063): ok | Frozen matrix |
| PsiPareto2 | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiPareto2Alt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pareto2_alt_recovers_lomax_quantiles_and_native_shape` | [line 1062](../implementation/gates-final6/sdk/sdk-tests.txt#L1062): ok | Frozen matrix |
| PsiPearson5 | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pearson5_native_shape_scale_have_independent_inverse_exponential_anchor` | [line 1064](../implementation/gates-final6/sdk/sdk-tests.txt#L1064): ok | Frozen matrix |
| PsiPearson5Alt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pearson5_alt_recovers_inverse_exponential_quantiles` | [line 1072](../implementation/gates-final6/sdk/sdk-tests.txt#L1072): ok | Frozen matrix |
| PsiPearson6 | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiPearson6Alt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pearson6_alt_mathematical_three_claim_core_matches_beta_prime_polynomial` | [line 1066](../implementation/gates-final6/sdk/sdk-tests.txt#L1066): ok | Frozen matrix |
| PsiPert | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiPertAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_pert_alt_three_percentiles_and_native_moments_preserve_weight_four` | [line 1068](../implementation/gates-final6/sdk/sdk-tests.txt#L1068): ok | Frozen matrix |
| PsiRayleigh | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_rayleigh_native_sigma_has_independent_chisquare_anchor` | [line 1067](../implementation/gates-final6/sdk/sdk-tests.txt#L1067): ok | Frozen matrix |
| PsiRayleighAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_rayleigh_alt_all_three_claims_use_sigma_not_weibull_scale` | [line 1065](../implementation/gates-final6/sdk/sdk-tests.txt#L1065): ok | Frozen matrix |
| PsiReciprocal | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiStudent | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiStudentAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_student_alt_percentile_recovers_cauchy_df_one_and_rejects_median` | [line 1069](../implementation/gates-final6/sdk/sdk-tests.txt#L1069): ok | Frozen matrix |
| PsiTriangGen | [crates/math/tests/elicitation.rs](../../crates/math/tests/elicitation.rs) / `generalized_triangle_recovers_independent_asymmetric_models_and_endpoints` | [line 941](../implementation/gates-final6/sdk/sdk-tests.txt#L941): ok | Frozen matrix |
| PsiTriangular | [crates/math/tests/deep_review.rs](../../crates/math/tests/deep_review.rs) / `triangular_scale_invariance_and_conditional_tail_probabilities` | [line 922](../implementation/gates-final6/sdk/sdk-tests.txt#L922): ok | Audit supplement |
| PsiTriangularAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_triangular_alt_three_percentiles_cover_both_piecewise_branches` | [line 1070](../implementation/gates-final6/sdk/sdk-tests.txt#L1070): ok | Frozen matrix |
| PsiUniform | [crates/math/tests/nominal.rs](../../crates/math/tests/nominal.rs) / `baseline17_means_have_the_same_canonical_spreadsheet_semantics` | [line 1007](../implementation/gates-final6/sdk/sdk-tests.txt#L1007): ok | Audit supplement |
| PsiUniformAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_uniform_alt_endpoint_and_percentile_forms_are_distinct` | [line 1071](../implementation/gates-final6/sdk/sdk-tests.txt#L1071): ok | Frozen matrix |
| PsiVary | [crates/math/tests/elicitation.rs](../../crates/math/tests/elicitation.rs) / `vary_actual_bounds_uses_the_declared_base_as_mode` | [line 942](../implementation/gates-final6/sdk/sdk-tests.txt#L942): ok | Frozen matrix |
| PsiWeibull | [crates/math/tests/deep_review.rs](../../crates/math/tests/deep_review.rs) / `weibull_cdf_preserves_finite_log_ratio_when_division_overflows_or_underflows` | [line 919](../implementation/gates-final6/sdk/sdk-tests.txt#L919): ok | Audit supplement |
| PsiWeibullAlt | [crates/math/tests/reference_functions.rs](../../crates/math/tests/reference_functions.rs) / `psi_weibull_alt_mathematical_two_claim_core_preserves_power_exponential_cdf` | [line 1073](../implementation/gates-final6/sdk/sdk-tests.txt#L1073): ok | Frozen matrix |
| PsiBernoulli | [crates/math/src/distributions.rs](../../crates/math/src/distributions.rs) / `distributions::tests::inverse_cdf_semantics_and_support` | [line 873](../implementation/gates-final6/sdk/sdk-tests.txt#L873): ok | Audit supplement |
| PsiBinomial | [crates/math/tests/deep_review.rs](../../crates/math/tests/deep_review.rs) / `binomial_small_exact_distribution_and_discrete_inverse_boundary` | [line 910](../implementation/gates-final6/sdk/sdk-tests.txt#L910): ok | Audit supplement |
| PsiGeometric | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiHyperGeo | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `extended_families_match_independent_identities` | [line 896](../implementation/gates-final6/sdk/sdk-tests.txt#L896): ok | Frozen matrix |
| PsiIntUniform | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiLogarithmic | [crates/math/tests/reference_families.rs](../../crates/math/tests/reference_families.rs) / `every_added_reference_family_has_an_independent_cdf_anchor` | [line 1029](../implementation/gates-final6/sdk/sdk-tests.txt#L1029): ok | Frozen matrix |
| PsiNegBinomial | [crates/math/tests/catalog.rs](../../crates/math/tests/catalog.rs) / `baseline_added_families_match_independent_closed_forms` | [line 899](../implementation/gates-final6/sdk/sdk-tests.txt#L899): ok | Frozen matrix |
| PsiPoisson | [crates/math/src/distributions.rs](../../crates/math/src/distributions.rs) / `distributions::tests::inverse_cdf_semantics_and_support` | [line 873](../implementation/gates-final6/sdk/sdk-tests.txt#L873): ok | Audit supplement |
| PsiCumul | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::custom_cumulative_is_not_equal_weight_empirical_interpolation` | [line 882](../implementation/gates-final6/sdk/sdk-tests.txt#L882): ok | Frozen matrix |
| PsiCumulD | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::custom_cumulative_is_not_equal_weight_empirical_interpolation` | [line 882](../implementation/gates-final6/sdk/sdk-tests.txt#L882): ok | Frozen matrix |
| PsiDiscrete | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::weighted_and_equal_support_lists_have_correct_atoms` | [line 885](../implementation/gates-final6/sdk/sdk-tests.txt#L885): ok | Frozen matrix |
| PsiDisUniform | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::weighted_and_equal_support_lists_have_correct_atoms` | [line 885](../implementation/gates-final6/sdk/sdk-tests.txt#L885): ok | Frozen matrix |
| PsiGeneral | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::histogram_and_general_preserve_normalized_probability_mass` | [line 883](../implementation/gates-final6/sdk/sdk-tests.txt#L883): ok | Frozen matrix |
| PsiHistogram | [crates/math/src/psi_aliases.rs](../../crates/math/src/psi_aliases.rs) / `psi_aliases::tests::histogram_and_general_preserve_normalized_probability_mass` | [line 883](../implementation/gates-final6/sdk/sdk-tests.txt#L883): ok | Frozen matrix |
| PsiCertified | [crates/solvercraft/tests/catalog_inputs.rs](../../crates/solvercraft/tests/catalog_inputs.rs) / `psicertified_resolves_only_caller_publications_and_named_sequential_sips` | [line 329](../implementation/gates-final6/sdk/sdk-tests.txt#L329): ok | Frozen matrix |
| PsiDistInfo | [crates/solvercraft/tests/catalog_inputs.rs](../../crates/solvercraft/tests/catalog_inputs.rs) / `psidistinfo_preserves_arithmetic_and_log_parameter_names_formulas_and_metadata` | [line 330](../implementation/gates-final6/sdk/sdk-tests.txt#L330): ok | Frozen matrix |
| PsiFit | [crates/solvercraft/tests/catalog_inputs.rs](../../crates/solvercraft/tests/catalog_inputs.rs) / `psifit_automatic_bic_policy_selects_candidates_and_invalidates_exact_snapshot_cache` | [line 334](../implementation/gates-final6/sdk/sdk-tests.txt#L334): ok | Frozen matrix |
| PsiMVLogNormal | [crates/math/tests/multivariate.rs](../../crates/math/tests/multivariate.rs) / `normal_and_latent_lognormal_population_moments_match_reproducible_draws` | [line 998](../implementation/gates-final6/sdk/sdk-tests.txt#L998): ok | Frozen matrix |
| PsiMVNormal | [crates/math/tests/multivariate.rs](../../crates/math/tests/multivariate.rs) / `covariance_map_has_an_independent_linear_algebra_oracle` | [line 997](../implementation/gates-final6/sdk/sdk-tests.txt#L997): ok | Frozen matrix |
| PsiMVResample | [crates/simulation/tests/resampling.rs](../../crates/simulation/tests/resampling.rs) / `resampling_preserves_intact_source_rows_or_columns_and_replays_exact_trials` | [line 1249](../implementation/gates-final6/sdk/sdk-tests.txt#L1249): ok | Frozen matrix |
| PsiResample | [crates/simulation/tests/resampling.rs](../../crates/simulation/tests/resampling.rs) / `resampling_preserves_intact_source_rows_or_columns_and_replays_exact_trials` | [line 1249](../implementation/gates-final6/sdk/sdk-tests.txt#L1249): ok | Frozen matrix |
| PsiMVShuffle | [crates/simulation/tests/resampling.rs](../../crates/simulation/tests/resampling.rs) / `shuffle_is_a_full_multiset_permutation_without_replacement` | [line 1248](../implementation/gates-final6/sdk/sdk-tests.txt#L1248): ok | Frozen matrix |
| PsiSip | [crates/simulation/tests/resampling.rs](../../crates/simulation/tests/resampling.rs) / `resampling_preserves_intact_source_rows_or_columns_and_replays_exact_trials` | [line 1249](../implementation/gates-final6/sdk/sdk-tests.txt#L1249): ok | Frozen matrix |
| PsiSlurp | [crates/simulation/tests/resampling.rs](../../crates/simulation/tests/resampling.rs) / `resampling_preserves_intact_source_rows_or_columns_and_replays_exact_trials` | [line 1249](../implementation/gates-final6/sdk/sdk-tests.txt#L1249): ok | Frozen matrix |
| PsiAR1 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arma_recursions_match_independent_two_step_algebra` | [line 1089](../implementation/gates-final6/sdk/sdk-tests.txt#L1089): ok | Frozen matrix |
| PsiAR2 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `gaussian_ar2_population_variance_matches_yule_walker_not_source_typo` | [line 1092](../implementation/gates-final6/sdk/sdk-tests.txt#L1092): ok | Frozen matrix |
| PsiMA1 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arma_recursions_match_independent_two_step_algebra` | [line 1089](../implementation/gates-final6/sdk/sdk-tests.txt#L1089): ok | Frozen matrix |
| PsiMA2 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arma_recursions_match_independent_two_step_algebra` | [line 1089](../implementation/gates-final6/sdk/sdk-tests.txt#L1089): ok | Frozen matrix |
| PsiARCH1 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arch_garch_and_egarch_match_independent_conditional_variance_algebra` | [line 1083](../implementation/gates-final6/sdk/sdk-tests.txt#L1083): ok | Frozen matrix |
| PsiARMA11 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arma_recursions_match_independent_two_step_algebra` | [line 1089](../implementation/gates-final6/sdk/sdk-tests.txt#L1089): ok | Frozen matrix |
| PsiEGARCH11 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arch_garch_and_egarch_match_independent_conditional_variance_algebra` | [line 1083](../implementation/gates-final6/sdk/sdk-tests.txt#L1083): ok | Frozen matrix |
| PsiGARCH11 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `arch_garch_and_egarch_match_independent_conditional_variance_algebra` | [line 1083](../implementation/gates-final6/sdk/sdk-tests.txt#L1083): ok | Frozen matrix |
| PsiAPARCH11 | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `aparch_power_two_zero_asymmetry_is_garch_and_leverage_changes_variance` | [line 1084](../implementation/gates-final6/sdk/sdk-tests.txt#L1084): ok | Frozen matrix |
| PsiTSSip | [crates/math/tests/time_series.rs](../../crates/math/tests/time_series.rs) / `tssip_replays_whole_columns_without_rng` | [line 1087](../implementation/gates-final6/sdk/sdk-tests.txt#L1087): ok | Frozen matrix |

## AppendixB: selected 19 native import and Calc tests

The native per-signature functions are separate executed tests. The real-browser nineteen-signature body is shared across these 19 rows and passed in both attempts; unsupported nesting/property preservation is verified in that browser body as well.

| Signature | Native engine selector | Calc selector | Final6 host-log lines |
|---|---|---|---|
| PsiUniform | `cmd::solvercraft::psi_import_tests::psi_uniform_import_executes_real_uniform` | `eval::solvercraft::tests::psi_uniform_default_override_domain_and_independent_parameters` | [engine 1808](../implementation/gates-final6/host/host-ci.txt#L1808), [Calc 1367](../implementation/gates-final6/host/host-ci.txt#L1367) |
| PsiIntUniform | `cmd::solvercraft::psi_import_tests::psi_intuniform_preserves_inclusive_discrete_endpoints` | `eval::solvercraft::tests::psi_intuniform_default_override_domain_and_independent_parameters` | [engine 1802](../implementation/gates-final6/host/host-ci.txt#L1802), [Calc 1364](../implementation/gates-final6/host/host-ci.txt#L1364) |
| PsiNormal | `cmd::solvercraft::psi_import_tests::psi_normal_sigma_changes_the_distribution` | `eval::solvercraft::tests::psi_normal_default_override_domain_and_independent_parameters` | [engine 1804](../implementation/gates-final6/host/host-ci.txt#L1804), [Calc 1358](../implementation/gates-final6/host/host-ci.txt#L1358) |
| PsiLogNormal | `cmd::solvercraft::psi_import_tests::psi_lognormal_uses_arithmetic_mean_and_stddev` | `eval::solvercraft::tests::psi_lognormal_default_override_domain_and_independent_parameters` | [engine 1807](../implementation/gates-final6/host/host-ci.txt#L1807), [Calc 1361](../implementation/gates-final6/host/host-ci.txt#L1361) |
| PsiLogNorm2 | `cmd::solvercraft::psi_import_tests::psi_lognorm2_sigma_is_not_variance` | `eval::solvercraft::tests::psi_lognorm2_default_override_domain_and_independent_parameters` | [engine 1806](../implementation/gates-final6/host/host-ci.txt#L1806), [Calc 1355](../implementation/gates-final6/host/host-ci.txt#L1355) |
| PsiTriangular | `cmd::solvercraft::psi_import_tests::psi_triangular_preserves_min_mode_max` | `eval::solvercraft::tests::psi_triangular_default_override_domain_and_independent_parameters` | [engine 1810](../implementation/gates-final6/host/host-ci.txt#L1810), [Calc 1363](../implementation/gates-final6/host/host-ci.txt#L1363) |
| PsiPert | `cmd::solvercraft::psi_import_tests::psi_pert_uses_the_fixed_four_weight` | `eval::solvercraft::tests::psi_pert_default_override_domain_and_independent_parameters` | [engine 1813](../implementation/gates-final6/host/host-ci.txt#L1813), [Calc 1365](../implementation/gates-final6/host/host-ci.txt#L1365) |
| PsiBeta | `cmd::solvercraft::psi_import_tests::psi_beta_retains_two_shape_order` | `eval::solvercraft::tests::psi_beta_default_override_domain_and_independent_parameters` | [engine 1796](../implementation/gates-final6/host/host-ci.txt#L1796), [Calc 1354](../implementation/gates-final6/host/host-ci.txt#L1354) |
| PsiGamma | `cmd::solvercraft::psi_import_tests::psi_gamma_shape_scale_order_has_an_independent_cdf` | `eval::solvercraft::tests::psi_gamma_default_override_domain_and_independent_parameters` | [engine 1803](../implementation/gates-final6/host/host-ci.txt#L1803), [Calc 1360](../implementation/gates-final6/host/host-ci.txt#L1360) |
| PsiExponential | `cmd::solvercraft::psi_import_tests::psi_exponential_mean_is_converted_to_rate` | `eval::solvercraft::tests::psi_exponential_default_override_domain_and_independent_parameters` | [engine 1798](../implementation/gates-final6/host/host-ci.txt#L1798), [Calc 1356](../implementation/gates-final6/host/host-ci.txt#L1356) |
| PsiWeibull | `cmd::solvercraft::psi_import_tests::psi_weibull_uses_nondegenerate_shape_scale` | `eval::solvercraft::tests::psi_weibull_default_override_domain_and_independent_parameters` | [engine 1812](../implementation/gates-final6/host/host-ci.txt#L1812), [Calc 1370](../implementation/gates-final6/host/host-ci.txt#L1370) |
| PsiBernoulli | `cmd::solvercraft::psi_import_tests::psi_bernoulli_keeps_zero_mass` | `eval::solvercraft::tests::psi_bernoulli_default_override_domain_and_independent_parameters` | [engine 1793](../implementation/gates-final6/host/host-ci.txt#L1793), [Calc 1350](../implementation/gates-final6/host/host-ci.txt#L1350) |
| PsiBinomial | `cmd::solvercraft::psi_import_tests::psi_binomial_keeps_count_and_success_probability` | `eval::solvercraft::tests::psi_binomial_default_override_domain_and_independent_parameters` | [engine 1794](../implementation/gates-final6/host/host-ci.txt#L1794), [Calc 1349](../implementation/gates-final6/host/host-ci.txt#L1349) |
| PsiPoisson | `cmd::solvercraft::psi_import_tests::psi_poisson_has_the_independent_zero_mass` | `eval::solvercraft::tests::psi_poisson_default_override_domain_and_independent_parameters` | [engine 1809](../implementation/gates-final6/host/host-ci.txt#L1809), [Calc 1369](../implementation/gates-final6/host/host-ci.txt#L1369) |
| PsiNegBinomial | `cmd::solvercraft::psi_import_tests::psi_negbinomial_probability_is_not_complemented` | `eval::solvercraft::tests::psi_negbinomial_default_override_domain_and_independent_parameters` | [engine 1805](../implementation/gates-final6/host/host-ci.txt#L1805), [Calc 1351](../implementation/gates-final6/host/host-ci.txt#L1351) |
| PsiGeometric | `cmd::solvercraft::psi_import_tests::psi_geometric_counts_failures_from_zero` | `eval::solvercraft::tests::psi_geometric_default_override_domain_and_independent_parameters` | [engine 1801](../implementation/gates-final6/host/host-ci.txt#L1801), [Calc 1352](../implementation/gates-final6/host/host-ci.txt#L1352) |
| PsiDisUniform | `cmd::solvercraft::psi_import_tests::psi_disuniform_uses_equal_mass_list` | `eval::solvercraft::tests::psi_disuniform_default_override_domain_and_independent_parameters` | [engine 1800](../implementation/gates-final6/host/host-ci.txt#L1800), [Calc 1353](../implementation/gates-final6/host/host-ci.txt#L1353) |
| PsiDiscrete | `cmd::solvercraft::psi_import_tests::psi_discrete_normalizes_nonunit_weights` | `eval::solvercraft::tests::psi_discrete_default_override_domain_and_independent_parameters` | [engine 1799](../implementation/gates-final6/host/host-ci.txt#L1799), [Calc 1357](../implementation/gates-final6/host/host-ci.txt#L1357) |
| PsiCumul | `cmd::solvercraft::psi_import_tests::psi_cumul_uses_piecewise_cdf_knots` | `eval::solvercraft::tests::psi_cumul_default_override_domain_and_independent_parameters` | [engine 1797](../implementation/gates-final6/host/host-ci.txt#L1797), [Calc 1362](../implementation/gates-final6/host/host-ci.txt#L1362) |

The six additional native policy/negative groups also passed:

- `cmd::solvercraft::psi_import_tests::explicit_invalid_selection_and_apply_types_cannot_broaden_an_import` ([host log line 1785](../implementation/gates-final6/host/host-ci.txt#L1785)).
- `cmd::solvercraft::psi_import_tests::mixed_supported_and_unsupported_cells_are_atomic_per_formula_and_one_undo` ([host log line 1789](../implementation/gates-final6/host/host-ci.txt#L1789)).
- `cmd::solvercraft::psi_import_tests::outside_policy_reports_loss_and_strict_vendor_preservation_is_refused` ([host log line 1790](../implementation/gates-final6/host/host-ci.txt#L1790)).
- `cmd::solvercraft::psi_import_tests::parenthesized_declaration_is_discoverable_and_unsupported_properties_are_never_nominal_overrides` ([host log line 1792](../implementation/gates-final6/host/host-ci.txt#L1792)).
- `cmd::solvercraft::psi_import_tests::invalid_imported_parameters_and_parameter_errors_never_become_simulation_results` ([host log line 1795](../implementation/gates-final6/host/host-ci.txt#L1795)).
- `cmd::solvercraft::psi_import_tests::unsupported_nested_properties_arity_and_blank_keep_whole_formula_and_history` ([host log line 1814](../implementation/gates-final6/host/host-ci.txt#L1814)).

## AppendixC: selected mathematical test-source hashes

| Source | SHA-256 |
|---|---|
| `crates/math/src/distributions.rs` | `659c242c4c7cd19c3545037a0233027dd2110edf3ff073502b9f5f5f582ae9c2` |
| `crates/math/src/psi_aliases.rs` | `9d22f7852493e31171e70753641c8f003db8feb0a56ea7a54c5dae77c4af92f2` |
| `crates/math/tests/catalog.rs` | `400d10b560c31ba8ce6a4db94918b15fa66753945d144b884e96288647d2c13f` |
| `crates/math/tests/deep_review.rs` | `fd6fe45ea2c015e747e2e6abbfe30def738eb4ac56fd0536cc9741101f9d4fc2` |
| `crates/math/tests/elicitation.rs` | `cc139961997bda3dfef204bc30122d451c86704a9843da1dd6737038768ca7f0` |
| `crates/math/tests/metalog.rs` | `0e9a35c6905fbe6483fcd17900fd8bc937eae9df71fe04496cff535a480308e1` |
| `crates/math/tests/multivariate.rs` | `3aa19e654af5317bffd70ca466a459302b3d6c5cc9bab5033ea97d5d89fb0150` |
| `crates/math/tests/nominal.rs` | `37cacba74f8bfba7bfd068b5532bb69d11171d2730b2b7521be1a4c05d3560c4` |
| `crates/math/tests/reference_families.rs` | `ab26c5e33e9dc4c6b21512afab2c0ade86aa4f8c3799abb5bb0cdc28f58737b1` |
| `crates/math/tests/reference_functions.rs` | `e735f41b3db85145180ff63de8481544a6716ee27cea610a45b996601cdc6465` |
| `crates/math/tests/time_series.rs` | `68b608204b45c25d645f1fe46f16d8cfcfd0e0ec2c108204d33c68c42ca3a820` |
| `crates/simulation/tests/resampling.rs` | `9e7e89d2d81bdb681fe9b19ca1835ce300723ab75e66939bf4b048af2535257f` |
| `crates/solvercraft/tests/catalog_inputs.rs` | `d947cc690dbba4c23d98a2fbc3ea16c32ee7cb625518fcdab1b0625b2c76a22d` |


The SDK/host log hashes above identify the current **strict public excerpts**,
not full original stdout or a merely path-redacted full log. Only permitted
public test/result/summary lines are retained; omitted lines are blank, so all
111 mathematical and 19 native/Calc references keep their original positions.
Full diagnostic originals remain private. Fixed-field JSON reports retain real
counts, measurements, versions and source/binary identities; their
`full_local_report_sha256` identifies the private source report. Public package
inventories contain Name/Version/SPDX, not complete feature trees or local
NOTICE-hash metadata. See the [public evidence guide](../implementation/public-evidence.md).
