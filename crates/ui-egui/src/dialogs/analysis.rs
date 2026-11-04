use super::show::{form_fields, ok_cancel};
use super::{Dialog, Field, Json, Map, SheetApp, json, vec2};

pub(super) fn goal_seek(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let get = |k: &str| d.values.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let (mut set, mut to, mut changing) = (get("set"), get("to"), get("changing"));
    egui::Grid::new("gs").show(ui, |ui| {
        ui.label("Set cell:");
        ui.text_edit_singleline(&mut set);
        ui.end_row();
        ui.label("To value:");
        ui.text_edit_singleline(&mut to);
        ui.end_row();
        ui.label("By changing cell:");
        ui.text_edit_singleline(&mut changing);
        ui.end_row();
    });
    d.values.insert("set".into(), json!(set.clone()));
    d.values.insert("to".into(), json!(to.clone()));
    d.values.insert("changing".into(), json!(changing.clone()));
    let mut open = true;
    ok_cancel(ui, confirm, &mut open);
    if *confirm {
        match app.run("data.goalSeek", json!({"set": set, "to": to.parse::<f64>().unwrap_or(0.0), "changing": changing})) {
            Ok(r) => app.toast = Some((format!("Goal Seeking found a solution: {}", r["value"]), crate::now_ms())),
            Err(e) => app.message = Some(("Goal Seek".into(), crate::clean_error(&e))),
        }
    }
    if !open {
        *confirm = true;
    }
}

/// The fields of one Analysis ToolPak tool's form (input and options; output is drawn apart).
fn toolpak_fields(tool: &str) -> Vec<Field> {
    use Field::*;
    let grouped = || Choice { key: "groupedBy", label: "Grouped By:", options: vec![("columns", "Columns"), ("rows", "Rows")] };
    let labels = || Check { key: "labels", label: "Labels in first row" };
    let range = || Text { key: "range", label: "Input Range:" };
    let two = || vec![Text { key: "range1", label: "Variable 1 Range:" }, Text { key: "range2", label: "Variable 2 Range:" }];
    match tool {
        "descriptive" => vec![
            range(),
            grouped(),
            labels(),
            Check { key: "summary", label: "Summary statistics" },
            Text { key: "confidence", label: "Confidence Level for Mean (%), blank for none:" },
            Text { key: "kthLargest", label: "Kth Largest (blank for none):" },
            Text { key: "kthSmallest", label: "Kth Smallest (blank for none):" },
        ],
        "histogram" => vec![
            range(),
            Text { key: "bins", label: "Bin Range (blank for automatic bins):" },
            labels(),
            Check { key: "pareto", label: "Pareto (sorted histogram)" },
            Check { key: "cumulative", label: "Cumulative Percentage" },
        ],
        "correlation" | "covariance" | "rankPercentile" => vec![range(), grouped(), labels()],
        "anovaSingle" => vec![range(), grouped(), labels(), Text { key: "alpha", label: "Alpha:" }],
        "regression" => vec![
            Text { key: "inputY", label: "Input Y Range:" },
            Text { key: "inputX", label: "Input X Range:" },
            labels(),
            Check { key: "constantZero", label: "Constant is Zero" },
            Text { key: "confidence", label: "Confidence Level (%):" },
            Check { key: "residuals", label: "Residuals" },
            Check { key: "standardizedResiduals", label: "Standardized Residuals" },
            Check { key: "normalProbability", label: "Normal Probability" },
        ],
        "movingAverage" => {
            vec![range(), labels(), Text { key: "interval", label: "Interval:" }, Check { key: "standardErrors", label: "Standard Errors" }]
        }
        "exponentialSmoothing" => {
            vec![range(), labels(), Text { key: "damping", label: "Damping factor:" }, Check { key: "standardErrors", label: "Standard Errors" }]
        }
        "fTest" => {
            let mut v = two();
            v.extend([labels(), Text { key: "alpha", label: "Alpha:" }]);
            v
        }
        _ => {
            let mut v = two();
            v.extend([
                Text { key: "hypothesizedDifference", label: "Hypothesized Mean Difference:" },
                labels(),
                Text { key: "alpha", label: "Alpha:" },
            ]);
            v
        }
    }
}

/// The starting values of a tool's form: the selection as its input, a new sheet as output.
pub(super) fn toolpak_defaults(sel: &str, tool: &str) -> Map<String, Json> {
    let mut v = json!({
        "tool": tool, "range": sel, "inputY": "", "inputX": "", "range1": sel, "range2": "", "bins": "",
        "groupedBy": "columns", "labels": false, "summary": true, "confidence": "", "kthLargest": "", "kthSmallest": "",
        "interval": "3", "damping": "0.3", "alpha": "0.05", "hypothesizedDifference": "0",
        "outputTo": "newSheet", "output": "", "newSheet": "",
    });
    if tool == "regression" {
        v["confidence"] = json!("95");
        v["inputY"] = json!(sel);
    }
    v.as_object().cloned().unwrap_or_default()
}

/// Data › Data Analysis: pick a tool, then fill in its form. OK runs `data.analysisToolPak`
/// (through the dialog's command); a report that would replace data asks first.
pub(super) fn analysis_toolpak(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use gridcraft_engine::cmd::analysis::TOOLS;
    if d.values.get("askOverwrite").and_then(Json::as_bool) == Some(true) {
        ui.label("The output range already holds data. Replace it with the report?");
        ui.horizontal(|ui| {
            if ui.button("Replace").clicked() {
                d.values.remove("askOverwrite");
                d.values.insert("overwrite".into(), json!(true));
                *confirm = true;
            }
            if ui.button("Cancel").clicked() {
                d.values.remove("askOverwrite");
            }
        });
        return;
    }
    let tool = d.values.get("tool").and_then(Json::as_str).unwrap_or("").to_string();
    let Some((_, label)) = TOOLS.iter().find(|(id, _)| *id == tool) else {
        ui.label(egui::RichText::new("Analysis Tools").strong());
        let pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
        let mut chosen = false;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (id, label) in TOOLS {
                let r = ui.selectable_label(pick == *id, *label);
                if r.clicked() {
                    d.values.insert("pick".into(), json!(id));
                }
                if r.double_clicked() {
                    chosen = true;
                }
            }
        });
        let mut ok = false;
        ok_cancel(ui, &mut ok, open);
        let pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
        if (ok || chosen) && TOOLS.iter().any(|(id, _)| *id == pick) {
            let sel = app.session.active().map(|doc| doc.selection.current().a1()).unwrap_or_default();
            d.values = toolpak_defaults(&sel, &pick);
        }
        return;
    };
    d.title = label.to_string();
    d.fields = toolpak_fields(&tool);
    form_fields(ui, d);
    ui.separator();
    ui.label(egui::RichText::new("Output options").strong());
    let to = d.values.get("outputTo").and_then(Json::as_str).unwrap_or("newSheet").to_string();
    for (key, choice, text_label) in [("output", "range", "Output Range:"), ("newSheet", "newSheet", "New Worksheet Ply:")] {
        ui.horizontal(|ui| {
            if ui.radio(to == choice, text_label).clicked() {
                d.values.insert("outputTo".into(), json!(choice));
            }
            let mut t = d.values.get(key).and_then(Json::as_str).unwrap_or("").to_string();
            if ui.add(egui::TextEdit::singleline(&mut t).desired_width(160.0)).changed() {
                d.values.insert(key.into(), json!(t));
                d.values.insert("outputTo".into(), json!(choice));
            }
        });
    }
    ok_cancel(ui, confirm, open);
}

/// Data › Solver: the Solver Parameters form. Solve runs `data.solver` through the dialog's
/// command; the result opens Solver Results.
pub(super) fn solver(ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let text = |d: &Dialog, k: &str| d.values.get(k).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
    let edit = |ui: &mut egui::Ui, d: &mut Dialog, k: &str, width: f32, label: &str| {
        let mut t = text(d, k);
        if ui.add(egui::TextEdit::singleline(&mut t).desired_width(width)).accessible_name(lang.tr(label)).changed() {
            d.values.insert(k.into(), json!(t));
        }
    };
    ui.horizontal(|ui| {
        ui.label("Set Objective:");
        edit(ui, d, "objective", 200.0, "Set Objective:");
    });
    let goal = text(d, "goal");
    ui.horizontal(|ui| {
        ui.label("To:");
        for (g, label) in [("max", "Max"), ("min", "Min"), ("value", "Value Of:")] {
            if ui.radio(goal == g, label).clicked() {
                d.values.insert("goal".into(), json!(g));
            }
        }
        edit(ui, d, "value", 80.0, "Value Of:");
    });
    ui.label("By Changing Variable Cells:");
    edit(ui, d, "variables", 420.0, "By Changing Variable Cells:");
    ui.label("Subject to the Constraints:");
    let list: Vec<String> = d
        .values
        .get("constraints")
        .and_then(Json::as_array)
        .map(|a| a.iter().map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string())).collect())
        .unwrap_or_default();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_size(vec2(420.0, 90.0));
        egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
            for (i, c) in list.iter().enumerate() {
                if ui.selectable_label(d.list_index == i, c).clicked() {
                    d.list_index = i;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        edit(ui, d, "newLhs", 110.0, "Constraint Cell Reference");
        let op = text(d, "newOp");
        craft_ui::ComboBox::from_id_salt("solverOp")
            .selected_text(op.clone())
            .width(60.0)
            .show_ui(ui, |ui| {
                for o in ["<=", "=", ">=", "int", "bin"] {
                    if ui.selectable_label(op == o, o).clicked() {
                        d.values.insert("newOp".into(), json!(o));
                    }
                }
            })
            .response
            .accessible_name(lang.tr("Constraint Operator"));
        if !matches!(op.as_str(), "int" | "bin") {
            edit(ui, d, "newRhs", 110.0, "Constraint Value");
        }
        if ui.button("Add").clicked() {
            let lhs = text(d, "newLhs");
            if !lhs.trim().is_empty() {
                let rhs = if matches!(op.as_str(), "int" | "bin") { String::new() } else { format!(" {}", text(d, "newRhs").trim()) };
                let mut list = list.clone();
                list.push(format!("{} {op}{rhs}", lhs.trim()));
                d.values.insert("constraints".into(), json!(list));
                d.values.insert("newLhs".into(), json!(""));
                d.values.insert("newRhs".into(), json!(""));
            }
        }
        if ui.add_enabled(d.list_index < list.len(), egui::Button::new("Delete")).clicked() {
            let mut list = list.clone();
            if d.list_index < list.len() {
                list.remove(d.list_index);
            }
            d.values.insert("constraints".into(), json!(list));
        }
    });
    let mut nonneg = d.values.get("nonNegative").and_then(Json::as_bool).unwrap_or(true);
    if ui.checkbox(&mut nonneg, "Make Unconstrained Variables Non-Negative").changed() {
        d.values.insert("nonNegative".into(), json!(nonneg));
    }
    let method = text(d, "method");
    ui.horizontal(|ui| {
        ui.label("Select a Solving Method:");
        let label = |m: &str| match m {
            "auto" => "Automatic",
            "branch_and_bound" => "Integer branch and bound",
            "projected_gradient" => "Projected gradient",
            "differential_evolution" => "Differential evolution (heuristic)",
            "simplex" => "Simplex LP",
            "evolutionary" => "Evolutionary",
            _ => "GRG Nonlinear",
        };
        craft_ui::ComboBox::from_id_salt("solverMethod")
            .selected_text(label(&method))
            .show_ui(ui, |ui| {
                let methods: &[&str] = if d.name == "solvercraft" {
                    &["auto", "simplex", "branch_and_bound", "projected_gradient", "differential_evolution"]
                } else {
                    &["grg", "simplex", "evolutionary"]
                };
                for &m in methods {
                    if ui.selectable_label(method == m, label(m)).clicked() {
                        d.values.insert("method".into(), json!(m));
                    }
                }
            })
            .response
            .accessible_name(lang.tr("Select a Solving Method:"));
    });
    ui.label(
        egui::RichText::new(match method.as_str() {
            "simplex" => "Simplex LP: for linear models; integer and binary constraints by branch and bound.",
            "evolutionary" => "Evolutionary: a genetic search for models that aren't smooth; every variable cell needs bounds.",
            "auto" => "Structural analysis selects a supported linear, quadratic or local nonlinear method.",
            "branch_and_bound" => "Linear relaxation and integer branch and bound.",
            "projected_gradient" => "A smooth local search with variable bounds only.",
            "differential_evolution" => "A bounded heuristic; the best found solution is not a global optimality proof.",
            _ => "GRG Nonlinear: a local optimum of a smooth model, from the current values.",
        })
        .small(),
    );
    ui.add_space(8.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let t = crate::theme::Tokens::get(ui.ctx());
        let solve = ui.add(egui::Button::new(egui::RichText::new("  Solve  ").color(t.on_accent)).fill(t.accent));
        if solve.clicked() && !text(d, "objective").trim().is_empty() {
            *confirm = true;
        }
        if ui.button(" Close ").clicked() {
            *open = false;
        }
    });
}

/// Solver Results: the outcome, and keeping or restoring the variable cells.
pub(super) fn solver_results(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let message = d.values.get("message").and_then(Json::as_str).unwrap_or("").to_string();
    ui.label(message);
    let changed = d.values.get("changed").and_then(Json::as_bool).unwrap_or(false);
    let restore = d.values.get("restore").and_then(Json::as_bool).unwrap_or(false);
    if changed {
        ui.add_space(4.0);
        if ui.radio(!restore, "Keep Solver Solution").clicked() {
            d.values.insert("restore".into(), json!(false));
        }
        if ui.radio(restore, "Restore Original Values").clicked() {
            d.values.insert("restore".into(), json!(true));
        }
    }
    // Reports describe the solution whether it is kept or not.
    let solved = d.values.get("values").and_then(Json::as_object).is_some_and(|v| !v.is_empty());
    if solved {
        ui.add_space(4.0);
        ui.label("Reports:");
        for (key, label) in [("answerReport", "Answer"), ("limitsReport", "Limits")] {
            let mut on = d.values.get(key).and_then(Json::as_bool).unwrap_or(false);
            if ui.checkbox(&mut on, label).changed() {
                d.values.insert(key.into(), json!(on));
            }
        }
    }
    let mut confirm = false;
    ok_cancel(ui, &mut confirm, open);
    if confirm {
        // The solution (and the saved model) is one undo step.
        if changed && restore {
            app.run_or_alert("edit.undo", json!({}));
        }
        let reports: Vec<&str> = [("answerReport", "answer"), ("limitsReport", "limits")]
            .into_iter()
            .filter(|(key, _)| d.values.get(*key).and_then(Json::as_bool) == Some(true))
            .map(|(_, kind)| kind)
            .collect();
        if solved && !reports.is_empty() {
            let field = |k: &str| d.values.get(k).cloned().unwrap_or(Json::Null);
            let params = json!({"reports": reports, "model": field("model"), "values": field("values"), "original": field("original"), "message": field("message")});
            app.run_or_alert("data.solverReport", params);
        }
        *open = false;
    }
}

fn solvercraft_text(d: &Dialog, key: &str) -> String {
    d.values.get(key).map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).unwrap_or_default()
}
fn solvercraft_field(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut value = solvercraft_text(d, key);
        if ui.add(egui::TextEdit::singleline(&mut value).desired_width(230.0)).accessible_name(label).changed() {
            d.values.insert(key.into(), json!(value));
        }
    });
}
/// The first integrated Solvercraft surface: model input and non-mutating previews.
pub(super) fn solvercraft(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let simulation = d.tab == "simulation";
    ui.horizontal(|ui| {
        if ui.selectable_label(!simulation, "Optimization").clicked() {
            d.tab = "optimization".into();
        }
        if ui.selectable_label(simulation, "Simulation").clicked() {
            d.tab = "simulation".into();
        }
    });
    ui.separator();
    if let Some(sheet) = d.values.get("sheet").and_then(Json::as_str) {
        ui.label(format!("Model sheet: {sheet}"));
    }
    if d.tab != "simulation" {
        d.command = Some("solvercraft.solve");
        ui.label(egui::RichText::new("Preview first, then apply. This run uses a fixed computational budget.").small());
        solver(ui, d, confirm, open);
    } else {
        d.command = Some("solvercraft.simulate");
        ui.label("Describe one uncertain input and the output cells to measure.");
        solvercraft_field(ui, d, "simulationInput", "Input cell:");
        let family = solvercraft_text(d, "distribution");
        ui.horizontal(|ui| {
            ui.label("Distribution:");
            craft_ui::ComboBox::from_id_salt("solvercraftDistribution").selected_text(&family).show_ui(ui, |ui| {
                for value in ["uniform", "normal", "triangular", "exponential", "bernoulli"] {
                    if ui.selectable_label(family == value, value).clicked() {
                        d.values.insert("distribution".into(), json!(value));
                    }
                }
            });
        });
        match family.as_str() {
            "normal" => {
                solvercraft_field(ui, d, "parameter1", "Mean:");
                solvercraft_field(ui, d, "parameter2", "Standard deviation:");
            }
            "triangular" => {
                solvercraft_field(ui, d, "parameter1", "Minimum:");
                solvercraft_field(ui, d, "parameter2", "Most likely:");
                solvercraft_field(ui, d, "parameter3", "Maximum:");
            }
            "exponential" => solvercraft_field(ui, d, "parameter1", "Rate:"),
            "bernoulli" => solvercraft_field(ui, d, "parameter1", "Probability:"),
            _ => {
                solvercraft_field(ui, d, "parameter1", "Minimum:");
                solvercraft_field(ui, d, "parameter2", "Maximum:");
            }
        }
        solvercraft_field(ui, d, "simulationOutputs", "Output cells:");
        solvercraft_field(ui, d, "trials", "Trials:");
        solvercraft_field(ui, d, "seed", "Seed:");
        let latin = d.values.get("latinHypercube").and_then(Json::as_bool).unwrap_or(false);
        let mut enabled = latin;
        if ui.checkbox(&mut enabled, "Latin Hypercube sampling").changed() {
            d.values.insert("latinHypercube".into(), json!(enabled));
        }
        ui.label(egui::RichText::new("This bounded run previews results; it does not change your worksheet.").small());
        ui.horizontal(|ui| {
            if ui.button("Simulate").clicked() {
                *confirm = true;
            }
            if ui.button("Close").clicked() {
                *open = false;
            }
        });
    }
    // The surface shares no mathematical implementation with the engine.
    let _ = app;
}

pub(super) fn solvercraft_results(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let envelope = Json::Object(d.values.clone());
    let result = envelope.get("result").cloned().unwrap_or(Json::Null);
    ui.horizontal(|ui| {
        ui.strong("Termination:");
        ui.label(result.get("termination").and_then(Json::as_str).unwrap_or("unknown"));
    });
    if let Some(feasible) = result.get("feasibility").and_then(Json::as_str) {
        ui.label(format!("Feasibility: {feasible}"));
        ui.label(format!("Optimality: {}", result.get("optimality").and_then(Json::as_str).unwrap_or("unknown")));
        if let Some(objective) = result.get("objective").and_then(Json::as_f64) {
            ui.strong(format!("Objective: {objective:.10}"));
        }
        if let Some(gap) = result.get("relative_gap").and_then(Json::as_f64) {
            ui.label(format!("Relative gap: {:.6}%", gap * 100.0));
        }
        let bindings = envelope.get("bindings").and_then(Json::as_array);
        if let Some(values) = result.get("values").and_then(Json::as_array) {
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                egui::Grid::new("solvercraftValues").striped(true).show(ui, |ui| {
                    for (i, value) in values.iter().enumerate() {
                        let binding = bindings.and_then(|v| v.get(i));
                        let sheet = binding.and_then(|b| b.get("sheet")).and_then(Json::as_str).unwrap_or("");
                        let cell = binding.and_then(|b| b.get("cell")).and_then(Json::as_str).unwrap_or("");
                        ui.label(format!("{sheet}!{cell}"));
                        ui.monospace(value.to_string());
                        ui.end_row();
                    }
                });
            });
        }
    }
    if let Some(summaries) = result.get("summaries").and_then(Json::as_array) {
        ui.label(format!("Completed trials: {}", result.get("completed_trials").and_then(Json::as_u64).unwrap_or(0)));
        let outputs = envelope.get("outputs").and_then(Json::as_array);
        egui::Grid::new("solvercraftSummaries").striped(true).show(ui, |ui| {
            for text in ["Output", "Mean", "Std. dev.", "Minimum", "Maximum"] {
                ui.strong(text);
            }
            ui.end_row();
            for (i, summary) in summaries.iter().enumerate() {
                ui.label(outputs.and_then(|a| a.get(i)).and_then(Json::as_str).unwrap_or("Output"));
                let mean = summary.get("mean").and_then(Json::as_f64).unwrap_or(0.0);
                let minimum = summary.get("minimum").and_then(Json::as_f64).unwrap_or(0.0);
                let maximum = summary.get("maximum").and_then(Json::as_f64).unwrap_or(0.0);
                ui.label(format!("{mean:.6}"));
                ui.label(summary.get("variance").and_then(Json::as_f64).map(|v| format!("{:.6}", v.sqrt())).unwrap_or_else(|| "—".into()));
                ui.label(format!("{minimum:.6}"));
                ui.label(format!("{maximum:.6}"));
                ui.end_row();
            }
        });
        if let Some(samples) = result.get("samples").and_then(Json::as_array) {
            let values: Vec<f64> = samples
                .iter()
                .filter_map(|row| row.as_array().and_then(|r| r.first()).and_then(Json::as_f64))
                .filter(|v| v.is_finite())
                .take(100_000)
                .collect();
            solvercraft_histogram(ui, &values);
        }
    }
    if let Some(warnings) = result.get("warnings").and_then(Json::as_array) {
        for warning in warnings.iter().filter_map(Json::as_str) {
            ui.label(warning);
        }
    }
    ui.separator();
    let applicable = result.get("feasibility").and_then(Json::as_str) == Some("feasible")
        && envelope.get("bindings").and_then(Json::as_array).is_some_and(|a| !a.is_empty());
    ui.horizontal(|ui| {
        if applicable && ui.button("Apply solution").clicked() {
            match app.run("solvercraft.result.apply", envelope.clone()) {
                Ok(_) => *open = false,
                Err(e) => app.message = Some(("Solvercraft".into(), crate::clean_error(&e))),
            }
        }
        if ui.button("Close").clicked() {
            *open = false;
        }
    });
}

fn solvercraft_histogram(ui: &mut egui::Ui, values: &[f64]) {
    if values.is_empty() {
        return;
    }
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut counts = [0_u32; 24];
    let scale = low.abs().max(high.abs()).max(1.0);
    let low_scaled = low / scale;
    let span = high / scale - low_scaled;
    for value in values {
        let index = if span > 0.0 { (((value / scale - low_scaled) / span * 24.0).floor() as usize).min(23) } else { 12 };
        if let Some(count) = counts.get_mut(index) {
            *count = count.saturating_add(1);
        }
    }
    ui.label("First output — frequency");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(430.0, 130.0), egui::Sense::hover());
    let max = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let color = crate::theme::Tokens::get(ui.ctx()).accent;
    let width = rect.width() / 24.0;
    for (i, count) in counts.iter().enumerate() {
        let x = rect.left() + i as f32 * width;
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 1.0, rect.bottom() - rect.height() * (*count as f32 / max)),
            egui::pos2(x + width - 1.0, rect.bottom()),
        );
        ui.painter().rect_filled(bar, 0.0, color);
    }
    ui.label(format!("{low:.6} to {high:.6}"));
}
