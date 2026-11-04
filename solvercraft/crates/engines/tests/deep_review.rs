//! Independent numerical cross-review regressions and exact small LP reference.
// Exact reference arithmetic is limited to deliberately small integer coefficients.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use solvercraft_engines::solve;
use solvercraft_types::*;

fn var(name: &str, kind: VariableKind, low: Option<f64>, high: Option<f64>) -> Variable {
    Variable { name: name.into(), kind, lower: low, upper: high }
}
fn row(terms: &[(usize, f64)], relation: Relation, rhs: f64) -> LinearConstraint {
    LinearConstraint {
        name: "review row".into(),
        terms: terms.iter().map(|(variable, coefficient)| LinearTerm { variable: *variable, coefficient: *coefficient }).collect(),
        relation,
        rhs,
    }
}
fn problem(variables: Vec<Variable>, direction: Direction, cost: &[f64], constraints: Vec<LinearConstraint>) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "independent deep review".into(),
        variables,
        constraints,
        objective: Objective {
            direction,
            offset: 0.0,
            linear: cost.iter().enumerate().map(|(variable, coefficient)| LinearTerm { variable, coefficient: *coefficient }).collect(),
            quadratic: Vec::new(),
        },
    }
}
fn run(input: &OptimizationModel) -> SolveResult {
    solve(input, &SolveOptions::default(), &NoControl).expect("valid review model")
}
fn compensated_shift_case(order: bool) -> OptimizationModel {
    let middle = if order { [-1e16, 1.0] } else { [1.0, -1e16] };
    problem(
        vec![
            var("a", VariableKind::Continuous, Some(1e16), Some(1e16)),
            var("b", VariableKind::Continuous, Some(middle[0]), Some(middle[0])),
            var("c", VariableKind::Continuous, Some(middle[1]), Some(middle[1])),
            var("decision", VariableKind::Continuous, Some(0.0), Some(1.0)),
        ],
        Direction::Minimize,
        &[0.0, 0.0, 0.0, 1.0],
        vec![row(&[(0, 1.0), (1, 1.0), (2, 1.0), (3, 1.0)], Relation::GreaterEqual, 1.0)],
    )
}
#[test]
fn compensated_fixed_offset_does_not_change_feasible_region() {
    let input = compensated_shift_case(false);
    let reference = solvercraft_model::validate_solution(&input, &[1e16, 1.0, -1e16, 0.0], &SolveOptions::default()).expect("reference validation");
    let result = run(&input);
    eprintln!("FIXED_SHIFT reference={reference:?} result={result:?}");
    assert!(reference.feasible);
    assert_eq!(reference.objective, 0.0);
    assert_eq!(result.optimality, Optimality::Optimal);
    assert_eq!(result.objective, Some(0.0));
}
#[test]
fn permuting_fixed_variables_is_a_legitimate_control() {
    let result = run(&compensated_shift_case(true));
    eprintln!("FIXED_SHIFT_CONTROL {result:?}");
    assert_eq!(result.objective, Some(0.0));
    assert_eq!(result.optimality, Optimality::Optimal);
}
#[test]
fn rhs_and_fixed_offsets_are_aggregated_in_a_single_compensated_sum() {
    let input = problem(
        vec![
            var("large", VariableKind::Continuous, Some(1e16), Some(1e16)),
            var("small", VariableKind::Continuous, Some(1.0), Some(1.0)),
            var("decision", VariableKind::Continuous, Some(-2.0), Some(0.0)),
        ],
        Direction::Maximize,
        &[0.0, 0.0, 1.0],
        vec![row(&[(0, 1.0), (1, 1.0), (2, 1.0)], Relation::LessEqual, 1e16)],
    );
    let result = run(&input);
    eprintln!("RHS_OFFSET_CANCELLATION {result:?}");
    assert_eq!(result.optimality, Optimality::Optimal);
    assert_eq!(result.objective, Some(-1.0));
}
#[test]
fn cancellation_does_not_validate_an_actually_infeasible_constraint() {
    let mut input = compensated_shift_case(false);
    input.constraints[0].relation = Relation::LessEqual;
    input.constraints[0].rhs = 0.0;
    // The exact fixed contribution is 1, and x>=0, so 1+x<=0 is impossible.
    let result = run(&input);
    eprintln!("CANCELLATION_FALSE_FEASIBILITY {result:?}");
    assert_ne!(result.feasibility, Feasibility::Feasible);
}
#[test]
fn objective_cancellation_keeps_a_meaningful_reduced_cost() {
    let input = problem(
        vec![var("x", VariableKind::Continuous, Some(0.0), Some(1.0)); 3],
        Direction::Maximize,
        &[1e16, 1.0, -1e16],
        vec![row(&[(0, 1.0), (1, -1.0)], Relation::Equal, 0.0), row(&[(1, 1.0), (2, -1.0)], Relation::Equal, 0.0)],
    );
    let result = run(&input);
    eprintln!("OBJECTIVE_CANCELLATION {result:?}");
    if result.optimality == Optimality::Optimal {
        assert_eq!(result.objective, Some(1.0));
    } else {
        assert_eq!(result.termination, Termination::NumericalFailure);
    }
}
#[test]
fn tiny_reduced_cost_does_not_claim_zero_gap_over_a_wide_interval() {
    let input = problem(vec![var("x", VariableKind::Continuous, Some(0.0), Some(1e12))], Direction::Minimize, &[-1e-9], Vec::new());
    let result = run(&input);
    eprintln!("TINY_COST_WIDE_BOUND {result:?}");
    if result.optimality == Optimality::Optimal {
        assert!(result.objective.is_some_and(|x| (x + 1000.0).abs() < 1e-6));
    } else {
        assert_eq!(result.termination, Termination::NumericalFailure);
    }
}
fn large_integer_case(base: f64) -> OptimizationModel {
    problem(
        vec![
            var("fixed", VariableKind::Integer, Some(base + 2.0), Some(base + 2.0)),
            var("decision", VariableKind::Integer, Some(base), Some(base + 2.0)),
        ],
        Direction::Maximize,
        &[0.0, 1.0],
        vec![row(&[(0, 1.0), (1, -1.0)], Relation::GreaterEqual, 1.0)],
    )
}
#[test]
fn integer_optimum_outside_exact_f64_integer_range_is_not_falsely_certified() {
    let base = 9_007_199_254_740_992_i128;
    let input = large_integer_case(base as f64);
    match solve(&input, &SolveOptions::default(), &NoControl) {
        Ok(result) => {
            eprintln!("INTEGER_PRECISION {result:?}");
            if result.optimality == Optimality::Optimal {
                let actual = result.values.as_ref().and_then(|v| v.get(1)).copied().expect("integer point");
                assert_eq!(actual as i128, base + 1, "the representable input has an odd exact-integer optimum that f64 cannot encode");
            }
        }
        Err(error) => eprintln!("INTEGER_PRECISION_REJECTED {error}"),
    }
}
#[test]
fn odd_integer_optimum_inside_exact_f64_range_is_a_control() {
    let base = 4_503_599_627_370_496_i128;
    let result = run(&large_integer_case(base as f64));
    eprintln!("INTEGER_PRECISION_CONTROL {result:?}");
    assert_eq!(result.optimality, Optimality::Optimal);
    assert_eq!(result.values.as_ref().and_then(|v| v.get(1)).copied().expect("point") as i128, base + 1);
}
#[test]
fn lost_continuous_translation_residual_does_not_claim_zero_gap() {
    let mut input = large_integer_case(9_007_199_254_740_992.0);
    for variable in &mut input.variables {
        variable.kind = VariableKind::Continuous;
    }
    let result = run(&input);
    eprintln!("CONTINUOUS_PRECISION {result:?}");
    // Exact transformed optimum is y=2^53+1; rounding its +1 displacement
    // back into original coordinates loses a full variable unit (> tolerance).
    assert_ne!(result.optimality, Optimality::Optimal);
    assert_eq!(result.termination, Termination::NumericalFailure);
}
#[derive(Clone, Copy)]
struct Rational {
    i: i128,
    d: i128,
}
impl Rational {
    fn new(i: i128, d: i128) -> Self {
        if d < 0 { Self { i: -i, d: -d } } else { Self { i, d } }
    }
    fn less(self, other: Self) -> bool {
        self.i * other.d < other.i * self.d
    }
    fn value(self) -> f64 {
        self.i as f64 / self.d as f64
    }
}
fn exact_vertex_oracle(rows: &[[i128; 3]], cost: [i128; 2], direction: Direction) -> Option<Rational> {
    let mut best = None;
    for (i, a) in rows.iter().enumerate() {
        for b in rows.iter().skip(i + 1) {
            let det = a[0] * b[1] - b[0] * a[1];
            if det == 0 {
                continue;
            }
            let x = Rational::new(a[2] * b[1] - b[2] * a[1], det);
            let y = Rational::new(a[0] * b[2] - b[0] * a[2], det);
            // Both coordinates have the same positive denominator.
            if rows.iter().any(|r| r[0] * x.i + r[1] * y.i > r[2] * x.d) {
                continue;
            }
            let objective = Rational::new(cost[0] * x.i + cost[1] * y.i, x.d);
            best = Some(match best {
                None => objective,
                Some(old) => {
                    if match direction {
                        Direction::Minimize => objective.less(old),
                        Direction::Maximize => old.less(objective),
                    } {
                        objective
                    } else {
                        old
                    }
                }
            });
        }
    }
    best
}
#[test]
fn randomized_bounded_lps_match_an_exact_rational_vertex_oracle() {
    let mut state = 0x89abcdef_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 32) as u32
    };
    for case in 0..300 {
        let cost = [i128::from(next() % 11) - 5, i128::from(next() % 11) - 5];
        let direction = if next() % 2 == 0 { Direction::Minimize } else { Direction::Maximize };
        let mut rows = vec![[1, 0, 3], [-1, 0, 2], [0, 1, 3], [0, -1, 2]];
        for _ in 0..3 {
            rows.push([i128::from(next() % 7) - 3, i128::from(next() % 7) - 3, i128::from(next() % 15) - 7]);
        }
        let scale = match case % 3 {
            0 => 1e-6,
            1 => 1.0,
            _ => 1e6,
        };
        let input = problem(
            vec![var("bounded", VariableKind::Continuous, Some(-2.0), Some(3.0)); 2],
            direction,
            &cost.map(|v| v as f64),
            rows.iter()
                .skip(4)
                .map(|r| row(&[(0, r[0] as f64 * scale), (1, r[1] as f64 * scale)], Relation::LessEqual, r[2] as f64 * scale))
                .collect(),
        );
        let result = run(&input);
        match exact_vertex_oracle(&rows, cost, direction) {
            Some(expected) => {
                assert_eq!(result.optimality, Optimality::Optimal, "case {case} {input:?} {result:?}");
                let actual = result.objective.expect("objective");
                assert!((actual - expected.value()).abs() < 1e-6, "case {case}: {actual} != {} {input:?} {result:?}", expected.value());
            }
            None => assert_eq!(result.feasibility, Feasibility::Infeasible, "case {case}: {input:?} {result:?}"),
        }
    }
}

fn mixed_integer_oracle(rows: &[[i128; 4]], cost: [i128; 3], direction: Direction) -> Option<Rational> {
    let mut best: Option<Rational> = None;
    for x in -2_i128..=3 {
        for y in -2_i128..=3 {
            let mut low = Rational::new(-2, 1);
            let mut high = Rational::new(3, 1);
            let mut feasible = true;
            for row in rows {
                let remaining = row[3] - row[0] * x - row[1] * y;
                if row[2] > 0 {
                    let upper = Rational::new(remaining, row[2]);
                    if upper.less(high) {
                        high = upper;
                    }
                } else if row[2] < 0 {
                    let lower = Rational::new(remaining, row[2]);
                    if low.less(lower) {
                        low = lower;
                    }
                } else if remaining < 0 {
                    feasible = false;
                    break;
                }
            }
            if !feasible || high.less(low) {
                continue;
            }
            let use_low = match direction {
                Direction::Minimize => cost[2] >= 0,
                Direction::Maximize => cost[2] < 0,
            };
            let z = if use_low { low } else { high };
            let value = Rational::new((cost[0] * x + cost[1] * y - 7) * z.d + cost[2] * z.i, z.d);
            best = Some(match best {
                None => value,
                Some(old) => {
                    if match direction {
                        Direction::Minimize => value.less(old),
                        Direction::Maximize => old.less(value),
                    } {
                        value
                    } else {
                        old
                    }
                }
            });
        }
    }
    best
}
#[test]
fn mixed_integer_models_and_interrupted_bounds_match_an_exact_interval_oracle() {
    let mut state = 0x3141592653589793_u64;
    let mut next = || {
        state = state.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        (state >> 32) as u32
    };
    for case in 0..64 {
        let direction = if next() % 2 == 0 { Direction::Minimize } else { Direction::Maximize };
        let cost = [i128::from(next() % 11) - 5, i128::from(next() % 11) - 5, i128::from(next() % 11) - 5];
        let rows: Vec<[i128; 4]> = (0..3)
            .map(|_| [i128::from(next() % 7) - 3, i128::from(next() % 7) - 3, i128::from(next() % 7) - 3, i128::from(next() % 17) - 8])
            .collect();
        let mut input = problem(
            vec![
                var("x", VariableKind::Integer, Some(-2.0), Some(3.0)),
                var("y", VariableKind::Integer, Some(-2.0), Some(3.0)),
                var("z", VariableKind::Continuous, Some(-2.0), Some(3.0)),
            ],
            direction,
            &cost.map(|x| x as f64),
            rows.iter().map(|r| row(&[(0, r[0] as f64), (1, r[1] as f64), (2, r[2] as f64)], Relation::LessEqual, r[3] as f64)).collect(),
        );
        input.objective.offset = -7.0;
        let oracle = mixed_integer_oracle(&rows, cost, direction);
        for limit in [0, 1, 2, 3, 5, 10_000] {
            let options = SolveOptions { max_nodes: limit, ..SolveOptions::default() };
            let result = solve(&input, &options, &NoControl).expect("bounded review model");
            assert!(result.statistics.nodes <= limit);
            if let Some(expected) = oracle {
                let expected = expected.value();
                if result.optimality == Optimality::Optimal {
                    assert!(
                        result.objective.is_some_and(|v| (v - expected).abs() < 1e-6),
                        "case {case}, node_limit{limit}, expected{expected}, result{result:?}, model{input:?}"
                    );
                }
                assert_ne!(result.feasibility, Feasibility::Infeasible, "case {case} {result:?}");
                if let Some(bound) = result.best_bound {
                    let valid = match direction {
                        Direction::Minimize => bound <= expected + 1e-6,
                        Direction::Maximize => bound >= expected - 1e-6,
                    };
                    assert!(valid, "case {case}, node_limit{limit}: invalid bound{bound} vs optimum{expected}: {result:?}");
                }
            } else {
                assert_ne!(result.feasibility, Feasibility::Feasible, "case {case}: exact oracle has no feasible point: {result:?}");
                if limit == 10_000 {
                    assert_eq!(result.feasibility, Feasibility::Infeasible, "case {case} {result:?}");
                }
            }
            if let Some(values) = &result.values {
                assert_eq!(values.len(), 3);
                assert!(values.iter().all(|v| (-2.0 - 1e-7..=3.0 + 1e-7).contains(v)));
                assert!((values[0] - values[0].round()).abs() < 1e-7);
                assert!((values[1] - values[1].round()).abs() < 1e-7);
                for row in &rows {
                    let lhs = row[0] as f64 * values[0] + row[1] as f64 * values[1] + row[2] as f64 * values[2];
                    assert!(lhs <= row[3] as f64 + 1e-7);
                }
            }
            if let (Some(objective), Some(bound), Some(gap)) = (result.objective, result.best_bound, result.relative_gap) {
                let difference = match direction {
                    Direction::Minimize => objective - bound,
                    Direction::Maximize => bound - objective,
                };
                let expected_gap = difference.max(0.0) / objective.abs().max(bound.abs()).max(1.0);
                assert!((gap - expected_gap).abs() < 1e-12, "case {case}: {result:?}");
            }
        }
    }
}
#[test]
fn objective_unit_scaling_does_not_change_the_optimal_point() {
    for scale in [1e-200, 1e-20, 1e-12, 1e-9, 1.0, 1e9, 1e100] {
        let input = problem(
            vec![var("x", VariableKind::Continuous, Some(0.0), Some(4.0)), var("y", VariableKind::Continuous, Some(0.0), Some(6.0))],
            Direction::Maximize,
            &[3.0 * scale, 5.0 * scale],
            vec![row(&[(0, 3.0), (1, 2.0)], Relation::LessEqual, 18.0)],
        );
        let result = run(&input);
        assert_eq!(result.optimality, Optimality::Optimal, "scale{scale} {result:?}");
        let values = result.values.expect("scaled LP point");
        assert!((3.0 * values[0] + 5.0 * values[1] - 36.0).abs() < 1e-6, "scale{scale}, point{values:?}");
    }
}
#[test]
fn requested_mip_gap_keeps_best_found_and_direction_correct() {
    for direction in [Direction::Minimize, Direction::Maximize] {
        let sign = if direction == Direction::Minimize { -1.0 } else { 1.0 };
        let mut input = problem(
            vec![var("x", VariableKind::Integer, Some(0.0), Some(3.0))],
            direction,
            &[sign],
            vec![row(&[(0, 1.0)], Relation::LessEqual, 1.5)],
        );
        input.objective.offset = -7.0;
        let options = SolveOptions { relative_gap: 0.2, ..SolveOptions::default() };
        let result = solve(&input, &options, &NoControl).expect("gap solve");
        assert_eq!(result.feasibility, Feasibility::Feasible);
        assert_eq!(result.optimality, Optimality::BestFound);
        assert_eq!(result.objective, Some(-7.0 + sign));
        assert_eq!(result.best_bound, Some(-7.0 + sign * 1.5));
        assert!(result.relative_gap.is_some_and(|g| g <= 0.2));
        assert!(result.warnings.iter().any(|w| w.contains("requested relative MIP gap")));
    }
}
#[test]
fn a_large_objective_constant_does_not_change_the_integer_argmax() {
    for offset in [0.0, 1e16, -1e16] {
        let mut input = problem(
            vec![var("x", VariableKind::Integer, Some(0.0), Some(2.0)); 2],
            Direction::Maximize,
            &[3.0, 2.0],
            vec![row(&[(0, 2.0), (1, 1.0)], Relation::LessEqual, 3.0)],
        );
        input.objective.offset = offset;
        let result = run(&input);
        eprintln!("MIP_OFFSET offset={offset} {result:?}");
        assert_eq!(result.optimality, Optimality::Optimal);
        let point = result.values.expect("integer point");
        assert_eq!(3.0 * point[0] + 2.0 * point[1], 5.0, "objective constant must not change the optimum");
    }
}

#[test]
fn separable_qp_matches_independent_sum_of_squares_identities() {
    for direction in [Direction::Minimize, Direction::Maximize] {
        for clipped in [false, true] {
            let sign = if direction == Direction::Minimize { 1.0 } else { -1.0 };
            let mut input = problem(
                vec![
                    var("x", VariableKind::Continuous, Some(0.0), Some(if clipped { 0.25 } else { 1.0 })),
                    var("y", VariableKind::Continuous, Some(if clipped { -0.3 } else { -1.0 }), Some(0.0)),
                ],
                direction,
                &[-6.0 * sign, 20.0 * sign],
                Vec::new(),
            );
            input.objective.offset = 5.0 * sign;
            input.objective.quadratic =
                vec![QuadraticTerm { left: 0, right: 0, coefficient: 9.0 * sign }, QuadraticTerm { left: 1, right: 1, coefficient: 25.0 * sign }];
            let result = run(&input);
            assert_eq!(result.optimality, Optimality::Optimal);
            let point = result.values.expect("analytic quadratic point");
            // Independent nonnegative square decomposition, not an engine-derived result.
            let squared_loss = (3.0 * point[0] - 1.0).powi(2) + (5.0 * point[1] + 2.0).powi(2);
            let reference = if clipped { 0.3125 } else { 0.0 };
            assert!((squared_loss - reference).abs() < 1e-12);
            assert!(result.objective.is_some_and(|v| (v - sign * reference).abs() < 1e-12));
        }
    }
}
#[test]
fn a_linear_free_coordinate_gives_a_genuine_unbounded_separable_qp() {
    let mut input = problem(
        vec![var("quadratic", VariableKind::Continuous, None, None), var("linear", VariableKind::Continuous, None, Some(0.0))],
        Direction::Minimize,
        &[0.0, 1.0],
        Vec::new(),
    );
    input.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: 1.0 });
    let result = run(&input);
    assert_eq!(result.optimality, Optimality::Unbounded);
    assert_eq!(result.feasibility, Feasibility::Feasible);
    assert!(result.values.is_some());
    assert_eq!(result.best_bound, None);
    assert_eq!(result.relative_gap, None);
}
#[test]
fn quadratic_limits_and_unsupported_classes_do_not_report_an_optimum() {
    let mut input = problem(vec![var("x", VariableKind::Continuous, Some(0.0), Some(1.0)); 2], Direction::Minimize, &[-1.0, -1.0], Vec::new());
    input.objective.quadratic.push(QuadraticTerm { left: 0, right: 0, coefficient: 1.0 });
    let result = solve(&input, &SolveOptions { max_iterations: 1, ..SolveOptions::default() }, &NoControl).expect("quadratic limit");
    assert_eq!(result.termination, Termination::IterationLimit);
    assert_eq!(result.optimality, Optimality::Unknown);
    assert_eq!(result.statistics.iterations, 1);
    let result = solve(&input, &SolveOptions { max_evaluations: 0, ..SolveOptions::default() }, &NoControl).expect("quadratic evaluation limit");
    assert_eq!(result.termination, Termination::EvaluationLimit);
    assert_eq!(result.statistics.evaluations, 0);
    input.objective.quadratic.push(QuadraticTerm { left: 0, right: 1, coefficient: 0.25 });
    assert!(solve(&input, &SolveOptions::default(), &NoControl).is_err_and(|e| e.code == "convexity_not_proven"));
}

#[test]
fn node_limited_offset_pairs_keep_identical_incumbents_and_termination() {
    for direction in [Direction::Minimize, Direction::Maximize] {
        let sign = if direction == Direction::Minimize { -1.0 } else { 1.0 };
        let mut input = problem(
            vec![var("x", VariableKind::Integer, Some(0.0), Some(2.0)); 2],
            direction,
            &[3.0 * sign, 2.0 * sign],
            vec![row(&[(0, 2.0), (1, 1.0)], Relation::LessEqual, 3.0)],
        );
        for limit in [0, 1, 2, 3, 6] {
            let options = SolveOptions { max_nodes: limit, ..SolveOptions::default() };
            input.objective.offset = 0.0;
            let reference = solve(&input, &options, &NoControl).expect("reference limited solve");
            for offset in [1e16, -1e16] {
                input.objective.offset = offset;
                let result = solve(&input, &options, &NoControl).expect("offset limited solve");
                assert_eq!(result.values, reference.values);
                assert_eq!(result.termination, reference.termination);
                assert_eq!(result.optimality, reference.optimality);
                assert_eq!(result.statistics.nodes, reference.statistics.nodes);
                if reference.relative_gap.is_some_and(|gap| gap > 0.0) {
                    assert!(result.relative_gap.is_some_and(|gap| gap > 0.0), "rounded objective offset must not fabricate zero gap: {result:?}");
                }
                if result.optimality == Optimality::Optimal {
                    assert!(result.warnings.iter().any(|w| w.contains("rounded outwards")));
                    assert!(result.relative_gap.is_some_and(|gap| gap > 0.0));
                }
            }
        }
    }
}
#[test]
fn significant_unrepresentable_working_cost_is_a_numerical_failure() {
    let input = problem(
        vec![
            var("large fixed contribution", VariableKind::Continuous, Some(1e16), Some(1e16)),
            var("x", VariableKind::Integer, Some(0.0), Some(2.0)),
            var("y", VariableKind::Integer, Some(0.0), Some(2.0)),
        ],
        Direction::Maximize,
        &[1.0, 3.0, 2.0],
        vec![row(&[(1, 2.0), (2, 1.0)], Relation::LessEqual, 3.0)],
    );
    let result = run(&input);
    assert_eq!(result.termination, Termination::NumericalFailure);
    assert_ne!(result.optimality, Optimality::Optimal);
    assert!(result.warnings.iter().any(|w| w.contains("Constant-free objective loses")));
}
