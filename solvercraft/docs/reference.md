# Solvercraft reference scope and dependency baseline

Last updated: 2026-10-31. Status: implementation baseline, not a release certificate.

## Product scope

Solvercraft is a workbook-independent Rust optimization and simulation library. Gridcraft is its first planned host. The long-term behavior reference is Frontline Analytic Solver Optimization + Simulation, with the Solver SDK informing the reusable API. This is a clean-room implementation program: public descriptions and mathematical methods can inform behavior; proprietary engines, source, interpreter code, artwork and documentation are not included.

The implementation baseline is Gridcraft revision `981fad14e0b90a7906a02f29785a6d0eb214fee6`, workspace version 0.14.2. The existing nonlinear Gridcraft engine is an augmented-Lagrangian method, despite its GRG label. The initial Solvercraft contract names its separate methods explicitly. Neither an alternative nonlinear method nor a successful smoke test establishes GRG compatibility.

The nested workspace is independently buildable so it can be tested without building the complete Gridcraft application. Its first integrated delivery is **Solvercraft alpha**. A feature is supported only when its implementation and relevant acceptance tests exist. The masterplan's R1, R2 and Frontline-parity labels are broader gates, not synonyms for the alpha.

## Release vocabulary

| Label | Required evidence |
| --- | --- |
| Alpha | Explicitly listed implemented features, independent numerical checks, bounded inputs and honest unsupported-feature errors |
| R1 optimization | Model/compiler support, validated LP/MILP and each advertised QP/SOCP/NLP/heuristic class, capabilities, jobs/cancellation, workbook integration, migration, reports and platform checks |
| R2 simulation | The frozen distribution list, sampling, dependencies, statistics, fitting, plots and end-to-end host execution |
| R3 risk optimization | Independent validation scenarios, stated approximation assumptions, risk measures and correct information timing |
| R4 advanced features | Individually verified discrete/global methods, decision tools and compatibility adapters |
| Frontline parity | A versioned function-by-function reference inventory with implementation, tests, platform status and explicit remaining differences |

The broader masterplan contains 26 tasks and 96 requirements. An implemented subset must retain the remaining requirements as open. No feature count or speed comparison is inferred from a marketing page.

## Current data-contract boundary

The shared contract is in [types/src/lib.rs](../crates/types/src/lib.rs). The following are schema observations, not assertions that every represented feature is implemented.

| Area | Represented in the initial schema | Additional gate before broader claims |
| --- | --- | --- |
| Optimization model | Continuous/integer/binary variables, optional finite lower/upper bounds, linear constraints and a linear/quadratic objective | General expression/evaluator semantics, nonlinear constraints, conic constraints and target-value goals |
| Objective convention | Each quadratic term is coefficient times the two named variables; no implicit half factor | Canonical duplicate/cross-term handling and conversion tests for any external backend |
| Methods | Auto, simplex, branch and bound, projected gradient and differential evolution | A capability registry and validation of each method/problem pairing |
| Result | Termination, feasibility and optimality fields, optional solution/objective/bound/gap, counters and warnings; optional SolveRun manifest wrapper | Workbook revision/run identity, compiler/backend-build identity and complete host reproducibility metadata |
| Simulation inputs | Constant, uniform, normal, lognormal, triangular, Bernoulli, binomial, Poisson, exponential, Weibull and empirical | The full 17-family R2 baseline and the later frozen reference catalog |
| Simulation output | Completed-trial count, summaries, optional retained samples and sampling version | Explicit invalid-trial policy/counts and a full run manifest |
| Risk summaries | Named 95% VaR/CVaR fields | Stated loss convention, quantile definition and correct mass handling at a discrete quantile |
| Control | Cancellation query and progress callback | Verified polling within every expensive path; nonblocking host job management |

For distribution parameters, normal standard deviation differs from variance; lognormal parameters describe the underlying normal; exponential uses a rate; Weibull uses shape and scale; triangular order is low, mode, high. Empirical weighting/interpolation is a separate capability and must not be assumed from the enum name.

### Result semantics

- `termination` explains why computation ended. A budget stop or cancellation does not erase a valid incumbent.
- `feasibility = feasible` requires original-model verification within stated tolerances.
- `feasibility = infeasible` is reserved for a justified model-infeasibility conclusion. A failed local search or a violating candidate is insufficient.
- `optimality = optimal` requires a justified bound/termination condition, not merely a stationary point or low constraint violation.
- `stationary` does not guarantee a local minimum; nonconvex objectives can have saddle points or maxima.
- `unbounded` is not inferred solely from a large objective or iterate magnitude.
- Bound and relative-gap fields stay absent when no valid mathematical bound exists.

Bounds use absent sides rather than JSON Infinity. All external numeric inputs and all emitted numeric results require finite-value validation. A successful serialization call alone is not validation: nonfinite floating-point values can otherwise become invalid or misleading boundary data.

Seeds and 64-bit identities need a lossless external representation. SolveOptions and SimulationOptions now emit their seeds as decimal JSON strings through a shared serializer; the decoder also accepts native JSON integer input for compatibility. Native Rust values remain u64. Round-trip tests must include values above 2^53 and the maximum supported seed. Other identities require their own lossless boundary checks.

## Initial implementation paths and limits

These paths describe the implemented alpha components present in source. Each still requires its relevant numerical, resource and platform evidence before a release claim. General algorithms do not become supported merely because a shared enum can name them.

| Path | Implemented class | Guarantee boundary |
| --- | --- | --- |
| engines LP/MILP | Sparse model coefficients converted into a bounded dense two-phase simplex; integer search through branch and bound | Not a mature sparse internal backend; no exposed basis/duals/sensitivity or exact rational certificate |
| engines/separable_qp.rs | Analytic continuous separable convex minimization, or concave maximization, over bounds | No cross-variable quadratic terms, general constraints or integer domains |
| compile expression API | Structural affine/quadratic classification, expression evaluation and reverse-mode derivatives for supported operators | Not a complete Excel/formula compiler; unknown/domain/nonsmooth cases remain explicit |
| solvercraft/nonlinear.rs | Bound-only continuous projected-gradient search using AD, scaled coordinates and Armijo/Barzilai-Borwein steps | Strongest success is Stationary; not GRG and not a local/global minimum certificate; general constraints rejected |
| solvercraft/heuristic.rs | Deterministic differential evolution on finite variable bounds, including expression constraints and integer repair | Heuristic best feasible point only; failure to find one is not an infeasibility proof and no global bound is supplied |
| solvercraft/risk.rs | Weighted finite-scenario expected loss/CVaR linear formulations; symmetric independent coefficient-box counterparts for selected <= rows | In-sample finite-scenario calculation, not out-of-sample validation, chance guarantees, arbitrary robust sets or multi-stage recourse |
| solvercraft/jobs.rs | Bounded native solve/simulation workers, cancellation/progress and shared completed results | WASM worker integration remains separate; direct callback APIs do not establish a responsive host interface |
| solvercraft/provenance.rs | Bounded streaming diagnostic model fingerprint with options, package version and platform | FNV-1a fingerprint is diagnostic, not authentication or a workbook revision/conflict check |

The local and differential-evolution APIs operate on ExpressionModel separately from the structured OptimizationModel solver. They must not be described as a universal Auto dispatch for every represented model. The bound-only smooth method explicitly excludes general constraints; the constrained DE method does not replace a general constrained smooth NLP solver.

Finite-scenario risk transformations preflight the derived variables, rows, terms and memory estimate before copying or expanding a model. Weighted arithmetic uses compensated accumulation, and discrete CVaR handles partial mass at the quantile boundary. Original-model/result consistency and numerical-range regression tests remain required even when the mathematical reformulation is exact.

## Actual dependency baseline

The initial nested workspace declares only local Solvercraft crates plus `serde = 1.0.228` and `serde_json = 1.0.149`. Computation otherwise uses Rust's standard library. Dependency versions here describe the initial workspace manifest; the lockfile is authoritative for the resolved build.

External solver/statistics candidates below are **not adopted engines**. The integrator reported unavailable DNS for new Git/Cargo downloads and no cached microlp or Clarabel source. The implementation therefore starts with a bounded in-tree Rust baseline informed by the existing Gridcraft LP implementation. It must not attribute that baseline's behavior, scale or correctness guarantees to an external library.

The available toolchain reported by the integrator is Rust 1.91.1, while the selected Gridcraft workspace declares Rust 1.95. The nested workspace declares Rust 1.86. Native core tests do not imply that the full Gridcraft workspace was built or that WASM executed.

## Candidate dependencies and evaluation constraints

The version observations below come from primary project documentation consulted on 2026-10-31. Web documentation and local package resolution can differ; inspect actual resolved source and pin it before adoption.

| Candidate | Verified documentation observation | Intended use and restriction |
| --- | --- | --- |
| microlp 0.6.0 | Rust/WASM LP and integer/binary modeling; options include time limit, node limit, MIP gap, integrality tolerance, warm start and tolerances | Possible sparse LP/MILP adapter; numerical behavior and interruption must be tested first |
| microlp features | No default-enabled feature; optional highs feature | Keep highs off in the standard pure-Rust build |
| Clarabel 0.11.1 | Rust convex quadratic/conic solver with CSC matrix input; default feature is serde | Possible QP/SOCP adapter; do not imply integer or nonconvex-QP support |
| Clarabel optional features | SDP, Python, Julia, Pardiso and BLAS-related paths exist | Exclude native-library paths from the default Rust/WASM closure |
| rand_chacha 0.10.0 | Portable deterministic Rust RNG; stream identifiers and word-position controls | Candidate for versioned, schedule-independent streams; explicit seeding rather than OS entropy |
| statrs 0.19.1 | Distribution functions and optional undefined moments; rand and nalgebra are optional features | Candidate for CDF/quantile/reference work; inspect minimal feature closure before use |
| HiGHS | MIT-licensed C++ LP/MIP/QP engine | Optional independent comparison program or explicitly external engine; never label it pure Rust |

Microlp's options and statistics are non-exhaustive structs. Use their constructors/defaults rather than relying on a complete struct literal. Its documented node limit does not count the root relaxation. A node budget alone therefore cannot establish responsive interruption of a large root LP. Its public documentation does not establish a dual/basis/sensitivity or cancellation-callback API.

Different documentation observations have reported microlp 0.6.0 and 0.7.0 under unversioned latest links. This is unresolved provenance, not a reason to assume either API without resolving the actual package.

## License and provenance inventory

| Component/source | Current disposition | Before redistribution/adoption |
| --- | --- | --- |
| Solvercraft and existing Gridcraft code | Workspace license is MIT OR Apache-2.0 | Preserve upstream notices and document any copied/extracted implementation |
| serde and serde_json | Only external dependency families in the initial manifest | Retain their resolved license/notice metadata and audit transitive crates |
| microlp | Candidate; project identifies Apache-2.0 | Inspect exact version, source, feature closure, notices and numerical limitations |
| Clarabel | Candidate; project identifies Apache-2.0 | Inspect exact version and native/transitive feature implications |
| HiGHS | Candidate comparison/external engine; project identifies MIT | Keep C++ dependency and optional deployment separate from the Rust core |
| RNG/distribution candidates | Not part of the adopted baseline | Verify exact Cargo license metadata and all algorithm/table provenance |
| Frontline material | Behavioral reference only | Link sources; do not bundle proprietary source, assets or copied handbooks |

This is a dependency/provenance inventory, not a completed transitive license audit. No commercial engine is bundled, and no proprietary Frontline source is used. New tables, example workbooks and assets need their own provenance checks under Gridcraft's rules.

## Open requirements before R1 or parity

The initial components do not establish a complete spreadsheet-expression compiler, SOCP representation, a mature sparse internal LP/MIP backend, target-value goals, arbitrary constrained smooth NLP, complete derivative coverage, or global MINLP guarantees. Structural analysis, a restricted bound-only AD solver and finite-bound constrained DE are narrower supported paths, not those broader claims.

Full R1 additionally requires actual Gridcraft panel/commands, snapshots with deterministic volatile-function policy, background jobs, bounded cancellation, revision-aware application, undo/redo, XLSX model persistence, structure/reference migration, legacy solver-name import/export, appropriate reports, and the full repository/platform quality gates. UI access, workbook behavior and WASM runtime behavior each need their own executed evidence.

R2 additionally requires the complete selected distribution inventory; correlation/copula semantics; fitting and sampling-aware uncertainty; output charts; stream consistency; and bounded statistics/storage. Retained exact samples, exact quantiles and approximate sketches are distinct capabilities. R3/R4 and full PSI/Frontline compatibility remain separate work.

## Primary sources

- Gridcraft revision and agent rules: https://github.com/storytold/gridcraft/tree/981fad14e0b90a7906a02f29785a6d0eb214fee6
- Frontline product overview: https://www.solver.com/products-overview
- Frontline nonlinear method descriptions: https://www.solver.com/excel-solver-nonlinear-optimization
- Frontline sampling: https://www.solver.com/simulation-random-number-generators
- Frontline copulas: https://www.solver.com/risk-solver-help/psi-function-help/simulation/property-functions/psicopula
- Frontline PSI triangular signature: https://www.solver.com/risk-solver-help/psi-function-help/simulation/distributions/psiTriangular
- Microlp versioned API: https://docs.rs/microlp/0.6.0/microlp/
- Microlp options: https://docs.rs/microlp/0.6.0/microlp/struct.SolveOptions.html
- Microlp statistics: https://docs.rs/microlp/0.6.0/microlp/struct.Stats.html
- Microlp features: https://docs.rs/crate/microlp/0.6.0/features
- Clarabel Rust formulation: https://clarabel.org/stable/rust/getting_started_rs/
- Clarabel feature tree: https://docs.rs/crate/clarabel/0.11.1/features
- ChaCha RNG: https://docs.rs/rand_chacha/0.10.0/rand_chacha/
- Statistical functions: https://docs.rs/statrs/0.19.1/statrs/

The masterplan's source registry records which Frontline guides were opened, found only through search, or not fully retrieved. No function-complete Frontline reference inventory has yet been verified.

## Revision history

| Date | Change |
| --- | --- |
| 2026-10-31 | Initial alpha scope, contract review, candidate-dependency observations and outstanding parity gates |
| 2026-10-31 | Record lossless seed codec, restricted QP/AD/DE paths, finite-scenario risk, native jobs and diagnostic manifests; retain general NLP and integration gates |

