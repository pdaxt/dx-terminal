use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge},
};

/// Colored gauge with label
pub fn capacity_gauge<'a>(title: &'a str, used: f64, total: f64, color: Color) -> Gauge<'a> {
    let pct = if total > 0.0 { (used / total * 100.0).min(100.0) } else { 0.0 };
    let label = format!("{:.1}/{:.0} ({:.0}%)", used, total, pct);
    Gauge::default()
        .block(Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .gauge_style(Style::default().fg(color))
        .ratio(pct / 100.0)
        .label(label)
}

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

/// Health badge color
pub fn health_color(health: &str) -> Color {
    match health {
        "ok" => Color::Green,
        "done" => Color::Blue,
        "error" => Color::Red,
        "stuck" => Color::Yellow,
        "idle" => Color::DarkGray,
        _ => Color::DarkGray,
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
    pty_running: bool,
    selected: bool,
) -> Line<'a> {
    let tc = theme_color(theme_fg);
    let sc = status_color(status);
    let pty_indicator = if pty_running { "▶" } else { "·" };

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
        Span::raw(" "),
        Span::styled(
            truncate(task, 30).to_string(),
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", end)
    }
}
