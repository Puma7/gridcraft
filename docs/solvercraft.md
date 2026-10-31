# Solvercraft integration

Last updated: 2026-10-31. Status: alpha integration; full application validation pending.

Solvercraft introduces advanced optimization and simulation alongside the existing Solver. The new code is under [solvercraft/](../solvercraft/README.md), an independently buildable Rust workspace. Gridcraft's engine depends on its facade; formula/workbook bindings remain in the engine and the UI only sends commands.

The implemented numerical scope is LP/MILP, bounds-only separable convex QP, bound-constrained smooth AD optimization, constrained finite-bound differential evolution, Monte Carlo/LHS and finite-scenario expectation/CVaR/box-robust formulations. Each path reports its actual guarantee. Sparse model coefficients do not imply a sparse internal LP implementation.

## Host commands and state

- solvercraft.capabilities reports the supported formula/method subset.
- solvercraft.validate translates and checks the workbook or a portable model.
- solvercraft.solve and solvercraft.simulate return bounded synchronous previews.
- solvercraft.result.apply revalidates identity, revision, formulas, bounds and constraints before one transactional edit.

Programmatic calls do not open dialogs. The initial ribbon entry opens a modal UI with optimization/simulation tabs and an explicit apply action. Preview and simulation leave the live workbook untouched.

## Layering and workspace ownership

Solvercraft types sit at L0; math/model at L1; compile at L2; engines/simulation at L3; the facade at L4. Gridcraft engine remains L5 and UI L6. The parent excludes the nested Solvercraft workspace and its member paths, avoiding accidental inherited Gridcraft versions, editions and lints. The path dependency still participates in layer validation.

## Verification and open gates

The standalone core has 136 passing native tests and recorded CLI examples. This is not a complete application build or browser/WASM test. Before merging, run the outer cargo xtask ci and the four new engine integration tests with Gridcraft's required toolchain, verify model/preview/apply/undo in the running UI, and test WASM.

The current UI is modal and synchronous. Async sidebar operation, workbook model persistence, advanced sensitivity reports, general conic optimization and full Frontline function parity remain open. See the SDK [reference](../solvercraft/docs/reference.md) and [acceptance](../solvercraft/docs/acceptance.md).
