# Loom — Architecture

## Overview

Loom is a **decision assistant engine** built on three minimal traits (`Predicate`, `Action`, `Valuation`) operating on `Vec<f64>`. Domain-specific semantics come from `AttributeSchema` — a JSON-defined mapping from attribute names to vector indices. The engine never touches domain types directly.

Two input modes, one engine:
- **Real-time** — events fire (from webhooks, user input, or automation) → decisions generated → ranked by utility → user chooses → timeline records
- **Simulation** — fork from a snapshot → fast-forward N steps with events/passives → compare utility across forks

## Crate Map

```
loom-core/     Engine — zero IO, pure computation
  traits.rs    Predicate, Action, Valuation traits + compositors
  event.rs     Condition, AttributeEffect, Transform, Decision, Outcome, Event, Project
  events.rs    ResolvedEvent, determine_firing() — unified event core
  schema.rs    AttributeSchema, DynamicState, AttributeDef, AttributeKind
  simulation.rs  MC engine (run, run_dynamic, run_schedule, batch_compare)
  scoring.rs   GoalVector, Threshold, DecisionAnalysis, pareto_frontier
  named.rs     Named* config types (resolve to engine types via schema)
  distribution.rs  Percentile stats, time bands
  state.rs     StateVector trait (alternative to DynamicState)

loom-store/    SQLite persistence — depends on loom-core
  lib.rs       Store (schemas, decisions, passives, goals, states)
  event.rs     NamedEvent CRUD + DecisionVariant + active-event runtime
  timeline.rs  TimelineStore (snapshots, forks, forecast/actual)
  template.rs  Template system — complete domain presets

loom-tui/      Terminal UI — depends on loom-core + loom-store
  app.rs       App state, dashboard data, fork explorer, CRUD logic
  ui.rs        ratatui rendering (dashboard, fork explorer, all screens)
  main.rs      Event loop, keybindings
```

## The Trait Layer

The engine is built on three minimal traits:

```rust
trait Predicate: Debug { fn evaluate(&self, state: &[f64]) -> bool; }
trait Action: Debug     { fn apply(&self, state: &mut [f64]); }
trait Valuation: Debug  { fn score(&self, state: &[f64]) -> f64; }
```

### Built-in Implementations

| Type | Trait | Delegates To |
|---|---|---|
| `Condition` | `Predicate` | `Condition::check()` |
| `AttributeEffect` | `Action` | `AttributeEffect::apply()` |
| `Transform` | `Action` | `Transform::apply()` |
| `GoalVector` | `Valuation` | `GoalVector::utility()` |

### Compositors

| Type | Kind | Logic |
|---|---|---|
| `All(Vec<dyn Predicate>)` | Predicate | AND |
| `Any(Vec<dyn Predicate>)` | Predicate | OR |
| `Sequence(Vec<dyn Action>)` | Action | Sequential apply |
| `When { guard, action }` | Action | Apply if guard passes |
| `OneOf { branches }` | — | Weighted random with guards (`sample_and_apply`) |

### Simulation Entry Points

- **`run()`** — accepts concrete `Decision` + `GoalVector`. Wraps in compositors, delegates to `run_dynamic()`.
- **`run_dynamic()`** — accepts `&dyn Predicate`, `&dyn Action`, outcome branches, `&dyn Valuation`. The extensible API.
- **`run_schedule()`** — accepts `DecisionSchedule` (multi-step decisions at specific steps).
- **`run_and_analyze()`** — convenience: runs then computes `DecisionAnalysis`.
- **`batch_compare()`** — free function: N decisions against same state with shared events/passives, returns ranked `Vec<(label, DecisionAnalysis)>`.

## Unified Event Core

`loom-core/src/events.rs` provides a pure, string-free event firing function used by both Simulation and TimelineStore:

```rust
pub fn determine_firing(
    events: &[ResolvedEvent],
    state: &[f64],
    active_ids: &HashSet<usize>,
    fired_prev: &HashSet<usize>,
    resolved_prev: &HashSet<usize>,
) -> Vec<usize>  // sorted by priority desc
```

`ResolvedEvent` is fully resolved — no strings, no DB, no schema lookups at runtime. All attribute indices, chain references (event IDs as `usize`), and precondition compositors are baked in at construct time via `resolve_events()`.

The timeline runtime (`TimelineStore::check_and_advance_events`) resolves templates once, then delegates firing decisions to `determine_firing()`. The simulation engine calls it directly.

### Event Economy

Events support chains, suppression, priority, and precondition modes:

| Field | Type | Purpose |
|---|---|---|
| `precondition_mode` | `All` / `Any` | AND vs OR for preconditions |
| `priority` | `i32` | Higher fires first |
| `triggered_by` | `Vec<String>` | Chain source — fire when these fire/resolve |
| `triggers_event_id` | `Option<String>` | Chain target — fire this when self fires |
| `triggers_on_resolve` | `Option<String>` | Chain target — fire this when self ends |
| `suppressed_by` | `Vec<String>` | Blocked while these are active |
| `decision_templates` | `Vec<DecisionVariant>` | Generated decision options |

### Decision Variants

When an event fires, it can generate ephemeral decision options:

```rust
pub struct DecisionVariant {
    pub label: String,
    pub cost: Vec<NamedEffect>,
    pub outcomes: Vec<NamedOutcome>,
}
```

These appear in the dashboard with a `⚡` prefix while the event is active and disappear when it resolves. The `G` key applies the first generated decision.

## Configuration Layer: Named → Engine

Config authors write human-readable JSON using attribute names. The `Named*` types resolve to engine types via `AttributeSchema`:

```
NamedCondition { attribute: "health.stress", operator: Gt, value: 70 }
  ↓ resolve(&schema) → schema.index_of("health.stress") = 5
Condition      { attribute_index: 5, operator: Gt, value: 70.0 }
```

### Resolution Chain

```
JSON file / DB row
  ↓ serde
NamedDecision / NamedEvent / NamedGoalVector / NamedProject
  ↓ .resolve(&schema)
Decision     / Event     / GoalVector     / Project  (engine types)
  ↓ impl Predicate / impl Action / impl Valuation  (traits)
Simulation::run_dynamic()
```

### Attribute Types

`AttributeDef` supports a `kind` field — `Continuous` (default) or `Boolean`. Boolean attributes are `f64` 0.0/1.0 flags, pairing naturally with `ComparisonOp::Eq`. The engine ignores this — it's metadata for the TUI and config layer.

### Group Targeting

`NamedEffect` can target a single attribute or an entire group. At resolve time, `group_indices("wealth")` expands a single named effect to N engine-level `AttributeEffect`s — one per attribute in the group.

## Simulation Engine Flow

### Single Decision

```
1. Check preconditions → if fail, return unavailable
2. For each MC run:
   a. Clone initial state
   b. Apply cost (deterministic)
   c. Sample outcome (weighted random, condition-gated pool)
   d. Apply outcome transform
   e. Clamp state to attribute bounds
   f. For each forward step (1..horizon):
      - Tick passives (frequency check)
      - Fire events (determine_firing)
      - Tick active projects (decrement, check interrupt/complete)
      - Clamp state
      - Record utility via valuation.score()
   g. Record final state
3. Return SimulationResult
```

### Decision Schedule

```
For each MC run:
  For each step (0..horizon):
    1. Apply scheduled decisions at this step
       - Check preconditions (abort if required and fails)
       - Apply cost → sample outcome → apply transform
    2. Tick passives
    3. Fire events
    4. Tick projects
    5. Clamp state
    6. Record utility
```

### Scoring

```rust
utility_i = weight_i * state[i]
if state[i] < cliff.min:
    utility_i *= (1 - cliff.penalty)
```

### Decision Analysis

Computed from raw `SimulationResult`:
- **Utility distribution**: mean, std, p5, p25, p50, p75, p95
- **Attribute outcomes**: per-attribute distributions at horizon end
- **Utility over time**: min/mean/max bands at each step
- **Outcome probabilities**: per-entry outcome sampling frequencies

### Pareto Frontier

```rust
pub fn pareto_frontier(scores: &[(String, Vec<f64>)]) -> Vec<usize>
```

Returns non-dominated alternatives. Point A dominates B if it's >= in all dimensions and > in at least one. Useful for multi-goal comparison without scalarizing.

## Projects (Timed Decisions)

Multi-step decisions with interruption support:

```rust
pub struct Project {
    pub id: String,
    pub label: String,
    pub preconditions: Vec<Condition>,
    pub cost: Vec<AttributeEffect>,
    pub duration: usize,               // steps to complete
    pub on_complete: Transform,         // applied when duration reaches 0
    pub interrupt: Option<InterruptConfig>,
}

pub struct InterruptConfig {
    pub event_ids: Vec<usize>,         // events that interrupt
    pub on_interrupt: Transform,        // applied on interruption
}
```

During simulation, active projects tick down each step. Events matching `interrupt.event_ids` trigger `on_interrupt` and cancel the project. Successful completion applies `on_complete`.

## Persistence (loom-store)

Database: `~/.loom/loom.db` (SQLite, WAL mode, foreign keys on).

### Tables

| Table | Content |
|---|---|
| `schemas` | Attribute definitions (name, unit, bounds, group, kind) |
| `decisions` | Per-schema: id, label, preconditions, cost, outcomes |
| `passives` | Per-schema: id, label, frequency, effects |
| `goals` | Per-schema: named goal vectors (weights + cliffs) |
| `states` | Named state snapshots for save/load/branch |
| `timelines` | Named journals tied to a schema |
| `snapshots` | Ordered attribute snapshots with journal entries, linked via `parent_id` |
| `forks` | Timeline fork records (parent → child at snapshot) |
| `events` | Event templates (preconditions, delay/duration/cooldown, chains, suppression, priority, decision_templates) |
| `active_events` | Per-timeline event instances (phase lifecycle) |

### Template System

`loom-store/src/template.rs` provides `Template` — a complete domain preset as a single JSON file:

```rust
pub struct Template {
    pub name: String,
    pub description: String,
    pub schema: AttributeSchema,
    pub decisions: Vec<NamedDecision>,
    pub passives: Vec<NamedPassiveEffect>,
    pub goals: HashMap<String, NamedGoalVector>,
    pub events: Vec<NamedEvent>,
}
```

`Store::seed_from_template()` upserts everything in one call. `Template::from_file(path)` deserializes from JSON.

### Event Runtime

When a timeline snapshot is appended:

```
1. Resolve NamedEvents to ResolvedEvents (schema lookup, group expansion, cross-references)
2. Collect active event IDs from DB
3. Call determine_firing() for the pure firing decision
4. For each firing event:
   - delay > 0: create pending entry with countdown
   - delay == 0: fire immediately, apply effects, emit generated decisions
5. Advance existing active_events:
   - pending → active → cooldown → delete
6. Return AppliedEventEffect list (effects + generated decisions)
```

## TUI Architecture

### Primary Screen: Dashboard

The default home screen. Three sections: STATE (attribute values), DECISIONS (ranked by utility), RECENT (last 5 timeline snapshots). Dynamic height based on terminal size.

A **master timeline** is auto-created on first schema load. The dashboard always shows the master's latest snapshot as current state.

Keybindings: `↑↓/jk` scroll, `S` simulate, `F` fork explorer, `G` apply generated decision, `R` refresh, `L` switch schema, `Tab` legacy tabs, `Q` quit.

### Fork Explorer

Press `F` on a decision → fork from master HEAD → `batch_compare()` against top alternatives → side-by-side comparison with utility scores and attribute deltas. `Enter` applies to master timeline with forecast attached. `Esc` discards.

### Legacy Tabs (Tab key)

```
Tab 1 (Timeline)   — Journal: create timelines, append snapshots, fork, view events
Tab 2 (Explore)    — Simulate: pick decision, run MC, inspect results
Tab 3 (Config)     — Edit: CRUD for schemas, decisions, passives, goals, events
```

### Screen Routing

```
Dashboard → ForkExplorer
         → SchemaList → List → Detail → Results
                              ↓
                       StateManager (save/load/branch)
                              ↓
                       EditDecisions → EditDecisionDetail
                       EditPassives  → EditPassiveDetail
                       EditGoals     → EditGoalDetail
                       EditEvents    → EditEventsDetail
TimelineBrowser → SnapshotList → SnapshotDetail
               → ForkBrowser
```

## State Vector Semantics

All domain types converge to `Vec<f64>`:

```
DynamicState (schema-driven, flexible)
  ↔ Vec<f64> (engine)
  ↔ StateVector trait (user-defined, compile-time)

JSON schema → DynamicState (runtime-defined, no codegen)
YourDomainType → impl StateVector → to_vec() / from_vec()
```

`DynamicState` wraps `Vec<f64>` + `Arc<AttributeSchema>`. It derefs to `&[f64]` for direct engine access and provides named access (`get("wealth.cash")`, `set("wealth.cash", value)`) for TUI/API consumers.

## Design Decisions

**Why index-based engine, name-based config?** Separation of concerns. The engine operates on flat float arrays — fast, no hash lookups. Config authors use human-readable names. The schema bridges them at resolve time.

**Why `Vec<f64>` instead of generics?** MC simulation clones state thousands of times. `Vec<f64>` is compact, cache-friendly, and allocator-optimized. Generic `StateVector` is available but optional.

**Why SQLite instead of JSON files?** Timeline snapshots need ACID. Concurrent sessions would clash with flat files. SQLite provides queries, foreign keys, cascading deletes, and a single file to back up.

**Why traits?** Concrete types (`Condition`, `AttributeEffect`, `Transform`) cover the 90% case. Traits unlock the 10% — custom dynamics without modifying the engine. The hybrid approach keeps both paths.

**Why two input modes, one engine?** Real-time decision filtering and historical simulation use the same core loop (events → decisions → ranking → recording). The difference is only where events originate — external signals or template definitions.
