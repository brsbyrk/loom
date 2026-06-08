//! Event and decision data model — Condition, AttributeEffect, Transform, Event, Decision, Outcome.
//!
//! All types derive `Serialize`/`Deserialize` so event/decision definitions can be loaded from
//! configuration files (JSON, YAML, etc.). This is the data-driven foundation.

use serde::{Deserialize, Serialize};

// ── Conditions ──────────────────────────────────────────────────────────────────────

/// Comparison operator for numeric conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    /// attribute == value
    Eq,
    /// attribute != value
    Neq,
    /// attribute < value
    Lt,
    /// attribute <= value
    Lte,
    /// attribute > value
    Gt,
    /// attribute >= value
    Gte,
}

/// A threshold condition on a single attribute.
///
/// Evaluates to `true` when the attribute at `attribute_index` satisfies the `operator`
/// relative to `value`. Used as preconditions for events/decisions and as branch guards
/// inside `ConditionalTransform`.
///
/// # Example (JSON)
/// ```json
/// {"attribute_index": 2, "operator": "Gt", "value": 70.0}
/// ```
/// Means: "attribute[2] > 70.0"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    /// Index into the state vector.
    pub attribute_index: usize,
    /// Comparison to perform.
    pub operator: ComparisonOp,
    /// Threshold value to compare against.
    pub value: f64,
}

impl Condition {
    /// Evaluate this condition against a state slice.
    #[inline]
    pub fn check(&self, state: &[f64]) -> bool {
        let attr = state[self.attribute_index];
        match self.operator {
            ComparisonOp::Eq => (attr - self.value).abs() < f64::EPSILON,
            ComparisonOp::Neq => (attr - self.value).abs() >= f64::EPSILON,
            ComparisonOp::Lt => attr < self.value,
            ComparisonOp::Lte => attr <= self.value,
            ComparisonOp::Gt => attr > self.value,
            ComparisonOp::Gte => attr >= self.value,
        }
    }
}

// ── Attribute effects ────────────────────────────────────────────────────────────────

/// A single effect applied to one attribute.
///
/// The final delta is computed as:
/// ```text
/// effect = delta + Σ(scaling_factor_k * state[scaling_attribute_k])
/// ```
///
/// This allows state-dependent effects: "gain 10% of current wealth" would be
/// `delta=0.0, scaling=[(WEALTH, 0.1)]`.
///
/// # Example (JSON)
/// ```json
/// {"attribute_index": 0, "delta": 5000.0}
/// ```
/// Means: attribute[0] += 5000.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributeEffect {
    /// Which attribute this effect targets.
    pub attribute_index: usize,
    /// Base additive change.
    pub delta: f64,
    /// Optional state-dependent scaling: list of (source_attribute_index, multiplier).
    /// Each entry contributes `multiplier * state[source_attribute]` to the delta.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scaling: Vec<(usize, f64)>,
}

impl AttributeEffect {
    /// Create a simple fixed-delta effect (no scaling).
    pub fn fixed(attribute_index: usize, delta: f64) -> Self {
        Self {
            attribute_index,
            delta,
            scaling: Vec::new(),
        }
    }

    /// Create a proportional effect: `attribute += multiplier * state[source]`
    pub fn proportional(attribute_index: usize, source_attribute: usize, multiplier: f64) -> Self {
        Self {
            attribute_index,
            delta: 0.0,
            scaling: vec![(source_attribute, multiplier)],
        }
    }

    /// Compute the actual delta given the current state.
    pub fn compute_delta(&self, state: &[f64]) -> f64 {
        let scaled: f64 = self
            .scaling
            .iter()
            .map(|(idx, scale)| scale * state[*idx])
            .sum();
        self.delta + scaled
    }

    /// Apply this effect to a mutable state slice.
    pub fn apply(&self, state: &mut [f64]) {
        state[self.attribute_index] += self.compute_delta(state);
    }
}

// ── Transforms ───────────────────────────────────────────────────────────────────────

/// Source of a scripted transform — inline code or a file reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScriptSource {
    /// Inline script body (e.g., Rhai source).
    #[serde(rename = "inline")]
    Inline(String),
    /// Path to an external script file.
    #[serde(rename = "path")]
    File(String),
}

/// How a decision or event changes the state vector.
///
/// Two modes:
/// - **Declarative**: engine-inspectable, serializable, composable. Use for the 90% case.
/// - **Scripted**: arbitrary logic via a script hook. Opaque to the engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Transform {
    /// Declarative transform with unconditional effects and conditional branches.
    Declarative {
        /// Effects that are always applied.
        #[serde(default)]
        effects: Vec<AttributeEffect>,
        /// Conditional group effects: when a condition holds, apply all effects in the group.
        /// Evaluated in order; first match wins (no fall-through).
        #[serde(default)]
        conditional: Vec<(Condition, Vec<AttributeEffect>)>,
        /// Effects applied when none of the conditional guards match (the "else" branch).
        #[serde(default)]
        default_conditional: Vec<AttributeEffect>,
    },
    /// Full-power transform implemented by an external script. The engine treats this
    /// as a black box: it calls the script, gets back a new state vector, but cannot
    /// analyze or introspect the logic.
    Scripted {
        /// Where to find the script code.
        source: ScriptSource,
    },
}

impl Transform {
    /// Create a simple unconditional declarative transform.
    pub fn simple(effects: Vec<AttributeEffect>) -> Self {
        Transform::Declarative {
            effects,
            conditional: Vec::new(),
            default_conditional: Vec::new(),
        }
    }

    /// Apply this transform to a state slice.
    ///
    /// For declarative transforms, this mutates the slice in place.
    /// For scripted transforms, this is a stub that will be implemented when scripting is wired up.
    pub fn apply(&self, state: &mut [f64]) {
        match self {
            Transform::Declarative {
                effects,
                conditional,
                default_conditional,
            } => {
                // 1. Apply unconditional effects.
                for effect in effects {
                    effect.apply(state);
                }
                // 2. Check conditional branches — first match wins.
                let mut matched = false;
                for (cond, cond_effects) in conditional {
                    if cond.check(state) {
                        for effect in cond_effects {
                            effect.apply(state);
                        }
                        matched = true;
                        break;
                    }
                }
                // 3. Else branch.
                if !matched {
                    for effect in default_conditional {
                        effect.apply(state);
                    }
                }
            }
            Transform::Scripted { .. } => {
                // Script hooks — to be implemented in Phase 1+ when Rhai/WASM is wired.
                // For now, this is a no-op.
            }
        }
    }
}

// ── Outcomes ─────────────────────────────────────────────────────────────────────────

/// A single probabilistic outcome branch within a decision or event.
///
/// Each outcome has:
/// - A weight (used for probabilistic sampling: P = weight / sum(all outcome weights))
/// - An optional condition (this branch is only eligible if the condition holds)
/// - A transform to apply when this branch is selected
///
/// # Example (JSON)
/// ```json
/// {
///   "weight": 70.0,
///   "condition": null,
///   "transform": {"Declarative": {"effects": [{"attribute_index": 0, "delta": 40000.0}]}}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    /// Human-readable label for display purposes (e.g., "Great culture fit").
    #[serde(default)]
    pub label: String,
    /// Relative weight for probabilistic sampling.
    pub weight: f64,
    /// Optional guard — this outcome is only valid when the condition holds.
    /// If the condition fails, this outcome is excluded from the sampling pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// The state transform applied when this outcome is selected.
    pub transform: Transform,
}

// ── Decisions ────────────────────────────────────────────────────────────────────────

/// A decision the agent/user can take — a choice that changes state probabilistically.
///
/// Decisions differ from raw `Event`s semantically: a decision is a *choice* (voluntary),
/// while an event is something that *happens* (external). Structurally they share the same
/// shape — preconditions, cost, probabilistic outcomes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    /// Unique identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Conditions that must all hold for this decision to be available.
    #[serde(default)]
    pub preconditions: Vec<Condition>,
    /// Immediate deterministic cost paid *before* the outcome is sampled.
    #[serde(default)]
    pub cost: Vec<AttributeEffect>,
    /// Weighted outcome branches.
    pub outcomes: Vec<Outcome>,
}

impl Decision {
    /// Check whether all preconditions are satisfied given the current state.
    pub fn available(&self, state: &[f64]) -> bool {
        self.preconditions.iter().all(|c| c.check(state))
    }

    /// Return the outcomes that are eligible (condition holds) given the current state.
    /// Used during simulation to construct the sampling pool.
    pub fn eligible_outcomes(&self, state: &[f64]) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| {
                o.condition.as_ref().map_or(true, |c| c.check(state))
            })
            .collect()
    }
}

// ── Events ───────────────────────────────────────────────────────────────────────────

/// An external event — something that *happens* to the agent (not a choice).
///
/// Events may be triggered passively by conditions, by the simulation clock, or as
/// second-order consequences of decisions. Structurally identical to a decision minus
/// the semantic framing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Unique identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Conditions that must all hold for this event to fire.
    #[serde(default)]
    pub preconditions: Vec<Condition>,
    /// Immediate deterministic effect.
    #[serde(default)]
    pub cost: Vec<AttributeEffect>,
    /// Weighted outcome branches.
    pub outcomes: Vec<Outcome>,
}

impl Event {
    /// Check whether all preconditions are satisfied given the current state.
    pub fn fires(&self, state: &[f64]) -> bool {
        self.preconditions.iter().all(|c| c.check(state))
    }
}

// ── Decision schedules ──────────────────────────────────────────────────────────────

/// A decision scheduled at a specific absolute step during simulation.
///
/// Step 0 fires before any passives tick. Step N fires after the N-th passive tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduledDecision {
    /// Absolute step index in the horizon.
    pub at_step: usize,
    /// The decision to apply at this step.
    pub decision: Decision,
    /// If `true`, the entire simulation run is marked failed when preconditions
    /// aren't met at this step. If `false`, the decision is simply skipped.
    #[serde(default)]
    pub required: bool,
}

/// A schedule of decisions to apply at specific steps.
///
/// During simulation, at each step the engine checks whether a decision is
/// scheduled. If multiple decisions share the same step, they're applied in
/// schedule order. The schedule should be sorted by `at_step` for predictable
/// behavior.
///
/// # Example (JSON)
///
/// ```json
/// {
///   "entries": [
///     {"at_step": 0,  "decision": { "id": "take_job", ... }},
///     {"at_step": 12, "decision": { "id": "invest", ... }, "required": true}
///   ]
/// }
/// ```
///
/// This means: take the job at step 0 (start), then invest at step 12.
/// If the invest preconditions fail at step 12, the run is aborted (required=true).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionSchedule {
    /// Ordered list of scheduled decisions.
    pub entries: Vec<ScheduledDecision>,
}

impl DecisionSchedule {
    /// Create a schedule from a single decision at step 0.
    pub fn single(decision: Decision) -> Self {
        Self {
            entries: vec![ScheduledDecision {
                at_step: 0,
                decision,
                required: false,
            }],
        }
    }

    /// Get all decisions scheduled at a given step.
    pub fn at_step(&self, step: usize) -> Vec<&ScheduledDecision> {
        self.entries.iter().filter(|e| e.at_step == step).collect()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_check_eq() {
        let c = Condition { attribute_index: 0, operator: ComparisonOp::Eq, value: 50.0 };
        assert!(c.check(&[50.0, 100.0]));
        assert!(!c.check(&[51.0, 100.0]));
    }

    #[test]
    fn condition_check_gt() {
        let c = Condition { attribute_index: 0, operator: ComparisonOp::Gt, value: 50.0 };
        assert!(c.check(&[75.0, 100.0]));
        assert!(!c.check(&[50.0, 100.0]));
    }

    #[test]
    fn attribute_effect_fixed() {
        let e = AttributeEffect::fixed(0, 10.0);
        let mut state = vec![50.0, 100.0];
        e.apply(&mut state);
        assert!((state[0] - 60.0).abs() < f64::EPSILON);
        assert!((state[1] - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn attribute_effect_proportional() {
        // wealth[0] += 10% of wealth[0]
        let e = AttributeEffect::proportional(0, 0, 0.1);
        let mut state = vec![50000.0, 100.0];
        e.apply(&mut state);
        assert!((state[0] - 55000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn declarative_transform_conditional_match() {
        let t = Transform::Declarative {
            effects: vec![AttributeEffect::fixed(0, 10.0)],
            conditional: vec![(
                Condition { attribute_index: 1, operator: ComparisonOp::Gt, value: 50.0 },
                vec![AttributeEffect::fixed(1, -5.0)],
            )],
            default_conditional: vec![AttributeEffect::fixed(1, 5.0)],
        };
        let mut state = vec![100.0, 75.0]; // 75 > 50 → conditional fires
        t.apply(&mut state);
        assert!((state[0] - 110.0).abs() < f64::EPSILON); // +10 fixed
        assert!((state[1] - 70.0).abs() < f64::EPSILON);  // -5 conditional
    }

    #[test]
    fn declarative_transform_conditional_no_match_uses_default() {
        let t = Transform::Declarative {
            effects: vec![AttributeEffect::fixed(0, 10.0)],
            conditional: vec![(
                Condition { attribute_index: 1, operator: ComparisonOp::Gt, value: 50.0 },
                vec![AttributeEffect::fixed(1, -5.0)],
            )],
            default_conditional: vec![AttributeEffect::fixed(1, 5.0)],
        };
        let mut state = vec![100.0, 25.0]; // 25 ≤ 50 → default
        t.apply(&mut state);
        assert!((state[0] - 110.0).abs() < f64::EPSILON); // +10 fixed
        assert!((state[1] - 30.0).abs() < f64::EPSILON);  // +5 default
    }

    #[test]
    fn decision_available() {
        let d = Decision {
            id: "test".into(),
            label: "Test".into(),
            preconditions: vec![
                Condition { attribute_index: 0, operator: ComparisonOp::Gt, value: 50.0 },
            ],
            cost: vec![],
            outcomes: vec![],
        };
        assert!(d.available(&[75.0]));
        assert!(!d.available(&[25.0]));
    }

    #[test]
    fn outcome_condition_gating() {
        let d = Decision {
            id: "test".into(),
            label: "Test".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![
                Outcome {
                    label: "".into(),
                    weight: 1.0,
                    condition: Some(Condition { attribute_index: 0, operator: ComparisonOp::Gt, value: 50.0 }),
                    transform: Transform::simple(vec![]),
                },
                Outcome {
                    label: "".into(),
                    weight: 1.0,
                    condition: None,
                    transform: Transform::simple(vec![]),
                },
            ],
        };
        // State where first outcome's condition fails → only second is eligible
        let eligible = d.eligible_outcomes(&[25.0]);
        assert_eq!(eligible.len(), 1);
        // State where both are eligible (no condition on second, first's condition passes)
        let eligible = d.eligible_outcomes(&[75.0]);
        assert_eq!(eligible.len(), 2);
    }
}
