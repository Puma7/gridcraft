//! Run this compiled test WASM with the runner's cwd set to actual Trunk dist.
#![cfg(target_arch = "wasm32")]
use gridcraft_engine::Session;
use gridcraft_engine::core::{CellRef, Value};
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

async fn sleep(milliseconds: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window().expect("browser window").set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds).expect("timer");
    });
    JsFuture::from(promise).await.expect("timer resolved");
}
fn workbook() -> Session {
    // wasm-bindgen-test-runner wraps Worker in a blob to forward console logs.
    // That instrumentation changes classic-worker relative import resolution,
    // breaking Trunk's real loader. Restore the original native constructor
    // from its unmodified prototype, exactly as used by the actual app page.
    // This changes no browser security policy and introduces no mock worker.
    let global = js_sys::global();
    let worker = js_sys::Reflect::get(&global, &JsValue::from_str("Worker")).expect("Worker global");
    let prototype = js_sys::Reflect::get(&worker, &JsValue::from_str("prototype")).expect("Worker prototype");
    let native = js_sys::Reflect::get(&prototype, &JsValue::from_str("constructor")).expect("native Worker constructor");
    assert!(native.is_function());
    js_sys::Reflect::set(&global, &JsValue::from_str("Worker"), &native).expect("restore native transport");
    let mut session = Session::new();
    session.new_workbook();
    session.execute("cell.set", json!({"cell":"A1","input":"2"})).expect("input");
    session.execute("cell.set", json!({"cell":"B1","input":"=2*A1"})).expect("formula");
    session
}
fn value(session: &Session, cell: &str) -> Value {
    session.active().expect("document").wb.active().expect("sheet").value(CellRef::parse(cell).expect("reference"))
}
fn linear_request() -> Json {
    json!({"kind":"optimization","request":{"objective":"B1","variables":"A1","constraints":["A1 <= 10"],"goal":"max","nonNegative":true}})
}
async fn completed(session: &mut Session, id: &Json) -> Json {
    let start = js_sys::Date::now();
    loop {
        let status = session.execute("solvercraft.job.get", json!({"id":id})).expect("status");
        assert_ne!(status["status"], "failed", "{status}");
        if status["status"] == "completed" {
            return status;
        }
        assert!(js_sys::Date::now() - start < 30000.0, "browser worker timed out: {status}");
        sleep(10).await;
    }
}

#[wasm_bindgen_test(async)]
async fn actual_worker_solves_preview_apply_undo_and_rejects_stale_cross_document_use() {
    let mut session = workbook();
    let job = session.execute("solvercraft.job.start", linear_request()).expect("start real worker");
    assert_eq!(job["execution"], "browser_worker_snapshot");
    assert!(job["admission_ms"].as_f64().expect("admission timing") < 25.0, "normal-model admission exceeded 25ms: {job}");
    let id = &job["id"];
    let mut foreign = workbook();
    assert!(foreign.execute("solvercraft.job.get", json!({"id":id})).is_err());
    let status = completed(&mut session, id).await;
    assert!(status["startup_ms"].as_f64().expect("cold worker timing") >= 0.0);
    assert_eq!(status["stale"], false);
    let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("preview");
    assert_eq!(preview["manifest"]["snapshot"], preview["snapshot"]);
    assert_eq!(preview["manifest"]["backend"]["engine"], preview["result"]["engine"]);
    assert_eq!(preview["manifest"]["request_location"], "preview.request");
    assert!(preview["manifest"]["runtime_limits"].is_object());
    assert_eq!(preview["result"]["optimality"], "optimal", "{preview}");
    assert!((preview["result"]["objective"].as_f64().expect("objective") - 20.0).abs() < 1e-7);
    assert_eq!(value(&session, "A1"), Value::Number(2.0));
    assert_eq!(value(&session, "B1"), Value::Number(4.0));
    session.execute("solvercraft.job.apply", json!({"id":id})).expect("explicit apply");
    assert_eq!(value(&session, "A1"), Value::Number(10.0));
    assert_eq!(value(&session, "B1"), Value::Number(20.0));
    session.execute("edit.undo", json!({})).expect("one undo");
    assert_eq!(value(&session, "A1"), Value::Number(2.0));
    assert_eq!(value(&session, "B1"), Value::Number(4.0));
    assert!(session.execute("solvercraft.job.apply", json!({"id":id})).is_err(), "revision changed after apply/undo");
    console_log!(
        "WORKER_TIMING {}",
        json!({"admission_ms":job["admission_ms"],"startup_ms":status["startup_ms"],"worker_compute_ms":preview["worker_compute_ms"]})
    );
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("release result");
}

#[wasm_bindgen_test(async)]
async fn actual_worker_auto_miqp_honors_the_original_integer_domain() {
    let mut session = workbook();
    session.execute("cell.set", json!({"cell":"B1","input":"=(A1-1.5)^2"})).expect("quadratic formula");
    let job = session
        .execute(
            "solvercraft.job.start",
            json!({"kind":"optimization","request":{
                "objective":"B1","goal":"min","method":"auto",
                "variables":[{"cell":"A1","lower":0.0,"upper":3.0,"kind":"integer"}]
            }}),
        )
        .expect("MIQP worker");
    let id = &job["id"];
    completed(&mut session, id).await;
    let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("MIQP result");
    let result = &preview["result"];
    assert_eq!(result["optimality"], "optimal", "{preview}");
    assert_eq!(result["feasibility"], "feasible");
    let x = result["values"][0].as_f64().expect("integer decision");
    assert!(x == 1.0 || x == 2.0, "integer optimum must not be the relaxation x=1.5: {preview}");
    assert!((result["objective"].as_f64().expect("objective") - 0.25).abs() < 1e-7);
    assert_eq!(value(&session, "A1"), Value::Number(2.0));
    session.execute("solvercraft.job.apply", json!({"id":id})).expect("apply original-row-checked MIQP");
    assert_eq!(value(&session, "A1"), Value::Number(x));
    assert_eq!(value(&session, "B1"), Value::Number(0.25));
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("forget MIQP");
    for (goal, formula, expected) in [
        ("max", "=1000000000000*A1+500*C1", 500.0),
        ("max", "=10000000000000000+1000000000000*A1+500*C1", 1e16 + 500.0),
        ("min", "=-1000000000000*A1-500*C1", -500.0),
    ] {
        for (cell, input) in [("A1", "0"), ("C1", "0"), ("B1", formula)] {
            session.execute("cell.set", json!({"cell":cell,"input":input})).expect("binary oracle fixture");
        }
        let job = session
            .execute(
                "solvercraft.job.start",
                json!({"kind":"optimization","request":{
                    "objective":"B1","goal":goal,"method":"auto",
                    "variables":[{"cell":"A1","lower":0.0,"upper":1.0,"kind":"binary"},{"cell":"C1","lower":0.0,"upper":1.0,"kind":"binary"}],
                    "constraints":["1000000000*A1+C1 <= 1"]
                }}),
            )
            .expect("actual near-integer binary worker");
        let id = &job["id"];
        completed(&mut session, id).await;
        let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("binary oracle result");
        assert_eq!(preview["result"]["feasibility"], "feasible");
        assert_eq!(preview["result"]["optimality"], "optimal", "{preview}");
        assert_eq!(preview["result"]["values"], json!([0.0, 1.0]));
        assert_eq!(preview["result"]["objective"].as_f64(), Some(expected));
        assert_eq!(preview["result"]["best_bound"].as_f64(), Some(expected));
        assert_eq!(preview["result"]["relative_gap"].as_f64(), Some(0.0));
        assert_eq!(value(&session, "A1"), Value::Number(0.0));
        assert_eq!(value(&session, "C1"), Value::Number(0.0));
        session.execute("solvercraft.job.forget", json!({"id":id})).expect("forget binary oracle");
    }
}

struct Heartbeat {
    ticks: Rc<Cell<u32>>,
    max_gap_ms: Rc<Cell<f64>>,
    handle: i32,
    _callback: Closure<dyn FnMut()>,
}

#[wasm_bindgen_test(async)]
async fn actual_worker_simulation_preserves_calc_rounding_for_each_output_and_trial() {
    let mut session = workbook();
    for (cell, input) in [
        ("A1", "1"),
        ("A2", "1e16"),
        ("A3", "-1e16"),
        ("E1", "1"),
        ("E2", "1"),
        ("E3", "1"),
        ("B1", "=SUM(1e16,A1,-1e16)"),
        ("C1", "=(1e16+A1)-1e16"),
        ("D1", "=SUMPRODUCT(A1:A3,E1:E3)"),
    ] {
        session.execute("cell.set", json!({"cell":cell,"input":input})).expect("rounding fixture");
    }
    let expected: Vec<f64> = ["B1", "C1", "D1"]
        .iter()
        .map(|cell| match value(&session, cell) {
            Value::Number(number) => number,
            other => panic!("numeric Calc baseline required: {cell}: {other:?}"),
        })
        .collect();
    assert_eq!(expected[1], 0.0, "ordered binary arithmetic baseline");
    session.execute("cell.set", json!({"cell":"A1","input":"2"})).expect("restore live input");
    let live_before: Vec<Value> = ["A1", "B1", "C1", "D1"].iter().map(|cell| value(&session, cell)).collect();
    let job = session
        .execute(
            "solvercraft.job.start",
            json!({"kind":"simulation","request":{
                "inputs":[{"cell":"A1","distribution":{"kind":"constant","value":1.0}}],"outputs":"B1:D1",
                "options":{"trials":16,"seed":"42","retain_samples":true}
            }}),
        )
        .expect("ordered simulation worker");
    let id = &job["id"];
    completed(&mut session, id).await;
    let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("ordered simulation result");
    assert_eq!(preview["result"]["completed_trials"], 16, "{preview}");
    let rows = preview["result"]["samples"].as_array().expect("retained trials");
    assert_eq!(rows.len(), 16);
    for row in rows {
        let actual: Vec<f64> = row.as_array().expect("output row").iter().map(|value| value.as_f64().expect("numeric output")).collect();
        assert_eq!(actual, expected, "each sampled output must preserve actual Calc evaluation order");
    }
    let summaries = preview["result"]["summaries"].as_array().expect("summaries");
    assert_eq!(summaries.len(), expected.len());
    for (summary, expected) in summaries.iter().zip(expected) {
        assert_eq!(summary["mean"].as_f64(), Some(expected));
        assert_eq!(summary["variance"].as_f64(), Some(0.0));
    }
    let live_after: Vec<Value> = ["A1", "B1", "C1", "D1"].iter().map(|cell| value(&session, cell)).collect();
    assert_eq!(live_after, live_before, "all simulation values remain in the worker snapshot");
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("release rounding fixture");
}
#[wasm_bindgen_test(async)]
async fn actual_browser_imports_nineteen_psi_signatures_and_preserves_vendor_context_refusals() {
    // Independent population moments check the imported parameter conventions.
    // The heavy LogNorm2 case is a bounded sampling sanity check, not a claim
    // that a finite sample proves its CDF or every tail moment.
    let cases = [
        ("PsiUniform(-2,6)", 2.0, 16.0 / 3.0),
        ("PsiIntUniform(-2,2)", 0.0, 2.0),
        ("PsiNormal(2,3)", 2.0, 9.0),
        ("PsiLogNormal(4,2)", 4.0, 4.0),
        ("PsiLogNorm2(1,2)", 3.0_f64.exp(), (4.0_f64.exp() - 1.0) * 6.0_f64.exp()),
        ("PsiTriangular(0,2,8)", 10.0 / 3.0, 26.0 / 9.0),
        ("PsiPert(0,2,4)", 2.0, 4.0 / 7.0),
        ("PsiBeta(2,3)", 0.4, 0.04),
        ("PsiGamma(2,3)", 6.0, 18.0),
        ("PsiExponential(2)", 2.0, 4.0),
        ("PsiWeibull(2,3)", 1.5 * std::f64::consts::PI.sqrt(), 9.0 * (1.0 - std::f64::consts::PI / 4.0)),
        ("PsiBernoulli(0.25)", 0.25, 3.0 / 16.0),
        ("PsiBinomial(4,0.25)", 1.0, 0.75),
        ("PsiPoisson(2)", 2.0, 2.0),
        ("PsiNegBinomial(2,0.25)", 6.0, 24.0),
        ("PsiGeometric(0.25)", 3.0, 12.0),
        ("PsiDisUniform({1,2,6})", 3.0, 14.0 / 3.0),
        ("PsiDiscrete({1,10},{3,1})", 3.25, 15.1875),
        ("PsiCumul(0,10,{2,6},{0.2,0.8})", 4.2, 457.0 / 75.0),
    ];
    let mut session = workbook();
    for (index, (formula, _, _)) in cases.iter().enumerate() {
        let row = index + 1;
        session.execute("cell.set", json!({"cell":format!("A{row}"),"input":format!("={formula}")})).expect("vendor input");
        session.execute("cell.set", json!({"cell":format!("B{row}"),"input":format!("=SC.OUTPUT(A{row})")})).expect("declared output");
    }
    session.execute("cell.set", json!({"cell":"C1","input":"=1+2*PsiUniform(0,1)"})).expect("nested vendor formula");
    session.execute("cell.set", json!({"cell":"C2","input":"=PsiNormal(0,1,PsiStatic(4))"})).expect("documented vendor pre-run property");
    let formula = |s: &Session, cell: &str| {
        s.active()
            .expect("document")
            .wb
            .active()
            .expect("sheet")
            .cell(CellRef::parse(cell).expect("reference"))
            .expect("cell")
            .formula
            .as_ref()
            .expect("formula")
            .text
            .clone()
    };
    let original_inputs: Vec<String> = (1..=cases.len()).map(|row| formula(&session, &format!("A{row}"))).collect();
    let unsupported_originals = [formula(&session, "C1"), formula(&session, "C2")];
    let initial_revision = session.active().expect("document").revision;
    let strict = session
        .execute("solvercraft.worksheet.importPsi", json!({"apply":true,"outside_policy":"preserve_vendor"}))
        .expect("honest vendor-context refusal");
    assert!(strict["changes"].as_array().expect("changes").is_empty());
    assert_eq!(strict["unsupported"].as_array().expect("refusals").len(), cases.len() + 2);
    assert!(
        strict["unsupported"]
            .as_array()
            .expect("refusals")
            .iter()
            .all(|entry| entry["reason"] == "vendor_display_context_unavailable" && entry["preserved"] == true)
    );
    assert_eq!(strict["outside_policy"], "preserve_vendor");
    assert_eq!(strict["apply_requested"], true);
    assert_eq!(strict["applied"], false, "a refused conversion is not an applied edit");
    assert_eq!(strict["vendor_display_context_preserved"], false);
    assert_eq!(session.active().expect("document").revision, initial_revision);
    for (index, original) in original_inputs.iter().enumerate() {
        assert_eq!(&formula(&session, &format!("A{}", index + 1)), original);
    }
    let preview = session.execute("solvercraft.worksheet.importPsi", json!({})).expect("read-only import preview");
    assert_eq!(preview["apply_requested"], false);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["outside_policy"], "solvercraft_nominal");
    assert_eq!(preview["vendor_display_context_preserved"], false);
    assert_eq!(preview["changes"].as_array().expect("supported signatures").len(), cases.len());
    assert_eq!(preview["unsupported"].as_array().expect("unsupported formulas").len(), 2);
    assert!(
        preview["unsupported"]
            .as_array()
            .expect("unsupported formulas")
            .iter()
            .all(|entry| entry["reason"] == "unsupported_nested_psi_or_property" && entry["preserved"] == true)
    );
    assert_eq!(session.active().expect("document").revision, initial_revision);
    let applied = session.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).expect("explicit nominal-policy import");
    assert_eq!(applied["apply_requested"], true);
    assert_eq!(applied["applied"], true);
    assert_eq!(session.active().expect("document").revision, initial_revision + 1, "all supported formulas are one edit");
    session.execute("edit.undo", json!({})).expect("undo the complete import");
    for (index, original) in original_inputs.iter().enumerate() {
        assert_eq!(&formula(&session, &format!("A{}", index + 1)), original);
    }
    session.execute("edit.redo", json!({})).expect("redo the complete import");
    for (index, (_, mean, _)) in cases.iter().enumerate() {
        let Value::Number(nominal) = value(&session, &format!("A{}", index + 1)) else { panic!("imported nominal is numeric") };
        assert!((nominal - mean).abs() <= 1e-10 * mean.abs().max(1.0), "incorrect imported nominal for {}: {nominal}", cases[index].0);
    }
    assert_eq!([formula(&session, "C1"), formula(&session, "C2")], unsupported_originals);
    let discovered = session.execute("solvercraft.worksheet.discover", json!({})).expect("discover imported declarations");
    assert_eq!(discovered["inputs"].as_array().expect("input bindings").len(), cases.len());
    assert_eq!(discovered["outputs"].as_array().expect("output bindings").len(), cases.len());
    let distribution = |row: usize| {
        discovered["inputs"]
            .as_array()
            .expect("inputs")
            .iter()
            .find(|input| input["cell"].as_str().expect("input cell").ends_with(&format!("!A{row}")))
            .expect("imported input")["distribution"]
            .clone()
    };
    assert_eq!(distribution(3)["std_dev"], 3.0);
    assert_eq!(distribution(5)["log_mean"], 1.0);
    assert_eq!(distribution(5)["log_std_dev"], 2.0);
    assert_eq!(distribution(9)["shape"], 2.0);
    assert_eq!(distribution(9)["scale"], 3.0);
    assert_eq!(distribution(15)["successes"], 2);
    assert_eq!(distribution(15)["probability"], 0.25);
    let live_before: Vec<Value> = (1..=cases.len()).map(|row| value(&session, &format!("A{row}"))).collect();
    let live_revision = session.active().expect("document").revision;
    let job = session
        .execute("solvercraft.job.start", json!({"kind":"simulation","request":{"options":{"trials":2048,"seed":"42","retain_samples":true}}}))
        .expect("real imported-PSI worker");
    assert_eq!(job["execution"], "browser_worker_snapshot");
    let id = &job["id"];
    completed(&mut session, id).await;
    let result = session.execute("solvercraft.job.result", json!({"id":id})).expect("imported simulation result");
    assert_eq!(result["result"]["completed_trials"], 2048);
    assert_eq!(result["result"]["termination"], "completed");
    let outputs = result["outputs"].as_array().expect("captured output names");
    assert_eq!(outputs.len(), cases.len());
    let summaries = result["result"]["summaries"].as_array().expect("summaries");
    assert_eq!(summaries.len(), cases.len());
    for (output, summary) in outputs.iter().zip(summaries) {
        let cell = output.as_str().expect("output reference").rsplit('!').next().expect("cell reference");
        let index = cell.strip_prefix('B').expect("declared output column").parse::<usize>().expect("output row") - 1;
        let (name, mean, variance) = cases[index];
        assert_eq!(summary["count"], 2048);
        assert!(
            (summary["mean"].as_f64().expect("sample mean") - mean).abs() <= 8.0 * (variance / 2048.0).sqrt(),
            "imported distribution sampling sanity failed for {name}: {summary}"
        );
        assert!(summary["variance"].as_f64().expect("sample variance") > 0.0, "stochastic input cannot be replaced by its outside nominal: {name}");
    }
    let rows = result["result"]["samples"].as_array().expect("retained rows");
    assert_eq!(rows.len(), 2048);
    assert!(rows.iter().all(|row| row.as_array().expect("output row").len() == cases.len()));
    assert_eq!(session.active().expect("document").revision, live_revision);
    assert_eq!((1..=cases.len()).map(|row| value(&session, &format!("A{row}"))).collect::<Vec<_>>(), live_before);
    assert_eq!([formula(&session, "C1"), formula(&session, "C2")], unsupported_originals);
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("release imported-PSI worker");
    session.execute("edit.undo", json!({})).expect("worker queries did not add workbook history");
    for (index, original) in original_inputs.iter().enumerate() {
        assert_eq!(&formula(&session, &format!("A{}", index + 1)), original);
    }
}

impl Heartbeat {
    fn start() -> Self {
        let ticks = Rc::new(Cell::new(0_u32));
        let counter = Rc::clone(&ticks);
        let max_gap_ms = Rc::new(Cell::new(0.0_f64));
        let max_gap = Rc::clone(&max_gap_ms);
        let last_tick = Cell::new(js_sys::Date::now());
        let callback = Closure::wrap(Box::new(move || {
            let now = js_sys::Date::now();
            max_gap.set(max_gap.get().max(now - last_tick.replace(now)));
            counter.set(counter.get().saturating_add(1));
        }) as Box<dyn FnMut()>);
        let handle = web_sys::window()
            .expect("window")
            .set_interval_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), 10)
            .expect("heartbeat timer");
        Self { ticks, max_gap_ms, handle, _callback: callback }
    }
}
impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(self.handle);
        }
    }
}

#[wasm_bindgen_test(async)]
async fn actual_worker_simulation_reports_progress_while_window_timers_remain_responsive() {
    let mut session = workbook();
    let heartbeat = Heartbeat::start();
    let job = session
        .execute(
            "solvercraft.job.start",
            json!({"kind":"simulation","request":{
                "inputs":[{"cell":"A1","distribution":{"kind":"uniform","low":0.0,"high":1.0}}],"outputs":"B1",
                "options":{"trials":2000000,"seed":"42","retain_samples":false,"max_memory_bytes":33554432}
            }}),
        )
        .expect("simulation worker");
    assert!(job["admission_ms"].as_f64().expect("admission timing") < 25.0, "normal-model admission exceeded 25ms: {job}");
    let id = &job["id"];
    let start = js_sys::Date::now();
    let mut saw_progress = false;
    let mut running_tick = None;
    let mut max_status_ms = 0.0_f64;
    let mut last_progress = None;
    let mut last_progress_ms = None;
    let mut max_progress_gap_ms = 0.0_f64;
    let mut observed_progress_updates = 0;
    let status = loop {
        let before = js_sys::Date::now();
        let status = session.execute("solvercraft.job.get", json!({"id":id})).expect("nonblocking status");
        max_status_ms = max_status_ms.max(js_sys::Date::now() - before);
        assert_ne!(status["status"], "failed", "{status}");
        if status["status"] == "running" && running_tick.is_none() {
            running_tick = Some(heartbeat.ticks.get());
            last_progress_ms = Some(js_sys::Date::now());
        }
        saw_progress |= status["progress"].is_object();
        let now = js_sys::Date::now();
        if let Some(last) = last_progress_ms {
            max_progress_gap_ms = max_progress_gap_ms.max(now - last);
        }
        if status["progress"].is_object() && last_progress.as_ref() != Some(&status["progress"]) {
            observed_progress_updates += 1;
            last_progress = Some(status["progress"].clone());
            last_progress_ms = Some(now);
        }
        if status["status"] == "completed" {
            break status;
        }
        assert!(js_sys::Date::now() - start < 30000.0, "{status}");
        sleep(10).await;
    };
    let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("simulation preview");
    assert_eq!(preview["manifest"]["effective_options"]["simulation"]["seed"], "42");
    assert_eq!(preview["manifest"]["backend"]["formula_evaluator"], "gridcraft-ordered-cell-ir-v1");
    assert_eq!(preview["manifest"]["backend"]["sampling_version"], preview["result"]["sampling_version"]);
    assert_eq!(preview["request"]["options"]["max_memory_bytes"], 33554432);
    assert_eq!(preview["result"]["completed_trials"], 2000000, "{preview}");
    assert!(preview["result"]["samples"].is_null());
    let mean = preview["result"]["summaries"][0]["mean"].as_f64().expect("mean");
    assert!((mean - 1.0).abs() < 0.003, "Uniform(0,2) analytic mean");
    assert!(saw_progress, "actual worker must send visible progress");
    assert!(observed_progress_updates >= 2, "long simulation must expose multiple distinct progress updates");
    assert!(max_progress_gap_ms <= 250.0, "observed active-worker progress gap exceeded250ms: {max_progress_gap_ms}");
    assert!(heartbeat.ticks.get() > running_tick.expect("running phase observed"), "window timer must run during computation");
    assert!(heartbeat.max_gap_ms.get() < 25.0, "normal-model window heartbeat gap exceeded25ms: {}", heartbeat.max_gap_ms.get());
    assert!(max_status_ms < 25.0, "normal-model status query exceeded 25ms: {max_status_ms}ms");
    assert_eq!(value(&session, "A1"), Value::Number(2.0));
    console_log!(
        "WORKER_SIM_TIMING {}",
        json!({"admission_ms":job["admission_ms"],"startup_ms":status["startup_ms"],"worker_compute_ms":preview["worker_compute_ms"],"heartbeat_ticks":heartbeat.ticks.get(),"max_window_heartbeat_gap_ms":heartbeat.max_gap_ms.get(),"max_status_ms":max_status_ms,"observed_progress_updates":observed_progress_updates,"max_observed_progress_gap_ms":max_progress_gap_ms})
    );
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("release simulation");
}

#[wasm_bindgen_test(async)]
async fn hard_cancel_and_deadline_never_invent_an_unreceived_incumbent() {
    for deadline in [false, true] {
        let mut session = workbook();
        let mut request = linear_request();
        if deadline {
            request["limits"] = json!({"max_time_millis":0});
        }
        let job = session.execute("solvercraft.job.start", request).expect("start");
        let id = &job["id"];
        if deadline {
            sleep(10).await;
        } else {
            session.execute("solvercraft.job.cancel", json!({"id":id})).expect("hard cancel");
        }
        completed(&mut session, id).await;
        let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("typed interrupted result");
        assert_eq!(preview["result"]["termination"], if deadline { "time_limit" } else { "cancelled" });
        assert_eq!(preview["result"]["feasibility"], "unknown");
        assert_eq!(preview["result"]["optimality"], "unknown");
        assert!(preview["result"]["values"].is_null() && preview["result"]["objective"].is_null());
        assert_eq!(preview["manifest"]["backend"]["result_observed"], false);
        assert!(preview["manifest"]["backend"]["engine"].is_null(), "hard cancellation must not attest an unseen engine");
        assert!(session.execute("solvercraft.job.apply", json!({"id":id})).is_err());
        assert_eq!(value(&session, "A1"), Value::Number(2.0));
        session.execute("solvercraft.job.forget", json!({"id":id})).expect("forget interrupted job");
    }
}

#[wasm_bindgen_test(async)]
async fn editing_the_live_workbook_makes_worker_result_stale_without_changing_its_snapshot() {
    let mut session = workbook();
    let job = session.execute("solvercraft.job.start", linear_request()).expect("start");
    let id = &job["id"];
    session.execute("cell.set", json!({"cell":"A1","input":"3"})).expect("edit live workbook");
    assert_eq!(completed(&mut session, id).await["stale"], true);
    let preview = session.execute("solvercraft.job.result", json!({"id":id})).expect("old snapshot result");
    assert_eq!(preview["result"]["optimality"], "optimal");
    assert!(session.execute("solvercraft.job.apply", json!({"id":id})).is_err());
    assert_eq!(value(&session, "A1"), Value::Number(3.0));
    assert_eq!(value(&session, "B1"), Value::Number(6.0));
    session.execute("solvercraft.job.forget", json!({"id":id})).expect("forget stale job");
}

#[wasm_bindgen_test(async)]
async fn actual_trunk_app_canvas_loads_and_renders_in_the_same_browser() {
    let document = web_sys::window().expect("window").document().expect("document");
    let iframe: web_sys::HtmlIFrameElement = document.create_element("iframe").expect("frame element").dyn_into().expect("iframe type");
    iframe.set_src("/index.html?sample=sales");
    iframe.set_id("solvercraft-app-acceptance-frame");
    iframe
        .set_attribute("style", "position:fixed;inset:0;width:100%;height:100%;border:0;z-index:2147483647;background:white")
        .expect("frame layout");
    document.body().expect("body").append_child(&iframe).expect("attach actual app");
    let start = js_sys::Date::now();
    loop {
        if let Some(app) = iframe.content_document() {
            let canvas = app.get_element_by_id("gridcraft_canvas");
            let loading = app.get_element_by_id("gridcraft_loading");
            if canvas.is_some() && loading.is_none() {
                break;
            }
        }
        assert!(js_sys::Date::now() - start < 30000.0, "actual Trunk app did not initialize its canvas");
        sleep(50).await;
    }
    // Allow actual egui frames, then expose only a test-page DOM readiness
    // marker so the external WebDriver can capture the real rendered canvas.
    sleep(500).await;
    let app = iframe.content_document().expect("same-origin actual app");
    let canvas = app.get_element_by_id("gridcraft_canvas").expect("canvas");
    console_log!(
        "ACTUAL_APP_CANVAS {}",
        json!({"iframe_width":iframe.client_width(),"iframe_height":iframe.client_height(),"canvas_width":canvas.get_attribute("width"),"canvas_height":canvas.get_attribute("height")})
    );
    iframe.set_attribute("data-solvercraft-app-ready", "true").expect("test page readiness");
    console_log!("ACTUAL_TRUNK_APP_READY {}ms", js_sys::Date::now() - start);
    sleep(2500).await;
    iframe.remove();
}
