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
    h.get_by_label(name).click();
    h.run_steps(2);
    h.get_by_label("Run simulation").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Run simulation").click();
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.query_by_label("Result details").is_none() && Instant::now() < deadline {
        h.run_steps(4);
        std::thread::sleep(Duration::from_millis(5));
    }
    h.get_by_label("Result details").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Result details").click();
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
            "outputs":"B1:C1","options":{"trials":64,"seed":"42","retain_samples":true}
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
    let title = "Tornado: one-factor change in output1 mean from Base";
    let preview = simulate_saved_model(
        &mut h,
        "Tornado acceptance",
        json!({
            "inputs":[{"cell":"A1","distribution":{"kind":"constant","value":2.0}}],"outputs":"B1",
            "options":{"trials":16,"seed":"42","retain_samples":true},
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
