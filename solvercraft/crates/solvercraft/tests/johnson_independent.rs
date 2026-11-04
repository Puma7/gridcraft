//! Independent population quadrature, not empirical samples or fitter output.
//! NumPy 2.2.0 hermgauss at 128 and 256 nodes agreed in all four moments
//! within 3.3e-15 relative to 1+|moment|. Symmetry gives exact zero skewness.
use solvercraft::{math, types::Distribution};
use std::error::Error;

struct Fixture {
    su: bool,
    gamma: f64,
    delta: f64,
    location: f64,
    scale: f64,
    moments: [f64; 4],
    // Independent closed-form translation of normal z = -1, 0, +1.
    quantiles: [f64; 3],
}
const FIXTURES: &[Fixture] = &[
    Fixture {
        su: true,
        gamma: 0.0,
        delta: 30.0,
        location: 1e6,
        scale: 100.0,
        moments: [1e6, 3.3351860428098545, 0.0, 3.004451861005953],
        quantiles: [999996.6660493484, 1000000.0, 1000003.3339506516],
    },
    Fixture {
        su: true,
        gamma: -1.5,
        delta: 3.0,
        location: -7.0,
        scale: 4.0,
        moments: [-4.796542779592993, 1.6004287932849586, 0.5118017464775885, 3.8858701576376102],
        quantiles: [-6.330242624049936, -4.91561877802501, -3.2672446352285065],
    },
    Fixture {
        su: false,
        gamma: 0.0,
        delta: 30.0,
        location: 1e6,
        scale: 100.0,
        moments: [1000050.0, 0.8331020017434304, 0.0, 2.997781474181728],
        quantiles: [1000049.1667438186, 1000050.0, 1000050.8332561814],
    },
    Fixture {
        su: false,
        gamma: 6.0,
        delta: 2.0,
        location: -7.0,
        scale: 4.0,
        moments: [-6.7893201840755575, 0.10323263626686258, 1.4123218777756368, 6.442730703717024],
        quantiles: [-6.882751076994575, -6.810296507289733, -6.696567279915026],
    },
    Fixture {
        su: false,
        gamma: -4.0,
        delta: 2.0,
        location: 7.0,
        scale: 0.25,
        moments: [7.2177483659056945, 0.014069376653941257, -1.0619721545922727, 4.68460850618498],
        quantiles: [7.204393619048411, 7.22019926949447, 7.231035454994689],
    },
    Fixture {
        su: true,
        gamma: 0.0,
        delta: 1.0,
        location: 3.0,
        scale: 2.0,
        moments: [3.0, 3.574648541865522, 0.0, 36.18813111550278],
        quantiles: [0.6495976127123972, 3.0, 5.350402387287603],
    },
];

fn moment_array(value: &math::johnson_moments::PopulationMoments) -> [f64; 4] {
    [value.mean, value.std_dev, value.skewness, value.kurtosis]
}

#[test]
fn johnson_population_matches_independent_quadrature_including_near_normal_shapes() -> Result<(), Box<dyn Error>> {
    for fixture in FIXTURES {
        let distribution = if fixture.su {
            Distribution::JohnsonSu { gamma: fixture.gamma, delta: fixture.delta, location: fixture.location, scale: fixture.scale }
        } else {
            Distribution::JohnsonSb { gamma: fixture.gamma, delta: fixture.delta, location: fixture.location, scale: fixture.scale }
        };
        let actual = moment_array(&math::johnson_moments::population_moments(&distribution)?);
        for (index, (&value, &expected)) in actual.iter().zip(&fixture.moments).enumerate() {
            let tolerance = if index == 0 { 8.0 * f64::EPSILON * expected.abs() + 1e-10 * fixture.moments[1] } else { 3e-8 * (1.0 + expected.abs()) };
            assert!(
                (value - expected).abs() <= tolerance,
                "SU={} gamma={} delta={} moment{index}: {value} versus independent {expected}",
                fixture.su,
                fixture.gamma,
                fixture.delta
            );
        }
    }
    Ok(())
}

#[test]
fn johnson_inverse_preserves_independent_quantiles_after_actual_parameter_rounding() -> Result<(), Box<dyn Error>> {
    for fixture in FIXTURES {
        let [mean, std_dev, skewness, kurtosis] = fixture.moments;
        let fit = math::johnson_moments::fit(mean, std_dev, skewness, kurtosis)?;
        assert_eq!(fit.family, if fixture.su { math::johnson_moments::JohnsonFamily::Su } else { math::johnson_moments::JohnsonFamily::Sb });
        for (p, expected) in [0.15865525393145707, 0.5, 0.8413447460685429].into_iter().zip(fixture.quantiles) {
            let actual = math::quantile(&fit.distribution, p)?;
            // Scale by population spread, not the possibly huge affine mean.
            let tolerance = std_dev * 8e-5 + 8.0 * f64::EPSILON * expected.abs();
            assert!(
                (actual - expected).abs() <= tolerance,
                "SU={} gamma={} delta={} p={p}: {actual} versus independent {expected}; returned {:?}",
                fixture.su,
                fixture.gamma,
                fixture.delta,
                fit.distribution
            );
        }
        let actual = moment_array(&math::johnson_moments::population_moments(&fit.distribution)?);
        for (&value, &expected) in actual.iter().zip(&fixture.moments) {
            assert!((value - expected).abs() <= 1e-6 * (1.0 + expected.abs()));
        }
    }
    Ok(())
}
