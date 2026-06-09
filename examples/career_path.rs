//! Example: Multi-decision career path — take job at step 0, invest at step 12.
//!
//! Demonstrates `run_schedule()`: decisions fire at specific steps during the
//! simulation horizon. Passives tick between decisions.
//!
//! Usage:
//! ```bash
//! cargo run --example career_path
//! ```

use loom_core::{
    AttributeSchema, DynamicState, NamedDecisionSchedule, NamedGoalVector,
    NamedPassiveEffect, Simulation,
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/configs");

    // ── Load schema ──────────────────────────────────────────────────────────────
    let schema = Arc::new(AttributeSchema::from_path(&format!(
        "{config_dir}/attribute_schema.json"
    ))?);

    // ── Load named configs ───────────────────────────────────────────────────────
    let schedule = NamedDecisionSchedule::from_path(
        &format!("{config_dir}/career_schedule.json"),
        &schema,
    )?;
    let passives = NamedPassiveEffect::list_from_path(
        &format!("{config_dir}/passives.json"),
        &schema,
    )?;
    let goal = NamedGoalVector::from_path(&format!("{config_dir}/goal.json"), &schema)?;

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

    // ── Run schedule ─────────────────────────────────────────────────────────────
    let sim = Simulation {
        horizon: 24,
        monte_carlo_runs: 1000,
        passives,
        events: Vec::new(),
    };

    let result = sim.run_schedule(&state, &schedule, &goal);
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  LOOM SCHEDULE: Career path");
    println!("  Step 0: Take Nvidia job");
    println!("  Step 12: Invest 50% cash → stocks (if cash > $20k)");
    println!("══════════════════════════════════════════════════");
    println!();

    if !result.decision_available {
        println!("  First decision's preconditions NOT met.");
        return Ok(());
    }

    println!("  Schedule aborted: {} / {} runs", result.schedule_aborted, sim.monte_carlo_runs);
    println!();

    // Per-decision outcome probabilities
    println!("  ── Per-decision outcome probabilities ───");
    println!();
    // Group counts by schedule entry
    let mut entry_counts: Vec<Vec<(usize, usize)>> = vec![Vec::new(); schedule.entries.len()];
    for ((entry_idx, outcome_idx), &count) in &result.outcome_counts {
        entry_counts[*entry_idx].push((*outcome_idx, count));
    }
    for (i, sched) in schedule.entries.iter().enumerate() {
        println!("    Entry {} (step {}): \"{}\"", i, sched.at_step, sched.decision.label);
        if entry_counts[i].is_empty() {
            println!("      (no samples — decision skipped or run aborted)");
            continue;
        }
        let total: usize = entry_counts[i].iter().map(|(_, c)| c).sum();
        for (outcome_idx, count) in &entry_counts[i] {
            let pct = *count as f64 / total as f64 * 100.0;
            let label = sched.decision.outcomes.get(*outcome_idx)
                .map_or("unknown", |o| o.label.as_str());
            println!("      {}: {:.1}%", label, pct);
        }
    }
    println!();

    // Per-attribute final distributions
    println!("  ── Final attribute outcomes ───");
    println!();
    println!(
        "    {:<22} {:>12} {:>12} {:>12}",
        "Attribute", "Initial", "Mean", "Delta"
    );
    println!("    {:-<60}", "");
    for (i, attr) in schema.attributes.iter().enumerate() {
        let initial = state.get(&attr.name).unwrap_or(0.0);
        let finals: Vec<f64> = result.final_states.iter().map(|s| s[i]).collect();
        let mean = finals.iter().sum::<f64>() / finals.len() as f64;
        let delta = mean - initial;
        if delta.abs() > 0.5 {
            println!(
                "    {:<22} {:>12.1} {:>12.1} {:>+12.1}",
                attr.name, initial, mean, delta
            );
        }
    }
    println!();

    // Utility over time
    println!("  ── Utility over time ───");
    println!();
    println!(
        "    {:<6} {:>12} {:>12} {:>12}",
        "Step", "Mean", "Min", "Max"
    );
    println!("    {:-<42}", "");
    let traces = &result.utility_traces;
    let num_steps = traces.first().map_or(0, |t| t.len());
    for step in 0..num_steps {
        let values: Vec<f64> = traces.iter().map(|t| t[step]).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        println!("    {:<6} {:>12.1} {:>12.1} {:>12.1}", step, mean, min, max);
    }
    println!();

    Ok(())
}
