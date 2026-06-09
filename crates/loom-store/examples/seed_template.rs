//! Seed the database from a template file — one call to populate everything.
//!
//! Demonstrates loading a template JSON file and seeding all domain data:
//! schema, decisions, passives, goals, and events.
//!
//! Usage:
//! ```bash
//! cargo run --example seed_template -p loom-store
//! ```

use loom_store::{Store, Template};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let store = Store::open_default()?;

    // Load and seed life coach template
    let template_path = format!("{manifest_dir}/../../templates/life_coach.json");
    let template = Template::from_file(&template_path)?;
    let name = store.seed_from_template(&template)?;
    println!("Seeded '{}' from template '{}'", name, template.name);

    // Verify
    let decs = store.list_decisions(&name)?;
    let pass = store.list_passives(&name)?;
    let goals = store.list_goals(&name)?;
    let evts = store.list_events(&name)?;
    println!(
        "  {} decisions, {} passives, {} goals, {} events",
        decs.len(),
        pass.len(),
        goals.len(),
        evts.len()
    );

    Ok(())
}
