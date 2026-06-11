# Loom

A **decision assistant engine** — answers "What should I do right now?" Model any domain as a vector of attributes, define decisions/events/goals, and let the engine rank your options.

- **Dashboard** — one screen: your state, ranked decisions, recent activity
- **Fork Explorer** — pause, fork N paths, fast-forward, compare utilities
- **Timeline** — git-for-life journal with snapshots, forks, forecast-vs-actual
- **Templates** — pre-built domains (Life Coach, Financial Planner) in single JSON files
- **Engine** — Monte Carlo simulation, event chains, Pareto optimization, 70 tests

## Quick Start

```bash
# Build
cargo build

# Seed from a template (recommended)
cargo run --example seed_template -p loom-store

# Or seed both classic schemas
cargo run --example seed -p loom-store

# Launch
cargo run -p loom-tui
```

Database lives at `~/.loom/loom.db`. Everything — schemas, decisions, passives, goals, events, timelines — in one SQLite file.

## Dashboard

The default home screen. Three sections:

```
┌─ STATE — life_coach ──────────────────────────┐
│  wealth.cash: 50000          wealth.stocks: 25000 │
│  health.physical: 75         health.stress: 30     │
│  skills.rust: 70             skills.python: 45     │
│  time_free: 40                                    │
├─ DECISIONS (ranked) — 3 total ──────────────────┤
│  > 1. Take remote job at Nvidia   util: +142.3   │
│    2. Address your declining health  util: +45.7  │
│    3. ✗ Job search options  (unavailable)        │
├─ RECENT ────────────────────────────────────────┤
│  2026-06-10  ✓ Took remote job                   │
│  2026-06-09  ⚡ Health scare — stress +15         │
└──────────────────────────────────────────────────┘
↑↓/jk scroll  S simulate  R refresh  L schema  F fork  Tab tabs  Q quit
```

A **master timeline** is auto-created on first run. The dashboard always shows your latest state.

### Keybindings

| Key | Action |
|---|---|
| `↑↓/j k` | Scroll decisions |
| `S` | Simulate highlighted decision (MC + utility score) |
| `F` | Fork explorer — compare this decision against alternatives |
| `G` | Apply an event-generated decision |
| `R` | Refresh dashboard data |
| `L` | Switch template/schema |
| `Tab` | Legacy 3-tab view (Timeline / Explore / Config) |
| `Q` | Quit |

## Fork Explorer

Press `F` on any decision to fork from your current state:

```
┌─ FORK — Take remote job at Nvidia ─────────────┐
│  Source: master snapshot #3                      │
│                                                  │
│  1. Take remote job at Nvidia   util: +142.3     │
│     wealth.cash: +40000  skills.rust: +20        │
│  2. Address declining health    util: +45.7      │
│     health.physical: +20  wealth.cash: -500      │
│                                                  │
│  Enter=apply  Esc=cancel  ↑↓=navigate            │
└──────────────────────────────────────────────────┘
```

`Enter` commits the decision to your master timeline as a snapshot with an attached forecast. `Esc` discards the fork.

## Templates

Complete domain presets in single JSON files:

```bash
# Seed from a template
cargo run --example seed_template -p loom-store
```

Included templates:
- **life_coach.json** — personal life: career, health, skills, relationships, 9 events with burnout cascade
- **financial_planner.json** — finances: stocks, debt, real estate, 8 events with market crash cascade

Custom templates are just JSON files with a schema, decisions, passives, goals, and events.

## Config Format

All config uses human-readable attribute names. The engine resolves them to indices via the schema.

### Attribute Schema

```json
{
  "version": 1,
  "attributes": [
    {"name": "wealth.cash", "unit": "$", "kind": "continuous"},
    {"name": "health.physical", "unit": "pts", "bounds": [0, 100]},
    {"name": "trait_ambitious", "kind": "boolean"},
    {"name": "time_free", "group": "resources", "unit": "hrs"}
  ]
}
```

`kind` defaults to `"continuous"`. Boolean attributes are 0.0/1.0 flags.

### Decisions

```json
{
  "id": "take_rust_job",
  "label": "Accept Rust Position",
  "preconditions": [
    {"attribute": "skills.rust", "operator": "Gt", "value": 60}
  ],
  "cost": [{"attribute": "time_free", "delta": -20}],
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

### Events (with chains, suppression, and generated decisions)

```json
{
  "id": "market_correction",
  "label": "Market correction",
  "preconditions": [{"attribute": "stocks", "operator": "Gt", "value": 5000}],
  "precondition_mode": "all",
  "priority": 20,
  "delay": 2, "duration": 3, "cooldown": 24,
  "effects": [{"attribute": "stocks", "delta": -1500}],
  "triggers_event_id": "panic_selling",
  "triggers_on_resolve": "regulatory_volatility",
  "decision_templates": [
    {
      "label": "Sell everything",
      "cost": [],
      "outcomes": [
        {"weight": 70, "label": "Cut losses", "transform": {"type": "declarative", "effects": [{"attribute": "cash", "delta": 5000}]}},
        {"weight": 30, "label": "Miss rebound", "transform": {"type": "declarative", "effects": []}}
      ]
    },
    {
      "label": "Hold steady",
      "cost": [],
      "outcomes": [
        {"weight": 100, "label": "Weather the storm", "transform": {"type": "declarative", "effects": [{"attribute": "health.stress", "delta": 10}]}}
      ]
    }
  ],
  "triggered_by": [],
  "suppressed_by": []
}
```

Key event fields:
- `precondition_mode`: `"all"` (AND, default) or `"any"` (OR)
- `priority`: higher fires first when multiple events trigger same step
- `triggers_event_id` / `triggers_on_resolve`: chain events on fire/resolve
- `triggered_by`: event IDs that trigger this one (chain source)
- `suppressed_by`: event IDs that block this while active
- `decision_templates`: generated decision options (ephemeral, tied to event lifecycle)

### Passives

Recurring effects: `every_step`, `every` (N steps), or `when` (conditional):

```json
{
  "id": "salary",
  "label": "Monthly salary",
  "frequency": {"type": "every_step"},
  "effects": [{"attribute": "wealth.cash", "delta": 5000}]
}
```

### Goal Vectors

```json
{
  "weights": {
    "wealth.cash": 1.0,
    "health.physical": 0.5,
    "health.stress": -0.3
  },
  "cliffs": {
    "health.physical": {"min": 30, "penalty": 1.0}
  }
}
```

Weights: positive = maximize, negative = minimize. Cliffs: penalty when attribute drops below `min`.

### Group-Targeted Effects

```json
{"group": "wealth", "delta": -500}
```

Expands to one effect per attribute in the group. Also supports proportional scaling: `"scaling": [["wealth.cash", 0.05]]` means 5% of current value.

## Engine Traits

```rust
pub trait Predicate: Debug { fn evaluate(&self, state: &[f64]) -> bool; }
pub trait Action: Debug     { fn apply(&self, state: &mut [f64]); }
pub trait Valuation: Debug  { fn score(&self, state: &[f64]) -> f64; }
```

Compositors: `All`, `Any`, `Sequence`, `When`, `OneOf`. Use `Simulation::run_dynamic()` for trait objects:

```rust
let sim = Simulation::new(horizon, runs);
let result = sim.run_dynamic(&state, &precondition, &cost, &outcomes, &goal);
```

Built-in types (`Condition`, `AttributeEffect`, `Transform`, `GoalVector`) implement these traits. Custom implementations slot into the same engine.

## Pareto Frontier

```rust
use loom_core::pareto_frontier;

let scores = vec![
    ("Take job".into(), vec![142.0, -5.0]),      // (wealth utility, stress impact)
    ("Stay put".into(), vec![50.0, 15.0]),
];
let frontier = pareto_frontier(&scores);  // → vec![0]  (Take job dominates)
```

Multi-objective ranking without scalarizing to a single number.

## Batch Comparison

```rust
use loom_core::batch_compare;

let results = batch_compare(&state, &decisions, &passives, &events, &goal, 24, 1000);
// → Vec<(decision_label, DecisionAnalysis)> sorted by utility desc
```

Compare N decisions against the same initial state with shared events/passives.

## Timed Decisions (Projects)

Multi-step decisions that tick over N steps with interruption support:

```json
{
  "id": "rust_certification",
  "label": "Rust Certification",
  "preconditions": [{"attribute": "skills.rust", "operator": "Gt", "value": 50}],
  "cost": [{"attribute": "time_free", "delta": -10}],
  "duration": 8,
  "on_complete": {"type": "declarative", "effects": [{"attribute": "skills.rust", "delta": 20}]},
  "interrupt": {
    "event_ids": ["health_scare", "burnout"],
    "on_interrupt": {"type": "declarative", "effects": [{"attribute": "health.stress", "delta": 10}]}
  }
}
```

## Timeline

A named journal tied to a schema. Append snapshots — ordered state vectors with journal entries. Each snapshot links to its parent, forming a chain.

- **Fork** at any snapshot to create a parallel timeline
- **Attach forecasts** — record what the simulation predicted
- **Resolve outcomes** — record what actually happened, compare against forecast
- **Events** auto-fire each step with delay/duration/cooldown management

## Architecture

```
Config Layer (JSON/DB)
  NamedDecision, NamedEvent, NamedEffect, …
  ↓ resolve(&AttributeSchema)
Engine (loom-core)
  Predicate → Action → Valuation
  Monte Carlo simulation on Vec<f64>
  ↓
Store (loom-store)
  SQLite: schemas, decisions, passives, goals, events, timelines
  ↓
TUI (loom-tui)
  Dashboard, Fork Explorer, 3-tab legacy view
```

Three crates:
- **loom-core** — Engine. Traits, compositors, MC simulation, scoring, event core, Pareto frontier.
- **loom-store** — SQLite persistence. CRUD, timeline, event runtime, template system.
- **loom-tui** — Terminal UI. Dashboard, Fork Explorer, config editors, timeline browser.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design.

## Tests

```bash
cargo test  # 70 tests, all pass
```

## License

MIT
