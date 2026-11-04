use solvercraft::{stochastic::*, types::*};
fn variable(name: &str, lower: Option<f64>, upper: Option<f64>) -> Variable {
    Variable { name: name.into(), lower, upper, kind: VariableKind::Continuous }
}
fn term(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn inventory() -> TwoStageProblem {
    TwoStageProblem {
        name: "two-stage inventory".into(),
        direction: Direction::Minimize,
        first_stage_variables: vec![variable("order", Some(0.0), Some(10.0))],
        first_stage_costs: vec![2.0],
        first_stage_offset: 0.0,
        first_stage_constraints: vec![],
        scenarios: [(3.0, 1.0), (8.0, 1.0)]
            .into_iter()
            .map(|(demand, weight)| RecourseScenario {
                name: format!("demand {demand}"),
                weight,
                variables: vec![variable("shortage", Some(0.0), None)],
                costs: vec![6.0],
                offset: 0.0,
                constraints: vec![RecourseConstraint {
                    name: "satisfy demand".into(),
                    first_stage_terms: vec![term(0, 1.0)],
                    recourse_terms: vec![term(0, 1.0)],
                    relation: Relation::GreaterEqual,
                    rhs: demand,
                }],
            })
            .collect(),
    }
}
#[test]
fn recourse_matches_independent_piecewise_newsvendor_oracle() {
    let problem = inventory();
    let result = solve_two_stage(&problem, &SolveOptions::default(), &NoControl).expect("solve");
    assert_eq!(result.solve.optimality, Optimality::Optimal);
    let x = result.first_stage_values.expect("stage one")[0];
    // Independent enumeration of all breakpoints of the convex piecewise-linear cost.
    let candidates = [0.0_f64, 3.0, 8.0, 10.0];
    let oracle = candidates
        .into_iter()
        .map(|q| (q, 2.0 * q + 3.0 * (3.0 - q).max(0.0) + 3.0 * (8.0 - q).max(0.0)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("oracle");
    assert!((x - oracle.0).abs() < 1e-8);
    assert!((result.expected_total_cost.expect("cost") - oracle.1).abs() < 1e-8);
    let outcomes = result.outcomes.expect("outcomes");
    assert!(outcomes.iter().all(|o| o.values[0].abs() < 1e-8));
}
#[test]
fn first_stage_is_shared_and_cannot_use_future_demand() {
    let problem = inventory();
    let compiled = compile_two_stage(&problem, &SolveOptions::default()).expect("compile");
    assert_eq!(compiled.first_stage_length, 1);
    assert_eq!(compiled.model.variables.len(), 3);
    assert_eq!(compiled.model.constraints[0].terms[0].variable, 0);
    assert_eq!(compiled.model.constraints[1].terms[0].variable, 0);
    assert_eq!(compiled.scenarios[0].start, 1);
    assert_eq!(compiled.scenarios[1].start, 2);
    let actual = solve_two_stage(&problem, &SolveOptions::default(), &NoControl).expect("solve").expected_total_cost.expect("cost");
    let perfect_information = (2.0 * 3.0 + 2.0 * 8.0) / 2.0;
    assert!((actual - 16.0).abs() < 1e-8);
    assert!(actual > perfect_information);
}
#[test]
fn recourse_weights_support_maximum_and_subnormal_equal_mass() {
    for weight in [1.0, f64::MAX, f64::from_bits(1)] {
        let mut problem = inventory();
        for scenario in &mut problem.scenarios {
            scenario.weight = weight;
        }
        let result = solve_two_stage(&problem, &SolveOptions::default(), &NoControl).expect("weights");
        assert!((result.expected_total_cost.expect("cost") - 16.0).abs() < 1e-8);
    }
}
#[test]
fn recourse_dimension_handles_precision_and_memory_are_rejected() {
    let mut problem = inventory();
    problem.scenarios[0].constraints[0].first_stage_terms[0].variable = 1;
    assert_eq!(compile_two_stage(&problem, &SolveOptions::default()).expect_err("handle").code, "invalid_two_stage");
    let problem = inventory();
    assert_eq!(
        compile_two_stage(&problem, &SolveOptions { max_memory_bytes: 1, ..SolveOptions::default() }).expect_err("memory").code,
        "memory_limit"
    );
    let mut problem = inventory();
    problem.scenarios[0].weight = f64::from_bits(1);
    problem.scenarios[1].weight = f64::MAX;
    assert_eq!(compile_two_stage(&problem, &SolveOptions::default()).expect_err("precision").code, "numerical_failure");
}
#[test]
fn recourse_strict_inputs_and_integer_first_stage_are_supported() {
    let mut problem = inventory();
    problem.first_stage_variables[0].kind = VariableKind::Integer;
    let result = solve_two_stage(&problem, &SolveOptions::default(), &NoControl).expect("integer");
    assert_eq!(result.first_stage_values.expect("x"), vec![8.0]);
    let mut json = serde_json::to_value(&problem).expect("serialize");
    json["scenarios"][0]["future_knowledge"] = serde_json::json!(true);
    assert!(serde_json::from_value::<TwoStageProblem>(json).is_err());
}
#[test]
fn benders_multicut_matches_deterministic_equivalent_and_breakpoint_oracle() {
    let problem = inventory();
    let decomposed = solvercraft::benders::solve(&problem, &SolveOptions::default(), &NoControl).expect("Benders");
    assert!(decomposed.converged);
    assert_eq!(decomposed.termination, Termination::Completed);
    assert!(decomposed.cuts > 0);
    assert!((decomposed.objective.expect("cost") - 16.0).abs() < 1e-7);
    assert!((decomposed.lower_bound.expect("lower") - 16.0).abs() < 1e-7);
    assert!((decomposed.first_stage_values.expect("x")[0] - 8.0).abs() < 1e-7);
}
#[test]
fn benders_rejects_integer_or_unproven_complete_recourse_and_honors_cancel() {
    let mut problem = inventory();
    problem.scenarios[0].variables[0].kind = VariableKind::Integer;
    assert_eq!(
        solvercraft::benders::solve(&problem, &SolveOptions::default(), &NoControl).expect_err("integer recourse").code,
        "unsupported_benders_structure"
    );
    let mut problem = inventory();
    problem.scenarios[0].constraints[0].recourse_terms[0].coefficient = -1.0;
    assert!(solvercraft::benders::solve(&problem, &SolveOptions::default(), &NoControl).is_err());
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let result = solvercraft::benders::solve(&inventory(), &SolveOptions::default(), &Cancel).expect("cancel");
    assert!(!result.converged);
    assert_eq!(result.termination, Termination::Cancelled);
    assert!(result.objective.is_none());
}
#[test]
fn benders_moves_scenario_constants_outside_epigraph_to_preserve_cancellation() {
    let mut problem = inventory();
    let scenario = problem.scenarios[1].clone();
    problem.scenarios = vec![scenario.clone(), scenario.clone(), scenario];
    for (scenario, offset) in problem.scenarios.iter_mut().zip([1e17, -1.0, -1e17]) {
        scenario.offset = offset;
    }
    let solved = solvercraft::benders::solve(&problem, &SolveOptions::default(), &NoControl).expect("Benders constants");
    assert!(solved.converged);
    assert!((solved.objective.expect("cost") - (16.0 - 1.0 / 3.0)).abs() < 1e-8);
}
#[test]
fn benders_two_decisions_and_integer_stage_match_independent_oracles() {
    let mut p = inventory();
    p.first_stage_variables[0].kind = VariableKind::Integer;
    p.first_stage_costs[0] = 1.2;
    p.first_stage_variables[0].upper = Some(4.0);
    for (scenario, (demand, weight)) in p.scenarios.iter_mut().zip([(1.0, 1.0), (4.0, 3.0)]) {
        scenario.constraints[0].rhs = demand;
        scenario.weight = weight;
        scenario.costs[0] = 2.0;
    }
    let result = solvercraft::benders::solve(&p, &SolveOptions::default(), &NoControl).expect("integer Benders");
    let oracle = (0..=4)
        .map(|q| {
            let q = q as f64;
            1.2 * q + 0.5 * (1.0 - q).max(0.0) + 1.5 * (4.0 - q).max(0.0)
        })
        .fold(f64::INFINITY, f64::min);
    assert!(result.converged);
    assert!((result.objective.expect("cost") - oracle).abs() < 1e-7);
    let p = TwoStageProblem {
        name: "two shared resources".into(),
        direction: Direction::Minimize,
        first_stage_variables: vec![variable("x", Some(0.0), Some(3.0)), variable("y", Some(0.0), Some(3.0))],
        first_stage_costs: vec![0.0, 0.0],
        first_stage_offset: 0.0,
        first_stage_constraints: vec![LinearConstraint {
            name: "resource".into(),
            terms: vec![term(0, 1.0), term(1, 1.0)],
            relation: Relation::LessEqual,
            rhs: 3.0,
        }],
        scenarios: [(3.0, 0.0), (0.0, 3.0)]
            .into_iter()
            .map(|(a, b)| RecourseScenario {
                name: "demand vector".into(),
                weight: 1.0,
                variables: vec![variable("short x", Some(0.0), None), variable("short y", Some(0.0), None)],
                costs: vec![4.0, 4.0],
                offset: 0.0,
                constraints: vec![
                    RecourseConstraint {
                        name: "demand x".into(),
                        first_stage_terms: vec![term(0, 1.0)],
                        recourse_terms: vec![term(0, 1.0)],
                        relation: Relation::GreaterEqual,
                        rhs: a,
                    },
                    RecourseConstraint {
                        name: "demand y".into(),
                        first_stage_terms: vec![term(1, 1.0)],
                        recourse_terms: vec![term(1, 1.0)],
                        relation: Relation::GreaterEqual,
                        rhs: b,
                    },
                ],
            })
            .collect(),
    };
    let benders = solvercraft::benders::solve(&p, &SolveOptions::default(), &NoControl).expect("Benders");
    let de = solve_two_stage(&p, &SolveOptions::default(), &NoControl).expect("DE");
    assert!(benders.converged);
    assert!((benders.objective.expect("Benders cost") - 6.0).abs() < 1e-7);
    assert!((de.expected_total_cost.expect("DE cost") - 6.0).abs() < 1e-7);
}
