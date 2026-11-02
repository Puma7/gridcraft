//! Render the real Solvercraft shell offscreen; not a screenshot of vendor software.
use gridcraft_ui_egui::{Services, SheetApp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("Usage: cargo run -p solvercraft-app --example snapshot -- output.png")?;
    let _ = gridcraft_ui_egui::fonts::catalog();
    let (mut app, selected) = solvercraft_app::workbench(false, Services::default())?;
    app.ui.dark = true;
    let mut prepared = false;
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_pixels_per_point(1.0).with_max_steps(12).wgpu().build_ui_state(
            move |ui, app: &mut SheetApp| {
                let ctx = ui.ctx().clone();
                if !prepared {
                    SheetApp::setup_context(&ctx, true);
                    if let Err(error) = solvercraft_app::select_model(app, &ctx, selected.as_deref()) {
                        app.message = Some(("Solvercraft".into(), error));
                    }
                    prepared = true;
                }
                app.logic(&ctx);
                solvercraft_app::heading(app, ui);
                app.ui(ui);
            },
            app,
        );
    harness.run_steps(16);
    harness.render()?.save(&output)?;
    println!("Rendered {output}");
    Ok(())
}
