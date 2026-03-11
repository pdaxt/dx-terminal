//! Build environment MCP tools.
//!
//! Manage hacker-themed tmux build environments from MCP:
//! - dx_build_status: List all builds with theme colors and pane info
//! - dx_build_create: Create or restyle a build environment
//! - dx_build_restyle: Refresh colors on all builds
//! - dx_build_send: Send a command to a specific build pane
//! - dx_build_rename: Rename a build window

use crate::build;
use serde_json::json;

/// GET build environment status
pub fn build_status() -> String {
    let builds = build::build_status();
    let sessions = build::session_count();

    let build_list: Vec<serde_json::Value> = builds
        .iter()
        .map(|b| {
            json!({
                "number": b.number,
                "name": b.name,
                "theme": b.theme,
                "theme_desc": build::theme_desc(b.number),
                "pane_count": b.pane_count,
                "panes": b.panes.iter().map(|p| json!({
                    "index": p.pane_index,
                    "pane_id": p.pane_id,
                    "command": p.command,
                    "cwd": p.cwd,
                    "colors": {
                        "bg": p.colors.bg,
                        "fg": p.colors.fg,
                        "zsh_color": p.colors.zsh_color,
                    }
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    json!({
        "builds": build_list,
        "total_builds": builds.len(),
        "total_panes": builds.iter().map(|b| b.pane_count).sum::<usize>(),
        "sessions": sessions,
        "max_builds": 5,
    })
    .to_string()
}

/// Create or restyle a build environment
pub fn build_create(number: Option<u8>) -> String {
    let num = match number {
        Some(n) => n,
        None => {
            let next = build::next_build_num();
            if next > 5 {
                return json!({
                    "error": "Max 5 builds supported. Use rename to relabel existing builds."
                })
                .to_string();
            }
            next
        }
    };

    match build::create_build(num) {
        Ok(results) => json!({
            "status": "created",
            "build": num,
            "theme": build::theme_name(num),
            "theme_desc": build::theme_desc(num),
            "results": results,
        })
        .to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// Restyle all existing builds
pub fn build_restyle() -> String {
    match build::restyle_all() {
        Ok(results) => json!({
            "status": "restyled",
            "results": results,
        })
        .to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// Send a command to a specific build pane
pub fn build_send(build_num: u8, pane_num: u8, command: String) -> String {
    match build::send_to_build(build_num, pane_num, &command) {
        Ok(msg) => json!({"status": "sent", "message": msg}).to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// Rename a build window
pub fn build_rename(build_num: u8, new_name: String) -> String {
    match build::rename_build(build_num, &new_name) {
        Ok(results) => json!({
            "status": "renamed",
            "from": format!("build-{}", build_num),
            "to": new_name,
            "results": results,
        })
        .to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}
