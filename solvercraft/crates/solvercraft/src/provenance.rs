//! Portable run identity; fingerprints are diagnostic, never authorization tokens.
use serde::{Deserialize, Serialize};
use solvercraft_types::*;
use std::sync::Arc;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u32,
    pub solvercraft_version: String,
    pub model_fingerprint: String,
    pub fingerprint_algorithm: String,
    pub options: SolveOptions,
    pub platform: String,
    #[serde(default)]
    pub entrypoint: String,
    #[serde(default)]
    pub snapshot: Option<crate::run_manifest::SnapshotIdentity>,
    #[serde(default)]
    pub actual_engine: Option<String>,
    #[serde(default)]
    pub result_location: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveRun {
    pub manifest: RunManifest,
    pub result: SolveResult,
}
pub fn manifest(model: &OptimizationModel, options: &SolveOptions) -> Result<RunManifest> {
    solvercraft_model::validate(model, options)?;
    struct HashWriter {
        hash: u64,
        count: u64,
        limit: u64,
        exceeded: bool,
    }
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.count.checked_add(bytes.len() as u64);
            if next.is_none_or(|n| n > self.limit) {
                self.exceeded = true;
                return Err(std::io::Error::other("Manifest exceeds byte budget"));
            }
            for byte in bytes {
                self.hash ^= u64::from(*byte);
                self.hash = self.hash.wrapping_mul(0x100000001b3);
            }
            self.count = next.ok_or_else(|| std::io::Error::other("Byte counter overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = HashWriter { hash: 0xcbf29ce484222325, count: 0, limit: options.max_memory_bytes, exceeded: false };
    if let Err(error) = serde_json::to_writer(&mut writer, model) {
        return Err(SolverError::new(if writer.exceeded { "memory_limit" } else { "serialization" }, error.to_string()));
    }
    let hash = writer.hash;
    Ok(RunManifest {
        schema_version: SCHEMA_VERSION,
        solvercraft_version: env!("CARGO_PKG_VERSION").into(),
        model_fingerprint: format!("{hash:016x}"),
        fingerprint_algorithm: "fnv1a64-diagnostic-v1".into(),
        options: options.clone(),
        platform: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        entrypoint: "solve".into(),
        snapshot: Some(crate::run_manifest::SnapshotIdentity::DiagnosticRequestFingerprint { value: format!("{hash:016x}") }),
        actual_engine: None,
        result_location: "SolveRun.result".into(),
    })
}
/// Borrowed compatibility path; use solve_run_owned for integer QP and engines
/// retaining their owned callback state.
pub fn solve_run(model: &OptimizationModel, options: &SolveOptions, control: &dyn RunControl) -> Result<SolveRun> {
    let mut manifest = manifest(model, options)?;
    let result = solvercraft_engines::solve(model, options, control)?;
    manifest.actual_engine = Some(result.engine.clone());
    Ok(SolveRun { manifest, result })
}
/// Owned callback variant reaches integer quadratic engines without discarding run identity.
pub fn solve_run_owned(model: &OptimizationModel, options: &SolveOptions, control: Arc<dyn RunControl>) -> Result<SolveRun> {
    let mut manifest = manifest(model, options)?;
    let result = solvercraft_engines::solve_owned(model, options, control)?;
    manifest.actual_engine = Some(result.engine.clone());
    Ok(SolveRun { manifest, result })
}
