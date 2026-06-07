//! Loom TUI — terminal-based decision explorer with state persistence.
//!
//! Loads configuration from SQLite (~/.loom/loom.db), not from JSON files.
//! Run `cargo run --example seed -p loom-store` first to populate the DB.

mod app;
mod ui;

use app::{App, Screen};
use loom_core::DynamicState;
use loom_store::Store;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Load from DB ────────────────────────────────────────────────────────────
    let store = Store::open_default()?;

    let schema = Arc::new(
        store
            .get_schema("personal")?
            .ok_or("No schema 'personal' in DB — run seed first:\n  cargo run --example seed -p loom-store")?,
    );

    let named_decisions = store.list_decisions("personal")?;
    let decisions: Vec<_> = named_decisions
        .iter()
        .map(|nd| nd.resolve(&schema))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to resolve decision: {e}"))?;

    let named_passives = store.list_passives("personal")?;
    let passives: Vec<_> = named_passives
        .iter()
        .map(|np| np.resolve(&schema))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to resolve passive: {e}"))?;

    let named_goal = store
        .get_goal("personal", "default")?
        .ok_or("No goal 'default' in DB — run seed first")?;
    let goal = named_goal
        .resolve(&schema)
        .map_err(|e| format!("failed to resolve goal: {e}"))?;

    // ── Initial state ────────────────────────────────────────────────────────────
    let mut current_state = DynamicState::new(schema.clone());
    current_state.set("wealth.cash", 50000.0);
    current_state.set("wealth.stocks", 25000.0);
    current_state.set("wealth.house_value", 200000.0);
    current_state.set("wealth.debt", 50000.0);
    current_state.set("health.physical", 75.0);
    current_state.set("health.stress", 30.0);
    current_state.set("skills.rust", 70.0);
    current_state.set("skills.python", 45.0);
    current_state.set("skills.negotiation", 55.0);
    current_state.set("social.bob", 60.0);
    current_state.set("social.alice", 85.0);
    current_state.set("time_free", 40.0);

    // ── App ──────────────────────────────────────────────────────────────────────
    let mut app = App::new(store, schema, decisions, passives, goal, current_state);

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
            match &app.screen {
                Screen::List => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
                    KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                    KeyCode::Enter => app.screen = Screen::Detail,
                    KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
                    KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
                    _ => {}
                },
                Screen::Detail => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Esc => {
                        app.screen = Screen::List;
                        app.scroll.offset = 0;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
                    KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
                    _ => {}
                },
                Screen::Results => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Esc => {
                        app.screen = Screen::List;
                        app.scroll.offset = 0;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
                    KeyCode::Char('s') | KeyCode::Char('S') => app.open_state_manager(),
                    _ => {}
                },
                Screen::StateManager => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
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
                            app.delete_state();
                        }
                    }
                    KeyCode::Char('b') | KeyCode::Char('B') => {
                        if !app.input_mode {
                            app.input_mode = true;
                            app.branching = true;
                            app.save_name.clear();
                            app.save_note = format!("branch from {}",
                                app.saved_states.get(app.state_idx)
                                    .map_or("?", |s| s.name.as_str()));
                        }
                    }
                    KeyCode::Char(c) => app.input_char(c),
                    KeyCode::Backspace => app.input_backspace(),
                    _ => {}
                },
                Screen::Error(_) => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    _ => {}
                },
            }
        }
    }
}
