//! Loom SQLite store — all configuration and state persistence.
//!
//! # Tables
//!
//! - `schemas` — attribute definitions
//! - `decisions` — per-schema decisions with outcomes
//! - `passives` — per-schema recurring effects
//! - `goals` — per-schema goal vectors
//! - `states` — saved simulation state snapshots
//! - `timelines` — named timelines (git-for-life snapshots)
//! - `snapshots` — ordered attribute-value snapshots with journal entries
//! - `forks` — timeline fork records
//!
//! # Architecture
//!
//! The store returns `Named*` types from `loom-core`. Consumers resolve them to engine
//! types via `resolve(&schema)`. The engine never touches the database.
//!
//! Database location: `~/.loom/loom.db` (unified — schemas + states + timelines in one file).

pub mod event;
pub mod template;
pub mod timeline;

pub use event::{AppliedEventEffect, NamedEvent, PreconditionMode};
pub use template::Template;
pub use timeline::{ForkRow, SnapshotRow, TimelineRow, TimelineStore, TimelineSummary};

use loom_core::{
    AttributeSchema, NamedCondition, NamedDecision, NamedEffect, NamedFrequency,
    NamedGoalVector, NamedOutcome, NamedPassiveEffect, Threshold,
};
#[cfg(test)]
use loom_core::NamedTransform;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(dead_code)]
const DB_PATH: &str = ".loom/loom.db";

// ── Store ──────────────────────────────────────────────────────────────────────────

pub struct Store {
    pub conn: Connection,
}

impl Store {
    /// Open (or create) the database at the default path `~/.loom/loom.db`.
    pub fn open_default() -> SqlResult<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let dir = format!("{home}/.loom");
        std::fs::create_dir_all(&dir).ok();
        Self::open(&format!("{dir}/loom.db"))
    }

    /// Open (or create) the database at a custom path.
    pub fn open(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schemas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                attributes_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schema_id INTEGER NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,
                decision_id TEXT NOT NULL,
                label TEXT NOT NULL,
                preconditions_json TEXT NOT NULL DEFAULT '[]',
                cost_json TEXT NOT NULL DEFAULT '[]',
                outcomes_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(schema_id, decision_id)
            );

            CREATE TABLE IF NOT EXISTS passives (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schema_id INTEGER NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,
                passive_id TEXT NOT NULL,
                label TEXT NOT NULL,
                frequency_json TEXT NOT NULL,
                effects_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(schema_id, passive_id)
            );

            CREATE TABLE IF NOT EXISTS goals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schema_id INTEGER NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                weights_json TEXT NOT NULL,
                cliffs_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(schema_id, name)
            );

            CREATE TABLE IF NOT EXISTS states (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                note TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                state_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS timelines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                schema_id INTEGER NOT NULL REFERENCES schemas(id),
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timeline_id INTEGER NOT NULL REFERENCES timelines(id) ON DELETE CASCADE,
                parent_id INTEGER REFERENCES snapshots(id),
                step INTEGER NOT NULL,
                attributes_json TEXT NOT NULL,
                entry_text TEXT NOT NULL DEFAULT '',
                decision_id INTEGER,
                decision_chosen_outcome TEXT,
                forecast_json TEXT,
                actual_outcome_json TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS forks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_timeline_id INTEGER NOT NULL REFERENCES timelines(id),
                fork_snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                child_timeline_id INTEGER NOT NULL REFERENCES timelines(id),
                label TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                schema_id INTEGER NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,
                event_id TEXT NOT NULL,
                label TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                preconditions_json TEXT NOT NULL DEFAULT '[]',
                delay INTEGER NOT NULL DEFAULT 0,
                duration INTEGER NOT NULL DEFAULT 1,
                cooldown INTEGER NOT NULL DEFAULT 0,
                effects_json TEXT NOT NULL DEFAULT '[]',
                spawns_decision_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(schema_id, event_id)
            );

            CREATE TABLE IF NOT EXISTS active_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timeline_id INTEGER NOT NULL REFERENCES timelines(id) ON DELETE CASCADE,
                event_template_id INTEGER NOT NULL REFERENCES events(id),
                phase TEXT NOT NULL DEFAULT 'pending',
                delay_remaining INTEGER NOT NULL DEFAULT 0,
                duration_remaining INTEGER NOT NULL DEFAULT 0,
                cooldown_remaining INTEGER NOT NULL DEFAULT 0,
                UNIQUE(timeline_id, event_template_id)
            );",
        )?;

        // Phase 2 event economy migration: add new columns to events table.
        // SQLite doesn't support IF NOT EXISTS for ALTER TABLE, so we check
        // PRAGMA table_info first.
        {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(events)")?;
            let existing: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            if !existing.contains(&"triggered_by_json".to_string()) {
                self.conn.execute_batch(
                    "ALTER TABLE events ADD COLUMN triggered_by_json TEXT NOT NULL DEFAULT '[]';
                     ALTER TABLE events ADD COLUMN suppressed_by_json TEXT NOT NULL DEFAULT '[]';
                     ALTER TABLE events ADD COLUMN triggers_event_id TEXT;
                     ALTER TABLE events ADD COLUMN triggers_on_resolve TEXT;
                     ALTER TABLE events ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE events ADD COLUMN precondition_mode TEXT NOT NULL DEFAULT 'All';",
                )?;
            }
        }
        Ok(())
    }
}

// ── Schema CRUD ────────────────────────────────────────────────────────────────────

/// Summary row returned by list_schemas — lightweight, no attribute data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSummary {
    pub id: i64,
    pub name: String,
    pub attribute_count: usize,
    pub created_at: String,
}

impl Store {
    /// Insert or replace a schema. `attributes_json` should be the `"attributes"` array
    /// from the schema JSON (the `[AttributeDef, ...]` array, not the full schema object).
    pub fn upsert_schema(&self, name: &str, attributes_json: &str) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO schemas (name, attributes_json) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET attributes_json = ?2, created_at = datetime('now')",
            params![name, attributes_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Load a full `AttributeSchema` by name.
    pub fn get_schema(&self, name: &str) -> SqlResult<Option<AttributeSchema>> {
        let mut stmt = self
            .conn
            .prepare("SELECT attributes_json FROM schemas WHERE name = ?1")?;
        let mut rows = stmt.query_map(params![name], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })?;
        match rows.next() {
            Some(Ok(json_str)) => {
                // Reconstruct full schema JSON: {"version":1,"attributes":[...]}
                let full = format!(r#"{{"version":1,"attributes":{}}}"#, json_str);
                let schema: AttributeSchema =
                    serde_json::from_str(&full).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                Ok(Some(schema))
            }
            _ => Ok(None),
        }
    }

    /// List all schemas (summary only — no attribute data).
    pub fn list_schemas(&self) -> SqlResult<Vec<SchemaSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, attributes_json, created_at FROM schemas ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(2)?;
            let count = json_str.matches("\"name\"").count();
            Ok(SchemaSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                attribute_count: count,
                created_at: row.get(3)?,
            })
        })?;
        let mut schemas = Vec::new();
        for row in rows {
            schemas.push(row?);
        }
        Ok(schemas)
    }

    /// Delete a schema and all its associated decisions, passives, and goals (CASCADE).
    pub fn delete_schema(&self, name: &str) -> SqlResult<usize> {
        self.conn
            .execute("DELETE FROM schemas WHERE name = ?1", params![name])
    }
}

// ── Decision CRUD ──────────────────────────────────────────────────────────────────

impl Store {
    /// Insert or replace a decision. Returns the row ID.
    pub fn upsert_decision(
        &self,
        schema_name: &str,
        decision: &NamedDecision,
    ) -> SqlResult<i64> {
        let schema_id = self.schema_id(schema_name)?;
        let preconditions_json = serde_json::to_string(&decision.preconditions).unwrap();
        let cost_json = serde_json::to_string(&decision.cost).unwrap();
        let outcomes_json = serde_json::to_string(&decision.outcomes).unwrap();

        self.conn.execute(
            "INSERT INTO decisions (schema_id, decision_id, label, preconditions_json, cost_json, outcomes_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(schema_id, decision_id) DO UPDATE SET
                label = ?3, preconditions_json = ?4, cost_json = ?5, outcomes_json = ?6,
                created_at = datetime('now')",
            params![
                schema_id,
                decision.id,
                decision.label,
                preconditions_json,
                cost_json,
                outcomes_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Load a single decision by (schema_name, decision_id).
    pub fn get_decision(
        &self,
        schema_name: &str,
        decision_id: &str,
    ) -> SqlResult<Option<NamedDecision>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT decision_id, label, preconditions_json, cost_json, outcomes_json
             FROM decisions WHERE schema_id = ?1 AND decision_id = ?2",
        )?;
        let mut rows = stmt.query_map(params![schema_id, decision_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        match rows.next() {
            Some(Ok((id, label, precond_json, cost_json, outcomes_json))) => {
                let preconditions: Vec<NamedCondition> =
                    serde_json::from_str(&precond_json).unwrap_or_default();
                let cost: Vec<NamedEffect> =
                    serde_json::from_str(&cost_json).unwrap_or_default();
                let outcomes: Vec<NamedOutcome> =
                    serde_json::from_str(&outcomes_json).unwrap_or_default();
                Ok(Some(NamedDecision {
                    id,
                    label,
                    preconditions,
                    cost,
                    outcomes,
                }))
            }
            _ => Ok(None),
        }
    }

    /// List all decisions for a schema.
    pub fn list_decisions(&self, schema_name: &str) -> SqlResult<Vec<NamedDecision>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT decision_id, label, preconditions_json, cost_json, outcomes_json
             FROM decisions WHERE schema_id = ?1 ORDER BY decision_id",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut decs = Vec::new();
        for row in rows {
            let (id, label, precond_json, cost_json, outcomes_json) = row?;
            let preconditions: Vec<NamedCondition> =
                serde_json::from_str(&precond_json).unwrap_or_default();
            let cost: Vec<NamedEffect> =
                serde_json::from_str(&cost_json).unwrap_or_default();
            let outcomes: Vec<NamedOutcome> =
                serde_json::from_str(&outcomes_json).unwrap_or_default();
            decs.push(NamedDecision {
                id,
                label,
                preconditions,
                cost,
                outcomes,
            });
        }
        Ok(decs)
    }

    /// Delete a decision.
    pub fn delete_decision(&self, schema_name: &str, decision_id: &str) -> SqlResult<usize> {
        let schema_id = self.schema_id(schema_name)?;
        self.conn.execute(
            "DELETE FROM decisions WHERE schema_id = ?1 AND decision_id = ?2",
            params![schema_id, decision_id],
        )
    }
}

// ── Passives CRUD ──────────────────────────────────────────────────────────────────

impl Store {
    /// Insert or replace a passive effect.
    pub fn upsert_passive(
        &self,
        schema_name: &str,
        passive: &NamedPassiveEffect,
    ) -> SqlResult<i64> {
        let schema_id = self.schema_id(schema_name)?;
        let frequency_json = serde_json::to_string(&passive.frequency).unwrap();
        let effects_json = serde_json::to_string(&passive.effects).unwrap();

        self.conn.execute(
            "INSERT INTO passives (schema_id, passive_id, label, frequency_json, effects_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(schema_id, passive_id) DO UPDATE SET
                label = ?3, frequency_json = ?4, effects_json = ?5,
                created_at = datetime('now')",
            params![schema_id, passive.id, passive.label, frequency_json, effects_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List all passives for a schema.
    pub fn list_passives(&self, schema_name: &str) -> SqlResult<Vec<NamedPassiveEffect>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT passive_id, label, frequency_json, effects_json
             FROM passives WHERE schema_id = ?1 ORDER BY passive_id",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut passives = Vec::new();
        for row in rows {
            let (id, label, freq_json, effects_json) = row?;
            let frequency: NamedFrequency =
                serde_json::from_str(&freq_json).unwrap_or(NamedFrequency::EveryStep);
            let effects: Vec<NamedEffect> =
                serde_json::from_str(&effects_json).unwrap_or_default();
            passives.push(NamedPassiveEffect {
                id,
                label,
                frequency,
                effects,
            });
        }
        Ok(passives)
    }

    /// Delete a passive.
    pub fn delete_passive(&self, schema_name: &str, passive_id: &str) -> SqlResult<usize> {
        let schema_id = self.schema_id(schema_name)?;
        self.conn.execute(
            "DELETE FROM passives WHERE schema_id = ?1 AND passive_id = ?2",
            params![schema_id, passive_id],
        )
    }
}

// ── Goals CRUD ─────────────────────────────────────────────────────────────────────

impl Store {
    /// Insert or replace a goal vector.
    pub fn upsert_goal(&self, schema_name: &str, name: &str, goal: &NamedGoalVector) -> SqlResult<i64> {
        let schema_id = self.schema_id(schema_name)?;
        let weights_json = serde_json::to_string(&goal.weights).unwrap();
        let cliffs_json = serde_json::to_string(&goal.cliffs).unwrap();

        self.conn.execute(
            "INSERT INTO goals (schema_id, name, weights_json, cliffs_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(schema_id, name) DO UPDATE SET
                weights_json = ?3, cliffs_json = ?4, created_at = datetime('now')",
            params![schema_id, name, weights_json, cliffs_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Load a goal by (schema_name, goal_name).
    pub fn get_goal(
        &self,
        schema_name: &str,
        goal_name: &str,
    ) -> SqlResult<Option<NamedGoalVector>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT weights_json, cliffs_json FROM goals WHERE schema_id = ?1 AND name = ?2",
        )?;
        let mut rows = stmt.query_map(params![schema_id, goal_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        match rows.next() {
            Some(Ok((weights_json, cliffs_json))) => {
                let weights: HashMap<String, f64> =
                    serde_json::from_str(&weights_json).unwrap_or_default();
                let cliffs: HashMap<String, Threshold> =
                    serde_json::from_str(&cliffs_json).unwrap_or_default();
                Ok(Some(NamedGoalVector { weights, cliffs }))
            }
            _ => Ok(None),
        }
    }

    /// List all goal names for a schema.
    pub fn list_goals(&self, schema_name: &str) -> SqlResult<Vec<String>> {
        let schema_id = self.schema_id(schema_name)?;
        let mut stmt = self.conn.prepare(
            "SELECT name FROM goals WHERE schema_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![schema_id], |row| row.get(0))?;
        let mut names = Vec::new();
        for row in rows {
            names.push(row?);
        }
        Ok(names)
    }

    /// Delete a goal.
    pub fn delete_goal(&self, schema_name: &str, goal_name: &str) -> SqlResult<usize> {
        let schema_id = self.schema_id(schema_name)?;
        self.conn.execute(
            "DELETE FROM goals WHERE schema_id = ?1 AND name = ?2",
            params![schema_id, goal_name],
        )
    }
}

// ── State persistence ──────────────────────────────────────────────────────────────

/// A saved state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub created_at: String,
    pub values: Vec<f64>,
}

impl Store {
    /// Save a state snapshot. Updates if a state with the same name exists.
    pub fn save_state(&self, name: &str, note: &str, values: &[f64]) -> SqlResult<()> {
        let json = serde_json::to_string(values).unwrap();
        self.conn.execute(
            "INSERT INTO states (name, note, state_json) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET note = ?2, state_json = ?3, created_at = datetime('now')",
            params![name, note, json],
        )?;
        Ok(())
    }

    /// Load a state by name. Returns None if not found.
    pub fn load_state(&self, name: &str) -> SqlResult<Option<SavedState>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, note, created_at, state_json FROM states WHERE name = ?1",
        )?;
        let mut rows = stmt.query_map(params![name], |row| {
            let json_str: String = row.get(4)?;
            let values: Vec<f64> = serde_json::from_str(&json_str).unwrap_or_default();
            Ok(SavedState {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                created_at: row.get(3)?,
                values,
            })
        })?;
        match rows.next() {
            Some(Ok(state)) => Ok(Some(state)),
            _ => Ok(None),
        }
    }

    /// List all saved states, most recent first.
    pub fn list_states(&self) -> SqlResult<Vec<SavedState>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, note, created_at, state_json FROM states ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(4)?;
            let values: Vec<f64> = serde_json::from_str(&json_str).unwrap_or_default();
            Ok(SavedState {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                created_at: row.get(3)?,
                values,
            })
        })?;
        let mut states = Vec::new();
        for row in rows {
            states.push(row?);
        }
        Ok(states)
    }

    /// Delete a state by name.
    pub fn delete_state(&self, name: &str) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM states WHERE name = ?1", params![name])
    }

    /// Branch: clone a saved state under a new name.
    pub fn branch_state(&self, from_name: &str, new_name: &str, note: &str) -> SqlResult<bool> {
        if let Some(source) = self.load_state(from_name)? {
            self.save_state(new_name, note, &source.values)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// ── Template seeding ──────────────────────────────────────────────────────────────

impl Store {
    /// Seed the database from a template — upserts everything in one call.
    /// Returns the schema name used (template.name, lowercased with underscores).
    pub fn seed_from_template(&self, template: &Template) -> SqlResult<String> {
        let schema_name = template.name.to_lowercase().replace(' ', "_");

        // 1. Schema
        let attributes_json = serde_json::to_string(&template.schema.attributes).unwrap();
        self.upsert_schema(&schema_name, &attributes_json)?;

        // 2. Decisions
        for d in &template.decisions {
            self.upsert_decision(&schema_name, d)?;
        }

        // 3. Passives
        for p in &template.passives {
            self.upsert_passive(&schema_name, p)?;
        }

        // 4. Goals
        for (name, goal) in &template.goals {
            self.upsert_goal(&schema_name, name, goal)?;
        }

        // 5. Events
        for e in &template.events {
            self.upsert_event(&schema_name, e)?;
        }

        Ok(schema_name)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────────

impl Store {
    /// Resolve a schema name to its internal ID.
    fn schema_id(&self, name: &str) -> SqlResult<i64> {
        self.conn
            .query_row(
                "SELECT id FROM schemas WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Store {
        // Use in-memory DB for tests
        Store::open(":memory:").unwrap()
    }

    fn test_schema_json() -> &'static str {
        r#"[
            {"name": "wealth.cash", "unit": "$"},
            {"name": "health.stress", "unit": "pts", "bounds": [0, 100]},
            {"name": "skills.rust", "unit": "pts", "bounds": [0, 100]}
        ]"#
    }

    #[test]
    fn schema_crud() {
        let store = test_store();
        store.upsert_schema("test", test_schema_json()).unwrap();

        let schema = store.get_schema("test").unwrap().unwrap();
        assert_eq!(schema.dimension(), 3);
        assert_eq!(schema.attributes[0].name, "wealth.cash");

        let list = store.list_schemas().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test");

        store.delete_schema("test").unwrap();
        assert!(store.get_schema("test").unwrap().is_none());
    }

    #[test]
    fn decision_crud() {
        let store = test_store();
        store.upsert_schema("test", test_schema_json()).unwrap();

        let dec = NamedDecision {
            id: "quit_job".into(),
            label: "Quit job".into(),
            preconditions: vec![NamedCondition {
                attribute: "wealth.cash".into(),
                operator: loom_core::ComparisonOp::Gt,
                value: 10000.0,
            }],
            cost: vec![NamedEffect::fixed("health.stress", -20.0)],
            outcomes: vec![NamedOutcome {
                label: "Freedom".into(),
                weight: 100.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![NamedEffect::fixed("wealth.cash", -50000.0)],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            }],
        };

        store.upsert_decision("test", &dec).unwrap();

        let loaded = store.get_decision("test", "quit_job").unwrap().unwrap();
        assert_eq!(loaded.id, "quit_job");
        assert_eq!(loaded.preconditions.len(), 1);
        assert_eq!(loaded.outcomes[0].label, "Freedom");

        let list = store.list_decisions("test").unwrap();
        assert_eq!(list.len(), 1);

        store.delete_decision("test", "quit_job").unwrap();
        assert!(store.get_decision("test", "quit_job").unwrap().is_none());
    }

    #[test]
    fn passives_crud() {
        let store = test_store();
        store.upsert_schema("test", test_schema_json()).unwrap();

        let passive = NamedPassiveEffect {
            id: "salary".into(),
            label: "Monthly salary".into(),
            frequency: NamedFrequency::EveryStep,
            effects: vec![NamedEffect::fixed("wealth.cash", 3333.33)],
        };

        store.upsert_passive("test", &passive).unwrap();

        let list = store.list_passives("test").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "salary");
        assert_eq!(list[0].effects[0].delta, 3333.33);
    }

    #[test]
    fn goals_crud() {
        let store = test_store();
        store.upsert_schema("test", test_schema_json()).unwrap();

        let mut weights = HashMap::new();
        weights.insert("wealth.cash".to_string(), 1.0);
        weights.insert("health.stress".to_string(), -0.3);
        let mut cliffs = HashMap::new();
        cliffs.insert(
            "health.stress".to_string(),
            Threshold {
                min: 30.0,
                penalty: 1.0,
            },
        );

        let goal = NamedGoalVector { weights, cliffs };
        store.upsert_goal("test", "default", &goal).unwrap();

        let loaded = store.get_goal("test", "default").unwrap().unwrap();
        assert!((*loaded.weights.get("wealth.cash").unwrap() - 1.0).abs() < f64::EPSILON);
        assert_eq!(loaded.cliffs.get("health.stress").unwrap().min, 30.0);
    }

    #[test]
    fn state_crud() {
        let store = test_store();

        let values = vec![50000.0, 30.0, 70.0];
        store.save_state("start", "initial state", &values).unwrap();

        let loaded = store.load_state("start").unwrap().unwrap();
        assert_eq!(loaded.values, values);
        assert_eq!(loaded.note, "initial state");

        let list = store.list_states().unwrap();
        assert_eq!(list.len(), 1);

        store.branch_state("start", "fork", "branched").unwrap();
        assert_eq!(store.list_states().unwrap().len(), 2);

        store.delete_state("start").unwrap();
        assert_eq!(store.list_states().unwrap().len(), 1);
    }

    #[test]
    fn cascade_delete() {
        let store = test_store();
        store.upsert_schema("test", test_schema_json()).unwrap();

        let dec = NamedDecision {
            id: "d".into(),
            label: "D".into(),
            preconditions: vec![],
            cost: vec![],
            outcomes: vec![],
        };
        store.upsert_decision("test", &dec).unwrap();

        let passive = NamedPassiveEffect {
            id: "p".into(),
            label: "P".into(),
            frequency: NamedFrequency::EveryStep,
            effects: vec![],
        };
        store.upsert_passive("test", &passive).unwrap();

        let mut w = HashMap::new();
        w.insert("wealth.cash".to_string(), 1.0);
        store
            .upsert_goal("test", "g", &NamedGoalVector { weights: w, cliffs: HashMap::new() })
            .unwrap();

        // Delete schema — children should cascade
        store.delete_schema("test").unwrap();
        assert!(store.list_decisions("test").is_err()); // schema_id no longer exists
    }
}
