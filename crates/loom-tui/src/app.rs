//! Application state for the Loom TUI.

use loom_core::{
    AttributeSchema, Decision, DecisionAnalysis, DynamicState, GoalVector, PassiveEffect,
    Simulation,
};
use loom_store::{SavedState, Store};
use std::sync::Arc;

/// Which screen the TUI is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Decision list browser.
    List,
    /// Detail view of a single decision.
    Detail,
    /// State manager — save/load/branch states.
    StateManager,
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
    /// Persistence store for state snapshots.
    pub store: Store,

    /// Loaded attribute schema.
    pub schema: Arc<AttributeSchema>,
    /// All loaded decisions.
    pub decisions: Vec<Decision>,
    /// All loaded passive effects.
    pub passives: Vec<PassiveEffect>,
    /// Loaded goal vector.
    pub goal: GoalVector,
    /// Current state (mutable via state manager load/save).
    pub current_state: DynamicState,

    /// Current screen.
    pub screen: Screen,
    /// Previous screen (for back-navigation from state manager).
    pub prev_screen: Screen,
    /// Selected decision index in list view.
    pub selected_idx: usize,
    /// Selected state index in state manager.
    pub state_idx: usize,
    /// Scroll offset for detail/results views.
    pub scroll: ScrollState,

    /// List of saved states (refreshed on state manager entry).
    pub saved_states: Vec<SavedState>,
    /// Name input buffer for saving a new state.
    pub save_name: String,
    /// Note input buffer.
    pub save_note: String,
    /// Are we in "input mode" (typing a name)?
    pub input_mode: bool,
    /// Is the current input a branch operation?
    pub branching: bool,

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
    pub horizon: usize,
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
    /// Create an app with a store, schema, and initial state.
    pub fn new(
        store: Store,
        schema: Arc<AttributeSchema>,
        decisions: Vec<Decision>,
        passives: Vec<PassiveEffect>,
        goal: GoalVector,
        current_state: DynamicState,
    ) -> Self {
        Self {
            store,
            schema,
            decisions,
            passives,
            goal,
            current_state,
            screen: Screen::List,
            prev_screen: Screen::List,
            selected_idx: 0,
            state_idx: 0,
            scroll: ScrollState::default(),
            saved_states: Vec::new(),
            save_name: String::new(),
            save_note: String::new(),
            input_mode: false,
            branching: false,
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
        let analysis = sim.run_and_analyze(&self.current_state, decision, &self.goal);
        self.last_decision = Some(decision.clone());
        self.last_result = Some(analysis);
        self.screen = Screen::Results;
        self.scroll = ScrollState::default();
    }

    /// Open the state manager, refreshing the saved states list.
    pub fn open_state_manager(&mut self) {
        self.saved_states = self.store.list_states().unwrap_or_default();
        self.state_idx = if self.saved_states.is_empty() { 0 } else { 0 };
        self.prev_screen = self.screen.clone();
        self.screen = Screen::StateManager;
        self.input_mode = false;
    }

    /// Save the current state (or branch if branch-mode was activated).
    pub fn save_current_state(&mut self) -> bool {
        if self.save_name.trim().is_empty() {
            return false;
        }
        let ok = if self.branching {
            let from = self.saved_states.get(self.state_idx).map(|s| s.name.clone());
            match from {
                Some(src) => self
                    .store
                    .branch_state(&src, &self.save_name, &self.save_note)
                    .unwrap_or(false),
                None => false,
            }
        } else {
            let values = self.current_state.as_slice().to_vec();
            self.store
                .save_state(&self.save_name, &self.save_note, &values)
                .is_ok()
        };
        if ok {
            self.saved_states = self.store.list_states().unwrap_or_default();
            self.save_name.clear();
            self.save_note.clear();
            self.input_mode = false;
            self.branching = false;
        }
        ok
    }

    /// Load a saved state into current_state.
    pub fn load_state(&mut self) -> bool {
        if let Some(saved) = self.saved_states.get(self.state_idx) {
            let state = DynamicState::from_vec(saved.values.clone(), self.schema.clone());
            self.current_state = state;
            self.screen = self.prev_screen.clone();
            true
        } else {
            false
        }
    }

    /// Delete the selected saved state.
    pub fn delete_state(&mut self) -> bool {
        if let Some(saved) = self.saved_states.get(self.state_idx) {
            let name = saved.name.clone();
            if self.store.delete_state(&name).is_ok() {
                self.saved_states = self.store.list_states().unwrap_or_default();
                if self.state_idx >= self.saved_states.len() && !self.saved_states.is_empty() {
                    self.state_idx = self.saved_states.len() - 1;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Select previous state in the state manager list.
    pub fn state_prev(&mut self) {
        if !self.saved_states.is_empty() {
            self.state_idx = self
                .state_idx
                .checked_sub(1)
                .unwrap_or(self.saved_states.len() - 1);
        }
    }

    /// Select next state in the state manager list.
    pub fn state_next(&mut self) {
        if !self.saved_states.is_empty() {
            self.state_idx = (self.state_idx + 1) % self.saved_states.len();
        }
    }

    /// Return a reference to the currently selected decision.
    pub fn selected_decision(&self) -> Option<&Decision> {
        self.decisions.get(self.selected_idx)
    }

    /// Move selection down in decision list.
    pub fn select_next(&mut self) {
        if !self.decisions.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.decisions.len();
        }
    }

    /// Move selection up in decision list.
    pub fn select_prev(&mut self) {
        if !self.decisions.is_empty() {
            self.selected_idx = self
                .selected_idx
                .checked_sub(1)
                .unwrap_or(self.decisions.len() - 1);
        }
    }

    /// Push a character into the current input buffer.
    pub fn input_char(&mut self, c: char) {
        if self.input_mode {
            self.save_name.push(c);
        }
    }

    /// Backspace in the current input buffer.
    pub fn input_backspace(&mut self) {
        if self.input_mode {
            self.save_name.pop();
        }
    }
}
