# SDK execution and process boundaries

Status: 2026-10-31. Read the immutable requirements and current acceptance matrix alongside this guide. Per-platform evidence is recorded in [verification.md](verification.md).

## Modern execution

The shared entry point is workflow::execute_owned(&ExecutionRequest, Arc<dyn RunControl>). A strict request contains a validated project::PortableModel, caller run/snapshot/host context, optional min/max/target goal, derivative settings and wall-clock limits. Its result retains actual engine, options, lossless decimal seeds/model revisions, platform, original objective and target reachability. A target goal adds an equality between the original objective and target, then solves feasibility. Target attainment uses the original objective and feasibility tolerance; a failed local search is not a proof that the target is unreachable.

The manifest fingerprint is diagnostic identity, not cryptographic authentication. Source revision and snapshot identity come from the host; the SDK does not attest arbitrary caller metadata. Candidates are re-evaluated against original expressions and the original algebraic model before being returned as feasible.

Use engines::solve_owned, runtime::solve_with_limits_owned or provenance::solve_run_owned when retaining callbacks or using automatic mixed-conic routing. Ownership preserves controls through backend callbacks and integer relaxations. The legacy borrowed solve entry point retains its documented class limits; another entry point's capability list does not extend it.

OwnedDeadlineControl and its borrowed counterpart use a portable monotonic clock. Zero milliseconds permits no engine work. User cancellation and deadline expiration remain separate. Cooperative limits cannot interrupt caller code that never checks its control. Native jobs::start_job/start_shared_job enforce a shared four-worker quota and retain results in handles. Native-thread jobs are unavailable on WASM; Gridcraft supplies its own Rust WebWorker transport.

## Portable projects and migration

ModelLibrary stores named schema-2 algebraic/expression models with opaque object/property bindings, options and metadata. IDs/revisions are strings; revisions are canonical decimal u64 values. Validated operations enforce identity/name uniqueness and expected revisions atomically. JSON import/export is byte bounded.

project::migrate_v1_algebraic accepts a schema-1 algebraic model plus explicit new ID/options, validates the original, converts and validates the result. It invents no workbook bindings or results. Process commands: project-roundtrip and migrate-v1. The executable [ArtCraft layout example](../../crates/solvercraft/examples/artcraft_layout.rs) demonstrates independent object bindings and portable roundtrip: minimum cost140 at widths(20,100). It is a reuse example, not a shipped second-product integration.

## Advanced JSON CLI

solvercraft-cli <command> request.json accepts one strict typed object. Unknown fields, surplus arguments and incompatible model classes fail explicitly. Use - or omit path for stdin, bounded to16MiB. Exact request definitions live in [advanced.rs](../../crates/cli/src/advanced.rs); process tests execute real binaries.

| Commands | Workflow |
|---|---|
| execute | Portable model, context, goal, derivative and runtime settings |
| solve-qp, solve-sparse, solve-conic, solve-mixed-conic | Detailed QP/sparse/conic results, actual dual/bound data where valid |
| solve-basis | Validated LP vertex-basis warm start and basis sensitivity |
| solve-constrained, solve-multistart, solve-global | Constrained local, seeded multistart or restricted interval certificates |
| solve-discrete, finite | SOS/indicator conic transformations or finite domains/reification |
| simulate-advanced, parameter-grid, fit | Copulas/Sobol/invalid-trial policies/stopping, parameter runs or fitting |
| recourse, benders, chance, saa | Two-stage linear recourse, checked decomposition, scenario chance or train/holdout |
| decision-tree, information-value, multiobjective | Backward evaluation, information timing/cost or multiple goals |
| robust-box, robust-budget, robust-ellipsoid | Explicit uncertainty counterpart plus solved result |

All simulation process paths inspect borrowed outputs with one shared AST/depth/memory budget before evaluator allocation. Parameter-grid variables are ordered as constant run parameters then sampled inputs. Common random numbers versus distinct seeds is explicit.

## Optional loopback service

Native-only Server::bind is explicit; importing the SDK never starts a listener. The serve CLI command reads ServerConfig, binds loopback and reports its address. Embedding hosts call Server::run with an atomic stop flag. Admission and the shutdown cancellation sweep share a registry lock: a connection blocked in a read cannot start work after shutdown returns.

The protocol is newline-delimited JSON. Every request contains schema_version:1, a nonempty string id and command:{method,params}. Authenticate each connection using method authenticate with params:{token:...}. Other methods: capabilities, solve, simulate, get, cancel, forget. Job IDs are decimal strings. Active-job get polls contain small status/progress; terminal get includes the quota-bounded result. Params:{job_id,include_manifest:true} also retrieves retained run identity, effective options/limits, platform and actual returned engine. Failed executions retain identity while reporting that no typed engine result was received. Forgetting a running job fails; cancel, await terminal state, then forget. Tokens come from the embedding host and are never returned.

Configuration bounds connections, retained jobs, input/output bytes, per-job memory and time. Worker capacity is shared with other SDK jobs. Retained-sample requests that exceed the response quota fail before admission. Remote deployments supply their own authenticated transport/gateway; the built-in service binds loopback and contacts no legacy PSI service.
