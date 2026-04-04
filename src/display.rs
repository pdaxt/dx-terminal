use colored::Colorize;
use serde_json::Value;

/// Print a table with headers and rows, auto-sizing columns.
pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("{}", "  No results.".dimmed());
        return;
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(strip_ansi(cell).len());
            }
        }
    }

    // Cap widths at 60 chars
    for w in &mut widths {
        *w = (*w).min(60);
    }

    // Header
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    println!("  {}", header_line.join("  ").bold().cyan());

    let separator: String = widths.iter().map(|w| "─".repeat(*w)).collect::<Vec<_>>().join("──");
    println!("  {}", separator.dimmed());

    // Rows
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let w = widths.get(i).copied().unwrap_or(20);
                let visible_len = strip_ansi(cell).len();
                if visible_len > w {
                    format!("{}…", &cell[..cell.len().min(w.saturating_sub(1))])
                } else {
                    let pad = w.saturating_sub(visible_len);
                    format!("{}{}", cell, " ".repeat(pad))
                }
            })
            .collect();
        println!("  {}", cells.join("  "));
    }
    println!();
}

/// Print a status badge.
pub fn status(label: &str, msg: &str) {
    println!("  {} {}", label.green().bold(), msg);
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("  {} {}", "✗".red().bold(), msg);
}

/// Print a section header.
pub fn header(title: &str) {
    println!("\n  {}", title.bold().underline());
    println!();
}

/// Print a key-value pair.
pub fn kv(key: &str, value: &str) {
    println!("  {:>14}  {}", key.dimmed(), value);
}

/// Parse JSON string, printing error if it fails.
pub fn parse_json(s: &str) -> Result<Value, String> {
    serde_json::from_str(s).map_err(|e| format!("Failed to parse response: {}", e))
}

/// Extract error from JSON response.
pub fn check_error(v: &Value) -> Result<(), String> {
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        Err(err.to_string())
    } else {
        Ok(())
    }
}

/// Strip ANSI escape codes for width calculation.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            result.push(c);
        }
    }
    result
}

/// Truncate string to max length.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max.saturating_sub(1)])
    } else {
        s.to_string()
    }
}
