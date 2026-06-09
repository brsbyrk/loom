//! Loom TUI — terminal-based decision explorer with state persistence.
//!
//! Loads configuration from SQLite (~/.loom/loom.db), not from JSON files.
//! Run `cargo run --example seed -p loom-store` first to populate the DB.

mod app;
mod ui;

use app::{App, EditField, Screen, ScrollState, TimelineInputAction};
use loom_core::{DynamicState, NamedCondition, NamedEffect, NamedOutcome, NamedTransform};
use loom_store::{AppliedEventEffect, NamedEvent, Store, TimelineStore};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Load from DB ────────────────────────────────────────────────────────────
    let store = Store::open_default()?;
    let schema_list = store.list_schemas()?;

    // ── App (start with dashboard if schemas exist) ────────────────────────────
    let mut app = App::new_browsing(store, schema_list);
    app.load_timelines();

    // Auto-load first schema on startup so dashboard has data
    if !app.schema_list.is_empty() {
        let name = app.schema_list[0].name.clone();
        let _ = app.load_schema(&name);
        app.screen = Screen::Dashboard;
        app.refresh_dashboard();
    }

    // ── Terminal ─────────────────────────────────────────────────────────────────
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result?;

    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            // Global tab switching — 1=Timeline, 2=Explore, 3=Config
            match key.code {
                KeyCode::Char('1') => {
                    app.tab = 0;
                    app.load_timelines();
                    app.screen = Screen::TimelineBrowser;
                    continue;
                }
                KeyCode::Char('2') => {
                    app.tab = 1;
                    if app.schema_name.is_empty() && !app.schema_list.is_empty() {
                        app.screen = Screen::SchemaList;
                    } else if app.schema_name.is_empty() {
                        app.screen = Screen::SchemaList;
                    } else {
                        app.screen = Screen::List;
                    }
                    continue;
                }
                KeyCode::Char('3') => {
                    app.tab = 2;
                    if app.schema_name.is_empty() {
                        app.screen = Screen::SchemaList;
                    } else {
                        app.screen = Screen::EditDecisions;
                    }
                    continue;
                }
                // Events editor shortcut from Timeline tab
                KeyCode::Char('4') => {
                    app.tab = 2;
                    if app.schema_name.is_empty() {
                        app.screen = Screen::SchemaList;
                    } else {
                        app.screen = Screen::EditEvents;
                    }
                    continue;
                }
                _ => {}
            }
            match app.screen.clone() {
                Screen::SchemaList => handle_schema_list(app, key.code),
                Screen::List => handle_list(app, key.code),
                Screen::Detail => handle_detail(app, key.code),
                Screen::Results => handle_results(app, key.code),
                Screen::StateManager => handle_state_manager(app, key.code),
                Screen::Error(_) => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    _ => {}
                },
                Screen::EditDecisions => handle_edit_decisions(app, key.code),
                Screen::EditDecisionDetail => handle_edit_decision_detail(app, key.code),
                Screen::EditPassives => handle_edit_passives(app, key.code),
                Screen::EditPassiveDetail => handle_edit_passive_detail(app, key.code),
                Screen::EditGoals => handle_edit_goals(app, key.code),
                Screen::EditGoalDetail => handle_edit_goal_detail(app, key.code),
                Screen::TimelineBrowser => handle_timeline_browser(app, key.code),
                Screen::SnapshotList => handle_snapshot_list(app, key.code),
                Screen::SnapshotDetail => handle_snapshot_detail(app, key.code),
                Screen::ForkBrowser => handle_fork_browser(app, key.code),
                Screen::EditEvents => handle_edit_events(app, key.code),
                Screen::EditEventsDetail => handle_edit_events_detail(app, key.code),
                Screen::Dashboard => handle_dashboard(app, key.code),
                Screen::ForkExplorer => handle_fork_explorer(app, key.code),
            }
        }
    }
}

// ── Schema list handler ────────────────────────────────────────────────────────

fn handle_schema_list(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.schema_list.is_empty() {
                app.schema_idx = app
                    .schema_idx
                    .checked_sub(1)
                    .unwrap_or(app.schema_list.len() - 1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.schema_list.is_empty() {
                app.schema_idx = (app.schema_idx + 1) % app.schema_list.len();
            }
        }
        KeyCode::Enter => {
            if let Some(s) = app.schema_list.get(app.schema_idx) {
                let name = s.name.clone();
                match app.load_schema(&name) {
                    Ok(()) => {
                        app.selected_idx = 0;
                        app.screen = Screen::Dashboard;
                        app.refresh_dashboard();
                    }
                    Err(e) => app.screen = Screen::Error(e),
                }
            }
        }
        _ => {}
    }
}

// ── Screen handlers ───────────────────────────────────────────────────────────

fn handle_list(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Enter => app.screen = Screen::Detail,
        KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
        KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
        KeyCode::Char('e') | KeyCode::Char('E') => app.open_edit_decisions(),
        KeyCode::Char('p') | KeyCode::Char('P') => app.open_edit_passives(),
        KeyCode::Char('g') | KeyCode::Char('G') => app.open_edit_goals(),
        _ => {}
    }
}

fn handle_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::List;
            app.scroll.offset = 0;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
        KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
        _ => {}
    }
}

fn handle_results(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::List;
            app.scroll.offset = 0;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
        KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
        _ => {}
    }
}

fn handle_state_manager(app: &mut App, code: KeyCode) {
    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.delete_state();
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            if app.input_mode {
                app.input_mode = false;
                app.save_name.clear();
                app.save_note.clear();
            } else {
                app.screen = app.prev_screen.clone();
            }
        }
        KeyCode::Enter => {
            if app.input_mode {
                app.save_current_state();
            } else {
                app.load_state();
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            if !app.input_mode {
                app.load_state();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.input_mode {
                app.state_prev()
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.input_mode {
                app.state_next()
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if !app.input_mode {
                app.input_mode = true;
                app.save_name.clear();
                app.save_note.clear();
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if !app.input_mode {
                if let Some(s) = app.saved_states.get(app.state_idx) {
                    app.confirm_delete = Some(s.name.clone());
                }
            }
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            if !app.input_mode {
                app.input_mode = true;
                app.branching = true;
                app.save_name.clear();
                app.save_note = format!(
                    "branch from {}",
                    app.saved_states
                        .get(app.state_idx)
                        .map_or("?", |s| s.name.as_str())
                );
            }
        }
        KeyCode::Char(c) => app.input_char(c),
        KeyCode::Backspace => app.input_backspace(),
        _ => {}
    }
}

// ── Edit decisions list ───────────────────────────────────────────────────────

fn handle_edit_decisions(app: &mut App, code: KeyCode) {
    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let idx = app.edit_decision_idx;
                app.delete_edit_decision(idx);
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Up | KeyCode::Char('k') => app.edit_decision_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.edit_decision_next(),
        KeyCode::Enter => {
            if !app.edit_decisions.is_empty() {
                app.open_decision_detail(app.edit_decision_idx);
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(d) = app.edit_decisions.get(app.edit_decision_idx) {
                app.confirm_delete = Some(d.label.clone());
            }
        }
        _ => {}
    }
}

// ── Edit decision detail ──────────────────────────────────────────────────────

fn handle_edit_decision_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.save_decision_edit();
        }
        KeyCode::Tab => {
            if let Some(ref mut state) = app.edit_decision_detail {
                state.active_field = match state.active_field {
                    EditField::Label => EditField::Preconditions,
                    EditField::Preconditions => EditField::CostEffects,
                    EditField::CostEffects => EditField::Outcomes,
                    EditField::Outcomes => EditField::Label,
                    _ => EditField::Label,
                };
                state.list_idx = 0;
                state.sub_list_idx = 0;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.edit_decision_detail {
                match state.active_field {
                    EditField::Preconditions | EditField::CostEffects => {
                        if state.list_idx > 0 {
                            state.list_idx -= 1;
                        }
                    }
                    EditField::Outcomes => {
                        if state.sub_list_idx > 0
                            && state.sub_list_idx == state.list_idx
                        {
                            // Moving from sub-effects back to outcome list
                            state.sub_list_idx = 0;
                        } else if state.list_idx > 0 {
                            state.list_idx -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.edit_decision_detail {
                match state.active_field {
                    EditField::Preconditions => {
                        if state.list_idx + 1 < state.preconditions.len() {
                            state.list_idx += 1;
                        }
                    }
                    EditField::CostEffects => {
                        if state.list_idx + 1 < state.cost.len() {
                            state.list_idx += 1;
                        }
                    }
                    EditField::Outcomes => {
                        if state.sub_list_idx > 0 {
                            // Move down within sub-effects
                            if let Some(outcome) =
                                state.outcomes.get(state.list_idx)
                            {
                                if let NamedTransform::Declarative { effects, .. } =
                                    &outcome.transform
                                {
                                    if state.sub_list_idx + 1 <= effects.len() {
                                        state.sub_list_idx += 1;
                                    }
                                }
                            }
                            // Exited sub-list
                            if let Some(outcome) =
                                state.outcomes.get(state.list_idx)
                            {
                                if let NamedTransform::Declarative { effects, .. } =
                                    &outcome.transform
                                {
                                    if state.sub_list_idx > effects.len() {
                                        state.sub_list_idx = 0;
                                        if state.list_idx + 1 < state.outcomes.len() {
                                            state.list_idx += 1;
                                        }
                                    }
                                }
                            }
                        } else if state.list_idx + 1 < state.outcomes.len() {
                            state.list_idx += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut state) = app.edit_decision_detail {
                match state.active_field {
                    EditField::Preconditions => {
                        state.preconditions.push(NamedCondition {
                            attribute: "attr".into(),
                            operator: loom_core::ComparisonOp::Gt,
                            value: 0.0,
                        });
                        state.list_idx = state.preconditions.len() - 1;
                    }
                    EditField::CostEffects => {
                        state.cost.push(NamedEffect {
                            attribute: Some("attr".into()),
                            group: None,
                            delta: 0.0,
                            scaling: vec![],
                        });
                        state.list_idx = state.cost.len() - 1;
                    }
                    EditField::Outcomes => {
                        state.outcomes.push(NamedOutcome {
                            label: "new_outcome".into(),
                            weight: 50.0,
                            condition: None,
                            transform: NamedTransform::Declarative {
                                effects: vec![NamedEffect {
                                    attribute: Some("attr".into()),
                                    group: None,
                                    delta: 0.0,
                                    scaling: vec![],
                                }],
                                conditional: vec![],
                                default_conditional: vec![],
                            },
                        });
                        state.list_idx = state.outcomes.len() - 1;
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(ref mut state) = app.edit_decision_detail {
                match state.active_field {
                    EditField::Preconditions => {
                        if !state.preconditions.is_empty()
                            && state.list_idx < state.preconditions.len()
                        {
                            state.preconditions.remove(state.list_idx);
                            if state.list_idx >= state.preconditions.len()
                                && !state.preconditions.is_empty()
                            {
                                state.list_idx = state.preconditions.len() - 1;
                            }
                        }
                    }
                    EditField::CostEffects => {
                        if !state.cost.is_empty()
                            && state.list_idx < state.cost.len()
                        {
                            state.cost.remove(state.list_idx);
                            if state.list_idx >= state.cost.len()
                                && !state.cost.is_empty()
                            {
                                state.list_idx = state.cost.len() - 1;
                            }
                        }
                    }
                    EditField::Outcomes => {
                        if state.sub_list_idx > 0 {
                            // Delete sub-effect within outcome
                            if let Some(outcome) =
                                state.outcomes.get_mut(state.list_idx)
                            {
                                if let NamedTransform::Declarative { effects, .. } =
                                    &mut outcome.transform
                                {
                                    let idx = state.sub_list_idx - 1;
                                    if idx < effects.len() {
                                        effects.remove(idx);
                                        state.sub_list_idx = 0;
                                    }
                                }
                            }
                        } else if !state.outcomes.is_empty()
                            && state.list_idx < state.outcomes.len()
                        {
                            state.outcomes.remove(state.list_idx);
                            if state.list_idx >= state.outcomes.len()
                                && !state.outcomes.is_empty()
                            {
                                state.list_idx = state.outcomes.len() - 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            if let Some(ref mut state) = app.edit_decision_detail {
                if state.active_field == EditField::Outcomes
                    && !state.outcomes.is_empty()
                    && state.list_idx < state.outcomes.len()
                {
                    // Toggle sub-edit mode
                    if state.sub_list_idx == 0 {
                        // Enter sub-list editing of outcome effects
                        if let NamedTransform::Declarative { effects, .. } =
                            &state.outcomes[state.list_idx].transform
                        {
                            if !effects.is_empty() {
                                state.sub_list_idx = 1; // Start at first effect
                            }
                        }
                    } else {
                        state.sub_list_idx = 0; // Exit sub-list
                    }
                }
            }
        }
        _ => {}
    }
}

// ── Edit passives list ────────────────────────────────────────────────────────

fn handle_edit_passives(app: &mut App, code: KeyCode) {
    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let idx = app.edit_passive_idx;
                app.delete_edit_passive(idx);
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Up | KeyCode::Char('k') => app.edit_passive_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.edit_passive_next(),
        KeyCode::Enter => {
            if !app.edit_passives.is_empty() {
                app.open_passive_detail(app.edit_passive_idx);
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(p) = app.edit_passives.get(app.edit_passive_idx) {
                app.confirm_delete = Some(p.label.clone());
            }
        }
        _ => {}
    }
}

// ── Edit passive detail ───────────────────────────────────────────────────────

fn handle_edit_passive_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.save_passive_edit();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.edit_passive_detail {
                if state.list_idx > 0 {
                    state.list_idx -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.edit_passive_detail {
                if state.list_idx + 1 < state.effects.len() {
                    state.list_idx += 1;
                }
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut state) = app.edit_passive_detail {
                state.effects.push(NamedEffect {
                    attribute: Some("attr".into()),
                    group: None,
                    delta: 0.0,
                    scaling: vec![],
                });
                state.list_idx = state.effects.len() - 1;
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(ref mut state) = app.edit_passive_detail {
                if !state.effects.is_empty() && state.list_idx < state.effects.len() {
                    state.effects.remove(state.list_idx);
                    if state.list_idx >= state.effects.len() && !state.effects.is_empty() {
                        state.list_idx = state.effects.len() - 1;
                    }
                }
            }
        }
        _ => {}
    }
}

// ── Edit goals list ───────────────────────────────────────────────────────────

fn handle_edit_goals(app: &mut App, code: KeyCode) {
    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let idx = app.edit_goal_idx;
                app.delete_edit_goal(idx);
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::List;
        }
        KeyCode::Up | KeyCode::Char('k') => app.edit_goal_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.edit_goal_next(),
        KeyCode::Enter => {
            if !app.edit_goals.is_empty() {
                app.open_goal_detail(app.edit_goal_idx);
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some((name, _)) = app.edit_goals.get(app.edit_goal_idx) {
                app.confirm_delete = Some(name.clone());
            }
        }
        _ => {}
    }
}

// ── Edit goal detail ──────────────────────────────────────────────────────────

fn handle_edit_goal_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.save_goal_edit();
        }
        KeyCode::Tab => {
            if let Some(ref mut state) = app.edit_goal_detail {
                state.show_weights = !state.show_weights;
                state.list_idx = 0;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.edit_goal_detail {
                if state.list_idx > 0 {
                    state.list_idx -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.edit_goal_detail {
                let max = if state.show_weights {
                    state.weights.len()
                } else {
                    state.cliffs.len()
                };
                if state.list_idx + 1 < max {
                    state.list_idx += 1;
                }
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut state) = app.edit_goal_detail {
                if state.show_weights {
                    state.weights.push(("attr".into(), 0.0));
                    state.list_idx = state.weights.len() - 1;
                } else {
                    state.cliffs.push((
                        "attr".into(),
                        loom_core::Threshold {
                            min: 0.0,
                            penalty: 1.0,
                        },
                    ));
                    state.list_idx = state.cliffs.len() - 1;
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(ref mut state) = app.edit_goal_detail {
                if state.show_weights {
                    if !state.weights.is_empty() && state.list_idx < state.weights.len() {
                        state.weights.remove(state.list_idx);
                        if state.list_idx >= state.weights.len() && !state.weights.is_empty() {
                            state.list_idx = state.weights.len() - 1;
                        }
                    }
                } else {
                    if !state.cliffs.is_empty() && state.list_idx < state.cliffs.len() {
                        state.cliffs.remove(state.list_idx);
                        if state.list_idx >= state.cliffs.len() && !state.cliffs.is_empty() {
                            state.list_idx = state.cliffs.len() - 1;
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Timeline screen handlers
// ══════════════════════════════════════════════════════════════════════════════

fn handle_timeline_browser(app: &mut App, code: KeyCode) {
    // Handle input mode for creating timeline
    if app.timeline_input_mode {
        match app.timeline_input_action {
            TimelineInputAction::CreateTimelineName => {
                match code {
                    KeyCode::Esc => {
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Enter => {
                        let name = app.input_buffer.trim().to_string();
                        if !name.is_empty() && !app.schema_list.is_empty() {
                            // Next step: select schema
                            app.timeline_input_action = TimelineInputAction::CreateTimelineSchema;
                            app.input_prompt = "Select schema (↑↓ Enter):".into();
                            app.create_timeline_schema_idx = 0;
                        }
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    _ => {}
                }
            }
            TimelineInputAction::CreateTimelineSchema => {
                match code {
                    KeyCode::Esc => {
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.create_timeline_schema_idx > 0 {
                            app.create_timeline_schema_idx -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.create_timeline_schema_idx + 1 < app.schema_list.len() {
                            app.create_timeline_schema_idx += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(s) = app.schema_list.get(app.create_timeline_schema_idx) {
                            let timeline_name = app.input_buffer.trim().to_string();
                            app.create_timeline(&timeline_name, s.id);
                            app.timeline_input_mode = false;
                            app.input_buffer.clear();
                            app.timeline_input_action = TimelineInputAction::None;
                            // Refresh and select the new timeline
                            app.load_timelines();
                            if let Some(idx) = app.timelines.iter().position(|t| t.name == timeline_name) {
                                app.timeline_idx = idx;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        return;
    }

    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(tl) = app.timelines.get(app.timeline_idx) {
                    let ts = TimelineStore::new(&app.store.conn);
                    let _ = ts.delete_timeline(tl.id);
                    app.load_timelines();
                    if app.timeline_idx >= app.timelines.len() && !app.timelines.is_empty() {
                        app.timeline_idx = app.timelines.len() - 1;
                    }
                }
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.timelines.is_empty() {
                app.timeline_idx = app.timeline_idx
                    .checked_sub(1)
                    .unwrap_or(app.timelines.len() - 1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.timelines.is_empty() {
                app.timeline_idx = (app.timeline_idx + 1) % app.timelines.len();
            }
        }
        KeyCode::Enter => {
            if !app.timelines.is_empty() {
                app.open_timeline(app.timeline_idx);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if app.schema_list.is_empty() {
                app.screen = Screen::Error("No schemas exist. Run `cargo run --example seed -p loom-store` first.".into());
            } else {
                app.timeline_input_mode = true;
                app.input_buffer.clear();
                app.input_prompt = "Enter timeline name:".into();
                app.timeline_input_action = TimelineInputAction::CreateTimelineName;
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(tl) = app.timelines.get(app.timeline_idx) {
                app.confirm_delete = Some(tl.name.clone());
            }
        }
        _ => {}
    }
}

fn handle_snapshot_list(app: &mut App, code: KeyCode) {
    // Handle input mode
    if app.timeline_input_mode {
        match app.timeline_input_action {
            TimelineInputAction::AppendSnapshotEntry => {
                match code {
                    KeyCode::Esc => {
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Enter => {
                        let entry = app.input_buffer.trim().to_string();
                        app.append_snapshot_with_events(&entry);
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    _ => {}
                }
            }
            TimelineInputAction::ForkName => {
                match code {
                    KeyCode::Esc => {
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Enter => {
                        let name = app.input_buffer.trim().to_string();
                        app.timeline_input_action = TimelineInputAction::ForkLabel;
                        app.input_buffer.clear();
                        app.input_prompt = "Enter fork label:".into();
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    _ => {}
                }
            }
            TimelineInputAction::ForkLabel => {
                match code {
                    KeyCode::Esc => {
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Enter => {
                        let label = app.input_buffer.trim().to_string();
                        let name = app.timelines.get(app.timeline_idx)
                            .map(|t| format!("{} (fork)", t.name))
                            .unwrap_or_else(|| "fork".into());
                        app.fork_timeline(&name, &label);
                        app.timeline_input_mode = false;
                        app.input_buffer.clear();
                        app.timeline_input_action = TimelineInputAction::None;
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    _ => {}
                }
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::TimelineBrowser;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.snapshots.is_empty() {
                app.snapshot_idx = app.snapshot_idx
                    .checked_sub(1)
                    .unwrap_or(app.snapshots.len() - 1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.snapshots.is_empty() {
                app.snapshot_idx = (app.snapshot_idx + 1) % app.snapshots.len();
            }
        }
        KeyCode::Enter => {
            if !app.snapshots.is_empty() {
                app.screen = Screen::SnapshotDetail;
                app.scroll = ScrollState::default();
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            app.timeline_input_mode = true;
            app.input_buffer.clear();
            app.input_prompt = "Entry text for new snapshot:".into();
            app.timeline_input_action = TimelineInputAction::AppendSnapshotEntry;
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            if !app.snapshots.is_empty() {
                app.timeline_input_mode = true;
                app.input_buffer.clear();
                app.input_prompt = "Enter fork timeline name:".into();
                app.timeline_input_action = TimelineInputAction::ForkName;
            }
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            if !app.forks.is_empty() {
                app.screen = Screen::ForkBrowser;
                app.fork_idx = 0;
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            // Run simulation from current snapshot's state
            if !app.snapshots.is_empty() {
                let snap = &app.snapshots[app.snapshot_idx];
                let values: Vec<f64> = serde_json::from_str(&snap.attributes_json).unwrap_or_default();
                let schema = app.schema.clone();
                app.current_state = DynamicState::from_vec(values, schema);
                app.screen = Screen::List;
            }
        }
        _ => {}
    }
}

fn handle_snapshot_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::SnapshotList;
            app.scroll.offset = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.scroll.offset > 0 {
                app.scroll.offset -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll.offset += 1;
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            // Run simulation from this snapshot
            if let Some(snap) = app.snapshots.get(app.snapshot_idx) {
                let values: Vec<f64> = serde_json::from_str(&snap.attributes_json).unwrap_or_default();
                let schema = app.schema.clone();
                app.current_state = DynamicState::from_vec(values, schema);
                app.screen = Screen::List;
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            // Resolve outcome
            app.timeline_input_mode = true;
            app.input_buffer.clear();
            app.input_prompt = "Enter actual outcome deltas JSON:".into();
            app.timeline_input_action = TimelineInputAction::ResolveOutcome;
        }
        _ => {}
    }
}

fn handle_fork_browser(app: &mut App, code: KeyCode) {
    // Handle input mode if needed (inherit from snapshot list's input handling)
    // For now, just navigation
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            if let Some(id) = app.active_timeline_id {
                app.load_snapshots(id);
            }
            app.screen = Screen::SnapshotList;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.forks.is_empty() {
                app.fork_idx = app.fork_idx
                    .checked_sub(1)
                    .unwrap_or(app.forks.len() - 1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.forks.is_empty() {
                app.fork_idx = (app.fork_idx + 1) % app.forks.len();
            }
        }
        KeyCode::Enter => {
            // Open child timeline
            if let Some(fork) = app.forks.get(app.fork_idx) {
                let child_id = fork.child_timeline_id;
                // Find the child in timelines list
                if let Some(idx) = app.timelines.iter().position(|t| t.id == child_id) {
                    app.timeline_idx = idx;
                    app.open_timeline(idx);
                }
            }
        }
        _ => {}
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Event editor handlers
// ══════════════════════════════════════════════════════════════════════════════

fn handle_edit_events(app: &mut App, code: KeyCode) {
    // Confirmation mode
    if app.confirm_delete.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let idx = app.edit_event_idx;
                app.delete_edit_event(idx);
                app.confirm_delete = None;
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::EditDecisions;
        }
        KeyCode::Up | KeyCode::Char('k') => app.edit_event_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.edit_event_next(),
        KeyCode::Enter => {
            if !app.edit_events.is_empty() {
                app.open_event_detail(app.edit_event_idx);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            let new_event = NamedEvent {
                id: format!("event_{}", app.edit_events.len() + 1),
                label: "New Event".into(),
                description: String::new(),
                preconditions: vec![],
                delay: 0,
                duration: 1,
                cooldown: 0,
                effects: vec![],
                spawns_decision_id: None,
                ..Default::default()
            };
            let _ = app.store.upsert_event(&app.schema_name, &new_event);
            app.edit_events = app.store.list_events(&app.schema_name).unwrap_or_default();
            app.edit_event_idx = app.edit_events.len().saturating_sub(1);
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(e) = app.edit_events.get(app.edit_event_idx) {
                app.confirm_delete = Some(e.label.clone());
            }
        }
        _ => {}
    }
}

fn handle_edit_events_detail(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.save_event_edit();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.edit_event_detail {
                if state.list_idx > 0 {
                    state.list_idx -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.edit_event_detail {
                let max = state.preconditions.len().max(state.effects.len());
                if state.list_idx + 1 < max {
                    state.list_idx += 1;
                }
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(ref mut state) = app.edit_event_detail {
                state.preconditions.push(NamedCondition {
                    attribute: "attr".into(),
                    operator: loom_core::ComparisonOp::Gt,
                    value: 0.0,
                });
                state.list_idx = state.preconditions.len() - 1;
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(ref mut state) = app.edit_event_detail {
                if state.list_idx < state.preconditions.len() {
                    state.preconditions.remove(state.list_idx);
                    if state.list_idx >= state.preconditions.len() && !state.preconditions.is_empty() {
                        state.list_idx = state.preconditions.len() - 1;
                    }
                }
            }
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            if let Some(ref mut state) = app.edit_event_detail {
                state.effects.push(NamedEffect {
                    attribute: Some("attr".into()),
                    group: None,
                    delta: 0.0,
                    scaling: vec![],
                });
                state.list_idx = state.preconditions.len() + state.effects.len() - 1;
            }
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            if let Some(ref mut state) = app.edit_event_detail {
                let eff_idx = state.list_idx.saturating_sub(state.preconditions.len());
                if eff_idx < state.effects.len() {
                    state.effects.remove(eff_idx);
                    if state.list_idx > state.preconditions.len() + state.effects.len() {
                        state.list_idx = state.preconditions.len().saturating_add(state.effects.len()).saturating_sub(1);
                    }
                }
            }
        }
        _ => {}
    }
}

// ── Dashboard handler ────────────────────────────────────────────────────────

fn handle_dashboard(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Tab => {
            app.screen = Screen::List; // Go to existing tabs
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            app.screen = Screen::SchemaList; // Load different schema
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            // Simulate selected decision
            if let Some((idx, _, _, true)) = app.dashboard_decisions.get(app.dashboard_scroll) {
                app.selected_idx = *idx;
                app.run_simulation();
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh_dashboard();
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            app.open_fork_explorer();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.dashboard_scroll > 0 {
                app.dashboard_scroll -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.dashboard_scroll + 1 < app.dashboard_decisions.len() {
                app.dashboard_scroll += 1;
            }
        }
        _ => {}
    }
}

// ── Fork Explorer handler ────────────────────────────────────────────────────

fn handle_fork_explorer(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => std::process::exit(0),
        KeyCode::Esc => {
            app.screen = Screen::Dashboard;
        }
        KeyCode::Enter => {
            app.apply_fork_decision();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.fork_explorer_idx > 0 {
                app.fork_explorer_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.fork_explorer_idx + 1 < app.fork_explorer_results.len() {
                app.fork_explorer_idx += 1;
            }
        }
        _ => {}
    }
}
