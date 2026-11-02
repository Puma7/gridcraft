//! GridCraft's calculation engine: the evaluator (references, names, tables, LET/LAMBDA,
//! dynamic arrays) and dependency-driven recalculation with spills and cycle detection.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod eval;
pub mod recalc;
mod trimrange;

pub use eval::solvercraft::{
    PSI_CORE_SIGNATURES, PsiCoreSignature, WorksheetDistribution, WorksheetNominalBasis, distribution_arity, worksheet_distribution,
};
pub use eval::{Area, SPECIAL_FUNCTIONS, is_known_function};
pub use recalc::{Calc, Key, evaluate, evaluate_at, now_serial};

#[cfg(test)]
mod tests;
