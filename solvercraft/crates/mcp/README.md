# Solvercraft MCP

`solvercraft-mcp` is an on-demand stdio MCP adapter to the independent Solvercraft
SDK. It does not start a network listener, create a spreadsheet or require
Gridcraft. An MCP client starts the executable and owns its lifetime.

From the Solvercraft workspace:

```sh
cargo build --release --locked -p solvercraft-mcp
```

Example MCP client configuration; use the absolute executable path when needed:

```json
{
  "mcpServers": {
    "solvercraft": { "command": "solvercraft-mcp", "args": [] }
  }
}
```

The negotiated protocol defaults to `2025-06-18`; `2025-03-26` and `2024-11-05`
are supported explicitly. Complete the `initialize` request and
`notifications/initialized` notification before using tools. Every message is a
single JSON-RPC object followed by a newline. Stdout contains protocol messages
only. Closing stdin cancels owned jobs and ends the process after a bounded
shutdown grace period.

| Tool | Arguments |
|---|---|
| `solvercraft_capabilities` | `{}` |
| `solvercraft_start_solve` | `{model, options?, limits?}` |
| `solvercraft_start_simulation` | `{inputs, outputs?, options?, limits?}` |
| `solvercraft_start_execute` | `{request}`; the SDK schema-2 `ExecutionRequest` with portable model and ArtCraft/host context |
| `solvercraft_job_get` | `{job_id, include_result?:true}` |
| `solvercraft_job_cancel` | `{job_id}` |
| `solvercraft_job_forget` | `{job_id}`; terminal jobs only |

The model payload of `start_solve` matches the CLI's `solve` request. Simulation
inputs are typed SDK distributions, and optional outputs are bounded expression
ASTs; no executable code is accepted. `start_execute` preserves portable object
bindings, revision strings, min/max/target goals and the shared SDK's nonlinear
and conic routing. Discover concrete support using capabilities.

Each start returns `{job_id, result_requires_polling:true}`. The ID is a decimal
string scoped to this process. Poll `job_get` with a short delay; use
`include_result:false` for small progress polls. Terminal responses retain the
actual manifest and optional `result:{ok:true,value:...}` or
`result:{ok:false,error:...}`. Execution values contain an `ExecutionRun` with its
own portable manifest and `result` field. A `completed` job only means that
execution ended; inspect termination, feasibility and optimality in the value.
A local or heuristic result is not a global-optimum certificate.

Default quotas are 2 MiB per request, 4 MiB per response, 16 retained jobs,
256 MiB memory per job, 512 MiB total retained memory reservations and 60 seconds
per job. Native work also shares the SDK's process-wide four-worker quota.
Wall-clock and cancellation checks are cooperative. Decision vectors, portable
execution metadata, simulation summaries and retained samples that cannot fit
the response quota are rejected before starting. Forget terminal
jobs to release their reservations; nothing is persisted across process exits.

Optional `--config PATH` reads at most 64 KiB of strict Config JSON. All fields
are optional and defaulted: `max_request_bytes`, `max_response_bytes`,
`max_retained_jobs`, `max_job_memory_bytes`, `max_reserved_memory_bytes`,
`max_job_time_millis` and `shutdown_grace_millis`. Invalid ranges/unknown fields
are refused. Configuration is a trusted process startup option, not an MCP file
access tool. The adapter does not add authentication or a central service.
