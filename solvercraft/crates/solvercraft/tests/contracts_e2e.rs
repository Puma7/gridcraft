#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use solvercraft::{
    project::*,
    registry::{self, ProblemClass},
    runtime::{self, RuntimeLimits},
    types::*,
};
use std::collections::BTreeMap;
fn model() -> OptimizationModel {
    OptimizationModel {
        schema_version: 1,
        name: "planning".into(),
        variables: vec![Variable { name: "width".into(), lower: Some(0.0), upper: Some(10.0), kind: VariableKind::Continuous }],
        objective: Objective {
            direction: Direction::Maximize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![],
    }
}
fn portable() -> PortableModel {
    PortableModel {
        schema_version: 2,
        id: "18446744073709551615".into(),
        revision: "9007199254740993".into(),
        name: "ArtCraft layout".into(),
        definition: ModelDefinition::Algebraic(model()),
        options: SolveOptions { seed: u64::MAX, ..Default::default() },
        bindings: vec![ParameterBinding { parameter: 0, object_id: "frame:18446744073709551615".into(), property: "width".into() }],
        metadata: BTreeMap::new(),
    }
}
#[test]
fn portable_json_preserves_large_string_ids_revisions_and_maximum_seed() {
    let mut library = ModelLibrary::default();
    library.insert(portable()).unwrap();
    let bytes = export(&library).unwrap();
    let restored = import(&bytes).unwrap();
    assert_eq!(restored.models[0].id, "18446744073709551615");
    assert_eq!(restored.models[0].revision, "9007199254740993");
    assert_eq!(restored.models[0].options.seed, u64::MAX);
    assert_eq!(restored.models[0].bindings, library.models[0].bindings);
    let restored_model = match &restored.models[0].definition {
        ModelDefinition::Algebraic(m) => m,
        _ => panic!("wrong variant"),
    };
    let result = solvercraft::solve(restored_model, &restored.models[0].options, &NoControl).unwrap();
    assert_eq!(result.objective, Some(10.0));
}
#[test]
fn saved_model_updates_are_atomic_revision_checked_and_collision_safe() {
    let mut library = ModelLibrary::default();
    let original = portable();
    library.insert(original.clone()).unwrap();
    assert!(library.insert(original.clone()).is_err());
    let mut collision = original.clone();
    collision.id = "second".into();
    collision.name = "ARTCRAFT LAYOUT".into();
    assert!(library.insert(collision).is_err());
    let before = export(&library).unwrap();
    assert!(library.replace(&original.id, "0", original.clone()).is_err());
    assert_eq!(export(&library).unwrap(), before);
    library.replace(&original.id, &original.revision, original.clone()).unwrap();
    assert_eq!(library.models[0].revision, "9007199254740994");
    assert!(library.remove(&original.id, &original.revision).is_err());
    library.remove(&original.id, "9007199254740994").unwrap();
    assert!(library.models.is_empty());
}
#[test]
fn project_schema_and_binding_failures_are_not_silently_migrated() {
    for bytes in [
        br#"[]"#.as_slice(),
        br#"{"schema_version":999,"models":[]}"#,
        br#"{"schema_version":2,"models":[],"typo":1}"#,
        br#"{"schema_version":2,"schema_version":2,"models":[]}"#,
    ] {
        assert!(import(bytes).is_err());
    }
    let mut p = portable();
    p.bindings[0].parameter = 1;
    assert!(p.validate().is_err());
    p.bindings[0].parameter = 0;
    p.bindings.push(p.bindings[0].clone());
    assert!(p.validate().is_err());
    let original = model();
    let migrated =
        migrate_v1_algebraic(&serde_json::to_vec(&original).unwrap(), "old-model".into(), SolveOptions { seed: u64::MAX, ..Default::default() })
            .unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.revision, "0");
    assert_eq!(migrated.options.seed, u64::MAX);
    assert!(migrated.bindings.is_empty());
    let mut invalid = original;
    invalid.schema_version = 0;
    assert!(migrate_v1_algebraic(&serde_json::to_vec(&invalid).unwrap(), "id".into(), SolveOptions::default()).is_err());
}
#[test]
fn backend_registry_rejects_unavailable_method_substitution() {
    let simplex = registry::select(ProblemClass::Linear, None).unwrap();
    assert_eq!(simplex.id, "dense_two_phase_simplex");
    assert!(!simplex.basis);
    assert!(!simplex.dual_values);
    assert!(registry::select(ProblemClass::Linear, Some("projected_gradient_ad")).is_err());
    assert!(registry::select(ProblemClass::Linear, Some("grg")).is_err());
    for engine in registry::engines() {
        assert!(engine.progress && engine.cancellation);
        assert!(!engine.guarantee.is_empty());
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn expired_wall_clock_is_time_limit_and_user_cancel_remains_distinct() {
    let result = runtime::solve_with_limits(&model(), &SolveOptions::default(), RuntimeLimits { max_time_millis: Some(0) }, &NoControl).unwrap();
    assert_eq!(result.termination, Termination::TimeLimit);
    assert_ne!(result.optimality, Optimality::Optimal);
    struct Cancel;
    impl RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let result = runtime::solve_with_limits(&model(), &SolveOptions::default(), RuntimeLimits { max_time_millis: Some(0) }, &Cancel).unwrap();
    assert_eq!(result.termination, Termination::Cancelled);
    let result = runtime::solve_with_limits(&model(), &SolveOptions::default(), RuntimeLimits::default(), &NoControl).unwrap();
    assert_eq!(result.termination, Termination::Completed);
    assert_eq!(result.objective, Some(10.0));
}
