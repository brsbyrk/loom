//! Resolved event templates and pure event firing logic — no strings, no DB, no schema.
//! Used by both Simulation (what-if) and TimelineStore (journaling).
//!
//! The core idea: `determine_firing()` is a pure function that decides *which events trigger*
//! given the current state, active suppression set, and chain triggers from the previous step.
//! Callers (Simulation, TimelineStore) handle lifecycle (delay/duration/cooldown) and effect
//! application on top of this core.

use crate::traits::{Action, Predicate};
use std::collections::HashSet;

/// A fully resolved event template — engine-native, no named references.
///
/// All attribute lookups, group expansions, and condition resolution happened
/// at construct time via `resolve_events()`.
#[derive(Debug)]
pub struct ResolvedEvent {
    /// Stable identifier within the events array (positional index).
    pub id: usize,
    /// Human-readable label for display.
    pub label: String,
    /// Precondition — already resolved, AND/OR baked in via All/Any compositors.
    pub precondition: Box<dyn Predicate>,
    /// Effects to apply when this event fires.
    pub effects: Vec<Box<dyn Action>>,
    /// Events that trigger this one (any of fire or resolve from previous step).
    pub triggered_by: Vec<usize>,
    /// While any of these events are active, this event cannot fire.
    pub suppressed_by: Vec<usize>,
    /// Higher fires first when multiple events trigger in the same step.
    pub priority: i32,
    /// Steps between trigger and fire (delay countdown).
    pub delay: u32,
    /// Steps over which effects are spread (0 = instant, one-shot).
    pub duration: u32,
    /// Steps before the event can trigger again after finishing.
    pub cooldown: u32,
    /// Event to chain-trigger when this one fires (forward chain, for caller use).
    pub triggers_event_id: Option<usize>,
    /// Event to chain-trigger when this one resolves (forward chain, for caller use).
    pub triggers_on_resolve_id: Option<usize>,
}

/// Pure event firing logic — no DB, no schema, no lifecycle.
///
/// Determines which events trigger given the current state, active suppressors,
/// and chain triggers from the previous step.
///
/// # Arguments
/// * `events` — all resolved event templates
/// * `state` — current attribute values
/// * `active_ids` — IDs of events currently active (suppression scope; these are skipped)
/// * `fired_prev` — IDs that fired in the previous step (trigger chain)
/// * `resolved_prev` — IDs that resolved in the previous step (trigger chain)
///
/// # Returns
/// Event IDs that should fire, sorted by priority (highest first), then by id
/// for deterministic tie-breaking.
pub fn determine_firing(
    events: &[ResolvedEvent],
    state: &[f64],
    active_ids: &HashSet<usize>,
    fired_prev: &HashSet<usize>,
    resolved_prev: &HashSet<usize>,
) -> Vec<usize> {
    // Build suppression set: all active event IDs
    let suppressors: HashSet<usize> = active_ids.iter().copied().collect();

    // Determine which events fire
    let mut firing: Vec<(i32, usize)> = Vec::new();

    for evt in events {
        // Skip if already active
        if active_ids.contains(&evt.id) {
            continue;
        }

        // Suppression check: if any suppressor is active, can't fire
        if evt
            .suppressed_by
            .iter()
            .any(|sid| suppressors.contains(sid))
        {
            continue;
        }

        // Trigger path: chain from previous step
        let triggered = evt
            .triggered_by
            .iter()
            .any(|tid| fired_prev.contains(tid) || resolved_prev.contains(tid));

        // State path: preconditions
        let preconditions_met = evt.precondition.evaluate(state);

        if triggered || preconditions_met {
            firing.push((evt.priority, evt.id));
        }
    }

    // Sort by priority descending, then id ascending for deterministic tie-breaking
    firing.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    firing.into_iter().map(|(_, id)| id).collect()
}

// ── Tests ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::AttributeEffect;

    fn always_true() -> Box<dyn Predicate> {
        #[derive(Debug)]
        struct AlwaysTrue;
        impl Predicate for AlwaysTrue {
            fn evaluate(&self, _state: &[f64]) -> bool {
                true
            }
        }
        Box::new(AlwaysTrue)
    }

    fn always_false() -> Box<dyn Predicate> {
        #[derive(Debug)]
        struct AlwaysFalse;
        impl Predicate for AlwaysFalse {
            fn evaluate(&self, _state: &[f64]) -> bool {
                false
            }
        }
        Box::new(AlwaysFalse)
    }

    fn noop_effect() -> Box<dyn Action> {
        Box::new(AttributeEffect::fixed(0, 0.0))
    }

    fn make_events() -> Vec<ResolvedEvent> {
        vec![
            ResolvedEvent {
                id: 0,
                label: "event_0".into(),
                precondition: always_true(),
                effects: vec![noop_effect()],
                triggered_by: vec![],
                suppressed_by: vec![],
                priority: 0,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
            ResolvedEvent {
                id: 1,
                label: "event_1".into(),
                precondition: always_false(),
                effects: vec![noop_effect()],
                triggered_by: vec![0], // triggered by event 0 firing
                suppressed_by: vec![],
                priority: 5,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
            ResolvedEvent {
                id: 2,
                label: "event_2".into(),
                precondition: always_true(),
                effects: vec![noop_effect()],
                triggered_by: vec![],
                suppressed_by: vec![0], // suppressed while event 0 is active
                priority: 10,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
        ]
    }

    #[test]
    fn fires_when_precondition_met() {
        let events = make_events();
        let state = vec![0.0];
        let active = HashSet::new();
        let fired_prev = HashSet::new();
        let resolved_prev = HashSet::new();

        let result = determine_firing(&events, &state, &active, &fired_prev, &resolved_prev);
        // event_0 fires (precondition true), event_2 fires (precondition true, not suppressed since 0 not active yet)
        assert_eq!(result, vec![2, 0]); // priority 10 before 0
    }

    #[test]
    fn chain_trigger_from_previous_step() {
        let events = make_events();
        let state = vec![0.0];
        let active = HashSet::new();
        let mut fired_prev = HashSet::new();
        fired_prev.insert(0); // event_0 fired last step
        let resolved_prev = HashSet::new();

        let result = determine_firing(&events, &state, &active, &fired_prev, &resolved_prev);
        // event_0, event_1 (triggered by event_0), event_2 all fire
        assert!(result.contains(&0));
        assert!(result.contains(&1)); // chain-triggered
        assert!(result.contains(&2));
    }

    #[test]
    fn suppression_blocks_firing() {
        let events = make_events();
        let state = vec![0.0];
        let mut active = HashSet::new();
        active.insert(0); // event_0 is active
        let fired_prev = HashSet::new();
        let resolved_prev = HashSet::new();

        let result = determine_firing(&events, &state, &active, &fired_prev, &resolved_prev);
        // event_0 is skipped (active), event_1 not triggered (no chain), event_2 is suppressed by event_0
        assert!(result.is_empty());
    }

    #[test]
    fn active_event_not_retriggered() {
        let events = make_events();
        let state = vec![0.0];
        let mut active = HashSet::new();
        active.insert(0);
        let fired_prev = HashSet::new();
        let resolved_prev = HashSet::new();

        let result = determine_firing(&events, &state, &active, &fired_prev, &resolved_prev);
        assert!(!result.contains(&0)); // already active
    }

    #[test]
    fn priority_sort_order() {
        let events = vec![
            ResolvedEvent {
                id: 0,
                label: "low".into(),
                precondition: always_true(),
                effects: vec![noop_effect()],
                triggered_by: vec![],
                suppressed_by: vec![],
                priority: 0,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
            ResolvedEvent {
                id: 1,
                label: "high".into(),
                precondition: always_true(),
                effects: vec![noop_effect()],
                triggered_by: vec![],
                suppressed_by: vec![],
                priority: 100,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
            ResolvedEvent {
                id: 2,
                label: "mid".into(),
                precondition: always_true(),
                effects: vec![noop_effect()],
                triggered_by: vec![],
                suppressed_by: vec![],
                priority: 50,
                delay: 0,
                duration: 1,
                cooldown: 0,
                triggers_event_id: None,
                triggers_on_resolve_id: None,
            },
        ];
        let state = vec![0.0];
        let active = HashSet::new();
        let fired_prev = HashSet::new();
        let resolved_prev = HashSet::new();

        let result = determine_firing(&events, &state, &active, &fired_prev, &resolved_prev);
        assert_eq!(result, vec![1, 2, 0]); // high → mid → low
    }
}
