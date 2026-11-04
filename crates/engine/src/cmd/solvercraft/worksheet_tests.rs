#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::{CellRef, Session, Value, json};
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({})).unwrap();
    s
}
fn set(s: &mut Session, cell: &str, input: &str) {
    s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
}
#[test]
fn all_seventeen_sc_families_have_independent_nominal_references() {
    let mut s = session();
    let cases = [
        ("SC.UNIFORM(0,2)", 1.0),
        ("SC.DISCRETEUNIFORM(1,3)", 2.0),
        ("SC.NORMAL(2,3)", 2.0),
        ("SC.LOGNORMAL(0,0)", 1.0),
        ("SC.TRIANGULAR(0,1,5)", 2.0),
        ("SC.BETAPERT(0,1,6,4)", 10.0 / 6.0),
        ("SC.BETA(2,3)", 0.4),
        ("SC.GAMMA(2,3)", 6.0),
        ("SC.EXPONENTIAL(0.5)", 2.0),
        ("SC.WEIBULL(1,3)", 3.0),
        ("SC.BERNOULLI(0.25)", 0.25),
        ("SC.BINOMIAL(4,0.25)", 1.0),
        ("SC.POISSON(2)", 2.0),
        ("SC.NEGATIVEBINOMIAL(2,0.5)", 2.0),
        ("SC.GEOMETRIC(0.25)", 3.0),
        ("SC.EMPIRICAL({1,2,6})", 3.0),
        ("SC.EMPIRICALCONTINUOUS({0,2,8})", 3.0),
    ];
    for (i, (formula, expected)) in cases.iter().enumerate() {
        let cell = format!("A{}", i + 1);
        set(&mut s, &cell, &format!("={formula}"));
        let Value::Number(value) = s.doc().unwrap().wb.active().unwrap().value(CellRef::parse(&cell).unwrap()) else {
            panic!("{formula} did not return a number")
        };
        assert!((value - expected).abs() < 1e-10, "{formula}: {value} != {expected}");
    }
    set(&mut s, "B1", "=SC.OUTPUT(SUM(A1:A17))");
    let discovered = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovered["inputs"].as_array().unwrap().len(), 17);
    let result = s.execute("solvercraft.simulate", json!({"options":{"trials":8,"seed":"7"}})).unwrap();
    assert_eq!(result["result"]["completed_trials"], 8);
}
#[test]
fn verified_psi_shims_preserve_parameter_conventions_and_strict_domains() {
    let mut s = session();
    let cases = [
        ("PsiUniform(0,2)", 1.0),
        ("PsiIntUniform(1,3)", 2.0),
        ("PsiNormal(2,3)", 2.0),
        ("PsiLogNormal(2,1)", 2.0),
        ("PsiLogNorm2(0,1)", 0.5_f64.exp()),
        ("PsiTriangular(0,1,5)", 2.0),
        ("PsiPert(0,1,6)", 10.0 / 6.0),
        ("PsiBeta(2,3)", 0.4),
        ("PsiGamma(2,3)", 6.0),
        ("PsiExponential(2)", 2.0),
        ("PsiWeibull(1,3)", 3.0),
        ("PsiBernoulli(0.25)", 0.25),
        ("PsiBinomial(4,0.25)", 1.0),
        ("PsiPoisson(2)", 2.0),
        ("PsiNegBinomial(2,0.5)", 2.0),
        ("PsiGeometric(0.25)", 3.0),
        ("PsiDisUniform({1,2,6})", 3.0),
        ("PsiDiscrete({1,10},{0.75,0.25})", 3.25),
        ("PsiCumul(0,10,{2,6},{0.2,0.8})", 4.2),
    ];
    for (i, (formula, _)) in cases.iter().enumerate() {
        set(&mut s, &format!("A{}", i + 1), &format!("={formula}"));
    }
    let imported = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    assert_eq!(imported["changes"].as_array().unwrap().len(), cases.len());
    for (i, (formula, expected)) in cases.iter().enumerate() {
        let Value::Number(value) = s.doc().unwrap().wb.active().unwrap().value(CellRef::parse(&format!("A{}", i + 1)).unwrap()) else {
            panic!("{formula} shim did not return a number")
        };
        assert!((value - expected).abs() < 1e-10, "{formula}: {value} != {expected}");
    }
    for invalid in [
        "SC.PSI.NORMAL(0,0)",
        "SC.PSI.UNIFORM(1,1)",
        "SC.PSI.INTUNIFORM(1,1)",
        "SC.PSI.TRIANGULAR(1,1,1)",
        "SC.PSI.PERT(0,0,1)",
        "SC.PSI.BINOMIAL(0,0.5)",
        "SC.PSI.POISSON(0)",
        "SC.PSI.LOGNORMAL(1,0)",
        "SC.PSI.GEOMETRIC(0)",
        "SC.PSI.CUMUL(0,10,{0,5},{0.2,0.5})",
    ] {
        set(&mut s, "C1", &format!("={invalid}"));
        assert!(matches!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("C1").unwrap()), Value::Error(_)), "{invalid}");
    }
    set(&mut s, "C1", "=SC.PSI.GEOMETRIC(1)");
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("C1").unwrap()), Value::number(0.0));
}
