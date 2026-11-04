#![allow(clippy::unwrap_used)]
use solvercraft_math::time_series::{
    Integration, Postprocess, Process, Seasonality, SeriesAddress, TimeSeriesLimits, TimeSeriesSpec, Transformation, replay_tssip, sample_path,
    simulate_with_shocks, validate, value_at,
};
use solvercraft_types::{NoControl, RunControl};
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-13 * (1.0 + b.abs()), "actual {a}, expected {b}");
}
fn spec(process: Process) -> TimeSeriesSpec {
    TimeSeriesSpec { mean: 10.0, process, postprocess: Postprocess::default() }
}
fn replay(process: Process, shocks: &[f64]) -> solvercraft_math::time_series::TimeSeriesPath {
    simulate_with_shocks(&spec(process), shocks, &TimeSeriesLimits::default(), &NoControl).unwrap()
}
#[test]
fn arma_recursions_match_independent_two_step_algebra() {
    let fixtures = [
        (Process::Ar1 { innovation_std: 2.0, coefficient: 0.5, initial_value: 14.0 }, [13.0, 9.5]),
        (Process::Ar2 { innovation_std: 2.0, first: 0.4, second: -0.1, initial_latest: 14.0, initial_previous: 8.0 }, [12.8, 8.72]),
        (Process::Ma1 { innovation_std: 2.0, coefficient: 0.5, initial_error: 4.0 }, [13.0, 8.5]),
        (Process::Ma2 { innovation_std: 2.0, first: 0.5, second: -0.2, initial_latest_error: 4.0, initial_previous_error: -1.0 }, [13.2, 7.7]),
        (Process::Arma11 { innovation_std: 2.0, ar: 0.4, ma: 0.3, initial_value: 14.0, initial_error: 4.0 }, [13.8, 9.82]),
    ];
    for (process, expected) in fixtures {
        let path = replay(process, &[0.5, -1.0]);
        assert_eq!(path.steps, 2);
        assert_eq!(path.standard_normal_shocks, [0.5, -1.0]);
        assert_eq!(path.conditional_std, [2.0, 2.0]);
        for (a, b) in path.values.iter().zip(expected) {
            close(*a, b);
        }
    }
}
#[test]
fn arch_garch_and_egarch_match_independent_conditional_variance_algebra() {
    let arch = replay(Process::Arch1 { variance_intercept: 1.0, error_coefficient: 0.5, initial_value: 14.0 }, &[0.5, -1.0]);
    close(arch.conditional_std[0], 3.0);
    close(arch.values[0], 11.5);
    close(arch.conditional_std[1], 2.125_f64.sqrt());
    close(arch.values[1], 10.0 - 2.125_f64.sqrt());
    let garch = replay(
        Process::Garch11 { variance_intercept: 1.0, error_coefficient: 0.2, variance_coefficient: 0.3, initial_value: 14.0, initial_std: 2.0 },
        &[0.5, -1.0],
    );
    close(garch.conditional_std[0], 5.4_f64.sqrt());
    close(garch.values[0], 10.0 + 0.5 * 5.4_f64.sqrt());
    close(garch.conditional_std[1], 1.7);
    close(garch.values[1], 8.3);
    let egarch = replay(
        Process::Egarch11 {
            log_variance_intercept: -0.2,
            error_coefficient: 0.3,
            log_variance_coefficient: 0.4,
            theta: -0.5,
            gamma: 0.7,
            initial_value: 14.0,
            initial_std: 2.0,
        },
        &[0.5, -1.0],
    );
    let abs_mean = (2.0 / std::f64::consts::PI).sqrt();
    let log_var1 = -0.2 + 0.3 * (-0.5 * 2.0 + 0.7 * (2.0 - abs_mean)) + 0.4 * 4.0_f64.ln();
    let log_var2 = -0.2 + 0.3 * (-0.5 * 0.5 + 0.7 * (0.5 - abs_mean)) + 0.4 * log_var1;
    close(egarch.conditional_std[0], (0.5 * log_var1).exp());
    close(egarch.conditional_std[1], (0.5 * log_var2).exp());
    close(egarch.values[1], 10.0 - (0.5 * log_var2).exp());
}
#[test]
fn aparch_power_two_zero_asymmetry_is_garch_and_leverage_changes_variance() {
    let shocks = [0.5, -1.0, 0.0, 2.0, -0.3];
    let g = replay(
        Process::Garch11 { variance_intercept: 1.0, error_coefficient: 0.2, variance_coefficient: 0.3, initial_value: 14.0, initial_std: 2.0 },
        &shocks,
    );
    let a = replay(
        Process::Aparch11 {
            power_intercept: 1.0,
            error_coefficient: 0.2,
            persistence: 0.3,
            power: 2.0,
            asymmetry: 0.0,
            initial_value: 14.0,
            initial_std: 2.0,
        },
        &shocks,
    );
    for (x, y) in g.values.iter().zip(&a.values) {
        close(*x, *y);
    }
    for (x, y) in g.conditional_std.iter().zip(&a.conditional_std) {
        close(*x, *y);
    }
    let a = replay(
        Process::Aparch11 {
            power_intercept: 1.0,
            error_coefficient: 0.2,
            persistence: 0.3,
            power: 1.0,
            asymmetry: 0.4,
            initial_value: 14.0,
            initial_std: 2.0,
        },
        &[1.0, -1.0],
    );
    close(a.conditional_std[0], 1.0 + 0.2 * (4.0 - 0.4 * 4.0) + 0.3 * 2.0);
    let s = a.conditional_std[0];
    close(a.conditional_std[1], 1.0 + 0.2 * (s - 0.4 * s) + 0.3 * s);
}
fn constant() -> TimeSeriesSpec {
    TimeSeriesSpec {
        mean: 1.0,
        process: Process::Ar1 { innovation_std: 0.0, coefficient: 0.0, initial_value: 1.0 },
        postprocess: Postprocess::default(),
    }
}
#[test]
fn integration_and_seasonality_have_explicit_initial_conditions_and_order() {
    let cases = [
        (Postprocess { integration: Integration::First { initial_level: 10.0 }, ..Postprocess::default() }, vec![11.0, 12.0, 13.0, 14.0]),
        (
            Postprocess { integration: Integration::Second { initial_level: 10.0, initial_difference: 2.0 }, ..Postprocess::default() },
            vec![13.0, 17.0, 22.0, 28.0],
        ),
        (
            Postprocess { seasonality: Seasonality::Additive { terms: vec![100.0, 200.0], starting_phase: 1 }, ..Postprocess::default() },
            vec![201.0, 101.0, 201.0, 101.0],
        ),
        (
            Postprocess { seasonality: Seasonality::First { previous_season: vec![10.0, 20.0] }, ..Postprocess::default() },
            vec![11.0, 21.0, 12.0, 22.0],
        ),
        (
            Postprocess {
                seasonality: Seasonality::Second { previous_season: vec![10.0, 20.0], older_season: vec![6.0, 14.0] },
                ..Postprocess::default()
            },
            vec![15.0, 27.0, 21.0, 35.0],
        ),
        (
            Postprocess {
                integration: Integration::First { initial_level: 1.0 },
                seasonality: Seasonality::Additive { terms: vec![2.0], starting_phase: 0 },
                transformation: Transformation::Square { shift: 3.0 },
            },
            vec![19.0, 28.0, 39.0, 52.0],
        ),
    ];
    for (postprocess, expected) in cases {
        let mut config = constant();
        config.postprocess = postprocess;
        let path = simulate_with_shocks(&config, &[0.0; 4], &TimeSeriesLimits::default(), &NoControl).unwrap();
        assert_eq!(path.values, expected);
    }
    let mut config = constant();
    config.postprocess.transformation = Transformation::Exponential { shift: 2.0 };
    close(simulate_with_shocks(&config, &[0.0], &TimeSeriesLimits::default(), &NoControl).unwrap().values[0], 1.0_f64.exp() + 2.0);
}
#[test]
fn addressed_paths_are_prefix_stable_and_trial_binding_sensitive() {
    let config = spec(Process::Arma11 { innovation_std: 2.0, ar: 0.4, ma: 0.3, initial_value: 14.0, initial_error: 4.0 });
    let address = SeriesAddress { seed: u64::MAX, binding: 99, trial: 7 };
    let short = sample_path(&config, 3, address, &TimeSeriesLimits::default(), &NoControl).unwrap();
    let long = sample_path(&config, 10, address, &TimeSeriesLimits::default(), &NoControl).unwrap();
    assert_eq!(short.values, long.values[..3]);
    assert_eq!(short.standard_normal_shocks, long.standard_normal_shocks[..3]);
    assert_eq!(value_at(&config, 2, address, &TimeSeriesLimits::default(), &NoControl).unwrap(), long.values[2]);
    let other_trial = sample_path(&config, 3, SeriesAddress { trial: 8, ..address }, &TimeSeriesLimits::default(), &NoControl).unwrap();
    let other_binding = sample_path(&config, 3, SeriesAddress { binding: 98, ..address }, &TimeSeriesLimits::default(), &NoControl).unwrap();
    assert_ne!(short.standard_normal_shocks, other_trial.standard_normal_shocks);
    assert_ne!(short.standard_normal_shocks, other_binding.standard_normal_shocks);
}
struct Cancel;
impl RunControl for Cancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}
#[test]
fn malformed_domains_limits_numeric_errors_and_cancellation_are_explicit() {
    let limits = TimeSeriesLimits::default();
    assert!(replay_tssip(&[vec![1.0], vec![2.0, 3.0]], 0, &limits, &NoControl).is_err());
    assert!(replay_tssip(&[vec![f64::NAN]], 0, &limits, &NoControl).is_err());
    assert!(simulate_with_shocks(&constant(), &[f64::NAN], &limits, &NoControl).is_err());
    assert!(simulate_with_shocks(&constant(), &[0.0], &limits, &Cancel).is_err());
    assert!(sample_path(&constant(), 1, SeriesAddress { seed: 0, binding: 0, trial: 0 }, &limits, &Cancel).is_err());
    assert!(validate(&constant(), 100, &TimeSeriesLimits { max_memory_bytes: 100, ..limits.clone() }).is_err());
    assert!(validate(&constant(), 2, &TimeSeriesLimits { max_steps: 1, ..limits.clone() }).is_err());
    assert!(value_at(&constant(), usize::MAX, SeriesAddress { seed: 0, binding: 0, trial: 0 }, &limits, &NoControl).is_err());
    let nonstationary = spec(Process::Ar2 { innovation_std: 1.0, first: 0.8, second: 0.4, initial_latest: 10.0, initial_previous: 10.0 });
    assert!(validate(&nonstationary, 1, &limits).is_err());
    assert!(validate(&nonstationary, 1, &TimeSeriesLimits { require_stationary: false, ..limits.clone() }).is_ok());
    let invalid =
        spec(Process::Garch11 { variance_intercept: 1.0, error_coefficient: -0.1, variance_coefficient: 0.2, initial_value: 1.0, initial_std: 1.0 });
    assert!(validate(&invalid, 1, &limits).is_err());
    let overflow = spec(Process::Egarch11 {
        log_variance_intercept: 2000.0,
        error_coefficient: 0.0,
        log_variance_coefficient: 0.0,
        theta: 0.0,
        gamma: 0.0,
        initial_value: 10.0,
        initial_std: 1.0,
    });
    assert!(simulate_with_shocks(&overflow, &[0.0], &limits, &NoControl).is_err());
    let mut bad_season = constant();
    bad_season.postprocess.seasonality = Seasonality::Second { previous_season: vec![1.0], older_season: vec![] };
    assert!(validate(&bad_season, 1, &limits).is_err());
}
#[test]
fn tssip_replays_whole_columns_without_rng() {
    let matrix = [vec![1.0, 10.0], vec![2.0, 20.0], vec![3.0, 30.0]];
    assert_eq!(replay_tssip(&matrix, 0, &TimeSeriesLimits::default(), &NoControl).unwrap(), [1.0, 2.0, 3.0]);
    assert_eq!(replay_tssip(&matrix, 1, &TimeSeriesLimits::default(), &NoControl).unwrap(), [10.0, 20.0, 30.0]);
    assert!(replay_tssip(&matrix, 2, &TimeSeriesLimits::default(), &NoControl).is_err());
    assert!(replay_tssip(&matrix, 0, &TimeSeriesLimits::default(), &Cancel).is_err());
}
#[test]
fn no_op_configurations_reject_unknown_fields_instead_of_ignoring_them() {
    use serde::{
        Deserialize,
        de::value::{Error, MapDeserializer},
    };
    let valid = || MapDeserializer::<_, Error>::new([("kind", "none")].into_iter());
    assert!(matches!(Integration::deserialize(valid()), Ok(Integration::None {})));
    assert!(matches!(Seasonality::deserialize(valid()), Ok(Seasonality::None {})));
    let extra = || MapDeserializer::<_, Error>::new([("kind", "none"), ("silently_ignored_option", "wrong")].into_iter());
    assert!(Integration::deserialize(extra()).is_err());
    assert!(Seasonality::deserialize(extra()).is_err());
}
#[test]
fn json_configuration_variants_and_full_width_addresses_are_strict() {
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::{Value, json};
    fn check<T: Serialize + DeserializeOwned>(fixtures: &[Value]) {
        for fixture in fixtures {
            let parsed: T = serde_json::from_value(fixture.clone()).unwrap();
            let encoded = serde_json::to_value(parsed).unwrap();
            let _: T = serde_json::from_value(encoded).unwrap();
            let mut extra = fixture.clone();
            extra.as_object_mut().unwrap().insert("silently_ignored".into(), json!(true));
            assert!(serde_json::from_value::<T>(extra).is_err(), "fixture {fixture}");
        }
    }
    check::<Process>(&[
        json!({"kind":"ar1","innovation_std":1,"coefficient":0.5,"initial_value":0}),
        json!({"kind":"ar2","innovation_std":1,"first":0.5,"second":0.2,"initial_latest":0,"initial_previous":0}),
        json!({"kind":"ma1","innovation_std":1,"coefficient":0.5,"initial_error":0}),
        json!({"kind":"ma2","innovation_std":1,"first":0.5,"second":0.2,"initial_latest_error":0,"initial_previous_error":0}),
        json!({"kind":"arma11","innovation_std":1,"ar":0.5,"ma":0.2,"initial_value":0,"initial_error":0}),
        json!({"kind":"arch1","variance_intercept":1,"error_coefficient":0.2,"initial_value":0}),
        json!({"kind":"garch11","variance_intercept":1,"error_coefficient":0.2,"variance_coefficient":0.3,"initial_value":0,"initial_std":1}),
        json!({"kind":"egarch11","log_variance_intercept":0,"error_coefficient":0.2,"log_variance_coefficient":0.3,"theta":-0.5,"gamma":0.5,"initial_value":0,"initial_std":1}),
        json!({"kind":"aparch11","power_intercept":1,"error_coefficient":0.2,"persistence":0.3,"power":2,"asymmetry":0,"initial_value":0,"initial_std":1}),
    ]);
    check::<Integration>(&[
        json!({"kind":"none"}),
        json!({"kind":"first","initial_level":1}),
        json!({"kind":"second","initial_level":1,"initial_difference":2}),
    ]);
    check::<Seasonality>(&[
        json!({"kind":"none"}),
        json!({"kind":"additive","terms":[1,2],"starting_phase":0}),
        json!({"kind":"first","previous_season":[1,2]}),
        json!({"kind":"second","previous_season":[1,2],"older_season":[0,1]}),
    ]);
    check::<Transformation>(&[json!({"kind":"identity","shift":0}), json!({"kind":"exponential","shift":1}), json!({"kind":"square","shift":2})]);
    check::<TimeSeriesLimits>(&[json!({})]);
    let address = SeriesAddress { seed: u64::MAX, binding: u64::MAX - 1, trial: u64::MAX - 2 };
    let encoded = serde_json::to_value(address).unwrap();
    assert_eq!(encoded["seed"], u64::MAX.to_string());
    let decoded: SeriesAddress = serde_json::from_value(encoded).unwrap();
    assert_eq!((decoded.seed, decoded.binding, decoded.trial), (address.seed, address.binding, address.trial));
    assert!(serde_json::from_value::<SeriesAddress>(json!({"seed":"18446744073709551616","binding":"0","trial":"0"})).is_err());
    assert!(serde_json::from_value::<SeriesAddress>(json!({"seed":1.5,"binding":"0","trial":"0"})).is_err());
}
#[test]
fn gaussian_ar2_population_variance_matches_yule_walker_not_source_typo() {
    // Independent Yule-Walker variance: sigma^2*(1-phi2)/
    // ((1+phi2)*((1-phi2)^2-phi1^2)); no stationary initialization claimed.
    let config = TimeSeriesSpec {
        mean: 0.0,
        process: Process::Ar2 { innovation_std: 1.0, first: 0.5, second: 0.2, initial_latest: 0.0, initial_previous: 0.0 },
        postprocess: Postprocess::default(),
    };
    let mut sum = 0.0;
    let mut square = 0.0;
    let n = 6000;
    for trial in 0..n {
        let x = value_at(&config, 79, SeriesAddress { seed: 2048, binding: 3, trial }, &TimeSeriesLimits::default(), &NoControl).unwrap();
        sum += x;
        square += x * x;
    }
    let mean = sum / n as f64;
    let variance = square / n as f64 - mean * mean;
    let expected = 0.8 / (1.2 * (0.8 * 0.8 - 0.5 * 0.5));
    assert!(mean.abs() < 0.06, "mean={mean}");
    assert!((variance - expected).abs() < 0.09, "variance={variance}, expected={expected}");
}
