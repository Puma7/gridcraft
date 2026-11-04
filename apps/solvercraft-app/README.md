# Solvercraft desktop workbench

`solvercraft` is a standalone local application over Gridcraft's reusable
command-driven workbook frontend and the independent Solvercraft SDK. It opens
directly to the Solvercraft pane, with an original production-planning model
and a seeded margin-simulation model. The examples are unsaved workbooks; they
do not write files until the user chooses Save.

```sh
cargo run --release -p solvercraft-app
cargo run --release -p solvercraft-app -- --blank
cargo run --release -p solvercraft-app -- workbook.xlsx
```

Use the saved-model buttons in the right pane to switch examples. Production
planning has optimum `[2,6]`, contribution `36`. The simulation evaluates actual
sampled sales minus costs. Start previews, inspect progress, cancel, and apply
an optimization result explicitly. Application remains one Undo step. Workbook
open/save, saved model persistence and unsaved-close prompts use Gridcraft's
existing commands and native host services.

The app starts no listener, hosted service or automatic update check. Its
Gridcraft-specific update flow is disabled to prevent installing a different
product's binary. External agents use the separate `solvercraft-mcp` process;
other ArtCraft hosts can embed the independent Rust SDK without this UI.

The application version follows the containing Gridcraft workspace. The SDK
has its own version and records that identity in run manifests. Official
ArtCraft Launcher listing and download publication require the separately
reviewed catalog integration and matching release artifacts.

For visual verification, the existing egui test renderer can render the real
workbench without opening a visible desktop window:

```sh
cargo run -p solvercraft-app --example snapshot -- solvercraft.png
```

Use the documented Craft font input for acceptance builds, as for Gridcraft.
The native Windows shell defaults to DirectX 12/FXC and respects explicit wgpu
backend overrides. Only the exercised platform is evidence of execution;
cross-platform source support does not imply signed installers for every OS.
