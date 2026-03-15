use serde_json::json;

use crate::agents::AgentType;
use crate::app::App;
use crate::session_controller;

use super::super::types::{
    SessionControlSendRequest, SessionControlStartRequest, SessionControlStatusRequest,
    SessionControlStopRequest,
};

pub async fn session_control_start(app: &App, req: SessionControlStartRequest) -> String {
    match app
        .session_controller
        .start(
            req.pane,
            req.mission,
            parse_agent_type(req.agent_type.as_deref()),
        )
        .await
    {
        Ok(watcher) => json!({
            "status": "started",
            "watcher": session_controller::watcher_to_value(&watcher),
        })
        .to_string(),
        Err(err) => json!({"error": err.to_string()}).to_string(),
    }
}

pub async fn session_control_stop(app: &App, req: SessionControlStopRequest) -> String {
    match app.session_controller.stop(&req.pane).await {
        Ok(Some(watcher)) => json!({
            "status": "stopped",
            "watcher": session_controller::watcher_to_value(&watcher),
        })
        .to_string(),
        Ok(None) => json!({
            "error": format!("pane '{}' is not supervised", req.pane),
        })
        .to_string(),
        Err(err) => json!({"error": err.to_string()}).to_string(),
    }
}

pub async fn session_control_status(app: &App, req: SessionControlStatusRequest) -> String {
    match app.session_controller.status(&req.pane).await {
        Some(watcher) => json!({
            "status": "ok",
            "watcher": session_controller::watcher_to_value(&watcher),
        })
        .to_string(),
        None => json!({
            "error": format!("pane '{}' is not supervised", req.pane),
        })
        .to_string(),
    }
}

pub async fn session_control_list(app: &App) -> String {
    let watchers = app.session_controller.list().await;
    session_controller::watchers_to_value(&watchers).to_string()
}

pub async fn session_control_send(app: &App, req: SessionControlSendRequest) -> String {
    match app
        .session_controller
        .send_instruction(&req.pane, &req.instruction)
        .await
    {
        Ok(()) => json!({
            "status": "sent",
            "pane": req.pane,
            "instruction": req.instruction,
        })
        .to_string(),
        Err(err) => json!({"error": err.to_string()}).to_string(),
    }
}

fn parse_agent_type(agent_type: Option<&str>) -> AgentType {
    match agent_type
        .unwrap_or("unknown")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "claude" | "claude_code" | "claudecode" => AgentType::ClaudeCode,
        "codex" | "codex_cli" | "codexcli" => AgentType::CodexCli,
        "gemini" | "gemini_cli" | "geminicli" => AgentType::GeminiCli,
        "opencode" | "open_code" => AgentType::OpenCode,
        _ => AgentType::Unknown,
    }
}
