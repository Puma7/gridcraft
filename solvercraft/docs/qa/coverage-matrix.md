# Solvercraft masterplan coverage matrix

Review date: 2026-10-31.

The review started from PR predecessor **690a098a6c4c9d966d66b6c30c036e0e42534bfd**, on Gridcraft base **981fad14e0b90a7906a02f29785a6d0eb214fee6**. The local review tree is a materialized source snapshot without Git metadata. Local QA fixes, added tests and their source hashes are described in [runtime-review.md](runtime-review.md); the integrator must associate the resulting commit with this report.

## Classification rules

- **Implemented + tested (native core)** means the named, bounded native capability has implementation and executed numerical/contract evidence. It is not proof of all inputs or all platforms.
- **Partial** means a subset exists or core evidence is available while part of the actual requirement remains open.
- **Absent** means no implementation of the requested feature was found. A rejecting guard or an enum name does not count as its implementation.
- **Untestable in full host** means relevant integration source exists, but this environment has not executed its full-app/UI/workbook behavior.
- **Broken** is reserved for an unresolved executed counterexample. Findings corrected and retested during this review are recorded separately.

Statuses apply to the masterplan requirement as written, not just the easiest sub-feature. No percentage below is a Frontline-parity or production-readiness score. The complete Gridcraft build, 17 host integration tests (4 existing + 13 new), visual UI validation and actual WASM/browser execution remain unexecuted.

## Evidence keys

| Key | Evidence |
| --- | --- |
| T | Shared types/strict seed codecs, native schema/round-trip tests |
| M | model crate validation and original-model arithmetic tests |
| C | compile crate structural analysis and reverse-mode differentiation tests |
| E | engines crate LP/MILP and restricted separable-QP tests |
| N | facade nonlinear.rs and nonlinear_tests.rs: finite-bound continuous AD-local path |
| D | facade heuristic.rs and heuristic_tests.rs: finite-bound constrained DE path |
| Math | math crate distribution/RNG/empirical-risk tests |
| S | simulation crate execution, online statistics, fitting and tests |
| R | facade risk.rs finite-scenario and independent-box counterpart |
| J | facade jobs.rs and native lifecycle tests |
| P | facade provenance.rs bounded diagnostic manifest |
| F | facade SDK exports and capabilities |
| IR | facade tests/independent.rs, analytically derived and exhaustively enumerated controls |
| RR | facade tests/runtime_review.rs, added executed review counterexamples and controls |
| CLI | JSON CLI process tests/source; latest all-process result belongs in the consolidated QA report |
| G | Gridcraft engine/UI integration source and currently unexecuted host tests |
| REF | reference.md and original masterplan/source registry |

The predecessor verification report recorded 136 passing standalone tests. That historical number is not the current QA test count. The integrator's final frozen run recorded 265 standalone tests passed, zero failed or ignored, plus formatting, all-targets Clippy, release and Linux cross-compile checks; see [verification-report.json](verification-report.json). This review separately records the exact suites it executed and their outcomes.

## Requirement classifications

| ID and feature | Status | Evidence | Scope or remaining work |
| --- | --- | --- | --- |

| R001 Zieldefinition | Partial | T, IR | Min/max work; target-value goals are not a complete SDK model capability. |
| R002 Variablentypen | Implemented + tested (native core) | M, E, IR | Continuous/integer/binary validation and finite oracle cases pass in the native core. |
| R003 Bounds | Implemented + tested (native core) | M, IR | Free, one-sided, fixed and binary-intersected bounds are tested; no JSON Infinity sides. |
| R004 Mehrblattmodelle | Untestable in full host | G | Multi-sheet binding is present in host source; the full-app integration tests have not executed. |
| R005 Mehrere Modelle | Absent | G | No persistent multiple-model registry/workbook model storage. |
| R006 Affine Extraktion | Partial | C, G | Core affine analysis is tested; spreadsheet SUM/SUMPRODUCT translation remains source-only. |
| R007 QP Extraktion | Implemented + tested (native core) | C, IR | Explicit quadratic and cross-term extraction and analytic arithmetic are tested. |
| R008 Konvexität | Partial | E, C | Separable diagonal curvature is checked; no general symbolic convexity proof system. |
| R009 Nichtlinearität | Implemented + tested (native core) | C, IR | Structural cubic counterexample is rejected despite matching sampled probe points. |
| R010 Automatische Differentiation | Implemented + tested (native core) | C, IR | Supported AD operators have analytic-gradient/domain tests; this is not complete Excel differentiation. |
| R011 Differenzenrückfall | Absent | N | The implemented local method uses AD; no general finite-difference fallback is supplied. |
| R012 Sparse LP | Partial | E, IR | Bounded LP cases pass, but sparse input feeds a dense tableau; mature sparse internal LP is absent. |
| R013 MILP | Implemented + tested (native core) | E, IR | Small signed binary/integer models are independently exhaustively checked. |
| R014 Warm Start | Absent | E | No implemented warm-start interface or verified resumed-backend state. |
| R015 Konvexe QP | Partial | E, IR | Only analytic separable continuous bound-QP is supported, not general constrained convex QP. |
| R016 SOCP | Absent | E | No SOCP representation/engine. |
| R017 Glatte NLP | Partial | N | Finite-bound continuous AD-local search is tested; general smooth nonlinear constraints are rejected. |
| R018 Methodenname | Implemented + tested (native core) | N, D, README | New methods identify projected-gradient/AD or DE and do not claim to be GRG. |
| R019 Nichtglatte Suche | Implemented + tested (native core) | D | Finite-bound nonsmooth DE has original-expression feasibility and integer-repair tests. |
| R020 Mehrfachstart | Partial | D | Seeded population search and work limits exist; a separate configurable multi-start program is absent. |
| R021 Enginefähigkeiten | Partial | F | Capabilities list implemented classes; no full per-engine dual/basis/warm-start/WASM/cancel contract registry. |
| R022 Jobstart | Partial | J, RR, G | Real native SDK jobs pass lifecycle tests; Gridcraft commands remain synchronous. |
| R023 Fortschritt | Partial | J, RR, G | Progress/status callbacks work; 250-ms host responsiveness and async UI have not been demonstrated. |
| R024 Abbruch | Partial | E, N, D, J, RR | Evaluation/iteration/node/memory/cancel paths exist; wall-clock budgets and measured host cancel latency remain open. |
| R025 Übernahme | Untestable in full host | G | Revision-guarded one-edit apply is in source; full-app undo/redo execution is unverified. |
| R026 Konfliktschutz | Untestable in full host | G | Identity/revision/protection/spill validation has source evidence but no executed full-host test. |
| R027 Originalmodellprüfung | Partial | M, E, N, D, RR, G | Core candidates are verified and risk objective consistency is checked; host re-evaluation/apply remains unexecuted. |
| R028 Ergebnisstatus | Implemented + tested (native core) | T, E, N, D, RR | Termination, feasibility and optimality are distinct; stationary/heuristic results do not fabricate optimality. |
| R029 Gap | Implemented + tested (native core) | E, IR | Native finite MIP bound/gap and interrupted-incumbent tests exist; these are floating-point guarantees. |
| R030 Ribbonposition | Untestable in full host | G | Ribbon code places Solvercraft after Solver; visual and input validation have not run. |
| R031 Aufgabenpanel | Partial | G, README | A modal optimization/simulation flow exists in source, not the planned right task pane. |
| R032 Zugänglichkeit | Untestable in full host | G | Keyboard/focus/theme/language behavior has not been visually or interactively validated. |
| R033 Programmatische API | Partial | CLI, G | CLI structured responses are exercised; full engine/MCP behavior and host empty-input semantics remain separate gates. |
| R034 Antwortbericht | Partial | F, CLI, G | Structured result/summary exists, but no complete workbook answer-report feature. |
| R035 Dualwerte | Absent | E | No valid exposed dual-value backend API. |
| R036 Sensitivität | Absent | E | No basis intervals, shadow-price report or full parameter sensitivity program. |
| R037 JSON Export | Partial | T, CLI | Versioned JSON models/seeds have round-trip coverage; complete model import/export lifecycle is absent. |
| R038 XLSX Persistenz | Absent | G | XLSX model metadata persistence is explicitly not implemented. |
| R039 Strukturmigration | Absent | G | No saved binding migration through structural workbook edits. |
| R040 Legacy Solver | Absent | G | No solver_* legacy model import/export compatibility layer. |
| R041 Snapshotsemantik | Untestable in full host | G | Private workbook/Calc comparison is in source; no executed full-app Trial equivalence check. |
| R042 Zeit und Zufall | Partial | G | Unsupported volatile/dynamic expressions are restricted; complete explicit frozen time/RAND host semantics are not tested. |
| R043 Dynamische Formeln | Partial | C, G | Supported AST subset and conservative rejection exist; broad dynamic/reference/LAMBDA behavior is absent. |
| R044 Formelfehler | Partial | M, C, N, D, S, G | Native domain/nonfinite/arity failures are tested; full workbook formula-error behavior is unexecuted. |
| R045 Verteilungsbasis | Partial | T, Math, S | Ten stochastic families plus Constant are present, not all 17 planned R2 families. |
| R046 Verteilungskatalog | Absent | REF | No version-frozen complete reference-function catalog with all later families. |
| R047 Inverse CDF | Partial | Math | Implemented families have quantile/CDF tests; incomplete distribution catalog and difficult-tail coverage remain. |
| R048 Verteilungsmodifikatoren | Absent | Math | No complete truncation/shift/scale/mixture/compound distribution feature set. |
| R049 RNG Streams | Implemented + tested (native core) | Math, S | Addressed seeded sampling is repeatable and evaluation-order independent in native tests; no RNG-quality certification implied. |
| R050 Monte Carlo | Implemented + tested (native core) | S, IR | Actual Monte Carlo execution has fixed-seed analytic-moment controls. |
| R051 Latin Hypercube | Implemented + tested (native core) | S, IR | LHS exact stratum occupation is tested independently per dimension. |
| R052 Randomized Sobol | Absent | Math | No randomized Sobol implementation or licensed direction-table inventory. |
| R053 Rangkorrelation | Absent | S | No rank-correlation modeling. |
| R054 Gaussian Copula | Absent | S | No Gaussian copula implementation. |
| R055 Weitere Copulas | Absent | S | No Student-t/Clayton/Frank/Gumbel copula implementation. |
| R056 Online Statistik | Partial | S | Streaming moments, counts and hard failures are implemented; no configurable invalid-trial accounting/continuation policy. |
| R057 Quantile | Implemented + tested (native core) | Math, S, IR | Retained exact empirical quantiles have a stated inverse-CDF convention; streaming mode explicitly omits them. |
| R058 VaR und CVaR | Implemented + tested (native core) | Math, S, R, RR | Discrete upper-tail ES/CVaR and weighting controls pass; review-found weighted-mass bug was fixed and retested. |
| R059 Verteilungsfitting | Partial | S | Elementary normal/uniform/exponential MLE fitting exists; AIC/BIC, general fitting and adjusted goodness-of-fit do not. |
| R060 Unsicherheitsintervalle | Absent | S | No general sampling-aware confidence-interval/stopping-rule API. |
| R061 SC Formeln | Absent | G | No SC.* worksheet stochastic function registration/binding semantics. |
| R062 Simulation Charts | Partial | G | Retained-sample histogram exists in source; CDF/scatter/box/tornado/quantile-band suite is absent and UI unexecuted. |
| R063 Parameterläufe | Absent | F | No parameter-grid/simulation-sweep feature. |
| R064 Trialkonsistenz | Partial | S, G | Same-trial input slices are tested; persisted spreadsheet stochastic bindings and SC.* behavior are absent. |
| R065 SAA | Partial | R, RR | Fixed user-supplied finite scenarios are optimized; no full sampled SAA/training workflow. |
| R066 Unabhängige Validierung | Absent | R | No independent out-of-sample validation engine; no such probability guarantee is claimed. |
| R067 Risikoziele | Partial | R, RR | Finite affine expected-loss/CVaR optimization exists; variance penalties and broad simulation optimization remain open. |
| R068 Chance Constraints | Absent | R | No individual/joint chance-constraint implementation. |
| R069 Big M | Absent | R | No Big-M chance/indicator transformation; therefore no derived-bound Big-M capability to count. |
| R070 Robuste Gegenstücke | Partial | R, RR | Independent symmetric coefficient-box <= counterpart is tested; budgeted/ellipsoid robust sets are absent. |
| R071 Recourse | Absent | R | No two-stage recourse deterministic-equivalent API. |
| R072 Nichtantizipativität | Absent | R | No stage/nonanticipativity model representation or validation. |
| R073 Benders | Absent | R | No Benders/L-shaped decomposition. |
| R074 Entscheidungsbäume | Absent | F | No decision-tree model/backward evaluation. |
| R075 Value of Information | Absent | F | No value-of-information calculation. |
| R076 Mehrziel | Absent | F | No weighted/lexicographic/epsilon-constraint multiobjective API. |
| R077 SOS und Indikatoren | Absent | E | No SOS/indicator/piecewise transformation set. |
| R078 Finite Domains | Absent | E, D | Integer bounds/repair are not AllDifferent/reification/finite-domain constraint programming. |
| R079 Globalverfahren | Absent | N, D | Global algorithms/certificates are absent; safety checks correctly avoid local-as-global claims. |
| R080 Mixed Integer Conic | Absent | E | No MIQP/MISOCP backend or valid mixed-conic relaxation. |
| R081 PSI Import | Absent | G | No tested Psi* signature/semantics compatibility layer. |
| R082 Headless Server | Partial | CLI, J | Headless JSON CLI and native SDK jobs exist; no authenticated quota-controlled network job server. |
| R083 Rust SDK | Implemented + tested (native core) | F, CLI, RR | Standalone Rust SDK/CLI executes without Gridcraft or egui dependencies. |
| R084 ArtCraft Reuse | Partial | F, CLI, G | CLI is an independent SDK consumer; no demonstrated integration in a second ArtCraft product. |
| R085 Referenzfreeze | Partial | REF | Alpha scope and source provenance are explicit; full product/version/function reference freeze is incomplete. |
| R086 Lizenzen | Partial | REF, Cargo.lock | Existing license notices/minimal dependency baseline documented; full transitive/version/table inventory remains open. |
| R087 Unabhängige Orakel | Implemented + tested (native core) | IR, RR | Analytic and complete finite-enumeration oracles are independent of the tested solver. |
| R088 Rust und WASM | Partial | README, verification | Native Rust tests and a Linux compile check are recorded; actual WASM/browser execution is untested. |
| R089 Ressourcen | Partial | M, R, J, RR, S, CLI | Preallocation/count/input caps cover implemented paths; no whole-app memory/CPU bound or arbitrary evaluator containment. |
| R090 Keine Panics | Partial | M, C, R, J, RR | Structured finite/domain failures and native panic containment are tested; full dependency/host no-crash guarantee is unverified. |
| R091 Erfüllungsmatrix | Partial | This matrix, REF | All 96 masterplan rows are classified here, not a complete Frontline function-by-function parity inventory. |
| R092 Leistungsnachweis | Absent | verification | No reproducible large-model/hard-MIP/RAM performance benchmark program. |
| R093 CI Gates | Partial | verification, RR | Standalone native tests/fmt/clippy evidence exists; full Gridcraft cargo xtask ci remains unexecuted. |
| R094 Dokumentationspflege | Partial | REF, README, validation | Solvercraft scope/validation docs exist; all parent parity/roadmap integration remains an integration review gate. |
| R095 Manifest | Partial | P, RR | Diagnostic model fingerprint, package version/options/platform exist; workbook revision/run ID/build identity and simulation manifest are incomplete. |
| R096 Schema Migration | Partial | T, CLI, RR | Schema version validation and lossless seeds are tested; no complete persisted schema-migration policy/implementation. |

## Classification distribution

| Classification | Requirements |
| --- | --- |
| Implemented + tested (native core) | 17 |
| Partial | 40 |
| Absent | 33 |
| Untestable in full host | 6 |
| Broken | 0 |


These counts describe the 96 masterplan rows only. A large missing capability such as general constrained nonlinear optimization is not equal in effort or impact to one small completed contract rule.

## README and release-claim assessment

The alpha README correctly discloses dense internal LP storage, restricted separable QP, AD stationarity rather than a minimum proof, DE heuristic results, synchronous/modal host integration and unexecuted full-host/WASM gates. Its finite-scenario/box-robust language is narrower than full stochastic/robust optimization and must remain so.

The historical 136-test artifact remains a predecessor record; current consolidated evidence is the 265-test QA report. Neither CLI background-independent jobs nor successful native SDK tests justify an asynchronous Gridcraft UI, a verified workbook undo/apply workflow, XLSX model persistence or browser support.

The implementation is an alpha foundation with meaningful working numerical features. It does not meet the full R1/R2/R3 gates or documented Frontline feature parity.

## Requirement-source identity


- Source: original masterplan requirements.csv, 96 unique IDs.
- SHA-256: 699e87ae50a62ff5605b0d40dee38f4b2f8b7200a4c92f825d2972a57e24de55.

