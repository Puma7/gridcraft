//! Solvercraft host boundaries: these tests use the real Gridcraft Session,
//! Workbook, Calc and command registry. They require a complete Gridcraft source
//! checkout and are not counted as executed by the standalone SDK test suite.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use gridcraft_core::{CellRef, Value};
use gridcraft_engine::Session;
use serde_json::{Value as Json, json};
use std::sync::Arc;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({})).unwrap();
    s
}
fn set(s: &mut Session, cell: &str, input: &str) {
    s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
}
fn value(s: &Session, sheet: &str, cell: &str) -> Value {
    let wb = &s.doc().unwrap().wb;
    wb.sheet(wb.sheet_index(sheet).unwrap()).unwrap().value(CellRef::parse(cell).unwrap())
}
fn constant_simulation(input: &str, output: &str, value: f64) -> Json {
    json!({
        "inputs":[{"cell":input,"distribution":{"kind":"constant","value":value}}],
        "outputs":output,"options":{"trials":4,"retain_samples":true,"seed":"0"}
    })
}

#[test]
fn explicit_negative_lower_bound_overrides_the_nonnegative_default() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    let preview = s
        .execute(
            "solvercraft.solve",
            json!({
                "objective":"B1","variables":"A1","goal":"min",
                "constraints":["A1 >= -2","A1 <= 1"]
            }),
        )
        .unwrap();
    assert_eq!(preview["result"]["objective"], json!(-2.0));
    assert_eq!(preview["result"]["values"], json!([-2.0]));
    // The preview remains read-only even when the chosen solution is negative.
    assert_eq!(value(&s, "Sheet1", "A1"), Value::number(0.0));
}

#[test]
fn omitted_lower_bounds_remain_nonnegative_and_false_allows_free_variables() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    let default = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1"})).unwrap();
    assert_eq!(default["model"]["variables"][0]["lower"], json!(0.0));
    let free = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1","nonNegative":false})).unwrap();
    assert!(free["model"]["variables"][0]["lower"].is_null());
}

#[test]
fn derived_spill_cells_are_not_decision_variables_or_simulation_inputs() {
    let mut s = session();
    set(&mut s, "A1", "=SEQUENCE(2)");
    set(&mut s, "B1", "=A2");
    assert!(s.doc().unwrap().wb.active().unwrap().spill_anchor(CellRef::parse("A2").unwrap()).is_some());
    let before = s.doc().unwrap().wb.clone();
    let solve = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A2"}));
    assert!(solve.unwrap_err().to_string().to_lowercase().contains("spill"));
    let simulate = s.execute("solvercraft.simulate", constant_simulation("A2", "B1", 7.0));
    assert!(simulate.unwrap_err().to_string().to_lowercase().contains("spill"));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}

#[test]
fn derived_spill_outputs_are_not_frozen_as_cached_objective_constants() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "C1", "=SEQUENCE(2,1,A1,1)");
    assert_eq!(value(&s, "Sheet1", "C2"), Value::number(1.0));
    let before = s.doc().unwrap().wb.clone();
    let objective = s.execute(
        "solvercraft.validate",
        json!({
            "objective":"C2","variables":[{"cell":"A1","lower":0,"upper":10}]
        }),
    );
    assert!(objective.unwrap_err().to_string().to_lowercase().contains("spill"));
    let output = s.execute("solvercraft.simulate", constant_simulation("A1", "C2", 5.0));
    assert!(output.unwrap_err().to_string().to_lowercase().contains("spill"));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}

#[test]
fn min_and_max_do_not_coerce_blank_references_to_zero() {
    for (function, input) in [("MIN", 5.0), ("MAX", -5.0)] {
        let mut s = session();
        set(&mut s, "A1", "1");
        set(&mut s, "B1", &format!("={function}(A1:A2)"));
        let before = s.doc().unwrap().wb.clone();
        // The ordered simulation IR now reuses Calc's aggregate provenance:
        // blank references are ignored, including for negative MAX inputs.
        let result = s.execute("solvercraft.simulate", constant_simulation("A1", "B1", input)).unwrap();
        assert_eq!(result["result"]["summaries"][0]["mean"], json!(input));
        assert!(result["result"]["samples"].as_array().unwrap().iter().all(|row| row == &json!([input])));
        // Optimization's mathematical compiler still rejects this case.
        let err = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1"})).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("blank"), "{err}");
        assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    }
}
#[test]
fn named_blank_min_keeps_calc_provenance_and_optimization_remains_explicit() {
    let mut s = session();
    set(&mut s, "A1", "1");
    s.execute("formulas.defineName", json!({"name":"BlankInput","refersTo":"=Sheet1!A2"})).unwrap();
    set(&mut s, "B1", "=MIN(A1,BlankInput)");
    let result = s.execute("solvercraft.simulate", constant_simulation("A1", "B1", 5.0)).unwrap();
    assert_eq!(result["result"]["summaries"][0]["mean"], json!(5.0));
    assert!(result["result"]["samples"].as_array().unwrap().iter().all(|row| row == &json!([5.0])));
    let error = s.execute("solvercraft.validate", json!({"objective":"B1","variables":"A1"})).unwrap_err();
    assert!(error.to_string().to_lowercase().contains("blank"));
}
#[test]
fn sum_keeps_its_distinct_blank_reference_semantics() {
    let mut s = session();
    set(&mut s, "A1", "1");
    set(&mut s, "B1", "=SUM(A1:A2)");
    let result = s.execute("solvercraft.simulate", constant_simulation("A1", "B1", 5.0)).unwrap();
    assert_eq!(result["result"]["summaries"][0]["mean"], json!(5.0));
}

#[test]
fn switching_active_sheet_does_not_rebind_the_preview_or_add_an_extra_undo() {
    let mut s = session();
    set(&mut s, "A1", "0");
    s.execute("home.insertSheet", json!({"name":"Other"})).unwrap();
    set(&mut s, "A1", "0");
    s.execute("sheet.activate", json!({"sheet":"Sheet1"})).unwrap();
    set(&mut s, "B1", "=3*A1+5*Other!A1");
    let preview = s
        .execute(
            "solvercraft.solve",
            json!({
                "objective":"B1","variables":["A1","Other!A1"],
                "constraints":["A1 <= 2","Other!A1 <= 3"]
            }),
        )
        .unwrap();
    assert_eq!(preview["result"]["objective"], json!(21.0));
    s.execute("sheet.activate", json!({"sheet":"Other"})).unwrap();
    s.execute("solvercraft.result.apply", preview).unwrap();
    assert_eq!(value(&s, "Sheet1", "A1"), Value::number(2.0));
    assert_eq!(value(&s, "Other", "A1"), Value::number(3.0));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(value(&s, "Sheet1", "A1"), Value::number(0.0));
    assert_eq!(value(&s, "Other", "A1"), Value::number(0.0));
}

#[test]
fn protected_target_refusal_does_not_change_cells_or_history() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    s.execute("review.protectSheet", json!({})).unwrap();
    let preview = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=1"]})).unwrap();
    let before = s.doc().unwrap().wb.clone();
    let error = s.execute("solvercraft.result.apply", preview).unwrap_err();
    assert!(error.to_string().to_lowercase().contains("protected"));
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
    // One undo still undoes sheet protection rather than a failed solution.
    s.execute("edit.undo", json!({})).unwrap();
    assert!(!s.doc().unwrap().wb.active().unwrap().is_protected());
    assert_eq!(value(&s, "Sheet1", "A1"), Value::number(0.0));
}

#[test]
fn stale_after_undo_and_different_document_snapshots_are_refused() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    let preview = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=1"]})).unwrap();
    set(&mut s, "C1", "7");
    s.execute("edit.undo", json!({})).unwrap();
    assert!(s.execute("solvercraft.result.apply", preview.clone()).is_err());
    s.execute("file.new", json!({})).unwrap();
    let before = s.doc().unwrap().wb.clone();
    assert!(s.execute("solvercraft.result.apply", preview).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn tampered_infeasible_values_are_revalidated_against_original_constraints() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=A1");
    let mut preview = s.execute("solvercraft.solve", json!({"objective":"B1","variables":"A1","constraints":["A1<=1"]})).unwrap();
    preview["result"]["values"] = json!([100.0]);
    preview["result"]["objective"] = json!(100.0);
    let before = s.doc().unwrap().wb.clone();
    assert!(s.execute("solvercraft.result.apply", preview).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
#[test]
fn smooth_narrow_bounds_are_checked_against_the_real_workbook_objective() {
    let mut s = session();
    set(&mut s, "A1", "0");
    set(&mut s, "B1", "=(10000000*A1-4)^2");
    let preview = s
        .execute(
            "solvercraft.solve",
            json!({
                "objective":"B1","variables":"A1","goal":"min","method":"projected_gradient",
                "constraints":["A1>=0","A1<=0.0000005"]
            }),
        )
        .unwrap();
    assert_eq!(preview["result"]["optimality"], json!("stationary"));
    assert!(preview["result"]["objective"].as_f64().unwrap() < 1e-18);
    s.execute("solvercraft.result.apply", preview).unwrap();
    let Value::Number(result) = value(&s, "Sheet1", "B1") else { panic!("expected numeric formula output") };
    assert!(result < 1e-18);
}
#[test]
fn mismatched_sumproduct_shapes_and_whole_column_bindings_fail_safely() {
    let mut s = session();
    set(&mut s, "A1", "0");
    let before = s.doc().unwrap().wb.clone();
    assert!(s.execute("solvercraft.validate", json!({"objective":"=SUMPRODUCT(A1:A2,B1:C1)","variables":"A1"})).is_err());
    assert!(s.execute("solvercraft.validate", json!({"objective":"A1","variables":"A:A"})).is_err());
    assert!(Arc::ptr_eq(&before, &s.doc().unwrap().wb));
}
