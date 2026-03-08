use crate::hub_client::QueueTask;
use crate::app::AppState;
use chrono::{NaiveDateTime, Utc};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct QueuePanelWidget;

impl QueuePanelWidget {
    pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
        // Filter out stale completed/failed tasks (older than 1 hour)
        let tasks: Vec<&QueueTask> = state
            .queue_tasks
            .iter()
            .filter(|t| {
                if t.status == "done" || t.status == "failed" {
                    // Keep if completed_at is within the last hour
                    t.completed_at
                        .as_ref()
                        .and_then(|ts| NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S").ok())
                        .map(|dt| Utc::now().naive_utc().signed_duration_since(dt).num_hours() < 1)
                        .unwrap_or(false)
                } else {
                    true // Always show running/pending/blocked
                }
            })
            .collect();

        let pending = tasks.iter().filter(|t| t.status == "pending").count();
        let running = tasks.iter().filter(|t| t.status == "running").count();
        let blocked = tasks
            .iter()
            .filter(|t| !t.depends_on.is_empty() && t.status == "pending")
            .count();

        let title = if state.hub_connected {
            format!(
                " Queue ({} run, {} pend, {} blk) ",
                running, pending, blocked
            )
        } else {
            " Queue (disconnected) ".to_string()
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta));

        if tasks.is_empty() {
            let msg = if state.hub_connected {
                "No tasks in queue"
            } else {
                "Hub API not connected"
            };
            let paragraph = Paragraph::new(Line::from(Span::styled(
                msg,
                Style::default().fg(Color::DarkGray),
            )))
            .block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let max_lines = (area.height as usize).saturating_sub(2); // borders
        let mut lines: Vec<Line> = Vec::new();

        for (i, &task) in tasks.iter().enumerate() {
            if i >= max_lines {
                break;
            }

            let (status_str, status_color) = match task.status.as_str() {
                "running" => ("RUN", Color::Green),
                "pending" if !task.depends_on.is_empty() => ("BLK", Color::Magenta),
                "pending" => ("PND", Color::Yellow),
                "done" => ("DON", Color::DarkGray),
                "failed" => ("ERR", Color::Red),
                _ => ("???", Color::DarkGray),
            };

            let priority_color = match task.priority {
                1 => Color::Red,
                2 => Color::Yellow,
                3 => Color::Green,
                _ => Color::DarkGray,
            };

            // Truncate project and task to fit
            let project = truncate(
                &task
                    .project
                    .replace("/tmp/", "")
                    .replace("/Users/pran/Projects/", ""),
                15,
            );
            let task_name = truncate(&task.task, 30);

            let spans = vec![
                Span::styled(
                    format!(" {} ", status_str),
                    Style::default()
                        .fg(Color::Black)
                        .bg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("P{}", task.priority),
                    Style::default().fg(priority_color),
                ),
                Span::raw(" "),
                Span::styled(format!("{:<15}", project), Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(task_name, Style::default().fg(Color::White)),
            ];

            // Add pane assignment if running
            let mut all_spans = spans;
            if let Some(pane) = task.pane {
                all_spans.push(Span::raw(" "));
                all_spans.push(Span::styled(
                    format!("[P{}]", pane),
                    Style::default().fg(Color::Green),
                ));
            }

            // Add dependency info if blocked
            if !task.depends_on.is_empty() && task.status == "pending" {
                all_spans.push(Span::raw(" "));
                all_spans.push(Span::styled(
                    format!("wait:{}", task.depends_on.len()),
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Add elapsed time for running tasks
            if task.status == "running" {
                if let Some(ref started) = task.started_at {
                    if let Ok(dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                        let elapsed = Utc::now().naive_utc().signed_duration_since(dt);
                        let mins = elapsed.num_minutes();
                        let display = if mins >= 60 {
                            format!("{}h{}m", mins / 60, mins % 60)
                        } else {
                            format!("{}m", mins)
                        };
                        all_spans.push(Span::raw(" "));
                        all_spans.push(Span::styled(display, Style::default().fg(Color::DarkGray)));
                    }
                }
            }

            lines.push(Line::from(all_spans));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
