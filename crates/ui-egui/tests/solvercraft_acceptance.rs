//! Actual egui input, asynchronous preview, explicit apply and workbook undo.
#![cfg(not(target_arch = "wasm32"))]
#[path = "common/mod.rs"]
mod common;

use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gridcraft_engine::Session;
use gridcraft_engine::core::{CellRef, Value};
use gridcraft_ui_egui::i18n::Language;
use serde_json::json;
use std::time::Duration;
use web_time::Instant;

fn harness() -> common::Harness {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("cell.set", json!({"cell":"A1","input":"2"})).unwrap();
    session.execute("cell.set", json!({"cell":"B1","input":"=2*A1"})).unwrap();
    let mut h = common::os_harness(session, egui::os::OperatingSystem::Windows);
    h.set_size(egui::vec2(1600.0, 1000.0));
    h.run_steps(3);
    h
}

fn value(harness: &common::Harness, reference: &str) -> Value {
    harness.state().session.active().unwrap().wb.active().unwrap().value(CellRef::parse(reference).unwrap())
}

#[allow(clippy::disallowed_methods, reason = "This egui integration test is excluded on wasm; web_time aliases the native std clock.")]
fn start_preview(h: &mut common::Harness) {
    configure_optimization(h);
    h.state_mut().run("ui.solvercraft", json!({"on":true})).unwrap();
    h.run_steps(3);
    h.get_by_label("Calculate optimization").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Calculate optimization").click();
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.query_by_label("Apply solution (one undo step)").is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.query_by_label("Apply solution (one undo step)").is_some(), "preview did not complete within ten seconds");
}

fn configure_optimization(h: &mut common::Harness) {
    let model = h
        .state_mut()
        .run(
            "solvercraft.model.create",
            json!({
                "name": "Fixture optimization", "kind": "optimization",
                "request": {"objective": "B1", "variables": "A1", "constraints": ["A1 <= 10"], "goal": "max"}
            }),
        )
        .unwrap();
    let ctx = h.ctx.clone();
    gridcraft_ui_egui::solvercraft::open_model(h.state_mut(), &ctx, model["id"].as_str().unwrap()).unwrap();
    h.run_steps(3);
}

fn click_control(h: &mut common::Harness, caption: &str) {
    use egui::accesskit::Role;
    h.query_by_role_and_label(Role::TextInput, caption)
        .or_else(|| h.query_by_role_and_label(Role::Button, caption))
        .or_else(|| h.query_by_role_and_label(Role::ComboBox, caption))
        .unwrap_or_else(|| h.get_by_label(caption))
        .scroll_to_me();
    h.run_steps(5);
    h.query_by_role_and_label(Role::TextInput, caption)
        .or_else(|| h.query_by_role_and_label(Role::Button, caption))
        .or_else(|| h.query_by_role_and_label(Role::ComboBox, caption))
        .unwrap_or_else(|| h.get_by_label(caption))
        .click();
    h.run_steps(4);
}

fn type_control(h: &mut common::Harness, caption: &str, text: &str) {
    click_control(h, caption);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(2);
    h.input_mut().events.push(egui::Event::Text(text.into()));
    h.run_steps(3);
    h.key_press(egui::Key::Tab);
    h.run_steps(3);
}

#[allow(clippy::disallowed_methods, reason = "Native UI test waits on a bounded background job.")]
fn wait_for_control(h: &mut common::Harness, caption: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while h.query_by_label(caption).is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.query_by_label(caption).is_some(), "missing completed run control: {caption}");
}

fn forget_jobs(h: &mut common::Harness) {
    let jobs = h.state_mut().run("solvercraft.job.list", json!({})).unwrap();
    for job in jobs["jobs"].as_array().unwrap() {
        h.state_mut().run("solvercraft.job.forget", json!({"id": job["id"]})).unwrap();
    }
}

#[test]
fn a_user_builds_an_integer_optimization_without_json() {
    let mut h = harness();
    h.state_mut().run("range.setValues", json!({"range": "A1:C1", "values": [[0, 0, "=3*A1+5*B1"]]})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on": true})).unwrap();
    h.run_steps(4);
    assert!(h.get_by_label("Calculate optimization").accesskit_node().is_disabled());
    h.state_mut().run("selection.set", json!({"cell": "C1"})).unwrap();
    h.run_steps(3);
    click_control(&mut h, "Use selection as objective");
    click_control(&mut h, "Maximize");
    h.state_mut().run("selection.set", json!({"range": "A1:B1"})).unwrap();
    h.run_steps(3);
    click_control(&mut h, "Use selection as decision cells");
    click_control(&mut h, "Variable type 1");
    click_control(&mut h, "Whole numbers");
    click_control(&mut h, "Variable 1 lower enabled");
    type_control(&mut h, "Variable 1 lower", "0");
    click_control(&mut h, "Variable 1 upper enabled");
    type_control(&mut h, "Variable 1 upper", "6");
    click_control(&mut h, "Add a condition");
    for (left, right) in [("A1", "4"), ("B1", "6"), ("3*A1+2*B1", "18")] {
        type_control(&mut h, "Left cell or expression", left);
        type_control(&mut h, "Right cell or value", right);
        click_control(&mut h, "Add constraint");
    }
    // Edit and remove ordinary rows, then restore the intended third limit.
    type_control(&mut h, "Limit or cell 3", "15");
    type_control(&mut h, "Limit or cell 3", "18");
    type_control(&mut h, "Left cell or expression", "A1");
    type_control(&mut h, "Right cell or value", "100");
    click_control(&mut h, "Add constraint");
    click_control(&mut h, "Remove condition 4");
    assert!(h.query_by_label("Solvercraft constraints").is_none());
    assert!(h.query_by_label("Decision cells JSON").is_none());
    click_control(&mut h, "Save model");
    let models = h.state_mut().run("solvercraft.model.list", json!({})).unwrap();
    let model = h.state_mut().run("solvercraft.model.get", json!({"id": models["models"][0]["id"]})).unwrap();
    assert_eq!(model["request"]["variables"][0]["kind"], "integer");
    assert_eq!(model["request"]["variables"][0]["upper"], 6.0);
    assert_eq!(model["request"]["constraints"].as_array().unwrap().len(), 3);
    click_control(&mut h, "Calculate optimization");
    wait_for_control(&mut h, "Apply solution (one undo step)");
    assert_eq!(value(&h, "A1"), Value::Number(0.0));
    click_control(&mut h, "Apply solution (one undo step)");
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
    assert_eq!(value(&h, "B1"), Value::Number(6.0));
    assert_eq!(value(&h, "C1"), Value::Number(36.0));
    forget_jobs(&mut h);
}

#[test]
fn a_user_builds_a_simulation_and_selects_each_output_without_json() {
    let mut h = harness();
    h.state_mut().run("range.setValues", json!({"range": "A1:C1", "values": [[0, "=2*A1", "=100+A1"]]})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on": true})).unwrap();
    h.run_steps(4);
    click_control(&mut h, "Simulation");
    assert!(h.get_by_label("Run simulation").accesskit_node().is_disabled());
    h.state_mut().run("selection.set", json!({"cell": "A1"})).unwrap();
    click_control(&mut h, "Add uncertain input");
    click_control(&mut h, "Distribution 1");
    click_control(&mut h, "Normal");
    type_control(&mut h, "Input 1 · Mean", "10");
    type_control(&mut h, "Input 1 · Standard deviation", "-1");
    assert!(h.get_by_label("Run simulation").accesskit_node().is_disabled());
    type_control(&mut h, "Input 1 · Standard deviation", "2");
    h.state_mut().run("selection.set", json!({"range": "B1:C1"})).unwrap();
    h.run_steps(3);
    click_control(&mut h, "Use selection as outputs");
    type_control(&mut h, "Number of simulation trials", "256");
    assert!(h.query_by_label("Solvercraft simulation inputs").is_none());
    assert!(h.query_by_label("Random seed").is_none());
    click_control(&mut h, "Save model");
    click_control(&mut h, "Run simulation");
    wait_for_control(&mut h, "Simulation chart type");
    assert!(
        h.query_by_label("Apply solution (one undo step)").is_none(),
        "simulation offers its result sheet instead of an unavailable optimization action"
    );
    let jobs = h.state_mut().run("solvercraft.job.list", json!({})).unwrap();
    let preview = h.state_mut().run("solvercraft.job.result", json!({"id": jobs["jobs"][0]["id"]})).unwrap();
    assert_eq!(preview["result"]["completed_trials"], 256);
    assert_eq!(preview["request"]["inputs"][0]["distribution"], json!({"kind": "normal", "mean": 10.0, "std_dev": 2.0}));
    let first_mean = preview["result"]["summaries"][0]["mean"].as_f64().unwrap();
    let second_mean = preview["result"]["summaries"][1]["mean"].as_f64().unwrap();
    assert!(h.query_by_label(&format!("Mean  {}", gridcraft_engine::core::number_to_text(first_mean))).is_some());
    let second = preview["outputs"][1].as_str().unwrap();
    click_control(&mut h, "Result cell");
    click_control(&mut h, second);
    assert!(h.query_by_label(&format!("Mean  {}", gridcraft_engine::core::number_to_text(second_mean))).is_some());
    assert_eq!(value(&h, "A1"), Value::Number(0.0), "simulation leaves source inputs unchanged");
    click_control(&mut h, "Create result sheet");
    assert_eq!(value(&h, "C5"), Value::Number(256.0));
    forget_jobs(&mut h);
}

#[test]
fn focusing_numeric_bounds_does_not_change_their_values() {
    let mut h = harness();
    for (lower, upper) in [(1e-12, 2e-12), (1.234567890123456, 2e300)] {
        let model = h
            .state_mut()
            .run(
                "solvercraft.model.create",
                json!({
                "name": format!("Physical quantities {lower}"), "kind": "optimization",
                "request": {"objective":"B1", "variables":[{"cell":"A1", "lower":lower, "upper":upper}], "goal":"max"}
                    }),
            )
            .unwrap();
        let ctx = h.ctx.clone();
        gridcraft_ui_egui::solvercraft::open_model(h.state_mut(), &ctx, model["id"].as_str().unwrap()).unwrap();
        h.run_steps(4);
        for caption in ["Variable 1 lower", "Variable 1 upper"] {
            click_control(&mut h, caption);
            h.key_press(egui::Key::Tab);
            h.run_steps(4);
        }
        click_control(&mut h, "Save model");
        let saved = h.state_mut().run("solvercraft.model.get", json!({"id":model["id"]})).unwrap();
        assert_eq!(saved["request"]["variables"][0]["lower"].as_f64(), Some(lower));
        assert_eq!(saved["request"]["variables"][0]["upper"].as_f64(), Some(upper));
    }
}

#[test]
fn empirical_controls_follow_expert_edits_and_reject_incomplete_observations() {
    let mut h = harness();
    let model = h
        .state_mut()
        .run(
            "solvercraft.model.create",
            json!({
                "name": "Observed demand", "kind": "simulation",
                "request": {"inputs": [{"cell": "A1", "distribution": {"kind": "empirical", "values": [1.0, 2.0]}}], "outputs": "B1"}
            }),
        )
        .unwrap();
    let ctx = h.ctx.clone();
    gridcraft_ui_egui::solvercraft::open_model(h.state_mut(), &ctx, model["id"].as_str().unwrap()).unwrap();
    h.run_steps(4);
    click_control(&mut h, "Expert settings");
    click_control(&mut h, "Raw simulation configuration");
    type_control(&mut h, "Solvercraft simulation inputs", r#"[{"cell":"A1","distribution":{"kind":"empirical","values":[10,20]}}]"#);
    assert_eq!(h.get_by_label("Observed values 1").value().as_deref(), Some("10\n20"));
    type_control(&mut h, "Observed values 1", "10\n-");
    assert_eq!(h.get_by_label("Observed values 1").value().as_deref(), Some("10\n-"));
    assert!(h.get_by_label("Run simulation").accesskit_node().is_disabled());
    type_control(&mut h, "Observed values 1", "10\n30");
    click_control(&mut h, "Save model");
    let saved = h.state_mut().run("solvercraft.model.get", json!({"id":model["id"]})).unwrap();
    assert_eq!(saved["request"]["inputs"][0]["distribution"]["values"], json!([10.0, 30.0]));
}

#[test]
fn an_infeasible_custom_model_explains_why_no_solution_can_be_applied() {
    let mut h = harness();
    let model = h
        .state_mut()
        .run(
            "solvercraft.model.create",
            json!({
                "name":"Conflicting limits", "kind":"optimization",
                "request":{"objective":"B1","variables":"A1","constraints":["B1 <= 1","B1 >= 2"],"goal":"max"}
            }),
        )
        .unwrap();
    let ctx = h.ctx.clone();
    gridcraft_ui_egui::solvercraft::open_model(h.state_mut(), &ctx, model["id"].as_str().unwrap()).unwrap();
    h.run_steps(4);
    click_control(&mut h, "Calculate optimization");
    wait_for_control(&mut h, "No solution satisfies all conditions. Check the limits and conflicting conditions.");
    assert!(h.get_by_label("Apply solution (one undo step)").accesskit_node().is_disabled());
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
    forget_jobs(&mut h);
}

#[test]
fn ribbon_opens_the_real_pane_after_solver() {
    let mut h = harness();
    h.state_mut().ui.ribbon_tab = "Data".into();
    h.run_steps(3);
    let solver = h.get_by_label("Solver").rect();
    let solvercraft = h.get_by_label("Solvercraft").rect();
    assert!(solvercraft.left() >= solver.right(), "Solvercraft follows Solver in the ribbon");
    h.get_by_label("Solvercraft").click();
    h.run_steps(3);
    assert_eq!(h.state().grid.pane.as_deref(), Some("solvercraft"));
    assert!(h.query_all_by_label("Objective cell or formula").next().is_some());
    assert!(h.unnamed_widgets().is_empty(), "visible controls lack accessible names: {:?}", h.unnamed_widgets());
    assert!(h.state().dialog.is_none());
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
}

#[test]
#[allow(clippy::disallowed_methods, reason = "Native-only UI test; web_time aliases the native monotonic clock.")]
fn cold_storage_template_opens_and_runs_all_three_models() {
    fn wait_for(h: &mut common::Harness, label: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while h.query_by_label(label).is_none() && Instant::now() < deadline {
            h.run_steps(4);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            h.query_by_label(label).is_some(),
            "missing completed job control: {label}; jobs: {:?}",
            h.state_mut().run("solvercraft.job.list", json!({}))
        );
    }

    let mut h = harness();
    h.state_mut().ui.ribbon_tab = "Data".into();
    h.run_steps(3);
    click_control(&mut h, "Solvercraft");
    click_control(&mut h, "Open Microgrid");
    let doc = h.state().session.active().unwrap();
    let first_copy = doc.uid;
    assert!(doc.path.is_none());
    assert_eq!(doc.wb.active().unwrap().name, "Dashboard");
    assert_eq!(doc.wb.active().unwrap().charts.len(), 3);
    assert_eq!(h.state().session.documents().len(), 2);
    click_control(&mut h, "Model settings");
    assert_eq!(h.get_by_label("Saved models").value().as_deref(), Some("1 · Optimize cost plan"));

    for (model, sheet, objective) in
        [("1 · Optimize cost plan", "CostPlan", 1069.3404838709675), ("2 · Optimize reserve plan", "ReservePlan", 1172.274494623656)]
    {
        // Start with a broken energy balance so the test verifies that Apply writes the solution.
        h.state_mut().run("cell.set", json!({"sheet": sheet, "cell": "E11", "input": "0"})).unwrap();
        click_control(&mut h, "Saved models");
        click_control(&mut h, model);
        click_control(&mut h, "Calculate optimization");
        wait_for(&mut h, "Apply solution (one undo step)");
        assert!(!h.get_by_label("Apply solution (one undo step)").accesskit_node().is_disabled());
        click_control(&mut h, "Apply solution (one undo step)");
        let wb = &h.state().session.active().unwrap().wb;
        let plan = wb.sheet(wb.sheet_index(sheet).unwrap()).unwrap();
        let Value::Number(cost) = plan.value(CellRef::parse("B7").unwrap()) else { panic!("missing objective") };
        assert!((cost - objective).abs() < 1e-6);
        assert_eq!(plan.value(CellRef::parse("N7").unwrap()), Value::Number(0.0));
    }

    click_control(&mut h, "Saved models");
    click_control(&mut h, "3 · Simulate both plans");
    click_control(&mut h, "Run simulation");
    wait_for(&mut h, "Create result sheet");
    click_control(&mut h, "Create result sheet");
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().name, "Solver Result 1");
    assert_eq!(value(&h, "C5"), Value::Number(4096.0));
    // Release retained jobs so this fixture does not consume the process-wide test quota.
    let jobs = h.state_mut().run("solvercraft.job.list", json!({})).unwrap();
    for job in jobs["jobs"].as_array().unwrap() {
        h.state_mut().run("solvercraft.job.forget", json!({"id": job["id"]})).unwrap();
    }

    click_control(&mut h, "Open Microgrid");
    let doc = h.state().session.active().unwrap();
    assert_ne!(doc.uid, first_copy);
    assert!(doc.path.is_none());
    assert_eq!(doc.wb.sheets.len(), 6);
    assert_eq!(h.state().session.documents().len(), 3);
}

#[test]
#[allow(clippy::disallowed_methods, reason = "Native UI job test uses the monotonic native clock.")]
fn guided_microgrid_runs_applies_and_updates_the_dashboard() {
    let mut h = harness();
    h.state_mut().run("file.new", json!({"sample": "cold-storage-microgrid"})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on": true})).unwrap();
    h.state_mut().run("cell.set", json!({"sheet": "CostPlan", "cell": "E11", "input": "0"})).unwrap();
    h.state_mut().run("cell.set", json!({"sheet": "Assumptions", "cell": "B25", "input": "0.45"})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Objective cell or formula").is_none());
    h.get_by_label("Run full comparison").click();
    let deadline = Instant::now() + Duration::from_secs(30);
    while h.query_by_label("Results from this run").is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.query_by_label("Results from this run").is_some(), "the guided run did not finish");
    let wb = &h.state().session.active().unwrap().wb;
    for sheet in ["CostPlan", "ReservePlan"] {
        let plan = wb.sheet(wb.sheet_index(sheet).unwrap()).unwrap();
        assert_eq!(plan.value(CellRef::parse("N7").unwrap()), Value::Number(0.0));
    }
    let dashboard = wb.sheet(wb.sheet_index("Dashboard").unwrap()).unwrap();
    let Value::Number(gap) = dashboard.value(CellRef::parse("C13").unwrap()) else { panic!("missing simulated gap") };
    assert!((gap - 0.45).abs() <= 1.0 / 4096.0, "outage frequency must follow the changed probability: {gap}");
    assert_eq!(dashboard.value(CellRef::parse("C50").unwrap()), Value::Number(gap));
    h.get_by_label("Open result sheet").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Open result sheet").click();
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().name, "Solver Result 1");
    assert_eq!(value(&h, "C5"), Value::Number(4096.0));
    h.state_mut().run("edit.undo", json!({})).unwrap();
    h.run_steps(4);
    h.get_by_label("Open result sheet").click();
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().name, "Solver Result 1");
    h.state_mut().run("cell.set", json!({"sheet": "Assumptions", "cell": "B25", "input": "0.1"})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Workbook changed. Run again to refresh the comparison.").is_some());
    // A completed report can be reopened without disguising subsequently changed inputs.
    h.get_by_label("Open result sheet").click();
    h.run_steps(4);
    assert!(h.query_by_label("Workbook changed. Run again to refresh the comparison.").is_some());
    h.get_by_label("Run full comparison").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Run full comparison").click();
    h.run_steps(4);
    let deadline = Instant::now() + Duration::from_secs(30);
    while h.query_by_label("Results from this run").is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.query_by_label("Results from this run").is_some());
    assert!(h.query_by_label("Workbook changed. Run again to refresh the comparison.").is_none());
    let wb = &h.state().session.active().unwrap().wb;
    let dashboard = wb.sheet(wb.sheet_index("Dashboard").unwrap()).unwrap();
    let Value::Number(gap) = dashboard.value(CellRef::parse("C13").unwrap()) else { panic!("missing refreshed gap") };
    assert!((gap - 0.1).abs() <= 1.0 / 4096.0);
    let jobs = h.state_mut().run("solvercraft.job.list", json!({})).unwrap();
    assert_eq!(jobs["jobs"].as_array().unwrap().len(), 1, "completed optimizations release their retained jobs");
    for job in jobs["jobs"].as_array().unwrap() {
        h.state_mut().run("solvercraft.job.forget", json!({"id": job["id"]})).unwrap();
    }
}

#[test]
#[allow(clippy::disallowed_methods, reason = "Native UI job test uses the monotonic native clock.")]
fn guided_microgrid_stops_when_inputs_change_during_a_run() {
    let mut h = harness();
    h.state_mut().run("file.new", json!({"sample": "cold-storage-microgrid"})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on": true})).unwrap();
    h.run_steps(4);
    h.get_by_label("Run full comparison").click();
    h.run_steps(1);
    h.state_mut().run("cell.set", json!({"sheet": "CostPlan", "cell": "E11", "input": "0"})).unwrap();
    h.run_steps(20);
    assert!(h.query_by_label("Workbook changed during the comparison. Run again to use the new inputs.").is_some());
    let wb = &h.state().session.active().unwrap().wb;
    assert_eq!(wb.sheet(wb.sheet_index("CostPlan").unwrap()).unwrap().value(CellRef::parse("E11").unwrap()), Value::Number(0.0));
    // Closing the fixture cancels and releases its owned worker.
    h.state_mut().run("file.close", json!({"force": true, "document": true})).unwrap();
}

#[test]
fn pane_preview_requires_apply_and_reverts_in_one_undo_step() {
    let mut h = harness();
    start_preview(&mut h);
    assert_eq!(value(&h, "A1"), Value::Number(2.0), "preview must leave original inputs intact");
    let disabled = h.get_by_label("Apply solution (one undo step)").accesskit_node().is_disabled();
    assert!(!disabled, "completed preview cannot be applied: {:?}", h.state_mut().run("solvercraft.job.list", json!({})));
    h.get_by_label("Apply solution (one undo step)").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Apply solution (one undo step)").click();
    h.run_steps(3);
    assert!(h.query_by_label("Solution applied to the workbook.").is_some());
    assert_eq!(value(&h, "A1"), Value::Number(10.0));
    assert_eq!(value(&h, "B1"), Value::Number(20.0));
    h.get_by_label("Create result sheet").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Create result sheet").click();
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().name, "Solver Result 1");
    assert_eq!(value(&h, "C13"), Value::Number(2.0));
    assert_eq!(value(&h, "D13"), Value::Number(10.0));
    h.state_mut().run("edit.undo", json!({})).unwrap();
    h.run_steps(4);
    assert_eq!(value(&h, "A1"), Value::Number(10.0));
    h.state_mut().run("edit.undo", json!({})).unwrap();
    h.run_steps(3);
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
    assert_eq!(value(&h, "B1"), Value::Number(4.0));
}

#[test]
fn pane_picks_a_cell_and_disjoint_ranges_on_different_sheets() {
    use egui::accesskit::Role;
    let mut h = harness();
    let app = h.state_mut();
    app.run("home.insertSheet", json!({"name":"Model input"})).unwrap();
    app.run("selection.set", json!({"cell":"D5"})).unwrap();
    app.run("ui.solvercraft", json!({"on":true})).unwrap();
    h.run_steps(4);
    h.get_by_label("Use selection as objective").click();
    h.run_steps(3);
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Objective cell or formula").value().as_deref(), Some("'Model input'!D5"));
    let app = h.state_mut();
    app.run("home.insertSheet", json!({"name":"Lots"})).unwrap();
    app.run("selection.set", json!({"range":"B2:B3,D4"})).unwrap();
    h.run_steps(4);
    assert!(h.get_by_label("Use selection as objective").accesskit_node().is_disabled());
    h.get_by_label("Use selection as decision cells").click();
    h.run_steps(3);
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Decision cells").value().as_deref(), Some("(Lots!B2:B3,Lots!D4)"));
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Objective cell or formula").value().as_deref(), Some("'Model input'!D5"));
}

#[test]
fn workbook_edit_disables_apply_for_a_stale_preview() {
    let mut h = harness();
    start_preview(&mut h);
    assert!(!h.get_by_label("Apply solution (one undo step)").accesskit_node().is_disabled());
    h.state_mut().run("cell.set", json!({"cell":"A1","input":"3"})).unwrap();
    h.run_steps(20);
    assert!(h.get_by_label("Apply solution (one undo step)").accesskit_node().is_disabled(), "stale preview must visibly disable Apply");
    assert!(h.query_by_label("Workbook changed: recalculate to apply. You can still archive this run.").is_some());
    assert_eq!(value(&h, "A1"), Value::Number(3.0));
    assert_eq!(value(&h, "B1"), Value::Number(6.0));
}

#[test]
fn pane_command_honors_visibility_and_german_controls() {
    let mut h = harness();
    h.state_mut().ui.language = Language::De;
    h.state_mut().run("ui.solvercraft", json!({"on":true})).unwrap();
    h.run_steps(3);
    assert!(h.query_all_by_label("Zielzelle oder Formel").next().is_some());
    assert!(h.query_by_label("Optimierung berechnen").is_some());
    assert!(h.unnamed_widgets().is_empty(), "visible controls lack accessible names: {:?}", h.unnamed_widgets());
    h.state_mut().run("ui.solvercraft", json!({"on":false})).unwrap();
    h.run_steps(3);
    assert!(h.query_all_by_label("Zielzelle oder Formel").next().is_none());
    assert_eq!(h.state().grid.pane, None);
}

#[test]
#[allow(clippy::disallowed_methods, reason = "Native-only UI test; web_time aliases the native monotonic clock.")]
fn pane_keyboard_focus_and_preview_shortcut_work_in_both_themes_and_languages() {
    use egui::{Event, Key, Modifiers, accesskit::Role};
    for language in [Language::En, Language::De] {
        for dark in [false, true] {
            let mut h = harness();
            configure_optimization(&mut h);
            h.state_mut().ui.language = language;
            h.state_mut().ui.system_theme = false;
            h.state_mut().ui.dark = dark;
            h.state_mut().run("ui.solvercraft", json!({"on":true})).unwrap();
            h.run_steps(4);
            assert_eq!(h.ctx.theme(), if dark { egui::Theme::Dark } else { egui::Theme::Light });
            assert!(h.unnamed_widgets().is_empty(), "{language:?} dark={dark}: {:?}", h.unnamed_widgets());
            let objective = if language == Language::De { "Zielzelle oder Formel" } else { "Objective cell or formula" };
            h.get_by_role_and_label(Role::TextInput, objective).click();
            h.run_steps(2);
            assert!(h.get_by_role_and_label(Role::TextInput, objective).is_focused());
            let initial_focus = h.ctx.memory(|memory| memory.focused());
            h.key_press(Key::Tab);
            h.run_steps(2);
            assert!(h.ctx.memory(|memory| memory.focused()).is_some());
            assert_ne!(h.ctx.memory(|memory| memory.focused()), initial_focus, "Tab must move keyboard focus");
            h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
            h.run_steps(2);
            assert!(h.get_by_role_and_label(Role::TextInput, objective).is_focused(), "Shift+Tab must return to the objective field");
            h.key_press_modifiers(Modifiers::COMMAND, Key::A);
            h.run_steps(1);
            h.input_mut().events.push(Event::Text("=2*A1".into()));
            h.run_steps(2);
            assert_eq!(h.get_by_role_and_label(Role::TextInput, objective).value().as_deref(), Some("=2*A1"));
            assert_eq!(value(&h, "A1"), Value::Number(2.0), "pane typing must not edit the selected workbook cell");
            h.key_press_modifiers(Modifiers::CTRL, Key::Enter);
            let apply =
                if language == Language::De { "L\u{00f6}sung \u{00fc}bernehmen (ein Undo-Schritt)" } else { "Apply solution (one undo step)" };
            let deadline = Instant::now() + Duration::from_secs(10);
            while h.query_by_label(apply).is_none() && Instant::now() < deadline {
                h.run_steps(4);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(h.query_by_label(apply).is_some(), "Ctrl+Enter must start a preview for {language:?}, dark={dark}");
            assert!(!h.get_by_label(apply).accesskit_node().is_disabled());
            assert_eq!(value(&h, "A1"), Value::Number(2.0));
            assert_eq!(value(&h, "B1"), Value::Number(4.0));
        }
    }
}

#[allow(clippy::disallowed_methods, reason = "Native-only UI test; web_time aliases the native monotonic clock.")]
fn simulate_saved_model(h: &mut common::Harness, name: &str, request: serde_json::Value, ready_label: &str) -> serde_json::Value {
    h.state_mut().run("solvercraft.model.create", json!({"name":name,"kind":"simulation","request":request})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on":true})).unwrap();
    h.run_steps(3);
    h.get_by_label("Refresh models").click();
    h.run_steps(2);
    click_control(h, "Saved models");
    click_control(h, name);
    h.run_steps(2);
    h.get_by_label("Run simulation").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Run simulation").click();
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.query_by_label(ready_label).is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    h.get_by_label(ready_label).scroll_to_me();
    h.run_steps(12);
    assert!(h.query_by_label(ready_label).is_some(), "simulation did not expose {ready_label}");
    let list = h.state_mut().run("solvercraft.job.list", json!({})).unwrap();
    let jobs = list["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    h.state_mut().run("solvercraft.job.result", json!({"id":jobs[0]["id"]})).unwrap()
}

#[test]
fn simulation_chart_choices_and_parameter_tornado_use_completed_preview_data() {
    let mut h = harness();
    h.state_mut().run("cell.set", json!({"cell":"B1","input":"=A1"})).unwrap();
    h.state_mut().run("cell.set", json!({"cell":"C1","input":"1e300"})).unwrap();
    let preview = simulate_saved_model(
        &mut h,
        "Chart acceptance",
        json!({
            "inputs":[{"cell":"A1","distribution":{"kind":"uniform","low":1.0,"high":3.0}}],
            "outputs":"B1:C1","options":{"trials":64,"seed":"42","retain_samples":true,"max_memory_bytes":8388608}
        }),
        "Simulation chart type",
    );
    let samples = preview["result"]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 64);
    let mut sorted = Vec::new();
    for row in samples {
        let x = row[0].as_f64().unwrap();
        assert!((1.0..=3.0).contains(&x));
        assert_eq!(row[1], 1e300);
        sorted.push(x);
    }
    sorted.sort_by(f64::total_cmp);
    for pair in preview["result"]["summaries"][0]["quantiles"].as_array().unwrap() {
        let probability = pair[0].as_f64().unwrap();
        let rank = ((64.0 * probability).ceil() as usize).saturating_sub(1);
        assert_eq!(pair[1].as_f64().unwrap(), sorted[rank], "inverse empirical CDF from actual retained trials");
    }
    for kind in ["CDF", "Scatter", "Box", "Quantile bands", "Histogram"] {
        h.get_by_label("Simulation chart type").scroll_to_me();
        h.run_steps(20);
        let center = h.get_by_label("Simulation chart type").rect().center();
        assert!(h.ctx.content_rect().contains(center), "chart control did not scroll into the viewport: {center:?}");
        h.get_by_label("Simulation chart type").click();
        h.run_steps(2);
        h.query_all_by_label(kind).last().unwrap_or_else(|| panic!("chart menu did not expose {kind}; control center {center:?}")).click();
        h.run_steps(3);
        assert_eq!(h.get_by_label("Simulation chart type").value().as_deref(), Some(kind));
        assert!(h.unnamed_widgets().is_empty());
    }
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
    assert_eq!(value(&h, "B1"), Value::Number(2.0));
    assert_eq!(value(&h, "C1"), Value::Number(1e300));

    let mut h = harness();
    h.state_mut().run("cell.set", json!({"cell":"C1","input":"10"})).unwrap();
    h.state_mut().run("cell.set", json!({"cell":"B1","input":"=A1*C1"})).unwrap();
    let title = "Tornado: one-factor change in selected output mean from Base";
    let preview = simulate_saved_model(
        &mut h,
        "Tornado acceptance",
        json!({
            "inputs":[{"cell":"A1","distribution":{"kind":"constant","value":2.0}}],"outputs":"B1",
            "options":{"trials":16,"seed":"42","retain_samples":true,"max_memory_bytes":8388608},
            "parameter_grid":[{"cell":"C1","low":5.0,"base":10.0,"high":15.0}],
            "parameter_cases":[{"name":"Base","values":[{"cell":"C1","value":10.0}]},
                {"name":"C1 low","values":[{"cell":"C1","value":5.0}]},
                {"name":"C1 high","values":[{"cell":"C1","value":15.0}]}]
        }),
        title,
    );
    let cases = preview["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for (case, expected) in cases.iter().zip([20.0, 10.0, 30.0]) {
        assert_eq!(case["preview"]["result"]["summaries"][0]["mean"], expected);
    }
    h.get_by_label(title).scroll_to_me();
    h.run_steps(20);
    for (case, expected_delta) in cases.iter().skip(1).zip([-10.0, 10.0]) {
        let name = case["name"].as_str().unwrap();
        let text = format!("{name}: {expected_delta:+.6e}");
        assert!(h.query_by_label(&text).is_some(), "actual Tornado must expose {text}; cases {cases:?}");
    }
    assert!(h.unnamed_widgets().is_empty());
    assert_eq!(value(&h, "A1"), Value::Number(2.0));
    assert_eq!(value(&h, "B1"), Value::Number(20.0));
    assert_eq!(value(&h, "C1"), Value::Number(10.0));
}

#[test]
fn pane_reviews_legacy_formulas_before_conversion() {
    let mut h = harness();
    h.state_mut().run("cell.set", json!({"cell":"A1","input":"=PsiUniform(0,2)"})).unwrap();
    h.state_mut().run("cell.set", json!({"cell":"B1","input":"=2*A1+PsiOutput()"})).unwrap();
    h.state_mut().run("ui.solvercraft", json!({"on":true})).unwrap();
    h.run_steps(4);
    h.get_by_label("Import and run history").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Import and run history").click();
    h.run_steps(4);
    h.get_by_label("Import simulation formulas").click();
    h.run_steps(4);
    let hint = "Finds Psi formulas and embedded simulation settings. The scan leaves cells untouched.";
    assert!(h.query_by_label(hint).is_none());
    h.get_by_label("Help with the import scan").click();
    h.run_steps(4);
    assert!(h.query_by_label(hint).is_some(), "click exposes contextual help");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    h.get_by_label("Scan workbook").click();
    h.run_steps(4);
    let current_formula = |h: &common::Harness| {
        h.state().session.active().unwrap().wb.active().unwrap().cell(CellRef::parse("B1").unwrap()).unwrap().formula.as_ref().unwrap().text.clone()
    };
    assert_eq!(current_formula(&h), "2*A1+PSIOUTPUT()");
    click_control(&mut h, "Convert supported cells");
    h.run_steps(4);
    assert_eq!(current_formula(&h), "SC.OUTPUT(2*A1+0)");
    assert!(h.query_by_label("Run simulation").is_some());
    assert!(h.query_by_label("Solvercraft simulation inputs").is_none(), "worksheet mode hides manual JSON inputs");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(current_formula(&h), "2*A1+PSIOUTPUT()");
}

#[test]
fn pane_updates_statistic_cells_from_a_completed_run() {
    let mut h = harness();
    h.state_mut().run("cell.set", json!({"cell":"C1","input":"=PsiMean(B1)"})).unwrap();
    h.state_mut().run("cell.set", json!({"cell":"D1","input":"=PsiTarget(B1,4)"})).unwrap();
    let previous = value(&h, "C1");
    let preview = simulate_saved_model(
        &mut h,
        "Worksheet statistics",
        json!({"inputs":[{"cell":"A1","distribution":{"kind":"constant","value":2.0}}],"outputs":"B1","options":{"trials":16,"max_memory_bytes":1048576}}),
        "Simulation chart type",
    );
    assert_eq!(preview["worksheet_statistics"].as_array().unwrap().len(), 2);
    assert_eq!(value(&h, "C1"), previous);
    h.get_by_label("Update statistic cells").scroll_to_me();
    h.run_steps(8);
    h.get_by_label("Update statistic cells").click();
    h.run_steps(8);
    assert_eq!(value(&h, "C1"), Value::Number(4.0));
    assert_eq!(value(&h, "D1"), Value::Number(1.0));
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(value(&h, "C1"), previous);
}
