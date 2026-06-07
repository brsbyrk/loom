//! Rendering functions for the Loom TUI.

use crate::app::{App, Screen};
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

// ── Entry point ──────────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::List => render_list(f, app),
        Screen::Detail => render_detail(f, app),
        Screen::Results => render_results(f, app),
        Screen::Error(msg) => render_error(f, msg),
    }
}

// ── List screen ──────────────────────────────────────────────────────────────────────

fn render_list(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    // Tabs row
    let tabs = vec!["Decisions", "Schema", "Config"];
    let tab_labels: Vec<Line> = tabs.iter().map(|t| Line::from(*t)).collect();
    let tab_bar = ratatui::widgets::Tabs::new(tab_labels)
        .select(0)
        .style(Style::default().fg(ACCENT))
        .divider(" ");
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
                    Span::styled(format!("({})", preconds), Style::default().fg(DIM)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!("   {} ", d.label), Style::default().fg(Color::White)),
                    Span::styled(format!("({})", preconds), Style::default().fg(DIM)),
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
            let status = if cond.check(&app.initial_state) {
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
            Span::styled(format!("{:.0}% ", pct), Style::default().fg(ACCENT).bold()),
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

        for (idx, prob) in &analysis.outcome_probabilities {
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
            let initial = app.initial_state.get(&attr.name).unwrap_or(0.0);
            // Only show attributes that changed by >0.5
            if (dist.mean - initial).abs() < 0.5
                && (dist.p5 - dist.p95).abs() < 0.5
            {
                continue;
            }
            let delta = dist.mean - initial;
            let delta_str = if delta >= 0.0 {
                Span::styled(format!("+{:.1}", delta), Style::default().fg(GOOD))
            } else {
                Span::styled(format!("{:.1}", delta), Style::default().fg(BAD))
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
                    "   Step 0: {:.0}  →  Step {}: {:.0}  Δ={:+.0}",
                    first.mean,
                    last.step,
                    last.mean,
                    delta
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
