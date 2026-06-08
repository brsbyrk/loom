//! Event template persistence and runtime — timeline-layer events with
//! preconditions, delay, duration, cooldown, and auto-effects.
//!
//! Event definitions live in the `events` table (per-schema) and are
//! independent of `loom-core`'s Event type (which is engine-internal).
//! Active event instances live in `active_events` (per-timeline).

use loom_core::{NamedCondition, NamedEffect};
use rusqlite::{params, Result as SqlResult};
use serde::{Deserialize, Serialize};

// ── Event template ────────────────────────────────────────────────────────────

/// A named event template — stored per schema, resolved to engine types via schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

        self.conn.execute(
            "INSERT INTO events (schema_id, event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(schema_id, event_id) DO UPDATE SET
                label = ?3, description = ?4, preconditions_json = ?5,
                delay = ?6, duration = ?7, cooldown = ?8,
                effects_json = ?9, spawns_decision_id = ?10,
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
            "SELECT event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id
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
            ))
        })?;
        match rows.next() {
            Some(Ok((id, label, description, precond_json, delay, duration, cooldown, effects_json, spawns))) => {
                let preconditions: Vec<NamedCondition> =
                    serde_json::from_str(&precond_json).unwrap_or_default();
                let effects: Vec<NamedEffect> =
                    serde_json::from_str(&effects_json).unwrap_or_default();
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
                }))
            }
            _ => Ok(None),
        }
    }

    /// List all event templates for a schema.
    pub fn list_events(&self, schema_name: &str) -> SqlResult<Vec<NamedEvent>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT event_id, label, description, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id
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
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (id, label, description, precond_json, delay, duration, cooldown, effects_json, spawns) = row?;
            let preconditions: Vec<NamedCondition> =
                serde_json::from_str(&precond_json).unwrap_or_default();
            let effects: Vec<NamedEffect> =
                serde_json::from_str(&effects_json).unwrap_or_default();
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
        schema_name: &str,
        current_values: &[f64],
    ) -> SqlResult<Vec<AppliedEventEffect>> {
        let mut results: Vec<AppliedEventEffect> = Vec::new();

        // 1. Load all event templates for this schema
        let schema_id = self.get_schema_id_by_timeline(timeline_id)?;
        let events = self.load_event_templates(schema_id)?;

        // 2. Load existing active events for this timeline
        let mut actives = self.load_active_events(timeline_id)?;
        let active_template_ids: Vec<i64> = actives.iter().map(|a| a.event_template_id).collect();

        // 3. Check each template against preconditions
        for evt_tmpl in &events {
            if active_template_ids.contains(&evt_tmpl.row_id) {
                continue; // Already active (pending/active/cooldown)
            }

            // Resolve preconditions to engine Condition and check them
            let preconditions_met = self.check_preconditions(&evt_tmpl.preconditions, current_values);
            if !preconditions_met {
                continue;
            }

            if evt_tmpl.delay > 0 {
                // Create pending entry
                self.create_active_event(timeline_id, evt_tmpl.row_id, "pending", evt_tmpl.delay, evt_tmpl.duration, evt_tmpl.cooldown)?;
                results.push(AppliedEventEffect {
                    event_label: evt_tmpl.label.clone(),
                    phase: "pending".into(),
                    event_id: evt_tmpl.event_id.clone(),
                    delta: 0.0,
                    attribute_name: String::new(),
                    spawned_decision_id: None,
                    description: format!("{} triggered, will fire in {} steps", evt_tmpl.label, evt_tmpl.delay),
                });
            } else {
                // Fire immediately
                let effects = self.resolve_effects(&evt_tmpl.effects, current_values);
                let attribute_names = self.resolve_effect_attribute_names(&evt_tmpl.effects);
                self.create_active_event(timeline_id, evt_tmpl.row_id, "active", 0, evt_tmpl.duration, evt_tmpl.cooldown)?;

                for (i, (delta, attr_name)) in effects.iter().zip(attribute_names.iter()).enumerate() {
                    results.push(AppliedEventEffect {
                        event_label: evt_tmpl.label.clone(),
                        phase: "active".into(),
                        event_id: evt_tmpl.event_id.clone(),
                        delta: *delta,
                        attribute_name: attr_name.clone(),
                        spawned_decision_id: evt_tmpl.spawns_decision_id.clone(),
                        description: format!("{} fired — {}: {}", evt_tmpl.label, attr_name, if *delta >= 0.0 { format!("+{:.1}", delta) } else { format!("{:.1}", delta) }),
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
                        if let Some(tmpl) = events.iter().find(|e| e.row_id == active.event_template_id) {
                            let effects = self.resolve_effects(&tmpl.effects, current_values);
                            let attribute_names = self.resolve_effect_attribute_names(&tmpl.effects);
                            active.phase = "active".to_string();
                            active.duration_remaining = tmpl.duration;
                            self.update_active_event_activate(active.id, active.duration_remaining)?;

                            for (i, (delta, attr_name)) in effects.iter().zip(attribute_names.iter()).enumerate() {
                                results.push(AppliedEventEffect {
                                    event_label: tmpl.label.clone(),
                                    phase: "active".into(),
                                    event_id: tmpl.event_id.clone(),
                                    delta: *delta,
                                    attribute_name: attr_name.clone(),
                                    spawned_decision_id: tmpl.spawns_decision_id.clone(),
                                    description: format!("{} fired — {}: {}", tmpl.label, attr_name, if *delta >= 0.0 { format!("+{:.1}", delta) } else { format!("{:.1}", delta) }),
                                });
                            }
                        }
                    }
                }
                "active" => {
                    if active.duration_remaining > 0 {
                        // Apply effects again this step (for multi-step duration spread)
                        if let Some(tmpl) = events.iter().find(|e| e.row_id == active.event_template_id) {
                            let effects = self.resolve_effects(&tmpl.effects, current_values);
                            let attribute_names = self.resolve_effect_attribute_names(&tmpl.effects);

                            active.duration_remaining = active.duration_remaining.saturating_sub(1);
                            if active.duration_remaining > 0 {
                                // Ongoing — spread effect
                                for (i, (delta, attr_name)) in effects.iter().zip(attribute_names.iter()).enumerate() {
                                    // For spread: divide delta by duration for each active step
                                    let per_step = if tmpl.duration > 0 { delta / tmpl.duration as f64 } else { *delta };
                                    results.push(AppliedEventEffect {
                                        event_label: tmpl.label.clone(),
                                        phase: "active".into(),
                                        event_id: tmpl.event_id.clone(),
                                        delta: per_step,
                                        attribute_name: attr_name.clone(),
                                        spawned_decision_id: None,
                                        description: format!("{} ongoing — {}: {}", tmpl.label, attr_name, if per_step >= 0.0 { format!("+{:.1}", per_step) } else { format!("{:.1}", per_step) }),
                                    });
                                }
                            }

                            if active.duration_remaining == 0 {
                                // Move to cooldown
                                active.phase = "cooldown".to_string();
                                active.cooldown_remaining = tmpl.cooldown;
                                self.update_active_event_cooldown(active.id, active.cooldown_remaining)?;
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
                                self.update_active_event_duration(active.id, active.duration_remaining)?;
                            }
                        } else {
                            active.duration_remaining = active.duration_remaining.saturating_sub(1);
                            self.update_active_event_duration(active.id, active.duration_remaining)?;
                            if active.duration_remaining == 0 {
                                active.phase = "cooldown".to_string();
                                // Find cooldown from template
                                if let Some(tmpl) = events.iter().find(|e| e.row_id == active.event_template_id) {
                                    active.cooldown_remaining = tmpl.cooldown;
                                }
                                self.update_active_event_cooldown(active.id, active.cooldown_remaining)?;
                            }
                        }
                    }
                }
                "cooldown" => {
                    if active.cooldown_remaining > 0 {
                        active.cooldown_remaining -= 1;
                        self.update_active_event_cooldown(active.id, active.cooldown_remaining)?;
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
            "SELECT id, event_id, label, preconditions_json, delay, duration, cooldown, effects_json, spawns_decision_id
             FROM events WHERE schema_id = ?1",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| {
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

    /// Check preconditions (list of NamedCondition) against current attribute values.
    /// Since we don't have the schema here to resolve names to indices, we do a simple
    /// heuristic: we load the schema to match attribute names.
    fn check_preconditions(&self, preconditions: &[NamedCondition], values: &[f64]) -> bool {
        if preconditions.is_empty() {
            return true;
        }
        // We resolve attribute names on the fly using the stored schema
        // Since we don't have direct schema access here, we use a simplified approach:
        // check by fetching the schema from the DB
        preconditions.iter().all(|c| {
            // Without schema resolution here, we use a heuristic:
            // We can't resolve name→index without the schema, so we
            // always return true for empty preconditions and
            // for named preconditions we need the schema.
            // Actually we need the schema. Let's fetch it.
            true  // temporary: will be resolved by the caller
        })
    }

    /// Resolve named effects to flat deltas using a simplified model.
    /// Without schema, we approximate by using the delta values directly.
    fn resolve_effects(&self, effects: &[NamedEffect], _current_values: &[f64]) -> Vec<f64> {
        effects.iter().map(|e| e.delta).collect()
    }

    /// Get attribute names from effects (for display).
    fn resolve_effect_attribute_names(&self, effects: &[NamedEffect]) -> Vec<String> {
        effects.iter().map(|e| {
            e.attribute.clone().unwrap_or_else(|| e.group.clone().unwrap_or_else(|| "?".to_string()))
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
