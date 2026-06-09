//! Seed the database with the example configuration from JSON files,
//! plus inline event templates and their spawned decisions.
//!
//! Reads the existing example configs from both `examples/configs/` (schema "personal")
//! and `examples/configs_financial/` (schema "financial") and inserts them into
//! ~/.loom/loom.db. Also seeds event templates and decisions inline (no JSON files).
//!
//! Usage:
//! ```bash
//! cargo run --example seed -p loom-store
//! ```

use loom_core::{
    ComparisonOp::{Gt, Lt},
    NamedCondition, NamedDecision, NamedEffect, NamedGoalVector, NamedOutcome,
    NamedPassiveEffect, NamedTransform,
};
use loom_store::{NamedEvent, Store};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let store = Store::open_default()?;

    // ── Seed "personal" schema from examples/configs/ ────────────────────────────
    seed_schema(
        &store,
        "personal",
        &format!("{manifest_dir}/../../examples/configs"),
    )?;
    seed_personal_events(&store, "personal")?;

    // ── Seed "financial" schema from examples/configs_financial/ ─────────────────
    seed_schema(
        &store,
        "financial",
        &format!("{manifest_dir}/../../examples/configs_financial"),
    )?;
    seed_financial_events(&store, "financial")?;

    // ── Verify ──────────────────────────────────────────────────────────────────
    println!();
    println!("── Verification ──");
    let schemas = store.list_schemas()?;
    println!("Schemas: {}", schemas.len());
    for s in &schemas {
        let decs = store.list_decisions(&s.name)?;
        let pass = store.list_passives(&s.name)?;
        let goals = store.list_goals(&s.name)?;
        let evts = store.list_events(&s.name)?;
        println!(
            "  {}: {} decisions, {} passives, {} goals, {} events",
            s.name,
            decs.len(),
            pass.len(),
            goals.len(),
            evts.len()
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

// ── Personal schema: events and spawned decisions ──────────────────────────

fn seed_personal_events(store: &Store, schema: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("── Seeding personal events inline ──");

    // 1. car_breakdown
    let events = vec![
        NamedEvent {
            id: "car_breakdown".into(),
            label: "Car breaks down".into(),
            description: "Your car needs major repairs after a stressful week".into(),
            preconditions: vec![NamedCondition {
                attribute: "health.stress".into(),
                operator: Gt,
                value: 30.0,
            }],
            delay: 2,
            duration: 1,
            cooldown: 30,
            effects: vec![NamedEffect::fixed("wealth.cash", -2000.0)],
            spawns_decision_id: None,
            ..Default::default()
        },
        // 2. health_scare
        NamedEvent {
            id: "health_scare".into(),
            label: "Health scare".into(),
            description: "Your body is giving you warning signs".into(),
            preconditions: vec![NamedCondition {
                attribute: "health.physical".into(),
                operator: Lt,
                value: 40.0,
            }],
            delay: 1,
            duration: 1,
            cooldown: 24,
            effects: vec![
                NamedEffect::fixed("health.physical", -8.0),
                NamedEffect::fixed("health.stress", 15.0),
            ],
            spawns_decision_id: Some("improve_health".into()),
            ..Default::default()
        },
        // 3. unexpected_bonus
        NamedEvent {
            id: "unexpected_bonus".into(),
            label: "Unexpected work bonus".into(),
            description: "Your manager surprises you with a performance bonus".into(),
            preconditions: vec![NamedCondition {
                attribute: "skills.rust".into(),
                operator: Gt,
                value: 70.0,
            }],
            delay: 0,
            duration: 1,
            cooldown: 20,
            effects: vec![
                NamedEffect::fixed("wealth.cash", 8000.0),
                NamedEffect::fixed("health.stress", -5.0),
            ],
            spawns_decision_id: None,
            ..Default::default()
        },
        // 4. social_conflict
        NamedEvent {
            id: "social_conflict".into(),
            label: "Social conflict".into(),
            description: "Your stress is affecting your relationships".into(),
            preconditions: vec![NamedCondition {
                attribute: "health.stress".into(),
                operator: Gt,
                value: 60.0,
            }],
            delay: 2,
            duration: 1,
            cooldown: 18,
            effects: vec![
                NamedEffect::fixed("social.alice", -10.0),
                NamedEffect::fixed("social.bob", -10.0),
                NamedEffect::fixed("health.stress", 5.0),
            ],
            spawns_decision_id: None,
            ..Default::default()
        },
        // 5. layoff_wave
        NamedEvent {
            id: "layoff_wave".into(),
            label: "Layoff wave hits your company".into(),
            description: "Your company announces mass layoffs".into(),
            preconditions: vec![NamedCondition {
                attribute: "skills.rust".into(),
                operator: Lt,
                value: 50.0,
            }],
            delay: 3,
            duration: 1,
            cooldown: 48,
            effects: vec![
                NamedEffect::fixed("wealth.cash", -30000.0),
                NamedEffect::fixed("health.stress", 25.0),
            ],
            spawns_decision_id: Some("job_search_options".into()),
            ..Default::default()
        },
        // 6. burnout
        NamedEvent {
            id: "burnout".into(),
            label: "Burnout".into(),
            description: "Chronic stress leads to burnout — you can't function at full capacity".into(),
            preconditions: vec![NamedCondition {
                attribute: "health.stress".into(),
                operator: Gt,
                value: 75.0,
            }],
            delay: 2,
            duration: 3,
            cooldown: 30,
            // Per-step effects; the runtime divides by duration and applies each step.
            effects: vec![
                NamedEffect::fixed("health.physical", -4.0),
                NamedEffect::fixed("health.stress", 3.0),
            ],
            spawns_decision_id: None,
            ..Default::default()
        },
    ];

    for event in &events {
        store.upsert_event(schema, event)?;
        println!("  ✓ Event '{}' upserted", event.id);
    }

    // ── Personal spawned decisions ──────────────────────────────────────────

    // 7. improve_health (spawned by health_scare)
    let improve_health = NamedDecision {
        id: "improve_health".into(),
        label: "Address your declining health".into(),
        preconditions: vec![],
        cost: vec![],
        outcomes: vec![
            NamedOutcome {
                label: "Join a gym".into(),
                weight: 60.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("health.physical", 20.0),
                        NamedEffect::fixed("wealth.cash", -500.0),
                        NamedEffect::fixed("time_free", -5.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Change diet".into(),
                weight: 30.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("health.physical", 10.0),
                        NamedEffect::fixed("wealth.cash", -200.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Ignore it".into(),
                weight: 10.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![NamedEffect::fixed("health.stress", 10.0)],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
        ],
    };

    store.upsert_decision(schema, &improve_health)?;
    println!("  ✓ Decision 'improve_health' upserted (spawned by health_scare)");

    // 8. job_search_options (spawned by layoff_wave)
    let job_search = NamedDecision {
        id: "job_search_options".into(),
        label: "Find a new job".into(),
        preconditions: vec![],
        cost: vec![],
        outcomes: vec![
            NamedOutcome {
                label: "Take first offer".into(),
                weight: 50.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("wealth.cash", 40000.0),
                        NamedEffect::fixed("skills.rust", 5.0),
                        NamedEffect::fixed("health.stress", -5.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Wait for better".into(),
                weight: 30.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("wealth.cash", 60000.0),
                        NamedEffect::fixed("health.stress", 10.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Switch careers".into(),
                weight: 20.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("wealth.cash", 30000.0),
                        NamedEffect::fixed("skills.rust", -10.0),
                        NamedEffect::fixed("skills.python", 20.0),
                        NamedEffect::fixed("health.stress", 5.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
        ],
    };

    store.upsert_decision(schema, &job_search)?;
    println!("  ✓ Decision 'job_search_options' upserted (spawned by layoff_wave)");

    Ok(())
}

// ── Financial schema: events and spawned decisions ─────────────────────────

fn seed_financial_events(store: &Store, schema: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("── Seeding financial events inline ──");

    // 9. market_correction
    let events = vec![
        NamedEvent {
            id: "market_correction".into(),
            label: "Stock market correction".into(),
            description: "The market drops sharply over several weeks".into(),
            preconditions: vec![NamedCondition {
                attribute: "stocks".into(),
                operator: Gt,
                value: 5000.0,
            }],
            delay: 2,
            duration: 3,
            cooldown: 24,
            // Per-step effect; runtime divides by duration.
            effects: vec![NamedEffect::fixed("stocks", -1500.0)],
            spawns_decision_id: Some("sell_or_hold".into()),
            ..Default::default()
        },
        // 10. rental_vacancy
        NamedEvent {
            id: "rental_vacancy".into(),
            label: "Rental property vacancy".into(),
            description: "Your tenant moves out, property sits empty".into(),
            preconditions: vec![NamedCondition {
                attribute: "real_estate".into(),
                operator: Gt,
                value: 0.0,
            }],
            delay: 1,
            duration: 3,
            cooldown: 18,
            effects: vec![NamedEffect::fixed("monthly_income", -300.0)],
            spawns_decision_id: None,
            ..Default::default()
        },
        // 11. interest_rate_hike
        NamedEvent {
            id: "interest_rate_hike".into(),
            label: "Interest rate hike".into(),
            description: "Central bank raises rates, your debt payments increase".into(),
            preconditions: vec![NamedCondition {
                attribute: "debt".into(),
                operator: Gt,
                value: 10000.0,
            }],
            delay: 3,
            duration: 2,
            cooldown: 36,
            effects: vec![NamedEffect::fixed("monthly_expenses", 200.0)],
            spawns_decision_id: None,
            ..Default::default()
        },
        // 12. medical_emergency
        NamedEvent {
            id: "medical_emergency".into(),
            label: "Medical emergency".into(),
            description: "Unexpected health issue requires immediate treatment".into(),
            preconditions: vec![],
            delay: 0,
            duration: 1,
            cooldown: 30,
            effects: vec![
                NamedEffect::fixed("cash", -4000.0),
                NamedEffect::fixed("debt", 4000.0),
            ],
            spawns_decision_id: None,
            ..Default::default()
        },
    ];

    for event in &events {
        store.upsert_event(schema, event)?;
        println!("  ✓ Event '{}' upserted", event.id);
    }

    // ── Financial spawned decisions ─────────────────────────────────────────

    // 13. sell_or_hold (spawned by market_correction)
    let sell_or_hold = NamedDecision {
        id: "sell_or_hold".into(),
        label: "React to the market correction".into(),
        preconditions: vec![],
        cost: vec![],
        outcomes: vec![
            NamedOutcome {
                label: "Sell everything".into(),
                weight: 30.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("cash", 10000.0),
                        NamedEffect::fixed("stocks", -15000.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Hold steady".into(),
                weight: 50.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
            NamedOutcome {
                label: "Buy the dip".into(),
                weight: 20.0,
                condition: None,
                transform: NamedTransform::Declarative {
                    effects: vec![
                        NamedEffect::fixed("cash", -5000.0),
                        NamedEffect::fixed("stocks", 5000.0),
                    ],
                    conditional: vec![],
                    default_conditional: vec![],
                },
            },
        ],
    };

    store.upsert_decision(schema, &sell_or_hold)?;
    println!("  ✓ Decision 'sell_or_hold' upserted (spawned by market_correction)");

    Ok(())
}
