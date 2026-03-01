use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::{TuiMode, input};
use super::dashboard::DashboardData;

/// Render the appropriate overlay based on current TUI mode
pub fn render_overlay(f: &mut Frame, area: Rect, mode: &TuiMode, _data: &DashboardData) {
    match mode {
        TuiMode::Navigate => {} // No overlay
        TuiMode::Command { input: input_str, cursor } => {
            render_command_bar(f, area, input_str, *cursor);
        }
        TuiMode::Input { form } => {
            render_form(f, area, form);
        }
        TuiMode::Confirm { message, .. } => {
            render_confirm(f, area, message);
        }
        TuiMode::Executing { description, .. } => {
            render_executing(f, area, description);
        }
        TuiMode::Result { message, is_error, .. } => {
            render_result(f, area, message, *is_error);
        }
    }
}

/// Command bar at the bottom (vim-style :command)
fn render_command_bar(f: &mut Frame, area: Rect, input_str: &str, cursor: usize) {
    let bar = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    f.render_widget(Clear, bar);

    let display = format!(":{}", input_str);
    let cursor_pos = cursor + 1; // +1 for the ':'

    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled(&display[..cursor_pos.min(display.len())], Style::default().fg(Color::White)),
        Span::styled(
            if cursor_pos < display.len() { &display[cursor_pos..cursor_pos + 1] } else { " " },
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::styled(
            if cursor_pos + 1 < display.len() { &display[cursor_pos + 1..] } else { "" },
            Style::default().fg(Color::White),
        ),
    ]));
    f.render_widget(paragraph, bar);
}

/// Centered modal form
fn render_form(f: &mut Frame, area: Rect, form: &input::FormState) {
    let width = 50u16.min(area.width - 4);
    let height = (form.fields.len() as u16 * 2 + 4).min(area.height - 4);
    let popup = centered_rect(width, height, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {} ", form.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let field_constraints: Vec<Constraint> = form.fields.iter()
        .flat_map(|_| vec![Constraint::Length(1), Constraint::Length(1)])
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(field_constraints)
        .split(inner);

    for (i, field) in form.fields.iter().enumerate() {
        let label_style = if i == form.focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let required_marker = if field.required { "*" } else { "" };
        let label = Paragraph::new(format!("{}{}", field.label, required_marker))
            .style(label_style);
        f.render_widget(label, chunks[i * 2]);

        let value_display = if field.value.is_empty() && i != form.focused {
            Span::styled(&field.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(&field.value, Style::default().fg(Color::White))
        };

        let value_style = if i == form.focused {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let value = Paragraph::new(Line::from(value_display)).style(value_style);
        f.render_widget(value, chunks[i * 2 + 1]);
    }
}

/// Confirmation dialog
fn render_confirm(f: &mut Frame, area: Rect, message: &str) {
    let popup = centered_rect(40, 5, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let text = Paragraph::new(vec![
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" yes  "),
            Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" no"),
        ]),
    ]).alignment(Alignment::Center);
    f.render_widget(text, inner);
}

/// Executing spinner
fn render_executing(f: &mut Frame, area: Rect, description: &str) {
    let popup = centered_rect(40, 3, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let text = Paragraph::new(description)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);
    f.render_widget(text, inner);
}

/// Result message (success/error)
fn render_result(f: &mut Frame, area: Rect, message: &str, is_error: bool) {
    let width = (message.len() as u16 + 6).min(area.width - 4).max(30);
    let popup = centered_rect(width, 3, area);
    f.render_widget(Clear, popup);

    let color = if is_error { Color::Red } else { Color::Green };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let text = Paragraph::new(message)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center);
    f.render_widget(text, inner);
}

/// Create a centered rectangle
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
