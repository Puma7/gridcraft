#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_simulation::grid::*;
use solvercraft_types::*;
fn axes() -> Vec<ParameterAxis> {
    vec![ParameterAxis { name: "price".into(), values: vec![2.0, 5.0] }, ParameterAxis { name: "quantity".into(), values: vec![3.0, 7.0] }]
}
#[test]
fn cartesian_parameter_values_are_constant_and_result_coordinates_are_unique() {
    let options = GridOptions { simulation: SimulationOptions { trials: 17, retain_samples: true, ..Default::default() }, ..Default::default() };
    let result = simulate_grid(&axes(), &[], &options, &NoControl, |p, _| Ok(vec![p[0] * p[1]])).unwrap();
    assert_eq!(result.requested_runs, 4);
    assert_eq!(result.runs.len(), 4);
    for (run, expected) in result.runs.iter().zip([6.0, 14.0, 15.0, 35.0]) {
        assert_eq!(run.result.summaries[0].mean, expected);
        assert_eq!(run.result.summaries[0].variance, Some(0.0));
        assert!(run.result.samples.as_ref().unwrap().iter().all(|row| row[0] == expected));
    }
    assert_eq!(result.runs[0].coordinates, vec![0, 0]);
    assert_eq!(result.runs[3].coordinates, vec![1, 1]);
}
#[test]
fn common_random_numbers_and_distinct_seeds_have_explicit_repeatable_semantics() {
    let distributions = [Distribution::Uniform { low: 0.0, high: 1.0 }];
    let options = GridOptions {
        simulation: SimulationOptions { trials: 32, retain_samples: true, seed: u64::MAX, ..Default::default() },
        ..Default::default()
    };
    let common = simulate_grid(&axes(), &distributions, &options, &NoControl, |_, x| Ok(x.to_vec())).unwrap();
    for run in &common.runs {
        assert_eq!(run.effective_seed, u64::MAX);
        assert_eq!(run.result.samples, common.runs[0].result.samples);
    }
    let distinct_options = GridOptions { streams: GridStreams::DistinctSeeds, ..options };
    let distinct = simulate_grid(&axes(), &distributions, &distinct_options, &NoControl, |_, x| Ok(x.to_vec())).unwrap();
    let repeated = simulate_grid(&axes(), &distributions, &distinct_options, &NoControl, |_, x| Ok(x.to_vec())).unwrap();
    assert_ne!(distinct.runs[0].effective_seed, distinct.runs[1].effective_seed);
    assert_ne!(distinct.runs[0].result.samples, distinct.runs[1].result.samples);
    assert_eq!(distinct.runs[1].result.samples, repeated.runs[1].result.samples);
}
#[test]
fn grid_limits_and_cancellation_precede_evaluator_work() {
    let options = GridOptions { max_runs: 3, ..Default::default() };
    let mut called = false;
    assert!(
        simulate_grid(&axes(), &[], &options, &NoControl, |_, _| {
            called = true;
            Ok(vec![1.0])
        })
        .is_err()
    );
    assert!(!called);
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let result = simulate_grid(&axes(), &[], &GridOptions::default(), &Cancel, |_, _| {
        called = true;
        Ok(vec![1.0])
    })
    .unwrap();
    assert_eq!(result.termination, Termination::Cancelled);
    assert!(result.runs.is_empty());
    assert!(!called);
    let options = GridOptions { max_total_trials: 3, simulation: SimulationOptions { trials: 1, ..Default::default() }, ..Default::default() };
    assert!(simulate_grid(&axes(), &[], &options, &NoControl, |_, _| Ok(vec![1.0])).is_err());
}
