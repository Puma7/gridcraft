//! Answer reports describe the source model and validated candidate. Backend
//! duals and sensitivity intervals are never inferred from a local optimum.
use super::{
    CommandSpec, Feasibility, Json, NoControl, Relation, Result, SOLVE, Session, Snapshot, SolveResult, bad, bounds_model, cmd, has_doc, json,
    prepare, rounded_candidate, sdk_error, snapshot, validate_candidate, verify_workbook_objective,
};
pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.report.lpSensitivity", "Solvercraft LP basis sensitivity", [], None,"{request:model request,snapshot?:preview snapshot,warm_start?:LpBasis} -> checked LP vertex basis, normalized-row shadow prices and one-at-a-time RHS/objective validity intervals. Continuous LP only; never inferred from local NLP or integer results.",has_doc,lp_sensitivity),
        cmd!(query "solvercraft.report.answer", "Solvercraft answer report", [], None, "Pass a solve preview unchanged (or {preview}). Returns original expressions, values, row activities/slacks, variable bounds, tolerances and distinct status claims. Requires the preview's workbook revision; never changes cells.",has_doc,answer),
    ]
}
fn lp_sensitivity(s: &mut Session, p: &Json) -> Result<Json> {
    let current = snapshot(s)?;
    if let Some(captured) = p.get("snapshot") {
        let captured: Snapshot = serde_json::from_value(captured.clone()).map_err(sdk_error)?;
        if captured.document_uid != current.document_uid || captured.revision != current.revision {
            return Err(bad(SOLVE, "Sensitivity report requires the original workbook revision"));
        }
    }
    let mut request = p.get("request").cloned().ok_or_else(|| bad(SOLVE, "Original model request is required"))?;
    if !request.is_object() {
        return Err(bad(SOLVE, "Model request must be an object"));
    }
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(s.doc()?.calc.fixed_now.unwrap_or_else(gridcraft_calc::now_serial));
    }
    if request.get("sheet").is_none() && request.get("model").is_none() {
        request["sheet"] = json!(s.doc()?.wb.active().ok_or_else(|| bad(SOLVE, "Active sheet is missing"))?.name);
    }
    let prepared = prepare(&s.doc()?.wb, &request)?;
    if prepared.has_auxiliaries() {
        return Err(bad(SOLVE, "LP sensitivity is not available for auxiliary affine formulations"));
    }
    let model = prepared.model.as_ref().ok_or_else(|| bad(SOLVE, "LP sensitivity requires a structurally compiled linear model"))?;
    let warm: Option<::solvercraft::engines::LpBasis> = p.get("warm_start").cloned().map(serde_json::from_value).transpose().map_err(sdk_error)?;
    let detail = ::solvercraft::engines::solve_lp_with_basis(model, &prepared.options, &NoControl, warm.as_ref()).map_err(sdk_error)?;
    if let Some(values) = detail.result.values.as_ref().filter(|_| detail.result.feasibility == Feasibility::Feasible) {
        validate_candidate(&prepared, values)?;
        verify_workbook_objective(s.doc()?, &request, &prepared, values)?;
    }
    Ok(
        json!({"analysis":"lp_basis_intervals","snapshot":current,"request":request,"detail":detail,"scope":"Original-coordinate normalized rows; one RHS or objective coefficient changes at a time. Simultaneous changes require basis revalidation."}),
    )
}
fn answer(s: &mut Session, p: &Json) -> Result<Json> {
    let preview = p.get("preview").unwrap_or(p);
    let captured: Snapshot =
        serde_json::from_value(preview.get("snapshot").cloned().ok_or_else(|| bad(SOLVE, "Preview snapshot is required"))?).map_err(sdk_error)?;
    let current = snapshot(s)?;
    if captured.document_uid != current.document_uid || captured.revision != current.revision {
        return Err(bad(SOLVE, "Answer report requires the original workbook revision"));
    }
    let request = preview.get("request").ok_or_else(|| bad(SOLVE, "Original request is required"))?;
    let prepared = prepare(&s.doc()?.wb, request)?;
    let result: SolveResult =
        serde_json::from_value(preview.get("result").cloned().ok_or_else(|| bad(SOLVE, "Result is required"))?).map_err(sdk_error)?;
    let model = prepared
        .model
        .as_ref()
        .filter(|_| !prepared.has_auxiliaries())
        .cloned()
        .or_else(|| prepared.expression.as_ref().map(bounds_model))
        .ok_or_else(|| bad(SOLVE, "Original model is missing"))?;
    let mut variables = Vec::new();
    let mut constraints = Vec::new();
    let mut checked_objective = None;
    if let Some(values) = result.values.as_ref() {
        if values.len() != model.variables.len() {
            return Err(bad(SOLVE, "Result dimension differs from the original model"));
        }
        if result.feasibility == Feasibility::Feasible {
            validate_candidate(&prepared, values)?;
        }
        let rounded = rounded_candidate(&prepared, values)?;
        let values = rounded.as_slice();
        if result.feasibility == Feasibility::Feasible {
            validate_candidate(&prepared, values)?;
            verify_workbook_objective(s.doc()?, request, &prepared, values)?;
        }
        checked_objective = Some(if let Some(expression) = prepared.expression.as_ref() {
            ::solvercraft::compile::evaluate(&expression.objective, values).map_err(sdk_error)?
        } else {
            ::solvercraft::model::evaluate_objective(&model, values).map_err(sdk_error)?
        });
        for (variable, value) in model.variables.iter().zip(values) {
            if !value.is_finite() {
                return Err(bad(SOLVE, "Report values must be finite"));
            }
            let (lower, upper) = ::solvercraft::model::effective_bounds(variable).map_err(sdk_error)?;
            variables.push(json!({"name":variable.name,"kind":variable.kind,"value":value,"lower":lower,"upper":upper,"lower_slack":lower.map(|lo|value-lo),"upper_slack":upper.map(|hi|hi-value)}));
        }
        if let Some(expression) = prepared.expression.as_ref() {
            for row in &expression.constraints {
                let left = ::solvercraft::compile::evaluate(&row.left, values).map_err(sdk_error)?;
                let right = ::solvercraft::compile::evaluate(&row.right, values).map_err(sdk_error)?;
                let residual = left - right;
                if !residual.is_finite() {
                    return Err(bad(SOLVE, "Constraint residual exceeds finite report arithmetic"));
                }
                let slack = match row.relation {
                    Relation::LessEqual => -residual,
                    Relation::GreaterEqual => residual,
                    Relation::Equal => -residual.abs(),
                };
                constraints.push(json!({"name":row.name,"left_expression":row.left,"right_expression":row.right,"relation":row.relation,"left_value":left,"right_value":right,"slack":slack,"violation":(-slack).max(0.0),"within_tolerance":slack>=-prepared.options.feasibility_tolerance}));
            }
        }
    }
    Ok(
        json!({"schema_version":1,"snapshot":captured,"name":model.name,"direction":model.objective.direction,"objective":{"source":request.get("objective"),"validated_value":checked_objective,"reported_value":result.objective},"variables":variables,"constraints":constraints,"source_constraints":request.get("constraints"),"portable_model":prepared.model,"expression_model":prepared.expression,"tolerances":{"feasibility":prepared.options.feasibility_tolerance,"integrality":prepared.options.integrality_tolerance,"optimality":prepared.options.optimality_tolerance},"result":result,"dual_data":{"available":false,"reason":"This solve-result contract carries no validated backend dual solution."},"sensitivity":{"available":false,"reason":"LP basis ranges, local derivatives and parameter simulations are distinct analyses; no basis intervals were supplied by this backend."}}),
    )
}
