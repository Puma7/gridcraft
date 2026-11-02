use solvercraft_math::time_series::*;
use solvercraft_types::NoControl;
fn check(actual: f64, expected: f64, scale: f64) {
    assert!((actual - expected).abs() <= 3e-12 * scale, "actual={actual},expected={expected},scale={scale}");
}
#[test]
fn independent_recurrence_matrix_for_every_process() {
    let shocks = [-2.0_f64, -1.0, -0.25, 0.0, 0.5, 1.0, 2.0, 0.25, -0.5, -1.5, 1.5, 0.0, 1.0, -1.0, 0.5, -0.25, 0.75];
    let mut paths = 0;
    let mut coordinates = 0;
    for s in [1e-100_f64, 1.0, 1e100] {
        for m in [-2.0_f64, 0.0, 3.0] {
            let mean = m * s;
            let mut processes = Vec::new();
            for a in [-0.8_f64, -0.3, 0.0, 0.3, 0.8] {
                processes.push(Process::Ar1 { innovation_std: s, coefficient: a, initial_value: mean + 2.0 * s });
                processes.push(Process::Ma1 { innovation_std: s, coefficient: a, initial_error: 2.0 * s });
            }
            for a in [-0.5_f64, 0.0, 0.5] {
                for b in [-0.2_f64, 0.0, 0.2] {
                    processes.push(Process::Ar2 {
                        innovation_std: s,
                        first: a,
                        second: b,
                        initial_latest: mean + 2.0 * s,
                        initial_previous: mean - s,
                    });
                    processes.push(Process::Ma2 {
                        innovation_std: s,
                        first: a,
                        second: b,
                        initial_latest_error: 2.0 * s,
                        initial_previous_error: -s,
                    });
                    processes.push(Process::Arma11 { innovation_std: s, ar: a, ma: b, initial_value: mean + 2.0 * s, initial_error: 2.0 * s });
                }
            }
            for a in [0.0_f64, 0.1, 0.4] {
                processes.push(Process::Arch1 { variance_intercept: 0.5 * s * s, error_coefficient: a, initial_value: mean + 2.0 * s });
                for b in [0.0_f64, 0.2, 0.5] {
                    processes.push(Process::Garch11 {
                        variance_intercept: 0.5 * s * s,
                        error_coefficient: a,
                        variance_coefficient: b,
                        initial_value: mean + 2.0 * s,
                        initial_std: s,
                    });
                }
            }
            for theta in [-0.5_f64, 0.0, 0.5] {
                for gamma in [0.2_f64, 0.7] {
                    processes.push(Process::Egarch11 {
                        log_variance_intercept: -0.2 + 0.6 * 2.0 * s.ln(),
                        error_coefficient: 0.3,
                        log_variance_coefficient: 0.4,
                        theta,
                        gamma,
                        initial_value: mean + 2.0 * s,
                        initial_std: s,
                    });
                }
            }
            for power in [1.0_f64, 2.0, 3.0] {
                for asymmetry in [-0.5_f64, 0.0, 0.5] {
                    processes.push(Process::Aparch11 {
                        power_intercept: (0.5 * s).powf(power),
                        error_coefficient: 0.2,
                        persistence: 0.3,
                        power,
                        asymmetry,
                        initial_value: mean + 2.0 * s,
                        initial_std: s,
                    });
                }
            }
            for process in processes {
                let spec = TimeSeriesSpec { mean, process: process.clone(), postprocess: Postprocess::default() };
                let actual = simulate_with_shocks(&spec, &shocks, &TimeSeriesLimits::default(), &NoControl).unwrap();
                let mut center = 2.0 * s;
                let mut previous = -s;
                let mut error = 2.0 * s;
                let mut prev_error = -s;
                let mut std = s;
                let mut z_prev = 2.0;
                for (i, z) in shocks.iter().enumerate() {
                    let next_std = match process {
                        Process::Arch1 { variance_intercept, error_coefficient, .. } => {
                            (variance_intercept + error_coefficient * error * error).sqrt()
                        }
                        Process::Garch11 { variance_intercept, error_coefficient, variance_coefficient, .. } => {
                            (variance_intercept + error_coefficient * error * error + variance_coefficient * std * std).sqrt()
                        }
                        Process::Egarch11 { log_variance_intercept, error_coefficient, log_variance_coefficient, theta, gamma, .. } => (0.5
                            * (log_variance_intercept
                                + error_coefficient * (theta * z_prev + gamma * (z_prev.abs() - (2.0 / std::f64::consts::PI).sqrt()))
                                + log_variance_coefficient * 2.0 * std.ln()))
                        .exp(),
                        Process::Aparch11 { power_intercept, error_coefficient, persistence, power, asymmetry, .. } => {
                            (power_intercept + error_coefficient * (error.abs() - asymmetry * error).powf(power) + persistence * std.powf(power))
                                .powf(1.0 / power)
                        }
                        _ => s,
                    };
                    let innovation = next_std * z;
                    let next = match process {
                        Process::Ar1 { coefficient, .. } => coefficient * center + innovation,
                        Process::Ar2 { first, second, .. } => first * center + second * previous + innovation,
                        Process::Ma1 { coefficient, .. } => coefficient * error + innovation,
                        Process::Ma2 { first, second, .. } => first * error + second * prev_error + innovation,
                        Process::Arma11 { ar, ma, .. } => ar * center + ma * error + innovation,
                        _ => innovation,
                    };
                    check(actual.values[i], mean + next, s);
                    check(actual.conditional_std[i], next_std, s);
                    coordinates += 1;
                    previous = center;
                    center = next;
                    prev_error = error;
                    error = innovation;
                    std = next_std;
                    z_prev = *z;
                }
                paths += 1;
            }
        }
    }
    assert_eq!(paths, 576);
    assert_eq!(coordinates, 9792);
    eprintln!("distinct time-series specs={paths}, step-oracle tuples={coordinates} (value and conditional std each)");
}
