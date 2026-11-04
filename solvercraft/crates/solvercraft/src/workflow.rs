//! One explicit SDK execution contract shared by non-workbook hosts and the CLI.
use crate::{
    compile::{self, Expr, ExpressionConstraint, ExpressionModel},
    nonlinear::NlpOptions,
    project::{ModelDefinition, PortableModel},
    runtime::{DeadlineControl, OwnedDeadlineControl, RuntimeLimits},
    types::*,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Goal {
    Minimize {},
    Maximize {},
    Target { value: f64 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContext {
    pub run_id: String,
    pub snapshot_id: String,
    pub snapshot_revision: Option<String>,
    pub host: String,
    /// Optional caller-supplied source identity; not a verified build attestation.
    pub source_revision: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequest {
    pub schema_version: u32,
    pub model: PortableModel,
    pub context: ExecutionContext,
    #[serde(default)]
    pub goal: Option<Goal>,
    #[serde(default)]
    pub limits: RuntimeLimits,
    #[serde(default)]
    pub nonlinear: NlpOptions,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionManifest {
    pub schema_version: u32,
    pub model_id: String,
    pub model_revision: String,
    pub context: ExecutionContext,
    pub input_fingerprint: String,
    pub fingerprint_algorithm: String,
    pub solvercraft_version: String,
    pub engine: String,
    pub options: SolveOptions,
    pub runtime_limits: RuntimeLimits,
    pub goal: Option<Goal>,
    pub nonlinear: NlpOptions,
    pub platform: String,
    pub sampling_version: String,
    pub elapsed_millis: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRun {
    pub manifest: ExecutionManifest,
    pub result: SolveResult,
    pub original_objective: Option<f64>,
    pub target_reached: Option<bool>,
}
fn error(code: &str, message: &str) -> SolverError {
    SolverError::new(code, message)
}
fn sum(mut values: Vec<Expr>) -> Expr {
    while values.len() > 1 {
        let mut next = Vec::with_capacity(values.len().div_ceil(2));
        let mut iter = values.into_iter();
        while let Some(left) = iter.next() {
            next.push(if let Some(right) = iter.next() { Expr::Add { left: Box::new(left), right: Box::new(right) } } else { left });
        }
        values = next;
    }
    values.pop().unwrap_or_else(|| Expr::constant(0.0))
}
fn row_terms(terms: &[LinearTerm]) -> Expr {
    sum(terms.iter().map(|t| Expr::Multiply { left: Box::new(Expr::constant(t.coefficient)), right: Box::new(Expr::variable(t.variable)) }).collect())
}
pub fn expression_from_algebraic(model: &OptimizationModel, options: &SolveOptions) -> Result<ExpressionModel> {
    crate::model::validate(model, options)?;
    let terms = model.constraints.iter().try_fold(model.objective.linear.len(), |n, r| {
        n.checked_add(r.terms.len()).ok_or_else(|| error("expression_limit", "Expression conversion size overflow"))
    })?;
    let estimate = terms
        .checked_mul(4)
        .and_then(|n| n.checked_add(model.objective.quadratic.len().checked_mul(6)?))
        .and_then(|n| n.checked_add(model.constraints.len().checked_mul(3)?))
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| error("expression_limit", "Expression conversion size overflow"))?;
    if estimate > compile::MAX_EXPRESSION_NODES || (estimate as u64).checked_mul(256).is_none_or(|n| n > options.max_memory_bytes) {
        return Err(error("memory_limit", "Algebraic-to-expression conversion exceeds its node/memory budget"));
    }
    let mut objective = vec![Expr::constant(model.objective.offset), row_terms(&model.objective.linear)];
    for term in &model.objective.quadratic {
        objective.push(Expr::Multiply {
            left: Box::new(Expr::constant(term.coefficient)),
            right: Box::new(Expr::Multiply { left: Box::new(Expr::variable(term.left)), right: Box::new(Expr::variable(term.right)) }),
        });
    }
    let converted = ExpressionModel {
        name: model.name.clone(),
        variables: model.variables.clone(),
        direction: model.objective.direction,
        objective: sum(objective),
        constraints: model
            .constraints
            .iter()
            .map(|r| ExpressionConstraint { name: r.name.clone(), left: row_terms(&r.terms), relation: r.relation, right: Expr::constant(r.rhs) })
            .collect(),
    };
    compile::validate_expression_model(&converted, options)?;
    Ok(converted)
}
pub fn validate_original_expression(model: &ExpressionModel, options: &SolveOptions, result: &mut SolveResult) -> Result<()> {
    let Some(values) = result.values.as_deref() else {
        return Ok(());
    };
    compile::validate_expression_model(model, options)?;
    let bound_model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: model.name.clone(),
        variables: model.variables.clone(),
        objective: Objective { direction: Direction::Minimize, offset: 0.0, linear: vec![], quadratic: vec![] },
        constraints: vec![],
    };
    let checked = crate::model::validate_solution(&bound_model, values, options)?;
    if !checked.feasible {
        return Err(error("numerical_failure", "Candidate violates original variable bounds or integrality"));
    }
    let objective = compile::evaluate(&model.objective, values)?;
    let reported = result.objective.ok_or_else(|| error("numerical_failure", "Candidate has no reported objective"))?;
    let scale = objective.abs().max(reported.abs()).max(1.0);
    if !reported.is_finite() || (objective / scale - reported / scale).abs() > options.optimality_tolerance {
        return Err(error("numerical_failure", "Compiled and original expression objectives disagree"));
    }
    let mut violation = checked.max_violation;
    for row in &model.constraints {
        let residual = compile::evaluate(&row.left, values)? - compile::evaluate(&row.right, values)?;
        if !residual.is_finite() {
            return Err(error("numerical_failure", "Original constraint residual overflowed"));
        }
        let v = match row.relation {
            Relation::LessEqual => residual.max(0.0),
            Relation::Equal => residual.abs(),
            Relation::GreaterEqual => (-residual).max(0.0),
        };
        if v > options.feasibility_tolerance {
            return Err(error("numerical_failure", "Candidate violates an original expression constraint"));
        }
        violation = violation.max(v);
    }
    result.objective = Some(objective);
    result.max_violation = Some(violation);
    Ok(())
}
/// Borrowed compatibility path. Compiled integer QP uses solve_expression_owned
/// so the underlying engine can retain cancellation/progress callbacks safely.
pub fn solve_expression(model: &ExpressionModel, options: &SolveOptions, nlp: &NlpOptions, control: &dyn RunControl) -> Result<SolveResult> {
    solve_expression_impl(model, options, nlp, control, |compiled| crate::solve(compiled, options, control))
}
/// Owned-control variant preserves the same method/configuration/original checks.
pub fn solve_expression_owned(
    model: &ExpressionModel,
    options: &SolveOptions,
    nlp: &NlpOptions,
    control: Arc<dyn RunControl>,
) -> Result<SolveResult> {
    let engine_control = Arc::clone(&control);
    solve_expression_impl(model, options, nlp, control.as_ref(), |compiled| solvercraft_engines::solve_owned(compiled, options, engine_control))
}
fn solve_expression_impl<F>(
    model: &ExpressionModel,
    options: &SolveOptions,
    nlp: &NlpOptions,
    control: &dyn RunControl,
    solve_compiled: F,
) -> Result<SolveResult>
where
    F: FnOnce(&OptimizationModel) -> Result<SolveResult>,
{
    compile::validate_expression_model(model, options)?;
    nlp.validate()?;
    let mut result = match options.method {
        SolveMethod::DifferentialEvolution => crate::heuristic::solve(model, options, control)?,
        SolveMethod::ProjectedGradient => {
            if model.constraints.is_empty() {
                bound_only(model, options, nlp, control)?
            } else {
                crate::constrained::solve(model, options, nlp, control)?
            }
        }
        SolveMethod::Simplex | SolveMethod::BranchAndBound => solve_compiled(&compile::compile(model, options)?)?,
        SolveMethod::Auto => {
            let objective = compile::classify(&model.objective, model.variables.len())?;
            let mut affine_rows = true;
            for row in &model.constraints {
                for expression in [&row.left, &row.right] {
                    if !matches!(
                        compile::classify(expression, model.variables.len())?,
                        compile::ExpressionClass::Affine | compile::ExpressionClass::Constant
                    ) {
                        affine_rows = false;
                    }
                }
            }
            if objective != compile::ExpressionClass::Nonlinear && affine_rows {
                solve_compiled(&compile::compile(model, options)?)?
            } else if model.variables.iter().any(|v| v.kind != VariableKind::Continuous) {
                crate::heuristic::solve(model, options, control)?
            } else if model.constraints.is_empty() {
                bound_only(model, options, nlp, control)?
            } else {
                crate::constrained::solve(model, options, nlp, control)?
            }
        }
    };
    validate_original_expression(model, options, &mut result)?;
    Ok(result)
}
fn bound_only(model: &ExpressionModel, options: &SolveOptions, nlp: &NlpOptions, control: &dyn RunControl) -> Result<SolveResult> {
    let mut start = Vec::new();
    start.try_reserve(model.variables.len()).map_err(|_| error("memory_limit", "Cannot reserve the nonlinear start"))?;
    start.resize(model.variables.len(), 0.0);
    crate::nonlinear::solve_from_with_derivatives(model, &start, options, control, nlp.derivative_mode, nlp.finite_difference_step)
}
fn fingerprint<T: Serialize>(value: &T, limit: u64) -> Result<String> {
    struct Hash {
        value: u64,
        bytes: u64,
        limit: u64,
    }
    impl std::io::Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.limit)
                .ok_or_else(|| std::io::Error::other("Manifest byte budget"))?;
            for b in bytes {
                self.value ^= u64::from(*b);
                self.value = self.value.wrapping_mul(0x100000001b3);
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hash = Hash { value: 0xcbf29ce484222325, bytes: 0, limit };
    serde_json::to_writer(&mut hash, value).map_err(|e| SolverError::new("manifest_limit", e.to_string()))?;
    Ok(format!("{:016x}", hash.value))
}
/// Borrowed compatibility execution; execute_owned reaches retained-callback
/// engines while preserving this exact goal/context/configuration contract.
pub fn execute(request: &ExecutionRequest, control: &dyn RunControl) -> Result<ExecutionRun> {
    execute_impl(
        request,
        |limits| DeadlineControl::new(control, limits),
        |model, options, nlp, deadline| solve_expression(model, options, nlp, deadline),
        |deadline, result| deadline.apply_termination(result),
    )
}
/// Primary execution entry point for hosts owning callback/cancellation state.
pub fn execute_owned(request: &ExecutionRequest, control: Arc<dyn RunControl>) -> Result<ExecutionRun> {
    execute_impl(
        request,
        |limits| OwnedDeadlineControl::new(control, limits).map(Arc::new),
        |model, options, nlp, deadline| {
            let callback: Arc<dyn RunControl> = deadline.clone();
            solve_expression_owned(model, options, nlp, callback)
        },
        |deadline, result| deadline.apply_termination(result),
    )
}
fn execute_impl<C, Make, Solve, Finish>(request: &ExecutionRequest, make_deadline: Make, solve: Solve, finish: Finish) -> Result<ExecutionRun>
where
    Make: FnOnce(RuntimeLimits) -> Result<C>,
    Solve: FnOnce(&ExpressionModel, &SolveOptions, &NlpOptions, &C) -> Result<SolveResult>,
    Finish: FnOnce(&C, &mut SolveResult),
{
    request.model.validate()?;
    request.nonlinear.validate()?;
    if request.schema_version != 2
        || [&request.context.run_id, &request.context.snapshot_id, &request.context.host]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        || request.context.snapshot_revision.as_ref().is_some_and(|s| s.is_empty() || s.len() > 256)
        || request.context.source_revision.as_ref().is_some_and(|s| s.len() > 128 || s.is_empty())
    {
        return Err(error("invalid_context", "Expected schema 2 and bounded lossless run/snapshot/host identities"));
    }
    let fingerprint = fingerprint(request, request.model.options.max_memory_bytes)?;
    let started = crate::runtime::monotonic_now();
    let deadline = make_deadline(request.limits)?;
    let mut expression = match &request.model.definition {
        ModelDefinition::Expression(m) => m.clone(),
        ModelDefinition::Algebraic(m) => expression_from_algebraic(m, &request.model.options)?,
    };
    let original = expression.objective.clone();
    match &request.goal {
        Some(Goal::Minimize {}) => expression.direction = Direction::Minimize,
        Some(Goal::Maximize {}) => expression.direction = Direction::Maximize,
        Some(Goal::Target { value }) => expression = compile::with_target_with_options(&expression, *value, &request.model.options)?,
        None => {}
    }
    let mut result = solve(&expression, &request.model.options, &request.nonlinear, &deadline)?;
    finish(&deadline, &mut result);
    let mut original_objective = result.values.as_deref().map(|values| compile::evaluate(&original, values)).transpose()?;
    if let (ModelDefinition::Algebraic(source), Some(values)) = (&request.model.definition, result.values.as_deref()) {
        let checked = crate::model::validate_solution(source, values, &request.model.options)?;
        if !checked.feasible {
            return Err(error("numerical_failure", "Candidate violates the original algebraic model"));
        }
        let actual = crate::model::evaluate_objective(source, values)?;
        if !matches!(request.goal, Some(Goal::Target { .. })) {
            let reported = result.objective.ok_or_else(|| error("numerical_failure", "Candidate has no objective"))?;
            let scale = actual.abs().max(reported.abs()).max(1.0);
            if (actual / scale - reported / scale).abs() > request.model.options.optimality_tolerance {
                return Err(error("numerical_failure", "Expression conversion changed the original algebraic objective"));
            }
        }
        original_objective = Some(actual);
    }
    let target_reached = match (&request.goal, original_objective) {
        (Some(Goal::Target { value }), Some(objective)) => Some((objective - value).abs() <= request.model.options.feasibility_tolerance),
        (Some(Goal::Target { .. }), None) => Some(false),
        _ => None,
    };
    let manifest = ExecutionManifest {
        schema_version: 2,
        model_id: request.model.id.clone(),
        model_revision: request.model.revision.clone(),
        context: request.context.clone(),
        input_fingerprint: fingerprint,
        fingerprint_algorithm: "fnv1a64-diagnostic-v1-not-authentication".into(),
        solvercraft_version: env!("CARGO_PKG_VERSION").into(),
        engine: result.engine.clone(),
        options: request.model.options.clone(),
        runtime_limits: request.limits,
        goal: request.goal.clone(),
        nonlinear: request.nonlinear.clone(),
        platform: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        sampling_version: crate::math::SAMPLING_VERSION.into(),
        elapsed_millis: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    };
    Ok(ExecutionRun { manifest, result, original_objective, target_reached })
}
