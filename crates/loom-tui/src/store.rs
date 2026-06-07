//! State persistence — save, load, branch, and compare simulation states.
//!
//! States are stored as JSON float arrays in a SQLite database. The schema
//! is NOT stored (it's loaded from config files and assumed stable for saved states).
//!
//! Database location: `~/.loom/states.db`

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

/// A saved state snapshot, as loaded from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub created_at: String,
    pub values: Vec<f64>,
}

/// State store backed by SQLite.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) the state database at the given path.
    pub fn open(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS states (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                note TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                state_json TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }

    /// Open the default store at ~/.loom/states.db.
    pub fn open_default() -> SqlResult<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let dir = format!("{home}/.loom");
        std::fs::create_dir_all(&dir).ok();
        Self::open(&format!("{dir}/states.db"))
    }

    /// Save a state snapshot. Updates if a state with the same name exists.
    pub fn save(&self, name: &str, note: &str, values: &[f64]) -> SqlResult<()> {
        let json = serde_json::to_string(values).unwrap();
        self.conn.execute(
            "INSERT INTO states (name, note, state_json) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET note = ?2, state_json = ?3, created_at = datetime('now')",
            params![name, note, json],
        )?;
        Ok(())
    }

    /// Load a state by name. Returns None if not found.
    pub fn load(&self, name: &str) -> SqlResult<Option<SavedState>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, note, created_at, state_json FROM states WHERE name = ?1")?;
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
    pub fn list(&self) -> SqlResult<Vec<SavedState>> {
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
    pub fn delete(&self, name: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM states WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// Branch: clone a saved state under a new name.
    pub fn branch(&self, from_name: &str, new_name: &str, note: &str) -> SqlResult<bool> {
        let source = self.load(from_name)?;
        match source {
            Some(s) => {
                self.save(new_name, note, &s.values)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}
