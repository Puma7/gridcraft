#![allow(clippy::unwrap_used, clippy::panic)]
use solvercraft_math::copula::DependenceSpec;
use solvercraft_math::{PreparedDistribution, validate_distribution};
use solvercraft_types::Distribution as D;

#[test]
fn floating_support_boundaries_keep_their_rust_ieee_bits_through_json() {
    for literal in ["5e-324", "2.225073858507201e-308", "0.9999999999999999", "1.0000000000000002", "1.7976931348623157e308"] {
        let reference = literal.parse::<f64>().unwrap();
        let json = format!("{{\"kind\":\"constant\",\"value\":{literal}}}");
        let d: D = serde_json::from_str(&json).unwrap();
        let D::Constant { value } = d else { panic!("Expected Constant") };
        assert_eq!(value.to_bits(), reference.to_bits(), "literal{literal}");
        let serialized = serde_json::to_string(&D::Constant { value }).unwrap();
        let back: D = serde_json::from_str(&serialized).unwrap();
        assert_eq!(PreparedDistribution::new(&back).unwrap().quantile(0.5).unwrap().to_bits(), reference.to_bits());
    }
    let d: D = serde_json::from_str(r#"{"kind":"discrete_uniform","low":9007199254740993,"high":9007199254740993}"#).unwrap();
    assert!(matches!(d, D::DiscreteUniform { low: 9007199254740993, high: 9007199254740993 }));
    assert!(validate_distribution(&d).is_err(), "large integer was preserved, then refused rather than silently rounded");
}

#[test]
fn nested_distribution_replay_preserves_parameters_and_strict_fields() {
    let d = D::ShiftScale {
        base: Box::new(D::Beta { alpha: 1.0_f64.next_up(), beta: 0.1_f64.next_up() }),
        shift: -1.0_f64.next_up(),
        scale: 3.0_f64.next_down(),
    };
    let serialized = serde_json::to_string(&d).unwrap();
    let back: D = serde_json::from_str(&serialized).unwrap();
    let original = PreparedDistribution::new(&d).unwrap();
    let restored = PreparedDistribution::new(&back).unwrap();
    for trial in 0..32 {
        assert_eq!(original.sample(u64::MAX, 7, trial).unwrap().to_bits(), restored.sample(u64::MAX, 7, trial).unwrap().to_bits());
    }
    assert!(serde_json::from_str::<D>(r#"{"kind":"constant","value":1,"ignored":true}"#).is_err());
    assert!(serde_json::from_str::<DependenceSpec>(r#"{"kind":"independent","correlation":[[1.0]]}"#).is_err());
    assert!(serde_json::from_str::<D>(r#"{"kind":"normal","mean":0,"std_dev":null}"#).is_err());
}
