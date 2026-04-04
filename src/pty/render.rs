//! Terminal renderer — converts `vt100::Screen` cells into ratatui `Line`/`Span`.
//!
//! This is the F3 component of G10 (Kill tmux). It bridges the PTY pool's
//! vt100 terminal state into ratatui widgets for both the grid pane cards
//! (small preview) and the focused pane view (full terminal rendering).
//!
//! All public functions take `&vt100::Screen` directly — the pool handles
//! locking and provides convenience methods that call into this module.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Convert a vt100 color to a ratatui color.
/// Maps the 16 standard ANSI colors by index, passes through 256-color and RGB.
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
            8 => Color::DarkGray,
            9 => Color::LightRed,
            10 => Color::LightGreen,
            11 => Color::LightYellow,
            12 => Color::LightBlue,
            13 => Color::LightMagenta,
            14 => Color::LightCyan,
            15 => Color::White,
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
pub fn render_row(screen: &vt100::Screen, row: u16) -> Line<'static> {
    let (_, cols) = screen.size();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();
    let mut first = true;

    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };

        // Skip wide-char continuation cells (the second half of a CJK char)
        if cell.is_wide_continuation() {
            continue;
        }

        let style = cell_style(cell);
        let contents = cell.contents();
        let text = if contents.is_empty() { " " } else { contents };

        if first {
            current_style = style;
            current_text.push_str(text);
            first = false;
        } else if style == current_style {
            current_text.push_str(text);
        } else {
            spans.push(Span::styled(
                std::mem::take(&mut current_text),
                current_style,
            ));
            current_style = style;
            current_text.push_str(text);
        }
    }

    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    Line::from(spans)
}

/// Render the full visible screen into ratatui `Line`s.
pub fn render_screen(screen: &vt100::Screen) -> Vec<Line<'static>> {
    let (rows, _) = screen.size();
    (0..rows).map(|row| render_row(screen, row)).collect()
}

/// Render the last N non-empty lines (for grid preview cards).
///
/// Scans backwards from the bottom of the screen to find the last row with
/// content, then returns up to `max_lines` rendered rows ending at that point.
pub fn render_tail(screen: &vt100::Screen, max_lines: usize) -> Vec<Line<'static>> {
    let (rows, _) = screen.size();

    // Find last non-empty row by checking row contents (most reliable)
    let mut last_nonempty: usize = 0;
    for row in (0..rows).rev() {
        let row_text = screen.contents_between(row, 0, row + 1, 0);
        if !row_text.trim().is_empty() {
            last_nonempty = row as usize + 1;
            break;
        }
    }

    let start = last_nonempty.saturating_sub(max_lines);
    (start..last_nonempty)
        .map(|row| render_row(screen, row as u16))
        .collect()
}

/// Get cursor position if visible, None if hidden.
pub fn cursor_visible(screen: &vt100::Screen) -> Option<(u16, u16)> {
    if screen.hide_cursor() {
        None
    } else {
        Some(screen.cursor_position())
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
        let parser = vt100::Parser::new(24, 80, 0);
        let cell = parser.screen().cell(0, 0).unwrap();
        let style = cell_style(cell);
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_render_row_empty() {
        let parser = vt100::Parser::new(24, 80, 0);
        let line = render_row(parser.screen(), 0);
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
        assert!(
            line.spans.len() >= 2,
            "Expected >= 2 spans, got {}: {:?}",
            line.spans.len(),
            line.spans.iter().map(|s| &s.content).collect::<Vec<_>>()
        );

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "RED");
        assert_eq!(first.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_render_row_with_bold() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"\x1b[1mBOLD\x1b[0m normal");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "BOLD");
        assert!(first.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_render_row_with_rgb() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"\x1b[38;2;255;128;0mORANGE\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "ORANGE");
        assert_eq!(first.style.fg, Some(Color::Rgb(255, 128, 0)));
    }

    #[test]
    fn test_render_row_style_coalescing() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"AAAAABBBBB");
        let line = render_row(parser.screen(), 0);
        // All same style — first span should contain the full text + trailing spaces
        let first_content: String = line.spans[0].content.to_string();
        assert!(first_content.starts_with("AAAAABBBBB"));
    }

    #[test]
    fn test_render_row_inverse() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"\x1b[7mINVERSE\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "INVERSE");
        assert!(first.style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn test_render_screen() {
        let mut parser = vt100::Parser::new(5, 40, 0);
        parser.process(b"Line 1\r\nLine 2\r\nLine 3");
        let lines = render_screen(parser.screen());
        assert_eq!(lines.len(), 5); // All 5 rows rendered

        let row0: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(row0.starts_with("Line 1"));

        let row1: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(row1.starts_with("Line 2"));
    }

    #[test]
    fn test_render_tail() {
        let mut parser = vt100::Parser::new(10, 40, 0);
        parser.process(b"A\r\nB\r\nC\r\nD\r\nE");
        let tail = render_tail(parser.screen(), 3);
        assert_eq!(tail.len(), 3);

        let texts: Vec<String> = tail
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect();
        assert_eq!(texts, vec!["C", "D", "E"]);
    }

    #[test]
    fn test_render_tail_empty_screen() {
        let parser = vt100::Parser::new(10, 40, 0);
        let tail = render_tail(parser.screen(), 5);
        assert!(tail.is_empty());
    }

    #[test]
    fn test_cursor_visible() {
        let parser = vt100::Parser::new(24, 80, 0);
        // By default, cursor should be visible at (0, 0)
        let pos = cursor_visible(parser.screen());
        assert_eq!(pos, Some((0, 0)));
    }

    #[test]
    fn test_cursor_hidden() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[?25l = hide cursor
        parser.process(b"\x1b[?25l");
        let pos = cursor_visible(parser.screen());
        assert_eq!(pos, None);
    }

    #[test]
    fn test_render_mixed_attributes() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // Bold red on blue background, underlined
        parser.process(b"\x1b[1;31;44;4mSTYLED\x1b[0m plain");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "STYLED");
        assert_eq!(first.style.fg, Some(Color::Red));
        assert_eq!(first.style.bg, Some(Color::Blue));
        assert!(first.style.add_modifier.contains(Modifier::BOLD));
        assert!(first.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_render_256_color() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"\x1b[38;5;196mCOLOR\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "COLOR");
        assert_eq!(first.style.fg, Some(Color::Indexed(196)));
    }

    #[test]
    fn test_render_dim() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[2m = dim/faint
        parser.process(b"\x1b[2mDIM\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "DIM");
        assert!(first.style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn test_render_background_color() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[42m = green background
        parser.process(b"\x1b[42mBG\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "BG");
        assert_eq!(first.style.bg, Some(Color::Green));
    }

    #[test]
    fn test_render_cursor_after_text() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello");
        let pos = cursor_visible(parser.screen());
        // Cursor should be at (0, 5) — after "Hello"
        assert_eq!(pos, Some((0, 5)));
    }

    #[test]
    fn test_render_cursor_after_newline() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Line1\r\nLine2");
        let pos = cursor_visible(parser.screen());
        // Cursor should be at row 1, col 5
        assert_eq!(pos, Some((1, 5)));
    }

    #[test]
    fn test_render_wide_char() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // CJK character (漢) occupies 2 cells
        parser.process("漢字".as_bytes());
        let line = render_row(parser.screen(), 0);

        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Should contain the wide chars without doubling
        assert!(text.contains("漢字"), "Wide chars not found: {:?}", text);
    }

    #[test]
    fn test_render_tail_more_than_content() {
        let mut parser = vt100::Parser::new(10, 40, 0);
        parser.process(b"Only one line");
        let tail = render_tail(parser.screen(), 5);
        // Should return just 1 line, not 5
        assert_eq!(tail.len(), 1);
    }

    #[test]
    fn test_render_screen_preserves_row_count() {
        let parser = vt100::Parser::new(3, 20, 0);
        let lines = render_screen(parser.screen());
        // Empty screen should still return all rows
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_render_italic() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[3m = italic
        parser.process(b"\x1b[3mITALIC\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "ITALIC");
        assert!(first.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_render_rgb_background() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[48;2;0;255;0m = RGB green background
        parser.process(b"\x1b[48;2;0;255;0mBG_RGB\x1b[0m");
        let line = render_row(parser.screen(), 0);

        let first = &line.spans[0];
        assert_eq!(first.content.as_ref(), "BG_RGB");
        assert_eq!(first.style.bg, Some(Color::Rgb(0, 255, 0)));
    }

    #[test]
    fn test_multiple_style_transitions() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // Red, then green, then blue — 3 distinct style spans
        parser.process(b"\x1b[31mR\x1b[32mG\x1b[34mB\x1b[0m");
        let line = render_row(parser.screen(), 0);

        assert!(
            line.spans.len() >= 3,
            "Expected >= 3 style transitions, got {}",
            line.spans.len()
        );
        assert_eq!(line.spans[0].style.fg, Some(Color::Red));
        assert_eq!(line.spans[1].style.fg, Some(Color::Green));
        assert_eq!(line.spans[2].style.fg, Some(Color::Blue));
    }
}
