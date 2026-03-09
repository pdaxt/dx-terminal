//! WebSocket handler for real-time bidirectional communication.
//!
//! Server → Client: terminal output diffs, state events, pane status
//! Client → Server: spawn, kill, talk, queue commands

use std::sync::Arc;
use std::collections::HashMap;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::app::App;
use crate::state::events::StateEvent;
use crate::tmux;
use crate::mcp::{tools, types};

type AppState = Arc<App>;
type WsSender = Arc<tokio::sync::Mutex<SplitSink<WebSocket, Message>>>;

/// GET /ws — Upgrade to WebSocket
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(socket: WebSocket, app: Arc<App>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to state events
    let event_rx = app.state.event_bus.subscribe();

    // Send initial full state snapshot
    let snapshot = build_full_snapshot(&app).await;
    let init_msg = json!({
        "type": "init",
        "data": snapshot,
    });
    if sender.send(Message::Text(init_msg.to_string().into())).await.is_err() {
        return;
    }

    // Shared sender for multiple tasks
    let sender: WsSender = Arc::new(tokio::sync::Mutex::new(sender));

    // --- Task 1: Forward state events to client ---
    let event_sender = Arc::clone(&sender);
    let event_handle = tokio::spawn(forward_events(event_rx, event_sender));

    // --- Task 2: Poll tmux pane output every 1s, push diffs ---
    let poll_sender = Arc::clone(&sender);
    let poll_app = Arc::clone(&app);
    let poll_handle = tokio::spawn(poll_terminal_output(poll_app, poll_sender));

    // --- Task 3: Receive commands from client ---
    let cmd_app = Arc::clone(&app);
    let cmd_sender = Arc::clone(&sender);
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(cmd) = serde_json::from_str::<Value>(&text) {
                    let result = handle_client_command(&cmd_app, &cmd).await;
                    let response = json!({
                        "type": "cmd_result",
                        "cmd": cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("unknown"),
                        "result": result,
                    });
                    let mut s = cmd_sender.lock().await;
                    if s.send(Message::Text(response.to_string().into())).await.is_err() {
                        break;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup
    event_handle.abort();
    poll_handle.abort();
}

/// Build complete state snapshot for initial connection
async fn build_full_snapshot(app: &App) -> Value {
    // Use async state read instead of blocking_read()
    let state = app.state.get_state_snapshot().await;
    let max_panes = crate::config::pane_count();

    let mut panes = Vec::new();
    for i in 1..=max_panes {
        let ps = state.panes.get(&i.to_string());
        let tmux_target = format!("claude6:{}.{}", (i - 1) / 3, (i - 1) % 3);

        // Capture current terminal output via spawn_blocking (tmux is sync)
        let target = tmux_target.clone();
        let output = tokio::task::spawn_blocking(move || {
            tmux::capture_output(&target)
        }).await.unwrap_or_default();

        let lines: Vec<&str> = output.lines().collect();
        let tail: String = lines.iter().rev().take(50).rev()
            .copied().collect::<Vec<&str>>().join("\n");

        panes.push(json!({
            "pane": i,
            "theme": crate::config::theme_name(i),
            "status": ps.map(|p| p.status.as_str()).unwrap_or("idle"),
            "project": ps.map(|p| p.project.as_str()).unwrap_or("--"),
            "task": ps.map(|p| {
                let t = &p.task;
                if t.len() > 80 { &t[..80] } else { t.as_str() }
            }).unwrap_or("--"),
            "role": ps.map(|p| crate::config::role_short(&p.role)).unwrap_or("--"),
            "output": tail,
            "line_count": lines.len(),
        }));
    }

    // Queue
    let q = crate::queue::load_queue();
    let queue_summary = json!({
        "pending": q.tasks.iter().filter(|t| t.status == crate::queue::QueueStatus::Pending).count(),
        "running": q.tasks.iter().filter(|t| t.status == crate::queue::QueueStatus::Running).count(),
        "done": q.tasks.iter().filter(|t| t.status == crate::queue::QueueStatus::Done).count(),
        "failed": q.tasks.iter().filter(|t| t.status == crate::queue::QueueStatus::Failed).count(),
    });

    // Screens
    let screens = {
        let mgr = app.screens.read().unwrap();
        let screen_list = mgr.list_screens();
        json!({
            "count": screen_list.len(),
            "names": screen_list.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        })
    };

    json!({
        "panes": panes,
        "queue": queue_summary,
        "screens": screens,
        "active_count": panes.iter().filter(|p| p["status"] == "active").count(),
        "total_panes": max_panes,
    })
}

/// Forward state events from EventBus → WebSocket
async fn forward_events(
    mut rx: broadcast::Receiver<StateEvent>,
    sender: WsSender,
) {
    loop {
        match rx.recv().await {
            Ok(event) => {
                let msg = match &event {
                    StateEvent::PaneSpawned { pane, project, role } => json!({
                        "type": "pane_spawned",
                        "pane": pane, "project": project, "role": role,
                    }),
                    StateEvent::PaneKilled { pane, reason } => json!({
                        "type": "pane_killed",
                        "pane": pane, "reason": reason,
                    }),
                    StateEvent::PaneStatusChanged { pane, status } => json!({
                        "type": "pane_status",
                        "pane": pane, "status": status,
                    }),
                    StateEvent::LogAppended { pane, event, summary } => json!({
                        "type": "log",
                        "pane": pane, "event": event, "summary": summary,
                    }),
                    StateEvent::QueueChanged { action, task_id, task } => json!({
                        "type": "queue",
                        "action": action, "task_id": task_id, "task": task,
                    }),
                    StateEvent::StateRefreshed => json!({"type": "refresh"}),
                };
                let mut s = sender.lock().await;
                if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("WS event stream lagged by {} events", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Poll tmux pane output every 1s and push diffs to WebSocket
async fn poll_terminal_output(
    app: Arc<App>,
    sender: WsSender,
) {
    let mut prev_outputs: HashMap<u8, String> = HashMap::new();
    let interval = tokio::time::Duration::from_secs(1);

    loop {
        tokio::time::sleep(interval).await;

        // Use async state read instead of blocking_read()
        let state = app.state.get_state_snapshot().await;
        let max_panes = crate::config::pane_count();

        // Collect which panes are active and their tmux targets
        let mut active_panes: Vec<(u8, String)> = Vec::new();
        for i in 1..=max_panes {
            let ps = state.panes.get(&i.to_string());
            let is_active = ps.map(|p| p.status == "active").unwrap_or(false);
            if is_active {
                let tmux_target = format!("claude6:{}.{}", (i - 1) / 3, (i - 1) % 3);
                active_panes.push((i, tmux_target));
            }
        }

        if active_panes.is_empty() {
            continue;
        }

        // Capture all active pane outputs via spawn_blocking
        let captures: Vec<(u8, String)> = tokio::task::spawn_blocking(move || {
            active_panes.iter().map(|(i, target)| {
                (*i, tmux::capture_output(target))
            }).collect()
        }).await.unwrap_or_default();

        let mut updates = Vec::new();
        for (i, output) in captures {
            let prev = prev_outputs.get(&i).map(|s| s.as_str()).unwrap_or("");
            if output != prev {
                // Extract only the new lines (diff)
                let new_lines = if output.len() > prev.len() && output.starts_with(prev) {
                    output[prev.len()..].to_string()
                } else {
                    // Output scrolled/changed completely — send last 30 lines
                    let lines: Vec<&str> = output.lines().collect();
                    let tail_start = lines.len().saturating_sub(30);
                    lines[tail_start..].join("\n")
                };

                if !new_lines.trim().is_empty() {
                    updates.push(json!({
                        "pane": i,
                        "output": new_lines,
                        "full_lines": output.lines().count(),
                    }));
                }

                prev_outputs.insert(i, output);
            }
        }

        if !updates.is_empty() {
            let msg = json!({
                "type": "terminal_output",
                "updates": updates,
            });
            let mut s = sender.lock().await;
            if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                break;
            }
        }
    }
}

/// Handle commands received from web client
async fn handle_client_command(app: &App, cmd: &Value) -> Value {
    let cmd_type = cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("");

    match cmd_type {
        "spawn" => {
            let pane = cmd.get("pane").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let project = cmd.get("project").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let role = cmd.get("role").and_then(|r| r.as_str()).map(|s| s.to_string());
            let task = cmd.get("task").and_then(|t| t.as_str()).map(|s| s.to_string());
            if pane.is_empty() || project.is_empty() {
                return json!({"error": "pane and project required"});
            }
            let result = tools::spawn(app, types::SpawnRequest {
                pane, project, role, task, prompt: None,
            }).await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "kill" => {
            let pane = cmd.get("pane").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let reason = cmd.get("reason").and_then(|r| r.as_str()).map(|s| s.to_string());
            if pane.is_empty() {
                return json!({"error": "pane required"});
            }
            let result = tools::kill(app, types::KillRequest { pane, reason }).await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "talk" => {
            let pane = cmd.get("pane").and_then(|p| p.as_u64()).unwrap_or(0) as u8;
            let message = cmd.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            if pane == 0 || message.is_empty() {
                return json!({"error": "pane (number) and message required"});
            }
            // Send via tmux in spawn_blocking
            let target = format!("claude6:{}.{}", (pane - 1) / 3, (pane - 1) % 3);
            match tokio::task::spawn_blocking(move || {
                tmux::send_command(&target, &message)
            }).await {
                Ok(Ok(())) => json!({"status": "sent", "pane": pane}),
                Ok(Err(e)) => json!({"error": format!("{}", e)}),
                Err(e) => json!({"error": format!("task join error: {}", e)}),
            }
        }
        "queue_add" => {
            let project = cmd.get("project").and_then(|p| p.as_str()).unwrap_or("").to_string();
            let task = cmd.get("task").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let role = cmd.get("role").and_then(|r| r.as_str()).map(|s| s.to_string());
            let priority = cmd.get("priority").and_then(|p| p.as_u64()).map(|p| p as u8);
            if project.is_empty() || task.is_empty() {
                return json!({"error": "project and task required"});
            }
            let result = tools::queue_add(app, types::QueueAddRequest {
                project, task, role, priority, prompt: None, depends_on: None, max_retries: None,
            }).await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "screen_add" => {
            let name = cmd.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
            let layout = cmd.get("layout").and_then(|l| l.as_str()).map(|s| s.to_string());
            let panes = cmd.get("panes").and_then(|p| p.as_u64()).map(|p| p as u8);
            let result = tools::screen_tools::add_screen(app, name, layout, panes);
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "screen_rm" => {
            let screen_ref = cmd.get("screen").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let force = cmd.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            if screen_ref.is_empty() {
                return json!({"error": "screen name/id required"});
            }
            let result = tools::screen_tools::remove_screen(app, screen_ref, force);
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "capture" => {
            // On-demand full capture of a specific pane via spawn_blocking
            let pane = cmd.get("pane").and_then(|p| p.as_u64()).unwrap_or(0) as u8;
            if pane == 0 {
                return json!({"error": "pane number required"});
            }
            let target = format!("claude6:{}.{}", (pane - 1) / 3, (pane - 1) % 3);
            let output = tokio::task::spawn_blocking(move || {
                tmux::capture_output(&target)
            }).await.unwrap_or_default();
            json!({"pane": pane, "output": output, "lines": output.lines().count()})
        }
        _ => json!({"error": format!("unknown command: {}", cmd_type)}),
    }
}
