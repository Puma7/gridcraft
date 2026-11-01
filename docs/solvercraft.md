# Solvercraft integration

Last updated: 2026-10-31. Status: development alpha; evidence is scoped to named checks.

Solvercraft adds optimization and simulation beside the existing Solver. The ribbon button opens a right task pane; programmatic engine, CLI and MCP calls execute commands without dialogs. The [independent Rust SDK](../solvercraft/README.md) owns mathematics and portable models. The host owns workbook bindings, exact spreadsheet simulation semantics, saved models and transactional application.

The pane starts actual background jobs: bounded native SDK workers or a separate Rust/Trunk Web Worker binary. Progress and status queries do not wait for solving. Previews leave live cells unchanged. Applying a feasible solution checks document identity/revision, original expressions, the real Calc result, protection, bounds and integer rounding before one undoable edit. Unknown or stale previews cannot be applied. A cooperatively cancelled native run can expose only an actually returned and revalidated feasible incumbent.

Saved models use the owned OPC part `xl/solvercraft/models.json`, its content type and workbook relationship. Stable hidden reference names migrate bindings through structural edits; invalid references fail explicitly. Tests cover two roundtrips, Strict Open XML input, collision rules, deletion, undo and sheet/row migration. Legacy Solver names have an explicit same-sheet linear import/export subset with representability reporting; unsupported shapes are reported before writes.

Simulation uses a bounded topological cell IR over immutable sampled inputs, evaluated with Gridcraft's actual value evaluator. It preserves SUM grouping and IEEE order, SUMPRODUCT, reference/text/boolean/blank coercion, errors, scoped/relative names and frozen NOW/TODAY. This differs deliberately from optimization's structural mathematical compiler, which remains protected by original-Calc candidate checks. Dynamic references, spills, legacy array cells and functions outside the declared subset are refused; cached values never stand in for unsupported formulas.

The worksheet adapter supports17 SC distribution families plus verified numeric Psi shims. Unsupported properties and ambiguous reference domains remain explicit. Parameter comparisons, histogram/step CDF/scatter/box/quantile-band/tornado displays and checked LP answer/basis sensitivity reports use actual results. Every ordinary solve/simulation preview carries a bounded diagnostic manifest with effective options, lossless seed, SDK/host/platform versions, frozen document/revision/time, actual engine/sampler identity and bindings. Effective simulation requests are retained for reruns. Fingerprints and caller labels are not build attestations.

See [host implementation and test evidence](../solvercraft/docs/implementation/gridcraft.md), [browser transport measurements](../solvercraft/docs/implementation/browser-worker-native-transport-report.json) and [current acceptance review](../solvercraft/docs/qa/acceptance-current.md). Native host regression tests, actual egui input/render tests, WASM compilation and actual generated-worker Chrome tests are distinct evidence. The final rebuilt-worker/UI/full-CI checkpoint is recorded there; compile success alone is never browser execution or complete Excel/Frontline parity.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | minor | Replace obsolete modal/synchronous scope with async task pane, owned XLSX models, ordered Calc simulation and bounded manifests/evidence |
