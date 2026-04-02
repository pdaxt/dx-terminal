use std::io::{self, Write};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const SPINNER_VERBS: &[&str] = &[
    "Thinking",
    "Analyzing",
    "Reading",
    "Processing",
    "Examining",
    "Evaluating",
    "Searching",
    "Scanning",
    "Investigating",
    "Exploring",
    "Reasoning",
    "Computing",
    "Architecting",
    "Synthesizing",
    "Parsing",
    "Tracing",
    "Mapping",
    "Resolving",
    "Building",
    "Crafting",
];

/// Print a styled tool call indicator
pub fn print_tool_call(name: &str, input: &str) {
    let icon = match name {
        "read_file" | "Read" => "\x1b[36m◆\x1b[0m",   // cyan
        "write_file" | "Write" => "\x1b[33m◆\x1b[0m", // yellow
        "edit_file" | "Edit" => "\x1b[33m◆\x1b[0m",   // yellow
        "bash" => "\x1b[35m◆\x1b[0m",                 // magenta
        "glob" | "Glob" => "\x1b[36m◆\x1b[0m",        // cyan
        "grep" | "Grep" => "\x1b[36m◆\x1b[0m",        // cyan
        "git" => "\x1b[32m◆\x1b[0m",                  // green
        _ => "\x1b[37m◆\x1b[0m",                      // white
    };

    // Extract the most useful part of the input for display
    let summary = summarize_tool_input(name, input);
    eprintln!("{icon} \x1b[1m{name}\x1b[0m {summary}");
}

fn summarize_tool_input(name: &str, input: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(input).unwrap_or_default();

    match name {
        "read_file" | "Read" => {
            if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                return format!("\x1b[2m{path}\x1b[0m");
            }
        }
        "write_file" | "Write" => {
            if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                return format!("\x1b[2m{path}\x1b[0m");
            }
        }
        "edit_file" | "Edit" => {
            if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                return format!("\x1b[2m{path}\x1b[0m");
            }
        }
        "bash" => {
            if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                let truncated = if cmd.len() > 60 {
                    format!("{}...", &cmd[..57])
                } else {
                    cmd.to_string()
                };
                return format!("\x1b[2m$ {truncated}\x1b[0m");
            }
        }
        "glob" | "Glob" => {
            if let Some(pattern) = parsed.get("pattern").and_then(|v| v.as_str()) {
                return format!("\x1b[2m{pattern}\x1b[0m");
            }
        }
        "grep" | "Grep" => {
            if let Some(pattern) = parsed.get("pattern").and_then(|v| v.as_str()) {
                return format!("\x1b[2m/{pattern}/\x1b[0m");
            }
        }
        "git" => {
            if let Some(args) = parsed.get("args").and_then(|v| v.as_str()) {
                return format!("\x1b[2mgit {args}\x1b[0m");
            }
        }
        _ => {}
    }

    String::new()
}

/// Print the response border (Claude Code style)
#[allow(dead_code)]
pub fn print_response_start() {
    eprint!("\x1b[2m⎿ \x1b[0m");
}

/// Print thinking indicator
#[allow(dead_code)]
pub fn print_thinking_start() {
    eprintln!("\x1b[2;3mthinking...\x1b[0m");
}

/// Print a spinner with animated dots and rotating verb.
/// Dots cycle: ...  .. .  (empty) .  ..  ...
pub fn print_spinner(frame: usize, elapsed_secs: f64) {
    let spinner = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
    let verb_idx = (elapsed_secs as usize / 3) % SPINNER_VERBS.len();
    let verb = SPINNER_VERBS[verb_idx];

    // Animated dots: cycles through phases
    let dot_phase = ((elapsed_secs * 3.0) as usize) % 6;
    let dots = match dot_phase {
        0 => "   ",
        1 => ".  ",
        2 => ".. ",
        3 => "...",
        4 => ".. ",
        5 => ".  ",
        _ => "...",
    };

    // Color shifts toward yellow/red as time increases
    let color = if elapsed_secs < 5.0 {
        "\x1b[36m" // cyan
    } else if elapsed_secs < 15.0 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[31m" // red (stalling)
    };

    eprint!("\r{color}{spinner}\x1b[0m \x1b[2m{verb}{dots}\x1b[0m   ");
    io::stderr().flush().ok();
}

/// Clear the spinner line
pub fn clear_spinner() {
    eprint!("\r\x1b[K");
    io::stderr().flush().ok();
}

/// Print the summary footer
pub fn print_summary(tool_calls: usize, iterations: usize, tokens: u32, elapsed_secs: f64) {
    let cost_estimate = if tokens > 0 {
        // Rough estimate: ~$0.001 per 1K tokens for local, ~$0.003 for API
        format!(" (~${:.3})", tokens as f64 * 0.000003)
    } else {
        String::new()
    };

    eprintln!();
    eprintln!(
        "\x1b[2m─── {} tool calls │ {} iterations │ {} tokens{} │ {:.1}s ───\x1b[0m",
        tool_calls, iterations, tokens, cost_estimate, elapsed_secs
    );
}

/// Format markdown-ish text for terminal output
pub fn format_output(text: &str) -> String {
    let mut result = String::new();
    for line in text.lines() {
        if line.starts_with("```") {
            result.push_str(&format!("\x1b[2m{line}\x1b[0m\n"));
        } else if let Some(stripped) = line.strip_prefix("# ") {
            result.push_str(&format!("\x1b[1m{stripped}\x1b[0m\n"));
        } else if let Some(stripped) = line.strip_prefix("## ") {
            result.push_str(&format!("\x1b[1m{stripped}\x1b[0m\n"));
        } else if line.starts_with("- **") || line.starts_with("* **") {
            // Bold items in lists
            result.push_str(&format!("{line}\n"));
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
