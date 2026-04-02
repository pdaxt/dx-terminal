//! Build environment management — hacker-themed tmux build environments.
//!
//! Manages 5 color-coded build environments across all dx-build tmux sessions.
//! Each build has 3 vertical panes with unique neon color families:
//! - build-1: Bloodstream (crimson / ember / rust)
//! - build-2: Matrix (phosphor / jade / mint)
//! - build-3: Ghost Protocol (cobalt / electric / ice)
//! - build-4: Neon Noir (violet / magenta / pink)
//! - build-5: Molten (gold / amber / flame)

use serde::{Deserialize, Serialize};
use std::process::Command;

const MAX_BUILDS: u8 = 5;
const PROJECTS_DIR: &str = "/Users/pran/Projects";

/// Color palette for a single pane
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaneColors {
    pub bg: String,
    pub fg: String,
    pub zsh_color: String,
}

/// Info about a build environment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildInfo {
    pub number: u8,
    pub name: String,
    pub theme: String,
    pub pane_count: usize,
    pub panes: Vec<BuildPane>,
}

/// Info about a single pane within a build
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildPane {
    pub pane_id: String,
    pub pane_index: u8,
    pub colors: PaneColors,
    pub cwd: String,
    pub command: String,
}

/// Theme name for a build number
pub fn theme_name(n: u8) -> &'static str {
    match n {
        1 => "Bloodstream",
        2 => "Matrix",
        3 => "Ghost Protocol",
        4 => "Neon Noir",
        5 => "Molten",
        _ => "Unknown",
    }
}

/// Theme description for a build number
pub fn theme_desc(n: u8) -> &'static str {
    match n {
        1 => "crimson / ember / rust",
        2 => "phosphor / jade / mint",
        3 => "cobalt / electric / ice",
        4 => "violet / magenta / pink",
        5 => "gold / amber / flame",
        _ => "default",
    }
}

/// Get color palette for a specific build pane
pub fn palette(build: u8, pane: u8) -> PaneColors {
    let (bg, fg, zsh) = match (build, pane) {
        (1, 1) => ("#080404", "#e84040", "red"),
        (1, 2) => ("#080605", "#d4723a", "208"),
        (1, 3) => ("#070604", "#c49332", "214"),
        (2, 1) => ("#030806", "#3ddc84", "green"),
        (2, 2) => ("#040805", "#68d391", "114"),
        (2, 3) => ("#050804", "#a8e6a0", "157"),
        (3, 1) => ("#040408", "#5c9aff", "blue"),
        (3, 2) => ("#030508", "#38bdf8", "cyan"),
        (3, 3) => ("#040607", "#67e8f9", "159"),
        (4, 1) => ("#060408", "#a78bfa", "magenta"),
        (4, 2) => ("#070407", "#e879f9", "213"),
        (4, 3) => ("#080406", "#f472b6", "211"),
        (5, 1) => ("#080704", "#fbbf24", "yellow"),
        (5, 2) => ("#080504", "#f59e0b", "214"),
        (5, 3) => ("#080404", "#ef6c00", "202"),
        _ => ("#0a0a0a", "#888888", "white"),
    };
    PaneColors {
        bg: bg.to_string(),
        fg: fg.to_string(),
        zsh_color: zsh.to_string(),
    }
}

/// Get all dx-build tmux sessions
fn get_sessions() -> Vec<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|s| s.starts_with("dx-build"))
            .map(|s| s.to_string())
            .collect(),
        _ => vec![],
    }
}

/// Style a single pane with colors and prompt
fn style_pane(pane_id: &str, build_num: u8, pane_num: u8) {
    let colors = palette(build_num, pane_num);
    let label = format!("⚡B{}", build_num);

    // Set pane colors
    let _ = Command::new("tmux")
        .args([
            "select-pane",
            "-t",
            pane_id,
            "-P",
            &format!("bg={},fg={}", colors.bg, colors.fg),
        ])
        .output();

    // Set zsh prompt and clear
    let prompt_cmd = format!(
        "cd {} && export PROMPT='%F{{{}}}{}{{.{}}} %~%f %F{{{}}}❯%f '",
        PROJECTS_DIR, colors.zsh_color, label, pane_num, colors.zsh_color
    );
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", pane_id, &prompt_cmd, "Enter"])
        .output();

    let _ = Command::new("tmux")
        .args(["send-keys", "-t", pane_id, "clear", "Enter"])
        .output();
}

/// Style all panes in a build window for a given session
fn style_build(session: &str, window: &str, build_num: u8) -> usize {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            &format!("{}:{}", session, window),
            "-F",
            "#{pane_id}",
        ])
        .output();

    let pane_ids: Vec<String> = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => return 0,
    };

    for (i, pane_id) in pane_ids.iter().enumerate() {
        style_pane(pane_id, build_num, (i + 1) as u8);
    }

    pane_ids.len()
}

/// Create a build window in all dx-build sessions (or restyle if exists)
pub fn create_build(build_num: u8) -> Result<Vec<String>, String> {
    if !(1..=MAX_BUILDS).contains(&build_num) {
        return Err(format!(
            "Build number must be 1-{}. Got: {}",
            MAX_BUILDS, build_num
        ));
    }

    let sessions = get_sessions();
    if sessions.is_empty() {
        return Err("No dx-build tmux sessions found.".to_string());
    }

    let window_name = format!("build-{}", build_num);
    let mut results = Vec::new();

    for session in &sessions {
        // Check if window already exists
        let exists = Command::new("tmux")
            .args(["list-windows", "-t", session, "-F", "#{window_name}"])
            .output()
            .map(|o| {
                o.status.success()
                    && String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .any(|l| l.trim() == window_name)
            })
            .unwrap_or(false);

        if exists {
            let count = style_build(session, &window_name, build_num);
            results.push(format!(
                "{}:{} — restyled ({} panes)",
                session, window_name, count
            ));
        } else {
            // Create window with 3 vertical panes
            let _ = Command::new("tmux")
                .args([
                    "new-window",
                    "-t",
                    session,
                    "-n",
                    &window_name,
                    "-c",
                    PROJECTS_DIR,
                ])
                .output();
            let _ = Command::new("tmux")
                .args([
                    "split-window",
                    "-t",
                    &format!("{}:{}", session, window_name),
                    "-h",
                    "-c",
                    PROJECTS_DIR,
                ])
                .output();
            let _ = Command::new("tmux")
                .args([
                    "split-window",
                    "-t",
                    &format!("{}:{}", session, window_name),
                    "-h",
                    "-c",
                    PROJECTS_DIR,
                ])
                .output();
            let _ = Command::new("tmux")
                .args([
                    "select-layout",
                    "-t",
                    &format!("{}:{}", session, window_name),
                    "even-horizontal",
                ])
                .output();

            let count = style_build(session, &window_name, build_num);
            results.push(format!(
                "{}:{} — created {} ({})",
                session,
                window_name,
                theme_name(build_num),
                count
            ));
        }
    }

    Ok(results)
}

/// Restyle all existing build windows across all sessions
pub fn restyle_all() -> Result<Vec<String>, String> {
    let sessions = get_sessions();
    if sessions.is_empty() {
        return Err("No dx-build tmux sessions found.".to_string());
    }

    let first_session = &sessions[0];
    let mut results = Vec::new();

    // Find all build-N windows in first session
    let output = Command::new("tmux")
        .args(["list-windows", "-t", first_session, "-F", "#{window_name}"])
        .output();

    let windows: Vec<String> = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.starts_with("build-"))
            .map(|s| s.to_string())
            .collect(),
        _ => return Err("Failed to list windows.".to_string()),
    };

    for window in &windows {
        let num: u8 = window
            .strip_prefix("build-")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        if !(1..=MAX_BUILDS).contains(&num) {
            continue;
        }

        for session in &sessions {
            style_build(session, window, num);
        }
        results.push(format!(
            "{} ({}) — restyled across {} sessions",
            window,
            theme_name(num),
            sessions.len()
        ));
    }

    Ok(results)
}

/// Get status of all build environments
pub fn build_status() -> Vec<BuildInfo> {
    let sessions = get_sessions();
    if sessions.is_empty() {
        return vec![];
    }

    let first_session = &sessions[0];
    let mut builds = Vec::new();

    for n in 1..=MAX_BUILDS {
        let window_name = format!("build-{}", n);

        // Check if window exists
        let output = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                &format!("{}:{}", first_session, window_name),
                "-F",
                "#{pane_id}|#{pane_current_command}|#{pane_current_path}",
            ])
            .output();

        let panes: Vec<BuildPane> = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .lines()
                .enumerate()
                .filter_map(|(i, line)| {
                    let parts: Vec<&str> = line.splitn(3, '|').collect();
                    if parts.len() >= 3 {
                        Some(BuildPane {
                            pane_id: parts[0].to_string(),
                            pane_index: (i + 1) as u8,
                            colors: palette(n, (i + 1) as u8),
                            cwd: parts[2].to_string(),
                            command: parts[1].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            _ => continue,
        };

        if !panes.is_empty() {
            builds.push(BuildInfo {
                number: n,
                name: window_name,
                theme: theme_name(n).to_string(),
                pane_count: panes.len(),
                panes,
            });
        }
    }

    builds
}

/// Find the next available build number
pub fn next_build_num() -> u8 {
    let builds = build_status();
    let max = builds.iter().map(|b| b.number).max().unwrap_or(0);
    max + 1
}

/// Rename build windows across all sessions
pub fn rename_build(old_num: u8, new_name: &str) -> Result<Vec<String>, String> {
    let sessions = get_sessions();
    if sessions.is_empty() {
        return Err("No dx-build tmux sessions found.".to_string());
    }

    let old_window = format!("build-{}", old_num);
    let mut results = Vec::new();

    for session in &sessions {
        let output = Command::new("tmux")
            .args([
                "rename-window",
                "-t",
                &format!("{}:{}", session, old_window),
                new_name,
            ])
            .output();

        if output.map(|o| o.status.success()).unwrap_or(false) {
            results.push(format!("{}: {} → {}", session, old_window, new_name));
        }
    }

    if results.is_empty() {
        Err(format!("Window {} not found in any session.", old_window))
    } else {
        Ok(results)
    }
}

/// Send a command to a specific build pane
pub fn send_to_build(build_num: u8, pane_num: u8, command: &str) -> Result<String, String> {
    let sessions = get_sessions();
    let session = sessions.first().ok_or("No dx-build sessions found.")?;
    let target = format!("{}:build-{}.{}", session, build_num, pane_num - 1);

    Command::new("tmux")
        .args(["send-keys", "-t", &target, command, "Enter"])
        .output()
        .map_err(|e| format!("Failed to send command: {}", e))?;

    Ok(format!("Sent to build-{} pane {}", build_num, pane_num))
}

/// Get the session count
pub fn session_count() -> usize {
    get_sessions().len()
}
