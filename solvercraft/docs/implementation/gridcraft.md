# Gridcraft host integration

Implementation evidence, 2026-10-31. This document describes actual host source and
executed host checks, not standalone SDK tests counted as UI evidence. The pinned
requirements remain in `../masterplan/requirements.csv` unchanged.

## Host contracts

`solvercraft.solve`, `solvercraft.simulate` and `solvercraft.validate` remain
bounded synchronous APIs for compatibility. They never open a dialog and never
apply a preview automatically. The right-hand Solvercraft task pane uses
`solvercraft.job.start`, not the synchronous solve command.

`solvercraft.job.start` accepts `{kind:"optimization"|"simulation",request,...}`
or a saved `model_id`, and optional `limits.max_time_millis`. Its response contains
a decimal-string job ID and `{document_uid,revision}` decimal strings. Work runs
on a native SDK worker against captured `Arc<Workbook>` plus a cloned Calc, with
no live Session access. Admission resolves and retains the canonical worksheet
name before worker capture: active-tab view state can mutate through a shared
Workbook Arc without changing its revision. Nested parameter requests inherit
the same binding context, and a deterministic delayed-worker/tab-switch test
checks this case. Status/progress reads clone only a small status record
and result Arc. `job.result` copies the completed JSON preview explicitly.
`job.cancel` requests cooperative cancellation; `job.apply` routes through the
same validated, atomic, undoable `result.apply` transaction. `job.forget` releases
terminal results. Closing a document cancels its jobs and releases terminal
snapshots; active cancelled jobs remain reserved until terminal. Other Sessions'
live documents are never inferred to be orphaned from a local document list.

There are four SDK workers, at most 64 retained host jobs, and a 2 GiB aggregate
reservation limit including a 4x allowance for engine memory/JSON transport.
Default host job engine memory is 64 MiB; explicit budgets are checked against
the aggregate reservation. These are accounting limits, not an operating-system
heap measurement. Wall-clock cancellation is cooperative and cannot interrupt
an arbitrary uncooperative evaluation.

`solvercraft.model.create/get/list/update/delete/import/export` store multiple
named models with monotonic decimal-string IDs. Names use the same Unicode casefold for reads and writes. The bounded schema is version 1, with at most 32 models, a 1 MiB registry
part, 128 KiB request JSON, and 256 KiB binding-formula text per model. Unknown
schema versions, duplicated IDs, private-name collisions and invalidated bindings
fail explicitly.

## Actual XLSX persistence and reference migration

Model metadata is in **`xl/solvercraft/models.json`**, with content type
`application/vnd.solvercraft.workbook-models+json` and a workbook relationship
`https://solvercraft.org/relationships/workbook-models/v1`. The XLSX reader matches
that full URI and the writer preserves the owned `xl/solvercraft/` namespace.
This uses real OPC parts and relationships, not only hidden metadata names.

Objective, scalar decision cells, source constraint expressions, simulation
inputs and output expressions use separate hidden global formula names. Relative
references are qualified and anchored; locally scoped names are resolved while
saving. Existing structural-edit machinery adjusts these formula names for
insertions/deletions and sheet renames. Deleted references cause explicit
`#REF!` binding errors instead of silent retargeting. A copied sheet does not
silently duplicate the workbook's global models. Callers can export/import a
model explicitly. Unsupported structured/dynamic formulas remain unsupported at
validation, rather than being sampled to claim linearity.

`model.importLegacy` reads existing `solver_*` names without deleting them. It
maps only supported settings, identifies projected gradient as distinct from
legacy GRG, and reports unrepresented legacy reports/search tuning. Legacy CSV
variable/reference lists expand before structural translation; typed integer
Simplex imports use actual branch-and-bound, and32-bit legacy seeds remain
lossless. `model.exportLegacy` previews representability by default. Only
`apply:true` writes Solver_* names, as one undo step, while preserving the owned
registry. The explicit subset is same-sheet linear scalar-reference models
with bounds/int/bin rows. Cross-sheet, nonlinear/quadratic/portable/simulation
models and unsupported legacy settings produce reports/errors before writes.

## Evaluation and mathematical routing

Cell references, named references, arithmetic, SUM and SUMPRODUCT are translated
structurally. The host checks shapes, cycles, recursion, ranges and allocations.
It supports multiple sheets and supported smooth operations. No finite-difference
affine probe or cached-value substitution is used to prove linearity. Explicit
DE routes to the actual heuristic. Smooth continuous bound-only NLP routes to the
actual projected-gradient engine. General smooth continuous constraints route to
the actual augmented-Lagrangian engine, with optional derivative settings under
`nlp`. Compiled LP/MILP/QP routes through the owned callback SDK engine API, including
actual finite-domain Auto MIQP in native and browser jobs. Auto fallback is limited to
unsupported mathematical classes; numerical/budget failures are not downgraded.

Every feasible candidate is checked against the original compiler expressions
and original workbook formulas on a private Calc snapshot before preview/apply.
Apply rechecks document identity/revision, protection, spill/formula decisions,
domains, constraints and original worksheet evaluation. All cell writes form
one normal Gridcraft transaction and one undo step. Integer residuals must first
meet the declared tolerance/precision policy; raw candidates and the explicitly
rounded values are both checked against original rows and actual Calc. Preview
`apply_values` records the validated values that apply independently recomputes.

NOW/TODAY use a captured UTC 1900-system serial, retained in the preview request;
1904-workbook conversion follows Gridcraft's date utilities. The new
`gridcraft_calc::evaluate_at` lets original-expression validation use the same
instant. The live workbook resumes its normal clock on ordinary recalculation.
RAND/RANDBETWEEN and unsupported dynamic formulas are explicitly rejected for
deterministic optimization. Simulation uses declared SC streams instead.

`solvercraft.report.answer` produces a versioned JSON answer report containing
the source objective/constraints, original expressions, variable values/bounds,
row activities/slacks/violations, tolerances and distinct termination,
feasibility/optimality claims. It requires the original workbook revision.
The plain solve-result contract does not invent duals. The separate
`solvercraft.report.lpSensitivity` command consumes the actual SDK checked
original-coordinate LP vertex basis: normalized-row shadow prices, one-at-a-time
RHS/objective validity intervals, and revalidated warm-start reuse. It rejects
integer/nonlinear models and unavailable bases explicitly. The pane exposes
answer/LP reports and copying their JSON. Simultaneous coefficient changes require
basis revalidation; these are not local NLP derivatives or Monte Carlo effects.

## Reproducible host run manifests

Ordinary solve/simulation previews retain their effective request and a bounded
schema1 diagnostic manifest. It records SDK/host version and target platform,
lossless document UID/revision, frozen canonical1900 serial, full options with
decimal-string seed, nonlinear configuration, actual engine or sampler/version,
and resolved input/output bindings. Simulation records both the original total
memory budget (used for reruns) and the engine budget after its conservative IR
reservation. Request/manifest JSON is streamed through a1MiB limit before
admission; its FNV identity is diagnostic, not a cryptographic attestation.
Native/browser jobs additionally record actual wall-clock limits. Terminated
browser workers explicitly record that engine validation/results were unseen.
A test with seed18446744073709551615 verifies exact retained simulation results
when rerunning preview.request.

## Worksheet simulation

All seventeen R2 families are actual Calc declarations backed by shared SDK math
validation and deterministic nominal values:

| Formula | Core parameters |
| --- | --- |
| SC.UNIFORM | low, high |
| SC.DISCRETEUNIFORM | inclusive integer low, high |
| SC.NORMAL | mean, standard deviation |
| SC.LOGNORMAL | underlying-normal log mean, log standard deviation |
| SC.TRIANGULAR | low, mode, high |
| SC.BETAPERT | low, mode, high, weight |
| SC.BETA | alpha, beta; unit interval |
| SC.GAMMA | shape, scale |
| SC.EXPONENTIAL | rate |
| SC.WEIBULL | shape, scale |
| SC.BERNOULLI | probability |
| SC.BINOMIAL | trials, probability |
| SC.POISSON | rate |
| SC.NEGATIVEBINOMIAL | successes, probability; counts failures |
| SC.GEOMETRIC | probability; counts failures from zero |
| SC.EMPIRICAL | equal-weight observation range |
| SC.EMPIRICALCONTINUOUS | observation range; linear empirical quantile |

A final optional nominal argument overrides the ordinary-cell value. Otherwise
canonical finite analytic means are used. Shared math exposes a labelled median
fallback for undefined/unavailable means in additional families; numeric overflow
remains an error. SC.LOGNORMALMEAN additionally supports arithmetic mean/std,
SC.CATEGORICAL explicit values/weights, and SC.CUMUL unambiguous piecewise CDF knots.
SC.OUTPUT(value[,label]) is an identity expression. This is explicit Solvercraft
ordinary-cell semantics, not a claim of Frontline ValueToDisplay parity.

`worksheet.discover` finds stable top-level declaration/SC.OUTPUT cells. Simulation
may omit bindings to discover them, or explicitly override them. Distribution
parameters are fixed per run. Each uncertain cell is sampled once per trial; all
reads share that value. A private Calc refreshes volatile dependencies at the one
captured timestamp, so a NOW() cell read by both a parameter and an output agrees.
Live cells, formulas and history remain unchanged.

The simulation evaluator is a separate bounded topological cell IR, not SDK Expr
rewrites. Constant/Input/Formula nodes capture all dependencies once; each trial
reads immutable sampled bindings and evaluates parsed formulas through the real
Gridcraft Evaluator and a private value Host. SUM reference streaming/grouping,
SUMPRODUCT multiplication order, errors, bool/text/blank coercion, scoped and
relative names, arrays inside reductions and date-system conversion therefore
follow Calc. No per-trial workbook copy or live-cell mutation is used. Unsupported
functions, dynamic references, spill/array-producing cells, cycles and expansion
beyond96 levels/100000 nodes/32768 array cells fail in preflight. Conservative
IR/transient allocation is reserved from the requested memory, and combined
prepared-expression trial work is capped at100million units. Cancellation during
a trial preserves only earlier complete rows. This is a declared supported
subset; differential tests are evidence, not a universal equivalence proof.


SC.MEAN(values), SC.QUANTILE(values,p) and SC.CVAR(losses,alpha) operate explicit
sample ranges, not a global last-job registry. Quantile is the empirical lower
inverse CDF; CVaR integrates the upper loss tail with fractional boundary mass.

`simulateParameters` compares up to32 named constant-parameter cases with common
random numbers and a million-trial combined cap. The pane offers a fifteen-axis
low/base/high one-factor grid (one base plus two changes per parameter), stores
its formula bindings in the registry and runs comparisons asynchronously through
`job.start`. Retained case memory is divided from the combined budget.

`worksheet.importPsi` previews by default and applies as one undo transaction.
Nineteen verified core signatures map to distinct SC.PSI strict shims: Uniform,
IntUniform, Normal, LogNormal, LogNorm2, Triangular, Pert, Beta, Gamma, Exponential,
Weibull, Bernoulli, Binomial, Poisson, NegBinomial, Geometric, DisUniform, Discrete,
and Cumul. Parameter order and domains are checked individually: arithmetic
LogNormal differs from underlying-normal LogNorm2, Exponential uses mean/scale,
Pert fixes weight4, discrete counts preserve their starting convention. Psi
Normal/Uniform/Poisson/Binomial zero-degenerate extensions are not silently
accepted. Property modifiers remain unsupported and preserved. Cumul accepts
strictly increasing knots with unambiguous endpoints; unspecified endpoint atoms
and improper geometric/negative-binomial p=0 fail explicitly. Ordinary-cell
ValueToDisplay differences are reported. Cells with any unsupported Psi call are
not partially translated. Evidence is cross-mapped in the frozen reference
catalog and the source worksheet tests.

## UI and browser worker

Solvercraft sits immediately right of Solver and opens Gridcraft's actual right
pane. `ui.solvercraft {on:true}` explicitly opens it for automation. Per-document
state edits objective/min/max/target value, variables, constraints, method,
distribution bindings, trials/seed, time limit and parameter grid. It saves/loads
models, starts/polls jobs every150ms, cancels, reports and atomically applies.
Completed previews compare document UID/revision every frame, so edits disable
Apply without copying the retained sample matrix again. Principal labels have
English/German text and theme tokens; Ctrl+Enter submits. All ComboBoxes and
numeric controls have explicit accessible names.

Histogram, exact step empirical CDF and scatter use real retained rows with a
visible10000-row display cap. Scatter requires two outputs. Boxplots use full-run
empirical quartiles with explicitly labelled min/max whiskers. Quantile bands use
full-run5/25/50/75/95percentiles by output column and are labelled distribution
bands, not confidence intervals. Tornado shows actual one-factor changes in the
first output mean from the base parameter case, not regression/causality claims.

The web app includes a second Rust binary `gridcraft-solver-worker`. Trunk's
`data-type=worker`/`data-loader-shim` generates its loader; no handwritten product
JavaScript is added. The main-thread adapter uses actual Web Workers with a Ready
handshake, bounded16MiB serialized workbook transport, progress messages, and a
128MiB result-message limit. The worker reconstructs a private Calc and executes
the same model/SDK functions. Admission, cold startup and worker compute timings
are separate. Four active workers/64retained jobs/2GiB aggregate reservations are
bounded. Default browser wall-clock watchdog is five minutes unless explicitly
specified. Hard cancel/deadline terminates that worker and reports Unknown
feasibility/no untransmitted incumbent. Received feasible results still enter the
normal original-model validation and atomic revision-guarded apply path.

## Executed evidence and remaining gates

Exact Rust1.95 native focused engine checks passed40unit tests plus all13existing
host integration regressions. Cases include two ordinary OPC roundtrips, owned
JSON surviving Strict Open XML import and Transitional resave, row/sheet binding
migration and deleted references, Unicode name collisions, atomic apply/undo/
protection/stale identity, validated integer rounding, actual constrained NLP and
owned Auto MIQP routing, LP shadow-price1.5/warm-start reoptimization, legacy
linear typed-domain export/import/undo and refusal, parameter common random
numbers, saved SC parameter-grid migration/native jobs, all17SC nominal families,
all19Psi strict shims, frozen clocks and exact manifest reruns with a full64-bit
seed. Caller-budget target equality is preflighted before objective copying;
the exact target-row semantics/label are unchanged. Six ordered-IR tests compare29 arithmetic/function/coercion cases over100
seeded input vectors bit-for-bit against actual Calc, plus domain/first-error
cases, relative/scoped/union names,1904 time, retained rows, budgets and cancellation.

Final native engine/UI all-target Clippy -Dwarnings passed in23.34s including
the extreme-axis chart/test update. The rebuilt ordered-IR/manifest worker WASM
compilation passed in10.62s; final engine WASM Clippy -Dwarnings passed in6.65s. Independent validation already executed four
actual generated-loader Chrome Worker cases: LP preview/apply/undo/stale/foreign
identity,2million-trial Monte Carlo with real progress/window heartbeats, hard
cancel and deadline. Recorded normal LP/MC admission was below1ms and observed
status queries at most1ms; these are measured scoped runs, not a universal timing
bound or a cold-start claim. The current seven-case rebuild adds owned MIQP,
ordered SUM/SUMPRODUCT and independent caller behavior.

Independent native UI validation now passed all6 actual cases, including
DE/light/dark keyboard, stale preview and all5 chart ComboBox selections plus
saved parameter Tornado with independently expected Base/Low/High means20/10/30
and deltas-10/+10. All9 render-used chart data/helper tests passed, covering
bin boundaries, duplicate CDF mass, scatter pairs, full-run boxquartiles and
quantile bands, one-factor effects and tiny/large/overflowing ranges. A strict
accessible1600x1000 render also passed. Chart coordinates
now retain tiny ranges, close large-offset values and overflowing opposite-sign
spans; unrelated output2 cannot collapse histogram/CDF output1. Pure numeric
regressions cover these cases. Broad host CI/assets/layers/WASM executes against
the final frozen sources. Active `cargo xtask parity` regenerated the276/292catalog checklist, and
`cargo xtask keytips` regenerated both the production Rust table and its tests.
Raw and canonical-LF hashes for the41 owned/generated source paths are retained
in `host-checkpoint.json`; these diagnostic hashes are not build attestations.

Unsupported Psi properties and explicitly ambiguous
reference-domain cases remain listed, never counted as full Frontline parity.

## Parallel fixture stability checkpoint

The full native engine library suite passed all679 tests with default parallel
scheduling on24 logical processors (RUST_TEST_THREADS unset), in14.35s. Seven
host fixtures share the SDK's real four-worker pool; a test-only guard now
serializes those fixtures and cancels/waits for terminal work before releasing
its lock, including on unwind. The rest of the suite remains parallel. The
production quota and the SDK's separate saturation test are unchanged. The
engine all-target Clippy -Dwarnings check passed in54.31s after this fixture fix.
Use task-local TEMP/TMP under $WORKSPACE_ROOT/solvercraft-temp/grid-host-tests;
the sandbox's default package temp is outside the task's writable roots.
