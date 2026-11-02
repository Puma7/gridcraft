//! Tool discovery is explicit; no command shell, filesystem or network tools.
use serde_json::{Value, json};

pub(crate) fn known(name: &str) -> bool {
    [
        "solvercraft_capabilities",
        "solvercraft_start_solve",
        "solvercraft_start_simulation",
        "solvercraft_start_execute",
        "solvercraft_job_get",
        "solvercraft_job_cancel",
        "solvercraft_job_forget",
    ]
    .contains(&name)
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn tool(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":schema,
        "annotations":{"readOnlyHint":read_only,"destructiveHint":false,"idempotentHint":read_only,"openWorldHint":false}})
}
fn limits() -> Value {
    object(
        json!({"max_time_millis":{"type":["integer","null"],"minimum":0,"description":"Clamped to the MCP process time quota. Zero permits no engine work."}}),
        &[],
    )
}
fn solve_options() -> Value {
    object(
        json!({
            "method":{"type":"string","enum":["auto","simplex","branch_and_bound","projected_gradient","differential_evolution"]},
            "max_evaluations":{"type":"integer","minimum":0},"max_iterations":{"type":"integer","minimum":0},
            "max_nodes":{"type":"integer","minimum":0},"max_memory_bytes":{"type":"integer","minimum":1},
            "feasibility_tolerance":{"type":"number","exclusiveMinimum":0},"integrality_tolerance":{"type":"number","exclusiveMinimum":0},
            "optimality_tolerance":{"type":"number","exclusiveMinimum":0},"relative_gap":{"type":"number","minimum":0},
            "seed":{"type":"string","pattern":"^[0-9]+$","description":"Lossless decimal u64 seed."}
        }),
        &[],
    )
}
fn model() -> Value {
    let term = object(json!({"variable":{"type":"integer","minimum":0},"coefficient":{"type":"number"}}), &["variable", "coefficient"]);
    object(
        json!({
            "schema_version":{"type":"integer","const":1},"name":{"type":"string"},
            "variables":{"type":"array","items":object(json!({"name":{"type":"string"},"lower":{"type":["number","null"]},
                "upper":{"type":["number","null"]},"kind":{"type":"string","enum":["continuous","integer","binary"]}}),&["name","lower","upper","kind"])},
            "objective":object(json!({"direction":{"type":"string","enum":["minimize","maximize"]},"offset":{"type":"number"},
                "linear":{"type":"array","items":term},"quadratic":{"type":"array","items":object(json!({"left":{"type":"integer","minimum":0},
                    "right":{"type":"integer","minimum":0},"coefficient":{"type":"number"}}),&["left","right","coefficient"])}}),&["direction","offset","linear","quadratic"]),
            "constraints":{"type":"array","items":object(json!({"name":{"type":"string"},"terms":{"type":"array","items":term},
                "relation":{"type":"string","enum":["less_equal","equal","greater_equal"]},"rhs":{"type":"number"}}),&["name","terms","relation","rhs"])}
        }),
        &["schema_version", "name", "variables", "objective", "constraints"],
    )
}
pub(crate) fn definitions() -> Vec<Value> {
    let job_id = json!({"type":"string","pattern":"^[0-9]+$","description":"Exact job_id returned by this MCP process. Jobs cannot be shared between process instances."});
    vec![
        tool(
            "solvercraft_capabilities",
            "Inspect actual SDK engines, mathematical classes and host quotas. Local/heuristic results are not global proofs. No workbook or network daemon is started.",
            object(json!({}), &[]),
            true,
        ),
        tool(
            "solvercraft_start_solve",
            "Start an algebraic optimization job. The response contains job_id; poll solvercraft_job_get. Preserves termination, feasibility and optimality separately, and records the actual engine/options/limits.",
            object(json!({"model":model(),"options":solve_options(),"limits":limits()}), &["model"]),
            false,
        ),
        tool(
            "solvercraft_start_simulation",
            "Start bounded Monte Carlo or Latin-hypercube simulation of typed independent distributions. Inputs use the SDK Distribution tagged object (e.g. {kind:uniform,low:0,high:1}). Outputs are bounded Expr ASTs; omitted outputs expose sampled inputs. No executable code is accepted. Poll job_get for summaries, samples and seed/version manifest.",
            object(
                json!({
                    "inputs":{"type":"array","minItems":1,"maxItems":4096,"items":{"type":"object","required":["kind"],"description":"SDK Distribution; discover supported families with solvercraft_capabilities."}},
                    "outputs":{"type":"array","maxItems":4096,"items":{"type":"object","required":["op"],"description":"SDK Expr, e.g. {op:variable,index:0}, {op:constant,value:2}, or recursive {op:add,left:Expr,right:Expr}."}},
                    "options":object(json!({"trials":{"type":"integer","minimum":1,"maximum":10000000},"seed":{"type":"string","pattern":"^[0-9]+$"},
                        "method":{"type":"string","enum":["monte_carlo","latin_hypercube"]},"retain_samples":{"type":"boolean"},"max_memory_bytes":{"type":"integer","minimum":1}}),&[]),
                    "limits":limits()
                }),
                &["inputs"],
            ),
            false,
        ),
        tool(
            "solvercraft_start_execute",
            "Start the portable schema-2 SDK workflow for ArtCraft objects or other hosts. Supports algebraic and expression models, min/max/target goals, nonlinear derivatives and owned conic routing under actual engine capabilities. Caller context is diagnostic metadata, not authentication. Poll job_get for the ExecutionRun and original-objective qualification.",
            object(
                json!({"request":object(json!({
                "schema_version":{"type":"integer","const":2},
                "model":{"type":"object","required":["schema_version","id","revision","name","definition"],"description":"PortableModel schema2: definition={kind:algebraic|expression,model:typedModel}; options=SolveOptions; optional object/property bindings and metadata. Revisions are decimal u64 strings."},
                "context":object(json!({"run_id":{"type":"string"},"snapshot_id":{"type":"string"},"snapshot_revision":{"type":["string","null"]},
                    "host":{"type":"string"},"source_revision":{"type":["string","null"]}}),&["run_id","snapshot_id","snapshot_revision","host","source_revision"]),
                "goal":{"type":["object","null"],"description":"Tagged {kind:minimize}, {kind:maximize}, or {kind:target,value:number}."},
                "limits":limits(),"nonlinear":{"type":"object","description":"SDK NlpOptions; omitted uses defaults. Unknown fields are rejected."}
            }), &["schema_version","model","context"])}),
                &["request"],
            ),
            false,
        ),
        tool(
            "solvercraft_job_get",
            "Read job status/progress and optional terminal result plus actual configuration manifest. include_result=false keeps polls small. Completed means execution ended; inspect result.ok, termination, feasibility and optimality before using values.",
            object(json!({"job_id":job_id,"include_result":{"type":"boolean","default":true}}), &["job_id"]),
            true,
        ),
        tool(
            "solvercraft_job_cancel",
            "Request cooperative cancellation of a job in this process. Poll until a terminal status; a bounded partial result can be returned with its real termination status.",
            object(json!({"job_id":job_id}), &["job_id"]),
            false,
        ),
        tool(
            "solvercraft_job_forget",
            "Release a terminal job and its memory reservation. A running job must be cancelled and reach a terminal status first.",
            object(json!({"job_id":job_id}), &["job_id"]),
            false,
        ),
    ]
}
