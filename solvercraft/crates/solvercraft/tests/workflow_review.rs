use solvercraft::{
    compile::{Expr, ExpressionModel},
    nonlinear::{DerivativeMode, NlpOptions},
    types::*,
};
#[test]
fn workflow_honors_requested_finite_differences_for_bound_only_nonlinear_models() {
    let model = ExpressionModel {
        name: "configured derivatives".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective: Expr::Add {
            left: Box::new(Expr::Exp { value: Box::new(Expr::Variable { index: 0 }) }),
            right: Box::new(Expr::Power { base: Box::new(Expr::Variable { index: 0 }), exponent: 2 }),
        },
        constraints: vec![],
    };
    let nlp = NlpOptions { derivative_mode: DerivativeMode::FiniteDifference, finite_difference_step: 1e-5, ..Default::default() };
    for method in [SolveMethod::ProjectedGradient, SolveMethod::Auto] {
        let result =
            solvercraft::workflow::solve_expression(&model, &SolveOptions { method, ..Default::default() }, &nlp, &NoControl).expect("workflow");
        assert!(result.warnings.iter().any(|w| w.contains("finite-difference")), "Derivative selection silently replaced: {result:?}");
    }
}
#[test]
fn execution_manifest_preserves_goal_derivative_configuration_and_lossless_context() {
    use solvercraft::{
        project::{ModelDefinition, PortableModel},
        workflow::*,
    };
    let model = ExpressionModel {
        name: "manifest expression".into(),
        variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
        direction: Direction::Minimize,
        objective: Expr::Exp { value: Box::new(Expr::Variable { index: 0 }) },
        constraints: vec![],
    };
    let options = SolveOptions { method: SolveMethod::ProjectedGradient, seed: u64::MAX, ..Default::default() };
    let request = ExecutionRequest {
        schema_version: 2,
        model: PortableModel {
            schema_version: 2,
            id: "9007199254740993".into(),
            revision: "18446744073709551615".into(),
            name: "manifest example".into(),
            definition: ModelDefinition::Expression(model),
            options,
            bindings: vec![],
            metadata: Default::default(),
        },
        context: ExecutionContext {
            run_id: "review-run".into(),
            snapshot_id: "review-snapshot".into(),
            snapshot_revision: Some("9007199254740993".into()),
            host: "independent-review".into(),
            source_revision: Some("synthetic-fixture".into()),
        },
        goal: Some(Goal::Maximize {}),
        limits: Default::default(),
        nonlinear: NlpOptions { derivative_mode: DerivativeMode::FiniteDifference, finite_difference_step: 1e-5, ..Default::default() },
    };
    let run = execute(&request, &NoControl).expect("execution");
    let json = serde_json::to_value(&run.manifest).expect("manifest serialization");
    assert_eq!(json["model_id"], "9007199254740993");
    assert_eq!(json["model_revision"], "18446744073709551615");
    assert_eq!(json["context"]["snapshot_revision"], "9007199254740993");
    assert_eq!(json["options"]["seed"], u64::MAX.to_string());
    assert_eq!(json["goal"]["kind"], "maximize");
    assert_eq!(json["nonlinear"]["derivative_mode"], "finite_difference");
    assert_eq!(json["nonlinear"]["finite_difference_step"], 1e-5);
    assert_eq!(json["engine"], run.result.engine);
    assert!((run.original_objective.expect("original objective") - 2.0_f64.exp()).abs() < 1e-7);
    assert!(!run.manifest.input_fingerprint.is_empty());
}
#[test]
fn binary64_json_roundtrip_preserves_values_near_a_density_support_boundary() {
    let value = 0.999_999_999_999_998_3_f64;
    let encoded = serde_json::to_string(&value).expect("encode binary64");
    let decoded: f64 = serde_json::from_str(&encoded).expect("decode binary64");
    assert_eq!(decoded.to_bits(), value.to_bits(), "JSON moved a supported float by one ULP: {encoded} ->{decoded}");
}
