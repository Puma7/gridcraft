//! Converts a bounded passive settings record into an ordinary explicit simulation request.
use std::collections::{HashMap, HashSet};

use ::solvercraft::types::{Distribution, SamplingMethod};

use super::super::{CellRef, Json, MAX_BINDINGS, Result, SOLVE, Session, Value, Workbook, bad, binding_name, json, sdk_error};

// Catalog identity for the supported settings schema. It is data only.
const CATALOG_ID: &str = "c83304d1-89d3-424f-bc8f-d9975f89ff33";

pub(super) fn import(session: &mut Session, _: &Json) -> Result<Json> {
    let wb = &session.doc()?.wb;
    let Some(properties) = gridcraft_xlsx::visible_web_extension_properties(wb, CATALOG_ID).map_err(sdk_error)? else {
        return Ok(json!({"request":null,"notes":[]}));
    };
    let request = decode(wb, properties)?;
    Ok(json!({
        "request": request,
        "notes": ["The sampling method and seed were imported. Solvercraft uses its own random generator and streams, so trials may differ from the source."]
    }))
}

fn property(properties: &mut HashMap<String, String>, key: &str) -> Result<String> {
    properties.remove(key).ok_or_else(|| bad(SOLVE, "A required embedded simulation setting is missing"))
}

fn quoted(properties: &mut HashMap<String, String>, key: &str) -> Result<String> {
    serde_json::from_str(&property(properties, key)?).map_err(|_| bad(SOLVE, "Invalid embedded simulation setting"))
}

fn decimal<T: std::str::FromStr>(text: &str) -> Result<T> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(bad(SOLVE, "Invalid embedded simulation number"));
    }
    text.parse().map_err(|_| bad(SOLVE, "Embedded simulation number is out of range"))
}

fn normal(formula: &str) -> Result<Distribution> {
    let invalid = || bad(SOLVE, "Embedded simulation currently supports only literal two-argument normal distributions");
    let (name, arguments) = formula.split_once('(').ok_or_else(invalid)?;
    if !name.eq_ignore_ascii_case("PsiNormal") {
        return Err(invalid());
    }
    let (mean, std_dev) = arguments.strip_suffix(')').and_then(|args| args.split_once(',')).ok_or_else(invalid)?;
    let distribution =
        Distribution::Normal { mean: mean.trim().parse().map_err(|_| invalid())?, std_dev: std_dev.trim().parse().map_err(|_| invalid())? };
    ::solvercraft::math::validate_distribution(&distribution).map_err(sdk_error)?;
    Ok(distribution)
}

fn decode(wb: &Workbook, mut properties: HashMap<String, String>) -> Result<Json> {
    let [sheet] = wb.sheets.as_slice() else {
        return Err(bad(SOLVE, "Embedded simulation cell references require a workbook with one worksheet"));
    };
    let input_count: usize = decimal(&property(&mut properties, "numDists")?)?;
    let output_count: usize = decimal(&property(&mut properties, "numOutputs")?)?;
    if !(1..=MAX_BINDINGS).contains(&input_count) || !(1..=MAX_BINDINGS).contains(&output_count) {
        return Err(bad(SOLVE, "Embedded simulation binding count is out of range"));
    }
    let trials: u64 = decimal(&quoted(&mut properties, "Trials")?)?;
    let seed: u64 = decimal(&quoted(&mut properties, "RandomSeed")?)?;
    if !(1..=1_000_000).contains(&trials) {
        return Err(bad(SOLVE, "Embedded simulation trial count is out of range"));
    }
    let method = match quoted(&mut properties, "SamplingMethod")?.as_str() {
        "LatinHyperCube" => SamplingMethod::LatinHypercube,
        _ => return Err(bad(SOLVE, "Unsupported embedded simulation sampling method")),
    };
    if quoted(&mut properties, "RandomGenerator")? != "CMRG" || quoted(&mut properties, "RandomStreams")? != "Single" {
        return Err(bad(SOLVE, "Unsupported embedded simulation random stream configuration"));
    }
    properties.remove("UniqueID");
    let mut seen = HashSet::new();
    let mut inputs = Vec::with_capacity(input_count);
    for index in 0..input_count {
        let entry = quoted(&mut properties, &format!("dist{index}"))?;
        let (address, formula) = entry.split_once(':').ok_or_else(|| bad(SOLVE, "Invalid embedded simulation input"))?;
        let cell = CellRef::parse(address).ok_or_else(|| bad(SOLVE, "Invalid embedded simulation input cell"))?;
        if !seen.insert(cell) {
            return Err(bad(SOLVE, "Embedded simulation input cells must be unique"));
        }
        if sheet.spill_anchor(cell).is_some()
            || !sheet.cell(cell).is_some_and(|entry| entry.formula.is_none() && matches!(entry.value, Value::Number(n) if n.is_finite()))
        {
            return Err(bad(SOLVE, "Embedded simulation inputs must be independent numeric cells"));
        }
        inputs.push(json!({"cell":binding_name(wb,0,cell)?,"distribution":normal(formula)?}));
    }
    seen.clear();
    let mut outputs = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let address = quoted(&mut properties, &format!("output{index}"))?;
        let cell = CellRef::parse(&address).ok_or_else(|| bad(SOLVE, "Invalid embedded simulation output cell"))?;
        if sheet.cell(cell).is_none() {
            return Err(bad(SOLVE, "An embedded simulation output cell is missing"));
        }
        if !seen.insert(cell) {
            return Err(bad(SOLVE, "Embedded simulation output cells must be unique"));
        }
        outputs.push(binding_name(wb, 0, cell)?);
    }
    if !properties.is_empty() {
        return Err(bad(SOLVE, "The workbook contains unsupported embedded simulation settings"));
    }
    Ok(json!({"inputs": inputs,"outputs":outputs,"options":{"trials":trials,"seed":seed.to_string(),"method":method}}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use gridcraft_model::{Preserved, PreservedPart, PreservedRel};

    use super::*;

    const PANE_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/webextensiontaskpanes";
    const EXTENSION_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/webextension";
    const COMMAND: &str = "solvercraft.worksheet.importSimulationSettings";

    fn embedded_session(extra_property: &str, external: bool) -> Session {
        let mut session = Session::new();
        session.new_workbook();
        session.execute("cell.set", json!({"cell":"A1","input":"4"})).unwrap();
        session.execute("cell.set", json!({"cell":"B1","input":"=A1*2+1"})).unwrap();
        let panes = r#"<taskpanes><taskpane visibility="1"><webextensionref id="r1"/></taskpane><taskpane visibility="0"><webextensionref id="r2"/></taskpane></taskpanes>"#;
        let properties = format!(
            r#"<webextension><reference id="{CATALOG_ID}"/><properties>
            <property name="numDists" value="1"/><property name="numOutputs" value="1"/>
            <property name="Trials" value="&quot;64&quot;"/><property name="RandomSeed" value="&quot;9007199254740993&quot;"/>
            <property name="SamplingMethod" value="&quot;LatinHyperCube&quot;"/>
            <property name="RandomGenerator" value="&quot;CMRG&quot;"/><property name="RandomStreams" value="&quot;Single&quot;"/>
            <property name="dist0" value="&quot;$A$1:PsiNormal(4,0.5)&quot;"/><property name="output0" value="&quot;$B$1&quot;"/>
            {extra_property}</properties></webextension>"#
        );
        let visible = "xl/webextensions/sample.xml";
        let pane_path = "xl/webextensions/panes.xml";
        let kept = Preserved {
            package_rels: vec![PreservedRel { rel_type: PANE_REL.into(), target: pane_path.into(), ..Default::default() }],
            parts: vec![
                PreservedPart {
                    name: pane_path.into(),
                    content_type: "application/vnd.ms-office.webextensiontaskpanes+xml".into(),
                    data: Arc::from(panes.as_bytes()),
                    rels: vec![
                        PreservedRel { id: "r1".into(), rel_type: EXTENSION_REL.into(), target: visible.into(), external },
                        // This hidden pane is deliberately invalid, and must never be followed.
                        PreservedRel { id: "r2".into(), rel_type: EXTENSION_REL.into(), target: "missing".into(), ..Default::default() },
                    ],
                },
                PreservedPart {
                    name: visible.into(),
                    content_type: "application/vnd.ms-office.webextension+xml".into(),
                    data: Arc::from(properties.into_bytes()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        Arc::make_mut(&mut session.doc_mut().unwrap().wb).preserved = Some(Arc::new(kept));
        session
    }

    #[test]
    fn reads_visible_settings_into_an_executable_draft_without_editing_cells() {
        let mut session = embedded_session("", false);
        let revision = session.doc().unwrap().revision;
        let imported = session.execute(COMMAND, json!({})).unwrap();
        assert_eq!(session.doc().unwrap().revision, revision);
        assert_eq!(imported["request"]["options"]["seed"], "9007199254740993");
        assert_eq!(imported["request"]["options"]["method"], "latin_hypercube");
        let run = session.execute("solvercraft.simulate", imported["request"].clone()).unwrap();
        assert_eq!(run["result"]["completed_trials"], 64);
        assert!((run["result"]["summaries"][0]["mean"].as_f64().unwrap() - 9.0).abs() < 0.2);
    }

    #[test]
    fn refuses_external_relationships_and_ambiguous_or_unknown_settings() {
        assert!(embedded_session("", true).execute(COMMAND, json!({})).is_err());
        let duplicate = r#"<property name="numDists" value="2"/>"#;
        assert!(embedded_session(duplicate, false).execute(COMMAND, json!({})).is_err());
        let unknown = r#"<property name="Correlation" value="1"/>"#;
        assert!(embedded_session(unknown, false).execute(COMMAND, json!({})).is_err());
    }
}
