use super::{Json, SheetApp, json, label};
pub(super) fn controls(app: &mut SheetApp, ui: &mut egui::Ui, preview: &Json, enabled: bool, de: bool) -> Result<Option<Json>, String> {
    let mut result = Ok(None);
    ui.horizontal_wrapped(|ui| {
        for (command, title) in [
            ("solvercraft.report.answer", label(de, "Answer report", "Antwortbericht")),
            ("solvercraft.report.lpSensitivity", label(de, "LP sensitivity / shadow prices", "LP-Sensitivität / Schattenpreise")),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(title)).clicked() {
                result = app.run(command, preview.clone()).map(Some);
            }
        }
    });
    result
}
pub(super) fn show(ui: &mut egui::Ui, report: &Json) {
    if report["analysis"] == json!("lp_basis_intervals") {
        ui.strong("Checked LP basis: one coefficient/RHS at a time");
        let sensitivity = &report["detail"]["sensitivity"];
        if let (Some(prices), Some(names)) = (sensitivity["rhs_shadow_prices"].as_array(), sensitivity["row_descriptions"].as_array()) {
            for (name, price) in names.iter().zip(prices) {
                ui.label(format!("{}: shadow price {price}", name.as_str().unwrap_or("row")));
            }
        }
        for field in ["rhs_changes", "objective_changes"] {
            if let Some(changes) = sensitivity[field].as_array() {
                ui.label(field);
                for item in changes {
                    ui.monospace(format!(
                        "{}: Δ [{}, {}]",
                        item["item"],
                        item.get("lower").filter(|v| !v.is_null()).map_or_else(|| "-∞".into(), Json::to_string),
                        item.get("upper").filter(|v| !v.is_null()).map_or_else(|| "+∞".into(), Json::to_string)
                    ));
                }
            }
        }
        if sensitivity.is_null() {
            ui.label("No checked basis/sensitivity is available for this solution.");
        }
    } else {
        ui.strong("Answer report");
        ui.label(format!("Validated objective: {}", report["objective"]["validated_value"]));
        if let Some(rows) = report["constraints"].as_array() {
            for row in rows {
                ui.label(format!(
                    "{}: left {} · right {} · slack {}",
                    row["name"].as_str().unwrap_or("row"),
                    row["left_value"],
                    row["right_value"],
                    row["slack"]
                ));
            }
        }
    }
    if ui.button("Copy report JSON").clicked() {
        ui.ctx().copy_text(report.to_string());
    }
}
