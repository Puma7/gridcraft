//! Algorithm-specific capability contracts. False means unavailable, not emulated.
use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemClass {
    Linear,
    MixedIntegerLinear,
    SeparableQuadraticBounds,
    ConvexQuadraticLinearConstraints,
    SecondOrderCone,
    MixedIntegerConic,
    SmoothBounds,
    SmoothConstrained,
    NonsmoothFiniteBounds,
    PolynomialFiniteBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCapabilities {
    pub id: String,
    pub version: String,
    pub classes: Vec<ProblemClass>,
    pub integer: bool,
    pub warm_start: bool,
    pub dual_values: bool,
    pub basis: bool,
    pub sensitivity_intervals: bool,
    pub conflict_certificate: bool,
    pub progress: bool,
    pub cancellation: bool,
    pub deterministic: bool,
    pub wasm_compatible_source: bool,
    pub wasm_execution_verified: bool,
    pub guarantee: String,
    pub interface: String,
    pub limits: String,
}

pub fn engines() -> Vec<EngineCapabilities> {
    let mut registered: Vec<_> = [
        (
            "dense_two_phase_simplex",
            vec![ProblemClass::Linear],
            false,
            "Floating-point optimum or unboundedness only after original-model checks; numerical stops remain unknown",
        ),
        (
            "branch_and_bound",
            vec![ProblemClass::MixedIntegerLinear],
            true,
            "Valid relaxation bounds and checked incumbents; reported gap uses original objective units",
        ),
        ("analytic_separable_qp", vec![ProblemClass::SeparableQuadraticBounds], false, "Analytic separable convex bound-QP only"),
        (
            "projected_gradient_ad",
            vec![ProblemClass::SmoothBounds],
            false,
            "Stationarity in supported smooth finite-bound domains, no global optimum certificate",
        ),
        (
            "differential_evolution",
            vec![ProblemClass::NonsmoothFiniteBounds],
            true,
            "Best original-model feasible point found; no optimality or infeasibility proof",
        ),
    ]
    .into_iter()
    .map(|(id, classes, integer, guarantee)| EngineCapabilities {
        id: id.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        classes,
        integer,
        warm_start: false,
        dual_values: false,
        basis: false,
        sensitivity_intervals: false,
        conflict_certificate: false,
        progress: true,
        cancellation: true,
        deterministic: true,
        wasm_compatible_source: true,
        wasm_execution_verified: matches!(id, "dense_two_phase_simplex" | "branch_and_bound" | "projected_gradient_ad"),
        guarantee: guarantee.into(),
        interface: "borrowed RunControl".into(),
        limits: "Validated dimensions, finite arithmetic, checked memory and work budgets apply".into(),
    })
    .collect();
    for (id, classes, integer, warm, duals, basis, sensitivity, wasm_tested, interface, limits, guarantee) in [
        (
            "convex_qp_admm",
            vec![ProblemClass::ConvexQuadraticLinearConstraints],
            false,
            true,
            true,
            false,
            false,
            true,
            "borrowed RunControl",
            "512 dense variables; sufficient PSD recognition; documented numerical range",
            "Original primal/KKT checks and a valid lower bound required for Optimal; otherwise feasible best point or stationarity only",
        ),
        (
            "clarabel_0_11_1",
            vec![ProblemClass::Linear, ProblemClass::ConvexQuadraticLinearConstraints, ProblemClass::SecondOrderCone],
            false,
            false,
            true,
            false,
            false,
            true,
            "owned Arc<dyn RunControl>; native borrowed bridge",
            "Sparse CSC/QDLDL, conservative fill budget, continuous convex models; unverified rays stay Unknown",
            "Independent original-model and dual/cone checks; global bounds only when residual corrections are valid",
        ),
        (
            "finite_integer_conic",
            vec![ProblemClass::MixedIntegerConic],
            true,
            false,
            true,
            false,
            false,
            false,
            "owned Arc<dyn RunControl>",
            "Finite integer bounds; branch-and-bound, no branch-and-cut claim",
            "Certified relaxation bounds and original-feasible incumbents; unresolved leaves prevent Optimal",
        ),
        (
            "checked_lp_vertex_basis",
            vec![ProblemClass::Linear],
            false,
            true,
            true,
            true,
            true,
            false,
            "borrowed RunControl",
            "64 variables, 512 normalized rows, bounded basis search; exact representability checks",
            "Original-coordinate basis reuse plus local RHS/objective intervals; invalid warm states rejected",
        ),
        (
            "augmented_lagrangian_ad_fd",
            vec![ProblemClass::SmoothConstrained],
            false,
            false,
            false,
            false,
            false,
            true,
            "borrowed RunControl",
            "Continuous finite bounds and supported smooth expressions; local method",
            "Original feasibility, complementarity and projected KKT checks before Stationary; no global bound",
        ),
        (
            "polynomial_box_interval",
            vec![ProblemClass::PolynomialFiniteBox],
            false,
            false,
            false,
            false,
            false,
            false,
            "borrowed RunControl",
            "Finite boxes and supported polynomials; bounded tree and replay witnesses",
            "Outward interval bounds with replayable partition/incumbent verification; no local-as-global substitution",
        ),
    ] {
        registered.push(EngineCapabilities {
            id: id.into(),
            version: if id == "clarabel_0_11_1" { "0.11.1".into() } else { env!("CARGO_PKG_VERSION").into() },
            classes,
            integer,
            warm_start: warm,
            dual_values: duals,
            basis,
            sensitivity_intervals: sensitivity,
            conflict_certificate: false,
            progress: true,
            cancellation: true,
            deterministic: true,
            wasm_compatible_source: true,
            wasm_execution_verified: wasm_tested,
            guarantee: guarantee.into(),
            interface: interface.into(),
            limits: limits.into(),
        });
    }
    registered
}

pub fn select(class: ProblemClass, preferred: Option<&str>) -> Result<EngineCapabilities> {
    engines()
        .into_iter()
        .find(|engine| preferred.is_none_or(|id| engine.id == id) && engine.classes.contains(&class))
        .ok_or_else(|| SolverError::new("unsupported_engine", "No registered engine supports the requested problem class and selection"))
}
