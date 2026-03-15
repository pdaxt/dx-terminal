//! WebSocket handler for real-time bidirectional communication.
//!
//! Server → Client: sequenced deltas from RuntimeReplicator via EventBus
//! Client → Server: spawn, kill, talk, queue commands
//!
//! ## Architecture (post-replicator)
//!
//! - NO per-client tmux polling — the RuntimeReplicator does this once
//! - NO per-client JSONL tailing — the RuntimeReplicator uses cursor-based SessionTailer
//! - Each WS connection subscribes to EventBus and forwards sequenced events
//! - On connect: full snapshot + current seq number
//! - On lag: client detects seq gap and requests resync

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::app::App;
use crate::mcp::{tools, types};
use crate::session_stream;
use crate::state::events::{next_seq, StateEvent};
use crate::tmux;

type AppState = Arc<App>;
type WsSender = Arc<tokio::sync::Mutex<SplitSink<WebSocket, Message>>>;

/// GET /ws — Upgrade to WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade, State(app): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(socket: WebSocket, app: Arc<App>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to state events BEFORE building snapshot to avoid missing events
    let event_rx = app.state.event_bus.subscribe();

    // Send initial full state snapshot with current seq
    let snapshot = build_full_snapshot(&app).await;
    let current_seq = next_seq();
    let init_msg = json!({
        "type": "init",
        "seq": current_seq,
        "data": snapshot,
    });
    if sender
        .send(Message::Text(init_msg.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // Shared sender for multiple tasks
    let sender: WsSender = Arc::new(tokio::sync::Mutex::new(sender));

    // --- Task 1: Forward sequenced state events to client ---
    // This now includes OutputChunk, SessionEventChunk, PaneUpsert etc.
    // from the RuntimeReplicator — no per-client polling needed.
    let event_sender = Arc::clone(&sender);
    let event_app = Arc::clone(&app);
    let event_handle = tokio::spawn(forward_events(event_rx, event_sender, event_app));

    // --- Task 2: Forward sync events to client ---
    let sync_sender = Arc::clone(&sender);
    let sync_app = Arc::clone(&app);
    let sync_handle = tokio::spawn(forward_sync_events(sync_app, sync_sender));

    // --- Task 3: Receive commands from client ---
    let cmd_app = Arc::clone(&app);
    let cmd_sender = Arc::clone(&sender);
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(cmd) = serde_json::from_str::<Value>(&text) {
                    // Handle resync request
                    if cmd.get("cmd").and_then(|c| c.as_str()) == Some("resync") {
                        let snapshot = build_full_snapshot(&cmd_app).await;
                        let seq = next_seq();
                        let msg = json!({
                            "type": "init",
                            "seq": seq,
                            "data": snapshot,
                        });
                        let mut s = cmd_sender.lock().await;
                        if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                            break;
                        }
                        continue;
                    }

                    let result = handle_client_command(&cmd_app, &cmd).await;
                    let response = json!({
                        "type": "cmd_result",
                        "seq": next_seq(),
                        "cmd": cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("unknown"),
                        "result": result,
                    });
                    let mut s = cmd_sender.lock().await;
                    if s.send(Message::Text(response.to_string().into()))
                        .await
                        .is_err()
                    {
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
    sync_handle.abort();
}

/// Build complete state snapshot for initial connection.
/// Merges DX Terminal state with auto-discovered live tmux panes.
async fn build_full_snapshot(app: &App) -> Value {
    let state = app.state.get_state_snapshot().await;
    let live_panes = tokio::task::spawn_blocking(|| tmux::discover_live_panes())
        .await
        .unwrap_or_default();
    let resolved_panes = crate::runtime_panes::resolve_runtime_panes(&state, &live_panes, None);

    let mut panes = Vec::new();
    let themes = crate::config::all_themes();

    for resolved in resolved_panes {
        let pane_num = resolved.pane;
        let ps = &resolved.pane_state;
        let live = resolved.live.as_ref();
        let tmux_target = ps.tmux_target.clone();

        let pty_output = if tmux_target.is_none() {
            let pty = app.pty_lock();
            if pty.has_agent(pane_num) {
                let screen = pty.screen_text(pane_num).unwrap_or_default();
                if screen.trim().is_empty() {
                    pty.last_output(pane_num, 80).unwrap_or_default()
                } else {
                    screen
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let output = if let Some(ref target) = tmux_target {
            let t = target.clone();
            if live.is_some() || tmux::pane_exists(&t) {
                tokio::task::spawn_blocking(move || tmux::capture_output_extended(&t, 80))
                    .await
                    .unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            pty_output
        };

        let line_vec: Vec<&str> = output.lines().collect();
        let tail: String = line_vec
            .iter()
            .rev()
            .take(50)
            .rev()
            .copied()
            .collect::<Vec<&str>>()
            .join("\n");

        let theme_idx = (pane_num as usize).saturating_sub(1) % themes.len();
        let has_runtime_output =
            (tmux_target.is_some() || !output.trim().is_empty()) && !output.trim().is_empty();
        let status = if !ps.status.trim().is_empty() {
            ps.status.as_str()
        } else if has_runtime_output {
            "active"
        } else {
            "idle"
        };

        let project = if !ps.project.trim().is_empty() && ps.project != "--" {
            ps.project.clone()
        } else if let Some(lp) = live {
            if let Some(ref jp) = lp.jsonl_path {
                let jp_clone = jp.clone();
                let jsonl_cwd =
                    tokio::task::spawn_blocking(move || crate::tmux::read_jsonl_cwd(&jp_clone))
                        .await
                        .unwrap_or(None);
                if let Some(jcwd) = jsonl_cwd {
                    crate::runtime_panes::project_from_cwd(&jcwd)
                } else {
                    crate::runtime_panes::project_from_cwd(&lp.cwd)
                }
            } else {
                crate::runtime_panes::project_from_cwd(&lp.cwd)
            }
        } else {
            "--".to_string()
        };

        let provider = if let Some(lp) = live {
            tmux::infer_provider(&lp.command, &lp.window_name, lp.jsonl_path.as_deref()).to_string()
        } else if let Some(ref provider) = ps.provider {
            provider.clone()
        } else {
            "unknown".to_string()
        };

        let task = if !ps.task.trim().is_empty() {
            let task = &ps.task;
            if task.len() > 80 {
                task[..80].to_string()
            } else {
                task.clone()
            }
        } else if let Some(ref target) = tmux_target {
            format!("{} in {}", tmux::provider_label(&provider), target)
        } else {
            "--".to_string()
        };

        let role = if !ps.role.trim().is_empty() {
            crate::config::role_short(&ps.role).to_string()
        } else {
            tmux::provider_short(&provider).to_ascii_uppercase()
        };

        // JSONL session info
        let (jsonl_path, session_id) = if let Some(lp) = live {
            (lp.jsonl_path.clone(), lp.session_id.clone())
        } else {
            (None, None)
        };

        // Get last 20 structured events from JSONL
        let session_events = if let Some(ref jp) = jsonl_path {
            let jp_clone = jp.clone();
            tokio::task::spawn_blocking(move || session_stream::tail_session_events(&jp_clone, 20))
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        panes.push(json!({
            "pane": pane_num,
            "theme": themes[theme_idx].0,
            "status": status,
            "project": project,
            "task": task,
            "role": role,
            "dxos_session_id": ps.dxos_session_id.clone(),
            "provider": provider,
            "provider_label": tmux::provider_label(&provider),
            "provider_short": tmux::provider_short(&provider),
            "runtime_adapter": ps.runtime_adapter.clone().or_else(|| {
                if tmux_target.is_some() {
                    Some("tmux_migration_adapter".to_string())
                } else if !output.trim().is_empty() {
                    Some("pty_native_adapter".to_string())
                } else {
                    None
                }
            }),
            "output": tail,
            "pty_running": !output.trim().is_empty(),
            "line_count": line_vec.len(),
            "tmux_target": tmux_target,
            "live": live.is_some(),
            "jsonl_path": jsonl_path,
            "session_id": session_id,
            "events": session_events,
            "cwd": live.map(|l| l.cwd.clone()),
            "command": live.map(|l| l.command.clone()),
            "window_name": live.map(|l| l.window_name.clone()),
            "browser_port": crate::config::pane_browser_port(pane_num),
            "browser_profile_root": crate::config::pane_browser_profile_root(pane_num),
            "browser_artifacts_root": crate::config::pane_browser_artifacts_root(pane_num),
            "workspace_path": ps.workspace_path.clone(),
            "branch_name": ps.branch_name.clone(),
            "base_branch": ps.base_branch.clone(),
            "model": ps.model.clone(),
            "state_backed": resolved.state_backed,
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

    // Screens (from tmux sessions)
    let screens = {
        let mgr = app.screens.read().unwrap();
        let screen_list = mgr.list_screens();
        json!({
            "count": screen_list.len(),
            "names": screen_list.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
        })
    };

    let active_count = panes.iter().filter(|p| p["status"] == "active").count();
    let total_panes = panes.len();

    // Collect unique workspaces from all panes
    let mut workspaces: Vec<String> = panes
        .iter()
        .filter_map(|p| p["project"].as_str())
        .filter(|s| *s != "--")
        .map(|s| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    workspaces.sort();

    json!({
        "panes": panes,
        "queue": queue_summary,
        "screens": screens,
        "active_count": active_count,
        "total_panes": total_panes,
        "live_discovered": live_panes.len(),
        "workspaces": workspaces,
    })
}

/// Forward state events from EventBus → WebSocket with sequence numbers.
/// Includes all event types (OutputChunk, SessionEventChunk, PaneUpsert, etc.)
/// since the RuntimeReplicator now publishes through the EventBus.
async fn forward_events(mut rx: broadcast::Receiver<StateEvent>, sender: WsSender, _app: Arc<App>) {
    loop {
        match rx.recv().await {
            Ok(event) => {
                let seq = next_seq();
                let msg = match &event {
                    StateEvent::PaneUpsert { pane, data } => json!({
                        "type": "pane_upsert",
                        "seq": seq,
                        "pane": pane, "data": data,
                    }),
                    StateEvent::PaneRemoved { pane, reason } => json!({
                        "type": "pane_removed",
                        "seq": seq,
                        "pane": pane, "reason": reason,
                    }),
                    StateEvent::PaneSpawned {
                        pane,
                        project,
                        role,
                    } => json!({
                        "type": "pane_spawned",
                        "seq": seq,
                        "pane": pane, "project": project, "role": role,
                    }),
                    StateEvent::PaneKilled { pane, reason } => json!({
                        "type": "pane_killed",
                        "seq": seq,
                        "pane": pane, "reason": reason,
                    }),
                    StateEvent::PaneStatusChanged { pane, status } => json!({
                        "type": "pane_status",
                        "seq": seq,
                        "pane": pane, "status": status,
                    }),
                    StateEvent::OutputChunk {
                        pane,
                        output,
                        full_lines,
                        tmux_target,
                    } => json!({
                        "type": "terminal_output",
                        "seq": seq,
                        "updates": [{ "pane": pane, "output": output, "full_lines": full_lines, "tmux_target": tmux_target }],
                    }),
                    StateEvent::SessionEventChunk { pane, events } => json!({
                        "type": "session_events",
                        "seq": seq,
                        "updates": [{ "pane": pane, "events": events }],
                    }),
                    StateEvent::LogAppended {
                        pane,
                        event,
                        summary,
                    } => json!({
                        "type": "log",
                        "seq": seq,
                        "pane": pane, "event": event, "summary": summary,
                    }),
                    StateEvent::QueueUpsert { task_id, task } => json!({
                        "type": "queue_upsert",
                        "seq": seq,
                        "task_id": task_id, "task": task,
                    }),
                    StateEvent::QueueRemoved { task_id } => json!({
                        "type": "queue_removed",
                        "seq": seq,
                        "task_id": task_id,
                    }),
                    StateEvent::QueueChanged {
                        action,
                        task_id,
                        task,
                    } => json!({
                        "type": "queue",
                        "seq": seq,
                        "action": action, "task_id": task_id, "task": task,
                    }),
                    StateEvent::VisionChanged {
                        project,
                        summary,
                        feature_id,
                        feature_title,
                        phase,
                        state,
                        readiness,
                    } => json!({
                        "type": "vision_changed",
                        "seq": seq,
                        "project": project,
                        "summary": summary,
                        "feature_id": feature_id,
                        "feature_title": feature_title,
                        "phase": phase,
                        "state": state,
                        "readiness": readiness,
                    }),
                    StateEvent::DebateChanged {
                        project,
                        debate_id,
                        title,
                        status,
                        action,
                    } => json!({
                        "type": "debate_changed",
                        "seq": seq,
                        "project": project,
                        "debate_id": debate_id,
                        "title": title,
                        "status": status,
                        "action": action,
                    }),
                    StateEvent::SessionContractChanged {
                        project,
                        session_id,
                        role,
                        status,
                        action,
                    } => json!({
                        "type": "dxos_session_changed",
                        "seq": seq,
                        "project": project,
                        "session_id": session_id,
                        "role": role,
                        "status": status,
                        "action": action,
                    }),
                    StateEvent::WorkflowRunChanged {
                        project,
                        workflow_run_id,
                        workflow_id,
                        status,
                        action,
                    } => json!({
                        "type": "workflow_run_changed",
                        "seq": seq,
                        "project": project,
                        "workflow_run_id": workflow_run_id,
                        "workflow_id": workflow_id,
                        "status": status,
                        "action": action,
                    }),
                    StateEvent::AuditLogged {
                        project,
                        action_id,
                        kind,
                        target,
                        outcome,
                    } => json!({
                        "type": "audit_logged",
                        "seq": seq,
                        "project": project,
                        "action_id": action_id,
                        "kind": kind,
                        "target": target,
                        "outcome": outcome,
                    }),
                    StateEvent::SyncStatusChanged { project, data } => json!({
                        "type": "sync_status",
                        "seq": seq,
                        "project": project, "data": data,
                    }),
                    StateEvent::StateRefreshed => json!({
                        "type": "refresh",
                        "seq": seq,
                    }),
                };
                let mut s = sender.lock().await;
                if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Notify client of lag so it can request resync
                tracing::debug!("WS event stream lagged by {} events", n);
                let msg = json!({
                    "type": "lagged",
                    "seq": next_seq(),
                    "missed": n,
                });
                let mut s = sender.lock().await;
                if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Forward sync events (file changes, git commits, pushes) to WebSocket client
async fn forward_sync_events(app: Arc<App>, sender: WsSender) {
    let sync_rx = {
        let sync_mgr = app.sync_manager.read().unwrap();
        sync_mgr.as_ref().map(|mgr| mgr.event_tx.subscribe())
    };

    let mut rx: broadcast::Receiver<crate::sync::SyncEvent> = match sync_rx {
        Some(rx) => rx,
        None => {
            // No sync manager — just sleep forever
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }
    };

    loop {
        match rx.recv().await {
            Ok(event) => {
                let msg = json!({
                    "type": "sync_event",
                    "seq": next_seq(),
                    "event": serde_json::to_value(&event).unwrap_or(json!(null)),
                });
                let mut s = sender.lock().await;
                if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("WS sync event stream lagged by {} events", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Handle commands received from web client
async fn handle_client_command(app: &App, cmd: &Value) -> Value {
    let cmd_type = cmd.get("cmd").and_then(|c| c.as_str()).unwrap_or("");

    match cmd_type {
        "spawn" => {
            let pane = cmd
                .get("pane")
                .and_then(|p| p.as_str())
                .unwrap_or("auto")
                .to_string();
            let project = cmd
                .get("project")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let role = cmd
                .get("role")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            let provider = cmd
                .get("provider")
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());
            let model = cmd
                .get("model")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let runtime_adapter = cmd
                .get("runtime_adapter")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let client_request_id = cmd
                .get("client_request_id")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let feature_id = cmd
                .get("feature_id")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let stage = cmd
                .get("stage")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let supervisor_session_id = cmd
                .get("supervisor_session_id")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let task = cmd
                .get("task")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            if project.is_empty() {
                return json!({"error": "project required"});
            }
            let result = tools::spawn(
                app,
                types::SpawnRequest {
                    pane,
                    project,
                    session_id: None,
                    role,
                    provider,
                    model,
                    runtime_adapter,
                    client_request_id,
                    feature_id,
                    stage,
                    supervisor_session_id,
                    task,
                    prompt: None,
                    autonomous: None,
                },
            )
            .await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "kill" => {
            let pane = cmd
                .get("pane")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let reason = cmd
                .get("reason")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            if pane.is_empty() {
                return json!({"error": "pane required"});
            }
            let result = tools::kill(app, types::KillRequest { pane, reason }).await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "talk" => {
            let pane = cmd.get("pane").and_then(|p| p.as_u64()).unwrap_or(0) as u8;
            let message = cmd
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            if pane == 0 || message.is_empty() {
                return json!({"error": "pane (number) and message required"});
            }
            let pane_data = app.state.get_pane(pane).await;
            if let Some(target) = pane_data
                .tmux_target
                .filter(|target| tmux::pane_exists(target))
            {
                match tokio::task::spawn_blocking(move || tmux::send_command(&target, &message))
                    .await
                {
                    Ok(Ok(())) => {
                        json!({"status": "sent", "pane": pane, "runtime_adapter": "tmux_migration_adapter"})
                    }
                    Ok(Err(e)) => json!({"error": format!("{}", e)}),
                    Err(e) => json!({"error": format!("task join error: {}", e)}),
                }
            } else {
                let send_result = {
                    let mut pty = app.pty_lock();
                    pty.send_line(pane, &message)
                };
                match send_result {
                    Ok(()) => {
                        json!({"status": "sent", "pane": pane, "runtime_adapter": "pty_native_adapter"})
                    }
                    Err(e) => json!({"error": format!("{}", e)}),
                }
            }
        }
        "queue_add" => {
            let project = cmd
                .get("project")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            let task = cmd
                .get("task")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let role = cmd
                .get("role")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            let priority = cmd
                .get("priority")
                .and_then(|p| p.as_u64())
                .map(|p| p as u8);
            if project.is_empty() || task.is_empty() {
                return json!({"error": "project and task required"});
            }
            let result = tools::queue_add(
                app,
                types::QueueAddRequest {
                    project,
                    task,
                    role,
                    priority,
                    prompt: None,
                    depends_on: None,
                    max_retries: None,
                },
            )
            .await;
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "screen_add" => {
            let name = cmd
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            let layout = cmd
                .get("layout")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string());
            let panes = cmd.get("panes").and_then(|p| p.as_u64()).map(|p| p as u8);
            let result = tools::screen_tools::add_screen(app, name, layout, panes);
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "screen_rm" => {
            let screen_ref = cmd
                .get("screen")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let force = cmd.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            if screen_ref.is_empty() {
                return json!({"error": "screen name/id required"});
            }
            let result = tools::screen_tools::remove_screen(app, screen_ref, force);
            serde_json::from_str(&result).unwrap_or(json!({"raw": result}))
        }
        "capture" => {
            let pane = cmd.get("pane").and_then(|p| p.as_u64()).unwrap_or(0) as u8;
            if pane == 0 {
                return json!({"error": "pane number required"});
            }
            let pane_data = app.state.get_pane(pane).await;
            if let Some(target) = pane_data
                .tmux_target
                .filter(|target| tmux::pane_exists(target))
            {
                let output = tokio::task::spawn_blocking(move || tmux::capture_output(&target))
                    .await
                    .unwrap_or_default();
                json!({"pane": pane, "output": output, "lines": output.lines().count(), "runtime_adapter": "tmux_migration_adapter"})
            } else {
                let output = {
                    let pty = app.pty_lock();
                    pty.screen_text(pane)
                        .or_else(|| pty.last_output(pane, 80))
                        .unwrap_or_default()
                };
                json!({"pane": pane, "output": output, "lines": output.lines().count(), "runtime_adapter": "pty_native_adapter"})
            }
        }
        _ => json!({"error": format!("unknown command: {}", cmd_type)}),
    }
}
