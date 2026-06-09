//! Rendering functions for the Loom TUI.

use crate::app::{App, EditField, Screen};
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
    // Draw top tab bar for all screens except SchemaList
    draw_tab_bar(f, app);

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
        Screen::TimelineBrowser => render_timeline_browser(f, app),
        Screen::SnapshotList => render_snapshot_list(f, app),
        Screen::SnapshotDetail => render_snapshot_detail(f, app),
        Screen::ForkBrowser => render_fork_browser(f, app),
        Screen::EditEvents => render_edit_events(f, app),
        Screen::EditEventsDetail => render_edit_events_detail(f, app),
        Screen::Dashboard => render_dashboard(f, app),
        Screen::ForkExplorer => render_fork_explorer(f, app),
    }
}

// ── Top tab bar ──────────────────────────────────────────────────────────────────────

fn draw_tab_bar(f: &mut Frame, app: &App) {
    let area = f.area();
    // Reserve 1 line at top for tab bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    f.render_widget(ratatui::widgets::Clear, chunks[0]);

    let tab_names = ["Timeline", "Explore", "Config"];
    let spans: Vec<Span> = tab_names.iter().enumerate().map(|(i, name)| {
        let is_active = i == app.tab;
        let tab_style = if is_active {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(DIM)
        };
        let label = if is_active {
            format!(" [{name}] ")
        } else {
            format!("  {name}  ")
        };
        Span::styled(label, tab_style)
    }).collect();

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(HEADER_BG));
    f.render_widget(bar, chunks[0]);

    // Adjust f.area() for subsequent rendering
    let _inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    // We don't consume the area; each render function will layout its own area.
    // We just use the full area minus the top line by using a modified approach.
    // Actually, we'll let each render function use the full area and they'll handle
    // their own layout. We'll leave chunks[0] drawn behind and continue.
    _ = f.area(); // reference to ensure f is used
}

// ── Schema list screen ───────────────────────────────────────────────────────────────

fn render_schema_list(f: &mut Frame, app: &App) {
    // Account for tab bar: shift content down by 1
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

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

    f.render_widget(block, chunks[1]);

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
    f.render_widget(footer, chunks[2]);
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

// ══════════════════════════════════════════════════════════════════════════════
// Timeline screen renderers
// ══════════════════════════════════════════════════════════════════════════════

// ── Timeline Browser ────────────────────────────────────────────────────────────────

fn render_timeline_browser(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    // Header
    if app.timeline_input_mode {
        match app.timeline_input_action {
            crate::app::TimelineInputAction::CreateTimelineName => {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", app.input_prompt),
                    Style::default().fg(ACCENT).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    format!(" > {}▌", app.input_buffer),
                    Style::default().fg(Color::White).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    " Enter to confirm, Esc to cancel",
                    Style::default().fg(DIM),
                )));
            }
            crate::app::TimelineInputAction::CreateTimelineSchema => {
                lines.push(Line::from(Span::styled(
                    format!(" Create timeline \"{}\" — select schema:", app.input_buffer),
                    Style::default().fg(ACCENT).bold(),
                )));
                lines.push(Line::raw(""));
                for (i, s) in app.schema_list.iter().enumerate() {
                    let cursor = if i == app.create_timeline_schema_idx { " > " } else { "   " };
                    let style = if i == app.create_timeline_schema_idx {
                        Style::default().fg(ACCENT).bold()
                    } else {
                        Style::default().fg(Color::White)
                    };
                    lines.push(Line::from(vec![
                        Span::styled(cursor, style),
                        Span::styled(format!("{:<20}", s.name), style),
                        Span::styled(
                            format!("({} attributes)", s.attribute_count),
                            Style::default().fg(DIM),
                        ),
                    ]));
                }
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(
                    " ↑↓ navigate, Enter confirm, Esc cancel",
                    Style::default().fg(DIM),
                )));
            }
            _ => {}
        }
    } else if app.confirm_delete.is_some() {
        lines.push(Line::from(Span::styled(
            format!(" Delete timeline \"{}\"? (y/n)", app.confirm_delete.as_ref().unwrap()),
            Style::default().fg(BAD).bold(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " Timeline Browser ",
            Style::default().fg(ACCENT).bold(),
        )));
        lines.push(Line::raw(""));

        if app.timelines.is_empty() {
            lines.push(Line::from(Span::styled(
                " No timelines yet.",
                Style::default().fg(DIM),
            )));
            lines.push(Line::from(Span::styled(
                " Press N to create a new timeline.",
                Style::default().fg(DIM),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!(" {} timelines:", app.timelines.len()),
                Style::default().fg(DIM),
            )));
            for (i, tl) in app.timelines.iter().enumerate() {
                let cursor = if i == app.timeline_idx { " > " } else { "   " };
                let style = if i == app.timeline_idx {
                    Style::default().fg(ACCENT).bold()
                } else {
                    Style::default().fg(Color::White)
                };
                // Resolve schema name
                let schema_name = app.schema_list
                    .iter()
                    .find(|s| s.id == tl.schema_id)
                    .map(|s| s.name.as_str())
                    .unwrap_or("?");
                lines.push(Line::from(vec![
                    Span::styled(cursor, style),
                    Span::styled(format!("{:<24}", tl.name), style),
                    Span::styled(
                        format!("(schema: {:<12}, {} snapshots) ", schema_name, tl.snapshot_count),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(
                        &tl.created_at,
                        Style::default().fg(DIM),
                    ),
                ]));
            }
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Timeline Browser ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(block, chunks[0]);

    let footer = if app.timeline_input_mode || app.confirm_delete.is_some() {
        Paragraph::new(Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
            Span::raw("cancel  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm"),
        ]))
        .style(Style::default().bg(HEADER_BG))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
            Span::raw("navigate  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("open  "),
            Span::styled(" N ", Style::default().fg(ACCENT).bold()),
            Span::raw("new  "),
            Span::styled(" D ", Style::default().fg(ACCENT).bold()),
            Span::raw("delete  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
        .style(Style::default().bg(HEADER_BG))
    };
    f.render_widget(footer, chunks[1]);
}

// ── Snapshot List ───────────────────────────────────────────────────────────────────

fn render_snapshot_list(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    // Header
    if app.timeline_input_mode {
        match app.timeline_input_action {
            crate::app::TimelineInputAction::AppendSnapshotEntry => {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", app.input_prompt),
                    Style::default().fg(ACCENT).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    format!(" > {}▌", app.input_buffer),
                    Style::default().fg(Color::White).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    " Enter to confirm, Esc to cancel",
                    Style::default().fg(DIM),
                )));
            }
            crate::app::TimelineInputAction::ForkName => {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", app.input_prompt),
                    Style::default().fg(ACCENT).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    format!(" > {}▌", app.input_buffer),
                    Style::default().fg(Color::White).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    " Enter to continue, Esc to cancel",
                    Style::default().fg(DIM),
                )));
            }
            crate::app::TimelineInputAction::ForkLabel => {
                lines.push(Line::from(Span::styled(
                    format!(" {} ", app.input_prompt),
                    Style::default().fg(ACCENT).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    format!(" > {}▌", app.input_buffer),
                    Style::default().fg(Color::White).bold(),
                )));
                lines.push(Line::from(Span::styled(
                    " Enter to fork, Esc to cancel",
                    Style::default().fg(DIM),
                )));
            }
            _ => {}
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" Timeline: {} ", app.active_timeline_name),
                Style::default().fg(ACCENT).bold(),
            ),
            Span::styled(
                format!(" ({})", app.active_timeline_schema_name),
                Style::default().fg(DIM),
            ),
        ]));
        lines.push(Line::raw(""));

        if app.snapshots.is_empty() {
            lines.push(Line::from(Span::styled(
                " No snapshots yet.",
                Style::default().fg(DIM),
            )));
            lines.push(Line::from(Span::styled(
                " Press A to append a snapshot using the current state.",
                Style::default().fg(DIM),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!(" {} snapshots:", app.snapshots.len()),
                Style::default().fg(DIM),
            )));
            lines.push(Line::raw(""));

            for (i, snap) in app.snapshots.iter().enumerate() {
                let is_head = i == app.snapshots.len() - 1;
                let cursor = if i == app.snapshot_idx { " > " } else { "   " };
                let style = if i == app.snapshot_idx {
                    Style::default().fg(ACCENT).bold()
                } else {
                    Style::default().fg(Color::White)
                };

                // Show first few attribute values
                let values: Vec<f64> = serde_json::from_str(&snap.attributes_json).unwrap_or_default();
                let preview: String = values.iter().take(3).map(|v| format!("{v:.0}")).collect::<Vec<_>>().join(", ");

                let head_marker = if is_head { " ◄ HEAD" } else { "" };
                let forecast_badge = if snap.forecast_json.is_some() { " [forecast ✓]" } else { "" };
                let resolved_badge = if snap.actual_outcome_json.is_some() { " [resolved]" } else { "" };

                let entry_preview = if snap.entry_text.len() > 30 {
                    format!("{}…", &snap.entry_text[..30])
                } else {
                    snap.entry_text.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(cursor, style),
                    Span::styled(
                        format!("Step {:>3}: {}", snap.step, entry_preview),
                        style,
                    ),
                    Span::styled(
                        format!("[{preview}]{forecast_badge}{resolved_badge}{head_marker}"),
                        Style::default().fg(DIM),
                    ),
                ]));
            }
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            " ↑↓ navigate  Enter detail  A append  R sim from here  F fork  B forks  Esc back",
            Style::default().fg(DIM),
        )));
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Snapshots — {} ", app.active_timeline_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(block, chunks[0]);

    let footer = if app.timeline_input_mode {
        Paragraph::new(Line::from(vec![
            Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
            Span::raw("cancel  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("confirm"),
        ]))
        .style(Style::default().bg(HEADER_BG))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
            Span::raw("navigate  "),
            Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
            Span::raw("detail  "),
            Span::styled(" A ", Style::default().fg(ACCENT).bold()),
            Span::raw("append  "),
            Span::styled(" R ", Style::default().fg(ACCENT).bold()),
            Span::raw("sim  "),
            Span::styled(" F ", Style::default().fg(ACCENT).bold()),
            Span::raw("fork  "),
            Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
            Span::raw("back  "),
            Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
            Span::raw("quit"),
        ]))
        .style(Style::default().bg(HEADER_BG))
    };
    f.render_widget(footer, chunks[1]);
}

// ── Snapshot Detail ────────────────────────────────────────────────────────────────

fn render_snapshot_detail(f: &mut Frame, app: &App) {
    let snap = match app.snapshots.get(app.snapshot_idx) {
        Some(s) => s,
        None => {
            render_error(f, "No snapshot selected.");
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
            format!(" Snapshot Step {} ", snap.step),
            Style::default().fg(ACCENT).bold(),
        ),
        if snap.step as usize == app.snapshots.len() - 1 {
            Span::styled(" ◄ HEAD", Style::default().fg(GOOD))
        } else {
            Span::raw("")
        },
    ]));
    lines.push(Line::raw(""));

    // Entry text
    if !snap.entry_text.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Entry: {}", snap.entry_text),
            Style::default().fg(YELLOW),
        )));
        lines.push(Line::raw(""));
    }

    // Created at
    lines.push(Line::from(Span::styled(
        format!(" Created: {}", snap.created_at),
        Style::default().fg(DIM),
    )));
    lines.push(Line::raw(""));

    // Attribute values — resolved by schema names
    lines.push(Line::from(Span::styled(
        " Attribute values:",
        Style::default().fg(DIM),
    )));
    let values: Vec<f64> = serde_json::from_str(&snap.attributes_json).unwrap_or_default();
    for (i, val) in values.iter().enumerate() {
        let attr_name = app.schema.attributes.get(i).map(|a| a.name.as_str()).unwrap_or("?");
        lines.push(Line::from(Span::raw(format!(
            "   {:<24} = {:.2}", attr_name, val
        ))));
    }
    lines.push(Line::raw(""));

    // Decision info
    if let Some(decision_id) = snap.decision_id {
        let decision_name = app.decisions.iter()
            .find(|d| d.label.contains(&decision_id.to_string()))
            .map(|d| d.label.as_str())
            .unwrap_or("?");
        lines.push(Line::from(Span::styled(
            format!(" Decision: {} (ID: {})", decision_name, decision_id),
            Style::default().fg(ACCENT),
        )));
        if let Some(ref chosen) = snap.decision_chosen_outcome {
            lines.push(Line::from(Span::styled(
                format!(" Chosen outcome: {chosen}"),
                Style::default().fg(YELLOW),
            )));
        }
        lines.push(Line::raw(""));
    }

    // Forecast info
    if let Some(ref forecast_json) = snap.forecast_json {
        lines.push(Line::from(Span::styled(
            " Forecast:",
            Style::default().fg(DIM),
        )));
        // Try to show pretty-printed forecast summary
        if let Ok(forecast_val) = serde_json::from_str::<serde_json::Value>(forecast_json) {
            if let Some(probs) = forecast_val.get("outcome_probabilities") {
                if let Some(arr) = probs.as_array() {
                    for (i, prob_val) in arr.iter().enumerate() {
                        if let Some(p) = prob_val.as_f64() {
                            let outcome_label = app
                                .last_decision
                                .as_ref()
                                .and_then(|d| d.outcomes.get(i))
                                .map(|o| o.label.as_str())
                                .unwrap_or("outcome");
                            lines.push(Line::from(Span::raw(format!(
                                "   {outcome_label:<24} {:.1}%",
                                p * 100.0
                            ))));
                        }
                    }
                }
            }
            if let Some(util) = forecast_val.get("expected_utility") {
                if let Some(u) = util.as_f64() {
                    lines.push(Line::from(Span::raw(format!("   Expected utility: {u:.1}"))));
                }
            }
        } else {
            // Raw JSON
            lines.push(Line::from(Span::raw(format!("   {forecast_json}"))));
        }
        lines.push(Line::raw(""));
    }

    // Actual outcome
    if let Some(ref actual_json) = snap.actual_outcome_json {
        lines.push(Line::from(Span::styled(
            " Resolved actual outcome:",
            Style::default().fg(GOOD),
        )));
        if let Ok(deltas) = serde_json::from_str::<Vec<f64>>(actual_json) {
            for (i, delta) in deltas.iter().enumerate() {
                let attr_name = app.schema.attributes.get(i).map(|a| a.name.as_str()).unwrap_or("?");
                let sign = if *delta >= 0.0 { "+" } else { "" };
                lines.push(Line::from(Span::raw(format!(
                    "   {attr_name:<24} {sign}{:.2}", delta
                ))));
            }
        } else {
            lines.push(Line::from(Span::raw(format!("   {actual_json}"))));
        }
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(Span::styled(
        " Esc back  R sim from here  U resolve outcome  ↑↓ scroll",
        Style::default().fg(DIM),
    )));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" Snapshot Step {} ", snap.step))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll.offset as u16, 0));

    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
        Span::styled(" R ", Style::default().fg(ACCENT).bold()),
        Span::raw("sim  "),
        Span::styled(" U ", Style::default().fg(ACCENT).bold()),
        Span::raw("resolve  "),
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("scroll  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, chunks[1]);
}

// ── Fork Browser ───────────────────────────────────────────────────────────────────

fn render_fork_browser(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        format!(" Forks from \"{}\":", app.active_timeline_name),
        Style::default().fg(ACCENT).bold(),
    )));
    lines.push(Line::raw(""));

    if app.forks.is_empty() {
        lines.push(Line::from(Span::styled(
            " No forks yet. Press F on a snapshot to create a fork.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, fk) in app.forks.iter().enumerate() {
            let cursor = if i == app.fork_idx { " > " } else { "   " };
            let style = if i == app.fork_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };

            // Find child timeline name
            let child_name = app.timelines
                .iter()
                .find(|t| t.id == fk.child_timeline_id)
                .map(|t| t.name.as_str())
                .unwrap_or("?");

            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!("{:<20}", fk.label), style),
                Span::styled(
                    format!("→ {child_name}"),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        " ↑↓ navigate  Enter open child timeline  Esc back",
        Style::default().fg(DIM),
    )));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Fork Browser ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(block, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("open child  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("back  "),
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

// ══════════════════════════════════════════════════════════════════════════════
// Event editor screens
// ══════════════════════════════════════════════════════════════════════════════

// ── Edit Events list screen ────────────────────────────────────────────────

fn render_edit_events(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " Edit Events — event templates fire automatically when appending snapshots.",
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
                    .title(" Edit Events ")
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

    if app.edit_events.is_empty() {
        lines.push(Line::from(Span::styled(
            " No events defined. Press N to add one.",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, e) in app.edit_events.iter().enumerate() {
            let cursor = if i == app.edit_event_idx { " > " } else { "   " };
            let style = if i == app.edit_event_idx {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };
            let precond_count = e.preconditions.len();
            let effect_count = e.effects.len();
            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!("{:<24}", e.label), style),
                Span::styled(
                    format!("({precond_count} pre, {effect_count} effects, delay={}, dur={}, cd={})", e.delay, e.duration, e.cooldown),
                    Style::default().fg(DIM),
                ),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Events ")
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
        Span::styled(" N ", Style::default().fg(ACCENT).bold()),
        Span::raw("new  "),
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

// ── Edit Events Detail screen ────────────────────────────────────────────

fn render_edit_events_detail(f: &mut Frame, app: &App) {
    let state = match &app.edit_event_detail {
        Some(s) => s,
        None => {
            render_error(f, "No event edit state.");
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" Editing Event [{}]: ", state.idx),
        Style::default().fg(ACCENT).bold(),
    )));

    // Event ID
    lines.push(Line::from(vec![
        Span::styled(" ID: ", Style::default().fg(DIM)),
        Span::styled(&state.event_id, Style::default().fg(Color::White)),
    ]));

    // Label
    lines.push(Line::from(vec![
        Span::styled(" Label: ", Style::default().fg(DIM)),
        Span::styled(&state.label, Style::default().fg(Color::White)),
    ]));

    // Description
    lines.push(Line::from(vec![
        Span::styled(" Desc: ", Style::default().fg(DIM)),
        Span::styled(&state.description, Style::default().fg(DIM)),
    ]));

    // Delay, Duration, Cooldown
    lines.push(Line::from(vec![
        Span::styled(" Delay: ", Style::default().fg(DIM)),
        Span::styled(format!("{}", state.delay), Style::default().fg(Color::White)),
        Span::styled("  Duration: ", Style::default().fg(DIM)),
        Span::styled(format!("{}", state.duration), Style::default().fg(Color::White)),
        Span::styled("  Cooldown: ", Style::default().fg(DIM)),
        Span::styled(format!("{}", state.cooldown), Style::default().fg(Color::White)),
    ]));

    // Spawns decision
    lines.push(Line::from(vec![
        Span::styled(" Spawns decision: ", Style::default().fg(DIM)),
        Span::styled(&state.spawns_decision_id, Style::default().fg(YELLOW)),
    ]));
    lines.push(Line::raw(""));

    // Preconditions
    lines.push(Line::from(Span::styled(
        " Preconditions:",
        Style::default().fg(DIM),
    )));
    for (j, c) in state.preconditions.iter().enumerate() {
        let sel = if j == state.list_idx { " > " } else { "   " };
        let s = if j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("{} {:?} {}", c.attribute, c.operator, c.value), s),
        ]));
    }
    lines.push(Line::from(Span::styled(
        " [a] add condition  [d] delete selected",
        Style::default().fg(DIM),
    )));
    lines.push(Line::raw(""));

    // Effects
    lines.push(Line::from(Span::styled(
        " Effects:",
        Style::default().fg(DIM),
    )));
    for (j, eff) in state.effects.iter().enumerate() {
        let sel = if j == state.list_idx { " > " } else { "   " };
        let s = if j == state.list_idx {
            Style::default().fg(ACCENT).bold()
        } else {
            Style::default().fg(Color::White)
        };
        let sign = if eff.delta >= 0.0 { "+" } else { "" };
        let aname = eff.attribute.as_deref().unwrap_or("?");
        lines.push(Line::from(vec![
            Span::styled(sel, s),
            Span::styled(format!("{aname} {sign}{}", eff.delta), s),
        ]));
    }
    lines.push(Line::from(Span::styled(
        " [e] add effect  [f] delete selected effect",
        Style::default().fg(DIM),
    )));

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        " [Esc] save & back  Tab: cycle fields  Enter: edit text",
        Style::default().fg(DIM),
    )));

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Edit Event Detail ")
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

// ── Dashboard screen ─────────────────────────────────────────────────────────────────

fn render_dashboard(f: &mut Frame, app: &App) {
    let area = f.area();
    // Reserve top line for tab bar + bottom line for footer
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let body = inner[1];

    // Compute dynamic heights based on content
    let available = body.height as usize;
    let attr_rows = (app.schema.attributes.len() + 1) / 2; // 2 per row, ceil
    let state_h = (2 + attr_rows.max(1)).min(available / 3);
    let decision_h = (2 + app.dashboard_decisions.len().min(12).max(1)).min(available.saturating_sub(state_h + 4));
    let recent_h = available.saturating_sub(state_h + decision_h);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(state_h as u16),
            Constraint::Length(decision_h as u16),
            Constraint::Length(recent_h as u16),
        ])
        .split(body);

    // ── STATE block ──────────────────────────────────────────────────────────────
    let mut state_lines: Vec<Line> = Vec::new();
    let attrs: Vec<String> = app
        .schema
        .attributes
        .iter()
        .map(|a| {
            let val = app.current_state.get(&a.name).unwrap_or(0.0);
            format!("{}= {:.0}", a.name, val)
        })
        .collect();

    if !attrs.is_empty() {
        // Arrange in rows of 2
        for chunk in attrs.chunks(2) {
            let left = chunk.first().cloned().unwrap_or_default();
            let right = chunk.get(1).cloned().unwrap_or_default();
            state_lines.push(Line::from(vec![
                Span::styled(format!("  {:<30}", left), Style::default().fg(Color::White)),
                Span::styled(format!("{:<30}", right), Style::default().fg(DIM)),
            ]));
        }
    } else {
        state_lines.push(Line::from(Span::styled(
            "  (no schema loaded)",
            Style::default().fg(DIM),
        )));
    }

    let state_block = Paragraph::new(state_lines)
        .block(
            Block::default()
                .title(format!(" STATE — {} ", app.schema_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(state_block, sections[0]);

    // ── DECISIONS block ─────────────────────────────────────────────────────────
    let mut decision_lines: Vec<Line> = Vec::new();
    if app.dashboard_decisions.is_empty() {
        decision_lines.push(Line::from(Span::styled(
            "  No decisions loaded.",
            Style::default().fg(DIM),
        )));
    } else {
        let visible = &app.dashboard_decisions;
        let start = app.dashboard_scroll.min(visible.len().saturating_sub(1));
        for (i, (_, label, score, available)) in visible.iter().enumerate().skip(start).take(10) {
            let cursor = if i == app.dashboard_scroll { " >" } else { "  " };
            if *available {
                let util_str = if *score >= 0.0 {
                    format!("+{:.1}", score)
                } else {
                    format!("{:.1}", score)
                };
                decision_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} {}. {} ", cursor, i + 1, label),
                        Style::default().fg(GOOD),
                    ),
                    Span::styled(format!("util: {}", util_str), Style::default().fg(Color::White)),
                ]));
            } else {
                decision_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} {}. ✗ {} ", cursor, i + 1, label),
                        Style::default().fg(DIM),
                    ),
                    Span::styled("(unavailable)", Style::default().fg(DIM)),
                ]));
            }
        }
        if visible.len() > 10 {
            decision_lines.push(Line::from(Span::styled(
                format!("  ... and {} more (↑↓ to scroll)", visible.len() - 10),
                Style::default().fg(DIM),
            )));
        }
    }

    let decisions_block = Paragraph::new(decision_lines)
        .block(
            Block::default()
                .title(format!(" DECISIONS (ranked) — {} total ", app.dashboard_decisions.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(decisions_block, sections[1]);

    // ── RECENT block ────────────────────────────────────────────────────────────
    let mut recent_lines: Vec<Line> = Vec::new();
    if app.dashboard_recent.is_empty() {
        recent_lines.push(Line::from(Span::styled(
            "  No timeline activity yet.",
            Style::default().fg(DIM),
        )));
    } else {
        for (ts, entry) in &app.dashboard_recent {
            recent_lines.push(Line::from(vec![
                Span::styled(format!("  {}  ", ts), Style::default().fg(DIM)),
                Span::styled(entry, Style::default().fg(Color::White)),
            ]));
        }
    }

    let recent_block = Paragraph::new(recent_lines)
        .block(
            Block::default()
                .title(" RECENT ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(recent_block, sections[2]);

    // ── Footer ──────────────────────────────────────────────────────────────────
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓/jk ", Style::default().fg(ACCENT).bold()),
        Span::raw("scroll  "),
        Span::styled(" S ", Style::default().fg(ACCENT).bold()),
        Span::raw("simulate  "),
        Span::styled(" F ", Style::default().fg(ACCENT).bold()),
        Span::raw("fork  "),
        Span::styled(" R ", Style::default().fg(ACCENT).bold()),
        Span::raw("refresh  "),
        Span::styled(" L ", Style::default().fg(ACCENT).bold()),
        Span::raw("schema  "),
        Span::styled(" Tab ", Style::default().fg(ACCENT).bold()),
        Span::raw("tabs  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, inner[2]);
}

// ── Fork Explorer screen ─────────────────────────────────────────────────────────

fn render_fork_explorer(f: &mut Frame, app: &App) {
    let area = f.area();
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    let body = inner[1];

    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(" Fork Explorer: ", Style::default().fg(ACCENT).bold()),
        Span::styled(
            &app.fork_explorer_fork_name,
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::raw(""));

    // Source snapshot info
    if let Some(snap_id) = app.fork_explorer_snapshot_id {
        lines.push(Line::from(Span::styled(
            format!(" Forked from snapshot #{snap_id}"),
            Style::default().fg(DIM),
        )));
    }
    lines.push(Line::raw(""));

    // Comparison table
    if app.fork_explorer_results.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No comparison results.",
            Style::default().fg(DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " Decision          Utility    Top Attribute Deltas",
            Style::default().fg(DIM),
        )));
        lines.push(Line::from(Span::styled(
            " ────────────────  ─────────  ──────────────────────",
            Style::default().fg(DIM),
        )));

        for (i, (label, analysis)) in app.fork_explorer_results.iter().enumerate() {
            let is_selected = i == app.fork_explorer_idx;
            let cursor = if is_selected { " >" } else { "  " };
            let style = if is_selected {
                Style::default().fg(ACCENT).bold()
            } else {
                Style::default().fg(Color::White)
            };

            let util_str = if analysis.utility_distribution.mean >= 0.0 {
                format!("+{:.1}", analysis.utility_distribution.mean)
            } else {
                format!("{:.1}", analysis.utility_distribution.mean)
            };

            // Compute top attribute deltas (attribute changes from current state)
            let mut deltas: Vec<(String, f64)> = Vec::new();
            for (j, attr) in app.schema.attributes.iter().enumerate() {
                if j < analysis.attribute_outcomes.len() {
                    let current = app.current_state.get(&attr.name).unwrap_or(0.0);
                    let outcome_mean = analysis.attribute_outcomes[j].mean;
                    let delta = outcome_mean - current;
                    if delta.abs() > 0.5 {
                        deltas.push((attr.name.clone(), delta));
                    }
                }
            }
            // Sort by absolute delta descending, take top 3
            deltas.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
            deltas.truncate(3);

            let delta_str: String = deltas
                .iter()
                .map(|(name, d)| {
                    let sign = if *d >= 0.0 { "+" } else { "" };
                    format!("{name} {sign}{d:.1}")
                })
                .collect::<Vec<_>>()
                .join(", ");

            let delta_display = if delta_str.is_empty() {
                "(no significant changes)".to_string()
            } else {
                delta_str
            };

            lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(format!(" {:<16} ", label), style),
                Span::styled(format!(" {:<9} ", util_str), style),
                Span::styled(delta_display, Style::default().fg(DIM)),
            ]));
        }
    }

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Fork Explorer ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(block, body);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓/jk ", Style::default().fg(ACCENT).bold()),
        Span::raw("navigate  "),
        Span::styled(" Enter ", Style::default().fg(ACCENT).bold()),
        Span::raw("apply to master  "),
        Span::styled(" Esc ", Style::default().fg(ACCENT).bold()),
        Span::raw("cancel  "),
        Span::styled(" Q ", Style::default().fg(ACCENT).bold()),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(HEADER_BG));
    f.render_widget(footer, inner[2]);
}
