//! Event template persistence and runtime — timeline-layer events with
//! preconditions, delay, duration, cooldown, and auto-effects.
//!
//! Event definitions live in the `events` table (per-schema) and are
//! independent of `loom-core`'s Event type (which is engine-internal).
//! Active event instances live in `active_events` (per-timeline).

use loom_core::{AttributeSchema, NamedCondition, NamedEffect};
use rusqlite::{params, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── Event template ────────────────────────────────────────────────────────────

/// Controls whether all preconditions must be met (AND) or any one suffices (OR).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreconditionMode {
    #[default]
    All,
    Any,
}

/// A named event template — stored per schema, resolved to engine types via schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NamedEvent {
    /// Unique identifier within the schema.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Conditions that must all hold for this event to trigger.
    #[serde(default)]
    pub preconditions: Vec<NamedCondition>,
    /// Steps between trigger and fire (delay countdown).
    #[serde(default)]
    pub delay: u32,
    /// Steps over which effects are spread.
    #[serde(default)]
    pub duration: u32,
    /// Steps before the event can trigger again after finishing.
    #[serde(default)]
    pub cooldown: u32,
    /// Attribute effects applied when the event fires.
    #[serde(default)]
    pub effects: Vec<NamedEffect>,
    /// Optional decision ID to spawn when the event fires.
    #[serde(default)]
    pub spawns_decision_id: Option<String>,
    /// Event IDs that trigger this event (chain trigger path).
    #[serde(default)]
    pub triggered_by: Vec<String>,
    /// While this event is active, these event IDs cannot trigger.
    #[serde(default)]
    pub suppressed_by: Vec<String>,
    /// Fires another event when THIS event fires.
    #[serde(default)]
    pub triggers_event_id: Option<String>,
    /// Fires another event when THIS event resolves.
    #[serde(default)]
    pub triggers_on_resolve: Option<String>,
    /// Priority — higher fires first.
    #[serde(default)]
    pub priority: i32,
    /// Precondition mode: All (AND) or Any (OR).
    #[serde(default)]
    pub precondition_mode: PreconditionMode,
}

/// An event effect that was applied during this step — returned by
/// `check_and_advance_events` so the caller can apply deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedEventEffect {
    /// Event label for display.
    pub event_label: String,
    /// Phase: "triggered" | "active" | "resolved"
    pub phase: String,
    /// The event's event_id.
    pub event_id: String,
    /// Flat delta applied to a single attribute.
    pub delta: f64,
    /// Attribute name this effect targets.
    pub attribute_name: String,
    /// Optional decision ID spawned by this event.
    pub spawned_decision_id: Option<String>,
    /// Human-readable description.
    pub description: String,
}

// ── Event CRUD on Store ─────────────────────────────────────────────────────

impl crate::Store {
    /// Insert or replace an event template. Returns the row ID.
    pub fn upsert_event(&self, schema_name: &str, event: &NamedEvent) -> SqlResult<i64> {
        let schema_id = self.schema_id(schema_name)?;
        let preconditions_json = serde_json::to_string(&event.preconditions).unwrap();
        let effects_json = serde_json::to_string(&event.effects).unwrap();
        let triggered_by_json = serde_json::to_string(&event.triggered_by).unwrap();
        let suppressed_by_json = serde_json::to_string(&event.suppressed_by).unwrap();
        let precondition_mode_str = match event.precondition_mode {
            PreconditionMode::All => "All",
            PreconditionMode::Any => "Any",
        };

        self.conn.execute(
            "INSERT INTO events (schema_id, event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(schema_id, event_id) DO UPDATE SET
                label = ?3, description = ?4, preconditions_json = ?5,
                delay = ?6, duration = ?7, cooldown = ?8,
                effects_json = ?9, spawns_decision_id = ?10,
                triggered_by_json = ?11, suppressed_by_json = ?12,
                triggers_event_id = ?13, triggers_on_resolve = ?14,
                priority = ?15, precondition_mode = ?16,
                created_at = datetime('now')",
            params![
                schema_id,
                event.id,
                event.label,
                event.description,
                preconditions_json,
                event.delay,
                event.duration,
                event.cooldown,
                effects_json,
                event.spawns_decision_id,
                triggered_by_json,
                suppressed_by_json,
                event.triggers_event_id,
                event.triggers_on_resolve,
                event.priority,
                precondition_mode_str,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Load a single event template by (schema_name, event_id).
    pub fn get_event(
        &self,
        schema_name: &str,
        event_id: &str,
    ) -> SqlResult<Option<NamedEvent>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode
             FROM events WHERE schema_id = ?1 AND event_id = ?2",
        )?;
        let mut rows = stmt.query_map(params![schema_id, event_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, u32>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, i32>(13)?,
                row.get::<_, String>(14)?,
            ))
        })?;
        match rows.next() {
            Some(Ok((id, label, description, precond_json, delay, duration, cooldown, effects_json, spawns, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode_str))) => {
                let preconditions: Vec<NamedCondition> =
                    serde_json::from_str(&precond_json).unwrap_or_default();
                let effects: Vec<NamedEffect> =
                    serde_json::from_str(&effects_json).unwrap_or_default();
                let triggered_by: Vec<String> =
                    serde_json::from_str(&triggered_by_json).unwrap_or_default();
                let suppressed_by: Vec<String> =
                    serde_json::from_str(&suppressed_by_json).unwrap_or_default();
                let precondition_mode = match precondition_mode_str.as_str() {
                    "Any" => PreconditionMode::Any,
                    _ => PreconditionMode::All,
                };
                Ok(Some(NamedEvent {
                    id,
                    label,
                    description,
                    preconditions,
                    delay,
                    duration,
                    cooldown,
                    effects,
                    spawns_decision_id: spawns,
                    triggered_by,
                    suppressed_by,
                    triggers_event_id,
                    triggers_on_resolve,
                    priority,
                    precondition_mode,
                }))
            }
            _ => Ok(None),
        }
    }

    /// List all event templates for a schema.
    pub fn list_events(&self, schema_name: &str) -> SqlResult<Vec<NamedEvent>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode
             FROM events WHERE schema_id = ?1 ORDER BY event_id",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, u32>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, i32>(13)?,
                row.get::<_, String>(14)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (id, label, description, precond_json, delay, duration, cooldown, effects_json, spawns, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode_str) = row?;
            let preconditions: Vec<NamedCondition> =
                serde_json::from_str(&precond_json).unwrap_or_default();
            let effects: Vec<NamedEffect> =
                serde_json::from_str(&effects_json).unwrap_or_default();
            let triggered_by: Vec<String> =
                serde_json::from_str(&triggered_by_json).unwrap_or_default();
            let suppressed_by: Vec<String> =
                serde_json::from_str(&suppressed_by_json).unwrap_or_default();
            let precondition_mode = match precondition_mode_str.as_str() {
                "Any" => PreconditionMode::Any,
                _ => PreconditionMode::All,
            };
            events.push(NamedEvent {
                id,
                label,
                description,
                preconditions,
                delay,
                duration,
                cooldown,
                effects,
                spawns_decision_id: spawns,
                triggered_by,
                suppressed_by,
                triggers_event_id,
                triggers_on_resolve,
                priority,
                precondition_mode,
            });
        }
        Ok(events)
    }

    /// Delete an event template.
    pub fn delete_event(&self, schema_name: &str, event_id: &str) -> SqlResult<usize> {
        let schema_id = self.schema_id(schema_name)?;
        self.conn.execute(
            "DELETE FROM events WHERE schema_id = ?1 AND event_id = ?2",
            params![schema_id, event_id],
        )
    }
}

// ── Active event runtime on TimelineStore ───────────────────────────────────

/// Row type for active_events table (used internally).
#[derive(Debug, Clone)]
pub struct ActiveEventRow {
    pub id: i64,
    pub timeline_id: i64,
    pub event_template_id: i64,
    pub phase: String,
    pub delay_remaining: u32,
    pub duration_remaining: u32,
    pub cooldown_remaining: u32,
}

impl crate::timeline::TimelineStore<'_> {
    /// Check and advance all events for a timeline on the current step.
    ///
    /// Called when appending a new snapshot. Returns the list of auto-effects
    /// that should be applied to the new snapshot's attribute values.
    ///
    /// Algorithm:
    /// 1. For all event templates matching the timeline's schema:
    ///    - Skip if already active (pending/active/cooldown entry exists)
    ///    - Check preconditions against current attribute values
    /// 2. For each event that meets preconditions:
    ///    - If delay > 0: create active_event with phase='pending', delay_remaining=delay
    ///    - If delay == 0: create active_event with phase='active', duration_remaining=duration
    ///      Report its effects as auto-effects for this step
    /// 3. For all existing active_events on this timeline:
    ///    - 'pending': decrement delay_remaining. If 0, move to 'active', report effects
    ///    - 'active': if duration_remaining > 0, decrement and report effects this step too.
    ///      If duration_remaining hits 0, move to 'cooldown' phase
    ///    - 'cooldown': decrement cooldown_remaining. If 0, delete the active_event entry
    pub fn check_and_advance_events(
        &self,
        timeline_id: i64,
        schema: &AttributeSchema,
        current_values: &[f64],
    ) -> SqlResult<Vec<AppliedEventEffect>> {
        let mut results: Vec<AppliedEventEffect> = Vec::new();

        // 1. Load all event templates for this schema
        let schema_id = self.get_schema_id_by_timeline(timeline_id)?;
        let events = self.load_event_templates(schema_id)?;

        // 2. Load existing active events for this timeline
        let mut actives = self.load_active_events(timeline_id)?;
        let active_template_ids: Vec<i64> = actives.iter().map(|a| a.event_template_id).collect();

        // 3. Collect which events are firing or resolving this step (for chains)
        let mut firing_this_step: HashSet<String> = HashSet::new();
        let mut resolving_this_step: HashSet<String> = HashSet::new();

        // First pass: determine what fires
        let mut to_activate: Vec<(i32, &EventTemplateRow)> = Vec::new();

        for evt_tmpl in &events {
            if active_template_ids.contains(&evt_tmpl.row_id) {
                continue; // Already active
            }

            // Suppression check
            let suppressed = evt_tmpl.suppressed_by.iter().any(|supp_id| {
                actives.iter().any(|a| {
                    events
                        .iter()
                        .any(|e| e.row_id == a.event_template_id && e.event_id == *supp_id)
                })
            });
            if suppressed {
                continue;
            }

            // Trigger path
            let triggered = evt_tmpl.triggered_by.iter().any(|trigger_id| {
                firing_this_step.contains(trigger_id)
                    || resolving_this_step.contains(trigger_id)
            });

            // State path
            let preconditions_met = if evt_tmpl.precondition_mode == PreconditionMode::All {
                evt_tmpl
                    .preconditions
                    .iter()
                    .all(|nc| {
                        nc.resolve(schema)
                            .map(|c| c.check(current_values))
                            .unwrap_or(false)
                    })
            } else {
                evt_tmpl.preconditions.is_empty()
                    || evt_tmpl
                        .preconditions
                        .iter()
                        .any(|nc| {
                            nc.resolve(schema)
                                .map(|c| c.check(current_values))
                                .unwrap_or(false)
                        })
            };

            if triggered || preconditions_met {
                to_activate.push((evt_tmpl.priority, evt_tmpl));
            }
        }

        // Sort by priority descending, then original row_id order
        to_activate.sort_by_key(|(prio, evt)| (-prio, evt.row_id));

        // Process in priority order
        for (_prio, evt_tmpl) in &to_activate {
            if evt_tmpl.delay > 0 {
                // Create pending entry
                self.create_active_event(
                    timeline_id,
                    evt_tmpl.row_id,
                    "pending",
                    evt_tmpl.delay,
                    evt_tmpl.duration,
                    evt_tmpl.cooldown,
                )?;
                results.push(AppliedEventEffect {
                    event_label: evt_tmpl.label.clone(),
                    phase: "pending".into(),
                    event_id: evt_tmpl.event_id.clone(),
                    delta: 0.0,
                    attribute_name: String::new(),
                    spawned_decision_id: None,
                    description: format!(
                        "{} triggered, will fire in {} steps",
                        evt_tmpl.label, evt_tmpl.delay
                    ),
                });
            } else {
                // Fire immediately
                self.create_active_event(
                    timeline_id,
                    evt_tmpl.row_id,
                    "active",
                    0,
                    evt_tmpl.duration,
                    evt_tmpl.cooldown,
                )?;

                firing_this_step.insert(evt_tmpl.event_id.clone());

                // Chain: if this event fires, queue its triggers_event_id
                if let Some(ref chain_id) = evt_tmpl.triggers_event_id {
                    firing_this_step.insert(chain_id.clone());
                }

                for (attr_name, delta) in
                    self.resolve_effects(&evt_tmpl.effects, current_values, schema)
                {
                    results.push(AppliedEventEffect {
                        event_label: evt_tmpl.label.clone(),
                        phase: "active".into(),
                        event_id: evt_tmpl.event_id.clone(),
                        delta,
                        attribute_name: attr_name.clone(),
                        spawned_decision_id: evt_tmpl.spawns_decision_id.clone(),
                        description: format!(
                            "{} fired — {}: {}",
                            evt_tmpl.label,
                            attr_name,
                            if delta >= 0.0 {
                                format!("+{:.1}", delta)
                            } else {
                                format!("{:.1}", delta)
                            }
                        ),
                    });
                }
            }
        }

        // 4. Advance existing active events
        let mut to_delete: Vec<i64> = Vec::new();
        for active in &mut actives {
            match active.phase.as_str() {
                "pending" => {
                    if active.delay_remaining > 0 {
                        active.delay_remaining -= 1;
                        self.update_active_event_delay(active.id, active.delay_remaining)?;
                    }
                    if active.delay_remaining == 0 {
                        // Find the template to fire effects
                        if let Some(tmpl) =
                            events.iter().find(|e| e.row_id == active.event_template_id)
                        {
                            active.phase = "active".to_string();
                            active.duration_remaining = tmpl.duration;
                            self.update_active_event_activate(
                                active.id,
                                active.duration_remaining,
                            )?;

                            firing_this_step.insert(tmpl.event_id.clone());

                            // Chain on fire
                            if let Some(ref chain_id) = tmpl.triggers_event_id {
                                firing_this_step.insert(chain_id.clone());
                            }

                            for (attr_name, delta) in
                                self.resolve_effects(&tmpl.effects, current_values, schema)
                            {
                                results.push(AppliedEventEffect {
                                    event_label: tmpl.label.clone(),
                                    phase: "active".into(),
                                    event_id: tmpl.event_id.clone(),
                                    delta,
                                    attribute_name: attr_name.clone(),
                                    spawned_decision_id: tmpl.spawns_decision_id.clone(),
                                    description: format!(
                                        "{} fired — {}: {}",
                                        tmpl.label,
                                        attr_name,
                                        if delta >= 0.0 {
                                            format!("+{:.1}", delta)
                                        } else {
                                            format!("{:.1}", delta)
                                        }
                                    ),
                                });
                            }
                        }
                    }
                }
                "active" => {
                    if active.duration_remaining > 0 {
                        // Apply effects again this step (for multi-step duration spread)
                        if let Some(tmpl) =
                            events.iter().find(|e| e.row_id == active.event_template_id)
                        {
                            active.duration_remaining =
                                active.duration_remaining.saturating_sub(1);
                            if active.duration_remaining > 0 {
                                // Ongoing — spread effect
                                for (attr_name, delta) in
                                    self.resolve_effects(&tmpl.effects, current_values, schema)
                                {
                                    // For spread: divide delta by duration for each active step
                                    let per_step = if tmpl.duration > 0 {
                                        delta / tmpl.duration as f64
                                    } else {
                                        delta
                                    };
                                    results.push(AppliedEventEffect {
                                        event_label: tmpl.label.clone(),
                                        phase: "active".into(),
                                        event_id: tmpl.event_id.clone(),
                                        delta: per_step,
                                        attribute_name: attr_name.clone(),
                                        spawned_decision_id: None,
                                        description: format!(
                                            "{} ongoing — {}: {}",
                                            tmpl.label,
                                            attr_name,
                                            if per_step >= 0.0 {
                                                format!("+{:.1}", per_step)
                                            } else {
                                                format!("{:.1}", per_step)
                                            }
                                        ),
                                    });
                                }
                            }

                            if active.duration_remaining == 0 {
                                // Move to cooldown
                                active.phase = "cooldown".to_string();
                                active.cooldown_remaining = tmpl.cooldown;
                                self.update_active_event_cooldown(
                                    active.id,
                                    active.cooldown_remaining,
                                )?;

                                resolving_this_step.insert(tmpl.event_id.clone());

                                // Chain on resolve
                                if let Some(ref chain_id) = tmpl.triggers_on_resolve {
                                    resolving_this_step.insert(chain_id.clone());
                                }

                                results.push(AppliedEventEffect {
                                    event_label: tmpl.label.clone(),
                                    phase: "resolved".into(),
                                    event_id: tmpl.event_id.clone(),
                                    delta: 0.0,
                                    attribute_name: String::new(),
                                    spawned_decision_id: None,
                                    description: format!("{} resolved", tmpl.label),
                                });
                            } else {
                                self.update_active_event_duration(
                                    active.id,
                                    active.duration_remaining,
                                )?;
                            }
                        } else {
                            active.duration_remaining =
                                active.duration_remaining.saturating_sub(1);
                            self.update_active_event_duration(
                                active.id,
                                active.duration_remaining,
                            )?;
                            if active.duration_remaining == 0 {
                                active.phase = "cooldown".to_string();
                                // Find cooldown from template
                                if let Some(tmpl) =
                                    events.iter().find(|e| e.row_id == active.event_template_id)
                                {
                                    active.cooldown_remaining = tmpl.cooldown;
                                }
                                self.update_active_event_cooldown(
                                    active.id,
                                    active.cooldown_remaining,
                                )?;
                            }
                        }
                    }
                }
                "cooldown" => {
                    if active.cooldown_remaining > 0 {
                        active.cooldown_remaining -= 1;
                        self.update_active_event_cooldown(
                            active.id,
                            active.cooldown_remaining,
                        )?;
                    }
                    if active.cooldown_remaining == 0 {
                        to_delete.push(active.id);
                    }
                }
                _ => {}
            }
        }

        // 5. Delete finished cooldown events
        for id in to_delete {
            self.delete_active_event(id)?;
        }

        Ok(results)
    }
}

// ── Internal helpers for TimelineStore ─────────────────────────────────────

/// Internal struct for event template data loaded from DB.
#[derive(Debug, Clone)]
struct EventTemplateRow {
    row_id: i64,
    event_id: String,
    label: String,
    preconditions: Vec<NamedCondition>,
    delay: u32,
    duration: u32,
    cooldown: u32,
    effects: Vec<NamedEffect>,
    spawns_decision_id: Option<String>,
    triggered_by: Vec<String>,
    suppressed_by: Vec<String>,
    triggers_event_id: Option<String>,
    triggers_on_resolve: Option<String>,
    priority: i32,
    precondition_mode: PreconditionMode,
}

impl crate::timeline::TimelineStore<'_> {
    fn get_schema_id_by_timeline(&self, timeline_id: i64) -> SqlResult<i64> {
        self.0.query_row(
            "SELECT schema_id FROM timelines WHERE id = ?1",
            params![timeline_id],
            |row| row.get(0),
        )
    }

    fn load_event_templates(&self, schema_id: i64) -> SqlResult<Vec<EventTemplateRow>> {
        let mut stmt = self.0.prepare(
            "SELECT id, event_id, label, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id, triggered_by_json, suppressed_by_json, triggers_event_id, triggers_on_resolve, priority, precondition_mode
             FROM events WHERE schema_id = ?1",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| {
            let precondition_mode_str: String = row.get(14)?;
            Ok(EventTemplateRow {
                row_id: row.get(0)?,
                event_id: row.get(1)?,
                label: row.get(2)?,
                preconditions: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                delay: row.get(4)?,
                duration: row.get(5)?,
                cooldown: row.get(6)?,
                effects: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                spawns_decision_id: row.get(8)?,
                triggered_by: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                suppressed_by: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
                triggers_event_id: row.get(11)?,
                triggers_on_resolve: row.get(12)?,
                priority: row.get(13)?,
                precondition_mode: match precondition_mode_str.as_str() {
                    "Any" => PreconditionMode::Any,
                    _ => PreconditionMode::All,
                },
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn load_active_events(&self, timeline_id: i64) -> SqlResult<Vec<ActiveEventRow>> {
        let mut stmt = self.0.prepare(
            "SELECT id, timeline_id, event_template_id, phase, delay_remaining, duration_remaining, cooldown_remaining
             FROM active_events WHERE timeline_id = ?1",
        )?;
        let rows = stmt.query_map(params![timeline_id], |row| {
            Ok(ActiveEventRow {
                id: row.get(0)?,
                timeline_id: row.get(1)?,
                event_template_id: row.get(2)?,
                phase: row.get(3)?,
                delay_remaining: row.get(4)?,
                duration_remaining: row.get(5)?,
                cooldown_remaining: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn create_active_event(
        &self,
        timeline_id: i64,
        event_template_id: i64,
        phase: &str,
        delay_remaining: u32,
        duration_remaining: u32,
        cooldown_remaining: u32,
    ) -> SqlResult<()> {
        self.0.execute(
            "INSERT OR IGNORE INTO active_events (timeline_id, event_template_id, phase, delay_remaining, duration_remaining, cooldown_remaining)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![timeline_id, event_template_id, phase, delay_remaining, duration_remaining, cooldown_remaining],
        )?;
        Ok(())
    }

    fn update_active_event_delay(&self, id: i64, delay_remaining: u32) -> SqlResult<()> {
        self.0.execute(
            "UPDATE active_events SET delay_remaining = ?1 WHERE id = ?2",
            params![delay_remaining, id],
        )?;
        Ok(())
    }

    fn update_active_event_activate(&self, id: i64, duration_remaining: u32) -> SqlResult<()> {
        self.0.execute(
            "UPDATE active_events SET phase = 'active', duration_remaining = ?1 WHERE id = ?2",
            params![duration_remaining, id],
        )?;
        Ok(())
    }

    fn update_active_event_duration(&self, id: i64, duration_remaining: u32) -> SqlResult<()> {
        self.0.execute(
            "UPDATE active_events SET duration_remaining = ?1 WHERE id = ?2",
            params![duration_remaining, id],
        )?;
        Ok(())
    }

    fn update_active_event_cooldown(&self, id: i64, cooldown_remaining: u32) -> SqlResult<()> {
        self.0.execute(
            "UPDATE active_events SET phase = 'cooldown', cooldown_remaining = ?1 WHERE id = ?2",
            params![cooldown_remaining, id],
        )?;
        Ok(())
    }

    fn delete_active_event(&self, id: i64) -> SqlResult<()> {
        self.0.execute("DELETE FROM active_events WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Resolve named effects to (attribute_name, computed_delta) pairs.
    /// Properly handles group expansion, scaling, and compute_delta.
    fn resolve_effects(&self, effects: &[NamedEffect], current_values: &[f64], schema: &AttributeSchema) -> Vec<(String, f64)> {
        effects.iter().flat_map(|ne| {
            ne.resolve(schema).unwrap_or_default().into_iter().map(|ae| {
                let delta = ae.compute_delta(current_values);
                let attr_name = schema.at(ae.attribute_index)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "?".to_string());
                (attr_name, delta)
            })
        }).collect()
    }

    /// Get active event status data for display.
    pub fn get_active_events_status(&self, timeline_id: i64) -> SqlResult<Vec<(String, String, u32, u32, u32)>> {
        // Returns (event_label, phase, delay_remaining OR duration_remaining OR cooldown_remaining, original_delay/duration/cooldown, total_steps_left)
        let actives = self.load_active_events(timeline_id)?;
        let mut result = Vec::new();
        for a in &actives {
            // Get template to find label
            let label: String = self.0.query_row(
                "SELECT label FROM events WHERE id = ?1",
                params![a.event_template_id],
                |row| row.get(0),
            ).unwrap_or_else(|_| format!("#{}", a.event_template_id));

            let (remaining, total) = match a.phase.as_str() {
                "pending" => (a.delay_remaining, a.delay_remaining),
                "active" => (a.duration_remaining, a.duration_remaining),
                "cooldown" => (a.cooldown_remaining, a.cooldown_remaining),
                _ => (0, 0),
            };
            result.push((label, a.phase.clone(), remaining, total, a.cooldown_remaining));
        }
        Ok(result)
    }
}
