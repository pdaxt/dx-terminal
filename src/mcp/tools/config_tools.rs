//! Configuration tools: set_mcps, set_preamble, config_show.

use super::super::types::*;
use super::helpers::json_err;
use crate::app::App;
use crate::claude;
use crate::config;

/// Execute os_set_mcps logic
pub async fn set_mcps(app: &App, req: SetMcpsRequest) -> String {
    app.state
        .set_project_mcps(&req.project, req.mcps.clone())
        .await;

    let project_path = config::resolve_project_path(&req.project);
    match claude::set_project_mcps(&project_path, &req.mcps) {
        Ok(()) => serde_json::json!({
            "status": "ok",
            "project": req.project,
            "mcps": req.mcps,
            "project_path": project_path,
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "status": "partial",
            "state_updated": true,
            "claude_json_error": e.to_string(),
        })
        .to_string(),
    }
}

/// Execute os_set_preamble logic
pub async fn set_preamble(_app: &App, req: SetPreambleRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    match claude::write_preamble(pane_num, &req.content) {
        Ok(path) => serde_json::json!({
            "status": "ok",
            "pane": pane_num,
            "path": path,
            "size": req.content.len(),
        })
        .to_string(),
        Err(e) => json_err(&format!("Failed to write preamble: {}", e)),
    }
}

/// Execute os_config_show logic
pub async fn config_show(app: &App, req: ConfigShowRequest) -> String {
    if let Some(pane_ref) = &req.pane {
        if !pane_ref.is_empty() {
            let pane_num = match config::resolve_pane(pane_ref) {
                Some(n) => n,
                None => return json_err(&format!("Invalid pane: {}", pane_ref)),
            };
            let pane_data = app.state.get_pane(pane_num).await;
            let mcps = app.state.get_project_mcps(&pane_data.project).await;
            let (has_pty, running) = {
                let pty = app.pty_lock();
                (pty.has_agent(pane_num), pty.is_running(pane_num))
            };

            return serde_json::json!({
                "pane": pane_num,
                "theme": config::theme_name(pane_num),
                "project": pane_data.project,
                "project_path": pane_data.project_path,
                "role": pane_data.role,
                "task": pane_data.task,
                "status": pane_data.status,
                "pty_active": has_pty,
                "pty_running": running,
                "preamble_exists": claude::preamble_exists(pane_num),
                "project_mcps": mcps,
            })
            .to_string();
        }
    }

    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let pty = app.pty_lock();
    let mut result = serde_json::Map::new();
    for (i, pd) in &pane_states {
        result.insert(
            i.to_string(),
            serde_json::json!({
                "theme": config::theme_name(*i),
                "project": pd.project,
                "role": pd.role,
                "task": pd.task,
                "status": pd.status,
                "pty_active": pty.has_agent(*i),
            }),
        );
    }
    drop(pty);
    serde_json::Value::Object(result).to_string()
}
