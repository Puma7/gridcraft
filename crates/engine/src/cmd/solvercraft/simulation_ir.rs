//! Immutable, bounded cell program for simulation. Formula nodes deliberately use
//! Gridcraft's value evaluator: spreadsheet IEEE operation order, coercions,
//! reference provenance and errors are not mathematical compiler rewrites.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gridcraft_calc::eval::{Evaluator, Host};
use gridcraft_core::{CellError, CellRef, RangeRef, Value};
use gridcraft_formula::{BinOp, Expr, UnOp};
use gridcraft_model::Workbook;
use solvercraft::types::{RunControl, SolverError};

use super::{CellBinding, MAX_DEPTH, MAX_NODES, MAX_RANGE_CELLS, Result, SOLVE, bad, reference_sheet};

#[derive(Clone, Copy)]
struct Shape {
    rows: usize,
    cols: usize,
    reference: bool,
}
impl Shape {
    const SCALAR: Self = Self { rows: 1, cols: 1, reference: false };
    fn cells(self) -> usize {
        self.rows.saturating_mul(self.cols)
    }
    fn broadcast(self, other: Self) -> Self {
        Self { rows: self.rows.max(other.rows), cols: self.cols.max(other.cols), reference: false }
    }
}
enum Operation {
    Constant(Value),
    Input(usize),
    Formula(Arc<Expr>),
}
struct CellNode {
    binding: CellBinding,
    operation: Operation,
}

/// A topological cell IR. Every referenced cell is captured in this program;
/// the evaluator's Host can never fall back to a cached/live workbook value.
/// Scratch values are overwritten for every trial and repeated input reads use
/// the same immutable sampled slice. No workbook cloning occurs per trial.
pub(super) struct Program<'a> {
    workbook: &'a Workbook,
    nodes: Vec<CellNode>,
    indexes: HashMap<CellBinding, usize>,
    values: Vec<Value>,
    outputs: Vec<usize>,
    inputs: usize,
    now: f64,
    reserved_bytes: u64,
    work_units: u64,
}
struct Builder<'a> {
    workbook: &'a Workbook,
    nodes: Vec<CellNode>,
    indexes: HashMap<CellBinding, usize>,
    inputs: HashMap<CellBinding, usize>,
    visiting: HashSet<CellBinding>,
    visits: usize,
    bytes: u64,
    limit: u64,
    work: u64,
}
impl<'a> Program<'a> {
    pub(super) fn compile(workbook: &'a Workbook, inputs: &[CellBinding], outputs: &[CellBinding], now: f64, limit: u64) -> Result<Self> {
        let mut builder = Builder {
            workbook,
            nodes: Vec::new(),
            indexes: HashMap::new(),
            inputs: inputs.iter().copied().enumerate().map(|(i, binding)| (binding, i)).collect(),
            visiting: HashSet::new(),
            visits: 0,
            bytes: 0,
            limit,
            work: 0,
        };
        builder.charge((inputs.len() as u64).saturating_mul(128).saturating_add((outputs.len() as u64).saturating_mul(16)).saturating_add(512))?;
        let outputs = outputs.iter().map(|binding| builder.cell(*binding, 0)).collect::<Result<Vec<_>>>()?;
        let values = vec![Value::Empty; builder.nodes.len()];
        Ok(Self {
            workbook,
            nodes: builder.nodes,
            indexes: builder.indexes,
            values,
            outputs,
            inputs: inputs.len(),
            // Host clocks use the canonical1900 serial; NOW/TODAY perform the
            // workbook1904 conversion inside Gridcraft's function library.
            now,
            reserved_bytes: builder.bytes,
            work_units: builder.work,
        })
    }
    /// Conservative IR/scratch/transient-array reservation, deducted from the
    /// simulation engine's own allocation budget before admitting a run.
    pub(super) fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }
    pub(super) fn check_work(&self, trials: u64) -> Result<()> {
        if self.work_units.checked_mul(trials).is_none_or(|work| work > 100_000_000) {
            return Err(bad(SOLVE, "Simulation exceeds the 100,000,000 prepared-expression work budget"));
        }
        Ok(())
    }
    pub(super) fn evaluate(&mut self, inputs: &[f64], control: &dyn RunControl) -> solvercraft::types::Result<Vec<f64>> {
        if inputs.len() != self.inputs || inputs.iter().any(|value| !value.is_finite()) {
            return Err(SolverError::new("simulation_input", "Trial inputs must match the finite captured input vector."));
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if control.is_cancelled() {
                return Err(cancelled());
            }
            let value = match &node.operation {
                Operation::Constant(value) => value.clone(),
                Operation::Input(input) => {
                    inputs.get(*input).copied().map(Value::Number).ok_or_else(|| SolverError::new("simulation_input", "Missing trial input."))?
                }
                Operation::Formula(expression) => {
                    let mut host =
                        TrialHost { workbook: self.workbook, indexes: &self.indexes, values: &self.values, now: self.now, control, cancelled: false };
                    let value = Evaluator::new(&mut host, node.binding.0, node.binding.1).value(expression);
                    if host.cancelled || control.is_cancelled() {
                        return Err(cancelled());
                    }
                    // Calc stores scalar cell results and displays an empty
                    // formula result as zero. Array-producing cells were
                    // rejected before execution; arrays inside reductions stay.
                    match value {
                        Value::Empty => Value::Number(0.0),
                        Value::Array(_) => value.scalar(),
                        other => other,
                    }
                }
            };
            let slot = self.values.get_mut(index).ok_or_else(|| SolverError::new("simulation_ir", "Invalid prepared value index."))?;
            *slot = value;
        }
        self.outputs
            .iter()
            .map(|index| match self.values.get(*index) {
                Some(Value::Number(value)) if value.is_finite() => Ok(*value),
                Some(Value::Empty) => Ok(0.0),
                Some(Value::Error(error)) => {
                    let cell = self.nodes.get(*index).map_or_else(|| "?".into(), |node| node.binding.1.a1());
                    Err(SolverError::new("worksheet_error", format!("Simulation output {cell} evaluates to {error}.")))
                }
                _ => Err(SolverError::new("worksheet_output", "Simulation outputs must be finite numeric scalar cells.")),
            })
            .collect()
    }
}
fn cancelled() -> SolverError {
    SolverError::new("cancelled", "The current simulation trial was cancelled before completion.")
}
struct TrialHost<'a> {
    workbook: &'a Workbook,
    indexes: &'a HashMap<CellBinding, usize>,
    values: &'a [Value],
    now: f64,
    control: &'a dyn RunControl,
    cancelled: bool,
}
impl Host for TrialHost<'_> {
    fn workbook(&self) -> &Workbook {
        self.workbook
    }
    fn cell_value(&mut self, sheet: usize, cell: CellRef) -> Value {
        if self.control.is_cancelled() {
            self.cancelled = true;
            return Value::Error(CellError::Calc);
        }
        self.indexes.get(&(sheet, cell)).and_then(|index| self.values.get(*index)).cloned().unwrap_or(Value::Error(CellError::Ref))
    }
    fn now_serial(&self) -> f64 {
        self.now
    }
    // Volatile random functions are rejected in preflight. There is no random
    // source in this host: stochastic cells are addressed input bindings.
    fn random(&mut self) -> f64 {
        0.0
    }
    fn visit_range(&mut self, sheet: usize, range: RangeRef, visit: &mut dyn FnMut(&Value) -> bool) -> bool {
        // Calc's non-spill range fast path streams numeric stored cells in
        // row-major order directly into the outer SUM accumulator. Visiting
        // empty captured cells is harmless: Acc skips them, rather than adding0.
        for cell in range.iter() {
            let value = self.cell_value(sheet, cell);
            if self.cancelled || !visit(&value) {
                break;
            }
        }
        true
    }
}
impl Builder<'_> {
    fn charge(&mut self, bytes: u64) -> Result<()> {
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| bad(SOLVE, "Simulation IR memory budget overflow"))?;
        if self.bytes > self.limit {
            return Err(bad(SOLVE, "Simulation IR exceeds the requested memory budget"));
        }
        Ok(())
    }
    fn enter(&mut self, depth: usize) -> Result<()> {
        self.visits = self.visits.saturating_add(1);
        self.work = self.work.saturating_add(1);
        if depth > MAX_DEPTH || self.visits > MAX_NODES {
            return Err(bad(SOLVE, "Simulation dependency expansion exceeds the depth/node budget"));
        }
        self.charge(64)
    }
    fn cell(&mut self, binding: CellBinding, depth: usize) -> Result<usize> {
        self.enter(depth)?;
        if let Some(index) = self.indexes.get(&binding) {
            return Ok(*index);
        }
        let sheet = self.workbook.sheet(binding.0).ok_or_else(|| bad(SOLVE, "Unknown simulation sheet"))?;
        if sheet.spill_anchor(binding.1).is_some() || sheet.spill_ranges.contains_key(&binding.1) {
            return Err(bad(SOLVE, "Simulation does not support spill references"));
        }
        if !self.visiting.insert(binding) {
            return Err(bad(SOLVE, "Circular simulation formula reference"));
        }
        self.charge(320)?;
        let operation = if let Some(index) = self.inputs.get(&binding) {
            Operation::Input(*index)
        } else if let Some(formula) = sheet.cell(binding.1).and_then(|cell| cell.formula.as_ref()) {
            if formula.text.len() > 8192 {
                return Err(bad(SOLVE, "Simulation formula exceeds the 8192-byte input budget"));
            }
            if formula.array.is_some() {
                return Err(bad(SOLVE, "Simulation does not support legacy array formula cells"));
            }
            let expression = formula.expr_arc().ok_or_else(|| bad(SOLVE, "A simulation formula cannot be parsed"))?;
            let shape = self.expression(binding, &expression, depth + 1)?;
            if shape.cells() != 1 {
                return Err(bad(SOLVE, "Simulation does not support array-producing cell formulas; reduce the array to a scalar"));
            }
            Operation::Formula(expression)
        } else {
            let value = sheet.value(binding.1);
            if matches!(value, Value::Array(_)) {
                return Err(bad(SOLVE, "Simulation constant cells must be scalar"));
            }
            Operation::Constant(value)
        };
        self.visiting.remove(&binding);
        let index = self.nodes.len();
        self.nodes.push(CellNode { binding, operation });
        self.indexes.insert(binding, index);
        Ok(index)
    }
    fn expression(&mut self, at: CellBinding, expression: &Expr, depth: usize) -> Result<Shape> {
        self.enter(depth)?;
        let shape = match expression {
            Expr::Number(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Error(_) | Expr::Missing => Shape::SCALAR,
            Expr::Paren(value) => self.expression(at, value, depth + 1)?,
            Expr::Name(name) => self.name(at, name, depth + 1)?,
            Expr::Ref(reference) => {
                let sheet = reference_sheet(self.workbook, at.0, reference)?;
                let range = reference.range();
                if range.count() > MAX_RANGE_CELLS {
                    return Err(bad(SOLVE, "Simulation range exceeds the expansion budget"));
                }
                for cell in range.iter() {
                    self.cell((sheet, cell), depth + 1)?;
                }
                Shape { rows: range.height() as usize, cols: range.width() as usize, reference: true }
            }
            Expr::Array(rows) => {
                let count = rows.len().checked_mul(rows.first().map_or(0, Vec::len)).ok_or_else(|| bad(SOLVE, "Simulation array size overflow"))?;
                if count == 0 || count > MAX_RANGE_CELLS as usize || rows.iter().any(|row| row.len() != rows.first().map_or(0, Vec::len)) {
                    return Err(bad(SOLVE, "Simulation array shape exceeds the budget"));
                }
                for value in rows.iter().flatten() {
                    if self.expression(at, value, depth + 1)?.cells() != 1 {
                        return Err(bad(SOLVE, "Nested simulation arrays are unsupported"));
                    }
                }
                Shape { rows: rows.len(), cols: rows.first().map_or(0, Vec::len), reference: false }
            }
            Expr::Unary(UnOp::Neg | UnOp::Plus | UnOp::Percent, value) => {
                let mut shape = self.expression(at, value, depth + 1)?;
                shape.reference = false;
                shape
            }
            Expr::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow, left, right) => {
                self.expression(at, left, depth + 1)?.broadcast(self.expression(at, right, depth + 1)?)
            }
            Expr::Binary(BinOp::Union, left, right) => {
                let left = self.expression(at, left, depth + 1)?;
                let right = self.expression(at, right, depth + 1)?;
                if !left.reference || !right.reference {
                    return Err(bad(SOLVE, "Simulation union operands must be references"));
                }
                Shape { rows: 1, cols: left.cells().saturating_add(right.cells()), reference: true }
            }
            Expr::Call(name, arguments) => self.call(at, name, arguments, depth + 1)?,
            _ => return Err(bad(SOLVE, "Unsupported simulation construct; cached values are never substituted")),
        };
        if shape.cells() > MAX_RANGE_CELLS as usize {
            return Err(bad(SOLVE, "Simulation broadcast/union exceeds the array budget"));
        }
        // Cover values, coercion vectors, function lifting and transient arrays.
        self.work = self.work.saturating_add(shape.cells() as u64);
        self.charge((shape.cells() as u64).saturating_mul(128))?;
        Ok(shape)
    }
    fn name(&mut self, at: CellBinding, name: &str, depth: usize) -> Result<Shape> {
        let (sheet, bare) = match name.split_once('!') {
            Some((sheet, bare)) => {
                (self.workbook.sheet_index(&gridcraft_core::unquote_sheet(sheet)).ok_or_else(|| bad(SOLVE, "Unknown named simulation sheet"))?, bare)
            }
            None => (at.0, name),
        };
        let definition = self.workbook.name(bare, sheet).ok_or_else(|| bad(SOLVE, format!("Unknown simulation name: {name}")))?;
        if definition.formula.len() > 8192 {
            return Err(bad(SOLVE, "Simulation name formula exceeds the 8192-byte input budget"));
        }
        let expression = gridcraft_formula::parse_name(&definition.formula).map_err(|_| bad(SOLVE, "Invalid simulation name formula"))?;
        let expression = if gridcraft_formula::adjust::has_relative_refs(&expression) {
            gridcraft_formula::adjust::shift_relative_wrapping(expression, i64::from(at.1.row), i64::from(at.1.col))
        } else {
            expression
        };
        self.expression((definition.scope.unwrap_or(at.0), at.1), &expression, depth + 1)
    }
    fn call(&mut self, at: CellBinding, name: &str, arguments: &[Expr], depth: usize) -> Result<Shape> {
        if name == "SC.OUTPUT" && (1..=2).contains(&arguments.len()) {
            return self.expression(at, arguments.first().ok_or_else(|| bad(SOLVE, "SC.OUTPUT requires a value"))?, depth + 1);
        }
        if matches!(name, "NOW" | "TODAY") && arguments.is_empty() {
            return Ok(Shape::SCALAR);
        }
        if !matches!(name, "SUM" | "SUMPRODUCT" | "MIN" | "MAX" | "SIN" | "COS" | "EXP" | "LN" | "SQRT" | "ABS") {
            return Err(bad(SOLVE, format!("Function {name} is outside the ordered simulation subset")));
        }
        let spec = gridcraft_functions::lookup(name).ok_or_else(|| bad(SOLVE, "Unknown simulation function"))?;
        if !spec.accepts(arguments.len()) {
            return Err(bad(SOLVE, "Invalid simulation function arity"));
        }
        let mut shape = Shape::SCALAR;
        for (index, argument) in arguments.iter().enumerate() {
            let argument_shape = self.expression(at, argument, depth + 1)?;
            if spec.is_scalar_arg(index) {
                shape = shape.broadcast(argument_shape);
            }
        }
        Ok(shape)
    }
}

#[cfg(test)]
mod tests;
