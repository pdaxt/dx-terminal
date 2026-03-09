//! Tmux integration: spawn Claude agents in real tmux panes that users can see.
//!
//! This replaces the internal PTY approach with visible tmux windows.
//! DX Terminal creates windows, runs claude there, monitors via capture-pane.

use std::process::Command;
use anyhow::{Context, Result};

/// Default tmux session for factory agents
const DEFAULT_SESSION: &str = "claude6";

/// Info about a spawned tmux agent
#[derive(Clone, Debug)]
pub struct TmuxAgent {
    /// Full tmux target (e.g., "claude6:11.1")
    pub target: String,
    /// Window index
    pub window: u32,
    /// Window name
    pub name: String,
}

/// Create a new tmux window and return its target.
/// Window is named after the task (e.g., "factory-dev", "factory-qa").
pub fn create_window(name: &str) -> Result<TmuxAgent> {
    let session = active_session().unwrap_or_else(|| DEFAULT_SESSION.to_string());

    // Clean the name for tmux (no special chars)
    let clean_name: String = name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let clean_name = clean_name.trim_matches('-');

    // Create new window
    let output = Command::new("tmux")
        .args(["new-window", "-t", &session, "-n", clean_name, "-P", "-F", "#{window_index}"])
        .output()
        .context("Failed to create tmux window")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux new-window failed: {}", stderr.trim());
    }

    let window_idx: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);

    let target = format!("{}:{}.1", session, window_idx);

    Ok(TmuxAgent {
        target,
        window: window_idx,
        name: clean_name.to_string(),
    })
}

/// Send a command to a tmux pane (types it and presses Enter).
pub fn send_command(target: &str, cmd: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["send-keys", "-t", target, cmd, "Enter"])
        .output()
        .context("Failed to send tmux command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux send-keys failed: {}", stderr.trim());
    }
    Ok(())
}

/// Spawn a Claude agent in a new tmux window.
/// Returns the TmuxAgent with target info.
pub fn spawn_agent(
    window_name: &str,
    project_path: &str,
    prompt: &str,
) -> Result<TmuxAgent> {
    let agent = create_window(window_name)?;

    // cd to project directory
    send_command(&agent.target, &format!("cd {}", shell_escape(project_path)))?;

    // Small delay to let cd complete
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Build the claude command — escape the prompt for shell
    let claude_bin = resolve_claude_binary();
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let cmd = format!("{} --dangerously-skip-permissions -p '{}'", claude_bin, escaped_prompt);
    send_command(&agent.target, &cmd)?;

    Ok(agent)
}

/// Capture the current screen content of a tmux pane.
pub fn capture_output(target: &str) -> String {
    Command::new("tmux")
        .args(["capture-pane", "-t", target, "-p", "-S", "-50"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// Check if the agent in a tmux pane has finished (shell prompt visible).
pub fn check_done(target: &str) -> bool {
    let output = capture_output(target);
    if output.trim().is_empty() {
        return false;
    }

    let lines: Vec<&str> = output.trim().lines().collect();
    if let Some(last) = lines.last() {
        let trimmed = last.trim();
        // Shell prompt patterns — means Claude exited
        trimmed.ends_with('$')
            || trimmed.ends_with("$ ")
            || trimmed.ends_with('%')
            || trimmed.ends_with("% ")
            || trimmed.contains("Claude exited")
    } else {
        false
    }
}

/// Check if a tmux pane has an error (rate limit, crash, etc.)
pub fn check_error(target: &str) -> Option<String> {
    let output = capture_output(target);
    let patterns = [
        "Error:", "FATAL:", "panic:", "Traceback",
        "rate limit", "hit your limit", "SIGTERM",
    ];
    for pat in &patterns {
        if output.contains(pat) {
            return Some(pat.to_string());
        }
    }
    None
}

/// Kill a tmux window (closes the agent).
pub fn kill_window(target: &str) -> Result<()> {
    // Extract window part (e.g., "claude6:11" from "claude6:11.1")
    let window_target = if let Some(dot) = target.rfind('.') {
        &target[..dot]
    } else {
        target
    };

    let output = Command::new("tmux")
        .args(["kill-window", "-t", window_target])
        .output()
        .context("Failed to kill tmux window")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("tmux kill-window failed: {}", stderr.trim());
    }
    Ok(())
}

/// Send Ctrl-C to a tmux pane (interrupt running process).
pub fn send_interrupt(target: &str) -> Result<()> {
    Command::new("tmux")
        .args(["send-keys", "-t", target, "C-c", ""])
        .output()
        .context("Failed to send Ctrl-C")?;
    Ok(())
}

/// Get the active tmux session name (first attached session in claude6 group).
fn active_session() -> Option<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let sessions = String::from_utf8_lossy(&output.stdout);
    // Prefer "claude6" if it exists
    for line in sessions.lines() {
        let name = line.trim();
        if name == DEFAULT_SESSION {
            return Some(name.to_string());
        }
    }
    // Otherwise use first session
    sessions.lines().next().map(|s| s.trim().to_string())
}

/// List all tmux windows in the default session.
pub fn list_windows() -> Vec<(u32, String, bool)> {
    let session = active_session().unwrap_or_else(|| DEFAULT_SESSION.to_string());
    let output = Command::new("tmux")
        .args(["list-windows", "-t", &session, "-F", "#{window_index}|#{window_name}|#{window_active}"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.splitn(3, '|').collect();
                    if parts.len() >= 3 {
                        let idx: u32 = parts[0].parse().ok()?;
                        let name = parts[1].to_string();
                        let active = parts[2] == "1";
                        Some((idx, name, active))
                    } else {
                        None
                    }
                })
                .collect()
        }
        _ => vec![],
    }
}

/// Shell-escape a path (wrap in single quotes).
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Resolve "claude" to an absolute path.
fn resolve_claude_binary() -> String {
    let candidates = ["/opt/homebrew/bin/claude", "/usr/local/bin/claude"];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    if let Ok(output) = Command::new("which").arg("claude").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    "claude".to_string()
}
