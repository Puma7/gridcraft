use solvercraft::{
    compile::{Expr, ExpressionConstraint, ExpressionModel},
    nonlinear::NlpOptions,
    types::*,
};
fn variable(lo: f64, hi: f64) -> Variable {
    Variable { name: "x".into(), lower: Some(lo), upper: Some(hi), kind: VariableKind::Continuous }
}
fn quadratic(center: f64, scale: f64, offset: f64, direction: Direction, lo: f64, hi: f64) -> ExpressionModel {
    let square =
        Expr::Power { base: Box::new(Expr::Subtract { left: Box::new(Expr::variable(0)), right: Box::new(Expr::constant(center)) }), exponent: 2 };
    let signed_scale = if direction == Direction::Minimize { scale } else { -scale };
    ExpressionModel {
        name: "closed-form signed quadratic".into(),
        variables: vec![variable(lo, hi)],
        direction,
        objective: Expr::Add {
            left: Box::new(Expr::constant(offset)),
            right: Box::new(Expr::Multiply { left: Box::new(Expr::constant(signed_scale)), right: Box::new(square) }),
        },
        constraints: vec![],
    }
}
fn check_local(result: &SolveResult, expected_x: f64, expected_value: f64) {
    assert_eq!(result.feasibility, Feasibility::Feasible, "{result:?}");
    assert!(result.values.as_ref().is_some_and(|values| (values[0] - expected_x).abs() < 2e-5), "{result:?}, x*={expected_x}");
    assert!(result.objective.is_some_and(|value| (value - expected_value).abs() < 1e-7), "{result:?}, f*={expected_value}");
    assert_ne!(result.optimality, Optimality::Optimal);
    assert!(result.best_bound.is_none() && result.relative_gap.is_none());
}

#[test]
fn local_multistart_and_global_quadratic_classification_matches_closed_forms_and_replayed_proofs() {
    let options = SolveOptions::default();
    let mut local_count = 0;
    let mut restart_count = 0;
    let mut global_count = 0;
    for center in [-2.0_f64, -1.0, 0.0, 1.0, 2.0] {
        for (lo, hi) in [(-3.0, 3.0), (-1.0, 1.0), (0.0, 2.0)] {
            for scale in [0.25, 1.0, 4.0] {
                for direction in [Direction::Minimize, Direction::Maximize] {
                    for offset in [0.0, 1024.0] {
                        let model = quadratic(center, scale, offset, direction, lo, hi);
                        let expected_x = center.clamp(lo, hi);
                        let signed_scale = if direction == Direction::Minimize { scale } else { -scale };
                        let expected_value = offset + signed_scale * (expected_x - center).powi(2);
                        for start in [lo, (lo + hi) * 0.5, hi] {
                            for mode in [solvercraft::nonlinear::DerivativeMode::Automatic, solvercraft::nonlinear::DerivativeMode::FiniteDifference]
                            {
                                let result =
                                    solvercraft::nonlinear::solve_from_with_derivatives(&model, &[start], &options, &NoControl, mode, 1e-6).unwrap();
                                check_local(&result, expected_x, expected_value);
                                local_count += 1;
                            }
                        }
                        for seed in [0, 42] {
                            let result = solvercraft::multistart::solve(
                                &model,
                                &options,
                                &NlpOptions::default(),
                                &solvercraft::multistart::MultiStartOptions { starts: 4, seed },
                                &NoControl,
                            )
                            .unwrap();
                            check_local(&result.solution, expected_x, expected_value);
                            restart_count += 1;
                        }
                        let global = solvercraft::global::solve_box_polynomial(&model, &options, &NoControl).unwrap();
                        assert_eq!(global.solution.optimality, Optimality::Optimal, "{global:?}");
                        assert!((global.solution.objective.unwrap() - expected_value).abs() < 1e-7);
                        assert!(solvercraft::global::verify_box_certificate(&model, &global, &options).unwrap());
                        let mut forged = global.clone();
                        forged.certificate.signed_lower_bound = Some(1e6);
                        assert!(!solvercraft::global::verify_box_certificate(&model, &forged, &options).unwrap_or(false));
                        let mut forged_point = global.clone();
                        forged_point.solution.objective = Some(expected_value + 1.0);
                        assert!(!solvercraft::global::verify_box_certificate(&model, &forged_point, &options).unwrap_or(false));
                        global_count += 1;
                    }
                }
            }
        }
    }
    assert_eq!((local_count, restart_count, global_count), (1080, 360, 180));
    eprintln!("nonlinear: {local_count} local + {restart_count} multistart closed forms; {global_count} global certificates +360 rejected forgeries");
}

#[test]
fn constrained_quadratic_matrix_has_original_unit_kkt_endpoints_without_global_claims() {
    let mut checked = 0;
    for center in [-2.0_f64, -1.0, 0.0, 1.0, 2.0] {
        for limit in [-2.0_f64, 0.0, 2.0] {
            for scale in [0.25, 1.0, 4.0] {
                for direction in [Direction::Minimize, Direction::Maximize] {
                    for relation in [Relation::LessEqual, Relation::GreaterEqual] {
                        let mut model = quadratic(center, scale, 0.0, direction, -3.0, 3.0);
                        model.constraints.push(ExpressionConstraint {
                            name: "active limit".into(),
                            left: Expr::variable(0),
                            right: Expr::constant(limit),
                            relation,
                        });
                        let expected_x = if relation == Relation::LessEqual { center.min(limit) } else { center.max(limit) };
                        let expected_value = if direction == Direction::Minimize { scale } else { -scale } * (expected_x - center).powi(2);
                        let result = solvercraft::constrained::solve(&model, &SolveOptions::default(), &NlpOptions::default(), &NoControl).unwrap();
                        check_local(&result, expected_x, expected_value);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 180);
    eprintln!("constrained nonlinear: {checked} independent KKT endpoints, no global bounds");
}
