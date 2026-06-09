# Loom — Architecture

## Overview

Loom is a **data-driven Monte Carlo simulation engine** with persistence and a terminal UI. The core insight: all domain models reduce to `Vec<f64>` with schema-defined semantics. The engine never touches domain types directly — it operates on float slices through trait objects.

## Crate Map

```
loom-core/     Engine — zero IO, pure computation
  traits.rs    Predicate, Action, Valuation, compositors
  event.rs     Condition, AttributeEffect, Transform, Decision, Outcome, Event
  schema.rs    AttributeSchema, DynamicState
  simulation.rs  MC engine (run, run_dynamic, run_schedule)
  scoring.rs   GoalVector, Threshold, DecisionAnalysis
  named.rs     Named* config types (resolve to engine types via schema)
  distribution.rs  Percentile stats, time bands
  state.rs     StateVector trait (alternative to DynamicState)

loom-store/    SQLite persistence — depends on loom-core
  lib.rs       Store (schemas, decisions, passives, goals, states)
  event.rs     NamedEvent CRUD + active-event runtime on timelines
  timeline.rs  TimelineStore (snapshots, forks, forecast/actual)

loom-tui/      Terminal UI — depends on loom-core + loom-store
  app.rs       App state, screen routing, CRUD logic
  ui.rs        ratatui rendering
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
| `OneOf { branches }` | — | Weighted random with guards (requires RNG) |

`OneOf` does not implement `Action` because weighted sampling needs an external RNG. Use `sample_and_apply(state, rng)` instead.

### Simulation Entry Points

- **`run()`** — accepts concrete `Decision` + `GoalVector`. Wraps them in compositors and delegates to `run_dynamic()`. This is the convenience API.
- **`run_dynamic()`** — accepts `&dyn Predicate`, `&dyn Action`, outcome branches, `&dyn Valuation`. This is the extensible API. Custom `Action`/`Predicate` implementations work here without modifying the engine.
- **`run_schedule()`** — accepts `DecisionSchedule` (multi-step decisions). Not yet trait-ified (uses concrete types internally).
- **`run_and_analyze()`** — convenience: runs then computes `DecisionAnalysis`.

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
NamedDecision / NamedPassiveEffect / NamedGoalVector / NamedEvent
  ↓ .resolve(&schema)
Decision     / PassiveEffect     / GoalVector     / Condition+AE[]  (engine types)
  ↓ impl Predicate / impl Action / impl Valuation  (traits)
Simulation::run() / Simulation::run_dynamic()
```

This two-layer architecture means:
- **Config authors** never think about attribute indices
- **The engine** never thinks about attribute names
- **The schema** is the single source of truth for the mapping

### Group Targeting

`NamedEffect` can target either a single attribute or an entire group:

```json
{"group": "wealth", "delta": -500}
```

At resolve time, `group_indices("wealth")` returns all indices in that group, and the single named effect expands to N engine-level `AttributeEffect`s — one per attribute.

## Simulation Engine Flow

### Single Decision (`run` / `run_dynamic`)

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
      - Clamp state
      - Record utility via valuation.score()
   g. Record final state
3. Return SimulationResult
```

### Decision Schedule (`run_schedule`)

```
For each MC run:
  For each step (0..horizon):
    1. Apply scheduled decisions at this step
       - Check preconditions (abort if required and fails)
       - Apply cost → sample outcome → apply transform
    2. Tick passives
    3. Clamp state
    4. Record utility
```

### Scoring

```rust
// Per-attribute utility:
utility_i = weight_i * state[i]

// Cliff penalty:
if state[i] < cliff.min:
    utility_i *= (1 - cliff.penalty)
```

`GoalVector` maps 1:1 to state indices. `NamedGoalVector` uses `HashMap<String, f64>` keyed by attribute name — resolved to a flat `Vec<f64>` matching schema order.

### Decision Analysis

`DecisionAnalysis` is computed from raw `SimulationResult`:

- **Utility distribution**: mean, std, percentiles (p5, p25, p50, p75, p95) of final utilities across all runs
- **Attribute outcomes**: per-attribute distributions at end of horizon
- **Utility over time**: min/mean/max bands at each step (for charting trajectories)
- **Outcome probabilities**: per-schedule-entry outcome sampling frequencies

## Persistence (loom-store)

Database: `~/.loom/loom.db` (SQLite, WAL mode, foreign keys on).

### Tables

| Table | Content |
|---|---|
| `schemas` | Attribute definitions (name → unit, bounds, group) |
| `decisions` | Per-schema: id, label, preconditions, cost, outcomes |
| `passives` | Per-schema: id, label, frequency, effects |
| `goals` | Per-schema: named goal vectors (weights + cliffs) |
| `states` | Named state snapshots for save/load/branch |
| `timelines` | Named journals tied to a schema |
| `snapshots` | Ordered attribute snapshots with journal entries, linked via `parent_id` |
| `forks` | Timeline fork records (parent → child at snapshot) |
| `events` | Event templates per-schema (delay, duration, cooldown, effects) |
| `active_events` | Per-timeline event instances (phase lifecycle) |

### Event Runtime

When a timeline snapshot is appended, the event runtime (`check_and_advance_events`) runs:

```
1. Load all event templates for the timeline's schema
2. For each template not already active:
   - Check preconditions against current attribute values
   - If met:
     - delay > 0: create active_event(phase=pending, delay_remaining=delay)
     - delay == 0: create active_event(phase=active), apply effects now
3. Advance existing active_events:
   - pending: decrement delay → if 0, move to active, apply effects
   - active: decrement duration → apply effects each step (spread over duration)
   - cooldown: decrement cooldown → if 0, delete (event becomes re-triggerable)
4. Return AppliedEventEffect list for display
```

## TUI Architecture

### Tab System

```
Tab 1 (Timeline)   — Journal: create timelines, append snapshots, fork, view events
Tab 2 (Explore)    — Simulate: pick decision, run MC, inspect results
Tab 3 (Config)     — Edit: CRUD for schemas, decisions, passives, goals, events
```

### Screen Routing

```
SchemaList → List → Detail → Results
                  ↓
           StateManager (save/load/branch states)
                  ↓
           EditDecisions → EditDecisionDetail
           EditPassives  → EditPassiveDetail
           EditGoals     → EditGoalDetail
           EditEvents    → EditEventsDetail
TimelineBrowser → SnapshotList → SnapshotDetail
               → ForkBrowser
```

### Keybindings (Explore tab)

| Key | Action |
|---|---|
| `↑↓/j k` | Navigate decisions |
| `Enter` | View decision detail |
| `r` | Run simulation on selected decision |
| `s` | Open state manager (save/load/branch) |
| `e` | Edit decisions |
| `p` | Edit passives |
| `g` | Edit goals |
| `Esc` | Back |
| `q` | Quit |

### Timeline tab

| Key | Action |
|---|---|
| `n` | New timeline / append snapshot |
| `f` | Fork timeline at current snapshot |
| `Enter` | Open snapshot detail |
| `d` | Delete snapshot |
| `4` | Jump to event editor |

## State Vector Semantics

The engine is domain-agnostic. All domain types converge to `Vec<f64>`:

```
DynamicState (schema-driven, flexible)
  ↔ Vec<f64> (engine)
  ↔ StateVector trait (user-defined, compile-time)

YourDomainType → impl StateVector → to_vec() / from_vec()
or
JSON schema → DynamicState (runtime-defined, no codegen)
```

`DynamicState` wraps `Vec<f64>` + `Arc<AttributeSchema>`. It derefs to `&[f64]` so the engine reads it natively. It also provides named access (`get("wealth.cash")`, `set("wealth.cash", value)`) for TUI/API consumers.

## Design Decisions

**Why index-based engine, name-based config?** Separation of concerns. The engine is fast (flat float arrays, no hash lookups). The config is readable (no magic numbers). The schema bridges them at resolve time.

**Why `Vec<f64>` instead of generics?** Monte Carlo simulation clones state thousands of times. `Vec<f64>` is compact, cache-friendly, and allocator-optimized. Generic `StateVector` is available but optional — `DynamicState` is the primary interface.

**Why SQLite instead of JSON files?** Timeline snapshots need ACID. Multiple concurrent TUI sessions could clash with flat files. SQLite gives us queries, foreign keys, cascading deletes, and a single file to back up.

**Why traits now?** The concrete types (`Condition`, `AttributeEffect`, `Transform`) cover the 90% case. Traits unlock the 10% — custom dynamics (network effects, emergent behavior, domain-specific interactions) without modifying the engine. The hybrid approach keeps both paths.
