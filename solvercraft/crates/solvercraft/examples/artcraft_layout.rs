//! A second, executable host example using object parameters, without a workbook.
//! This is a reuse example, not a claim of a shipped Designcraft integration.
use solvercraft::{
    project::{ModelDefinition, ModelLibrary, ParameterBinding, PortableModel, export, import},
    types::*,
};
fn run() -> Result<()> {
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "Two text-frame widths".into(),
        variables: vec![
            Variable { name: "headline width".into(), lower: Some(20.0), upper: Some(80.0), kind: VariableKind::Continuous },
            Variable { name: "body width".into(), lower: Some(40.0), upper: Some(100.0), kind: VariableKind::Continuous },
        ],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: 2.0 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            quadratic: vec![],
        },
        constraints: vec![LinearConstraint {
            name: "available line width".into(),
            terms: vec![LinearTerm { variable: 0, coefficient: 1.0 }, LinearTerm { variable: 1, coefficient: 1.0 }],
            relation: Relation::GreaterEqual,
            rhs: 120.0,
        }],
    };
    let mut library = ModelLibrary::default();
    library.insert(PortableModel {
        schema_version: 2,
        id: "layout:1".into(),
        revision: "0".into(),
        name: model.name.clone(),
        definition: ModelDefinition::Algebraic(model),
        options: SolveOptions::default(),
        bindings: vec![
            ParameterBinding { parameter: 0, object_id: "headline".into(), property: "width".into() },
            ParameterBinding { parameter: 1, object_id: "body".into(), property: "width".into() },
        ],
        metadata: Default::default(),
    })?;
    let restored = import(&export(&library)?)?;
    let saved = restored.models.first().ok_or_else(|| SolverError::new("example", "Missing imported model"))?;
    let ModelDefinition::Algebraic(model) = &saved.definition else {
        return Err(SolverError::new("example", "Unexpected model kind"));
    };
    let result = solvercraft::solve(model, &saved.options, &NoControl)?;
    if result.objective != Some(140.0) {
        return Err(SolverError::new("example", "Independent layout optimum is 140 at widths (20,100)"));
    }
    println!("{}", serde_json::json!({"host":"ArtCraft parameter-binding reuse example","bindings":saved.bindings,"result":result}));
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
