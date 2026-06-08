//! Timeline layer — git-for-life snapshots, forks, and forecast-vs-actual tracking.
//!
//! # Tables
//!
//! - `timelines` — named timelines, each tied to a schema
//! - `snapshots` — ordered attribute-value snapshots with journal entries
//! - `forks` — record of a timeline fork (branch point to child timeline)
//!
//! # Architecture
//!
//! `TimelineStore` wraps a `rusqlite::Connection` borrow and provides CRUD
//! for timelines, snapshots, and forks. It is a complementary module to the
//! main `Store` — they share the same database file.

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

// ── Data types ──────────────────────────────────────────────────────────────────

/// Lightweight timeline summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSummary {
    pub id: i64,
    pub name: String,
    pub schema_id: i64,
    pub snapshot_count: usize,
    pub created_at: String,
}

/// Full timeline row (no snapshot count).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineRow {
    pub id: i64,
    pub name: String,
    pub schema_id: i64,
    pub created_at: String,
}

/// A single snapshot in a timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRow {
    pub id: i64,
    pub timeline_id: i64,
    pub parent_id: Option<i64>,
    pub step: i64,
    pub attributes_json: String,
    pub entry_text: String,
    pub decision_id: Option<i64>,
    pub decision_chosen_outcome: Option<String>,
    pub forecast_json: Option<String>,
    pub actual_outcome_json: Option<String>,
    pub created_at: String,
}

/// A fork record linking a parent timeline snapshot to a child timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkRow {
    pub id: i64,
    pub parent_timeline_id: i64,
    pub fork_snapshot_id: i64,
    pub child_timeline_id: i64,
    pub label: String,
    pub created_at: String,
}

// ── TimelineStore ───────────────────────────────────────────────────────────────

pub struct TimelineStore<'a>(pub &'a Connection);

impl<'a> TimelineStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self(conn)
    }

    // ── Timelines ──────────────────────────────────────────────────────────────

    /// Create a new timeline for a schema. Returns the timeline ID.
    pub fn create_timeline(&self, name: &str, schema_id: i64) -> SqlResult<i64> {
        self.0.execute(
            "INSERT INTO timelines (name, schema_id) VALUES (?1, ?2)",
            params![name, schema_id],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// List all timelines with snapshot counts.
    pub fn list_timelines(&self) -> SqlResult<Vec<TimelineSummary>> {
        let mut stmt = self.0.prepare(
            "SELECT t.id, t.name, t.schema_id, t.created_at,
                    (SELECT COUNT(*) FROM snapshots s WHERE s.timeline_id = t.id) AS snapshot_count
             FROM timelines t ORDER BY t.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TimelineSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_id: row.get(2)?,
                snapshot_count: row.get::<_, i64>(4)? as usize,
                created_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Get a single timeline by ID.
    pub fn get_timeline(&self, id: i64) -> SqlResult<Option<TimelineRow>> {
        let mut stmt = self
            .0
            .prepare("SELECT id, name, schema_id, created_at FROM timelines WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(TimelineRow {
                id: row.get(0)?,
                name: row.get(1)?,
                schema_id: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            _ => Ok(None),
        }
    }

    /// Delete a timeline (cascades to snapshots and forks).
    pub fn delete_timeline(&self, id: i64) -> SqlResult<()> {
        self.0.execute("DELETE FROM timelines WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ── Snapshots ──────────────────────────────────────────────────────────────

    /// Append a snapshot to a timeline. Auto-increments step.
    /// parent_id is the previous snapshot's ID (or None for root).
    pub fn append_snapshot(
        &self,
        timeline_id: i64,
        parent_id: Option<i64>,
        entry_text: &str,
        values: &[f64],
    ) -> SqlResult<i64> {
        let attributes_json = serde_json::to_string(values).unwrap();
        // Determine next step number
        let next_step: i64 = self
            .0
            .query_row(
                "SELECT COALESCE(MAX(step), -1) + 1 FROM snapshots WHERE timeline_id = ?1",
                params![timeline_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.0.execute(
            "INSERT INTO snapshots (timeline_id, parent_id, step, attributes_json, entry_text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timeline_id, parent_id, next_step, attributes_json, entry_text],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// List all snapshots for a timeline, ordered by step ascending.
    pub fn list_snapshots(&self, timeline_id: i64) -> SqlResult<Vec<SnapshotRow>> {
        let mut stmt = self.0.prepare(
            "SELECT id, timeline_id, parent_id, step, attributes_json, entry_text,
                    decision_id, decision_chosen_outcome, forecast_json, actual_outcome_json, created_at
             FROM snapshots WHERE timeline_id = ?1 ORDER BY step ASC",
        )?;
        let rows = stmt.query_map(params![timeline_id], |row| {
            Ok(SnapshotRow {
                id: row.get(0)?,
                timeline_id: row.get(1)?,
                parent_id: row.get(2)?,
                step: row.get(3)?,
                attributes_json: row.get(4)?,
                entry_text: row.get(5)?,
                decision_id: row.get(6)?,
                decision_chosen_outcome: row.get(7)?,
                forecast_json: row.get(8)?,
                actual_outcome_json: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Get a single snapshot by ID.
    pub fn get_snapshot(&self, id: i64) -> SqlResult<Option<SnapshotRow>> {
        let mut stmt = self.0.prepare(
            "SELECT id, timeline_id, parent_id, step, attributes_json, entry_text,
                    decision_id, decision_chosen_outcome, forecast_json, actual_outcome_json, created_at
             FROM snapshots WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(SnapshotRow {
                id: row.get(0)?,
                timeline_id: row.get(1)?,
                parent_id: row.get(2)?,
                step: row.get(3)?,
                attributes_json: row.get(4)?,
                entry_text: row.get(5)?,
                decision_id: row.get(6)?,
                decision_chosen_outcome: row.get(7)?,
                forecast_json: row.get(8)?,
                actual_outcome_json: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            _ => Ok(None),
        }
    }

    /// Attach a decision forecast to a snapshot.
    pub fn attach_forecast(
        &self,
        snapshot_id: i64,
        decision_id: i64,
        chosen_outcome: &str,
        forecast_json: &str,
    ) -> SqlResult<()> {
        self.0.execute(
            "UPDATE snapshots SET decision_id = ?1, decision_chosen_outcome = ?2, forecast_json = ?3 WHERE id = ?4",
            params![decision_id, chosen_outcome, forecast_json, snapshot_id],
        )?;
        Ok(())
    }

    /// Resolve a snapshot's actual outcome (post-hoc correction).
    pub fn resolve_outcome(&self, snapshot_id: i64, actual_deltas_json: &str) -> SqlResult<()> {
        self.0.execute(
            "UPDATE snapshots SET actual_outcome_json = ?1 WHERE id = ?2",
            params![actual_deltas_json, snapshot_id],
        )?;
        Ok(())
    }

    // ── Forks ──────────────────────────────────────────────────────────────────

    /// Fork a timeline: creates a new timeline, copies all snapshots up to (and
    /// including) the fork point, returns the child timeline ID.
    pub fn fork_timeline(
        &self,
        parent_timeline_id: i64,
        at_snapshot_id: i64,
        new_name: &str,
        label: &str,
    ) -> SqlResult<i64> {
        // Get the parent timeline to find its schema_id
        let parent = self
            .get_timeline(parent_timeline_id)?
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("parent timeline not found".into()))?;

        // Create the child timeline
        let child_id = self.create_timeline(new_name, parent.schema_id)?;

        // Get all snapshots up to (and including) the fork point
        let mut stmt = self.0.prepare(
            "SELECT id, parent_id, step, attributes_json, entry_text,
                    decision_id, decision_chosen_outcome, forecast_json, actual_outcome_json, created_at
             FROM snapshots WHERE timeline_id = ?1 AND step <= (
                 SELECT step FROM snapshots WHERE id = ?2
             ) ORDER BY step ASC",
        )?;
        let rows = stmt.query_map(
            params![parent_timeline_id, at_snapshot_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(1)?, // parent_id
                    row.get::<_, i64>(2)?,          // step
                    row.get::<_, String>(3)?,       // attributes_json
                    row.get::<_, String>(4)?,       // entry_text
                    row.get::<_, Option<i64>>(5)?,  // decision_id
                    row.get::<_, Option<String>>(6)?, // decision_chosen_outcome
                    row.get::<_, Option<String>>(7)?, // forecast_json
                    row.get::<_, Option<String>>(8)?, // actual_outcome_json
                ))
            },
        )?;

        let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for r in rows {
            let (
                old_parent_id,
                _step,
                attributes_json,
                entry_text,
                decision_id,
                decision_chosen_outcome,
                forecast_json,
                actual_outcome_json,
            ) = r?;

            let new_parent_id = old_parent_id.and_then(|pid| id_map.get(&pid).copied());

            self.0.execute(
                "INSERT INTO snapshots (timeline_id, parent_id, step, attributes_json, entry_text,
                        decision_id, decision_chosen_outcome, forecast_json, actual_outcome_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    child_id,
                    new_parent_id,
                    _step,
                    attributes_json,
                    entry_text,
                    decision_id,
                    decision_chosen_outcome,
                    forecast_json,
                    actual_outcome_json,
                ],
            )?;
            let new_id = self.0.last_insert_rowid();
            // For root snapshot, old_parent_id is None so we only insert into map when old_parent_id is Some
            if let Some(oid) = old_parent_id {
                id_map.insert(oid, new_id);
            } else {
                // Also insert root into map
                id_map.insert(at_snapshot_id, new_id);
            }
        }

        // Record the fork
        self.0.execute(
            "INSERT INTO forks (parent_timeline_id, fork_snapshot_id, child_timeline_id, label)
             VALUES (?1, ?2, ?3, ?4)",
            params![parent_timeline_id, at_snapshot_id, child_id, label],
        )?;

        Ok(child_id)
    }

    /// List forks originating from a timeline.
    pub fn list_forks(&self, timeline_id: i64) -> SqlResult<Vec<ForkRow>> {
        let mut stmt = self.0.prepare(
            "SELECT id, parent_timeline_id, fork_snapshot_id, child_timeline_id, label, created_at
             FROM forks WHERE parent_timeline_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![timeline_id], |row| {
            Ok(ForkRow {
                id: row.get(0)?,
                parent_timeline_id: row.get(1)?,
                fork_snapshot_id: row.get(2)?,
                child_timeline_id: row.get(3)?,
                label: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
