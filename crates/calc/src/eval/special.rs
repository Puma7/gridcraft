use super::{Area, BinOp, Ev, Expr, Reference, SheetSel, Value, Workbook};
use gridcraft_core::{CellRef, MAX_COLS, MAX_ROWS, RangeRef};

/// SUMIF and AVERAGEIF with a sum range: Excel sizes the sum range like the criteria range.
pub(super) fn resizes_sum_range(name: &str, args: usize) -> bool {
    args == 3 && matches!(name, "SUMIF" | "AVERAGEIF")
}

/// Height and width of a single-area reference or an array argument.
pub(super) fn ev_shape(ev: &Ev) -> Option<(u32, u32)> {
    match ev {
        Ev::R(areas) => match areas.as_slice() {
            [a] => Some((a.range.height(), a.range.width())),
            _ => None,
        },
        Ev::V(Value::Array(a)) => Some((u32::try_from(a.rows).unwrap_or(u32::MAX), u32::try_from(a.cols).unwrap_or(u32::MAX))),
        Ev::V(_) => Some((1, 1)),
        Ev::L(_) => None,
    }
}

/// The range SUMIF/AVERAGEIF add up: from the sum range's top-left cell, as many rows and columns
/// as the criteria range (`height` × `width`), cut off at the sheet's edge.
pub fn resized_sum_range(sum: RangeRef, height: u32, width: u32) -> RangeRef {
    let row = sum.start.row.saturating_add(height.saturating_sub(1)).min(MAX_ROWS - 1);
    let col = sum.start.col.saturating_add(width.saturating_sub(1)).min(MAX_COLS - 1);
    RangeRef::new(sum.start, CellRef::new(row, col))
}

/// Collects the static references of an expression with their sheets resolved; `dynamic` is
/// set when the formula uses INDIRECT/OFFSET/names/tables whose targets can change.
pub fn precedents(wb: &Workbook, sheet: usize, e: &Expr) -> (Vec<Area>, bool) {
    let mut out = Vec::new();
    let mut dynamic = false;
    let push = |out: &mut Vec<Area>, r: &Reference, range: RangeRef| match &r.sheet {
        SheetSel::Current => out.push(Area { sheet, range }),
        SheetSel::Named(n) => {
            if let Some(i) = wb.sheet_index(n) {
                out.push(Area { sheet: i, range });
            }
        }
        SheetSel::Span(a, b) => {
            if let (Some(i), Some(j)) = (wb.sheet_index(a), wb.sheet_index(b)) {
                for s in i.min(j)..=i.max(j) {
                    out.push(Area { sheet: s, range });
                }
            }
        }
    };
    e.walk(&mut |x| match x {
        Expr::Ref(r) => push(&mut out, r, r.range()),
        // The cells SUMIF/AVERAGEIF really read from the sum range (see `resized_sum_range`); when
        // the criteria range's size isn't known statically, recalculate the formula every time.
        Expr::Call(n, args) if resizes_sum_range(n, args.len()) => match (args.first(), args.get(2)) {
            (Some(Expr::Ref(c)), Some(Expr::Ref(s))) => {
                let (crit, sum) = (c.range(), s.range());
                push(&mut out, s, resized_sum_range(sum, crit.height(), crit.width()));
            }
            (_, Some(Expr::Number(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Error(_) | Expr::Array(_) | Expr::Missing)) => {}
            _ => dynamic = true,
        },
        Expr::Call(n, _)
            if matches!(n.as_str(), "INDIRECT" | "OFFSET" | "NOW" | "TODAY" | "RAND" | "RANDBETWEEN" | "RANDARRAY" | "CELL" | "INFO") =>
        {
            dynamic = true
        }
        Expr::Name(_) | Expr::Struct(_) => dynamic = true,
        Expr::Call(n, _) if gridcraft_functions::lookup(n).is_none() && !is_special(n) => dynamic = true,
        // A range ending in a function's result (`A1:INDEX(B1:B9,n)`) covers the cells between
        // the references inside it, not only those references.
        Expr::Binary(BinOp::Range, a, b) if !matches!((&**a, &**b), (Expr::Ref(_), Expr::Ref(_))) => {
            let mut inner = precedents(wb, sheet, a).0;
            inner.extend(precedents(wb, sheet, b).0);
            let mut boxes: Vec<Area> = Vec::new();
            for area in inner {
                match boxes.iter_mut().find(|x| x.sheet == area.sheet) {
                    Some(x) => x.range = x.range.union(&area.range),
                    None => boxes.push(area),
                }
            }
            out.extend(boxes);
        }
        _ => {}
    });
    (out, dynamic)
}

fn is_special(n: &str) -> bool {
    if super::solvercraft::distribution_arity(n).is_some() {
        return true;
    }
    matches!(
        n,
        "IF" | "IFS"
            | "SC.NORMAL"
            | "SC.UNIFORM"
            | "SC.TRIANGULAR"
            | "SC.OUTPUT"
            | "SC.MEAN"
            | "SC.QUANTILE"
            | "SC.CVAR"
            | "IFERROR"
            | "IFNA"
            | "CHOOSE"
            | "SWITCH"
            | "ROW"
            | "COLUMN"
            | "ROWS"
            | "COLUMNS"
            | "AREAS"
            | "ISREF"
            | "ISFORMULA"
            | "FORMULATEXT"
            | "SHEET"
            | "SHEETS"
            | "OFFSET"
            | "INDIRECT"
            | "INDEX"
            | "CELL"
            | "TRIMRANGE"
            | "SUBTOTAL"
            | "AGGREGATE"
            | "TEXT"
            | "LET"
            | "LAMBDA"
            | "ISOMITTED"
            | "MAP"
            | "REDUCE"
            | "SCAN"
            | "BYROW"
            | "BYCOL"
            | "MAKEARRAY"
    )
}

/// Names of functions the evaluator implements itself (merged into the function list for UI).
pub const SPECIAL_FUNCTIONS: &[&str] = &[
    "SC.PSI.UNIFORM",
    "SC.PSI.INTUNIFORM",
    "SC.PSI.NORMAL",
    "SC.PSI.LOGNORMAL",
    "SC.PSI.LOGNORM2",
    "SC.PSI.TRIANGULAR",
    "SC.PSI.PERT",
    "SC.PSI.BETA",
    "SC.PSI.GAMMA",
    "SC.PSI.EXPONENTIAL",
    "SC.PSI.WEIBULL",
    "SC.PSI.BERNOULLI",
    "SC.PSI.BINOMIAL",
    "SC.PSI.POISSON",
    "SC.PSI.NEGBINOMIAL",
    "SC.PSI.GEOMETRIC",
    "SC.PSI.DISUNIFORM",
    "SC.PSI.DISCRETE",
    "SC.PSI.CUMUL",
    "SC.PSI.LEVY",
    "SC.PSI.CHISQUARE",
    "SC.PSI.ERLANG",
    "SC.PSI.FDIST",
    "SC.PSI.KUMARASWAMY",
    "SC.PSI.LOGISTIC",
    "SC.PSI.MAXEXTREME",
    "SC.PSI.MINEXTREME",
    "SC.PSI.RAYLEIGH",
    "SC.PSI.RECIPROCAL",
    "SC.PSI.STUDENT",
    "SC.PSI.HYPSECANT",
    "SC.PSI.METALOG",
    "SC.DISCRETEUNIFORM",
    "SC.LOGNORMAL",
    "SC.LOGNORMALMEAN",
    "SC.BETAPERT",
    "SC.BETA",
    "SC.GAMMA",
    "SC.EXPONENTIAL",
    "SC.WEIBULL",
    "SC.BERNOULLI",
    "SC.BINOMIAL",
    "SC.POISSON",
    "SC.NEGATIVEBINOMIAL",
    "SC.GEOMETRIC",
    "SC.EMPIRICAL",
    "SC.EMPIRICALCONTINUOUS",
    "SC.CATEGORICAL",
    "SC.CUMUL",
    "SC.NORMAL",
    "SC.UNIFORM",
    "SC.TRIANGULAR",
    "SC.OUTPUT",
    "SC.MEAN",
    "SC.QUANTILE",
    "SC.CVAR",
    "IF",
    "IFS",
    "IFERROR",
    "IFNA",
    "CHOOSE",
    "SWITCH",
    "ROW",
    "COLUMN",
    "ROWS",
    "COLUMNS",
    "AREAS",
    "ISREF",
    "ISFORMULA",
    "FORMULATEXT",
    "SHEET",
    "SHEETS",
    "OFFSET",
    "INDIRECT",
    "INDEX",
    "CELL",
    "TRIMRANGE",
    "SUBTOTAL",
    "AGGREGATE",
    "TEXT",
    "LET",
    "LAMBDA",
    "ISOMITTED",
    "MAP",
    "REDUCE",
    "SCAN",
    "BYROW",
    "BYCOL",
    "MAKEARRAY",
];

pub fn is_known_function(n: &str) -> bool {
    is_special(&n.to_ascii_uppercase()) || gridcraft_functions::lookup(n).is_some()
}
