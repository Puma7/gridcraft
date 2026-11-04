#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;

#[test]
fn structured_editing_retains_unknown_variable_and_condition_metadata() {
    let variables = json!([{"cell":"A1:B1","kind":"integer","lower":-4,"upper":9,"host":{"binding":"owned"}}]);
    assert_eq!(reference_rows(&variables.to_string()).unwrap(), variables.as_array().unwrap().clone());
    let condition = json!({"lhs":"'x=y'!A1","op":"<=","rhs":4,"host":{"rule":"owned"}});
    assert_eq!(condition_object(&condition), Some(condition));
    assert!(reference_rows("[true]").is_err());
    assert!(reference_rows("[[]]").is_err());
}

#[test]
fn condition_editor_uses_the_formula_parser_for_quoted_sheet_names() {
    let object = condition_object(&json!("'a=b'!A1 <= 7")).unwrap();
    assert_eq!(object["lhs"], "'a=b'!A1");
    assert_eq!(object["op"], "<=");
    assert_eq!(object["rhs"], "7");
}

#[test]
fn incomplete_models_and_pending_conditions_cannot_run() {
    let mut state = State::default();
    assert!(readiness(&state, false).is_some());
    state.objective = "C1".into();
    state.variables = "A1:B1".into();
    assert!(readiness(&state, false).is_none());
    state.constraint_left = "A1".into();
    assert!(readiness(&state, false).is_some());
    state.constraint_left.clear();
    state.variables = r#"[{"cell":"A1","lower":10,"upper":1}]"#.into();
    assert!(readiness(&state, false).is_some());
    state.kind = "simulation".into();
    state.inputs = r#"[{"cell":"A1","distribution":{"kind":"uniform","low":0,"high":1}}]"#.into();
    state.outputs = "C1".into();
    assert!(readiness(&state, false).is_none());
    state.seed = "18446744073709551616".into();
    assert!(readiness(&state, false).is_some());
}
