# Solvercraft across ArtCraft applications

Solvercraft is a local application and reusable computation toolkit. It does not
require a hosted service, an account or a listening network port. All entry
points use the same Rust optimization and simulation SDK.

| Use case | Entry point | Host requirements |
|---|---|---|
| Interactive modeling, simulation and workbook results | `solvercraft` desktop app | Native desktop |
| Spreadsheet workflows in Gridcraft | Data > Solvercraft | Existing Gridcraft native/web app |
| Another ArtCraft application | Rust `solvercraft` SDK | Caller supplies its object/property bindings |
| External AI agent | `solvercraft-mcp` over standard input/output | Local MCP client can launch a process |
| Scripts and other programming languages | `solvercraft-cli` with JSON | Caller can execute a local process |

## Desktop application

From the repository root:

```sh
cargo run --release --locked -p solvercraft-app
cargo run --release --locked -p solvercraft-app -- book.xlsx
```

The standalone application reuses Gridcraft's worksheet and Solvercraft task
pane. This gives it the same model editor, background jobs, progress,
cancellation, result preview, explicit application and Undo semantics. The
dedicated launch starts with example optimization and simulation models; the
application also opens ordinary workbooks. Keep results as a preview until
applying them is intended.

The SDK is Layer 4, Gridcraft's adapter/command/UI layers retain their existing
boundaries, and the application shell is Layer 7. The app does not replace the
independent SDK or require other ArtCraft programs to depend on its worksheet UI.

## Shared SDK for other ArtCraft hosts

An ArtCraft application can use the library directly:

```toml
[dependencies]
solvercraft = { path = "../gridcraft/solvercraft/crates/solvercraft" }
```

Use a path appropriate to the actual checkout. A distributed application should
pin the source revision and retain the engine/sampler version in its run record.
The dependency is workbook independent and does not require egui or a network
library. No central process is necessary for an in-process Rust host.

`project::PortableModel` stores an algebraic or expression model with opaque
`ParameterBinding { parameter, object_id, property }` values. These can refer to
frame widths, object positions, render budgets or other host parameters. They are
not interpreted as workbook addresses by the SDK. A host owns these mappings and
must check document identity/revision before applying an accepted result.

`workflow::execute_owned` accepts the portable model, run/snapshot context, goal
and runtime limits. It returns the original objective, qualified solver result
and execution manifest. Use `jobs::start_shared_job` for bounded native background
work, or a host-owned worker on WASM. A host applies validated results as its own
single undoable transaction; the SDK does not silently mutate host objects.

The executable, workbook-independent example is:

```sh
cargo run --manifest-path solvercraft/Cargo.toml --locked -p solvercraft --example artcraft_layout
```

It roundtrips two object-width bindings and verifies the independent layout
optimum of 140 at widths `[20,100]`. This is an executable host-adapter example,
not a claim that every ArtCraft application's repository has already adopted it.

## External agents without a standing service

Build the standalone MCP entry point:

```sh
cargo build --manifest-path solvercraft/Cargo.toml --release --locked -p solvercraft-mcp
```

Register the resulting executable in a local MCP client:

```json
{
  "mcpServers": {
    "solvercraft": {
      "command": "/absolute/path/to/solvercraft-mcp",
      "args": []
    }
  }
}
```

On Windows use the actual absolute path to `solvercraft-mcp.exe` in the
client's configuration. The client starts and owns this process. No Gridcraft
window or workbook is needed. Standard output carries only newline-delimited
MCP JSON-RPC; inputs, outputs and retained jobs belong to this process. Closing
the connection requests cancellation and shuts it down.

The supported MCP protocol is explicitly negotiated (including `2025-06-18`),
not an unrestricted claim of every future protocol revision. Available tools:

| Tool | Purpose |
|---|---|
| `solvercraft_capabilities` | Discover actual methods, limits and mathematical classes |
| `solvercraft_start_solve` | Start an algebraic optimization request |
| `solvercraft_start_simulation` | Start a seeded simulation with expression outputs |
| `solvercraft_start_execute` | Start a portable host model through the shared workflow API |
| `solvercraft_job_get` | Poll status/progress and retrieve a retained result |
| `solvercraft_job_cancel` | Request cooperative cancellation |
| `solvercraft_job_forget` | Release a terminal retained job |

The start tools return a decimal-string `job_id`. Poll `solvercraft_job_get`
with that ID until the job is terminal. Check `result.ok` before using
`result.value`; failures contain a structured error. With `start_execute`, the
value is an `ExecutionRun` with its own `result` and portable manifest. Preserve
feasibility, termination, optimality, original objective, engine and seed in any
agent-facing answer. A best-found or local result is not a global certificate.

Request/output size, retained jobs, worker capacity, memory reservation and
execution time are bounded. Cancellation is cooperative. Results are ephemeral;
persist desired models/results before forgetting a job or ending the process.
The optional pre-existing TCP service remains an alternative for hosts that
explicitly choose it; neither the app nor standalone MCP launches it.

For workbook-aware agents, continue to use `gridcraft-cli mcp` and the
[complete workbook example](agent-quickstart.md#workbook-access-through-mcp).

## ArtCraft Launcher catalog

The [launcher integration submission](../../integrations/artcraft-launcher/README.md)
contains a version-pinned patch for the official launcher's catalog and details.
The app's portable Windows package follows the launcher's expected naming and
archive layout. A local package and a proposed catalog entry do not mean that a
release has been published or the official catalog has accepted the application.

The official launcher maintainers must approve third-party sources. No automatic
upstream merge, store registration or publisher identity is claimed. This leaves
the application, shared SDK and external-agent access usable independently of
catalog review.

See [numerical and compatibility limits](reference-gaps.md) and the
[current app/agent checks](app-agent-verification-2026-11-02.md), alongside the
[original scope and evidence](verification.md). The integration does not enlarge
the mathematical guarantees of any engine.
