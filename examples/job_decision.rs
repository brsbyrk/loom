//! Example: "Take remote job at Nvidia" — decision simulation.
//!
//! Loads all configuration from JSON files and runs a Monte Carlo simulation.
//! Uses human-readable attribute names (e.g., "health.stress") — no raw indices.
//!
//! Usage:
//! ```bash
//! cargo run --example job_decision
//! ```

use loom_core::{
    AttributeSchema, DecisionAnalysis, DynamicState, NamedDecision, NamedGoalVector,
    NamedPassiveEffect, Simulation,
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/configs");

    // ── Load schema ──────────────────────────────────────────────────────────────
    let schema = Arc::new(AttributeSchema::from_path(&format!(
        "{config_dir}/attribute_schema.json"
    ))?);

    // ── Load named configs and resolve to engine types ───────────────────────────
    let decision = NamedDecision::from_path(
        &format!("{config_dir}/job_decision.json"),
        &schema,
    )?;
    let passives = NamedPassiveEffect::list_from_path(
        &format!("{config_dir}/passives.json"),
        &schema,
    )?;
    let goal = NamedGoalVector::from_path(
        &format!("{config_dir}/goal.json"),
        &schema,
    )?;

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

    // ── Output ───────────────────────────────────────────────────────────────────
    print_analysis(&analysis, &decision, &schema);

    Ok(())
}

fn print_analysis(
    analysis: &DecisionAnalysis,
    decision: &loom_core::Decision,
    schema: &AttributeSchema,
) {
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  LOOM SIMULATION: \"{}\"", decision.label);
    println!("══════════════════════════════════════════════════");
    println!();

    if !analysis.decision_available {
        println!("  DECISION UNAVAILABLE — preconditions not met.");
        println!();
        println!("  Required:");
        for cond in &decision.preconditions {
            let name = schema
                .at(cond.attribute_index)
                .map_or(format!("attr[{}]", cond.attribute_index), |a| {
                    a.name.clone()
                });
            println!("    - {name} {:?} {}", cond.operator, cond.value);
        }
        return;
    }

    // Outcome probabilities
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

    // Utility distribution
    println!("  ── Utility score (final) ───");
    println!();
    let util = &analysis.utility_distribution;
    println!("    Mean: {:>10.1}", util.mean);
    println!("    Std:  {:>10.1}", util.std);
    println!("    P5:   {:>10.1}", util.p5);
    println!("    P50:  {:>10.1}", util.p50);
    println!("    P95:  {:>10.1}", util.p95);
    println!("    Min:  {:>10.1}", util.min);
    println!("    Max:  {:>10.1}", util.max);
    println!();

    // Per-attribute final distributions
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

    // Utility over time summary
    println!("  ── Utility over time (first 4 + last 2 steps) ───");
    println!();
    println!(
        "    {:<6} {:>10} {:>10} {:>10}",
        "Step", "Mean", "Min", "Max"
    );
    println!("    {:-<36}", "");
    let bands = &analysis.utility_over_time;
    let show_steps: Vec<usize> = if bands.len() <= 6 {
        (0..bands.len()).collect()
    } else {
        let mut steps: Vec<usize> = (0..4).collect();
        steps.push(bands.len() - 2);
        steps.push(bands.len() - 1);
        steps
    };
    let mut prev_step = 0;
    for &step in &show_steps {
        if step > prev_step + 1 && step - prev_step > 1 {
            println!("    ...");
        }
        let band = &bands[step];
        println!(
            "    {:<6} {:>10.1} {:>10.1} {:>10.1}",
            step, band.mean, band.min, band.max
        );
        prev_step = step;
    }
    println!();
    println!();
}
