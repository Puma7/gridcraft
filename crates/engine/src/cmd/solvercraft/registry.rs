//! Versioned workbook models. The OPC part owns metadata; hidden formula names
//! own references so Gridcraft's normal structural edit machinery migrates them.
use std::sync::Arc;

use gridcraft_formula::{RefKind, SheetSel};
use gridcraft_model::{DefinedName, PreservedPart, PreservedRel};

use super::{
    CommandSpec, Deserialize, EngineError, FormulaExpr, HashSet, Json, MAX_BINDINGS, MAX_CONSTRAINTS, MAX_DEPTH, Result, SOLVE, Serialize, Session,
    Workbook, bad, binding_name, cells, cmd, constraint_parts, edit, has_doc, json, parse, sdk_error, str_param, variable_entries, worksheet,
};

const PART: &str = "xl/solvercraft/models.json";
const CONTENT_TYPE: &str = "application/vnd.solvercraft.workbook-models+json";
const RELATIONSHIP: &str = "https://solvercraft.org/relationships/workbook-models/v1";
const MAX_MODELS: usize = 32;
const MAX_BYTES: usize = 1_048_576;
const MAX_REQUEST_BYTES: usize = 131_072;
const PREFIX: &str = "_solvercraft_ref_";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    next_id: String,
    models: Vec<StoredModel>,
}
impl Default for Registry {
    fn default() -> Self {
        Self { schema_version: 1, next_id: "1".into(), models: Vec::new() }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredModel {
    id: String,
    name: String,
    kind: String,
    request: Json,
    anchors: Vec<String>,
    #[serde(default)]
    anchor_bytes: usize,
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.model.list", "List Solvercraft models", [], None, "{} -> versioned workbook model summaries.", has_doc, list),
        cmd!(query "solvercraft.model.get", "Get Solvercraft model", [], None, "{id:string} -> request with migrated references; deleted bindings return an error.", has_doc, get),
        cmd!(
            "solvercraft.model.create",
            "Save Solvercraft model",
            [],
            None,
            "{name,kind?:optimization|simulation,request} -> saved model. Metadata uses an owned XLSX part and references use hidden formula names.",
            has_doc,
            create
        ),
        cmd!(
            "solvercraft.model.update",
            "Update Solvercraft model",
            [],
            None,
            "{id,name?,kind?,request?} -> updated saved model, one undo step.",
            has_doc,
            update
        ),
        cmd!(
            "solvercraft.model.delete",
            "Delete Solvercraft model",
            [],
            None,
            "{id} -> deletes the model and its private reference names, one undo step.",
            has_doc,
            delete
        ),
        cmd!(query "solvercraft.model.export", "Export Solvercraft model", [], None, "{id} -> versioned portable workbook-binding JSON with current references.", has_doc, export),
        cmd!(
            "solvercraft.model.import",
            "Import Solvercraft model",
            [],
            None,
            "{project:{format:solvercraft-workbook-model,schema_version:1,name,kind,request},name?} -> a new model; incompatible versions fail explicitly.",
            has_doc,
            import
        ),
        cmd!(
            "solvercraft.model.importLegacy",
            "Import legacy Solver model",
            [],
            None,
            "{sheet?,name?} -> imports sheet-scoped solver_* names. Original names remain available to the existing Solver.",
            has_doc,
            legacy
        ),
    ]
}

fn read(wb: &Workbook) -> Result<Registry> {
    let Some(kept) = wb.preserved.as_deref() else { return Ok(Registry::default()) };
    let parts = kept.parts.iter().filter(|p| p.name == PART).collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(Registry::default());
    }
    if parts.len() != 1 {
        return Err(bad(SOLVE, "Duplicate Solvercraft model parts"));
    }
    let part = parts[0];
    if part.content_type != CONTENT_TYPE || part.data.len() > MAX_BYTES {
        return Err(bad(SOLVE, "Invalid or oversized Solvercraft model part"));
    }
    if !kept.rels.iter().any(|r| r.rel_type == RELATIONSHIP && r.target == PART && !r.external) {
        return Err(bad(SOLVE, "Solvercraft model part is missing its workbook relationship"));
    }
    let registry: Registry = serde_json::from_slice(&part.data).map_err(sdk_error)?;
    if registry.schema_version != 1 {
        return Err(bad(SOLVE, "Unsupported Solvercraft workbook model schema version"));
    }
    if registry.models.len() > MAX_MODELS {
        return Err(bad(SOLVE, "Too many stored Solvercraft models"));
    }
    let next = registry.next_id.parse::<u64>().map_err(sdk_error)?;
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for model in &registry.models {
        validate_meta(&model.name, &model.kind)?;
        let id = model.id.parse::<u64>().map_err(sdk_error)?;
        if id == 0 || id >= next || !ids.insert(id) || !names.insert(gridcraft_formula::fold_name(&model.name)) {
            return Err(bad(SOLVE, "Invalid or duplicate Solvercraft model identity"));
        }
        if model.anchors.len() > MAX_BINDINGS.saturating_mul(4) {
            return Err(bad(SOLVE, "Stored model exceeds the reference budget"));
        }
        let expected = format!("{PREFIX}{}_", model.id);
        if model.anchors.iter().any(|name| !name.starts_with(&expected)) {
            return Err(bad(SOLVE, "Stored model references another model's private namespace"));
        }
    }
    Ok(registry)
}

fn write(wb: &mut Workbook, registry: &Registry) -> Result<()> {
    let bytes = serde_json::to_vec(registry).map_err(sdk_error)?;
    if bytes.len() > MAX_BYTES {
        return Err(bad(SOLVE, "Solvercraft registry exceeds its byte budget"));
    }
    let kept = Arc::make_mut(wb.preserved.get_or_insert_with(|| Arc::new(Default::default())));
    kept.parts.retain(|p| p.name != PART);
    kept.rels.retain(|r| r.rel_type != RELATIONSHIP);
    kept.parts.push(PreservedPart { name: PART.into(), content_type: CONTENT_TYPE.into(), data: bytes.into(), rels: Vec::new() });
    kept.rels.push(PreservedRel { id: "solvercraftModels".into(), rel_type: RELATIONSHIP.into(), target: PART.into(), external: false });
    Ok(())
}
fn validate_meta(name: &str, kind: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 256 {
        return Err(bad(SOLVE, "Model name must contain 1 to 256 bytes"));
    }
    if !matches!(kind, "optimization" | "simulation") {
        return Err(bad(SOLVE, "Model kind must be optimization or simulation"));
    }
    Ok(())
}
fn find<'a>(registry: &'a Registry, p: &Json) -> Result<&'a StoredModel> {
    let id = str_param(p, "id").ok_or_else(|| bad(SOLVE, "A decimal string model id is required"))?;
    registry.models.iter().find(|m| m.id == id).ok_or_else(|| bad(SOLVE, "Saved model does not exist"))
}
fn resolve(wb: &Workbook, model: &StoredModel) -> Result<Json> {
    fn visit(wb: &Workbook, model: &StoredModel, value: &Json, depth: usize) -> Result<Json> {
        if depth > MAX_DEPTH {
            return Err(bad(SOLVE, "Stored request exceeds nesting budget"));
        }
        if let Some(name) = value.get("$solvercraft_ref").and_then(Json::as_str) {
            if !model.anchors.iter().any(|a| a == name) {
                return Err(bad(SOLVE, "Unknown saved binding"));
            }
            let entries = wb.names.iter().filter(|n| n.scope.is_none() && n.name == name).collect::<Vec<_>>();
            if entries.len() != 1 {
                return Err(bad(SOLVE, "Saved binding is missing or duplicated"));
            }
            let expression = parse(&entries[0].formula)?;
            let mut broken = false;
            expression.walk(&mut |e| {
                if matches!(e, FormulaExpr::Error(_)) {
                    broken = true
                }
            });
            if broken {
                return Err(bad(SOLVE, "Saved binding was invalidated by a structural edit (#REF!)"));
            }
            return Ok(json!(gridcraft_formula::print(&expression)));
        }
        match value {
            Json::Array(values) => values.iter().map(|v| visit(wb, model, v, depth + 1)).collect::<Result<Vec<_>>>().map(Json::Array),
            Json::Object(values) => values
                .iter()
                .map(|(k, v)| Ok((k.clone(), visit(wb, model, v, depth + 1)?)))
                .collect::<Result<serde_json::Map<_, _>>>()
                .map(Json::Object),
            other => Ok(other.clone()),
        }
    }
    visit(wb, model, &model.request, 0)
}
fn resolved(wb: &Workbook, model: &StoredModel) -> Result<Json> {
    Ok(json!({"id":model.id,"name":model.name,"kind":model.kind,"schema_version":1,"request":resolve(wb,model)?}))
}
fn list(s: &mut Session, _: &Json) -> Result<Json> {
    let wb = &s.doc()?.wb;
    let registry = read(wb)?;
    Ok(
        json!({"schema_version":1,"models":registry.models.iter().map(|m| json!({"id":m.id,"name":m.name,"kind":m.kind,"bindings_valid":resolve(wb,m).is_ok()})).collect::<Vec<_>>()}),
    )
}
pub(super) fn get(s: &mut Session, p: &Json) -> Result<Json> {
    let registry = read(&s.doc()?.wb)?;
    resolved(&s.doc()?.wb, find(&registry, p)?)
}

/// Pin relative references and locally scoped names to their original sheet.
fn qualify(wb: &Workbook, base: usize, text: &str, depth: usize) -> Result<FormulaExpr> {
    qualify_inner(wb, base, text, depth, &mut 0)
}
fn qualify_inner(wb: &Workbook, base: usize, text: &str, depth: usize, nodes: &mut usize) -> Result<FormulaExpr> {
    if depth > 32 {
        return Err(bad(SOLVE, "Saved named reference exceeds the recursion budget"));
    }
    let expression = parse(text)?;
    expression.walk(&mut |_| *nodes = nodes.saturating_add(1));
    if *nodes > 8192 {
        return Err(bad(SOLVE, "Saved reference expansion exceeds the node budget"));
    }
    let sheet = wb.sheet(base).ok_or(EngineError::NoDocument)?.name.clone();
    let mut error = None;
    let qualified = expression.map(&mut |node| match node {
        FormulaExpr::Ref(mut reference) => {
            if reference.sheet == SheetSel::Current {
                reference.sheet = SheetSel::Named(sheet.clone())
            }
            match &mut reference.kind {
                RefKind::Cell(a) => {
                    a.row_abs = true;
                    a.col_abs = true;
                }
                RefKind::Range(a, b) => {
                    a.row_abs = true;
                    a.col_abs = true;
                    b.row_abs = true;
                    b.col_abs = true;
                }
                RefKind::Rows(_, a, _, b) | RefKind::Cols(_, a, _, b) => {
                    *a = true;
                    *b = true;
                }
            }
            FormulaExpr::Ref(reference)
        }
        FormulaExpr::Name(name) => match wb.name(&name, base) {
            Some(defined) => match qualify_inner(wb, base, &defined.formula, depth + 1, nodes) {
                Ok(expr) => expr,
                Err(e) => {
                    error = Some(e);
                    FormulaExpr::Name(name)
                }
            },
            None => {
                error = Some(bad(SOLVE, format!("Unknown saved name {name}")));
                FormulaExpr::Name(name)
            }
        },
        other => other,
    });
    if let Some(error) = error {
        return Err(error);
    }
    Ok(qualified)
}
fn anchor(wb: &mut Workbook, model: &mut StoredModel, base: usize, text: &str) -> Result<Json> {
    if model.anchors.len() >= MAX_BINDINGS.saturating_mul(4) {
        return Err(bad(SOLVE, "Model exceeds the saved reference budget"));
    }
    let formula = gridcraft_formula::print(&qualify(wb, base, text, 0)?);
    model.anchor_bytes = model.anchor_bytes.saturating_add(formula.len());
    if formula.len() > 8192 || model.anchor_bytes > 262_144 {
        return Err(bad(SOLVE, "Saved binding formulas exceed the byte budget"));
    }
    let name = format!("{PREFIX}{}_{}", model.id, model.anchors.len());
    if wb.names.iter().any(|n| n.name.eq_ignore_ascii_case(&name)) {
        return Err(bad(SOLVE, "A private Solvercraft binding name already exists"));
    }
    wb.names.push(DefinedName {
        name: name.clone(),
        scope: None,
        formula,
        comment: "Solvercraft binding; updated by structural edits".into(),
        hidden: true,
    });
    model.anchors.push(name.clone());
    Ok(json!({"$solvercraft_ref":name}))
}
fn store(wb: &mut Workbook, id: String, name: String, kind: String, request: &Json) -> Result<StoredModel> {
    validate_meta(&name, &kind)?;
    if !request.is_object() {
        return Err(bad(SOLVE, "Saved request must be an object"));
    }
    let mut request = request.clone();
    if kind == "simulation" && (request.get("inputs").is_none() || request.get("outputs").is_none()) {
        let (inputs, outputs) = worksheet::discover(wb)?;
        if request.get("inputs").is_none() {
            request["inputs"] = json!(inputs.iter().map(|input| json!({"cell":input["cell"]})).collect::<Vec<_>>());
        }
        if request.get("outputs").is_none() {
            request["outputs"] = json!(outputs);
        }
    }
    let request = &request;
    if !request.is_object() || serde_json::to_vec(request).map_err(sdk_error)?.len() > MAX_REQUEST_BYTES {
        return Err(bad(SOLVE, "Model request must be a bounded JSON object"));
    }
    let base = str_param(request, "sheet")
        .map(|name| wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "Saved model sheet does not exist")))
        .transpose()?
        .unwrap_or(wb.active_sheet());
    let mut model = StoredModel { id, name, kind: kind.clone(), request: request.clone(), anchors: Vec::new(), anchor_bytes: 0 };
    // Every persisted reference becomes qualified, so the obsolete active-sheet name is unnecessary.
    let mut template = request.clone();
    if let Some(object) = template.as_object_mut() {
        object.remove("sheet");
        object.remove("_snapshot_time_serial");
    }
    if kind == "optimization" && request.get("model").is_none() {
        let objective = str_param(request, "objective").ok_or_else(|| bad(SOLVE, "objective is required"))?;
        template["objective"] = anchor(wb, &mut model, base, objective)?;
        let entries = variable_entries(request.get("variables").ok_or_else(|| bad(SOLVE, "variables is required"))?)?;
        let mut vars = Vec::new();
        for entry in entries {
            let text = entry.as_str().or_else(|| entry.get("cell").and_then(Json::as_str)).ok_or_else(|| bad(SOLVE, "Each variable needs a cell"))?;
            for (si, cell) in cells(wb, base, text, MAX_BINDINGS.saturating_sub(vars.len()))? {
                let reference = binding_name(wb, si, cell)?;
                let mut object = if entry.is_object() { entry.clone() } else { json!({}) };
                object["cell"] = anchor(wb, &mut model, base, &reference)?;
                vars.push(object);
            }
        }
        template["variables"] = json!(vars);
        if let Some(rows) = request.get("constraints").and_then(Json::as_array) {
            if rows.len() > MAX_CONSTRAINTS {
                return Err(bad(SOLVE, "Too many saved constraints"));
            }
            let mut constraints = Vec::new();
            for row in rows {
                let (lhs, op, rhs) = constraint_parts(row)?;
                let left = anchor(wb, &mut model, base, &lhs)?;
                let right = if matches!(op.as_str(), "int" | "bin") { json!("") } else { anchor(wb, &mut model, base, &rhs)? };
                constraints.push(json!({"lhs":left,"op":op,"rhs":right}));
            }
            template["constraints"] = json!(constraints);
        }
    } else if kind == "simulation" {
        let inputs = request.get("inputs").and_then(Json::as_array).ok_or_else(|| bad(SOLVE, "Simulation inputs are required"))?;
        if inputs.len() > MAX_BINDINGS {
            return Err(bad(SOLVE, "Too many saved inputs"));
        }
        let mut saved = Vec::new();
        for input in inputs {
            let text = input.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Input needs a cell"))?;
            let mut input = input.clone();
            input["cell"] = anchor(wb, &mut model, base, text)?;
            saved.push(input);
        }
        template["inputs"] = json!(saved);
        let outputs = request.get("outputs").ok_or_else(|| bad(SOLVE, "Outputs are required"))?;
        let outputs = match outputs {
            Json::String(s) => vec![s.clone()],
            Json::Array(v) => v
                .iter()
                .map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(SOLVE, "Outputs must be reference strings")))
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(bad(SOLVE, "Outputs must be references")),
        };
        if outputs.len() > MAX_BINDINGS {
            return Err(bad(SOLVE, "Too many saved outputs"));
        }
        template["outputs"] = Json::Array(outputs.iter().map(|text| anchor(wb, &mut model, base, text)).collect::<Result<Vec<_>>>()?);
    }
    for field in ["parameter_cases", "parameter_grid"] {
        if let Some(rows) = request.get(field).and_then(Json::as_array) {
            if rows.len() > 32 {
                return Err(bad(SOLVE, "Too many saved parameter cases"));
            }
            let mut saved = Vec::new();
            for row in rows {
                let mut row = row.clone();
                if field == "parameter_grid" {
                    let text = row.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Parameter grid cell is required"))?.to_string();
                    row["cell"] = anchor(wb, &mut model, base, &text)?;
                } else {
                    let values = row
                        .get("values")
                        .and_then(Json::as_array)
                        .filter(|values| values.len() <= MAX_BINDINGS)
                        .ok_or_else(|| bad(SOLVE, "Parameter case values are required"))?;
                    let mut converted = Vec::new();
                    for value in values {
                        let mut value = value.clone();
                        let text = value.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Parameter cell is required"))?.to_string();
                        value["cell"] = anchor(wb, &mut model, base, &text)?;
                        converted.push(value);
                    }
                    row["values"] = json!(converted);
                }
                saved.push(row);
            }
            template[field] = json!(saved);
        }
    }
    model.request = template;
    Ok(model)
}
fn create(s: &mut Session, p: &Json) -> Result<Json> {
    let name = str_param(p, "name").ok_or_else(|| bad(SOLVE, "A model name is required"))?.to_string();
    let kind = str_param(p, "kind").unwrap_or("optimization").to_string();
    let request = p.get("request").ok_or_else(|| bad(SOLVE, "A model request is required"))?.clone();
    edit(s, |cx| {
        let mut registry = read(&cx.wb)?;
        if registry.models.len() >= MAX_MODELS {
            return Err(bad(SOLVE, "Workbook model limit reached"));
        }
        if registry.models.iter().any(|m| gridcraft_formula::same_name(&m.name, &name)) {
            return Err(bad(SOLVE, "A model with this name already exists"));
        }
        let id = registry.next_id.clone();
        registry.next_id = id.parse::<u64>().map_err(sdk_error)?.checked_add(1).ok_or_else(|| bad(SOLVE, "Model id budget exhausted"))?.to_string();
        let model = store(&mut cx.wb, id, name, kind, &request)?;
        let output = resolved(&cx.wb, &model)?;
        registry.models.push(model);
        write(&mut cx.wb, &registry)?;
        cx.structural = true;
        Ok(output)
    })
}
fn update(s: &mut Session, p: &Json) -> Result<Json> {
    edit(s, |cx| {
        let mut registry = read(&cx.wb)?;
        let old = find(&registry, p)?.clone();
        let name = str_param(p, "name").unwrap_or(&old.name).to_string();
        if registry.models.iter().any(|m| m.id != old.id && gridcraft_formula::same_name(&m.name, &name)) {
            return Err(bad(SOLVE, "A model with this name already exists"));
        }
        let request = p.get("request").cloned().map(Ok).unwrap_or_else(|| resolve(&cx.wb, &old))?;
        cx.wb.names.retain(|n| !old.anchors.contains(&n.name));
        let model = store(&mut cx.wb, old.id.clone(), name, str_param(p, "kind").unwrap_or(&old.kind).into(), &request)?;
        let output = resolved(&cx.wb, &model)?;
        let slot = registry.models.iter_mut().find(|m| m.id == old.id).ok_or_else(|| bad(SOLVE, "Model disappeared"))?;
        *slot = model;
        write(&mut cx.wb, &registry)?;
        cx.structural = true;
        Ok(output)
    })
}
fn delete(s: &mut Session, p: &Json) -> Result<Json> {
    edit(s, |cx| {
        let mut registry = read(&cx.wb)?;
        let old = find(&registry, p)?.clone();
        cx.wb.names.retain(|n| !old.anchors.contains(&n.name));
        registry.models.retain(|m| m.id != old.id);
        write(&mut cx.wb, &registry)?;
        cx.structural = true;
        Ok(json!({"deleted":old.id}))
    })
}
fn export(s: &mut Session, p: &Json) -> Result<Json> {
    let model = get(s, p)?;
    Ok(json!({"format":"solvercraft-workbook-model","schema_version":1,"name":model["name"],"kind":model["kind"],"request":model["request"]}))
}
fn import(s: &mut Session, p: &Json) -> Result<Json> {
    let project = p.get("project").ok_or_else(|| bad(SOLVE, "project is required"))?;
    if project.get("format").and_then(Json::as_str) != Some("solvercraft-workbook-model")
        || project.get("schema_version").and_then(Json::as_u64) != Some(1)
    {
        return Err(bad(SOLVE, "Unsupported workbook project format/version"));
    }
    let mut params = project.clone();
    if let Some(name) = p.get("name") {
        params["name"] = name.clone();
    }
    create(s, &params)
}
fn legacy(s: &mut Session, p: &Json) -> Result<Json> {
    let wb = &s.doc()?.wb;
    let sheet = str_param(p, "sheet")
        .map(|name| wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "Unknown legacy model sheet")))
        .transpose()?
        .unwrap_or(wb.active_sheet());
    let mut request = super::super::solver::saved_model(wb, sheet).ok_or_else(|| bad(SOLVE, "No saved solver_* model exists on this sheet"))?;
    super::legacy::import_request(wb, sheet, &mut request)?;
    if let Some(seed) = request
        .get("randomSeed")
        .and_then(Json::as_f64)
        .filter(|seed| seed.is_finite() && *seed >= 0.0 && *seed <= u32::MAX as f64 && seed.fract() == 0.0)
    {
        request["options"] = json!({"seed":(seed as u64).to_string()});
    }
    request["sheet"] = json!(wb.sheet(sheet).ok_or(EngineError::NoDocument)?.name);
    let integer = request
        .get("constraints")
        .and_then(Json::as_array)
        .is_some_and(|rows| rows.iter().any(|row| constraint_parts(row).is_ok_and(|(_, op, _)| matches!(op.as_str(), "int" | "bin"))));
    // Method names are mapped truthfully to the implemented Solvercraft engines.
    request["method"] = json!(match request.get("method").and_then(Json::as_str) {
        Some("simplex") if integer => "branch_and_bound",
        Some("simplex") => "simplex",
        Some("evolutionary") => "differential_evolution",
        _ if integer => "auto",
        _ => "projected_gradient",
    });
    let mut result = create(s, &json!({"name":str_param(p,"name").unwrap_or("Imported Solver model"),"request":request}))?;
    result["import_notes"] = json!([
        "Legacy GRG is not reproduced as a GRG algorithm. Continuous local search uses projected_gradient; integer linear models use auto/branch_and_bound.",
        "Legacy Simplex with integer domains maps to actual branch_and_bound; a pure Simplex relaxation never silently drops integer restrictions.",
        "Legacy reports and search tuning have no lossless Solvercraft mapping; original solver_* names are preserved."
    ]);
    Ok(result)
}
