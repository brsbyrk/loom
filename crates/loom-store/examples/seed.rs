//! Seed the database with the example configuration from JSON files.
//!
//! Reads the existing example configs and inserts them into ~/.loom/loom.db
//! under the schema name "personal".
//!
//! Usage:
//! ```bash
//! cargo run --example seed -p loom-store
//! ```

use loom_core::{NamedDecision, NamedGoalVector, NamedPassiveEffect};
use loom_store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/configs");

    let store = Store::open_default()?;

    // ── Schema ──────────────────────────────────────────────────────────────────
    let schema_json = std::fs::read_to_string(format!("{config_dir}/attribute_schema.json"))?;
    // Parse to get just the attributes array
    let schema_value: serde_json::Value = serde_json::from_str(&schema_json)?;
    let attributes = &schema_value["attributes"];
    let attributes_str = serde_json::to_string(attributes)?;

    store.upsert_schema("personal", &attributes_str)?;
    println!("✓ Schema 'personal' upserted");

    // ── Decision ────────────────────────────────────────────────────────────────
    let decision_json = std::fs::read_to_string(format!("{config_dir}/job_decision.json"))?;
    let decision: NamedDecision = serde_json::from_str(&decision_json)?;
    store.upsert_decision("personal", &decision)?;
    println!("✓ Decision '{}' upserted", decision.id);

    // ── Passives ────────────────────────────────────────────────────────────────
    let passives_json = std::fs::read_to_string(format!("{config_dir}/passives.json"))?;
    let passives: Vec<NamedPassiveEffect> = serde_json::from_str(&passives_json)?;
    for p in &passives {
        store.upsert_passive("personal", p)?;
        println!("✓ Passive '{}' upserted", p.id);
    }

    // ── Goal ────────────────────────────────────────────────────────────────────
    let goal_json = std::fs::read_to_string(format!("{config_dir}/goal.json"))?;
    let goal: NamedGoalVector = serde_json::from_str(&goal_json)?;
    store.upsert_goal("personal", "default", &goal)?;
    println!("✓ Goal 'default' upserted");

    // ── Verify ──────────────────────────────────────────────────────────────────
    println!();
    println!("── Verification ──");
    let schemas = store.list_schemas()?;
    println!("Schemas: {}", schemas.len());
    let decs = store.list_decisions("personal")?;
    println!("Decisions: {}", decs.len());
    let pass = store.list_passives("personal")?;
    println!("Passives: {}", pass.len());
    let goals = store.list_goals("personal")?;
    println!("Goals: {}", goals.len());

    println!();
    println!("Done. DB at ~/.loom/loom.db");
    Ok(())
}
