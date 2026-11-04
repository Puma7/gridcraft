//! Human-readable parameter controls over the SDK's distribution JSON.
use super::{Json, SheetApp, help, json, label};
use crate::theme::Tokens;

struct Parameter {
    key: &'static str,
    en: &'static str,
    de: &'static str,
    initial: f64,
}
struct Family {
    id: &'static str,
    en: &'static str,
    de: &'static str,
    parameters: &'static [Parameter],
}
macro_rules! family {
    ($id:literal, $en:literal, $de:literal, $(($key:literal, $pe:literal, $pd:literal, $v:expr)),* $(,)?) => {
        Family { id: $id, en: $en, de: $de, parameters: &[$(Parameter { key: $key, en: $pe, de: $pd, initial: $v }),*] }
    };
}
const FAMILIES: &[Family] = &[
    family!("uniform", "Uniform", "Gleichverteilung", ("low", "Minimum", "Minimum", 0.0), ("high", "Maximum", "Maximum", 1.0)),
    family!(
        "normal",
        "Normal",
        "Normalverteilung",
        ("mean", "Mean", "Mittelwert", 0.0),
        ("std_dev", "Standard deviation", "Standardabweichung", 1.0)
    ),
    family!(
        "triangular",
        "Triangular",
        "Dreiecksverteilung",
        ("low", "Minimum", "Minimum", 0.0),
        ("mode", "Most likely value", "Wahrscheinlichster Wert", 0.5),
        ("high", "Maximum", "Maximum", 1.0)
    ),
    family!(
        "beta_pert",
        "PERT",
        "PERT-Verteilung",
        ("low", "Minimum", "Minimum", 0.0),
        ("mode", "Most likely value", "Wahrscheinlichster Wert", 0.5),
        ("high", "Maximum", "Maximum", 1.0),
        ("weight", "Weight", "Gewichtung", 4.0)
    ),
    family!(
        "lognormal",
        "Lognormal",
        "Lognormalverteilung",
        ("log_mean", "Mean of logarithm", "Mittelwert des Logarithmus", 0.0),
        ("log_std_dev", "Standard deviation of logarithm", "Standardabweichung des Logarithmus", 1.0)
    ),
    family!(
        "discrete_uniform",
        "Uniform whole numbers",
        "Gleichverteilte ganze Zahlen",
        ("low", "Minimum", "Minimum", 0.0),
        ("high", "Maximum", "Maximum", 10.0)
    ),
    family!("bernoulli", "Yes / no (Bernoulli)", "Ja / Nein (Bernoulli)", ("probability", "Probability of 1", "Wahrscheinlichkeit für 1", 0.5)),
    family!(
        "binomial",
        "Success count (Binomial)",
        "Anzahl Erfolge (Binomial)",
        ("trials", "Number of attempts", "Anzahl Versuche", 10.0),
        ("probability", "Success probability", "Erfolgswahrscheinlichkeit", 0.5)
    ),
    family!("poisson", "Event count (Poisson)", "Anzahl Ereignisse (Poisson)", ("rate", "Expected event count", "Erwartete Ereigniszahl", 5.0)),
    family!("exponential", "Exponential", "Exponentialverteilung", ("rate", "Rate (1 / mean)", "Rate (1 / Mittelwert)", 1.0)),
    family!("weibull", "Weibull", "Weibull-Verteilung", ("shape", "Shape", "Form", 2.0), ("scale", "Scale", "Skalierung", 1.0)),
    family!("beta", "Beta", "Beta-Verteilung", ("alpha", "Alpha shape", "Form Alpha", 2.0), ("beta", "Beta shape", "Form Beta", 2.0)),
    family!("gamma", "Gamma", "Gamma-Verteilung", ("shape", "Shape", "Form", 2.0), ("scale", "Scale", "Skalierung", 1.0)),
    family!(
        "negative_binomial",
        "Failures before successes",
        "Fehlversuche vor Erfolgen",
        ("successes", "Required successes", "Benötigte Erfolge", 5.0),
        ("probability", "Success probability", "Erfolgswahrscheinlichkeit", 0.5)
    ),
    family!(
        "geometric",
        "Failures before first success",
        "Fehlversuche vor erstem Erfolg",
        ("probability", "Success probability", "Erfolgswahrscheinlichkeit", 0.5)
    ),
    family!("empirical", "Observed values (discrete)", "Beobachtete Werte (diskret)",),
    family!("empirical_continuous", "Observed values (interpolated)", "Beobachtete Werte (interpoliert)",),
    family!("constant", "Fixed value", "Fester Wert", ("value", "Value", "Wert", 0.0)),
    family!(
        "student_t",
        "Student t",
        "Student-t-Verteilung",
        ("degrees_of_freedom", "Degrees of freedom", "Freiheitsgrade", 5.0),
        ("location", "Location", "Lage", 0.0),
        ("scale", "Scale", "Skalierung", 1.0)
    ),
    family!("chi_squared", "Chi-squared", "Chi-Quadrat-Verteilung", ("degrees_of_freedom", "Degrees of freedom", "Freiheitsgrade", 5.0)),
    family!("logistic", "Logistic", "Logistische Verteilung", ("location", "Location", "Lage", 0.0), ("scale", "Scale", "Skalierung", 1.0)),
    family!("laplace", "Laplace", "Laplace-Verteilung", ("location", "Location", "Lage", 0.0), ("scale", "Scale", "Skalierung", 1.0)),
    family!("gumbel", "Gumbel", "Gumbel-Verteilung", ("location", "Location", "Lage", 0.0), ("scale", "Scale", "Skalierung", 1.0)),
    family!("pareto", "Pareto", "Pareto-Verteilung", ("minimum", "Minimum", "Minimum", 1.0), ("shape", "Shape", "Form", 2.0)),
    family!("rayleigh", "Rayleigh", "Rayleigh-Verteilung", ("scale", "Scale", "Skalierung", 1.0)),
];

fn initial(family: &Family) -> Json {
    let mut value = json!({"kind": family.id});
    for parameter in family.parameters {
        value[parameter.key] = if whole(family.id, parameter.key) { json!(parameter.initial as i64) } else { json!(parameter.initial) };
    }
    if family.id.starts_with("empirical") {
        value["values"] = json!([0.0, 1.0]);
    }
    value
}
fn whole(family: &str, key: &str) -> bool {
    family == "discrete_uniform" || matches!(key, "trials" | "successes")
}

pub(super) fn controls(ui: &mut egui::Ui, value: &mut Json, index: usize, de: bool, editor: &mut super::builder::Editor) {
    let mut kind = value["kind"].as_str().unwrap_or("").to_owned();
    let current = FAMILIES.iter().find(|family| family.id == kind);
    let caption = current.map_or(kind.as_str(), |family| label(de, family.en, family.de)).to_owned();
    craft_ui::ComboBox::from_id_salt(("distribution", index))
        .selected_text(caption)
        .show_ui(ui, |ui| {
            for family in FAMILIES {
                ui.selectable_value(&mut kind, family.id.into(), label(de, family.en, family.de));
            }
        })
        .response
        .accessible_name(format!("{} {}", label(de, "Distribution", "Verteilung"), index + 1));
    if kind != value["kind"].as_str().unwrap_or("")
        && let Some(family) = FAMILIES.iter().find(|family| family.id == kind)
    {
        *value = initial(family);
        editor.observations.remove(&index);
    }
    if let Some(family) = FAMILIES.iter().find(|family| family.id == kind) {
        for parameter in family.parameters {
            parameter_control(ui, value, parameter, family.id, index, de);
        }
        if family.id.starts_with("empirical") {
            let text = editor.observations.entry(index).or_insert_with(|| {
                value["values"].as_array().map(|values| values.iter().map(Json::to_string).collect::<Vec<_>>().join("\n")).unwrap_or_default()
            });
            ui.label(label(de, "Observed values, one per line", "Beobachtete Werte, ein Wert pro Zeile"));
            let changed = ui
                .add(egui::TextEdit::multiline(text).desired_rows(3).char_limit(131_072).desired_width(f32::INFINITY))
                .accessible_name(format!("Observed values {}", index + 1))
                .on_hover_text(label(
                    de,
                    "Use a decimal point, for example 12.5. All values have equal weight.",
                    "Verwende den Dezimalpunkt, zum Beispiel 12.5. Alle Werte sind gleich gewichtet.",
                ))
                .changed();
            let parsed = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|part| part.trim().parse::<f64>().ok().filter(|n| n.is_finite()))
                .collect::<Option<Vec<_>>>();
            match parsed.filter(|values| !values.is_empty() && values.len() <= 100_000) {
                Some(values) => {
                    if changed {
                        value["values"] = json!(values);
                    }
                }
                None => editor.issues.push(
                    label(
                        de,
                        "Enter finite observed values, one per line with a decimal point.",
                        "Endliche Beobachtungswerte zeilenweise mit Dezimalpunkt eingeben.",
                    )
                    .into(),
                ),
            }
        }
    } else {
        ui.label(label(
            de,
            "This distribution uses the expert configuration below.",
            "Diese Verteilung wird über die Expertenkonfiguration unten bearbeitet.",
        ));
    }
}

fn parameter_control(ui: &mut egui::Ui, distribution: &mut Json, parameter: &Parameter, kind: &str, index: usize, de: bool) {
    let caption = label(de, parameter.en, parameter.de);
    ui.horizontal_wrapped(|ui| {
        ui.label(caption);
        let response = if whole(kind, parameter.key) {
            let mut value = distribution[parameter.key].as_i64().unwrap_or(parameter.initial as i64);
            let response = ui.add(egui::DragValue::new(&mut value));
            if response.changed() {
                distribution[parameter.key] = json!(value);
            }
            response
        } else {
            let percent = parameter.key == "probability";
            let mut value = distribution[parameter.key].as_f64().unwrap_or(parameter.initial) * if percent { 100.0 } else { 1.0 };
            let widget = egui::DragValue::new(&mut value).speed(if percent { 1.0 } else { 0.1 });
            let response = ui.add(if percent { widget.suffix(" %").range(0.0..=100.0) } else { widget });
            if response.changed() {
                distribution[parameter.key] = json!(value / if percent { 100.0 } else { 1.0 });
            }
            response
        };
        response.accessible_name(format!("{} {} · {caption}", label(de, "Input", "Eingabe"), index + 1));
    });
}

pub(super) fn preview(app: &mut SheetApp, ui: &mut egui::Ui, value: &Json, index: usize, de: bool, editor: &mut super::builder::Editor) {
    if editor.previews.get(&index).is_none_or(|(source, _)| source != value) {
        let result = app.run("solvercraft.distribution.preview", json!({"distribution": value}));
        editor.previews.insert(index, (value.clone(), result));
    }
    let Some((_, result)) = editor.previews.get(&index) else { return };
    match result {
        Ok(result) => {
            let points = result["points"].as_array().map_or(&[][..], Vec::as_slice);
            if points.is_empty() {
                ui.small(label(de, "Preview unavailable for this distribution.", "Für diese Verteilung ist keine Vorschau verfügbar."));
            } else {
                draw(ui, points, de);
            }
        }
        Err(error) => {
            ui.colored_label(Tokens::get(ui.ctx()).danger, error);
            editor.issues.push(format!("{} {}", label(de, "Check distribution", "Verteilung prüfen"), index + 1));
        }
    }
}

fn draw(ui: &mut egui::Ui, points: &[Json], de: bool) {
    let low = points.first().and_then(|point| point[0].as_f64()).unwrap_or(0.0);
    let high = points.last().and_then(|point| point[0].as_f64()).unwrap_or(low);
    ui.horizontal_wrapped(|ui| {
        ui.small(label(de, "Distribution preview", "Verteilungsvorschau"));
        help(ui, label(de, "About the distribution preview", "Zur Verteilungsvorschau"), label(de,
            "The curve shows the probability of a value at or below the horizontal position. The displayed range covers the 1st to 99th percentiles.",
            "Die Kurve zeigt die Wahrscheinlichkeit für einen Wert bis zur waagerechten Position. Der angezeigte Bereich reicht vom 1. bis zum 99. Perzentil."));
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 62.0), egui::Sense::hover());
    let t = Tokens::get(ui.ctx());
    let path = points
        .iter()
        .filter_map(|point| {
            Some(egui::pos2(
                rect.left() + super::unit_position(point[0].as_f64()?, low, high) as f32 * rect.width(),
                rect.bottom() - point[1].as_f64()? as f32 * rect.height(),
            ))
        })
        .collect::<Vec<_>>();
    ui.painter().rect_filled(rect, 3.0, t.header_bg);
    ui.painter().add(egui::Shape::line(path, egui::Stroke::new(2.0, t.accent)));
    ui.small(format!("1%  {}     ·     99%  {}", gridcraft_engine::core::number_to_text(low), gridcraft_engine::core::number_to_text(high)));
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    #[test]
    fn every_offered_family_has_a_valid_sdk_preview() {
        let mut session = gridcraft_engine::Session::new();
        for family in super::FAMILIES {
            let value = super::initial(family);
            let result = session.execute("solvercraft.distribution.preview", super::json!({"distribution": value}));
            assert!(result.is_ok(), "{}: {result:?}", family.id);
            assert_eq!(result.unwrap()["points"].as_array().unwrap().len(), 33);
        }
    }
}
