# Solvercraft final7 verification

Final7 was executed on 2026-11-01 against the corrected frozen source. The
[summary and complete report map](final7-verification-summary.json) contains
actual counts, hashes and explicit repository-relative evidence paths. Public
reports are fixed-field projections; test excerpts retain original line positions
while replacing unapproved diagnostic lines with blank lines. Complete original
reports remain private. No raw commands, environment, sessions or repeated full
source maps were published.

## Frozen inputs and executed gates

The identity covers **791 source/build/data inputs, including 750 Rust files**.
Raw digest: `8d74eebf83ef7986d0dd5cb302ce159413b49d7e51c5327947a2d8444e505065`.
LF-canonical digest: `dc58726c5be86ddda8e5c6bfffdfe25507a29b66bedd51e50f376bcfbc3ad472`.
All final native/SDK gates kept these before/after digests. Active source inputs
and isolated worker-test source were rechecked against the frozen copy. The
compiled reference matrix is SHA-256
`b71c419a21c70fde8c9f0ac54efc43a16373b47dd567ee00e901e20b01344108`.
Original requirements CSV and reference-catalog inputs remain unchanged.

An earlier run used a preliminary matrix before twelve test-scope descriptions
were corrected from nominal-basis assertions to nominal-value assertions. Those
private preliminary results were retained and were not substituted for the final
coherent gates. Production and test code did not change in that correction.

| Final gate | Passed | Failed | Ignored |
|---|---:|---:|---:|
| Full host CI: all 10 stages | 2904 | 0 | 1 |
| SDK fmt, all-target Clippy and workspace tests | 630 | 0 | 0 |
| Native UI interactions | 6 | 0 | 0 |
| Chart module: eight numeric contracts and one lifecycle test | 9 | 0 | 0 |
| Actual Chrome SDK/WASM | 13 | 0 | 0 |
| Actual product-worker suite, first attempt | 9 | 0 | 0 |

The host ran i18n-report, fmt, Clippy with warnings denied, native tests,
doctests, assets, layers, WASM, parity and keytips. Its corrected warm-cache run
completed in 89.47 seconds. One existing
release-only timing-sensitive scaling test remains explicitly ignored. Scoped
reruns are not counted again in the full-CI total. Rust/Cargo 1.95.0 and the
pinned font inputs were used; verified tool versions are recorded in projections.

## Selected 31-signature PSI import scope

The global run passed **31 distinct native signature tests**, **40 native import
contract test bodies**, and **37 Calc test bodies**. The actual browser exercises
the two disjoint blocks separately:

- Existing block: 19 signatures, 2,048 trials, 19 outputs.
- Additional block: 12 signatures, 4,096 trials, 12 outputs, 24 independent CDF
  anchors. This is not a claim that all 31 signatures ran 4,096 trials.

The new block covers ChiSquare, Erlang, FDist, Logistic, MaxExtreme, MinExtreme,
Rayleigh, Reciprocal, Student, Levy, Kumaraswamy and HypSecant. It passes the raw
Discovery inputs/outputs directly to the real worker, checks declared parameter
conventions and nominal-basis metadata in the captured request and manifest,
and verifies single-edit import/Undo/Redo and an unchanged live workbook.
Student df=1 and Levy use labelled median fallbacks; no finite population-mean
or variance assumption is made for those families.

[The bounded numerical CDF witness](psi-core12-cdf-final7-report.json) preserves
all 24 thresholds, expected probabilities, observed counts and errors. Maximum
observed absolute error was 0.020751953125, below the predeclared threshold
0.054792830314635735. The Hoeffding/union calculation is an ideal-IID sampling
sanity threshold for fixed anchors, not a proof of the PRNG or the entire CDF.
Cauchy, Laplace and HyperGeo remain excluded from this import block because of
unresolved source-contract conflicts. Unsupported properties and nested calls
are preserved; explicit solvercraft_nominal conversion does not establish
legacy PSI display/PreRun parity.

## Worker responsiveness and visual scope

The final worker suite passed on its **first actual attempt** after the owned
builds completed. Observed simulation admission was 0.765 ms,
maximum status-query duration 1 ms, maximum active progress
gap 58 ms and maximum window heartbeat gap
12 ms; there were 29
distinct updates. Cold startup (24.725 ms) and computation
(1450.455 ms) are separate. R023's original status-access
target is 250 ms; the extra 25 ms heartbeat test is not a hard product guarantee.

The actual Trunk app rendered and the German/dark Solvercraft pane was opened
and visually inspected again. Screenshot SHA-256 is
`dc4c740aa6bd43b16439f81b9d2e7180d3a481f5d6dc295754f8aded1b12d4fa`.
Identical pixels reflect unchanged UI and the same synthetic demo state; the new
PSI evidence comes from separate actual Session/Worker assertions. The official
browser-test runner supplies its own no-sandbox/disable-dev-shm-usage arguments.
The outer managed OS boundary remains; Chrome's own sandbox compatibility is
not established. No product worker mock or manual security flag was added.

## Fresh release oracles and measurements

Fresh release identity was measured, not assumed:
`68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`. All independent suites were rerun: 30 LP vertex
cases, 20 QP KKT cases, 20 robust uncertainty-vertex models plus four tiny
infeasible regressions, and three exact binary variants plus a node-limit
control. All passed. The binary harness uses deep-copied immutable request
snapshots and checks them again after the separate control. Both recorded
models and exact outputs are retained in the compact projection.

| Five fresh processes per workload | Median wall time | Maximum OS peak working set |
|---|---:|---:|
| LP, 100 variables / 500 rows, analytic objective 3,775 | 194.54 ms | 11681792 bytes |
| Streaming MC, 100,000 trials / 20 inputs / 10 outputs, seed 42 | 432.79 ms | 6799360 bytes |

Measurements include startup, JSON and calculation, with no discarded warmup.
This is a shared-host observation after owned builds ended, not an isolated
speedup or commercial comparison. Every sample retains its request hash,
effective options, engine/sampler identity, errors, timing and observed memory.
Public dependency inventories expose package/version/SPDX fields; fuller feature
and license/NOTICE diagnostics are retained privately alongside checked-in notices.

## Historical evidence and interpretation

[Final6](final6-verification-summary.json) remains unchanged, including its first
worker attempt 7/1 with the additional 26 ms heartbeat failure and its one
controlled repeat 8/0 at 12 ms. The old binary-oracle mutable-record export error
also remains explicitly superseded by the corrected deepcopy V2 evidence; it
must not be read as a correct saved third-case model. These histories are not
aggregated into final7's first-attempt worker result.

The original 96-row acceptance assessment is separate from finite numerical
cases. Selected imports do not imply all 272 catalog functions or proprietary
engine/runtime parity. Local nonlinear stationarity is distinct from a proved
global bound. See the versioned summary, projection ledger and current scope
audit for exact evidence boundaries.
