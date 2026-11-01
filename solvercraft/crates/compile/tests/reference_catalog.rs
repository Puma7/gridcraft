//! Inventory integrity tests do not substitute for function parity tests.
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::collections::BTreeSet;
fn catalog() -> Value {
    serde_json::from_str(include_str!("../../../docs/implementation/reference-catalog.json")).unwrap()
}
#[test]
fn frozen_inventory_identifies_the_exact_source_and_retains_all_defined_entries() {
    let c = catalog();
    assert_eq!(c["schema_version"], 1);
    assert_eq!(c["sources"][0]["pages"], 570);
    assert_eq!(c["sources"][0]["bytes"], 19_434_011);
    assert_eq!(c["sources"][0]["sha256"], "2a166ac37367f2db870808d590c41ee8310c1676aa3a73164193e999d3d333c6");
    let functions = c["functions"].as_array().unwrap();
    assert_eq!(functions.len(), 272);
    let mut names = BTreeSet::new();
    let mut tests = BTreeSet::new();
    for f in functions {
        assert!(names.insert(f["name"].as_str().unwrap().to_ascii_lowercase()));
        assert!(tests.insert(f["acceptance"]["test_id"].as_str().unwrap()));
        assert!(!f["source_pages"].as_array().unwrap().is_empty());
        for page in f["source_pages"].as_array().unwrap() {
            let p = page.as_u64().unwrap();
            assert!((261..=389).contains(&p) || (410..=446).contains(&p));
        }
        assert!(!f["guarantee"].as_str().unwrap().is_empty());
        assert!(f["platforms"].is_object());
        assert!(!f["signatures"].as_array().unwrap().is_empty() || f["signature_documentation"] == "reference_describes_selector_arity_only");
    }
}
#[test]
fn unsupported_families_and_modeling_functions_are_not_deleted_to_improve_coverage() {
    let c = catalog();
    let functions = c["functions"].as_array().unwrap();
    for name in [
        "PsiMetalog",
        "PsiMetalogFit",
        "PsiMetalogSPT",
        "PsiMomentFit",
        "PsiMyerson",
        "PsiBurr12",
        "PsiFatigueLife",
        "PsiCumulD",
        "PsiAR1",
        "PsiAPARCH11",
        "PsiSigmaDefectShiftPPM",
        "PsiDimActive",
        "PsiTableCube",
        "PsiBoxIterator",
    ] {
        let f = functions.iter().find(|f| f["name"] == name).unwrap();
        assert_eq!(f["implementation"]["complete_reference_semantics"], false);
        assert_eq!(f["acceptance"]["status"], "open");
    }
    assert_eq!(c["summary"]["complete_reference_semantics_entries"], 0);
    assert_eq!(c["summary"]["open_acceptance_entries"], 272);
}
#[test]
fn literal_source_conflicts_and_distribution_parameter_differences_stay_visible() {
    let c = catalog();
    assert!(c["document_errata"].as_array().unwrap().len() >= 12);
    assert_eq!(c["unresolved_named_mentions"].as_array().unwrap().len(), 5);
    let f = c["functions"].as_array().unwrap();
    let find = |name| f.iter().find(|f| f["name"] == name).unwrap();
    assert!(find("PsiLogNormal")["implementation"]["sdk_mapping_note"].as_str().unwrap().contains("arithmetic-to-log"));
    assert!(find("PsiLogNorm2")["implementation"]["sdk_mapping_note"].as_str().unwrap().contains("log-mean"));
    assert!(find("PsiExponential")["implementation"]["sdk_mapping_note"].as_str().unwrap().contains("reciprocal"));
    assert_eq!(find("PsiTriangular")["implementation"]["psi_import"], "partial_exact_three_required_arguments");
}
#[test]
fn executable_matrix_has_each_frozen_function_and_keeps_missing_evidence_explicit() {
    let catalog = catalog();
    let matrix: Value = serde_json::from_str(include_str!("../../../docs/implementation/reference-implementation.json")).unwrap();
    let expected: BTreeSet<_> = catalog["functions"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
    let rows = matrix["functions"].as_array().unwrap();
    let actual: BTreeSet<_> = rows.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert_eq!(actual, expected);
    assert_eq!(rows.len(), 272);
    assert_eq!(rows.iter().filter(|r| r["r046_in_scope"] == true).count(), 111);
    for row in rows {
        assert_eq!(row["source"]["sha256"], catalog["sources"][0]["sha256"]);
        assert!(!row["planned_exact_signature_test"].as_str().unwrap().is_empty());
        assert!(row["platforms"].is_object());
        assert_eq!(row["guarantee"]["full_reference_parity"], false);
        for evidence in row["tests"].as_array().unwrap() {
            assert_eq!(evidence["source_file_sha256"].as_str().unwrap().len(), 64);
            let command = evidence["command"].as_array().unwrap();
            assert_eq!(command[0], "cargo");
            assert_eq!(command[1], "test");
            assert_eq!(command.last().unwrap(), "--exact");
            assert!(!evidence["execution"].as_str().unwrap().contains("PENDING"));
        }
    }
}
