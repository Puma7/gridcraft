//! Statistics from one completed worksheet simulation.
use std::collections::HashMap;
use std::sync::Arc;

use ::solvercraft::types::{SimulationResult, Termination};

use super::{
    CellBinding, FormulaExpr, Json, MAX_BINDINGS, Result, SOLVE, Session, Snapshot, Workbook, bad, binding_name, cells, edit, json, snapshot,
};

#[derive(Clone, Copy)]
enum Kind {
    Mean,
    Min,
    Max,
    Range,
    Variance,
    StdDev,
    Target(f64),
}

fn kind(name: &str, args: &[FormulaExpr]) -> Option<Kind> {
    let name = super::worksheet::psi_name(name).to_ascii_uppercase();
    match args {
        [FormulaExpr::Ref(_)] => match name.as_str() {
            "PSIMEAN" => Some(Kind::Mean),
            "PSIMIN" => Some(Kind::Min),
            "PSIMAX" => Some(Kind::Max),
            "PSIRANGE" => Some(Kind::Range),
            "PSIVARIANCE" => Some(Kind::Variance),
            "PSISTDDEV" => Some(Kind::StdDev),
            _ => None,
        },
        [FormulaExpr::Ref(_), threshold] if name == "PSITARGET" => {
            let value = match threshold {
                FormulaExpr::Number(n) => *n,
                FormulaExpr::Unary(op, expr) => match (op, expr.as_ref()) {
                    (super::UnOp::Neg, FormulaExpr::Number(n)) => -*n,
                    (super::UnOp::Plus, FormulaExpr::Number(n)) => *n,
                    _ => return None,
                },
                _ => return None,
            };
            value.is_finite().then_some(Kind::Target(value))
        }
        _ => None,
    }
}

pub(super) fn source(wb: &Workbook, cell: CellBinding, name: &str, args: &[FormulaExpr]) -> Option<CellBinding> {
    kind(name, args)?;
    let FormulaExpr::Ref(reference) = args.first()? else { return None };
    let range = reference.range();
    if !range.is_single() {
        return None;
    }
    let binding = (super::reference_sheet(wb, cell.0, reference).ok()?, range.start);
    (binding != cell).then_some(binding)
}

struct Item {
    cell: CellBinding,
    output: usize,
    kind: Kind,
    count: u64,
}

pub(super) struct Statistics {
    items: Vec<Item>,
    targets: Vec<usize>,
}

impl Statistics {
    pub(super) fn collect(wb: &Workbook, outputs: &[CellBinding], trials: u64) -> Result<Self> {
        let mut items = Vec::new();
        // Keep the same workbook scan bound as declaration discovery. Explicit
        // runs on larger books still work without worksheet-stat previews.
        if wb.sheets.iter().map(|s| s.cells.len()).fold(0usize, usize::saturating_add) > super::worksheet::SCAN_CELLS {
            return Ok(Self { items, targets: Vec::new() });
        }
        let indexes: HashMap<_, _> = outputs.iter().enumerate().map(|(index, &cell)| (cell, index)).collect();
        for (sheet, sh) in wb.sheets.iter().enumerate() {
            for (cell, value) in sh.cells.iter() {
                if value.formula.as_ref().is_none_or(|f| f.array.is_some_and(|a| !a.is_single())) || sh.spill_anchor(cell).is_some() {
                    continue;
                }
                let Some((name, args)) = super::worksheet::top_call(wb, sheet, cell) else { continue };
                let Some(source) = source(wb, (sheet, cell), &name, &args) else { continue };
                let (Some(&output), Some(kind)) = (indexes.get(&source), kind(&name, &args)) else { continue };
                if items.len() == MAX_BINDINGS {
                    return Err(bad(SOLVE, "Too many worksheet statistics"));
                }
                items.push(Item { cell: (sheet, cell), output, kind, count: 0 });
            }
        }
        let targets: Vec<_> = items.iter().enumerate().filter_map(|(index, item)| matches!(item.kind, Kind::Target(_)).then_some(index)).collect();
        if (targets.len() as u64).saturating_mul(trials) > 100_000_000 {
            return Err(bad(SOLVE, "Worksheet statistics exceed the threshold comparison budget"));
        }
        Ok(Self { items, targets })
    }

    pub(super) fn observe(&mut self, outputs: &[f64]) {
        for &index in &self.targets {
            if let Some(item) = self.items.get_mut(index)
                && let Kind::Target(threshold) = item.kind
                && outputs.get(item.output).is_some_and(|value| *value <= threshold)
            {
                item.count += 1;
            }
        }
    }

    pub(super) fn finish(self, wb: &Workbook, names: &[String], result: &SimulationResult, trials: u64) -> Result<Vec<Json>> {
        if result.termination != Termination::Completed || result.completed_trials != trials || trials == 0 {
            return Ok(Vec::new());
        }
        self.items
            .into_iter()
            .filter_map(|item| {
                let summary = result.summaries.get(item.output).filter(|summary| summary.count == trials);
                let Some(summary) = summary else {
                    return Some(Err(bad(SOLVE, "Worksheet statistics require a complete output summary")));
                };
                let (function, value) = match item.kind {
                    Kind::Mean => ("PsiMean", Some(summary.mean)),
                    Kind::Min => ("PsiMin", Some(summary.minimum)),
                    Kind::Max => ("PsiMax", Some(summary.maximum)),
                    Kind::Range => ("PsiRange", Some(summary.maximum - summary.minimum)),
                    Kind::Variance => ("PsiVariance", summary.variance),
                    Kind::StdDev => ("PsiStdDev", summary.variance.map(f64::sqrt)),
                    Kind::Target(_) => ("PsiTarget", Some(item.count as f64 / trials as f64)),
                };
                let value = value.filter(|value| value.is_finite())?;
                Some(
                    binding_name(wb, item.cell.0, item.cell.1)
                        .map(|cell| json!({"cell":cell,"output":names.get(item.output),"function":function,"value":value})),
                )
            })
            .collect()
    }
}

pub(super) fn apply(s: &mut Session, preview: &Json) -> Result<Json> {
    let expected: Snapshot = serde_json::from_value(preview["snapshot"].clone()).map_err(super::sdk_error)?;
    let current = snapshot(s)?;
    if expected.document_uid != current.document_uid || expected.revision != current.revision {
        return Err(bad(SOLVE, "The simulation belongs to an earlier workbook revision; run it again"));
    }
    let rows = preview["worksheet_statistics"]
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= MAX_BINDINGS)
        .ok_or_else(|| bad(SOLVE, "The completed simulation has no worksheet statistics to apply"))?;
    let mut updates = std::collections::BTreeMap::new();
    let wb = &s.doc()?.wb;
    if wb.calc.mode == gridcraft_model::CalcMode::Manual {
        return Err(bad(SOLVE, "Enable automatic calculation before applying worksheet statistics"));
    }
    for row in rows {
        let target = row["cell"].as_str().ok_or_else(|| bad(SOLVE, "Missing worksheet statistic cell"))?;
        let target = cells(wb, wb.active_sheet(), target, 1)?.into_iter().next().ok_or_else(|| bad(SOLVE, "Missing worksheet statistic cell"))?;
        let value = row["value"].as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(SOLVE, "Invalid worksheet statistic value"))?;
        let Some((name, args)) = super::worksheet::top_call(wb, target.0, target.1) else {
            return Err(bad(SOLVE, "Missing worksheet statistic formula"));
        };
        if source(wb, target, &name, &args).is_none() {
            return Err(bad(SOLVE, "Unsupported worksheet statistic formula"));
        }
        updates.insert(target, (gridcraft_formula::print(&FormulaExpr::Call(name, args)), value));
    }
    edit(s, |cx| {
        let old = std::mem::replace(&mut cx.wb.simulation_statistics, Arc::new(updates));
        let changed: Vec<_> = old.keys().chain(cx.wb.simulation_statistics.keys()).copied().collect();
        for (sheet, cell) in changed {
            cx.touch(sheet, cell);
        }
        Ok(())
    })?;
    Ok(json!({"updated":rows.len()}))
}
