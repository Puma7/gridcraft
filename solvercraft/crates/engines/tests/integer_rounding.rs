use solvercraft_engines::solve;
use solvercraft_types::*;

fn model(direction: Direction) -> OptimizationModel {
    let sign = if direction == Direction::Maximize { 1.0 } else { -1.0 };
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "near-integral LP is not an integer certificate".into(),
        variables: vec![
            Variable { name: "x".into(), kind: VariableKind::Binary, lower: Some(0.0), upper: Some(1.0) },
            Variable { name: "y".into(), kind: VariableKind::Binary, lower: Some(0.0), upper: Some(1.0) },
        ],
        objective: Objective {
            direction,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: sign * 1e12 }, LinearTerm { variable: 1, coefficient: sign * 500.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "scaled capacity".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1e9 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            relation: Relation::LessEqual,
            rhs: 1.0,
        }],
    }
}

#[test]
fn feasible_rounding_cannot_close_an_lp_node_with_a_material_objective_gap() -> Result<()> {
    // Exhaustive independent integer oracle: x=1 always violates capacity;
    // among x=0 choices, y=1 wins with magnitude500. LP x=1e-9,y=0 has
    // magnitude1000 but lies inside default integer-distance tolerance.
    for (direction, objective) in [(Direction::Maximize, 500.0), (Direction::Minimize, -500.0)] {
        let result = solve(&model(direction), &SolveOptions::default(), &NoControl)?;
        assert_eq!(result.values, Some(vec![0.0, 1.0]));
        assert_eq!(result.objective, Some(objective));
        assert_eq!(result.optimality, Optimality::Optimal);
        assert_eq!(result.best_bound, Some(objective));
        assert_eq!(result.relative_gap, Some(0.0));
    }
    Ok(())
}

#[test]
fn interrupted_near_integral_rounding_retains_incumbent_and_the_unclosed_bound() -> Result<()> {
    let options = SolveOptions { max_nodes: 1, ..Default::default() };
    let result = solve(&model(Direction::Maximize), &options, &NoControl)?;
    assert_eq!(result.termination, Termination::NodeLimit);
    assert_eq!(result.values, Some(vec![0.0, 0.0]));
    assert_eq!(result.objective, Some(0.0));
    assert_eq!(result.optimality, Optimality::BestFound);
    assert!(result.best_bound.is_some_and(|upper| upper >= 500.0));
    assert!(result.relative_gap.is_some_and(|gap| gap > 0.0));
    Ok(())
}

#[test]
fn mixed_conic_near_integral_relaxations_never_certify_the_rounded_zero_point() -> Result<()> {
    use solvercraft_engines::{ConicModel, solve_mixed_conic_owned};
    for maximum_nodes in [1, 32] {
        let options = SolveOptions { max_nodes: maximum_nodes, ..Default::default() };
        let result = solve_mixed_conic_owned(
            &ConicModel { base: model(Direction::Maximize), second_order_cones: vec![] },
            &options,
            std::sync::Arc::new(NoControl),
        )?;
        if result.result.optimality == Optimality::Optimal {
            assert_eq!(result.result.values, Some(vec![0.0, 1.0]));
            assert_eq!(result.result.objective, Some(500.0));
        }
        assert!(result.result.best_bound.is_none_or(|upper| upper >= 500.0));
        assert_ne!(result.result.feasibility, Feasibility::Infeasible);
        if result.result.objective == Some(0.0) {
            assert_ne!(result.result.relative_gap, Some(0.0));
            assert_ne!(result.result.optimality, Optimality::Optimal);
        }
    }
    Ok(())
}
