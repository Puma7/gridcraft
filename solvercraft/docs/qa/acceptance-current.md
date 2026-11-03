# Current acceptance against the original 96 requirements

Latest numerical evidence: the [expanded qualification](numerical-expansion-2026-11-02.md)
extends the first review with larger independent matrices and additional
repairs, including explicit precision refusals. These limits apply to the
bounded historical criteria below; their original rows remain unchanged.

Numerical follow-up, 2026-11-02: the [independent numerical review](numerical-review-2026-11-02.md)
found and repaired silent errors that the historical final7 gates below did not
detect. Its current tests and qualifications supersede numerical confidence in
the affected cases. The original requirement rows and historical acceptance
records remain preserved; bounded acceptance does not establish correctness for
all parameter values.

Review date: 2026-11-01. This is the **final7** source-bound bounded assessment. The [structured assessment](acceptance-current.json) preserves all original fields and statuses.

**All 96 original criteria retain their accepted bounded contracts.** This is criterion tracking, not universal correctness, all-platform coverage or full legacy PSI/272-PSI parity. The supported import scope is31 explicit top-level core forms; historical19-form results stay historical.

Original CSV SHA-256 `699e87ae50a62ff5605b0d40dee38f4b2f8b7200a4c92f825d2972a57e24de55`: 96 rows, 768 exact original field comparisons, zero mismatches. Historical planned/test IDs are unchanged.

## Current final7 evidence

Primary [final7-verification-summary.json](../implementation/final7-verification-summary.json) supplies the explicit actual report map.

- Raw SHA-256: `8d74eebf83ef7986d0dd5cb302ce159413b49d7e51c5327947a2d8444e505065`.
- Canonical SHA-256: `dc58726c5be86ddda8e5c6bfffdfe25507a29b66bedd51e50f376bcfbc3ad472`; 791 code/input files.
- Release CLI SHA-256: `68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`.
- Host CI: 2904 passed/0 failed/1 ignored, 10 recorded stages, 89.4699 s. SDK: 630/0/0 passed/failed/ignored. Scoped tests are not added again.
- Native UI 6; chart numerical 8 + lifecycle 1; actual Chrome SDK 13.
- Worker: initial: 9 passed/0 failed/0 ignored, exit 0. `aggregate_all_attempts_passed=true`. Counts describe the actual declared attempts; old final6 attempts are separate below.
- R081: 31 native per-signature cases, 40 contract bodies and 37 Calc bodies.
- PSI browser body `actual_browser_imports_nineteen_psi_signatures_and_preserves_vendor_context_refusals`: 19 signatures/2048 trials/19 outputs.
- PSI browser body `actual_browser_imports_twelve_additional_psi_cores_with_independent_cdf_anchors`: 12 signatures/4096 trials/12 outputs/24 CDF anchors.

The two browser import bodies keep their own trial/output counts;31 signatures are not claimed to share the new4096-trial body. The explicitly lossy Solvercraft nominal policy remains separate from vendor display state; preserve_vendor refuses unavailable context. Unsupported nesting/properties/Alt calls remain preserved whole.

Release oracle counts and per-workload measurements are actual final7 data, not reused final6 performance figures:

- Oracles: {"exact_binary_variants_passed": 3, "failed": 0, "lp_passed": 30, "node_limit_controls_passed": 1, "qp_passed": 20, "robust_tiny_infeasible_passed": 4, "robust_vertex_passed": 20}.
- Performance: continuous_lp_100_variables_500_rows: median 194.541 ms, peak 11,681,792 bytes; streaming_mc_100000_trials_20_inputs_10_outputs: median 432.793 ms, peak 6,799,360 bytes. No engine speedup, isolated-host or commercial comparison is inferred.

## Historical final6 assessment retained

The following previous checkpoint text is historical. Its worker initial7/1, controlled repeat8/0, unproven timing cause and binary-oracle metadata erratum are not relabelled as final7 results.

> # Current acceptance against the original 96 requirements
>
> Review date: 2026-11-01. This is the **final6** source-bound assessment of the unchanged original criteria. The [structured assessment](acceptance-current.json) retains all eight original CSV fields plus implementation/evidence/platform/guarantee/limits.
>
> **All 96 original criteria are accepted within their declared bounded contracts.** This is criterion tracking, not a product-completeness percentage, universal correctness proof or full legacy PSI/272-PSI compatibility claim. The [original-scope audit](original-scope-audit.md) explains why R046 covers 111 tested mathematical cores and R081 covers the actually advertised 19 import forms/policies; neither is expanded or narrowed to manufacture a count.
>
> Original [requirements.csv](../masterplan/requirements.csv): 96 rows, UTF-8 BOM, SHA-256 `699e87ae50a62ff5605b0d40dee38f4b2f8b7200a4c92f825d2972a57e24de55`. All 768 original field values were copied directly by UTF-8-sig `csv.DictReader` and compared exactly: 0 mismatches. Historical `planned`/`TEST-Rxxx` values remain unchanged. No repository encoding defect or repair is asserted.
>
> ## Final6 evidence and the retained worker failure
>
> Primary [final6-verification-summary.json](../implementation/final6-verification-summary.json) gives the explicit path map. Native/SDK gates accepted unchanged source across 791 code/compilation-input files.
>
> - Raw code/input SHA-256: `c4a349613fcb95e65a56429acd34c08a7f129102edb346c545854026e11795a7`.
> - Canonical SHA-256: `0ed5c92fd31d137b68115df67e910fd50ff8ef7a145896da86d1578b2eb43d76`.
> - Release CLI SHA-256: `68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`; the same binary as the previous measured checkpoint.
> - Host `cargo xtask ci`: all 10 stages,**2873 passed/0 failed/1 pre-existing ignored**, 547.64 s. SDK fmt/all-target Clippy and **630 passed/0 failed/0 ignored**. Scoped suites are not added again to these totals.
> - Native UI: **6 passed**. Chart module: **8 numerical contracts+1 retained-result lifecycle test**. Actual Chrome SDK: **13 passed**.
> - Product Worker: initial **7 passed/1 failed**, then exactly one identical controlled repeat **8 passed/0 failed**. The initial assertion observed 26 ms against an additional strictly-below 25 ms window-heartbeat guard; its cause is **unproven**. `aggregate_all_attempts_passed=false` remains explicit. The R081 19-signature body passed in **both** attempts. [Composite](../implementation/browser-worker-final6-report.json), [initial](../implementation/browser-worker-final6-initial-report.json), [repeat](../implementation/browser-worker-final6-rerun-report.json).
> - Controlled-repeat observed maxima: admission **0.81 ms**, status **1 ms**, active progress gap **57 ms**, heartbeat **12 ms**. The original R023 normal-model status-access target is 250 ms; no 25 ms product or hard realtime guarantee is claimed. Initial full progress/status maxima are unavailable after the failed assertion.
> - Independent release oracles: **30 LP, 20 QP, 20 ordinary robust+4 tiny signed cases, 3 exact binary variants+1 node-limit control**, all passed.
> - Five fresh processes per workload: LP 100 variables/500 rows median **180.8091 ms**, peak **11,677,696 bytes**, objective error 0; streaming MC 100000 trials/20 inputs/10 outputs median **527.5174 ms**, peak **6,893,568 bytes**, maximum mean error 0.001469193929023671. Shared-host/quieter follow-up data are **not an engine speedup claim**, isolated comparison or medium-class benchmark.
>
> Final5 and earlier checkpoints remain historical rather than being relabelled as final6. The 4071-comparison SciPy scalar grid is separate finite evidence, with zero failures and one excluded independently warned reference.
>
> ## Public evidence scope and binary-oracle erratum
>
> Current final6 JSON diagnostics are **fixed-field public projections**, and linked text files are **strict test/result/summary excerpts**. Omitted lines are blank so the 111 mathematical and 19 import/Calc log references retain their positions. Full original diagnostics remain private; complete stdout/stderr, environment/argv, driver/session dumps, feature trees and local NOTICE-hash metadata are not promised by these links. Public dependency inventories expose package Name/Version/SPDX; checked-in notices are linked separately. See the [public evidence guide](../implementation/public-evidence.md).
>
> The independent binary-oracle v1 stored incorrect request metadata for its third case because a mutable dictionary was reused, although minimization to **-500** had actually run. Historical final5 v1 is left unchanged. The validator reran **only three binary cases plus one node-limit control** on the identical release binary with deep-copied snapshots and a separate node request. The [current compact report](../implementation/exact-binary-oracle-final6-report.json) correctly records minimization with negative coefficients, **-500**, default seed **0** (not 42), and node limit **1**. Its private corrected full-report SHA-256 is `60c193c40707b83632dd2ce2aee7b8fb62f8489c3fb7ae6e1ea06069c12e9afa`; these documentation changes trigger no additional test run.
>
> ## Meaning of acceptance and remaining compatibility scope
>
> Accepted, bounded means executed evidence meets the original wording within the explicit mathematical/API contract. It does not certify arbitrary numeric magnitudes, all platforms, proprietary behavior or every attempted extra timing guard. A rejecting guard proves unsupported behavior rather than implementation; unsupported nesting/properties are not counted as implemented.
>
> - **R046:** all 111 frozen mathematical capability names have individually associated positive tests in SDK 630, across 73 test bodies/13 source files. The two explicit Alt constructions are genuine positive mathematical computations; their literal vendor arity/default conflicts remain open compatibility records.
> - **R081:** 19 selected top-level numeric calls/default policies have 25 new native engine tests, 21 Calc tests and one real-browser body covering 19 signatures/2048 trials/19 outputs. `solvercraft_nominal` is explicitly lossy; `preserve_vendor` refuses unavailable vendor display context. Unsupported nesting, properties and extra calls remain unchanged whole formulas.
> - The wider 272-function inventory retains full-vendor gaps, unknown contracts and numerical limits. R091 requires those gaps to remain visible; its truthful matrix is not 272-function implementation.

## Requirement-by-requirement assessment

| Original ID / feature / acceptance | Status | Implementation and executed evidence | Platform | Guarantee and remaining limits |
|---|---|---|---|---|
| **R001 - Zieldefinition**<br>Minimierung, Maximierung und Zielwert mit eindeutiger numerischer Semantik | Accepted, bounded | SDK workflow and host min/max/target goals with original objective reporting<br>[MANIFEST](#evidence-manifest), [HOST](#evidence-host), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Target adds equality requiring the original objective to equal the requested value; original units are preserved, and unreachable integer targets are infeasible.<br>**Limits:** None within the declared numeric range. |
| **R002 - Variablentypen**<br>Kontinuierlich, ganzzahlig und binär; ungültige Domains ablehnen | Accepted, bounded | Continuous/integer/binary domains, effective bounds and checked integer repair<br>[MODEL](#evidence-model), [ENGINES](#evidence-engines), [LOGIC](#evidence-logic) | Native Rust 1.95 + only explicitly cited Chrome cases | Invalid domains fail; finite mixed-conic integers stay in exactly distinguishable f64 range.<br>**Limits:** None within the supported domain contract. |
| **R003 - Bounds**<br>Freie, einseitige, beidseitige und fixierte Variablen; keine JSON-Infinity | Accepted, bounded | Optional finite lower/upper bounds, free and fixed coordinates, strict finite DTO validation<br>[MODEL](#evidence-model), [ENGINES](#evidence-engines), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | No JSON Infinity substitute; algorithms requiring finite boxes reject unsupported domains explicitly.<br>**Limits:** None within the declared algorithm/domain pairings. |
| **R004 - Mehrblattmodelle**<br>Ziel, Variablen und Constraints auf verschiedenen Blättern korrekt auswerten | Accepted, bounded | Host structural cell/name references across sheets and private original-Calc checks<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | Supported formulas only; cycles, missing sheets and unsupported references fail explicitly.<br>**Limits:** None for the tested supported cross-sheet formulas. |
| **R005 - Mehrere Modelle**<br>Mehrere benannte versionierte Modelle pro Workbook ohne Namenskollision | Accepted, bounded | Workbook named revisioned registry with collision checks and persistence<br>[HOST](#evidence-host), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Workbook quota 32; SDK portable-library quota 128; IDs/revisions are decimal strings.<br>**Limits:** None within the documented quotas and supported workbook format. |
| **R006 - Affine Extraktion**<br>SUM/SUMPRODUCT und unterstützte Ausdrücke strukturell analysieren | Accepted, bounded | Structural affine extraction including host SUM/SUMPRODUCT<br>[COMPILE](#evidence-compile), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Supported expression grammar is explicit; cached samples never prove affinity.<br>**Limits:** None for the advertised structural grammar. |
| **R007 - QP Extraktion**<br>Quadratische Terme und Sparse-Struktur exakt dem Modell zuordnen | Accepted, bounded | Structural quadratic/cross-term extraction with sparse model handles<br>[COMPILE](#evidence-compile), [QP](#evidence-qp), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | Each coefficient multiplies x_i*x_j with no hidden half factor; unrepresentable raw duplicate aggregates are refused.<br>**Limits:** None within the supported grammar/numerical range. |
| **R008 - Konvexität**<br>Nur nach festgelegten gültigen Regeln als konvex klassifizieren | Accepted, bounded | Conservative compositional and checked Hessian curvature recognition<br>[COMPILE](#evidence-compile), [QP](#evidence-qp) | Native Rust 1.95 + only explicitly cited Chrome cases | Unproved PSD remains unsupported/unknown; sampling is not a convexity proof.<br>**Limits:** None under the stated sufficient recognition rules. |
| **R009 - Nichtlinearität**<br>Stichproben allein liefern keinen Linearitätsbeweis | Accepted, bounded | Structural nonlinear classification and adversarial cancellation/disguised nonlinear tests<br>[COMPILE](#evidence-compile) | Native Rust 1.95 + only explicitly cited Chrome cases | No finite probe set is used as a general linearity certificate.<br>**Limits:** None for the advertised expression classes. |
| **R010 - Automatische Differentiation**<br>Gradienten gegen analytische Referenzen prüfen | Accepted, bounded | Reverse-mode differentiation with independent multivariable analytic controls<br>[COMPILE](#evidence-compile), [DIFF](#evidence-diff) | Native Rust 1.95 + only explicitly cited Chrome cases | Smooth supported operators only; nonsmooth/domain branches are explicit.<br>**Limits:** None within the declared differentiable grammar. |
| **R011 - Differenzenrückfall**<br>Skalierte Schritte, Bounds und Domainfehler beachten | Accepted, bounded | Scaled finite differences with bound-aware probes and original-domain errors<br>[DIFF](#evidence-diff) | Native Rust 1.95 + only explicitly cited Chrome cases | All probes count toward evaluation budgets; requested derivative mode is honored.<br>**Limits:** None within the declared differentiable grammar/range. |
| **R012 - Sparse LP**<br>Bekannte LP-Lösungen, Unbeschränktheit und Unzulässigkeit korrekt unterscheiden | Accepted, bounded | Actual Clarabel sparse CSC LP path plus checked dense LP references<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [PERF](#evidence-perf) | Native Rust 1.95 + only explicitly cited Chrome cases | Known optimal/infeasible/unbounded controls are distinct; unchecked rays remain Unknown rather than a false proof.<br>**Limits:** No unrestricted large-sparse performance claim; this is a scale limit, not a missing truthfulness condition. |
| **R013 - MILP**<br>Kleine Referenzen erschöpfend validieren; Incumbent und Schranken getrennt | Accepted, bounded | LP branch-and-bound with exhaustive integer oracles, including the corrected near-integral LP node-closure probe<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [MIP](#evidence-mip) | Native Rust 1.95 + only explicitly cited Chrome cases | Binary x, y with max 1e12*x+500*y and 1e9*x+y<=1 has exact optimum 500 at [0, 1]. The old rounded-zero Optimal certificate is fixed; a material incumbent/relaxation gap keeps the node open even inside integer-distance tolerance.<br>**Limits:** Exact min/max, offset 1e16 and one-node controls pass; mixed-conic unresolved leaves retain BestFound/valid bounds instead of fabricated Optimal. No branch-and-cut or unrestricted MIP claim. |
| **R014 - Warm Start**<br>Nur verfügbare Backendfähigkeit anbieten und Ergebnisse erneut validieren | Accepted, bounded | Real QP primal/dual warm starts and checked original-coordinate LP vertex-basis reuse<br>[BASIS](#evidence-basis), [QP](#evidence-qp), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | Warm state is revalidated; Clarabel warm starts are not advertised.<br>**Limits:** Sparse factor/tableau reuse is unavailable and explicitly unadvertised. |
| **R015 - Konvexe QP**<br>Primal-/Dualresiduen und KKT-Bedingungen prüfen | Accepted, bounded | General constrained continuous convex QP with primal/dual/KKT checks<br>[QP](#evidence-qp), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | Dense 512 cap and sufficient curvature proof; valid original-unit bound required for Optimal.<br>**Limits:** No universal PSD recognition or integer-QP claim from this continuous path. |
| **R016 - SOCP**<br>Bekannte Kegelreferenzen bestehen; keine automatische Integerfähigkeit | Accepted, bounded | Exact Lorentz cones with original primal/dual verification<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [ROBUST](#evidence-robust) | Native Rust 1.95 + only explicitly cited Chrome cases | Continuous cone capability is separate from finite integer-conic search.<br>**Limits:** No arbitrary unsupported cones or unchecked-ray proof. |
| **R017 - Glatte NLP**<br>Feasibility und Stationarität separat berichten; lokale Reichweite kenntlich | Accepted, bounded | Bound-only projected gradient and constrained augmented-Lagrangian AD/FD<br>[NLP](#evidence-nlp), [DIFF](#evidence-diff) | Native Rust 1.95 + only explicitly cited Chrome cases | Feasible/Stationary/stop reason separate; local method, no global or infeasibility proof.<br>**Limits:** No general nonsmooth/global NLP guarantee. |
| **R018 - Methodenname**<br>Augmented-Lagrangian-Verfahren nicht als implementiertes GRG ausgeben | Accepted, bounded | Explicit projected-gradient/augmented-Lagrangian names and method-specific refusal<br>[NLP](#evidence-nlp), [HOST](#evidence-host), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | No substitution labeled GRG; analytic QP progress no longer falsely says simplex.<br>**Limits:** None for current Solvercraft method labels. |
| **R019 - Nichtglatte Suche**<br>Bounds, integer repair und Originalmodellprüfung bestehen | Accepted, bounded | Finite-bound differential evolution with integer repair and original-model checks<br>[HEURISTIC](#evidence-heuristic), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Best feasible point found only; no infeasibility/global-optimality certificate.<br>**Limits:** No unrestricted finite-free-domain heuristic claim. |
| **R020 - Mehrfachstart**<br>Seed und Suchbudget dokumentieren; keine globale Garantie | Accepted, bounded | Seeded multistart with configured starts and shared cumulative budgets<br>[HEURISTIC](#evidence-heuristic), [NLP](#evidence-nlp) | Native Rust 1.95 + only explicitly cited Chrome cases | Multiple starts require finite boxes; no global guarantee.<br>**Limits:** None within its bounded local-search contract. |
| **R021 - Enginefähigkeiten**<br>Registry nennt Klassen, Dual-/Basisdaten, Cancel, WASM und Garantien | Accepted, bounded | Typed per-engine registry with classes, duals/bases/cancel/progress/WASM/limits<br>[API](#evidence-api), [BROWSER](#evidence-browser) | Native Rust 1.95 + only explicitly cited Chrome cases | Source compatibility and actual browser verification are distinct flags; unsupported selections fail.<br>**Limits:** Browser evidence applies to listed tested paths, not every engine/model. |
| **R022 - Jobstart**<br>Solve liefert Job-ID und Snapshotrevision ohne UI zu blockieren | Accepted, bounded | Native/browser jobs return lossless job ID and revision; omitted sheets are pinned at admission<br>[HOST](#evidence-host), [WORKER](#evidence-worker) | Native Rust 1.95 + only explicitly cited Chrome cases | Admission returns job ID/revision and pins the original snapshot; current measured admission 0.765 ms. Native parameter/barrier controls pass; no automatic apply or UI dialog.<br>**Limits:** Bounded supported native/browser jobs; arbitrary caller code is not preemptible. |
| **R023 - Fortschritt**<br>Statuszugriff während Lauf; Ziel spätestens 250 ms bei normalen Modellen | Accepted, bounded | Actual final7 product-worker progress and status/admission, with all declared attempts retained<br>[WORKER](#evidence-worker), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Metrics for attempt 0 (initial): admission 0.765 ms, status 1 ms, active progress gap 58 ms, heartbeat 12 ms. Observed status meets the original normal-model 250 ms target.<br>**Limits:** Current declared attempts: initial: 9 passed/0 failed/0 ignored, exit 0; aggregate_all_attempts_passed=true. Historical final6 timing failure/cause uncertainty remains preserved. No hard realtime/25ms product or arbitrary-callback guarantee. |
| **R024 - Abbruch**<br>Cancel und Auswertungs-/Knoten-/Zeitlimits liefern korrekten Teilstatus | Accepted, bounded | Evaluation/iteration/node budgets, cooperative native deadlines/cancel and hard worker termination<br>[API](#evidence-api), [ENGINES](#evidence-engines), [WORKER](#evidence-worker) | Native Rust 1.95 + only explicitly cited Chrome cases | Partial incumbent only when verified/received; hard worker termination has Unknown feasibility if none received.<br>**Limits:** No hard realtime guarantee for uncooperative native callbacks/factorization. |
| **R025 - Übernahme**<br>Ein atomarer undo-fähiger Enginebefehl mit Revisionstest | Accepted, bounded | One host apply transaction with document UID/revision validation and one-step undo<br>[HOST](#evidence-host), [UI](#evidence-ui), [WORKER](#evidence-worker) | Native Rust 1.95 + only explicitly cited Chrome cases | Preview never writes automatically; stale/foreign results cannot apply.<br>**Limits:** None for tested supported cell writes. |
| **R026 - Konfliktschutz**<br>Workbookänderung, Schutz und Spills können Übernahme verhindern | Accepted, bounded | Revision/document/protection/spill/formula decision guards<br>[HOST](#evidence-host), [UI](#evidence-ui), [WORKER](#evidence-worker) | Native Rust 1.95 + only explicitly cited Chrome cases | Invalid apply is refused atomically before live writes.<br>**Limits:** None for the documented host conflict guards. |
| **R027 - Originalmodellprüfung**<br>Jede Lösung samt gerundeten Integern gegen Originalrestriktionen prüfen | Accepted, bounded | Original compiled expressions and private workbook formulas checked before preview/apply; rounded integer conic incumbents rechecked<br>[HOST](#evidence-host), [CERT](#evidence-cert), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Executed native regression accepts valid rounded values and rejects original-row violations after rounding.<br>**Limits:** No feasible label is permitted from an unchecked repaired vector; only declared model classes/numeric ranges are accepted. |
| **R028 - Ergebnisstatus**<br>Abbruchgrund, Zulässigkeit und Optimalität nicht vermischen | Accepted, bounded | Separate termination, feasibility and optimality enums across SDK/host/API<br>[MODEL](#evidence-model), [API](#evidence-api), [ENGINES](#evidence-engines) | Native Rust 1.95 + only explicitly cited Chrome cases | Cancelled/TimeLimit/Unknown/BestFound/Stationary are not conflated.<br>**Limits:** None in the tested result contracts. |
| **R029 - Gap**<br>Nur bei zulässigem Incumbent und gültiger Schranke, Definition dokumentiert | Accepted, bounded | Gap only with checked incumbent and valid original-unit bound; near-integral rounding no longer closes an unfinished node<br>[CERT](#evidence-cert), [QP](#evidence-qp), [RISK](#evidence-risk), [MIP](#evidence-mip) | Native Rust 1.95 + only explicitly cited Chrome cases | Exact constant-free comparison prevents a large additive objective offset from hiding the 500-unit binary improvement. A one-node stop retains incumbent 0 and an open bound/nonzero gap; the conic probe can retain BestFound at 500 with bound 500.0000009476 and one unresolved leaf.<br>**Limits:** The specific conic numeric bound is one executed probe, not a promised exact iteration result. Unknown/unresolved leaves prevent Optimal; no global bound from local/heuristic paths. |
| **R030 - Ribbonposition**<br>Solvercraft unmittelbar rechts neben bestehendem Solver | Accepted, bounded | Actual Data ribbon places Solvercraft directly right of existing Solver<br>[UI](#evidence-ui) | Native Rust 1.95 + only explicitly cited Chrome cases | Executed real egui input test and actual Trunk screenshot.<br>**Limits:** None for the tested ribbon layout. |
| **R031 - Aufgabenpanel**<br>Ziel/Variablen/Constraints/Methode und Ergebnisvorschau bedienbar | Accepted, bounded | Task pane edits goal/variables/constraints/method, starts/polls and previews/applies<br>[UI](#evidence-ui), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Actual user interaction tested; supported fields/classes only.<br>**Limits:** Advanced SDK risk/decision modules are not implied to have complete custom workbook dialogs. |
| **R032 - Zugänglichkeit**<br>Fokus, Tastatur, Themes und deutsche/englische Beschriftung prüfen | Accepted, bounded | Five actual native UI tests include EN/DE x light/dark, focus, Tab/Shift-Tab, draft typing and Ctrl-Enter preview<br>[UI](#evidence-ui) | Native Rust 1.95 + only explicitly cited Chrome cases | Principal accessible names and actual dark German browser pane; input changes only draft until explicit apply.<br>**Limits:** No complete screen-reader or arbitrary-platform conformance certification is inferred from the executed matrix. |
| **R033 - Programmatische API**<br>CLI/MCP-Aufruf öffnet keinen Dialog; leere Parameter geben Daten oder Fehler | Accepted, bounded | Typed CLI/SDK/headless command boundaries return data or structured errors with no dialog<br>[API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Real processes test unknown/empty/invalid requests; no automatic result apply.<br>**Limits:** No unimplemented MCP service is inferred from the CLI. |
| **R034 - Antwortbericht**<br>Ziel, Variablen, Constraints, Toleranzen und Status nachvollziehbar | Accepted, bounded | Versioned host answer report contains source expressions, values/bounds, row activity/slack, tolerances/status<br>[HOST](#evidence-host), [UI](#evidence-ui) | Native Rust 1.95 + only explicitly cited Chrome cases | Requires matching workbook revision; no invented duals in plain solve results.<br>**Limits:** None for the supported answer-report fields. |
| **R035 - Dualwerte**<br>Nur valide vorhandene Backenddaten ausgeben | Accepted, bounded | Original-unit QP/conic duals and separately checked LP-basis shadow prices<br>[QP](#evidence-qp), [BASIS](#evidence-basis), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Only actual validated data; unavailable reports are explicit errors/absent fields.<br>**Limits:** No complete sparse-simplex dual/basis suite is advertised. |
| **R036 - Sensitivität**<br>LP-Basisintervalle, lokale Analyse und Parameterläufe fachlich trennen | Accepted, bounded | Checked LP basis intervals are distinct from local NLP or simulation parameter effects<br>[BASIS](#evidence-basis), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | One-at-a-time local allowable changes; simultaneous changes require revalidation.<br>**Limits:** 64-variable/512-row/combination caps are explicit; no full commercial sensitivity suite. |
| **R037 - JSON Export**<br>Versionierte portable Modelle mit verlustfreien IDs/Seeds | Accepted, bounded | Versioned SDK portable JSON and workbook model import/export with string IDs/seeds<br>[API](#evidence-api), [HOST](#evidence-host), [MANIFEST](#evidence-manifest) | Native Rust 1.95 + only explicitly cited Chrome cases | Binary 64/max-u64 fidelity and strict schemas tested.<br>**Limits:** None within supported schemas/byte quotas. |
| **R038 - XLSX Persistenz**<br>Eigene Parts und Beziehungen in Gridcraft vollständig roundtrippen | Accepted, bounded | Real owned OPC part and relationship survive repeated XLSX/Strict roundtrips<br>[HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Owned xl/solvercraft namespace; unsupported workbook semantics remain explicit.<br>**Limits:** No claim of full Excel/legacy PSI file compatibility. |
| **R039 - Strukturmigration**<br>Zell-/Blatt-/Tabellenänderungen migrieren Bindungen oder melden Fehler | Accepted, bounded | Anchored formula bindings use structural migration or explicit invalid-reference errors<br>[HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Row/sheet edits and deleted references tested; unsupported table/dynamic formulas reject.<br>**Limits:** No silent migration of unsupported reference classes. |
| **R040 - Legacy Solver**<br>solver_*-Modelle importieren; nicht darstellbaren Exportumfang melden | Accepted, bounded | Legacy solver_* import plus same-sheet typed-linear export preview/apply/undo<br>[HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Nonrepresentable features/options reported; existing legacy names are preserved on import.<br>**Limits:** No lossless export of arbitrary nonlinear/advanced Solvercraft models. |
| **R041 - Snapshotsemantik**<br>Trialvergleich; Live-Workbook und Historie bleiben während Probeauswertung unverändert | Accepted, bounded | Private workbook/Calc probes preserve live cells/history and pinned reference context<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | Deterministic native threaded-barrier regression verifies that changing shared active-tab view cannot retarget queued omitted-sheet or nested parameter requests.<br>**Limits:** Final7 rebuilt browser proof is accepted; unsupported dynamic formulas remain explicit. |
| **R042 - Zeit und Zufall**<br>NOW/TODAY fixieren; RAND-Semantik beim deterministischen Solve explizit | Accepted, bounded | Captured NOW/TODAY timestamp and explicit RAND/RANDBETWEEN deterministic-solve refusal<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | 1900/1904 date utility semantics; simulation uses declared addressed SC streams.<br>**Limits:** None within the supported clock/random policy. |
| **R043 - Dynamische Formeln**<br>INDIRECT/OFFSET/LAMBDA/Arrays über Fähigkeitenliste prüfen | Accepted, bounded | Explicit supported formula grammar and rejection of unsupported dynamic forms<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | INDIRECT/OFFSET/LAMBDA/unsupported arrays are not sampled and called linear.<br>**Limits:** These functions are unsupported capability entries, not silently emulated. |
| **R044 - Formelfehler**<br>Domain-/Missing-/Formelfehler nicht still als Null behandeln | Accepted, bounded | Domain/missing/reference/formula errors propagate explicitly through host/SDK<br>[WORKSHEET](#evidence-worksheet), [COMPILE](#evidence-compile), [SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | Invalid trials require an explicit policy; missing cells do not silently become numerical zero.<br>**Limits:** None for the tested error contracts. |
| **R045 - Verteilungsbasis**<br>Alle 17 benannten R2-Verteilungen mit Parameter- und Randfalltests | Accepted, bounded | All 17 original R2 distribution families implemented with host declarations<br>[MATH](#evidence-math), [WORKSHEET](#evidence-worksheet), [ORACLE](#evidence-oracle) | Native Rust 1.95 + only explicitly cited Chrome cases | Parameter/support/edge semantics explicit; empirical discrete/continuous are distinct.<br>**Limits:** No extra-catalog parity is inferred from the 17-family baseline. |
| **R046 - Verteilungskatalog**<br>Jede weitere eingefrorene Referenzfunktion einzeln testen | Accepted, bounded | All 111 frozen distribution/Alt/custom/special/time-series mathematical capabilities have individually associated positive tests in final7 SDK 630<br>[REF](#evidence-ref), [MATH](#evidence-math), [ORACLE](#evidence-oracle), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | SC-14 and masterplan 10.1 require mathematical capabilities and individually checked cores; all 111 names map to 73 positive SDK test bodies across 13 files. The 28Alt mathematical constructions are tested, including explicit 3-claim Pearson 6 and 2-claim Weibull alternatives.<br>**Limits:** No missing positive mathematical-core case in the accepted bounded catalog. Literal Pearson 6Alt/WeibullAlt vendor arity/defaults, proprietary catalogs and full worksheet/property parity remain explicit separate compatibility gaps; no 111-vendor-contract or full-domain equivalence claim. |
| **R047 - Inverse CDF**<br>Quantile, Support und Tailverhalten gegen Referenzen prüfen | Accepted, bounded | Prepared CDF/quantile/density with independent SciPy parameter-grid and tail controls<br>[MATH](#evidence-math), [ORACLE](#evidence-oracle) | Native Rust 1.95 + only explicitly cited Chrome cases | 4071 finite comparisons; one warned reference excluded; stated numerical ranges, no universal relative-tail accuracy claim.<br>**Limits:** No all-parameter/all-tail mathematical error certificate. |
| **R048 - Verteilungsmodifikatoren**<br>Trunkierung, Shift, Scale, Mixture und Compound separat spezifizieren | Accepted, bounded | Separate truncation/shift/scale/mixture/compound contracts and tests<br>[MATH](#evidence-math), [REF](#evidence-ref) | Native Rust 1.95 + only explicitly cited Chrome cases | Atoms/reflections and bounded preparation explicit; generic compound CDF/PDF unavailable rather than fabricated.<br>**Limits:** Unsupported generic compound queries are explicit capability limits. |
| **R049 - RNG Streams**<br>Gleiche Samples unabhängig von Workerzahl und Auswertungsreihenfolge | Accepted, bounded | Versioned addressed RNG independent of trial/binding evaluation order and scheduling<br>[MATH](#evidence-math), [SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | Legacy goldens and immutable addressed identities retained; no mutable thread-order stream.<br>**Limits:** Only fixed version/seed/binding/trial contracts promise reproducibility. |
| **R050 - Monte Carlo**<br>IID-Sampling und Seedmanifest reproduzierbar | Accepted, bounded | IID Monte Carlo and lossless seed manifests with repeatable addressed draws<br>[SIM](#evidence-sim), [API](#evidence-api), [MANIFEST](#evidence-manifest) | Native Rust 1.95 + only explicitly cited Chrome cases | Native host/server and real CLI simulation manifests disclose seed, sampler, effective configuration and diagnostic or real host snapshot identity.<br>**Limits:** Final rebuilt browser/release evidence remains separate under R088/R093. |
| **R051 - Latin Hypercube**<br>Strata, Dimensionen, Replikate und Abhängigkeiten korrekt | Accepted, bounded | LHS strata/dimensions with explicit dependence transformation and independent-seed replication rules<br>[SIM](#evidence-sim), [INTERVAL](#evidence-interval) | Native Rust 1.95 + only explicitly cited Chrome cases | Repeated seed is refused as an independent LHS replicate even if metadata labels differ.<br>**Limits:** No IID single-run interval is inferred from LHS samples. |
| **R052 - Randomized Sobol**<br>Richtungstabellen lizenziert; Unsicherheit über unabhängige Scrambles | Accepted, bounded | Randomized 64-dimensional Sobol-v2 with licensed Joe-Kuo directions and independent replicate intervals<br>[SOBOL](#evidence-sobol), [INTERVAL](#evidence-interval) | Native Rust 1.95 + only explicitly cited Chrome cases | 16384 independent SciPy point anchors; separate independent randomization identities.<br>**Limits:** No unrestricted dimensions or single-scramble IID error bar. |
| **R053 - Rangkorrelation**<br>Rang- vs Pearson-Korrelation klar; ungültige Matrizen ablehnen | Accepted, bounded | Explicit latent-Gaussian Pearson correlation vs continuous Gaussian-copula Spearman conversion and PSD checks<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native Rust 1.95 + only explicitly cited Chrome cases | The matrix is not generally Pearson correlation of transformed nonnormal outputs; discrete ties need not reproduce target Spearman exactly. Invalid matrices reject; singular PSD/zero variance tested, no silent repair.<br>**Limits:** Parameter/dimension caps remain explicit. |
| **R054 - Gaussian Copula**<br>Marginalverteilungen und spezifizierte Abhängigkeit validieren | Accepted, bounded | Gaussian copula margins/quadrants/dependence checked independently<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native Rust 1.95 + only explicitly cited Chrome cases | Supported continuous/discrete marginal behavior and singular handling are explicit.<br>**Limits:** No universal empirical correlation exactness for finite samples. |
| **R055 - Weitere Copulas**<br>Student-t/Clayton/Frank/Gumbel mit erlaubten Parametern und Dimensionen | Accepted, bounded | Student-t/Clayton/Frank/Gumbel including permitted negative/reflected cases<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native Rust 1.95 + only explicitly cited Chrome cases | Documented bivariate/multivariate theta and dimension ranges; analytic CDF/tau/margins controls.<br>**Limits:** No unrestricted Archimedean parameter/dimension support. |
| **R056 - Online Statistik**<br>Momente und Fehlertrials korrekt; Speicher nicht proportional zu Vollmatrix | Accepted, bounded | Compensated online moments, valid/invalid counts and explicit Fail/Skip policies<br>[SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | Streaming storage is not the trials-by-input-by-output full matrix; Skip warns of selection bias.<br>**Limits:** Retained exact statistics still require bounded retained rows. |
| **R057 - Quantile**<br>Exakt oder klar markierter Sketch mit definierter Semantik | Accepted, bounded | Exact retained empirical lower-inverse quantiles, clear absence in streaming-only mode<br>[MATH](#evidence-math), [SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | No unlabeled sketch/external-sort claim; retained memory checked.<br>**Limits:** No exact quantiles from unretained arbitrary streams. |
| **R058 - VaR und CVaR**<br>Verlustkonvention und diskrete Quantilsmasse korrekt | Accepted, bounded | Upper-loss VaR/CVaR with fractional mass at discrete quantile boundary<br>[RISK](#evidence-risk), [MATH](#evidence-math), [SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | Unequal raw weights, permutations, alpha0 and tiny/extreme weights independently tested.<br>**Limits:** Unsupported conditioning/range returns errors. |
| **R059 - Verteilungsfitting**<br>Schätzung, AIC/BIC und geeignete Güteprüfung mit Parameterfit beachten | Accepted, bounded | 15 automatic estimators plus bounded local likelihood fitting, AIC/BIC and refitted bootstrap diagnostics<br>[FIT](#evidence-fit) | Native Rust 1.95 + only explicitly cited Chrome cases | Mixed PMF/PDF rankings refused; parameter-fit-aware GOF; generic fitting is local, not certified global MLE.<br>**Limits:** Weighted/censored fitting unavailable and explicitly outside this supported fit contract. |
| **R060 - Unsicherheitsintervalle**<br>Samplingannahmen und Stoppregeln passen zum Verfahren | Accepted, bounded | Procedure-specific fixed-sample/replicate intervals and bounded MC checkpoint stopping<br>[INTERVAL](#evidence-interval), [SIM](#evidence-sim) | Native Rust 1.95 + only explicitly cited Chrome cases | Gaussian/finiteness/independent-replicate assumptions explicit; large-df t critical value approximate; off-checkpoint adaptive cancellation has no fabricated CI.<br>**Limits:** No distribution-free coverage for arbitrary data-dependent stopping or Skip policy. |
| **R061 - SC Formeln**<br>Stabile Input-/Outputbindungen; definierte Semantik außerhalb Simulation | Accepted, bounded | Stable discovered/explicit SC input/output bindings with deterministic outside-simulation nominal policy<br>[WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | Means where available, labelled fallback for undefined means; not PSI display/default parity.<br>**Limits:** No full PSI property/outside-simulation equivalence. |
| **R062 - Simulation Charts**<br>Histogramm/CDF/Scatter/Box/Tornado/Quantilband mit korrekten Daten | Accepted, bounded | Final7 native UI 6 passed; renderer numerical contracts 8 and retained-result lifecycle 1 passed<br>[UI](#evidence-ui), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | All five sample chart choices and parameter Tornado are exercised; tiny/large/subnormal/independent-axis data and retained quantiles are checked. The chart total 9 is not a count of numerical oracles alone.<br>**Limits:** Only the declared data/display contracts and executed UI/browser scope are accepted; no complete PSI or arbitrary platform chart parity. |
| **R063 - Parameterläufe**<br>Werte pro Simulation konstant und Ergebnisse eindeutig vergleichbar | Accepted, bounded | SDK parameter grid and host constant-parameter cases with CRN/distinct streams<br>[SIM](#evidence-sim), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | Parameters constant per run; run coordinates/seeds disclosed; combined trial/memory caps.<br>**Limits:** No automatic causal interpretation of parameter effects. |
| **R064 - Trialkonsistenz**<br>Mehrere Reads derselben Zufallszelle liefern denselben Trialwert | Accepted, bounded | Each uncertain cell sampled once per trial; ordered simulation IR preserves repeated reads and Calc arithmetic<br>[WORKSHEET](#evidence-worksheet) | Native Rust 1.95 + only explicitly cited Chrome cases | Six executed native controls include 29 formulas x100 addressed vectors with exact-bit Calc comparison, error/coercion/names/clock/history/cancel cases.<br>**Limits:** The final7 product-worker differential check is accepted; no arbitrary unsupported formula semantics are inferred. |
| **R065 - SAA**<br>Trainingsszenarien für Kandidatenvergleich fixieren | Accepted, bounded | One frozen materialized SAA training set for all candidate comparisons<br>[SAA](#evidence-saa) | Native Rust 1.95 + only explicitly cited Chrome cases | Independent affine-loss marginals/deterministic feasible set; candidate order does not redraw.<br>**Limits:** No arbitrary endogenous or correlated uncertain-program compiler claim. |
| **R066 - Unabhängige Validierung**<br>Eigenes Seedset und getrennte Unsicherheit für finale Lösung | Accepted, bounded | Distinct-seed final-decision holdout with separate mean/variance/SE/labelled interval<br>[SAA](#evidence-saa) | Native Rust 1.95 + only explicitly cited Chrome cases | The holdout evaluates a fixed selected decision; no population-optimality/CVaR-interval certificate.<br>**Limits:** No adaptive reuse of the holdout as training. |
| **R067 - Risikoziele**<br>Erwartungswert, Varianzstrafe und CVaR mit Originalmodellprüfung | Accepted, bounded | Expected loss, nonnegative mean-variance penalty and upper-tail CVaR with original-risk checks<br>[RISK](#evidence-risk), [SAA](#evidence-saa) | Native Rust 1.95 + only explicitly cited Chrome cases | Final coefficient-assembly allowance over finite boxes adjusts bounds/status; preceding moment checks are conditioning guards, not full interval proofs.<br>**Limits:** No unsupported unbounded coefficient-error/global statistical certificate. |
| **R068 - Chance Constraints**<br>Einzelne und gemeinsame Bedingungen; keine erfundene Wahrscheinlichkeit | Accepted, bounded | Individual and joint finite-distribution chance constraints with original violation-mass recheck<br>[LOGIC](#evidence-logic) | Native Rust 1.95 + only explicitly cited Chrome cases | Declared finite scenarios are the probability law; not a population confidence assertion.<br>**Limits:** No invented probability outside the finite model. |
| **R069 - Big M**<br>Nur aus gültigen Grenzen abgeleitet; bei fehlenden Bounds ablehnen | Accepted, bounded | Outward finite-domain-derived M with exact slack gate and missing-bound refusal<br>[LOGIC](#evidence-logic), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | Original small RHS stays separate from huge M; equalities use two gates; expansion preflight.<br>**Limits:** No unrestricted unbounded-domain Big M. |
| **R070 - Robuste Gegenstücke**<br>Box/Budget/Ellipsoid nur unter passenden Strukturannahmen | Accepted, bounded | Weighted-support box and budget counterparts with checked owned solves; checked ellipsoid Lorentz counterpart<br>[ROBUST](#evidence-robust), [RISK](#evidence-risk) | Native Rust 1.95 + only explicitly cited Chrome cases | Original support/objective is verified before retaining Feasible/bounds. Exact product ordering/fractional budgets and directed arithmetic prevent false support from auxiliary slack or formation loss.<br>**Limits:** Plain compilation alone is not original-risk tolerance certification. Twenty independent original-space LP vertex cases plus four tiny signed cases pass; no arbitrary robust nonlinear/uncertain-RHS claim. |
| **R071 - Recourse**<br>Zweistufiges lineares deterministisches Äquivalent korrekt | Accepted, bounded | Two-stage linear deterministic equivalent with common first stage and local recourse<br>[RECOURSE](#evidence-recourse) | Native Rust 1.95 + only explicitly cited Chrome cases | Finite scenarios; zero-weight scenarios retain feasibility; integer stages supported by equivalent route.<br>**Limits:** No arbitrary multistage/endogenous/nonlinear recourse. |
| **R072 - Nichtantizipativität**<br>Erste Entscheidungen über Szenarien identisch; kein Zukunftswissen | Accepted, bounded | Nonanticipativity structural through one shared first-stage variable block<br>[RECOURSE](#evidence-recourse) | Native Rust 1.95 + only explicitly cited Chrome cases | Independent newsvendor and two-resource/exhaustive controls prohibit scenario foresight.<br>**Limits:** No inferred arbitrary information structure. |
| **R073 - Benders**<br>Nur zulässige Teilproblemklassen und valide Cuts | Accepted, bounded | Actual multicut Benders with explicitly solved primal/dual recourse and valid checked cuts<br>[RECOURSE](#evidence-recourse) | Native Rust 1.95 + only explicitly cited Chrome cases | Complete continuous nonnegative recourse subclass, finite first-stage bounds and cumulative budgets.<br>**Limits:** Integer/incomplete recourse rejected; no general L-shaped/MINLP solver claim. |
| **R074 - Entscheidungsbäume**<br>Rückwärtsauswertung und Nutzen an analytischen Fällen prüfen | Accepted, bounded | Acyclic finite decision graph backward expected-utility evaluation<br>[LOGIC](#evidence-logic) | Native Rust 1.95 + only explicitly cited Chrome cases | Linear/CARA utility, edge costs, cycles/domains bounded and validated.<br>**Limits:** No arbitrary utility family or inferred spreadsheet tree. |
| **R075 - Value of Information**<br>Informationszeitpunkt und Kosten in Baumrechnung beachten | Accepted, bounded | Prior/post-signal/perfect-information action comparison with acquisition costs before utility<br>[LOGIC](#evidence-logic), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Consistent normalized law fixes phantom VOI; linear/CARA analytic free/cost/revealing controls.<br>**Limits:** No unsupported information timing or arbitrary dynamic information model. |
| **R076 - Mehrziel**<br>Gewichtete, lexikografische und epsilon-Constraint-Verfahren nachvollziehbar | Accepted, bounded | Weighted, lexicographic and epsilon-constraint linear-goal methods with trace<br>[LOGIC](#evidence-logic) | Native Rust 1.95 + only explicitly cited Chrome cases | Actual normalized scalarization weights and imposed priority bounds disclosed; sequential budgets.<br>**Limits:** No complete nonconvex Pareto-front enumeration. |
| **R077 - SOS und Indikatoren**<br>Gültige Reformulierungen und Bounds mit kleinen Referenzen prüfen | Accepted, bounded | SOS1/SOS2 adjacency and indicator slack-gate formulations with finite bounds<br>[LOGIC](#evidence-logic), [CERT](#evidence-cert) | Native Rust 1.95 + only explicitly cited Chrome cases | Original logical semantic validation before returning incumbents.<br>**Limits:** No arbitrary native commercial indicator/cut engine. |
| **R078 - Finite Domains**<br>AllDifferent/Reifikation nur als tatsächlich freigegebene Fähigkeit | Accepted, bounded | Finite one-hot domains, AllDifferent and explicit separated reification<br>[LOGIC](#evidence-logic) | Native Rust 1.95 + only explicitly cited Chrome cases | Strict complements require a caller-supplied positive gap; unsupported equality disjunction explicit.<br>**Limits:** No invisible continuous strict-complement emulation. |
| **R079 - Globalverfahren**<br>Keine globale Schranke aus lokaler NLP-Lösung | Accepted, bounded | Polynomial finite-box interval branch-and-bound and replayable witnesses separate from NLP<br>[GLOBAL](#evidence-global) | Native Rust 1.95 + only explicitly cited Chrome cases | Local stationary points never produce a global bound; unsupported global classes explicitly rejected.<br>**Limits:** No general constrained/nonpolynomial global optimizer. |
| **R080 - Mixed Integer Conic**<br>MIQP/MISOCP nur nach eigenständiger Backend-/Relaxierungsprüfung | Accepted, bounded | Finite integer MIQP/MISOCP branch-and-bound using true convex conic relaxations<br>[QP](#evidence-qp), [CERT](#evidence-cert), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Exhaustive independent optima/bounds and interrupted frontier checks; unknown leaves remain unresolved.<br>**Limits:** No general unbounded-domain MICP, cutting planes or parallel MIP. |
| **R081 - PSI Import**<br>Jede unterstützte Signatur und Defaultsemantik separat geprüft | Accepted, bounded | 31 advertised top-level numeric PSI forms pass 40 native contract bodies, 37 Calc bodies, and distinct real-browser bodies: 19 signatures/2048 trials/19 outputs; 12 signatures/4096 trials/12 outputs/24 CDF anchors<br>[REF](#evidence-ref), [PSI](#evidence-psi), [WORKSHEET](#evidence-worksheet), [WORKER](#evidence-worker), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | Original SC-23 selects supported signatures; it does not require 272 imports. Native per-signature cases verify asymmetric parameter/CDF oracles, discovery, actual trial evaluation, previews and undo. Default solvercraft_nominal is explicitly lossy; preserve_vendor refuses context loss. Unsupported nested/property/extra calls are preserved whole. Labelled analytic_mean, median_fallback and explicit_override provenance is retained separately from the uncertain distribution; this is not vendor display-state equivalence.<br>**Limits:** No missing case within the advertised 31-form/policy contract. Full vendor display lifecycle, unadvertised defaults/properties/Alt calls and remaining worksheet names remain unsupported. Actual browser body scopes are separate: 19 signatures/2048 trials/19 outputs; 12 signatures/4096 trials/12 outputs/24 CDF anchors; no 31-signature/4096-trial combined body is inferred. |
| **R082 - Headless Server**<br>Optionaler Dienst mit Jobs, Authentisierung, Quoten und Cancel | Accepted, bounded | Explicit optional loopback service with per-connection auth/jobs/quotas/poll/cancel/forget<br>[API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | No listener without explicit bind/run; shutdown admission race fixed; bounded JSON and shared AST limits tested.<br>**Limits:** Remote TLS gateway and production deployment operations are separate host responsibilities. |
| **R083 - Rust SDK**<br>Eigenständiger ausführbarer Host ohne Workbook-/egui-Abhängigkeit | Accepted, bounded | Independently buildable Rust SDK/CLI without workbook or egui<br>[API](#evidence-api), [DEPS](#evidence-deps) | Native Rust 1.95 + only explicitly cited Chrome cases | Standalone core and native executable actually run.<br>**Limits:** No second GUI application implied. |
| **R084 - ArtCraft Reuse**<br>Zweiter Host nutzt gleiche Engine-/Modellverträge; echte Integration oder Beispiel klar benannt | Accepted, bounded | Executed standalone artcraft_layout example uses shared engines/portable model/bindings<br>[API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Clearly an example, not a shipped second ArtCraft application; optimum 140 at widths 20/100.<br>**Limits:** None for the original criterion's explicit example option. |
| **R085 - Referenzfreeze**<br>Produkt-/Versionsscope und Featureinventar nachvollziehbar | Accepted, bounded | Pinned full 570-page legacy PSI 2025Q1 scope, source hash and unfiltered 272-function inventory<br>[REF](#evidence-ref) | Native Rust 1.95 + only explicitly cited Chrome cases | 28 alternate forms, five unresolved mentions and source conflicts retained; later product revisions separate.<br>**Limits:** No implementation parity inferred from inventory presence. |
| **R086 - Lizenzen**<br>Direkte/transitive Abhängigkeiten, Daten und Notices vor Übernahme prüfen | Accepted, bounded | Recorded dependency/license review with public Name/Version/SPDX projections and checked-in Rust/Sobol notices<br>[DEPS](#evidence-deps), [SOBOL](#evidence-sobol), [ORACLE](#evidence-oracle) | Native Rust 1.95 + only explicitly cited Chrome cases | Clarabel 0.11.1 (Apache-2.0) and licensed direction data remain recorded; SciPy/NumPy are test-only. Public inventory JSON lists package names, versions and SPDX, rather than complete local feature trees or NOTICE-hash metadata.<br>**Limits:** Complete local diagnostic metadata remain private. Checked-in notices are linked separately. This is not a vulnerability audit or a proprietary-engine license grant. |
| **R087 - Unabhängige Orakel**<br>Analytische/erschöpfende Referenzen unabhängig vom getesteten Backend | Accepted, bounded | Independent analytic/exhaustive/NumPy/SciPy/Fraction numerical references<br>[CERT](#evidence-cert), [ORACLE](#evidence-oracle), [PERF](#evidence-perf), [RECOURSE](#evidence-recourse) | Native Rust 1.95 + only explicitly cited Chrome cases | Oracles compare actual original objective/bound/semantics, not only backend statuses.<br>**Limits:** Finite fixtures and tolerance grids do not prove all inputs. |
| **R088 - Rust und WASM**<br>Standardfeaturebaum ohne native Pflichtengine; echter Browserlauf | Accepted, bounded | Pure-Rust numerical default closure: actual final7 Chrome SDK 13 and worker 9 unique bodies<br>[BROWSER](#evidence-browser), [WORKER](#evidence-worker), [DEPS](#evidence-deps), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | Source-bound builds and actual browser runtime are distinct checks. Current declared worker attempts: initial: 9 passed/0 failed/0 ignored, exit 0. Import runtime scopes: 19 signatures/2048 trials/19 outputs; 12 signatures/4096 trials/12 outputs/24 CDF anchors.<br>**Limits:** Only explicitly executed Chrome cases are runtime evidence; aggregate_all_attempts_passed=true. Historical final6 failure remains disclosed. No universal platform or hard timing guarantee. |
| **R089 - Ressourcen**<br>Vor Allokationen Größen, Nonzeros, Rekursion, Trials und Output begrenzen | Accepted, bounded | Final7 source-bound resource/error/admission gates include AST/nonzero/trial/output/transform checks before expansion<br>[API](#evidence-api), [CERT](#evidence-cert), [MODEL](#evidence-model), [SIM](#evidence-sim), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | AST/nonzero/trial/output/transform admission and original robust-support regressions pass within SDK 630/host 2904, including strict import range/apply types before scanning/editing. Quotas are checked accounting boundaries, not measured universal RSS ceilings.<br>**Limits:** Configured quotas are accounting/input/output bounds, not an OS-RSS quota or hard preemption guarantee; declared size/numeric limits remain. |
| **R090 - Keine Panics**<br>Result-Fehler und geprüfte Zugriffe; Abhängigkeiten auf Fehlerverhalten prüfen | Accepted, bounded | Validated indices/shapes, structured errors and dependency-adapter failure paths pass final7 negative/regression gates<br>[API](#evidence-api), [CERT](#evidence-cert), [DEPS](#evidence-deps), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | SDK 630/0/0 and host 2904/0/1 passed/failed/ignored. Structured negative/panic-containment paths are exercised. Historical final6 timing failure remains disclosed separately.<br>**Limits:** No universal no-panic theorem: abort/OOM/panic-abort and arbitrary nonreturning or panicking caller code remain outside recoverable direct-call guarantees. |
| **R091 - Erfüllungsmatrix**<br>Jede Referenzfunktion mit Test, Plattform und Garantie; Lücken offen | Accepted, bounded | 96 original criteria plus audited 272-function source/test/platform/guarantee matrix with executable exact-test runner<br>[REF](#evidence-ref) | Native Rust 1.95 + only explicitly cited Chrome cases | All 272 frozen reference records retain test/platform/guarantee/source evidence and visible full-vendor gaps. An inventory record or planned ID is not execution; original 96 criteria have their own scoped acceptance.<br>**Limits:** Full-reference compatibility gaps remain visible despite bounded acceptance of 111 math cores and 31 advertised imports. Matrix completeness is not 272-function implementation. |
| **R092 - Leistungsnachweis**<br>Hardware, Seeds, RAM, Laufzeit und Fehler mit Versionen dokumentieren | Accepted, bounded | 5 fresh release processes per recorded bounded workload, with hardware/seed/RAM/runtime/error metadata<br>[PERF](#evidence-perf), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | continuous_lp_100_variables_500_rows: median 194.541 ms, peak 11,681,792 bytes; streaming_mc_100000_trials_20_inputs_10_outputs: median 432.793 ms, peak 6,799,360 bytes. Individual errors, hardware, seeds and versions are retained in the actual linked performance report; no speedup/commercial comparison is inferred.<br>**Limits:** Shared-host/quieter follow-up measurements, not a demonstrated engine speedup or legacy PSI comparison. Proposed medium 10000-variable/50000-row/500000-nonzero performance and time-to-first-feasible remain unmeasured. |
| **R093 - CI Gates**<br>cargo xtask ci und passende fachliche Tests; keine erfundenen Resultate | Accepted, bounded | Final7 unchanged-source host CI 10 stages; SDK format/Clippy/tests, UI/chart and actual Chrome/worker gates are separately recorded<br>[GATES](#evidence-gates), [FINAL](#evidence-final) | Native Rust 1.95 + only explicitly cited Chrome cases | Host 2904/0/1 passed/failed/ignored in 89.4699 s; SDK 630/0/0. Worker: initial: 9 passed/0 failed/0 ignored, exit 0. Accepted source identity stayed unchanged.<br>**Limits:** Ignored tests remain ignored, not passed. Current aggregate_all_attempts_passed=true; historical final6 initial failure and unproven cause remain disclosed. No universal parity certificate. |
| **R094 - Dokumentationspflege**<br>Architektur, Roadmap, Gaps und relevante Parität nachführen | Accepted, bounded | Architecture/roadmap/implementation/QA/reference/gap documents track supported contracts and historical checkpoints<br>[REF](#evidence-ref), [GATES](#evidence-gates) | Native Rust 1.95 + only explicitly cited Chrome cases | Current final7 links, original-scope audit and exact target/conversion semantics are reconciled; historical final5/265-test checkpoints remain labelled.<br>**Limits:** Full vendor/import gaps stay explicit. The original CSV and all 768 fields are preserved; no historical repository-encoding repair is claimed. |
| **R095 - Manifest**<br>Snapshot, Engine, Toleranzen, Seed und Plattform pro Lauf dokumentieren | Accepted, bounded | Managed execute/host/server/CLI runs and admitted CLI numerical errors retain typed configuration/input/snapshot/engine/platform identity<br>[MANIFEST](#evidence-manifest), [API](#evidence-api) | Native Rust 1.95 + only explicitly cited Chrome cases | Actual process tests verify one error JSON, empty stdout, original code/message, repeated/changed fingerprints, configuration/platform and honest unknown engine without a typed result. Historical pre-final6 scoped CLI package: 66 tests, Clippy and formatting passed after the integer-rounding correction.<br>**Limits:** Raw decode failures stay bare because no typed run was admitted; low-level numerical APIs are wrapped by managed run entrypoints. Final7 browser/source gates are recorded; arbitrary external embeddings must use their managed run contract. |
| **R096 - Schema Migration**<br>Versionierte Verträge und validierte Migrationspfade | Accepted, bounded | Versioned strict portable schemas and explicit validated algebraic-v1 migration<br>[API](#evidence-api), [HOST](#evidence-host) | Native Rust 1.95 + only explicitly cited Chrome cases | No guessed IDs/bindings/results; unknown schemas rejected; migration preserves model semantics/options.<br>**Limits:** Only declared migrations are supported; no invented arbitrary future-schema conversion. |

## Evidence index

### Evidence MODEL

- [crates/types/src/lib.rs](../../crates/types/src/lib.rs)
- [crates/model/src/lib.rs](../../crates/model/src/lib.rs)
- [crates/model/README.md](../../crates/model/README.md)

### Evidence COMPILE

- [crates/compile/tests/deep_review.rs](../../crates/compile/tests/deep_review.rs)
- [crates/compile/src/tests.rs](../../crates/compile/src/tests.rs)
- [docs/qa/compiler-review.md](../../docs/qa/compiler-review.md)

### Evidence DIFF

- [crates/compile/src/finite_difference.rs](../../crates/compile/src/finite_difference.rs)
- [crates/solvercraft/src/nonlinear_tests.rs](../../crates/solvercraft/src/nonlinear_tests.rs)
- [crates/solvercraft/tests/workflow_review.rs](../../crates/solvercraft/tests/workflow_review.rs)

### Evidence ENGINES

- [crates/engines/tests/deep_review.rs](../../crates/engines/tests/deep_review.rs)
- [crates/engines/tests/conic.rs](../../crates/engines/tests/conic.rs)
- [docs/implementation/engines.md](../../docs/implementation/engines.md)

### Evidence CERT

- [crates/engines/tests/certificate_review.rs](../../crates/engines/tests/certificate_review.rs)
- [crates/engines/tests/exact_bridge.rs](../../crates/engines/tests/exact_bridge.rs)
- [crates/model/README.md](../../crates/model/README.md)

### Evidence QP

- [crates/engines/tests/convex_qp.rs](../../crates/engines/tests/convex_qp.rs)
- [crates/engines/tests/mixed_conic.rs](../../crates/engines/tests/mixed_conic.rs)
- [docs/implementation/engines.md](../../docs/implementation/engines.md)

### Evidence BASIS

- [crates/engines/tests/basis.rs](../../crates/engines/tests/basis.rs)
- [crates/engines/tests/certificate_review.rs](../../crates/engines/tests/certificate_review.rs)

### Evidence NLP

- [crates/solvercraft/src/constrained_tests.rs](../../crates/solvercraft/src/constrained_tests.rs)
- [crates/solvercraft/tests/nonlinear_workflows.rs](../../crates/solvercraft/tests/nonlinear_workflows.rs)
- [docs/implementation/nonlinear.md](../../docs/implementation/nonlinear.md)

### Evidence HEURISTIC

- [crates/solvercraft/src/heuristic_tests.rs](../../crates/solvercraft/src/heuristic_tests.rs)
- [crates/solvercraft/src/multistart.rs](../../crates/solvercraft/src/multistart.rs)
- [crates/solvercraft/tests/nonlinear_workflows.rs](../../crates/solvercraft/tests/nonlinear_workflows.rs)

### Evidence GLOBAL

- [crates/solvercraft/src/global_tests.rs](../../crates/solvercraft/src/global_tests.rs)
- [crates/solvercraft/tests/global_review.rs](../../crates/solvercraft/tests/global_review.rs)
- [crates/solvercraft/tests/nonlinear_workflows.rs](../../crates/solvercraft/tests/nonlinear_workflows.rs)

### Evidence HOST

- [docs/implementation/gridcraft.md](../../docs/implementation/gridcraft.md)
- [docs/implementation/host-checkpoint.json](../../docs/implementation/host-checkpoint.json)
- [../crates/engine/src/cmd/solvercraft/host_tests.rs](../../../crates/engine/src/cmd/solvercraft/host_tests.rs)
- [../crates/engine/tests/it/solvercraft_deep_review.rs](../../../crates/engine/tests/it/solvercraft_deep_review.rs)
- [docs/implementation/final7-verification-summary.json](../../docs/implementation/final7-verification-summary.json)

### Evidence WORKSHEET

- [../crates/engine/src/cmd/solvercraft/worksheet_tests.rs](../../../crates/engine/src/cmd/solvercraft/worksheet_tests.rs)
- [../crates/engine/src/cmd/solvercraft/simulation_ir/tests.rs](../../../crates/engine/src/cmd/solvercraft/simulation_ir/tests.rs)
- [docs/implementation/gridcraft.md](../../docs/implementation/gridcraft.md)
- [../crates/engine/src/cmd/solvercraft/psi_import_tests.rs](../../../crates/engine/src/cmd/solvercraft/psi_import_tests.rs)
- [../crates/calc/src/eval/solvercraft/tests.rs](../../../crates/calc/src/eval/solvercraft/tests.rs)

### Evidence UI

- [../crates/ui-egui/tests/solvercraft_acceptance.rs](../../../crates/ui-egui/tests/solvercraft_acceptance.rs)
- [../crates/ui-egui/src/solvercraft.rs](../../../crates/ui-egui/src/solvercraft.rs)
- [../crates/ui-egui/src/solvercraft/report.rs](../../../crates/ui-egui/src/solvercraft/report.rs)

### Evidence WORKER


### Evidence API

- [crates/solvercraft/tests/api_review.rs](../../crates/solvercraft/tests/api_review.rs)
- [crates/cli/tests/api_review.rs](../../crates/cli/tests/api_review.rs)
- [crates/solvercraft/tests/contracts_e2e.rs](../../crates/solvercraft/tests/contracts_e2e.rs)
- [crates/solvercraft/tests/server_e2e.rs](../../crates/solvercraft/tests/server_e2e.rs)

### Evidence MATH

- [crates/math/tests/deep_review.rs](../../crates/math/tests/deep_review.rs)
- [crates/math/tests/numerical_review.rs](../../crates/math/tests/numerical_review.rs)
- [docs/implementation/simulation.md](../../docs/implementation/simulation.md)

### Evidence ORACLE

- [tests/e2e/distribution_oracle.py](../../tests/e2e/distribution_oracle.py)
- [docs/qa/distribution-oracle-report.json](../../docs/qa/distribution-oracle-report.json)

### Evidence SIM

- [crates/simulation/tests/advanced.rs](../../crates/simulation/tests/advanced.rs)
- [crates/simulation/tests/numerical_review.rs](../../crates/simulation/tests/numerical_review.rs)
- [docs/implementation/simulation.md](../../docs/implementation/simulation.md)

### Evidence SOBOL

- [crates/math/src/sobol.rs](../../crates/math/src/sobol.rs)
- [crates/math/tests/numerical_review.rs](../../crates/math/tests/numerical_review.rs)

### Evidence FIT

- [crates/simulation/tests/numerical_review.rs](../../crates/simulation/tests/numerical_review.rs)
- [crates/simulation/src/fitting.rs](../../crates/simulation/src/fitting.rs)
- [docs/implementation/simulation.md](../../docs/implementation/simulation.md)

### Evidence INTERVAL

- [crates/simulation/src/intervals.rs](../../crates/simulation/src/intervals.rs)
- [crates/simulation/tests/numerical_review.rs](../../crates/simulation/tests/numerical_review.rs)
- [docs/implementation/simulation.md](../../docs/implementation/simulation.md)

### Evidence SAA

- [crates/solvercraft/tests/saa_risk.rs](../../crates/solvercraft/tests/saa_risk.rs)
- [crates/solvercraft/src/saa.rs](../../crates/solvercraft/src/saa.rs)
- [docs/implementation/stochastic.md](../../docs/implementation/stochastic.md)

### Evidence RISK

- [crates/solvercraft/tests/saa_risk.rs](../../crates/solvercraft/tests/saa_risk.rs)
- [crates/solvercraft/tests/runtime_review.rs](../../crates/solvercraft/tests/runtime_review.rs)
- [crates/solvercraft/src/risk.rs](../../crates/solvercraft/src/risk.rs)

### Evidence LOGIC

- [crates/solvercraft/tests/advanced_models.rs](../../crates/solvercraft/tests/advanced_models.rs)
- [crates/engines/tests/discrete.rs](../../crates/engines/tests/discrete.rs)
- [docs/implementation/stochastic.md](../../docs/implementation/stochastic.md)

### Evidence ROBUST

- [crates/solvercraft/tests/ellipsoid.rs](../../crates/solvercraft/tests/ellipsoid.rs)
- [crates/solvercraft/tests/saa_risk.rs](../../crates/solvercraft/tests/saa_risk.rs)
- [crates/solvercraft/tests/runtime_review.rs](../../crates/solvercraft/tests/runtime_review.rs)
- [docs/implementation/stochastic.md](../../docs/implementation/stochastic.md)
- [docs/qa/ellipsoid-precision.md](../../docs/qa/ellipsoid-precision.md)
- [docs/implementation/robust-oracle-fixed-debug-report.json](../../docs/implementation/robust-oracle-fixed-debug-report.json)

### Evidence RECOURSE

- [crates/solvercraft/tests/stochastic.rs](../../crates/solvercraft/tests/stochastic.rs)
- [docs/implementation/stochastic.md](../../docs/implementation/stochastic.md)

### Evidence REF

- [docs/implementation/reference-freeze.md](../../docs/implementation/reference-freeze.md)
- [docs/implementation/reference-catalog.json](../../docs/implementation/reference-catalog.json)
- [docs/implementation/reference-implementation.json](../../docs/implementation/reference-implementation.json)
- [docs/implementation/reference-gaps.md](../../docs/implementation/reference-gaps.md)
- [tests/e2e/reference_matrix.py](../../tests/e2e/reference_matrix.py)
- [docs/qa/original-scope-audit.md](../../docs/qa/original-scope-audit.md)

### Evidence DEPS

- [docs/implementation/engines.md](../../docs/implementation/engines.md)
- [licenses/THIRD_PARTY.md](../../licenses/THIRD_PARTY.md)
- [crates/math/third_party/BOOST_LICENSE_1_0.txt](../../crates/math/third_party/BOOST_LICENSE_1_0.txt)

### Evidence BROWSER


### Evidence PERF

- [docs/implementation/performance-final7-report.json](../../docs/implementation/performance-final7-report.json)

### Evidence GATES

- [docs/implementation/verification-final7.md](../../docs/implementation/verification-final7.md)
- [docs/implementation/final7-verification-summary.json](../../docs/implementation/final7-verification-summary.json)
- [docs/implementation/public-evidence.md](../../docs/implementation/public-evidence.md)

### Evidence MANIFEST

- [crates/solvercraft/tests/workflow_review.rs](../../crates/solvercraft/tests/workflow_review.rs)
- [crates/solvercraft/tests/owned_control.rs](../../crates/solvercraft/tests/owned_control.rs)
- [crates/solvercraft/tests/run_manifest.rs](../../crates/solvercraft/tests/run_manifest.rs)
- [crates/cli/tests/manifests.rs](../../crates/cli/tests/manifests.rs)
- [crates/solvercraft/src/workflow.rs](../../crates/solvercraft/src/workflow.rs)
- [docs/implementation/run-manifests.md](../../docs/implementation/run-manifests.md)
- [docs/implementation/sdk-runtime.md](../../docs/implementation/sdk-runtime.md)
- [docs/implementation/verification-final7.md](../../docs/implementation/verification-final7.md)

### Evidence MIP

- [crates/engines/tests/integer_rounding.rs](../../crates/engines/tests/integer_rounding.rs)
- [crates/cli/tests/end_to_end.rs](../../crates/cli/tests/end_to_end.rs)
- [crates/cli/tests/manifests.rs](../../crates/cli/tests/manifests.rs)
- [docs/implementation/engines.md](../../docs/implementation/engines.md)

### Evidence FINAL

- [docs/implementation/final7-verification-summary.json](../../docs/implementation/final7-verification-summary.json)
- [docs/implementation/public-evidence.md](../../docs/implementation/public-evidence.md)

### Evidence PSI

- [../crates/engine/src/cmd/solvercraft/psi_import_tests.rs](../../../crates/engine/src/cmd/solvercraft/psi_import_tests.rs)
- [../crates/calc/src/eval/solvercraft/tests.rs](../../../crates/calc/src/eval/solvercraft/tests.rs)
- [tests/worker-browser/tests/worker.rs](../../tests/worker-browser/tests/worker.rs)
- [docs/implementation/gridcraft.md](../../docs/implementation/gridcraft.md)
- [docs/qa/original-scope-audit.md](../../docs/qa/original-scope-audit.md)

### Evidence HISTORY_FINAL6

- [docs/implementation/browser-smoke-final6-report.json](../../docs/implementation/browser-smoke-final6-report.json)
- [docs/implementation/browser-worker-final6-initial-report.json](../../docs/implementation/browser-worker-final6-initial-report.json)
- [docs/implementation/browser-worker-final6-report.json](../../docs/implementation/browser-worker-final6-report.json)
- [docs/implementation/browser-worker-final6-rerun-report.json](../../docs/implementation/browser-worker-final6-rerun-report.json)
- [docs/implementation/dependency-final6-inventory.json](../../docs/implementation/dependency-final6-inventory.json)
- [docs/implementation/exact-binary-oracle-final6-report.json](../../docs/implementation/exact-binary-oracle-final6-report.json)
- [docs/implementation/final6-verification-summary.json](../../docs/implementation/final6-verification-summary.json)
- [docs/implementation/frozen-final6-source-report.json](../../docs/implementation/frozen-final6-source-report.json)
- [docs/implementation/gates-final6/host/host-ci.txt](../../docs/implementation/gates-final6/host/host-ci.txt)
- [docs/implementation/gates-final6/host/report.json](../../docs/implementation/gates-final6/host/report.json)
- [docs/implementation/gates-final6/host/summary.json](../../docs/implementation/gates-final6/host/summary.json)
- [docs/implementation/gates-final6/sdk/report.json](../../docs/implementation/gates-final6/sdk/report.json)
- [docs/implementation/gates-final6/sdk/summary.json](../../docs/implementation/gates-final6/sdk/summary.json)
- [docs/implementation/linear-oracle-final6-report.json](../../docs/implementation/linear-oracle-final6-report.json)
- [docs/implementation/native-ui-controls-final6-report.json](../../docs/implementation/native-ui-controls-final6-report.json)
- [docs/implementation/performance-final6-report.json](../../docs/implementation/performance-final6-report.json)
- [docs/implementation/quadratic-oracle-final6-report.json](../../docs/implementation/quadratic-oracle-final6-report.json)
- [docs/implementation/release-final6-build-report.json](../../docs/implementation/release-final6-build-report.json)
- [docs/implementation/robust-oracle-final6-report.json](../../docs/implementation/robust-oracle-final6-report.json)
- [docs/implementation/trunk-final6-build-report.json](../../docs/implementation/trunk-final6-build-report.json)
- [docs/implementation/verification-final6.md](../../docs/implementation/verification-final6.md)
- [docs/implementation/worker-browser-final6-build-report.json](../../docs/implementation/worker-browser-final6-build-report.json)
