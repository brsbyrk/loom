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

pub use state::StateVector;
pub use event::{
    AttributeEffect, ComparisonOp, Condition, Decision, Event, Outcome, ScriptSource, Transform,
};
