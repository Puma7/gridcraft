//! Bounded native background jobs. WASM hosts must use direct cooperative engine callbacks.
use solvercraft_types::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[cfg(not(target_arch = "wasm32"))]
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
#[cfg(not(target_arch = "wasm32"))]
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(target_arch = "wasm32"))]
const MAX_ACTIVE: usize = 4;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}
#[derive(Debug, Clone)]
pub struct JobSnapshot<T: Clone> {
    pub id: u64,
    pub status: JobStatus,
    pub progress: Option<ProgressEvent>,
    pub result: Option<Result<Arc<T>>>,
}
pub struct JobHandle<T: Clone> {
    id: u64,
    cancel: Arc<AtomicBool>,
    state: Arc<Mutex<JobSnapshot<T>>>,
}
impl<T: Clone> JobHandle<T> {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn snapshot(&self) -> Result<JobSnapshot<T>> {
        self.state.lock().map(|s| s.clone()).map_err(|_| SolverError::new("job_state", "Job state lock poisoned"))
    }
}
#[cfg(not(target_arch = "wasm32"))]
struct Control<T: Clone + Send + Sync> {
    cancel: Arc<AtomicBool>,
    state: Arc<Mutex<JobSnapshot<T>>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Clone + Send + Sync> RunControl for Control<T> {
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
    fn progress(&self, event: &ProgressEvent) {
        if let Ok(mut state) = self.state.lock() {
            state.progress = Some(event.clone());
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn spawn<T, F>(operation: F) -> Result<JobHandle<T>>
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce(&dyn RunControl) -> Result<T> + Send + 'static,
{
    ACTIVE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| if v < MAX_ACTIVE { Some(v + 1) } else { None })
        .map_err(|_| SolverError::new("job_limit", "Four Solvercraft jobs are already active"))?;
    let id = match NEXT_ID.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1)) {
        Ok(id) => id,
        Err(_) => {
            ACTIVE.fetch_sub(1, Ordering::AcqRel);
            return Err(SolverError::new("job_id", "Job ID capacity exhausted"));
        }
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(JobSnapshot { id, status: JobStatus::Queued, progress: None, result: None }));
    let handle = JobHandle { id, cancel: Arc::clone(&cancel), state: Arc::clone(&state) };
    let spawned = std::thread::Builder::new().name(format!("solvercraft-{id}")).spawn(move || {
        struct Slot;
        impl Drop for Slot {
            fn drop(&mut self) {
                ACTIVE.fetch_sub(1, Ordering::AcqRel);
            }
        }
        let _slot = Slot;
        if let Ok(mut s) = state.lock() {
            s.status = JobStatus::Running;
        }
        let control = Control { cancel, state: Arc::clone(&state) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(&control)))
            .unwrap_or_else(|_| Err(SolverError::new("job_panic", "Background evaluator panicked")))
            .map(Arc::new);
        if let Ok(mut s) = state.lock() {
            s.status = if result.is_ok() { JobStatus::Completed } else { JobStatus::Failed };
            s.result = Some(result);
        }
    });
    if let Err(e) = spawned {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
        return Err(SolverError::new("job_spawn", e.to_string()));
    }
    Ok(handle)
}
pub fn start_solve(model: OptimizationModel, options: SolveOptions) -> Result<JobHandle<SolveResult>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn(move |c| solvercraft_engines::solve(&model, &options, c))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (model, options);
        Err(SolverError::new("unsupported_host", "Use direct engine callbacks in a WASM worker"))
    }
}
pub fn start_simulation<F>(distributions: Vec<Distribution>, options: SimulationOptions, evaluator: F) -> Result<JobHandle<SimulationResult>>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>> + Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn(move |c| solvercraft_simulation::simulate(&distributions, &options, c, evaluator))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (distributions, options, evaluator);
        Err(SolverError::new("unsupported_host", "Use direct simulation callbacks in a WASM worker"))
    }
}
