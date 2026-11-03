//! Reading passive properties from the visible pane of a preserved workbook extension.
use std::collections::HashMap;

use gridcraft_model::{
    Workbook,
    preserved::{Preserved, PreservedPart, PreservedRel},
};

use crate::{
    IoError,
    xml::{self, El},
};

const PANE_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/webextensiontaskpanes";
const EXTENSION_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/webextension";
const PANE_CONTENT: &str = "application/vnd.ms-office.webextensiontaskpanes+xml";
const EXTENSION_CONTENT: &str = "application/vnd.ms-office.webextension+xml";
const MAX_PART_BYTES: usize = 256 * 1024;
const MAX_PANES: usize = 16;

fn invalid(message: &str) -> IoError {
    IoError::Format(message.into())
}

fn read_part<'a>(kept: &'a Preserved, rel: &PreservedRel, content_type: &str) -> Result<(&'a PreservedPart, El), IoError> {
    if rel.external {
        return Err(invalid("External add-in relationships cannot supply simulation settings"));
    }
    let mut parts = kept.parts.iter().filter(|part| part.name == rel.target);
    let part = parts.next().ok_or_else(|| invalid("Missing embedded add-in part"))?;
    if parts.next().is_some() || part.content_type != content_type || part.data.len() > MAX_PART_BYTES {
        return Err(invalid("Invalid or oversized embedded add-in part"));
    }
    Ok((part, xml::parse(&part.data)?))
}

/// Reads a single matching visible pane by its extension catalog identifier.
/// Resolves internal relationships from already preserved XLSX data and never opens
/// an external target, resolves a store URL, or activates an add-in.
pub fn visible_web_extension_properties(wb: &Workbook, catalog_id: &str) -> Result<Option<HashMap<String, String>>, IoError> {
    let Some(kept) = &wb.preserved else { return Ok(None) };
    let mut roots = kept.package_rels.iter().filter(|rel| rel.rel_type == PANE_REL);
    let Some(root) = roots.next() else { return Ok(None) };
    if roots.next().is_some() {
        return Err(invalid("Multiple embedded task pane parts are ambiguous"));
    }
    let (part, panes) = read_part(kept, root, PANE_CONTENT)?;
    if panes.name != "taskpanes" || panes.children.len() > MAX_PANES {
        return Err(invalid("Invalid embedded task pane list"));
    }
    let mut matched = None;
    for pane in panes.kids("taskpane").filter(|pane| pane.attr_bool("visibility") == Some(true)) {
        let mut refs = pane.kids("webextensionref");
        let id = refs.next().and_then(|reference| reference.attr("id")).ok_or_else(|| invalid("Visible add-in pane has no relationship"))?;
        let mut rels = part.rels.iter().filter(|rel| rel.id == id);
        let rel = rels.next().ok_or_else(|| invalid("Visible add-in relationship is missing"))?;
        if refs.next().is_some() || rels.next().is_some() || rel.rel_type != EXTENSION_REL {
            return Err(invalid("Invalid visible add-in relationship"));
        }
        let (_, extension) = read_part(kept, rel, EXTENSION_CONTENT)?;
        if extension.name != "webextension" {
            return Err(invalid("Invalid embedded add-in element"));
        }
        let mut references = extension.kids("reference");
        let reference = references.next();
        if references.next().is_some() {
            return Err(invalid("Ambiguous embedded add-in identifier"));
        }
        if !reference.and_then(|reference| reference.attr("id")).is_some_and(|id| id.eq_ignore_ascii_case(catalog_id)) {
            continue;
        }
        let mut containers = extension.kids("properties");
        let props = containers.next().ok_or_else(|| invalid("Embedded add-in properties are missing"))?;
        if containers.next().is_some() || matched.is_some() {
            return Err(invalid("Multiple matching embedded settings are ambiguous"));
        }
        let mut properties = HashMap::new();
        for property in props.kids("property") {
            let (Some(key), Some(value)) = (property.attr("name"), property.attr("value")) else {
                return Err(invalid("Invalid embedded add-in property"));
            };
            if properties.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err(invalid("Duplicate embedded add-in property"));
            }
        }
        matched = Some(properties);
    }
    Ok(matched)
}
