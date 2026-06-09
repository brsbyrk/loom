//! Attribute schema — defines the structure, grouping, and metadata of the state vector.
//!
//! Loaded from JSON at runtime. The engine never touches this directly — it operates on
//! raw `&[f64]`. Consumers (TUI, examples, domain crates) use the schema to interpret
//! what the numbers mean.
//!
//! # Schema format (JSON)
//!
//! ```json
//! {
//!   "version": 1,
//!   "attributes": [
//!     {"name": "wealth.cash", "group": "wealth", "unit": "$", "kind": "continuous"},
//!     {"name": "health.physical", "group": "health", "unit": "pts", "bounds": [0, 100]},
//!     {"name": "trait_ambitious", "kind": "boolean"}
//!   ]
//! }
//! ```
//!
//! `kind` defaults to `"continuous"` when omitted. Boolean attributes are
//! treated as 0.0/1.0 flags — the engine operates on f64 regardless.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Complete attribute space definition.
///
/// Ordinal position in the `attributes` vector is the index used by the engine
/// (i.e., `attributes[0]` is index 0 in the state vector).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributeSchema {
    /// Schema format version (currently 1).
    #[serde(default = "default_version")]
    pub version: u32,

    /// All attributes in index order. The position in this vector IS the engine index.
    pub attributes: Vec<AttributeDef>,
}

fn default_version() -> u32 {
    1
}

/// Whether an attribute is a continuous value (default) or a boolean flag.
///
/// The engine operates on `f64` regardless — this is metadata for the TUI and
/// config layer. Boolean attributes are conventionally 0.0 (false) or 1.0 (true)
/// and pair naturally with `ComparisonOp::Eq`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttributeKind {
    /// Unbounded real value (default).
    #[default]
    Continuous,
    /// Boolean flag: 0.0 = false, 1.0 = true.
    Boolean,
}

/// One attribute slot in the state vector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributeDef {
    /// Unique name (e.g., "wealth.cash", "health.physical", "skills.rust").
    /// Dotted names imply a hierarchy: "wealth.cash" is "cash" under "wealth".
    pub name: String,

    /// Optional logical group for aggregation (e.g., "wealth", "health", "skills").
    /// When `None`, derived from the first segment of a dotted `name`.
    #[serde(default)]
    pub group: Option<String>,

    /// Display unit (e.g., "$", "hrs", "pts", "%").
    #[serde(default)]
    pub unit: Option<String>,

    /// Clamping bounds: (min, max). When set, the attribute cannot leave this range.
    /// The engine applies clamping automatically during simulation.
    #[serde(default)]
    pub bounds: Option<(f64, f64)>,

    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Value kind — continuous (default) or boolean.
    /// The engine ignores this; it is metadata for the TUI and config layer.
    #[serde(default)]
    pub kind: AttributeKind,
}

impl AttributeDef {
    /// Returns the effective group: explicit `group` if set, otherwise the first
    /// segment of a dotted `name` (e.g., "health" from "health.physical").
    pub fn effective_group(&self) -> &str {
        self.group
            .as_deref()
            .unwrap_or_else(|| self.name.split('.').next().unwrap_or(&self.name))
    }
}

impl AttributeSchema {
    /// Load a schema from a JSON file path.
    pub fn from_path(path: &str) -> Result<Self, SchemaError> {
        let content = std::fs::read_to_string(path).map_err(SchemaError::Io)?;
        serde_json::from_str(&content).map_err(SchemaError::Parse)
    }

    /// Load a schema from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, SchemaError> {
        serde_json::from_str(json).map_err(SchemaError::Parse)
    }

    /// Number of attributes (dimensionality of the state vector).
    pub fn dimension(&self) -> usize {
        self.attributes.len()
    }

    /// Look up an attribute's index by name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.attributes.iter().position(|a| a.name == name)
    }

    /// Look up an attribute definition by index.
    pub fn at(&self, index: usize) -> Option<&AttributeDef> {
        self.attributes.get(index)
    }

    /// All indices belonging to a logical group.
    pub fn group_indices(&self, group_name: &str) -> Vec<usize> {
        self.attributes
            .iter()
            .enumerate()
            .filter(|(_, a)| a.effective_group() == group_name)
            .map(|(i, _)| i)
            .collect()
    }

    /// All distinct group names.
    pub fn group_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .attributes
            .iter()
            .map(|a| a.effective_group())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Build a name → index lookup map (cached for performance).
    pub fn name_map(&self) -> HashMap<String, usize> {
        self.attributes
            .iter()
            .enumerate()
            .map(|(i, a)| (a.name.clone(), i))
            .collect()
    }
}

/// Schema loading errors.
#[derive(Debug)]
pub enum SchemaError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::Io(e) => write!(f, "I/O error: {e}"),
            SchemaError::Parse(e) => write!(f, "JSON parse error: {e}"),
        }
    }
}

impl std::error::Error for SchemaError {}

// ── Dynamic state ────────────────────────────────────────────────────────────────────

/// The engine's universal state container — schema-driven, no hardcoded domain types.
///
/// Internally wraps a `Vec<f64>` (the engine's native format) plus a shared schema
/// reference for display and named access. Cloning is cheap (the schema is `Arc`-wrapped).
#[derive(Debug, Clone)]
pub struct DynamicState {
    values: Vec<f64>,
    schema: Arc<AttributeSchema>,
}

impl DynamicState {
    /// Create a new state with all attributes initialized to zero.
    pub fn new(schema: Arc<AttributeSchema>) -> Self {
        let dim = schema.dimension();
        Self {
            values: vec![0.0; dim],
            schema,
        }
    }

    /// Create a state from a raw float vector. Panics if the length doesn't match the schema.
    pub fn from_vec(values: Vec<f64>, schema: Arc<AttributeSchema>) -> Self {
        assert_eq!(
            values.len(),
            schema.dimension(),
            "DynamicState length {} does not match schema dimension {}",
            values.len(),
            schema.dimension()
        );
        Self { values, schema }
    }

    /// Access the schema.
    pub fn schema(&self) -> &AttributeSchema {
        &self.schema
    }

    /// Named read: get the value of an attribute by name. Returns `None` if unknown.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.schema.index_of(name).map(|i| self.values[i])
    }

    /// Named write: set the value of an attribute by name. Returns `None` if unknown.
    pub fn set(&mut self, name: &str, value: f64) -> Option<()> {
        self.schema
            .index_of(name)
            .map(|i| {
                self.values[i] = value;
            })
    }

    /// Apply clamping to all attributes that have bounds defined.
    pub fn clamp(&mut self) {
        for (i, attr) in self.schema.attributes.iter().enumerate() {
            if let Some((min, max)) = attr.bounds {
                self.values[i] = self.values[i].clamp(min, max);
            }
        }
    }

    /// Access the raw float slice (for engine operations).
    pub fn as_slice(&self) -> &[f64] {
        &self.values
    }

    /// Mutable access to the raw float slice (for engine operations).
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.values
    }
}

// Implement Deref so DynamicState can be passed where &[f64] is expected.
impl std::ops::Deref for DynamicState {
    type Target = [f64];

    fn deref(&self) -> &[f64] {
        &self.values
    }
}

impl std::ops::DerefMut for DynamicState {
    fn deref_mut(&mut self) -> &mut [f64] {
        &mut self.values
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> AttributeSchema {
        AttributeSchema {
            version: 1,
            attributes: vec![
                AttributeDef {
                    name: "wealth.cash".into(),
                    group: None,
                    unit: Some("$".into()),
                    bounds: None,
                    description: None,
                    kind: AttributeKind::Continuous,
                },
                AttributeDef {
                    name: "health.physical".into(),
                    group: None,
                    unit: Some("pts".into()),
                    bounds: Some((0.0, 100.0)),
                    description: None,
                    kind: AttributeKind::Continuous,
                },
                AttributeDef {
                    name: "time_free".into(),
                    group: Some("resources".into()),
                    unit: Some("hrs".into()),
                    bounds: None,
                    description: None,
                    kind: AttributeKind::Continuous,
                },
            ],
        }
    }

    #[test]
    fn effective_group_derives_from_dotted_name() {
        let schema = test_schema();
        assert_eq!(schema.attributes[0].effective_group(), "wealth");
        assert_eq!(schema.attributes[1].effective_group(), "health");
    }

    #[test]
    fn effective_group_respects_explicit() {
        let schema = test_schema();
        assert_eq!(schema.attributes[2].effective_group(), "resources");
    }

    #[test]
    fn index_of() {
        let schema = test_schema();
        assert_eq!(schema.index_of("wealth.cash"), Some(0));
        assert_eq!(schema.index_of("health.physical"), Some(1));
        assert_eq!(schema.index_of("nonexistent"), None);
    }

    #[test]
    fn group_indices() {
        let schema = test_schema();
        assert_eq!(schema.group_indices("wealth"), vec![0]);
        assert_eq!(schema.group_indices("health"), vec![1]);
        assert_eq!(schema.group_indices("resources"), vec![2]);
    }

    #[test]
    fn group_names() {
        let schema = test_schema();
        let mut names = schema.group_names();
        names.sort();
        assert_eq!(names, vec!["health", "resources", "wealth"]);
    }

    #[test]
    fn dynamic_state_named_access() {
        let schema = Arc::new(test_schema());
        let mut state = DynamicState::new(schema);
        state.set("wealth.cash", 50000.0);
        state.set("health.physical", 75.0);
        assert_eq!(state.get("wealth.cash"), Some(50000.0));
        assert_eq!(state.get("health.physical"), Some(75.0));
        assert_eq!(state.get("nonexistent"), None);
    }

    #[test]
    fn dynamic_state_clamping() {
        let schema = Arc::new(test_schema());
        let mut state = DynamicState::from_vec(vec![0.0, 150.0, 0.0], schema);
        state.clamp();
        assert_eq!(state[1], 100.0); // clamped from 150
    }

    #[test]
    fn from_json_round_trip() {
        let json = r#"{
            "version": 1,
            "attributes": [
                {"name": "a", "group": "g", "unit": "u", "bounds": [0, 100]},
                {"name": "b"}
            ]
        }"#;
        let schema = AttributeSchema::from_json(json).unwrap();
        let re_json = serde_json::to_string_pretty(&schema).unwrap();
        let schema2 = AttributeSchema::from_json(&re_json).unwrap();
        assert_eq!(schema2.dimension(), 2);
        assert_eq!(schema2.attributes[0].name, "a");
        assert_eq!(schema2.attributes[0].group.as_deref(), Some("g"));
    }
}
