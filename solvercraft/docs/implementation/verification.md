# Reproducible verification

Final candidate 5 passed the complete host CI, the separate SDK gates, actual
Chrome/WASM tests, native UI interactions and independent numerical oracles.
These results establish the stated test cases. They do not establish all 96
requirements or Frontline feature parity; see
[the current acceptance matrix](../qa/acceptance-current.md) and the immutable
[original requirements](../masterplan/requirements.csv).

## Tested source and tools

The baseline is Gridcraft commit `95f7b46ff94fe833b2f3356db58dc6754916f43c`,
with the reviewed Solvercraft changes. The frozen auxiliary Git checkout was
copied with hash verification: 1,056 files, 24,572,232 bytes, all-file raw digest
`e75ea4fcbbf6a621cc22b1bd284a72204858d386b96010f02e192b4e494451b5`.
[The snapshot proof](frozen-final-source-report.json) records an unchanged source
while copying and a matching destination. Reports and prose were added later;
the tested code and compiled reference inputs remained frozen.

The host/SDK verifier identity covers 789 source/build/data files, including literal
`include_str!`/`include_bytes!` inputs. All host/SDK gates retained raw source digest
`fcc9d93350d3c5d3342b02c775096867d01c0e7addfdc9099ac0160cb460ecfe`.
The LF-canonical digest is
`9188b27559e9f23fabe2faa8c353c2e5fdca1bc6ab113955375ccd46111e88bb`.
An independent post-run comparison found all 789 active and frozen file hashes
identical. Raw hashes bind Windows worktree bytes; canonical hashes allow
comparison to LF Git blobs without changing the tested files. Immutable datasets
have explicit `-text` rules and must match their raw hashes, including
`exact_products.txt` SHA-256
`322362fe5fcc15e3a8bde984cbb90e22cded6d49949ee80ffa5d9761e2d0e491`.

Actual tools: Rust/Cargo 1.95.0, Rustfmt 1.9.0, Clippy 0.1.95,
`wasm-bindgen-test-runner` 0.2.130, Trunk 0.21.14, Chrome 156.0.8078.74 and
ChromeDriver 156.0.8078.80. Both native MSVC and `wasm32-unknown-unknown`
targets executed. Exact tool paths/versions and hashes are retained in the
machine-readable reports. Rustfmt and Clippy resolve to the same 1.95 toolchain;
the declared and tested SDK MSRV is 1.95.

## Executed gates

| Gate | Actual result | Evidence |
|---|---|---|
| Host `cargo xtask ci` | All 10 stages passed; 2,827 tests passed, 0 failed, 1 existing ignored test | [Summary](gates-final/host/summary.json), [full log](gates-final/host/host-ci.txt) |
| SDK fmt, Clippy, workspace tests | All passed; 630 tests passed, 0 failed, 0 ignored | [Summary](gates-final/sdk/summary.json), [full tests](gates-final/sdk/sdk-tests.txt) |
| Native Solvercraft interactions | 6 test bodies passed, including EN/DE by light/dark keyboard/focus matrix and all five selectable charts plus Tornado | [UI report](native-ui-controls-report.json), [full log](gates-final/host/host-ui.txt) |
| Chart-module tests | 8 numerical chart contracts plus 1 retained-result lifecycle test passed | [Full log](gates-final/host/host-charts.txt) |
| Actual Chrome SDK/WASM | 13 test bodies passed | [Report](browser-smoke-final-report.json) |
| Actual product worker and Trunk app | 7 test bodies passed against hash-matched, accepted production assets | [Report](browser-worker-final-report.json), [Trunk build](trunk-final-build-report.json), [test build](worker-browser-final-build-report.json) |

The host stages are `i18n-report`, `fmt`, `clippy`, `test`, `doctest`, `assets`,
`layers`, `wasm`, `parity`, and `keytips`. Full CI exited 0 in 722.25 seconds,
including compilation. The test counts sum successful test-target summaries;
the additional scoped UI/chart reruns are not added to that total. The existing
ignored test is `recalc_with_range_formulas_scales_linearly`, documented upstream
as timing-sensitive and intended for release execution. No doctests were skipped
because of a failing build.

Native GUI tests use the project's existing font pin
`b076417b5880bd4154bd22f00695b1d28c6bea83`: 13 allowed OFL font faces,
43,390,680 bytes, with hashes and licenses retained. Disallowed font families
were not downloaded. Required font inputs and task-local writable temp paths
are recorded in the gate identity.

The strict cross-platform `IMSQRT(i)` regression compares the parsed components
with analytic `1/sqrt(2)` within `2e-15`; exact representable formatting cases
remain exact. This test-only correction addresses the Windows final-decimal
trigonometric difference. Async host fixtures also serialize their seven
process-local job-quota consumers and await terminal cleanup. Production quotas
and numerical semantics were not relaxed to make those tests pass.

## Browser responsiveness and native controls

The actual worker executes serialized workbook snapshots through the production
Session command API. Tests cover preview/apply/Undo, stale and cross-document
rejection, hard cancel/deadline with no invented incumbent, ordered Calc rounding,
mixed-integer QP, and the exact scaled binary regression. The actual Trunk canvas
loaded in 1,382 ms; its German dark-theme Solvercraft pane was opened through
pointer input and visually inspected. The screenshot SHA-256 is
`dc4c740aa6bd43b16439f81b9d2e7180d3a481f5d6dc295754f8aded1b12d4fa`.
The raw PNG is retained as test evidence, not a product asset.

For the documented 2-million-trial streaming simulation, admission took 1.15 ms,
cold worker startup 36.445 ms and computation 2,320.055 ms. There were 47 distinct
visible progress updates; the **maximum** observed active progress gap was 65 ms,
maximum window heartbeat gap 22 ms, and maximum status-query duration 1 ms.
These are observed maxima, with polling resolution and a shared host, not
hard real-time guarantees. Cold startup is separate from the admission budget.

Native tests also exercise saved-model controls, explicit single-step application,
stale-result disabling, focus, Tab/Shift+Tab, draft typing and Ctrl+Enter in both
languages and themes. Eight renderer-used numerical tests independently check
histogram bin boundaries, duplicate CDF mass, same-trial scatter pairing,
full-run Box/quantile summaries, single-factor Tornado ranking and very small,
very large, adjacent and overflowing numeric ranges.

The official WASM test runner itself adds `no-sandbox` and
`disable-dev-shm-usage`. The actual browser runs retain the outer managed OS
boundary; they do not prove Chrome's own sandbox compatibility. A separate
normal-sandbox preview failed at startup. No manual browser security flag was
added. A test-only restoration of the native Worker constructor avoids the
runner's console-forwarding blob changing Trunk's relative loader resolution;
the product worker and generated Trunk loader remain unchanged.

## Independent numerical oracles

All cases below execute the same final release CLI, SHA-256
`68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`.
[The release build proof](release-final-build-report.json) binds it to unchanged
source. The deterministic generated LP/QP/robust suites use seed `20261031` and
NumPy 2.2.0; they do not call another Solvercraft backend as their oracle.

| Oracle | Result and acceptance |
|---|---|
| [30 LP cases](linear-oracle-final-report.json) | Independent active-constraint vertex enumeration; min/max, equalities, bounds, negative ranges and row scaling |
| [20 convex QPs](quadratic-oracle-final-report.json) | Independent active-set KKT systems; maximum original objective error `1.154e-8`, maximum original constraint violation `4.071e-10` |
| [20 robust models plus 4 tiny infeasible cases](robust-oracle-final-report.json) | Explicit uncertainty vertices and original-space LP enumeration; maximum objective error `1.244e-14`, support violation `8.438e-15` |
| [Exact binary enumeration](exact-binary-oracle-final-report.json) | All four binary points checked with integer arithmetic; optimum `(0,1)`/500, offset `1e16+500`, reversed minimization -500; one-node control reports an open bound and no false optimality |

The scaled binary case rejects the former false-optimal zero solution caused by
treating a near-integer relaxation as a closed node. The robust tiny-variable
cases reject the former false feasible point at `x=+/-1e-9` with radius `1e12`:
the original support is 1,000. Both regressions are also executable CLI tests;
the binary variants execute inside the real SDK WASM and product-worker tests.

Two independent Johnson tests additionally use six SU/SB fixtures generated by
128/256-node NumPy Gauss-Hermite quadrature, including near-normal shapes,
reflected skewness, heavy tails and large affine means. These finite examples
do not establish rigorous quadrature bounds for every possible shape.

## Bounded release-process measurements

[All ten samples](performance-final-report.json) preserve requests, seed 42,
effective engine/sampler identities, errors, source/binary hashes and memory.
Each measured sample starts a fresh process; startup, JSON and computation are
included. No warmup was discarded. These samples ran after the owned builds
finished while the native CI test executables were running. This is a shared-host
baseline, not an isolated speed comparison or a claim against another product.

| Workload, five repetitions each | Median wall time | Maximum observed OS peak working set |
|---|---|---|
| LP, 100 variables and 500 rows; analytic optimum 3,775 | 260.20 ms | 11,759,616 bytes |
| Streaming MC, 100,000 trials, 20 inputs, 10 outputs; analytic mean 1 and variance 1/6 | 712.77 ms | 6,799,360 bytes |

The host is Windows 11 x86_64, 16 physical/24 logical CPUs and 68,442,624,000
bytes RAM. Memory combines 5 ms RSS sampling with the OS peak working set; this
is not an allocation-level memory proof. Statistical checks use the documented
fixed seed and thresholds, rather than claiming exact Monte Carlo moments.

## Reproduction and limits

Use Rust 1.95 and prepend its toolchain directory to PATH; set RUSTC, RUSTDOC and
RUSTFMT to that same toolchain. Use task-local Cargo/toolchain/temp/target paths,
the pinned permitted fonts, `CRAFT_FONTS_REQUIRED=1`, and
`CARGO_NET_GIT_FETCH_WITH_CLI=true` where the Windows libgit2 revocation check
fails. TLS verification is retained. From the nested `solvercraft` directory:

```powershell
python tests/e2e/verify.py --gate sdk-fmt --gate sdk-clippy --gate sdk-tests
python tests/e2e/verify.py --gate host-ci --gate host-ui --gate host-charts
python tests/e2e/linear_oracle.py $SolvercraftCli --report lp.json
python tests/e2e/quadratic_oracle.py $SolvercraftCli --output qp.json
python tests/e2e/robust_oracle.py $SolvercraftCli --report robust.json
python tests/e2e/performance.py $SolvercraftCli --repeat 5 --output performance.json --work-dir $TaskTemp
python tests/e2e/license_inventory.py --output dependency-inventory.json
```

`run_wasm_browser.py --help`, `build_trunk.py --help` and
`build_worker_browser.py --help` expose the actual-browser commands. The isolated
harnesses have their own locked manifests; the runner requires matching official
wasm-bindgen and Chrome/ChromeDriver versions. Generated production assets must
match an accepted unchanged-source Trunk build before worker tests run.
[SDK dependency metadata](dependency-inventory.json) covers 56 resolved packages;
[worker-harness metadata](worker-browser-dependency-inventory.json) covers 141,
with features, SPDX fields and license/NOTICE hashes. The licensed Sobol direction
data and BSL-1.0 text are recorded separately.

Reports distinguish floating-point optimal bounds from local stationarity and
limited/cancelled runs. This finite verification does not establish correctness
for every ill-conditioned model, global nonlinear optimality, untested operating
systems, or every original requirement. R046's two ambiguous legacy constructor
arities and R081's proprietary vendor-import scope remain separately documented
acceptance gaps. No proprietary algorithm source was assumed available.
