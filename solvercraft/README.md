# Solvercraft

Solvercraft is a workbook-independent Rust optimization, simulation and risk-analysis SDK with a JSON CLI and Gridcraft integration. It is a development alpha. The immutable [96 requirements](docs/masterplan/requirements.csv), [current acceptance review](docs/qa/acceptance-current.md) and [272-function inventory](docs/implementation/reference-implementation.json) describe its scope and remaining gaps. All 96 original criteria have bounded final6 acceptance; this is not a product-completeness percentage or full Frontline/272-PSI compatibility.

## Implemented capabilities

- Dense two-phase LP/MILP, sparse primal-dual LP/QP/SOCP through Clarabel, coupled convex QP, finite-domain MIQP/MISOCP, checked LP basis warm starts and sensitivity, SOS/indicators and finite-domain constraints.
- Structural affine/quadratic extraction, reverse AD, bounded finite differences, constrained augmented-Lagrangian NLP, seeded multistart, differential evolution and a bounded polynomial-box interval certificate. Local and heuristic results remain distinct from global guarantees.
- Monte Carlo, Latin hypercube and randomized Sobol with versioned addressed streams, copulas, distribution modifiers, online moments, empirical risk, likelihood fitting with refit-bootstrap diagnostics, sampling-aware intervals and parameter grids.
- Sample-average training and independent holdout, expected-loss/variance/CVaR objectives, scenario chance constraints, box/budget/ellipsoid counterparts, two-stage recourse, checked Benders, decision trees, information value and multiple objectives.
- Portable versioned model libraries, execution manifests, cooperative deadlines, bounded native jobs and an authenticated, quota-controlled loopback service.

Each capability has finite numerical, dimension and resource limits. Consult the [engine registry](crates/solvercraft/src/registry.rs), the CLI capabilities command and the [implementation documents](docs/implementation/verification.md). A method's existence does not imply unrestricted classes or identical native/browser performance.

## Build and run

Rust **1.95** or newer is required. From this directory:

~~~sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --release -p solvercraft-cli -- capabilities
cargo run --release -p solvercraft-cli -- solve examples/lp.json
cargo run --release -p solvercraft-cli -- solve-expr examples/nonlinear.json
cargo run --release -p solvercraft-cli -- simulate examples/simulation.json
cargo run --release -p solvercraft-cli -- risk examples/risk.json
cargo run -p solvercraft --example artcraft_layout
~~~

Use --offline when the locked dependencies are cached. Requests/responses are JSON; omit the input path or use - for stdin. Errors are structured JSON on stderr with a nonzero exit status. The [SDK/runtime guide](docs/implementation/sdk-runtime.md) describes modern execution, advanced commands, model migration and the optional service.

This nested workspace has its own lockfile and is excluded from parent membership. Gridcraft consumes path dependencies. Run both the SDK checks and parent cargo xtask ci; parent tests do not execute all excluded SDK tests.

## Gridcraft integration

The Data ribbon places Solvercraft immediately after Solver. Its right task pane starts optimization/simulation jobs on immutable snapshots, displays progress/cancellation and offers explicit result application. Native hosts use bounded workers; the web application uses a compiled Rust WebWorker. The legacy data.solver command remains available.

Original worksheet evaluation, document identity/revision, domains, protection and spills guard result application. One accepted result forms one undoable edit. Multiple saved models live in owned XLSX parts and relationships; hidden formula bindings track supported structural edits. SC worksheet inputs/outputs, answer reports, six simulation chart types, parameter runs and verified PSI subsets are documented in the [host guide](docs/implementation/gridcraft.md). Unsupported semantics produce explicit errors.

## Verification

The [final6 verification](docs/implementation/verification-final6.md) and [current acceptance](docs/qa/acceptance-current.md) link source-bound evidence: host CI all ten stages (2873 passed, one existing ignored), SDK 630, actual Chrome SDK 13, native UI 6 and chart module 8 numerical+1 lifecycle. The worker report retains initial 7/1 at an extra heartbeat guard and exactly one identical repeat 8/0; its aggregate all-attempts result is false, with the first cause unproven. The nineteen-signature PSI case passed in both. Original R023 asks for 250 ms normal-model status access, not a 25 ms product guarantee. Final5 and other earlier reports remain historical.

The [scope audit](docs/qa/original-scope-audit.md) distinguishes 111 positively tested mathematical capabilities from the nineteen supported PSI conversion forms and the wider 272-function inventory. Unsupported nesting/properties remain preserved, and the SC nominal outside-value conversion is explicitly lossy.

Published final6 diagnostics are [fixed-field projections and strict test-result excerpts](docs/implementation/public-evidence.md), not complete local stdout/environment/driver dumps or feature trees. Full originals remain private; the [scope audit](docs/qa/original-scope-audit.md) preserves the 111+19 evidence locations. The current binary-oracle projection includes the corrected request metadata after a three-case/one-control recheck on the same binary; historical v1 remains unchanged.

The [reference freeze](docs/implementation/reference-freeze.md) records official versions, hashes and documentation conflicts. The [function matrix](docs/implementation/reference-implementation.json) separates tested subsets from remaining functions/parameters. Unsupported functions do not count as implemented.

Sampling records solvercraft-addressed-mix64-v2; randomized Sobol records its own version and licensed direction table. Preserve engine/sampler versions, inputs, seeds and tolerances. Integer domains at absolute values of 2^53 or above are rejected because adjacent integers are not representable in binary 64.

Project license: MIT OR Apache-2.0. Dependency and numerical-data notices accompany the implementation. No proprietary Frontline implementation or vendor artwork is included.
