use solvercraft::risk::{weighted_mean, weighted_upper_tail_cvar, weighted_variance};

#[test]
fn adjacent_large_losses_retain_their_true_population_variance() {
    let low = 1e10_f64;
    let gap = low.next_up() - low;
    let actual = weighted_variance(&[low, low.next_up()], &[1.0, 1.0]).unwrap();
    assert_eq!(actual, gap * gap / 4.0, "rounded mean must not double variance");
}

#[test]
fn unresolved_positive_cvar_confidence_cannot_form_an_unbounded_probability_counterpart() {
    use solvercraft::{risk::*, types::*};
    for confidence in [f64::from_bits(1), 1e-20] {
        let problem = ScenarioProblem {
            name: "unresolved CVaR tail budget".into(),
            variables: vec![Variable { name: "x".into(), lower: Some(-2.0), upper: Some(2.0), kind: VariableKind::Continuous }],
            constraints: vec![],
            scenarios: [-1.0, 0.0, 1.0].into_iter().map(|a| Scenario { weight: 1.0, offset: -1.0, coefficients: vec![a] }).collect(),
            measure: RiskMeasure::ConditionalValueAtRisk { confidence },
        };
        let answer = optimize(&problem, &SolveOptions::default(), &NoControl);
        assert_eq!(answer.unwrap_err().code, "numerical_failure");
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 32.0 * f64::EPSILON * expected.abs().max(1.0), "actual={actual}, oracle={expected}");
}

fn tail_oracle(values: &[f64; 3], weights: &[f64; 3], confidence: f64) -> f64 {
    let mut indices = [0, 1, 2];
    indices.sort_by(|i, j| values[*j].total_cmp(&values[*i]));
    let tail = (1.0 - confidence) * weights.iter().sum::<f64>();
    let mut remaining = tail;
    let mut area = 0.0;
    for i in indices {
        let mass = remaining.min(weights[i]);
        area += mass * values[i];
        remaining -= mass;
    }
    area / tail
}

#[test]
fn twenty_nine_thousand_weighted_risk_laws_match_pairwise_variance_and_tail_mass_oracles() {
    let mut checked = 0;
    for value_code in 0..125 {
        let values = [(value_code % 5) as f64 - 2.0, ((value_code / 5) % 5) as f64 - 2.0, (value_code / 25) as f64 - 2.0];
        for weight_code in 1..27 {
            let weights = [(weight_code % 3) as f64, ((weight_code / 3) % 3) as f64, (weight_code / 9) as f64];
            let total = weights.iter().sum::<f64>();
            let mean = (0..3).map(|i| weights[i] * values[i]).sum::<f64>() / total;
            // Independent pairwise identity; no rounded mean enters this oracle.
            let variance = (0..3).flat_map(|i| ((i + 1)..3).map(move |j| weights[i] * weights[j] * (values[i] - values[j]).powi(2))).sum::<f64>()
                / (total * total);
            for scale in [1e-200, 1.0, 1e200] {
                let scaled_weights = weights.map(|v| v * scale);
                for offset in [0.0, 1_099_511_627_776.0, -1_099_511_627_776.0] {
                    let shifted = values.map(|v| v + offset);
                    close(weighted_mean(&shifted, &scaled_weights).unwrap(), mean + offset);
                    close(weighted_variance(&shifted, &scaled_weights).unwrap(), variance);
                    for confidence in [0.0, 0.25, 0.5, 0.75] {
                        close(
                            weighted_upper_tail_cvar(&shifted, &scaled_weights, confidence).unwrap(),
                            offset + tail_oracle(&values, &weights, confidence),
                        );
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 29_250);
    eprintln!("weighted risk: {checked} laws x6 numeric values, 0 refusals");
}
