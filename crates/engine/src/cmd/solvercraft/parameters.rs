//! Parameter comparisons use independent workbook copies and common random
//! numbers; each parameter value is constant throughout one simulation.
use super::{
    Cell, CommandSpec, EngineError, HashSet, Json, MAX_BINDINGS, NoControl, Result, SOLVE, Session, SimulationOptions, Snapshot, Value, Workbook,
    bad, cells, cmd, has_doc, json, sdk_error, simulate_snapshot, snapshot, str_param, worksheet,
};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.simulateParameters", "Solvercraft parameter comparison", [], None, "{request:simulation request,parameter_cases:[{name,values:[{cell,value}]}]} -> comparable per-case simulation previews. At most32 cases/1million combined trials; same seed/streams across cases; never edits the live workbook.", has_doc, run),
    ]
}
fn run(s: &mut Session, p: &Json) -> Result<Json> {
    let request = p.get("request").ok_or_else(|| bad(SOLVE, "A simulation request is required"))?;
    let cases = p.get("parameter_cases").ok_or_else(|| bad(SOLVE, "parameter_cases is required"))?;
    execute(&s.doc()?.wb, &s.doc()?.calc, snapshot(s)?, request, cases, &NoControl)
}
pub(super) fn execute(
    wb: &Workbook,
    calc: &gridcraft_calc::Calc,
    snapshot: Snapshot,
    request: &Json,
    cases: &Json,
    control: &dyn ::solvercraft::types::RunControl,
) -> Result<Json> {
    if !request.is_object() {
        return Err(bad(SOLVE, "Simulation request must be an object"));
    }
    let mut request = request.clone();
    super::pin_request_sheet(wb, &mut request, "simulation")?;
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(calc.fixed_now.unwrap_or_else(gridcraft_calc::now_serial));
    }
    let request = &request;
    let cases = cases.as_array().filter(|v| !v.is_empty() && v.len() <= 32).ok_or_else(|| bad(SOLVE, "parameter_cases must contain1 to32 cases"))?;
    let options: SimulationOptions = serde_json::from_value(request.get("options").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    if options.trials.checked_mul(cases.len() as u64).is_none_or(|n| n > 1_000_000) {
        return Err(bad(SOLVE, "Combined parameter simulation exceeds one million trials"));
    }
    let base = str_param(request, "sheet")
        .map(|name| wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "Parameter model sheet does not exist")))
        .transpose()?
        .unwrap_or(wb.active_sheet());
    let mut names = HashSet::new();
    let mut results = Vec::new();
    for case in cases {
        if control.is_cancelled() {
            break;
        }
        let name = case
            .get("name")
            .and_then(Json::as_str)
            .filter(|n| !n.trim().is_empty() && n.len() <= 256)
            .ok_or_else(|| bad(SOLVE, "Each parameter case needs a bounded name"))?;
        if !names.insert(name.to_string()) {
            return Err(bad(SOLVE, "Parameter case names must be unique"));
        }
        let values = case
            .get("values")
            .and_then(Json::as_array)
            .filter(|v| !v.is_empty() && v.len() <= MAX_BINDINGS)
            .ok_or_else(|| bad(SOLVE, "Each case needs bounded parameter values"))?;
        let mut candidate = wb.clone();
        let mut seen = HashSet::new();
        let mut changed = Vec::new();
        for item in values {
            let text = item.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Parameter cell is required"))?;
            let key = cells(wb, base, text, 1)?.into_iter().next().ok_or_else(|| bad(SOLVE, "Parameter cell is missing"))?;
            if !seen.insert(key) {
                return Err(bad(SOLVE, "Parameter cells must be unique within a case"));
            }
            let value = item
                .get("value")
                .and_then(Json::as_f64)
                .filter(|v| v.is_finite())
                .ok_or_else(|| bad(SOLVE, "Parameter values must be finite numbers"))?;
            let sh = candidate.sheet_mut(key.0).ok_or(EngineError::NoDocument)?;
            if sh.spill_anchor(key.1).is_some() || sh.cell(key.1).is_some_and(|c| c.formula.is_some()) {
                return Err(bad(SOLVE, "Parameter cells must be independent constants"));
            }
            let style = sh.style_id(key.1);
            sh.set_cell(key.1, Cell { value: Value::number(value), formula: None, style });
            changed.push(key);
        }
        // A parameter may not also be overwritten by the uncertain input stream.
        let input_entries = match request.get("inputs").and_then(Json::as_array) {
            Some(inputs) => inputs.clone(),
            None => worksheet::discover(&candidate)?.0,
        };
        for entry in input_entries {
            let text = entry.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Input cell is required"))?;
            if cells(&candidate, base, text, 1)?.iter().any(|key| seen.contains(key)) {
                return Err(bad(SOLVE, "A parameter cannot also be a stochastic input"));
            }
        }
        candidate.calc.mode = gridcraft_model::CalcMode::Automatic;
        let mut calculation = calc.clone();
        calculation.cells_changed(&mut candidate, &changed);
        let mut per_case = request.clone();
        if !per_case.get("options").is_some_and(Json::is_object) {
            per_case["options"] = json!({});
        }
        per_case["options"]["max_memory_bytes"] = json!(options.max_memory_bytes / (cases.len() as u64));
        let preview = simulate_snapshot(&candidate, &calculation, snapshot.clone(), &per_case, control)?;
        results.push(json!({"name":name,"parameters":values,"preview":preview}));
    }
    Ok(
        json!({"cases":results,"requested_cases":cases.len(),"snapshot":snapshot,"seed_policy":"common_random_numbers","parameter_semantics":"constant_within_case","cancelled":control.is_cancelled()}),
    )
}
