//! Independent analytic/exhaustive checks of public engine certificates.
use solvercraft_engines::*;
use solvercraft_types::*;
use std::sync::Arc;
fn var(name: &str, lower: Option<f64>, upper: Option<f64>, kind: VariableKind) -> Variable {
    Variable { name: name.into(), lower, upper, kind }
}
fn term(variable: usize, coefficient: f64) -> LinearTerm {
    LinearTerm { variable, coefficient }
}
fn base(variables: Vec<Variable>, direction: Direction, linear: Vec<LinearTerm>) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: String::new(),
        variables,
        objective: Objective { direction, offset: 0.0, linear, quadratic: vec![] },
        constraints: vec![],
    }
}
#[test]
fn cone_validator_enforces_allocation_budget_before_component_scratch() {
    let model = ConicModel {
        base: base(vec![], Direction::Minimize, vec![]),
        second_order_cones: vec![SecondOrderConeConstraint {
            name: "scratch budget".into(),
            components: vec![AffineExpression { offset: 0.0, terms: vec![] }; 20_000],
            upper: AffineExpression { offset: 0.0, terms: vec![] },
        }],
    };
    let options = SolveOptions { max_memory_bytes: 1, ..Default::default() };
    // Merely collecting components consumes at least160000 bytes. The public
    // original-model validator must honor the caller's one-byte work budget.
    assert!(validate_conic_solution(&model, &[], &options).is_err());
}
#[test]
fn warm_lp_basis_does_not_bypass_explicit_method_validation() {
    let model = base(vec![var("x", Some(0.0), Some(1.0), VariableKind::Continuous)], Direction::Minimize, vec![term(0, -1.0)]);
    let initial = solve_lp_with_basis(&model, &SolveOptions::default(), &NoControl, None).expect("initial basis");
    let basis = initial.basis.expect("basis");
    for method in [SolveMethod::ProjectedGradient, SolveMethod::DifferentialEvolution] {
        let options = SolveOptions { method, ..Default::default() };
        assert_eq!(
            solve_lp_with_basis(&model, &options, &NoControl, None).expect_err("cold request rejects unrelated method").code,
            "unsupported_method"
        );
        assert_eq!(
            solve_lp_with_basis(&model, &options, &NoControl, Some(&basis)).expect_err("warm request must reject the same unrelated method").code,
            "unsupported_method"
        );
    }
}
#[test]
fn conic_certified_bound_encloses_analytic_circle_optimum_in_both_directions() {
    for direction in [Direction::Minimize, Direction::Maximize] {
        for coefficients in [[3.0, 4.0], [-4.0, 3.0], [0.0, -5.0]] {
            let model = ConicModel {
                base: base(
                    vec![var("x", Some(-2.0), Some(2.0), VariableKind::Continuous), var("y", Some(-2.0), Some(2.0), VariableKind::Continuous)],
                    direction,
                    vec![term(0, coefficients[0]), term(1, coefficients[1])],
                ),
                second_order_cones: vec![SecondOrderConeConstraint {
                    name: "unit circle".into(),
                    components: vec![
                        AffineExpression { offset: 0.0, terms: vec![term(0, 1.0)] },
                        AffineExpression { offset: 0.0, terms: vec![term(1, 1.0)] },
                    ],
                    upper: AffineExpression { offset: 1.0, terms: vec![] },
                }],
            };
            let solved = solve_socp_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).expect("SOCP");
            let truth = if direction == Direction::Minimize { -5.0 } else { 5.0 };
            let point = solved.result.values.as_ref().expect("feasible point");
            assert!(point[0].hypot(point[1]) <= 1.0 + 1e-7);
            let bound = solved.result.best_bound.expect("restored finite-box bound");
            if direction == Direction::Minimize {
                assert!(bound <= truth + 1e-9, "invalid lower bound {bound}, true{truth}, {solved:?}");
            } else {
                assert!(bound >= truth - 1e-9, "invalid upper bound {bound}, true{truth}, {solved:?}");
            }
            assert!((solved.result.objective.expect("objective") - truth).abs() < 1e-6);
        }
    }
}
fn mixed_example(direction: Direction, seed: i32) -> ConicModel {
    let sign = if direction == Direction::Minimize { 1.0 } else { -1.0 };
    let mut model = base(
        vec![var("x", Some(0.0), Some(1.0), VariableKind::Binary), var("y", Some(0.0), Some(1.0), VariableKind::Binary)],
        direction,
        vec![term(0, sign * (-3.0 + (seed % 3) as f64)), term(1, sign * (-4.0 + (seed % 5) as f64))],
    );
    model.objective.offset = 7.0;
    model.objective.quadratic = vec![
        QuadraticTerm { left: 0, right: 0, coefficient: sign * 2.0 },
        QuadraticTerm { left: 1, right: 1, coefficient: sign * 2.0 },
        QuadraticTerm { left: 0, right: 1, coefficient: sign * 0.5 },
    ];
    model.constraints.push(LinearConstraint {
        name: "choose".into(),
        terms: vec![term(0, 1.0), term(1, 1.0)],
        relation: Relation::GreaterEqual,
        rhs: 0.25,
    });
    ConicModel {
        base: model,
        second_order_cones: vec![SecondOrderConeConstraint {
            name: "asymmetric disk".into(),
            components: vec![
                AffineExpression { offset: -0.2, terms: vec![term(0, 1.0)] },
                AffineExpression { offset: -0.3, terms: vec![term(1, 1.0)] },
            ],
            upper: AffineExpression { offset: 0.9, terms: vec![] },
        }],
    }
}
fn exhaustive_binary(model: &ConicModel) -> f64 {
    let mut best = if model.base.objective.direction == Direction::Minimize { f64::INFINITY } else { f64::NEG_INFINITY };
    for x in [0.0_f64, 1.0] {
        for y in [0.0_f64, 1.0] {
            if x + y < 0.25 || (x - 0.2).hypot(y - 0.3) > 0.9 {
                continue;
            }
            let point = [x, y];
            let mut value = model.base.objective.offset;
            for t in &model.base.objective.linear {
                value += t.coefficient * point[t.variable];
            }
            for t in &model.base.objective.quadratic {
                value += t.coefficient * point[t.left] * point[t.right];
            }
            best = if model.base.objective.direction == Direction::Minimize { best.min(value) } else { best.max(value) };
        }
    }
    best
}
#[test]
fn mixed_conic_bounds_never_cross_exhaustive_binary_optima_even_when_interrupted() {
    for direction in [Direction::Minimize, Direction::Maximize] {
        for seed in 0..8 {
            let model = mixed_example(direction, seed);
            let truth = exhaustive_binary(&model);
            for nodes in [1, 100] {
                let options = SolveOptions { max_nodes: nodes, ..Default::default() };
                let report = solve_mixed_conic_owned(&model, &options, Arc::new(NoControl)).expect("mixed conic");
                assert_ne!(report.result.feasibility, Feasibility::Infeasible, "enumerated feasible model, {report:?}");
                assert_ne!(report.result.optimality, Optimality::Unbounded);
                if let Some(bound) = report.result.best_bound {
                    if direction == Direction::Minimize {
                        assert!(bound <= truth + 1e-8, "bad global lower{bound}, true{truth}, {report:?}");
                    } else {
                        assert!(bound >= truth - 1e-8, "bad global upper{bound}, true{truth}, {report:?}");
                    }
                }
                if let Some(value) = report.result.objective {
                    if direction == Direction::Minimize {
                        assert!(value >= truth - 1e-7);
                    } else {
                        assert!(value <= truth + 1e-7);
                    }
                }
                if report.result.optimality == Optimality::Optimal {
                    assert!((report.result.objective.expect("incumbent") - truth).abs() < 1e-7);
                }
                if nodes == 100 {
                    assert!((report.result.objective.expect("complete finite search incumbent") - truth).abs() < 1e-6);
                }
            }
        }
    }
}
fn basis_example() -> OptimizationModel {
    let mut model = base(
        vec![var("x", Some(0.0), Some(2.0), VariableKind::Continuous), var("y", Some(0.0), Some(3.0), VariableKind::Continuous)],
        Direction::Maximize,
        vec![term(0, 3.0), term(1, 2.0)],
    );
    model.constraints.push(LinearConstraint {
        name: "resource".into(),
        terms: vec![term(0, 1.0), term(1, 1.0)],
        relation: Relation::LessEqual,
        rhs: 4.0,
    });
    model
}
#[test]
fn lp_basis_rhs_perturbations_match_piecewise_oracle_and_forgery_is_rejected() {
    let model = basis_example();
    let options = SolveOptions::default();
    let original = solve_lp_with_basis(&model, &options, &NoControl, None).expect("basis");
    let basis = original.basis.expect("active basis");
    for rhs in [3.5, 3.75, 4.0, 4.25, 4.5, 7.0] {
        let mut updated = model.clone();
        updated.constraints[0].rhs = rhs;
        let report = solve_lp_with_basis(&updated, &options, &NoControl, Some(&basis)).expect("warm");
        // Coefficient3>2 assigns x first, then y, independently of any backend.
        let x = rhs.min(2.0);
        let y = (rhs - x).min(3.0);
        let truth = 3.0 * x + 2.0 * y;
        assert!((report.result.objective.expect("obj") - truth).abs() < 1e-8);
        assert!(report.result.best_bound.expect("upper") >= truth - 1e-8);
        if rhs <= 4.5 {
            assert!(report.warm_start_used);
        } else {
            assert!(!report.warm_start_used);
        }
    }
    let sensitivity = original.sensitivity.expect("sensitivity");
    assert!((sensitivity.rhs_shadow_prices[0] - 2.0).abs() < 1e-10);
    let mut forged = basis.clone();
    forged.active_rows = vec![0, 0];
    assert_eq!(solve_lp_with_basis(&model, &options, &NoControl, Some(&forged)).expect_err("duplicate basis").code, "invalid_warm_start");
    let mut forged = basis.clone();
    forged.row_coefficients[0][0] = f64::NAN;
    assert_eq!(solve_lp_with_basis(&model, &options, &NoControl, Some(&forged)).expect_err("NaN basis").code, "invalid_warm_start");
    let mut changed = model.clone();
    changed.objective.linear[0].coefficient = 0.5;
    let report = solve_lp_with_basis(&changed, &options, &NoControl, Some(&basis)).expect("objective update");
    assert!(!report.warm_start_used);
    assert!((report.result.objective.expect("obj") - 6.5).abs() < 1e-8);
}
#[test]
fn finite_tiny_blocker_prevents_unbounded_ray_and_cancel_never_claims_optimal() {
    let mut model = base(vec![var("x", Some(0.0), None, VariableKind::Continuous)], Direction::Maximize, vec![term(0, 1.0)]);
    model.constraints.push(LinearConstraint {
        name: "tiny finite blocker".into(),
        terms: vec![term(0, 1e-20)],
        relation: Relation::LessEqual,
        rhs: 1.0,
    });
    let result = solve_sparse_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).expect("finite sparse model");
    assert_ne!(result.result.optimality, Optimality::Unbounded);
    assert_ne!(result.result.feasibility, Feasibility::Infeasible);
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let mixed = mixed_example(Direction::Minimize, 0);
    let report = solve_mixed_conic_owned(&mixed, &SolveOptions::default(), Arc::new(Cancel)).expect("cancel");
    assert_eq!(report.result.termination, Termination::Cancelled);
    assert_ne!(report.result.optimality, Optimality::Optimal);
    assert_eq!(report.relaxation_solves, 0);
}
#[test]
fn big_m_compilation_never_cuts_off_an_original_feasible_active_point() {
    for upper in [10.0, 1e20] {
        let input = DiscreteConicModel {
            model: ConicModel {
                base: base(
                    vec![var("x", Some(0.0), Some(upper), VariableKind::Continuous), var("b", Some(0.0), Some(1.0), VariableKind::Binary)],
                    Direction::Maximize,
                    vec![term(0, 1.0)],
                ),
                second_order_cones: vec![],
            },
            indicators: vec![IndicatorConstraint {
                binary: 1,
                active_value: true,
                constraint: LinearConstraint { name: "active x limit".into(), terms: vec![term(0, 1.0)], relation: Relation::LessEqual, rhs: 3.0 },
            }],
            special_ordered_sets: vec![],
        };
        let options = SolveOptions::default();
        let point = [3.0, 1.0];
        assert!(validate_discrete_solution(&input, &point, &options).expect("original check").0);
        let compiled = compile_discrete(&input, &options).expect("finite-bound compilation");
        let mut extended = point.to_vec();
        extended.resize(compiled.model.base.variables.len(), 0.0);
        assert!(
            solvercraft_model::validate_solution(&compiled.model.base, &extended, &options).expect("compiled check").feasible,
            "Compiled Big M cut off original active x=3,b=1 at upper{upper}; {:?}",
            compiled.model.base.constraints
        );
    }
}
#[test]
fn fixed_basis_cancellation_certificate_accounts_for_product_rounding() {
    let coefficient = 9_007_199_254_740_991.0;
    let x = 1.0 + f64::EPSILON;
    let model = base(
        vec![var("x", Some(x), Some(x), VariableKind::Continuous), var("y", Some(1.0), Some(1.0), VariableKind::Continuous)],
        Direction::Minimize,
        vec![term(0, coefficient), term(1, -9_007_199_254_740_992.0)],
    );
    // Exact dyadic input arithmetic: (2^53-1)*(1+2^-52)-2^53 =1-2^-52.
    // Multiplying each f64 term first rounds the first product to2^53 and loses1.
    let truth = 1.0 - f64::EPSILON;
    let options = SolveOptions::default();
    let initial = match solve_lp_with_basis(&model, &options, &NoControl, None) {
        Ok(result) => result,
        Err(error) => {
            assert_eq!(error.code, "numeric_range");
            return;
        }
    };
    let basis = initial.basis.expect("fixed basis");
    let warm = match solve_lp_with_basis(&model, &options, &NoControl, Some(&basis)) {
        Ok(result) => result,
        Err(error) => {
            assert_eq!(error.code, "numeric_range");
            return;
        }
    };
    if warm.result.optimality == Optimality::Optimal {
        assert!(
            (warm.result.objective.expect("objective") - truth).abs() <= options.optimality_tolerance,
            "Claimed precision ignores product residual; {warm:?}"
        );
    }
}
#[test]
fn exact_products_independent_carries_subnormal_ties_and_huge_cancellations() {
    use solvercraft_model::ExactProducts;
    let tiny = f64::from_bits(1);
    for (coefficient, expected) in [(1.0, 0_u64), (3.0, 2), (5.0, 2), (7.0, 4)] {
        let mut sum = ExactProducts::default();
        sum.add_product(coefficient * tiny, 0.5).expect("half subnormal");
        assert_eq!(sum.finish().expect("rounded").to_bits(), expected);
    }
    let mut sum = ExactProducts::default();
    for _ in 0..4 {
        sum.add_triple(tiny, 0.5, 0.5).expect("quarter");
    }
    assert_eq!(sum.finish().expect("collective underflow"), tiny);
    for reverse in [false, true] {
        let mut sum = ExactProducts::default();
        if reverse {
            sum.add(tiny).expect("tiny");
        }
        for _ in 0..1000 {
            sum.add_triple(f64::MAX, f64::MAX, f64::MAX).expect("huge positive triple");
        }
        for _ in 0..1000 {
            sum.add_triple(-f64::MAX, f64::MAX, f64::MAX).expect("huge negative triple");
        }
        if !reverse {
            sum.add(tiny).expect("tiny");
        }
        assert_eq!(sum.finish().expect("exact overflow cancellation"), tiny);
    }
    let model = base(
        vec![
            var("x", Some(1.0 + f64::EPSILON), Some(1.0 + f64::EPSILON), VariableKind::Continuous),
            var("y", Some(1.0), Some(1.0), VariableKind::Continuous),
        ],
        Direction::Minimize,
        vec![term(0, 9_007_199_254_740_991.0), term(1, -9_007_199_254_740_992.0)],
    );
    assert_eq!(solvercraft_model::evaluate_objective(&model, &[1.0 + f64::EPSILON, 1.0]).expect("original dyadic objective"), 1.0 - f64::EPSILON);
    assert_eq!(std::mem::size_of::<solvercraft_model::CompensatedSum>(), 16, "streaming moment allocations retain their original budget");
}
