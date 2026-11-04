//! Exports a retained snapshot as values through the workbook report writer.
use super::{Json, MAX_BINDINGS, Result, SOLVE, Session, Value, bad, json, snapshot};
use crate::cmd::analysis::{Grid, write_report};

fn scalar(value: &Json) -> Value {
    if let Some(text) = value.as_str() {
        Value::text(text)
    } else if let Some(number) = value.as_f64() {
        Value::number(number)
    } else {
        Value::Empty
    }
}

fn simulation(grid: &mut Grid, row: &mut u32, preview: &Json, de: bool) -> Result<()> {
    let result = &preview["result"];
    grid.txt(*row, 0, if de { "Abbruchstatus" } else { "Termination" });
    grid.put(*row, 1, scalar(&result["termination"]));
    grid.put(*row, 2, scalar(&result["completed_trials"]));
    grid.txt(*row, 3, if de { "Versuche" } else { "Trials" });
    *row += 1;
    grid.labels(
        *row,
        0,
        if de {
            &["Ausgabe", "Mittelwert", "Minimum", "Maximum", "Varianz", "Quantil", "Wert"]
        } else {
            &["Output", "Mean", "Minimum", "Maximum", "Variance", "Quantile", "Value"]
        },
    );
    *row += 1;
    if let Some(summaries) = result["summaries"].as_array() {
        for (i, summary) in summaries.iter().enumerate() {
            if *row > MAX_BINDINGS as u32 {
                return Err(bad(SOLVE, "The result sheet exceeds its summary row limit"));
            }
            grid.put(*row, 0, scalar(&preview["outputs"][i]));
            for (col, key) in ["mean", "minimum", "maximum", "variance"].iter().enumerate() {
                grid.put(*row, col as u32 + 1, scalar(&summary[*key]));
            }
            if let Some(quantiles) = summary["quantiles"].as_array().filter(|q| !q.is_empty()) {
                for quantile in quantiles {
                    grid.put(*row, 5, scalar(&quantile[0]));
                    grid.put(*row, 6, scalar(&quantile[1]));
                    *row += 1;
                }
            } else {
                *row += 1;
            }
        }
    }
    *row += 1;
    Ok(())
}

pub(super) fn write(s: &mut Session, params: &Json, preview: &Json) -> Result<Json> {
    let de = params["language"].as_str() == Some("de");
    let revision = snapshot(s)?.revision;
    let stale = preview["snapshot"]["revision"].as_str() != Some(revision.as_str());
    let mut grid = Grid::default();
    grid.column_widths = vec![210.0, 110.0, 100.0, 100.0, 95.0, 75.0, 100.0];
    grid.txt(0, 0, if de { "Solvercraft-Ergebnis" } else { "Solvercraft result" });
    grid.txt(1, 0, if de { "Quellrevision" } else { "Source revision" });
    grid.put(1, 1, scalar(&preview["snapshot"]["revision"]));
    if stale {
        grid.txt(2, 0, if de { "Archivierter Lauf: Arbeitsmappe wurde geändert." } else { "Archived run: workbook has changed." });
    }
    let mut row = 4;
    if let Some(cases) = preview["cases"].as_array() {
        grid.txt(3, 0, if de { "Ausgeführte / angeforderte Fälle" } else { "Run / requested cases" });
        grid.put(3, 1, Value::number(cases.len() as f64));
        grid.put(3, 2, scalar(&preview["requested_cases"]));
        for case in cases {
            grid.put(row, 0, scalar(&case["name"]));
            row += 1;
            simulation(&mut grid, &mut row, &case["preview"], de)?;
        }
    } else if preview["result"].get("completed_trials").is_some() {
        grid.txt(3, 0, if de { "Zufallsstartwert" } else { "Random seed" });
        grid.put(3, 1, scalar(&preview["request"]["options"]["seed"]));
        simulation(&mut grid, &mut row, preview, de)?;
    } else {
        let result = &preview["result"];
        let fields = [
            ("termination", "Termination", "Abbruchstatus"),
            ("feasibility", "Feasibility", "Zulässigkeit"),
            ("optimality", "Optimality", "Optimalität"),
            ("objective", "Objective", "Zielfunktionswert"),
            ("best_bound", "Best bound", "Beste Schranke"),
            ("relative_gap", "Relative gap", "Relative Lücke"),
        ];
        for (key, en, german) in fields {
            grid.txt(row, 0, if de { german } else { en });
            grid.put(row, 1, scalar(&result[key]));
            row += 1;
        }
        row += 1;
        grid.labels(row, 0, if de { &["Blatt", "Zelle", "Vorher", "Vorschlag"] } else { &["Sheet", "Cell", "Before", "Proposed"] });
        if let Some(bindings) = preview["bindings"].as_array() {
            for (i, binding) in bindings.iter().enumerate() {
                row += 1;
                grid.put(row, 0, scalar(&binding["sheet"]));
                grid.put(row, 1, scalar(&binding["cell"]));
                grid.put(row, 2, scalar(&preview["initial_values"][i]));
                grid.put(row, 3, scalar(&preview["apply_values"][i]));
            }
        }
    }
    let wb = &s.doc()?.wb;
    let name = if let Some(name) = params["name"].as_str() {
        name.trim().to_string()
    } else {
        let prefix = if de { "Solver-Ergebnis" } else { "Solver Result" };
        (1..=wb.sheets.len().saturating_add(1))
            .map(|i| format!("{prefix} {i}"))
            .find(|name| wb.sheet_index(name).is_none())
            .ok_or_else(|| bad(SOLVE, "No available result sheet name"))?
    };
    wb.check_sheet_name(&name, None).map_err(|error| bad(SOLVE, error))?;
    write_report(s, &json!({"outputTo":"newSheet","newSheet":name}), "solvercraft", grid)
}
