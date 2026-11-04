#![allow(clippy::expect_used)]
use solvercraft::{
    compile::{Expr, ExpressionModel},
    nonlinear::{DerivativeMode, NlpOptions},
    project::{ModelDefinition, PortableModel},
    runtime::{OwnedDeadlineControl, RuntimeLimits},
    types::*,
    workflow::{ExecutionContext, ExecutionRequest, Goal},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn quadratic(kind: VariableKind) -> OptimizationModel {
    OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: "owned auto quadratic".into(),
        variables: vec![Variable { name: "x".into(), kind, lower: Some(0.0), upper: Some(4.0) }],
        objective: Objective {
            direction: Direction::Minimize,
            offset: 0.0,
            linear: vec![LinearTerm { variable: 0, coefficient: -2.6 }],
            quadratic: vec![QuadraticTerm { left: 0, right: 0, coefficient: 1.0 }],
        },
        constraints: vec![],
    }
}
#[derive(Default)]
struct Capture {
    cancelled: AtomicBool,
    events: Mutex<Vec<ProgressEvent>>,
}
impl RunControl for Capture {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    fn progress(&self, event: &ProgressEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
    }
}
fn request(definition: ModelDefinition) -> ExecutionRequest {
    ExecutionRequest {
        schema_version: 2,
        model: PortableModel {
            schema_version: 2,
            id: "9007199254740993".into(),
            revision: u64::MAX.to_string(),
            name: "owned workflow".into(),
            definition,
            options: SolveOptions { seed: u64::MAX, ..Default::default() },
            bindings: vec![],
            metadata: Default::default(),
        },
        context: ExecutionContext {
            run_id: "owned-run".into(),
            snapshot_id: "owned-snapshot".into(),
            snapshot_revision: Some("9007199254740995".into()),
            host: "owned-control-test".into(),
            source_revision: Some("fixture-source".into()),
        },
        goal: None,
        limits: RuntimeLimits::default(),
        nonlinear: NlpOptions::default(),
    }
}
#[test]
fn owned_runtime_auto_honors_integer_quadratic_and_forwards_actual_progress() -> Result<()> {
    let capture = Arc::new(Capture::default());
    let callback: Arc<dyn RunControl> = capture.clone();
    let integer = solvercraft::runtime::solve_with_limits_owned(
        &quadratic(VariableKind::Integer),
        &SolveOptions::default(),
        RuntimeLimits::default(),
        callback,
    )?;
    assert_eq!(integer.values, Some(vec![1.0]));
    assert_eq!(integer.optimality, Optimality::Optimal, "{integer:?}");
    assert!(capture.events.lock().is_ok_and(|events| events.iter().any(|e| e.stage == "conic_branch_and_bound")), "Owned callbacks disappeared");
    let continuous = solvercraft::runtime::solve_with_limits_owned(
        &quadratic(VariableKind::Continuous),
        &SolveOptions::default(),
        RuntimeLimits::default(),
        Arc::new(NoControl),
    )?;
    assert!(continuous.values.as_ref().is_some_and(|x| (x[0] - 1.3).abs() < 1e-7));
    assert!(
        solvercraft::runtime::solve_with_limits_owned(
            &quadratic(VariableKind::Integer),
            &SolveOptions { method: SolveMethod::Simplex, ..Default::default() },
            RuntimeLimits::default(),
            Arc::new(NoControl)
        )
        .is_err_and(|e| e.code == "unsupported_method")
    );
    Ok(())
}
#[test]
fn zero_time_and_parent_cancel_keep_distinct_termination_causes() -> Result<()> {
    let limits = RuntimeLimits { max_time_millis: Some(0) };
    let timed =
        solvercraft::runtime::solve_with_limits_owned(&quadratic(VariableKind::Integer), &SolveOptions::default(), limits, Arc::new(NoControl))?;
    assert_eq!(timed.termination, Termination::TimeLimit);
    assert_eq!(timed.statistics.nodes, 0);
    assert_eq!(timed.values, None);
    let cancelled = Arc::new(Capture::default());
    cancelled.cancelled.store(true, Ordering::SeqCst);
    let stopped =
        solvercraft::runtime::solve_with_limits_owned(&quadratic(VariableKind::Integer), &SolveOptions::default(), limits, cancelled.clone())?;
    assert_eq!(stopped.termination, Termination::Cancelled);
    assert_eq!(stopped.statistics.nodes, 0);
    let deadline = OwnedDeadlineControl::new(cancelled, limits)?;
    assert!(deadline.is_cancelled());
    assert!(!deadline.timed_out());
    Ok(())
}
#[test]
fn owned_provenance_and_compiled_expression_reach_the_same_miqp() -> Result<()> {
    let model = quadratic(VariableKind::Integer);
    let options = SolveOptions { seed: u64::MAX, ..Default::default() };
    let run = solvercraft::provenance::solve_run_owned(&model, &options, Arc::new(NoControl))?;
    assert_eq!(run.result.values, Some(vec![1.0]), "{run:?}");
    assert_eq!(run.result.optimality, Optimality::Optimal);
    assert_eq!(run.manifest.options.seed, u64::MAX);
    assert_eq!(run.manifest.model_fingerprint, solvercraft::provenance::manifest(&model, &options)?.model_fingerprint);
    let expression = solvercraft::workflow::expression_from_algebraic(&model, &options)?;
    let compiled = solvercraft::workflow::solve_expression_owned(&expression, &options, &NlpOptions::default(), Arc::new(NoControl))?;
    assert_eq!(compiled.values, run.result.values);
    assert_eq!(compiled.optimality, Optimality::Optimal);
    assert!(
        solvercraft::workflow::solve_expression_owned(
            &expression,
            &SolveOptions { method: SolveMethod::Simplex, ..options },
            &NlpOptions::default(),
            Arc::new(NoControl)
        )
        .is_err_and(|e| e.code == "unsupported_method")
    );
    Ok(())
}
#[test]
fn owned_execution_preserves_context_goal_and_original_algebraic_checks() -> Result<()> {
    let mut input = request(ModelDefinition::Algebraic(quadratic(VariableKind::Integer)));
    input.goal = Some(Goal::Minimize {});
    let run = solvercraft::workflow::execute_owned(&input, Arc::new(NoControl))?;
    assert_eq!(run.result.values, Some(vec![1.0]));
    assert_eq!(run.result.optimality, Optimality::Optimal);
    assert!(run.original_objective.is_some_and(|v| (v + 1.6).abs() < 1e-8));
    let manifest = serde_json::to_value(&run.manifest).map_err(|e| SolverError::new("test", e.to_string()))?;
    assert_eq!(manifest["model_id"], "9007199254740993");
    assert_eq!(manifest["model_revision"], u64::MAX.to_string());
    assert_eq!(manifest["context"]["snapshot_revision"], "9007199254740995");
    assert_eq!(manifest["options"]["seed"], u64::MAX.to_string());
    assert_eq!(manifest["goal"]["kind"], "minimize");
    input.limits.max_time_millis = Some(0);
    let timed = solvercraft::workflow::execute_owned(&input, Arc::new(NoControl))?;
    assert_eq!(timed.result.termination, Termination::TimeLimit);
    assert_eq!(timed.result.statistics.nodes, 0);
    assert_eq!(timed.original_objective, None);
    input.context.run_id.clear();
    assert!(solvercraft::workflow::execute_owned(&input, Arc::new(NoControl)).is_err_and(|e| e.code == "invalid_context"));
    Ok(())
}
#[test]
fn owned_target_execution_reports_the_original_objective_and_reach_status() -> Result<()> {
    let mut model = quadratic(VariableKind::Integer);
    model.objective.linear[0].coefficient = 1.0;
    model.objective.quadratic.clear();
    let mut input = request(ModelDefinition::Algebraic(model));
    input.goal = Some(Goal::Target { value: 1.3 });
    let run = solvercraft::workflow::execute_owned(&input, Arc::new(NoControl))?;
    assert_eq!(run.result.values, None, "{run:?}");
    assert_eq!(run.result.feasibility, Feasibility::Infeasible);
    assert_eq!(run.target_reached, Some(false));
    input.goal = Some(Goal::Target { value: 2.0 });
    let reached = solvercraft::workflow::execute_owned(&input, Arc::new(NoControl))?;
    assert_eq!(reached.result.values, Some(vec![2.0]), "{reached:?}");
    assert_eq!(reached.original_objective, Some(2.0));
    assert_eq!(reached.result.objective, Some(2.0));
    assert_eq!(reached.target_reached, Some(true));
    Ok(())
}
#[test]
fn owned_nonlinear_path_preserves_fd_configuration_and_direction() -> Result<()> {
    let model = ExpressionModel {
        name: "owned FD exponential".into(),
        variables: vec![Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(-2.0), upper: Some(2.0) }],
        direction: Direction::Minimize,
        objective: Expr::Exp { value: Box::new(Expr::variable(0)) },
        constraints: vec![],
    };
    let mut input = request(ModelDefinition::Expression(model.clone()));
    input.goal = Some(Goal::Maximize {});
    input.nonlinear = NlpOptions {
        derivative_mode: DerivativeMode::FiniteDifference,
        finite_difference_step: 1e-5,
        max_inner_iterations: 91,
        ..Default::default()
    };
    let run = solvercraft::workflow::execute_owned(&input, Arc::new(NoControl))?;
    assert!(run.result.warnings.iter().any(|w| w.contains("finite-difference")), "{run:?}");
    assert!(run.original_objective.is_some_and(|v| (v - 2.0_f64.exp()).abs() < 1e-7));
    assert_eq!(run.manifest.nonlinear.derivative_mode, DerivativeMode::FiniteDifference);
    assert_eq!(run.manifest.nonlinear.finite_difference_step, 1e-5);
    assert_eq!(run.manifest.nonlinear.max_inner_iterations, 91);
    let direct = solvercraft::workflow::solve_expression_owned(&model, &input.model.options, &input.nonlinear, Arc::new(NoControl))?;
    let borrowed = solvercraft::workflow::solve_expression(&model, &input.model.options, &input.nonlinear, &NoControl)?;
    assert_eq!(direct.values, borrowed.values);
    assert_eq!(direct.statistics.evaluations, borrowed.statistics.evaluations);
    Ok(())
}
#[test]
fn owned_constrained_route_honors_configured_zero_outer_iterations() -> Result<()> {
    let model = ExpressionModel {
        name: "owned configured outer budget".into(),
        variables: vec![Variable { name: "x".into(), kind: VariableKind::Continuous, lower: Some(-2.0), upper: Some(2.0) }],
        direction: Direction::Minimize,
        objective: Expr::Exp { value: Box::new(Expr::variable(0)) },
        constraints: vec![solvercraft::compile::ExpressionConstraint {
            name: "nonlinear row".into(),
            left: Expr::Power { base: Box::new(Expr::variable(0)), exponent: 2 },
            relation: Relation::LessEqual,
            right: Expr::constant(1.0),
        }],
    };
    let nlp = NlpOptions {
        max_outer_iterations: 0,
        max_inner_iterations: 1,
        derivative_mode: DerivativeMode::FiniteDifference,
        finite_difference_step: 1e-5,
        ..Default::default()
    };
    let owned = solvercraft::workflow::solve_expression_owned(&model, &SolveOptions::default(), &nlp, Arc::new(NoControl))?;
    let borrowed = solvercraft::workflow::solve_expression(&model, &SolveOptions::default(), &nlp, &NoControl)?;
    assert_eq!(owned.termination, Termination::IterationLimit, "{owned:?}");
    assert_eq!(owned.statistics.iterations, 0);
    assert_eq!(owned.values, borrowed.values);
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_owned_jobs_keep_arc_callbacks_quota_and_default_miqp_route() -> Result<()> {
    use solvercraft::jobs;
    use std::sync::mpsc;
    use std::time::Duration;
    let (ready_tx, ready_rx) = mpsc::channel();
    let mut handles = Vec::new();
    let mut releases = Vec::new();
    for _ in 0..4 {
        let (release_tx, release_rx) = mpsc::channel();
        let ready = ready_tx.clone();
        releases.push(release_tx);
        handles.push(jobs::start_shared_job(move |control| {
            control.progress(&ProgressEvent { stage: "owned_job_barrier".into(), iterations: 1, evaluations: 0, nodes: 0, best_objective: None });
            ready.send(()).map_err(|e| SolverError::new("test", e.to_string()))?;
            release_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| SolverError::new("test", e.to_string()))?;
            Ok(control.is_cancelled())
        })?);
    }
    for _ in 0..4 {
        ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| SolverError::new("test", e.to_string()))?;
    }
    assert!(jobs::start_solve(quadratic(VariableKind::Integer), SolveOptions::default()).is_err_and(|e| e.code == "job_limit"));
    handles[0].cancel();
    for release in releases {
        release.send(()).map_err(|e| SolverError::new("test", e.to_string()))?;
    }
    for (i, handle) in handles.iter().enumerate() {
        let started = solvercraft::runtime::monotonic_now();
        loop {
            let snapshot = handle.snapshot()?;
            if let Some(result) = snapshot.result {
                let value = result?;
                assert_eq!(*value, i == 0);
                let again = handle.snapshot()?.result.expect("stored result")?;
                assert!(Arc::ptr_eq(&value, &again));
                assert!(snapshot.progress.is_some());
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let solved = jobs::start_solve(quadratic(VariableKind::Integer), SolveOptions::default())?;
    let started = solvercraft::runtime::monotonic_now();
    loop {
        let snapshot = solved.snapshot()?;
        if let Some(result) = snapshot.result {
            let result = result?;
            assert_eq!(result.values, Some(vec![1.0]));
            assert_eq!(result.optimality, Optimality::Optimal);
            assert!(snapshot.progress.is_some());
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
