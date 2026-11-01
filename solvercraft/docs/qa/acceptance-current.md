# Current acceptance against the original 96 requirements

Review date: 2026-10-31. This assessment records the accepted **final5** source-bound gates. It preserves the original criteria; it is not a full Frontline-parity claim, an all-input correctness certificate or an assertion about untested platforms. The structured [acceptance-current.json](acceptance-current.json) retains all eight original CSV fields plus implementation, evidence, platform, guarantee and remaining work.

Original [requirements.csv](../masterplan/requirements.csv): **96 rows**, UTF-8 BOM, SHA-256 `699e87ae50a62ff5605b0d40dee38f4b2f8b7200a4c92f825d2972a57e24de55`. The repository copy is byte-identical to the planning artifact. Original `planned` statuses and `TEST-Rxxx` labels remain historical fields, not invented execution results.

**94 criteria are accepted within their declared bounded contracts; R046 and R081 remain partial.** This is criterion tracking, not a product-completeness percentage. R081 is a substantial PSI signature/property/default compatibility scope; the result does not mean only two individual vendor functions remain.

## Accepted final5 evidence

Primary entry: [final-verification-summary.json](../implementation/final-verification-summary.json). Host and SDK gates accepted an unchanged frozen source; 789 active/frozen code and compilation-input files match. No new commit identity is invented for the materialized checkout.

- Raw code/compilation-input SHA-256: `fcc9d93350d3c5d3342b02c775096867d01c0e7addfdc9099ac0160cb460ecfe`.
- Canonical SHA-256: `9188b27559e9f23fabe2faa8c353c2e5fdca1bc6ab113955375ccd46111e88bb`.
- Release CLI SHA-256: `68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`.
- Full host `cargo xtask ci`: all 10 stages accepted; **2827 passed, 0 failed, 1 pre-existing ignored**, 722.25 s. The ignored `recalc_with_range_formulas_scales_linearly` is visible and is not counted as passed.
- Nested SDK: **630 passed, 0 failed, 0 ignored**; formatting and all-target Clippy with warnings denied passed. Scoped engine89/CLI66 and other sub-suites are not added again to fabricate an aggregate.
- Actual native UI: **6 passed**. Chart module: **8 numerical renderer contracts + 1 retained-result lifecycle test**, not nine numerical oracles.
- Actual Chrome SDK: **13 passed**; actual rebuilt product worker: **7 passed**, including exact binary objective-500 variants. Observed maximum active progress gap **65 ms**, heartbeat **22 ms**, status query **1 ms**.
- Release independent oracles: **30 LP, 20 QP, 20 ordinary robust + 4 tiny signed cases, 3 exact binary variants + 1 node-limit control**, all passed on the recorded release binary.
- Performance: **5 fresh processes per workload**, with hardware, versions, seeds, RAM and numerical errors. LP median **260.2045 ms**, peak **11,759,616 bytes**, objective error **0**; streaming MC median **712.7748 ms**, peak **6,799,360 bytes**, maximum mean error **0.001469193929023671**. Shared-host concurrent native test activity is disclosed; no isolated speed comparison or medium-scale result is invented.

All final report links are in the evidence index below. Earlier final4, 265-test and other checkpoint results remain historical and are not relabelled as final5. The scalar SciPy grid remains its own finite source-hashed oracle: 4071 comparisons, zero failures and one excluded independently warned reference.

## Meaning of the statuses

- **Accepted, bounded**: executed evidence meets the original criterion within the explicitly supported mathematical/API contract. This does not certify arbitrary numeric magnitudes, every platform or proprietary behavior.
- **Partial**: a genuine portion of the original criterion remains missing or unresolved.
- **Verification pending** and **Confirmed failure** remain available classifications for future changes; neither has a current row in this assessment.

A rejecting guard is evidence of honest unsupported behavior, not implementation of a missing required feature. Source inventories and planned test IDs are not executed signature/default tests. SDK mathematical cores and exact worksheet compatibility remain separate.

## Remaining acceptance work

1. **R046:** resolve the source-backed Pearson6Alt and WeibullAlt argument/default conflicts without inventing parameters. The frozen 111-entry distribution/time-series scope has 109 unambiguous executable core subsets plus two tested own mathematical variants; that is not a claim that all 111 vendor contracts are complete.
2. **R081:** complete the wider PSI worksheet signature/default/property/outside-simulation semantics beyond the 19 verified core imports, with individual native/browser evidence. The full 272-function matrix truthfully retains open vendor-parity records.

## Requirement-by-requirement assessment

| Original ID / feature / acceptance | Status | Implementation and executed evidence | Platform | Guarantee and remaining acceptance work |
|---|---|---|---|---|
| **R001 — Zieldefinition**<br>Minimierung, Maximierung und Zielwert mit eindeutiger numerischer Semantik | Accepted, bounded | SDK workflow and host min/max/target goals with original objective reporting<br>[MANIFEST](#evidence-manifest), [HOST](#evidence-host), [API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Target adds equality requiring the original objective to equal the requested value; original units are preserved, and unreachable integer targets are infeasible.<br>**Remaining/limits:** None within the declared numeric range. |
| **R002 — Variablentypen**<br>Kontinuierlich, ganzzahlig und binär; ungültige Domains ablehnen | Accepted, bounded | Continuous/integer/binary domains, effective bounds and checked integer repair<br>[MODEL](#evidence-model), [ENGINES](#evidence-engines), [LOGIC](#evidence-logic) | Native SDK/process + explicitly cited browser subset | Invalid domains fail; finite mixed-conic integers stay in exactly distinguishable f64 range.<br>**Remaining/limits:** None within the supported domain contract. |
| **R003 — Bounds**<br>Freie, einseitige, beidseitige und fixierte Variablen; keine JSON-Infinity | Accepted, bounded | Optional finite lower/upper bounds, free and fixed coordinates, strict finite DTO validation<br>[MODEL](#evidence-model), [ENGINES](#evidence-engines), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | No JSON Infinity substitute; algorithms requiring finite boxes reject unsupported domains explicitly.<br>**Remaining/limits:** None within the declared algorithm/domain pairings. |
| **R004 — Mehrblattmodelle**<br>Ziel, Variablen und Constraints auf verschiedenen Blättern korrekt auswerten | Accepted, bounded | Host structural cell/name references across sheets and private original-Calc checks<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | Supported formulas only; cycles, missing sheets and unsupported references fail explicitly.<br>**Remaining/limits:** None for the tested supported cross-sheet formulas. |
| **R005 — Mehrere Modelle**<br>Mehrere benannte versionierte Modelle pro Workbook ohne Namenskollision | Accepted, bounded | Workbook named revisioned registry with collision checks and persistence<br>[HOST](#evidence-host), [API](#evidence-api) | Native host + cited final5 Chrome subset | Workbook quota32; SDK portable-library quota128; IDs/revisions are decimal strings.<br>**Remaining/limits:** None within the documented quotas and supported workbook format. |
| **R006 — Affine Extraktion**<br>SUM/SUMPRODUCT und unterstützte Ausdrücke strukturell analysieren | Accepted, bounded | Structural affine extraction including host SUM/SUMPRODUCT<br>[COMPILE](#evidence-compile), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | Supported expression grammar is explicit; cached samples never prove affinity.<br>**Remaining/limits:** None for the advertised structural grammar. |
| **R007 — QP Extraktion**<br>Quadratische Terme und Sparse-Struktur exakt dem Modell zuordnen | Accepted, bounded | Structural quadratic/cross-term extraction with sparse model handles<br>[COMPILE](#evidence-compile), [QP](#evidence-qp), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | Each coefficient multiplies x_i*x_j with no hidden half factor; unrepresentable raw duplicate aggregates are refused.<br>**Remaining/limits:** None within the supported grammar/numerical range. |
| **R008 — Konvexität**<br>Nur nach festgelegten gültigen Regeln als konvex klassifizieren | Accepted, bounded | Conservative compositional and checked Hessian curvature recognition<br>[COMPILE](#evidence-compile), [QP](#evidence-qp) | Native SDK/process + explicitly cited browser subset | Unproved PSD remains unsupported/unknown; sampling is not a convexity proof.<br>**Remaining/limits:** None under the stated sufficient recognition rules. |
| **R009 — Nichtlinearität**<br>Stichproben allein liefern keinen Linearitätsbeweis | Accepted, bounded | Structural nonlinear classification and adversarial cancellation/disguised nonlinear tests<br>[COMPILE](#evidence-compile) | Native SDK/process + explicitly cited browser subset | No finite probe set is used as a general linearity certificate.<br>**Remaining/limits:** None for the advertised expression classes. |
| **R010 — Automatische Differentiation**<br>Gradienten gegen analytische Referenzen prüfen | Accepted, bounded | Reverse-mode differentiation with independent multivariable analytic controls<br>[COMPILE](#evidence-compile), [DIFF](#evidence-diff) | Native SDK/process + explicitly cited browser subset | Smooth supported operators only; nonsmooth/domain branches are explicit.<br>**Remaining/limits:** None within the declared differentiable grammar. |
| **R011 — Differenzenrückfall**<br>Skalierte Schritte, Bounds und Domainfehler beachten | Accepted, bounded | Scaled finite differences with bound-aware probes and original-domain errors<br>[DIFF](#evidence-diff) | Native SDK/process + explicitly cited browser subset | All probes count toward evaluation budgets; requested derivative mode is honored.<br>**Remaining/limits:** None within the declared differentiable grammar/range. |
| **R012 — Sparse LP**<br>Bekannte LP-Lösungen, Unbeschränktheit und Unzulässigkeit korrekt unterscheiden | Accepted, bounded | Actual Clarabel sparse CSC LP path plus checked dense LP references<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [PERF](#evidence-perf) | Native SDK/process + explicitly cited browser subset | Known optimal/infeasible/unbounded controls are distinct; unchecked rays remain Unknown rather than a false proof.<br>**Remaining/limits:** No unrestricted large-sparse performance claim; this is a scale limit, not a missing truthfulness condition. |
| **R013 — MILP**<br>Kleine Referenzen erschöpfend validieren; Incumbent und Schranken getrennt | Accepted, bounded | LP branch-and-bound with exhaustive integer oracles, including the corrected near-integral LP node-closure probe<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [MIP](#evidence-mip) | Native SDK/process + explicitly cited browser subset | Binary x,y with max 1e12*x+500*y and 1e9*x+y<=1 has exact optimum 500 at [0,1]. The old rounded-zero Optimal certificate is fixed; a material incumbent/relaxation gap keeps the node open even inside integer-distance tolerance.<br>**Remaining/limits:** Exact min/max, offset 1e16 and one-node controls pass; mixed-conic unresolved leaves retain BestFound/valid bounds instead of fabricated Optimal. No branch-and-cut or unrestricted MIP claim. |
| **R014 — Warm Start**<br>Nur verfügbare Backendfähigkeit anbieten und Ergebnisse erneut validieren | Accepted, bounded | Real QP primal/dual warm starts and checked original-coordinate LP vertex-basis reuse<br>[BASIS](#evidence-basis), [QP](#evidence-qp), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | Warm state is revalidated; Clarabel warm starts are not advertised.<br>**Remaining/limits:** Sparse factor/tableau reuse is unavailable and explicitly unadvertised. |
| **R015 — Konvexe QP**<br>Primal-/Dualresiduen und KKT-Bedingungen prüfen | Accepted, bounded | General constrained continuous convex QP with primal/dual/KKT checks<br>[QP](#evidence-qp), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | Dense512 cap and sufficient curvature proof; valid original-unit bound required for Optimal.<br>**Remaining/limits:** No universal PSD recognition or integer-QP claim from this continuous path. |
| **R016 — SOCP**<br>Bekannte Kegelreferenzen bestehen; keine automatische Integerfähigkeit | Accepted, bounded | Exact Lorentz cones with original primal/dual verification<br>[ENGINES](#evidence-engines), [CERT](#evidence-cert), [ROBUST](#evidence-robust) | Native SDK/process + explicitly cited browser subset | Continuous cone capability is separate from finite integer-conic search.<br>**Remaining/limits:** No arbitrary unsupported cones or unchecked-ray proof. |
| **R017 — Glatte NLP**<br>Feasibility und Stationarität separat berichten; lokale Reichweite kenntlich | Accepted, bounded | Bound-only projected gradient and constrained augmented-Lagrangian AD/FD<br>[NLP](#evidence-nlp), [DIFF](#evidence-diff) | Native SDK/process + explicitly cited browser subset | Feasible/Stationary/stop reason separate; local method, no global or infeasibility proof.<br>**Remaining/limits:** No general nonsmooth/global NLP guarantee. |
| **R018 — Methodenname**<br>Augmented-Lagrangian-Verfahren nicht als implementiertes GRG ausgeben | Accepted, bounded | Explicit projected-gradient/augmented-Lagrangian names and method-specific refusal<br>[NLP](#evidence-nlp), [HOST](#evidence-host), [API](#evidence-api) | Native host + cited final5 Chrome subset | No substitution labeled GRG; analytic QP progress no longer falsely says simplex.<br>**Remaining/limits:** None for current Solvercraft method labels. |
| **R019 — Nichtglatte Suche**<br>Bounds, integer repair und Originalmodellprüfung bestehen | Accepted, bounded | Finite-bound differential evolution with integer repair and original-model checks<br>[HEURISTIC](#evidence-heuristic), [HOST](#evidence-host) | Native SDK/process + explicitly cited browser subset | Best feasible point found only; no infeasibility/global-optimality certificate.<br>**Remaining/limits:** No unrestricted finite-free-domain heuristic claim. |
| **R020 — Mehrfachstart**<br>Seed und Suchbudget dokumentieren; keine globale Garantie | Accepted, bounded | Seeded multistart with configured starts and shared cumulative budgets<br>[HEURISTIC](#evidence-heuristic), [NLP](#evidence-nlp) | Native SDK/process + explicitly cited browser subset | Multiple starts require finite boxes; no global guarantee.<br>**Remaining/limits:** None within its bounded local-search contract. |
| **R021 — Enginefähigkeiten**<br>Registry nennt Klassen, Dual-/Basisdaten, Cancel, WASM und Garantien | Accepted, bounded | Typed per-engine registry with classes, duals/bases/cancel/progress/WASM/limits<br>[API](#evidence-api), [BROWSER](#evidence-browser) | Native SDK/process + explicitly cited browser subset | Source compatibility and actual browser verification are distinct flags; unsupported selections fail.<br>**Remaining/limits:** Browser evidence applies to listed tested paths, not every engine/model. |
| **R022 — Jobstart**<br>Solve liefert Job-ID und Snapshotrevision ohne UI zu blockieren | Accepted, bounded | Native/browser jobs return lossless job ID and revision; omitted sheets are pinned at admission<br>[HOST](#evidence-host), [WORKER](#evidence-worker) | Native host + cited final5 Chrome subset | Native40+13 gate includes a deterministic threaded-barrier active-sheet race and nested parameter-job controls. No automatic apply or UI dialog.<br>**Remaining/limits:** Final5 source-bound browser/full-CI evidence is accepted; arbitrary caller code is not preemptible. |
| **R023 — Fortschritt**<br>Statuszugriff während Lauf; Ziel spätestens 250 ms bei normalen Modellen | Accepted, bounded | Final5 actual product worker reports progress while window timers/status remain responsive<br>[WORKER](#evidence-worker), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | Observed maximum active progress gap 65 ms, window heartbeat 22 ms and status query 1 ms in the final product-worker run; the ordinary-case 250 ms target is met by measured evidence.<br>**Remaining/limits:** Observed bounded Windows/Chrome workload, not a hard realtime or arbitrary-callback guarantee. |
| **R024 — Abbruch**<br>Cancel und Auswertungs-/Knoten-/Zeitlimits liefern korrekten Teilstatus | Accepted, bounded | Evaluation/iteration/node budgets, cooperative native deadlines/cancel and hard worker termination<br>[API](#evidence-api), [ENGINES](#evidence-engines), [WORKER](#evidence-worker) | Native host + cited final5 Chrome subset | Partial incumbent only when verified/received; hard worker termination has Unknown feasibility if none received.<br>**Remaining/limits:** No hard realtime guarantee for uncooperative native callbacks/factorization. |
| **R025 — Übernahme**<br>Ein atomarer undo-fähiger Enginebefehl mit Revisionstest | Accepted, bounded | One host apply transaction with document UID/revision validation and one-step undo<br>[HOST](#evidence-host), [UI](#evidence-ui), [WORKER](#evidence-worker) | Native host + cited final5 Chrome subset | Preview never writes automatically; stale/foreign results cannot apply.<br>**Remaining/limits:** None for tested supported cell writes. |
| **R026 — Konfliktschutz**<br>Workbookänderung, Schutz und Spills können Übernahme verhindern | Accepted, bounded | Revision/document/protection/spill/formula decision guards<br>[HOST](#evidence-host), [UI](#evidence-ui), [WORKER](#evidence-worker) | Native host + cited final5 Chrome subset | Invalid apply is refused atomically before live writes.<br>**Remaining/limits:** None for the documented host conflict guards. |
| **R027 — Originalmodellprüfung**<br>Jede Lösung samt gerundeten Integern gegen Originalrestriktionen prüfen | Accepted, bounded | Original compiled expressions and private workbook formulas checked before preview/apply; rounded integer conic incumbents rechecked<br>[HOST](#evidence-host), [CERT](#evidence-cert), [API](#evidence-api) | Native host + cited final5 Chrome subset | Executed native regression accepts valid rounded values and rejects original-row violations after rounding.<br>**Remaining/limits:** No feasible label is permitted from an unchecked repaired vector; only declared model classes/numeric ranges are accepted. |
| **R028 — Ergebnisstatus**<br>Abbruchgrund, Zulässigkeit und Optimalität nicht vermischen | Accepted, bounded | Separate termination, feasibility and optimality enums across SDK/host/API<br>[MODEL](#evidence-model), [API](#evidence-api), [ENGINES](#evidence-engines) | Native SDK/process + explicitly cited browser subset | Cancelled/TimeLimit/Unknown/BestFound/Stationary are not conflated.<br>**Remaining/limits:** None in the tested result contracts. |
| **R029 — Gap**<br>Nur bei zulässigem Incumbent und gültiger Schranke, Definition dokumentiert | Accepted, bounded | Gap only with checked incumbent and valid original-unit bound; near-integral rounding no longer closes an unfinished node<br>[CERT](#evidence-cert), [QP](#evidence-qp), [RISK](#evidence-risk), [MIP](#evidence-mip) | Native SDK/process + explicitly cited browser subset | Exact constant-free comparison prevents a large additive objective offset from hiding the 500-unit binary improvement. A one-node stop retains incumbent 0 and an open bound/nonzero gap; the conic probe can retain BestFound at 500 with bound 500.0000009476 and one unresolved leaf.<br>**Remaining/limits:** The specific conic numeric bound is one executed probe, not a promised exact iteration result. Unknown/unresolved leaves prevent Optimal; no global bound from local/heuristic paths. |
| **R030 — Ribbonposition**<br>Solvercraft unmittelbar rechts neben bestehendem Solver | Accepted, bounded | Actual Data ribbon places Solvercraft directly right of existing Solver<br>[UI](#evidence-ui) | Native host + cited final5 Chrome subset | Executed real egui input test and actual Trunk screenshot.<br>**Remaining/limits:** None for the tested ribbon layout. |
| **R031 — Aufgabenpanel**<br>Ziel/Variablen/Constraints/Methode und Ergebnisvorschau bedienbar | Accepted, bounded | Task pane edits goal/variables/constraints/method, starts/polls and previews/applies<br>[UI](#evidence-ui), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | Actual user interaction tested; supported fields/classes only.<br>**Remaining/limits:** Advanced SDK risk/decision modules are not implied to have complete custom workbook dialogs. |
| **R032 — Zugänglichkeit**<br>Fokus, Tastatur, Themes und deutsche/englische Beschriftung prüfen | Accepted, bounded | Five actual native UI tests include EN/DE x light/dark, focus, Tab/Shift-Tab, draft typing and Ctrl-Enter preview<br>[UI](#evidence-ui) | Native host + cited final5 Chrome subset | Principal accessible names and actual dark German browser pane; input changes only draft until explicit apply.<br>**Remaining/limits:** No complete screen-reader or arbitrary-platform conformance certification is inferred from the executed matrix. |
| **R033 — Programmatische API**<br>CLI/MCP-Aufruf öffnet keinen Dialog; leere Parameter geben Daten oder Fehler | Accepted, bounded | Typed CLI/SDK/headless command boundaries return data or structured errors with no dialog<br>[API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Real processes test unknown/empty/invalid requests; no automatic result apply.<br>**Remaining/limits:** No unimplemented MCP service is inferred from the CLI. |
| **R034 — Antwortbericht**<br>Ziel, Variablen, Constraints, Toleranzen und Status nachvollziehbar | Accepted, bounded | Versioned host answer report contains source expressions, values/bounds, row activity/slack, tolerances/status<br>[HOST](#evidence-host), [UI](#evidence-ui) | Native host + cited final5 Chrome subset | Requires matching workbook revision; no invented duals in plain solve results.<br>**Remaining/limits:** None for the supported answer-report fields. |
| **R035 — Dualwerte**<br>Nur valide vorhandene Backenddaten ausgeben | Accepted, bounded | Original-unit QP/conic duals and separately checked LP-basis shadow prices<br>[QP](#evidence-qp), [BASIS](#evidence-basis), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | Only actual validated data; unavailable reports are explicit errors/absent fields.<br>**Remaining/limits:** No complete sparse-simplex dual/basis suite is advertised. |
| **R036 — Sensitivität**<br>LP-Basisintervalle, lokale Analyse und Parameterläufe fachlich trennen | Accepted, bounded | Checked LP basis intervals are distinct from local NLP or simulation parameter effects<br>[BASIS](#evidence-basis), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | One-at-a-time local allowable changes; simultaneous changes require revalidation.<br>**Remaining/limits:** 64-variable/512-row/combination caps are explicit; no full commercial sensitivity suite. |
| **R037 — JSON Export**<br>Versionierte portable Modelle mit verlustfreien IDs/Seeds | Accepted, bounded | Versioned SDK portable JSON and workbook model import/export with string IDs/seeds<br>[API](#evidence-api), [HOST](#evidence-host), [MANIFEST](#evidence-manifest) | Native host + cited final5 Chrome subset | Binary64/max-u64 fidelity and strict schemas tested.<br>**Remaining/limits:** None within supported schemas/byte quotas. |
| **R038 — XLSX Persistenz**<br>Eigene Parts und Beziehungen in Gridcraft vollständig roundtrippen | Accepted, bounded | Real owned OPC part and relationship survive repeated XLSX/Strict roundtrips<br>[HOST](#evidence-host) | Native host + cited final5 Chrome subset | Owned xl/solvercraft namespace; unsupported workbook semantics remain explicit.<br>**Remaining/limits:** No claim of full Excel/Frontline file compatibility. |
| **R039 — Strukturmigration**<br>Zell-/Blatt-/Tabellenänderungen migrieren Bindungen oder melden Fehler | Accepted, bounded | Anchored formula bindings use structural migration or explicit invalid-reference errors<br>[HOST](#evidence-host) | Native host + cited final5 Chrome subset | Row/sheet edits and deleted references tested; unsupported table/dynamic formulas reject.<br>**Remaining/limits:** No silent migration of unsupported reference classes. |
| **R040 — Legacy Solver**<br>solver_*-Modelle importieren; nicht darstellbaren Exportumfang melden | Accepted, bounded | Legacy solver_* import plus same-sheet typed-linear export preview/apply/undo<br>[HOST](#evidence-host) | Native host + cited final5 Chrome subset | Nonrepresentable features/options reported; existing legacy names are preserved on import.<br>**Remaining/limits:** No lossless export of arbitrary nonlinear/advanced Solvercraft models. |
| **R041 — Snapshotsemantik**<br>Trialvergleich; Live-Workbook und Historie bleiben während Probeauswertung unverändert | Accepted, bounded | Private workbook/Calc probes preserve live cells/history and pinned reference context<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | Deterministic native threaded-barrier regression verifies that changing shared active-tab view cannot retarget queued omitted-sheet or nested parameter requests.<br>**Remaining/limits:** Final5 rebuilt browser proof is accepted; unsupported dynamic formulas remain explicit. |
| **R042 — Zeit und Zufall**<br>NOW/TODAY fixieren; RAND-Semantik beim deterministischen Solve explizit | Accepted, bounded | Captured NOW/TODAY timestamp and explicit RAND/RANDBETWEEN deterministic-solve refusal<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | 1900/1904 date utility semantics; simulation uses declared addressed SC streams.<br>**Remaining/limits:** None within the supported clock/random policy. |
| **R043 — Dynamische Formeln**<br>INDIRECT/OFFSET/LAMBDA/Arrays über Fähigkeitenliste prüfen | Accepted, bounded | Explicit supported formula grammar and rejection of unsupported dynamic forms<br>[HOST](#evidence-host), [WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | INDIRECT/OFFSET/LAMBDA/unsupported arrays are not sampled and called linear.<br>**Remaining/limits:** These functions are unsupported capability entries, not silently emulated. |
| **R044 — Formelfehler**<br>Domain-/Missing-/Formelfehler nicht still als Null behandeln | Accepted, bounded | Domain/missing/reference/formula errors propagate explicitly through host/SDK<br>[WORKSHEET](#evidence-worksheet), [COMPILE](#evidence-compile), [SIM](#evidence-sim) | Native host + cited final5 Chrome subset | Invalid trials require an explicit policy; missing cells do not silently become numerical zero.<br>**Remaining/limits:** None for the tested error contracts. |
| **R045 — Verteilungsbasis**<br>Alle 17 benannten R2-Verteilungen mit Parameter- und Randfalltests | Accepted, bounded | All17 original R2 distribution families implemented with host declarations<br>[MATH](#evidence-math), [WORKSHEET](#evidence-worksheet), [ORACLE](#evidence-oracle) | Native host + cited final5 Chrome subset | Parameter/support/edge semantics explicit; empirical discrete/continuous are distinct.<br>**Remaining/limits:** No extra-catalog parity is inferred from the17-family baseline. |
| **R046 — Verteilungskatalog**<br>Jede weitere eingefrorene Referenzfunktion einzeln testen | Partial | 111 frozen distribution/Alt/custom/special/time-series entries mapped: 109 unambiguous executable core subsets plus two tested mathematical variants<br>[REF](#evidence-ref), [MATH](#evidence-math), [ORACLE](#evidence-oracle) | Native SDK/process + explicitly cited browser subset | R046 is the distribution mathematical catalog; worksheet signatures/defaults/properties belong R081. All 28 alternates have individual evidence, but mathematical variants are not automatically exact vendor signatures.<br>**Remaining/limits:** Pearson6Alt and WeibullAlt retain unresolved published argument counts/defaults. Do not invent parameters or assert all 111 vendor functions complete; resolve source-backed semantics before closing. |
| **R047 — Inverse CDF**<br>Quantile, Support und Tailverhalten gegen Referenzen prüfen | Accepted, bounded | Prepared CDF/quantile/density with independent SciPy parameter-grid and tail controls<br>[MATH](#evidence-math), [ORACLE](#evidence-oracle) | Native SDK/process + explicitly cited browser subset | 4071 finite comparisons; one warned reference excluded; stated numerical ranges, no universal relative-tail accuracy claim.<br>**Remaining/limits:** No all-parameter/all-tail mathematical error certificate. |
| **R048 — Verteilungsmodifikatoren**<br>Trunkierung, Shift, Scale, Mixture und Compound separat spezifizieren | Accepted, bounded | Separate truncation/shift/scale/mixture/compound contracts and tests<br>[MATH](#evidence-math), [REF](#evidence-ref) | Native SDK/process + explicitly cited browser subset | Atoms/reflections and bounded preparation explicit; generic compound CDF/PDF unavailable rather than fabricated.<br>**Remaining/limits:** Unsupported generic compound queries are explicit capability limits. |
| **R049 — RNG Streams**<br>Gleiche Samples unabhängig von Workerzahl und Auswertungsreihenfolge | Accepted, bounded | Versioned addressed RNG independent of trial/binding evaluation order and scheduling<br>[MATH](#evidence-math), [SIM](#evidence-sim) | Native SDK/process + explicitly cited browser subset | Legacy goldens and immutable addressed identities retained; no mutable thread-order stream.<br>**Remaining/limits:** Only fixed version/seed/binding/trial contracts promise reproducibility. |
| **R050 — Monte Carlo**<br>IID-Sampling und Seedmanifest reproduzierbar | Accepted, bounded | IID Monte Carlo and lossless seed manifests with repeatable addressed draws<br>[SIM](#evidence-sim), [API](#evidence-api), [MANIFEST](#evidence-manifest) | Native SDK/process + explicitly cited browser subset | Native host/server and real CLI simulation manifests disclose seed, sampler, effective configuration and diagnostic or real host snapshot identity.<br>**Remaining/limits:** Final rebuilt browser/release evidence remains separate under R088/R093. |
| **R051 — Latin Hypercube**<br>Strata, Dimensionen, Replikate und Abhängigkeiten korrekt | Accepted, bounded | LHS strata/dimensions with explicit dependence transformation and independent-seed replication rules<br>[SIM](#evidence-sim), [INTERVAL](#evidence-interval) | Native SDK/process + explicitly cited browser subset | Repeated seed is refused as an independent LHS replicate even if metadata labels differ.<br>**Remaining/limits:** No IID single-run interval is inferred from LHS samples. |
| **R052 — Randomized Sobol**<br>Richtungstabellen lizenziert; Unsicherheit über unabhängige Scrambles | Accepted, bounded | Randomized64-dimensional Sobol-v2 with licensed Joe-Kuo directions and independent replicate intervals<br>[SOBOL](#evidence-sobol), [INTERVAL](#evidence-interval) | Native SDK/process + explicitly cited browser subset | 16384 independent SciPy point anchors; separate independent randomization identities.<br>**Remaining/limits:** No unrestricted dimensions or single-scramble IID error bar. |
| **R053 — Rangkorrelation**<br>Rang- vs Pearson-Korrelation klar; ungültige Matrizen ablehnen | Accepted, bounded | Explicit latent-Gaussian Pearson correlation vs continuous Gaussian-copula Spearman conversion and PSD checks<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native SDK/process + explicitly cited browser subset | The matrix is not generally Pearson correlation of transformed nonnormal outputs; discrete ties need not reproduce target Spearman exactly. Invalid matrices reject; singular PSD/zero variance tested, no silent repair.<br>**Remaining/limits:** Parameter/dimension caps remain explicit. |
| **R054 — Gaussian Copula**<br>Marginalverteilungen und spezifizierte Abhängigkeit validieren | Accepted, bounded | Gaussian copula margins/quadrants/dependence checked independently<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native SDK/process + explicitly cited browser subset | Supported continuous/discrete marginal behavior and singular handling are explicit.<br>**Remaining/limits:** No universal empirical correlation exactness for finite samples. |
| **R055 — Weitere Copulas**<br>Student-t/Clayton/Frank/Gumbel mit erlaubten Parametern und Dimensionen | Accepted, bounded | Student-t/Clayton/Frank/Gumbel including permitted negative/reflected cases<br>[SIM](#evidence-sim), [MATH](#evidence-math) | Native SDK/process + explicitly cited browser subset | Documented bivariate/multivariate theta and dimension ranges; analytic CDF/tau/margins controls.<br>**Remaining/limits:** No unrestricted Archimedean parameter/dimension support. |
| **R056 — Online Statistik**<br>Momente und Fehlertrials korrekt; Speicher nicht proportional zu Vollmatrix | Accepted, bounded | Compensated online moments, valid/invalid counts and explicit Fail/Skip policies<br>[SIM](#evidence-sim) | Native SDK/process + explicitly cited browser subset | Streaming storage is not the trials-by-input-by-output full matrix; Skip warns of selection bias.<br>**Remaining/limits:** Retained exact statistics still require bounded retained rows. |
| **R057 — Quantile**<br>Exakt oder klar markierter Sketch mit definierter Semantik | Accepted, bounded | Exact retained empirical lower-inverse quantiles, clear absence in streaming-only mode<br>[MATH](#evidence-math), [SIM](#evidence-sim) | Native SDK/process + explicitly cited browser subset | No unlabeled sketch/external-sort claim; retained memory checked.<br>**Remaining/limits:** No exact quantiles from unretained arbitrary streams. |
| **R058 — VaR und CVaR**<br>Verlustkonvention und diskrete Quantilsmasse korrekt | Accepted, bounded | Upper-loss VaR/CVaR with fractional mass at discrete quantile boundary<br>[RISK](#evidence-risk), [MATH](#evidence-math), [SIM](#evidence-sim) | Native SDK/process + explicitly cited browser subset | Unequal raw weights, permutations, alpha0 and tiny/extreme weights independently tested.<br>**Remaining/limits:** Unsupported conditioning/range returns errors. |
| **R059 — Verteilungsfitting**<br>Schätzung, AIC/BIC und geeignete Güteprüfung mit Parameterfit beachten | Accepted, bounded | 15 automatic estimators plus bounded local likelihood fitting, AIC/BIC and refitted bootstrap diagnostics<br>[FIT](#evidence-fit) | Native SDK/process + explicitly cited browser subset | Mixed PMF/PDF rankings refused; parameter-fit-aware GOF; generic fitting is local, not certified global MLE.<br>**Remaining/limits:** Weighted/censored fitting unavailable and explicitly outside this supported fit contract. |
| **R060 — Unsicherheitsintervalle**<br>Samplingannahmen und Stoppregeln passen zum Verfahren | Accepted, bounded | Procedure-specific fixed-sample/replicate intervals and bounded MC checkpoint stopping<br>[INTERVAL](#evidence-interval), [SIM](#evidence-sim) | Native SDK/process + explicitly cited browser subset | Gaussian/finiteness/independent-replicate assumptions explicit; large-df t critical value approximate; off-checkpoint adaptive cancellation has no fabricated CI.<br>**Remaining/limits:** No distribution-free coverage for arbitrary data-dependent stopping or Skip policy. |
| **R061 — SC Formeln**<br>Stabile Input-/Outputbindungen; definierte Semantik außerhalb Simulation | Accepted, bounded | Stable discovered/explicit SC input/output bindings with deterministic outside-simulation nominal policy<br>[WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | Means where available, labelled fallback for undefined means; not PSI display/default parity.<br>**Remaining/limits:** No full PSI property/outside-simulation equivalence. |
| **R062 — Simulation Charts**<br>Histogramm/CDF/Scatter/Box/Tornado/Quantilband mit korrekten Daten | Accepted, bounded | Six real native UI tests plus eight renderer-used numerical contracts and one retained-result lifecycle test passed final5<br>[UI](#evidence-ui), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | All five sample chart choices and parameter Tornado are exercised; tiny/large/subnormal/independent-axis data and retained quantiles are checked. The nine-test chart-module count includes one lifecycle case, not nine numerical oracles.<br>**Remaining/limits:** Only the declared data/display contracts and executed UI/browser scope are accepted; no complete PSI or arbitrary platform chart parity. |
| **R063 — Parameterläufe**<br>Werte pro Simulation konstant und Ergebnisse eindeutig vergleichbar | Accepted, bounded | SDK parameter grid and host constant-parameter cases with CRN/distinct streams<br>[SIM](#evidence-sim), [HOST](#evidence-host) | Native host + cited final5 Chrome subset | Parameters constant per run; run coordinates/seeds disclosed; combined trial/memory caps.<br>**Remaining/limits:** No automatic causal interpretation of parameter effects. |
| **R064 — Trialkonsistenz**<br>Mehrere Reads derselben Zufallszelle liefern denselben Trialwert | Accepted, bounded | Each uncertain cell sampled once per trial; ordered simulation IR preserves repeated reads and Calc arithmetic<br>[WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | Six executed native controls include 29 formulas x100 addressed vectors with exact-bit Calc comparison, error/coercion/names/clock/history/cancel cases.<br>**Remaining/limits:** The final5 product-worker differential check is accepted; no arbitrary unsupported formula semantics are inferred. |
| **R065 — SAA**<br>Trainingsszenarien für Kandidatenvergleich fixieren | Accepted, bounded | One frozen materialized SAA training set for all candidate comparisons<br>[SAA](#evidence-saa) | Native SDK/process + explicitly cited browser subset | Independent affine-loss marginals/deterministic feasible set; candidate order does not redraw.<br>**Remaining/limits:** No arbitrary endogenous or correlated uncertain-program compiler claim. |
| **R066 — Unabhängige Validierung**<br>Eigenes Seedset und getrennte Unsicherheit für finale Lösung | Accepted, bounded | Distinct-seed final-decision holdout with separate mean/variance/SE/labelled interval<br>[SAA](#evidence-saa) | Native SDK/process + explicitly cited browser subset | The holdout evaluates a fixed selected decision; no population-optimality/CVaR-interval certificate.<br>**Remaining/limits:** No adaptive reuse of the holdout as training. |
| **R067 — Risikoziele**<br>Erwartungswert, Varianzstrafe und CVaR mit Originalmodellprüfung | Accepted, bounded | Expected loss, nonnegative mean-variance penalty and upper-tail CVaR with original-risk checks<br>[RISK](#evidence-risk), [SAA](#evidence-saa) | Native SDK/process + explicitly cited browser subset | Final coefficient-assembly allowance over finite boxes adjusts bounds/status; preceding moment checks are conditioning guards, not full interval proofs.<br>**Remaining/limits:** No unsupported unbounded coefficient-error/global statistical certificate. |
| **R068 — Chance Constraints**<br>Einzelne und gemeinsame Bedingungen; keine erfundene Wahrscheinlichkeit | Accepted, bounded | Individual and joint finite-distribution chance constraints with original violation-mass recheck<br>[LOGIC](#evidence-logic) | Native SDK/process + explicitly cited browser subset | Declared finite scenarios are the probability law; not a population confidence assertion.<br>**Remaining/limits:** No invented probability outside the finite model. |
| **R069 — Big M**<br>Nur aus gültigen Grenzen abgeleitet; bei fehlenden Bounds ablehnen | Accepted, bounded | Outward finite-domain-derived M with exact slack gate and missing-bound refusal<br>[LOGIC](#evidence-logic), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | Original small RHS stays separate from huge M; equalities use two gates; expansion preflight.<br>**Remaining/limits:** No unrestricted unbounded-domain Big M. |
| **R070 — Robuste Gegenstücke**<br>Box/Budget/Ellipsoid nur unter passenden Strukturannahmen | Accepted, bounded | Weighted-support box and budget counterparts with checked owned solves; checked ellipsoid Lorentz counterpart<br>[ROBUST](#evidence-robust), [RISK](#evidence-risk) | Native SDK/process + explicitly cited browser subset | Original support/objective is verified before retaining Feasible/bounds. Exact product ordering/fractional budgets and directed arithmetic prevent false support from auxiliary slack or formation loss.<br>**Remaining/limits:** Plain compilation alone is not original-risk tolerance certification. Twenty independent original-space LP vertex cases plus four tiny signed cases pass; no arbitrary robust nonlinear/uncertain-RHS claim. |
| **R071 — Recourse**<br>Zweistufiges lineares deterministisches Äquivalent korrekt | Accepted, bounded | Two-stage linear deterministic equivalent with common first stage and local recourse<br>[RECOURSE](#evidence-recourse) | Native SDK/process + explicitly cited browser subset | Finite scenarios; zero-weight scenarios retain feasibility; integer stages supported by equivalent route.<br>**Remaining/limits:** No arbitrary multistage/endogenous/nonlinear recourse. |
| **R072 — Nichtantizipativität**<br>Erste Entscheidungen über Szenarien identisch; kein Zukunftswissen | Accepted, bounded | Nonanticipativity structural through one shared first-stage variable block<br>[RECOURSE](#evidence-recourse) | Native SDK/process + explicitly cited browser subset | Independent newsvendor and two-resource/exhaustive controls prohibit scenario foresight.<br>**Remaining/limits:** No inferred arbitrary information structure. |
| **R073 — Benders**<br>Nur zulässige Teilproblemklassen und valide Cuts | Accepted, bounded | Actual multicut Benders with explicitly solved primal/dual recourse and valid checked cuts<br>[RECOURSE](#evidence-recourse) | Native SDK/process + explicitly cited browser subset | Complete continuous nonnegative recourse subclass, finite first-stage bounds and cumulative budgets.<br>**Remaining/limits:** Integer/incomplete recourse rejected; no general L-shaped/MINLP solver claim. |
| **R074 — Entscheidungsbäume**<br>Rückwärtsauswertung und Nutzen an analytischen Fällen prüfen | Accepted, bounded | Acyclic finite decision graph backward expected-utility evaluation<br>[LOGIC](#evidence-logic) | Native SDK/process + explicitly cited browser subset | Linear/CARA utility, edge costs, cycles/domains bounded and validated.<br>**Remaining/limits:** No arbitrary utility family or inferred spreadsheet tree. |
| **R075 — Value of Information**<br>Informationszeitpunkt und Kosten in Baumrechnung beachten | Accepted, bounded | Prior/post-signal/perfect-information action comparison with acquisition costs before utility<br>[LOGIC](#evidence-logic), [API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Consistent normalized law fixes phantom VOI; linear/CARA analytic free/cost/revealing controls.<br>**Remaining/limits:** No unsupported information timing or arbitrary dynamic information model. |
| **R076 — Mehrziel**<br>Gewichtete, lexikografische und epsilon-Constraint-Verfahren nachvollziehbar | Accepted, bounded | Weighted, lexicographic and epsilon-constraint linear-goal methods with trace<br>[LOGIC](#evidence-logic) | Native SDK/process + explicitly cited browser subset | Actual normalized scalarization weights and imposed priority bounds disclosed; sequential budgets.<br>**Remaining/limits:** No complete nonconvex Pareto-front enumeration. |
| **R077 — SOS und Indikatoren**<br>Gültige Reformulierungen und Bounds mit kleinen Referenzen prüfen | Accepted, bounded | SOS1/SOS2 adjacency and indicator slack-gate formulations with finite bounds<br>[LOGIC](#evidence-logic), [CERT](#evidence-cert) | Native SDK/process + explicitly cited browser subset | Original logical semantic validation before returning incumbents.<br>**Remaining/limits:** No arbitrary native commercial indicator/cut engine. |
| **R078 — Finite Domains**<br>AllDifferent/Reifikation nur als tatsächlich freigegebene Fähigkeit | Accepted, bounded | Finite one-hot domains, AllDifferent and explicit separated reification<br>[LOGIC](#evidence-logic) | Native SDK/process + explicitly cited browser subset | Strict complements require a caller-supplied positive gap; unsupported equality disjunction explicit.<br>**Remaining/limits:** No invisible continuous strict-complement emulation. |
| **R079 — Globalverfahren**<br>Keine globale Schranke aus lokaler NLP-Lösung | Accepted, bounded | Polynomial finite-box interval branch-and-bound and replayable witnesses separate from NLP<br>[GLOBAL](#evidence-global) | Native SDK/process + explicitly cited browser subset | Local stationary points never produce a global bound; unsupported global classes explicitly rejected.<br>**Remaining/limits:** No general constrained/nonpolynomial global optimizer. |
| **R080 — Mixed Integer Conic**<br>MIQP/MISOCP nur nach eigenständiger Backend-/Relaxierungsprüfung | Accepted, bounded | Finite integer MIQP/MISOCP branch-and-bound using true convex conic relaxations<br>[QP](#evidence-qp), [CERT](#evidence-cert), [API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Exhaustive independent optima/bounds and interrupted frontier checks; unknown leaves remain unresolved.<br>**Remaining/limits:** No general unbounded-domain MICP, cutting planes or parallel MIP. |
| **R081 — PSI Import**<br>Jede unterstützte Signatur und Defaultsemantik separat geprüft | Partial | 19 individually checked core PSI imports and strict SC.PSI shims with explicit conversion/default limits<br>[REF](#evidence-ref), [WORKSHEET](#evidence-worksheet) | Native host + cited final5 Chrome subset | Properties/ambiguous custom atoms remain preserved/unsupported; SDK constructors are not worksheet parity.<br>**Remaining/limits:** Complete frozen supported-signature/default/property mapping and independent host cases per claimed reference entry; full-reference records remain open. |
| **R082 — Headless Server**<br>Optionaler Dienst mit Jobs, Authentisierung, Quoten und Cancel | Accepted, bounded | Explicit optional loopback service with per-connection auth/jobs/quotas/poll/cancel/forget<br>[API](#evidence-api) | Native SDK/process + explicitly cited browser subset | No listener without explicit bind/run; shutdown admission race fixed; bounded JSON and shared AST limits tested.<br>**Remaining/limits:** Remote TLS gateway and production deployment operations are separate host responsibilities. |
| **R083 — Rust SDK**<br>Eigenständiger ausführbarer Host ohne Workbook-/egui-Abhängigkeit | Accepted, bounded | Independently buildable Rust SDK/CLI without workbook or egui<br>[API](#evidence-api), [DEPS](#evidence-deps) | Native SDK/process + explicitly cited browser subset | Standalone core and native executable actually run.<br>**Remaining/limits:** No second GUI application implied. |
| **R084 — ArtCraft Reuse**<br>Zweiter Host nutzt gleiche Engine-/Modellverträge; echte Integration oder Beispiel klar benannt | Accepted, bounded | Executed standalone artcraft_layout example uses shared engines/portable model/bindings<br>[API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Clearly an example, not a shipped second ArtCraft application; optimum140 at widths20/100.<br>**Remaining/limits:** None for the original criterion's explicit example option. |
| **R085 — Referenzfreeze**<br>Produkt-/Versionsscope und Featureinventar nachvollziehbar | Accepted, bounded | Pinned full570-page Frontline2025Q1 scope, source hash and unfiltered272-function inventory<br>[REF](#evidence-ref) | Documentation / recorded final5 reports | 28 alternate forms, five unresolved mentions and source conflicts retained; later product revisions separate.<br>**Remaining/limits:** No implementation parity inferred from inventory presence. |
| **R086 — Lizenzen**<br>Direkte/transitive Abhängigkeiten, Daten und Notices vor Übernahme prüfen | Accepted, bounded | Resolved production/test dependency feature/license inventories and licensed Sobol data notices<br>[DEPS](#evidence-deps), [SOBOL](#evidence-sobol), [ORACLE](#evidence-oracle) | Documentation / recorded final5 reports | Clarabel0.11.1 Apache2; direct/transitive notices recorded; SciPy/NumPy test-only.<br>**Remaining/limits:** Not a vulnerability audit or proprietary-engine license grant. |
| **R087 — Unabhängige Orakel**<br>Analytische/erschöpfende Referenzen unabhängig vom getesteten Backend | Accepted, bounded | Independent analytic/exhaustive/NumPy/SciPy/Fraction numerical references<br>[CERT](#evidence-cert), [ORACLE](#evidence-oracle), [PERF](#evidence-perf), [RECOURSE](#evidence-recourse) | Native SDK/process + explicitly cited browser subset | Oracles compare actual original objective/bound/semantics, not only backend statuses.<br>**Remaining/limits:** Finite fixtures and tolerance grids do not prove all inputs. |
| **R088 — Rust und WASM**<br>Standardfeaturebaum ohne native Pflichtengine; echter Browserlauf | Accepted, bounded | Pure-Rust numerical default closure with actual final5 Chrome SDK13 and product-worker7 execution<br>[BROWSER](#evidence-browser), [WORKER](#evidence-worker), [DEPS](#evidence-deps), [FINAL](#evidence-final) | Native SDK/process + explicitly cited browser subset | Accepted Trunk/test-WASM builds and actual browser test reports are source-bound; host CI includes its WASM stage. Runtime execution is not inferred from cross-compilation.<br>**Remaining/limits:** Only listed Chrome cases are browser-runtime evidence; no current Linux/macOS or universal browser execution claim. |
| **R089 — Ressourcen**<br>Vor Allokationen Größen, Nonzeros, Rekursion, Trials und Output begrenzen | Accepted, bounded | Final5 source-bound resource/error/admission gates include AST/nonzero/trial/output/transform checks before expansion<br>[API](#evidence-api), [CERT](#evidence-cert), [MODEL](#evidence-model), [SIM](#evidence-sim), [FINAL](#evidence-final) | Native SDK/process + explicitly cited browser subset | Corrected typed response preflight, per-output summary bounds, target/MICP admission and original robust support regressions pass within SDK630/host2827 and current CLI/browser controls.<br>**Remaining/limits:** Configured quotas are accounting/input/output bounds, not an OS-RSS quota or hard preemption guarantee; declared size/numeric limits remain. |
| **R090 — Keine Panics**<br>Result-Fehler und geprüfte Zugriffe; Abhängigkeiten auf Fehlerverhalten prüfen | Accepted, bounded | Validated indices/shapes, structured errors and dependency-adapter failure paths pass final5 negative/regression gates<br>[API](#evidence-api), [CERT](#evidence-cert), [DEPS](#evidence-deps), [FINAL](#evidence-final) | Native SDK/process + explicitly cited browser subset | SDK630 passed with zero failures/ignored; host2827 passed with zero failures and one pre-existing ignored performance test. Native evaluator panic containment and slot recovery are exercised.<br>**Remaining/limits:** No universal no-panic theorem: abort/OOM/panic-abort and arbitrary nonreturning or panicking caller code remain outside recoverable direct-call guarantees. |
| **R091 — Erfüllungsmatrix**<br>Jede Referenzfunktion mit Test, Plattform und Garantie; Lücken offen | Accepted, bounded | 96 original criteria plus audited272-function source/test/platform/guarantee matrix with executable exact-test runner<br>[REF](#evidence-ref) | Documentation / recorded final5 reports | Missing implementations/tests and source conflicts remain explicit. Audit verifies source/test hashes and actual selectors; planned IDs are not counted as execution.<br>**Remaining/limits:** Implementation gaps stay open under R046/R081; truthful matrix completeness is not full function parity. |
| **R092 — Leistungsnachweis**<br>Hardware, Seeds, RAM, Laufzeit und Fehler mit Versionen dokumentieren | Accepted, bounded | Five fresh release processes per bounded LP and streaming-MC workload record hardware/seed/RAM/runtime/error/versions<br>[PERF](#evidence-perf), [FINAL](#evidence-final) | Documentation / recorded final5 reports | The final release binary is source-bound. LP (100 variables/500 rows): median 260.2045 ms, peak 11,759,616 bytes, objective error 0. MC (100,000 trials/20 inputs/10 outputs): median 712.7748 ms, peak 6,799,360 bytes, maximum mean error 0.001469193929023671.<br>**Remaining/limits:** Shared host with native regression executables running concurrently; no isolated speed comparison. Proposed medium 10,000-variable/50,000-row/500,000-nonzero class and time-to-first-feasible remain unmeasured and are not claimed as supported performance. |
| **R093 — CI Gates**<br>cargo xtask ci und passende fachliche Tests; keine erfundenen Resultate | Accepted, bounded | Final5 unchanged-source cargo xtask ci passed all 10 stages; nested SDK fmt/clippy/tests and actual UI/browser gates also passed<br>[GATES](#evidence-gates), [FINAL](#evidence-final) | Documentation / recorded final5 reports | Host: 2827 passed/0 failed/1 pre-existing ignored in 722.25 s. SDK: 630 passed/0 failed/0 ignored. Native UI: 6; chart module: 8 numerical + 1 lifecycle; actual Chrome SDK: 13; product worker: 7, all source-bound.<br>**Remaining/limits:** Ignored pre-existing recalc_with_range_formulas_scales_linearly remains visible and is not counted as passed. These finite tests do not imply full vendor parity. |
| **R094 — Dokumentationspflege**<br>Architektur, Roadmap, Gaps und relevante Parität nachführen | Accepted, bounded | Architecture/roadmap/implementation/QA/reference/gap documents track supported contracts and historical checkpoints<br>[REF](#evidence-ref), [GATES](#evidence-gates) | Documentation / recorded final5 reports | Historical initial reports are labelled; exact target-equality semantics and current reference/import/platform gaps are reconciled.<br>**Remaining/limits:** Current final5 reports are linked; historical builds remain labelled and no full-parity claim is made. |
| **R095 — Manifest**<br>Snapshot, Engine, Toleranzen, Seed und Plattform pro Lauf dokumentieren | Accepted, bounded | Managed execute/host/server/CLI runs and admitted CLI numerical errors retain typed configuration/input/snapshot/engine/platform identity<br>[MANIFEST](#evidence-manifest), [API](#evidence-api) | Native SDK/process + explicitly cited browser subset | Actual process tests verify one error JSON, empty stdout, original code/message, repeated/changed fingerprints, configuration/platform and honest unknown engine without a typed result. Latest scoped CLI package: 66 tests, Clippy and formatting passed after the integer-rounding correction.<br>**Remaining/limits:** Raw decode failures stay bare because no typed run was admitted; low-level numerical APIs are wrapped by managed run entrypoints. Final5 browser/source gates are recorded; arbitrary external embeddings must use their managed run contract. |
| **R096 — Schema Migration**<br>Versionierte Verträge und validierte Migrationspfade | Accepted, bounded | Versioned strict portable schemas and explicit validated algebraic-v1 migration<br>[API](#evidence-api), [HOST](#evidence-host) | Native SDK/process + explicitly cited browser subset | No guessed IDs/bindings/results; unknown schemas rejected; migration preserves model semantics/options.<br>**Remaining/limits:** Only declared migrations are supported; no invented arbitrary future-schema conversion. |

## Evidence index

These are actual source/tests or source-bound execution reports. Historical implementation notes retain their own checkpoint scope; the final5 summaries and row qualifications govern this assessment.

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
- [docs/implementation/gates-final/host/summary.json](../../docs/implementation/gates-final/host/summary.json)
- [docs/implementation/final-verification-summary.json](../../docs/implementation/final-verification-summary.json)

### Evidence WORKSHEET

- [../crates/engine/src/cmd/solvercraft/worksheet_tests.rs](../../../crates/engine/src/cmd/solvercraft/worksheet_tests.rs)
- [../crates/engine/src/cmd/solvercraft/simulation_ir/tests.rs](../../../crates/engine/src/cmd/solvercraft/simulation_ir/tests.rs)
- [docs/implementation/gridcraft.md](../../docs/implementation/gridcraft.md)

### Evidence UI

- [../crates/ui-egui/tests/solvercraft_acceptance.rs](../../../crates/ui-egui/tests/solvercraft_acceptance.rs)
- [../crates/ui-egui/src/solvercraft.rs](../../../crates/ui-egui/src/solvercraft.rs)
- [../crates/ui-egui/src/solvercraft/report.rs](../../../crates/ui-egui/src/solvercraft/report.rs)
- [docs/implementation/browser-app-solvercraft-pane-report.json](../../docs/implementation/browser-app-solvercraft-pane-report.json)
- [docs/implementation/native-ui-focus-matrix-report.json](../../docs/implementation/native-ui-focus-matrix-report.json)
- [docs/implementation/native-ui-controls-report.json](../../docs/implementation/native-ui-controls-report.json)

### Evidence WORKER

- [docs/implementation/browser-worker-final-report.json](../../docs/implementation/browser-worker-final-report.json)
- [docs/implementation/trunk-final-build-report.json](../../docs/implementation/trunk-final-build-report.json)
- [docs/implementation/worker-browser-final-build-report.json](../../docs/implementation/worker-browser-final-build-report.json)

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
- [docs/implementation/browser-smoke-sobol64-report.json](../../docs/implementation/browser-smoke-sobol64-report.json)

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

### Evidence DEPS

- [docs/implementation/dependency-inventory.json](../../docs/implementation/dependency-inventory.json)
- [docs/implementation/dependency-inventory.features.txt](../../docs/implementation/dependency-inventory.features.txt)
- [docs/implementation/engines.md](../../docs/implementation/engines.md)

### Evidence BROWSER

- [docs/implementation/browser-smoke-final-report.json](../../docs/implementation/browser-smoke-final-report.json)
- [docs/implementation/browser-worker-final-report.json](../../docs/implementation/browser-worker-final-report.json)
- [docs/implementation/trunk-final-build-report.json](../../docs/implementation/trunk-final-build-report.json)
- [docs/implementation/worker-browser-final-build-report.json](../../docs/implementation/worker-browser-final-build-report.json)

### Evidence PERF

- [docs/implementation/performance-final-report.json](../../docs/implementation/performance-final-report.json)
- [docs/implementation/linear-oracle-final-report.json](../../docs/implementation/linear-oracle-final-report.json)
- [docs/implementation/quadratic-oracle-final-report.json](../../docs/implementation/quadratic-oracle-final-report.json)
- [docs/implementation/robust-oracle-final-report.json](../../docs/implementation/robust-oracle-final-report.json)
- [docs/implementation/exact-binary-oracle-final-report.json](../../docs/implementation/exact-binary-oracle-final-report.json)
- [docs/implementation/release-final-build-report.json](../../docs/implementation/release-final-build-report.json)

### Evidence GATES

- [docs/implementation/verification.md](../../docs/implementation/verification.md)
- [docs/implementation/final-verification-summary.json](../../docs/implementation/final-verification-summary.json)
- [docs/implementation/gates-final/host/summary.json](../../docs/implementation/gates-final/host/summary.json)
- [docs/implementation/gates-final/host/report.json](../../docs/implementation/gates-final/host/report.json)
- [docs/implementation/gates-final/sdk/summary.json](../../docs/implementation/gates-final/sdk/summary.json)
- [docs/implementation/gates-final/sdk/report.json](../../docs/implementation/gates-final/sdk/report.json)
- [docs/implementation/release-final-build-report.json](../../docs/implementation/release-final-build-report.json)

### Evidence MANIFEST

- [crates/solvercraft/tests/workflow_review.rs](../../crates/solvercraft/tests/workflow_review.rs)
- [crates/solvercraft/tests/owned_control.rs](../../crates/solvercraft/tests/owned_control.rs)
- [crates/solvercraft/tests/run_manifest.rs](../../crates/solvercraft/tests/run_manifest.rs)
- [crates/cli/tests/manifests.rs](../../crates/cli/tests/manifests.rs)
- [crates/solvercraft/src/workflow.rs](../../crates/solvercraft/src/workflow.rs)
- [docs/implementation/run-manifests.md](../../docs/implementation/run-manifests.md)
- [docs/implementation/sdk-runtime.md](../../docs/implementation/sdk-runtime.md)
- [docs/implementation/verification.md](../../docs/implementation/verification.md)

### Evidence MIP

- [crates/engines/tests/integer_rounding.rs](../../crates/engines/tests/integer_rounding.rs)
- [crates/cli/tests/end_to_end.rs](../../crates/cli/tests/end_to_end.rs)
- [crates/cli/tests/manifests.rs](../../crates/cli/tests/manifests.rs)
- [docs/implementation/engines.md](../../docs/implementation/engines.md)

### Evidence FINAL

- [docs/implementation/final-verification-summary.json](../../docs/implementation/final-verification-summary.json)
- [docs/implementation/gates-final/host/summary.json](../../docs/implementation/gates-final/host/summary.json)
- [docs/implementation/gates-final/host/report.json](../../docs/implementation/gates-final/host/report.json)
- [docs/implementation/gates-final/sdk/summary.json](../../docs/implementation/gates-final/sdk/summary.json)
- [docs/implementation/gates-final/sdk/report.json](../../docs/implementation/gates-final/sdk/report.json)
- [docs/implementation/release-final-build-report.json](../../docs/implementation/release-final-build-report.json)

