//! Seed the database with the example configuration from JSON files.
//!
//! Reads the existing example configs from both `examples/configs/` (schema "personal")
//! and `examples/configs_financial/` (schema "financial") and inserts them into
//! ~/.loom/loom.db.
//!
//! Usage:
//! ```bash
//! cargo run --example seed -p loom-store
//! ```

use loom_core::{NamedDecision, NamedGoalVector, NamedPassiveEffect};
use loom_store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let store = Store::open_default()?;

    // ── Seed "personal" schema from examples/configs/ ────────────────────────────
    seed_schema(
        &store,
        "personal",
        &format!("{manifest_dir}/../../examples/configs"),
    )?;

    // ── Seed "financial" schema from examples/configs_financial/ ─────────────────
    seed_schema(
        &store,
        "financial",
        &format!("{manifest_dir}/../../examples/configs_financial"),
    )?;

    // ── Verify ──────────────────────────────────────────────────────────────────
    println!();
    println!("── Verification ──");
    let schemas = store.list_schemas()?;
    println!("Schemas: {}", schemas.len());
    for s in &schemas {
        let decs = store.list_decisions(&s.name)?;
        let pass = store.list_passives(&s.name)?;
        let goals = store.list_goals(&s.name)?;
        println!(
            "  {}: {} decisions, {} passives, {} goals",
            s.name,
            decs.len(),
            pass.len(),
            goals.len()
        );
    }

    println!();
    println!("Done. DB at ~/.loom/loom.db");
    Ok(())
}

fn seed_schema(store: &Store, name: &str, config_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("── Seeding schema '{name}' from {config_dir} ──");

    // ── Schema ──────────────────────────────────────────────────────────────
    let schema_path = format!("{config_dir}/attribute_schema.json");
    let schema_json = std::fs::read_to_string(&schema_path)?;
    let schema_value: serde_json::Value = serde_json::from_str(&schema_json)?;
    let attributes = &schema_value["attributes"];
    let attributes_str = serde_json::to_string(attributes)?;

    store.upsert_schema(name, &attributes_str)?;
    println!("  ✓ Schema '{name}' upserted");

    // ── Decisions ───────────────────────────────────────────────────────────
    // Try decisions.json (array) first, then fall back to job_decision.json (single)
    let decisions_path = format!("{config_dir}/decisions.json");
    let decision_path_single = format!("{config_dir}/job_decision.json");

    if std::fs::metadata(&decisions_path).is_ok() {
        let decisions_json = std::fs::read_to_string(&decisions_path)?;
        let decisions: Vec<NamedDecision> = serde_json::from_str(&decisions_json)?;
        for d in &decisions {
            store.upsert_decision(name, d)?;
            println!("  ✓ Decision '{}' upserted", d.id);
        }
    } else if std::fs::metadata(&decision_path_single).is_ok() {
        let decision_json = std::fs::read_to_string(&decision_path_single)?;
        let decision: NamedDecision = serde_json::from_str(&decision_json)?;
        store.upsert_decision(name, &decision)?;
        println!("  ✓ Decision '{}' upserted", decision.id);
    } else {
        println!("  ⚠ No decisions found");
    }

    // ── Passives ────────────────────────────────────────────────────────────
    let passives_path = format!("{config_dir}/passives.json");
    if std::fs::metadata(&passives_path).is_ok() {
        let passives_json = std::fs::read_to_string(&passives_path)?;
        let passives: Vec<NamedPassiveEffect> = serde_json::from_str(&passives_json)?;
        for p in &passives {
            store.upsert_passive(name, p)?;
            println!("  ✓ Passive '{}' upserted", p.id);
        }
    } else {
        println!("  ⚠ No passives found");
    }

    // ── Goal ────────────────────────────────────────────────────────────────
    let goal_path = format!("{config_dir}/goal.json");
    if std::fs::metadata(&goal_path).is_ok() {
        let goal_json = std::fs::read_to_string(&goal_path)?;
        let goal: NamedGoalVector = serde_json::from_str(&goal_json)?;
        store.upsert_goal(name, "default", &goal)?;
        println!("  ✓ Goal 'default' upserted");
    } else {
        println!("  ⚠ No goal found");
    }

    Ok(())
}
