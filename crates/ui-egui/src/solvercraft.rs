//! Thin task pane: model persistence, work and mutation all go through engine commands.
use crate::{SheetApp, i18n::Language};
use serde_json::{Value as Json, json};
use std::collections::HashMap;
use std::sync::Arc;
mod import;
mod report;

#[derive(Clone)]
struct Parameter {
    cell: String,
    low: f64,
    base: f64,
    high: f64,
}

#[derive(Clone)]
struct State {
    name: String,
    selected: Option<String>,
    kind: String,
    objective: String,
    variables: String,
    constraints: String,
    constraint_left: String,
    constraint_op: String,
    constraint_right: String,
    goal: String,
    method: String,
    nonnegative: bool,
    inputs: String,
    outputs: String,
    trials: u64,
    seed: String,
    request: Json,
    models: Vec<Json>,
    jobs: Vec<Json>,
    job: Option<String>,
    status: Json,
    preview: Option<Arc<Json>>,
    error: Option<String>,
    last_poll: f64,
    chart: String,
    target: String,
    time_seconds: u64,
    parameter_grid: Vec<Parameter>,
    compare_parameters: bool,
    worksheet_bindings: bool,
    psi_import: import::State,
    analysis: Option<Arc<Json>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            name: "Model 1".into(),
            selected: None,
            kind: "optimization".into(),
            objective: "B1".into(),
            variables: "A1".into(),
            constraints: "A1 <= 10".into(),
            constraint_left: String::new(),
            constraint_op: "<=".into(),
            constraint_right: String::new(),
            goal: "max".into(),
            method: "auto".into(),
            nonnegative: true,
            inputs: r#"[{"cell":"A1","distribution":{"kind":"uniform","low":0.0,"high":1.0}}]"#.into(),
            outputs: "B1".into(),
            trials: 1000,
            seed: "0".into(),
            request: json!({}),
            models: Vec::new(),
            jobs: Vec::new(),
            job: None,
            status: Json::Null,
            preview: None,
            error: None,
            last_poll: f64::NEG_INFINITY,
            chart: "Histogram".into(),
            target: "0".into(),
            time_seconds: 60,
            parameter_grid: vec![Parameter { cell: "C1".into(), low: 1.0, base: 2.0, high: 3.0 }],
            compare_parameters: false,
            worksheet_bindings: false,
            psi_import: import::State::default(),
            analysis: None,
        }
    }
}
fn label(de: bool, en: &'static str, german: &'static str) -> &'static str {
    if de { german } else { en }
}
fn help(ui: &mut egui::Ui, name: &str, text: &str) {
    let button = ui.small_button("?").accessible_name(name);
    let opened = egui::Popup::from_toggle_button_response(&button).show(|ui| {
        ui.set_max_width(300.0);
        ui.label(text);
    });
    if opened.is_none() {
        button.on_hover_text(text);
    }
}

fn field(ui: &mut egui::Ui, title: &str, value: &mut String) {
    ui.label(title);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).accessible_name(title);
}
fn text_or_json(text: &str) -> Result<Json, String> {
    if text.trim_start().starts_with('[') { serde_json::from_str(text).map_err(|e| e.to_string()) } else { Ok(json!(text)) }
}
fn constraint_rows(text: &str) -> Result<Vec<Json>, String> {
    if text.trim_start().starts_with('[') {
        serde_json::from_str(text).map_err(|e| e.to_string())
    } else {
        Ok(text.lines().map(str::trim).filter(|line| !line.is_empty()).map(|line| json!(line)).collect())
    }
}

fn draft(state: &State) -> Result<Json, String> {
    let mut request = state.request.clone();
    if !request.is_object() {
        request = json!({});
    }
    if state.kind == "optimization" {
        request["objective"] = json!(state.objective);
        request["variables"] = text_or_json(&state.variables)?;
        request["constraints"] = json!(constraint_rows(&state.constraints)?);
        request["goal"] = json!(state.goal);
        request["method"] = json!(state.method);
        request["nonNegative"] = json!(state.nonnegative);
        if state.goal == "value" {
            let target = state.target.parse::<f64>().map_err(|e| e.to_string())?;
            if !target.is_finite() {
                return Err("Target must be finite".into());
            }
            request["value"] = json!(target);
        }
    } else {
        if state.worksheet_bindings {
            if let Some(object) = request.as_object_mut() {
                object.remove("inputs");
                object.remove("outputs");
            }
        } else {
            request["inputs"] = serde_json::from_str(&state.inputs).map_err(|e| e.to_string())?;
            request["outputs"] = text_or_json(&state.outputs)?;
        }
        if !request.get("options").is_some_and(Json::is_object) {
            request["options"] = json!({});
        }
        request["options"]["trials"] = json!(state.trials);
        request["options"]["seed"] = json!(state.seed);
        request["options"]["retain_samples"] = json!(true);
        if state.compare_parameters {
            let baseline = state.parameter_grid.iter().map(|p| json!({"cell":p.cell,"value":p.base})).collect::<Vec<_>>();
            let mut cases = vec![json!({"name":"Base","values":baseline})];
            for (i, parameter) in state.parameter_grid.iter().enumerate() {
                for (suffix, value) in [("low", parameter.low), ("high", parameter.high)] {
                    let mut values = baseline.clone();
                    if let Some(slot) = values.get_mut(i) {
                        *slot = json!({"cell":parameter.cell,"value":value});
                    }
                    cases.push(json!({"name":format!("{} {suffix}",parameter.cell),"values":values}));
                }
            }
            request["parameter_cases"] = json!(cases);
            request["parameter_grid"] =
                json!(state.parameter_grid.iter().map(|p| json!({"cell":p.cell,"low":p.low,"base":p.base,"high":p.high})).collect::<Vec<_>>());
        } else if let Some(object) = request.as_object_mut() {
            object.remove("parameter_cases");
            object.remove("parameter_grid");
        }
    }
    Ok(request)
}
fn variable_text(value: &Json) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(rows) = value.as_array().filter(|rows| !rows.is_empty()) {
        let cells = rows
            .iter()
            .map(|row| row.as_str().or_else(|| row.as_object().filter(|object| object.len() == 1).and_then(|object| object.get("cell")?.as_str())))
            .collect::<Option<Vec<_>>>();
        if let Some(cells) = cells.filter(|cells| cells.iter().all(|cell| !cell.chars().any(char::is_control))) {
            // Cell-formula syntax permits reference unions inside parentheses,
            // unlike the name-only parser's bare top-level comma form.
            return if cells.len() == 1 { cells.join("") } else { format!("({})", cells.join(",")) };
        }
    }
    // Bounds, integer kinds and any future metadata stay losslessly editable.
    value.to_string()
}

fn constraint_text(value: &Json) -> String {
    let Some(rows) = value.as_array() else {
        return String::new();
    };
    let plain = rows
        .iter()
        .map(|row| {
            if let Some(text) = row.as_str() {
                return Some(text.to_string());
            }
            let object = row.as_object().filter(|object| object.len() == 3)?;
            let left = object.get("lhs")?.as_str()?;
            let op = object.get("op")?.as_str()?;
            if !matches!(op, "<=" | "=" | "==" | ">=" | "≤" | "≥") {
                return None;
            }
            let right = match object.get("rhs")? {
                Json::String(text) => text.clone(),
                Json::Number(number) => number.to_string(),
                _ => return None,
            };
            // The text parser splits at its first relation. Preserve an object if
            // an operand itself contains one (including quoted sheet names).
            if left.chars().chain(right.chars()).any(|character| character.is_control() || "<>=≤≥".contains(character)) {
                return None;
            }
            Some(format!("{left} {op} {right}"))
        })
        .collect::<Option<Vec<_>>>();
    plain.map(|rows| rows.join("\n")).unwrap_or_else(|| value.to_string())
}

fn load(state: &mut State, model: Json) {
    state.job = None;
    state.preview = None;
    state.analysis = None;
    state.status = Json::Null;
    state.selected = model["id"].as_str().map(str::to_string);
    state.name = model["name"].as_str().unwrap_or("Model").into();
    state.kind = model["kind"].as_str().unwrap_or("optimization").into();
    state.request = model["request"].clone();
    let p = &state.request;
    state.objective = p["objective"].as_str().unwrap_or("B1").into();
    state.variables = variable_text(&p["variables"]);
    state.constraints = constraint_text(&p["constraints"]);
    state.constraint_left.clear();
    state.constraint_op = "<=".into();
    state.constraint_right.clear();
    state.goal = p["goal"].as_str().unwrap_or("max").into();
    state.target = p["value"].as_f64().map(|v| v.to_string()).unwrap_or_else(|| "0".into());
    state.method = p["method"].as_str().unwrap_or("auto").into();
    state.nonnegative = p["nonNegative"].as_bool().unwrap_or(true);
    state.inputs = p["inputs"].to_string();
    state.outputs = p["outputs"].as_str().map(str::to_string).unwrap_or_else(|| p["outputs"].to_string());
    state.trials = p["options"]["trials"].as_u64().unwrap_or(1000);
    state.seed = p["options"]["seed"].as_str().unwrap_or("0").into();
    state.worksheet_bindings = p.get("inputs").is_none() && p.get("outputs").is_none();
    state.compare_parameters = p.get("parameter_cases").is_some();
    if let Some(grid) = p["parameter_grid"].as_array() {
        state.parameter_grid = grid
            .iter()
            .take(15)
            .filter_map(|row| {
                Some(Parameter {
                    cell: row["cell"].as_str()?.into(),
                    low: row["low"].as_f64()?,
                    base: row["base"].as_f64()?,
                    high: row["high"].as_f64()?,
                })
            })
            .collect();
    }
    state.error = None;
}
fn refresh(app: &mut SheetApp, state: &mut State) {
    match app.run("solvercraft.model.list", json!({})) {
        Ok(r) => state.models = r["models"].as_array().cloned().unwrap_or_default(),
        Err(e) => state.error = Some(e),
    }
}

/// Reveal and select a saved model for an embedding application. The host does
/// not duplicate editor state or bypass the engine's model validation.
pub fn open_model(app: &mut SheetApp, ctx: &egui::Context, id: &str) -> Result<(), String> {
    let model = app.run("solvercraft.model.get", json!({"id":id}))?;
    let uid = app.session.active().map(|doc| doc.uid).ok_or_else(|| "No workbook is open".to_string())?;
    let mut state = State::default();
    load(&mut state, model);
    refresh(app, &mut state);
    ctx.data_mut(|memory| memory.insert_temp(egui::Id::unique(("solvercraft-pane", uid)), state));
    app.run("ui.solvercraft", json!({"on":true})).map(|_| ())
}

/// The active range is qualified at capture time so switching sheets later
/// cannot change the model's meaning.
fn selected_reference(app: &SheetApp, single_cell: bool) -> Option<String> {
    let doc = app.session.active()?;
    let sheet = gridcraft_engine::core::quote_sheet(&doc.wb.active()?.name);
    let range = doc.selection.current();
    if single_cell && (doc.selection.ranges.len() != 1 || !range.is_single()) {
        return None;
    }
    let mut refs = doc.selection.ranges.iter().map(|range| format!("{sheet}!{range}"));
    let first = refs.next()?;
    if doc.selection.ranges.len() == 1 { Some(first) } else { Some(format!("({first},{})", refs.collect::<Vec<_>>().join(","))) }
}

fn pick_reference(ui: &mut egui::Ui, title: &str, value: &mut String, selection: Option<&str>, de: bool) {
    let button = ui.add_enabled(selection.is_some(), egui::Button::new(title));
    let button = button.on_hover_text(selection.unwrap_or(label(de, "Select one cell first", "Zuerst eine einzelne Zelle markieren")));
    if button.clicked()
        && let Some(reference) = selection
    {
        *value = reference.to_owned();
    }
}

fn model_editor(app: &SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    let cell = selected_reference(app, true);
    let range = selected_reference(app, false);
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.kind, "optimization".into(), label(de, "Optimization", "Optimierung"));
        ui.selectable_value(&mut state.kind, "simulation".into(), "Simulation");
        help(
            ui,
            label(de, "Help with model types", "Hilfe zu Modelltypen"),
            label(de, "Optimization finds values that satisfy constraints and improve a goal. Simulation measures how uncertain inputs affect outputs.", "Optimierung sucht Werte, die Nebenbedingungen erfüllen und ein Ziel verbessern. Simulation misst, wie unsichere Eingaben die Ausgaben beeinflussen."),
        );
    });
    if state.kind == "optimization" {
        field(ui, label(de, "Objective cell or formula", "Zielzelle oder Formel"), &mut state.objective);
        pick_reference(ui, label(de, "Use selection as objective", "Markierung als Zielzelle"), &mut state.objective, cell.as_deref(), de);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.goal, "min".into(), "Min");
            ui.selectable_value(&mut state.goal, "max".into(), "Max");
            ui.selectable_value(&mut state.goal, "value".into(), label(de, "Target", "Zielwert"));
        });
        if state.goal == "value" {
            field(ui, label(de, "Target value", "Zielwert"), &mut state.target);
        }
        field(ui, label(de, "Decision cells", "Entscheidungszellen"), &mut state.variables);
        // A JSON model may carry integer kinds and bounds. Preserve those
        // descriptors until the user explicitly edits that field.
        if !state.variables.trim_start().starts_with('[') {
            pick_reference(
                ui,
                label(de, "Use selection as decision cells", "Markierung als Entscheidungszellen"),
                &mut state.variables,
                range.as_deref(),
                de,
            );
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(label(de, "Constraints", "Nebenbedingungen"));
            help(
                ui,
                label(de, "Help with constraints", "Hilfe zu Nebenbedingungen"),
                label(de, "Enter one condition per line, such as A1 <= 10. Use the sheet selection below to build a condition from cells on any sheet.", "Eine Bedingung pro Zeile eingeben, etwa A1 <= 10. Über die Blattauswahl unten lassen sich Zellen aus beliebigen Blättern einsetzen."),
            );
        });
        ui.add(egui::TextEdit::multiline(&mut state.constraints).desired_rows(4).desired_width(f32::INFINITY))
            .accessible_name("Solvercraft constraints");
        ui.collapsing(label(de, "Add a constraint from the sheet", "Nebenbedingung aus dem Blatt hinzufügen"), |ui| {
            field(ui, label(de, "Left cell or expression", "Linke Zelle oder Ausdruck"), &mut state.constraint_left);
            pick_reference(ui, label(de, "Use selection on the left", "Markierung links einsetzen"), &mut state.constraint_left, cell.as_deref(), de);
            ui.horizontal(|ui| {
                for op in ["<=", "=", ">="] {
                    ui.selectable_value(&mut state.constraint_op, op.into(), op);
                }
            });
            field(ui, label(de, "Right cell or value", "Rechte Zelle oder Wert"), &mut state.constraint_right);
            pick_reference(
                ui,
                label(de, "Use selection on the right", "Markierung rechts einsetzen"),
                &mut state.constraint_right,
                cell.as_deref(),
                de,
            );
            let ready = !state.constraint_left.trim().is_empty() && !state.constraint_right.trim().is_empty();
            if ui.add_enabled(ready, egui::Button::new(label(de, "Add constraint", "Nebenbedingung hinzufügen"))).clicked() {
                match constraint_rows(&state.constraints) {
                    Ok(mut rows) => {
                        rows.push(json!({"lhs":state.constraint_left.trim(),"op":state.constraint_op,"rhs":state.constraint_right.trim()}));
                        state.constraints = constraint_text(&json!(rows));
                        state.constraint_left.clear();
                        state.constraint_right.clear();
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        });
        ui.checkbox(&mut state.nonnegative, label(de, "Unbounded variables are nonnegative", "Variablen ohne Untergrenze sind nichtnegativ"));
        craft_ui::ComboBox::from_id_salt("solvercraft-method")
            .selected_text(&state.method)
            .show_ui(ui, |ui| {
                for method in ["auto", "simplex", "branch_and_bound", "projected_gradient", "differential_evolution"] {
                    ui.selectable_value(&mut state.method, method.into(), method);
                }
            })
            .response
            .accessible_name(label(de, "Solving method", "Lösungsmethode"));
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut state.worksheet_bindings, label(de, "Use worksheet declarations", "Deklarationen aus dem Blatt verwenden"));
            help(
                ui,
                label(de, "Help with worksheet declarations", "Hilfe zu Blattdeklarationen"),
                label(de, "Solvercraft finds distribution cells and SC.OUTPUT formulas in the workbook. Turn this off to specify cells manually.", "Solvercraft findet Verteilungszellen und SC.OUTPUT-Formeln in der Mappe. Zum manuellen Angeben von Zellen diese Option deaktivieren."),
            );
        });
        if !state.worksheet_bindings {
            ui.label(label(de, "Inputs (JSON)", "Eingaben (JSON)"));
            ui.add(egui::TextEdit::multiline(&mut state.inputs).desired_rows(4).desired_width(f32::INFINITY))
                .accessible_name("Solvercraft simulation inputs");
            field(ui, label(de, "Output references or JSON array", "Ausgabereferenzen oder JSON-Array"), &mut state.outputs);
            pick_reference(ui, label(de, "Use selection as outputs", "Markierung als Ausgaben"), &mut state.outputs, range.as_deref(), de);
        }
        ui.horizontal(|ui| {
            ui.label(label(de, "Trials", "Versuche"));
            ui.add(egui::DragValue::new(&mut state.trials).range(1..=1_000_000)).accessible_name(label(de, "Trials", "Versuche"));
        });
        field(ui, "Seed", &mut state.seed);
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut state.compare_parameters, label(de, "Compare parameters", "Parameter vergleichen"));
            help(
                ui,
                label(de, "Help with parameter comparisons", "Hilfe zum Parametervergleich"),
                label(
                    de,
                    "Each comparison varies one parameter. The other parameters stay at their base values.",
                    "Pro Vergleich ändert sich ein Parameter. Die übrigen Parameter behalten ihre Basiswerte.",
                ),
            );
        });
        if state.compare_parameters {
            let mut remove = None;
            for (i, p) in state.parameter_grid.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    field(ui, label(de, "Parameter cell", "Parameterzelle"), &mut p.cell);
                    pick_reference(ui, label(de, "Use selection as parameter", "Markierung als Parameter"), &mut p.cell, cell.as_deref(), de);
                    ui.horizontal_wrapped(|ui| {
                        for (name, value) in [("Low", &mut p.low), ("Base", &mut p.base), ("High", &mut p.high)] {
                            ui.label(name);
                            ui.add(egui::DragValue::new(value).speed(0.1).range(-1e100..=1e100))
                                .accessible_name(format!("Parameter {} {name}", i + 1));
                        }
                        if ui.small_button(label(de, "Remove", "Entfernen")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(index) = remove {
                state.parameter_grid.remove(index);
            }
            if ui.add_enabled(state.parameter_grid.len() < 15, egui::Button::new(label(de, "Add parameter", "Parameter hinzufügen"))).clicked() {
                state.parameter_grid.push(Parameter { cell: "C1".into(), low: 0.0, base: 1.0, high: 2.0 });
            }
        }
    }
    ui.horizontal(|ui| {
        ui.label(label(de, "Time limit (seconds)", "Zeitlimit (Sekunden)"));
        ui.add(egui::DragValue::new(&mut state.time_seconds).range(1..=3600)).accessible_name(label(
            de,
            "Time limit seconds",
            "Zeitlimit in Sekunden",
        ));
    });
}

fn poll_job(app: &mut SheetApp, state: &mut State, id: &str, now: f64) {
    if now - state.last_poll >= 0.15 {
        state.last_poll = now;
        match app.run("solvercraft.job.get", json!({"id":id})) {
            Ok(status) => {
                state.status = status;
                if state.preview.is_none() && state.status["status"] == json!("completed") {
                    match app.run("solvercraft.job.result", json!({"id":id})) {
                        Ok(preview) => state.preview = Some(Arc::new(preview)),
                        Err(e) => state.error = Some(e),
                    }
                }
            }
            Err(e) => state.error = Some(e),
        }
    }
}

pub fn show(app: &mut SheetApp, ui: &mut egui::Ui) {
    let Some(doc) = app.session.active() else { return };
    let key = egui::Id::unique(("solvercraft-pane", doc.uid));
    let open_key = egui::Id::unique("solvercraft-pane-documents");
    ui.ctx().data_mut(|memory| {
        let mut documents = memory.get_temp::<Vec<u64>>(open_key).unwrap_or_default();
        if !documents.contains(&doc.uid) {
            documents.push(doc.uid);
        }
        memory.insert_temp(open_key, documents);
    });
    let current_uid = doc.uid.to_string();
    let current_revision = doc.revision.to_string();
    let mut state = ui.ctx().data_mut(|m| m.get_temp::<State>(key)).unwrap_or_default();
    if let Some(preview) = &state.preview {
        state.status["stale"] = json!(
            preview["snapshot"]["document_uid"].as_str() != Some(&current_uid) || preview["snapshot"]["revision"].as_str() != Some(&current_revision)
        );
    }
    let de = app.ui.language == Language::De;
    ui.horizontal(|ui| {
        if ui.button(label(de, "Refresh models", "Modelle laden")).clicked() {
            refresh(app, &mut state)
        }
        if ui.button(label(de, "New", "Neu")).clicked() {
            state = State::default();
            refresh(app, &mut state)
        }
    });
    let models = state.models.clone();
    for model in models {
        let id = model["id"].as_str().unwrap_or("");
        let name = model["name"].as_str().unwrap_or("Model");
        if ui.selectable_label(state.selected.as_deref() == Some(id), name).clicked() {
            match app.run("solvercraft.model.get", json!({"id":id})) {
                Ok(m) => load(&mut state, m),
                Err(e) => state.error = Some(e),
            }
        }
    }
    ui.separator();
    field(ui, label(de, "Model name", "Modellname"), &mut state.name);
    import::show(app, ui, &mut state, &current_revision, de);
    job_history(app, ui, &mut state, de);
    let before = draft(&state).ok();
    model_editor(app, ui, &mut state, de);
    if before != draft(&state).ok() {
        state.job = None;
        state.preview = None;
        state.analysis = None;
        state.status = Json::Null;
    }
    ui.horizontal(|ui| {
        if ui.button(label(de, "Save model", "Modell speichern")).clicked() {
            match draft(&state) {
                Ok(request) => {
                    let (command, params) = if let Some(id) = &state.selected {
                        ("solvercraft.model.update", json!({"id":id,"name":state.name,"kind":state.kind,"request":request}))
                    } else {
                        ("solvercraft.model.create", json!({"name":state.name,"kind":state.kind,"request":request}))
                    };
                    match app.run(command, params) {
                        Ok(model) => {
                            load(&mut state, model);
                            refresh(app, &mut state)
                        }
                        Err(e) => state.error = Some(e),
                    }
                }
                Err(e) => state.error = Some(e),
            }
        }
        if ui.add_enabled(state.selected.is_some(), egui::Button::new(label(de, "Delete", "Löschen"))).clicked() {
            match app.run("solvercraft.model.delete", json!({"id":state.selected})) {
                Ok(_) => {
                    state.selected = None;
                    refresh(app, &mut state)
                }
                Err(e) => state.error = Some(e),
            }
        }
    });
    let running = matches!(state.status["status"].as_str(), Some("queued" | "running"));
    let action = if state.kind == "optimization" {
        label(de, "Calculate optimization", "Optimierung berechnen")
    } else {
        label(de, "Run simulation", "Simulation berechnen")
    };
    let start = ui.add_enabled(!running, egui::Button::new(action)).on_hover_text("Ctrl+Enter").clicked()
        || (!running && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)));
    if start {
        match draft(&state) {
            Ok(request) => match app.run(
                "solvercraft.job.start",
                json!({"kind":state.kind,"request":request,"limits":{"max_time_millis":state.time_seconds.saturating_mul(1000)}}),
            ) {
                Ok(job) => {
                    state.job = job["id"].as_str().map(str::to_string);
                    state.status = json!({"status":"queued"});
                    state.preview = None;
                    state.analysis = None;
                    state.error = None;
                    state.last_poll = f64::NEG_INFINITY
                }
                Err(e) => state.error = Some(e),
            },
            Err(e) => state.error = Some(e),
        }
    }
    if let Some(id) = state.job.clone() {
        let now = ui.input(|i| i.time);
        poll_job(app, &mut state, &id, now);
        ui.label(format!("Job {id}: {}", state.status["status"].as_str().unwrap_or("unknown")));
        if let Some(progress) = state.status.get("progress").filter(|x| x.is_object()) {
            ui.label(format!("{} · {} evaluations · {} nodes", progress["stage"].as_str().unwrap_or(""), progress["evaluations"], progress["nodes"]));
        }
        if state.status["stale"].as_bool() == Some(true) {
            ui.label(label(
                de,
                "Workbook changed: recalculate to apply. You can still archive this run.",
                "Arbeitsmappe geändert: zur Übernahme neu berechnen. Dieser Lauf bleibt archivierbar.",
            ));
        }
        if let Some(error) = state.status.get("error").filter(|e| !e.is_null()) {
            ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        }
        if state.status["status"] == json!("failed") && ui.button(label(de, "Forget failed job", "Fehlgeschlagenen Lauf freigeben")).clicked() {
            match app.run("solvercraft.job.forget", json!({"id":id})) {
                Ok(_) => {
                    state.job = None;
                    state.status = Json::Null;
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
        if matches!(state.status["status"].as_str(), Some("queued" | "running")) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
            if ui.button(label(de, "Cancel", "Abbrechen")).clicked()
                && let Err(e) = app.run("solvercraft.job.cancel", json!({"id":id}))
            {
                state.error = Some(e)
            }
        }
        if let Some(preview) = state.preview.clone() {
            let applicable = preview["result"]["feasibility"] == json!("feasible")
                && preview["bindings"].as_array().is_some_and(|a| !a.is_empty())
                && !state.status["stale"].as_bool().unwrap_or(true);
            if ui
                .add_enabled(applicable, egui::Button::new(label(de, "Apply solution (one undo step)", "Lösung übernehmen (ein Undo-Schritt)")))
                .clicked()
            {
                match app.run("solvercraft.job.apply", json!({"id":id})) {
                    Ok(_) => {
                        state.status["stale"] = json!(true);
                        state.error = None
                    }
                    Err(e) => state.error = Some(e),
                }
            }
            result_sheet_button(app, ui, &mut state, &id, de);
            ui.collapsing(label(de, "Result details", "Ergebnisdetails"), |ui| {
                result(ui, &preview, &mut state.chart);
            });
            match report::controls(app, ui, &preview, applicable && state.status["stale"] != json!(true), de) {
                Ok(Some(report)) => state.analysis = Some(Arc::new(report)),
                Err(error) => state.error = Some(error),
                Ok(None) => {}
            }
            if let Some(analysis) = &state.analysis {
                report::show(ui, analysis);
            }
            if ui.button(label(de, "Forget result", "Ergebnis freigeben")).clicked() {
                match app.run("solvercraft.job.forget", json!({"id":id})) {
                    Ok(_) => {
                        state.job = None;
                        state.preview = None;
                        state.analysis = None;
                        state.status = Json::Null
                    }
                    Err(e) => state.error = Some(e),
                }
            }
        }
    }
    if let Some(error) = &state.error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    ui.ctx().data_mut(|m| m.insert_temp(key, state));
}
/// Release retained result Arcs when their document actually closes. egui's
/// temporary data map otherwise keeps per-document state beyond pane visibility.
pub(crate) fn forget_closed(ctx: &egui::Context, open: &[u64]) {
    ctx.data_mut(|memory| {
        let key = egui::Id::unique("solvercraft-pane-documents");
        let mut known = memory.get_temp::<Vec<u64>>(key).unwrap_or_default();
        known.retain(|uid| {
            if open.contains(uid) {
                true
            } else {
                memory.remove::<State>(egui::Id::unique(("solvercraft-pane", *uid)));
                false
            }
        });
        memory.insert_temp(key, known);
    });
}
fn job_history(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    ui.collapsing(label(de, "Job history", "Laufhistorie"), |ui| {
        if ui.button(label(de, "Refresh jobs", "Läufe aktualisieren")).clicked() {
            match app.run("solvercraft.job.list", json!({})) {
                Ok(list) => state.jobs = list["jobs"].as_array().cloned().unwrap_or_default(),
                Err(error) => state.error = Some(error),
            }
        }
        for job in state.jobs.clone() {
            let Some(id) = job["id"].as_str() else { continue };
            let title = format!("{id}: {}", job["status"].as_str().unwrap_or("unknown"));
            if ui.selectable_label(state.job.as_deref() == Some(id), title).clicked() {
                state.job = Some(id.into());
                state.status = job;
                state.preview = None;
                state.analysis = None;
                state.error = None;
                state.last_poll = f64::NEG_INFINITY;
            }
        }
    });
}

fn result_sheet_button(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, id: &str, de: bool) {
    let report_button = ui.button(label(de, "Create result sheet", "Ergebnisblatt erstellen"));
    if report_button
        .on_hover_text(label(
            de,
            "Archives this run as values. To apply a solution, do so before creating the sheet.",
            "Archiviert diesen Lauf als Werte. Eine Lösung zuerst übernehmen und dann das Blatt erstellen.",
        ))
        .clicked()
    {
        match app.run("solvercraft.job.report", json!({"id":id,"language":if de {"de"} else {"en"}})) {
            Ok(_) => {
                state.status["stale"] = json!(true);
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
    }
}

fn result(ui: &mut egui::Ui, preview: &Json, chart: &mut String) {
    if let Some(cases) = preview["cases"].as_array() {
        parameter_results(ui, cases);
        return;
    }
    let result = &preview["result"];
    ui.separator();
    if let Some(objective) = preview["request"]["objective"].as_str() {
        ui.label(format!("Preview objective: {objective} · {}", preview["request"]["goal"].as_str().unwrap_or("max")));
    }
    ui.strong(result["termination"].as_str().unwrap_or("unknown"));
    for field in ["objective", "feasibility", "optimality", "completed_trials"] {
        if let Some(value) = result.get(field).filter(|v| !v.is_null()) {
            ui.label(format!("{field}: {value}"));
        }
    }
    if let Some(values) = preview.get("apply_values").unwrap_or(&result["values"]).as_array() {
        for (i, value) in values.iter().enumerate() {
            let binding = &preview["bindings"][i];
            ui.monospace(format!("{}!{} = {value}", binding["sheet"].as_str().unwrap_or(""), binding["cell"].as_str().unwrap_or("")));
        }
    }
    if let Some(summaries) = result["summaries"].as_array() {
        for (i, summary) in summaries.iter().enumerate() {
            ui.label(format!(
                "{}: mean {} · min {} · max {}",
                preview["outputs"][i].as_str().unwrap_or("output"),
                summary["mean"],
                summary["minimum"],
                summary["maximum"]
            ));
        }
    }
    if let Some(rows) = result["samples"].as_array() {
        craft_ui::ComboBox::from_id_salt("solvercraft-chart")
            .selected_text(chart.as_str())
            .show_ui(ui, |ui| {
                for kind in ["Histogram", "CDF", "Scatter", "Box", "Quantile bands"] {
                    ui.selectable_value(chart, kind.into(), kind);
                }
            })
            .response
            .accessible_name("Simulation chart type");
        if chart == "Box" || chart == "Quantile bands" {
            chart_quantiles(ui, &result["summaries"], chart);
        } else {
            chart_samples(ui, rows, chart);
        }
    }
}
/// Unit coordinates retain tiny ranges and close large-offset differences.
/// Halving is needed only when the finite endpoints' difference overflows.
fn unit_position(value: f64, low: f64, high: f64) -> f64 {
    if low == high {
        return 0.5;
    }
    let span = high - low;
    let position = if span.is_finite() { (value - low) / span } else { (value * 0.5 - low * 0.5) / (high * 0.5 - low * 0.5) };
    position.clamp(0.0, 1.0)
}
fn sample_points(rows: &[Json], scatter: bool) -> Vec<(f64, f64)> {
    rows.iter()
        .filter_map(|row| {
            let values = row.as_array()?;
            Some((values.first()?.as_f64()?, if scatter { values.get(1)?.as_f64()? } else { 0.0 }))
        })
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .take(10_000)
        .collect()
}
fn histogram_bins(points: &[(f64, f64)], low: f64, high: f64) -> [usize; 20] {
    let mut bins = [0usize; 20];
    for point in points {
        let index = ((unit_position(point.0, low, high) * 20.0) as usize).min(19);
        if let Some(bin) = bins.get_mut(index) {
            *bin += 1;
        }
    }
    bins
}
fn cdf_steps(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut values = points.iter().map(|point| point.0).collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let count = values.len().max(1) as f64;
    values.into_iter().enumerate().map(|(index, value)| (value, (index + 1) as f64 / count)).collect()
}
fn chart_samples(ui: &mut egui::Ui, rows: &[Json], kind: &str) {
    if kind == "Scatter" && rows.first().and_then(Json::as_array).is_none_or(|row| row.len() < 2) {
        ui.label("Scatter requires two output columns.");
        return;
    }
    let mut points = sample_points(rows, kind == "Scatter");
    if points.is_empty() {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 150.0), egui::Sense::hover());
    let t = crate::theme::Tokens::get(ui.ctx());
    let painter = ui.painter();
    let rect = rect.shrink(5.0);
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let lo = points.first().map_or(0.0, |p| p.0);
    let hi = points.last().map_or(1.0, |p| p.0);
    let x = |value: f64| rect.left() + unit_position(value, lo, hi) as f32 * rect.width();
    if kind == "Histogram" {
        let bins = histogram_bins(&points, lo, hi);
        let max = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
        for (i, count) in bins.iter().enumerate() {
            let left = rect.left() + i as f32 * rect.width() / 20.0;
            let right = rect.left() + (i + 1) as f32 * rect.width() / 20.0;
            let top = rect.bottom() - (*count as f32 / max) * rect.height();
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right - 1.0, rect.bottom())), 0.0, t.accent);
        }
    } else if kind == "CDF" {
        let mut last = egui::pos2(rect.left(), rect.bottom());
        for (value, probability) in cdf_steps(&points) {
            let next = egui::pos2(x(value), rect.bottom() - probability as f32 * rect.height());
            let corner = egui::pos2(next.x, last.y);
            painter.line_segment([last, corner], egui::Stroke::new(1.5, t.accent));
            painter.line_segment([corner, next], egui::Stroke::new(1.5, t.accent));
            last = next;
        }
    } else {
        let low = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let high = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        for point in &points {
            painter.circle_filled(egui::pos2(x(point.0), rect.bottom() - unit_position(point.1, low, high) as f32 * rect.height()), 1.5, t.accent);
        }
    }
    ui.small(format!("{} retained trials shown; x [{:.6e}, {:.6e}]", points.len(), lo, hi));
}
fn quantile(summary: &Json, p: f64) -> Option<f64> {
    summary["quantiles"]
        .as_array()?
        .iter()
        .find(|pair| pair[0].as_f64().is_some_and(|q| (q - p).abs() < 1e-12))?
        .get(1)?
        .as_f64()
        .filter(|v| v.is_finite())
}
fn summary_quantiles(summary: &Json) -> Option<[f64; 7]> {
    let values = [
        summary["minimum"].as_f64()?,
        quantile(summary, 0.05)?,
        quantile(summary, 0.25)?,
        quantile(summary, 0.5)?,
        quantile(summary, 0.75)?,
        quantile(summary, 0.95)?,
        summary["maximum"].as_f64()?,
    ];
    (values.iter().all(|value| value.is_finite()) && values.windows(2).all(|pair| pair[0] <= pair[1])).then_some(values)
}
fn chart_quantiles(ui: &mut egui::Ui, summaries: &Json, kind: &str) {
    let Some(summaries) = summaries.as_array() else { return };
    let data = summaries.iter().filter_map(summary_quantiles).collect::<Vec<_>>();
    if data.is_empty() {
        ui.label("Exact retained-sample quantiles are unavailable.");
        return;
    }
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 160.0), egui::Sense::hover());
    let rect = rect.shrink(8.0);
    let painter = ui.painter();
    let t = crate::theme::Tokens::get(ui.ctx());
    if kind == "Box" {
        let d = data[0];
        let x = |v: f64| rect.left() + unit_position(v, d[0], d[6]) as f32 * rect.width();
        let y = rect.center().y;
        painter.line_segment([egui::pos2(x(d[0]), y), egui::pos2(x(d[6]), y)], egui::Stroke::new(1.5, t.accent));
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x(d[2]), y - 20.0), egui::pos2(x(d[4]), y + 20.0)),
            0.0,
            t.accent.gamma_multiply(0.3),
        );
        for (value, height) in [(d[0], 10.0), (d[3], 25.0), (d[6], 10.0)] {
            painter.line_segment([egui::pos2(x(value), y - height), egui::pos2(x(value), y + height)], egui::Stroke::new(2.0, t.accent));
        }
        ui.small(format!("Output1: min {:.6e}, Q25 {:.6e}, median {:.6e}, Q75 {:.6e}, max {:.6e}; min/max whiskers", d[0], d[2], d[3], d[4], d[6]));
    } else {
        let lo = data.iter().map(|d| d[1]).fold(f64::INFINITY, f64::min);
        let hi = data.iter().map(|d| d[5]).fold(f64::NEG_INFINITY, f64::max);
        let y = |v: f64| rect.bottom() - unit_position(v, lo, hi) as f32 * rect.height();
        let width = rect.width() / data.len() as f32;
        for (i, d) in data.iter().enumerate() {
            let x = rect.left() + (i as f32 + 0.5) * width;
            painter.line_segment([egui::pos2(x, y(d[1])), egui::pos2(x, y(d[5]))], egui::Stroke::new(width.min(20.0), t.accent.gamma_multiply(0.25)));
            painter.line_segment([egui::pos2(x, y(d[2])), egui::pos2(x, y(d[4]))], egui::Stroke::new(width.min(12.0), t.accent.gamma_multiply(0.5)));
            painter.circle_filled(egui::pos2(x, y(d[3])), 3.0, t.accent);
        }
        ui.small("Output columns in order: 5–95% band, 25–75% band and median. Empirical distribution bands, not confidence intervals.");
    }
}
fn parameter_map(case: &Json) -> Option<HashMap<&str, f64>> {
    let values = case["parameters"].as_array().filter(|values| !values.is_empty() && values.len() <= 4096)?;
    let map = values
        .iter()
        .map(|entry| Some((entry["cell"].as_str()?, entry["value"].as_f64().filter(|value| value.is_finite())?)))
        .collect::<Option<HashMap<_, _>>>()?;
    (map.len() == values.len()).then_some(map)
}
fn parameter_effects(cases: &[Json]) -> Vec<(String, f64)> {
    let Some(base) = cases.first() else { return Vec::new() };
    let Some(base_mean) = base["preview"]["result"]["summaries"][0]["mean"].as_f64().filter(|value| value.is_finite()) else { return Vec::new() };
    let Some(base_values) = parameter_map(base) else { return Vec::new() };
    let mut effects = cases
        .iter()
        .take(32)
        .filter_map(|case| {
            let values = parameter_map(case)?;
            // Missing/differently spelled bindings cannot establish a one-factor
            // comparison. Never infer unchanged constants from an incomplete case.
            if values.len() != base_values.len() || values.keys().any(|cell| !base_values.contains_key(cell)) {
                return None;
            }
            let changed = values.iter().filter(|(cell, value)| base_values.get(*cell).is_none_or(|base| *base != **value)).count();
            let delta = case["preview"]["result"]["summaries"][0]["mean"].as_f64()? - base_mean;
            (changed == 1 && delta.is_finite()).then(|| (case["name"].as_str().unwrap_or("Case").to_string(), delta))
        })
        .collect::<Vec<_>>();
    effects.sort_by(|left, right| right.1.abs().total_cmp(&left.1.abs()));
    effects
}
fn parameter_results(ui: &mut egui::Ui, cases: &[Json]) {
    if cases.is_empty() {
        ui.label("No completed parameter cases.");
        return;
    }
    ui.strong("Parameter comparison — common random numbers");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("Case");
        let result = &case["preview"]["result"];
        ui.label(format!("{name}: {} trials · mean {}", result["completed_trials"], result["summaries"][0]["mean"]));
    }
    let effects = parameter_effects(cases);
    if effects.is_empty() {
        return;
    }
    let maximum = effects.iter().map(|e| e.1.abs()).fold(0.0_f64, f64::max);
    ui.strong("Tornado: one-factor change in output1 mean from Base");
    for (name, delta) in effects {
        ui.label(format!("{name}: {delta:+.6e}"));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 14.0), egui::Sense::hover());
        let center = rect.center().x;
        let relative = if maximum > 0.0 { delta / maximum } else { 0.0 };
        let end = center + relative as f32 * rect.width() / 2.0;
        let t = crate::theme::Tokens::get(ui.ctx());
        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(center.min(end), rect.top()), egui::pos2(center.max(end), rect.bottom())),
            0.0,
            t.accent,
        );
    }
    ui.small("Finite parameter stress comparisons; not regression coefficients or a causality claim.");
    ui.small("Tornado requires the same complete parameter bindings and exactly one changed value per case.");
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    #[test]
    fn saved_plain_models_remain_readable_and_preserve_the_solved_model() {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        session.execute("range.setValues", super::json!({"range":"A1:C1","values":[[0,0,"=3*A1+5*B1"]]})).unwrap();
        let saved=session.execute("solvercraft.model.create",super::json!({
            "name":"Readable model","request":{"objective":"C1","goal":"max","variables":"A1:B1","constraints":["A1 <= 4","B1 <= 6","3*A1+2*B1 <= 18"]}
        })).unwrap();
        let request = &saved["request"];
        let before = session.execute("solvercraft.validate", request.clone()).unwrap();
        let mut state = super::State::default();
        super::load(&mut state, saved);
        assert!(!state.variables.starts_with('['));
        assert!(!state.constraints.starts_with('['));
        let roundtrip = super::draft(&state).unwrap();
        let after = session.execute("solvercraft.validate", roundtrip.clone()).unwrap();
        assert_eq!(before["model"], after["model"]);
        assert_eq!(session.execute("solvercraft.solve", roundtrip).unwrap()["result"]["objective"], 36.0);
    }

    #[test]
    fn readable_load_keeps_bounds_kinds_and_ambiguous_constraint_objects() {
        let variables = super::json!([{"cell":"A1","kind":"integer","lower":-4,"upper":9},{"cell":"B1","binding":"host-owned"}]);
        assert_eq!(super::variable_text(&variables), variables.to_string());
        let constraints = super::json!([{"lhs":"'x=y'!A1","op":"<=","rhs":4}]);
        assert_eq!(super::constraint_text(&constraints), constraints.to_string());
        let advanced = super::json!([{"lhs":"A1","op":"<=","rhs":4,"future_metadata":"retain"}]);
        assert_eq!(super::constraint_text(&advanced), advanced.to_string());
        let multiline = super::json!([{"cell":"'Two\nlines'!A1"}]);
        assert_eq!(super::variable_text(&multiline), multiline.to_string());
    }
    use super::{Arc, State, cdf_steps, forget_closed, histogram_bins, json, parameter_effects, sample_points, summary_quantiles, unit_position};
    #[test]
    fn histogram_contract_places_boundaries_in_twenty_bins_and_retains_every_row() {
        let points = [(0.0, 0.0), (0.05, 0.0), (0.999, 0.0), (1.0, 0.0)];
        let bins = histogram_bins(&points, 0.0, 1.0);
        assert_eq!((bins[0], bins[1], bins[19]), (1, 1, 2));
        assert_eq!(bins.iter().sum::<usize>(), 4);
        let constant = histogram_bins(&[(7.0, 0.0); 3], 7.0, 7.0);
        assert_eq!(constant[10], 3);
    }
    #[test]
    fn cdf_contract_orders_values_and_preserves_duplicate_probability_jumps() {
        assert_eq!(cdf_steps(&[(3.0, 0.0), (1.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), vec![(1.0, 0.25), (1.0, 0.5), (2.0, 0.75), (3.0, 1.0)]);
        assert!(cdf_steps(&[]).is_empty());
    }
    #[test]
    fn scatter_contract_keeps_same_trial_pairs_when_sorted_and_skips_incomplete_rows() {
        let rows = [json!([2.0, 300.0]), json!([1.0, 100.0]), json!([3.0, 200.0]), json!([4.0])];
        let mut points = sample_points(&rows, true);
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        assert_eq!(points, vec![(1.0, 100.0), (2.0, 300.0), (3.0, 200.0)]);
        let coordinates = points.iter().map(|(x, y)| (unit_position(*x, 1.0, 3.0), unit_position(*y, 100.0, 300.0))).collect::<Vec<_>>();
        assert_eq!(coordinates, vec![(0.0, 0.0), (0.5, 1.0), (1.0, 0.5)]);
    }
    fn summary_fixture() -> super::Json {
        json!({"count":20000,"minimum":-10.0,"maximum":100.0,"quantiles":[[0.05,0.0],[0.25,10.0],[0.5,20.0],[0.75,40.0],[0.95,80.0]]})
    }
    #[test]
    fn box_contract_uses_full_run_quartiles_and_min_max_whiskers() {
        let values = summary_quantiles(&summary_fixture()).unwrap();
        assert_eq!([values[0], values[2], values[3], values[4], values[6]], [-10.0, 10.0, 20.0, 40.0, 100.0]);
        let mut invalid = summary_fixture();
        invalid["quantiles"][2][1] = json!(90.0);
        assert!(summary_quantiles(&invalid).is_none());
    }
    #[test]
    fn quantile_band_contract_keeps_5_25_50_75_95_values_and_output_order() {
        let first = summary_fixture();
        let second = json!({"minimum":100.0,"maximum":200.0,"quantiles":[[0.05,110.0],[0.25,120.0],[0.5,130.0],[0.75,140.0],[0.95,190.0]]});
        let data = [first, second].iter().filter_map(summary_quantiles).collect::<Vec<_>>();
        assert_eq!(&data[0][1..6], &[0.0, 10.0, 20.0, 40.0, 80.0]);
        assert_eq!(&data[1][1..6], &[110.0, 120.0, 130.0, 140.0, 190.0]);
        assert_eq!(unit_position(data[0][1], 0.0, 190.0), 0.0);
        assert_eq!(unit_position(data[1][5], 0.0, 190.0), 1.0);
    }
    #[test]
    fn tornado_contract_uses_first_case_base_and_only_complete_single_factor_changes() {
        let case = |name: &str, mean: f64, parameters: super::Json| json!({"name":name,"parameters":parameters,"preview":{"result":{"summaries":[{"mean":mean}]}}});
        let cases = [
            case("Base", 100.0, json!([{"cell":"C1","value":10},{"cell":"D1","value":2}])),
            case("Low", 95.0, json!([{"cell":"C1","value":5},{"cell":"D1","value":2.0}])),
            case("High", 110.0, json!([{"cell":"D1","value":2},{"cell":"C1","value":15}])),
            case("Two factors", 200.0, json!([{"cell":"C1","value":15},{"cell":"D1","value":3}])),
            case("Incomplete", 300.0, json!([{"cell":"C1","value":5}])),
            case("Unchanged", 100.0, json!([{"cell":"C1","value":10.0},{"cell":"D1","value":2.0}])),
        ];
        assert_eq!(parameter_effects(&cases), vec![("High".to_string(), 10.0), ("Low".to_string(), -5.0)]);
    }
    #[test]
    fn charts_keep_tiny_close_large_and_overflowing_axis_ranges() {
        let large = 1e300_f64;
        for (low, middle, high) in [
            (1e-300, 2e-300, 3e-300),
            (large, f64::from_bits(large.to_bits() + 1), f64::from_bits(large.to_bits() + 2)),
            (-1e308, 0.0, 1e308),
            (f64::from_bits(1), f64::from_bits(2), f64::from_bits(3)),
        ] {
            assert_eq!(unit_position(low, low, high), 0.0);
            assert!((unit_position(middle, low, high) - 0.5).abs() < 1e-14);
            assert_eq!(unit_position(high, low, high), 1.0);
        }
        assert_eq!(unit_position(3.0, 3.0, 3.0), 0.5);
    }
    #[test]
    fn histogram_and_cdf_ignore_unrelated_output_magnitude_and_scatter_uses_independent_axes() {
        let rows = [json!([1.0, 1e300]), json!([2.0, 1e300]), json!([3.0, 1e300])];
        let points = sample_points(&rows, false);
        assert_eq!(points, vec![(1.0, 0.0), (2.0, 0.0), (3.0, 0.0)]);
        let bins = histogram_bins(&points, 1.0, 3.0);
        assert_eq!(bins.iter().sum::<usize>(), 3);
        assert_eq!((bins[0], bins[10], bins[19]), (1, 1, 1));
        let scatter = sample_points(&rows, true);
        for (index, point) in scatter.iter().enumerate() {
            assert_eq!(unit_position(point.0, 1.0, 3.0), index as f64 / 2.0);
            assert_eq!(unit_position(point.1, 1e300, 1e300), 0.5);
        }
    }
    #[test]
    fn closed_document_releases_the_panes_retained_result() {
        let ctx = egui::Context::default();
        let preview = Arc::new(json!({"samples":[[1.0]]}));
        let weak = Arc::downgrade(&preview);
        let state = State { preview: Some(preview), ..State::default() };
        ctx.data_mut(|memory| {
            memory.insert_temp(egui::Id::unique(("solvercraft-pane", 42_u64)), state);
            memory.insert_temp(egui::Id::unique("solvercraft-pane-documents"), vec![42_u64]);
        });
        forget_closed(&ctx, &[]);
        assert!(weak.upgrade().is_none());
    }
}
