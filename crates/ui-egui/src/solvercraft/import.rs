//! Read-only review and explicit, revision-bound application of legacy formulas.
use super::{Json, label};
use crate::SheetApp;
use serde_json::json;
use std::sync::Arc;

const ROWS_PER_PAGE: usize = 20;

#[derive(Clone, Default)]
pub(super) struct State {
    report: Option<Arc<Json>>,
    page: usize,
}

fn reason(code: &str, de: bool) -> &str {
    match code {
        "unsupported_nested_psi_or_property" => label(de, "Nested Psi function or property", "Verschachtelte Psi-Funktion oder Eigenschaft"),
        "unsupported_missing_required_argument" => label(de, "Required argument missing", "Pflichtargument fehlt"),
        "unsupported_array_formula" => label(de, "Array formula preserved", "Matrixformel bleibt erhalten"),
        "unsupported_signature" => label(de, "Function or arguments unsupported", "Funktion oder Argumente nicht unterstützt"),
        _ => code,
    }
}

pub(super) fn show(app: &mut SheetApp, ui: &mut egui::Ui, state: &mut super::State, revision: &str, de: bool) {
    ui.collapsing(label(de, "Import Frontline simulation", "Frontline-Simulation importieren"), |ui| {
        ui.label(label(
            de,
            "Scan this workbook for Psi formulas without changing cells.",
            "Arbeitsmappe nach Psi-Formeln durchsuchen. Zellen bleiben bei der Prüfung unverändert.",
        ));
        if ui.button(label(de, "Scan workbook", "Arbeitsmappe prüfen")).clicked() {
            match app.run("solvercraft.worksheet.importPsi", json!({})) {
                Ok(report) => {
                    state.psi_import = State { report: Some(Arc::new(report)), page: 0 };
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
        let Some(report) = state.psi_import.report.clone() else { return };
        let changes = report["changes"].as_array().map_or(&[][..], Vec::as_slice);
        let unsupported = report["unsupported"].as_array().map_or(&[][..], Vec::as_slice);
        let applied = report["applied"].as_bool() == Some(true);
        ui.label(if de {
            format!("{} {} · {} bleiben unverändert", changes.len(), if applied { "übernommen" } else { "konvertierbar" }, unsupported.len())
        } else {
            format!("{} {} · {} preserved", changes.len(), if applied { "converted" } else { "convertible" }, unsupported.len())
        });
        if !changes.is_empty() {
            ui.label(label(
                de,
                "Converted cells use Solvercraft nominal values outside runs. Frontline display settings are lost.",
                "Konvertierte Zellen verwenden außerhalb eines Laufs Solvercraft-Nennwerte. Frontline-Anzeigeeinstellungen gehen dabei verloren.",
            ));
        }
        let count = changes.len() + unsupported.len();
        let pages = count.div_ceil(ROWS_PER_PAGE);
        if pages > 1 {
            ui.horizontal(|ui| {
                let page = &mut state.psi_import.page;
                if ui.add_enabled(*page > 0, egui::Button::new("‹")).clicked() {
                    *page -= 1;
                }
                ui.label(format!("{} / {pages}", *page + 1));
                if ui.add_enabled(*page + 1 < pages, egui::Button::new("›")).clicked() {
                    *page += 1;
                }
            });
        }
        for (row, supported) in unsupported
            .iter()
            .map(|row| (row, false))
            .chain(changes.iter().map(|row| (row, true)))
            .skip(state.psi_import.page * ROWS_PER_PAGE)
            .take(ROWS_PER_PAGE)
        {
            ui.group(|ui| {
                let detail = if supported { label(de, "Convertible", "Konvertierbar") } else { reason(row["reason"].as_str().unwrap_or(""), de) };
                ui.strong(format!("{} · {detail}", row["cell"].as_str().unwrap_or("")));
                ui.monospace(row["source_formula"].as_str().unwrap_or(""));
                if let Some(formula) = row["formula"].as_str() {
                    ui.monospace(formula);
                }
            });
        }
        let stale = report["snapshot"]["revision"].as_str() != Some(revision);
        if stale && !applied {
            ui.label(label(de, "Workbook changed. Scan again before applying.", "Arbeitsmappe geändert. Vor der Übernahme erneut prüfen."));
        }
        if !applied
            && ui
                .add_enabled(
                    !stale && !changes.is_empty(),
                    egui::Button::new(label(de, "Convert supported cells", "Unterstützte Zellen konvertieren")),
                )
                .clicked()
        {
            match app.run("solvercraft.worksheet.importPsi", json!({"apply":true,"expected_snapshot":report["snapshot"]})) {
                Ok(result) => {
                    state.psi_import.report = Some(Arc::new(result));
                    if state.selected.is_none() {
                        state.kind = "simulation".into();
                        state.worksheet_bindings = true;
                    }
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
    });
}
