//! Shared helpers used across all micro MCP modules.

use std::path::PathBuf;
use chrono::{Local, NaiveDateTime};
use crate::app::App;
use crate::config;
use crate::claude;
use crate::state;
use crate::tracker;
use crate::workspace;

/// JSON error response
pub fn json_err(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

/// Truncate string with ellipsis
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", end)
    }
}

/// Save agent output to file before killing (prevents irreversible output loss)
pub fn save_agent_output(app: &App, pane_num: u8, reason: &str) -> Option<String> {
    // Try tmux first by checking state synchronously via blocking_read (this runs in sync context)
    let state = app.state.blocking_read();
    let tmux_target = state.panes.get(&pane_num.to_string())
        .and_then(|p| p.tmux_target.clone());
    drop(state);

    let output = if let Some(ref target) = tmux_target {
        crate::tmux::capture_output(target)
    } else {
        // PTY fallback
        let pty = app.pty_lock();
        if !pty.has_agent(pane_num) {
            return None;
        }
        let o = pty.last_output(pane_num, 200).unwrap_or_default();
        let s = pty.screen_text(pane_num).unwrap_or_default();
        drop(pty);
        if !s.trim().is_empty() { s } else { o }
    };

    if output.trim().is_empty() {
        return None;
    }

    let dir = config::output_logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    let filename = format!("pane{}_{}.log", pane_num, Local::now().format("%Y%m%d_%H%M%S"));
    let path = dir.join(&filename);

    let content = format!(
        "=== DX Terminal Output Log ===\nPane: {}\nReason: {}\nTimestamp: {}\n\n=== Output ===\n{}\n",
        pane_num, reason, state::now(), output
    );
    let _ = std::fs::write(&path, &content);
    Some(path.to_string_lossy().to_string())
}

/// Extract meaningful result from agent output (PR URL, commit hash, etc.)
pub fn extract_result(app: &App, pane_num: u8) -> String {
    // Try tmux first
    let state = app.state.blocking_read();
    let tmux_target = state.panes.get(&pane_num.to_string())
        .and_then(|p| p.tmux_target.clone());
    drop(state);

    let text_owned = if let Some(ref target) = tmux_target {
        crate::tmux::capture_output(target)
    } else {
        let pty = app.pty_lock();
        let output = pty.last_output(pane_num, 50).unwrap_or_default();
        let screen = pty.screen_text(pane_num).unwrap_or_default();
        drop(pty);
        if !screen.trim().is_empty() { screen } else { output }
    };

    let text = &text_owned;
    let mut results = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // PR URL
        if trimmed.contains("github.com") && trimmed.contains("/pull/") {
            for word in trimmed.split_whitespace() {
                if word.contains("github.com") && word.contains("/pull/") {
                    results.push(format!("PR: {}", word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != ':' && c != '.' && c != '-' && c != '_')));
                    break;
                }
            }
        }
        // Git commit hash
        if trimmed.starts_with('[') && trimmed.contains(']') {
            for word in trimmed.split_whitespace() {
                let clean = word.trim_matches(|c: char| !c.is_ascii_hexdigit());
                if clean.len() >= 7 && clean.len() <= 40 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                    results.push(format!("commit: {}", &clean[..7.min(clean.len())]));
                    break;
                }
            }
        }
    }

    if results.is_empty() {
        text.lines().rev()
            .find(|l| !l.trim().is_empty() && !l.contains('$') && !l.contains('%'))
            .map(|l| truncate(l.trim(), 200))
            .unwrap_or_else(|| "auto-completed".into())
    } else {
        results.join("; ")
    }
}

// ========== Micro-Helpers: Composable building blocks ==========

/// Build pane_id string from pane number
pub fn pane_id_str(pane_num: u8) -> String {
    let window = (pane_num as u32 - 1) / 3 + 1;
    let pane = (pane_num as u32 - 1) % 3 + 1;
    format!("{}:{}.{}", config::session_name(), window, pane)
}

/// Result of workspace preparation (worktree + branch setup)
pub struct WorkspaceSetup {
    pub spawn_cwd: String,
    pub ws_path: Option<String>,
    pub ws_branch: Option<String>,
    pub ws_base: Option<String>,
    pub project_path: String,
    pub project_name: String,
}

/// Prepare workspace: resolve path, create worktree, claim branch
pub fn prepare_workspace(project: &str, pane_num: u8, task: &str) -> WorkspaceSetup {
    let project_path = config::resolve_project_path(project);
    let project_name = PathBuf::from(&project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project.to_string());

    let skip_worktrees = std::env::var("DX_SKIP_WORKTREES").is_ok();
    let (spawn_cwd, ws_path, ws_branch, ws_base) = if !skip_worktrees && workspace::is_git_repo(&project_path) {
        match workspace::create_worktree(&project_path, pane_num, task) {
            Ok(info) => {
                tracing::info!("Created worktree for pane {}: {} (branch {})", pane_num, info.worktree_path, info.branch_name);
                (info.worktree_path.clone(), Some(info.worktree_path), Some(info.branch_name), Some(info.base_branch))
            }
            Err(e) => {
                tracing::warn!("Worktree creation failed for pane {}, using direct path: {}", pane_num, e);
                (project_path.clone(), None, None, None)
            }
        }
    } else {
        if skip_worktrees {
            tracing::info!("Skipping worktree for pane {} (DX_SKIP_WORKTREES set)", pane_num);
        }
        (project_path.clone(), None, None, None)
    };

    WorkspaceSetup { spawn_cwd, ws_path, ws_branch, ws_base, project_path, project_name }
}

/// Pre-spawn cleanup: kill stale processes and clear locks for a pane
pub fn cleanup_pane_resources(pane_num: u8) {
    let home = std::env::var("HOME").unwrap_or_default();
    let profiles_dir = format!("{}/.playwright-profiles", home);

    // Scan all profile dirs matching this pane
    if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
        let prefix = format!("pane-{}-", pane_num);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) {
                continue;
            }
            let lock = entry.path().join("SingletonLock");
            if !lock.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&lock) {
                let pid_str: String = content.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(pid) = pid_str.parse::<i32>() {
                    // Check if PID is alive using libc::kill(pid, 0)
                    if unsafe { libc::kill(pid, 0) } != 0 {
                        let _ = std::fs::remove_file(&lock);
                        tracing::info!("Cleaned stale SingletonLock for pane {} (dead PID {})", pane_num, pid);
                    }
                }
            }
        }
    }

    tracing::debug!("Pre-spawn cleanup complete for pane {}", pane_num);
}

/// Select and configure MCPs for a project — auto-route if none explicitly set
pub async fn select_mcps(app: &App, project_name: &str, project_path: &str, task: &str, role: &str) -> Vec<String> {
    let mut mcps = app.state.get_project_mcps(project_name).await;
    if mcps.is_empty() {
        let matches = crate::mcp_registry::route_mcps(project_name, task, role);
        mcps = matches.iter()
            .filter(|m| m.score >= 20)
            .map(|m| m.name.clone())
            .collect();
        if !mcps.is_empty() {
            app.state.set_project_mcps(project_name, mcps.clone()).await;
        }
    }
    if !mcps.is_empty() {
        let _ = claude::set_project_mcps(project_path, &mcps);
    }
    mcps
}

/// Calculate ACU (Agent Compute Units) from a start timestamp
pub fn calculate_acu(started_at: &str) -> f64 {
    if let Ok(start_dt) = NaiveDateTime::parse_from_str(started_at, "%Y-%m-%dT%H:%M:%S") {
        let now = Local::now().naive_local();
        let hours = (now - start_dt).num_seconds() as f64 / 3600.0;
        (hours * 100.0).round() / 100.0
    } else {
        0.0
    }
}

/// Git result from finalize_git
pub struct GitResult {
    pub info: serde_json::Value,
}

/// Commit, push, create PR, optionally auto-merge, and cleanup worktree
pub fn finalize_git(ws: &str, branch: &str, base_project: &str, pane_num: u8, task: &str, summary: &str, acu: f64) -> GitResult {
    let commit_msg = if summary.is_empty() {
        format!("Pane {}: {}", pane_num, truncate(task, 60))
    } else {
        summary.to_string()
    };
    let commit_result = workspace::commit_all(ws, &commit_msg);
    let push_result = workspace::push_branch(ws, branch);
    let pr_title = format!("[Pane {}] {}", pane_num, truncate(task, 50));
    let pr_body = format!(
        "## Task\n{}\n\n## Summary\n{}\n\n## ACU\n{:.2}\n\nAutomated PR from DX Terminal pane {}",
        task, if summary.is_empty() { "completed" } else { summary }, acu, pane_num
    );
    let pr_result = workspace::create_pr(ws, &pr_title, &pr_body);
    let pr_url = pr_result.as_ref().cloned().unwrap_or_default();
    let auto_merge = workspace::auto_merge_pr(ws, &pr_url).unwrap_or_else(|e| e.to_string());
    let remove_result = workspace::remove_worktree(base_project, ws);

    let info = serde_json::json!({
        "commit": commit_result.unwrap_or_else(|e| e.to_string()),
        "push": push_result.unwrap_or_else(|e| e.to_string()),
        "pr": pr_result.unwrap_or_else(|e| e.to_string()),
        "auto_merge": auto_merge,
        "worktree_removed": remove_result.is_ok(),
        "branch": branch,
    });
    GitResult { info }
}

/// Attach commits/files to tracker issue (feature-to-code bridge)
pub fn attach_code_to_issue(space: &str, issue_id: &str, ws: &str, base: &str, started: &str) {
    let files = workspace::files_changed(ws, base);
    let commits = workspace::commits_since(ws, started);
    if !commits.is_empty() || !files.is_empty() {
        let _ = tracker::issue_attach_code(space, issue_id, &commits, &files);
    }
}

/// Check if all children of a feature are done, auto-close parent
pub fn check_feature_closure(space: &str, issue_id: &str) -> bool {
    if let Some(issue) = tracker::find_issue(space, issue_id) {
        if let Some(parent_id) = issue.get("parent").and_then(|v| v.as_str()) {
            let children_status = tracker::issue_children(space, parent_id);
            let total = children_status["count"].as_u64().unwrap_or(0);
            let done_count = children_status["done"].as_u64().unwrap_or(0);
            if total > 0 && done_count == total {
                let _ = tracker::issue_update_full(
                    space, parent_id, "done", "", "", "", "", "",
                    "", "", 0.0, 0.0, "", "",
                );
                let _ = tracker::issue_comment(
                    space, parent_id,
                    &format!("All {} micro-features completed. Feature auto-closed.", total),
                    "dx-terminal",
                );
                return true;
            }
        }
    }
    false
}

/// Register agent in coordination DB
pub fn update_agents_json(pane_num: u8, project: &str, task: &str) {
    let _ = crate::multi_agent::agent_register(&pane_id_str(pane_num), project, task, &[]);
}

/// Deregister agent from coordination DB
pub fn remove_from_agents_json(pane_num: u8) {
    let _ = crate::multi_agent::agent_deregister(&pane_id_str(pane_num));
}

/// MCP tool: machine identity for one or all panes
pub fn machine_info_tool(req: &super::super::types::MachineInfoRequest) -> String {
    let pane = req.pane.as_ref().and_then(|p| config::resolve_pane(p));
    crate::machine::machine_info(pane).to_string()
}

/// MCP tool: list all registered machines with network info
pub fn machine_list_tool() -> String {
    crate::machine::machine_list().to_string()
}
