#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;
// These seven fixtures exercise the same real4-worker SDK pool. Keep their
// lifecycle isolated while the rest of the engine suite remains parallel.
static ASYNC_FIXTURES: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct AsyncFixture {
    _guard: std::sync::MutexGuard<'static, ()>,
}
fn async_fixture() -> AsyncFixture {
    AsyncFixture { _guard: ASYNC_FIXTURES.lock().unwrap_or_else(std::sync::PoisonError::into_inner) }
}
impl Drop for AsyncFixture {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let drained = super::jobs::finish_test_jobs();
            if !std::thread::panicking() {
                assert!(drained.is_ok(), "{drained:?}");
            }
        }
    }
}
fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({})).unwrap();
    s
}
fn set(s: &mut Session, cell: &str, input: &str) {
    s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
}
fn request() -> Json {
    json!({"objective":"B1","variables":"A1","constraints":["A1<=5"]})
}
fn save(s: &mut Session, name: &str) -> Json {
    s.execute("solvercraft.model.create", json!({"name":name,"request":request()})).unwrap()
}
fn wait(s: &mut Session, id: &Json) -> Json {
    let limit = ::solvercraft::runtime::monotonic_now() + std::time::Duration::from_secs(10);
    loop {
        let state = s.execute("solvercraft.job.get", json!({"id":id})).unwrap();
        if matches!(state["status"].as_str(), Some("completed" | "failed")) {
            assert_eq!(state["status"], "completed", "{state}");
            return s.execute("solvercraft.job.result", json!({"id":id})).unwrap();
        }
        assert!(::solvercraft::runtime::monotonic_now() < limit, "{state}");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn solvercraft_models_have_lossless_distinct_ids_and_undoable_edits() {
    let mut s = session();
    let a = save(&mut s, "A");
    let b = save(&mut s, "B");
    assert_ne!(a["id"], b["id"]);
    assert!(s.execute("solvercraft.model.create", json!({"name":"a","request":request()})).is_err());
    s.execute("solvercraft.model.delete", json!({"id":a["id"]})).unwrap();
    assert_eq!(s.execute("solvercraft.model.list", json!({})).unwrap()["models"].as_array().unwrap().len(), 1);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.execute("solvercraft.model.list", json!({})).unwrap()["models"].as_array().unwrap().len(), 2);
    s.execute("solvercraft.model.update", json!({"id":a["id"],"name":"Renamed"})).unwrap();
    assert_eq!(s.execute("solvercraft.model.get", json!({"id":a["id"]})).unwrap()["name"], "Renamed");
}
#[test]
fn solvercraft_model_names_use_one_unicode_casefold_for_writes_and_reads() {
    let mut s = session();
    let first = save(&mut s, "Öptimum");
    let before = Arc::clone(&s.doc().unwrap().wb);
    assert!(s.execute("solvercraft.model.create", json!({"name":"öPTIMUM","request":request()})).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    assert_eq!(s.execute("solvercraft.model.get", json!({"id":first["id"]})).unwrap()["name"], "Öptimum");
}
#[test]
fn solvercraft_owned_xlsx_part_relationship_and_anchor_names_roundtrip_twice() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=2*A1");
    let model = save(&mut s, "Roundtrip");
    for _ in 0..2 {
        let bytes = gridcraft_xlsx::write_xlsx(&s.doc().unwrap().wb).unwrap();
        let (wb, report) = gridcraft_xlsx::read_xlsx(&bytes).unwrap();
        assert!(!report.warnings.iter().any(|s| s.contains("Solvercraft") || s.contains("workbook-models")), "{:?}", report.warnings);
        let kept = wb.preserved.as_deref().unwrap();
        assert!(kept.parts.iter().any(|p| p.name == "xl/solvercraft/models.json"));
        assert!(
            kept.rels
                .iter()
                .any(|r| r.rel_type == "https://solvercraft.org/relationships/workbook-models/v1" && r.target == "xl/solvercraft/models.json")
        );
        s.doc_mut().unwrap().wb = Arc::new(wb);
        let loaded = s.execute("solvercraft.model.get", json!({"id":model["id"]})).unwrap();
        assert_eq!(loaded["name"], "Roundtrip");
        assert!(loaded["request"]["objective"].as_str().unwrap().contains("$B$1"));
    }
}
#[test]
fn solvercraft_owned_json_survives_strict_open_xml_input_and_transitional_resave() {
    let mut s = session();
    let model = save(&mut s, "Strict");
    let source = gridcraft_xlsx::write_xlsx(&s.doc().unwrap().wb).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..archive.len() {
        let mut part = archive.by_index(i).unwrap();
        let name = part.name().to_string();
        let mut data = Vec::new();
        part.read_to_end(&mut data).unwrap();
        if name.ends_with(".xml") || name.ends_with(".rels") {
            data = String::from_utf8(data)
                .unwrap()
                .replace("http://schemas.openxmlformats.org/spreadsheetml/2006/main", "http://purl.oclc.org/ooxml/spreadsheetml/main")
                .replace(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                    "http://purl.oclc.org/ooxml/officeDocument/relationships",
                )
                .into_bytes();
        }
        output.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        output.write_all(&data).unwrap();
    }
    let strict = output.finish().unwrap().into_inner();
    let (loaded, _) = gridcraft_xlsx::read_xlsx(&strict).unwrap();
    s.doc_mut().unwrap().wb = Arc::new(loaded);
    assert_eq!(s.execute("solvercraft.model.get", json!({"id":model["id"]})).unwrap()["name"], "Strict");
    let saved = gridcraft_xlsx::write_xlsx(&s.doc().unwrap().wb).unwrap();
    let (loaded, _) = gridcraft_xlsx::read_xlsx(&saved).unwrap();
    s.doc_mut().unwrap().wb = Arc::new(loaded);
    assert_eq!(s.execute("solvercraft.model.get", json!({"id":model["id"]})).unwrap()["name"], "Strict");
}
#[test]
fn solvercraft_saved_bindings_follow_row_insertion_and_sheet_rename() {
    let mut s = session();
    set(&mut s, "B1", "=A1");
    let saved = save(&mut s, "Moves");
    s.execute("home.insertRows", json!({"rows":"1:1"})).unwrap();
    s.execute("sheet.rename", json!({"name":"Moved sheet"})).unwrap();
    let model = s.execute("solvercraft.model.get", json!({"id":saved["id"]})).unwrap();
    assert_eq!(model["request"]["objective"], "'Moved sheet'!$B$2");
    assert_eq!(model["request"]["variables"][0]["cell"], "'Moved sheet'!$A$2");
    let validated = s.execute("solvercraft.validate", model["request"].clone()).unwrap();
    assert_eq!(validated["bindings"][0]["sheet"], "Moved sheet");
}
#[test]
fn solvercraft_saved_deleted_reference_fails_explicitly() {
    let mut s = session();
    let saved = save(&mut s, "Deleted");
    s.execute("home.deleteRows", json!({"rows":"1:1"})).unwrap();
    assert!(s.execute("solvercraft.model.get", json!({"id":saved["id"]})).is_err());
}
#[test]
fn solvercraft_export_import_versions_and_collisions_are_checked() {
    let mut s = session();
    let saved = save(&mut s, "Portable");
    let mut project = s.execute("solvercraft.model.export", json!({"id":saved["id"]})).unwrap();
    assert!(s.execute("solvercraft.model.import", json!({"project":project})).is_err());
    let imported = s.execute("solvercraft.model.import", json!({"project":project,"name":"Second"})).unwrap();
    assert_ne!(saved["id"], imported["id"]);
    project["schema_version"] = json!(999);
    assert!(s.execute("solvercraft.model.import", json!({"project":project,"name":"Third"})).is_err());
}
#[test]
fn solvercraft_async_snapshot_is_readonly_and_apply_is_one_undo_step() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=2*A1");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let revision = s.doc().unwrap().revision;
    let job = s.execute("solvercraft.job.start", json!({"request":request()})).unwrap();
    assert!(job["id"].is_string());
    let preview = wait(&mut s, &job["id"]);
    assert_eq!(preview["result"]["objective"], 10.0);
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    assert_eq!(s.doc().unwrap().revision, revision);
    s.execute("solvercraft.job.apply", json!({"id":job["id"]})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(5.0));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(0.0));
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
}
#[test]
fn solvercraft_async_revision_and_document_guards_cannot_be_bypassed() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "B1", "=A1");
    let job = s.execute("solvercraft.job.start", json!({"request":request()})).unwrap();
    wait(&mut s, &job["id"]);
    set(&mut s, "C1", "7");
    assert_eq!(s.execute("solvercraft.job.get", json!({"id":job["id"]})).unwrap()["stale"], true);
    assert!(s.execute("solvercraft.job.apply", json!({"id":job["id"]})).is_err());
    s.execute("file.new", json!({})).unwrap();
    assert!(s.execute("solvercraft.job.get", json!({"id":job["id"]})).is_err());
    assert!(s.execute("solvercraft.job.cancel", json!({"id":job["id"]})).is_err());
    s.set_active(0);
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
}
#[test]
fn solvercraft_parameter_cases_are_constant_and_share_comparable_trial_streams() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "C1", "3");
    set(&mut s, "B1", "=A1*C1");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let result=s.execute("solvercraft.simulateParameters",json!({"request":{"inputs":[{"cell":"A1","distribution":{"kind":"uniform","low":1.0,"high":2.0}}],"outputs":"B1","options":{"trials":20,"seed":"123","retain_samples":true}},"parameter_cases":[{"name":"two","values":[{"cell":"C1","value":2.0}]},{"name":"four","values":[{"cell":"C1","value":4.0}]}]})).unwrap();
    let first = result["cases"][0]["preview"]["result"]["samples"].as_array().unwrap();
    let second = result["cases"][1]["preview"]["result"]["samples"].as_array().unwrap();
    assert_eq!(first.len(), 20);
    for (a, b) in first.iter().zip(second) {
        assert_eq!(a[0].as_f64().unwrap() * 2.0, b[0].as_f64().unwrap());
    }
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn solvercraft_saved_parameter_grid_and_sc_bindings_migrate_and_run_asynchronously() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "C1", "2");
    set(&mut s, "A1", "=SC.NORMAL(C1,0)");
    set(&mut s, "B1", "=SC.OUTPUT(A1)");
    let model=s.execute("solvercraft.model.create",json!({"name":"SC parameters","kind":"simulation","request":{"options":{"trials":4,"seed":"0"},"parameter_grid":[{"cell":"C1","low":1.0,"base":2.0,"high":4.0}],"parameter_cases":[{"name":"four","values":[{"cell":"C1","value":4.0}]}]}})).unwrap();
    s.execute("home.insertRows", json!({"rows":"1:1"})).unwrap();
    let loaded = s.execute("solvercraft.model.get", json!({"id":model["id"]})).unwrap();
    assert!(loaded["request"]["parameter_grid"][0]["cell"].as_str().unwrap().contains("$C$2"));
    let job = s.execute("solvercraft.job.start", json!({"model_id":model["id"]})).unwrap();
    let result = wait(&mut s, &job["id"]);
    assert_eq!(result["cases"][0]["preview"]["result"]["summaries"][0]["mean"], 4.0);
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
}
#[test]
fn solvercraft_host_routes_real_general_nonlinear_constraints() {
    let mut s = session();
    set(&mut s, "A1", "0.5");
    set(&mut s, "B1", "=(A1-2)^2");
    let preview=s.execute("solvercraft.solve",json!({"objective":"B1","goal":"min","variables":[{"cell":"A1","lower":0.0,"upper":3.0}],"constraints":["A1^2<=1"],"method":"projected_gradient","options":{"feasibility_tolerance":0.00001,"optimality_tolerance":0.00001}})).unwrap();
    assert_eq!(preview["result"]["feasibility"], "feasible", "{preview}");
    assert!((preview["result"]["values"][0].as_f64().unwrap() - 1.0).abs() < 0.001, "{preview}");
    assert!(preview["result"]["engine"].as_str().unwrap().contains("augmented"), "{preview}");
}
#[test]
fn solvercraft_async_zero_deadline_is_a_time_limit_not_success() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "B1", "=A1");
    let job = s.execute("solvercraft.job.start", json!({"request":request(),"limits":{"max_time_millis":0}})).unwrap();
    let preview = wait(&mut s, &job["id"]);
    assert_eq!(preview["result"]["termination"], "time_limit");
    assert!(s.execute("solvercraft.job.apply", json!({"id":job["id"]})).is_err());
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
}
#[test]
fn solvercraft_auto_miqp_reaches_owned_backend_in_sync_and_async_hosts() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=(A1-1.5)^2");
    let request = json!({"objective":"B1","goal":"min","variables":[{"cell":"A1","kind":"integer","lower":0.0,"upper":3.0}]});
    let preview = s.execute("solvercraft.solve", request.clone()).unwrap();
    assert_eq!(preview["result"]["feasibility"], "feasible");
    assert!((preview["result"]["objective"].as_f64().unwrap() - 0.25).abs() < 1e-6, "{preview}");
    let job = s.execute("solvercraft.job.start", json!({"request":request})).unwrap();
    let result = wait(&mut s, &job["id"]);
    assert_eq!(result["result"]["feasibility"], "feasible");
    assert!((result["result"]["objective"].as_f64().unwrap() - 0.25).abs() < 1e-6);
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
}
#[test]
fn solvercraft_integer_rounding_is_rechecked_against_original_rows_before_apply() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    set(&mut s, "C1", "=100000000*A1");
    let request = json!({"objective":"B1","goal":"min","variables":[{"cell":"A1","kind":"integer","lower":0.0,"upper":1.0}],"constraints":["C1>=5"]});
    let preview = s.execute("solvercraft.solve", request).unwrap();
    let mut invalid = preview.clone();
    invalid["result"]["values"] = json!([0.00000005]);
    assert!(s.execute("solvercraft.result.apply", invalid).is_err());
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(0.0));
    let mut near = preview;
    near["result"]["values"] = json!([0.99999999]);
    let applied = s.execute("solvercraft.result.apply", near).unwrap();
    assert_eq!(applied["values"], json!([1.0]));
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(1.0));
}
#[test]
fn solvercraft_sc_declared_inputs_share_trial_values_and_do_not_replace_formulas() {
    let mut s = session();
    set(&mut s, "A1", "=SC.UNIFORM(1,2)");
    set(&mut s, "B1", "=SC.OUTPUT(A1-A1)");
    let before = Arc::clone(&s.doc().unwrap().wb);
    assert_eq!(before.active().unwrap().value(CellRef::parse("A1").unwrap()), Value::number(1.5));
    let discovered = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovered["inputs"].as_array().unwrap().len(), 1);
    let output = s.execute("solvercraft.simulate", json!({"options":{"trials":40,"seed":"18446744073709551615","retain_samples":true}})).unwrap();
    assert_eq!(output["result"]["completed_trials"], 40);
    assert!(output["result"]["samples"].as_array().unwrap().iter().all(|row| row[0] == 0.0));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn solvercraft_sc_statistics_have_explicit_empirical_loss_semantics() {
    let mut s = session();
    set(&mut s, "A1", "=SC.CVAR({0,0,0,10},0.5)");
    set(&mut s, "A2", "=SC.QUANTILE({1,2,3,4},0.5)");
    set(&mut s, "A3", "=SC.NORMAL(3,-1)");
    set(&mut s, "A4", "=SC.TRIANGULAR(0,3,9)");
    let sh = s.doc().unwrap().wb.active().unwrap();
    assert_eq!(sh.value(CellRef::parse("A1").unwrap()), Value::number(5.0));
    assert_eq!(sh.value(CellRef::parse("A2").unwrap()), Value::number(2.0));
    assert!(matches!(sh.value(CellRef::parse("A3").unwrap()), Value::Error(_)));
    assert_eq!(sh.value(CellRef::parse("A4").unwrap()), Value::number(4.0));
}
#[test]
fn solvercraft_now_today_are_frozen_in_translation_and_original_calc_validation() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1+NOW()-TODAY()");
    let preview =
        s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=1"],"_snapshot_time_serial":46000.125})).unwrap();
    assert_eq!(preview["request"]["_snapshot_time_serial"], 46000.125);
    assert_eq!(preview["result"]["objective"], 1.125);
    s.execute("solvercraft.result.apply", preview).unwrap();
    assert_eq!(gridcraft_calc::evaluate_at(&s.doc().unwrap().wb, 0, CellRef::new(0, 0), "NOW()-TODAY()", 46000.125), Value::number(0.125));
    set(&mut s, "B1", "=A1+RAND()");
    assert!(s.execute("solvercraft.solve", request()).is_err());
}
#[test]
fn solvercraft_sc_distribution_parameters_and_outputs_read_one_frozen_clock() {
    let mut s = session();
    set(&mut s, "C1", "=NOW()");
    set(&mut s, "A1", "=SC.NORMAL(C1,0)");
    set(&mut s, "B1", "=SC.OUTPUT(A1-C1)");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let output = s
        .execute("solvercraft.simulate", json!({"_snapshot_time_serial":46000.125,"options":{"trials":3,"seed":"0","retain_samples":true}}))
        .unwrap();
    assert_eq!(output["result"]["samples"], json!([[0.0], [0.0], [0.0]]));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn solvercraft_answer_report_traces_original_rows_and_tolerances() {
    let mut s = session();
    set(&mut s, "B1", "=A1");
    set(&mut s, "C1", "=2*A1");
    let preview = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["C1<=6"]})).unwrap();
    let report = s.execute("solvercraft.report.answer", preview).unwrap();
    assert_eq!(report["objective"]["validated_value"], 3.0);
    assert_eq!(report["constraints"][0]["left_value"], 6.0);
    assert_eq!(report["constraints"][0]["slack"], 0.0);
    assert_eq!(report["variables"][0]["value"], 3.0);
    assert!(report["tolerances"]["feasibility"].is_number());
    assert_eq!(report["dual_data"]["available"], false);
}
#[test]
fn solvercraft_legacy_export_import_preserves_linear_typed_domains_and_is_undoable() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "A2", "0");
    set(&mut s, "B1", "=2*A1+3*A2");
    set(&mut s, "C1", "=A1+A2");
    let model=s.execute("solvercraft.model.create",json!({"name":"Typed export","request":{"objective":"B1","variables":[{"cell":"A1","kind":"integer","lower":-2.0,"upper":5.0},{"cell":"A2","kind":"binary"}],"nonNegative":false,"constraints":["C1<=4"]}})).unwrap();
    let before = Arc::clone(&s.doc().unwrap().wb);
    let preview = s.execute("solvercraft.model.exportLegacy", json!({"id":model["id"]})).unwrap();
    assert_eq!(preview["exportable"], true);
    assert_eq!(preview["applied"], false);
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    let applied = s.execute("solvercraft.model.exportLegacy", json!({"id":model["id"],"apply":true})).unwrap();
    assert_eq!(applied["applied"], true);
    assert!(s.doc().unwrap().wb.names.iter().any(|n| n.name == "solver_opt"));
    s.execute("edit.undo", json!({})).unwrap();
    assert!(!s.doc().unwrap().wb.names.iter().any(|n| n.name == "solver_opt"));
    s.execute("edit.redo", json!({})).unwrap();
    let imported = s.execute("solvercraft.model.importLegacy", json!({"name":"Imported typed model"})).unwrap();
    let validated = s.execute("solvercraft.validate", imported["request"].clone()).unwrap();
    assert_eq!(validated["expression_model"]["variables"][0]["kind"], "integer");
    assert_eq!(validated["expression_model"]["variables"][0]["lower"], -2.0);
    assert_eq!(validated["expression_model"]["variables"][1]["kind"], "binary");
    let solved = s.execute("solvercraft.solve", imported["request"].clone()).unwrap();
    assert_eq!(solved["result"]["objective"], 9.0);
    assert!(s.execute("solvercraft.model.get", json!({"id":model["id"]})).is_ok());
}
#[test]
fn solvercraft_legacy_export_reports_unsupported_models_without_overwriting_names() {
    let mut s = session();
    set(&mut s, "A1", "1");
    set(&mut s, "B1", "=A1*A1");
    let nonlinear = save(&mut s, "Quadratic");
    let before = Arc::clone(&s.doc().unwrap().wb);
    let report = s.execute("solvercraft.model.exportLegacy", json!({"id":nonlinear["id"]})).unwrap();
    assert_eq!(report["exportable"], false);
    assert!(!report["unsupported"].as_array().unwrap().is_empty());
    assert!(s.execute("solvercraft.model.exportLegacy", json!({"id":nonlinear["id"],"apply":true})).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    set(&mut s, "B1", "=A1");
    let inline = s
        .execute(
            "solvercraft.model.create",
            json!({"name":"Inline restriction","request":{"objective":"B1","variables":"A1","constraints":["A1+1<=5"]}}),
        )
        .unwrap();
    assert_eq!(s.execute("solvercraft.model.exportLegacy", json!({"id":inline["id"]})).unwrap()["exportable"], false);
}
#[test]
fn solvercraft_lp_basis_shadow_prices_intervals_and_warm_start_are_real() {
    let mut s = session();
    set(&mut s, "B1", "=3*A1");
    set(&mut s, "C1", "=2*A1");
    let request = json!({"objective":"B1","variables":"A1","constraints":["C1<=6"]});
    let report = s.execute("solvercraft.report.lpSensitivity", json!({"request":request})).unwrap();
    assert_eq!(report["detail"]["result"]["objective"], 9.0);
    assert!(!report["detail"]["basis"].is_null());
    assert!(
        report["detail"]["sensitivity"]["rhs_shadow_prices"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_f64)
            .any(|price| (price - 1.5).abs() < 1e-10)
    );
    let changed = json!({"objective":"B1","variables":"A1","constraints":["C1<=8"]});
    let warm = s.execute("solvercraft.report.lpSensitivity", json!({"request":changed,"warm_start":report["detail"]["basis"]})).unwrap();
    assert_eq!(warm["detail"]["result"]["objective"], 12.0);
    assert_eq!(warm["detail"]["warm_start_used"], true);
}
#[test]
fn solvercraft_closing_documents_releases_completed_job_reservations() {
    let _fixture = async_fixture();
    let mut s = session();
    for _ in 0..10 {
        set(&mut s, "B1", "=A1");
        let job = s.execute("solvercraft.job.start", json!({"request":request()})).unwrap();
        wait(&mut s, &job["id"]);
        let index = s.active_index();
        s.close_document(index);
        s.new_workbook();
    }
}
#[test]
fn solvercraft_psi_import_checks_individual_signatures_and_preserves_unknowns() {
    let mut s = session();
    set(&mut s, "A1", "=PsiTriangular(0,2,5)");
    set(&mut s, "A2", "=PsiTriangular(0,2,5,1)");
    set(&mut s, "A3", "=PsiUnknown(1)");
    let preview = s.execute("solvercraft.worksheet.importPsi", json!({})).unwrap();
    assert_eq!(preview["changes"].as_array().unwrap().len(), 1);
    assert_eq!(preview["unsupported"].as_array().unwrap().len(), 2);
    assert!(
        s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse("A1").unwrap()).unwrap().formula.as_ref().unwrap().text.contains("PSITRIANGULAR")
    );
    s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    assert!(
        s.doc()
            .unwrap()
            .wb
            .active()
            .unwrap()
            .cell(CellRef::parse("A1").unwrap())
            .unwrap()
            .formula
            .as_ref()
            .unwrap()
            .text
            .contains("SC.PSI.TRIANGULAR")
    );
    assert!(
        s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse("A2").unwrap()).unwrap().formula.as_ref().unwrap().text.contains("PSITRIANGULAR")
    );
    s.execute("edit.undo", json!({})).unwrap();
    assert!(
        s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse("A1").unwrap()).unwrap().formula.as_ref().unwrap().text.contains("PSITRIANGULAR")
    );
}
#[test]
fn solvercraft_run_manifests_keep_lossless_seed_effective_options_bindings_and_rerunnable_simulation() {
    let mut s = session();
    set(&mut s, "A1", "2");
    set(&mut s, "B1", "=2*A1");
    let seed = "18446744073709551615";
    let solved = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=3"],"options":{"seed":seed}})).unwrap();
    assert_eq!(solved["manifest"]["effective_options"]["solve"]["seed"], seed);
    assert_eq!(solved["manifest"]["backend"]["engine"], solved["result"]["engine"]);
    assert_eq!(solved["manifest"]["snapshot"], solved["snapshot"]);
    assert_eq!(solved["manifest"]["solvercraft_version"], ::solvercraft::VERSION);
    assert!(solved["manifest"]["platform"].as_str().unwrap().contains(std::env::consts::ARCH));
    assert!(solved["manifest"]["effective_options"]["solve"]["feasibility_tolerance"].is_number());
    assert_eq!(solved["manifest"]["bindings"]["variables"].as_array().unwrap().len(), 1);
    let preview=s.execute("solvercraft.simulate",json!({"inputs":[{"cell":"A1","distribution":{"kind":"uniform","low":0,"high":1}}],"outputs":"B1","options":{"seed":seed,"trials":12,"retain_samples":true}})).unwrap();
    assert_eq!(preview["manifest"]["effective_options"]["simulation"]["seed"], seed);
    assert_eq!(preview["request"]["options"]["seed"], seed);
    assert_eq!(preview["manifest"]["backend"]["sampling_version"], preview["result"]["sampling_version"]);
    assert_eq!(preview["manifest"]["bindings"]["inputs"][0]["cell"], "Sheet1!A1");
    assert_eq!(preview["manifest"]["bindings"]["outputs"][0], "Sheet1!B1");
    assert_eq!(preview["manifest"]["snapshot_time_1900_serial"], preview["request"]["_snapshot_time_serial"]);
    let repeated = s.execute("solvercraft.simulate", preview["request"].clone()).unwrap();
    assert_eq!(preview["result"], repeated["result"]);
    assert_eq!(preview["manifest"]["request_fingerprint"], repeated["manifest"]["request_fingerprint"]);
    let mut oversized = request();
    oversized["extra"] = json!("x".repeat(1024 * 1024));
    assert!(s.execute("solvercraft.solve", oversized).is_err());
}
#[test]
fn solvercraft_target_equality_preflights_caller_memory_before_copying_and_keeps_exact_semantics() {
    let mut s = session();
    set(&mut s, "A1", "0");
    let mut request = json!({"objective":"A1","goal":"min","variables":[{"cell":"A1","lower":0,"upper":10}]});
    let base = prepare(&s.doc().unwrap().wb, &request).unwrap().expression.unwrap();
    // One variable objective, its target-row copy, one target constant; shared
    // AST/variable/row estimate from the documented compiler contract.
    let exact = (3 * 160 + base.name.len() + base.variables[0].name.len() + "Target value".len() + 256 + 128) as u64;
    request["options"] = json!({"max_memory_bytes":exact-1});
    let low_options = SolveOptions { max_memory_bytes: exact - 1, ..SolveOptions::default() };
    ::solvercraft::compile::validate_expression_model(&base, &low_options).unwrap();
    request["goal"] = json!("value");
    request["value"] = json!(2.5);
    let before = Arc::clone(&s.doc().unwrap().wb);
    let error = s.execute("solvercraft.validate", request.clone()).unwrap_err();
    assert!(error.to_string().contains("memory"), "{error}");
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    // This isolates the pre-clone AST contract. The later compiled-model
    // validator/engine has its own additional allocation requirements.
    ::solvercraft::compile::preflight_target_named(&base, 2.5, "Target value", &SolveOptions { max_memory_bytes: exact, ..low_options }).unwrap();
    request["options"] = json!({});
    let checked = s.execute("solvercraft.validate", request.clone()).unwrap();
    assert_eq!(checked["expression_model"]["constraints"][0]["name"], "Target value");
    assert_eq!(checked["expression_model"]["constraints"][0]["relation"], "equal");
    assert_eq!(checked["expression_model"]["constraints"][0]["right"]["value"], 2.5);
    request["options"] = json!({});
    let preview = s.execute("solvercraft.solve", request).unwrap();
    assert_eq!(preview["result"]["objective"], 2.5);
    assert_eq!(preview["apply_values"][0], 2.5);
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn solvercraft_admission_pins_sheet_before_delayed_native_evaluation_and_nested_parameter_cases() {
    let _fixture = async_fixture();
    use std::sync::mpsc;
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=2*A1");
    set(&mut s, "C1", "10");
    s.execute("home.insertSheet", json!({"name":"Other"})).unwrap();
    set(&mut s, "A1", "9");
    set(&mut s, "B1", "=100*A1");
    set(&mut s, "C1", "100");
    s.execute("sheet.activate", json!({"sheet":"Sheet1"})).unwrap();
    let wb = Arc::clone(&s.doc().unwrap().wb);
    let calc = s.doc().unwrap().calc.clone();
    let captured = snapshot(&s).unwrap();
    let mut request = request();
    pin_request_sheet(&wb, &mut request, "optimization").unwrap();
    let pinned = request.clone();
    let worker_snapshot = captured.clone();
    let (ready_send, ready_receive) = mpsc::sync_channel(0);
    let (release_send, release_receive) = mpsc::sync_channel(0);
    let worker = std::thread::spawn(move || {
        ready_send.send(()).unwrap();
        release_receive.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        solve_snapshot(&wb, &calc, worker_snapshot, &request, Arc::new(NoControl)).unwrap()
    });
    ready_receive.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    s.execute("sheet.activate", json!({"sheet":"Other"})).unwrap();
    assert_eq!(snapshot(&s).unwrap().revision, captured.revision);
    release_send.send(()).unwrap();
    let result = worker.join().unwrap();
    assert_eq!(result["request"]["sheet"], "Sheet1");
    assert_eq!(result["bindings"][0]["sheet"], "Sheet1");
    assert_eq!(result["result"]["objective"], 10.0);
    assert_eq!(pinned["sheet"], result["request"]["sheet"]);
    let simulation = json!({"inputs":[{"cell":"A1","distribution":{"kind":"constant","value":1.0}}],"outputs":"B1","options":{"trials":2,"retain_samples":true},"parameter_cases":[{"name":"Base","values":[{"cell":"C1","value":10.0}]}]});
    let job = s.execute("solvercraft.job.start", json!({"kind":"simulation","request":simulation})).unwrap();
    s.execute("sheet.activate", json!({"sheet":"Sheet1"})).unwrap();
    let cases = wait(&mut s, &job["id"]);
    assert_eq!(cases["cases"][0]["preview"]["request"]["sheet"], "Other");
    assert_eq!(cases["cases"][0]["preview"]["result"]["summaries"][0]["mean"], 100.0);
    s.execute("solvercraft.job.forget", json!({"id":job["id"]})).unwrap();
    let mut null_sheet = json!({"sheet":null});
    pin_request_sheet(&s.doc().unwrap().wb, &mut null_sheet, "simulation").unwrap();
    assert_eq!(null_sheet["sheet"], "Sheet1");
    let mut invalid_sheet = json!({"sheet":42});
    assert!(pin_request_sheet(&s.doc().unwrap().wb, &mut invalid_sheet, "simulation").is_err());
    let mut portable = json!({"model":{},"sheet":42});
    pin_request_sheet(&s.doc().unwrap().wb, &mut portable, "optimization").unwrap();
    assert_eq!(portable["sheet"], 42);
    let mut simulation_metadata = json!({"model":{},"sheet":null});
    pin_request_sheet(&s.doc().unwrap().wb, &mut simulation_metadata, "simulation").unwrap();
    assert_eq!(simulation_metadata["sheet"], "Sheet1");
}

#[test]
fn result_sheet_archives_the_owned_snapshot_and_is_undoable() {
    let _fixture = async_fixture();
    let mut s = session();
    set(&mut s, "A1", "2");
    set(&mut s, "B1", "=3*A1");
    let job = s.execute("solvercraft.job.start", json!({"request":request()})).unwrap();
    let id = &job["id"];
    let preview = wait(&mut s, id);
    assert_eq!(preview["initial_values"], json!([2.0]));
    let mut other = session();
    assert!(other.execute("solvercraft.job.report", json!({"id":id})).is_err());
    s.execute("solvercraft.job.apply", json!({"id":id})).unwrap();
    let report = s.execute("solvercraft.job.report", json!({"id":id,"name":"Review"})).unwrap();
    assert_eq!(report["sheet"], "Review");
    let wb = &s.doc().unwrap().wb;
    let sheet = wb.sheet(wb.sheet_index("Review").unwrap()).unwrap();
    assert_eq!(sheet.value(CellRef::parse("A3").unwrap()), Value::text("Archived run: workbook has changed."));
    assert_eq!(sheet.value(CellRef::parse("C13").unwrap()), Value::number(2.0));
    assert_eq!(sheet.value(CellRef::parse("D13").unwrap()), Value::number(5.0));
    assert!(s.execute("solvercraft.job.report", json!({"id":id,"name":"Review"})).is_err());
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.doc().unwrap().wb.sheet_index("Review").is_none());
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::default()), Value::number(5.0));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::default()), Value::number(2.0));
}

#[test]
fn solvercraft_accepts_fixed_npv_coefficients_without_freezing_decision_cells() {
    let mut s = session();
    set(&mut s, "D1", "0.1");
    set(&mut s, "D2", "-20");
    set(&mut s, "D3", "44");
    set(&mut s, "B1", "=A1*NPV(D1,D2:D3)");
    let solved = s.execute("solvercraft.solve", request()).unwrap();
    let expected = 5.0 * (-20.0 / 1.1 + 44.0 / (1.1 * 1.1));
    assert!((solved["result"]["objective"].as_f64().unwrap() - expected).abs() < 1e-10);
    s.execute("solvercraft.result.apply", solved).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(CellRef::default()), Value::number(5.0));

    for formula in ["=NPV(D1,A1:D1)", "=NPV(D1,D2:D4)", "=NPV(D1,D2:D3+1)"] {
        set(&mut s, "B1", formula);
        let error = s.execute("solvercraft.validate", request()).unwrap_err();
        assert!(error.to_string().contains("NPV requires fixed numeric inputs"), "{error}");
    }
}

#[test]
fn solvercraft_legacy_import_uses_scoped_names_and_preserves_quoted_cross_sheet_bindings() {
    let mut s = session();
    s.execute("home.insertSheet", json!({"name":"Limits, Inc"})).unwrap();
    set(&mut s, "C1", "3");
    s.execute("home.insertSheet", json!({"name":"Model"})).unwrap();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=2*A1");
    let doc = s.doc_mut().unwrap();
    let wb = Arc::make_mut(&mut doc.wb);
    let model = wb.active_sheet();
    for (name, formula, scope) in [
        ("Cap", "'Limits, Inc'!$C$1", Some(model)),
        ("Cap", "Model!$Z$1", None),
        ("solver_opt", "Model!$B$1", Some(model)),
        ("solver_adj", "Model!$A$1", Some(model)),
        ("solver_num", "1", Some(model)),
        ("solver_lhs1", "Model!$A$1", Some(model)),
        ("solver_rel1", "1", Some(model)),
        ("solver_rhs1", "Cap", Some(model)),
        ("solver_eng", "2", Some(model)),
    ] {
        wb.names.push(gridcraft_model::DefinedName { name: name.into(), formula: formula.into(), scope, comment: String::new(), hidden: true });
    }
    let imported = s.execute("solvercraft.model.importLegacy", json!({})).unwrap();
    assert_eq!(imported["request"]["constraints"][0]["rhs"], "'Limits, Inc'!$C$1");
    let solved = s.execute("solvercraft.solve", imported["request"].clone()).unwrap();
    assert_eq!(solved["result"]["objective"], 6.0);
    Arc::make_mut(&mut s.doc_mut().unwrap().wb).names.retain(|name| name.name != "solver_rhs1");
    assert!(s.execute("solvercraft.model.importLegacy", json!({"name":"Incomplete"})).is_err());
}

#[test]
fn solvercraft_bond_price_coefficients_use_calc_and_cannot_depend_on_decisions() {
    let mut s = session();
    set(&mut s, "D1", "1000");
    set(&mut s, "D2", "40");
    set(&mut s, "D3", "=PRICE(DATE(2026,1,1),DATE(2028,1,1),D2/D1,0.07,100,1,0)*D1/100");
    set(&mut s, "B1", "=A1*D3");
    let solution = s.execute("solvercraft.solve", request()).unwrap();
    let expected = 5.0 * (40.0 / 1.07 + 1040.0 / 1.07_f64.powi(2));
    assert!((solution["result"]["objective"].as_f64().unwrap() - expected).abs() < 1e-9);
    s.execute("solvercraft.result.apply", solution).unwrap();
    set(&mut s, "B1", "=PRICE(DATE(2026,1,1),DATE(2028,1,1),A1,0.07,100,1,0)");
    let error = s.execute("solvercraft.validate", request()).unwrap_err();
    assert!(error.to_string().contains("Bond functions require fixed numeric inputs"), "{error}");
}

#[test]
fn psi_preview_marks_output_and_requires_fresh_snapshot_before_end_to_end_run() {
    let mut s = session();
    set(&mut s, "A1", "=PsiUniform(0,2)");
    set(&mut s, "B1", "=PsiOutput()+A1+A1");
    set(&mut s, "C1", "=A1+PsiOutput(\"label\")");
    set(&mut s, "D1", "=IF(TRUE,PsiOutput(),3)");
    let bytes = gridcraft_xlsx::write_xlsx(&s.doc().unwrap().wb).unwrap();
    let (workbook, _) = gridcraft_xlsx::read_xlsx(&bytes).unwrap();
    s.doc_mut().unwrap().wb = Arc::new(workbook);
    let command = "solvercraft.worksheet.importPsi";
    let before = super::snapshot(&s).unwrap().revision;
    let review = s.execute(command, json!({})).unwrap();
    assert_eq!(review["snapshot"]["revision"], before);
    assert_eq!(super::snapshot(&s).unwrap().revision, before);
    let changes = review["changes"].as_array().unwrap();
    let output = changes.iter().find(|row| row["cell"] == "Sheet1!B1").unwrap();
    assert_eq!(output["source_formula"], "=PSIOUTPUT()+A1+A1");
    assert_eq!(output["formula"], "=SC.OUTPUT(0+A1+A1)");
    assert_eq!(review["unsupported"].as_array().unwrap().len(), 2);
    assert_eq!(review["applied"], false);
    set(&mut s, "E1", "1");
    assert!(s.execute(command, json!({"apply":true,"expected_snapshot":review["snapshot"]})).is_err());
    let review = s.execute(command, json!({})).unwrap();
    let applied = s.execute(command, json!({"apply":true,"expected_snapshot":review["snapshot"]})).unwrap();
    assert_eq!(applied["changes"], review["changes"]);
    assert_eq!(applied["applied"], true);
    let discovered = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovered["outputs"], json!(["Sheet1!B1"]));
    let preview = s.execute("solvercraft.simulate", json!({"options":{"trials":8192,"seed":"340","retain_samples":true}})).unwrap();
    let mean = preview["result"]["summaries"][0]["mean"].as_f64().unwrap();
    let variance = preview["result"]["summaries"][0]["variance"].as_f64().unwrap();
    assert!((mean - 2.0).abs() < 0.06, "{mean}");
    assert!((variance - 4.0 / 3.0).abs() < 0.06, "{variance}");
    s.execute("edit.undo", json!({})).unwrap();
    let restored = s.execute(command, json!({})).unwrap();
    assert_eq!(restored["changes"], review["changes"]);
    assert_eq!(restored["unsupported"], review["unsupported"]);
}

#[test]
fn psi_import_converts_scalar_xll_cells_and_preserves_unsupported_post_run_statistics() {
    let mut s = session();
    set(&mut s, "A1", "=_xll.PsiTriangular(0,2,4,_xll.PsiBaseCase(3))");
    set(&mut s, "B1", "=_xll.PsiIntUniform(1,3)+_xll.PsiOutput()");
    set(&mut s, "C1", "=CHOOSE(B1,10,20,30)+A1+_xll.PsiOutput()");
    set(&mut s, "D1", "=_xll.PsiMean(C1)");
    set(&mut s, "E1", "=_xll.PsiTriangular(0,2,4,_xll.PsiBaseCase(3,1))");
    set(&mut s, "F1", "=5*B1");
    set(&mut s, "G1", "=_xll.PsiMean(F1)");
    let binding = CellRef::parse("A1").unwrap();
    let doc = s.doc_mut().unwrap();
    let wb = Arc::make_mut(&mut doc.wb);
    let sheet = wb.sheet_mut(0).unwrap();
    let mut cell = sheet.cell(binding).unwrap().clone();
    Arc::make_mut(cell.formula.as_mut().unwrap()).array = Some(gridcraft_core::RangeRef::parse("A1:A1").unwrap());
    sheet.set_cell(binding, cell);
    let marked = CellRef::parse("B1").unwrap();
    let mut cell = sheet.cell(marked).unwrap().clone();
    Arc::make_mut(cell.formula.as_mut().unwrap()).dynamic = true;
    sheet.set_cell(marked, cell);
    let report = s.execute("solvercraft.worksheet.importPsi", json!({"apply":true})).unwrap();
    assert_eq!(report["changes"].as_array().unwrap().len(), 3);
    assert_eq!(report["unsupported"].as_array().unwrap().len(), 3);
    assert!(s.doc().unwrap().wb.sheet(0).unwrap().cell(marked).unwrap().formula.as_ref().unwrap().dynamic);
    let discovered = s.execute("solvercraft.worksheet.discover", json!({})).unwrap();
    assert_eq!(discovered["outputs"], json!(["Sheet1!B1", "Sheet1!C1", "Sheet1!F1"]));
    assert_eq!(discovered["inputs"][0]["nominal"].as_f64(), Some(3.0));
    let result = s.execute("solvercraft.simulate", json!({"options":{"trials":4096,"seed":"840","retain_samples":true}})).unwrap();
    let mean = result["result"]["summaries"][1]["mean"].as_f64().unwrap();
    assert!((mean - 22.0).abs() < 0.7, "{mean}");
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.doc().unwrap().wb.sheet(0).unwrap().cell(binding).unwrap().formula.as_ref().unwrap().array.is_some());
}
