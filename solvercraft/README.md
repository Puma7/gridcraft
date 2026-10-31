# Solvercraft alpha

Solvercraft adds a workbook-independent Rust optimization and simulation SDK, a JSON CLI and an initial Gridcraft integration. The existing data.solver command remains available.

## Implemented scope

- Budgeted floating-point LP/MILP with original-model validation. Sparse input coefficients currently feed a dense two-phase tableau.
- Analytic continuous separable convex QP over variable bounds.
- Bound-constrained smooth nonlinear optimization with reverse automatic differentiation, projected steps, Armijo line search and BB updates. Results identify stationarity, not a proof of a local/global minimum.
- Finite-bound differential evolution with nonlinear constraints and integer/binary repair. Results identify the best feasible point found.
- Monte Carlo and Latin Hypercube simulation, addressed deterministic streams, streaming moments and optional exact empirical risk summaries.
- Finite-scenario expected-loss/CVaR optimization, a box-robust linear counterpart, native background SDK jobs and structured cancellation/budget results.

General QP/SOCP, certified global nonlinear optimization, smooth general nonlinear constraints, copulas, Sobol, recourse, decision trees, XLSX model persistence and complete Frontline compatibility remain open.

## Build and run the standalone workspace

From this directory:

~~~sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --release -p solvercraft-cli -- capabilities
cargo run --release -p solvercraft-cli -- solve examples/lp.json
cargo run --release -p solvercraft-cli -- solve-expr examples/nonlinear.json
cargo run --release -p solvercraft-cli -- solve-expr examples/evolution.json
cargo run --release -p solvercraft-cli -- simulate examples/simulation.json
cargo run --release -p solvercraft-cli -- risk examples/risk.json
~~~

Add --offline to Cargo commands when all locked dependencies are cached. CLI input/output is JSON; omit the input path or use - for stdin. Errors are structured JSON on stderr with a nonzero exit code.

## Gridcraft integration

The Data ribbon places Solvercraft immediately after Solver. The initial UI is a modal optimization/simulation dialog with summaries, a retained-sample histogram and explicit result application. Engine commands are solvercraft.capabilities, solvercraft.validate, solvercraft.solve, solvercraft.simulate and solvercraft.result.apply.

Workbook formulas are translated structurally for the documented subset. Candidate values are checked against original expressions and Gridcraft Calc on a private workbook copy. Applying a preview checks document identity/revision and commits the variable changes as one undoable edit.

The Gridcraft commands are currently bounded synchronous calls. Native SDK background jobs do not imply an asynchronous UI. The planned right sidebar and asynchronous host workflow remain separate work.

This nested workspace is excluded from automatic parent membership and is consumed as a path dependency at L4. Its own lockfile supports standalone development; the parent lockfile records the reachable SDK packages against the parent's locked dependencies.

## Validation and remaining gates

The reviewed standalone workspace passed **265 tests**, including **40 real CLI process tests**, with zero failures or ignored tests. Format checking, all-target Clippy with -D warnings, release CLI examples and a Linux-target compile check passed using Rust 1.91.1. An independent NumPy vertex-enumeration oracle also passed 30 LP comparisons against the release executable. See the [deep review and remaining gates](docs/qa/README.md), [96-requirement coverage matrix](docs/qa/coverage-matrix.md), and [current verification report](docs/qa/verification-report.json). The earlier [136-test report](docs/verification/report.json) is historical.

The complete Gridcraft build, 17 host tests (four existing plus 13 added during review), visual UI validation and actual WASM execution have not been run. The development environment lacks Gridcraft's required Rust 1.95 and a complete downloadable dependency/source checkout. The parent CI does not currently execute the excluded standalone workspace's test suite. These are merge gates; the alpha must not be presented as full R1/R2 or Frontline parity.

Sampling now identifies itself as `solvercraft-addressed-mix64-v2`. Seeds intentionally produce different streams from the initial alpha's v1 algorithm; retain the version with reproducibility records. Integer domains at absolute values of 2^53 or above are explicitly unsupported because adjacent integers are not representable in f64.

No proprietary Frontline code, documentation text or vendor artwork is included. Project license: MIT OR Apache-2.0.
