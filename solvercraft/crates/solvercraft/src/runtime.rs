//! Portable cooperative wall-clock budgets. These cannot interrupt arbitrary user code.
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// web-time uses browser Performance on WASM. On native it aliases std's clock,
// so Clippy's DefId-based restriction needs this one portable-clock exception.
#[allow(clippy::disallowed_methods)]
pub fn monotonic_now() -> web_time::Instant {
    web_time::Instant::now()
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeLimits {
    /// None means no wall-clock deadline; zero means no work may begin.
    pub max_time_millis: Option<u64>,
}

pub struct DeadlineControl<'a> {
    parent: &'a dyn RunControl,
    deadline: Option<web_time::Instant>,
    expired: std::sync::atomic::AtomicBool,
}
fn make_deadline(limits: RuntimeLimits) -> Result<Option<web_time::Instant>> {
    limits
        .max_time_millis
        .map(|millis| {
            monotonic_now()
                .checked_add(std::time::Duration::from_millis(millis))
                .ok_or_else(|| SolverError::new("invalid_time_limit", "Wall-clock deadline exceeds the platform range"))
        })
        .transpose()
}
fn is_stopped(parent: &dyn RunControl, deadline: Option<web_time::Instant>, expired: &AtomicBool) -> bool {
    if parent.is_cancelled() {
        return true;
    }
    if deadline.is_some_and(|deadline| monotonic_now() >= deadline) {
        expired.store(true, Ordering::Release);
        return true;
    }
    false
}
fn apply_time_limit(expired: bool, termination: &mut Termination) {
    if expired && *termination == Termination::Cancelled {
        *termination = Termination::TimeLimit;
    }
}

/// Ownership-preserving control for engines whose callbacks are retained.
/// The portable web-time clock supports native hosts and WASM workers.
pub struct OwnedDeadlineControl {
    parent: Arc<dyn RunControl>,
    deadline: Option<web_time::Instant>,
    expired: AtomicBool,
}
impl OwnedDeadlineControl {
    pub fn new(parent: Arc<dyn RunControl>, limits: RuntimeLimits) -> Result<Self> {
        Ok(Self { parent, deadline: make_deadline(limits)?, expired: AtomicBool::new(false) })
    }
    pub fn timed_out(&self) -> bool {
        self.expired.load(Ordering::Acquire)
    }
    pub fn apply_termination(&self, result: &mut SolveResult) {
        apply_time_limit(self.timed_out(), &mut result.termination);
    }
    pub fn apply_simulation_termination(&self, result: &mut SimulationResult) {
        apply_time_limit(self.timed_out(), &mut result.termination);
    }
}
impl RunControl for OwnedDeadlineControl {
    fn is_cancelled(&self) -> bool {
        is_stopped(self.parent.as_ref(), self.deadline, &self.expired)
    }
    fn progress(&self, event: &ProgressEvent) {
        self.parent.progress(event);
    }
}

impl<'a> DeadlineControl<'a> {
    pub fn new(parent: &'a dyn RunControl, limits: RuntimeLimits) -> Result<Self> {
        let deadline = make_deadline(limits)?;
        Ok(Self { parent, deadline, expired: std::sync::atomic::AtomicBool::new(false) })
    }
    pub fn timed_out(&self) -> bool {
        self.expired.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn apply_termination(&self, result: &mut SolveResult) {
        apply_time_limit(self.timed_out(), &mut result.termination);
    }
    pub fn apply_simulation_termination(&self, result: &mut SimulationResult) {
        apply_time_limit(self.timed_out(), &mut result.termination);
    }
}
impl RunControl for DeadlineControl<'_> {
    fn is_cancelled(&self) -> bool {
        is_stopped(self.parent, self.deadline, &self.expired)
    }
    fn progress(&self, event: &ProgressEvent) {
        self.parent.progress(event);
    }
}

/// Borrowed compatibility route. Retained-callback engines and integer QP use
/// solve_with_limits_owned; this preserves the existing borrowed engine limits.
pub fn solve_with_limits(model: &OptimizationModel, options: &SolveOptions, limits: RuntimeLimits, control: &dyn RunControl) -> Result<SolveResult> {
    let deadline = DeadlineControl::new(control, limits)?;
    let mut result = crate::solve(model, options, &deadline)?;
    deadline.apply_termination(&mut result);
    Ok(result)
}
/// Portable owned-control auto solve, including finite-domain integer QP.
pub fn solve_with_limits_owned(
    model: &OptimizationModel,
    options: &SolveOptions,
    limits: RuntimeLimits,
    control: Arc<dyn RunControl>,
) -> Result<SolveResult> {
    let deadline = Arc::new(OwnedDeadlineControl::new(control, limits)?);
    let callback: Arc<dyn RunControl> = deadline.clone();
    let mut result = solvercraft_engines::solve_owned(model, options, callback)?;
    deadline.apply_termination(&mut result);
    Ok(result)
}
