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
use crate::session_stream;
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

/// Build complete state snapshot for initial connection.
/// Merges DX Terminal state with auto-discovered live tmux panes.
async fn build_full_snapshot(app: &App) -> Value {
    let state = app.state.get_state_snapshot().await;
    let max_panes = crate::config::pane_count();

    // Auto-discover all live Claude panes across all tmux sessions
    let live_panes = tokio::task::spawn_blocking(|| {
        tmux::discover_live_panes()
    }).await.unwrap_or_default();

    let mut panes = Vec::new();

    // Map: first use DX Terminal state panes, then overlay/extend with live discovery
    let themes = crate::config::all_themes();
    let total_panes = std::cmp::max(max_panes as usize, live_panes.len());

    for i in 0..total_panes {
        let pane_num = (i + 1) as u8;
        let ps = state.panes.get(&pane_num.to_string());

        // Determine tmux target: prefer state, fall back to discovered live pane
        let (tmux_target, live) = if let Some(ref p) = ps {
            if let Some(ref t) = p.tmux_target {
                if tmux::pane_exists(t) {
                    (Some(t.clone()), None)
                } else if i < live_panes.len() {
                    (Some(live_panes[i].target.clone()), Some(&live_panes[i]))
                } else {
                    (None, None)
                }
            } else if i < live_panes.len() {
                (Some(live_panes[i].target.clone()), Some(&live_panes[i]))
            } else {
                (None, None)
            }
        } else if i < live_panes.len() {
            (Some(live_panes[i].target.clone()), Some(&live_panes[i]))
        } else {
            (None, None)
        };

        // Capture output
        let output = if let Some(ref target) = tmux_target {
            let t = target.clone();
            tokio::task::spawn_blocking(move || {
                tmux::capture_output_extended(&t, 80)
            }).await.unwrap_or_default()
        } else {
            String::new()
        };

        let line_vec: Vec<&str> = output.lines().collect();
        let tail: String = line_vec.iter().rev().take(50).rev()
            .copied().collect::<Vec<&str>>().join("\n");

        let theme_idx = i % themes.len();
        let status = if tmux_target.is_some() && !output.trim().is_empty() {
            if let Some(ref p) = ps { p.status.as_str() } else { "active" }
        } else {
            if let Some(ref p) = ps { p.status.as_str() } else { "idle" }
        };

        // Project: prefer JSONL cwd (most accurate), then tmux cwd, then state
        let project = if let Some(lp) = live {
            // If we have a JSONL session, its cwd might be more specific
            if let Some(ref jp) = lp.jsonl_path {
                // Read the JSONL header cwd (session start dir)
                let jp_clone = jp.clone();
                let jsonl_cwd = tokio::task::spawn_blocking(move || {
                    crate::tmux::read_jsonl_cwd(&jp_clone)
                }).await.unwrap_or(None);
                if let Some(jcwd) = jsonl_cwd {
                    project_from_cwd(&jcwd)
                } else {
                    project_from_cwd(&lp.cwd)
                }
            } else {
                project_from_cwd(&lp.cwd)
            }
        } else if let Some(ref p) = ps {
            p.project.clone()
        } else {
            "--".to_string()
        };

        let task = if let Some(ref p) = ps {
            let t = &p.task;
            if t.len() > 80 { t[..80].to_string() } else { t.clone() }
        } else if let Some(lp) = live {
            format!("Claude in {}", lp.target)
        } else {
            "--".to_string()
        };

        let role = if let Some(ref p) = ps {
            crate::config::role_short(&p.role).to_string()
        } else {
            "AG".to_string()  // Agent
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
            tokio::task::spawn_blocking(move || {
                session_stream::tail_session_events(&jp_clone, 20)
            }).await.unwrap_or_default()
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
            "output": tail,
            "line_count": line_vec.len(),
            "tmux_target": tmux_target,
            "live": live.is_some(),
            "jsonl_path": jsonl_path,
            "session_id": session_id,
            "events": session_events,
            "cwd": live.map(|l| l.cwd.clone()),
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

    // Collect unique workspaces from all panes
    let mut workspaces: Vec<String> = panes.iter()
        .filter_map(|p| p["project"].as_str())
        .filter(|s| *s != "--")
        .map(|s| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter().collect();
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

/// Extract project name from a working directory path.
/// e.g. "/Users/pran/Projects/dataxlr8-workspace" → "dataxlr8-workspace"
/// e.g. "/Users/pran/Projects" → "Projects"
fn project_from_cwd(cwd: &str) -> String {
    let path = std::path::Path::new(cwd);
    // If it's a direct child of ~/Projects, use the folder name
    // If it IS ~/Projects, use "Projects" (root workspace)
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cwd.to_string())
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

/// Poll tmux pane output every 1s and push diffs to WebSocket.
/// Auto-discovers live Claude panes across ALL tmux sessions.
async fn poll_terminal_output(
    app: Arc<App>,
    sender: WsSender,
) {
    // Key by pane_num for state-managed panes, or by tmux target for discovered ones
    let mut prev_outputs: HashMap<String, String> = HashMap::new();
    let interval = tokio::time::Duration::from_secs(1);

    loop {
        tokio::time::sleep(interval).await;

        let state = app.state.get_state_snapshot().await;
        let max_panes = crate::config::pane_count();

        // Collect targets: first from state, then merge with live discovery
        let mut pane_targets: Vec<(u8, String)> = Vec::new();

        // 1) State-managed panes with tmux targets
        for i in 1..=max_panes {
            if let Some(p) = state.panes.get(&i.to_string()) {
                if let Some(ref target) = p.tmux_target {
                    pane_targets.push((i, target.clone()));
                }
            }
        }

        // 2) Auto-discover live Claude panes from ALL tmux sessions
        let live_panes = tokio::task::spawn_blocking(|| {
            tmux::discover_live_panes()
        }).await.unwrap_or_default();

        // Merge discovered panes — assign pane numbers beyond state-managed ones
        let mut used_targets: std::collections::HashSet<String> = pane_targets.iter()
            .map(|(_, t)| t.clone()).collect();
        let mut next_pane = max_panes + 1;
        for lp in &live_panes {
            if !used_targets.contains(&lp.target) {
                pane_targets.push((next_pane, lp.target.clone()));
                used_targets.insert(lp.target.clone());
                next_pane += 1;
            }
        }

        // Also add discovered panes that match state panes with no target
        for i in 1..=max_panes {
            let has_target = pane_targets.iter().any(|(p, _)| *p == i);
            if !has_target && (i as usize) <= live_panes.len() {
                let lp = &live_panes[(i as usize) - 1];
                if !used_targets.contains(&lp.target) {
                    pane_targets.push((i, lp.target.clone()));
                    used_targets.insert(lp.target.clone());
                }
            }
        }

        if pane_targets.is_empty() {
            continue;
        }

        // Capture all pane outputs via spawn_blocking
        let captures: Vec<(u8, String, String)> = tokio::task::spawn_blocking(move || {
            pane_targets.iter().map(|(i, target)| {
                (*i, target.clone(), tmux::capture_output(target))
            }).collect()
        }).await.unwrap_or_default();

        let mut updates = Vec::new();
        for (pane_num, target, output) in captures {
            let key = format!("{}:{}", pane_num, target);
            let prev = prev_outputs.get(&key).map(|s| s.as_str()).unwrap_or("");
            if output != prev {
                // Extract diff
                let new_lines = if output.len() > prev.len() && output.starts_with(prev) {
                    output[prev.len()..].to_string()
                } else {
                    let lines: Vec<&str> = output.lines().collect();
                    let tail_start = lines.len().saturating_sub(30);
                    lines[tail_start..].join("\n")
                };

                if !new_lines.trim().is_empty() {
                    updates.push(json!({
                        "pane": pane_num,
                        "output": new_lines,
                        "full_lines": output.lines().count(),
                        "tmux_target": target,
                    }));
                }

                prev_outputs.insert(key, output);
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

        // --- JSONL session event streaming ---
        // Build pane→jsonl mapping from live panes
        let mut jsonl_polls: Vec<(u8, String)> = Vec::new();
        for (idx, lp) in live_panes.iter().enumerate() {
            if let Some(ref jp) = lp.jsonl_path {
                let pane_num = if idx < max_panes as usize {
                    (idx + 1) as u8
                } else {
                    max_panes + 1 + idx as u8
                };
                jsonl_polls.push((pane_num, jp.clone()));
            }
        }

        if !jsonl_polls.is_empty() {
            // Poll new JSONL events (use a static-ish tailer per connection)
            // For simplicity, we re-read last 5 events each cycle
            // (SessionTailer would be better but needs persistent state)
            let session_updates: Vec<Value> = tokio::task::spawn_blocking(move || {
                let mut results = Vec::new();
                for (pane_num, jp) in &jsonl_polls {
                    let events = session_stream::tail_session_events(jp, 5);
                    if !events.is_empty() {
                        results.push(json!({
                            "pane": pane_num,
                            "events": events,
                        }));
                    }
                }
                results
            }).await.unwrap_or_default();

            if !session_updates.is_empty() {
                let msg = json!({
                    "type": "session_events",
                    "updates": session_updates,
                });
                let mut s = sender.lock().await;
                if s.send(Message::Text(msg.to_string().into())).await.is_err() {
                    break;
                }
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
                pane, project, role, task, prompt: None, autonomous: None,
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
            // Get tmux target: first from state, then from live discovery
            let target = resolve_pane_target(app, pane).await;
            let target = match target {
                Some(t) => t,
                None => return json!({"error": format!("pane {} has no tmux target", pane)}),
            };
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
            let target = resolve_pane_target(app, pane).await;
            let target = match target {
                Some(t) => t,
                None => return json!({"pane": pane, "output": "", "lines": 0, "error": "no tmux target"}),
            };
            let output = tokio::task::spawn_blocking(move || {
                tmux::capture_output(&target)
            }).await.unwrap_or_default();
            json!({"pane": pane, "output": output, "lines": output.lines().count()})
        }
        _ => json!({"error": format!("unknown command: {}", cmd_type)}),
    }
}

/// Resolve a pane number to its tmux target.
/// First checks state, then falls back to auto-discovered live panes.
async fn resolve_pane_target(app: &App, pane: u8) -> Option<String> {
    // 1) Check state
    let pane_data = app.state.get_pane(pane).await;
    if let Some(ref t) = pane_data.tmux_target {
        if tmux::pane_exists(t) {
            return Some(t.clone());
        }
    }

    // 2) Fall back to live discovery (pane number maps to index)
    let live = tokio::task::spawn_blocking(|| tmux::discover_live_panes())
        .await.unwrap_or_default();

    let idx = (pane as usize).wrapping_sub(1);
    if idx < live.len() {
        Some(live[idx].target.clone())
    } else {
        None
    }
}
