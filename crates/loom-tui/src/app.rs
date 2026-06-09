//! Application state for the Loom TUI.

use loom_core::{
    AttributeSchema, Decision, DecisionAnalysis, DynamicState, GoalVector, NamedCondition,
    NamedDecision, NamedEffect, NamedGoalVector, NamedOutcome, NamedPassiveEffect, PassiveEffect,
    Simulation,
};
use loom_store::{ForkRow, SchemaSummary, SnapshotRow, Store, TimelineStore, TimelineSummary};
use loom_store::{AppliedEventEffect, NamedEvent};
use std::collections::HashMap;
use std::sync::Arc;

/// Which screen the TUI is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Schema (template) selection at startup.
    SchemaList,
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
    /// Edit decisions list.
    EditDecisions,
    /// Edit a single decision's fields.
    EditDecisionDetail,
    /// Edit passives list.
    EditPassives,
    /// Edit a single passive.
    EditPassiveDetail,
    /// Edit goals list.
    EditGoals,
    /// Edit a single goal's weights/cliffs.
    EditGoalDetail,
    /// Timeline browser — list all timelines.
    TimelineBrowser,
    /// Snapshot list for a timeline.
    SnapshotList,
    /// Detail view of a single snapshot.
    SnapshotDetail,
    /// Fork browser — list forks for a timeline.
    ForkBrowser,
    /// Edit events list.
    EditEvents,
    /// Edit a single event's fields.
    EditEventsDetail,
}

/// Scroll state for scrollable content.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
}

/// Sub-mode within an edit detail screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditField {
    /// Editing the label.
    Label,
    /// Editing preconditions (decision only).
    Preconditions,
    /// Editing cost effects.
    CostEffects,
    /// Editing outcomes (decision only).
    Outcomes,
    /// Editing frequency (passive only).
    Frequency,
    /// Editing passive effects.
    Effects,
    /// Editing weights (goal only).
    Weights,
    /// Editing cliffs (goal only).
    Cliffs,
}

/// State for the inline decision editor.
#[derive(Debug, Clone)]
pub struct DecisionEditState {
    pub idx: usize,
    pub label: String,
    pub preconditions: Vec<NamedCondition>,
    pub cost: Vec<NamedEffect>,
    pub outcomes: Vec<NamedOutcome>,
    pub text_buffer: String,
    pub active_field: EditField,
    /// When editing a list (preconditions, cost, outcomes), which item is selected.
    pub list_idx: usize,
    /// Sub-list index for editing outcome effects.
    pub sub_list_idx: usize,
    /// Are we editing a sub-item's field?
    pub in_sub_edit: bool,
    /// Buffer field identifier for sub-items: "attr", "op", "val", "weight", "label"
    pub sub_field: String,
}

/// State for the inline passive editor.
#[derive(Debug, Clone)]
pub struct PassiveEditState {
    pub idx: usize,
    pub label: String,
    pub passive_id: String,
    pub effects: Vec<NamedEffect>,
    pub text_buffer: String,
    pub list_idx: usize,
}

/// State for the inline goal editor.
#[derive(Debug, Clone)]
pub struct GoalEditState {
    pub idx: usize,
    pub goal_name: String,
    pub weights: Vec<(String, f64)>,
    pub cliffs: Vec<(String, loom_core::Threshold)>,
    pub text_buffer: String,
    pub list_idx: usize,
    pub show_weights: bool, // true=editing weights, false=editing cliffs
}

/// State for the inline event editor.
#[derive(Debug, Clone)]
pub struct EventEditState {
    pub idx: usize,
    pub event_id: String,
    pub label: String,
    pub description: String,
    pub preconditions: Vec<NamedCondition>,
    pub delay: u32,
    pub duration: u32,
    pub cooldown: u32,
    pub effects: Vec<NamedEffect>,
    pub spawns_decision_id: String,
    pub text_buffer: String,
    pub list_idx: usize,
}

/// The full application state.
pub struct App {
    /// Persistence store for state snapshots.
    pub store: Store,

    /// Loaded attribute schema.
    pub schema: Arc<AttributeSchema>,
    /// Current schema name.
    pub schema_name: String,
    /// All loaded decisions.
    pub decisions: Vec<Decision>,
    /// Named decisions (for editing/re-saving).
    pub named_decisions: Vec<NamedDecision>,
    /// All loaded passive effects.
    pub passives: Vec<PassiveEffect>,
    /// Named passive effects (for editing).
    pub named_passives: Vec<NamedPassiveEffect>,
    /// Loaded goal vector.
    pub goal: GoalVector,
    /// Named goal vector (for editing).
    pub named_goal: NamedGoalVector,
    /// Current state (mutable via state manager load/save).
    pub current_state: DynamicState,

    /// Available schema list.
    pub schema_list: Vec<SchemaSummary>,
    /// Selected schema index.
    pub schema_idx: usize,

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
    pub saved_states: Vec<loom_store::SavedState>,
    /// Name input buffer for saving a new state.
    pub save_name: String,
    /// Note input buffer.
    pub save_note: String,
    /// Are we in "input mode" (typing a name)?
    pub input_mode: bool,
    /// Is the current input a branch operation?
    pub branching: bool,
    /// Confirmation prompt: show "Delete X? (y/n)"
    pub confirm_delete: Option<String>,

    /// Simulation configuration.
    pub sim_config: SimConfig,
    /// Last simulation result.
    pub last_result: Option<DecisionAnalysis>,
    /// The decision that was simulated.
    pub last_decision: Option<Decision>,

    // ── Edit state ───────────────────────────────────────────────────────────────
    pub edit_decisions: Vec<NamedDecision>,
    pub edit_decision_idx: usize,
    pub edit_decision_detail: Option<DecisionEditState>,

    pub edit_passives: Vec<NamedPassiveEffect>,
    pub edit_passive_idx: usize,
    pub edit_passive_detail: Option<PassiveEditState>,

    pub edit_goals: Vec<(String, NamedGoalVector)>,
    pub edit_goal_idx: usize,
    pub edit_goal_detail: Option<GoalEditState>,

    pub edit_events: Vec<NamedEvent>,
    pub edit_event_idx: usize,
    pub edit_event_detail: Option<EventEditState>,

    pub active_events_status: Vec<String>, // Display strings for active events on current timeline
    pub last_event_effects: Vec<AppliedEventEffect>, // Effects from last append

    // ── Timeline state ──────────────────────────────────────────────────────────
    /// Tab: 0=Timeline, 1=Explore, 2=Config
    pub tab: usize,
    /// All timelines (cached).
    pub timelines: Vec<TimelineSummary>,
    /// Active timeline ID (for snapshot list view).
    pub active_timeline_id: Option<i64>,
    /// Active timeline name.
    pub active_timeline_name: String,
    /// Active timeline schema name (resolved).
    pub active_timeline_schema_name: String,
    /// Snapshots for the active timeline.
    pub snapshots: Vec<SnapshotRow>,
    /// Forks for the active timeline.
    pub forks: Vec<ForkRow>,
    /// Index into timelines list.
    pub timeline_idx: usize,
    /// Index into snapshots list.
    pub snapshot_idx: usize,
    /// Index into forks list.
    pub fork_idx: usize,
    /// Input buffer for timeline/snapshot creation prompts.
    pub input_buffer: String,
    /// Prompt label when in input mode.
    pub input_prompt: String,
    /// Are we in timeline input mode (create timeline, append snapshot, etc.)?
    pub timeline_input_mode: bool,
    /// Current action context for timeline_input_mode.
    pub timeline_input_action: TimelineInputAction,
    /// Selected schema index for create-timeline flow.
    pub create_timeline_schema_idx: usize,
}

/// What the timeline input prompt is asking for.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineInputAction {
    None,
    CreateTimelineName,
    CreateTimelineSchema,
    AppendSnapshotEntry,
    ForkName,
    ForkLabel,
    ResolveOutcome,
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
    /// Create an app with a store and initial state for schema browser.
    pub fn new_browsing(store: Store, schema_list: Vec<SchemaSummary>) -> Self {
        let empty_schema = Arc::new(AttributeSchema {
            version: 1,
            attributes: vec![],
        });
        Self {
            store,
            schema: empty_schema.clone(),
            schema_name: String::new(),
            decisions: Vec::new(),
            named_decisions: Vec::new(),
            passives: Vec::new(),
            named_passives: Vec::new(),
            goal: GoalVector { weights: vec![], cliffs: vec![] },
            named_goal: NamedGoalVector { weights: HashMap::new(), cliffs: HashMap::new() },
            current_state: DynamicState::from_vec(vec![], empty_schema.clone()),
            schema_list,
            schema_idx: 0,
            screen: Screen::TimelineBrowser,
            prev_screen: Screen::TimelineBrowser,
            selected_idx: 0,
            state_idx: 0,
            scroll: ScrollState::default(),
            saved_states: Vec::new(),
            save_name: String::new(),
            save_note: String::new(),
            input_mode: false,
            branching: false,
            confirm_delete: None,
            sim_config: SimConfig::default(),
            last_result: None,
            last_decision: None,
            edit_decisions: Vec::new(),
            edit_decision_idx: 0,
            edit_decision_detail: None,
            edit_passives: Vec::new(),
            edit_passive_idx: 0,
            edit_passive_detail: None,
            edit_goals: Vec::new(),
            edit_goal_idx: 0,
            edit_goal_detail: None,

            edit_events: Vec::new(),
            edit_event_idx: 0,
            edit_event_detail: None,

            active_events_status: Vec::new(),
            last_event_effects: Vec::new(),
            tab: 0,
            timelines: Vec::new(),
            active_timeline_id: None,
            active_timeline_name: String::new(),
            active_timeline_schema_name: String::new(),
            snapshots: Vec::new(),
            forks: Vec::new(),
            timeline_idx: 0,
            snapshot_idx: 0,
            fork_idx: 0,
            input_buffer: String::new(),
            input_prompt: String::new(),
            timeline_input_mode: false,
            timeline_input_action: TimelineInputAction::None,
            create_timeline_schema_idx: 0,
        }
    }

    /// Load all data for a selected schema.
    pub fn load_schema(&mut self, schema_name: &str) -> Result<(), String> {
        let schema = Arc::new(
            self.store
                .get_schema(schema_name)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Schema '{}' not found", schema_name))?,
        );
        self.schema = schema.clone();
        self.schema_name = schema_name.to_string();

        let named_decisions = self
            .store
            .list_decisions(schema_name)
            .map_err(|e| e.to_string())?;
        self.named_decisions = named_decisions.clone();
        self.decisions = named_decisions
            .iter()
            .map(|nd| nd.resolve(&schema))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("failed to resolve decision: {e}"))?;

        let named_passives = self.store.list_passives(schema_name).map_err(|e| e.to_string())?;
        self.named_passives = named_passives.clone();
        self.passives = named_passives
            .iter()
            .map(|np| np.resolve(&schema))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("failed to resolve passive: {e}"))?;

        let named_goal = self
            .store
            .get_goal(schema_name, "default")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("No goal 'default' in schema '{}'", schema_name))?;
        self.named_goal = named_goal.clone();
        self.goal = named_goal
            .resolve(&schema)
            .map_err(|e| format!("failed to resolve goal: {e}"))?;

        Ok(())
    }

    /// Set default initial state values for the current schema.
    pub fn init_default_state(&mut self) {
        let mut state = DynamicState::new(self.schema.clone());
        for attr in &self.schema.attributes {
            // Set reasonable defaults: midpoint of bounds or a default value
            if let Some(b) = &attr.bounds {
                let mid = (b.0 + b.1) / 2.0;
                state.set(&attr.name, mid);
            } else {
                state.set(&attr.name, 50000.0);
            }
        }
        // Override specific defaults for financial
        if self.schema_name == "financial" {
            state.set("cash", 25000.0);
            state.set("stocks", 10000.0);
            state.set("bonds", 5000.0);
            state.set("debt", 15000.0);
            state.set("monthly_income", 5000.0);
            state.set("monthly_expenses", 3500.0);
            state.set("credit_score", 700.0);
            state.set("risk_tolerance", 50.0);
            state.set("retirement_savings", 15000.0);
        }
        if self.schema_name == "personal" {
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
        }
        self.current_state = state;
    }

    /// Reload decisions/passives/goals for current schema from store.
    pub fn reload_data(&mut self) {
        let name = self.schema_name.clone();
        if name.is_empty() {
            return;
        }
        if let Err(e) = self.load_schema(&name) {
            self.screen = Screen::Error(e);
        }
    }

    /// Run a simulation on the currently selected decision.
    pub fn run_simulation(&mut self) {
        if self.decisions.is_empty() || self.selected_idx >= self.decisions.len() {
            return;
        }
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
        self.state_idx = 0;
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
        if self.timeline_input_mode {
            self.input_buffer.push(c);
        } else if self.input_mode {
            self.save_name.push(c);
        }
    }

    /// Backspace in the current input buffer.
    pub fn input_backspace(&mut self) {
        if self.timeline_input_mode {
            self.input_buffer.pop();
        } else if self.input_mode {
            self.save_name.pop();
        }
    }

    // ── Timeline methods ───────────────────────────────────────────────────────

    /// Load all timelines from the store.
    pub fn load_timelines(&mut self) {
        let ts = TimelineStore::new(&self.store.conn);
        self.timelines = ts.list_timelines().unwrap_or_default();
    }

    /// Load snapshots for a timeline.
    pub fn load_snapshots(&mut self, timeline_id: i64) {
        let ts = TimelineStore::new(&self.store.conn);
        self.snapshots = ts.list_snapshots(timeline_id).unwrap_or_default();
    }

    /// Load forks for a timeline.
    pub fn load_forks(&mut self, timeline_id: i64) {
        let ts = TimelineStore::new(&self.store.conn);
        self.forks = ts.list_forks(timeline_id).unwrap_or_default();
    }

    /// Create a new timeline and load it.
    pub fn create_timeline(&mut self, name: &str, schema_id: i64) -> i64 {
        let ts = TimelineStore::new(&self.store.conn);
        let id = ts.create_timeline(name, schema_id).unwrap_or(0);
        self.load_timelines();
        id
    }

    /// Append a snapshot to the active timeline using current state values.
    pub fn append_snapshot_to_timeline(&mut self, entry_text: &str) -> i64 {
        let timeline_id = match self.active_timeline_id {
            Some(id) => id,
            None => return 0,
        };
        let values = self.current_state.as_slice().to_vec();
        let parent_id = self.snapshots.last().map(|s| s.id);
        let ts = TimelineStore::new(&self.store.conn);
        let id = ts.append_snapshot(timeline_id, parent_id, entry_text, &values).unwrap_or(0);
        self.load_snapshots(timeline_id);
        self.snapshot_idx = self.snapshots.len().saturating_sub(1);
        id
    }

    /// Fork the active timeline at the selected snapshot.
    pub fn fork_timeline(&mut self, name: &str, label: &str) -> i64 {
        let timeline_id = match self.active_timeline_id {
            Some(id) => id,
            None => return 0,
        };
        let snapshot_id = match self.snapshots.get(self.snapshot_idx) {
            Some(s) => s.id,
            None => return 0,
        };
        let ts = TimelineStore::new(&self.store.conn);
        let child_id = ts.fork_timeline(timeline_id, snapshot_id, name, label).unwrap_or(0);
        self.load_forks(timeline_id);
        child_id
    }

    /// Resolve a snapshot's actual outcome.
    pub fn resolve_snapshot(&mut self, actual_deltas_json: &str) {
        let snapshot_id = match self.snapshots.get(self.snapshot_idx) {
            Some(s) => s.id,
            None => return,
        };
        let ts = TimelineStore::new(&self.store.conn);
        let _ = ts.resolve_outcome(snapshot_id, actual_deltas_json);
        if let Some(tid) = self.active_timeline_id {
            self.load_snapshots(tid);
        }
    }

    pub fn open_timeline(&mut self, idx: usize) {
        if let Some(tl) = self.timelines.get(idx).cloned() {
            self.active_timeline_id = Some(tl.id);
            self.active_timeline_name = tl.name.clone();
            // Resolve schema name
            let schema_name = self
                .schema_list
                .iter()
                .find(|s| s.id == tl.schema_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| format!("schema#{}", tl.schema_id));
            self.active_timeline_schema_name = schema_name.clone();
            self.load_snapshots(tl.id);
            self.load_forks(tl.id);
            self.snapshot_idx = 0;
            self.fork_idx = 0;
            self.screen = Screen::SnapshotList;
            self.scroll = ScrollState::default();

            // Also load schema data for this timeline
            if let Err(e) = self.load_schema(&schema_name) {
                eprintln!("Failed to load schema for timeline: {e}");
            } else {
                // Restore the last snapshot's state if there are snapshots
                if !self.snapshots.is_empty() {
                    let last = &self.snapshots[self.snapshots.len() - 1];
                    let values: Vec<f64> = serde_json::from_str(&last.attributes_json).unwrap_or_default();
                    self.current_state = DynamicState::from_vec(values, self.schema.clone());
                } else {
                    self.init_default_state();
                }
            }
        }
    }

    // ── Edit navigation ──────────────────────────────────────────────────────────

    pub fn open_edit_decisions(&mut self) {
        self.edit_decisions = self.named_decisions.clone();
        self.edit_decision_idx = 0;
        self.prev_screen = self.screen.clone();
        self.screen = Screen::EditDecisions;
    }

    pub fn open_edit_passives(&mut self) {
        self.edit_passives = self.named_passives.clone();
        self.edit_passive_idx = 0;
        self.prev_screen = self.screen.clone();
        self.screen = Screen::EditPassives;
    }

    pub fn open_edit_goals(&mut self) {
        let goals = match self.store.list_goals(&self.schema_name) {
            Ok(names) => names,
            Err(_) => vec!["default".to_string()],
        };
        self.edit_goals = goals
            .iter()
            .map(|n| {
                let g = self.store.get_goal(&self.schema_name, n).ok().flatten();
                (n.clone(), g.unwrap_or_else(|| self.named_goal.clone()))
            })
            .collect();
        self.edit_goal_idx = 0;
        self.prev_screen = self.screen.clone();
        self.screen = Screen::EditGoals;
    }

    pub fn edit_decision_prev(&mut self) {
        if !self.edit_decisions.is_empty() {
            self.edit_decision_idx = self
                .edit_decision_idx
                .checked_sub(1)
                .unwrap_or(self.edit_decisions.len() - 1);
        }
    }

    pub fn edit_decision_next(&mut self) {
        if !self.edit_decisions.is_empty() {
            self.edit_decision_idx =
                (self.edit_decision_idx + 1) % self.edit_decisions.len();
        }
    }

    pub fn edit_passive_prev(&mut self) {
        if !self.edit_passives.is_empty() {
            self.edit_passive_idx = self
                .edit_passive_idx
                .checked_sub(1)
                .unwrap_or(self.edit_passives.len() - 1);
        }
    }

    pub fn edit_passive_next(&mut self) {
        if !self.edit_passives.is_empty() {
            self.edit_passive_idx =
                (self.edit_passive_idx + 1) % self.edit_passives.len();
        }
    }

    pub fn edit_goal_prev(&mut self) {
        if !self.edit_goals.is_empty() {
            self.edit_goal_idx = self
                .edit_goal_idx
                .checked_sub(1)
                .unwrap_or(self.edit_goals.len() - 1);
        }
    }

    pub fn edit_goal_next(&mut self) {
        if !self.edit_goals.is_empty() {
            self.edit_goal_idx = (self.edit_goal_idx + 1) % self.edit_goals.len();
        }
    }

    /// Open a decision for inline editing.
    pub fn open_decision_detail(&mut self, idx: usize) {
        if let Some(d) = self.edit_decisions.get(idx) {
            self.edit_decision_detail = Some(DecisionEditState {
                idx,
                label: d.label.clone(),
                preconditions: d.preconditions.clone(),
                cost: d.cost.clone(),
                outcomes: d.outcomes.clone(),
                text_buffer: String::new(),
                active_field: EditField::Label,
                list_idx: 0,
                sub_list_idx: 0,
                in_sub_edit: false,
                sub_field: String::new(),
            });
            self.screen = Screen::EditDecisionDetail;
        }
    }

    /// Open a passive for inline editing.
    pub fn open_passive_detail(&mut self, idx: usize) {
        if let Some(p) = self.edit_passives.get(idx) {
            self.edit_passive_detail = Some(PassiveEditState {
                idx,
                label: p.label.clone(),
                passive_id: p.id.clone(),
                effects: p.effects.clone(),
                text_buffer: String::new(),
                list_idx: 0,
            });
            self.screen = Screen::EditPassiveDetail;
        }
    }

    /// Open a goal for inline editing.
    pub fn open_goal_detail(&mut self, idx: usize) {
        if let Some((name, g)) = self.edit_goals.get(idx) {
            let mut weights: Vec<(String, f64)> = g.weights.clone().into_iter().collect();
            weights.sort_by(|a, b| a.0.cmp(&b.0));
            let mut cliffs: Vec<(String, loom_core::Threshold)> =
                g.cliffs.clone().into_iter().collect();
            cliffs.sort_by(|a, b| a.0.cmp(&b.0));
            self.edit_goal_detail = Some(GoalEditState {
                idx,
                goal_name: name.clone(),
                weights,
                cliffs,
                text_buffer: String::new(),
                list_idx: 0,
                show_weights: true,
            });
            self.screen = Screen::EditGoalDetail;
        }
    }

    /// Save edits to a decision to the store and reload.
    pub fn save_decision_edit(&mut self) {
        if let Some(state) = self.edit_decision_detail.take() {
            let nd = NamedDecision {
                id: self.edit_decisions[state.idx].id.clone(),
                label: state.label,
                preconditions: state.preconditions,
                cost: state.cost,
                outcomes: state.outcomes,
            };
            let _ = self.store.upsert_decision(&self.schema_name, &nd);
            self.edit_decisions[state.idx] = nd;
            self.reload_data();
        }
        self.screen = Screen::EditDecisions;
    }

    /// Save edits to a passive to the store and reload.
    pub fn save_passive_edit(&mut self) {
        if let Some(state) = self.edit_passive_detail.take() {
            // Find original passive to preserve frequency
            let freq = self
                .named_passives
                .iter()
                .find(|p| p.id == state.passive_id)
                .map(|p| p.frequency.clone())
                .unwrap_or(loom_core::NamedFrequency::EveryStep);

            let np = NamedPassiveEffect {
                id: state.passive_id,
                label: state.label,
                frequency: freq,
                effects: state.effects,
            };
            let _ = self.store.upsert_passive(&self.schema_name, &np);
            self.reload_data();
        }
        self.screen = Screen::EditPassives;
    }

    /// Save edits to a goal to the store and reload.
    pub fn save_goal_edit(&mut self) {
        if let Some(state) = self.edit_goal_detail.take() {
            let mut weights = HashMap::new();
            for (name, w) in &state.weights {
                weights.insert(name.clone(), *w);
            }
            let mut cliffs = HashMap::new();
            for (name, t) in &state.cliffs {
                cliffs.insert(name.clone(), t.clone());
            }
            let ng = NamedGoalVector { weights, cliffs };
            let _ = self.store.upsert_goal(&self.schema_name, &state.goal_name, &ng);
            self.reload_data();
        }
        self.screen = Screen::EditGoals;
    }

    /// Delete a decision from the store.
    pub fn delete_edit_decision(&mut self, idx: usize) -> bool {
        if let Some(d) = self.edit_decisions.get(idx) {
            if self.store.delete_decision(&self.schema_name, &d.id).is_ok() {
                self.edit_decisions.remove(idx);
                if self.edit_decision_idx >= self.edit_decisions.len() && !self.edit_decisions.is_empty() {
                    self.edit_decision_idx = self.edit_decisions.len() - 1;
                }
                self.reload_data();
                return true;
            }
        }
        false
    }

    /// Delete a passive from the store.
    pub fn delete_edit_passive(&mut self, idx: usize) -> bool {
        if let Some(p) = self.edit_passives.get(idx) {
            if self.store.delete_passive(&self.schema_name, &p.id).is_ok() {
                self.edit_passives.remove(idx);
                if self.edit_passive_idx >= self.edit_passives.len() && !self.edit_passives.is_empty() {
                    self.edit_passive_idx = self.edit_passives.len() - 1;
                }
                self.reload_data();
                return true;
            }
        }
        false
    }

    /// Delete a goal from the store.
    pub fn delete_edit_goal(&mut self, idx: usize) -> bool {
        if let Some((name, _)) = self.edit_goals.get(idx) {
            if self.store.delete_goal(&self.schema_name, name).is_ok() {
                self.edit_goals.remove(idx);
                if self.edit_goal_idx >= self.edit_goals.len() && !self.edit_goals.is_empty() {
                    self.edit_goal_idx = self.edit_goals.len() - 1;
                }
                self.reload_data();
                return true;
            }
        }
        false
    }

    // ── Event edit methods ────────────────────────────────────────────────────

    pub fn open_edit_events(&mut self) {
        self.edit_events = self.store.list_events(&self.schema_name).unwrap_or_default();
        self.edit_event_idx = 0;
        self.prev_screen = self.screen.clone();
        self.screen = Screen::EditEvents;
    }

    pub fn edit_event_prev(&mut self) {
        if !self.edit_events.is_empty() {
            self.edit_event_idx = self
                .edit_event_idx
                .checked_sub(1)
                .unwrap_or(self.edit_events.len() - 1);
        }
    }

    pub fn edit_event_next(&mut self) {
        if !self.edit_events.is_empty() {
            self.edit_event_idx = (self.edit_event_idx + 1) % self.edit_events.len();
        }
    }

    pub fn open_event_detail(&mut self, idx: usize) {
        if let Some(e) = self.edit_events.get(idx) {
            self.edit_event_detail = Some(EventEditState {
                idx,
                event_id: e.id.clone(),
                label: e.label.clone(),
                description: e.description.clone(),
                preconditions: e.preconditions.clone(),
                delay: e.delay,
                duration: e.duration,
                cooldown: e.cooldown,
                effects: e.effects.clone(),
                spawns_decision_id: e.spawns_decision_id.clone().unwrap_or_default(),
                text_buffer: String::new(),
                list_idx: 0,
            });
            self.screen = Screen::EditEventsDetail;
        }
    }

    pub fn save_event_edit(&mut self) {
        if let Some(state) = self.edit_event_detail.take() {
            let ne = NamedEvent {
                id: state.event_id,
                label: state.label,
                description: state.description,
                preconditions: state.preconditions,
                delay: state.delay,
                duration: state.duration,
                cooldown: state.cooldown,
                effects: state.effects,
                spawns_decision_id: if state.spawns_decision_id.is_empty() {
                    None
                } else {
                    Some(state.spawns_decision_id)
                },
            };
            let _ = self.store.upsert_event(&self.schema_name, &ne);
            self.reload_data();
            self.edit_events = self.store.list_events(&self.schema_name).unwrap_or_default();
        }
        self.screen = Screen::EditEvents;
    }

    pub fn delete_edit_event(&mut self, idx: usize) -> bool {
        if let Some(e) = self.edit_events.get(idx) {
            if self.store.delete_event(&self.schema_name, &e.id).is_ok() {
                self.edit_events.remove(idx);
                if self.edit_event_idx >= self.edit_events.len() && !self.edit_events.is_empty() {
                    self.edit_event_idx = self.edit_events.len() - 1;
                }
                self.reload_data();
                return true;
            }
        }
        false
    }

    /// Append a snapshot with event auto-effects integrated.
    pub fn append_snapshot_with_events(&mut self, entry_text: &str) -> i64 {
        let timeline_id = match self.active_timeline_id {
            Some(id) => id,
            None => return 0,
        };
        let mut values = self.current_state.as_slice().to_vec();
        let parent_id = self.snapshots.last().map(|s| s.id);
        let ts = TimelineStore::new(&self.store.conn);

        // Check events
        if self.schema.dimension() > 0 {
            match ts.check_and_advance_events(timeline_id, &self.schema, &values) {
                Ok(effects) => {
                    self.last_event_effects = effects.clone();
                    // Apply deltas to values
                    for eff in &effects {
                        if let Some(idx) = self.schema.index_of(&eff.attribute_name) {
                            if idx < values.len() {
                                values[idx] += eff.delta;
                            }
                        }
                    }
                    // Build entry text with event info
                    let mut full_entry = entry_text.to_string();
                    for eff in &effects {
                        if eff.phase == "pending" {
                            full_entry.push_str(&format!("\n⏳ {}", eff.description));
                        } else if eff.phase == "active" {
                            full_entry.push_str(&format!("\n🚗 {}", eff.description));
                        } else if eff.phase == "resolved" {
                            full_entry.push_str(&format!("\n✅ {}", eff.description));
                        }
                        if let Some(ref dec_id) = eff.spawned_decision_id {
                            full_entry.push_str(&format!("\n❓ Decision spawned: {}", dec_id));
                        }
                    }
                    let id = ts
                        .append_snapshot(timeline_id, parent_id, &full_entry, &values)
                        .unwrap_or(0);

                    // Update current_state to match snapshot
                    self.current_state = DynamicState::from_vec(values, self.schema.clone());

                    // Refresh active event status
                    self.refresh_active_events_status();

                    self.load_snapshots(timeline_id);
                    self.snapshot_idx = self.snapshots.len().saturating_sub(1);
                    return id;
                }
                Err(e) => {
                    eprintln!("Event processing error: {e}");
                }
            }
        }

        // Fallback: plain append
        let id = ts
            .append_snapshot(timeline_id, parent_id, entry_text, &values)
            .unwrap_or(0);
        self.current_state = DynamicState::from_vec(values, self.schema.clone());
        self.load_snapshots(timeline_id);
        self.snapshot_idx = self.snapshots.len().saturating_sub(1);
        id
    }

    pub fn refresh_active_events_status(&mut self) {
        if let Some(timeline_id) = self.active_timeline_id {
            let ts = TimelineStore::new(&self.store.conn);
            if let Ok(statuses) = ts.get_active_events_status(timeline_id) {
                self.active_events_status = statuses
                    .iter()
                    .map(|(label, phase, remaining, _total, _cooldown)| {
                        match phase.as_str() {
                            "pending" => format!("⏳ {} pending ({} steps)", label, remaining),
                            "active" => format!("🚗 {} active ({}/?)", label, remaining),
                            "cooldown" => format!("🕐 {} cooldown ({} steps)", label, remaining),
                            _ => format!("{}: {}", label, phase),
                        }
                    })
                    .collect();
            }
        } else {
            self.active_events_status.clear();
        }
    }
}
