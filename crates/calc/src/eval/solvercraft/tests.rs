#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use gridcraft_core::{Array, CellRef};
use gridcraft_model::Workbook;
use std::sync::Arc;
fn n(x: f64) -> Value {
    Value::Number(x)
}
fn list(values: &[f64]) -> Value {
    Value::Array(Arc::new(Array::row(values.iter().copied().map(n).collect())))
}
fn close(actual: f64, expected: f64, tolerance: f64, context: &str) {
    assert!((actual - expected).abs() <= tolerance * (1.0 + expected.abs()), "{context}: actual={actual}, expected={expected}");
}
fn declaration(name: &str, args: &[Value]) -> WorksheetDistribution {
    worksheet_distribution(name, args).unwrap().expect("known declaration")
}

fn verify(name: &str, formula: &str, args: Vec<Value>, nominal: f64, probe: f64, cdf: f64, invalid: Vec<Value>) {
    assert_eq!(distribution_arity(name), Some(args.len()), "{name}");
    let default = declaration(name, &args);
    close(default.nominal, nominal, 2e-12, name);
    // These CDF anchors use independent analytic identities, not nominal mean
    // or a roundtrip through the decoder. They distinguish parameter swaps.
    close(solvercraft_math::cdf(&default.distribution, probe).unwrap(), cdf, 3e-7, name);
    let value = if probe == nominal { nominal * 0.5 } else { probe };
    let mut supplied = args.clone();
    supplied.push(n(value));
    let overridden = declaration(name, &supplied);
    assert_eq!(overridden.nominal, value, "{name}");
    close(solvercraft_math::cdf(&overridden.distribution, probe).unwrap(), cdf, 3e-7, name);
    for trial in 0..4 {
        assert_eq!(
            solvercraft_math::sample(&default.distribution, 17, 23, trial).unwrap(),
            solvercraft_math::sample(&overridden.distribution, 17, 23, trial).unwrap(),
            "nominal override changed trial distribution: {name}"
        );
    }
    assert!(matches!(worksheet_distribution(name, &invalid), Err(CellError::Num)), "invalid domain: {name}");
    let mut invalid_with_override = invalid;
    invalid_with_override.push(n(value));
    assert!(
        matches!(worksheet_distribution(name, &invalid_with_override), Err(CellError::Num)),
        "nominal override bypassed invalid parameters: {name}"
    );
    assert!(matches!(worksheet_distribution(name, &args[..args.len() - 1]), Err(CellError::Value)), "missing argument: {name}");
    supplied.push(n(1.0));
    assert!(matches!(worksheet_distribution(name, &supplied), Err(CellError::Value)), "too many arguments: {name}");
    let mut bad_override = args.clone();
    bad_override.push(n(f64::NAN));
    assert!(matches!(worksheet_distribution(name, &bad_override), Err(CellError::Num)), "nonfinite override: {name}");
    let mut error_override = args;
    error_override.push(Value::Error(CellError::Ref));
    assert!(matches!(worksheet_distribution(name, &error_override), Err(CellError::Ref)), "lost override error: {name}");
    let wb = Workbook::new();
    let at = CellRef::parse("A1").unwrap();
    for now in [45000.0, 48000.0] {
        let Value::Number(actual) = crate::evaluate_at(&wb, 0, at, formula, now) else {
            panic!("formula did not return a number: {formula}");
        };
        close(actual, nominal, 2e-12, formula);
        let body = formula.strip_suffix(')').unwrap();
        assert_eq!(crate::evaluate_at(&wb, 0, at, &format!("{body},{value})"), now), n(value), "formula nominal override: {formula}");
    }
}
macro_rules! psi_case {
    ($test:ident,$name:literal,$formula:literal,$args:expr,$mean:expr,$probe:expr,$cdf:expr,$invalid:expr) => {
        #[test]
        fn $test() {
            verify($name, $formula, $args, $mean, $probe, $cdf, $invalid);
        }
    };
}
psi_case!(
    psi_uniform_default_override_domain_and_independent_parameters,
    "SC.PSI.UNIFORM",
    "SC.PSI.UNIFORM(-3,9)",
    vec![n(-3.0), n(9.0)],
    3.0,
    0.0,
    0.25,
    vec![n(1.0), n(1.0)]
);
psi_case!(
    psi_intuniform_default_override_domain_and_independent_parameters,
    "SC.PSI.INTUNIFORM",
    "SC.PSI.INTUNIFORM(2,8)",
    vec![n(2.0), n(8.0)],
    5.0,
    4.0,
    3.0 / 7.0,
    vec![n(1.5), n(6.0)]
);
psi_case!(
    psi_normal_default_override_domain_and_independent_parameters,
    "SC.PSI.NORMAL",
    "SC.PSI.NORMAL(7,2)",
    vec![n(7.0), n(2.0)],
    7.0,
    9.0,
    0.841_344_746_068_542_9,
    vec![n(7.0), n(0.0)]
);
psi_case!(
    psi_lognormal_default_override_domain_and_independent_parameters,
    "SC.PSI.LOGNORMAL",
    "SC.PSI.LOGNORMAL(10,4)",
    vec![n(10.0), n(4.0)],
    10.0,
    10.0 / 1.16_f64.sqrt() * 1.16_f64.ln().sqrt().exp(),
    0.841_344_746_068_542_9,
    vec![n(0.0), n(4.0)]
);
psi_case!(
    psi_lognorm2_default_override_domain_and_independent_parameters,
    "SC.PSI.LOGNORM2",
    "SC.PSI.LOGNORM2(0.7,0.4)",
    vec![n(0.7), n(0.4)],
    0.78_f64.exp(),
    1.1_f64.exp(),
    0.841_344_746_068_542_9,
    vec![n(0.7), n(0.0)]
);
psi_case!(
    psi_triangular_default_override_domain_and_independent_parameters,
    "SC.PSI.TRIANGULAR",
    "SC.PSI.TRIANGULAR(0,2,9)",
    vec![n(0.0), n(2.0), n(9.0)],
    11.0 / 3.0,
    1.0,
    1.0 / 18.0,
    vec![n(1.0), n(1.0), n(1.0)]
);
psi_case!(
    psi_pert_default_override_domain_and_independent_parameters,
    "SC.PSI.PERT",
    "SC.PSI.PERT(0,1.5,6)",
    vec![n(0.0), n(1.5), n(6.0)],
    2.0,
    3.0,
    0.8125,
    vec![n(0.0), n(0.0), n(4.0)]
);
psi_case!(
    psi_beta_default_override_domain_and_independent_parameters,
    "SC.PSI.BETA",
    "SC.PSI.BETA(2,5)",
    vec![n(2.0), n(5.0)],
    2.0 / 7.0,
    0.5,
    0.890625,
    vec![n(0.0), n(5.0)]
);
psi_case!(
    psi_gamma_default_override_domain_and_independent_parameters,
    "SC.PSI.GAMMA",
    "SC.PSI.GAMMA(2,3)",
    vec![n(2.0), n(3.0)],
    6.0,
    3.0,
    1.0 - 2.0 * (-1.0_f64).exp(),
    vec![n(-1.0), n(3.0)]
);
psi_case!(
    psi_exponential_default_override_domain_and_independent_parameters,
    "SC.PSI.EXPONENTIAL",
    "SC.PSI.EXPONENTIAL(4)",
    vec![n(4.0)],
    4.0,
    4.0,
    1.0 - (-1.0_f64).exp(),
    vec![n(0.0)]
);
psi_case!(
    psi_weibull_default_override_domain_and_independent_parameters,
    "SC.PSI.WEIBULL",
    "SC.PSI.WEIBULL(2,3)",
    vec![n(2.0), n(3.0)],
    3.0 * std::f64::consts::PI.sqrt() / 2.0,
    3.0,
    1.0 - (-1.0_f64).exp(),
    vec![n(0.0), n(3.0)]
);
psi_case!(
    psi_bernoulli_default_override_domain_and_independent_parameters,
    "SC.PSI.BERNOULLI",
    "SC.PSI.BERNOULLI(0.3)",
    vec![n(0.3)],
    0.3,
    0.0,
    0.7,
    vec![n(1.01)]
);
psi_case!(
    psi_binomial_default_override_domain_and_independent_parameters,
    "SC.PSI.BINOMIAL",
    "SC.PSI.BINOMIAL(5,0.2)",
    vec![n(5.0), n(0.2)],
    1.0,
    0.0,
    0.8_f64.powi(5),
    vec![n(0.0), n(0.2)]
);
psi_case!(
    psi_poisson_default_override_domain_and_independent_parameters,
    "SC.PSI.POISSON",
    "SC.PSI.POISSON(2.3)",
    vec![n(2.3)],
    2.3,
    0.0,
    (-2.3_f64).exp(),
    vec![n(0.0)]
);
psi_case!(
    psi_negbinomial_default_override_domain_and_independent_parameters,
    "SC.PSI.NEGBINOMIAL",
    "SC.PSI.NEGBINOMIAL(3,0.25)",
    vec![n(3.0), n(0.25)],
    9.0,
    0.0,
    0.25_f64.powi(3),
    vec![n(0.0), n(0.25)]
);
psi_case!(
    psi_geometric_default_override_domain_and_independent_parameters,
    "SC.PSI.GEOMETRIC",
    "SC.PSI.GEOMETRIC(0.25)",
    vec![n(0.25)],
    3.0,
    0.0,
    0.25,
    vec![n(0.0)]
);
psi_case!(
    psi_disuniform_default_override_domain_and_independent_parameters,
    "SC.PSI.DISUNIFORM",
    "SC.PSI.DISUNIFORM({1,4,9})",
    vec![list(&[1.0, 4.0, 9.0])],
    14.0 / 3.0,
    4.0,
    2.0 / 3.0,
    vec![list(&[f64::NAN])]
);
psi_case!(
    psi_discrete_default_override_domain_and_independent_parameters,
    "SC.PSI.DISCRETE",
    "SC.PSI.DISCRETE({1,4,9},{2,3,5})",
    vec![list(&[1.0, 4.0, 9.0]), list(&[2.0, 3.0, 5.0])],
    5.9,
    4.0,
    0.5,
    vec![list(&[1.0, 4.0]), list(&[-1.0, 2.0])]
);
psi_case!(
    psi_cumul_default_override_domain_and_independent_parameters,
    "SC.PSI.CUMUL",
    "SC.PSI.CUMUL(0,10,{2,6},{0.2,0.7})",
    vec![n(0.0), n(10.0), list(&[2.0, 6.0]), list(&[0.2, 0.7])],
    4.6,
    4.0,
    0.45,
    vec![n(0.0), n(10.0), list(&[2.0, 6.0]), list(&[0.7, 0.2])]
);

#[test]
fn sc_psi_nominal_policy_does_not_consult_calc_rng_or_trial_display_state() {
    struct Host {
        wb: Workbook,
        draws: usize,
    }
    impl super::super::Host for Host {
        fn workbook(&self) -> &Workbook {
            &self.wb
        }
        fn cell_value(&mut self, _sheet: usize, _cell: CellRef) -> Value {
            Value::Empty
        }
        fn now_serial(&self) -> f64 {
            45000.0
        }
        fn random(&mut self) -> f64 {
            self.draws += 1;
            0.25
        }
    }
    let mut host = Host { wb: Workbook::new(), draws: 0 };
    for (formula, expected) in [("SC.PSI.UNIFORM(0,10)", 5.0), ("SC.PSI.UNIFORM(0,10,7)", 7.0), ("SC.PSI.BERNOULLI(0.7)", 0.7)] {
        let expression = gridcraft_formula::parse(formula).unwrap();
        for _ in 0..3 {
            let mut evaluator = Evaluator::new(&mut host, 0, CellRef::parse("A1").unwrap());
            assert_eq!(evaluator.value(&expression), n(expected));
        }
    }
    assert_eq!(host.draws, 0, "SC nominal cells must not draw pseudo pre-run samples");
}

#[test]
fn explicit_nominal_uses_normal_calc_coercion_and_preserves_existing_bound_rules() {
    let wb = Workbook::new();
    let at = CellRef::parse("A1").unwrap();
    for (formula, wanted) in [
        ("SC.PSI.NORMAL(7,2)", n(7.0)),
        ("SC.PSI.NORMAL(7,2,)", n(0.0)),
        ("SC.PSI.NORMAL(7,2,TRUE)", n(1.0)),
        ("SC.PSI.NORMAL(7,2,\"3\")", n(3.0)),
        ("SC.PSI.NORMAL(7,2,\"not a number\")", Value::Error(CellError::Value)),
        ("SC.PSI.UNIFORM(2,8,1)", Value::Error(CellError::Num)),
        ("SC.PSI.TRIANGULAR(2,3,8,9)", Value::Error(CellError::Num)),
        ("SC.PSI.PERT(2,3,8,1)", Value::Error(CellError::Num)),
    ] {
        assert_eq!(crate::evaluate_at(&wb, 0, at, formula, 45000.0), wanted, "{formula}");
    }
    let one = Value::Array(Arc::new(Array::scalar(n(3.0))));
    assert_eq!(declaration("SC.PSI.NORMAL", &[n(7.0), n(2.0), one]).nominal, 3.0);
    assert!(matches!(worksheet_distribution("SC.PSI.NORMAL", &[n(7.0), n(2.0), list(&[3.0, 4.0])]), Err(CellError::Value)));
    // The value remains an explicit SC ordinary-cell override; it is not
    // interpreted as a PsiStatic, PsiBaseCase or runtime sample-mean property.
    assert_eq!(declaration("SC.PSI.EXPONENTIAL", &[n(4.0), n(2.0)]).nominal, 2.0);
}
