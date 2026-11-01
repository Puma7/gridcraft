#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use std::sync::Arc;
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({})).unwrap();
    s
}
fn set(s: &mut Session, cell: &str, input: &str) {
    s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 2e-8 * (1.0 + expected.abs()), "{actual} != {expected}");
}
fn formula(s: &Session, cell: &str) -> String {
    s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse(cell).unwrap()).unwrap().formula.as_ref().unwrap().text.clone()
}
fn check_case(source: &str, kind: &str, parameters: &[(&str, f64)], nominal: f64, x: f64, probability: f64) {
    let mut s = session();
    set(&mut s, "A1", source);
    set(&mut s, "B1", "=SC.OUTPUT(A1)");
    set(&mut s, "C1", "=SC.OUTPUT(A1-A1)");
    let original = formula(&s, "A1");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let proposed = s.execute("solvercraft.worksheet.importPsi", json!({})).unwrap();
    assert_eq!(proposed["changes"].as_array().unwrap().len(), 1, "{source}: {proposed}");
    assert_eq!(proposed["outside_policy"], "solvercraft_nominal");
    assert_eq!(proposed["vendor_display_context_preserved"], false);
    assert_eq!(proposed["changes"][0]["vendor_display_context_preserved"], false);
    assert!(proposed["notes"].as_array().unwrap().iter().any(|note| note.as_str().unwrap_or("").contains("No vendor pre-run")));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    assert_eq!(formula(&s, "A1"), original);
    s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    let Value::Number(value) = s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()) else { panic!("{source}") };
    near(value, nominal);
    let discovered = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovered["inputs"].as_array().unwrap().len(), 1);
    assert_eq!(discovered["outputs"].as_array().unwrap().len(), 2);
    let descriptor = &discovered["inputs"][0]["distribution"];
    assert_eq!(descriptor["kind"], kind, "{source}");
    for (name, value) in parameters {
        near(descriptor[name].as_f64().unwrap(), *value);
    }
    let distribution: Distribution = serde_json::from_value(descriptor.clone()).unwrap();
    near(::solvercraft::math::cdf(&distribution, x).unwrap(), probability);
    let before = Arc::clone(&s.doc().unwrap().wb);
    let preview = s.execute("solvercraft.simulate", json!({"options":{"trials":32,"seed":"18446744073709551615","retain_samples":true}})).unwrap();
    assert_eq!(preview["result"]["completed_trials"], 32);
    let rows = preview["result"]["samples"].as_array().unwrap();
    assert_eq!(rows.len(), 32);
    for (trial, row) in rows.iter().enumerate() {
        assert_eq!(row.as_array().unwrap().len(), 2);
        assert_eq!(row[1], 0.0);
        let expected = ::solvercraft::math::sample(&distribution, u64::MAX, 0, trial as u64).unwrap();
        assert_eq!(row[0].as_f64().unwrap().to_bits(), expected.to_bits(), "{source} trial{trial}");
    }
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    assert!(s.execute("solvercraft.result.apply", preview).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(formula(&s, "A1"), original);
}
macro_rules! case {
    ($name:ident,$source:expr,$kind:expr,$parameters:expr,$nominal:expr,$x:expr,$cdf:expr) => {
        #[test]
        fn $name() {
            check_case($source, $kind, $parameters, $nominal, $x, $cdf);
        }
    };
}
case!(psi_uniform_import_executes_real_uniform, "=PsiUniform(-2,6)", "uniform", &[("low", -2.0), ("high", 6.0)], 2.0, 0.0, 0.25);
case!(
    psi_intuniform_preserves_inclusive_discrete_endpoints,
    "=PsiIntUniform(-2,2)",
    "discrete_uniform",
    &[("low", -2.0), ("high", 2.0)],
    0.0,
    0.0,
    0.6
);
case!(psi_normal_sigma_changes_the_distribution, "=PsiNormal(2,3)", "normal", &[("mean", 2.0), ("std_dev", 3.0)], 2.0, 5.0, 0.8413447460685429);
case!(
    psi_lognormal_uses_arithmetic_mean_and_stddev,
    "=PsiLogNormal(4,2)",
    "lognormal",
    &[("log_mean", 4.0_f64.ln() - 0.5 * 1.25_f64.ln()), ("log_std_dev", 1.25_f64.ln().sqrt())],
    4.0,
    4.0 / 1.25_f64.sqrt(),
    0.5
);
case!(
    psi_lognorm2_sigma_is_not_variance,
    "=PsiLogNorm2(1,2)",
    "lognormal",
    &[("log_mean", 1.0), ("log_std_dev", 2.0)],
    3.0_f64.exp(),
    3.0_f64.exp(),
    0.8413447460685429
);
case!(
    psi_triangular_preserves_min_mode_max,
    "=PsiTriangular(0,2,8)",
    "triangular",
    &[("low", 0.0), ("mode", 2.0), ("high", 8.0)],
    10.0 / 3.0,
    2.0,
    0.25
);
case!(
    psi_pert_uses_the_fixed_four_weight,
    "=PsiPert(0,2,4)",
    "beta_pert",
    &[("low", 0.0), ("mode", 2.0), ("high", 4.0), ("weight", 4.0)],
    2.0,
    1.0,
    0.103515625
);
case!(psi_beta_retains_two_shape_order, "=PsiBeta(2,3)", "beta", &[("alpha", 2.0), ("beta", 3.0)], 0.4, 0.5, 0.6875);
case!(
    psi_gamma_shape_scale_order_has_an_independent_cdf,
    "=PsiGamma(2,3)",
    "gamma",
    &[("shape", 2.0), ("scale", 3.0)],
    6.0,
    3.0,
    1.0 - 2.0 / std::f64::consts::E
);
case!(psi_exponential_mean_is_converted_to_rate, "=PsiExponential(2)", "exponential", &[("rate", 0.5)], 2.0, 2.0, 1.0 - 1.0 / std::f64::consts::E);
case!(
    psi_weibull_uses_nondegenerate_shape_scale,
    "=PsiWeibull(2,3)",
    "weibull",
    &[("shape", 2.0), ("scale", 3.0)],
    3.0 * std::f64::consts::PI.sqrt() / 2.0,
    3.0,
    1.0 - 1.0 / std::f64::consts::E
);
case!(psi_bernoulli_keeps_zero_mass, "=PsiBernoulli(0.25)", "bernoulli", &[("probability", 0.25)], 0.25, 0.0, 0.75);
case!(
    psi_binomial_keeps_count_and_success_probability,
    "=PsiBinomial(4,0.25)",
    "binomial",
    &[("trials", 4.0), ("probability", 0.25)],
    1.0,
    0.0,
    0.31640625
);
case!(psi_poisson_has_the_independent_zero_mass, "=PsiPoisson(2)", "poisson", &[("rate", 2.0)], 2.0, 0.0, (-2.0_f64).exp());
case!(
    psi_negbinomial_probability_is_not_complemented,
    "=PsiNegBinomial(2,0.25)",
    "negative_binomial",
    &[("successes", 2.0), ("probability", 0.25)],
    6.0,
    0.0,
    0.0625
);
case!(psi_geometric_counts_failures_from_zero, "=PsiGeometric(0.25)", "geometric", &[("probability", 0.25)], 3.0, 0.0, 0.25);
case!(psi_disuniform_uses_equal_mass_list, "=PsiDisUniform({1,2,6})", "empirical", &[], 3.0, 1.0, 1.0 / 3.0);
case!(psi_discrete_normalizes_nonunit_weights, "=PsiDiscrete({1,10},{3,1})", "categorical", &[], 3.25, 1.0, 0.75);
case!(psi_cumul_uses_piecewise_cdf_knots, "=PsiCumul(0,10,{2,6},{0.2,0.8})", "mixture", &[], 4.2, 4.0, 0.5);

#[test]
fn unsupported_nested_properties_arity_and_blank_keep_whole_formula_and_history() {
    for source in [
        "=1+2*PsiUniform(0,1)",
        "=SUM(PsiNormal(0,1),2)",
        "=PsiNormal(PsiUniform(0,1),1)",
        "=PsiNormal(0,1,PsiShift(2))",
        "=PsiNormal(0,1,PsiName(\"x\"))",
        "=PsiUniform(0,2)+PsiUnknown(1)",
        "=PsiPert(0,1,4,4)",
        "=PsiNormal(0)",
        "=PsiUniform(,2)",
    ] {
        let mut s = session();
        set(&mut s, "A1", source);
        let original = formula(&s, "A1");
        let before = Arc::clone(&s.doc().unwrap().wb);
        let revision = s.doc().unwrap().revision;
        let result = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(result["changes"], json!([]), "{source}: {result}");
        assert_eq!(result["applied"], false);
        assert_eq!(result["apply_requested"], true);
        assert_eq!(result["unsupported"].as_array().unwrap().len(), 1, "{source}");
        assert_eq!(result["unsupported"][0]["preserved"], true);
        assert_eq!(formula(&s, "A1"), original);
        assert_eq!(s.doc().unwrap().revision, revision);
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
}
#[test]
fn outside_policy_reports_loss_and_strict_vendor_preservation_is_refused() {
    let mut s = session();
    set(&mut s, "A1", "=PsiNormal(2,3)");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let strict = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true,"outside_policy":"preserve_vendor"})).unwrap();
    assert_eq!(strict["changes"], json!([]));
    assert_eq!(strict["applied"], false);
    assert_eq!(strict["unsupported"][0]["reason"], "vendor_display_context_unavailable");
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    assert!(s.execute("solvercraft.worksheet.importPsi", json!({"outside_policy":"invented"})).is_err());
    let own = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true,"outside_policy":"solvercraft_nominal"})).unwrap();
    assert_eq!(own["outside_policy"], "solvercraft_nominal");
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Number(2.0));
}
#[test]
fn invalid_imported_parameters_and_parameter_errors_never_become_simulation_results() {
    for source in [
        "=PsiUniform(2,1)",
        "=PsiIntUniform(0.5,2)",
        "=PsiNormal(0,-1)",
        "=PsiLogNormal(0,1)",
        "=PsiLogNorm2(0,0)",
        "=PsiTriangular(0,3,2)",
        "=PsiPert(0,4,4)",
        "=PsiBeta(0,2)",
        "=PsiGamma(2,0)",
        "=PsiExponential(0)",
        "=PsiWeibull(0,3)",
        "=PsiBernoulli(1.1)",
        "=PsiBinomial(1.5,0.5)",
        "=PsiPoisson(-1)",
        "=PsiNegBinomial(0,0.5)",
        "=PsiGeometric(0)",
        "=PsiDisUniform({1,\"x\"})",
        "=PsiDiscrete({1,2},{0,0})",
        "=PsiCumul(0,10,{6,2},{0.2,0.8})",
    ] {
        let mut s = session();
        set(&mut s, "A1", source);
        set(&mut s, "B1", "=SC.OUTPUT(A1)");
        let imported = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(imported["changes"].as_array().unwrap().len(), 1, "{source}");
        assert!(matches!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Error(_)), "{source}");
        let before = Arc::clone(&s.doc().unwrap().wb);
        assert!(s.execute("solvercraft.simulate", json!({"options":{"trials":2}})).is_err(), "{source}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
    let mut s = session();
    set(&mut s, "A1", "=PsiNormal(1/0,2)");
    s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Error(gridcraft_core::CellError::Div0));
}
#[test]
fn mixed_supported_and_unsupported_cells_are_atomic_per_formula_and_one_undo() {
    let mut s = session();
    set(&mut s, "A1", "=PsiUniform(0,2)");
    set(&mut s, "A2", "=PsiUniform(0,2)+PsiUnknown(1)");
    set(&mut s, "A3", "=PsiNormal(0,1,PsiStatic(4))");
    let one = formula(&s, "A1");
    let two = formula(&s, "A2");
    let three = formula(&s, "A3");
    let response = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    assert_eq!(response["changes"].as_array().unwrap().len(), 1);
    assert_eq!(response["unsupported"].as_array().unwrap().len(), 2);
    assert_eq!(formula(&s, "A2"), two);
    assert_eq!(formula(&s, "A3"), three);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(formula(&s, "A1"), one);
    assert_eq!(formula(&s, "A2"), two);
    assert_eq!(formula(&s, "A3"), three);
    assert!(s.execute("solvercraft.worksheet.importPsi", json!({"outside_policy":42})).is_err());
}
#[test]
fn parenthesized_declaration_is_discoverable_and_unsupported_properties_are_never_nominal_overrides() {
    let mut s = session();
    set(&mut s, "A1", "=(PsiUniform(0,2))");
    set(&mut s, "B1", "=SC.OUTPUT(A1)");
    assert_eq!(s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap()["changes"].as_array().unwrap().len(), 1);
    assert_eq!(s.execute("solvercraft.worksheet.discover", json!({})).unwrap()["inputs"].as_array().unwrap().len(), 1);
    assert_eq!(s.execute("solvercraft.simulate", json!({"options":{"trials":2}})).unwrap()["result"]["completed_trials"], 2);
    for source in ["=PsiNormal(0,1,42)", "=PsiTriangular(0,1,4,PsiStatic(2),PsiShift(1))", "=PsiTriangular(0,1,4,PsiShift(1),PsiStatic(2))"] {
        set(&mut s, "A1", source);
        let before = Arc::clone(&s.doc().unwrap().wb);
        let response = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(response["changes"], json!([]));
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
}

#[test]
fn explicit_invalid_selection_and_apply_types_cannot_broaden_an_import() {
    let mut s = session();
    set(&mut s, "A1", "=PsiUniform(0,2)");
    set(&mut s, "A2", "=PsiNormal(3,2)");
    // Keep a real redo entry as well, so a rejected command cannot silently discard it.
    set(&mut s, "B1", "42");
    s.execute("edit.undo", json!({})).unwrap();
    let one = formula(&s, "A1");
    let two = formula(&s, "A2");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let revision = s.doc().unwrap().revision;
    let history = (s.doc().unwrap().undo.len(), s.doc().unwrap().redo.len());
    assert!(history.1 > 0);
    for params in [
        json!({"range":3,"apply":true}),
        json!({"range":null,"apply":true}),
        json!({"range":["A1"],"apply":true}),
        json!({"range":false,"apply":true}),
        json!({"range":"A1","apply":"true"}),
        json!({"range":"A1","apply":1}),
        json!({"range":"A1","apply":null}),
        json!({"range":"A1","apply":{}}),
    ] {
        assert!(s.execute("solvercraft.worksheet.importPsi", params.clone()).is_err(), "{params}");
        assert_eq!(formula(&s, "A1"), one);
        assert_eq!(formula(&s, "A2"), two);
        assert_eq!(s.doc().unwrap().revision, revision);
        assert_eq!((s.doc().unwrap().undo.len(), s.doc().unwrap().redo.len()), history);
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
    let scoped = s.execute("solvercraft.worksheet.importPsi", json!({"range":"A1","apply":true})).unwrap();
    assert_eq!(scoped["changes"].as_array().unwrap().len(), 1);
    assert_eq!(formula(&s, "A2"), two);
    assert_ne!(formula(&s, "A1"), one);
}
