use solvercraft::{
    compile::{Expr, ExpressionConstraint, ExpressionModel},
    types::*,
};
fn t(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn variable(lower: f64, upper: f64) -> Variable {
    Variable { name: "x".into(), lower: Some(lower), upper: Some(upper), kind: VariableKind::Continuous }
}
fn base() -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "independent highlevel".into(),
        variables: vec![variable(0.0, 3.0)],
        objective: Objective { direction: Direction::Maximize, offset: 0.0, linear: vec![t(0, 1.0)], quadratic: vec![] },
        constraints: vec![],
    }
}

#[test]
fn finite_chance_probabilities_match_direct_weighted_scenario_enumeration() {
    use solvercraft::chance::*;
    for (probability, expected) in [(0.0, 0.0), (0.2, 1.0), (0.5, 2.0), (1.0, 3.0)] {
        let problem = ChanceProblem {
            base: base(),
            scenarios: [(0.2, 0.0), (0.3, 1.0), (0.5, 2.0)]
                .iter()
                .map(|(weight, rhs)| ChanceScenario {
                    weight: *weight,
                    constraints: vec![LinearConstraint {
                        name: "scenario upper".into(),
                        terms: vec![t(0, 1.0)],
                        relation: Relation::LessEqual,
                        rhs: *rhs,
                    }],
                })
                .collect(),
            rule: ChanceRule::Joint { maximum_violation_probability: probability },
        };
        let result = solve(&problem, &SolveOptions::default(), &NoControl).unwrap();
        let x = result.solve.values.as_ref().unwrap()[0];
        assert!((x - expected).abs() < 1e-7, "p={probability}: {result:?}");
        let direct: [(f64, f64); 3] = [(0.2, 0.0), (0.3, 1.0), (0.5, 2.0)];
        let mass: f64 = direct.iter().filter(|(_, bound)| x > *bound + 1e-7).map(|(weight, _)| *weight).sum();
        assert!((result.joint_violation_probability.unwrap() - mass).abs() < 1e-12);
    }
}

#[test]
fn recourse_and_benders_match_independent_piecewise_inventory_breakpoints() {
    use solvercraft::stochastic::*;
    for first_cost in [0.5, 1.0, 2.0, 3.0, 5.0] {
        let problem = TwoStageProblem {
            name: "inventory breakpoint oracle".into(),
            direction: Direction::Minimize,
            first_stage_variables: vec![variable(0.0, 3.0)],
            first_stage_costs: vec![first_cost],
            first_stage_offset: 0.0,
            first_stage_constraints: vec![],
            scenarios: [1.0, 2.0]
                .iter()
                .map(|demand| RecourseScenario {
                    name: format!("demand{demand}"),
                    weight: 0.5,
                    offset: 0.0,
                    variables: vec![Variable { name: "shortage".into(), lower: Some(0.0), upper: None, kind: VariableKind::Continuous }],
                    costs: vec![4.0],
                    constraints: vec![RecourseConstraint {
                        name: "inventory balance".into(),
                        first_stage_terms: vec![t(0, 1.0)],
                        recourse_terms: vec![t(0, 1.0)],
                        relation: Relation::GreaterEqual,
                        rhs: *demand,
                    }],
                })
                .collect(),
        };
        let objective = |x: f64| first_cost * x + 2.0 * (1.0 - x).max(0.0) + 2.0 * (2.0 - x).max(0.0);
        let expected = [0.0, 1.0, 2.0, 3.0].into_iter().map(objective).fold(f64::INFINITY, f64::min);
        let extensive = solve_two_stage(&problem, &SolveOptions::default(), &NoControl).unwrap();
        let decomposition = solvercraft::benders::solve(&problem, &SolveOptions::default(), &NoControl).unwrap();
        assert!((extensive.expected_total_cost.unwrap() - expected).abs() < 1e-7, "{extensive:?}");
        assert!(decomposition.converged, "{decomposition:?}");
        assert!((objective(decomposition.first_stage_values.as_ref().unwrap()[0]) - expected).abs() < 1e-7, "{decomposition:?}");
        assert!(decomposition.lower_bound.unwrap() <= expected + 1e-7);
    }
}

#[test]
fn constrained_local_quadratic_endpoints_match_closed_form_without_claiming_global_bounds() {
    for (center, bound, relation) in [(0.5, 1.0, Relation::GreaterEqual), (-0.5, -1.0, Relation::LessEqual)] {
        for direction in [Direction::Minimize, Direction::Maximize] {
            let square = Expr::Power {
                base: Box::new(Expr::Subtract { left: Box::new(Expr::variable(0)), right: Box::new(Expr::constant(center)) }),
                exponent: 2,
            };
            let model = ExpressionModel {
                name: "closed-form constrained endpoint".into(),
                variables: vec![variable(-2.0, 2.0)],
                direction,
                objective: if direction == Direction::Minimize { square } else { Expr::Negate { value: Box::new(square) } },
                constraints: vec![ExpressionConstraint {
                    name: "active limit".into(),
                    left: Expr::variable(0),
                    right: Expr::constant(bound),
                    relation,
                }],
            };
            let result =
                solvercraft::constrained::solve(&model, &SolveOptions::default(), &solvercraft::nonlinear::NlpOptions::default(), &NoControl)
                    .unwrap();
            let x = result.values.as_ref().unwrap()[0];
            assert!((x - bound).abs() < 2e-6, "{result:?}");
            assert_eq!(result.feasibility, Feasibility::Feasible);
            assert_ne!(result.optimality, Optimality::Optimal);
            assert!(result.best_bound.is_none() && result.relative_gap.is_none());
        }
    }
}

#[test]
fn multistart_and_replayed_global_certificate_keep_distinct_guarantees() {
    let model = ExpressionModel {
        name: "double well exact zeros".into(),
        variables: vec![variable(-2.0, 2.0)],
        direction: Direction::Minimize,
        objective: Expr::Power {
            base: Box::new(Expr::Subtract {
                left: Box::new(Expr::Power { base: Box::new(Expr::variable(0)), exponent: 2 }),
                right: Box::new(Expr::constant(1.0)),
            }),
            exponent: 2,
        },
        constraints: vec![],
    };
    let options = SolveOptions::default();
    let local = solvercraft::multistart::solve(
        &model,
        &options,
        &solvercraft::nonlinear::NlpOptions::default(),
        &solvercraft::multistart::MultiStartOptions { starts: 8, seed: 42 },
        &NoControl,
    )
    .unwrap();
    assert!(local.solution.objective.unwrap() < 1e-10);
    assert!(local.solution.best_bound.is_none() && local.solution.relative_gap.is_none());
    assert_ne!(local.solution.optimality, Optimality::Optimal);
    let global = solvercraft::global::solve_box_polynomial(&model, &options, &NoControl).unwrap();
    assert_eq!(global.solution.optimality, Optimality::Optimal);
    assert!(global.solution.objective.unwrap().abs() < 1e-8);
    assert!(solvercraft::global::verify_box_certificate(&model, &global, &options).unwrap());
}
