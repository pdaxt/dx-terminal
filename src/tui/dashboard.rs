use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::config;
use crate::capacity;
use crate::queue;
use super::widgets;

/// Snapshot of pane data for rendering (no locks held during draw)
pub struct PaneSnapshot {
    pub pane: u8,
    pub theme: String,
    pub theme_fg: String,
    pub project: String,
    pub role: String,
    pub task: String,
    pub status: String,
    pub branch: Option<String>,
    pub pty_running: bool,
    pub line_count: usize,
}

/// Full dashboard snapshot — collected once per tick, no locks during render
pub struct DashboardData {
    pub panes: Vec<PaneSnapshot>,
    pub selected: u8,
    pub acu_used: f64,
    pub acu_total: f64,
    pub reviews_used: usize,
    pub reviews_total: usize,
    pub active_count: usize,
    pub pty_count: usize,
    pub selected_output: String,
    pub selected_screen: String,
    pub log_lines: Vec<String>,
    pub queue_lines: Vec<(String, String, String, String)>, // (status, priority, project, task)
    pub queue_pending: usize,
    pub queue_running: usize,
}

/// Collect all data in one pass (lock once, release)
pub fn collect_data(app: &App, selected: u8) -> DashboardData {
    // Blocking reads — TUI runs on its own thread, not async
    let state = app.state.blocking_read();

    let max_panes = config::pane_count();
    let mut panes = Vec::with_capacity(max_panes as usize);
    let mut active_count = 0;

    for i in 1..=max_panes {
        let pd = state.panes.get(&i.to_string()).cloned().unwrap_or_default();
        if pd.status == "active" {
            active_count += 1;
        }
        panes.push(PaneSnapshot {
            pane: i,
            theme: config::theme_name(i).to_string(),
            theme_fg: config::theme_fg(i).to_string(),
            project: pd.project,
            role: config::role_short(&pd.role).to_string(),
            task: pd.task,
            status: pd.status,
            branch: pd.branch_name,
            pty_running: false,
            line_count: 0,
        });
    }

    let log_lines: Vec<String> = state.activity_log.iter().take(5).map(|e| {
        let ts = e.ts.get(11..16).unwrap_or(&e.ts);
        format!("{} P{} {}", ts, e.pane, &e.summary)
    }).collect();

    drop(state);

    // PTY data
    let pty = app.pty_lock();
    let mut pty_count = 0;
    for ps in panes.iter_mut() {
        ps.pty_running = pty.is_running(ps.pane);
        ps.line_count = pty.line_count(ps.pane);
        if ps.pty_running {
            pty_count += 1;
        }
    }

    let selected_output = pty.last_output(selected, 40).unwrap_or_default();
    let selected_screen = pty.screen_text(selected).unwrap_or_default();
    drop(pty);

    let cap = capacity::load_capacity();

    // Queue data
    let q = queue::load_queue();
    let mut queue_pending = 0usize;
    let mut queue_running = 0usize;
    let queue_lines: Vec<(String, String, String, String)> = q.tasks.iter()
        .filter(|t| t.status != queue::QueueStatus::Done)
        .map(|t| {
            match t.status {
                queue::QueueStatus::Pending => queue_pending += 1,
                queue::QueueStatus::Running => queue_running += 1,
                _ => {}
            }
            let status = match t.status {
                queue::QueueStatus::Pending => "PEND",
                queue::QueueStatus::Running => "RUN ",
                queue::QueueStatus::Failed => "FAIL",
                queue::QueueStatus::Blocked => "BLOK",
                queue::QueueStatus::Done => "DONE",
            };
            let proj = t.project.split('/').last().unwrap_or(&t.project).to_string();
            (status.to_string(), format!("P{}", t.priority), proj, t.task.clone())
        })
        .collect();

    DashboardData {
        panes,
        selected,
        acu_used: cap.acu_used,
        acu_total: cap.acu_total,
        reviews_used: cap.reviews_used,
        reviews_total: cap.reviews_total,
        active_count,
        pty_count,
        selected_output,
        selected_screen,
        log_lines,
        queue_lines,
        queue_pending,
        queue_running,
    }
}

/// Render the full dashboard
pub fn render(f: &mut Frame, data: &DashboardData) {
    let pane_table_height = data.panes.len() as u16 + 3; // rows + header + 2 border
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                // Header bar
            Constraint::Length(pane_table_height), // Pane table (dynamic)
            Constraint::Min(8),                   // PTY output
            Constraint::Length(8),                 // Queue + Activity (split horizontal)
            Constraint::Length(1),                 // Help bar
        ])
        .split(f.area());

    // Split bottom panel horizontally: queue | activity
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Queue
            Constraint::Percentage(45), // Activity log
        ])
        .split(chunks[3]);

    render_header(f, chunks[0], data);
    render_pane_table(f, chunks[1], data);
    render_pty_output(f, chunks[2], data);
    render_queue(f, bottom[0], data);
    render_activity_log(f, bottom[1], data);
    render_help_bar(f, chunks[4]);
}

fn render_header(f: &mut Frame, area: Rect, data: &DashboardData) {
    let acu_pct = if data.acu_total > 0.0 {
        (data.acu_used / data.acu_total * 100.0) as u32
    } else {
        0
    };
    let acu_color = if acu_pct > 80 { Color::Red } else if acu_pct > 50 { Color::Yellow } else { Color::Green };
    let bottleneck = if data.reviews_used as f64 >= data.reviews_total as f64 * 0.8 {
        ("REVIEW", Color::Red)
    } else if acu_pct > 90 {
        ("COMPUTE", Color::Yellow)
    } else {
        ("BALANCED", Color::Green)
    };

    let header = Line::from(vec![
        Span::styled(" AgentOS ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("ACU ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1}/{:.0}", data.acu_used, data.acu_total),
            Style::default().fg(acu_color),
        ),
        Span::styled(format!(" ({}%)", acu_pct), Style::default().fg(Color::DarkGray)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Reviews ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", data.reviews_used, data.reviews_total),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Agents ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", data.active_count, data.panes.len()),
            Style::default().fg(if data.active_count > 0 { Color::Green } else { Color::DarkGray }),
        ),
        Span::styled(
            format!(" ({}▶)", data.pty_count),
            Style::default().fg(Color::Green),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(bottleneck.0, Style::default().fg(bottleneck.1).add_modifier(Modifier::BOLD)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(header).block(block);
    f.render_widget(p, area);
}

fn render_pane_table(f: &mut Frame, area: Rect, data: &DashboardData) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("  # ", Style::default().fg(Color::DarkGray)),
            Span::styled("Theme   ", Style::default().fg(Color::DarkGray)),
            Span::styled("Project     ", Style::default().fg(Color::DarkGray)),
            Span::styled("Role ", Style::default().fg(Color::DarkGray)),
            Span::styled("Status  ", Style::default().fg(Color::DarkGray)),
            Span::styled("▶ ", Style::default().fg(Color::DarkGray)),
            Span::styled("Branch/Task", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    for ps in &data.panes {
        lines.push(widgets::pane_line(
            ps.pane,
            &ps.theme_fg,
            &ps.theme,
            &ps.project,
            &ps.role,
            &ps.task,
            &ps.status,
            ps.branch.as_deref(),
            ps.pty_running,
            ps.pane == data.selected,
        ));
    }

    let block = Block::default()
        .title(" Panes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_pty_output(f: &mut Frame, area: Rect, data: &DashboardData) {
    let sel = &data.panes[(data.selected - 1) as usize];
    let branch_display = sel.branch.as_deref().unwrap_or("");
    let title = if !branch_display.is_empty() {
        format!(" P{} {} — {} [{}] ", sel.pane, sel.theme,
            if sel.project.is_empty() || sel.project == "--" { "idle" } else { &sel.project },
            branch_display)
    } else {
        format!(" P{} {} — {} ", sel.pane, sel.theme,
            if sel.project.is_empty() || sel.project == "--" { "idle" } else { &sel.project })
    };

    let tc = widgets::theme_color(&sel.theme_fg);

    // Prefer screen text (terminal state), fall back to line buffer
    let output = if !data.selected_screen.trim().is_empty() {
        &data.selected_screen
    } else if !data.selected_output.trim().is_empty() {
        &data.selected_output
    } else {
        "[No output — agent not running or no data yet]"
    };

    // Take last N lines that fit
    let available_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = output.lines()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(available_height)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tc));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_queue(f: &mut Frame, area: Rect, data: &DashboardData) {
    let title = format!(" Queue ({} pending, {} running) ", data.queue_pending, data.queue_running);
    let available = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = if data.queue_lines.is_empty() {
        vec![Line::from(Span::styled("  No queued tasks", Style::default().fg(Color::DarkGray)))]
    } else {
        data.queue_lines.iter().take(available).map(|(status, pri, proj, task)| {
            let sc = match status.trim() {
                "RUN" => Color::Green,
                "PEND" => Color::Yellow,
                "FAIL" => Color::Red,
                "BLOK" => Color::Magenta,
                _ => Color::DarkGray,
            };
            Line::from(vec![
                Span::styled(format!(" {} ", status), Style::default().fg(sc).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{} ", pri), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:<12}", widgets::truncate_pub(proj, 12)), Style::default().fg(Color::White)),
                Span::styled(widgets::truncate_pub(task, 30), Style::default().fg(Color::DarkGray)),
            ])
        }).collect()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_activity_log(f: &mut Frame, area: Rect, data: &DashboardData) {
    let lines: Vec<Line> = data.log_lines.iter().map(|l| {
        Line::from(Span::styled(l.as_str().to_string(), Style::default().fg(Color::DarkGray)))
    }).collect();

    let block = Block::default()
        .title(" Activity ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_help_bar(f: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" [1-9]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[q]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[k]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" kill  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[r]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" restart  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Tab]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" next  ", Style::default().fg(Color::DarkGray)),
    ]);
    let p = Paragraph::new(help);
    f.render_widget(p, area);
}
