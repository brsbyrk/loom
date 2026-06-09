# Loom

A **vector-state decision engine** for human-scale simulation. Model your life, career, finances, or any multi-attribute system — then run Monte Carlo what-if scenarios, track actual outcomes over time, and compare forecasts against reality.

## What It Does

- **Define** an attribute space (wealth, health, skills, social, time, …) as a JSON schema
- **Model** decisions with preconditions, costs, and probabilistic outcomes
- **Simulate** — Monte Carlo forward runs with passives, cliffs, and utility scoring
- **Journal** real outcomes in a git-for-life timeline (snapshots, forks, forecast vs actual)
- **Compare** decisions side-by-side: utility distributions, attribute projections, sensitivity
- **Automate** recurring effects and external events with delay/duration/cooldown lifecycle

## Quick Start

```bash
# Build
cargo build

# Seed the database with example schemas, decisions, passives, and events
cargo run --example seed -p loom-store

# Launch the TUI
cargo run -p loom-tui
```

Database lives at `~/.loom/loom.db`. Schema, decisions, passives, goals, states, timelines — all in one SQLite file.

## TUI — Three Tabs

| Key | Tab | What You Do |
|---|---|---|
| `1` | **Timeline** | Journal real life. Append snapshots, fork parallel paths, resolve outcomes. Events auto-fire when their preconditions are met. |
| `2` | **Explore** | Run ad-hoc simulations. Pick a decision, tweak initial state, run Monte Carlo, inspect utility traces and outcome distributions. |
| `3` | **Config** | Edit schemas, decisions, passives, goals, events. Inline CRUD — add/remove outcomes, change weights, adjust preconditions. |

Press `4` from the Timeline tab to jump to event configuration.

## Architecture

```
Config Layer (JSON/DB)
  NamedDecision, NamedEffect, NamedCondition, …
  ↓ resolve(&AttributeSchema)
Engine (loom-core)
  Predicate → Action → Valuation
  Monte Carlo simulation on Vec<f64>
  ↓
Store (loom-store)
  SQLite: schemas, decisions, passives, goals, states, timelines, events
  ↓
TUI (loom-tui)
  ratatui interface, 3-tab layout
```

Three crates:
- **loom-core** — Engine. Traits (`Predicate`, `Action`, `Valuation`), compositors (`All`, `Sequence`, `When`, `OneOf`), MC simulation, scoring, schema-driven state.
- **loom-store** — SQLite persistence. CRUD for all config types, timeline snapshots+forks, event runtime.
- **loom-tui** — Terminal UI. Schema browser, decision explorer, simulation results, inline config editor, timeline journal.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design.

## Config Format (Named Layer)

All config uses human-readable attribute names. The engine resolves them to indices via the schema.

### Attribute Schema

```json
{
  "version": 1,
  "attributes": [
    {"name": "wealth.cash", "unit": "$"},
    {"name": "health.physical", "unit": "pts", "bounds": [0, 100]},
    {"name": "skills.rust", "unit": "pts", "bounds": [0, 100]},
    {"name": "time_free", "group": "resources", "unit": "hrs"}
  ]
}
```

### Decision

```json
{
  "id": "take_rust_job",
  "label": "Accept Rust Position",
  "preconditions": [
    {"attribute": "skills.rust", "operator": "Gt", "value": 60}
  ],
  "cost": [
    {"attribute": "time_free", "delta": -20}
  ],
  "outcomes": [
    {
      "weight": 70,
      "label": "Great offer",
      "transform": {
        "type": "declarative",
        "effects": [
          {"attribute": "wealth.cash", "delta": 150000},
          {"attribute": "skills.rust", "delta": 15}
        ]
      }
    },
    {
      "weight": 30,
      "label": "Decent offer",
      "transform": {
        "type": "declarative",
        "effects": [
          {"attribute": "wealth.cash", "delta": 100000},
          {"attribute": "skills.rust", "delta": 5}
        ]
      }
    }
  ]
}
```

### Passives

Recurring effects that tick during simulation and timeline journaling:

```json
{
  "id": "monthly_salary",
  "label": "Monthly Salary",
  "frequency": {"type": "every_step"},
  "effects": [
    {"attribute": "wealth.cash", "delta": 5000}
  ]
}
```

Supports `every_step`, `every` (every N steps), and `when` (conditional):

```json
{
  "frequency": {
    "type": "when",
    "attribute": "health.stress",
    "operator": "Gt",
    "value": 70
  },
  "effects": [
    {"attribute": "health.physical", "delta": -2}
  ]
}
```

### Goal Vector

```json
{
  "weights": {
    "wealth.cash": 1.0,
    "health.physical": 0.5,
    "health.stress": -0.3,
    "skills.rust": 0.8,
    "time_free": 0.2
  },
  "cliffs": {
    "health.physical": {"min": 30, "penalty": 1.0}
  }
}
```

Weights: positive = maximize, negative = minimize, zero = ignore. Cliffs: penalty factor applied when the attribute drops below `min`. `penalty=1.0` means the attribute contributes nothing below the threshold.

### Events

Events have a lifecycle: **delay → active (duration spread) → cooldown → repeatable**.

```json
{
  "id": "health_scare",
  "label": "Health Scare",
  "description": "Health drops below 40 triggers a 3-step health event",
  "preconditions": [
    {"attribute": "health.physical", "operator": "Lt", "value": 40}
  ],
  "delay": 1,
  "duration": 3,
  "cooldown": 12,
  "effects": [
    {"attribute": "health.physical", "delta": -5},
    {"attribute": "wealth.cash", "delta": -2000}
  ]
}
```

Events can optionally spawn a decision when they fire (`spawns_decision_id`).

### Group-Targeted Effects

Instead of targeting individual attributes, target an entire group:

```json
{"group": "wealth", "delta": -500}
```

This expands to one effect per attribute in the "wealth" group. Also supports proportional scaling:

```json
{"attribute": "wealth.cash", "delta": 0, "scaling": [["wealth.cash", 0.05]]}
```

This means: `wealth.cash += 5% of current wealth.cash`.

## Engine Traits (Custom Actions)

loom-core exports three traits for building custom logic:

```rust
pub trait Predicate: Debug { fn evaluate(&self, state: &[f64]) -> bool; }
pub trait Action: Debug     { fn apply(&self, state: &mut [f64]); }
pub trait Valuation: Debug  { fn score(&self, state: &[f64]) -> f64; }
```

Plus compositors: `All`, `Any`, `Sequence`, `When`, `OneOf`. Use `Simulation::run_dynamic()` to run the engine with trait objects directly:

```rust
let precondition = All(vec![
    Box::new(condition1),
    Box::new(condition2),
]);
let cost = Sequence(vec![Box::new(effect1), Box::new(effect2)]);
let outcomes: &[(f64, Option<&dyn Predicate>, &dyn Action)] = &[...];
let sim = Simulation::new(horizon, runs);
let result = sim.run_dynamic(&state, &precondition, &cost, &outcomes, &goal);
```

Built-in types (`Condition`, `AttributeEffect`, `Transform`, `GoalVector`) implement these traits, so existing configs work unchanged. Custom `Action`/`Predicate` implementations slot into the same engine — build domain-specific dynamics without touching the engine.

## Timeline — Git-for-Life

A **timeline** is a named journal tied to a schema. You append **snapshots** — ordered state vectors with journal entries. Each snapshot links to its parent, forming a chain.

- **Fork** at any snapshot to create a parallel timeline (explore a what-if branch)
- **Attach forecasts** to snapshots when you make a decision — record what the simulation predicted
- **Resolve outcomes** later: record what actually happened, compare against the forecast
- **Events** auto-fire each step based on preconditions, with delay/duration/cooldown management

## Tests

```bash
cargo test  # 54 tests, all pass
```

## License

MIT
