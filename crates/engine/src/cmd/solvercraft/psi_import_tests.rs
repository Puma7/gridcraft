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
    check_case_with_basis(source, kind, parameters, nominal, x, probability, "analytic_mean");
}
fn check_case_with_basis(source: &str, kind: &str, parameters: &[(&str, f64)], nominal: f64, x: f64, probability: f64, basis: &str) {
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
    near(discovered["inputs"][0]["nominal"].as_f64().unwrap(), nominal);
    assert_eq!(discovered["inputs"][0]["nominal_basis"], basis);
    let descriptor = &discovered["inputs"][0]["distribution"];
    assert_eq!(descriptor["kind"], kind, "{source}");
    for (name, value) in parameters {
        let field = name.split('.').fold(descriptor, |current, key| &current[key]);
        near(field.as_f64().unwrap(), *value);
    }
    let distribution: Distribution = serde_json::from_value(descriptor.clone()).unwrap();
    near(::solvercraft::math::cdf(&distribution, x).unwrap(), probability);
    let before = Arc::clone(&s.doc().unwrap().wb);
    let preview = s.execute("solvercraft.simulate", json!({"options":{"trials":32,"seed":"18446744073709551615","retain_samples":true}})).unwrap();
    assert_eq!(preview["result"]["completed_trials"], 32);
    near(preview["request"]["inputs"][0]["worksheet_nominal"]["value"].as_f64().unwrap(), nominal);
    assert_eq!(preview["request"]["inputs"][0]["worksheet_nominal"]["basis"], basis);
    assert_eq!(preview["manifest"]["bindings"]["inputs"][0]["worksheet_nominal"], preview["request"]["inputs"][0]["worksheet_nominal"]);
    let rows = preview["result"]["samples"].as_array().unwrap();
    assert_eq!(rows.len(), 32);
    for (trial, row) in rows.iter().enumerate() {
        assert_eq!(row.as_array().unwrap().len(), 2);
        assert_eq!(row[1], 0.0);
        let expected = ::solvercraft::math::sample(&distribution, u64::MAX, 0, trial as u64).unwrap();
        assert_eq!(row[0].as_f64().unwrap().to_bits(), expected.to_bits(), "{source} trial{trial}");
    }
    // Discovery is an actual public input DTO: retain its metadata unchanged.
    let roundtrip = s.execute("solvercraft.simulate", json!({"inputs":discovered["inputs"],"outputs":discovered["outputs"],"options":{"trials":2,"seed":"18446744073709551615","retain_samples":true}})).unwrap();
    assert_eq!(roundtrip["result"]["samples"].as_array().unwrap(), &rows[..2]);
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

// The second scalar rollout is kept separate from the original nineteen cases.
// Its CDF identities are analytic oracles, including the heavy-tail families
// whose SC outside value is a labelled median, not a population mean.
fn scalar_failure_modes(source: &str, invalid: &str) {
    let body = source.strip_prefix('=').unwrap();
    let (name, arguments) = body.split_once('(').unwrap();
    let arguments = arguments.strip_suffix(')').unwrap().split(',').collect::<Vec<_>>();
    let mut blank = arguments.clone();
    blank[0] = "";
    for unsupported in [
        format!("={name}({})", arguments[..arguments.len() - 1].join(",")),
        format!("={name}({},42)", arguments.join(",")),
        format!("={name}({})", blank.join(",")),
        format!("={name}({},PsiStatic(4))", arguments.join(",")),
        format!("=1+{body}"),
        format!("={name}Alt({})", arguments.join(",")),
    ] {
        let mut s = session();
        set(&mut s, "A1", &unsupported);
        let before = Arc::clone(&s.doc().unwrap().wb);
        let revision = s.doc().unwrap().revision;
        let history = (s.doc().unwrap().undo.len(), s.doc().unwrap().redo.len());
        let response = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(response["changes"], json!([]), "{unsupported}");
        assert_eq!(response["applied"], false);
        assert_eq!(response["unsupported"].as_array().unwrap().len(), 1);
        assert_eq!(response["unsupported"][0]["preserved"], true);
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
        assert_eq!(s.doc().unwrap().revision, revision);
        assert_eq!((s.doc().unwrap().undo.len(), s.doc().unwrap().redo.len()), history);
    }
    let mut error_arguments = arguments.clone();
    error_arguments[0] = "1/0";
    let mut bad_sources = vec![invalid.to_string(), format!("={name}({})", error_arguments.join(","))];
    if name == "PsiFDist" {
        bad_sources.extend(["=PsiFDist(1.5,4)".to_string(), "=PsiFDist(2,1.5)".to_string()]);
    }
    for bad_source in bad_sources {
        let mut s = session();
        set(&mut s, "A1", &bad_source);
        set(&mut s, "B1", "=SC.OUTPUT(A1)");
        let imported = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(imported["changes"].as_array().unwrap().len(), 1, "{bad_source}");
        assert!(matches!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Error(_)), "{bad_source}");
        if bad_source.contains("1/0") {
            assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Error(gridcraft_core::CellError::Div0));
        }
        let before = Arc::clone(&s.doc().unwrap().wb);
        assert!(s.execute("solvercraft.worksheet.discover", json!({})).is_err(), "{bad_source}");
        assert!(s.execute("solvercraft.simulate", json!({"options":{"trials":2}})).is_err(), "{bad_source}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
    let mut s = session();
    set(&mut s, "A1", source);
    let before = Arc::clone(&s.doc().unwrap().wb);
    let strict = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true,"outside_policy":"preserve_vendor"})).unwrap();
    assert_eq!(strict["changes"], json!([]));
    assert_eq!(strict["unsupported"][0]["reason"], "vendor_display_context_unavailable");
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
macro_rules! scalar_case {
    ($name:ident,$source:expr,$kind:expr,$parameters:expr,$nominal:expr,$x:expr,$cdf:expr,$basis:expr,$invalid:expr) => {
        #[test]
        fn $name() {
            check_case_with_basis($source, $kind, $parameters, $nominal, $x, $cdf, $basis);
            scalar_failure_modes($source, $invalid);
        }
    };
}
scalar_case!(
    psi_levy_uses_location_scale_and_a_labelled_median,
    "=PsiLevy(3,2)",
    "levy",
    &[("location", 3.0), ("scale", 2.0)],
    3.0 + 2.0 / 0.674_489_750_196_081_7_f64.powi(2),
    5.0,
    0.317_310_507_862_914_1,
    "median_fallback",
    "=PsiLevy(3,0)"
);
scalar_case!(
    psi_chisquare_requires_integral_df_and_preserves_gamma_scale,
    "=PsiChiSquare(2)",
    "chi_squared",
    &[("degrees_of_freedom", 2.0)],
    2.0,
    2.0,
    1.0 - 1.0 / std::f64::consts::E,
    "analytic_mean",
    "=PsiChiSquare(1.5)"
);
scalar_case!(
    psi_erlang_requires_integer_shape_and_keeps_scale,
    "=PsiErlang(2,3)",
    "erlang",
    &[("shape", 2.0), ("scale", 3.0)],
    6.0,
    3.0,
    1.0 - 2.0 / std::f64::consts::E,
    "analytic_mean",
    "=PsiErlang(1.5,3)"
);
scalar_case!(
    psi_fdist_preserves_numerator_and_denominator_df,
    "=PsiFDist(2,4)",
    "f",
    &[("numerator_df", 2.0), ("denominator_df", 4.0)],
    2.0,
    2.0,
    0.75,
    "analytic_mean",
    "=PsiFDist(0,4)"
);
scalar_case!(
    psi_kumaraswamy_preserves_both_shapes_and_nonunit_bounds,
    "=PsiKumaraswamy(2,3,2,8)",
    "shift_scale",
    &[("base.alpha", 2.0), ("base.beta", 3.0), ("shift", 2.0), ("scale", 6.0)],
    166.0 / 35.0,
    5.0,
    37.0 / 64.0,
    "analytic_mean",
    "=PsiKumaraswamy(2,3,8,2)"
);
scalar_case!(
    psi_logistic_uses_location_and_positive_scale,
    "=PsiLogistic(3,2)",
    "logistic",
    &[("location", 3.0), ("scale", 2.0)],
    3.0,
    5.0,
    std::f64::consts::E / (1.0 + std::f64::consts::E),
    "analytic_mean",
    "=PsiLogistic(3,0)"
);
scalar_case!(
    psi_maxextreme_keeps_maximum_orientation,
    "=PsiMaxExtreme(3,2)",
    "gumbel",
    &[("location", 3.0), ("scale", 2.0)],
    3.0 + 2.0 * 0.577_215_664_901_532_9,
    5.0,
    (-1.0 / std::f64::consts::E).exp(),
    "analytic_mean",
    "=PsiMaxExtreme(3,0)"
);
scalar_case!(
    psi_minextreme_keeps_minimum_orientation,
    "=PsiMinExtreme(3,2)",
    "min_extreme",
    &[("location", 3.0), ("scale", 2.0)],
    3.0 - 2.0 * 0.577_215_664_901_532_9,
    5.0,
    1.0 - (-std::f64::consts::E).exp(),
    "analytic_mean",
    "=PsiMinExtreme(3,0)"
);
scalar_case!(
    psi_rayleigh_uses_scale_not_variance,
    "=PsiRayleigh(2)",
    "rayleigh",
    &[("scale", 2.0)],
    2.0 * std::f64::consts::FRAC_PI_2.sqrt(),
    2.0,
    1.0 - (-0.5_f64).exp(),
    "analytic_mean",
    "=PsiRayleigh(0)"
);
scalar_case!(
    psi_reciprocal_has_positive_loguniform_bounds,
    "=PsiReciprocal(2,8)",
    "reciprocal",
    &[("low", 2.0), ("high", 8.0)],
    6.0 / 4.0_f64.ln(),
    4.0,
    0.5,
    "analytic_mean",
    "=PsiReciprocal(0,8)"
);
scalar_case!(
    psi_student_one_df_has_cauchy_cdf_and_median_fallback,
    "=PsiStudent(1)",
    "student_t",
    &[("degrees_of_freedom", 1.0), ("location", 0.0), ("scale", 1.0)],
    0.0,
    1.0,
    0.75,
    "median_fallback",
    "=PsiStudent(1.5)"
);
scalar_case!(
    psi_hypsecant_converts_sd_parameter_to_sdk_scale,
    "=PsiHypSecant(3,2)",
    "hyperbolic_secant",
    &[("location", 3.0), ("scale", 4.0 / std::f64::consts::PI)],
    3.0,
    5.0,
    2.0 / std::f64::consts::PI * std::f64::consts::FRAC_PI_2.exp().atan(),
    "analytic_mean",
    "=PsiHypSecant(3,0)"
);

#[test]
fn shared_psi_registry_drives_capabilities_and_actual_known_worksheet_functions() {
    let mut s = session();
    let capabilities = s.execute("solvercraft.capabilities", json!({})).unwrap();
    let signatures = capabilities["psi_import"]["signatures"].as_array().unwrap();
    assert_eq!(signatures.len(), 31);
    assert_eq!(capabilities["psi_import"]["core_numeric_signatures"], signatures.len());
    let mut unique = HashSet::new();
    for signature in signatures {
        let worksheet = signature["worksheet_name"].as_str().unwrap();
        assert!(unique.insert(worksheet));
        assert!(gridcraft_calc::is_known_function(worksheet), "{worksheet}");
        assert_eq!(gridcraft_calc::distribution_arity(worksheet), Some(signature["required_arguments"].as_u64().unwrap() as usize));
        assert!(!signature["legacy_name"].as_str().unwrap().ends_with("Alt"));
    }
    for source in ["=PsiCauchy(3,2)", "=PsiLaplace(3,2)", "=PsiHyperGeo(3,4,10)"] {
        set(&mut s, "A1", source);
        let before = Arc::clone(&s.doc().unwrap().wb);
        let response = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
        assert_eq!(response["changes"], json!([]));
        assert_eq!(response["unsupported"][0]["reason"], "unsupported_signature");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
}

#[test]
fn explicit_simulation_distribution_and_worksheet_outside_override_have_separate_provenance() {
    let mut s = session();
    // This third argument is an SC extension, never a supported Psi property.
    set(&mut s, "A1", "=SC.PSI.STUDENT(1,8)");
    set(&mut s, "B1", "=SC.OUTPUT(A1)");
    let discovery = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovery["inputs"][0]["nominal"], 8.0);
    assert_eq!(discovery["inputs"][0]["nominal_basis"], "explicit_override");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let preview = s.execute("solvercraft.simulate",json!({"inputs":[{"cell":"A1","distribution":{"kind":"constant","value":7}}],"outputs":discovery["outputs"],"options":{"trials":2,"retain_samples":true}})).unwrap();
    assert_eq!(preview["result"]["samples"], json!([[7.0], [7.0]]));
    assert_eq!(preview["request"]["inputs"][0]["distribution"], json!({"kind":"constant","value":7.0}));
    assert_eq!(preview["request"]["inputs"][0]["worksheet_nominal"], json!({"value":8.0,"basis":"explicit_override"}));
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::Number(8.0));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}

#[test]
fn additional_scalar_domains_keep_real_locations_and_undefined_f_mean_labelled() {
    check_case_with_basis(
        "=PsiLevy(-3,2)",
        "levy",
        &[("location", -3.0), ("scale", 2.0)],
        -3.0 + 2.0 / 0.674_489_750_196_081_7_f64.powi(2),
        -1.0,
        0.317_310_507_862_914_1,
        "median_fallback",
    );
    // F(2,2) has no population mean. Its median and F(1)=1/2 remain finite.
    check_case_with_basis("=PsiFDist(2,2)", "f", &[("numerator_df", 2.0), ("denominator_df", 2.0)], 1.0, 1.0, 0.5, "median_fallback");
    // Two t degrees of freedom have a mean, despite an undefined variance.
    check_case_with_basis(
        "=PsiStudent(2)",
        "student_t",
        &[("degrees_of_freedom", 2.0), ("location", 0.0), ("scale", 1.0)],
        0.0,
        std::f64::consts::SQRT_2,
        0.5 + std::f64::consts::SQRT_2 / 4.0,
        "analytic_mean",
    );
}
