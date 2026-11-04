# Historical initial Solvercraft deep review, 31 October 2026

This page preserves the **earlier 265-test checkpoint** and its then-open environment/feature gaps. Its counts and unexecuted-host statements are historical. Current implementations, actual host/browser runs and remaining acceptance work are recorded in [acceptance-current.md](acceptance-current.md), the [SDK README](../../README.md) and [current verification guide](../implementation/verification.md). Later source changes need their own source-bound gates; historical results are not silently reassigned.

The original alpha was not ready for merge approval. Independent source review and executed counterexamples exposed incorrect numerical answers, false optimality claims and permissive input handling. This review corrected those failures and added regression coverage. It does not certify general numerical correctness or legacy PSI feature parity.

The reviewed scope is Solvercraft's eight Rust crates, CLI boundary, Gridcraft adapter, UI wiring and layering changes. It is not a review of every pre-existing Gridcraft feature. Review began at PR predecessor `690a098a6c4c9d966d66b6c30c036e0e42534bfd`, on Gridcraft base `981fad14e0b90a7906a02f29785a6d0eb214fee6`. The source inventory and final PR commit bind the results to the reviewed implementation.

## Executed evidence

| Gate | Result | What it proves |
| --- | --- | --- |
| Standalone workspace tests | **265 passed, 0 failed, 0 ignored** | Native Windows SDK and CLI suites, including 129 tests added since the 136-test predecessor |
| Actual CLI process E2E | **40 passed; included in 265** | Commands, stdin/files, malformed input, errors, original-expression consistency, optimization, simulation and risk |
| Independent NumPy oracle | **30/30 release CLI LP comparisons** | Synthetic bounded 2D/3D LP optima and feasibility checked by separate active-constraint enumeration |
| Exact small-model oracles | Included in 265 | 300 rational LP references and 384 mixed-integer/node-budget runs inside engine tests; not 684 additional Rust tests |
| Runtime controls | Included in 265 | Four-worker quota, cancellation, panic reporting, shared result identity and worker-slot reuse |
| Formatting and Clippy | Passed | Entire standalone workspace, all targets, warnings denied |
| Release build and examples | Passed | LP objective 36, 10,000-trial simulation, CVaR, smooth nonlinear stationary point and differential evolution examples |
| Linux target | Cargo check passed | Compilation/type checking only; no Linux execution |

See [verification-report.json](verification-report.json), [source-inventory.json](source-inventory.json), [release examples](release-examples.json), [release oracle report](linear-oracle-release-report.json) and the checked-in `.log` files. Source hashes normalize text to UTF-8/LF for comparison with GitHub files; the executable hash uses raw bytes. The predecessor's `docs/verification/report.json` remains historical. The separate [CLI reviewer report](linear-oracle-report.json) records a debug executable; the release oracle report records the executable used by consolidated verification.

## Reproduced defects and disposition

| Area | Counterexample or consequence | Disposition |
| --- | --- | --- |
| Linear optimization | `min -1e-9*x`, `0 <= x <= 1e12`, incorrectly returned zero as optimal instead of -1000 | Objective normalization and reduced-cost handling corrected; regression passed |
| Shared arithmetic/compiler/AD | `1e16 + 1 - 1e16` lost its representable residual; compilation and gradients could erase an improvement or constraint | Compensated accumulation and original-expression guards added; regressions passed |
| Simplex model translation | Fixed-variable offsets and RHS cancellation changed feasible regions | Aggregate residuals together; revalidate reconstructed coordinates; regressions passed |
| Mixed-integer optimization | Adding objective constant 1e16 changed the optimal integer point | Constant-free internal pruning/comparison and conservative public bounds/gaps; regressions passed |
| Integer precision | Adjacent integers beyond f64 exact precision could receive an incorrect certificate | Explicitly reject unsupported integer domains/candidates at absolute value >= 2^53 |
| Distributions | Valid finite Normal, Weibull and Triangular probabilities/quantiles overflowed, underflowed or became zero | Stable formulas and explicit numerical-range failures; deterministic regressions passed |
| Statistics and fitting | Variance of `[1e16, 1e16+2]` depended on observation order; uniform/exponential fitting unnecessarily overflowed variance | Centered/scaled compensated moments and family-specific MLE paths; regressions passed |
| Weighted risk | Unequal-weight CVaR and extreme finite weight scales produced wrong answers or range errors | Scaled compensated means/tail masses and original-risk objective checks; Decimal/rational controls passed |
| CLI/schema | Unknown commands exited successfully; misspelled options/fields and positional request arrays were accepted; malformed native argv could panic | Strict object/field/argument validation and structured errors; process regressions passed |
| Resources | Streaming simulation could evaluate an excessive output workload despite bounded retention | Explicit output-work cap; regression passed |
| Gridcraft adapter | Explicit negative bounds were narrowed by the implicit default; spill children were treated as constants/write targets; MIN/MAX blank references changed meaning | Source corrected or unsupported cases explicitly rejected; **13 new host regressions written but not executed** |

The RNG change is an advisory hardening rather than a demonstrated failure inside current simulator bounds: v1's XOR addressing admitted a constructed collision family at very large public binding addresses. Ordered domain mixing is now versioned **`solvercraft-addressed-mix64-v2`**, with fixed reproducibility vectors. Old seeds intentionally change output. No cryptographic or external RNG battery certification is claimed.

Detailed evidence: [optimization](optimization-review.md), [compiler/model/AD](compiler-review.md), [simulation/math](simulation-review.md), [CLI](e2e-review.md), [risk/runtime](runtime-review.md), [Gridcraft](gridcraft-review.md).

## Functional coverage and blocking gates

The [masterplan matrix](coverage-matrix.md) classifies all 96 requirements: **17 implemented and natively tested, 40 partial, 33 absent, 6 untestable in the full host**. These categories are not a readiness percentage. General constrained QP/SOCP, complete constrained smooth NLP, certified global optimization, copulas, Sobol sampling, recourse, decision trees and workbook model persistence remain substantial open work.

The full Gridcraft checkout is incomplete locally; the host probe stops at missing `crates/core/Cargo.toml`. The installed Rust is 1.91.1 while Gridcraft requires 1.95, and the exact host dependencies are not cached. No full host build, 17 host test executions, actual workbook/UI end-to-end run, or WASM/browser run passed. The parent workspace excludes the nested SDK, so its current CI is not evidence that these 265 tests ran remotely. The PR remains a draft and carries no merge approval.

## Repeat the executed checks

From `solvercraft/`, with the locked dependencies cached:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked --offline --no-fail-fast
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --release --locked --offline -p solvercraft-cli
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu --locked --offline
python tests/e2e/linear_oracle.py /absolute/path/to/solvercraft-cli --report oracle-report.json
```

The Python oracle requires NumPy (tested with Python 3.12.6 and NumPy 2.2.0). Its random generation seed is recorded. A test count is not a performance, concurrency-stress, arbitrary ill-conditioning or feature-completeness guarantee.
