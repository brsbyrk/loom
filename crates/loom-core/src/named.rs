//! Named configuration types — human-readable attribute references resolved to indices.
//!
//! The config author writes `"attribute": "health.stress"` instead of `"attribute_index": 5`.
//! The `resolve()` method converts to engine types using an `AttributeSchema`.
//!
//! # Usage
//!
//! ```ignore
//! // Load named configs from JSON, resolve to engine types via schema
//! let schema = AttributeSchema::from_path("schema.json")?;
//! let named: NamedDecision = serde_json::from_str(&json_str)?;
//! let decision: Decision = named.resolve(&schema)?;
//! ```

use crate::event::{
    AttributeEffect, ComparisonOp, Condition, Decision, Outcome, ScriptSource, Transform,
};
use crate::schema::AttributeSchema;
use crate::simulation::{Frequency, PassiveEffect};
use crate::scoring::{GoalVector, Threshold};
use serde::{Deserialize, Serialize};

// ── Resolve error ────────────────────────────────────────────────────────────────────

/// Error when a named attribute reference can't be found in the schema.
#[derive(Debug)]
pub struct ResolveError {
    pub name: String,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown attribute: '{}'", self.name)
    }
}

impl std::error::Error for ResolveError {}

// ── Helper: resolve an attribute name to an index ────────────────────────────────────

fn resolve_attr(schema: &AttributeSchema, name: &str) -> Result<usize, ResolveError> {
    schema
        .index_of(name)
        .ok_or_else(|| ResolveError { name: name.into() })
}

// ── NamedCondition ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedCondition {
    /// Attribute name (e.g., "wealth.cash").
    pub attribute: String,
    /// Comparison operator.
    pub operator: ComparisonOp,
    /// Threshold value.
    pub value: f64,
}

impl NamedCondition {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Condition, ResolveError> {
        Ok(Condition {
            attribute_index: resolve_attr(schema, &self.attribute)?,
            operator: self.operator,
            value: self.value,
        })
    }
}

// ── NamedEffect ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedEffect {
    /// Attribute name (e.g., "wealth.cash").
    pub attribute: String,
    /// Base additive delta.
    #[serde(default)]
    pub delta: f64,
    /// Optional state-dependent scaling: list of (attribute_name, multiplier).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scaling: Vec<(String, f64)>,
}

impl NamedEffect {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<AttributeEffect, ResolveError> {
        let attribute_index = resolve_attr(schema, &self.attribute)?;
        let scaling = self
            .scaling
            .iter()
            .map(|(name, mult)| resolve_attr(schema, name).map(|i| (i, *mult)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AttributeEffect {
            attribute_index,
            delta: self.delta,
            scaling,
        })
    }
}

// ── NamedTransform ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NamedTransform {
    /// Declarative transform with unconditional and conditional effects.
    Declarative {
        #[serde(default)]
        effects: Vec<NamedEffect>,
        #[serde(default)]
        conditional: Vec<(NamedCondition, Vec<NamedEffect>)>,
        #[serde(default)]
        default_conditional: Vec<NamedEffect>,
    },
    /// Scripted transform — opaque to the engine.
    Scripted {
        source: ScriptSource,
    },
}

impl NamedTransform {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Transform, ResolveError> {
        match self {
            NamedTransform::Declarative {
                effects,
                conditional,
                default_conditional,
            } => {
                let resolved_effects = effects
                    .iter()
                    .map(|e| e.resolve(schema))
                    .collect::<Result<Vec<_>, _>>()?;
                let resolved_conditional = conditional
                    .iter()
                    .map(|(cond, effs)| {
                        let c = cond.resolve(schema)?;
                        let e = effs
                            .iter()
                            .map(|e| e.resolve(schema))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok((c, e))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let resolved_default = default_conditional
                    .iter()
                    .map(|e| e.resolve(schema))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Transform::Declarative {
                    effects: resolved_effects,
                    conditional: resolved_conditional,
                    default_conditional: resolved_default,
                })
            }
            NamedTransform::Scripted { source } => Ok(Transform::Scripted {
                source: source.clone(),
            }),
        }
    }
}

// ── NamedOutcome ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedOutcome {
    /// Human-readable label.
    #[serde(default)]
    pub label: String,
    /// Relative weight for probabilistic sampling.
    pub weight: f64,
    /// Optional guard — this outcome is only valid when the condition holds.
    #[serde(default)]
    pub condition: Option<NamedCondition>,
    /// State transform applied when this outcome is selected.
    pub transform: NamedTransform,
}

impl NamedOutcome {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Outcome, ResolveError> {
        let condition = self
            .condition
            .as_ref()
            .map(|c| c.resolve(schema))
            .transpose()?;
        Ok(Outcome {
            label: self.label.clone(),
            weight: self.weight,
            condition,
            transform: self.transform.resolve(schema)?,
        })
    }
}

// ── NamedDecision ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedDecision {
    /// Unique identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Conditions that must all hold for this decision to be available.
    #[serde(default)]
    pub preconditions: Vec<NamedCondition>,
    /// Immediate deterministic cost paid before the outcome is sampled.
    #[serde(default)]
    pub cost: Vec<NamedEffect>,
    /// Weighted outcome branches.
    pub outcomes: Vec<NamedOutcome>,
}

impl NamedDecision {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Decision, ResolveError> {
        let preconditions = self
            .preconditions
            .iter()
            .map(|c| c.resolve(schema))
            .collect::<Result<Vec<_>, _>>()?;
        let cost = self
            .cost
            .iter()
            .map(|e| e.resolve(schema))
            .collect::<Result<Vec<_>, _>>()?;
        let outcomes = self
            .outcomes
            .iter()
            .map(|o| o.resolve(schema))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Decision {
            id: self.id.clone(),
            label: self.label.clone(),
            preconditions,
            cost,
            outcomes,
        })
    }
}

// ── NamedFrequency ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NamedFrequency {
    EveryStep,
    Every(usize),
    When(NamedCondition),
}

impl NamedFrequency {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Frequency, ResolveError> {
        match self {
            NamedFrequency::EveryStep => Ok(Frequency::EveryStep),
            NamedFrequency::Every(n) => Ok(Frequency::Every(*n)),
            NamedFrequency::When(cond) => Ok(Frequency::When(cond.resolve(schema)?)),
        }
    }
}

// ── NamedPassiveEffect ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedPassiveEffect {
    /// Unique identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// When this effect ticks.
    pub frequency: NamedFrequency,
    /// Deterministic attribute modifications applied when this effect fires.
    pub effects: Vec<NamedEffect>,
}

impl NamedPassiveEffect {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<PassiveEffect, ResolveError> {
        let effects = self
            .effects
            .iter()
            .map(|e| e.resolve(schema))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PassiveEffect {
            id: self.id.clone(),
            label: self.label.clone(),
            frequency: self.frequency.resolve(schema)?,
            effects,
        })
    }
}

// ── NamedGoalVector ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedGoalVector {
    /// Per-attribute weights, keyed by attribute name.
    pub weights: HashMap<String, f64>,
    /// Per-attribute cliff thresholds, keyed by attribute name.
    #[serde(default)]
    pub cliffs: HashMap<String, Threshold>,
}

use std::collections::HashMap;

impl NamedGoalVector {
    /// Resolve to a flat GoalVector matching the schema's attribute order.
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<GoalVector, ResolveError> {
        let dim = schema.dimension();
        let mut weights = vec![0.0; dim];
        let mut cliffs = vec![None; dim];

        for (name, &weight) in &self.weights {
            let idx = resolve_attr(schema, name)?;
            weights[idx] = weight;
        }
        for (name, threshold) in &self.cliffs {
            let idx = resolve_attr(schema, name)?;
            cliffs[idx] = Some(threshold.clone());
        }

        Ok(GoalVector { weights, cliffs })
    }
}

// ── Helper: resolve many ─────────────────────────────────────────────────────────────

impl NamedDecision {
    /// Convenience: load from JSON file and resolve.
    pub fn from_path(path: &str, schema: &AttributeSchema) -> Result<Decision, Box<dyn std::error::Error>> {
        let named: NamedDecision = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        Ok(named.resolve(schema)?)
    }
}

impl NamedPassiveEffect {
    /// Convenience: load a list from JSON file and resolve.
    pub fn list_from_path(
        path: &str,
        schema: &AttributeSchema,
    ) -> Result<Vec<PassiveEffect>, Box<dyn std::error::Error>> {
        let named: Vec<NamedPassiveEffect> =
            serde_json::from_str(&std::fs::read_to_string(path)?)?;
        named.iter().map(|n| n.resolve(schema)).collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

impl NamedGoalVector {
    /// Convenience: load from JSON file and resolve.
    pub fn from_path(
        path: &str,
        schema: &AttributeSchema,
    ) -> Result<GoalVector, Box<dyn std::error::Error>> {
        let named: NamedGoalVector = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        Ok(named.resolve(schema)?)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AttributeSchema;

    fn test_schema() -> AttributeSchema {
        AttributeSchema::from_json(
            r#"{
                "version": 1,
                "attributes": [
                    {"name": "wealth.cash", "unit": "$"},
                    {"name": "health.stress", "unit": "pts", "bounds": [0, 100]},
                    {"name": "skills.rust", "unit": "pts", "bounds": [0, 100]}
                ]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn resolve_condition_by_name() {
        let schema = test_schema();
        let named = NamedCondition {
            attribute: "health.stress".into(),
            operator: ComparisonOp::Gt,
            value: 50.0,
        };
        let cond = named.resolve(&schema).unwrap();
        assert_eq!(cond.attribute_index, 1);
        assert_eq!(cond.operator, ComparisonOp::Gt);
        assert_eq!(cond.value, 50.0);
    }

    #[test]
    fn resolve_effect_by_name() {
        let schema = test_schema();
        let named = NamedEffect {
            attribute: "wealth.cash".into(),
            delta: 1000.0,
            scaling: vec![],
        };
        let effect = named.resolve(&schema).unwrap();
        assert_eq!(effect.attribute_index, 0);
        assert_eq!(effect.delta, 1000.0);
    }

    #[test]
    fn resolve_effect_with_scaling_by_name() {
        let schema = test_schema();
        let named = NamedEffect {
            attribute: "health.stress".into(),
            delta: 10.0,
            scaling: vec![("health.stress".into(), 0.5)],
        };
        let effect = named.resolve(&schema).unwrap();
        assert_eq!(effect.attribute_index, 1);
        assert_eq!(effect.scaling, vec![(1, 0.5)]);
    }

    #[test]
    fn unknown_attribute_errors() {
        let schema = test_schema();
        let named = NamedCondition {
            attribute: "does.not.exist".into(),
            operator: ComparisonOp::Eq,
            value: 0.0,
        };
        assert!(named.resolve(&schema).is_err());
    }

    #[test]
    fn resolve_full_decision_json() {
        let schema = test_schema();
        let json = r#"{
            "id": "test_decision",
            "label": "Test",
            "preconditions": [
                {"attribute": "wealth.cash", "operator": "Gt", "value": 1000}
            ],
            "cost": [
                {"attribute": "wealth.cash", "delta": -500}
            ],
            "outcomes": [
                {
                    "label": "Good",
                    "weight": 70,
                    "transform": {
                        "type": "declarative",
                        "effects": [
                            {"attribute": "wealth.cash", "delta": 5000},
                            {"attribute": "health.stress", "delta": 10}
                        ]
                    }
                }
            ]
        }"#;
        let named: NamedDecision = serde_json::from_str(json).unwrap();
        let decision = named.resolve(&schema).unwrap();

        assert_eq!(decision.id, "test_decision");
        assert_eq!(decision.preconditions[0].attribute_index, 0);
        assert_eq!(decision.cost[0].attribute_index, 0);
        assert_eq!(decision.outcomes[0].label, "Good");
        assert_eq!(decision.outcomes[0].weight, 70.0);
    }

    #[test]
    fn resolve_named_goal_vector() {
        let schema = test_schema();
        let json = r#"{
            "weights": {
                "wealth.cash": 1.0,
                "health.stress": -0.3
            },
            "cliffs": {
                "health.stress": {"min": 30.0, "penalty": 1.0}
            }
        }"#;
        let named: NamedGoalVector = serde_json::from_str(json).unwrap();
        let goal = named.resolve(&schema).unwrap();

        assert_eq!(goal.dimension(), 3);
        assert!((goal.weights[0] - 1.0).abs() < f64::EPSILON); // wealth.cash
        assert!((goal.weights[1] + 0.3).abs() < f64::EPSILON); // health.stress
        assert_eq!(goal.weights[2], 0.0); // skills.rust — not set
        assert_eq!(goal.cliffs[1].as_ref().unwrap().min, 30.0);
    }

    #[test]
    fn resolve_named_passive() {
        let schema = test_schema();
        let json = r#"{
            "id": "stress_decay",
            "label": "Stress recovery",
            "frequency": {"type": "everystep"},
            "effects": [
                {"attribute": "health.stress", "delta": -2}
            ]
        }"#;
        let named: NamedPassiveEffect = serde_json::from_str(json).unwrap();
        let passive = named.resolve(&schema).unwrap();

        assert_eq!(passive.id, "stress_decay");
        assert_eq!(passive.effects[0].attribute_index, 1);
        assert_eq!(passive.effects[0].delta, -2.0);
    }
}
