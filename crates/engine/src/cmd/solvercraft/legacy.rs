//! Explicit metadata-only export to the original Solver's hidden solver_* names.
use super::{
    CommandSpec, Json, MAX_CONSTRAINTS, Relation, Result, SOLVE, Session, VariableKind, Workbook, bad, binding_name, cells, cmd, constraint_parts,
    has_doc, json, prepare, registry, relation, str_param,
};

fn legacy_cells(wb: &Workbook, sheet: usize, text: &str) -> Result<Vec<(usize, super::CellRef)>> {
    let mut result = Vec::new();
    for part in text.split(',').map(str::trim).filter(|part| !part.is_empty()) {
        result.extend(cells(wb, sheet, part, 4096usize.saturating_sub(result.len()))?);
    }
    if result.is_empty() {
        return Err(bad(SOLVE, "Legacy reference is empty"));
    }
    Ok(result)
}
/// The legacy name format uses comma-separated references rather than formula
/// union syntax. Expand it into stable scalar bindings and pair ranges exactly
/// as the original model parser does, including scalar RHS broadcasting.
pub(super) fn import_request(wb: &Workbook, sheet: usize, request: &mut Json) -> Result<()> {
    let variables = str_param(request, "variables").ok_or_else(|| bad(SOLVE, "Legacy model has no variables"))?;
    let variables = legacy_cells(wb, sheet, variables)?.into_iter().map(|(si, cell)| binding_name(wb, si, cell)).collect::<Result<Vec<_>>>()?;
    request["variables"] = json!(variables);
    if let Some(rows) = request.get("constraints").and_then(Json::as_array) {
        let mut constraints = Vec::new();
        for row in rows {
            let (left, op, right) = constraint_parts(row)?;
            let left = legacy_cells(wb, sheet, &left)?;
            let numeric = gridcraft_core::parse::parse_number_text(right.trim()).filter(|value| value.is_finite());
            let right_cells = if numeric.is_none() && !matches!(op.as_str(), "int" | "bin") { legacy_cells(wb, sheet, &right)? } else { Vec::new() };
            if !right_cells.is_empty() && right_cells.len() != 1 && right_cells.len() != left.len() {
                return Err(bad(SOLVE, "Legacy restriction sides have incompatible sizes"));
            }
            for (i, (si, cell)) in left.into_iter().enumerate() {
                if constraints.len() >= MAX_CONSTRAINTS {
                    return Err(bad(SOLVE, "Expanded legacy model exceeds the restriction budget"));
                }
                let rhs = if let Some(number) = numeric {
                    json!(number)
                } else if let Some((si, cell)) = right_cells.get(if right_cells.len() == 1 { 0 } else { i }) {
                    json!(binding_name(wb, *si, *cell)?)
                } else {
                    json!("")
                };
                constraints.push(json!({"lhs":binding_name(wb,si,cell)?,"op":op,"rhs":rhs}));
            }
        }
        request["constraints"] = json!(constraints);
    }
    Ok(())
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "solvercraft.model.exportLegacy",
        "Export legacy Solver model",
        [],
        None,
        "{id:string,apply?:false} -> representability report and canonical legacy params. apply:true writes only sheet-scoped solver_* model names as one undo step. Same-sheet linear models only; unsupported scope/shape/features are reported without silently exporting a subset.",
        has_doc,
        export
    )]
}
fn local_reference(wb: &Workbook, sheet: usize, text: &str, cap: usize) -> Result<String> {
    let references = cells(wb, sheet, text, cap)?;
    if references.iter().any(|(si, _)| *si != sheet) {
        return Err(bad(SOLVE, "Legacy export cannot represent cross-sheet references"));
    }
    Ok(references.iter().map(|(_, cell)| cell.a1()).collect::<Vec<_>>().join(","))
}
fn translate(wb: &Workbook, request: &Json) -> Result<(usize, Json)> {
    let prepared = prepare(wb, request)?;
    let model = prepared.model.as_ref().ok_or_else(|| bad(SOLVE, "Legacy export supports structurally compiled linear models only"))?;
    if !model.objective.quadratic.is_empty() {
        return Err(bad(SOLVE, "Quadratic/nonlinear Solvercraft models have no lossless legacy Simplex export"));
    }
    let sheet = prepared
        .bindings
        .first()
        .map(|binding| binding.0)
        .ok_or_else(|| bad(SOLVE, "Portable models have no worksheet bindings for legacy export"))?;
    if prepared.bindings.len() > 200 {
        return Err(bad(SOLVE, "Legacy Solver supports at most200 decision cells"));
    }
    if prepared.bindings.iter().any(|binding| binding.0 != sheet) {
        return Err(bad(SOLVE, "Legacy export cannot represent decision cells on multiple sheets"));
    }
    let sh = wb.sheet(sheet).ok_or_else(|| bad(SOLVE, "Model sheet is missing"))?;
    if sh.name.contains(',') {
        return Err(bad(SOLVE, "The legacy model-name format cannot losslessly reload sheet names containing a comma"));
    }
    let objective = str_param(request, "objective").ok_or_else(|| bad(SOLVE, "Legacy export requires an objective formula cell"))?;
    let base = str_param(request, "sheet").and_then(|name| wb.sheet_index(name)).unwrap_or(wb.active_sheet());
    let objective_cells = cells(wb, base, objective, 1)?;
    let (objective_sheet, objective_cell) = objective_cells.first().copied().ok_or_else(|| bad(SOLVE, "Objective cell is missing"))?;
    if objective_sheet != sheet {
        return Err(bad(SOLVE, "Legacy objective and decision cells must share one sheet"));
    }
    let mut constraints = Vec::new();
    if let Some(rows) = request.get("constraints").and_then(Json::as_array) {
        for row in rows {
            let (left, op, right) = constraint_parts(row)?;
            if matches!(op.as_str(), "int" | "bin") {
                continue;
            } // canonical domains below retain these.
            let left = local_reference(wb, sheet, &left, 1000)?;
            let right = match gridcraft_core::parse::parse_number_text(right.trim()).filter(|value| value.is_finite()) {
                Some(value) => json!(value),
                None => json!(local_reference(wb, sheet, &right, 1000)?),
            };
            let op = match relation(&op)? {
                Relation::Equal => "=",
                Relation::LessEqual => "<=",
                Relation::GreaterEqual => ">=",
            };
            constraints.push(json!({"lhs":left,"op":op,"rhs":right}));
        }
    }
    // Typed variables and inferred bounds are represented explicitly, so the
    // legacy checkbox cannot silently change free/negative/binary domains.
    for ((_, cell), variable) in prepared.bindings.iter().zip(&model.variables) {
        let reference = cell.a1();
        if let Some(lower) = variable.lower {
            constraints.push(json!({"lhs":reference,"op":">=","rhs":lower}));
        }
        if let Some(upper) = variable.upper {
            constraints.push(json!({"lhs":reference,"op":"<=","rhs":upper}));
        }
        let kind = match variable.kind {
            VariableKind::Continuous => None,
            VariableKind::Integer => Some("int"),
            VariableKind::Binary => Some("bin"),
        };
        if let Some(kind) = kind {
            constraints.push(json!({"lhs":reference,"op":kind}));
        }
    }
    if constraints.len() > 1000 {
        return Err(bad(SOLVE, "Legacy Solver supports at most1000 expanded restrictions"));
    }
    let mut params = json!({"objective":binding_name(wb,sheet,objective_cell)?,"variables":prepared.bindings.iter().map(|(_,cell)|cell.a1()).collect::<Vec<_>>().join(","),"constraints":constraints,"goal":str_param(request,"goal").unwrap_or("max"),"nonNegative":false,"method":"simplex"});
    if str_param(request, "goal") == Some("value") {
        let target = request
            .get("value")
            .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .filter(|v| v.is_finite())
            .ok_or_else(|| bad(SOLVE, "A finite target is required"))?;
        params["value"] = json!(target);
    }
    Ok((sheet, params))
}
fn export(s: &mut Session, p: &Json) -> Result<Json> {
    let apply = p.get("apply").and_then(Json::as_bool).unwrap_or(false);
    let saved = registry::get(s, p)?;
    let translated = if saved["kind"] == json!("optimization") {
        translate(&s.doc()?.wb, &saved["request"])
    } else {
        Err(bad(SOLVE, "Simulation models have no solver_* optimization representation"))
    };
    let (sheet, params) = match translated {
        Ok(value) => value,
        Err(error) => {
            if apply {
                return Err(error);
            }
            return Ok(json!({"exportable":false,"applied":false,"unsupported":[error.to_string()]}));
        }
    };
    let previous = super::super::solver::saved_model(&s.doc()?.wb, sheet).is_some();
    let sheet_name = s.doc()?.wb.sheet(sheet).ok_or_else(|| bad(SOLVE, "Target sheet is missing"))?.name.clone();
    let canonical = match super::super::solver::export_saved_model(s, &params, apply) {
        Ok(value) => value,
        Err(error) => {
            if apply {
                return Err(error);
            }
            return Ok(json!({"exportable":false,"applied":false,"unsupported":[error.to_string()]}));
        }
    };
    Ok(
        json!({"exportable":true,"applied":apply,"sheet":sheet_name,"would_replace_existing":previous,"model":canonical,"unsupported":[],"unrepresented_settings":["Solvercraft engine configuration and computational budgets","warm starts, dual/basis reports and run manifests","Solvercraft registry identity/version"],"notes":["Only the representable mathematical model is exported, using the legacy Simplex method.","Existing sheet-scoped solver_* names are replaced only when apply:true is invoked; Solvercraft's own registry is preserved."]}),
    )
}
