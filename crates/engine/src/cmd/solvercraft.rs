//! Workbook bindings for the independent Solvercraft SDK.
//!
//! Commands never open dialogs. Synchronous calls and native background jobs
//! preview immutable snapshots; explicit apply checks identity and revision.
use std::collections::{HashMap, HashSet};

mod extended_affine;
#[cfg(test)]
mod host_tests;
#[cfg(not(target_arch = "wasm32"))]
mod jobs;
#[cfg(target_arch = "wasm32")]
#[path = "solvercraft/jobs_web.rs"]
mod jobs;
mod legacy;
mod lot_sizing;
mod manifest;
mod parameters;
mod portfolio;
#[cfg(test)]
mod psi_import_tests;
mod registry;
mod reports;
mod result_sheet;
mod simulation_ir;
mod statistics;
mod worksheet;
#[cfg(test)]
mod worksheet_tests;

use ::solvercraft::compile::{Expr, ExpressionConstraint, ExpressionModel};
use ::solvercraft::types::{
    Direction, Distribution, Feasibility, NoControl, Objective, OptimizationModel, Relation, SCHEMA_VERSION, SimulationOptions, SolveMethod,
    SolveOptions, SolveResult, Variable, VariableKind,
};
use gridcraft_core::{CellRef, Value};
use gridcraft_formula::{BinOp, Expr as FormulaExpr, Reference, SheetSel, UnOp};
use gridcraft_model::{Cell, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::{CommandSpec, EngineError, Result, Session, always, bad, cmd, edit, has_doc, str_param};

const SOLVE: &str = "solvercraft.solve";
const MAX_BINDINGS: usize = 4096;
const MAX_CONSTRAINTS: usize = 4096;
const MAX_RANGE_CELLS: u64 = 32_768;
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 96;
type CellBinding = (usize, CellRef);
type VariableBindings = (Vec<Variable>, Vec<CellBinding>);

/// The session calls this only when that document actually closes. No other
/// session's live jobs are inferred to be orphaned from a local document list.
pub(crate) fn document_closed(uid: u64) {
    jobs::document_closed(uid);
}
/// Entry point for the separate Trunk-generated Solvercraft Web Worker binary.
#[cfg(target_arch = "wasm32")]
pub fn browser_worker_main() {
    jobs::worker_main();
}

pub fn specs() -> Vec<CommandSpec> {
    let mut specs = vec![
        cmd!(query "solvercraft.capabilities", "Solvercraft capabilities", [], None,
            "{} -> supported formula subset and execution capabilities; never opens a dialog.",
            always, capabilities),
        cmd!(query "solvercraft.validate", "Validate Solvercraft model", [], None,
            "{objective: cell|formula, goal?: min|max|value, value?: number, variables: range|string[]|[{cell,kind?,lower?,upper?}], constraints?: [string|{lhs,op,rhs}], nonNegative?: true, options?: SolveOptions} -> {model,bindings,class}; alternatively {model: OptimizationModel}.",
            has_doc, validate),
        cmd!(query "solvercraft.distribution.preview", "Preview a distribution", [], None,
            "{distribution} -> validated distribution and bounded inverse-CDF preview points. Does not sample or change the workbook.",
            always, distribution_preview),
        cmd!(query "solvercraft.solve", "Solvercraft", ["Data", "Analysis"], None,
            "Same parameters as solvercraft.validate. Returns a bounded synchronous preview {result,request,snapshot,bindings}; does not change cells. Use solvercraft.result.apply explicitly.",
            has_doc, solve),
        cmd!(query "solvercraft.simulate", "Solvercraft simulation", [], None,
            "{inputs:[{cell,distribution}],outputs:cell|range|string[],options?:SimulationOptions} -> {result,outputs,snapshot}. Simulates the supported structural formula subset without changing the workbook.",
            has_doc, simulate),
        cmd!(
            "solvercraft.result.apply",
            "Apply Solvercraft solution",
            [],
            None,
            "Pass a solvercraft.solve preview unchanged. Rebuilds and validates its original model, rejects a different/edited workbook, then applies all variable cells as one undo step.",
            has_doc,
            apply
        ),
    ];
    specs.extend(registry::specs());
    specs.extend(jobs::specs());
    specs.extend(worksheet::specs());
    specs.extend(parameters::specs());
    specs.extend(reports::specs());
    specs.extend(legacy::specs());
    specs
}

fn sdk_error(e: impl std::fmt::Display) -> EngineError {
    bad(SOLVE, e.to_string())
}

fn distribution_preview(_: &mut Session, p: &Json) -> Result<Json> {
    let source = p.get("distribution").ok_or_else(|| bad(SOLVE, "Choose a distribution"))?;
    if serde_json::to_vec(source).map_err(sdk_error)?.len() > 131_072 {
        return Err(bad(SOLVE, "The distribution exceeds the preview size limit"));
    }
    let distribution: Distribution = serde_json::from_value(source.clone()).map_err(sdk_error)?;
    ::solvercraft::math::validate_distribution(&distribution).map_err(sdk_error)?;
    let mut points = Vec::new();
    for index in 0..=32 {
        let probability = 0.01 + f64::from(index) * 0.98 / 32.0;
        match ::solvercraft::math::quantile(&distribution, probability) {
            Ok(value) if value.is_finite() => points.push(json!([value, probability])),
            _ => return Ok(json!({"valid": true, "points": [], "note": "A finite inverse-CDF preview is unavailable for this distribution."})),
        }
    }
    Ok(json!({"valid": true, "points": points}))
}

fn capabilities(_: &mut Session, _: &Json) -> Result<Json> {
    let psi_signatures = gridcraft_calc::PSI_CORE_SIGNATURES
        .iter()
        .map(|signature| json!({"legacy_name":signature.legacy_name,"worksheet_name":signature.worksheet_name,"required_arguments":signature.required_arguments}))
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": 1,
        "preview_only": true,
        "execution": "synchronous_bounded_and_background_snapshot",
        "background_jobs": {"native":!cfg!(target_arch="wasm32"),"wasm":cfg!(target_arch="wasm32"),"start_command":"solvercraft.job.start","cancel_command":"solvercraft.job.cancel","apply_command":"solvercraft.job.apply","retained_limit":64,"browser_cancel":"worker termination; no untransmitted incumbent claimed"},
        "automatic_apply": false,
        "sdk": ::solvercraft::capabilities(),
        "workbook_formula_subset": [
            "numeric constants", "cell and named references", "+ - * /",
            "nonnegative integer powers", "SUM", "SUMPRODUCT", "IF", "CHOOSE",
            "SIN", "COS", "EXP", "LN", "SQRT", "ABS", "MIN", "MAX", "NOW", "TODAY", "SC.OUTPUT"
        ],
        "worksheet_declarations":["SC.UNIFORM","SC.DISCRETEUNIFORM","SC.NORMAL","SC.LOGNORMAL","SC.TRIANGULAR","SC.BETAPERT","SC.BETA","SC.GAMMA","SC.EXPONENTIAL","SC.WEIBULL","SC.BERNOULLI","SC.BINOMIAL","SC.POISSON","SC.NEGATIVEBINOMIAL","SC.GEOMETRIC","SC.EMPIRICAL","SC.EMPIRICALCONTINUOUS","SC.OUTPUT"],
        "psi_import":{"core_numeric_signatures":psi_signatures.len(),"output_marker":"zero-argument PsiOutput in an additive formula or alone maps to SC.OUTPUT","signatures":psi_signatures,"top_level_declarations_only":true,"nested_and_property_calls":"preserved as unsupported","outside_policy_default":"solvercraft_nominal","outside_policies":["solvercraft_nominal","preserve_vendor"],"vendor_display_context_preserved":false,"property_modifiers":false,"ordinary_cell_policy":"SC finite analytic mean; explicit labelled median fallback for undefined/unavailable means","cumul_subset":"unambiguous strictly increasing knots; no endpoint atoms"},
        "reports":{"answer":true,"lp_checked_basis_shadow_prices_and_intervals":true,"simultaneous_parameter_changes":"must revalidate basis"},
        "worksheet_statistics":{"SC.MEAN":"explicit finite sample range","SC.QUANTILE":"empirical lower inverse CDF","SC.CVAR":"fractional upper loss tail"},
        "saved_models":{"schema_version":1,"xlsx_part":"xl/solvercraft/models.json","binding_migration":"hidden formula names; invalidated bindings fail explicitly"},
        "time_and_random":{"NOW_TODAY":"fixed UTC snapshot serial","deterministic_RAND":"unsupported; declare SC stochastic cells for simulation","background_default_memory_bytes":67108864_u64,"background_reserved_memory_limit_bytes":2147483648_u64},
        "unsupported_references": ["3D", "external workbooks", "dynamic references", "spills"],
        "formula_semantics_limits": ["Optimization MIN/MAX over blank referenced cells is explicitly rejected", "Simulation uses ordered Calc value semantics; array-producing cells, spills and unlisted functions are rejected"],
        "run_manifest":{"schema_version":1,"seed":"lossless decimal string","identity":"diagnostic configuration record; not cryptographic attestation","simulation_rerun":"preview.request"},
        "optimization_note": "The structural compiler and chosen engine may reject expressions outside their mathematical capabilities."
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Binding {
    sheet: String,
    cell: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Snapshot {
    document_uid: String,
    revision: String,
}

struct Prepared {
    model: Option<OptimizationModel>,
    lot_sizing: Option<lot_sizing::LotSizing>,
    risk_cap: Option<portfolio::RiskCap>,
    expression: Option<ExpressionModel>,
    bindings: Vec<(usize, CellRef)>,
    names: Vec<Binding>,
    options: SolveOptions,
}

impl Prepared {
    fn has_auxiliaries(&self) -> bool {
        self.model.as_ref().zip(self.expression.as_ref()).is_some_and(|(model, source)| model.variables.len() > source.variables.len())
    }
}

fn snapshot(s: &Session) -> Result<Snapshot> {
    let doc = s.doc()?;
    Ok(Snapshot { document_uid: doc.uid.to_string(), revision: doc.revision.to_string() })
}
/// Workbook active-tab state can change through a shared Arc without a new
/// revision. Capture it on admission, before a native thread can observe it.
fn pin_request_sheet(wb: &Workbook, request: &mut Json, kind: &str) -> Result<()> {
    if kind == "optimization" && request.get("model").is_some() {
        return Ok(());
    }
    let sheet = match request.get("sheet") {
        None | Some(Json::Null) => wb.active().ok_or(EngineError::NoDocument)?,
        Some(Json::String(name)) => {
            wb.sheet(wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "The model sheet no longer exists"))?).ok_or(EngineError::NoDocument)?
        }
        Some(_) => return Err(bad(SOLVE, "sheet must be a worksheet name")),
    };
    request["sheet"] = json!(sheet.name);
    Ok(())
}

fn options(p: &Json) -> Result<SolveOptions> {
    let mut opt: SolveOptions = serde_json::from_value(p.get("options").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    if let Some(x) = p.get("maxEvaluations") {
        let n =
            x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok())).ok_or_else(|| bad(SOLVE, "maxEvaluations must be a positive integer"))?;
        opt.max_evaluations = n;
    }
    if let Some(x) = str_param(p, "method").filter(|x| !x.is_empty()) {
        opt.method = serde_json::from_value(json!(x)).map_err(sdk_error)?;
    }
    Ok(opt)
}
fn snapshot_time(p: &Json) -> Result<f64> {
    match p.get("_snapshot_time_serial") {
        None => Ok(gridcraft_calc::now_serial()),
        Some(value) => value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(SOLVE, "Snapshot time must be a finite1900-system serial")),
    }
}

fn prepare(wb: &Workbook, p: &Json) -> Result<Prepared> {
    if !p.is_object() {
        return Err(bad(SOLVE, "Solvercraft parameters must be an object"));
    }
    let opt = options(p)?;
    if let Some(model) = p.get("model") {
        let model: OptimizationModel = serde_json::from_value(model.clone()).map_err(sdk_error)?;
        // Compilation/validation also happens in solve; validation must not execute the solver.
        ::solvercraft::model::validate(&model, &opt).map_err(sdk_error)?;
        return Ok(Prepared {
            model: Some(model),
            lot_sizing: None,
            risk_cap: None,
            expression: None,
            bindings: Vec::new(),
            names: Vec::new(),
            options: opt,
        });
    }
    let base = match str_param(p, "sheet") {
        Some(name) => wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "The model sheet no longer exists"))?,
        None => wb.active_sheet(),
    };
    let nonnegative = p.get("nonNegative").and_then(Json::as_bool).unwrap_or(true);
    let source = p.get("variables").ok_or_else(|| bad(SOLVE, "variables is required"))?;
    let (mut vars, bindings) = variable_bindings(wb, base, source)?;
    let mut tr = Translator::new(wb, &bindings);
    tr.frozen_now = snapshot_time(p)?;
    let objective_text = str_param(p, "objective").filter(|x| !x.trim().is_empty()).ok_or_else(|| bad(SOLVE, "objective is required"))?;
    let objective = tr.text(base, objective_text)?;
    let direction = match str_param(p, "goal").unwrap_or("max") {
        "min" | "minimize" => Direction::Minimize,
        "max" | "maximize" => Direction::Maximize,
        "value" => Direction::Minimize,
        _ => return Err(bad(SOLVE, "goal must be min, max or value")),
    };
    let mut cons = Vec::new();
    if let Some(list) = p.get("constraints") {
        let list = list.as_array().ok_or_else(|| bad(SOLVE, "constraints must be an array"))?;
        if list.len() > MAX_CONSTRAINTS {
            return Err(bad(SOLVE, "Too many constraints"));
        }
        for item in list {
            add_constraint(&mut tr, &mut vars, base, item, &mut cons)?;
        }
    }
    let target = if str_param(p, "goal") == Some("value") {
        let target = p
            .get("value")
            .and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|v| v.parse().ok())))
            .filter(|x| x.is_finite())
            .ok_or_else(|| bad(SOLVE, "A finite target value is required"))?;
        Some(target)
    } else {
        None
    };
    // The checkbox applies only to variables without an explicit lower bound.
    // Applying it before constraints would erase legitimate negative bounds.
    if nonnegative {
        for variable in &mut vars {
            if variable.lower.is_none() {
                variable.lower = Some(0.0);
            }
        }
    }
    let mut expr = ExpressionModel {
        name: str_param(p, "name").unwrap_or("Gridcraft model").to_string(),
        variables: vars,
        direction,
        objective,
        constraints: cons,
    };
    if let Some(target) = target {
        if expr.constraints.len() >= MAX_CONSTRAINTS {
            return Err(bad(SOLVE, "Target equality exceeds the host constraint budget"));
        }
        // Count the extra objective copy, constant and row metadata against
        // this caller's limits before materializing the exact equality.
        ::solvercraft::compile::preflight_target_named(&expr, target, "Target value", &opt).map_err(sdk_error)?;
        expr.constraints.push(ExpressionConstraint {
            name: "Target value".into(),
            left: expr.objective.clone(),
            relation: Relation::Equal,
            right: Expr::constant(target),
        });
    }
    // Structural compiler failure only permits an explicitly supported
    // expression backend. Other errors retain their original failure meaning.
    let lot_sizing = lot_sizing::recognize(&expr, &opt);
    let risk_cap = portfolio::recognize_risk_cap(&expr, &opt);
    let model = if lot_sizing.is_some() || risk_cap.is_some() {
        ::solvercraft::model::validate(&bounds_model(&expr), &opt).map_err(sdk_error)?;
        None
    } else {
        match ::solvercraft::compile::compile(&expr, &opt) {
            Ok(model) => Some(model),
            Err(e)
                if e.code == "numeric_range"
                    && p.get("extended_affine").and_then(Json::as_bool) == Some(true)
                    && matches!(opt.method, SolveMethod::Auto | SolveMethod::Simplex | SolveMethod::BranchAndBound) =>
            {
                Some(extended_affine::compile(&expr, &opt)?)
            }
            Err(e)
                if matches!(e.code.as_str(), "unsupported_expression" | "unsupported_constraint")
                    && (opt.method == SolveMethod::DifferentialEvolution
                        || (matches!(opt.method, SolveMethod::Auto | SolveMethod::ProjectedGradient)
                            && expr.variables.iter().all(|v| v.kind == VariableKind::Continuous))) =>
            {
                ::solvercraft::model::validate(&bounds_model(&expr), &opt).map_err(sdk_error)?;
                None
            }
            Err(e) => return Err(sdk_error(e)),
        }
    };
    let names = bindings
        .iter()
        .map(|(si, c)| Ok(Binding { sheet: wb.sheet(*si).ok_or(EngineError::NoDocument)?.name.clone(), cell: c.a1() }))
        .collect::<Result<Vec<_>>>()?;
    Ok(Prepared { model, lot_sizing, risk_cap, expression: Some(expr), bindings, names, options: opt })
}

fn variable_bindings(wb: &Workbook, base: usize, source: &Json) -> Result<VariableBindings> {
    let mut bindings = Vec::new();
    let mut vars = Vec::new();
    for item in variable_entries(source)? {
        let text =
            item.as_str().or_else(|| item.get("cell").and_then(Json::as_str)).ok_or_else(|| bad(SOLVE, "Each variable needs a cell or range"))?;
        for (sheet, cell) in cells(wb, base, text, MAX_BINDINGS)? {
            if bindings.contains(&(sheet, cell)) {
                return Err(bad(SOLVE, "Variable cells must not overlap"));
            }
            if bindings.len() >= MAX_BINDINGS {
                return Err(bad(SOLVE, "Too many variable cells"));
            }
            let sh = wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
            if sh.spill_anchor(cell).is_some() {
                return Err(bad(SOLVE, "A variable cannot be a derived spill cell; choose an independent constant input"));
            }
            if sh.cell(cell).is_some_and(|c| c.formula.is_some()) {
                return Err(bad(SOLVE, "Variable cells must contain constants, not formulas"));
            }
            numeric(&sh.value(cell))?;
            let lower = finite_option(&item, "lower")?;
            let upper = finite_option(&item, "upper")?;
            let kind = match item.get("kind").and_then(Json::as_str).unwrap_or("continuous") {
                "continuous" => VariableKind::Continuous,
                "integer" | "int" => VariableKind::Integer,
                "binary" | "bin" => VariableKind::Binary,
                _ => {
                    return Err(bad(SOLVE, "Variable kind must be continuous, integer or binary"));
                }
            };
            vars.push(Variable { name: binding_name(wb, sheet, cell)?, lower, upper, kind });
            bindings.push((sheet, cell));
        }
    }
    if bindings.is_empty() {
        return Err(bad(SOLVE, "At least one variable cell is required"));
    }
    Ok((vars, bindings))
}

fn variable_entries(source: &Json) -> Result<Vec<Json>> {
    match source {
        Json::String(s) => Ok(vec![json!(s)]),
        Json::Array(v) if v.len() <= MAX_BINDINGS => Ok(v.clone()),
        _ => Err(bad(SOLVE, "variables must be a reference string or an array of references/objects")),
    }
}
fn finite_option(item: &Json, key: &str) -> Result<Option<f64>> {
    match item.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(x) => x.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(SOLVE, format!("{key} must be finite or null"))),
    }
}
fn binding_name(wb: &Workbook, sheet: usize, cell: CellRef) -> Result<String> {
    let sh = wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
    Ok(format!("{}!{}", gridcraft_formula::quote_sheet(&sh.name), cell.a1()))
}
fn parse(text: &str) -> Result<FormulaExpr> {
    gridcraft_formula::parse(text.trim().trim_start_matches('=')).map_err(|e| bad(SOLVE, format!("Invalid expression: {e:?}")))
}
fn reference_sheet(wb: &Workbook, base: usize, r: &Reference) -> Result<usize> {
    match &r.sheet {
        SheetSel::Current => Ok(base),
        SheetSel::Named(name) => wb.sheet_index(name).ok_or_else(|| bad(SOLVE, format!("Unknown sheet: {name}"))),
        SheetSel::Span(..) => Err(bad(SOLVE, "3-D references are not supported by Solvercraft")),
    }
}
fn cells(wb: &Workbook, base: usize, text: &str, cap: usize) -> Result<Vec<(usize, CellRef)>> {
    let expr = gridcraft_formula::parse_name(text.trim()).map_err(|error| bad(SOLVE, format!("Invalid cell reference: {error:?}")))?;
    let mut out = Vec::new();
    fn collect(wb: &Workbook, base: usize, expr: &FormulaExpr, cap: usize, depth: usize, out: &mut Vec<(usize, CellRef)>) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(bad(SOLVE, "Reference nesting limit exceeded"));
        }
        match expr {
            FormulaExpr::Ref(r) => {
                let si = reference_sheet(wb, base, r)?;
                let range = r.range();
                if range.count() > cap.saturating_sub(out.len()) as u64 {
                    return Err(bad(SOLVE, "Reference exceeds the cell budget"));
                }
                out.extend(range.iter().map(|c| (si, c)));
                Ok(())
            }
            FormulaExpr::Binary(BinOp::Union, a, b) => {
                collect(wb, base, a, cap, depth + 1, out)?;
                collect(wb, base, b, cap, depth + 1, out)
            }
            FormulaExpr::Paren(a) => collect(wb, base, a, cap, depth + 1, out),
            FormulaExpr::Name(name) => {
                let (scope, bare) = match gridcraft_core::split_sheet(name) {
                    (Some(sheet), bare) => (wb.sheet_index(&sheet).ok_or_else(|| bad(SOLVE, format!("Unknown sheet: {sheet}")))?, bare),
                    (None, bare) => (base, bare),
                };
                let defined = wb.name(bare, scope).ok_or_else(|| bad(SOLVE, format!("Unknown range name: {name}")))?;
                let expr = gridcraft_formula::parse_name(&defined.formula).map_err(|error| bad(SOLVE, format!("Invalid range name: {error:?}")))?;
                if gridcraft_formula::adjust::has_relative_refs(&expr) {
                    return Err(bad(SOLVE, "Named Solvercraft ranges must use absolute references"));
                }
                collect(wb, defined.scope.unwrap_or(scope), &expr, cap, depth + 1, out)
            }
            _ => Err(bad(SOLVE, "Expected explicit cell references; dynamic references are not supported")),
        }
    }
    collect(wb, base, &expr, cap, 0, &mut out)?;
    Ok(out)
}
fn numeric(v: &Value) -> Result<f64> {
    match v {
        Value::Number(n) if n.is_finite() => Ok(*n),
        Value::Empty => Ok(0.0),
        _ => Err(bad(SOLVE, "The supported formula subset requires finite numeric cells")),
    }
}
fn relation(op: &str) -> Result<Relation> {
    match op {
        "<=" | "≤" => Ok(Relation::LessEqual),
        "=" | "==" => Ok(Relation::Equal),
        ">=" | "≥" => Ok(Relation::GreaterEqual),
        _ => Err(bad(SOLVE, "Constraint relation must be <=, = or >=")),
    }
}

fn constraint_parts(item: &Json) -> Result<(String, String, String)> {
    if let Some(text) = item.as_str() {
        let text = text.trim();
        let lower = text.to_ascii_lowercase();
        if lower.ends_with(" int") || lower.ends_with(" bin") {
            let (left, op) = text.rsplit_once(' ').ok_or_else(|| bad(SOLVE, "Invalid integrality constraint"))?;
            return Ok((left.to_string(), op.to_ascii_lowercase(), String::new()));
        }
        for op in ["<=", ">=", "≤", "≥", "="] {
            if let Some((left, right)) = text.split_once(op) {
                return Ok((left.trim().into(), op.into(), right.trim().into()));
            }
        }
        return Err(bad(SOLVE, "Constraint needs <=, =, >=, int or bin"));
    }
    let left = item.get("lhs").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Constraint needs lhs"))?;
    let op = item.get("op").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Constraint needs op"))?;
    let right = item.get("rhs").map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).unwrap_or_default();
    Ok((left.into(), op.into(), right))
}
fn expression_shape(expr: &FormulaExpr) -> Result<(usize, usize)> {
    match expr {
        FormulaExpr::Ref(reference) => {
            let range = reference.range();
            Ok((range.height() as usize, range.width() as usize))
        }
        FormulaExpr::Array(rows) => {
            let width = rows.first().map_or(0, Vec::len);
            if rows.iter().any(|r| r.len() != width) {
                return Err(bad(SOLVE, "Ragged array is not supported"));
            }
            Ok((rows.len(), width))
        }
        FormulaExpr::Paren(value) => expression_shape(value),
        _ => Ok((1, 1)),
    }
}
fn numeric_values(value: &Value) -> Result<Vec<f64>> {
    match value {
        Value::Array(array) => array.data.iter().map(numeric).collect(),
        value => Ok(vec![numeric(value)?]),
    }
}

fn add_constraint(tr: &mut Translator<'_>, vars: &mut [Variable], base: usize, item: &Json, out: &mut Vec<ExpressionConstraint>) -> Result<()> {
    let (lhs, op, rhs) = constraint_parts(item)?;
    if matches!(op.as_str(), "int" | "bin") {
        for key in cells(tr.wb, base, &lhs, MAX_BINDINGS)? {
            let index = *tr.bindings.get(&key).ok_or_else(|| bad(SOLVE, "Integrality applies only to a declared variable cell"))?;
            let v = vars.get_mut(index).ok_or_else(|| bad(SOLVE, "Invalid variable index"))?;
            v.kind = if op == "bin" { VariableKind::Binary } else { VariableKind::Integer };
        }
        return Ok(());
    }
    let rel = relation(&op)?;
    let left_expr = parse(&lhs)?;
    let right_expr = parse(&rhs)?;
    let left_shape = expression_shape(&left_expr)?;
    let right_shape = expression_shape(&right_expr)?;
    if left_shape != right_shape && right_shape != (1, 1) {
        return Err(bad(SOLVE, "Constraint sides must have matching shapes or a scalar right side"));
    }
    let left = tr.expanded(base, &left_expr, 0)?;
    let right = tr.expanded(base, &right_expr, 0)?;
    if left.len() != right.len() && right.len() != 1 {
        return Err(bad(SOLVE, "Constraint sides must have matching shapes or a scalar right side"));
    }
    if out.len().saturating_add(left.len()) > MAX_CONSTRAINTS {
        return Err(bad(SOLVE, "Too many expanded constraints"));
    }
    for (i, a) in left.into_iter().enumerate() {
        let b = right.get(if right.len() == 1 { 0 } else { i }).ok_or_else(|| bad(SOLVE, "Empty constraint right side"))?.clone();
        if let Expr::Variable { index } = &a
            && let Ok(value) = ::solvercraft::compile::evaluate(&b, &[])
        {
            let v = vars.get_mut(*index).ok_or_else(|| bad(SOLVE, "Invalid bound variable"))?;
            if matches!(rel, Relation::GreaterEqual | Relation::Equal) {
                v.lower = Some(v.lower.map_or(value, |x| x.max(value)));
            }
            if matches!(rel, Relation::LessEqual | Relation::Equal) {
                v.upper = Some(v.upper.map_or(value, |x| x.min(value)));
            }
            continue;
        }
        out.push(ExpressionConstraint { name: format!("Constraint {}", out.len() + 1), left: a, relation: rel, right: b });
    }
    Ok(())
}

struct Translator<'a> {
    wb: &'a Workbook,
    bindings: HashMap<(usize, CellRef), usize>,
    visiting: HashSet<(usize, CellRef)>,
    nodes: usize,
    frozen_now: f64,
}
impl<'a> Translator<'a> {
    fn new(wb: &'a Workbook, bindings: &[(usize, CellRef)]) -> Self {
        Self {
            wb,
            bindings: bindings.iter().enumerate().map(|(i, k)| (*k, i)).collect(),
            visiting: HashSet::new(),
            nodes: 0,
            frozen_now: gridcraft_calc::now_serial(),
        }
    }
    fn enter(&mut self, depth: usize) -> Result<()> {
        self.nodes = self.nodes.saturating_add(1);
        if depth > MAX_DEPTH || self.nodes > MAX_NODES { Err(bad(SOLVE, "Formula expansion exceeds the depth/node budget")) } else { Ok(()) }
    }
    fn text(&mut self, base: usize, text: &str) -> Result<Expr> {
        self.expr(base, &parse(text)?, 0)
    }
    fn cell(&mut self, sheet: usize, cell: CellRef, depth: usize) -> Result<Expr> {
        self.enter(depth)?;
        let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
        if sh.spill_anchor(cell).is_some() {
            return Err(bad(SOLVE, "Derived spill references are not supported; their cached values are not independent constants"));
        }
        if let Some(index) = self.bindings.get(&(sheet, cell)) {
            return Ok(Expr::variable(*index));
        }
        if !self.visiting.insert((sheet, cell)) {
            return Err(bad(SOLVE, "Circular formula reference"));
        }
        let result = if let Some(formula) = sh.cell(cell).and_then(|c| c.formula.as_ref()) {
            let expr = formula.expr_arc().ok_or_else(|| bad(SOLVE, "A referenced formula cannot be parsed"))?;
            // Bond prices often include face-value scaling around PRICE.
            // Fold the full cell only after proving it is decision-independent.
            let text = formula.text.to_ascii_uppercase();
            if (text.contains("PRICE") || text.contains("DURATION")) && self.static_bond_expression(sheet, &expr, depth + 1)? {
                self.evaluate_fixed(sheet, &expr)
            } else {
                self.expr(sheet, &expr, depth + 1)
            }
        } else {
            numeric(&sh.value(cell)).map(Expr::constant)
        };
        self.visiting.remove(&(sheet, cell));
        result
    }
    fn expanded(&mut self, base: usize, e: &FormulaExpr, depth: usize) -> Result<Vec<Expr>> {
        self.enter(depth)?;
        match e {
            FormulaExpr::Name(name) => {
                let (scope, expression) = self.defined_range(base, name)?;
                self.expanded(scope, &expression, depth + 1)
            }
            FormulaExpr::Ref(r) => {
                let si = reference_sheet(self.wb, base, r)?;
                let range = r.range();
                if range.count() > MAX_RANGE_CELLS {
                    return Err(bad(SOLVE, "Range exceeds the Solvercraft expansion budget"));
                }
                range.iter().map(|c| self.cell(si, c, depth + 1)).collect()
            }
            FormulaExpr::Array(rows) => {
                if rows.iter().map(Vec::len).sum::<usize>() > MAX_RANGE_CELLS as usize {
                    return Err(bad(SOLVE, "Array exceeds the expansion budget"));
                }
                rows.iter().flatten().map(|e| self.expr(base, e, depth + 1)).collect()
            }
            FormulaExpr::Paren(a) => self.expanded(base, a, depth + 1),
            _ => Ok(vec![self.expr(base, e, depth + 1)?]),
        }
    }
    fn extremum_argument(&mut self, base: usize, e: &FormulaExpr, depth: usize) -> Result<Vec<Expr>> {
        self.enter(depth)?;
        match e {
            FormulaExpr::Paren(value) => return self.extremum_argument(base, value, depth + 1),
            FormulaExpr::Name(name) => {
                let defined = self.wb.name(name, base).ok_or_else(|| bad(SOLVE, format!("Unknown name: {name}")))?;
                let expression = parse(&defined.formula)?;
                return self.extremum_argument(base, &expression, depth + 1);
            }
            FormulaExpr::Ref(reference) => {
                let sheet = reference_sheet(self.wb, base, reference)?;
                let range = reference.range();
                if range.count() > MAX_RANGE_CELLS {
                    return Err(bad(SOLVE, "Range exceeds the Solvercraft expansion budget"));
                }
                let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
                for cell in range.iter() {
                    if !self.bindings.contains_key(&(sheet, cell))
                        && sh.cell(cell).is_none_or(|stored| stored.formula.is_none())
                        && matches!(sh.value(cell), Value::Empty)
                    {
                        return Err(bad(
                            SOLVE,
                            "MIN/MAX over blank referenced cells is not yet supported; blank references must not be coerced to zero",
                        ));
                    }
                }
            }
            _ => {}
        }
        self.expanded(base, e, depth)
    }
    fn expr(&mut self, base: usize, e: &FormulaExpr, depth: usize) -> Result<Expr> {
        self.enter(depth)?;
        match e {
            FormulaExpr::Number(value) if value.is_finite() => Ok(Expr::constant(*value)),
            FormulaExpr::Ref(r) if r.range().is_single() => self.cell(reference_sheet(self.wb, base, r)?, r.range().start, depth + 1),
            FormulaExpr::Name(name) => {
                let n = self.wb.name(name, base).ok_or_else(|| bad(SOLVE, format!("Unknown name: {name}")))?;
                let text = n.formula.clone();
                self.expr(base, &parse(&text)?, depth + 1)
            }
            FormulaExpr::Paren(x) => self.expr(base, x, depth + 1),
            FormulaExpr::Unary(UnOp::Plus, x) => self.expr(base, x, depth + 1),
            FormulaExpr::Unary(UnOp::Neg, x) => Ok(Expr::Negate { value: Box::new(self.expr(base, x, depth + 1)?) }),
            FormulaExpr::Unary(UnOp::Percent, x) => {
                Ok(Expr::Multiply { left: Box::new(self.expr(base, x, depth + 1)?), right: Box::new(Expr::constant(0.01)) })
            }
            FormulaExpr::Binary(op, a, b) => {
                let left = Box::new(self.expr(base, a, depth + 1)?);
                if *op == BinOp::Pow {
                    let FormulaExpr::Number(exponent) = b.as_ref() else {
                        return Err(bad(SOLVE, "Only constant nonnegative integer powers are supported"));
                    };
                    if !exponent.is_finite() || *exponent < 0.0 || exponent.fract() != 0.0 || *exponent > 32.0 {
                        return Err(bad(SOLVE, "Power exponent must be an integer from 0 to 32"));
                    }
                    return Ok(Expr::Power { base: left, exponent: *exponent as u32 });
                }
                let right = Box::new(self.expr(base, b, depth + 1)?);
                match op {
                    BinOp::Add => Ok(Expr::Add { left, right }),
                    BinOp::Sub => Ok(Expr::Subtract { left, right }),
                    BinOp::Mul => Ok(Expr::Multiply { left, right }),
                    BinOp::Div => Ok(Expr::Divide { left, right }),
                    _ => Err(bad(SOLVE, "Unsupported operator in Solvercraft formula")),
                }
            }
            FormulaExpr::Call(name, args) => self.call(base, name, args, depth + 1),
            _ => Err(bad(SOLVE, "Unsupported formula construct; no cached-value or numerical-linearity substitution was made")),
        }
    }
    fn evaluate_fixed(&self, base: usize, expression: &FormulaExpr) -> Result<Expr> {
        let formula = gridcraft_formula::print(expression);
        numeric(&gridcraft_calc::evaluate_at(self.wb, base, CellRef::default(), &formula, self.frozen_now)).map(Expr::constant)
    }

    /// Admit a bounded, scalar, deterministic subset. Stored formula values,
    /// spills and decision-dependent inputs cannot be frozen.
    fn static_bond_expression(&mut self, base: usize, expr: &FormulaExpr, depth: usize) -> Result<bool> {
        self.enter(depth)?;
        Ok(match expr {
            FormulaExpr::Number(value) => value.is_finite(),
            FormulaExpr::Paren(inner) | FormulaExpr::Unary(UnOp::Neg | UnOp::Plus | UnOp::Percent, inner) => {
                self.static_bond_expression(base, inner, depth + 1)?
            }
            FormulaExpr::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div, left, right) => {
                self.static_bond_expression(base, left, depth + 1)? && self.static_bond_expression(base, right, depth + 1)?
            }
            FormulaExpr::Ref(reference) if reference.range().is_single() => {
                let sheet = reference_sheet(self.wb, base, reference)?;
                let cell = reference.range().start;
                let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
                !self.bindings.contains_key(&(sheet, cell))
                    && sh.spill_anchor(cell).is_none()
                    && sh.cell(cell).is_some_and(|value| value.formula.is_none() && matches!(value.value, Value::Number(n) if n.is_finite()))
            }
            FormulaExpr::Call(name, args) if matches!(name.as_str(), "DATE" | "PRICE" | "DURATION") => {
                for arg in args {
                    if !self.static_bond_expression(base, arg, depth + 1)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => false,
        })
    }

    /// Freeze an NPV only after proving that all its inputs are independent
    /// numeric literals or stored cells. Calc supplies the workbook semantics.
    fn fixed_npv(&mut self, base: usize, args: &[FormulaExpr], depth: usize) -> Result<Expr> {
        for arg in args {
            self.enter(depth)?;
            match arg {
                FormulaExpr::Number(number) if number.is_finite() => {}
                FormulaExpr::Ref(reference) => {
                    let sheet = reference_sheet(self.wb, base, reference)?;
                    let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
                    let range = reference.range();
                    if range.count() > MAX_RANGE_CELLS {
                        return Err(bad(SOLVE, "NPV range exceeds the expansion budget"));
                    }
                    for cell in range.iter() {
                        self.enter(depth)?;
                        if self.bindings.contains_key(&(sheet, cell))
                            || sh.spill_anchor(cell).is_some()
                            || sh.cell(cell).is_some_and(|value| value.formula.is_some())
                            || !matches!(sh.value(cell), Value::Number(n) if n.is_finite())
                        {
                            return Err(bad(SOLVE, "NPV requires fixed numeric inputs in optimization models"));
                        }
                    }
                }
                _ => return Err(bad(SOLVE, "NPV requires fixed numeric inputs in optimization models")),
            }
        }
        self.evaluate_fixed(base, &FormulaExpr::Call("NPV".into(), args.to_vec()))
    }
    fn call(&mut self, base: usize, name: &str, args: &[FormulaExpr], depth: usize) -> Result<Expr> {
        self.enter(depth)?;
        match name {
            "NOW" | "TODAY" if args.is_empty() => {
                let time = gridcraft_core::date::serial_in_system(self.wb.date_system, self.frozen_now);
                Ok(Expr::constant(if name == "TODAY" { time.floor() } else { time }))
            }
            "SC.OUTPUT" if (1..=2).contains(&args.len()) => {
                self.expr(base, args.first().ok_or_else(|| bad(SOLVE, "SC.OUTPUT requires a value"))?, depth + 1)
            }
            "PRICE" | "DURATION" => {
                let expression = FormulaExpr::Call(name.into(), args.to_vec());
                if !self.static_bond_expression(base, &expression, depth + 1)? {
                    return Err(bad(SOLVE, "Bond functions require fixed numeric inputs in optimization models"));
                }
                self.evaluate_fixed(base, &expression)
            }
            "NPV" if args.len() >= 2 => self.fixed_npv(base, args, depth + 1),
            "AVERAGE" | "VARP" | "VAR.P" | "VAR" | "VAR.S" | "COVAR" | "COVARIANCE.P" | "INDEX" => self.fixed_statistics(base, name, args, depth + 1),
            "QUADPRODUCT" | "_XLL.QUADPRODUCT" => self.quadratic_product(base, args, depth + 1),
            "SUM" => {
                let mut terms = Vec::new();
                for arg in args {
                    terms.extend(self.expanded(base, arg, depth + 1)?);
                }
                Ok(sum(&terms))
            }
            "SUMPRODUCT" => {
                if args.is_empty() {
                    return Err(bad(SOLVE, "SUMPRODUCT requires arguments"));
                }
                if args.len() > 32 {
                    return Err(bad(SOLVE, "SUMPRODUCT exceeds the 32-array budget"));
                }
                let shape = self.aggregate_shape(base, &args[0], depth + 1)?;
                for arg in args {
                    if self.aggregate_shape(base, arg, depth + 1)? != shape {
                        return Err(bad(SOLVE, "SUMPRODUCT argument shapes must match"));
                    }
                }
                let columns = args.iter().map(|a| self.expanded(base, a, depth + 1)).collect::<Result<Vec<_>>>()?;
                let len = columns.first().map_or(0, Vec::len);
                if columns.iter().any(|v| v.len() != len) {
                    return Err(bad(SOLVE, "SUMPRODUCT argument shapes must match"));
                }
                let mut terms = Vec::with_capacity(len);
                for i in 0..len {
                    let mut product = Expr::constant(1.0);
                    for column in &columns {
                        let value = column.get(i).ok_or_else(|| bad(SOLVE, "Invalid SUMPRODUCT shape"))?;
                        product = Expr::Multiply { left: Box::new(product), right: Box::new(value.clone()) };
                    }
                    terms.push(product);
                }
                Ok(sum(&terms))
            }
            "SIN" | "COS" | "EXP" | "LN" | "SQRT" | "ABS" if args.len() == 1 => {
                let value = Box::new(self.expr(base, args.first().ok_or_else(|| bad(SOLVE, "Missing function argument"))?, depth + 1)?);
                match name {
                    "SIN" => Ok(Expr::Sin { value }),
                    "COS" => Ok(Expr::Cos { value }),
                    "EXP" => Ok(Expr::Exp { value }),
                    "LN" => Ok(Expr::Ln { value }),
                    "SQRT" => Ok(Expr::Sqrt { value }),
                    _ => Ok(Expr::Abs { value }),
                }
            }
            "MIN" | "MAX" => {
                let mut terms = Vec::new();
                for arg in args {
                    terms.extend(self.extremum_argument(base, arg, depth + 1)?);
                }
                if terms.is_empty() {
                    return Err(bad(SOLVE, "MIN/MAX requires values"));
                }
                Ok(extremum(&terms, name == "MIN"))
            }
            _ => Err(bad(SOLVE, format!("Function {name} is outside the supported structural subset"))),
        }
    }
}
fn sum(terms: &[Expr]) -> Expr {
    match terms {
        [] => Expr::constant(0.0),
        [only] => only.clone(),
        _ => {
            let mid = terms.len() / 2;
            Expr::Add { left: Box::new(sum(&terms[..mid])), right: Box::new(sum(&terms[mid..])) }
        }
    }
}
fn extremum(terms: &[Expr], minimum: bool) -> Expr {
    match terms {
        [] => Expr::constant(0.0),
        [only] => only.clone(),
        _ => {
            let mid = terms.len() / 2;
            let left = Box::new(extremum(&terms[..mid], minimum));
            let right = Box::new(extremum(&terms[mid..], minimum));
            if minimum { Expr::Minimum { left, right } } else { Expr::Maximum { left, right } }
        }
    }
}

fn bounds_model(expr: &ExpressionModel) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: expr.name.clone(),
        variables: expr.variables.clone(),
        objective: Objective { direction: expr.direction, offset: 0.0, linear: Vec::new(), quadratic: Vec::new() },
        constraints: Vec::new(),
    }
}
fn validate_candidate(prepared: &Prepared, values: &[f64]) -> Result<()> {
    let bounds;
    let model = if let Some(model) = prepared.model.as_ref().filter(|_| !prepared.has_auxiliaries()) {
        model
    } else if let Some(expr) = prepared.expression.as_ref() {
        bounds = bounds_model(expr);
        &bounds
    } else {
        return Err(bad(SOLVE, "Missing original model"));
    };
    let checked = ::solvercraft::model::validate_solution(model, values, &prepared.options).map_err(sdk_error)?;
    if !checked.feasible {
        return Err(bad(SOLVE, "Result values violate the original model"));
    }
    if let Some(expr) = prepared.expression.as_ref() {
        ::solvercraft::compile::evaluate(&expr.objective, values).map_err(sdk_error)?;
        for row in &expr.constraints {
            let left = ::solvercraft::compile::evaluate(&row.left, values).map_err(sdk_error)?;
            let right = ::solvercraft::compile::evaluate(&row.right, values).map_err(sdk_error)?;
            let violation = match row.relation {
                Relation::Equal => (left - right).abs(),
                Relation::LessEqual => (left - right).max(0.0),
                Relation::GreaterEqual => (right - left).max(0.0),
            };
            if !violation.is_finite() || violation > prepared.options.feasibility_tolerance {
                return Err(bad(SOLVE, "Result values violate an original expression constraint"));
            }
        }
    }
    Ok(())
}
fn rounded_candidate(prepared: &Prepared, values: &[f64]) -> Result<Vec<f64>> {
    let variables = prepared
        .expression
        .as_ref()
        .map(|m| m.variables.as_slice())
        .or_else(|| prepared.model.as_ref().map(|m| m.variables.as_slice()))
        .ok_or_else(|| bad(SOLVE, "Original variables are missing"))?;
    if variables.len() != values.len() {
        return Err(bad(SOLVE, "Result dimension differs from original variables"));
    }
    variables
        .iter()
        .zip(values)
        .map(|(variable, value)| {
            if !value.is_finite() {
                return Err(bad(SOLVE, "Result values must be finite"));
            }
            if variable.kind == VariableKind::Continuous {
                return Ok(*value);
            }
            if value.abs() >= ::solvercraft::model::INTEGER_PRECISION_LIMIT {
                return Err(bad(SOLVE, "Integer candidate magnitude must be below2^53"));
            }
            let rounded = value.round();
            if (value - rounded).abs() > prepared.options.integrality_tolerance {
                return Err(bad(SOLVE, "Candidate exceeds the original integer tolerance"));
            }
            Ok(rounded)
        })
        .collect()
}
fn validate(s: &mut Session, p: &Json) -> Result<Json> {
    let prepared = prepare(&s.doc()?.wb, p)?;
    let class = match prepared.model.as_ref() {
        Some(_) if prepared.has_auxiliaries() => "extended_affine",
        Some(model) if model.objective.quadratic.is_empty() => "linear",
        Some(_) => "quadratic",
        None if prepared.lot_sizing.is_some() => "separable_lot_sizing",
        None if prepared.risk_cap.is_some() => "simplex_risk_cap",
        None => "nonlinear_expression",
    };
    Ok(json!({"model":prepared.model,"expression_model":prepared.expression,"bindings":prepared.names,"class":class,"snapshot":snapshot(s)?}))
}
fn solve(s: &mut Session, p: &Json) -> Result<Json> {
    let doc = s.doc()?;
    solve_snapshot(&doc.wb, &doc.calc, snapshot(s)?, p, std::sync::Arc::new(NoControl))
}
fn solve_snapshot(
    wb: &Workbook,
    calc: &gridcraft_calc::Calc,
    snapshot: Snapshot,
    p: &Json,
    control: std::sync::Arc<dyn ::solvercraft::types::RunControl>,
) -> Result<Json> {
    if !p.is_object() {
        return Err(bad(SOLVE, "Solvercraft parameters must be an object"));
    }
    manifest::fingerprint(p)?;
    let mut request = p.clone();
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(calc.fixed_now.unwrap_or_else(gridcraft_calc::now_serial));
    }
    pin_request_sheet(wb, &mut request, "optimization")?;
    let prepared = prepare(wb, &request)?;
    request["options"] = json!(prepared.options);
    let mut result = solve_prepared(wb, &request, &prepared, control)?;
    if prepared.has_auxiliaries() {
        project_extended_result(&prepared, &mut result)?;
    }
    let mut apply_values = None;
    if let Some(values) = result.values.as_ref()
        && result.feasibility == Feasibility::Feasible
    {
        validate_candidate(&prepared, values)?;
        let rounded = rounded_candidate(&prepared, values)?;
        validate_candidate(&prepared, &rounded)?;
        verify_workbook_snapshot(wb, calc, &request, &prepared, &rounded)?;
        apply_values = Some(rounded);
    }
    let initial_values = prepared.bindings.iter().map(|(si, cell)| wb.sheet(*si).and_then(|sh| numeric(&sh.value(*cell)).ok())).collect::<Vec<_>>();
    let nlp: ::solvercraft::nonlinear::NlpOptions =
        serde_json::from_value(request.get("nlp").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    let manifest = manifest::create(
        "optimization",
        &snapshot,
        &request,
        json!({"solve":prepared.options,"nonlinear":nlp}),
        json!({"engine":result.engine,"formula_evaluator":"structural-compiler-with-original-Calc-candidate-guard-v1"}),
        json!({"variables":prepared.names}),
    )?;
    Ok(
        json!({"result":result,"apply_values":apply_values,"initial_values":initial_values,"request":request,"snapshot":snapshot,"bindings":prepared.names,"manifest":manifest,"execution":"synchronous_bounded"}),
    )
}
fn solve_prepared(
    wb: &Workbook,
    request: &Json,
    prepared: &Prepared,
    control: std::sync::Arc<dyn ::solvercraft::types::RunControl>,
) -> Result<SolveResult> {
    if let (Some(route), Some(expression)) = (&prepared.lot_sizing, &prepared.expression) {
        return Ok(route.solve(expression, &prepared.options, control.as_ref()));
    }
    if let (Some(route), Some(expression)) = (&prepared.risk_cap, &prepared.expression) {
        return Ok(route.solve(expression, &prepared.options, control.as_ref()));
    }
    if let Some(model) = prepared.model.as_ref()
        && prepared.expression.is_some()
        && !model.objective.quadratic.is_empty()
        && matches!(prepared.options.method, SolveMethod::Auto | SolveMethod::Simplex)
        && let Some(result) = portfolio::solve_simplex_quadratic(model, &prepared.options, control.as_ref())?
        && (prepared.options.method == SolveMethod::Simplex
            || result.optimality == ::solvercraft::types::Optimality::Optimal
            || result.termination != ::solvercraft::types::Termination::Completed)
    {
        return Ok(result);
    }
    let result = match (prepared.expression.as_ref(), prepared.options.method) {
        (Some(expr), SolveMethod::DifferentialEvolution) => {
            ::solvercraft::heuristic::solve(expr, &prepared.options, control.as_ref()).map_err(sdk_error)?
        }
        (Some(expr), SolveMethod::ProjectedGradient) => {
            let start = starting_values(wb, prepared)?;
            solve_nonlinear(expr, &start, &prepared.options, request, control.as_ref())?
        }
        (Some(expr), SolveMethod::Auto) if prepared.model.is_none() => {
            let start = starting_values(wb, prepared)?;
            solve_nonlinear(expr, &start, &prepared.options, request, control.as_ref())?
        }
        _ => {
            let model = prepared.model.as_ref().ok_or_else(|| bad(SOLVE, "This method does not support the expression model"))?;
            match ::solvercraft::engines::solve_owned(model, &prepared.options, std::sync::Arc::clone(&control)) {
                Ok(result) => result,
                Err(e)
                    if matches!(e.code.as_str(), "unsupported_qp" | "unsupported_problem")
                        && !model.objective.quadratic.is_empty()
                        && prepared.options.method == SolveMethod::Auto
                        && prepared.expression.as_ref().is_some_and(|expr| expr.variables.iter().all(|v| v.kind == VariableKind::Continuous)) =>
                {
                    let expr = prepared.expression.as_ref().ok_or_else(|| bad(SOLVE, "Missing expression model"))?;
                    let start = starting_values(wb, prepared)?;
                    solve_nonlinear(expr, &start, &prepared.options, request, control.as_ref())?
                }
                Err(e) => return Err(sdk_error(e)),
            }
        }
    };
    Ok(result)
}

fn project_extended_result(prepared: &Prepared, result: &mut SolveResult) -> Result<()> {
    let expression = prepared.expression.as_ref().ok_or_else(|| bad(SOLVE, "Missing source expression"))?;
    let model = prepared.model.as_ref().ok_or_else(|| bad(SOLVE, "Missing extended model"))?;
    if let Some(values) = &mut result.values {
        if values.len() != model.variables.len() {
            return Err(bad(SOLVE, "Extended solution has an unexpected dimension"));
        }
        values.truncate(expression.variables.len());
        if result.feasibility == Feasibility::Feasible {
            let original = ::solvercraft::compile::evaluate(&expression.objective, values).map_err(sdk_error)?;
            let compiled = result.objective.ok_or_else(|| bad(SOLVE, "Extended solution has no objective"))?;
            let delta = (original - compiled).abs();
            if delta > prepared.options.feasibility_tolerance * (1.0 + original.abs().max(compiled.abs())) {
                return Err(bad(SOLVE, "Extended objective disagrees with the original expression"));
            }
            if delta > prepared.options.optimality_tolerance && result.optimality == ::solvercraft::types::Optimality::Optimal {
                result.optimality = ::solvercraft::types::Optimality::BestFound;
            }
            result.objective = Some(original);
            if let Some(bound) = result.best_bound {
                let outward = match expression.direction {
                    Direction::Minimize => bound.min(original),
                    Direction::Maximize => bound.max(original),
                };
                result.best_bound = Some(outward);
                result.relative_gap = Some((outward - original).abs() / original.abs().max(1.0));
            }
        }
    }
    result.warnings.push("This model uses exact affine auxiliary equalities; displayed values are projected to the original workbook cells.".into());
    Ok(())
}

fn solve_nonlinear(
    expr: &ExpressionModel,
    start: &[f64],
    options: &SolveOptions,
    request: &Json,
    control: &dyn ::solvercraft::types::RunControl,
) -> Result<SolveResult> {
    let nlp: ::solvercraft::nonlinear::NlpOptions =
        serde_json::from_value(request.get("nlp").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    if expr.constraints.is_empty() {
        ::solvercraft::nonlinear::solve_from_with_derivatives(expr, start, options, control, nlp.derivative_mode, nlp.finite_difference_step)
            .map_err(sdk_error)
    } else {
        ::solvercraft::constrained::solve_from(expr, start, options, &nlp, control).map_err(sdk_error)
    }
}
fn starting_values(wb: &Workbook, prepared: &Prepared) -> Result<Vec<f64>> {
    prepared
        .bindings
        .iter()
        .map(|(si, c)| {
            let sh = wb.sheet(*si).ok_or(EngineError::NoDocument)?;
            numeric(&sh.value(*c))
        })
        .collect()
}
fn apply(s: &mut Session, p: &Json) -> Result<Json> {
    let expected: Snapshot =
        serde_json::from_value(p.get("snapshot").cloned().ok_or_else(|| bad(SOLVE, "A solve preview snapshot is required"))?).map_err(sdk_error)?;
    let current = snapshot(s)?;
    if expected.document_uid != current.document_uid || expected.revision != current.revision {
        return Err(bad(SOLVE, "The workbook changed since this preview. Run Solvercraft again before applying."));
    }
    let request = p.get("request").ok_or_else(|| bad(SOLVE, "The original solve request is required"))?;
    let prepared = prepare(&s.doc()?.wb, request)?;
    if prepared.bindings.is_empty() {
        return Err(bad(SOLVE, "A portable model has no workbook bindings to apply"));
    }
    let result: SolveResult =
        serde_json::from_value(p.get("result").cloned().ok_or_else(|| bad(SOLVE, "A solve result is required"))?).map_err(sdk_error)?;
    if result.feasibility != Feasibility::Feasible {
        return Err(bad(SOLVE, "Only a verified feasible solution can be applied"));
    }
    let values = result.values.ok_or_else(|| bad(SOLVE, "The result has no variable values"))?;
    // Revalidation does not trust the received success flag or objective.
    validate_candidate(&prepared, &values)?;
    let values = rounded_candidate(&prepared, &values)?;
    validate_candidate(&prepared, &values)?;
    verify_workbook_objective(s.doc()?, request, &prepared, &values)?;
    edit(s, |cx| {
        for ((si, cell), value) in prepared.bindings.iter().zip(&values) {
            let sh = cx.sheet_mut(*si)?;
            let style = sh.style_id(*cell);
            sh.set_cell(*cell, Cell { value: Value::number(*value), formula: None, style });
            cx.touch(*si, *cell);
        }
        Ok(())
    })?;
    Ok(json!({"applied":true,"cells":prepared.names,"values":values}))
}

/// Check the compiler against Gridcraft's real evaluator on a private snapshot.
/// This does not write the live workbook or create history.
fn verify_workbook_objective(doc: &crate::DocState, request: &Json, prepared: &Prepared, values: &[f64]) -> Result<()> {
    verify_workbook_snapshot(&doc.wb, &doc.calc, request, prepared, values)
}
fn verify_workbook_snapshot(
    source: &Workbook,
    source_calc: &gridcraft_calc::Calc,
    request: &Json,
    prepared: &Prepared,
    values: &[f64],
) -> Result<()> {
    if prepared.bindings.is_empty() {
        return Ok(());
    }
    if values.len() != prepared.bindings.len() {
        return Err(bad(SOLVE, "Result variable count does not match bindings"));
    }
    let mut wb = source.clone();
    let mut changed = Vec::with_capacity(values.len());
    for ((si, cell), value) in prepared.bindings.iter().zip(values) {
        if !value.is_finite() {
            return Err(bad(SOLVE, "Result values must be finite"));
        }
        let sh = wb.sheet_mut(*si).ok_or(EngineError::NoDocument)?;
        let style = sh.style_id(*cell);
        sh.set_cell(*cell, Cell { value: Value::number(*value), formula: None, style });
        changed.push((*si, *cell));
    }
    wb.calc.mode = gridcraft_model::CalcMode::Automatic;
    let mut calc = source_calc.clone();
    let time = snapshot_time(request)?;
    calc.fixed_now = Some(time);
    calc.cells_changed(&mut wb, &changed);
    let sheet = str_param(request, "sheet").and_then(|name| wb.sheet_index(name)).unwrap_or(wb.active_sheet());
    let objective = str_param(request, "objective").ok_or_else(|| bad(SOLVE, "Objective is missing"))?;
    let actual = numeric(&gridcraft_calc::evaluate_at(&wb, sheet, CellRef::new(0, 0), objective, time))?;
    let predicted = match (prepared.model.as_ref().filter(|_| !prepared.has_auxiliaries()), prepared.expression.as_ref()) {
        (Some(model), _) => ::solvercraft::model::evaluate_objective(model, values).map_err(sdk_error)?,
        (_, Some(expr)) => ::solvercraft::compile::evaluate(&expr.objective, values).map_err(sdk_error)?,
        _ => return Err(bad(SOLVE, "The result has no model")),
    };
    let tolerance = prepared.options.feasibility_tolerance * (1.0 + actual.abs().max(predicted.abs()));
    if (actual - predicted).abs() > tolerance {
        return Err(bad(SOLVE, "Solvercraft formula translation disagrees with the workbook evaluator; solution was not applied"));
    }
    if let Some(rows) = request.get("constraints").and_then(Json::as_array) {
        for row in rows {
            let (left, op, right) = constraint_parts(row)?;
            if matches!(op.as_str(), "int" | "bin") {
                continue;
            }
            let rel = relation(&op)?;
            let left = numeric_values(&gridcraft_calc::evaluate_at(&wb, sheet, CellRef::new(0, 0), &left, time))?;
            let right = numeric_values(&gridcraft_calc::evaluate_at(&wb, sheet, CellRef::new(0, 0), &right, time))?;
            if left.len() != right.len() && right.len() != 1 {
                return Err(bad(SOLVE, "Workbook constraint shape changed"));
            }
            for (i, a) in left.iter().enumerate() {
                let b = right.get(if right.len() == 1 { 0 } else { i }).ok_or_else(|| bad(SOLVE, "Empty workbook constraint"))?;
                let violation = match rel {
                    Relation::Equal => (a - b).abs(),
                    Relation::LessEqual => (a - b).max(0.0),
                    Relation::GreaterEqual => (b - a).max(0.0),
                };
                if !violation.is_finite() || violation > prepared.options.feasibility_tolerance {
                    return Err(bad(SOLVE, "The workbook evaluator rejects a constraint; solution was not applied"));
                }
            }
        }
    }
    Ok(())
}

fn simulate(s: &mut Session, p: &Json) -> Result<Json> {
    simulate_snapshot(&s.doc()?.wb, &s.doc()?.calc, snapshot(s)?, p, &NoControl)
}
fn simulate_snapshot(
    wb: &Workbook,
    source_calc: &gridcraft_calc::Calc,
    snapshot: Snapshot,
    p: &Json,
    control: &dyn ::solvercraft::types::RunControl,
) -> Result<Json> {
    if !p.is_object() {
        return Err(bad(SOLVE, "Simulation parameters must be an object"));
    }
    manifest::fingerprint(p)?;
    // Worksheet declarations are stable cells, sampled once per trial. Explicit
    // bindings can override discovery without changing any live formula.
    let mut request = p.clone();
    pin_request_sheet(wb, &mut request, "simulation")?;
    if request.get("_snapshot_time_serial").is_none() {
        request["_snapshot_time_serial"] = json!(gridcraft_calc::now_serial());
    }
    // Refresh volatile dependencies on a private Calc at the one captured time.
    // A parameter read of C1=NOW() and an output read of C1 must agree exactly.
    let mut frozen = wb.clone();
    frozen.calc.mode = gridcraft_model::CalcMode::Automatic;
    let mut calculation = source_calc.clone();
    calculation.fixed_now = Some(snapshot_time(&request)?);
    calculation.cells_changed(&mut frozen, &[]);
    let wb = &frozen;
    if request.get("inputs").is_none() || request.get("outputs").is_none() {
        let (inputs, outputs) = worksheet::discover_at(wb, snapshot_time(&request)?)?;
        if request.get("inputs").is_none() {
            request["inputs"] = json!(inputs);
        }
        if request.get("outputs").is_none() {
            request["outputs"] = json!(outputs);
        }
    }
    let p = &request;
    let base = match str_param(p, "sheet") {
        Some(name) => wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "The model sheet no longer exists"))?,
        None => wb.active_sheet(),
    };
    let inputs = p.get("inputs").and_then(Json::as_array).ok_or_else(|| bad(SOLVE, "inputs must be an array"))?;
    if inputs.is_empty() || inputs.len() > MAX_BINDINGS {
        return Err(bad(SOLVE, "Simulation requires a bounded nonempty input list"));
    }
    let mut bindings = Vec::new();
    let mut distributions = Vec::new();
    let mut worksheet_nominals = Vec::new();
    for input in inputs {
        let text = input.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Each simulation input needs a cell"))?;
        let c = cells(wb, base, text, 1)?.into_iter().next().ok_or_else(|| bad(SOLVE, "Input cell is missing"))?;
        if bindings.contains(&c) {
            return Err(bad(SOLVE, "Simulation input cells must be unique"));
        }
        let sh = wb.sheet(c.0).ok_or(EngineError::NoDocument)?;
        if sh.spill_anchor(c.1).is_some() {
            return Err(bad(SOLVE, "A simulation input cannot be a derived spill cell; choose an independent constant input"));
        }
        let declared = worksheet::input_declaration_at(wb, c.0, c.1, snapshot_time(p)?)?;
        let worksheet_nominal = declared.as_ref().map(|d| json!({"value":d.nominal,"basis":d.nominal_basis.as_str()}));
        if sh.cell(c.1).is_some_and(|c| c.formula.is_some()) && declared.is_none() {
            return Err(bad(SOLVE, "Simulation inputs must be constant cells or SC distribution declarations"));
        }
        let distribution: Distribution = serde_json::from_value(
            input
                .get("distribution")
                .cloned()
                .or_else(|| declared.and_then(|d| serde_json::to_value(d.distribution).ok()))
                .ok_or_else(|| bad(SOLVE, "Input distribution is required"))?,
        )
        .map_err(sdk_error)?;
        bindings.push(c);
        distributions.push(distribution);
        worksheet_nominals.push(worksheet_nominal);
    }
    let outputs = p.get("outputs").ok_or_else(|| bad(SOLVE, "outputs is required"))?;
    let output_entries = match outputs {
        Json::String(s) => vec![s.clone()],
        Json::Array(a) if a.len() <= MAX_BINDINGS => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(SOLVE, "Output references must be strings")))
            .collect::<Result<Vec<_>>>()?,
        _ => return Err(bad(SOLVE, "outputs must be references")),
    };
    let mut output_cells = Vec::new();
    for text in output_entries {
        output_cells.extend(cells(wb, base, &text, MAX_BINDINGS.saturating_sub(output_cells.len()))?);
    }
    if output_cells.is_empty() {
        return Err(bad(SOLVE, "At least one output cell is required"));
    }
    let mut opt: SimulationOptions = serde_json::from_value(p.get("options").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    let requested_options = opt.clone();
    let mut program = simulation_ir::Program::compile(wb, &bindings, &output_cells, snapshot_time(p)?, opt.max_memory_bytes)?;
    program.check_work(opt.trials)?;
    opt.max_memory_bytes =
        opt.max_memory_bytes.checked_sub(program.reserved_bytes()).ok_or_else(|| bad(SOLVE, "Simulation IR exceeds memory budget"))?;
    let mut worksheet_statistics = statistics::Statistics::collect(wb, &output_cells, opt.trials)?;
    let result = ::solvercraft::simulation::simulate(&distributions, &opt, control, |values| {
        let outputs = program.evaluate(values, control)?;
        worksheet_statistics.observe(&outputs);
        Ok(outputs)
    })
    .map_err(sdk_error)?;
    let names = output_cells.iter().map(|(si, c)| binding_name(wb, *si, *c)).collect::<Result<Vec<_>>>()?;
    let worksheet_statistics = worksheet_statistics.finish(wb, &names, &result, opt.trials)?;
    let inputs = bindings
        .iter()
        .zip(&distributions)
        .zip(worksheet_nominals)
        .map(|(((sheet, cell), distribution), worksheet_nominal)| {
            let mut input = json!({"cell":binding_name(wb,*sheet,*cell)?,"distribution":distribution});
            if let Some(nominal) = worksheet_nominal {
                input["worksheet_nominal"] = nominal;
            }
            Ok(input)
        })
        .collect::<Result<Vec<_>>>()?;
    // Preserve the original total budget for a rerun; the engine budget records
    // the separate conservative IR reservation deducted for this execution.
    request["options"] = json!(requested_options);
    request["inputs"] = json!(inputs);
    request["outputs"] = json!(names);
    let manifest = manifest::create(
        "simulation",
        &snapshot,
        &request,
        json!({"simulation":requested_options,"engine":opt,"ir_reserved_bytes":program.reserved_bytes()}),
        json!({"sampler":requested_options.method,"sampling_version":result.sampling_version,"formula_evaluator":"gridcraft-ordered-cell-ir-v1"}),
        json!({"inputs":inputs,"outputs":names}),
    )?;
    Ok(
        json!({"result":result,"outputs":names,"worksheet_statistics":worksheet_statistics,"request":request,"snapshot":snapshot,"manifest":manifest,"execution":"synchronous_bounded"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", json!({})).unwrap();
        s
    }
    fn set(s: &mut Session, cell: &str, input: &str) {
        s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
    }
    #[test]
    fn structural_compile_preview_apply_and_undo() {
        let mut s = session();
        set(&mut s, "A1", "0");
        set(&mut s, "A2", "0");
        set(&mut s, "B1", "=3*A1+5*A2");
        set(&mut s, "B2", "=3*A1+2*A2");
        let request = json!({"objective":"B1","variables":"A1:A2","constraints":["A1 <= 4","A2 <= 6","B2 <= 18"]});
        let before = s.doc().unwrap().wb.clone();
        let result = s.execute("solvercraft.solve", request).unwrap();
        assert!(std::sync::Arc::ptr_eq(&before, &s.doc().unwrap().wb));
        assert_eq!(result["result"]["objective"], json!(36.0));
        s.execute("solvercraft.result.apply", result).unwrap();
        assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(2.0));
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(0.0));
    }
    #[test]
    fn stale_preview_and_empty_params_never_apply_or_open_dialog() {
        let mut s = session();
        set(&mut s, "A1", "0");
        set(&mut s, "B1", "=A1");
        let result = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=1"]})).unwrap();
        set(&mut s, "C1", "1");
        assert!(s.execute("solvercraft.result.apply", result).is_err());
        let count = s.ui_requests.len();
        assert!(s.execute("solvercraft.solve", json!({})).is_err());
        assert_eq!(count, s.ui_requests.len());
    }
    #[test]
    fn nonlinear_expressions_are_not_classified_as_linear() {
        let mut s = session();
        set(&mut s, "A1", "0");
        set(&mut s, "B1", "=SIN(A1)");
        let result = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1"})).unwrap();
        assert_eq!(result["class"], json!("nonlinear_expression"));
        assert!(s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1","method":"simplex"})).is_err());
    }
    #[test]
    fn simulation_does_not_edit_the_workbook() {
        let mut s = session();
        set(&mut s, "A1", "2");
        set(&mut s, "B1", "=2*A1");
        let before = s.doc().unwrap().wb.clone();
        let r=s.execute("solvercraft.simulate",json!({"inputs":[{"cell":"A1","distribution":{"kind":"constant","value":3.0}}],"outputs":"B1","options":{"trials":10,"retain_samples":true}})).unwrap();
        assert_eq!(r["result"]["summaries"][0]["mean"], json!(6.0));
        assert!(std::sync::Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
}
