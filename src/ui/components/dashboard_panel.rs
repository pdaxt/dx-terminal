//! Dashboard panel — shows capacity, sprint, board, MCPs, activity, session info.

use crate::hub_client::{AlertsResponse, AnalyticsDigest};
use crate::app::AppState;
use crate::state_reader::DashboardData;
use ratatui::{
    layout::{Constraint, Direction, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct DashboardWidget;

impl DashboardWidget {
    pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
        let dash = &state.dashboard;

        // Split into 5 columns
        let cols = ratatui::layout::Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ])
            .split(area);

        // Col 1: Capacity + Auto-cycle
        Self::render_capacity(frame, cols[0], dash);
        // Col 2: Sprint + Milestones + Board
        let mid = ratatui::layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(25),
                Constraint::Percentage(45),
            ])
            .split(cols[1]);
        Self::render_sprint(frame, mid[0], dash);
        Self::render_milestones(frame, mid[1], dash);
        Self::render_board(frame, mid[2], dash);
        // Col 3: MCPs + Activity
        let right = ratatui::layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(cols[2]);
        Self::render_mcps(frame, right[0], dash);
        Self::render_activity(frame, right[1], dash);
        // Col 4: Session + Processes + Multi-Agent
        let col4 = ratatui::layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ])
            .split(cols[3]);
        Self::render_session(frame, col4[0], dash);
        Self::render_processes(frame, col4[1], dash);
        Self::render_multi_agent(frame, col4[2], dash);
        // Col 5: Analytics (digest + alerts from API)
        let analytics = ratatui::layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(cols[4]);
        Self::render_digest(frame, analytics[0], &state.digest);
        Self::render_alerts(frame, analytics[1], &state.alerts);
    }

    fn gauge_spans(used: f64, total: f64, width: usize) -> Vec<Span<'static>> {
        let pct = if total > 0.0 { used / total } else { 0.0 };
        let filled = (pct * width as f64) as usize;
        let color = if pct > 0.8 {
            Color::Red
        } else if pct > 0.5 {
            Color::Yellow
        } else {
            Color::Green
        };

        vec![
            Span::styled(
                "\u{2588}".repeat(filled),
                Style::default().fg(color),
            ),
            Span::styled(
                "\u{2591}".repeat(width.saturating_sub(filled)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!(" {}/{}", used, total)),
        ]
    }

    fn render_capacity(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let cap = &dash.capacity;
        let auto = &dash.auto_config;
        let bn = cap.bottleneck();
        let bn_color = match bn {
            "REVIEW" => Color::Red,
            "COMPUTE" => Color::Yellow,
            _ => Color::Green,
        };

        let mut lines = vec![];

        // ACU gauge
        let mut acu_line = vec![Span::raw("ACU ")];
        acu_line.extend(Self::gauge_spans(cap.acu_used, cap.acu_total, 12));
        lines.push(Line::from(acu_line));

        // Review gauge
        let mut rev_line = vec![Span::raw("Rev ")];
        rev_line.extend(Self::gauge_spans(
            cap.reviews_used as f64,
            cap.reviews_total as f64,
            12,
        ));
        lines.push(Line::from(rev_line));

        // Bottleneck
        lines.push(Line::from(vec![
            Span::raw("Bot: "),
            Span::styled(bn, Style::default().fg(bn_color).add_modifier(Modifier::BOLD)),
        ]));

        // Auto-cycle
        lines.push(Line::from(vec![
            Span::raw("Assign: "),
            Span::styled(
                if auto.auto_assign { "ON" } else { "OFF" },
                Style::default().fg(if auto.auto_assign {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
            Span::raw("  Complete: "),
            Span::styled(
                if auto.auto_complete { "ON" } else { "OFF" },
                Style::default().fg(if auto.auto_complete {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("Par:{}  Cyc:{}s  ", auto.max_parallel, auto.cycle_interval)),
            Span::styled(
                format!("Role:{}", auto.default_role),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        if !auto.reserved_panes.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(
                    "Rsv: {}",
                    auto.reserved_panes
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }

        let block = Block::default()
            .title(" Capacity ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_sprint(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let lines = if let Some(sprint) = &dash.sprint {
            let pct = sprint.pct();
            let bar_w = 10;
            let filled = (pct / 100.0 * bar_w as f64) as usize;
            let bar_color = if pct >= 75.0 {
                Color::Green
            } else if pct >= 40.0 {
                Color::Yellow
            } else {
                Color::Red
            };

            let mut l = vec![
                Line::from(vec![
                    Span::styled(
                        &sprint.name,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({})", sprint.space),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::raw(format!("Issues: {}/{} ", sprint.done_issues, sprint.total_issues)),
                    Span::styled(
                        "\u{2588}".repeat(filled),
                        Style::default().fg(bar_color),
                    ),
                    Span::styled(
                        "\u{2591}".repeat(bar_w - filled),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(format!(" {:.0}%", pct)),
                ]),
                Line::from(Span::raw(format!(
                    "ACU: {}/{}",
                    sprint.used_acu, sprint.total_acu
                ))),
            ];

            if sprint.ended {
                l.push(Line::from(Span::styled(
                    "ENDED",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )));
            } else if sprint.days_left > 0 {
                let day_color = if sprint.days_left > 2 {
                    Color::Green
                } else {
                    Color::Yellow
                };
                l.push(Line::from(vec![
                    Span::raw("Days left: "),
                    Span::styled(
                        sprint.days_left.to_string(),
                        Style::default().fg(day_color),
                    ),
                ]));
            }
            l
        } else {
            vec![Line::from(Span::styled(
                "No sprint data",
                Style::default().fg(Color::DarkGray),
            ))]
        };

        let block = Block::default()
            .title(" Sprint ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_board(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let order = [
            "backlog",
            "todo",
            "in_progress",
            "review",
            "done",
            "closed",
        ];
        let icons: [(&str, &str, Color); 6] = [
            ("backlog", "\u{2610}", Color::DarkGray),
            ("todo", "\u{25cb}", Color::White),
            ("in_progress", "\u{25d4}", Color::Yellow),
            ("review", "\u{25d1}", Color::Cyan),
            ("done", "\u{2611}", Color::Green),
            ("closed", "\u{2612}", Color::DarkGray),
        ];
        let icon_map: std::collections::HashMap<&str, (&str, Color)> = icons
            .iter()
            .map(|(k, i, c)| (*k, (*i, *c)))
            .collect();

        let mut lines = vec![];
        if dash.board.spaces.is_empty() {
            lines.push(Line::from(Span::styled(
                "No issues",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (space_name, counts) in &dash.board.spaces {
                let total: usize = counts.values().sum();
                lines.push(Line::from(vec![
                    Span::styled(
                        space_name.as_str(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" ({})", total), Style::default().fg(Color::DarkGray)),
                ]));
                for status in &order {
                    if let Some(&count) = counts.get(*status) {
                        if count > 0 {
                            let (icon, color) = icon_map.get(status).unwrap_or(&(" ", Color::White));
                            let label = status.replace('_', " ");
                            lines.push(Line::from(vec![
                                Span::raw(format!("  {} ", icon)),
                                Span::styled(
                                    format!("{}: {}", label, count),
                                    Style::default().fg(*color),
                                ),
                            ]));
                        }
                    }
                }
            }
        }

        let block = Block::default()
            .title(" Board ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_milestones(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let max_lines = (area.height as usize).saturating_sub(2);
        let open: Vec<_> = dash
            .milestones
            .iter()
            .filter(|m| m.status != "closed" && m.status != "archived")
            .collect();

        let lines: Vec<Line> = if open.is_empty() {
            vec![Line::from(Span::styled(
                "No milestones",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            open.iter()
                .take(max_lines)
                .map(|m| {
                    let icon = match m.status.as_str() {
                        "open" => "\u{25cb}",
                        _ => "\u{25cf}",
                    };
                    let due = m
                        .due_date
                        .as_deref()
                        .map(|d| {
                            if d.len() >= 10 {
                                format!(" {}", &d[5..10])
                            } else {
                                format!(" {}", d)
                            }
                        })
                        .unwrap_or_default();
                    Line::from(vec![
                        Span::styled(
                            format!("{} ", icon),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            truncate_dash(&m.name, 16),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(due, Style::default().fg(Color::DarkGray)),
                    ])
                })
                .collect()
        };

        let title = format!(" Milestones ({}) ", open.len());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_processes(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let max_lines = (area.height as usize).saturating_sub(2);
        let active: Vec<_> = dash
            .processes
            .iter()
            .filter(|p| p.status == "active")
            .collect();

        let lines: Vec<Line> = if active.is_empty() {
            vec![Line::from(Span::styled(
                "No active workflows",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            active
                .iter()
                .take(max_lines)
                .map(|p| {
                    let pct = if p.total_steps > 0 {
                        (p.completed_steps as f64 / p.total_steps as f64 * 100.0) as u8
                    } else {
                        0
                    };
                    let bar_w = 6;
                    let filled = (pct as usize * bar_w) / 100;
                    let bar_color = if pct >= 75 {
                        Color::Green
                    } else if pct >= 40 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };
                    Line::from(vec![
                        Span::styled(
                            truncate_dash(&p.template, 12),
                            Style::default().fg(Color::White),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            "\u{2588}".repeat(filled),
                            Style::default().fg(bar_color),
                        ),
                        Span::styled(
                            "\u{2591}".repeat(bar_w.saturating_sub(filled)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(format!(" {}/{}", p.completed_steps, p.total_steps)),
                    ])
                })
                .collect()
        };

        let title = format!(" Workflows ({}) ", active.len());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_mcps(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let mut lines: Vec<Line> = dash
            .mcps
            .iter()
            .map(|m| {
                let icon = if m.is_rust { "\u{2699}" } else { "\u{2731}" };
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", icon),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<14}", m.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:>4}", m.tools),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(" \u{2713}", Style::default().fg(Color::Green)),
                ])
            })
            .collect();

        lines.push(Line::from(vec![Span::styled(
            format!("Total: {}+ tools", dash.total_mcp_tools()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]));

        let block = Block::default()
            .title(" MCPs ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_digest(frame: &mut Frame, area: Rect, digest: &AnalyticsDigest) {
        let lines = vec![
            Line::from(vec![
                Span::raw("Tool Calls: "),
                Span::styled(
                    digest.tool_calls.to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  Errors: "),
                Span::styled(
                    format!("{} ({})", digest.errors, digest.error_rate),
                    Style::default().fg(if digest.errors > 5 {
                        Color::Red
                    } else if digest.errors > 0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![
                Span::raw("Commits: "),
                Span::styled(
                    digest.commits.to_string(),
                    Style::default().fg(Color::Green),
                ),
                Span::raw("  Files: "),
                Span::styled(
                    digest.files_touched.to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::raw("Agents: "),
                Span::styled(
                    digest.agents_active.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  Tasks: "),
                Span::styled(
                    digest.tasks_completed.to_string(),
                    Style::default().fg(Color::Green),
                ),
            ]),
        ];

        let block = Block::default()
            .title(" 24h Digest ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_alerts(frame: &mut Frame, area: Rect, alerts: &AlertsResponse) {
        let lines: Vec<Line> = if alerts.alerts.is_empty() {
            vec![Line::from(Span::styled(
                "No alerts",
                Style::default().fg(Color::Green),
            ))]
        } else {
            alerts
                .alerts
                .iter()
                .take(5)
                .map(|a| {
                    let (icon, color) = match a.level.as_str() {
                        "critical" => ("!", Color::Red),
                        "warning" => ("~", Color::Yellow),
                        _ => ("i", Color::Cyan),
                    };
                    let detail = a
                        .pane_id
                        .as_deref()
                        .or(a.project.as_deref())
                        .or(a.error_rate.as_deref())
                        .unwrap_or("");
                    Line::from(vec![
                        Span::styled(
                            format!(" {} ", icon),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{:<8}", a.level.to_uppercase()),
                            Style::default().fg(color),
                        ),
                        Span::raw(format!("{:<16}", a.alert_type)),
                        Span::styled(detail.to_string(), Style::default().fg(Color::DarkGray)),
                    ])
                })
                .collect()
        };

        let title = if alerts.count > 0 {
            format!(" Alerts ({}) ", alerts.count)
        } else {
            " Alerts ".to_string()
        };
        let border_color = if alerts.alerts.iter().any(|a| a.level == "critical") {
            Color::Red
        } else if alerts.count > 0 {
            Color::Yellow
        } else {
            Color::Green
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_session(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let session = &dash.session;
        let mut lines = vec![];

        if !session.current_task.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Task: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    truncate_dash(&session.current_task, 25),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        if let Some(ref blocked) = session.blocked_on {
            lines.push(Line::from(vec![
                Span::styled("! ", Style::default().fg(Color::Red)),
                Span::styled(
                    truncate_dash(blocked, 25),
                    Style::default().fg(Color::Red),
                ),
            ]));
        }

        let done = session.completed.len();
        let next = session.next_steps.len();
        if done > 0 || next > 0 {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("\u{2713}{}", done),
                    Style::default().fg(Color::Green),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("\u{2192}{}", next),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        // Show first next step if available
        if let Some(step) = session.next_steps.first() {
            lines.push(Line::from(vec![
                Span::styled("  \u{2192} ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    truncate_dash(step, 22),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No session data",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let block = Block::default()
            .title(" Session ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_multi_agent(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let agents = &dash.multi_agent;
        let max_lines = (area.height as usize).saturating_sub(2);

        let lines: Vec<Line> = if agents.is_empty() {
            vec![Line::from(Span::styled(
                "No agents registered",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            agents
                .iter()
                .take(max_lines)
                .map(|a| {
                    // Extract pane number from pane_id like "claude6:1.1"
                    let pane_label = a
                        .pane_id
                        .rsplit(':')
                        .next()
                        .unwrap_or(&a.pane_id);
                    let ts = if a.last_update.len() > 16 {
                        &a.last_update[11..16]
                    } else if a.last_update.len() >= 5 {
                        &a.last_update[a.last_update.len() - 5..]
                    } else {
                        &a.last_update
                    };

                    Line::from(vec![
                        Span::styled(
                            format!("{:<5}", pane_label),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::styled(
                            format!("{:<10}", truncate_dash(&a.project, 10)),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            truncate_dash(&a.task, 15),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(" "),
                        Span::styled(ts.to_string(), Style::default().fg(Color::DarkGray)),
                    ])
                })
                .collect()
        };

        let title = format!(" Agents ({}) ", agents.len());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_activity(frame: &mut Frame, area: Rect, dash: &DashboardData) {
        let theme_colors: [(u8, Color); 9] = [
            (1, Color::Cyan),
            (2, Color::Green),
            (3, Color::Magenta),
            (4, Color::Rgb(255, 149, 0)),
            (5, Color::Red),
            (6, Color::Yellow),
            (7, Color::Gray),
            (8, Color::Rgb(0, 206, 201)),
            (9, Color::Rgb(253, 121, 168)),
        ];
        let color_map: std::collections::HashMap<u8, Color> =
            theme_colors.iter().copied().collect();

        let event_icons: std::collections::HashMap<&str, &str> = [
            ("spawn", "\u{25b6}"),
            ("kill", "\u{2717}"),
            ("complete", "\u{2713}"),
            ("reassign", "\u{21bb}"),
            ("restart", "\u{27f3}"),
            ("assign", "\u{2192}"),
        ]
        .into_iter()
        .collect();

        let lines: Vec<Line> = if dash.activity.is_empty() {
            vec![Line::from(Span::styled(
                "No activity",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            dash.activity
                .iter()
                .map(|e| {
                    let ts = if e.ts.len() > 16 {
                        &e.ts[11..16]
                    } else if e.ts.len() >= 5 {
                        &e.ts[e.ts.len() - 5..]
                    } else {
                        &e.ts
                    };
                    let color = color_map.get(&e.pane).copied().unwrap_or(Color::White);
                    let icon = event_icons.get(e.event.as_str()).unwrap_or(&"\u{2022}");
                    let summary: String = e.summary.chars().take(28).collect();

                    Line::from(vec![
                        Span::styled(format!("{} ", ts), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("P{}", e.pane), Style::default().fg(color)),
                        Span::raw(format!(" {} {}", icon, summary)),
                    ])
                })
                .collect()
        };

        let block = Block::default()
            .title(" Activity ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}

fn truncate_dash(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
