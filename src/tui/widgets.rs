use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Status badge color
pub fn status_color(status: &str) -> Color {
    match status {
        "active" => Color::Green,
        "done" => Color::Blue,
        "error" => Color::Red,
        "idle" | "" => Color::DarkGray,
        _ => Color::Yellow,
    }
}

/// Theme color from hex string
pub fn theme_color(hex: &str) -> Color {
    if hex.starts_with('#') && hex.len() == 7 {
        let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(255);
        Color::Rgb(r, g, b)
    } else {
        Color::White
    }
}

/// Pane summary line for the grid
pub fn pane_line<'a>(
    pane_num: u8,
    theme_fg: &str,
    theme_name: &str,
    project: &str,
    role: &str,
    task: &str,
    status: &str,
    branch: Option<&str>,
    pty_running: bool,
    selected: bool,
    health: &str,
    runtime: &str,
) -> Line<'a> {
    let tc = theme_color(theme_fg);
    let sc = status_color(status);
    let pty_indicator = if pty_running { "▶" } else { "·" };

    // Show branch if available, otherwise task
    let task_display = match branch {
        Some(b) if !b.is_empty() => format!("{} | {}", truncate(b, 20), truncate(task, 15)),
        _ => truncate(task, 30),
    };

    let mut spans = vec![
        Span::styled(
            format!(" {} ", pane_num),
            Style::default().fg(Color::Black).bg(tc).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<7}", theme_name),
            Style::default().fg(tc),
        ),
        Span::styled(
            format!("{:<12}", truncate(project, 12)),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("{:<5}", role),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            format!("{:<7}", status),
            Style::default().fg(sc),
        ),
        Span::styled(
            pty_indicator.to_string(),
            Style::default().fg(if pty_running { Color::Green } else { Color::DarkGray }),
        ),
        health_badge(health),
        if !runtime.is_empty() {
            Span::styled(
                format!(" {:<6}", runtime),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::styled("       ", Style::default())
        },
        Span::styled(
            task_display,
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if selected {
        for span in &mut spans {
            span.style = span.style.add_modifier(Modifier::REVERSED);
        }
    }

    Line::from(spans)
}

/// Health badge for a pane
pub fn health_badge(badge: &str) -> Span<'static> {
    match badge {
        "error" => Span::styled(" ERR", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        "done" => Span::styled("  OK", Style::default().fg(Color::Blue)),
        "stuck" => Span::styled(" STK", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        _ => Span::styled("    ", Style::default()),
    }
}

/// Gauge bar using Unicode block characters
pub fn gauge_bar(value: f64, max: f64, width: usize) -> (String, Color) {
    let pct = if max > 0.0 { (value / max * 100.0) as u32 } else { 0 };
    let filled = if max > 0.0 { (value / max * width as f64) as usize } else { 0 }.min(width);
    let empty = width.saturating_sub(filled);
    let color = if pct > 80 { Color::Red } else if pct > 50 { Color::Yellow } else { Color::Green };
    (format!("{}{}", "█".repeat(filled), "░".repeat(empty)), color)
}

/// Compact inline bar as a styled Span (for dashboard gauges)
pub fn mini_bar(pct: u16, width: usize, color: Color) -> Span<'static> {
    let filled = (pct as usize * width / 100).min(width);
    let empty = width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    Span::styled(bar, Style::default().fg(color))
}

/// Priority color
pub fn priority_color(priority: &str) -> Color {
    match priority {
        "critical" => Color::Red,
        "high" => Color::Yellow,
        "medium" => Color::White,
        "low" => Color::DarkGray,
        _ => Color::DarkGray,
    }
}

pub fn truncate_pub(s: &str, max: usize) -> String {
    truncate(s, max)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", end)
    }
}
