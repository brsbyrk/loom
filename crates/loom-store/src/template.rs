//! Template system — complete domain presets as serializable config bundles.
//! A template is everything needed to define a decision domain: schema,
//! decisions, passives, goals, and events.

use loom_core::{AttributeSchema, NamedDecision, NamedGoalVector, NamedPassiveEffect};
use serde::{Deserialize, Serialize};

/// A complete domain preset — load, preview, seed in one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Human-readable name for the template picker.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Full attribute schema (version + attributes).
    pub schema: AttributeSchema,
    /// All decisions for this domain.
    #[serde(default)]
    pub decisions: Vec<NamedDecision>,
    /// All passive effects.
    #[serde(default)]
    pub passives: Vec<NamedPassiveEffect>,
    /// Named goal vectors (keyed by goal name).
    #[serde(default)]
    pub goals: std::collections::HashMap<String, NamedGoalVector>,
    /// Event templates.
    #[serde(default)]
    pub events: Vec<crate::NamedEvent>,
}

impl Template {
    /// Load a template from a JSON file.
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Load a template from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
