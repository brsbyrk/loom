//! Example: "Take remote job at Nvidia" — loaded from SQLite, not JSON files.
//!
//! This is the DB-backed equivalent of `examples/job_decision.rs`.
//! Requires the DB to be seeded first: `cargo run --example seed -p loom-store`
//!
//! Usage:
//! ```bash
//! cargo run --example db_job_decision -p loom-store
//! ```

use loom_core::{DecisionAnalysis, DynamicState, Simulation};
use loom_store::Store;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_default()?;

    // ── Load from DB ────────────────────────────────────────────────────────────
    let schema = Arc::new(
        store
            .get_schema("personal")?
            .ok_or("Schema 'personal' not found — run `cargo run --example seed -p loom-store` first")?,
    );

    let named_decision = store
        .get_decision("personal", "take_remote_job")?
        .ok_or("Decision 'take_remote_job' not found")?;
    let decision = named_decision.resolve(&schema)?;

    let named_passives = store.list_passives("personal")?;
    let passives: Vec<_> = named_passives
        .iter()
        .map(|np| np.resolve(&schema))
        .collect::<Result<_, _>>()?;

    let named_goal = store
        .get_goal("personal", "default")?
        .ok_or("Goal 'default' not found")?;
    let goal = named_goal.resolve(&schema)?;

    // ── Initial state ────────────────────────────────────────────────────────────
    let mut state = DynamicState::new(schema.clone());
    state.set("wealth.cash", 50000.0);
    state.set("wealth.stocks", 25000.0);
    state.set("wealth.house_value", 200000.0);
    state.set("wealth.debt", 50000.0);
    state.set("health.physical", 75.0);
    state.set("health.stress", 30.0);
    state.set("skills.rust", 70.0);
    state.set("skills.python", 45.0);
    state.set("skills.negotiation", 55.0);
    state.set("social.bob", 60.0);
    state.set("social.alice", 85.0);
    state.set("time_free", 40.0);

    // ── Simulation ───────────────────────────────────────────────────────────────
    let mut sim = Simulation {
        horizon: 24,
        monte_carlo_runs: 1000,
        passives,
        events: Vec::new(),
        projects: Vec::new(),
        active_projects: Vec::new(),
    };

    let analysis = sim.run_and_analyze(&state, &decision, &goal);
    print_analysis(&analysis, &decision, &schema);

    Ok(())
}

fn print_analysis(
    analysis: &DecisionAnalysis,
    decision: &loom_core::Decision,
    schema: &loom_core::AttributeSchema,
) {
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  LOOM SIMULATION (DB-backed): \"{}\"", decision.label);
    println!("══════════════════════════════════════════════════");
    println!();

    if !analysis.decision_available {
        println!("  DECISION UNAVAILABLE — preconditions not met.");
        return;
    }

    println!("  ── Outcome probabilities ───");
    println!();
    for (idx, prob) in &analysis.outcome_probabilities[0] {
        let label = decision
            .outcomes
            .get(*idx)
            .map_or("unknown", |o| o.label.as_str());
        println!("    {}:  {:.1}%", label, prob * 100.0);
    }
    println!();

    println!("  ── Utility score (final) ───");
    println!();
    let util = &analysis.utility_distribution;
    println!("    Mean: {:>10.1}", util.mean);
    println!("    Std:  {:>10.1}", util.std);
    println!("    P5:   {:>10.1}", util.p5);
    println!("    P50:  {:>10.1}", util.p50);
    println!("    P95:  {:>10.1}", util.p95);
    println!();

    println!("  ── Attribute outcomes (final state) ───");
    println!();
    println!(
        "    {:<22} {:>10} {:>10} {:>10}",
        "Attribute", "Mean", "P5", "P95"
    );
    println!("    {:-<52}", "");
    for (i, attr) in schema.attributes.iter().enumerate() {
        let dist = &analysis.attribute_outcomes[i];
        println!(
            "    {:<22} {:>10.1} {:>10.1} {:>10.1}",
            attr.name, dist.mean, dist.p5, dist.p95
        );
    }
    println!();

    println!("  ── Utility over time ───");
    println!();
    println!(
        "    {:<6} {:>10} {:>10} {:>10}",
        "Step", "Mean", "Min", "Max"
    );
    for band in &analysis.utility_over_time {
        println!(
            "    {:<6} {:>10.1} {:>10.1} {:>10.1}",
            band.step, band.mean, band.min, band.max
        );
    }
    println!();
}
