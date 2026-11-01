//! Versioned, host-independent model exchange. Host objects are opaque stable IDs.
use crate::{
    compile::{self, ExpressionModel},
    types::*,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PORTABLE_SCHEMA_VERSION: u32 = 2;
pub const MAX_PROJECT_BYTES: usize = 16 * 1024 * 1024;
const MAX_MODELS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelDefinition {
    Algebraic(OptimizationModel),
    Expression(ExpressionModel),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParameterBinding {
    pub parameter: usize,
    /// A stable ID supplied and interpreted by the host; never a pointer.
    pub object_id: String,
    pub property: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableModel {
    pub schema_version: u32,
    /// String IDs are preserved exactly, including numeric-looking IDs above 2^53.
    pub id: String,
    /// Canonical decimal revision, checked before updates.
    pub revision: String,
    pub name: String,
    pub definition: ModelDefinition,
    #[serde(default)]
    pub options: SolveOptions,
    #[serde(default)]
    pub bindings: Vec<ParameterBinding>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelLibrary {
    pub schema_version: u32,
    pub models: Vec<PortableModel>,
}
impl Default for ModelLibrary {
    fn default() -> Self {
        Self { schema_version: PORTABLE_SCHEMA_VERSION, models: vec![] }
    }
}
fn bad(message: impl Into<String>) -> SolverError {
    SolverError::new("invalid_project", message)
}
fn short(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
fn revision(value: &str) -> Result<u64> {
    let number = value.parse::<u64>().map_err(|_| bad("Revision must be a canonical decimal u64 string"))?;
    if number.to_string() != value {
        return Err(bad("Revision must be a canonical decimal u64 string"));
    }
    Ok(number)
}
impl PortableModel {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != PORTABLE_SCHEMA_VERSION {
            return Err(bad("Unsupported portable model schema"));
        }
        if !short(&self.id, 128) || !short(&self.name, 256) {
            return Err(bad("Model ID/name is empty, oversized or contains control characters"));
        }
        revision(&self.revision)?;
        if self.bindings.len() > 4096 || self.metadata.len() > 128 {
            return Err(bad("Too many bindings or metadata entries"));
        }
        let count = match &self.definition {
            ModelDefinition::Algebraic(model) => {
                crate::model::validate(model, &self.options)?;
                model.variables.len()
            }
            ModelDefinition::Expression(model) => {
                // Classification traverses all expressions and validates domains/handles
                // without executing an optimizer or assuming they are polynomials.
                compile::validate_expression_model(model, &self.options)?;
                model.variables.len()
            }
        };
        let mut bound = BTreeSet::new();
        for binding in &self.bindings {
            if binding.parameter >= count || !short(&binding.object_id, 256) || !short(&binding.property, 256) || !bound.insert(binding.parameter) {
                return Err(bad("Invalid or duplicate parameter binding"));
            }
        }
        if self.metadata.iter().any(|(k, v)| !short(k, 128) || v.len() > 4096) {
            return Err(bad("Invalid metadata size"));
        }
        Ok(())
    }
}
impl ModelLibrary {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != PORTABLE_SCHEMA_VERSION || self.models.len() > MAX_MODELS {
            return Err(bad("Invalid library schema or model count"));
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for model in &self.models {
            model.validate()?;
            if !ids.insert(model.id.as_str()) || !names.insert(model.name.to_lowercase()) {
                return Err(bad("Duplicate model ID or name"));
            }
        }
        Ok(())
    }
    pub fn insert(&mut self, model: PortableModel) -> Result<()> {
        self.validate()?;
        model.validate()?;
        if self.models.len() >= MAX_MODELS || self.models.iter().any(|m| m.id == model.id || m.name.to_lowercase() == model.name.to_lowercase()) {
            return Err(bad("Model quota, duplicate ID or duplicate name"));
        }
        self.models.try_reserve(1).map_err(|_| SolverError::new("memory_limit", "Cannot reserve model registry entry"))?;
        self.models.push(model);
        Ok(())
    }
    pub fn replace(&mut self, id: &str, expected_revision: &str, mut model: PortableModel) -> Result<()> {
        self.validate()?;
        model.validate()?;
        let index = self.models.iter().position(|m| m.id == id).ok_or_else(|| bad("Unknown model ID"))?;
        let prior = self.models.get(index).ok_or_else(|| bad("Missing model"))?;
        if prior.revision != expected_revision {
            return Err(SolverError::new("revision_conflict", "The saved model changed"));
        }
        if model.id != id || self.models.iter().enumerate().any(|(i, m)| i != index && m.name.to_lowercase() == model.name.to_lowercase()) {
            return Err(bad("Model ID changed or name collides"));
        }
        model.revision = revision(expected_revision)?.checked_add(1).ok_or_else(|| bad("Revision exhausted"))?.to_string();
        let slot = self.models.get_mut(index).ok_or_else(|| bad("Missing model"))?;
        *slot = model;
        Ok(())
    }
    pub fn remove(&mut self, id: &str, expected_revision: &str) -> Result<PortableModel> {
        self.validate()?;
        let index = self.models.iter().position(|m| m.id == id).ok_or_else(|| bad("Unknown model ID"))?;
        if self.models.get(index).is_none_or(|m| m.revision != expected_revision) {
            return Err(SolverError::new("revision_conflict", "The saved model changed"));
        }
        Ok(self.models.remove(index))
    }
}
struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl std::io::Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().checked_add(bytes.len()).is_none_or(|n| n > self.limit) {
            return Err(std::io::Error::other("Project byte limit exceeded"));
        }
        self.bytes.try_reserve(bytes.len()).map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn export(library: &ModelLibrary) -> Result<Vec<u8>> {
    library.validate()?;
    let mut output = BoundedWriter { bytes: vec![], limit: MAX_PROJECT_BYTES };
    serde_json::to_writer_pretty(&mut output, library).map_err(|e| SolverError::new("project_export", e.to_string()))?;
    Ok(output.bytes)
}
pub fn import(bytes: &[u8]) -> Result<ModelLibrary> {
    if bytes.len() > MAX_PROJECT_BYTES || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
        return Err(bad("Expected a bounded JSON object"));
    }
    let library: ModelLibrary = serde_json::from_slice(bytes).map_err(|e| bad(e.to_string()))?;
    library.validate()?;
    Ok(library)
}
/// Explicit migration from the published schema-1 algebraic model contract.
/// No invented workbook bindings, IDs, seeds or optimizer results are inferred.
pub fn migrate_v1_algebraic(bytes: &[u8], id: String, options: SolveOptions) -> Result<PortableModel> {
    if bytes.len() > MAX_PROJECT_BYTES || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
        return Err(bad("Expected a bounded schema-1 model object"));
    }
    let model: OptimizationModel = serde_json::from_slice(bytes).map_err(|e| bad(e.to_string()))?;
    crate::model::validate(&model, &options)?;
    let portable = PortableModel {
        schema_version: PORTABLE_SCHEMA_VERSION,
        id,
        revision: "0".into(),
        name: model.name.clone(),
        definition: ModelDefinition::Algebraic(model),
        options,
        bindings: vec![],
        metadata: BTreeMap::new(),
    };
    portable.validate()?;
    Ok(portable)
}
