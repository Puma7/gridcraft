use solvercraft_types::*;
#[test]
fn finite_scenario_cvar_epigraph_survives_exact_original_arithmetic() -> Result<()> {
    let var = |name: &str, lower, upper| Variable { name: name.into(), kind: VariableKind::Continuous, lower, upper };
    let terms = |items: &[(usize, f64)]| {
        items.iter().map(|(variable, coefficient)| LinearTerm { variable: *variable, coefficient: *coefficient }).collect::<Vec<_>>()
    };
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "two-scenario CVaR exact bridge".into(),
        variables: vec![var("x", Some(0.0), Some(2.0)), var("eta", None, None), var("u0", Some(0.0), None), var("u1", Some(0.0), None)],
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: terms(&[(1, 1.0), (2, 1.0), (3, 1.0)]), quadratic: vec![] },
        constraints: vec![
            LinearConstraint { name: "scenario0".into(), terms: terms(&[(0, 1.0), (1, -1.0), (2, -1.0)]), relation: Relation::LessEqual, rhs: 0.0 },
            LinearConstraint { name: "scenario1".into(), terms: terms(&[(0, -3.0), (1, -1.0), (3, -1.0)]), relation: Relation::LessEqual, rhs: -4.0 },
        ],
    };
    let result = solvercraft_engines::solve(&model, &SolveOptions::default(), &NoControl)?;
    assert_eq!(result.optimality, Optimality::Optimal, "{result:?}");
    assert!(result.values.as_ref().is_some_and(|x| (x[0] - 1.0).abs() < 1e-8));
    Ok(())
}
