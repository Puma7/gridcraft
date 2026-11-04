//! Bounded diagnostic run identity. This records effective host configuration;
//! neither caller source labels nor FNV fingerprints are build attestations.
use super::{Result, SOLVE, Snapshot, bad, snapshot_time};
use serde_json::{Value as Json, json};
const MAX_BYTES: u64 = 1024 * 1024;
struct Fingerprint {
    bytes: u64,
    hash: u64,
}
impl std::io::Write for Fingerprint {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(|| std::io::Error::other("Host run request/manifest exceeds1MiB"))?;
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn fingerprint(value: &Json) -> Result<String> {
    let mut writer = Fingerprint { bytes: 0, hash: 0xcbf29ce484222325 };
    serde_json::to_writer(&mut writer, value).map_err(|e| bad(SOLVE, e.to_string()))?;
    Ok(format!("{:016x}", writer.hash))
}
pub(super) fn create(kind: &str, snapshot: &Snapshot, request: &Json, options: Json, backend: Json, bindings: Json) -> Result<Json> {
    let identity = fingerprint(request)?;
    let manifest = json!({
        "schema_version":1,"kind":kind,"solvercraft_version": ::solvercraft::VERSION,
        "gridcraft_engine_version":env!("CARGO_PKG_VERSION"),
        "platform":format!("{}-{}",std::env::consts::ARCH,std::env::consts::OS),
        "snapshot":snapshot,"snapshot_time_1900_serial":snapshot_time(request)?,
        "request_fingerprint":identity,"fingerprint_algorithm":"fnv1a64-diagnostic-v1",
        "effective_options":options,"backend":backend,"bindings":bindings,
        "request_location":"preview.request","identity_guarantee":"diagnostic configuration record; not cryptographic attestation"
    });
    fingerprint(&manifest)?;
    Ok(manifest)
}
pub(super) fn annotate_limits(preview: &mut Json, limits: &::solvercraft::runtime::RuntimeLimits) {
    if let Some(manifest) = preview.get_mut("manifest").and_then(Json::as_object_mut) {
        manifest.insert("runtime_limits".into(), json!(limits));
    }
    if let Some(cases) = preview.get_mut("cases").and_then(Json::as_array_mut) {
        for case in cases {
            if let Some(preview) = case.get_mut("preview") {
                annotate_limits(preview, limits);
            }
        }
    }
}
