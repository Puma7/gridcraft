#![allow(clippy::unwrap_used, clippy::panic)]
use serde::{Serialize, ser::SerializeSeq};
use solvercraft::run_manifest::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn streaming_fingerprint_is_bounded_before_a_generated_input_is_materialized() {
    struct Generated<'a>(&'a AtomicUsize);
    impl Serialize for Generated<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut sequence = serializer.serialize_seq(Some(1_000_000))?;
            for _ in 0..1_000_000 {
                self.0.fetch_add(1, Ordering::Relaxed);
                sequence.serialize_element(&123456789u64)?;
            }
            sequence.end()
        }
    }
    let count = AtomicUsize::new(0);
    let error = diagnostic_fingerprint(&Generated(&count), 64).unwrap_err();
    assert_eq!(error.code, "input_limit");
    assert!(count.load(Ordering::Relaxed) < 10);
    assert!(diagnostic_fingerprint(&0, 0).is_err());
    assert!(diagnostic_fingerprint(&0, MAX_REQUEST_BYTES + 1).is_err());
}
#[test]
fn typed_borrowed_configuration_and_top_level_result_survive_without_clone_bounds() {
    #[derive(Serialize)]
    struct NoCloneRequest {
        seed: String,
        data: Vec<i32>,
    }
    #[derive(Serialize)]
    struct Config<'a> {
        seed: &'a str,
        tolerance: f64,
    }
    #[derive(Serialize)]
    struct Output {
        values: Vec<f64>,
        certificate: &'static str,
    }
    let input = NoCloneRequest { seed: u64::MAX.to_string(), data: vec![1, 2, 3] };
    let config = Config { seed: &input.seed, tolerance: 1e-9 };
    let prepared = prepare("test", &input, config, 1024).unwrap();
    let result = Output { values: vec![42.0], certificate: "checked" };
    let run = prepared.complete(&result, "actual-test-engine-v1", "stdout:$").unwrap();
    let encoded = serde_json::to_value(&run).unwrap();
    assert_eq!(encoded["values"][0], 42.0);
    assert_eq!(encoded["certificate"], "checked");
    assert_eq!(encoded["manifest"]["effective_configuration"]["seed"], u64::MAX.to_string());
    assert_eq!(encoded["manifest"]["snapshot"]["kind"], "diagnostic_request_fingerprint");
    assert_eq!(encoded["manifest"]["actual_engine"], "actual-test-engine-v1");
    assert_eq!(encoded["manifest"]["input"]["serialized_bytes"], serde_json::to_vec(&input).unwrap().len());
    assert_eq!(encoded["manifest"]["platform"]["pointer_width"], usize::BITS);
    let repeated = diagnostic_fingerprint(&input, 1024).unwrap();
    assert_eq!(encoded["manifest"]["input"]["value"], repeated.value);
    let changed = NoCloneRequest { seed: input.seed.clone(), data: vec![1, 2, 4] };
    assert_ne!(diagnostic_fingerprint(&changed, 1024).unwrap().value, repeated.value);
}
#[test]
fn explicit_caller_identity_is_labelled_and_metadata_text_is_bounded() {
    let prepared = prepare("test", &123, (), 1024).unwrap().with_caller_snapshot("workbook-7", Some("revision-12")).unwrap();
    let result = serde_json::json!({"result":1});
    let run = prepared.complete(&result, "engine", "persisted-result:8").unwrap();
    let value = serde_json::to_value(run).unwrap();
    assert_eq!(value["manifest"]["snapshot"]["kind"], "caller_supplied");
    assert_eq!(value["manifest"]["snapshot"]["revision"], "revision-12");
    assert!(prepare("", &0, (), 32).is_err());
    assert!(prepare("line\nbreak", &0, (), 32).is_err());
    assert!(prepare("test", &0, (), 32).unwrap().with_caller_snapshot(&"x".repeat(1025), None).is_err());
    assert!(prepare("test", &0, (), 32).unwrap().complete(&result, "", "stdout:$").is_err());
}
