#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::Session;
use serde_json::json;
use solvercraft::types::NoControl;

fn session() -> Session {
    let mut session = Session::new();
    session.execute("file.new", json!({})).unwrap();
    session
}
fn set(session: &mut Session, cell: &str, input: &str) {
    session.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
}
fn binding(cell: &str) -> CellBinding {
    (0, CellRef::parse(cell).unwrap())
}
fn output(session: &Session, cell: &str) -> Value {
    session.doc().unwrap().wb.sheet(0).unwrap().value(CellRef::parse(cell).unwrap())
}
#[test]
fn solvercraft_ordered_simulation_matches_real_calc_for_every_translated_operation_and_100_seeded_inputs() {
    let mut actual = session();
    for (cell, input) in [
        ("A1", "1"),
        ("A2", "2"),
        ("A3", "3"),
        ("A5", "'4"),
        ("E1", "10000000000000000"),
        ("E2", "1"),
        ("E3", "-10000000000000000"),
        ("E4", "1"),
        ("E5", "2"),
    ] {
        set(&mut actual, cell, input);
    }
    let formulas = [
        "=SUM(1e16,A1,-1e16)",
        "=SUM(1e16,{-1e16,1})",
        "=SUM(A1:A3)",
        "=SUMPRODUCT(A1:A3,E1:E3)",
        "=MIN(A1:A4,TRUE,\"-3\")",
        "=MAX(A1:A4,TRUE,\"3\")",
        "=SIN(A1)+COS(A2)",
        "=EXP(A1/20)",
        "=LN(A2)",
        "=SQRT(A2)",
        "=ABS(A3)",
        "=+A1",
        "=-A1",
        "=A1%",
        "=A1^2",
        "=A1*A2/A2-A3",
        "=SC.OUTPUT(A1+A2,1/0)",
        "=IF(A1<A2,0.72,1)*IF(A1>=A3,0.6,1)",
        "=IF(A1=A2,1,0)+IF(A1<>A3,1,0)+IF(A3>A1,1,0)+IF(A3<=A1,1,0)",
        "=IF(A2>0,A1,1/0)",
        "=SUM(A1:A3,{TRUE,\"4\",5},TRUE,\"6\")",
        "=SUMPRODUCT({1e16,1,-1e16},{1,1,1})",
        "=SUM((A1:A2,A3:A4))",
        "=SUM(\"2\",TRUE,A4)",
        "=MIN(A4:A5)",
        "=SUMPRODUCT(A4:A5,E4:E5)",
        "=SUM(A5)",
        "=SUM(+A5)",
        "=SUM(1e16,A1:A3)",
        "=SUMPRODUCT(SIN(A1:A3),COS(A1:A3))",
        "=SUM((A1:A3+1)*2)",
        "=SUM({1,2;3,4}%)",
    ];
    let outputs = (0..formulas.len()).map(|row| (0, CellRef::new(row as u32, 1))).collect::<Vec<_>>();
    for (binding, formula) in outputs.iter().zip(formulas) {
        set(&mut actual, &binding.1.a1(), formula);
    }
    let frozen = Arc::clone(&actual.doc().unwrap().wb);
    let inputs = [binding("A1"), binding("A2"), binding("A3")];
    let mut program = Program::compile(&frozen, &inputs, &outputs, 46000.25, 128 * 1024 * 1024).unwrap();
    let mut state = 0x1234_5678_9876_abcd_u64;
    for trial in 0..100 {
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 11) as f64 / ((1_u64 << 53) as f64)
        };
        let values = if trial == 0 { [1.0, 1.0, 1.0] } else { [next() * 40.0 - 20.0, next() * 30.0 + 0.1, next() * 40.0 - 20.0] };
        for (binding, value) in inputs.iter().zip(values) {
            set(&mut actual, &binding.1.a1(), &value.to_string());
        }
        let simulated = program.evaluate(&values, &NoControl).unwrap();
        for ((binding, simulated), formula) in outputs.iter().zip(simulated).zip(formulas) {
            let expected = actual.doc().unwrap().wb.sheet(0).unwrap().value(binding.1);
            let Value::Number(expected) = expected else {
                panic!("Non-numeric reference {formula}: {expected:?}");
            };
            assert_eq!(simulated.to_bits(), expected.to_bits(), "trial {trial}, {formula}");
        }
    }
    assert_eq!(frozen.sheet(0).unwrap().value(inputs[0].1), Value::Number(1.0));
}
#[test]
fn solvercraft_ordered_simulation_preserves_domain_and_first_error_rules() {
    for formula in [
        "=LN(A1)^0",
        "=SQRT(A1)",
        "=0^0",
        "=1/0+LN(-1)",
        "=SUM(1e308,1e308,1/0)",
        "=SUM({1e308,1e308},1/0)",
        "=SUMPRODUCT(\"4\",A1)",
        "=MIN(A1,\"x\")",
    ] {
        let mut actual = session();
        set(&mut actual, "A1", "-1");
        set(&mut actual, "B1", formula);
        let expected = output(&actual, "B1");
        let Value::Error(expected) = expected else {
            panic!("Expected Calc error for {formula}: {expected:?}");
        };
        let frozen = Arc::clone(&actual.doc().unwrap().wb);
        let mut program = Program::compile(&frozen, &[binding("A1")], &[binding("B1")], 46000.0, 1024 * 1024).unwrap();
        let error = program.evaluate(&[-1.0], &NoControl).unwrap_err();
        assert!(error.message.contains(expected.as_str()), "{formula}: {error}");
    }
}
#[test]
fn solvercraft_ordered_simulation_retained_rows_use_sheet_rounding_and_leave_cells_untouched() {
    let mut actual = session();
    set(&mut actual, "A1", "1");
    set(&mut actual, "B1", "=SUM(1e16,A1,-1e16)");
    let before = Arc::clone(&actual.doc().unwrap().wb);
    let preview = actual.execute("solvercraft.simulate",json!({"inputs":[{"cell":"A1","distribution":{"kind":"constant","value":1.0}}],"outputs":"B1","options":{"trials":16,"retain_samples":true}})).unwrap();
    assert!(Arc::ptr_eq(&before, &actual.doc().unwrap().wb));
    assert_eq!(preview["result"]["summaries"][0]["mean"], 0.0);
    assert!(preview["result"]["samples"].as_array().unwrap().iter().all(|row| row == &json!([0.0])));
}
#[test]
fn solvercraft_ordered_simulation_rejects_hostile_expansion_and_unknown_cached_formulas() {
    let mut actual = session();
    set(&mut actual, "A1", "1");
    for formula in ["=SUM(A:A)", "=SUM(SEQUENCE(100000))", "=RAND()", "=INDIRECT(\"A1\")", "=SUMPRODUCT(A1:A300*B1:ZZ1)"] {
        set(&mut actual, "C1", formula);
        let frozen = Arc::clone(&actual.doc().unwrap().wb);
        assert!(Program::compile(&frozen, &[binding("A1")], &[binding("C1")], 46000.0, 128 * 1024 * 1024).is_err(), "{formula}");
    }
    set(&mut actual, "C1", "=SUM(A1:A20)");
    let frozen = Arc::clone(&actual.doc().unwrap().wb);
    assert!(Program::compile(&frozen, &[binding("A1")], &[binding("C1")], 46000.0, 1000).is_err());
    assert!(Program::compile(&frozen, &[binding("A1")], &[binding("C1")], 46000.0, 128 * 1024 * 1024).unwrap().evaluate(&[], &NoControl).is_err());
}

#[test]
fn solvercraft_ordered_simulation_resolves_relative_scoped_union_names_and_frozen_1904_time_like_calc() {
    use gridcraft_model::{Cell, DefinedName, Formula, Sheet};
    let mut workbook = Workbook::new();
    workbook.sheets.push(Arc::new(Sheet::new("Other")));
    workbook.date_system = gridcraft_core::DateSystem::D1904;
    for (sheet, cell, value) in [(0, "A1", 1.0), (0, "A2", 2.0), (1, "A1", 3.0)] {
        workbook.sheet_mut(sheet).unwrap().set_cell(CellRef::parse(cell).unwrap(), Cell::value(Value::Number(value)));
    }
    for (name, scope, formula) in
        [("Relative", None, "$A1"), ("Local", Some(1), "$A$1"), ("Bundle", None, "Sheet1!$A$1,Other!$A$1"), ("Chain", None, "SUM(Bundle)+1")]
    {
        workbook.names.push(DefinedName { name: name.into(), scope, formula: formula.into(), comment: String::new(), hidden: false });
    }
    let formulas = [("B2", "=Relative"), ("B3", "=Other!Local"), ("B4", "=SUM(Bundle)"), ("B5", "=Chain"), ("B6", "=NOW()+TODAY()")];
    for (cell, formula) in formulas {
        workbook.sheet_mut(0).unwrap().set_cell(CellRef::parse(cell).unwrap(), Cell::formula(Formula::new(formula)));
    }
    let mut calc = gridcraft_calc::Calc::new();
    calc.fixed_now = Some(46000.75);
    calc.rebuild(&workbook);
    calc.recalc_all(&mut workbook);
    let frozen = workbook.clone();
    let inputs = [binding("A1"), binding("A2"), (1, CellRef::parse("A1").unwrap())];
    let outputs = formulas.map(|(cell, _)| binding(cell));
    let mut program = Program::compile(&frozen, &inputs, &outputs, 46000.75, 1024 * 1024).unwrap();
    for values in [[4.0, 5.0, 6.0], [1e16, 1.0, -1e16], [-3.5, 0.0, 2.25]] {
        for ((sheet, cell), value) in inputs.iter().zip(values) {
            workbook.sheet_mut(*sheet).unwrap().set_cell(*cell, Cell::value(Value::Number(value)));
        }
        calc.cells_changed(&mut workbook, &inputs);
        let simulated = program.evaluate(&values, &NoControl).unwrap();
        for (output, simulated) in outputs.iter().zip(simulated) {
            assert_eq!(Value::Number(simulated), workbook.sheet(output.0).unwrap().value(output.1));
        }
    }
}
#[test]
fn solvercraft_ordered_simulation_checks_work_before_starting_trials_and_cancels_without_a_fake_row() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Cancel {
        calls: AtomicUsize,
    }
    impl solvercraft::types::RunControl for Cancel {
        fn is_cancelled(&self) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst) >= 4
        }
        fn progress(&self, _event: &solvercraft::types::ProgressEvent) {}
    }
    let mut actual = session();
    set(&mut actual, "A1", "1");
    set(&mut actual, "B1", "=SUM(A1:A20)");
    let frozen = Arc::clone(&actual.doc().unwrap().wb);
    let mut program = Program::compile(&frozen, &[binding("A1")], &[binding("B1")], 46000.0, 1024 * 1024).unwrap();
    assert!(program.check_work(u64::MAX).is_err());
    assert!(program.check_work(10).is_ok());
    let control = Cancel { calls: AtomicUsize::new(0) };
    let result = solvercraft::simulation::simulate(
        &[solvercraft::types::Distribution::Constant { value: 1.0 }],
        &solvercraft::types::SimulationOptions { trials: 10, retain_samples: true, ..Default::default() },
        &control,
        |inputs| program.evaluate(inputs, &control),
    )
    .unwrap();
    assert_eq!(result.termination, solvercraft::types::Termination::Cancelled);
    assert_eq!(result.completed_trials, 0);
    assert_eq!(result.samples, Some(Vec::new()));
    assert!(result.summaries.is_empty());
}
