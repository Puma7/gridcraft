# Gridcraft host integration cross review

Review date: 31 October 2026. Review baseline: Puma7/gridcraft PR head 690a098a6c4c9d966d66b6c30c036e0e42534bfd. The PR host file was retrieved through the GitHub connector to verify the baseline. Local corrections described below are additional changes after that baseline.

## Scope and verification

Reviewed the engine command implementation, command registration, UI dialogs and parameter translation, data-ribbon entry, parent Cargo/lockfile integration, layer rules, and real upstream API definitions for Workbook, Sheet, Formula AST, Calc, Session history and aggregate coercion.

This is a source-backed integration review. No complete Gridcraft engine or UI binary was compiled or executed here. The local checkout contains only a partial upstream snapshot. The attempted command was:

    cargo check -p gridcraft-engine --offline --locked

It failed before compilation because crates/core/Cargo.toml is absent. The installed compiler is Rust 1.91.1, whereas the parent workspace specifies Rust 1.95. wasm32-unknown-unknown is not installed. Standalone Solvercraft SDK tests from earlier work are not counted as host integration tests.

Rustfmt was run on the modified host file and the added test file. This checks syntax/formatting, not API type checking or runtime semantics.

The three local host corrections below are source-backed fixes, not host-runtime reproductions. There are **17 host tests that remain unexecuted: 13 newly added integration regressions plus the four existing inline Solvercraft command tests**. The root integrator reports 265 passing standalone core tests and passing core formatting, Clippy, release and Linux checks; those checks do not validate the real Gridcraft host or UI and are not substituted for these 17 tests.

## Source-confirmed correctness gaps and local corrections

### Explicit negative bounds were erased by the default nonnegative setting

In the reviewed PR, prepare assigned an implicit lower bound of zero while reading each variable, before add_constraint processed explicit bounds. The later bound merge used max(existing_lower, explicit_lower). Consequently a normal request minimizing A1 with A1 >= -2 and A1 <= 1 could not explore negative values with the default checkbox enabled.

This differs from the existing Solver implementation in crates/engine/src/cmd/solver/problem.rs: it records whether a lower bound was explicitly supplied and applies zero only to variables without one.

Correction: variable parsing now preserves the supplied lower bound or None. The nonnegative default is applied only after the explicit constraints have been processed. This keeps explicit negative lower bounds while retaining zero for genuinely unspecified lower bounds.

Regression coverage: explicit_negative_lower_bound_overrides_the_nonnegative_default and omitted_lower_bounds_remain_nonnegative_and_false_allows_free_variables.

Status: fixed locally; real engine regression tests have not yet been executed.

### Derived spill cells were accepted as variables and frozen as cached constants

The PR checked whether a stored cell had a formula, but a non-anchor spill cell can have no stored Formula while its value is derived from an array formula. Sheet::spill_anchor is the existing authoritative API for recognizing such cells.

There were two separate consequences:

1. A spill child could be declared as a decision variable or simulation input. Result application writes a constant into that location. The shared edit protection check does not prevent all dynamic-spill writes: check_arrays in cmd.rs specifically protects legacy arrays with Formula::array metadata. Calc::cells_changed reevaluates affected dynamic-array anchors, so the write can block the original spill.
2. Translator::cell treated a spill child used as an objective, intermediary or simulation output as its cached numeric value. For example, with A1=0 and C1=SEQUENCE(2,1,A1,1), C2 currently equals 1. Optimizing C2 over A1 in [0,10] could therefore compile a constant objective of 1. Verifying only the returned point can agree at A1=0 while missing the true maximum of 11. Simulation of C2 could similarly remain constant instead of following A1+1.

Correction: explicit spill_anchor rejection now covers decision cells, simulation input cells and every translated cell reference. This includes derived objectives and output cells. The implementation does not silently freeze derived spill values.

Regression coverage: derived_spill_cells_are_not_decision_variables_or_simulation_inputs and derived_spill_outputs_are_not_frozen_as_cached_objective_constants.

Status: fixed locally; real engine regression tests have not yet been executed.

### MIN/MAX blank-reference coercion changed simulated results

The host translator uses numeric(Value::Empty)=0 for arithmetic. Applying that same conversion while flattening MIN/MAX references is not equivalent to the real worksheet aggregate semantics.

The real functions/util.rs numbers routine skips blanks inside references and arrays. Thus MIN(A1:A2), with numeric A1=5 and blank A2, is 5. The reviewed translator instead constructed MIN(5,0), producing 0. MAX with a negative numeric input and a blank has the analogous error. Unlike solve, simulate does not run Calc validation at each trial, so this could silently change Monte Carlo outputs.

Correction: MIN/MAX over blank referenced cells is now explicitly rejected in this alpha subset, including parenthesized and named references. Declared decision/simulation bindings remain numeric inputs and are not confused with an ignored blank constant. The capability response states the limitation. SUM retains its valid arithmetic blank handling.

Regression coverage: min_and_max_do_not_coerce_blank_references_to_zero, named_blank_min_argument_is_also_rejected and sum_keeps_its_distinct_blank_reference_semantics.

Status: fixed locally by explicit rejection; full reference-aware MIN/MAX coercion is not claimed.

## Positive source checks

- All new engine actions are registered in cmd::command_specs.
- Preview commands use the query command form and do not directly edit Workbook/history.
- solve freezes the source sheet name into the returned request, so a later active-sheet change does not intentionally rebind unqualified references.
- apply compares document UID and revision, rebuilds the model, checks variable count and original constraints, and reevaluates the workbook objective.
- apply writes through the ordinary edit helper. The helper performs sheet protection and legacy array checks before committing, and Session::execute coalesces an undoable command to one history step.
- Real undo and redo increment DocState::revision, so an old preview is invalidated even when an edit is subsequently undone.
- The actual Formula AST types, Workbook lookup APIs, Sheet spill API, Calc::cells_changed and gridcraft_calc::evaluate signatures were checked against upstream source.
- Parent Cargo.lock contains the Solvercraft path packages and the gridcraft-engine dependency. There is no demonstrated missing-lockfile-entry defect.
- UI simulation parameters use the typed distribution schema, decimal-string seeds and an explicit MC/LHS choice. Invalid numeric entries become rejected inputs rather than silently selected alternate distributions.
- The result UI separates termination, feasibility and optimality and does not label a heuristic result globally optimal.

These are inspected properties, not substitutes for executing the tests.

## Remaining integration gates

### Full host and UI compilation

A complete upstream source checkout, the required toolchain and dependencies are still needed. The partial local snapshot cannot validate engine type checking, the real host Clippy configuration, UI rendering or browser execution. No production compile/lint error is asserted without that build.

The bounded variable parser was extracted from prepare into a separate helper while applying the fixes, keeping the source readable and avoiding unnecessary nesting under the repository cognitive-complexity limit.

### Register and execute the added real-engine tests

The new file is crates/engine/tests/it/solvercraft_deep_review.rs. The parent integration-test main.rs is not locally present. The root integrator must add:

    mod solvercraft_deep_review;

The new file contains 13 tests, in addition to the four existing inline tests in cmd/solvercraft.rs. All 17 host tests remain unexecuted. The new tests cover the corrected cases plus pinned cross-sheet application and one undo, protected-target atomic refusal, stale previews after undo or document switch, tampered infeasible result values, the narrow-bound nonlinear regression through the real workbook evaluator, and safe rejection of shape/size errors.

No stub Session, Workbook or Calc is used by these tests. All 17 must run against the actual engine before host integration is called verified.

### Nested-workspace CI coverage

The parent Cargo workspace excludes solvercraft and solvercraft/crates/*. Existing xtask quality commands use --workspace, so the SDK is built as a dependency of the engine but its own tests, CLI tests, formatting and test-target Clippy coverage are not automatically covered by those parent commands. The layer registry recognizes the new names, but parent metadata --no-deps does not enumerate all nested-workspace members.

The integration pipeline should explicitly run the standalone workspace's test, formatting, Clippy and relevant WASM gates, or deliberately unify workspace membership. No CI workflow was changed in this review.

### Preview envelope is not an authenticated immutable result

apply revalidates result values against the request supplied in the same JSON envelope. This rejects infeasible value tampering against an unchanged request. It does not prove that the request itself is the one originally used to produce the preview: a caller can modify both request and values while retaining the public document UID/revision.

This is not an access-control bypass, because the same authorized command client can already edit cells or submit another model. However, the API should not be documented as an authenticated immutable run record. Stronger provenance would need an opaque stored preview identifier or a server/session-held binding to the original request. No such guarantee is claimed by this review.

## Source references

PR host baseline:
https://github.com/Puma7/gridcraft/blob/690a098a6c4c9d966d66b6c30c036e0e42534bfd/crates/engine/src/cmd/solvercraft.rs

Actual upstream APIs, commit 981fad14e0b90a7906a02f29785a6d0eb214fee6:
- crates/model/src/sheet.rs: spill_anchor, value and spill metadata.
- crates/engine/src/cmd.rs: commit, check_protection and check_arrays.
- crates/engine/src/cmd/edit.rs: revision increments on undo/redo.
- crates/engine/src/cmd/solver/problem.rs: explicit lower-bound tracking.
- crates/calc/src/recalc.rs: cells_changed, spill-anchor invalidation and evaluate.
- crates/formula/src/ast.rs: Reference, SheetSel, Expr and uppercased Call names.
- crates/functions/src/util.rs: numbers and reference/array coercion.
- crates/ui-egui/src/theme.rs: Tokens::get and accent.
- crates/engine/tests/it/main.rs: real integration-test registration layout.

