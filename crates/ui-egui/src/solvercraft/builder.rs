//! General workbook model editor. JSON remains the canonical lossless representation.
use super::{Json, SheetApp, State, constraint_rows, field, help, json, label, pick_reference, selected_reference, text_or_json};
use crate::theme::Tokens;
use std::collections::HashMap;

const PAGE_SIZE: usize = 8;
#[cfg(test)]
mod tests;
#[derive(Clone, Default)]
pub(super) struct Editor {
    variable_page: usize,
    constraint_page: usize,
    input_page: usize,
    pub(super) observations: HashMap<usize, String>,
    pub(super) previews: HashMap<usize, (Json, Result<Json, String>)>,
    pub(super) issues: Vec<String>,
}

fn heading(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).strong().color(Tokens::get(ui.ctx()).accent_dark));
    ui.small(hint);
}
fn pages(ui: &mut egui::Ui, count: usize, page: &mut usize, name: &str) -> std::ops::Range<usize> {
    let last = count.saturating_sub(1) / PAGE_SIZE;
    *page = (*page).min(last);
    if last > 0 {
        ui.horizontal(|ui| {
            if ui.add_enabled(*page > 0, egui::Button::new("‹")).accessible_name(format!("Previous {name}")).clicked() {
                *page -= 1;
            }
            ui.small(format!("{} / {}", *page + 1, last + 1));
            if ui.add_enabled(*page < last, egui::Button::new("›")).accessible_name(format!("Next {name}")).clicked() {
                *page += 1;
            }
        });
    }
    *page * PAGE_SIZE..count.min((*page + 1) * PAGE_SIZE)
}
fn card(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().fill(t.window).stroke(egui::Stroke::new(1.0, t.separator)).corner_radius(6).inner_margin(8).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
}
fn reference_rows(text: &str) -> Result<Vec<Json>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    if text.len() > 131_072 {
        return Err("The cell configuration exceeds its size limit".into());
    }
    match text_or_json(text)? {
        Json::String(reference) => Ok(vec![json!({"cell": reference})]),
        Json::Array(rows) if rows.len() <= 4096 && rows.iter().all(|row| row.is_string() || row.is_object()) => {
            Ok(rows.into_iter().map(|row| if let Some(cell) = row.as_str() { json!({"cell": cell}) } else { row }).collect())
        }
        _ => Err("Use cell references or a bounded JSON array".into()),
    }
}

pub(super) fn show(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    state.editor.issues.clear();
    let previous_kind = state.kind.clone();
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.kind, "optimization".into(), label(de, "Optimization", "Optimierung"));
        ui.selectable_value(&mut state.kind, "simulation".into(), "Simulation");
        help(ui, label(de, "Help with model types", "Hilfe zu Modelltypen"), label(de,
            "Optimization chooses values to improve a target while respecting conditions. Simulation explores how uncertain inputs affect results.",
            "Optimierung wählt Werte, um ein Ziel unter Bedingungen zu verbessern. Simulation untersucht, wie unsichere Eingaben die Ergebnisse beeinflussen."));
    });
    if state.kind != previous_kind {
        state.request = json!({});
        state.editor = Editor::default();
        state.worksheet_bindings = false;
    }
    if state.kind == "optimization" {
        optimization(app, ui, state, de);
    } else {
        simulation(app, ui, state, de);
    }
    ui.collapsing(label(de, "Expert settings", "Experteneinstellungen"), |ui| expert(app, ui, state, de));
    if let Some(problem) = readiness(state, de) {
        ui.label(egui::RichText::new(problem).color(Tokens::get(ui.ctx()).text_dim));
    }
}

fn optimization(app: &SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    heading(
        ui,
        label(de, "1 · Set the goal", "1 · Ziel festlegen"),
        label(de, "Choose the calculated cell you want to improve.", "Wähle die berechnete Zelle, die du verbessern möchtest."),
    );
    field(ui, label(de, "Objective cell or formula", "Zielzelle oder Formel"), &mut state.objective);
    pick_reference(
        ui,
        label(de, "Use selection as objective", "Markierung als Zielzelle"),
        &mut state.objective,
        selected_reference(app, true).as_deref(),
        de,
    );
    ui.horizontal_wrapped(|ui| {
        for (value, en, german) in
            [("min", "Minimize", "Minimieren"), ("max", "Maximize", "Maximieren"), ("value", "Reach a value", "Zielwert erreichen")]
        {
            ui.selectable_value(&mut state.goal, value.into(), label(de, en, german));
        }
    });
    if state.goal == "value" {
        field(ui, label(de, "Target value", "Zielwert"), &mut state.target);
    }
    heading(
        ui,
        label(de, "2 · Choose what can change", "2 · Veränderbare Zellen wählen"),
        label(
            de,
            "Use input cells. Set a type and optional bounds for each cell or group.",
            "Verwende Eingabezellen. Lege Typ und optionale Grenzen je Zelle oder Bereich fest.",
        ),
    );
    variable_controls(app, ui, state, de);
    ui.checkbox(&mut state.nonnegative, label(de, "Nonnegative values by default", "Standardmäßig keine negativen Werte")).on_hover_text(label(
        de,
        "Cells without an explicit lower bound cannot become negative. An explicit lower bound takes precedence.",
        "Zellen ohne eigene Untergrenze dürfen nicht negativ werden. Eine ausdrücklich gesetzte Untergrenze hat Vorrang.",
    ));
    heading(
        ui,
        label(de, "3 · Add conditions", "3 · Bedingungen hinzufügen"),
        label(
            de,
            "Define the limits the solution must respect. Conditions are optional.",
            "Lege fest, welche Grenzen die Lösung einhalten muss. Bedingungen sind optional.",
        ),
    );
    conditions(app, ui, state, de);
}

fn variable_controls(app: &SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    let Ok(mut rows) = reference_rows(&state.variables) else {
        ui.colored_label(
            Tokens::get(ui.ctx()).danger,
            label(de, "Check the decision cells in the expert configuration.", "Prüfe die Entscheidungszellen in der Expertenkonfiguration."),
        );
        return;
    };
    let before = rows.clone();
    let mut remove = None;
    for index in pages(ui, rows.len(), &mut state.editor.variable_page, "variable groups") {
        let Some(row) = rows.get_mut(index) else { continue };
        ui.push_id(("variable", index), |ui| {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{} {}", label(de, "Cell group", "Zellbereich"), index + 1));
                    if ui.small_button("×").accessible_name(format!("Remove variable group {}", index + 1)).clicked() {
                        remove = Some(index);
                    }
                });
                let mut cell = row["cell"].as_str().unwrap_or("").to_owned();
                let caption = if index == 0 {
                    label(de, "Decision cells", "Entscheidungszellen").to_owned()
                } else {
                    format!("{} {}", label(de, "Decision cells", "Entscheidungszellen"), index + 1)
                };
                field(ui, &caption, &mut cell);
                pick_reference(
                    ui,
                    label(de, "Use selected cells", "Markierte Zellen verwenden"),
                    &mut cell,
                    selected_reference(app, false).as_deref(),
                    de,
                );
                if row["cell"].as_str() != Some(&cell) {
                    row["cell"] = json!(cell);
                }
                let mut kind = row["kind"].as_str().unwrap_or("continuous").to_owned();
                let original = kind.clone();
                craft_ui::ComboBox::from_id_salt("variable-kind")
                    .selected_text(variable_kind(&kind, de))
                    .show_ui(ui, |ui| {
                        for value in ["continuous", "integer", "binary"] {
                            ui.selectable_value(&mut kind, value.into(), variable_kind(value, de));
                        }
                    })
                    .response
                    .accessible_name(format!("Variable type {}", index + 1));
                if kind != original {
                    row["kind"] = json!(kind);
                }
                bound(ui, row, "lower", label(de, "Minimum", "Untergrenze"), index);
                bound(ui, row, "upper", label(de, "Maximum", "Obergrenze"), index);
                if row["lower"].as_f64().zip(row["upper"].as_f64()).is_some_and(|(low, high)| low > high) {
                    ui.colored_label(
                        Tokens::get(ui.ctx()).danger,
                        label(de, "Minimum must not exceed maximum.", "Die Untergrenze darf nicht über der Obergrenze liegen."),
                    );
                }
            })
        });
    }
    if let Some(index) = remove {
        rows.remove(index);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(rows.len() < 4096, egui::Button::new(label(de, "Add cell group", "Zellbereich hinzufügen"))).clicked() {
            rows.push(json!({"cell": ""}));
            state.editor.variable_page = rows.len().saturating_sub(1) / PAGE_SIZE;
        }
        if ui
            .add_enabled(
                rows.len() < 4096 && selected_reference(app, false).is_some(),
                egui::Button::new(label(de, "Use selection as decision cells", "Markierung als Entscheidungszellen")),
            )
            .clicked()
            && let Some(cell) = selected_reference(app, false)
        {
            rows.push(json!({"cell": cell}));
        }
    });
    if rows != before {
        state.variables = json!(rows).to_string();
    }
}
fn variable_kind(kind: &str, de: bool) -> &str {
    match kind {
        "continuous" => label(de, "Any number", "Beliebige Zahl"),
        "integer" | "int" => label(de, "Whole numbers", "Ganze Zahlen"),
        "binary" | "bin" => label(de, "Yes / no (0 or 1)", "Ja / Nein (0 oder 1)"),
        _ => kind,
    }
}
fn bound(ui: &mut egui::Ui, row: &mut Json, key: &str, caption: &str, index: usize) {
    let mut enabled = row[key].is_number();
    ui.horizontal(|ui| {
        if ui.checkbox(&mut enabled, caption).accessible_name(format!("Variable {} {key} enabled", index + 1)).changed() {
            if enabled {
                row[key] = json!(0.0);
            } else if let Some(object) = row.as_object_mut() {
                object.remove(key);
            }
        }
        if enabled {
            let mut value = row[key].as_f64().unwrap_or(0.0);
            if ui.add(egui::DragValue::new(&mut value).speed(0.1)).accessible_name(format!("Variable {} {key}", index + 1)).changed() {
                row[key] = json!(value);
            }
        }
    });
}

fn condition_object(row: &Json) -> Option<Json> {
    if row.is_object() {
        return Some(row.clone());
    }
    let text = row.as_str()?;
    let expression = gridcraft_engine::formula::parse(text.trim().trim_start_matches('=')).ok()?;
    let gridcraft_engine::formula::Expr::Binary(op, left, right) = expression else { return None };
    use gridcraft_engine::formula::BinOp;
    let relation = match op {
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Eq => "=",
        _ => return None,
    };
    Some(json!({"lhs": gridcraft_engine::formula::print(&left), "op": relation, "rhs": gridcraft_engine::formula::print(&right)}))
}
fn condition_fields(ui: &mut egui::Ui, row: &mut Json, index: usize, de: bool) -> bool {
    let mut left = row["lhs"].as_str().unwrap_or("").to_owned();
    let mut right = row["rhs"].as_str().map(str::to_owned).unwrap_or_else(|| row["rhs"].as_f64().map_or(String::new(), |n| n.to_string()));
    let mut op = row["op"].as_str().unwrap_or("<=").to_owned();
    let before = (left.clone(), op.clone(), right.clone());
    let mut remove = false;
    ui.horizontal(|ui| {
        let width = ((ui.available_width() - 80.0) / 2.0).max(30.0);
        ui.add_sized([width, 24.0], egui::TextEdit::singleline(&mut left).hint_text(label(de, "Cells", "Zellen")).char_limit(131_072))
            .accessible_name(format!("{} {}", label(de, "Condition cells", "Bedingungszellen"), index + 1));
        let caption = match op.as_str() {
            "int" => label(de, "Whole", "Ganz"),
            "bin" => "0/1",
            _ => &op,
        }
        .to_owned();
        ui.menu_button(caption, |ui| {
            for relation in ["<=", "=", ">=", "int", "bin"] {
                let caption = match relation {
                    "int" => label(de, "Whole numbers", "Ganze Zahlen"),
                    "bin" => "0 / 1",
                    _ => relation,
                };
                if ui.selectable_label(op == relation, caption).clicked() {
                    op = relation.into();
                    ui.close();
                }
            }
        })
        .response
        .accessible_name(format!("Condition relation {}", index + 1));
        if !matches!(op.as_str(), "int" | "bin") {
            ui.add_sized([width, 24.0], egui::TextEdit::singleline(&mut right).hint_text(label(de, "Limit", "Grenze")).char_limit(131_072))
                .accessible_name(format!("{} {}", label(de, "Limit or cell", "Grenze oder Zelle"), index + 1));
        }
        remove = ui.small_button("×").accessible_name(format!("Remove condition {}", index + 1)).clicked();
    });
    if before != (left.clone(), op.clone(), right.clone()) {
        row["lhs"] = json!(left);
        row["op"] = json!(op);
        row["rhs"] = json!(right);
    }
    remove
}
fn conditions(app: &SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    let Ok(mut rows) = constraint_rows(&state.constraints) else {
        ui.colored_label(
            Tokens::get(ui.ctx()).danger,
            label(de, "Check conditions in the expert configuration.", "Prüfe die Bedingungen in der Expertenkonfiguration."),
        );
        return;
    };
    let before = rows.clone();
    let mut remove = None;
    for index in pages(ui, rows.len(), &mut state.editor.constraint_page, "conditions") {
        let Some(row) = rows.get_mut(index) else { continue };
        ui.push_id(("constraint", index), |ui| {
            card(ui, |ui| {
                if let Some(mut object) = condition_object(row) {
                    let original = object.clone();
                    if condition_fields(ui, &mut object, index, de) {
                        remove = Some(index);
                    }
                    if object != original {
                        *row = object;
                    }
                } else {
                    if ui.small_button("×").accessible_name(format!("Remove condition {}", index + 1)).clicked() {
                        remove = Some(index);
                    }
                    ui.label(row.as_str().map(str::to_owned).unwrap_or_else(|| row.to_string()));
                    ui.small(label(
                        de,
                        "Edit this condition in the expert configuration.",
                        "Diese Bedingung in der Expertenkonfiguration bearbeiten.",
                    ));
                }
            })
        });
    }
    if let Some(index) = remove {
        rows.remove(index);
    }
    ui.collapsing(label(de, "Add a condition", "Bedingung hinzufügen"), |ui| {
        field(ui, label(de, "Left cell or expression", "Linke Zelle oder Ausdruck"), &mut state.constraint_left);
        pick_reference(
            ui,
            label(de, "Use selection on the left", "Markierung links einsetzen"),
            &mut state.constraint_left,
            selected_reference(app, false).as_deref(),
            de,
        );
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
            selected_reference(app, false).as_deref(),
            de,
        );
        if ui
            .add_enabled(
                !state.constraint_left.trim().is_empty() && !state.constraint_right.trim().is_empty() && rows.len() < 4096,
                egui::Button::new(label(de, "Add constraint", "Nebenbedingung hinzufügen")),
            )
            .clicked()
        {
            rows.push(json!({"lhs": state.constraint_left.trim(), "op": state.constraint_op, "rhs": state.constraint_right.trim()}));
            state.constraint_left.clear();
            state.constraint_right.clear();
            state.editor.constraint_page = rows.len().saturating_sub(1) / PAGE_SIZE;
        }
    });
    if rows != before {
        state.constraints = json!(rows).to_string();
    }
}

fn simulation(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    heading(
        ui,
        label(de, "1 · Define uncertain inputs", "1 · Unsichere Eingaben festlegen"),
        label(
            de,
            "Choose input cells and describe how their values can vary.",
            "Wähle Eingabezellen und beschreibe, wie ihre Werte schwanken können.",
        ),
    );
    let was_worksheet = state.worksheet_bindings;
    ui.checkbox(&mut state.worksheet_bindings, label(de, "Use worksheet declarations", "Deklarationen aus dem Blatt verwenden")).on_hover_text(
        label(
            de,
            "Use existing SC distribution and output formulas in the workbook.",
            "Vorhandene SC-Verteilungsformeln und Ausgabemarkierungen der Mappe verwenden.",
        ),
    );
    if was_worksheet && !state.worksheet_bindings {
        if state.inputs == "null" {
            state.inputs = "[]".into();
        }
        if state.outputs == "null" {
            state.outputs.clear();
        }
    }
    if !state.worksheet_bindings {
        inputs(app, ui, state, de);
    }
    heading(
        ui,
        label(de, "2 · Choose the results to watch", "2 · Ergebniszellen wählen"),
        label(
            de,
            "Select calculated output cells that depend on the uncertain inputs.",
            "Wähle berechnete Ergebniszellen, die von den unsicheren Eingaben abhängen.",
        ),
    );
    if !state.worksheet_bindings {
        field(ui, label(de, "Output cells", "Ergebniszellen"), &mut state.outputs);
        pick_reference(
            ui,
            label(de, "Use selection as outputs", "Markierung als Ausgaben"),
            &mut state.outputs,
            selected_reference(app, false).as_deref(),
            de,
        );
    } else {
        ui.small(label(de, "Outputs come from the worksheet declarations.", "Die Ergebniszellen stammen aus den Blattdeklarationen."));
    }
    heading(
        ui,
        label(de, "3 · Set the number of trials", "3 · Anzahl Versuche festlegen"),
        label(de, "Each trial recalculates your outputs with new input values.", "Jeder Versuch berechnet die Ergebnisse mit neuen Eingabewerten."),
    );
    ui.horizontal(|ui| {
        ui.label(label(de, "Trials", "Versuche"));
        ui.add(egui::DragValue::new(&mut state.trials).range(1..=1_000_000)).accessible_name(label(
            de,
            "Number of simulation trials",
            "Anzahl Simulationsversuche",
        ));
    });
}
fn inputs(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    let Ok(mut rows) = serde_json::from_str::<Vec<Json>>(&state.inputs) else {
        state.editor.issues.push(label(de, "Check the input configuration below.", "Prüfe die Eingabekonfiguration unten.").into());
        return;
    };
    if rows.len() > 4096 || rows.iter().any(|row| !row.is_object()) {
        state.editor.issues.push(
            label(
                de,
                "Each uncertain input must be a cell and distribution object.",
                "Jede unsichere Eingabe benötigt eine Zelle und eine Verteilung.",
            )
            .into(),
        );
        return;
    }
    let before = rows.clone();
    let mut remove = None;
    for index in pages(ui, rows.len(), &mut state.editor.input_page, "uncertain inputs") {
        let Some(row) = rows.get_mut(index) else { continue };
        ui.push_id(("input", index), |ui| {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{} {}", label(de, "Uncertain input", "Unsichere Eingabe"), index + 1));
                    if ui.small_button("×").accessible_name(format!("Remove uncertain input {}", index + 1)).clicked() {
                        remove = Some(index);
                    }
                });
                let mut cell = row["cell"].as_str().unwrap_or("").to_owned();
                field(ui, &format!("{} {}", label(de, "Input cell", "Eingabezelle"), index + 1), &mut cell);
                pick_reference(
                    ui,
                    label(de, "Use selected input cell", "Markierte Eingabezelle verwenden"),
                    &mut cell,
                    selected_reference(app, true).as_deref(),
                    de,
                );
                if row["cell"].as_str() != Some(&cell) {
                    row["cell"] = json!(cell);
                }
                super::distribution::controls(ui, &mut row["distribution"], index, de, &mut state.editor);
                super::distribution::preview(app, ui, &row["distribution"], index, de, &mut state.editor);
            })
        });
    }
    if let Some(index) = remove {
        rows.remove(index);
        state.editor.previews.clear();
        state.editor.observations = state
            .editor
            .observations
            .drain()
            .filter(|(key, _)| *key != index)
            .map(|(key, text)| (if key > index { key - 1 } else { key }, text))
            .collect();
    }
    if ui.add_enabled(rows.len() < 4096, egui::Button::new(label(de, "Add uncertain input", "Unsichere Eingabe hinzufügen"))).clicked() {
        rows.push(json!({"cell": selected_reference(app, true).unwrap_or_default(), "distribution": {"kind": "uniform", "low": 0.0, "high": 1.0}}));
        state.editor.input_page = rows.len().saturating_sub(1) / PAGE_SIZE;
    }
    if rows != before {
        state.inputs = json!(rows).to_string();
    }
}

fn expert(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    if state.kind == "optimization" {
        craft_ui::ComboBox::from_id_salt("solvercraft-method")
            .selected_text(method_name(&state.method, de))
            .show_ui(ui, |ui| {
                for method in ["auto", "simplex", "branch_and_bound", "projected_gradient", "differential_evolution"] {
                    ui.selectable_value(&mut state.method, method.into(), method_name(method, de));
                }
            })
            .response
            .accessible_name(label(de, "Solving method", "Lösungsmethode"));
        help(
            ui,
            label(de, "Choosing a solving method", "Lösungsmethode wählen"),
            label(
                de,
                "Automatic chooses a supported engine for the mathematical model. Change this only when you need a specific method.",
                "Automatisch wählt ein unterstütztes Verfahren für das mathematische Modell. Ändere dies nur, wenn du ein bestimmtes Verfahren benötigst.",
            ),
        );
        ui.collapsing(label(de, "Raw model configuration", "Technische Modellkonfiguration"), |ui| {
            field(ui, "Decision cells JSON", &mut state.variables);
            ui.label(label(de, "Conditions as text or JSON", "Bedingungen als Text oder JSON"));
            ui.add(egui::TextEdit::multiline(&mut state.constraints).desired_width(f32::INFINITY).char_limit(131_072))
                .accessible_name("Solvercraft constraints");
        });
    } else {
        field(ui, label(de, "Random seed", "Zufallsstartwert"), &mut state.seed);
        let mut method = state.request["options"]["method"].as_str().unwrap_or("monte_carlo").to_owned();
        let previous = method.clone();
        let caption = match method.as_str() {
            "latin_hypercube" => "Latin hypercube",
            "monte_carlo" => "Monte Carlo",
            _ => &method,
        }
        .to_owned();
        craft_ui::ComboBox::from_id_salt("sampling-method")
            .selected_text(caption)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut method, "monte_carlo".into(), "Monte Carlo");
                ui.selectable_value(&mut method, "latin_hypercube".into(), "Latin hypercube");
            })
            .response
            .accessible_name(label(de, "Sampling method", "Stichprobenverfahren"));
        if method != previous {
            if !state.request["options"].is_object() {
                state.request["options"] = json!({});
            }
            state.request["options"]["method"] = json!(method);
        }
        ui.collapsing(label(de, "Raw simulation configuration", "Technische Simulationskonfiguration"), |ui| {
            ui.label(label(
                de,
                "All supported distribution types and modifiers can be edited here.",
                "Hier lassen sich alle unterstützten Verteilungen und Modifikatoren bearbeiten.",
            ));
            if ui
                .add(egui::TextEdit::multiline(&mut state.inputs).desired_rows(4).desired_width(f32::INFINITY).char_limit(131_072))
                .accessible_name("Solvercraft simulation inputs")
                .changed()
            {
                state.editor.observations.clear();
                state.editor.previews.clear();
            }
            field(ui, label(de, "Output references or JSON array", "Ausgabereferenzen oder JSON-Array"), &mut state.outputs);
        });
        let before = (state.compare_parameters, state.parameter_grid.clone());
        parameter_controls(app, ui, state, de);
        state.parameters_edited |= before != (state.compare_parameters, state.parameter_grid.clone());
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
fn method_name(method: &str, de: bool) -> &str {
    match method {
        "auto" => label(de, "Automatic", "Automatisch"),
        "simplex" => label(de, "Linear optimization (Simplex)", "Lineare Optimierung (Simplex)"),
        "branch_and_bound" => label(de, "Whole-number optimization", "Ganzzahlige Optimierung"),
        "projected_gradient" => label(de, "Smooth nonlinear optimization", "Glatte nichtlineare Optimierung"),
        "differential_evolution" => label(de, "Evolutionary search", "Evolutionäre Suche"),
        _ => method,
    }
}
fn parameter_controls(app: &SheetApp, ui: &mut egui::Ui, state: &mut State, de: bool) {
    ui.checkbox(&mut state.compare_parameters, label(de, "Compare parameters", "Parameter vergleichen"));
    if !state.compare_parameters {
        return;
    }
    if state.parameter_grid.is_empty() && !state.parameters_edited && state.request["parameter_cases"].is_array() {
        ui.small(label(
            de,
            "The saved custom scenarios are preserved. Adding a parameter replaces them with a low/base/high comparison.",
            "Die gespeicherten Szenarien bleiben erhalten. Ein neuer Parameter ersetzt sie durch einen Niedrig-/Basis-/Hoch-Vergleich.",
        ));
    }
    let mut remove = None;
    for (index, parameter) in state.parameter_grid.iter_mut().enumerate() {
        ui.push_id(("parameter", index), |ui| {
            field(ui, label(de, "Parameter cell", "Parameterzelle"), &mut parameter.cell);
            pick_reference(
                ui,
                label(de, "Use selection as parameter", "Markierung als Parameter"),
                &mut parameter.cell,
                selected_reference(app, true).as_deref(),
                de,
            );
            ui.horizontal_wrapped(|ui| {
                for (en, german, value) in
                    [("Low", "Niedrig", &mut parameter.low), ("Base", "Basis", &mut parameter.base), ("High", "Hoch", &mut parameter.high)]
                {
                    ui.label(label(de, en, german));
                    ui.add(egui::DragValue::new(value).speed(0.1).range(-1e100..=1e100)).accessible_name(format!("Parameter {} {en}", index + 1));
                }
                if ui.button(label(de, "Remove", "Entfernen")).clicked() {
                    remove = Some(index);
                }
            });
        });
    }
    if let Some(index) = remove {
        state.parameter_grid.remove(index);
    }
    if ui.add_enabled(state.parameter_grid.len() < 15, egui::Button::new(label(de, "Add parameter", "Parameter hinzufügen"))).clicked() {
        state.parameter_grid.push(super::Parameter { cell: String::new(), low: 0.0, base: 1.0, high: 2.0 });
    }
}

pub(super) fn readiness(state: &State, de: bool) -> Option<String> {
    let issue = |en, german| Some(label(de, en, german).to_owned());
    if let Some(error) = state.editor.issues.first() {
        return Some(error.clone());
    }
    if state.kind == "optimization" {
        if state.objective.trim().is_empty() {
            return issue("Choose a target cell or formula.", "Wähle eine Zielzelle oder Formel.");
        }
        let rows = match reference_rows(&state.variables) {
            Ok(rows) => rows,
            Err(error) => return Some(error),
        };
        if rows.is_empty() || rows.iter().any(|row| row["cell"].as_str().is_none_or(|cell| cell.trim().is_empty())) {
            return issue("Choose the cells that Solvercraft may change.", "Wähle die Zellen, die Solvercraft verändern darf.");
        }
        if rows.iter().any(|row| row["lower"].as_f64().zip(row["upper"].as_f64()).is_some_and(|(low, high)| low > high)) {
            return issue("Correct the variable bounds.", "Korrigiere die Variablengrenzen.");
        }
        if state.goal == "value" && state.target.parse::<f64>().ok().is_none_or(|n| !n.is_finite()) {
            return issue("Enter a finite target value.", "Gib einen endlichen Zielwert ein.");
        }
        if constraint_rows(&state.constraints).is_err() {
            return issue("Check the condition configuration.", "Prüfe die Bedingungen.");
        }
        if constraint_rows(&state.constraints).ok().is_some_and(|rows| {
            rows.iter().filter(|row| row.is_object()).any(|row| {
                row["lhs"].as_str().is_none_or(|left| left.trim().is_empty())
                    || (!matches!(row["op"].as_str(), Some("int" | "bin"))
                        && (row["rhs"].is_null() || row["rhs"].as_str().is_some_and(|right| right.trim().is_empty())))
            })
        }) {
            return issue("Complete the cells and limits in each condition.", "Vervollständige Zellen und Grenzen in jeder Bedingung.");
        }
        if !state.constraint_left.trim().is_empty() || !state.constraint_right.trim().is_empty() {
            return issue("Add or clear the pending condition.", "Füge die begonnene Bedingung hinzu oder leere ihre Felder.");
        }
    } else {
        if state.seed.parse::<u64>().is_err() {
            return issue(
                "Enter a whole-number random seed of at most 18446744073709551615.",
                "Gib einen ganzzahligen Zufallsstartwert bis 18446744073709551615 ein.",
            );
        }
        if state.compare_parameters && state.parameter_grid.is_empty() && (state.parameters_edited || !state.request["parameter_cases"].is_array()) {
            return issue("Add a parameter to compare.", "Füge einen zu vergleichenden Parameter hinzu.");
        }
        if state.compare_parameters && state.parameter_grid.iter().any(|parameter| parameter.cell.trim().is_empty()) {
            return issue("Choose the cell for each comparison parameter.", "Wähle die Zelle für jeden Vergleichsparameter.");
        }
        if !state.worksheet_bindings {
            let rows = match serde_json::from_str::<Vec<Json>>(&state.inputs) {
                Ok(rows) => rows,
                Err(error) => return Some(error.to_string()),
            };
            if rows.is_empty() || rows.iter().any(|row| row["cell"].as_str().is_none_or(|cell| cell.trim().is_empty())) {
                return issue("Choose at least one uncertain input cell.", "Wähle mindestens eine unsichere Eingabezelle.");
            }
            if state.outputs.trim().is_empty() || state.outputs.trim() == "[]" {
                return issue("Choose the output cells to watch.", "Wähle die zu untersuchenden Ergebniszellen.");
            }
        }
    }
    None
}
