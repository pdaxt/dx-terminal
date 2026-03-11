//! Tmux integration: spawn Claude agents in real tmux panes that users can see.
//!
//! This replaces the internal PTY approach with visible tmux windows.
//! DX Terminal creates windows, runs claude there, monitors via capture-pane.

use anyhow::{Context, Result};
use std::process::Command;

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
    let clean_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let clean_name = clean_name.trim_matches('-');

    // Create new window
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            &session,
            "-n",
            clean_name,
            "-P",
            "-F",
            "#{window_index}",
        ])
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
///
/// `env_vars` are exported before running claude (e.g. P=3, MACHINE_IP, etc.)
/// `autonomous` controls whether --dangerously-skip-permissions is used.
pub fn spawn_agent(
    window_name: &str,
    project_path: &str,
    prompt: &str,
    env_vars: &[(String, String)],
    autonomous: bool,
) -> Result<TmuxAgent> {
    let agent = create_window(window_name)?;

    // Export environment variables
    if !env_vars.is_empty() {
        let exports: Vec<String> = env_vars
            .iter()
            .map(|(k, v)| format!("export {}={}", k, shell_escape(v)))
            .collect();
        send_command(&agent.target, &exports.join(" && "))?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // cd to project directory
    send_command(&agent.target, &format!("cd {}", shell_escape(project_path)))?;

    // Small delay to let cd complete
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Build the claude command — escape the prompt for shell
    let claude_bin = resolve_claude_binary();
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let perms_flag = if autonomous {
        " --dangerously-skip-permissions"
    } else {
        ""
    };
    let cmd = format!("{}{} -p '{}'", claude_bin, perms_flag, escaped_prompt);
    send_command(&agent.target, &cmd)?;

    Ok(agent)
}

/// Check if a tmux pane/window target exists.
/// Uses `tmux display-message` which validates the full target (session:window.pane).
pub fn pane_exists(target: &str) -> bool {
    Command::new("tmux")
        .args(["display-message", "-t", target, "-p", "#{pane_id}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
        "Error:",
        "FATAL:",
        "panic:",
        "Traceback",
        "rate limit",
        "hit your limit",
        "SIGTERM",
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
        .args([
            "list-windows",
            "-t",
            &session,
            "-F",
            "#{window_index}|#{window_name}|#{window_active}",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
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
            .collect(),
        _ => vec![],
    }
}

/// A live tmux pane running Claude (discovered from any session).
#[derive(Clone, Debug, serde::Serialize)]
pub struct LivePane {
    /// Full tmux target (e.g., "dx-build:1.1")
    pub target: String,
    /// Session name
    pub session: String,
    /// Window index
    pub window: u32,
    /// Pane index within window
    pub pane_idx: u32,
    /// Window name (e.g., "build-1")
    pub window_name: String,
    /// Process running in the pane (e.g., "claude")
    pub command: String,
    /// Working directory of the pane
    pub cwd: String,
    /// Shell PID of the pane
    pub pid: u32,
    /// Resolved JSONL session file (if found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonl_path: Option<String>,
    /// Session ID from the JSONL file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Discover all tmux panes running Claude across all sessions.
/// Returns them ordered by session, window, pane.
/// Also resolves each pane's working directory and JSONL session file.
pub fn discover_live_panes() -> Vec<LivePane> {
    let output = Command::new("tmux")
        .args([
            "list-panes", "-a", "-F",
            "#{session_name}|#{window_index}|#{pane_index}|#{window_name}|#{pane_current_command}|#{pane_current_path}|#{pane_pid}"
        ])
        .output();

    let mut panes: Vec<LivePane> = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(7, '|').collect();
                if parts.len() >= 7 {
                    let session = parts[0].to_string();
                    let window: u32 = parts[1].parse().ok()?;
                    let pane_idx: u32 = parts[2].parse().ok()?;
                    let window_name = parts[3].to_string();
                    let command = parts[4].to_string();
                    let cwd = parts[5].to_string();
                    let pid: u32 = parts[6].parse().unwrap_or(0);
                    if command == "claude" || command == "node" {
                        Some(LivePane {
                            target: format!("{}:{}.{}", session, window, pane_idx),
                            session,
                            window,
                            pane_idx,
                            window_name,
                            command,
                            cwd,
                            pid,
                            jsonl_path: None,
                            session_id: None,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect(),
        _ => vec![],
    };

    // Resolve JSONL session files for each pane
    resolve_jsonl_sessions(&mut panes);
    panes
}

/// Map each live pane to its Claude JSONL session file.
/// Uses cwd matching + most-recently-modified heuristic.
fn resolve_jsonl_sessions(panes: &mut [LivePane]) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/pran".into());
    let projects_dir = format!("{}/.claude/projects", home);

    // Scan all JSONL files modified in last 2 hours
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(7200);

    // Build index: project_dir_key -> Vec<(jsonl_path, session_id, cwd, mtime)>
    let mut jsonl_index: Vec<(String, String, String, u64)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip subdirectories (subagents, etc.) — only top-level dirs
            if path.is_dir() {
                // Scan JSONL files inside project dir
                if let Ok(files) = std::fs::read_dir(&path) {
                    for f in files.flatten() {
                        let fp = f.path();
                        if fp.extension().map(|e| e == "jsonl").unwrap_or(false)
                            && !fp.to_string_lossy().contains("/subagents/")
                        {
                            if let Ok(meta) = fp.metadata() {
                                let mtime = meta
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                if mtime > cutoff {
                                    // Read first line for sessionId and cwd
                                    if let Some((sid, cwd)) = read_jsonl_header(&fp) {
                                        jsonl_index.push((
                                            fp.to_string_lossy().to_string(),
                                            sid,
                                            cwd,
                                            mtime,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                // Top-level JSONL file
                if let Ok(meta) = path.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if mtime > cutoff {
                        if let Some((sid, cwd)) = read_jsonl_header(&path) {
                            jsonl_index.push((path.to_string_lossy().to_string(), sid, cwd, mtime));
                        }
                    }
                }
            }
        }
    }

    // Sort by mtime descending (most recent first)
    jsonl_index.sort_by(|a, b| b.3.cmp(&a.3));

    // Match each pane to a JSONL file by cwd
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pane in panes.iter_mut() {
        for (jpath, sid, cwd, _mtime) in &jsonl_index {
            if used.contains(jpath) {
                continue;
            }
            // Match: pane cwd starts with jsonl cwd or vice versa
            if pane.cwd == *cwd || pane.cwd.starts_with(cwd) || cwd.starts_with(&pane.cwd) {
                pane.jsonl_path = Some(jpath.clone());
                pane.session_id = Some(sid.clone());
                used.insert(jpath.clone());
                break;
            }
        }
    }
}

/// Read the first line of a JSONL file to extract sessionId and cwd.
fn read_jsonl_header(path: &std::path::Path) -> Option<(String, String)> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(f);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(&line).ok()?;
    let sid = v.get("sessionId")?.as_str()?.to_string();
    let cwd = v.get("cwd")?.as_str()?.to_string();
    Some((sid, cwd))
}

/// Read the effective project cwd from a JSONL file.
/// If the header cwd is a generic parent (e.g. ~/Projects), scan tool_use entries
/// for the most-referenced project subdirectory.
pub fn read_jsonl_cwd(path: &str) -> Option<String> {
    use std::io::BufRead;
    let (_, header_cwd) = read_jsonl_header(std::path::Path::new(path))?;

    // If the cwd already points to a specific project, use it
    let projects_parent = std::env::var("HOME")
        .map(|h| format!("{}/Projects", h))
        .unwrap_or_else(|_| "/Users/pran/Projects".to_string());
    if header_cwd != projects_parent {
        return Some(header_cwd);
    }

    // Header cwd is generic ~/Projects — scan file for most-used project path
    let prefix = format!("{}/", projects_parent);
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let f = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(f);
    let mut lines_read = 0usize;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        lines_read += 1;
        if lines_read > 500 {
            break;
        } // cap scan to first 500 lines

        // Find all occurrences of /Users/pran/Projects/<name>
        let mut start = 0;
        while let Some(pos) = line[start..].find(&prefix) {
            let abs = start + pos + prefix.len();
            if abs < line.len() {
                let rest = &line[abs..];
                let end = rest
                    .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
                    .unwrap_or(rest.len());
                if end > 0 {
                    let name = &rest[..end];
                    *counts.entry(name.to_string()).or_insert(0) += 1;
                }
            }
            start = abs;
        }
    }

    // Return the most-referenced project, or fall back to header cwd
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(name, _)| format!("{}/{}", projects_parent, name))
        .or(Some(header_cwd))
}

/// Capture output from a tmux pane — extended version with more lines for live view.
pub fn capture_output_extended(target: &str, lines: u32) -> String {
    Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            target,
            "-p",
            "-S",
            &format!("-{}", lines),
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
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
