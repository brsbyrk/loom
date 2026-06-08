//! Rendering functions for the Loom TUI.

use crate::app::{EditField, Screen};
use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

// ── Theme ────────────────────────────────────────────────────────────────────────────

const HEADER_BG: Color = Color::Rgb(30, 30, 40);
const ACCENT: Color = Color::Rgb(100, 200, 255);
const GOOD: Color = Color::Rgb(80, 220, 120);
const BAD: Color = Color::Rgb(255, 80, 80);
const DIM: Color = Color::Rgb(120, 120, 140);
const YELLOW: Color = Color::Rgb(255, 220, 80);

// ── Entry point ──────────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::SchemaList => render_schema_list(f, app),
        Screen::List => render_list(f, app),
        Screen::Detail => render_detail(f, app),
        Screen::Results => render_results(f, app),
        Screen::StateManager => render_state_manager(f, app),
        Screen::Error(msg) => render_error(f, msg),
        Screen::EditDecisions => render_edit_decisions(f, app),
        Screen::EditDecisionDetail => render_edit_decision_detail(f, app),
        Screen::EditPassives => render_edit_passives(f, app),
        Screen::EditPassiveDetail => render_edit_passive_detail(f, app),
        Screen::EditGoals => render_edit_goals(f, app),
        Screen::EditGoalDetail => render_edit_goal_detail(f, app),
    }
}

// ── Schema list screen ───────────────────────────────────────────────────────────────

fn render_schema_list(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " Select a schema (template):",
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    if app.schema_list.is_empty() {
        lines.push(Line::from(Span::styled(
            " No schemas found.",
            Style::default().fg(BAD),
        )));
        lines.push(Line::from(Span::styled(
            " Run `cargo run --example seed -p loom-store` or press N to create one.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, s) in app.schema_list.iter().enumerate() {
            let cursor = if i == app.schema_idx { " > " } else { "   " };
            let style = if i == app.schema_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(
                    format!("{:<20}", s.name),
                    style,
                ),
                Span::styled(
                    format!("({} attributes)", s.attribute_count),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Loom ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(block, chunks[0]);

    let footer = if app.schema_list.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::styled(" N ", Style::default().fg(ACCENT).bold()),
            Span::raw("create new  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
            Span::raw("navigate  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("select  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
    }
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── List screen ──────────────────────────────────────────────────────────────────────

fn render_list(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    // Tabs row — show schema name
    let tabs = vec![
        format!(" Decisions ({}) ", app.decisions.len()),
        format!(" Schema: {} ", app.schema_name),
    ];
    let tab_labels: Vec<Line> = tabs.iter().map(|t| Line::from(t.as_str())).collect();
    let tab_bar = ratatui::widgets::Tabs::new(tab_labels)
        .select(0)
        .style(Style::default().fg(ACCENT))
        .divider("  ");
    f.render_widget(tab_bar, chunks[0]);

    // Decision list
    let items: Vec<ListItem> = app
        .decisions
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let preconds: String = d
                .preconditions
                .iter()
                .map(|c| {
                    let name = app
                        .schema
                        .at(c.attribute_index)
                        .map_or(format!("[{}]", c.attribute_index), |a| a.name.clone());
                    format!("{name} {:?} {}", c.operator, c.value)
                })
                .collect::<Vec<_>>()
                .join(", ");

            let line = if i == app.selected_idx {
                Line::from(vec![
                    Span::styled(format!(" > {} ", d.label), Style::default().fg(ACCENT).bold()),
                    Span::styled(format!("({preconds})"), Style::default().fg(DIM)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!("   {} ", d.label), Style::default().fg(Color::White)),
                    Span::styled(format!("({preconds})"), Style::default().fg(DIM)),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Decisions ({}) ", app.decisions.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("");

    f.render_widget(list, chunks[1]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("details  "),
        Span::styled(" R ", Style::default().fg(ACCENT).bold()),
        Span::raw("run sim  "),
        Span::styled(" S ", Style::default().fg(ACCENT).bold()),
        Span::raw("states  "),
        Span::styled(" E ", Style::default().fg(ACCENT).bold()),
        Span::raw("edit  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[2]);
}

// ── Detail screen ────────────────────────────────────────────────────────────────────

fn render_detail(f: &mut Frame, app: &App) {
    let decision = match app.selected_decision() {
        Some(d) => d,
        None => {
            render_error(f, "No decision selected.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", decision.label),
            Style::default().fg(ACCENT).bold(),
        ),
    ]));
    lines.push(Line::raw(""));

    // Preconditions
    if !decision.preconditions.is_empty() {
        lines.push(Line::from(Span::styled(
            " Preconditions:",
            Style::default().fg(DIM),
        )));
        for cond in &decision.preconditions {
            let name = app
                .schema
                .at(cond.attribute_index)
                .map_or(format!("attr[{}]", cond.attribute_index), |a| a.name.clone());
            let status = if cond.check(&app.current_state) {
                Span::styled(" ✓", Style::default().fg(GOOD))
            } else {
                Span::styled(" ✗", Style::default().fg(BAD))
            };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("{name} {:?} {}", cond.operator, cond.value),
                    Style::default().fg(Color::White),
                ),
                status,
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Cost
    if !decision.cost.is_empty() {
        lines.push(Line::from(Span::styled(
            " Cost:",
            Style::default().fg(DIM),
        )));
        for effect in &decision.cost {
            let name = app
                .schema
                .at(effect.attribute_index)
                .map_or(format!("[{}]", effect.attribute_index), |a| {
                    a.name.clone()
                });
            let sign = if effect.delta >= 0.0 { "+" } else { "" };
            lines.push(Line::from(Span::raw(format!(
                "   {name} {sign}{:.1}",
                effect.delta
            ))));
        }
        lines.push(Line::raw(""));
    }

    // Outcomes
    lines.push(Line::from(Span::styled(
        format!(" Outcomes ({} branches):", decision.outcomes.len()),
        Style::default().fg(DIM),
    )));
    let total_weight: f64 = decision.outcomes.iter().map(|o| o.weight).sum();
    for outcome in &decision.outcomes {
        let pct = if total_weight > 0.0 {
            outcome.weight / total_weight * 100.0
        } else {
            0.0
        };
        let label = if outcome.label.is_empty() {
            "Unnamed".to_string()
        } else {
            outcome.label.clone()
        };
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(format!("{pct:.0}% "), Style::default().fg(ACCENT).bold()),
            Span::styled(label, Style::default().fg(Color::White)),
        ]));

        // Show effect summary
        if let loom_core::Transform::Declarative { effects, .. } = &outcome.transform {
            for effect in effects {
                let name = app
                    .schema
                    .at(effect.attribute_index)
                    .map_or(format!("[{}]", effect.attribute_index), |a| a.name.clone());
                let sign = if effect.delta >= 0.0 { "+" } else { "" };
                lines.push(Line::from(Span::raw(format!(
                    "        {name} {sign}{:.1}",
                    effect.delta
                ))));
            }
        }
    }

    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Decision: {} ", decision.label))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(detail, chunks[0]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" R ", Style::default().fg(ACCENT).bold()),
        Span::raw("run simulation  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Results screen ───────────────────────────────────────────────────────────────────

fn render_results(f: &mut Frame, app: &App) {
    let analysis = match &app.last_result {
        Some(a) => a,
        None => {
            render_error(f, "No simulation results.");
            return;
        }
    };
    let decision = match &app.last_decision {
        Some(d) => d,
        None => {
            render_error(f, "No simulation results.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(" Results: ", Style::default().fg(DIM)),
        Span::styled(&decision.label, Style::default().fg(ACCENT).bold()),
        Span::styled(
            format!(
                "  ({} runs, {} steps)",
                app.sim_config.runs, app.sim_config.horizon
            ),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::raw(""));

    if !analysis.decision_available {
        lines.push(Line::from(Span::styled(
            " DECISION UNAVAILABLE — preconditions not met.",
            Style::default().fg(BAD).bold(),
        )));
    } else {
        // Outcome probabilities
        lines.push(Line::from(Span::styled(
            " Outcome probabilities:",
            Style::default().fg(DIM),
        )));

        for (idx, prob) in &analysis.outcome_probabilities[0] {
            let label = decision
                .outcomes
                .get(*idx)
                .map_or("unknown", |o| o.label.as_str());
            let bar_len = (*prob * 40.0) as usize;
            let bar: String = "█".repeat(bar_len) + &"░".repeat(40usize.saturating_sub(bar_len));
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(bar, Style::default().fg(ACCENT)),
                Span::styled(format!("  {:.1}% ", prob * 100.0), Style::default().fg(Color::White)),
                Span::styled(label, Style::default().fg(DIM)),
            ]));
        }
        lines.push(Line::raw(""));

        // Utility summary
        let util = &analysis.utility_distribution;
        lines.push(Line::from(vec![
            Span::styled(" Utility: ", Style::default().fg(DIM)),
            Span::styled(format!("Mean {:.0}  ", util.mean), Style::default().fg(GOOD)),
            Span::styled(format!("Std {:.1}  ", util.std), Style::default().fg(Color::White)),
            Span::styled(format!("P50 {:.0}  ", util.p50), Style::default().fg(Color::White)),
            Span::styled(format!("P5 {:.0}–P95 {:.0}", util.p5, util.p95), Style::default().fg(DIM)),
        ]));
        lines.push(Line::raw(""));

        // Attribute outcomes — only show changed attributes
        lines.push(Line::from(Span::styled(
            " Attribute outcomes (changed only):",
            Style::default().fg(DIM),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "   {:<22} {:>8} {:>8} {:>8} {:>8}",
                "Attribute", "Mean", "P5", "P50", "P95"
            ),
            Style::default().fg(DIM),
        )));

        for (i, attr) in app.schema.attributes.iter().enumerate() {
            let dist = &analysis.attribute_outcomes[i];
            let initial = app.current_state.get(&attr.name).unwrap_or(0.0);
            // Only show attributes that changed by >0.5
            if (dist.mean - initial).abs() < 0.5
                && (dist.p5 - dist.p95).abs() < 0.5
            {
                continue;
            }
            let delta = dist.mean - initial;
            let delta_str = if delta >= 0.0 {
                Span::styled(format!("+{delta:.1}"), Style::default().fg(GOOD))
            } else {
                Span::styled(format!("{delta:.1}"), Style::default().fg(BAD))
            };
            lines.push(Line::from(vec![
                Span::raw(format!("   {:<22} ", attr.name)),
                Span::styled(
                    format!("{:>8.1} ", dist.mean),
                    Style::default().fg(Color::White),
                ),
                delta_str,
                Span::raw(format!("  | P5 {:>6.1}  P95 {:>6.1}", dist.p5, dist.p95)),
            ]));
        }
        lines.push(Line::raw(""));

        // Utility trend
        if !analysis.utility_over_time.is_empty() {
            lines.push(Line::from(Span::styled(
                " Utility trend:",
                Style::default().fg(DIM),
            )));
            let first = &analysis.utility_over_time[0];
            let last = analysis.utility_over_time.last().unwrap();
            let delta = last.mean - first.mean;
            let trend = if delta > 0.0 {
                Span::styled(" ↗", Style::default().fg(GOOD))
            } else {
                Span::styled(" ↘", Style::default().fg(BAD))
            };
            lines.push(Line::from(vec![
                Span::raw(format!(
                    "   Step 0: {:.0}  →  Step {}: {:.0}  Δ={delta:+.0}",
                    first.mean,
                    last.step,
                    last.mean,
                )),
                trend,
            ]));
        }
    }

    let results = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Line::from(format!(" Simulation Results ")))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(results, chunks[0]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" R ", Style::default().fg(ACCENT).bold()),
        Span::raw("re-run  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── State manager screen ─────────────────────────────────────────────────────────────

fn render_state_manager(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(Span::styled(
        " State Manager — saved states ",
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    // Current state summary
    let active_attrs: Vec<String> = app
        .schema
        .attributes
        .iter()
        .filter_map(|a| {
            let val = app.current_state.get(&a.name).unwrap_or(0.0);
            if val != 0.0 {
                Some(format!("{}= {val:.0}", a.name))
            } else {
                None
            }
        })
        .collect();
    lines.push(Line::from(Span::styled(
        format!(" Active state: {}", active_attrs.join(", ")),
        Style::default().fg(DIM),
    )));
    lines.push(Line::raw(""));

    // Confirmation prompt
    if let Some(msg) = &app.confirm_delete {
        lines.push(Line::from(Span::styled(
            format!(" Delete \"{msg}\"? (y/n) "),
            Style::default().fg(BAD).bold(),
        )));
        lines.push(Line::raw(""));
        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(Line::from(format!(" State Manager ")))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" y/n ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm/deny  "),
        ]))
        .style(Style::default().bg(HEADER_BG));
        f.render_widget(footer, chunks[1]);
        return;
    }

    // Saved states list
    if app.saved_states.is_empty() {
        lines.push(Line::from(Span::styled(
            " No saved states. Press N to save current state.",
            Style::default().fg(DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(" {} saved states:", app.saved_states.len()),
            Style::default().fg(DIM),
        )));
        for (i, state) in app.saved_states.iter().enumerate() {
            let cursor = if i == app.state_idx { " > " } else { "   " };
            let style = if i == app.state_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            let preview: String = state
                .values
                .iter()
                .take(4)
                .map(|v| format!("{v:.0}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(
                    format!("{:<20}", state.name),
                    style,
                ),
                Span::styled(
                    format!("[{preview}]"),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }
    lines.push(Line::raw(""));

    // Input mode
    if app.input_mode {
        lines.push(Line::from(Span::styled(
            " Saving new state...",
            Style::default().fg(ACCENT),
        )));
        lines.push(Line::from(vec![
            Span::raw(" Name: "),
            Span::styled(
                format!("{}▌", app.save_name),
                Style::default().fg(Color::White).bold(),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            " Enter to confirm, Esc to cancel",
            Style::default().fg(DIM),
        )));
    }

    let output = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Line::from(format!(" State Manager ")))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(output, chunks[0]);

    // Footer
    let footer = if app.input_mode {
        Paragraph::new(Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
            Span::raw("cancel  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("save  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
            Span::raw("select  "),
            Span::styled(" Enter/L ", Style::default().fg(ACCENT).bold()),
            Span::raw("load  "),
            Span::styled(" N ", Style::default().fg(ACCENT).bold()),
            Span::raw("save  "),
            Span::styled(" B ", Style::default().fg(ACCENT).bold()),
            Span::raw("branch  "),
            Span::styled(" D ", Style::default().fg(ACCENT).bold()),
            Span::raw("delete  "),
            Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
            Span::raw("back  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
    }
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Decisions list screen ───────────────────────────────────────────────────────

fn render_edit_decisions(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        " Edit Decisions:",
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    // Confirmation prompt
    if let Some(ref msg) = app.confirm_delete {
        lines.push(Line::from(Span::styled(
            format!(" Delete \"{msg}\"? (y/n)"),
            Style::default().fg(BAD).bold(),
        )));
        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Edit Decisions ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" y/n ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm/deny  "),
        ]))
        .style(Style::default().bg(HEADER_BG));
        f.render_widget(footer, chunks[1]);
        return;
    }

    if app.edit_decisions.is_empty() {
        lines.push(Line::from(Span::styled(
            " No decisions. Press N to add one.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, d) in app.edit_decisions.iter().enumerate() {
            let cursor = if i == app.edit_decision_idx { " > " } else { "   " };
            let style = if i == app.edit_decision_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            let precond_count = d.preconditions.len();
            let outcome_count = d.outcomes.len();
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!("{:<30}", d.label), style),
                Span::styled(
                    format!("({precond_count} pre, {outcome_count} outcomes)"),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Decisions ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("edit  "),
        Span::styled(" D ", Style::default().fg(ACCENT).bold()),
        Span::raw("delete  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Decision Detail screen ──────────────────────────────────────────────────────

fn render_edit_decision_detail(f: &mut Frame, app: &App) {
    let state = match &app.edit_decision_detail {
        Some(s) => s,
        None => {
            render_error(f, "No decision edit state.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" Editing Decision [{}]: ", state.idx),
        Style::default().fg(ACCENT).bold(),
    )));

    // ── Label ───────────────────────────────────────────────────────────────────────
    let is_label = state.active_field == EditField::Label;
    let label_style = if is_label {
        Style::default().fg(YELLOW).bold()
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(" Label: ", Style::default().fg(DIM)),
        Span::styled(format!("{} ", state.label), label_style),
        if is_label {
            Span::styled("▌", Style::default().fg(YELLOW))
        } else {
            Span::raw("")
        },
    ]));

    // ── Preconditions ───────────────────────────────────────────────────────────────
    let is_pre = state.active_field == EditField::Preconditions;
    let pre_header = if is_pre { "─ Preconditions (editing) ─" } else { " Preconditions:" };
    lines.push(Line::from(Span::styled(pre_header, Style::default().fg(DIM))));
    for (j, c) in state.preconditions.iter().enumerate() {
        let sel = if is_pre && j == state.list_idx { " > " } else { "   " };
        let s = if is_pre && j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("{} {:?} {}", c.attribute, c.operator, c.value), s),
        ]));
    }
    if is_pre {
        lines.push(Line::from(Span::styled(
            "   [a] add condition  [d] delete selected",
            Style::default().fg(DIM),
        )));
    }

    // ── Cost effects ────────────────────────────────────────────────────────────────
    let is_cost = state.active_field == EditField::CostEffects;
    let cost_header = if is_cost { "─ Cost effects (editing) ─" } else { " Cost:" };
    lines.push(Line::from(Span::styled(cost_header, Style::default().fg(DIM))));
    for (j, e) in state.cost.iter().enumerate() {
        let sel = if is_cost && j == state.list_idx { " > " } else { "   " };
        let s = if is_cost && j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        let sign = if e.delta >= 0.0 { "+" } else { "" };
        let aname = e.attribute.as_deref().unwrap_or("?");
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("{aname} {sign}{}", e.delta), s),
        ]));
    }
    if is_cost {
        lines.push(Line::from(Span::styled(
            "   [a] add effect  [d] delete selected",
            Style::default().fg(DIM),
        )));
    }

    // ── Outcomes ────────────────────────────────────────────────────────────────────
    let is_out = state.active_field == EditField::Outcomes;
    let out_header = if is_out { "─ Outcomes (editing) ─" } else { " Outcomes:" };
    lines.push(Line::from(Span::styled(out_header, Style::default().fg(DIM))));
    for (j, o) in state.outcomes.iter().enumerate() {
        let sel = if is_out && j == state.list_idx { " > " } else { "   " };
        let s = if is_out && j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("\"{}\" w={}", o.label, o.weight), s),
        ]));
        // Show effects for the outcome
        if let loom_core::NamedTransform::Declarative { effects, .. } = &o.transform {
            for (k, eff) in effects.iter().enumerate() {
                let prefix = if j == state.list_idx && is_out && k == state.sub_list_idx {
                    "     > "
                } else {
                    "       "
                };
                let sign = if eff.delta >= 0.0 { "+" } else { "" };
                let aname = eff.attribute.as_deref().unwrap_or("?");
                lines.push(Line::from(Span::raw(format!(
                    "{prefix}{aname} {sign}{}",
                    eff.delta
                ))));
            }
        }
    }
    if is_out {
        lines.push(Line::from(Span::styled(
            "   [a] add outcome  [d] delete selected  [e] edit sub-effects",
            Style::default().fg(DIM),
        )));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        " [Tab] switch field  [Enter] edit text  [Esc] save & back",
        Style::default().fg(DIM),
    )));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Decision Detail ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Tab ", Style::default().fg(ACCENT).bold()),
        Span::raw("field  "),
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("list item  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("save & back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Passives list screen ────────────────────────────────────────────────────────

fn render_edit_passives(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " Edit Passives:",
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    // Confirmation prompt
    if let Some(ref msg) = app.confirm_delete {
        lines.push(Line::from(Span::styled(
            format!(" Delete \"{msg}\"? (y/n)"),
            Style::default().fg(BAD).bold(),
        )));
        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Edit Passives ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" y/n ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm/deny  "),
        ]))
        .style(Style::default().bg(HEADER_BG));
        f.render_widget(footer, chunks[1]);
        return;
    }

    if app.edit_passives.is_empty() {
        lines.push(Line::from(Span::styled(
            " No passives. Press N to add one.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, p) in app.edit_passives.iter().enumerate() {
            let cursor = if i == app.edit_passive_idx { " > " } else { "   " };
            let style = if i == app.edit_passive_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            let effect_count = p.effects.len();
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!("{:<30}", p.label), style),
                Span::styled(
                    format!("({effect_count} effects)"),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Passives ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("edit  "),
        Span::styled(" D ", Style::default().fg(ACCENT).bold()),
        Span::raw("delete  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Passive Detail screen ──────────────────────────────────────────────────────

fn render_edit_passive_detail(f: &mut Frame, app: &App) {
    let state = match &app.edit_passive_detail {
        Some(s) => s,
        None => {
            render_error(f, "No passive edit state.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" Editing Passive [{}]: ", state.idx),
        Style::default().fg(ACCENT).bold(),
    )));

    lines.push(Line::from(vec![
        Span::styled(" ID: ", Style::default().fg(DIM)),
        Span::styled(&state.passive_id, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Label: ", Style::default().fg(DIM)),
        Span::styled(&state.label, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        " Effects:",
        Style::default().fg(DIM),
    )));

    for (j, e) in state.effects.iter().enumerate() {
        let sel = if j == state.list_idx { " > " } else { "   " };
        let s = if j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        let sign = if e.delta >= 0.0 { "+" } else { "" };
        let aname = e.attribute.as_deref().unwrap_or("?");
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("{aname} {sign}{}", e.delta), s),
        ]));
    }

    lines.push(Line::styled(
        " [a] add effect  [d] delete selected  [e] edit label  [Esc] save & back",
        Style::default().fg(DIM),
    ));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Passive Detail ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("select effect  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("save & back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Goals list screen ───────────────────────────────────────────────────────────

fn render_edit_goals(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " Edit Goals:",
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    // Confirmation prompt
    if let Some(ref msg) = app.confirm_delete {
        lines.push(Line::from(Span::styled(
            format!(" Delete \"{msg}\"? (y/n)"),
            Style::default().fg(BAD).bold(),
        )));
        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Edit Goals ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(block, chunks[0]);
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" y/n ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm/deny  "),
        ]))
        .style(Style::default().bg(HEADER_BG));
        f.render_widget(footer, chunks[1]);
        return;
    }

    if app.edit_goals.is_empty() {
        lines.push(Line::from(Span::styled(
            " No goals. Press N to add one.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, (name, g)) in app.edit_goals.iter().enumerate() {
            let cursor = if i == app.edit_goal_idx { " > " } else { "   " };
            let style = if i == app.edit_goal_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            let w_count = g.weights.len();
            let c_count = g.cliffs.len();
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!("{:<20}", name), style),
                Span::styled(
                    format!("({w_count} weights, {c_count} cliffs)"),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Goals ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("edit  "),
        Span::styled(" D ", Style::default().fg(ACCENT).bold()),
        Span::raw("delete  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Edit Goal Detail screen ─────────────────────────────────────────────────────────

fn render_edit_goal_detail(f: &mut Frame, app: &App) {
    let state = match &app.edit_goal_detail {
        Some(s) => s,
        None => {
            render_error(f, "No goal edit state.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" Editing Goal \"{}\": ", state.goal_name),
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    // Toggle between weights and cliffs
    let mode_str = if state.show_weights { "Weights" } else { "Cliffs" };
    lines.push(Line::from(Span::styled(
        format!(" [{mode_str}]  Tab to switch"),
        Style::default().fg(DIM),
    )));
    lines.push(Line::raw(""));

    if state.show_weights {
        for (j, (name, w)) in state.weights.iter().enumerate() {
            let sel = if j == state.list_idx { " > " } else { "   " };
            let s = if j == state.list_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(sel, s),
                Span::styled(format!("{name:<20} = {w:.2}"), s),
            ]));
        }
        lines.push(Line::from(Span::styled(
            " [a] add weight  [d] delete selected  [e] edit value",
            Style::default().fg(DIM),
        )));
    } else {
        for (j, (name, t)) in state.cliffs.iter().enumerate() {
            let sel = if j == state.list_idx { " > " } else { "   " };
            let s = if j == state.list_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![
                Span::styled(sel, s),
                Span::styled(format!("{name:<20} min={} pen={}", t.min, t.penalty), s),
            ]));
        }
        lines.push(Line::from(Span::styled(
            " [a] add cliff  [d] delete selected  [e] edit values",
            Style::default().fg(DIM),
        )));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        " [Esc] save & back",
        Style::default().fg(DIM),
    )));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Goal Detail ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("item  "),
        Span::styled(" Tab ", Style::default().fg(ACCENT).bold()),
        Span::raw("weights/cliffs  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("save & back  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Error screen ─────────────────────────────────────────────────────────────────────

fn render_error(f: &mut Frame, msg: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let text = Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(BAD))))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(BAD)));

    f.render_widget(text, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}
