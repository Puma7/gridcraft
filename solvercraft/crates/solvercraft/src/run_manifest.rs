//! Streaming diagnostic run identity shared by command-line and embedded hosts.
//!
//! The fingerprint describes the exact compact Serde JSON stream, including
//! expanded defaults. It is not a cryptographic digest, a workbook revision,
//! or a canonical encoding independent of the input type's map ordering.
use serde::{Deserialize, Serialize};
use solvercraft_types::{Result, SolverError};
use std::io::Write;

pub const MAX_REQUEST_BYTES: u64 = 16 * 1024 * 1024;
pub const FINGERPRINT_ALGORITHM: &str = "fnv1a64-serde-json-diagnostic-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticFingerprint {
    pub algorithm: String,
    pub value: String,
    pub serialized_bytes: u64,
}
struct FingerprintWriter {
    hash: u64,
    count: u64,
    limit: u64,
    exceeded: bool,
}
impl Write for FingerprintWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self.count.checked_add(bytes.len() as u64);
        if next.is_none_or(|count| count > self.limit) {
            self.exceeded = true;
            return Err(std::io::Error::other("Diagnostic request fingerprint exceeds byte limit"));
        }
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
        self.count = next.ok_or_else(|| std::io::Error::other("Fingerprint byte counter overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Hash a bounded serialized stream without materializing JSON or cloning input.
/// A caller may impose a smaller budget, but cannot bypass the 16 MiB ceiling.
pub fn diagnostic_fingerprint<T: Serialize + ?Sized>(request: &T, max_bytes: u64) -> Result<DiagnosticFingerprint> {
    if max_bytes == 0 || max_bytes > MAX_REQUEST_BYTES {
        return Err(SolverError::new("input_limit", "Fingerprint budget must be in 1..=16 MiB"));
    }
    let mut writer = FingerprintWriter { hash: 0xcbf29ce484222325, count: 0, limit: max_bytes, exceeded: false };
    if let Err(error) = serde_json::to_writer(&mut writer, request) {
        return Err(SolverError::new(if writer.exceeded { "input_limit" } else { "serialization" }, error.to_string()));
    }
    Ok(DiagnosticFingerprint { algorithm: FINGERPRINT_ALGORITHM.into(), value: format!("{:016x}", writer.hash), serialized_bytes: writer.count })
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SnapshotIdentity {
    DiagnosticRequestFingerprint {
        value: String,
    },
    /// Supplied by the embedding host; Solvercraft does not attest its truth.
    CallerSupplied {
        id: String,
        revision: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub operating_system: String,
    pub architecture: String,
    pub family: String,
    pub pointer_width: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct Manifest<C> {
    pub schema_version: u32,
    pub solvercraft_version: &'static str,
    pub entrypoint: String,
    pub platform: Platform,
    pub input: DiagnosticFingerprint,
    pub snapshot: SnapshotIdentity,
    /// Borrowed typed effective options, including seeds, tolerances and all
    /// workflow-specific settings. Input data are identified by `input`.
    pub effective_configuration: C,
    pub actual_engine: String,
    /// Host-supplied locator, e.g. `stdout:$.result` or a persisted result ID.
    pub result_location: String,
}
/// Object results only. `result` must not already contain a `manifest` key;
/// existing execution/provenance wrappers keep their original manifests.
#[derive(Debug, Serialize)]
pub struct Run<'a, R: ?Sized, C> {
    #[serde(flatten)]
    pub result: &'a R,
    pub manifest: Manifest<C>,
}
#[derive(Debug, Clone)]
pub struct PreparedManifest<C> {
    entrypoint: String,
    input: DiagnosticFingerprint,
    snapshot: SnapshotIdentity,
    configuration: C,
}
fn text(value: &str, label: &str) -> Result<String> {
    if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(SolverError::new("invalid_manifest", format!("{label} must contain 1..=1024 bytes without control characters")));
    }
    Ok(value.into())
}
/// Prepare identity before expensive execution; the configuration can borrow
/// the request and never needs a `serde_json::Value` or deep clone.
pub fn prepare<T: Serialize + ?Sized, C>(entrypoint: &str, request: &T, configuration: C, max_bytes: u64) -> Result<PreparedManifest<C>> {
    let entrypoint = text(entrypoint, "Entrypoint")?;
    let input = diagnostic_fingerprint(request, max_bytes)?;
    let snapshot = SnapshotIdentity::DiagnosticRequestFingerprint { value: input.value.clone() };
    Ok(PreparedManifest { entrypoint, input, snapshot, configuration })
}
impl<C> PreparedManifest<C> {
    pub fn with_caller_snapshot(mut self, id: &str, revision: Option<&str>) -> Result<Self> {
        self.snapshot = SnapshotIdentity::CallerSupplied {
            id: text(id, "Snapshot identity")?,
            revision: revision.map(|value| text(value, "Snapshot revision")).transpose()?,
        };
        Ok(self)
    }
    pub fn complete<'a, R: ?Sized>(self, result: &'a R, actual_engine: &str, result_location: &str) -> Result<Run<'a, R, C>> {
        Ok(Run {
            result,
            manifest: Manifest {
                schema_version: 1,
                solvercraft_version: crate::VERSION,
                entrypoint: self.entrypoint,
                platform: Platform {
                    operating_system: std::env::consts::OS.into(),
                    architecture: std::env::consts::ARCH.into(),
                    family: std::env::consts::FAMILY.into(),
                    pointer_width: usize::BITS,
                },
                input: self.input,
                snapshot: self.snapshot,
                effective_configuration: self.configuration,
                actual_engine: text(actual_engine, "Actual engine")?,
                result_location: text(result_location, "Result location")?,
            },
        })
    }
}
