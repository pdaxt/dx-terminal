//! Screen management MCP tools.
//!
//! These tools enable dynamic screen/pane management:
//! - dx_add_screen: Create new screens at runtime
//! - dx_remove_screen: Remove screens (kills agents first)
//! - dx_list_screens: List all screens with pane status
//! - dx_screen_summary: Get screen layout overview

use crate::app::App;
use serde_json::json;

/// Add a new screen with configurable layout
pub fn add_screen(
    app: &App,
    name: Option<String>,
    layout: Option<String>,
    panes: Option<u8>,
) -> String {
    let mgr = app.screens.read().unwrap();
    match mgr.add_screen(name, layout, panes) {
        Ok(screen) => {
            let total = mgr.total_panes();
            json!({
                "status": "created",
                "screen": {
                    "id": screen.id,
                    "name": screen.name,
                    "panes": screen.panes,
                    "layout": screen.layout,
                    "tmux_window": screen.tmux_window,
                },
                "total_screens": mgr.list_screens().len(),
                "total_panes": total,
            }).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// Remove a screen by ID or name
pub fn remove_screen(
    app: &App,
    screen_ref: String,
    force: bool,
) -> String {
    let mgr = app.screens.read().unwrap();

    // Check if any agents are running on this screen's panes
    if !force {
        if let Some(screen) = mgr.list_screens().iter().find(|s| {
            s.id.to_string() == screen_ref || s.name.to_lowercase() == screen_ref.to_lowercase()
        }) {
            let state = app.state.blocking_read();
            let active_panes: Vec<u8> = screen.panes.iter().filter(|p| {
                state.panes.get(&p.to_string())
                    .map(|ps| ps.status == "active")
                    .unwrap_or(false)
            }).copied().collect();

            if !active_panes.is_empty() {
                return json!({
                    "error": "Screen has active agents",
                    "active_panes": active_panes,
                    "hint": "Kill agents first or use force=true",
                }).to_string();
            }
        }
    }

    match mgr.remove_screen(&screen_ref) {
        Ok(removed) => {
            json!({
                "status": "removed",
                "screen": {
                    "id": removed.id,
                    "name": removed.name,
                    "panes": removed.panes,
                },
                "remaining_screens": mgr.list_screens().len(),
                "remaining_panes": mgr.total_panes(),
            }).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// List all screens with their panes and agent status
pub fn list_screens(app: &App) -> String {
    let mgr = app.screens.read().unwrap();
    let state = app.state.blocking_read();
    let screens = mgr.list_screens();

    let screen_data: Vec<serde_json::Value> = screens.iter().map(|s| {
        let pane_data: Vec<serde_json::Value> = s.panes.iter().map(|p: &u8| {
            let ps = state.panes.get(&p.to_string());
            json!({
                "pane": p,
                "theme": crate::config::theme_name(*p),
                "status": ps.map(|ps| ps.status.as_str()).unwrap_or("idle"),
                "project": ps.map(|ps| ps.project.as_str()).unwrap_or("--"),
                "task": ps.map(|ps| {
                    let t = &ps.task;
                    if t.len() > 40 { &t[..40] } else { t.as_str() }
                }).unwrap_or("--"),
                "role": ps.map(|ps| crate::config::role_short(&ps.role)).unwrap_or("--"),
            })
        }).collect();

        let active = pane_data.iter().filter(|p| p["status"] == "active").count();
        let idle = pane_data.len() - active;

        json!({
            "id": s.id,
            "name": s.name,
            "layout": s.layout,
            "pane_count": s.panes.len(),
            "active": active,
            "idle": idle,
            "panes": pane_data,
            "tmux_window": s.tmux_window,
        })
    }).collect();

    let total_active: usize = screen_data.iter()
        .map(|s| s["active"].as_u64().unwrap_or(0) as usize)
        .sum();

    json!({
        "screens": screen_data,
        "total_screens": screens.len(),
        "total_panes": mgr.total_panes(),
        "total_active": total_active,
        "total_idle": mgr.total_panes() as usize - total_active,
    }).to_string()
}

/// Get screen summary for the dashboard
pub fn screen_summary(app: &App) -> String {
    let mgr = app.screens.read().unwrap();
    mgr.summary().to_string()
}
