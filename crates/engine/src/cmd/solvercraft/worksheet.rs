//! Worksheet declarations and explicit, signature-aware legacy formula import.
use super::{
    Cell, CellRef, CommandSpec, FormulaExpr, Json, MAX_BINDINGS, Result, SOLVE, Session, Value, Workbook, bad, binding_name, cells, cmd, edit,
    has_doc, json, sdk_error, snapshot,
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
            "{range?:reference,apply?:false,outside_policy?:solvercraft_nominal|preserve_vendor,expected_snapshot?:{document_uid,revision}} -> proposed changes and unsupported signatures. Selected top-level numeric Psi calls map to SC.PSI shims; nested stochastic calls, properties and unknown calls are preserved. apply:true commits supported cells as one undo step. Default solvercraft_nominal is an explicitly lossy outside-value conversion; preserve_vendor refuses conversion because vendor display state is unavailable.",
            has_doc,
            import_psi
        ),
    ]
}
fn top_call(wb: &Workbook, sheet: usize, cell: CellRef) -> Option<(String, Vec<FormulaExpr>)> {
    let mut expression = wb.sheet(sheet)?.cell(cell)?.formula.as_ref()?.expr()?;
    while let FormulaExpr::Paren(inner) = expression {
        expression = *inner;
    }
    match expression {
        FormulaExpr::Call(name, args) => Some((name, args)),
        _ => None,
    }
}
pub(super) fn input_declaration_at(wb: &Workbook, sheet: usize, cell: CellRef, now: f64) -> Result<Option<gridcraft_calc::WorksheetDistribution>> {
    let Some((name, args)) = top_call(wb, sheet, cell) else { return Ok(None) };
    if gridcraft_calc::distribution_arity(&name).is_none() {
        return Ok(None);
    }
    let values =
        args.iter().map(|expression| gridcraft_calc::evaluate_at(wb, sheet, cell, &gridcraft_formula::print(expression), now)).collect::<Vec<_>>();
    gridcraft_calc::worksheet_distribution(&name, &values).map_err(sdk_error)
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
            if let Some(declaration) = input_declaration_at(wb, si, cell, now)? {
                if inputs.len() >= MAX_BINDINGS {
                    return Err(bad(SOLVE, "Too many SC distribution inputs"));
                }
                inputs.push(json!({"cell":binding_name(wb,si,cell)?,"distribution":declaration.distribution,"nominal":declaration.nominal,"nominal_basis":declaration.nominal_basis.as_str()}));
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
        json!({"inputs":inputs,"outputs":outputs,"parameter_semantics":"fixed_at_run_snapshot","outside_simulation":"finite analytic mean; labelled median fallback for undefined/unavailable means; optional explicit nominal override","trial_semantics":"one sample per declared input per trial"}),
    )
}

enum PsiContext {
    Absent,
    Core,
    Output,
    Unsupported(&'static str),
}
fn bare_output(expression: &FormulaExpr) -> bool {
    matches!(expression, FormulaExpr::Call(name, args)
        if name.strip_prefix("_xludf.").unwrap_or(name).eq_ignore_ascii_case("PSIOUTPUT") && args.is_empty())
}

fn additive_output(expression: &FormulaExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match node {
            FormulaExpr::Binary(gridcraft_formula::BinOp::Add, left, right) => {
                pending.push(left);
                pending.push(right);
            }
            FormulaExpr::Paren(inner) => pending.push(inner),
            _ if bare_output(node) => return true,
            _ => {}
        }
    }
    false
}

fn psi_context(expression: &FormulaExpr, outside_policy: &str) -> PsiContext {
    let mut count = 0usize;
    expression.walk(&mut |node| {
        if let FormulaExpr::Call(name, _) = node
            && name.strip_prefix("_xludf.").unwrap_or(name).to_ascii_uppercase().starts_with("PSI")
        {
            count = count.saturating_add(1);
        }
    });
    if count == 0 {
        return PsiContext::Absent;
    }
    let mut root = expression;
    while let FormulaExpr::Paren(inner) = root {
        root = inner;
    }
    let root_is_psi =
        matches!(root, FormulaExpr::Call(name, _) if name.strip_prefix("_xludf.").unwrap_or(name).to_ascii_uppercase().starts_with("PSI"));
    if outside_policy == "preserve_vendor" {
        PsiContext::Unsupported("vendor_display_context_unavailable")
    } else if count == 1 && additive_output(root) {
        PsiContext::Output
    } else if !root_is_psi || count != 1 {
        PsiContext::Unsupported("unsupported_nested_psi_or_property")
    } else if matches!(root, FormulaExpr::Call(_, args) if args.iter().any(|arg| matches!(arg, FormulaExpr::Missing))) {
        PsiContext::Unsupported("unsupported_missing_required_argument")
    } else {
        PsiContext::Core
    }
}
fn import_psi(s: &mut Session, p: &Json) -> Result<Json> {
    let range = match p.get("range") {
        None => None,
        Some(Json::String(range)) => Some(range.as_str()),
        Some(_) => return Err(bad(SOLVE, "range must be a reference string")),
    };
    let apply = match p.get("apply") {
        None => false,
        Some(Json::Bool(apply)) => *apply,
        Some(_) => return Err(bad(SOLVE, "apply must be a boolean")),
    };
    let outside_policy = match p.get("outside_policy") {
        None => "solvercraft_nominal",
        Some(Json::String(policy)) => policy.as_str(),
        Some(_) => return Err(bad(SOLVE, "outside_policy must be a named policy")),
    };
    if !matches!(outside_policy, "solvercraft_nominal" | "preserve_vendor") {
        return Err(bad(SOLVE, "outside_policy must be solvercraft_nominal or preserve_vendor"));
    }
    let stamp = snapshot(s)?;
    if let Some(expected) = p.get("expected_snapshot")
        && (expected["document_uid"].as_str() != Some(stamp.document_uid.as_str()) || expected["revision"].as_str() != Some(stamp.revision.as_str()))
    {
        return Err(bad(SOLVE, "Workbook changed after Psi preview; scan again"));
    }
    let wb = &s.doc()?.wb;
    let selected = if let Some(range) = range {
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
        let Some(expression) = original.expr_arc() else { continue };
        let context = psi_context(&expression, outside_policy);
        if matches!(context, PsiContext::Absent) {
            continue;
        }
        let source_formula = format!("={}", original.text);
        if original.array.is_some() || original.dynamic {
            unsupported.push(
                json!({"cell":binding_name(wb,si,cell)?,"reason":"unsupported_array_formula","source_formula":source_formula,"preserved":true}),
            );
            continue;
        }
        match context {
            PsiContext::Absent => continue,
            PsiContext::Unsupported(reason) => {
                unsupported.push(json!({"cell":binding_name(wb,si,cell)?,"reason":reason,"source_formula":source_formula,"preserved":true}));
                continue;
            }
            PsiContext::Output => {
                let arithmetic = (*expression).clone().map(&mut |node| if bare_output(&node) { FormulaExpr::Number(0.0) } else { node });
                let converted = FormulaExpr::Call("SC.OUTPUT".into(), vec![arithmetic]);
                changes.push((si, cell, source_formula, gridcraft_formula::print(&converted)));
                continue;
            }
            PsiContext::Core => {}
        }
        let mut rejected = Vec::new();
        let mut changed = false;
        let converted = (*expression).clone().map(&mut |node| {
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
            unsupported.push(json!({"cell":binding_name(wb,si,cell)?,"reason":"unsupported_signature","source_formula":source_formula,"signatures":rejected,"preserved":true}));
            continue;
        }
        if changed {
            changes.push((si, cell, source_formula, gridcraft_formula::print(&converted)));
        }
    }
    let proposed = changes
        .iter()
        .map(|(si, cell, source_formula, formula)| Ok(json!({"cell":binding_name(wb,*si,*cell)?,"source_formula":source_formula,"formula":format!("={formula}"),"outside_policy":outside_policy,"vendor_display_context_preserved":false})))
        .collect::<Result<Vec<_>>>()?;
    let applied = apply && !changes.is_empty();
    if applied {
        edit(s, |cx| {
            for (si, cell, _, text) in changes {
                let sh = cx.sheet_mut(si)?;
                let style = sh.style_id(cell);
                sh.set_cell(cell, Cell { value: Value::Empty, formula: Some(std::sync::Arc::new(gridcraft_model::Formula::new(&text))), style });
                cx.touch(si, cell);
            }
            Ok(())
        })?;
    }
    Ok(
        json!({"applied":applied,"apply_requested":apply,"snapshot":stamp,"changes":proposed,"unsupported":unsupported,"outside_policy":outside_policy,"vendor_display_context_preserved":false,"notes":["Core numeric signatures use SC.PSI shims. An additive zero-argument PsiOutput marker maps to SC.OUTPUT. Other nested stochastic calls, omitted required arguments and optional Psi property modifiers are preserved.","LogNormal uses arithmetic mean/std; LogNorm2 uses underlying-normal parameters. Exponential uses mean/scale; Gamma/Weibull use shape/scale. Pert weight is fixed4. Geometric/NegBinomial count failures.","SC nominal values use finite analytic means (documented median fallback where unavailable), distinct from Frontline ValueToDisplay settings. No vendor pre-run random, displayed-trial, sample-mean or base-case context is preserved.","PsiCumul supports strictly increasing probability/value knots and unambiguous endpoints; ambiguous endpoint atoms and improper p=0 geometric/negative-binomial parameters fail explicitly."]}),
    )
}
