# CLI end to end and independent differential review

Review target: the Solvercraft alpha source baseline identified by the integrator as PR commit 690a098a6c4c9d966d66b6c30c036e0e42534bfd, with the local fixes described below. This report covers the standalone executable, not Gridcraft's UI or workbook integration.

## Executed process tests

The integration tests in crates/cli/tests/end_to_end.rs spawn the actual Cargo-built solvercraft-cli executable through std::process::Command. They do not call CLI dispatch functions in-process.

Executed on native Windows with Rust 1.91.1:

    CARGO_TARGET_DIR=$BUILD_OUTPUT\validation
    CARGO_INCREMENTAL=0
    cargo test -p solvercraft-cli --test end_to_end --offline --locked -- --test-threads=4
    cargo clippy -p solvercraft-cli --all-targets --offline --locked -- -D warnings

The completed post-fix run, rebuilt after the numerical owners froze production and the addressed sampling version changed to v2, passed **40 tests, zero failures, zero ignored**. CLI all-targets Clippy with -D warnings also passed. Each process is guarded by a 20-second watchdog and a 4 MiB output-capture limit. The largest input test is 16 MiB plus one byte; no memory-exhaustion test was attempted.

The tests cover:

- default help, help/--help/-h, capabilities and the generated LP example;
- validate, compile, solve and solve-expr through the actual process;
- named file input, omitted-path stdin and explicit '-' stdin;
- smooth projected-gradient and differential-evolution dispatch, including truthful stationarity versus best-found statuses;
- multiple simulation outputs, retained samples, seed strings through u64::MAX and reproducibility across separate processes;
- upper-loss-tail risk summaries, expected-loss versus CVaR decisions and invalid confidence levels;
- demo execution with actual optimization and 10000 simulation trials;
- malformed/empty/scalar/array/trailing JSON, invalid methods/seeds/variables and missing files;
- exactly 16 MiB accepted and 16 MiB plus one byte rejected before model solving;
- unknown request/model/option/expression/distribution/risk fields, duplicate budget keys and unexpected command-line arguments;
- evaluation, iteration and node budget results, infeasibility and unboundedness framing;
- original-expression objective and constraint verification before publishing a compiled result;
- a Windows command-line argument containing an unpaired UTF-16 surrogate.

The successful process exit code means that a request was handled. It does not itself mean that a feasible optimum exists: infeasibility and budget interruption can be successful JSON responses with explicit result states. Validation/parse/unsupported/numerical errors instead have nonzero exit status, structured JSON on stderr and no partial success payload on stdout.

## Findings reproduced and fixed

| Finding | Evidence before the fix | Resolution |
|---|---|---|
| Unknown command looked successful | 'solv' printed help and exited 0 | Unknown commands now return unknown_command on stderr and fail; explicit help remains successful |
| Unknown fields silently changed intent | max_evaluatons was ignored and defaulted to 100000; unknown model, expression, distribution and risk fields were accepted | CLI request envelopes and coordinated shared input schemas reject unknown fields |
| Unexpected positional argument interpretation | [model,options] and [model] were accepted as SolveRequest by Serde's sequence visitor | CLI input must start with a JSON object; the original bytes still undergo typed deserialization, preserving duplicate-field detection |
| Non-Unicode argv crashed | An unpaired Windows surrogate caused std::env::args() to panic with exit 101 and non-JSON stderr | Checked args_os/into_string conversion yields structured invalid_arguments; Windows process regression passed |
| Compiled objective disagreed with the source expression | Maximizing (1e16*x+x)-1e16*x over [0,1] published optimal objective 1 at x=1, whereas original floating-point expression evaluation gives 0 | solve-expr checks candidate objective against the original expression before writing any output and refuses the mismatch as numerical_failure |
| Compiled feasibility disagreed with the source expression | With original constraint (1e16*x+x)-1e16*x >= 0.5, the compiled LP proposed x=1 despite original lhs=0 | Every original expression relation is re-evaluated; invalid claimed feasibility is refused as numerical_failure |

Unexpected extra command-line arguments were also rejected rather than ignored. This was identified in source and covered by process regression tests.

Original-expression objective comparison scales by the final reported/evaluated objective magnitudes, not by intermediate expression terms; otherwise catastrophic cancellation could hide a 1-versus-0 mismatch. Constraints use the configured absolute feasibility tolerance. The compiled branch also uses shared bounds/integrality/linear-constraint validation. Successful reported objectives are taken from original expression evaluation.

These checks verify the candidate; they do not prove numerical equivalence of every algebraic transformation at every point of an ill-conditioned model. Unsupported or unstable models may require rescaling or reformulation.

## Independent numerical process oracle

SciPy was not installed. NumPy 2.2.0 was available, so tests/e2e/linear_oracle.py supplies a separate small-LP oracle:

    python tests/e2e/linear_oracle.py <path-to-solvercraft-cli> --cases 30 --report docs/qa/linear-oracle-report.json

The oracle enumerates all candidate active-constraint vertices using NumPy linear solves. It shares neither simplex code nor model validation code with Solvercraft. Thirty fixed-seed bounded continuous LPs in two or three variables cover signed costs/bounds, minimization/maximization, equality planes, redundant/degenerate active sets and row scaling between 1e-2 and 1e2.

The final comparison against that same rebuilt CLI passed **30 out of 30 cases**. It independently checks original constraints at absolute tolerance 1e-7, finite decision values, the published objective against the returned values, and optimum objective agreement. Alternative optimal decision vectors are permitted.

The JSON report records Python/NumPy/platform versions, seed, execution time, each case's objective, Solvercraft/sampling versions and the executable SHA256. The recorded sampling version is solvercraft-addressed-mix64-v2. This identifies the tested executable; it is not a general solver-correctness or performance certificate. The oracle must be rerun if the executable is rebuilt from later numerical fixes.

## Limits and remaining verification

- Full Gridcraft UI, command integration, workbook revisions/undo and XLSX were not exercised here.
- The Windows invalid-argument regression was executed. A corresponding Unix invalid-byte regression is conditionally included but was not executed on this host.
- Native SDK jobs and cancellation are reviewed separately. The CLI currently has no cancel/job-control protocol; watchdog termination in the test harness is not evidence of graceful CLI cancellation.
- The LP oracle covers small bounded continuous models. It is not an industrial sparse-LP, hard-MIP, QP/SOCP or general nonlinear comparison.
- No web search, external service, third-party benchmark corpus or package installation was needed by these tests.
- Workspace-wide format/Clippy/tests and actual WASM/other-host execution remain integration gates. This review's CLI all-targets Clippy completed successfully after the in-progress dependency lint was fixed.

