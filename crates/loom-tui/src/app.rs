//! Application state for the Loom TUI.

use loom_core::{
    AttributeSchema, Decision, DecisionAnalysis, DynamicState, GoalVector, PassiveEffect,
    Simulation,
};
use std::sync::Arc;

/// Which screen the TUI is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Decision list browser.
    List,
    /// Detail view of a single decision.
    Detail,
    /// Simulation results.
    Results,
    /// Error screen.
    Error(String),
}

/// Scroll state for scrollable content.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
}

/// The full application state.
pub struct App {
    /// Loaded attribute schema.
    pub schema: Arc<AttributeSchema>,
    /// All loaded decisions.
    pub decisions: Vec<Decision>,
    /// All loaded passive effects.
    pub passives: Vec<PassiveEffect>,
    /// Loaded goal vector.
    pub goal: GoalVector,
    /// Initial state (configured from config or defaults).
    pub initial_state: DynamicState,

    /// Current screen.
    pub screen: Screen,
    /// Selected decision index in list view.
    pub selected_idx: usize,
    /// Scroll offset for detail/results views.
    pub scroll: ScrollState,

    /// Simulation configuration.
    pub sim_config: SimConfig,
    /// Last simulation result.
    pub last_result: Option<DecisionAnalysis>,
    /// The decision that was simulated.
    pub last_decision: Option<Decision>,
}

/// Simulation parameters.
#[derive(Debug, Clone)]
pub struct SimConfig {
    /// Forward horizon (steps).
    pub horizon: usize,
    /// Monte Carlo runs.
    pub runs: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            horizon: 24,
            runs: 1000,
        }
    }
}

impl App {
    /// Create an empty app (no configs loaded yet).
    /// Will be populated by the config loader.
    pub fn empty() -> Self {
        let schema = Arc::new(
            AttributeSchema::from_json(r#"{"version":1,"attributes":[]}"#).unwrap(),
        );
        let initial_state = DynamicState::new(schema.clone());
        Self {
            schema,
            decisions: Vec::new(),
            passives: Vec::new(),
            goal: GoalVector::linear(vec![]),
            initial_state,
            screen: Screen::List,
            selected_idx: 0,
            scroll: ScrollState::default(),
            sim_config: SimConfig::default(),
            last_result: None,
            last_decision: None,
        }
    }

    /// Run a simulation on the currently selected decision.
    pub fn run_simulation(&mut self) {
        let decision = &self.decisions[self.selected_idx];
        let sim = Simulation {
            horizon: self.sim_config.horizon,
            monte_carlo_runs: self.sim_config.runs,
            passives: self.passives.clone(),
        };
        let analysis = sim.run_and_analyze(&self.initial_state, decision, &self.goal);
        self.last_decision = Some(decision.clone());
        self.last_result = Some(analysis);
        self.screen = Screen::Results;
        self.scroll = ScrollState::default();
    }

    /// Return a reference to the currently selected decision.
    pub fn selected_decision(&self) -> Option<&Decision> {
        self.decisions.get(self.selected_idx)
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.decisions.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.decisions.len();
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if !self.decisions.is_empty() {
            self.selected_idx = self
                .selected_idx
                .checked_sub(1)
                .unwrap_or(self.decisions.len() - 1);
        }
    }
}
