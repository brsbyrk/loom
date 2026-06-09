//! **Loom** — a vector-state decision engine for human-scale simulation.
//!
//! Loom provides a generic framework for:
//! - Defining attribute vectors (wealth, health, skills, time, relationships, …)
//! - Defining decisions with preconditions, costs, and probabilistic outcomes
//! - Running Monte Carlo forward simulations
//! - Scoring outcomes against configurable goal vectors
//! - Producing Pareto tradeoff surfaces and sensitivity analysis
//!
//! The engine is domain-agnostic: it operates on `Vec<f64>` internally and bridges
//! to user-defined types via the [`StateVector`] trait.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Your domain crate / TUI / API          │
//! ├─────────────────────────────────────────┤
//! │  StateVector trait (user implements)    │
//! │  Decision/Event configs (JSON/DB/…)      │
//! ├─────────────────────────────────────────┤
//! │  loom-core engine                       │
//! │  ┌──────────┬───────────┬────────────┐  │
//! │  │ event.rs │ scoring.rs│ simulation │  │
//! │  │ data     │ utility   │ MC engine  │  │
//! │  │ model    │ + dist    │            │  │
//! │  └──────────┴───────────┴────────────┘  │
//! └─────────────────────────────────────────┘
//! ```

pub mod state;
pub mod event;
pub mod events;
pub mod schema;
pub mod distribution;
pub mod scoring;
pub mod simulation;
pub mod named;
pub mod traits;

pub use state::StateVector;
pub use event::{
    AttributeEffect, ComparisonOp, Condition, Decision, DecisionSchedule, Event, Outcome,
    Project, InterruptConfig, ActiveProject, ScheduledDecision, ScriptSource, Transform,
};
pub use events::{determine_firing, ResolvedEvent};
pub use schema::{AttributeDef, AttributeKind, AttributeSchema, DynamicState, SchemaError};
pub use distribution::{Distribution, TimeBand};
pub use scoring::{DecisionAnalysis, GoalVector, Threshold, pareto_frontier};
pub use simulation::{Frequency, PassiveEffect, Simulation, SimulationResult};
pub use traits::{Action, All, Any, OneOf, Predicate, Sequence, Valuation, When};
pub use named::{
    NamedCondition, NamedDecision, NamedDecisionSchedule, NamedEffect, NamedFrequency,
    NamedGoalVector, NamedInterruptConfig, NamedOutcome, NamedPassiveEffect, NamedProject,
    NamedScheduledDecision, NamedTransform, ResolveError,
};