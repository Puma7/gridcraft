//! Guided execution of the built-in example through the ordinary job commands.
use super::{Json, SheetApp, json, label};
use crate::theme::Tokens;
use std::sync::Arc;

const MODELS: [&str; 3] = ["1 · Optimize cost plan", "2 · Optimize reserve plan", "3 · Simulate both plans"];

#[derive(Clone, Default)]
pub(super) struct Guide {
    busy: bool,
    job: Option<String>,
    models: Vec<String>,
    results: Vec<Arc<Json>>,
    revision: u64,
    error: Option<String>,
    report: Option<String>,
    last_poll: f64,
}

pub(super) fn matches(app: &SheetApp) -> bool {
    app.session.active().is_some_and(|doc| {
        ["Dashboard", "Assumptions", "CostPlan", "ReservePlan", "StressTest", "QuickStart"].iter().all(|name| doc.wb.sheet_index(name).is_some())
    })
}

pub(super) fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    ui.add_sized([ui.available_width(), 34.0], egui::Button::new(egui::RichText::new(text).color(t.on_accent).strong()).fill(t.accent))
}

impl Guide {
    pub(super) fn busy(&self) -> bool {
        self.busy
    }

    fn revision(app: &SheetApp) -> u64 {
        app.session.active().map_or(0, |doc| doc.revision)
    }

    fn start(&mut self, app: &mut SheetApp) -> Result<(), String> {
        self.release(app)?;
        self.results.clear();
        self.report = None;
        self.error = None;
        self.models.clear();
        let listed = app.run("solvercraft.model.list", json!({}))?;
        for name in MODELS {
            let id = listed["models"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["name"] == name))
                .and_then(|model| model["id"].as_str())
                .ok_or_else(|| format!("Missing example model: {name}"))?;
            self.models.push(id.into());
        }
        self.revision = Self::revision(app);
        self.busy = true;
        self.start_step(app)
    }

    fn start_step(&mut self, app: &mut SheetApp) -> Result<(), String> {
        let model = self.models.get(self.results.len()).ok_or("The example has no next model")?;
        let job = app.run("solvercraft.job.start", json!({"model_id": model, "limits": {"max_time_millis": 60000}}))?;
        self.job = Some(job["id"].as_str().ok_or("The job did not return an identifier")?.into());
        self.last_poll = f64::NEG_INFINITY;
        Ok(())
    }

    fn release(&mut self, app: &mut SheetApp) -> Result<(), String> {
        if let Some(id) = &self.job {
            let status = app.run("solvercraft.job.get", json!({"id": id}))?;
            if matches!(status["status"].as_str(), Some("queued" | "running")) {
                return Err("The previous run is stopping. Try again when it has finished.".into());
            }
            app.run("solvercraft.job.forget", json!({"id": id}))?;
        }
        self.job = None;
        Ok(())
    }

    fn stop(&mut self, app: &mut SheetApp) -> Result<(), String> {
        self.busy = false;
        if let Some(id) = &self.job {
            app.run("solvercraft.job.cancel", json!({"id": id}))?;
        }
        Ok(())
    }

    fn tick(&mut self, app: &mut SheetApp, now: f64) -> Result<(), String> {
        if !self.busy || now - self.last_poll < 0.15 {
            return Ok(());
        }
        self.last_poll = now;
        if Self::revision(app) != self.revision {
            self.stop(app)?;
            return Err("Workbook changed during the comparison. Run again to use the new inputs.".into());
        }
        let id = self.job.as_deref().ok_or("The comparison has no active job")?;
        let status = app.run("solvercraft.job.get", json!({"id": id}))?;
        match status["status"].as_str() {
            Some("queued" | "running") => return Ok(()),
            Some("completed") => {}
            _ => return Err(format!("The comparison stopped: {}", status["error"])),
        }
        let result = app.run("solvercraft.job.result", json!({"id": id}))?;
        if self.results.len() < 2 {
            if result["result"]["optimality"] != "optimal" || result["result"]["feasibility"] != "feasible" {
                return Err("An optimal feasible plan could not be found. Check the model settings and inputs.".into());
            }
            app.run("solvercraft.job.apply", json!({"id": id}))?;
            self.results.push(Arc::new(result));
            self.revision = Self::revision(app);
            self.release(app)?;
            self.start_step(app)
        } else {
            if result["result"]["termination"] != "completed" || result["result"]["completed_trials"] != result["request"]["options"]["trials"] {
                return Err("The simulation stopped before all trials completed. Run the comparison again.".into());
            }
            let values = dashboard_values(&result)?;
            app.run("range.setValues", json!({"sheet": "Dashboard", "range": "B13", "values": values}))?;
            self.results.push(Arc::new(result));
            self.revision = Self::revision(app);
            self.busy = false;
            Ok(())
        }
    }

    fn show_steps(&self, ui: &mut egui::Ui, de: bool) {
        let t = Tokens::get(ui.ctx());
        let steps = [
            ("Cost plan", "Kostenplan", "Minimize forecast cost", "Prognosekosten minimieren"),
            ("Reserve plan", "Reserveplan", "Book backup and keep battery reserve", "Notstrom buchen und Batterie laden"),
            ("Risk comparison", "Risikovergleich", "Test both plans on the same random days", "Beide Pläne an denselben Zufallstagen testen"),
        ];
        for (i, (en, german, detail_en, detail_de)) in steps.iter().enumerate() {
            let done = i < self.results.len();
            let current = self.busy && i == self.results.len();
            egui::Frame::new().fill(if current { t.accent_soft } else { t.window }).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let marker = if done { "✓".into() } else { (i + 1).to_string() };
                    ui.label(egui::RichText::new(marker).strong().color(if done || current { t.accent_dark } else { t.text_dim }));
                    ui.strong(label(de, en, german));
                    if current {
                        ui.spinner();
                    }
                });
                ui.label(egui::RichText::new(label(de, detail_en, detail_de)).small().color(t.text_dim));
                if let Some(result) = self.results.get(i).filter(|_| i < 2)
                    && let Some(cost) = result["result"]["objective"].as_f64()
                {
                    ui.small(format!("{cost:.2} € · {}", label(de, "applied to sheet", "ins Blatt übernommen")));
                }
            });
        }
    }

    fn show_results(&self, ui: &mut egui::Ui, de: bool) -> Result<(), String> {
        let Some(simulation) = self.results.get(2) else { return Ok(()) };
        ui.small(format!("{} {}", simulation["result"]["completed_trials"], label(de, "paired trials", "gemeinsame Versuche")));
        let gaps = [mean(simulation, "StressTest!H25")?, mean(simulation, "StressTest!H45")?];
        let unserved = [mean(simulation, "StressTest!D25")?, mean(simulation, "StressTest!D45")?];
        let t = Tokens::get(ui.ctx());
        ui.label(label(de, "Days with an energy shortfall", "Tage mit Versorgungslücke"));
        for (i, gap) in gaps.iter().enumerate() {
            let plan = if i == 0 { label(de, "Cost plan", "Kostenplan") } else { label(de, "Reserve plan", "Reserveplan") };
            ui.horizontal(|ui| {
                ui.label(plan);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.strong(format!("{:.1}%", gap * 100.0));
                });
            });
            ui.add(egui::ProgressBar::new(*gap as f32).fill(t.cat[i]).desired_height(8.0));
        }
        if unserved[0] > 0.0 {
            ui.label(
                egui::RichText::new(format!(
                    "{:.0}% {}",
                    (1.0 - unserved[1] / unserved[0]) * 100.0,
                    label(de, "less unserved energy", "weniger fehlende Energie")
                ))
                .strong(),
            );
        }
        if let (Some(cost), Some(reserve)) = (self.results[0]["result"]["objective"].as_f64(), self.results[1]["result"]["objective"].as_f64()) {
            ui.small(format!("{:.2} € {}", reserve - cost, label(de, "extra forecast cost for reserve", "Mehrkosten für Reserve laut Prognose")));
        }
        Ok(())
    }
}

fn summary<'a>(preview: &'a Json, reference: &str) -> Result<&'a Json, String> {
    let index = preview["outputs"]
        .as_array()
        .and_then(|rows| rows.iter().position(|row| row.as_str() == Some(reference)))
        .ok_or_else(|| format!("The simulation did not return {reference}"))?;
    preview["result"]["summaries"].get(index).ok_or_else(|| format!("Missing summary for {reference}"))
}

fn mean(preview: &Json, reference: &str) -> Result<f64, String> {
    summary(preview, reference)?["mean"].as_f64().filter(|n| n.is_finite()).ok_or_else(|| format!("Invalid mean for {reference}"))
}

fn dashboard_values(preview: &Json) -> Result<Json, String> {
    let mut rows = Vec::new();
    for (plan, end) in [("A · Cost", 25), ("B · Reserve", 45)] {
        let total = format!("StressTest!F{end}");
        let p95 = summary(preview, &total)?["quantiles"]
            .as_array()
            .and_then(|rows| rows.iter().find(|pair| pair[0].as_f64() == Some(0.95)))
            .and_then(|pair| pair[1].as_f64())
            .filter(|n| n.is_finite())
            .ok_or("Missing P95 risk")?;
        rows.push(json!([
            plan,
            mean(preview, &format!("StressTest!H{end}"))?,
            mean(preview, &format!("StressTest!D{end}"))?,
            mean(preview, &format!("StressTest!B{end}"))?,
            mean(preview, &total)?,
            p95
        ]));
    }
    Ok(json!(rows))
}

pub(super) fn show(app: &mut SheetApp, ui: &mut egui::Ui, guide: &mut Guide, de: bool) {
    if let Err(error) = guide.tick(app, ui.input(|i| i.time)) {
        guide.busy = false;
        guide.error = Some(error);
    }
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(label(de, "COLD STORAGE MICROGRID", "KÜHLCAMPUS")).small().strong().color(t.accent_dark));
    ui.heading(label(de, "What does resilience cost?", "Was kostet Versorgungssicherheit?"));
    ui.label(label(
        de,
        "Compare a low-cost plan with a plan that keeps backup power ready.",
        "Vergleiche einen günstigen Fahrplan mit einem Plan, der Energie für Ausfälle vorhält.",
    ));
    ui.add_space(6.0);
    guide.show_steps(ui, de);
    ui.add_space(6.0);
    if guide.busy {
        ui.add(egui::ProgressBar::new(guide.results.len() as f32 / 3.0).text(label(de, "Comparison running", "Vergleich läuft")).animate(true));
        if ui.button(label(de, "Stop comparison", "Vergleich stoppen")).clicked() {
            guide.error = Some(match guide.stop(app) {
                Ok(()) => label(
                    de,
                    "Comparison stopped. Completed plans remain in the workbook.",
                    "Vergleich gestoppt. Bereits übernommene Pläne stehen in der Mappe.",
                )
                .into(),
                Err(error) => error,
            });
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    } else {
        if primary(ui, label(de, "Run full comparison", "Gesamten Vergleich starten")).clicked()
            && let Err(error) = guide.start(app)
        {
            guide.busy = false;
            guide.error = Some(error);
        }
        super::help(
            ui,
            label(de, "How the comparison works", "So funktioniert der Vergleich"),
            label(
                de,
                "Runs and applies both optimized plans, then simulates their risks and updates the Dashboard. Each plan can be undone. Model settings are below.",
                "Berechnet und übernimmt beide optimalen Fahrpläne, simuliert ihre Risiken und aktualisiert den Leitstand. Die Fahrpläne lassen sich rückgängig machen. Die Modelleinstellungen stehen unten.",
            ),
        );
    }
    if let Some(error) = &guide.error {
        ui.colored_label(t.danger, error);
    }
    if guide.results.len() == 3 {
        if Guide::revision(app) != guide.revision {
            ui.colored_label(
                t.warning_ink,
                label(
                    de,
                    "Workbook changed. Run again to refresh the comparison.",
                    "Arbeitsmappe geändert. Vergleich erneut starten, um ihn zu aktualisieren.",
                ),
            );
        }
        ui.strong(label(de, "Results from this run", "Ergebnis dieses Laufs"));
        if ui.button(label(de, "Open result sheet", "Ergebnisblatt öffnen")).clicked() {
            let current = Guide::revision(app) == guide.revision;
            let existing = guide.report.as_deref().filter(|sheet| app.session.active().is_some_and(|doc| doc.wb.sheet_index(sheet).is_some()));
            let result = if let Some(sheet) = existing {
                app.run("sheet.activate", json!({"sheet": sheet}))
            } else {
                app.run("solvercraft.job.report", json!({"id": guide.job, "language": if de {"de"} else {"en"}}))
            };
            match result {
                Ok(report) => {
                    if let Some(sheet) = report["sheet"].as_str() {
                        guide.report = Some(sheet.into());
                    }
                    if current {
                        guide.revision = Guide::revision(app);
                    }
                }
                Err(error) => guide.error = Some(error),
            }
        }
        if let Err(error) = guide.show_results(ui, de) {
            ui.colored_label(t.danger, error);
        }
    }
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        for (en, german, sheet) in [
            ("Dashboard", "Leitstand", "Dashboard"),
            ("Change inputs", "Eingaben ändern", "Assumptions"),
            ("Try an outage", "Ausfall ausprobieren", "StressTest"),
        ] {
            if ui.button(label(de, en, german)).clicked()
                && let Err(error) = app.run("sheet.activate", json!({"sheet": sheet}))
            {
                guide.error = Some(error);
            }
        }
    });
    ui.separator();
}
