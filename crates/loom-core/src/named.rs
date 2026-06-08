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

/// Error when a named reference can't be resolved.
#[derive(Debug)]
pub enum ResolveError {
    /// Referenced attribute name not found in schema.
    UnknownAttribute(String),
    /// Referenced group name not found or has no attributes.
    UnknownGroup(String),
    /// Neither attribute nor group was specified.
    NoTarget,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::UnknownAttribute(name) => write!(f, "unknown attribute: '{name}'"),
            ResolveError::UnknownGroup(name) => write!(f, "unknown or empty group: '{name}'"),
            ResolveError::NoTarget => write!(f, "effect must specify 'attribute' or 'group'"),
        }
    }
}

impl std::error::Error for ResolveError {}

// ── Helper: resolve an attribute name to an index ────────────────────────────────────

fn resolve_attr(schema: &AttributeSchema, name: &str) -> Result<usize, ResolveError> {
    schema
        .index_of(name)
        .ok_or_else(|| ResolveError::UnknownAttribute(name.into()))
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

/// A named attribute effect — can target a single attribute or an entire group.
///
/// Either `attribute` or `group` must be set. If `group` is set, the effect is
/// duplicated for every attribute in that group. The `delta` and `scaling` are
/// applied identically to each target.
///
/// # Examples (JSON)
///
/// ```json
/// {"attribute": "wealth.cash", "delta": 5000}
/// ```
/// → affects only wealth.cash.
///
/// ```json
/// {"group": "wealth", "delta": -500}
/// ```
/// → affects wealth.cash, wealth.stocks, wealth.house_value, wealth.debt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedEffect {
    /// Attribute name (e.g., "wealth.cash"). Set for single-attribute targeting.
    #[serde(default)]
    pub attribute: Option<String>,
    /// Group name (e.g., "wealth"). Set to target all attributes in a group.
    #[serde(default)]
    pub group: Option<String>,
    /// Base additive delta.
    #[serde(default)]
    pub delta: f64,
    /// Optional state-dependent scaling: list of (attribute_name, multiplier).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scaling: Vec<(String, f64)>,
}

impl NamedEffect {
    /// Create a simple fixed-delta effect on a single attribute.
    pub fn fixed(attribute: impl Into<String>, delta: f64) -> Self {
        Self {
            attribute: Some(attribute.into()),
            group: None,
            delta,
            scaling: vec![],
        }
    }

    /// Create a simple fixed-delta effect on an entire group.
    pub fn group_fixed(group: impl Into<String>, delta: f64) -> Self {
        Self {
            attribute: None,
            group: Some(group.into()),
            delta,
            scaling: vec![],
        }
    }

    /// Resolve to one or more engine-level `AttributeEffect`s.
    ///
    /// If `group` is set, returns one effect per attribute in the group.
    /// If `attribute` is set, returns a single effect.
    /// If neither is set, returns `ResolveError::NoTarget`.
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<Vec<AttributeEffect>, ResolveError> {
        // Resolve scaling sources once (same for all targets)
        let scaling: Vec<(usize, f64)> = self
            .scaling
            .iter()
            .map(|(name, mult)| resolve_attr(schema, name).map(|i| (i, *mult)))
            .collect::<Result<Vec<_>, _>>()?;

        let indices: Vec<usize> = if let Some(ref group) = self.group {
            let idxs = schema.group_indices(group);
            if idxs.is_empty() {
                return Err(ResolveError::UnknownGroup(group.clone()));
            }
            idxs
        } else if let Some(ref attr) = self.attribute {
            vec![resolve_attr(schema, attr)?]
        } else {
            return Err(ResolveError::NoTarget);
        };

        Ok(indices
            .into_iter()
            .map(|attribute_index| AttributeEffect {
                attribute_index,
                delta: self.delta,
                scaling: scaling.clone(),
            })
            .collect())
    }
}

/// Resolve a slice of `NamedEffect`s, flattening group expansions into a flat
/// `Vec<AttributeEffect>`.
fn resolve_effects(
    effects: &[NamedEffect],
    schema: &AttributeSchema,
) -> Result<Vec<AttributeEffect>, ResolveError> {
    effects
        .iter()
        .map(|e| e.resolve(schema))
        .collect::<Result<Vec<Vec<_>>, _>>()
        .map(|v| v.into_iter().flatten().collect())
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
                let resolved_effects = resolve_effects(effects, schema)?;
                let resolved_conditional = conditional
                    .iter()
                    .map(|(cond, effs)| {
                        let c = cond.resolve(schema)?;
                        let e = resolve_effects(effs, schema)?;
                        Ok((c, e))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let resolved_default = resolve_effects(default_conditional, schema)?;
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
        let cost = resolve_effects(&self.cost, schema)?;
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
        let effects = resolve_effects(&self.effects, schema)?;
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

// ── Named decision schedules ────────────────────────────────────────────────────────

use crate::event::{DecisionSchedule, ScheduledDecision};

/// Named version of [`ScheduledDecision`] — uses attribute names, resolved via schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedScheduledDecision {
    /// Absolute step index in the horizon.
    pub at_step: usize,
    /// The decision to apply at this step (named form).
    pub decision: NamedDecision,
    /// If true, abort the run when preconditions fail.
    #[serde(default)]
    pub required: bool,
}

impl NamedScheduledDecision {
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<ScheduledDecision, ResolveError> {
        Ok(ScheduledDecision {
            at_step: self.at_step,
            decision: self.decision.resolve(schema)?,
            required: self.required,
        })
    }
}

/// Named version of [`DecisionSchedule`] — resolved to engine types via schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NamedDecisionSchedule {
    /// Ordered list of scheduled decisions.
    pub entries: Vec<NamedScheduledDecision>,
}

impl NamedDecisionSchedule {
    /// Resolve to engine-level DecisionSchedule.
    pub fn resolve(&self, schema: &AttributeSchema) -> Result<DecisionSchedule, ResolveError> {
        let entries = self
            .entries
            .iter()
            .map(|e| e.resolve(schema))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DecisionSchedule { entries })
    }

    /// Convenience: load from JSON file and resolve.
    pub fn from_path(
        path: &str,
        schema: &AttributeSchema,
    ) -> Result<DecisionSchedule, Box<dyn std::error::Error>> {
        let named: NamedDecisionSchedule =
            serde_json::from_str(&std::fs::read_to_string(path)?)?;
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
            attribute: Some("wealth.cash".into()),
            group: None,
            delta: 1000.0,
            scaling: vec![],
        };
        let effects = named.resolve(&schema).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].attribute_index, 0);
        assert_eq!(effects[0].delta, 1000.0);
    }

    #[test]
    fn resolve_effect_with_scaling_by_name() {
        let schema = test_schema();
        let named = NamedEffect {
            attribute: Some("health.stress".into()),
            group: None,
            delta: 10.0,
            scaling: vec![("health.stress".into(), 0.5)],
        };
        let effects = named.resolve(&schema).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].attribute_index, 1);
        assert_eq!(effects[0].scaling, vec![(1, 0.5)]);
    }

    #[test]
    fn resolve_effect_by_group() {
        // Use a schema with multiple wealth attrs.
        let multi_schema = AttributeSchema::from_json(
            r#"{
                "version": 1,
                "attributes": [
                    {"name": "wealth.cash", "unit": "$"},
                    {"name": "wealth.stocks", "unit": "$"},
                    {"name": "health.physical", "unit": "pts"}
                ]
            }"#,
        )
        .unwrap();

        let named = NamedEffect::group_fixed("wealth", -500.0);
        let effects = named.resolve(&multi_schema).unwrap();
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].attribute_index, 0); // wealth.cash
        assert_eq!(effects[0].delta, -500.0);
        assert_eq!(effects[1].attribute_index, 1); // wealth.stocks
        assert_eq!(effects[1].delta, -500.0);
    }

    #[test]
    fn resolve_effect_group_not_found() {
        let schema = test_schema();
        let named = NamedEffect::group_fixed("nonexistent", 10.0);
        assert!(named.resolve(&schema).is_err());
    }

    #[test]
    fn resolve_effect_no_target_errors() {
        let schema = test_schema();
        let named = NamedEffect {
            attribute: None,
            group: None,
            delta: 10.0,
            scaling: vec![],
        };
        assert!(named.resolve(&schema).is_err());
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
