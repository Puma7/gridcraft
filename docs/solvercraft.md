# Solvercraft integration

Last updated: 2026-11-04. Status: development alpha; evidence is scoped to named checks.

Solvercraft adds optimization and simulation beside the existing Solver. The ribbon button opens a right task pane; programmatic engine, CLI and MCP calls execute commands without dialogs. The [independent Rust SDK](../solvercraft/README.md) owns mathematics and portable models. The host owns workbook bindings, exact spreadsheet simulation semantics, saved models and transactional application.

Choose **Data → Solvercraft → Open Microgrid** to open a fresh copy
of the English template. The dashboard compares two daily energy plans for a
fictional cold storage site. Select **Run full comparison** to optimize and apply
both plans and then simulate their risks. Three step cards show progress and
the applied plan costs. The completed comparison displays the shortage risk,
refreshes the dashboard and offers **Open result sheet**. Change the assumptions
and run again to compare a new scenario. Editing the workbook during a run stops
the workflow before another solution can be applied. **Model settings** exposes
the individual models. The StressTest sheet lets you explore a single outage
scenario with live formulas.

The [standalone Solvercraft application and on-demand MCP](../solvercraft/docs/implementation/artcraft-integration.md)
reuse this same core. The application opens a solver-focused workbook with
original optimization/simulation examples. Independent `solvercraft-mcp` exposes
algebraic solve, simulation, portable model execution and bounded job lifecycle
without a workbook or network listener. Other ArtCraft applications can embed
the object/property-binding SDK directly. Official launcher registration is a
separate maintainer-reviewed catalog submission.

The pane starts actual background jobs: bounded native SDK workers or a separate Rust/Trunk Web Worker binary. Progress and status queries do not wait for solving. Previews leave live cells unchanged. Applying a feasible solution checks document identity/revision, original expressions, the real Calc result, protection, bounds and integer rounding before one undoable edit. Unknown or stale previews cannot be applied. A cooperatively cancelled native run can expose only an actually returned and revalidated feasible incumbent.

Saved models use the owned OPC part `xl/solvercraft/models.json`, its content type and workbook relationship. Stable hidden reference names migrate bindings through structural edits; invalid references fail explicitly. Tests cover two roundtrips, Strict Open XML input, collision rules, deletion, undo and sheet/row migration. Legacy Solver names have an explicit same-sheet linear import/export subset with representability reporting; unsupported shapes are reported before writes.

Simulation uses a bounded topological cell IR over immutable sampled inputs, evaluated with Gridcraft's actual value evaluator. It preserves SUM grouping and IEEE order, SUMPRODUCT, reference/text/boolean/blank coercion, errors, scoped/relative names and frozen NOW/TODAY. This differs deliberately from optimization's structural mathematical compiler, which remains protected by original-Calc candidate checks. Dynamic references, spills, legacy array cells and functions outside the declared subset are refused; cached values never stand in for unsupported formulas.

The worksheet adapter supports 17 SC distribution families plus verified numeric Psi shims. Unsupported properties and ambiguous reference domains remain explicit. Parameter comparisons, histogram/step CDF/scatter/box/quantile-band/tornado displays and checked LP answer/basis sensitivity reports use actual results. Every ordinary solve/simulation preview carries a bounded diagnostic manifest with effective options, lossless seed, SDK/host/platform versions, frozen document/revision/time, actual engine/sampler identity and bindings. Effective simulation requests are retained for reruns. Fingerprints and caller labels are not build attestations.

See [host implementation and test evidence](../solvercraft/docs/implementation/gridcraft.md), [browser transport measurements](../solvercraft/docs/implementation/browser-worker-final6-report.json) and [current acceptance review](../solvercraft/docs/qa/acceptance-current.md). Native host regression tests, actual egui input/render tests, WASM compilation and actual generated-worker Chrome tests are distinct evidence. The final rebuilt-worker/UI/full-CI checkpoint is recorded there; compile success alone is never browser execution or complete Excel/legacy PSI parity.

The [final7 evidence](../solvercraft/docs/implementation/final7-verification-summary.json) supports bounded acceptance of the original 96 criteria and 31 selected top-level PSI import forms. R046 is the 111-entry mathematical catalog; R081 covers the actually supported signatures and explicit conversion policies. The reference catalog tracks 272 PSI functions and records remaining gaps. The [scope audit](../solvercraft/docs/qa/original-scope-audit.md) records the original wording and per-function test evidence.

Both historical final6 worker attempts remain visible: initial 7/1 at an additional 26 ms heartbeat observation, then one identical repeat 8/0. The cause of the initial timing failure is unproven; R081 passed in both. Final7 records its separate first-attempt worker suite. This is not a 25 ms product timing guarantee. The default PSI outside-value conversion is explicitly lossy and unsupported nested/property forms are preserved rather than applied.

Public final6 evidence is available as [compact fixed-field reports and strict test-result excerpts](../solvercraft/docs/implementation/public-evidence.md). Full local diagnostic originals stay private; line positions for the scoped test references remain stable. The current exact-binary oracle projection corrects v1 request-record aliasing after three cases plus one node-limit control were rechecked on the unchanged binary. See the [current scope audit](../solvercraft/docs/qa/original-scope-audit.md) and verification guide for the evidence boundary.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | minor | Replace obsolete modal/synchronous scope with async task pane, owned XLSX models, ordered Calc simulation and bounded manifests/evidence |
| 2026-11-01 | documentation | Link final6 original-criterion acceptance and both worker attempts; retain explicit selected PSI conversion limits |
| 2026-11-02 | minor | Add standalone Solvercraft app, reusable host SDK and on-demand independent MCP; link final7 selected import scope and reviewed launcher submission |
| 2026-11-03 | minor | Add the built-in Cold Storage Microgrid template |
| 2026-11-04 | minor | Guide the example through both optimizations and paired simulation, with progress and live result comparison |

### Review and import simulation formulas

Open **Data → Solvercraft → Import simulation formulas** and choose
**Scan workbook**. The preview lists proposed changes and preserved formulas.
The question-mark controls explain the scan and the conversion policy.
**Load simulation settings** opens a new draft when supported embedded
settings are present. It leaves the original workbook cells intact.

**Convert supported cells** applies the preview in one undo step. Rescan if
the workbook changed. A new simulation draft can discover worksheet
declarations, while saved models retain their current bindings.

The importer recognizes supported numeric PSI calls, add-in-prefixed formulas
and supported scalar legacy-array cells. The bare output marker can accompany
a numeric formula or a supported distribution. A single numeric base case
can set the nominal value for a supported distribution.

After a completed simulation, **Update statistic cells** fills the matching
single-cell mean, minimum, maximum, range, sample variance, sample standard
deviation and inclusive target-probability formulas with the run values.
Variance and standard deviation require at least two trials and a finite variance.
The formulas stay in place. Edits invalidate the run values, and Undo restores
the prior state. Select output cells explicitly when the sheet lacks declarations.
