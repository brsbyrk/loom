//! Loom TUI — terminal-based decision explorer.
//!
//! Loads decision configurations and lets you browse, inspect, and simulate decisions
//! interactively. Requires config files in `../../examples/configs/` relative to the
//! crate directory (i.e., in the workspace `examples/configs/`).
//!
//! # Controls
//!
//! - `↑`/`↓` — navigate decisions
//! - `Enter` — view decision details
//! - `R` — run Monte Carlo simulation
//! - `Esc` — go back
//! - `Q` — quit

mod app;
mod ui;

use app::{App, Screen};
use loom_core::{AttributeSchema, Decision, GoalVector, PassiveEffect};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Config loading ───────────────────────────────────────────────────────────
    // CARGO_MANIFEST_DIR is crates/loom-tui/; configs are at workspace_root/examples/configs/
    let config_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/configs");

    let schema = Arc::new(AttributeSchema::from_path(&format!(
        "{config_dir}/attribute_schema.json"
    ))?);
    let decision: Decision = serde_json::from_str(&std::fs::read_to_string(format!(
        "{config_dir}/job_decision.json"
    ))?)?;
    let decisions = vec![decision];
    let passives: Vec<PassiveEffect> = serde_json::from_str(&std::fs::read_to_string(format!(
        "{config_dir}/passives.json"
    ))?)?;
    let goal: GoalVector =
        serde_json::from_str(&std::fs::read_to_string(format!("{config_dir}/goal.json"))?)?;

    // ── Initial state ────────────────────────────────────────────────────────────
    let mut initial_state = loom_core::DynamicState::new(schema.clone());
    initial_state.set("wealth.cash", 50000.0);
    initial_state.set("wealth.stocks", 25000.0);
    initial_state.set("wealth.house_value", 200000.0);
    initial_state.set("wealth.debt", 50000.0);
    initial_state.set("health.physical", 75.0);
    initial_state.set("health.stress", 30.0);
    initial_state.set("skills.rust", 70.0);
    initial_state.set("skills.python", 45.0);
    initial_state.set("skills.negotiation", 55.0);
    initial_state.set("social.bob", 60.0);
    initial_state.set("social.alice", 85.0);
    initial_state.set("time_free", 40.0);

    // ── App ──────────────────────────────────────────────────────────────────────
    let mut app = App {
        schema,
        decisions,
        passives,
        goal,
        initial_state,
        ..App::empty()
    };

    // ── Terminal setup ───────────────────────────────────────────────────────────
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
                    _ => {}
                },
                Screen::Detail => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Esc => {
                        app.screen = Screen::List;
                        app.scroll.offset = 0;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
                    _ => {}
                },
                Screen::Results => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Esc => {
                        app.screen = Screen::List;
                        app.scroll.offset = 0;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => app.run_simulation(),
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
