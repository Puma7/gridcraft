use solvercraft::{
    compile::{Expr, ExpressionModel},
    types::*,
};
#[test]
fn fixed_subnormal_box_incumbent_stays_inside_its_certified_domain() {
    for fixed in [f64::from_bits(1), f64::MIN_POSITIVE, 1.0] {
        let model = ExpressionModel {
            name: "fixed-box midpoint".into(),
            variables: vec![Variable { name: "x".into(), lower: Some(fixed), upper: Some(fixed), kind: VariableKind::Continuous }],
            direction: Direction::Minimize,
            objective: Expr::Multiply { left: Box::new(Expr::Constant { value: 1e308 }), right: Box::new(Expr::Variable { index: 0 }) },
            constraints: vec![],
        };
        let options = SolveOptions { optimality_tolerance: 1e-20, ..Default::default() };
        let report = solvercraft::global::solve_box_polynomial(&model, &options, &NoControl).expect("fixed solve");
        let values = report.solution.values.as_ref().expect("incumbent");
        assert_eq!(values[0], fixed, "fixed bound must remain exact, report={report:?}");
        assert!(solvercraft::global::verify_box_certificate(&model, &report, &options).expect("verify"));
    }
}
