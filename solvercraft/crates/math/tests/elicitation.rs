#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_math::{PreparedDistribution, elicitation::*, nominal::nominal_value};
use solvercraft_types::Distribution as D;
#[test]
fn myerson_preserves_named_percentiles_for_both_skew_directions() {
    for (bottom, median, top) in [(1.0, 2.0, 4.0), (0.0, 2.0, 3.0), (-1.0, 0.0, 1.0)] {
        let d = myerson_percentiles(bottom, median, top, 0.5).unwrap();
        let p = PreparedDistribution::new(&d).unwrap();
        for (q, x) in [(0.25, bottom), (0.5, median), (0.75, top)] {
            assert!((p.quantile(q).unwrap() - x).abs() < 1e-12);
        }
    }
    assert!(myerson_percentiles(2.0, 1.0, 3.0, 0.5).is_err());
    assert!(myerson_percentiles(1.0, 2.0, 3.0, 0.0).is_err());
}
#[test]
fn generalized_triangle_recovers_independent_asymmetric_models_and_endpoints() {
    let truth = D::Triangular { low: -3.0, mode: 1.0, high: 9.0 };
    let base = PreparedDistribution::new(&truth).unwrap();
    for (p, r) in [(0.1, 0.8), (0.0, 0.8), (0.1, 1.0), (0.0, 1.0)] {
        let low = base.quantile(p).unwrap();
        let high = base.quantile(r).unwrap();
        let d = triangular_from_percentiles(low, 1.0, high, p, r).unwrap();
        let D::Triangular { low, mode, high } = d else {
            panic!("Expected a triangular model");
        };
        assert!((low + 3.0).abs() < 1e-11);
        assert_eq!(mode, 1.0);
        assert!((high - 9.0).abs() < 1e-11);
    }
    assert!(triangular_from_percentiles(1.0, 2.0, 3.0, 0.8, 0.2).is_err());
}
#[test]
fn vary_actual_bounds_uses_the_declared_base_as_mode() {
    let d = vary_actual_bounds(100.0, 95.0, 110.0, VaryFamily::Triangular).unwrap();
    assert!(matches!(d, D::Triangular { low: 95.0, mode: 100.0, high: 110.0 }));
    assert_eq!(nominal_value(&d).unwrap(), (95.0 + 100.0 + 110.0) / 3.0);
    assert!(vary_actual_bounds(100.0, 101.0, 110.0, VaryFamily::Uniform).is_err());
}
