# Solvercraft verification: final6

This is the additional frozen-source run of 2026-11-01. Historical final5 gate logs and version-specific reports
remain unchanged; the original shared entrypoints are preserved in the
[final5 publication](https://github.com/Puma7/gridcraft/tree/2d6604429e1101cd30f1140263af212a26bbc607). All paths and counts are indexed explicitly in
[final6-verification-summary.json](final6-verification-summary.json).

The coherent copy contains 1,068 files / 27,906,601 bytes, raw all-file snapshot hash
`fbb64773c50247a43878c495512fc174aad4941e2c25a0c5b51b62eb8ce106e8`.
The host/SDK identity covers 791 source/build/data files,
including 750 Rust files. Raw digest:
`c4a349613fcb95e65a56429acd34c08a7f129102edb346c545854026e11795a7`; LF-canonical digest:
`0ed5c92fd31d137b68115df67e910fd50ff8ef7a145896da86d1578b2eb43d76`. Every gate retained its source
digest. All active identity files and the isolated worker test-source hash match the
frozen copy after execution. The original compiled reference matrices and raw
datasets remain unchanged. All six owner-freeze hashes matched before testing.

## Native and SDK gates

| Gate | Passed | Failed | Ignored |
|---|---:|---:|---:|
| Complete 10-stage host CI | 2873 | 0 | 1 |
| Separate SDK fmt/Clippy/workspace tests | 630 | 0 | 0 |
| Actual native UI interactions | 6 | 0 | 0 |
| Chart module: 8 numeric contracts + 1 lifecycle | 9 | 0 | 0 |
| Actual Chrome SDK/WASM | 13 | 0 | 0 |

The host CI ran i18n-report, fmt, all-target Clippy with warnings denied, native
tests, doctests, assets, layers, WASM, parity and keytips. It exited 0 in
547.64 seconds. One pre-existing timing-sensitive release
scaling test remains explicitly ignored. Scoped UI/chart reruns are not added
again to the full-CI total. Native font inputs use the unchanged project pin.

Actual Rust/Cargo 1.95.0, Rustfmt 1.9.0, Clippy 0.1.95, Chrome 156.0.8078.74,
ChromeDriver 156.0.8078.80, wasm-bindgen 0.2.130 and Trunk 0.21.14 were used.
The public reports retain versions, results and selected source/binary hashes. Full invocation and environment diagnostics remain in local archives; see the [public evidence format](public-evidence.md).

## R081: actual import and worker execution

The native suite contains 19 distinct asymmetric signature tests and 6 policy or
negative groups. The new actual-browser body imports those 19 numeric signatures,
checks declared parameter conventions and outside nominal values, discovers
19 inputs / 19 outputs, and runs 2,048 seed-42 trials in the real product Worker.
Independent analytic means/variances provide bounded sampling sanity checks;
the heavy-tailed LogNorm2 case is not presented as a proof of its whole CDF.

Import preview, explicit apply, single-step Undo/Redo and untouched live-workbook
revision/history all pass. Nested stochastic expressions and documented
`PsiStatic` properties remain atomic unsupported originals. The default
`solvercraft_nominal` policy explicitly reports lossy outside-value conversion;
`preserve_vendor` refuses conversion instead of inventing a vendor display or
PreRun context. `apply_requested` and actual `applied` are checked separately.
This is evidence for the selected 19-signature scope, not all 272 catalog rows or
full proprietary runtime compatibility. The original-scope audit and acceptance
matrix judge requirements separately from these finite cases.

## Worker timing: both attempts remain visible

| Same 8 test bodies, same WASM and production assets | Passed | Failed | Ignored | Maximum window heartbeat gap |
|---|---:|---:|---:|---:|
| Initial attempt during native CI compilation | 7 | 1 | 0 | 26 ms |
| One controlled repeat after owned build/test processes completed | 8 | 0 | 0 | 12.0 ms |

[The composite worker report](browser-worker-final6-report.json) links both raw
attempts. There is no aggregate all-pass claim across both attempts. The first
failure was the additional strict `<25 ms` heartbeat assertion for a 10 ms timer in
`actual_worker_simulation_reports_progress_while_window_timers_remain_responsive`,
at worker.rs:459. Its window includes admission, cold startup, polling,
computation and result retrieval. No phase trace identifies the 26 ms interval;
concurrent compilation is context, while OS/build scheduling is an unproven
possible cause. Initial status/progress maxima were not exported before panic.
No product, test, threshold or security option changed for the single repeat.

R022 concerns job IDs/snapshot revisions and nonblocking admission. R023's
original normal-model status-access target is 250 ms; the additional 25 ms heartbeat
guard is not that original criterion. In the repeat, admission was
0.81 ms, maximum status query 1.0 ms,
maximum observed active progress gap 57.0 ms,
and there were 30 distinct updates. Startup
(25.865 ms) and computation (1480.335 ms)
are separate. These observations do not establish a hard real-time guarantee.

The actual Trunk canvas and German/dark Solvercraft pane were also rendered and
visually inspected. The official test runner adds `no-sandbox` and
`disable-dev-shm-usage` itself; execution retains the outer managed OS boundary,
and Chrome's own sandbox compatibility is not established. No manual disabling
flag or product worker mock was introduced.

## Release numerics and bounded measurements

The SDK's 153 source/manifest files are byte-identical to final5, and the newly
bound release executable therefore retains SHA-256
`68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`.
The release proof binds the existing binary to the new coherent source. All
oracles were actually rerun on 2026-11-01: 30 LP vertex cases, 20 QP KKT cases,
20 robust uncertainty-vertex cases + 4 tiny infeasible regressions, 3 exact binary
variants and 1 node-limit honesty control. They all pass. Large target offsets
are checked with strict representable integer expectations, not a relative
tolerance that hides the 500-unit improvement.

Five fresh release processes per workload were also measured after the owned
gates/builds completed. No warmup was discarded; process startup, JSON and
calculation are included. Shared-host observations do not imply an isolated
performance comparison.

| Workload | Median wall time | Maximum OS peak working set |
|---|---:|---:|
| 100-variable / 500-row LP, analytic optimum 3,775 | 180.81 ms | 11677696 bytes |
| 100,000-trial / 20-input / 10-output streaming MC, seed 42 | 527.52 ms | 6893568 bytes |

Requests, engine/sampler identities, tolerances, errors, seeds, hardware
dimensions, tool versions and memory observations are in the indexed JSON.
Dependency inventories were refreshed separately for 56 SDK and 141 worker-harness
resolved packages, with public package/version/SPDX lists; complete feature and local license/NOTICE metadata remain in the local archive. Reproduce with
the unchanged `tests/e2e` helpers using the pinned 1.95 toolchain and separate task
targets; the two Cargo workspaces and isolated browser manifests have independent
locked gates. Finite acceptance cases do not prove every ill-conditioned model,
global nonlinear optimality or untested platform.

## Original acceptance and public evidence

The current [96-criterion assessment](../qa/acceptance-current.md) preserves the
original CSV fields and distinguishes mathematical kernels, selected PSI imports
and broader vendor compatibility. See the [original-scope audit](../qa/original-scope-audit.md)
for the requirement-by-requirement interpretation and positive evidence.

Published diagnostics follow the [public-evidence policy](public-evidence.md).
Source/binary identities and numerical observations are retained; local paths
and nonessential environment metadata use portable placeholders. The initial
worker heartbeat failure and its one unchanged controlled repeat remain explicit.

## Compact public diagnostics and binary request provenance

Indexed diagnostics use the [fixed-field public evidence format](public-evidence.md),
not complete stdout, environment, invocation or driver dumps. The test logs are
line-stable public result excerpts. Package inventories expose name/version/SPDX;
complete feature trees and local notice-hash metadata remain in private archives.

The [binary-oracle export erratum](binary-oracle-export-erratum.md) supersedes a
historical mutable-request metadata error. Exactly 3+1 cases were rerun with
immutable request records on the unchanged release binary; minimization -500 and
the separate node 1 control now have correctly retained model/option identities.
