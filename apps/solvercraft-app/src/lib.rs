//! Solver-focused native shell over the existing command-driven workbook frontend.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

/// Build a local workbook. The original examples are unsaved and never touch user files.
pub fn workbench(blank: bool, services: Services) -> Result<(SheetApp, Option<String>), String> {
    let mut session = Session::new();
    session.new_workbook();
    let selected = if blank { None } else { Some(examples(&mut session)?) };
    let mut app = SheetApp::new(session, services);
    app.disable_updates();
    app.run("ui.solvercraft", json!({"on":true}))?;
    app.ui.ribbon_tab = "Data".into();
    Ok((app, selected))
}

fn execute(session: &mut Session, command: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    session.execute(command, params).map_err(|error| error.to_string())
}

fn examples(session: &mut Session) -> Result<String, String> {
    execute(
        session,
        "range.setValues",
        json!({"range":"A1:F7","values":[
            ["Production planning",null,null,null,"Margin simulation",null],
            ["Decision", "Units",null,null,"Input", "Value"],
            ["Product A",0,null,null,"Sales",100],
            ["Product B",0,null,null,"Cost",20],
            [null,null,null,null,null,null],
            ["Contribution","=3*B3+5*B4",null,null,"Margin","=F3-F4"],
            ["Capacity used","=3*B3+2*B4",null,null,null,null]
        ]}),
    )?;
    execute(session, "home.columnWidth", json!({"cols":"A:A","width":174}))?;
    execute(session, "home.columnWidth", json!({"cols":"E:E","width":164}))?;
    execute(session, "home.columnWidth", json!({"cols":"B:B","width":112}))?;
    execute(session, "home.columnWidth", json!({"cols":"F:F","width":112}))?;
    execute(session, "home.bold", json!({"range":"A1:F2"}))?;
    let production = execute(
        session,
        "solvercraft.model.create",
        json!({
            "name":"Production planning","kind":"optimization","request":{
                "objective":"B6","goal":"max","variables":"B3:B4","nonNegative":true,
                "constraints":["B3 <= 4","B4 <= 6","B7 <= 18"]
            }
        }),
    )?;
    execute(
        session,
        "solvercraft.model.create",
        json!({
            "name":"Margin simulation","kind":"simulation","request":{
                "inputs":[
                    {"cell":"F3","distribution":{"kind":"normal","mean":100,"std_dev":10}},
                    {"cell":"F4","distribution":{"kind":"triangular","low":10,"mode":20,"high":50}}
                ],"outputs":["F6"],"options":{
                    "trials":4096,"seed":"42","method":"monte_carlo","retain_samples":true,"max_memory_bytes":67108864
                }
            }
        }),
    )?;
    execute(session, "selection.set", json!({"range":"B3:B4"}))?;
    production["id"].as_str().map(str::to_owned).ok_or_else(|| "The saved example has no model id".into())
}

/// Select an existing workbook model, or just reveal the pane for a blank workbook.
pub fn select_model(app: &mut SheetApp, ctx: &egui::Context, selected: Option<&str>) -> Result<(), String> {
    let selected = if let Some(id) = selected {
        Some(id.to_string())
    } else {
        app.run("solvercraft.model.list", json!({}))?["models"]
            .as_array()
            .and_then(|models| models.first())
            .and_then(|model| model["id"].as_str())
            .map(str::to_owned)
    };
    if let Some(id) = selected {
        gridcraft_ui_egui::solvercraft::open_model(app, ctx, &id)
    } else {
        app.run("ui.solvercraft", json!({"on":true})).map(|_| ())
    }
}

/// App-level heading, with all workbook actions still sent through SheetApp's commands.
pub fn heading(app: &mut SheetApp, ui: &mut egui::Ui) {
    let tokens = gridcraft_ui_egui::theme::Tokens::get(ui.ctx());
    egui::Panel::top("solvercraft-heading").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Solvercraft");
            ui.colored_label(tokens.text_dim, "Optimization and simulation");
            ui.separator();
            if ui.button("Solver panel").clicked()
                && let Err(error) = app.run("ui.solvercraft", json!({"on":true}))
            {
                app.message = Some(("Solvercraft".into(), error));
            }
        });
    });
}

/// Same original icon supplied for the reviewable launcher catalog submission.
pub fn icon() -> egui::IconData {
    if let Ok(image) = image::load_from_memory(include_bytes!("../../../solvercraft/integrations/artcraft-launcher/solvercraft.png")) {
        let image = image.to_rgba8();
        return egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() };
    }
    procedural_icon()
}

fn procedural_icon() -> egui::IconData {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64i32 {
        for x in 0..64i32 {
            let curve = 46 - ((x - 10) * (x - 10) / 68).min(33);
            let on_curve = (10..=54).contains(&x) && (y - curve).abs() <= 2;
            let node = [(12, 46), (32, 39), (52, 20)].iter().any(|(cx, cy)| (x - cx) * (x - cx) + (y - cy) * (y - cy) <= 20);
            let color = if on_curve || node { [107, 224, 181, 255] } else { [25, 42, 62, 255] };
            rgba.extend_from_slice(&color);
        }
    }
    egui::IconData { rgba, width: 64, height: 64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_example_solves_applies_and_undoes_without_changing_the_other_model() {
        let (mut app, id) = workbench(false, Services::default()).unwrap();
        let model = app.run("solvercraft.model.get", json!({"id":id.unwrap()})).unwrap();
        let result = app.run("solvercraft.solve", model["request"].clone()).unwrap();
        assert_eq!(result["result"]["objective"], 36.0);
        assert_eq!(app.run("sheet.read", json!({"range":"B3:B4"})).unwrap()["values"], json!([[0.0], [0.0]]));
        app.run("solvercraft.result.apply", result).unwrap();
        assert_eq!(app.run("sheet.read", json!({"range":"B3:B4"})).unwrap()["values"], json!([[2.0], [6.0]]));
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(app.run("sheet.read", json!({"range":"B3:B4"})).unwrap()["values"], json!([[0.0], [0.0]]));
        assert_eq!(app.run("solvercraft.model.list", json!({})).unwrap()["models"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn saved_examples_roundtrip_and_simulation_is_real_and_reproducible() {
        let (mut app, _) = workbench(false, Services::default()).unwrap();
        let encoded = app.run("file.saveBytes", json!({"format":"xlsx"})).unwrap();
        let mut restored = Session::new();
        restored.execute("file.open", json!({"name":"examples.xlsx","base64":encoded["base64"]})).unwrap();
        let models = restored.execute("solvercraft.model.list", json!({})).unwrap();
        let simulation = models["models"].as_array().unwrap().iter().find(|m| m["kind"] == "simulation").unwrap();
        let model = restored.execute("solvercraft.model.get", json!({"id":simulation["id"]})).unwrap();
        let before = restored.execute("sheet.read", json!({"range":"F3:F6"})).unwrap();
        let first = restored.execute("solvercraft.simulate", model["request"].clone()).unwrap();
        let second = restored.execute("solvercraft.simulate", model["request"].clone()).unwrap();
        assert_eq!(first["result"], second["result"]);
        assert_ne!(first["result"], serde_json::Value::Null);
        assert_eq!(restored.execute("sheet.read", json!({"range":"F3:F6"})).unwrap(), before);
    }

    #[test]
    fn selecting_an_initial_model_loads_a_visible_valid_editor() {
        let (mut app, id) = workbench(false, Services::default()).unwrap();
        let ctx = egui::Context::default();
        select_model(&mut app, &ctx, id.as_deref()).unwrap();
        assert_eq!(app.grid.pane.as_deref(), Some("solvercraft"));
        assert!(select_model(&mut app, &ctx, Some("missing")).is_err());
    }
}
