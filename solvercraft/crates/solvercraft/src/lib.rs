//! Workbook-independent optimization, simulation and risk analysis.
#![forbid(unsafe_code)]
pub use solvercraft_compile as compile;
pub use solvercraft_engines as engines;
pub use solvercraft_engines::solve;
pub use solvercraft_math as math;
pub use solvercraft_model as model;
pub use solvercraft_simulation as simulation;
pub use solvercraft_types as types;

pub mod heuristic;
pub mod jobs;
pub mod nonlinear;
pub mod provenance;
pub mod risk;

#[cfg(test)]
mod runtime_tests;

/// Exact, versioned capabilities. An absent capability is not silently simulated.
pub fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "schema_version": types::SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "implementation": "pure Rust",
        "optimization": ["linear", "mixed_integer_linear", "separable_convex_quadratic_bounds", "smooth_continuous_finite_bounds_ad", "finite_bounds_differential_evolution"],
        "simulation": ["monte_carlo", "latin_hypercube"],
        "risk": ["finite_scenario_expected_loss", "finite_scenario_cvar", "box_robust_linear_counterpart"],
        "unavailable": ["grg", "general_qp", "socp", "global_nonlinear_proof", "copulas", "sobol", "basis_sensitivity", "general_smooth_nonlinear_constraints"],
        "status": "alpha; numerical and platform limits documented"
    })
}
