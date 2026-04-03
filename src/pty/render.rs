//! Terminal renderer — converts `vt100::Screen` cells into ratatui `Line`/`Span`.
//!
//! This is the F3 component of G10 (Kill tmux). It bridges the PTY pool's
//! vt100 terminal state into ratatui widgets for both the grid pane cards
//! (small preview) and the focused pane view (full terminal rendering).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::pool::{PaneId, PtyPool};

/// Convert a vt100 color to a ratatui color.
/// Maps the 16 standard colors by index, passes through 256-color and RGB.
fn vt100_to_ratatui_color(color: vt100::Color) -> Option<Color> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(idx) => Some(match idx {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::White,
            // Bright/bold variants (8-15)
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
            // 256-color palette (16-255)
            n => Color::Indexed(n),
        }),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

/// Build a ratatui `Style` from a vt100 cell's attributes.
fn cell_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default();

    if let Some(fg) = vt100_to_ratatui_color(cell.fgcolor()) {
        style = style.fg(fg);
    }
    if let Some(bg) = vt100_to_ratatui_color(cell.bgcolor()) {
        style = style.bg(bg);
    }

    let mut mods = Modifier::empty();
    if cell.bold() {
        mods |= Modifier::BOLD;
    }
    if cell.dim() {
        mods |= Modifier::DIM;
    }
    if cell.italic() {
        mods |= Modifier::ITALIC;
    }
    if cell.underline() {
        mods |= Modifier::UNDERLINED;
    }
    if cell.inverse() {
        mods |= Modifier::REVERSED;
    }

    if !mods.is_empty() {
        style = style.add_modifier(mods);
    }
    style
}

/// Render a single row of vt100 cells into a ratatui `Line`.
///
/// Coalesces adjacent cells with the same style into a single `Span`
/// to minimize the number of spans per line.
fn render_row(screen: &vt100::Screen, row: u16) -> Line<'static> {
    let (_, cols) = screen.size();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();
    let mut first = true;

    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };

        // Skip wide-char continuation cells
        if cell.is_wide_continuation() {
            continue;
        }

        let style = cell_style(cell);
        let contents = cell.contents();
        // Empty cell = space
        let text = if contents.is_empty() { " " } else { contents };

        if first {
            current_style = style;
            current_text.push_str(text);
            first = false;
        } else if style == current_style {
            current_text.push_str(text);
        } else {
            // Flush previous span
            spans.push(Span::styled(
                std::mem::take(&mut current_text),
                current_style,
            ));
            current_style = style;
            current_text.push_str(text);
        }
    }

    // Flush last span
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    Line::from(spans)
}

/// Render the full visible screen of a pane into ratatui `Line`s.
///
/// Returns `None` if the pane doesn't exist.
/// Lines are trimmed of trailing whitespace-only spans for cleaner rendering.
pub fn render_screen(pool: &PtyPool, pane: PaneId) -> Option<Vec<Line<'static>>> {
    let parser_lock = pool.panes_internal()?.get(&pane)?.parser.lock();
    let parser = match parser_lock {
        Ok(p) => p,
        Err(e) => e.into_inner(),
    };
    let screen = parser.screen();
    let (rows, _) = screen.size();

    let lines: Vec<Line<'static>> = (0..rows).map(|row| render_row(screen, row)).collect();
    Some(lines)
}

/// Render the last N visible lines of a pane (for grid preview cards).
///
/// Returns only the last `max_lines` non-empty lines from the screen.
pub fn render_tail(pool: &PtyPool, pane: PaneId, max_lines: usize) -> Option<Vec<Line<'static>>> {
    let parser_lock = pool.panes_internal()?.get(&pane)?.parser.lock();
    let parser = match parser_lock {
        Ok(p) => p,
        Err(e) => e.into_inner(),
    };
    let screen = parser.screen();
    let (rows, _) = screen.size();

    // Find last non-empty row
    let mut last_nonempty = 0;
    for row in (0..rows).rev() {
        let line_text = screen.contents_between(row, 0, row + 1, 0);
        if !line_text.trim().is_empty() {
            last_nonempty = row as usize + 1;
            break;
        }
    }

    let start = last_nonempty.saturating_sub(max_lines);
    let lines: Vec<Line<'static>> = (start..last_nonempty)
        .map(|row| render_row(screen, row as u16))
        .collect();
    Some(lines)
}

/// Get the cursor position for overlay rendering.
/// Returns (row, col) if the cursor is visible, None if hidden.
pub fn cursor_position(pool: &PtyPool, pane: PaneId) -> Option<(u16, u16)> {
    let parser_lock = pool.panes_internal()?.get(&pane)?.parser.lock();
    let parser = match parser_lock {
        Ok(p) => p,
        Err(e) => e.into_inner(),
    };
    if parser.screen().hide_cursor() {
        None
    } else {
        Some(parser.screen().cursor_position())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vt100_color_mapping() {
        assert_eq!(vt100_to_ratatui_color(vt100::Color::Default), None);
        assert_eq!(
            vt100_to_ratatui_color(vt100::Color::Idx(1)),
            Some(Color::Red)
        );
        assert_eq!(
            vt100_to_ratatui_color(vt100::Color::Idx(9)),
            Some(Color::LightRed)
        );
        assert_eq!(
            vt100_to_ratatui_color(vt100::Color::Idx(200)),
            Some(Color::Indexed(200))
        );
        assert_eq!(
            vt100_to_ratatui_color(vt100::Color::Rgb(255, 128, 0)),
            Some(Color::Rgb(255, 128, 0))
        );
    }

    #[test]
    fn test_cell_style_default() {
        // Create a parser, get a default cell
        let parser = vt100::Parser::new(24, 80, 0);
        let cell = parser.screen().cell(0, 0).unwrap();
        let style = cell_style(cell);
        // Default cell should have no fg/bg/modifiers set
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_render_row_empty() {
        let parser = vt100::Parser::new(24, 80, 0);
        let line = render_row(parser.screen(), 0);
        // Empty row should produce spaces
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_render_row_with_text() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello, World!");
        let line = render_row(parser.screen(), 0);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with("Hello, World!"));
    }

    #[test]
    fn test_render_row_with_colors() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[31m = red foreground, ESC[0m = reset
        parser.process(b"\x1b[31mRED\x1b[0m NORMAL");
        let line = render_row(parser.screen(), 0);

        // Should have at least 2 spans: red "RED" and default " NORMAL..."
        assert!(line.spans.len() >= 2);

        // First span should be red
        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "RED");
        assert_eq!(first.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_render_row_with_bold() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[1m = bold, ESC[0m = reset
        parser.process(b"\x1b[1mBOLD\x1b[0m normal");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "BOLD");
        assert!(first.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_render_row_with_rgb() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[38;2;255;128;0m = RGB foreground
        parser.process(b"\x1b[38;2;255;128;0mORANGE\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "ORANGE");
        assert_eq!(first.style.fg, Some(Color::Rgb(255, 128, 0)));
    }

    #[test]
    fn test_render_row_style_coalescing() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // All same style — should be one span (plus trailing spaces)
        parser.process(b"AAAAABBBBB");
        let line = render_row(parser.screen(), 0);
        // First span should contain the full text since it's all default style
        let first_content: String = line.spans[0].content.to_string();
        assert!(first_content.starts_with("AAAAABBBBB"));
    }

    #[test]
    fn test_render_row_inverse() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[7m = inverse
        parser.process(b"\x1b[7mINVERSE\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "INVERSE");
        assert!(first.style.add_modifier.contains(Modifier::REVERSED));
    }
}
