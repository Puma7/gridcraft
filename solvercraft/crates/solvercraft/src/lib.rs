//! Workbook-independent optimization, simulation and risk analysis.
#![forbid(unsafe_code)]
/// Version of this SDK, independent of the embedding host's package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub use solvercraft_compile as compile;
pub use solvercraft_engines as engines;
pub use solvercraft_engines::solve;
pub use solvercraft_math as math;
pub use solvercraft_model as model;
pub use solvercraft_simulation as simulation;
pub use solvercraft_types as types;

pub mod benders;
pub mod catalog_inputs;
pub mod chance;
pub mod constrained;
pub mod decision;
pub mod ellipsoid;
pub mod finite;
pub mod global;
pub mod heuristic;
pub mod jobs;
pub mod multiobjective;
pub mod multistart;
pub mod nonlinear;
pub mod project;
pub mod provenance;
pub mod registry;
pub mod risk;
pub mod run_manifest;
pub mod runtime;
pub mod saa;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod stochastic;
pub mod workflow;

#[cfg(test)]
mod runtime_tests;

/// Exact, versioned capabilities. An absent capability is not silently simulated.
pub fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "schema_version": types::SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "implementation": "pure Rust",
        "optimization": ["linear_dense_simplex", "linear_sparse_primal_dual", "mixed_integer_linear", "convex_quadratic_linear_constraints", "socp", "finite_integer_miqp_misocp", "smooth_continuous_finite_bounds_ad", "smooth_constraints_augmented_lagrangian", "finite_difference_fallback", "seeded_multistart", "finite_bounds_differential_evolution", "polynomial_finite_box_interval_certificate", "checked_lp_vertex_basis_sensitivity", "sos_and_indicators", "finite_domains_and_all_different"],
        "engines":registry::engines(),
        "simulation": ["monte_carlo", "latin_hypercube", "randomized_sobol", "gaussian_spearman_correlation", "gaussian_student_archimedean_copulas", "likelihood_fitting_aic_bic_refit_bootstrap", "bounded_hoeffding_stopping", "replicate_mean_intervals", "parameter_grids"],
        "distribution_catalog":math::catalog::distribution_catalog(),
        "risk": ["sample_average_training_and_independent_holdout", "finite_scenario_expected_loss", "finite_scenario_mean_variance", "finite_scenario_cvar", "individual_and_joint_scenario_chance_constraints", "box_budget_ellipsoid_robust_counterparts", "two_stage_linear_recourse", "checked_complete_recourse_benders", "decision_trees_and_information_value", "weighted_lexicographic_epsilon_multiobjective"],
        "runtime":["portable_schema_2_models","validated_schema_1_migration","context_and_configuration_manifests","cooperative_wall_clock_limits","bounded_native_jobs","authenticated_quota_loopback_server","artcraft_parameter_binding_example"],
        "unavailable": ["grg", "unrestricted_nonconvex_global_proof", "unrestricted_incomplete_or_integer_recourse_benders", "complete_frontline_psi_parity"],
        "status": "development alpha; consult per-engine limits and recorded platform evidence"
    })
}
