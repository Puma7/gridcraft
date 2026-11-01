use super::{Expr, ExpressionConstraint, ExpressionModel, MAX_EXPRESSION_NODES, error, preflight};
use solvercraft_types::{Relation, Result, SolveOptions};
const TARGET_ROW_NAME: &str = "Target objective value";

/// Preserve the objective and add an equality requiring its original value to
/// equal `target`. A target is a feasibility condition, not a squared surrogate.
pub fn with_target(model: &ExpressionModel, target: f64) -> Result<ExpressionModel> {
    with_target_with_options(model, target, &SolveOptions::default())
}

/// Borrowed preflight for the default target row. No model fields are cloned.
pub fn preflight_target(model: &ExpressionModel, target: f64, options: &SolveOptions) -> Result<()> {
    preflight_target_named(model, target, TARGET_ROW_NAME, options)
}

/// Borrowed preflight for a host's existing equality-row label. It checks the
/// original AST plus one copied objective, one target constant and the supplied
/// name using the same expanded model memory contract, before any field clone.
pub fn preflight_target_named(model: &ExpressionModel, target: f64, row_name: &str, options: &SolveOptions) -> Result<()> {
    if row_name.len() > solvercraft_model::MAX_NAME_BYTES {
        return Err(error("model_limit", "target constraint name is too long"));
    }
    if !target.is_finite() {
        return Err(error("non_finite", "objective target must be finite"));
    }
    super::validate_expression_model(model, options)?;
    if model.variables.len() > solvercraft_model::MAX_VARIABLES || model.constraints.len() >= solvercraft_model::MAX_CONSTRAINTS {
        return Err(error("model_limit", "target model has too many variables or constraints"));
    }
    let mut remaining = MAX_EXPRESSION_NODES;
    preflight(&model.objective, model.variables.len(), &mut remaining)?;
    preflight(&model.objective, model.variables.len(), &mut remaining)?;
    for row in &model.constraints {
        preflight(&row.left, model.variables.len(), &mut remaining)?;
        preflight(&row.right, model.variables.len(), &mut remaining)?;
    }
    if remaining == 0 {
        return Err(error("expression_limit", "target constant exceeds expression node budget"));
    }
    let nodes = MAX_EXPRESSION_NODES - remaining + 1;
    let row_count = model.constraints.len().checked_add(1).ok_or_else(|| error("memory_limit", "target constraint count overflow"))?;
    let mut metadata = model.name.len().checked_add(row_name.len()).ok_or_else(|| error("memory_limit", "target metadata overflow"))?;
    for variable in &model.variables {
        metadata = metadata.checked_add(variable.name.len()).ok_or_else(|| error("memory_limit", "target metadata overflow"))?;
    }
    for row in &model.constraints {
        metadata = metadata.checked_add(row.name.len()).ok_or_else(|| error("memory_limit", "target metadata overflow"))?;
    }
    let expanded = nodes
        .checked_mul(160)
        .and_then(|n| n.checked_add(metadata))
        .and_then(|n| model.variables.len().checked_mul(256).and_then(|v| n.checked_add(v)))
        .and_then(|n| row_count.checked_mul(128).and_then(|v| n.checked_add(v)))
        .ok_or_else(|| error("memory_limit", "target expression memory estimate overflow"))?;
    if u64::try_from(expanded).map_or(true, |bytes| bytes > options.max_memory_bytes) {
        return Err(error("memory_limit", "expanded target expression exceeds the caller memory budget"));
    }
    Ok(())
}

/// Add an original-objective target equality, checking the caller's actual
/// budget against the expanded AST/metadata before cloning any model fields.
/// The estimate is the same contract as `validate_expression_model`, not an RSS
/// measurement. Existing caller-retained storage can be reserved separately.
pub fn with_target_with_options(model: &ExpressionModel, target: f64, options: &SolveOptions) -> Result<ExpressionModel> {
    preflight_target(model, target, options)?;
    let row_count = model.constraints.len().checked_add(1).ok_or_else(|| error("memory_limit", "target constraint count overflow"))?;
    let mut constraints = Vec::new();
    constraints.try_reserve_exact(row_count).map_err(|_| error("memory_limit", "cannot allocate bounded target constraints"))?;
    constraints.extend(model.constraints.iter().cloned());
    constraints.push(ExpressionConstraint {
        name: TARGET_ROW_NAME.into(),
        left: model.objective.clone(),
        relation: Relation::Equal,
        right: Expr::constant(target),
    });
    Ok(ExpressionModel {
        name: model.name.clone(),
        variables: model.variables.clone(),
        direction: model.direction,
        objective: model.objective.clone(),
        constraints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use solvercraft_types::{Direction, SolveOptions, Variable, VariableKind};
    #[test]
    fn target_preserves_objective_and_enforces_original_units() {
        let model = ExpressionModel {
            name: "target".into(),
            variables: vec![Variable { name: "x".into(), lower: None, upper: None, kind: VariableKind::Continuous }],
            direction: Direction::Maximize,
            objective: Expr::variable(0),
            constraints: vec![],
        };
        let targeted = with_target(&model, 4.0).unwrap();
        assert_eq!(targeted.objective, model.objective);
        assert_eq!(targeted.direction, Direction::Maximize);
        let compiled = super::super::compile(&targeted, &SolveOptions::default()).unwrap();
        assert!(solvercraft_model::validate_solution(&compiled, &[4.0], &SolveOptions::default()).unwrap().feasible);
        assert!(!solvercraft_model::validate_solution(&compiled, &[0.0], &SolveOptions::default()).unwrap().feasible);
        assert_eq!(with_target(&model, f64::NAN).unwrap_err().code, "non_finite");
    }
    #[test]
    fn target_expansion_honors_caller_memory_before_cloning() {
        let model = ExpressionModel {
            name: "target".into(),
            variables: vec![Variable { name: "x".into(), lower: None, upper: None, kind: VariableKind::Continuous }],
            direction: Direction::Maximize,
            objective: Expr::variable(0),
            constraints: vec![],
        };
        let small = SolveOptions { max_memory_bytes: 600, ..SolveOptions::default() };
        super::super::validate_expression_model(&model, &small).unwrap();
        assert_eq!(with_target_with_options(&model, 4.0, &small).unwrap_err().code, "memory_limit");
        let exact = 3 * 160 + model.name.len() + 1 + TARGET_ROW_NAME.len() + 256 + 128;
        let enough = SolveOptions { max_memory_bytes: exact as u64, ..SolveOptions::default() };
        let transformed = with_target_with_options(&model, 4.0, &enough).unwrap();
        super::super::validate_expression_model(&transformed, &enough).unwrap();
        assert_eq!(transformed.objective, model.objective);
        assert_eq!(transformed.constraints[0].right, Expr::constant(4.0));
        assert_eq!(
            with_target_with_options(&model, 4.0, &SolveOptions { max_memory_bytes: exact as u64 - 1, ..enough }).unwrap_err().code,
            "memory_limit"
        );
        assert!(model.constraints.is_empty());
    }
    #[test]
    fn target_expansion_shares_one_total_node_limit_before_cloning() {
        let mut expressions = vec![Expr::constant(1.0); 26_000];
        while expressions.len() > 1 {
            let mut next = Vec::new();
            let mut source = expressions.into_iter();
            while let Some(left) = source.next() {
                next.push(if let Some(right) = source.next() { Expr::Add { left: Box::new(left), right: Box::new(right) } } else { left });
            }
            expressions = next;
        }
        let objective = expressions.pop().unwrap();
        let model = ExpressionModel { name: "nodes".into(), variables: vec![], direction: Direction::Minimize, objective, constraints: vec![] };
        super::super::validate_expression_model(&model, &SolveOptions::default()).unwrap();
        assert_eq!(with_target_with_options(&model, 1.0, &SolveOptions::default()).unwrap_err().code, "expression_limit");
    }
    #[test]
    fn borrowed_target_preflight_accounts_for_the_actual_host_row_name() {
        let model = ExpressionModel {
            name: "host".into(),
            variables: vec![],
            direction: Direction::Minimize,
            objective: Expr::constant(1.0),
            constraints: vec![],
        };
        let label = "Target value";
        let bytes = 3 * 160 + model.name.len() + label.len() + 128;
        let options = SolveOptions { max_memory_bytes: bytes as u64, ..SolveOptions::default() };
        preflight_target_named(&model, 2.0, label, &options).unwrap();
        assert_eq!(preflight_target(&model, 2.0, &options).unwrap_err().code, "memory_limit");
        assert_eq!(
            preflight_target_named(&model, 2.0, label, &SolveOptions { max_memory_bytes: bytes as u64 - 1, ..options }).unwrap_err().code,
            "memory_limit"
        );
        assert_eq!(
            preflight_target_named(&model, 2.0, &"x".repeat(solvercraft_model::MAX_NAME_BYTES + 1), &SolveOptions::default()).unwrap_err().code,
            "model_limit"
        );
    }
}
