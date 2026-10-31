//! Workbook bindings for the independent Solvercraft SDK.
//!
//! Commands never open dialogs. Solves are bounded synchronous previews; only
//! result.apply writes cells, after checking the document identity and revision.
use std::collections::{HashMap, HashSet};

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

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "solvercraft.capabilities", "Solvercraft capabilities", [], None,
            "{} -> supported formula subset and execution capabilities; never opens a dialog.",
            always, capabilities),
        cmd!(query "solvercraft.validate", "Validate Solvercraft model", [], None,
            "{objective: cell|formula, goal?: min|max|value, value?: number, variables: range|string[]|[{cell,kind?,lower?,upper?}], constraints?: [string|{lhs,op,rhs}], nonNegative?: true, options?: SolveOptions} -> {model,bindings,class}; alternatively {model: OptimizationModel}.",
            has_doc, validate),
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
    ]
}

fn sdk_error(e: impl std::fmt::Display) -> EngineError {
    bad(SOLVE, e.to_string())
}

fn capabilities(_: &mut Session, _: &Json) -> Result<Json> {
    Ok(json!({
        "schema_version": 1,
        "preview_only": true,
        "execution": "synchronous_bounded",
        "automatic_apply": false,
        "sdk": ::solvercraft::capabilities(),
        "workbook_formula_subset": [
            "numeric constants", "cell and named references", "+ - * /",
            "nonnegative integer powers", "SUM", "SUMPRODUCT",
            "SIN", "COS", "EXP", "LN", "SQRT", "ABS", "MIN", "MAX"
        ],
        "unsupported_references": ["3D", "external workbooks", "dynamic references", "spills"],
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
    expression: Option<ExpressionModel>,
    bindings: Vec<(usize, CellRef)>,
    names: Vec<Binding>,
    options: SolveOptions,
}

fn snapshot(s: &Session) -> Result<Snapshot> {
    let doc = s.doc()?;
    Ok(Snapshot { document_uid: doc.uid.to_string(), revision: doc.revision.to_string() })
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

fn prepare(wb: &Workbook, p: &Json) -> Result<Prepared> {
    if !p.is_object() {
        return Err(bad(SOLVE, "Solvercraft parameters must be an object"));
    }
    let opt = options(p)?;
    if let Some(model) = p.get("model") {
        let model: OptimizationModel = serde_json::from_value(model.clone()).map_err(sdk_error)?;
        // Compilation/validation also happens in solve; validation must not execute the solver.
        ::solvercraft::model::validate(&model, &opt).map_err(sdk_error)?;
        return Ok(Prepared { model: Some(model), expression: None, bindings: Vec::new(), names: Vec::new(), options: opt });
    }
    let base = match str_param(p, "sheet") {
        Some(name) => wb.sheet_index(name).ok_or_else(|| bad(SOLVE, "The model sheet no longer exists"))?,
        None => wb.active_sheet(),
    };
    let nonnegative = p.get("nonNegative").and_then(Json::as_bool).unwrap_or(true);
    let source = p.get("variables").ok_or_else(|| bad(SOLVE, "variables is required"))?;
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
            if sh.cell(cell).is_some_and(|c| c.formula.is_some()) {
                return Err(bad(SOLVE, "Variable cells must contain constants, not formulas"));
            }
            numeric(&sh.value(cell))?;
            let lower = finite_option(&item, "lower")?.or(if nonnegative { Some(0.0) } else { None });
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
    let mut tr = Translator::new(wb, &bindings);
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
    if str_param(p, "goal") == Some("value") {
        let target = p
            .get("value")
            .and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|v| v.parse().ok())))
            .filter(|x| x.is_finite())
            .ok_or_else(|| bad(SOLVE, "A finite target value is required"))?;
        cons.push(ExpressionConstraint {
            name: "Target value".into(),
            left: objective.clone(),
            relation: Relation::Equal,
            right: Expr::constant(target),
        });
    }
    let expr = ExpressionModel {
        name: str_param(p, "name").unwrap_or("Gridcraft model").to_string(),
        variables: vars,
        direction,
        objective,
        constraints: cons,
    };
    // Structural compiler failure only permits an explicitly supported
    // expression backend. Other errors retain their original failure meaning.
    let model = match ::solvercraft::compile::compile(&expr, &opt) {
        Ok(model) => Some(model),
        Err(e)
            if matches!(e.code.as_str(), "unsupported_expression" | "unsupported_constraint")
                && (opt.method == SolveMethod::DifferentialEvolution
                    || (matches!(opt.method, SolveMethod::Auto | SolveMethod::ProjectedGradient)
                        && expr.constraints.is_empty()
                        && expr.variables.iter().all(|v| v.kind == VariableKind::Continuous))) =>
        {
            ::solvercraft::model::validate(&bounds_model(&expr), &opt).map_err(sdk_error)?;
            None
        }
        Err(e) => return Err(sdk_error(e)),
    };
    let names = bindings
        .iter()
        .map(|(si, c)| Ok(Binding { sheet: wb.sheet(*si).ok_or(EngineError::NoDocument)?.name.clone(), cell: c.a1() }))
        .collect::<Result<Vec<_>>>()?;
    Ok(Prepared { model, expression: Some(expr), bindings, names, options: opt })
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
    let expr = parse(text)?;
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
}
impl<'a> Translator<'a> {
    fn new(wb: &'a Workbook, bindings: &[(usize, CellRef)]) -> Self {
        Self { wb, bindings: bindings.iter().enumerate().map(|(i, k)| (*k, i)).collect(), visiting: HashSet::new(), nodes: 0 }
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
        if let Some(index) = self.bindings.get(&(sheet, cell)) {
            return Ok(Expr::variable(*index));
        }
        if !self.visiting.insert((sheet, cell)) {
            return Err(bad(SOLVE, "Circular formula reference"));
        }
        let sh = self.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
        let result = if let Some(formula) = sh.cell(cell).and_then(|c| c.formula.as_ref()) {
            let expr = formula.expr_arc().ok_or_else(|| bad(SOLVE, "A referenced formula cannot be parsed"))?;
            self.expr(sheet, &expr, depth + 1)
        } else {
            numeric(&sh.value(cell)).map(Expr::constant)
        };
        self.visiting.remove(&(sheet, cell));
        result
    }
    fn expanded(&mut self, base: usize, e: &FormulaExpr, depth: usize) -> Result<Vec<Expr>> {
        self.enter(depth)?;
        match e {
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
    fn call(&mut self, base: usize, name: &str, args: &[FormulaExpr], depth: usize) -> Result<Expr> {
        self.enter(depth)?;
        match name {
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
                let shape = expression_shape(args.first().ok_or_else(|| bad(SOLVE, "Missing SUMPRODUCT argument"))?)?;
                for arg in args {
                    if expression_shape(arg)? != shape {
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
                    terms.extend(self.expanded(base, arg, depth + 1)?);
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
    let model = if let Some(model) = prepared.model.as_ref() {
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
fn validate(s: &mut Session, p: &Json) -> Result<Json> {
    let prepared = prepare(&s.doc()?.wb, p)?;
    let class = match prepared.model.as_ref() {
        Some(model) if model.objective.quadratic.is_empty() => "linear",
        Some(_) => "quadratic",
        None => "nonlinear_expression",
    };
    Ok(json!({"model":prepared.model,"expression_model":prepared.expression,"bindings":prepared.names,"class":class,"snapshot":snapshot(s)?}))
}
fn solve(s: &mut Session, p: &Json) -> Result<Json> {
    if !p.is_object() {
        return Err(bad(SOLVE, "Solvercraft parameters must be an object"));
    }
    let mut request = p.clone();
    if request.get("model").is_none() && request.get("sheet").is_none() {
        let sheet = s.doc()?.wb.active().ok_or(EngineError::NoDocument)?.name.clone();
        request["sheet"] = json!(sheet);
    }
    let prepared = prepare(&s.doc()?.wb, &request)?;
    let result = match (prepared.expression.as_ref(), prepared.options.method) {
        (Some(expr), SolveMethod::DifferentialEvolution) => {
            ::solvercraft::heuristic::solve(expr, &prepared.options, &NoControl).map_err(sdk_error)?
        }
        (Some(expr), SolveMethod::ProjectedGradient) => {
            let start = prepared
                .bindings
                .iter()
                .map(|(si, c)| {
                    let sh = s.doc()?.wb.sheet(*si).ok_or(EngineError::NoDocument)?;
                    numeric(&sh.value(*c))
                })
                .collect::<Result<Vec<_>>>()?;
            ::solvercraft::nonlinear::solve_from(expr, &start, &prepared.options, &NoControl).map_err(sdk_error)?
        }
        (Some(expr), SolveMethod::Auto) if prepared.model.is_none() => {
            let start = starting_values(s, &prepared)?;
            ::solvercraft::nonlinear::solve_from(expr, &start, &prepared.options, &NoControl).map_err(sdk_error)?
        }
        _ => {
            let model = prepared.model.as_ref().ok_or_else(|| bad(SOLVE, "This method does not support the expression model"))?;
            match ::solvercraft::solve(model, &prepared.options, &NoControl) {
                Ok(result) => result,
                Err(e)
                    if matches!(e.code.as_str(), "unsupported_qp" | "unsupported_problem")
                        && !model.objective.quadratic.is_empty()
                        && prepared.options.method == SolveMethod::Auto
                        && prepared
                            .expression
                            .as_ref()
                            .is_some_and(|expr| expr.constraints.is_empty() && expr.variables.iter().all(|v| v.kind == VariableKind::Continuous)) =>
                {
                    let expr = prepared.expression.as_ref().ok_or_else(|| bad(SOLVE, "Missing expression model"))?;
                    let start = starting_values(s, &prepared)?;
                    ::solvercraft::nonlinear::solve_from(expr, &start, &prepared.options, &NoControl).map_err(sdk_error)?
                }
                Err(e) => return Err(sdk_error(e)),
            }
        }
    };
    if let Some(values) = result.values.as_ref() {
        if result.feasibility == Feasibility::Feasible {
            validate_candidate(&prepared, values)?;
            verify_workbook_objective(s.doc()?, &request, &prepared, values)?;
        }
    }
    Ok(json!({"result":result,"request":request,"snapshot":snapshot(s)?,"bindings":prepared.names,"execution":"synchronous_bounded"}))
}
fn starting_values(s: &Session, prepared: &Prepared) -> Result<Vec<f64>> {
    prepared
        .bindings
        .iter()
        .map(|(si, c)| {
            let sh = s.doc()?.wb.sheet(*si).ok_or(EngineError::NoDocument)?;
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
    Ok(json!({"applied":true,"cells":prepared.names}))
}

/// Check the compiler against Gridcraft's real evaluator on a private snapshot.
/// This does not write the live workbook or create history.
fn verify_workbook_objective(doc: &crate::DocState, request: &Json, prepared: &Prepared, values: &[f64]) -> Result<()> {
    if prepared.bindings.is_empty() {
        return Ok(());
    }
    if values.len() != prepared.bindings.len() {
        return Err(bad(SOLVE, "Result variable count does not match bindings"));
    }
    let mut wb = (*doc.wb).clone();
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
    let mut calc = doc.calc.clone();
    calc.cells_changed(&mut wb, &changed);
    let sheet = str_param(request, "sheet").and_then(|name| wb.sheet_index(name)).unwrap_or(wb.active_sheet());
    let objective = str_param(request, "objective").ok_or_else(|| bad(SOLVE, "Objective is missing"))?;
    let actual = numeric(&gridcraft_calc::evaluate(&wb, sheet, CellRef::new(0, 0), objective))?;
    let predicted = match (prepared.model.as_ref(), prepared.expression.as_ref()) {
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
            let left = numeric_values(&gridcraft_calc::evaluate(&wb, sheet, CellRef::new(0, 0), &left))?;
            let right = numeric_values(&gridcraft_calc::evaluate(&wb, sheet, CellRef::new(0, 0), &right))?;
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
    let wb = &s.doc()?.wb;
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
    for input in inputs {
        let text = input.get("cell").and_then(Json::as_str).ok_or_else(|| bad(SOLVE, "Each simulation input needs a cell"))?;
        let c = cells(wb, base, text, 1)?.into_iter().next().ok_or_else(|| bad(SOLVE, "Input cell is missing"))?;
        if bindings.contains(&c) {
            return Err(bad(SOLVE, "Simulation input cells must be unique"));
        }
        let sh = wb.sheet(c.0).ok_or(EngineError::NoDocument)?;
        if sh.cell(c.1).is_some_and(|c| c.formula.is_some()) {
            return Err(bad(SOLVE, "Simulation inputs must be constant cells"));
        }
        let distribution: Distribution =
            serde_json::from_value(input.get("distribution").cloned().ok_or_else(|| bad(SOLVE, "Input distribution is required"))?)
                .map_err(sdk_error)?;
        bindings.push(c);
        distributions.push(distribution);
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
    let mut translator = Translator::new(wb, &bindings);
    let exprs = output_cells.iter().map(|(si, c)| translator.cell(*si, *c, 0)).collect::<Result<Vec<_>>>()?;
    let opt: SimulationOptions = serde_json::from_value(p.get("options").cloned().unwrap_or_else(|| json!({}))).map_err(sdk_error)?;
    let result = ::solvercraft::simulation::simulate(&distributions, &opt, &NoControl, |values| {
        exprs.iter().map(|expr| ::solvercraft::compile::evaluate(expr, values)).collect()
    })
    .map_err(sdk_error)?;
    let names = output_cells.iter().map(|(si, c)| binding_name(wb, *si, *c)).collect::<Result<Vec<_>>>()?;
    Ok(json!({"result":result,"outputs":names,"snapshot":snapshot(s)?,"execution":"synchronous_bounded"}))
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
