//! Worksheet declarations and explicit, signature-aware legacy formula import.
use super::{
    Cell, CellRef, CommandSpec, Distribution, FormulaExpr, Json, MAX_BINDINGS, Result, SOLVE, Session, Value, Workbook, bad, binding_name, cells,
    cmd, edit, has_doc, json, sdk_error, str_param,
};

const SCAN_CELLS: usize = 200_000;
pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.worksheet.discover", "Discover Solvercraft worksheet bindings", [], None, "{} -> supported SC distribution inputs and SC.OUTPUT cells. Parameters are captured once per run; each input shares one value per trial.", has_doc, discover_command),
        cmd!(
            "solvercraft.worksheet.importPsi",
            "Import supported Psi formulas",
            [],
            None,
            "{range?:reference,apply?:false} -> proposed changes and unsupported signatures. Nineteen individually checked numeric Psi core signatures map to strict SC.PSI shims; modifiers/unknown calls are preserved. apply:true commits supported cells as one undo step. Outside-simulation values follow explicit SC nominal policy, not Frontline ValueToDisplay settings.",
            has_doc,
            import_psi
        ),
    ]
}
fn top_call(wb: &Workbook, sheet: usize, cell: CellRef) -> Option<(String, Vec<FormulaExpr>)> {
    let expression = wb.sheet(sheet)?.cell(cell)?.formula.as_ref()?.expr()?;
    match expression {
        FormulaExpr::Call(name, args) => Some((name, args)),
        _ => None,
    }
}
pub(super) fn input_distribution_at(wb: &Workbook, sheet: usize, cell: CellRef, now: f64) -> Result<Option<Distribution>> {
    let Some((name, args)) = top_call(wb, sheet, cell) else { return Ok(None) };
    if gridcraft_calc::distribution_arity(&name).is_none() {
        return Ok(None);
    }
    let values =
        args.iter().map(|expression| gridcraft_calc::evaluate_at(wb, sheet, cell, &gridcraft_formula::print(expression), now)).collect::<Vec<_>>();
    gridcraft_calc::worksheet_distribution(&name, &values).map(|declaration| declaration.map(|d| d.distribution)).map_err(sdk_error)
}
pub(super) fn discover(wb: &Workbook) -> Result<(Vec<Json>, Vec<String>)> {
    discover_at(wb, gridcraft_calc::now_serial())
}
pub(super) fn discover_at(wb: &Workbook, now: f64) -> Result<(Vec<Json>, Vec<String>)> {
    if wb.sheets.iter().map(|s| s.cells.len()).fold(0usize, usize::saturating_add) > SCAN_CELLS {
        return Err(bad(SOLVE, "Automatic worksheet discovery exceeds the scan budget; supply explicit bindings"));
    }
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for (si, sheet) in wb.sheets.iter().enumerate() {
        for (cell, _) in sheet.cells.iter() {
            if let Some(distribution) = input_distribution_at(wb, si, cell, now)? {
                if inputs.len() >= MAX_BINDINGS {
                    return Err(bad(SOLVE, "Too many SC distribution inputs"));
                }
                inputs.push(json!({"cell":binding_name(wb,si,cell)?,"distribution":distribution}));
            }
            if top_call(wb, si, cell).is_some_and(|(name, _)| name == "SC.OUTPUT") {
                if outputs.len() >= MAX_BINDINGS {
                    return Err(bad(SOLVE, "Too many SC outputs"));
                }
                outputs.push(binding_name(wb, si, cell)?);
            }
        }
    }
    Ok((inputs, outputs))
}
fn discover_command(s: &mut Session, _: &Json) -> Result<Json> {
    let (inputs, outputs) = discover(&s.doc()?.wb)?;
    Ok(
        json!({"inputs":inputs,"outputs":outputs,"parameter_semantics":"fixed_at_run_snapshot","outside_simulation":"deterministic mean, or explicit nominal override","trial_semantics":"one sample per declared input per trial"}),
    )
}

fn import_psi(s: &mut Session, p: &Json) -> Result<Json> {
    let wb = &s.doc()?.wb;
    let selected = if let Some(range) = str_param(p, "range") {
        cells(wb, wb.active_sheet(), range, SCAN_CELLS)?
    } else {
        if wb.sheets.iter().map(|s| s.cells.len()).fold(0usize, usize::saturating_add) > SCAN_CELLS {
            return Err(bad(SOLVE, "Psi import exceeds the scan budget"));
        }
        wb.sheets.iter().enumerate().flat_map(|(si, sh)| sh.cells.iter().map(move |(cell, _)| (si, cell))).collect()
    };
    let mut changes = Vec::new();
    let mut unsupported = Vec::new();
    for (si, cell) in selected {
        let Some(original) = wb.sheet(si).and_then(|sh| sh.cell(cell)).and_then(|c| c.formula.as_ref()) else { continue };
        let Some(expression) = original.expr() else { continue };
        let mut rejected = Vec::new();
        let mut changed = false;
        let converted = expression.map(&mut |node| {
            let FormulaExpr::Call(name, args) = node else { return node };
            let normalized = name.strip_prefix("_xludf.").unwrap_or(&name).to_ascii_uppercase();
            if let Some(suffix) = normalized.strip_prefix("PSI") {
                let target = format!("SC.PSI.{suffix}");
                if gridcraft_calc::distribution_arity(&target) == Some(args.len()) {
                    changed = true;
                    return FormulaExpr::Call(target, args);
                }
            }
            if normalized.starts_with("PSI") {
                rejected.push(format!("{name} with {} arguments", args.len()));
            }
            FormulaExpr::Call(name, args)
        });
        if !rejected.is_empty() {
            unsupported.push(json!({"cell":binding_name(wb,si,cell)?,"signatures":rejected,"preserved":true}));
            continue;
        }
        if changed {
            changes.push((si, cell, gridcraft_formula::print(&converted)));
        }
    }
    let proposed = changes
        .iter()
        .map(|(si, cell, formula)| Ok(json!({"cell":binding_name(wb,*si,*cell)?,"formula":format!("={formula}")})))
        .collect::<Result<Vec<_>>>()?;
    let apply = p.get("apply").and_then(Json::as_bool).unwrap_or(false);
    if apply && !changes.is_empty() {
        edit(s, |cx| {
            for (si, cell, text) in changes {
                let sh = cx.sheet_mut(si)?;
                let style = sh.style_id(cell);
                sh.set_cell(cell, Cell { value: Value::Empty, formula: Some(std::sync::Arc::new(gridcraft_model::Formula::new(&text))), style });
                cx.touch(si, cell);
            }
            Ok(())
        })?;
    }
    Ok(
        json!({"applied":apply,"changes":proposed,"unsupported":unsupported,"notes":["Core numeric signatures use individually validated SC.PSI shims; optional Psi property modifiers are preserved as unsupported.","LogNormal uses arithmetic mean/std; LogNorm2 uses underlying-normal parameters. Exponential uses mean/scale; Gamma/Weibull use shape/scale. Pert weight is fixed4. Geometric/NegBinomial count failures.","SC nominal values use finite analytic means (documented median fallback where unavailable), distinct from Frontline ValueToDisplay settings.","PsiCumul supports strictly increasing probability/value knots and unambiguous endpoints; ambiguous endpoint atoms and improper p=0 geometric/negative-binomial parameters fail explicitly."]}),
    )
}
