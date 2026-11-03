# Solvercraft for applications and AI agents

Solvercraft has one workbook-independent Rust SDK and several entry points. Use
the same numerical model and result contracts from an ArtCraft application, a
local process or an agent. This is a development alpha; discover capabilities and
retain the actual result status, engine, seed and limits with each answer.

## Choose an entry point

| Task | Entry point |
|---|---|
| Open Solvercraft as a standalone desktop application | `solvercraft` (`cargo run -p solvercraft-app`) |
| Give an external agent workbook-independent tools | `solvercraft-mcp` over stdio, started by the MCP client |
| Solve or simulate without a spreadsheet | `solvercraft-cli`, or the Rust `solvercraft` SDK |
| Work with workbook cells, formulas, saved models and Undo | Gridcraft's Solvercraft pane or `gridcraft-cli mcp` |
| Control the desktop workbook an operator is viewing | `gridcraft-cli mcp --connect 7979` |
| Keep local optimization/simulation jobs across requests | `solvercraft-cli serve` with authenticated JSON-lines TCP |

The SDK belongs to Layer 4. The existing `solvercraft-cli` and Gridcraft native/web
applications belong to Layer 7; they consume the SDK through their appropriate
adapters. Another ArtCraft application can embed the SDK without depending on
Gridcraft or its interface. The executable [layout example](../../crates/solvercraft/examples/artcraft_layout.rs)
shows portable object bindings. It is an integration example, not a released
second ArtCraft application or an app-store registration. The
[ArtCraft integration guide](artcraft-integration.md) documents the standalone
app, shared host API, on-demand MCP entry point and launcher submission.

## Standalone MCP for external agents

Build `solvercraft-mcp` from the nested SDK workspace:

```sh
cargo build --manifest-path solvercraft/Cargo.toml --release --locked -p solvercraft-mcp
```

This command is run from the repository root. Configure a local MCP client with
the absolute executable path (`solvercraft-mcp.exe` on Windows), with no
arguments. It exposes optimization, simulation, portable host-model execution
and job lifecycle tools directly from the SDK. It requires no workbook, account,
listening network port or permanently running service. The MCP client starts the
process when needed. See the [tool table and lifecycle](artcraft-integration.md#external-agents-without-a-standing-service).

## Standalone calculations

Build from the `solvercraft` directory with Rust 1.95 or later:

```sh
cargo build --release --locked -p solvercraft-cli
cargo run --release -p solvercraft-cli -- capabilities
cargo run --release -p solvercraft-cli -- solve examples/lp.json
cargo run --release -p solvercraft-cli -- simulate examples/simulation.json
cargo run --release -p solvercraft-cli -- risk examples/risk.json
```

The LP example maximizes `3*a + 5*b`, with `0 <= a <= 4`, `0 <= b <= 6`
and `3*a + 2*b <= 18`. Its optimum is `a=2`, `b=6`, objective `36`.
Requests can instead come from standard input by passing `-`. Read the structured
JSON response and process exit status; a local or best-found result must retain
that qualification. Advanced commands, schemas and runtime contracts are in the
[SDK guide](sdk-runtime.md).

## Workbook access through MCP

Build `gridcraft-cli` from the repository root:

```sh
cargo build --release --locked -p gridcraft-cli
```

Register the resulting executable in an MCP client. This configuration starts a
headless workbook session; replace the command with its absolute path if the
executable is not on `PATH`:

```json
{
  "mcpServers": {
    "gridcraft": {
      "command": "gridcraft-cli",
      "args": ["mcp"]
    }
  }
}
```

Use `['mcp', '--in', 'book.xlsx']` as the argument list to open a file at startup.
For a visible desktop workbook, start `gridcraft --control 7979` and use
`['mcp', '--connect', '7979']`. The desktop control token is handled as documented
in the [MCP guide](../../../docs/mcp.md).

MCP clients handle initialization. For a direct stdio client, send one JSON-RPC
object per line, initialize first and then send `notifications/initialized`:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"solvercraft-example","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_commands","arguments":{"search":"solvercraft"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.capabilities","params":{}}}}
```

Check a response's top-level JSON-RPC `error` first. Successful tool responses
contain a JSON document in `result.content[0].text`; also check `result.isError`
before decoding it. Discover commands and capabilities rather
than assuming every worksheet formula or mathematical class is supported.

### Create, solve, preview and apply

In the same MCP session, create the three cells and validate the model:

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"write_range","arguments":{"range":"A1","values":[[0,0,"=3*A1+5*B1"]]}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.validate","params":{"objective":"C1","goal":"max","variables":[{"cell":"A1","lower":0,"upper":4},{"cell":"B1","lower":0,"upper":6}],"constraints":[{"lhs":"3*A1+2*B1","op":"<=","rhs":18}]}}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.job.start","params":{"kind":"optimization","request":{"objective":"C1","goal":"max","variables":[{"cell":"A1","lower":0,"upper":4},{"cell":"B1","lower":0,"upper":6}],"constraints":[{"lhs":"3*A1+2*B1","op":"<=","rhs":18}]},"limits":{"max_time_millis":10000}}}}}
```

Extract the decimal string `id` returned by `solvercraft.job.start`. In the next
requests, replace `JOB_ID` with that exact string. Keep the same MCP process:
its headless workbook and retained jobs live in that process.

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.job.get","params":{"id":"JOB_ID"}}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.job.result","params":{"id":"JOB_ID"}}}}
```

Poll `get` with a short delay while `status` is `queued` or `running`.
On `failed`, report its error. On `completed`, decode the job-result preview:
solver fields are in its `result` and run identity is in its `manifest`. Inspect
feasibility, termination/optimality status, original objective and engine. Retrieving
a result leaves the cells untouched. If the result is suitable and updating the
workbook is intended, apply it explicitly and inspect the cells:

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.job.apply","params":{"id":"JOB_ID"}}}}
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"read_range","arguments":{"range":"A1:C1"}}}
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"execute_command","arguments":{"command":"solvercraft.job.forget","params":{"id":"JOB_ID"}}}}
```

The expected row is `[2,6,36]`. Apply revalidates workbook identity, revision,
protection and original model semantics and creates one Undo step. If the workbook
changed, create a new snapshot/job. To abandon a running job, call
`solvercraft.job.cancel`, await a terminal status, then call `forget`. Save the
workbook explicitly through `save_workbook` when a durable workbook is intended.

Simulation uses `kind: 'simulation'` with `request.inputs` containing
`{cell, distribution}`, `request.outputs` containing cell addresses and
`request.options` containing trials, sampling method and a decimal-string seed.
Input cells must contain constants or supported SC declarations; supplying a
distribution does not override an arbitrary formula cell.
It uses the same start/get/result/cancel/forget lifecycle; simulation results are
analysis, not a set of optimized decision cells to apply.

## Optional local job service

`solvercraft-cli serve server.json` starts the SDK's optional native service.
`ServerConfig::local(token)` defines the configuration defaults, or supply all
fields listed in [server.rs](../../crates/solvercraft/src/server.rs). The token is
caller supplied, between 32 and 1024 bytes. The process reports its bound address
and accepts newline-delimited JSON objects over a loopback TCP connection.

Authenticate each connection before submitting work:

```json
{"schema_version":1,"id":"auth-1","command":{"method":"authenticate","params":{"token":"YOUR_CALLER_SUPPLIED_TOKEN_AT_LEAST_32_BYTES"}}}
{"schema_version":1,"id":"caps-1","command":{"method":"capabilities","params":{}}}
```

Supported methods are `solve`, `simulate`, `get`, `cancel` and `forget`, plus
authentication and capabilities. Service job parameters use `job_id`; Gridcraft
engine commands above use `id`. See the [service contract](sdk-runtime.md#optional-loopback-service)
for quotas, manifests and lifecycle details. This transport is JSON-lines TCP;
it is not HTTP, RASON or a standalone MCP endpoint. Advanced risk, conic,
recourse and other workflows remain available through the SDK and CLI. A remote
deployment needs its own authenticated gateway; the built-in service is local.

## Scope and numerical guarantees

The [verification](verification.md) and [compatibility gaps](reference-gaps.md)
record the exercised platforms, supported domains and actual limitations. The
31 PSI conversions cover selected top-level signatures. Acceptance of the 96
original criteria is bounded acceptance, not complete legacy PSI compatibility.
Preserve seeds, versions, effective options and qualification of local/heuristic
results in any agent-facing answer.
