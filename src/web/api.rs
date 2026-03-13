use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path as FsPath, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::app::App;
use crate::capacity;
use crate::config;
use crate::mcp::{tools, types};
use crate::queue;

type AppState = Arc<App>;
type ApiJson = Result<Json<Value>, (StatusCode, Json<Value>)>;

/// Parse MCP tool JSON string result into Value, logging failures
fn parse_mcp(result: &str) -> Value {
    serde_json::from_str(result).unwrap_or_else(|e| {
        tracing::warn!(
            "MCP tool returned unparseable JSON: {} (input: {})",
            e,
            &result[..result.len().min(200)]
        );
        json!({"error": "parse failed", "raw": &result[..result.len().min(500)]})
    })
}

fn maybe_emit_vision_change(
    app: &AppState,
    project_path: &str,
    result: &str,
    feature_id: Option<&str>,
) {
    crate::vision_events::emit_from_result(app.as_ref(), project_path, result, feature_id);
}

fn maybe_emit_focus_change(app: &AppState, focus: &crate::vision_focus::VisionFocusEntry) {
    let project = focus.project.clone().unwrap_or_else(|| {
        std::path::Path::new(&focus.project_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "--".to_string())
    });
    app.state
        .event_bus
        .send(crate::state::events::StateEvent::VisionChanged {
            project,
            summary: "Focus updated".to_string(),
            feature_id: focus.feature_id.clone(),
            feature_title: None,
            phase: None,
            state: None,
            readiness: None,
        });
}

fn maybe_emit_debate_change(app: &AppState, project_path: &str, result: &str) {
    if let Some(event) = crate::dxos::debate_event_from_result(project_path, result) {
        app.state.event_bus.send(event);
    }
}

fn maybe_emit_dxos_session_change(app: &AppState, project_path: &str, result: &str) {
    if let Some(event) = crate::dxos::session_event_from_result(project_path, result) {
        app.state.event_bus.send(event);
    }
}

fn control_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get("x-dx-control-token")
        .and_then(|value| value.to_str().ok())
    {
        let token = value.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn require_control_token(headers: &HeaderMap) -> Result<(), (StatusCode, Json<Value>)> {
    let Some(expected) = crate::config::control_token() else {
        return Ok(());
    };
    let provided = control_token_from_headers(headers);
    if provided.as_deref() == Some(expected.as_str()) {
        return Ok(());
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "DX control token required",
            "control_auth": crate::dxos::control_auth_contract(),
        })),
    ))
}

fn control_actor(headers: &HeaderMap) -> String {
    headers
        .get("x-dx-actor")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("portal-operator")
        .to_string()
}

fn deny_control_action(
    app: &AppState,
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    action_kind: &str,
    target: &str,
    status: StatusCode,
    message: &str,
) -> (StatusCode, Json<Value>) {
    let value = json!({
        "error": message,
        "actor": actor,
        "action_kind": action_kind,
        "target": target,
        "control_auth": crate::dxos::control_auth_contract(),
    });
    if !project_path.trim().is_empty() {
        if let Ok(record) = crate::dxos::append_audit_record(
            project_path,
            project_name,
            actor,
            action_kind,
            target,
            "denied",
            message,
            value.clone(),
        ) {
            app.state
                .event_bus
                .send(crate::state::events::StateEvent::AuditLogged {
                    project: record.project_name,
                    action_id: record.id,
                    kind: record.action_kind,
                    target: record.target,
                    outcome: record.outcome,
                });
        }
    }
    (status, Json(value))
}

fn require_control_access(
    app: &AppState,
    headers: &HeaderMap,
    project_path: &str,
    project_name: Option<&str>,
    action_kind: &str,
    target: &str,
) -> Result<String, (StatusCode, Json<Value>)> {
    if let Err(error) = require_control_token(headers) {
        return Err(error);
    }
    let actor = control_actor(headers);
    if let Err(error) =
        crate::dxos::authorize_operator_action(project_path, project_name, &actor, action_kind)
    {
        return Err(deny_control_action(
            app,
            project_path,
            project_name,
            &actor,
            action_kind,
            target,
            StatusCode::FORBIDDEN,
            &error,
        ));
    }
    Ok(actor)
}

fn record_control_action(
    app: &AppState,
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    action_kind: &str,
    target: &str,
    result: &Value,
) {
    if project_path.trim().is_empty() {
        return;
    }
    let outcome = if result.get("error").is_some() {
        "error"
    } else {
        "ok"
    };
    let summary = if outcome == "ok" {
        format!("{} {}", action_kind, target)
    } else {
        format!("{} failed for {}", action_kind, target)
    };
    if let Ok(record) = crate::dxos::append_audit_record(
        project_path,
        project_name,
        actor,
        action_kind,
        target,
        outcome,
        &summary,
        result.clone(),
    ) {
        app.state
            .event_bus
            .send(crate::state::events::StateEvent::AuditLogged {
                project: record.project_name,
                action_id: record.id,
                kind: record.action_kind,
                target: record.target,
                outcome: record.outcome,
            });
    }
}

/// GET / — Serve dashboard HTML
pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../assets/dashboard.html"))
}

// === Query parameter structs ===

/// Query parameter for filtering by tracker space
#[derive(Deserialize, Default)]
pub struct SpaceQuery {
    pub space: Option<String>,
}

/// Query parameter for selecting a sprint
#[derive(Deserialize, Default)]
pub struct SprintQuery {
    pub sprint: Option<String>,
}

/// Query parameter for pane output (line count)
#[derive(Deserialize, Default)]
pub struct PaneQuery {
    pub lines: Option<usize>,
}

#[derive(Deserialize, Default)]
pub struct PaneTalkBody {
    pub pane: u8,
    pub message: String,
}

#[derive(Deserialize, Default)]
pub struct PaneKillBody {
    pub pane: String,
    pub reason: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct PaneRestartBody {
    pub pane: String,
}

#[derive(Deserialize, Default)]
pub struct GatewayListQuery {
    pub running_only: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct GatewayToolQuery {
    pub mcp: Option<String>,
}

// === DX Terminal state endpoints — thin adapters over MCP tools ===

/// GET /api/status — All panes with PTY state (via tools::status)
pub async fn get_status(State(app): State<AppState>) -> Json<Value> {
    let result = tools::status(&app).await;
    Json(parse_mcp(&result))
}

/// GET /api/pane/:id — Single pane detail (via tools::config_show + tools::watch)
pub async fn get_pane(State(app): State<AppState>, Path(pane_ref): Path<String>) -> Json<Value> {
    // Get config data
    let config_result = tools::config_show(
        &app,
        types::ConfigShowRequest {
            pane: Some(pane_ref.clone()),
        },
    )
    .await;
    let mut data = parse_mcp(&config_result);

    // Get PTY data via watch
    let watch_result = tools::watch(
        &app,
        types::WatchRequest {
            pane: pane_ref,
            tail: Some(100),
            analyze_errors: Some(true),
        },
    )
    .await;
    let watch_data = parse_mcp(&watch_result);

    // Merge watch data into config data
    if let Some(obj) = data.as_object_mut() {
        if let Some(w) = watch_data.as_object() {
            for key in [
                "screen",
                "output",
                "pty_running",
                "line_count",
                "pty_done",
                "pty_error",
                "pty_done_marker",
                "health",
                "error_analysis",
                "progress",
            ] {
                if let Some(v) = w.get(key) {
                    obj.insert(key.to_string(), v.clone());
                }
            }
        }
    }

    Json(data)
}

/// POST /api/pane/talk — Send input into a live pane over the active substrate
pub async fn post_pane_talk(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PaneTalkBody>,
) -> ApiJson {
    if body.pane == 0 || body.message.trim().is_empty() {
        return Ok(Json(json!({"error": "pane and message required"})));
    }
    let pane_data = app.state.get_pane(body.pane).await;
    let actor = require_control_access(
        &app,
        &headers,
        &pane_data.project_path,
        Some(&pane_data.project),
        "pane_talk",
        &format!("pane:{}", body.pane),
    )?;
    if let Some(target) = pane_data
        .tmux_target
        .filter(|target| crate::tmux::pane_exists(target))
    {
        match tokio::task::spawn_blocking(move || crate::tmux::send_command(&target, &body.message))
            .await
        {
            Ok(Ok(())) => {
                let value = json!({
                    "status": "sent",
                    "pane": body.pane,
                    "runtime_adapter": "tmux_migration_adapter",
                });
                record_control_action(
                    &app,
                    &pane_data.project_path,
                    Some(&pane_data.project),
                    &actor,
                    "pane_talk",
                    &format!("pane:{}", body.pane),
                    &value,
                );
                Ok(Json(value))
            }
            Ok(Err(error)) => {
                let value = json!({"error": format!("{}", error)});
                record_control_action(
                    &app,
                    &pane_data.project_path,
                    Some(&pane_data.project),
                    &actor,
                    "pane_talk",
                    &format!("pane:{}", body.pane),
                    &value,
                );
                Ok(Json(value))
            }
            Err(error) => {
                let value = json!({"error": format!("task join error: {}", error)});
                record_control_action(
                    &app,
                    &pane_data.project_path,
                    Some(&pane_data.project),
                    &actor,
                    "pane_talk",
                    &format!("pane:{}", body.pane),
                    &value,
                );
                Ok(Json(value))
            }
        }
    } else {
        let send_result = {
            let mut pty = app.pty_lock();
            pty.send_line(body.pane, &body.message)
        };
        match send_result {
            Ok(()) => {
                let value = json!({
                    "status": "sent",
                    "pane": body.pane,
                    "runtime_adapter": "pty_native_adapter",
                });
                record_control_action(
                    &app,
                    &pane_data.project_path,
                    Some(&pane_data.project),
                    &actor,
                    "pane_talk",
                    &format!("pane:{}", body.pane),
                    &value,
                );
                Ok(Json(value))
            }
            Err(error) => {
                let value = json!({"error": format!("{}", error)});
                record_control_action(
                    &app,
                    &pane_data.project_path,
                    Some(&pane_data.project),
                    &actor,
                    "pane_talk",
                    &format!("pane:{}", body.pane),
                    &value,
                );
                Ok(Json(value))
            }
        }
    }
}

/// POST /api/pane/kill — Stop a live pane through the control-plane API
pub async fn post_pane_kill(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PaneKillBody>,
) -> ApiJson {
    let pane_num = body.pane.parse::<u8>().ok().unwrap_or(0);
    let pane_data = app.state.get_pane(pane_num).await;
    let actor = require_control_access(
        &app,
        &headers,
        &pane_data.project_path,
        Some(&pane_data.project),
        "pane_kill",
        &format!("pane:{}", pane_num),
    )?;
    let result = tools::kill(
        &app,
        types::KillRequest {
            pane: body.pane,
            reason: body.reason,
        },
    )
    .await;
    let value = parse_mcp(&result);
    record_control_action(
        &app,
        &pane_data.project_path,
        Some(&pane_data.project),
        &actor,
        "pane_kill",
        &format!("pane:{}", pane_num),
        &value,
    );
    Ok(Json(value))
}

/// POST /api/pane/restart — Restart a live pane through the control-plane API
pub async fn post_pane_restart(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PaneRestartBody>,
) -> ApiJson {
    let pane_num = body.pane.parse::<u8>().ok().unwrap_or(0);
    let pane_data = app.state.get_pane(pane_num).await;
    let actor = require_control_access(
        &app,
        &headers,
        &pane_data.project_path,
        Some(&pane_data.project),
        "pane_restart",
        &format!("pane:{}", pane_num),
    )?;
    let result = tools::restart(&app, types::RestartRequest { pane: body.pane }).await;
    let value = parse_mcp(&result);
    record_control_action(
        &app,
        &pane_data.project_path,
        Some(&pane_data.project),
        &actor,
        "pane_restart",
        &format!("pane:{}", pane_num),
        &value,
    );
    Ok(Json(value))
}

/// GET /api/pane/:id/output — PTY output (via tools::watch)
pub async fn get_pane_output(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
    Query(params): Query<PaneQuery>,
) -> Json<Value> {
    let result = tools::watch(
        &app,
        types::WatchRequest {
            pane: pane_ref,
            tail: params.lines.map(|l| l),
            analyze_errors: Some(false),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// GET /api/health — PTY health for all panes (via tools::health)
pub async fn get_health(State(app): State<AppState>) -> Json<Value> {
    let result = tools::health(&app).await;
    Json(parse_mcp(&result))
}

/// GET /api/logs — Activity log (via tools::logs)
pub async fn get_logs(
    State(app): State<AppState>,
    Query(_params): Query<SpaceQuery>,
) -> Json<Value> {
    let result = tools::logs(
        &app,
        types::LogsRequest {
            pane: None,
            lines: None,
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// GET /api/gateway/list — External MCP registry bridged through dx
pub async fn get_gateway_list(
    State(app): State<AppState>,
    Query(q): Query<GatewayListQuery>,
) -> Json<Value> {
    let result = tools::gateway_tools::gateway_list(
        &app,
        types::GatewayListRequest {
            running_only: q.running_only,
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// GET /api/gateway/tools?mcp=NAME — Tool schemas for one bridged external MCP
pub async fn get_gateway_tools(
    State(app): State<AppState>,
    Query(q): Query<GatewayToolQuery>,
) -> Json<Value> {
    let Some(mcp) = q.mcp.filter(|value| !value.trim().is_empty()) else {
        return Json(json!({"error": "mcp query param required"}));
    };
    let result = tools::gateway_tools::gateway_tools(
        &app,
        types::GatewayToolsRequest {
            mcp,
            auto_start: Some(true),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// POST /api/gateway/call — Invoke one tool on a bridged external MCP
pub async fn post_gateway_call(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let mcp = body["mcp"].as_str().unwrap_or("").to_string();
    let tool = body["tool"].as_str().unwrap_or("").to_string();
    if mcp.is_empty() || tool.is_empty() {
        return Json(json!({"error": "mcp and tool are required"}));
    }
    let result = tools::gateway_tools::gateway_call(
        &app,
        types::GatewayCallRequest {
            mcp,
            tool,
            arguments: body.get("arguments").cloned(),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

// === Tracker / capacity endpoints (file-based, no MCP equivalent) ===

/// GET /api/spaces — List tracker spaces
pub async fn get_spaces() -> Json<Value> {
    let spaces_dir = config::collab_root().join("spaces");
    let mut spaces = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&spaces_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                spaces.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    spaces.sort();
    Json(json!(spaces))
}

/// GET /api/agents — Agent list (via tools::status, filtered to active)
pub async fn get_agents(State(app): State<AppState>) -> Json<Value> {
    let result = tools::status(&app).await;
    let data = parse_mcp(&result);

    let agents: Vec<Value> = data["panes"]
        .as_array()
        .map(|panes| {
            panes
                .iter()
                .filter(|p| {
                    p["status"].as_str().map_or(false, |s| s == "active")
                        || p["pty_active"].as_bool().unwrap_or(false)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    Json(json!(agents))
}

/// GET /api/capacity/dashboard — Capacity data
pub async fn get_capacity_dashboard(Query(_params): Query<SpaceQuery>) -> Json<Value> {
    let cap = capacity::load_capacity();

    let acu_pct = if cap.acu_total > 0.0 {
        (cap.acu_used / cap.acu_total * 100.0) as i32
    } else {
        0
    };
    let reviews_pct = if cap.reviews_total > 0 {
        (cap.reviews_used as f64 / cap.reviews_total as f64 * 100.0) as i32
    } else {
        0
    };
    let bottleneck = if reviews_pct > 80 {
        "review"
    } else if acu_pct > 90 {
        "compute"
    } else {
        "balanced"
    };

    let cfg_path = config::capacity_root().join("config.json");
    let cfg = crate::state::persistence::read_json(&cfg_path);
    let mut roles = serde_json::Map::new();
    if let Some(role_cfg) = cfg.get("roles").and_then(|r| r.as_object()) {
        for (key, info) in role_cfg {
            let name = info.get("name").and_then(|v| v.as_str()).unwrap_or(key);
            roles.insert(
                key.clone(),
                json!({
                    "name": name,
                    "used": 0,
                    "pct": 0,
                }),
            );
        }
    }

    Json(json!({
        "acu_used": cap.acu_used,
        "acu_total": cap.acu_total,
        "acu_pct": acu_pct,
        "reviews_used": cap.reviews_used,
        "reviews_total": cap.reviews_total,
        "reviews_pct": reviews_pct,
        "bottleneck": bottleneck,
        "roles": roles,
    }))
}

/// GET /api/board — Kanban board
pub async fn get_board(Query(params): Query<SpaceQuery>) -> Json<Value> {
    let space = params.space.unwrap_or_default();
    if space.is_empty() {
        return Json(json!({}));
    }
    let board = build_board(&space);
    Json(json!(board))
}

/// GET /api/issues — Issues list
pub async fn get_issues(Query(params): Query<SpaceQuery>) -> Json<Value> {
    let space = params.space.unwrap_or_default();
    if space.is_empty() {
        return Json(json!([]));
    }
    let issues = load_all_issues(&space);
    Json(json!(issues))
}

/// GET /api/sprints — Sprint list
pub async fn get_sprints(Query(params): Query<SpaceQuery>) -> Json<Value> {
    let space = params.space.unwrap_or_default();
    let sprints = load_sprints(&space);
    Json(json!(sprints))
}

/// GET /api/burndown — Sprint burndown data
pub async fn get_burndown(Query(params): Query<SprintQuery>) -> Json<Value> {
    let sprint_id = params.sprint.unwrap_or_default();
    if sprint_id.is_empty() {
        return Json(json!({"error": "missing sprint parameter"}));
    }
    let data = compute_burndown(&sprint_id);
    Json(json!(data))
}

/// GET /api/mcps — List all available MCPs (via tools::mcp_list)
pub async fn get_mcps(
    State(app): State<AppState>,
    Query(params): Query<SpaceQuery>,
) -> Json<Value> {
    let result = tools::mcp_list(
        &app,
        types::McpListRequest {
            category: None,
            project: params.space,
        },
    )
    .await;
    let data = parse_mcp(&result);
    // Return just the mcps array for backward compat
    Json(data.get("mcps").cloned().unwrap_or(json!([])))
}

/// GET /api/mcps/route — Smart route MCPs (via tools::mcp_route)
#[derive(Deserialize, Default)]
pub struct McpRouteQuery {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

pub async fn get_mcp_route(
    State(app): State<AppState>,
    Query(params): Query<McpRouteQuery>,
) -> Json<Value> {
    let project = params.project.unwrap_or_default();
    if project.is_empty() {
        return Json(json!({"error": "missing project parameter"}));
    }
    let result = tools::mcp_route(
        &app,
        types::McpRouteRequest {
            project,
            task: params.task.unwrap_or_default(),
            role: params.role,
            apply: Some(false),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// GET /api/roles — Role config
pub async fn get_roles() -> Json<Value> {
    let cfg_path = config::capacity_root().join("config.json");
    let cfg = crate::state::persistence::read_json(&cfg_path);
    Json(cfg.get("roles").cloned().unwrap_or_else(|| json!({})))
}

/// POST /api/queue/add — Add task (via tools::queue_add)
pub async fn post_queue_add(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let project = body
        .get("project")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let task = body
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let role = body
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let priority = body
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u8);
    let prompt = body
        .get("prompt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let depends_on: Option<Vec<String>> =
        body.get("depends_on")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

    if project.is_empty() || task.is_empty() {
        return Json(json!({"error": "project and task are required"}));
    }

    let result = tools::queue_add(
        &app,
        types::QueueAddRequest {
            project,
            task,
            role,
            prompt,
            priority,
            depends_on,
            max_retries: None,
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// POST /api/queue/done — Mark task done (via tools::queue_done)
pub async fn post_queue_done(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let task_id = body
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let result_str = body
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or("done")
        .to_string();
    if task_id.is_empty() {
        return Json(json!({"error": "task_id required"}));
    }
    let result = tools::queue_done(
        &app,
        types::QueueDoneRequest {
            task_id,
            result: Some(result_str),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// POST /api/queue/delete — Remove a task from queue
pub async fn post_queue_delete(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let task_id = body.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    if task_id.is_empty() {
        return Json(json!({"error": "task_id required"}));
    }
    let mut q = queue::load_queue();
    let before = q.tasks.len();
    q.tasks.retain(|t| t.id != task_id);
    if q.tasks.len() == before {
        return Json(json!({"error": "task not found"}));
    }
    match queue::save_queue(&q) {
        Ok(()) => {
            app.state
                .event_bus
                .send(crate::state::events::StateEvent::QueueChanged {
                    action: "deleted".into(),
                    task_id: task_id.to_string(),
                    task: String::new(),
                });
            Json(json!({"status": "deleted", "task_id": task_id}))
        }
        Err(e) => Json(json!({"error": format!("{}", e)})),
    }
}

/// POST /api/queue/retry — Re-queue a failed task
pub async fn post_queue_retry(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let task_id = body.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    if task_id.is_empty() {
        return Json(json!({"error": "task_id required"}));
    }
    let mut q = queue::load_queue();
    if let Some(task) = q.tasks.iter_mut().find(|t| t.id == task_id) {
        if task.status == queue::QueueStatus::Failed || task.status == queue::QueueStatus::Done {
            task.status = queue::QueueStatus::Pending;
            task.pane = None;
            task.started_at = None;
            task.completed_at = None;
            task.result = None;
        } else {
            return Json(json!({"error": "can only retry failed/done tasks"}));
        }
    } else {
        return Json(json!({"error": "task not found"}));
    }
    match queue::save_queue(&q) {
        Ok(()) => {
            app.state
                .event_bus
                .send(crate::state::events::StateEvent::QueueChanged {
                    action: "retried".into(),
                    task_id: task_id.to_string(),
                    task: String::new(),
                });
            Json(json!({"status": "retried", "task_id": task_id}))
        }
        Err(e) => Json(json!({"error": format!("{}", e)})),
    }
}

/// GET /api/queue — Task queue (via tools::queue_list)
pub async fn get_queue(State(app): State<AppState>) -> Json<Value> {
    let result = tools::queue_list(&app, types::QueueListRequest { status: None }).await;
    Json(parse_mcp(&result))
}

// === Enhanced monitoring endpoints ===

/// GET /api/monitor — Full monitoring overview (via tools::monitor)
pub async fn get_monitor(State(app): State<AppState>) -> Json<Value> {
    let result = tools::monitor(
        &app,
        types::MonitorRequest {
            include_output: Some(false),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

/// GET /api/pane/:id/watch — Watch pane output (via tools::watch)
pub async fn get_watch(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
    Query(params): Query<PaneQuery>,
) -> Json<Value> {
    let result = tools::watch(
        &app,
        types::WatchRequest {
            pane: pane_ref,
            tail: params.lines.or(Some(50)),
            analyze_errors: Some(true),
        },
    )
    .await;
    Json(parse_mcp(&result))
}

// === Analytics endpoints ===

/// GET /api/analytics/digest — 24h daily digest
pub async fn get_analytics_digest() -> Json<Value> {
    Json(crate::dashboard::dash_daily_digest(None))
}

/// GET /api/analytics/alerts — Active alerts
pub async fn get_analytics_alerts() -> Json<Value> {
    Json(crate::dashboard::dash_alerts(None))
}

/// GET /api/analytics/quality?project=X — Project health score
#[derive(Deserialize, Default)]
pub struct QualityQuery {
    pub project: Option<String>,
}

pub async fn get_analytics_quality(Query(params): Query<QualityQuery>) -> Json<Value> {
    let project = params.project.unwrap_or_default();
    if project.is_empty() {
        return Json(json!({"error": "missing project parameter"}));
    }
    Json(crate::quality::project_health(&project))
}

/// GET /api/analytics/leaderboard — Agent rankings
pub async fn get_analytics_leaderboard() -> Json<Value> {
    Json(crate::dashboard::dash_leaderboard(7, None))
}

/// GET /api/analytics/overview — God view dashboard
pub async fn get_analytics_overview() -> Json<Value> {
    Json(crate::dashboard::dash_overview(None))
}

// === Build environment endpoints ===

/// GET /api/builds — All build environments with colors and pane info
pub async fn get_builds() -> Json<Value> {
    let builds = crate::build::build_status();
    let sessions = crate::build::session_count();

    let build_list: Vec<Value> = builds
        .iter()
        .map(|b| {
            json!({
                "number": b.number,
                "name": b.name,
                "theme": b.theme,
                "theme_desc": crate::build::theme_desc(b.number),
                "pane_count": b.pane_count,
                "panes": b.panes.iter().map(|p| json!({
                    "index": p.pane_index,
                    "pane_id": p.pane_id,
                    "command": p.command,
                    "cwd": p.cwd,
                    "colors": {
                        "bg": p.colors.bg,
                        "fg": p.colors.fg,
                    }
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Json(json!({
        "builds": build_list,
        "total_builds": builds.len(),
        "total_panes": builds.iter().map(|b| b.pane_count).sum::<usize>(),
        "sessions": sessions,
    }))
}

/// POST /api/builds/create — Create or restyle a build
pub async fn post_build_create(Json(body): Json<Value>) -> Json<Value> {
    let number = body.get("number").and_then(|v| v.as_u64()).map(|n| n as u8);
    let result = crate::mcp::tools::build_tools::build_create(number);
    Json(parse_mcp(&result))
}

/// POST /api/builds/restyle — Restyle all builds
pub async fn post_build_restyle() -> Json<Value> {
    let result = crate::mcp::tools::build_tools::build_restyle();
    Json(parse_mcp(&result))
}

/// POST /api/builds/send — Send command to a build pane
pub async fn post_build_send(Json(body): Json<Value>) -> Json<Value> {
    let build = body.get("build").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let pane = body.get("pane").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let command = body
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if build == 0 || pane == 0 || command.is_empty() {
        return Json(json!({"error": "build (1-5), pane (1-3), and command are required"}));
    }

    let result = crate::mcp::tools::build_tools::build_send(build, pane, command);
    Json(parse_mcp(&result))
}

/// POST /api/builds/rename — Rename a build window
pub async fn post_build_rename(Json(body): Json<Value>) -> Json<Value> {
    let build = body.get("build").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if build == 0 || name.is_empty() {
        return Json(json!({"error": "build (1-5) and name are required"}));
    }

    let result = crate::mcp::tools::build_tools::build_rename(build, name);
    Json(parse_mcp(&result))
}

// === Helpers (file-based tracker data) ===

fn load_all_issues(space: &str) -> Vec<Value> {
    let dir = config::collab_root()
        .join("spaces")
        .join(space)
        .join("issues");
    let mut issues = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(issue) = serde_json::from_str::<Value>(&content) {
                        issues.push(issue);
                    }
                }
            }
        }
    }
    issues
}

fn build_board(space: &str) -> Value {
    let statuses = [
        "backlog",
        "todo",
        "in_progress",
        "review",
        "done",
        "closed",
        "blocked",
    ];
    let mut columns: std::collections::HashMap<&str, Vec<Value>> =
        statuses.iter().map(|s| (*s, Vec::new())).collect();

    for issue in load_all_issues(space) {
        let status = issue
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("backlog");
        if let Some(col) = columns.get_mut(status) {
            col.push(json!({
                "id": issue.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "title": issue.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "type": issue.get("type").and_then(|v| v.as_str()).unwrap_or("task"),
                "priority": issue.get("priority").and_then(|v| v.as_str()).unwrap_or("medium"),
                "assignee": issue.get("assignee").and_then(|v| v.as_str()).unwrap_or(""),
                "estimated_acu": issue.get("estimated_acu").unwrap_or(&json!(0)),
                "actual_acu": issue.get("actual_acu").unwrap_or(&json!(0)),
                "role": issue.get("role").and_then(|v| v.as_str()).unwrap_or(""),
            }));
        }
    }

    let mut result = serde_json::Map::new();
    for status in &statuses {
        if let Some(cards) = columns.get(status) {
            if !cards.is_empty() {
                result.insert(status.to_string(), json!(cards));
            }
        }
    }
    Value::Object(result)
}

fn load_sprints(space: &str) -> Vec<Value> {
    let dir = config::capacity_root().join("sprints");
    let mut sprints = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(sprint) = serde_json::from_str::<Value>(&content) {
                        if space.is_empty()
                            || sprint.get("space").and_then(|v| v.as_str()) == Some(space)
                        {
                            sprints.push(sprint);
                        }
                    }
                }
            }
        }
    }
    sprints
}

fn compute_burndown(sprint_id: &str) -> Value {
    let path = config::capacity_root()
        .join("sprints")
        .join(format!("{}.json", sprint_id));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return json!({"error": "sprint not found"}),
    };
    let sprint: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return json!({"error": "invalid sprint data"}),
    };

    let planned_acu = sprint
        .get("planned")
        .and_then(|p| p.get("total_acu"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let days = sprint.get("days").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let start_date = sprint
        .get("start_date")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let start = match chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return json!({"error": "invalid start_date"}),
    };

    let daily_burn = if days > 0 {
        planned_acu / days as f64
    } else {
        0.0
    };

    let mut ideal = Vec::new();
    for d in 0..=days {
        let date = start + chrono::Duration::days(d as i64);
        ideal.push(json!({
            "day": d,
            "date": date.format("%Y-%m-%d").to_string(),
            "remaining": ((planned_acu - daily_burn * d as f64) * 100.0).round() / 100.0,
        }));
    }

    let log_path = config::capacity_root().join("work_log.json");
    let log = crate::state::persistence::read_json(&log_path);
    let entries = log
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut actual = Vec::new();
    let mut cumulative = 0.0;
    let today = chrono::Local::now().date_naive();
    for d in 0..=days {
        let date = start + chrono::Duration::days(d as i64);
        if date > today {
            break;
        }
        let date_str = date.format("%Y-%m-%d").to_string();
        let day_acu: f64 = entries
            .iter()
            .filter(|e| {
                e.get("logged_at")
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s.starts_with(&date_str))
            })
            .filter_map(|e| e.get("acu_spent").and_then(|v| v.as_f64()))
            .sum();
        cumulative += day_acu;
        actual.push(json!({
            "day": d,
            "date": date_str,
            "remaining": ((planned_acu - cumulative).max(0.0) * 100.0).round() / 100.0,
            "acu_burned": (day_acu * 100.0).round() / 100.0,
        }));
    }

    json!({
        "sprint": sprint_id,
        "planned_acu": planned_acu,
        "ideal": ideal,
        "actual": actual,
    })
}

// === Vision endpoints ===

#[derive(Deserialize, Default)]
pub struct VisionQuery {
    pub project: Option<String>,
    pub path: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosAdoptionStartBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub summary: Option<String>,
    pub objective: Option<String>,
    pub feature_id: Option<String>,
    pub stage: Option<String>,
    #[serde(default)]
    pub participants: Vec<String>,
    pub requested_by: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosAdoptionStatusBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub adoption_id: String,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosDebateStartBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub title: String,
    pub objective: String,
    pub stage: Option<String>,
    pub feature_id: Option<String>,
    #[serde(default)]
    pub participants: Vec<String>,
    pub requested_by: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosDebateProposalBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub debate_id: String,
    pub author: String,
    pub model: Option<String>,
    pub summary: String,
    pub rationale: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosDebateContradictionBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub debate_id: String,
    pub proposal_id: String,
    pub author: String,
    pub model: Option<String>,
    pub rationale: String,
}

#[derive(Deserialize, Default)]
pub struct DxosDebateVoteBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub debate_id: String,
    pub proposal_id: String,
    pub voter: String,
    pub model: Option<String>,
    pub stance: String,
    pub rationale: String,
}

#[derive(Deserialize, Default)]
pub struct DxosDebateDecisionBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub debate_id: String,
    pub chosen_proposal_id: String,
    pub decided_by: String,
    pub summary: String,
    pub rationale: String,
}

#[derive(Deserialize, Default)]
pub struct DxosSessionUpsertBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub session_id: Option<String>,
    pub role: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub autonomy_level: Option<String>,
    pub objective: String,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub allowed_capabilities: Vec<String>,
    #[serde(default)]
    pub allowed_repos: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    pub workspace_path: Option<String>,
    pub branch_name: Option<String>,
    pub browser_port: Option<u16>,
    pub pane: Option<u8>,
    pub runtime_adapter: Option<String>,
    pub tmux_target: Option<String>,
    pub feature_id: Option<String>,
    pub stage: Option<String>,
    pub supervisor_session_id: Option<String>,
    pub escalation_policy: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosSessionLaunchBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub pane: Option<String>,
    pub session_id: Option<String>,
    pub role: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub runtime_adapter: Option<String>,
    pub client_request_id: Option<String>,
    pub feature_id: Option<String>,
    pub stage: Option<String>,
    pub supervisor_session_id: Option<String>,
    pub task: String,
    pub prompt: Option<String>,
    pub autonomous: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct DxosSessionStatusBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub session_id: String,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosWorkDelegateBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub supervisor_session_id: String,
    pub worker_session_id: Option<String>,
    pub title: String,
    pub objective: String,
    pub feature_id: Option<String>,
    pub stage: Option<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosWorkBlockBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub work_order_id: String,
    pub blocker: String,
    pub requested_permission: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosWorkResolveBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub work_order_id: String,
    pub resolution: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosSessionBlockBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub worker_session_id: String,
    pub blocker: String,
    pub requested_permission: Option<String>,
    pub resolution_hint: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DxosProviderPluginsSyncBody {
    pub project: Option<String>,
    pub path: Option<String>,
    pub source_provider: Option<String>,
    pub target_provider: Option<String>,
    pub dry_run: Option<bool>,
}

fn resolve_project_path(q: &VisionQuery) -> String {
    if let Some(ref p) = q.path {
        return p.clone();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/pran".to_string());
    if let Some(ref name) = q.project {
        let direct = format!("{}/Projects/{}", home, name);
        // If direct path has a .vision, use it
        if std::path::Path::new(&direct)
            .join(".vision/vision.json")
            .exists()
        {
            return direct;
        }
        // Otherwise scan ~/Projects/* for a vision.json with matching project name
        let projects_dir = format!("{}/Projects", home);
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let vision_file = entry.path().join(".vision/vision.json");
                if vision_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&vision_file) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                            if v.get("project").and_then(|p| p.as_str()) == Some(name) {
                                return entry.path().to_string_lossy().to_string();
                            }
                        }
                    }
                }
            }
        }
        direct
    } else {
        format!("{}/Projects", home)
    }
}

const GUIDANCE_DOC_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "CODEX.md", "GEMINI.md"];

fn guidance_doc_kind(file_name: &str) -> &'static str {
    match file_name {
        "AGENTS.md" => "shared",
        "CLAUDE.md" => "claude",
        "CODEX.md" => "codex",
        "GEMINI.md" => "gemini",
        _ => "shared",
    }
}

fn guidance_doc_rank(file_name: &str) -> usize {
    match file_name {
        "AGENTS.md" => 0,
        "CLAUDE.md" => 1,
        "CODEX.md" => 2,
        "GEMINI.md" => 3,
        _ => 9,
    }
}

fn has_project_marker(path: &FsPath) -> bool {
    path.join(".git").exists()
        || path.join(".vision/vision.json").exists()
        || GUIDANCE_DOC_FILES
            .iter()
            .any(|name| path.join(name).exists())
}

fn find_project_root(candidate: &FsPath) -> Option<PathBuf> {
    let start = if candidate.is_file() {
        candidate.parent()?
    } else {
        candidate
    };

    for dir in start.ancestors() {
        if has_project_marker(dir) {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn project_name_from_path(project_path: &str) -> String {
    FsPath::new(project_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn matches_project_path(candidate: &str, project_path: &str) -> bool {
    if candidate.trim().is_empty() || project_path.trim().is_empty() {
        return false;
    }

    let candidate = FsPath::new(candidate);
    let project_root = FsPath::new(project_path);
    candidate.starts_with(project_root)
        || find_project_root(candidate)
            .as_deref()
            .map(|root| root == project_root)
            .unwrap_or(false)
}

fn collect_guidance_docs(cwd: &str, project_path: &str) -> Vec<Value> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen_roots = HashSet::new();

    for candidate in [Some(project_path), Some(cwd)].into_iter().flatten() {
        if candidate.trim().is_empty() {
            continue;
        }
        let path = FsPath::new(candidate);
        if let Some(root) = find_project_root(path) {
            let key = root.to_string_lossy().to_string();
            if seen_roots.insert(key) {
                roots.push(root);
            }
        } else {
            let key = path.to_string_lossy().to_string();
            if seen_roots.insert(key.clone()) {
                roots.push(PathBuf::from(key));
            }
        }
    }

    let mut seen_files = HashSet::new();
    let mut docs = Vec::new();
    for root in roots {
        for name in GUIDANCE_DOC_FILES {
            let file_path = root.join(name);
            if !file_path.exists() {
                continue;
            }
            let key = file_path.to_string_lossy().to_string();
            if !seen_files.insert(key.clone()) {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let preview: String = content.chars().take(1600).collect();
                docs.push(json!({
                    "name": *name,
                    "kind": guidance_doc_kind(name),
                    "path": key,
                    "root": root.to_string_lossy().to_string(),
                    "preview": preview,
                    "size": content.len(),
                    "modified_unix_ms": modified_unix_ms(&file_path),
                }));
            }
        }
    }

    docs.sort_by_key(|doc| {
        doc.get("name")
            .and_then(|value| value.as_str())
            .map(guidance_doc_rank)
            .unwrap_or(99)
    });
    docs
}

fn collect_vision_docs_for_path(project_path: &str) -> Vec<Value> {
    let base = FsPath::new(project_path).join(".vision");
    let mut docs = Vec::new();

    for subdir in &["research", "discovery", "design"] {
        let dir = base.join(subdir);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".md") {
                    let feature_id = fname.trim_end_matches(".md").to_string();
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let lines: Vec<&str> = content.lines().take(3).collect();
                    let preview = lines.join(" ").chars().take(150).collect::<String>();
                    docs.push(json!({
                        "type": *subdir,
                        "feature_id": feature_id,
                        "file": fname,
                        "path": entry.path().to_string_lossy().to_string(),
                        "preview": preview,
                        "size": content.len(),
                        "modified_unix_ms": modified_unix_ms(&entry.path()),
                    }));
                }
            }
        }
    }

    docs.sort_by(|a, b| {
        let a_feature = a.get("feature_id").and_then(|v| v.as_str()).unwrap_or("");
        let b_feature = b.get("feature_id").and_then(|v| v.as_str()).unwrap_or("");
        a_feature.cmp(b_feature)
    });
    docs
}

fn modified_unix_ms(path: &FsPath) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
}

fn git_file_inventory(project_path: &str) -> (HashSet<String>, HashMap<String, String>) {
    let root = FsPath::new(project_path);
    let mut tracked = HashSet::new();
    let mut dirty = HashMap::new();

    if let Ok(output) = Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
    {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if !line.trim().is_empty() {
                    tracked.insert(line.trim().to_string());
                }
            }
        }
    }

    if let Ok(output) = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
    {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if line.len() < 4 {
                    continue;
                }
                let status = &line[..2];
                let raw_path = line[3..].trim();
                let path = raw_path
                    .split(" -> ")
                    .last()
                    .unwrap_or(raw_path)
                    .trim_matches('"')
                    .to_string();
                let x = status.chars().next().unwrap_or(' ');
                let y = status.chars().nth(1).unwrap_or(' ');
                let label = if status == "??" {
                    "untracked"
                } else if x != ' ' && y != ' ' {
                    "staged+modified"
                } else if x != ' ' {
                    "staged"
                } else {
                    "modified"
                };
                dirty.insert(path, label.to_string());
            }
        }
    }

    (tracked, dirty)
}

fn annotate_docs_with_git(project_path: &str, docs: &mut [Value]) {
    let root = FsPath::new(project_path);
    let (tracked, dirty) = git_file_inventory(project_path);

    for doc in docs.iter_mut() {
        let Some(obj) = doc.as_object_mut() else {
            continue;
        };
        let Some(path) = obj.get("path").and_then(|value| value.as_str()) else {
            continue;
        };
        let abs = FsPath::new(path);
        let relative = abs
            .strip_prefix(root)
            .unwrap_or(abs)
            .to_string_lossy()
            .replace('\\', "/");
        let dirty_status = dirty.get(&relative).cloned();
        let is_tracked = tracked.contains(&relative);

        obj.insert("relative_path".to_string(), json!(relative));
        obj.insert(
            "git".to_string(),
            json!({
                "tracked": is_tracked,
                "dirty": dirty_status.is_some(),
                "status": dirty_status.unwrap_or_else(|| {
                    if is_tracked { "clean".to_string() } else { "unknown".to_string() }
                }),
            }),
        );
    }
}

fn documentation_sync_contract(project: &str) -> Value {
    json!({
        "mode": "snapshot_plus_events",
        "snapshot": format!("/api/project/brief?project={}", project),
        "wiki": format!("/wiki?project={}", project),
        "events": [
            "vision_changed",
            "sync_event",
            "sync_status",
            "pane_upsert",
            "pane_removed",
            "pane_status",
            "terminal_output",
            "session_events",
            "queue_upsert",
            "queue_removed"
        ],
        "authorities": [
            ".vision/vision.json",
            "AGENTS.md + provider guidance docs",
            "git status",
            "runtime state + tmux discovery"
        ],
        "remote_site": "A hosted dashboard should consume the same snapshot and event channels instead of keeping a second project state."
    })
}

fn documentation_health(
    guidance_docs: &[Value],
    vision_docs: &[Value],
    runtimes: &[Value],
    features: &[Value],
) -> Value {
    let active_providers = runtimes
        .iter()
        .filter_map(|runtime| {
            runtime
                .get("provider")
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
        })
        .filter(|provider| !provider.is_empty() && *provider != "unknown")
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|provider| provider.to_string())
        .collect::<Vec<_>>();

    let guidance_kinds = guidance_docs
        .iter()
        .filter_map(|doc| doc.get("kind").and_then(|value| value.as_str()))
        .collect::<HashSet<_>>();
    let missing_provider_guidance = active_providers
        .iter()
        .filter(|provider| matches!(provider.as_str(), "claude" | "codex" | "gemini"))
        .filter(|provider| !guidance_kinds.contains(provider.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let mut docs_by_feature: HashMap<String, usize> = HashMap::new();
    for doc in vision_docs {
        if let Some(feature_id) = doc.get("feature_id").and_then(|value| value.as_str()) {
            *docs_by_feature.entry(feature_id.to_string()).or_insert(0) += 1;
        }
    }

    let mut missing_feature_docs = Vec::new();
    let mut missing_acceptance = Vec::new();
    for feature in features {
        let feature_id = feature
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let phase = feature
            .get("phase")
            .or_else(|| feature.get("status"))
            .and_then(|value| value.as_str())
            .unwrap_or("planned")
            .to_string();
        if feature_id.is_empty() || phase == "planned" {
            continue;
        }

        if docs_by_feature.get(&feature_id).copied().unwrap_or(0) == 0 {
            missing_feature_docs.push(json!({
                "feature_id": feature_id.clone(),
                "title": feature.get("title").cloned().unwrap_or(json!("")),
                "phase": phase.clone(),
            }));
        }

        let acceptance_count = feature
            .get("acceptance_items")
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or_else(|| {
                feature
                    .get("acceptance_criteria")
                    .and_then(|value| value.as_array())
                    .map(|items| items.len())
                    .unwrap_or(0)
            });
        if matches!(
            phase.as_str(),
            "build" | "building" | "test" | "testing" | "done"
        ) && acceptance_count == 0
        {
            missing_acceptance.push(json!({
                "feature_id": feature_id,
                "title": feature.get("title").cloned().unwrap_or(json!("")),
                "phase": phase,
            }));
        }
    }

    let dirty_docs = guidance_docs
        .iter()
        .chain(vision_docs.iter())
        .filter_map(|doc| {
            let git = doc.get("git")?;
            if !git
                .get("dirty")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return None;
            }
            Some(json!({
                "name": doc.get("name")
                    .or_else(|| doc.get("file"))
                    .cloned()
                    .unwrap_or(json!("")),
                "path": doc.get("relative_path")
                    .or_else(|| doc.get("path"))
                    .cloned()
                    .unwrap_or(json!("")),
                "status": git.get("status").cloned().unwrap_or(json!("modified")),
            }))
        })
        .collect::<Vec<_>>();

    let status = if !guidance_kinds.contains("shared") || !missing_provider_guidance.is_empty() {
        "blocked"
    } else if !missing_feature_docs.is_empty()
        || !missing_acceptance.is_empty()
        || !dirty_docs.is_empty()
    {
        "attention"
    } else {
        "synced"
    };

    let summary = match status {
        "blocked" if !guidance_kinds.contains("shared") => {
            "AGENTS.md is missing, so shared operating guidance is not aligned.".to_string()
        }
        "blocked" => format!(
            "Active runtime guidance is missing for: {}.",
            missing_provider_guidance.join(", ")
        ),
        "attention" if !dirty_docs.is_empty() => {
            "Documentation has uncommitted changes or drift that the dashboard is surfacing live."
                .to_string()
        }
        "attention" if !missing_feature_docs.is_empty() => {
            "Features have advanced past planning without attached research or discovery docs."
                .to_string()
        }
        "attention" => {
            "Features in delivery phases still need acceptance coverage or doc cleanup.".to_string()
        }
        _ => "Filesystem docs, git, and dashboard state are aligned.".to_string(),
    };

    json!({
        "status": status,
        "summary": summary,
        "active_providers": active_providers,
        "shared_guidance_present": guidance_kinds.contains("shared"),
        "missing_provider_guidance": missing_provider_guidance,
        "dirty_docs": dirty_docs,
        "missing_feature_docs": missing_feature_docs,
        "missing_acceptance": missing_acceptance,
    })
}

fn recovery_assessment(
    project: &str,
    phase_counts: &HashMap<String, usize>,
    documentation_health: &Value,
    blocking_features: &[Value],
    ready_features: &[Value],
    client_review_features: &[Value],
    runtime_count: usize,
    worktree_count: usize,
    features: &[Value],
) -> Value {
    serde_json::to_value(crate::recovery_planning::assess(
        project,
        phase_counts,
        documentation_health,
        blocking_features,
        ready_features,
        client_review_features,
        runtime_count,
        worktree_count,
        features,
    ))
    .unwrap_or_else(|_| json!({}))
}

fn provider_json(command: &str, window_name: &str, jsonl_path: Option<&str>) -> Value {
    let provider = crate::tmux::infer_provider(command, window_name, jsonl_path);
    json!({
        "id": provider,
        "label": crate::tmux::provider_label(provider),
        "short": crate::tmux::provider_short(provider),
    })
}

/// GET /api/vision?project=NAME — Get vision for a project
pub async fn get_vision(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let result = crate::vision::get_vision(&path);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/vision/summary?project=NAME — Dashboard-friendly summary
pub async fn get_vision_summary(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let result = crate::vision::vision_summary(&path);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/project/brief?project=NAME — Canonical project execution/documentation summary
pub(crate) async fn build_project_brief_payload(
    app: &AppState,
    project_path: &str,
    project: &str,
) -> Value {
    let tree = crate::vision::vision_tree(&project_path);
    let tree_value = serde_json::from_str::<Value>(&tree).unwrap_or_else(|_| json!({}));
    let summary = crate::vision::vision_summary(&project_path);
    let summary_value = serde_json::from_str::<Value>(&summary).unwrap_or_else(|_| json!({}));
    let mut docs = collect_vision_docs_for_path(&project_path);
    let mut guidance_docs = collect_guidance_docs(&project_path, &project_path);
    annotate_docs_with_git(&project_path, &mut docs);
    annotate_docs_with_git(&project_path, &mut guidance_docs);
    let automation = crate::agent_assets::collect_automation_assets(&project_path);
    let vision_doc_count = docs.len();
    let guidance_doc_count = guidance_docs.len();
    let focus = crate::vision_focus::read_project_focus(&project_path);

    let state = app.state.get_state_snapshot().await;
    let live_panes = tokio::task::spawn_blocking(|| crate::tmux::discover_live_panes())
        .await
        .unwrap_or_default();
    let runtimes = collect_project_runtimes(&state, &live_panes, &project_path, &project);
    let runtime_count = runtimes.len();
    let worktree_count = runtimes
        .iter()
        .filter(|runtime| {
            runtime
                .get("workspace_path")
                .and_then(|value| value.as_str())
                .map(|value| !value.is_empty())
                .unwrap_or(false)
        })
        .count();

    let mut phase_counts: HashMap<String, usize> = HashMap::new();
    let mut blocking_features = Vec::new();
    let mut ready_features = Vec::new();
    let mut client_review_features = Vec::new();
    let mut feature_records = Vec::new();

    for goal in tree_value
        .get("goals")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        for feature in goal
            .get("features")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
        {
            let feature_id = feature
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let phase = feature
                .get("phase")
                .or_else(|| feature.get("status"))
                .and_then(|value| value.as_str())
                .unwrap_or("planned")
                .to_string();
            let readiness = feature
                .get("readiness")
                .cloned()
                .unwrap_or_else(|| json!({}));

            feature_records.push(feature.clone());
            *phase_counts.entry(phase.clone()).or_insert(0) += 1;

            let next_gate = match phase.as_str() {
                "planned" | "discovery" | "specifying" => Some("build"),
                "build" | "building" => Some("test"),
                "test" | "testing" => Some("done"),
                _ => None,
            };
            if let Some(gate) = next_gate {
                let ready_key = format!("ready_for_{}", gate);
                let blockers = readiness
                    .get("blockers")
                    .and_then(|value| value.get(gate))
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default();
                if readiness
                    .get(&ready_key)
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
                {
                    ready_features.push(json!({
                        "feature_id": feature_id,
                        "title": feature.get("title").cloned().unwrap_or(json!("")),
                        "phase": phase,
                        "next_gate": gate,
                    }));
                } else if !blockers.is_empty() {
                    blocking_features.push(json!({
                        "feature_id": feature_id,
                        "title": feature.get("title").cloned().unwrap_or(json!("")),
                        "phase": phase,
                        "next_gate": gate,
                        "blockers": blockers,
                    }));
                }
            }

            if readiness
                .get("discovery")
                .and_then(|value| value.get("design_required"))
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                && readiness
                    .get("discovery")
                    .and_then(|value| value.get("design_approved"))
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0)
                    == 0
            {
                client_review_features.push(json!({
                    "feature_id": feature_id,
                    "title": feature.get("title").cloned().unwrap_or(json!("")),
                    "phase": phase,
                    "design_options": readiness
                        .get("discovery")
                        .and_then(|value| value.get("design_options"))
                        .cloned()
                        .unwrap_or(json!(0)),
                }));
            }
        }
    }

    let git = crate::sync::git::status(FsPath::new(&project_path))
        .ok()
        .map(|status| {
            json!({
                "branch": status.branch,
                "dirty_files": status.dirty_count,
                "ahead": status.ahead,
                "behind": status.behind,
                "has_remote": status.has_remote,
            })
        })
        .unwrap_or_else(|| json!(null));

    let documentation_health =
        documentation_health(&guidance_docs, &docs, &runtimes, &feature_records);
    let documentation = json!({
        "health": documentation_health.clone(),
        "sync_contract": documentation_sync_contract(&project),
    });
    let recovery = recovery_assessment(
        &project,
        &phase_counts,
        &documentation_health,
        &blocking_features,
        &ready_features,
        &client_review_features,
        runtime_count,
        worktree_count,
        &feature_records,
    );
    let dxos = crate::dxos::control_plane_snapshot(&project_path, Some(&project));

    json!({
        "project": project,
        "path": project_path,
        "mission": tree_value.get("mission").cloned().unwrap_or(json!("")),
        "summary": tree_value.get("summary").cloned().unwrap_or(summary_value),
        "focus": focus,
        "wiki_url": format!("/wiki?project={}", project),
        "docs": {
            "vision_docs": docs,
            "guidance_docs": guidance_docs,
            "vision_doc_count": vision_doc_count,
            "guidance_doc_count": guidance_doc_count,
        },
        "documentation": documentation,
        "recovery": recovery,
        "automation": automation,
        "delivery": {
            "phase_counts": phase_counts,
            "blocking_features": blocking_features,
            "ready_features": ready_features,
            "client_review_features": client_review_features,
        },
        "runtime_contract": {
            "auth": crate::dxos::control_auth_contract(),
            "launch_broker": {
                "name": "dx_runtime_broker",
                "adapters": crate::runtime_broker::adapter_inventory(),
                "providers": crate::runtime_broker::provider_inventory(),
            },
            "browser_port_base": crate::config::browser_port_base(),
            "browser_port_formula": "browser_port_base + pane",
            "browser_profile_root_template": "~/.playwright-profiles/pane-N",
            "browser_artifacts_root_template": "~/Projects/test-artifacts/sessions/pane-N",
            "runtime_adapters": ["pty_native_adapter", "tmux_migration_adapter"],
            "provider_native_launch": true,
            "runtime_providers": ["claude", "codex", "gemini", "opencode"],
            "control_endpoints": {
                "session_launch": "/api/dxos/session/launch",
                "pane_talk": "/api/pane/talk",
                "pane_kill": "/api/pane/kill",
                "pane_restart": "/api/pane/restart",
                "pane_output": "/api/pane/{id}/output",
                "event_stream": "/api/events",
                "websocket": "/ws",
            },
        },
        "runtimes": runtimes,
        "runtime_count": runtime_count,
        "worktree_count": worktree_count,
        "git": git,
        "dxos": dxos,
    })
}

pub(crate) fn derive_adoption_defaults(project: &str, brief: &Value) -> Value {
    let recovery = brief
        .get("recovery")
        .cloned()
        .and_then(|value| {
            serde_json::from_value::<crate::recovery_planning::RecoveryPlan>(value).ok()
        })
        .unwrap_or_default();
    let focus = brief.get("focus").cloned().unwrap_or_else(|| json!({}));
    let defaults = crate::recovery_planning::derive_adoption_defaults(
        project,
        &recovery,
        focus.get("feature_id").and_then(Value::as_str),
        focus.get("stage").and_then(Value::as_str),
    );
    json!({
        "summary": defaults.summary,
        "objective": defaults.objective,
        "feature_id": defaults.feature_id,
        "stage": defaults.stage,
    })
}

pub async fn get_project_brief(
    State(app): State<AppState>,
    Query(q): Query<VisionQuery>,
) -> Json<Value> {
    let project_path = resolve_project_path(&q);
    let project = q
        .project
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| project_name_from_path(&project_path));

    Json(build_project_brief_payload(&app, &project_path, &project).await)
}

/// GET /api/dxos/control-plane?project=NAME — DXOS contract and governance summary
pub async fn get_dxos_control_plane(Query(q): Query<VisionQuery>) -> Json<Value> {
    let project_path = resolve_project_path(&q);
    let project = q.project.as_deref();
    Json(crate::dxos::control_plane_snapshot(&project_path, project))
}

/// GET /api/dxos/registry — Shared DXOS control-plane registry
pub async fn get_dxos_registry() -> Json<Value> {
    let result = crate::dxos::control_plane_registry();
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/dxos/provider-plugins — Provider interoperability bridge inventory
pub async fn get_dxos_provider_plugins() -> Json<Value> {
    Json(crate::provider_plugins::plugin_inventory())
}

/// GET /api/dxos/debates?project=NAME — Formal debate records for a project
pub async fn get_dxos_debates(Query(q): Query<VisionQuery>) -> Json<Value> {
    let project_path = resolve_project_path(&q);
    let result = crate::dxos::debate_list(&project_path, q.project.as_deref());
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/dxos/sessions?project=NAME — Session contracts and delegated work
pub async fn get_dxos_sessions(Query(q): Query<VisionQuery>) -> Json<Value> {
    let project_path = resolve_project_path(&q);
    let result = crate::dxos::session_list(&project_path, q.project.as_deref());
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/dxos/audit?project=NAME — Recent control-plane audit records
pub async fn get_dxos_audit(Query(q): Query<VisionQuery>) -> Json<Value> {
    let project_path = resolve_project_path(&q);
    let result = crate::dxos::audit_list(&project_path, q.project.as_deref(), 20);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/dxos/adoption/start — Start a governed project adoption workflow
pub async fn start_dxos_adoption(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosAdoptionStartBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let project = body
        .project
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| project_name_from_path(&project_path));
    let mut derived_recovery = None;
    let derived_defaults = if body.summary.is_none()
        || body.objective.is_none()
        || body.feature_id.is_none()
        || body.stage.is_none()
    {
        let brief = build_project_brief_payload(&app, &project_path, &project).await;
        let recovery = brief
            .get("recovery")
            .cloned()
            .and_then(|value| {
                serde_json::from_value::<crate::recovery_planning::RecoveryPlan>(value).ok()
            })
            .unwrap_or_default();
        derived_recovery = Some(recovery.clone());
        Some(derive_adoption_defaults(&project, &brief))
    } else {
        None
    };
    let follow_on_suggestions = derived_recovery
        .as_ref()
        .map(crate::recovery_planning::follow_on_suggestions)
        .unwrap_or_default();
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        Some(&project),
        "adoption_start",
        body.summary
            .as_deref()
            .or_else(|| {
                derived_defaults
                    .as_ref()
                    .and_then(|value| value.get("summary"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("project_adoption"),
    )?;
    let result = crate::dxos::start_project_adoption_with_plan(
        &project_path,
        Some(&project),
        body.summary.as_deref().or_else(|| {
            derived_defaults
                .as_ref()
                .and_then(|value| value.get("summary"))
                .and_then(Value::as_str)
        }),
        body.objective.as_deref().or_else(|| {
            derived_defaults
                .as_ref()
                .and_then(|value| value.get("objective"))
                .and_then(Value::as_str)
        }),
        body.feature_id.as_deref().or_else(|| {
            derived_defaults
                .as_ref()
                .and_then(|value| value.get("feature_id"))
                .and_then(Value::as_str)
        }),
        body.stage.as_deref().or_else(|| {
            derived_defaults
                .as_ref()
                .and_then(|value| value.get("stage"))
                .and_then(Value::as_str)
        }),
        body.participants,
        body.requested_by.as_deref(),
        follow_on_suggestions,
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        Some(&project),
        &actor,
        "adoption_start",
        value
            .get("adoption_id")
            .and_then(Value::as_str)
            .unwrap_or("project_adoption"),
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/adoption/status — Update a project adoption workflow status
pub async fn update_dxos_adoption_status(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosAdoptionStatusBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "adoption_status",
        &body.adoption_id,
    )?;
    let result = crate::dxos::update_project_adoption_status(
        &project_path,
        body.project.as_deref(),
        &body.adoption_id,
        &body.status,
        body.note.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "adoption_status",
        &body.adoption_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/debate/start — Start a formal debate
pub async fn start_dxos_debate(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosDebateStartBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "debate_start",
        &body.title,
    )?;
    let result = crate::dxos::debate_start(
        &project_path,
        body.project.as_deref(),
        &body.title,
        &body.objective,
        body.stage.as_deref(),
        body.feature_id.as_deref(),
        body.participants,
        body.requested_by.as_deref(),
    );
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "debate_start",
        &body.title,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/provider-plugins/sync — Export the shared DX manifest into a provider bridge
pub async fn sync_dxos_provider_plugins(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosProviderPluginsSyncBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let target = body
        .target_provider
        .clone()
        .unwrap_or_else(|| "claude".to_string());
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "provider_plugin_sync",
        &target,
    )?;
    let value = crate::provider_plugins::convert_provider_plugin(
        body.source_provider.as_deref(),
        &target,
        body.dry_run.unwrap_or(false),
    )
    .map(|result| {
        json!({
            "provider_plugins": crate::provider_plugins::plugin_inventory(),
            "result": result,
        })
    })
    .unwrap_or_else(|error| json!({"error": error.to_string()}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "provider_plugin_sync",
        &target,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/debate/proposal — Add a proposal to a debate
pub async fn add_dxos_debate_proposal(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosDebateProposalBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "debate_proposal",
        &body.debate_id,
    )?;
    let result = crate::dxos::debate_add_proposal(
        &project_path,
        body.project.as_deref(),
        &body.debate_id,
        &body.author,
        body.model.as_deref(),
        &body.summary,
        &body.rationale,
        body.evidence,
    );
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "debate_proposal",
        &body.debate_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/debate/contradiction — Add a contradiction to a proposal
pub async fn add_dxos_debate_contradiction(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosDebateContradictionBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "debate_contradiction",
        &body.debate_id,
    )?;
    let result = crate::dxos::debate_add_contradiction(
        &project_path,
        body.project.as_deref(),
        &body.debate_id,
        &body.proposal_id,
        &body.author,
        body.model.as_deref(),
        &body.rationale,
    );
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "debate_contradiction",
        &body.debate_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/debate/vote — Cast or update a vote
pub async fn vote_dxos_debate(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosDebateVoteBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "debate_vote",
        &body.debate_id,
    )?;
    let result = crate::dxos::debate_cast_vote(
        &project_path,
        body.project.as_deref(),
        &body.debate_id,
        &body.proposal_id,
        &body.voter,
        body.model.as_deref(),
        &body.stance,
        &body.rationale,
    );
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "debate_vote",
        &body.debate_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/debate/decision — Finalize a debate decision
pub async fn finalize_dxos_debate(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosDebateDecisionBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "debate_finalize",
        &body.debate_id,
    )?;
    let result = crate::dxos::debate_finalize(
        &project_path,
        body.project.as_deref(),
        &body.debate_id,
        &body.chosen_proposal_id,
        &body.decided_by,
        &body.summary,
        &body.rationale,
    );
    maybe_emit_debate_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "debate_finalize",
        &body.debate_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/session/upsert — Create or update a session contract
pub async fn upsert_dxos_session(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosSessionUpsertBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "session_upsert",
        body.session_id.as_deref().unwrap_or(&body.role),
    )?;
    let result = crate::dxos::upsert_session_contract(
        &project_path,
        body.project.as_deref(),
        body.session_id.as_deref(),
        &body.role,
        body.provider.as_deref(),
        body.model.as_deref(),
        body.autonomy_level.as_deref(),
        &body.objective,
        body.expected_outputs,
        body.allowed_capabilities,
        body.allowed_repos,
        body.allowed_paths,
        body.workspace_path.as_deref(),
        body.branch_name.as_deref(),
        body.browser_port,
        body.pane,
        body.runtime_adapter.as_deref(),
        body.tmux_target.as_deref(),
        body.feature_id.as_deref(),
        body.stage.as_deref(),
        body.supervisor_session_id.as_deref(),
        body.escalation_policy.as_deref(),
        body.status.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "session_upsert",
        body.session_id.as_deref().unwrap_or(&body.role),
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/session/launch — Launch a real runtime lane through the DX broker
pub async fn launch_dxos_session(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosSessionLaunchBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let requested_pane = body.pane.clone().unwrap_or_else(|| "auto".to_string());
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "session_launch",
        &requested_pane,
    )?;
    let result = tools::spawn(
        &app,
        types::SpawnRequest {
            pane: requested_pane.clone(),
            project: project_path.clone(),
            session_id: body.session_id,
            role: body.role,
            provider: body.provider,
            model: body.model,
            runtime_adapter: body.runtime_adapter,
            client_request_id: body.client_request_id,
            feature_id: body.feature_id,
            stage: body.stage,
            supervisor_session_id: body.supervisor_session_id,
            task: Some(body.task),
            prompt: body.prompt,
            autonomous: body.autonomous,
        },
    )
    .await;
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "session_launch",
        &requested_pane,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/session/status — Update a session status
pub async fn update_dxos_session_status(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosSessionStatusBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "session_status",
        &body.session_id,
    )?;
    let result = crate::dxos::update_session_status(
        &project_path,
        body.project.as_deref(),
        &body.session_id,
        &body.status,
        body.note.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "session_status",
        &body.session_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/work/delegate — Delegate a structured work order
pub async fn delegate_dxos_work(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosWorkDelegateBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "work_delegate",
        &body.title,
    )?;
    let result = crate::dxos::delegate_work_order(
        &project_path,
        body.project.as_deref(),
        &body.supervisor_session_id,
        body.worker_session_id.as_deref(),
        &body.title,
        &body.objective,
        body.feature_id.as_deref(),
        body.stage.as_deref(),
        body.required_capabilities,
        body.expected_outputs,
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "work_delegate",
        &body.title,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/work/block — Block a work order
pub async fn block_dxos_work(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosWorkBlockBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "work_block",
        &body.work_order_id,
    )?;
    let result = crate::dxos::work_order_block(
        &project_path,
        body.project.as_deref(),
        &body.work_order_id,
        &body.blocker,
        body.requested_permission.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "work_block",
        &body.work_order_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/work/resolve — Resolve a blocked work order
pub async fn resolve_dxos_work(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosWorkResolveBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "work_resolve",
        &body.work_order_id,
    )?;
    let result = crate::dxos::resolve_work_order(
        &project_path,
        body.project.as_deref(),
        &body.work_order_id,
        body.resolution.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let delivery = crate::dxos_runtime::deliver_work_order_resolution(
        app.as_ref(),
        &project_path,
        body.project.as_deref(),
        &result,
        body.resolution.as_deref(),
    )
    .await;
    if delivery.status == "failed" {
        if let (Some(session_id), Some(error)) = (
            delivery.worker_session_id.as_deref(),
            delivery.error.as_deref(),
        ) {
            let failure = crate::dxos::record_session_delivery_failure(
                &project_path,
                body.project.as_deref(),
                session_id,
                error,
            );
            maybe_emit_dxos_session_change(&app, &project_path, &failure);
        }
    }
    let mut value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "delivery".to_string(),
            serde_json::to_value(&delivery).unwrap_or_else(|_| json!({"status": "unknown"})),
        );
    }
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "work_resolve",
        &body.work_order_id,
        &value,
    );
    Ok(Json(value))
}

/// POST /api/dxos/session/block — Raise a worker blocker or permission request
pub async fn raise_dxos_session_blocker(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DxosSessionBlockBody>,
) -> ApiJson {
    let project_path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let actor = require_control_access(
        &app,
        &headers,
        &project_path,
        body.project.as_deref(),
        "session_block",
        &body.worker_session_id,
    )?;
    let result = crate::dxos::raise_session_blocker(
        &project_path,
        body.project.as_deref(),
        &body.worker_session_id,
        &body.blocker,
        body.requested_permission.as_deref(),
        body.resolution_hint.as_deref(),
    );
    maybe_emit_dxos_session_change(&app, &project_path, &result);
    let value = serde_json::from_str(&result).unwrap_or(json!({"raw": result}));
    record_control_action(
        &app,
        &project_path,
        body.project.as_deref(),
        &actor,
        "session_block",
        &body.worker_session_id,
        &value,
    );
    Ok(Json(value))
}

/// GET /api/vision/diff?project=NAME — Recent vision changes
pub async fn get_vision_diff(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let result = crate::vision::vision_diff(&path, 20);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/vision/list — All visions across projects
pub async fn list_visions() -> Json<Value> {
    let result = crate::vision::list_visions();
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/init — Initialize vision
pub async fn init_vision(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let path = body["path"].as_str().unwrap_or("").to_string();
    let project = body["project"].as_str().unwrap_or("").to_string();
    let mission = body["mission"].as_str().unwrap_or("").to_string();
    let repo = body["repo"].as_str().unwrap_or("").to_string();

    if project.is_empty() || mission.is_empty() {
        return Json(json!({"error": "project and mission required"}));
    }

    let project_path = if path.is_empty() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/pran".to_string());
        format!("{}/Projects/{}", home, project)
    } else {
        path
    };

    let result = crate::vision::init_vision(&project_path, &project, &mission, &repo);
    maybe_emit_vision_change(&app, &project_path, &result, None);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/sync — Sync vision to GitHub
pub async fn sync_vision(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let result = crate::vision::github_sync(&path);
    maybe_emit_vision_change(&app, &path, &result, None);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

// ── VDD: Vision-Driven Development ──

#[derive(Deserialize, Default)]
pub struct VisionDrillQuery {
    pub project: Option<String>,
    pub goal_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct VisionFeatureQuery {
    pub project: Option<String>,
    pub feature_id: Option<String>,
}

/// GET /api/vision/tree?project=NAME — Full vision tree with progress rollup
pub async fn get_vision_tree(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let result = crate::vision::vision_tree(&path);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/vision/drill?project=NAME&goal_id=G1 — Drill into goal features
pub async fn get_vision_drill(Query(q): Query<VisionDrillQuery>) -> Json<Value> {
    let vq = VisionQuery {
        project: q.project.clone(),
        path: None,
    };
    let path = resolve_project_path(&vq);
    let goal_id = q.goal_id.as_deref().unwrap_or("G1");
    let result = crate::vision::drill_down(&path, goal_id);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/vision/feature/readiness?project=NAME&feature_id=F1.1 — Get phase/state/readiness for a feature
pub async fn get_vision_feature_readiness(Query(q): Query<VisionFeatureQuery>) -> Json<Value> {
    let vq = VisionQuery {
        project: q.project.clone(),
        path: None,
    };
    let path = resolve_project_path(&vq);
    let feature_id = q.feature_id.as_deref().unwrap_or("");
    if feature_id.is_empty() {
        return Json(json!({"error": "feature_id required"}));
    }
    let result = crate::vision::feature_readiness(&path, feature_id);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// GET /api/vision/discovery/readiness?project=NAME&feature_id=F1.1 — Check discovery completeness for a feature
pub async fn get_vision_discovery_readiness(Query(q): Query<VisionFeatureQuery>) -> Json<Value> {
    let vq = VisionQuery {
        project: q.project.clone(),
        path: None,
    };
    let path = resolve_project_path(&vq);
    let feature_id = q.feature_id.as_deref().unwrap_or("");
    if feature_id.is_empty() {
        return Json(json!({"error": "feature_id required"}));
    }
    let result = crate::vision::discovery_ready_check(&path, feature_id);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/discovery/complete — Advance a feature to build if discovery is complete
pub async fn complete_vision_discovery(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    if feature_id.is_empty() {
        return Json(json!({"error": "feature_id required"}));
    }
    let result = crate::vision::complete_discovery(&path, feature_id);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/feature — Add feature under a goal
pub async fn add_vision_feature(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let goal_id = body["goal_id"].as_str().unwrap_or("");
    let title = body["title"].as_str().unwrap_or("");
    let description = body["description"].as_str().unwrap_or("");
    let criteria: Vec<String> = body["acceptance_criteria"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let result = crate::vision::add_feature(&path, goal_id, title, description, criteria);
    maybe_emit_vision_change(&app, &path, &result, None);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/discovery/start — Explicitly move a planned feature into discovery
pub async fn start_vision_discovery(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let result = crate::vision::start_discovery(&path, feature_id);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/acceptance — Add one acceptance criterion to a feature
pub async fn add_vision_acceptance(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let criterion = body["criterion"].as_str().unwrap_or("");
    let result = crate::vision::add_acceptance_criterion(&path, feature_id, criterion);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/acceptance/update — Update text or verification method for one acceptance criterion
pub async fn update_vision_acceptance(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let criterion_id = body["criterion_id"].as_str().unwrap_or("");
    let text = body["text"].as_str();
    let verification_method = body["verification_method"].as_str();
    let result = crate::vision::update_acceptance_criterion(
        &path,
        feature_id,
        criterion_id,
        text,
        verification_method,
    );
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/acceptance/verify — Set acceptance verification status with provider-neutral metadata
pub async fn verify_vision_acceptance(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let criterion_id = body["criterion_id"].as_str().unwrap_or("");
    let status = body["status"].as_str().unwrap_or("");
    let evidence: Vec<String> = body["evidence"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let verified_by = body["verified_by"].as_str();
    let verification_source = body["verification_source"].as_str();
    let result = crate::vision::verify_acceptance_criterion(
        &path,
        feature_id,
        criterion_id,
        status,
        evidence,
        verified_by,
        verification_source,
    );
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/question — Add question to a feature
pub async fn add_vision_question(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let question = body["question"].as_str().unwrap_or("");
    let blocking = body["blocking"].as_bool().unwrap_or(true);
    let result = crate::vision::add_question_with_blocking(&path, feature_id, question, blocking);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/answer — Answer a question with decision
pub async fn answer_vision_question(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let question_id = body["question_id"].as_str().unwrap_or("");
    let answer = body["answer"].as_str().unwrap_or("");
    let rationale = body["rationale"].as_str().unwrap_or("");
    let alternatives: Vec<String> = body["alternatives"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let result = crate::vision::answer_question(
        &path,
        feature_id,
        question_id,
        answer,
        rationale,
        alternatives,
    );
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/task — Add task to a feature
pub async fn add_vision_task(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let title = body["title"].as_str().unwrap_or("");
    let description = body["description"].as_str().unwrap_or("");
    let branch = body["branch"].as_str();
    let result = crate::vision::add_task(&path, feature_id, title, description, branch);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/task/status — Update task status with Git linking
pub async fn update_vision_task(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let task_id = body["task_id"].as_str().unwrap_or("");
    let status = body["status"].as_str().unwrap_or("");
    let branch = body["branch"].as_str();
    let pr = body["pr"].as_str();
    let commit = body["commit"].as_str();
    let result =
        crate::vision::update_task_status(&path, feature_id, task_id, status, branch, pr, commit);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/git-sync — Sync task statuses from Git
pub async fn git_sync_vision(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let result = crate::vision::sync_git_status(&path);
    maybe_emit_vision_change(&app, &path, &result, None);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/feature/status — Update feature status (planned→specifying→building→testing→done)
pub async fn update_vision_feature_status(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let status = body["status"].as_str().unwrap_or("");
    let result = crate::vision::update_feature_status(&path, feature_id, status);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/work — Assess work against vision
pub async fn assess_vision_work(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let description = body["description"].as_str().unwrap_or("");
    let result = crate::vision::assess_work(&path, description);
    if let Some(focus) =
        crate::vision_focus::upsert_focus_from_work_result(&path, &result, Some("web"))
    {
        maybe_emit_focus_change(&app, &focus);
    }
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

// ── VDD Research & Discovery Docs ──

#[derive(Deserialize, Default)]
pub struct VisionDocQuery {
    pub project: Option<String>,
    pub feature_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct VisionMockupQuery {
    pub project: Option<String>,
    pub feature_id: Option<String>,
    pub option_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct VisionFocusRequest {
    pub project: Option<String>,
    pub path: Option<String>,
    pub goal_id: Option<String>,
    pub feature_id: Option<String>,
    pub source: Option<String>,
}

/// GET /api/vision/focus — Read the persisted active goal/feature focus for a project.
pub async fn get_vision_focus(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    Json(json!({
        "project": q.project,
        "path": path,
        "focus": crate::vision_focus::read_project_focus(&path),
    }))
}

/// GET /api/vision/docs?project=NAME — List all research/discovery docs
pub async fn list_vision_docs(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let docs = collect_vision_docs_for_path(&path);
    Json(json!({ "project": q.project, "docs": docs }))
}

/// GET /api/vision/doc?project=NAME&feature_id=F-XXX — Get a specific research/discovery/design package
pub async fn get_vision_doc(Query(q): Query<VisionDocQuery>) -> Json<Value> {
    let vq = VisionQuery {
        project: q.project.clone(),
        path: None,
    };
    let path = resolve_project_path(&vq);
    let feature_id = q.feature_id.as_deref().unwrap_or("");
    if feature_id.is_empty() {
        return Json(json!({"error": "feature_id required"}));
    }

    let base = std::path::Path::new(&path).join(".vision");
    let mut result = json!({ "feature_id": feature_id });

    // Read research doc
    let research_path = base.join(format!("research/{}.md", feature_id));
    if research_path.exists() {
        let content = std::fs::read_to_string(&research_path).unwrap_or_default();
        result["research"] = json!({
            "content": content,
            "html": markdown_to_html(&content),
        });
    }

    // Read discovery doc
    let discovery_path = base.join(format!("discovery/{}.md", feature_id));
    if discovery_path.exists() {
        let content = std::fs::read_to_string(&discovery_path).unwrap_or_default();
        result["discovery"] = json!({
            "content": content,
            "html": markdown_to_html(&content),
        });
    }

    let design_path = base.join(format!("design/{}.md", feature_id));
    if design_path.exists() {
        let content = std::fs::read_to_string(&design_path).unwrap_or_default();
        result["design"] = json!({
            "content": content,
            "html": markdown_to_html(&content),
        });
    }

    if let Some(vision) = crate::vision::load_vision(&path) {
        if let Some(feature) = vision
            .features
            .iter()
            .find(|feature| feature.id == feature_id)
        {
            result["design_options"] = json!(feature
                .design_options
                .iter()
                .map(|option| {
                    json!({
                        "id": option.id,
                        "title": option.title,
                        "summary": option.summary,
                        "kind": option.kind,
                        "status": option.status,
                        "relative_path": option.relative_path,
                        "reference": option.reference,
                        "approved_by": option.approved_by,
                        "approved_at": option.approved_at,
                        "review_notes": option.review_notes,
                        "preview_url": format!(
                            "/vision/mockup?project={}&feature_id={}&option_id={}",
                            q.project.clone().unwrap_or_default(),
                            feature_id,
                            option.id
                        ),
                    })
                })
                .collect::<Vec<_>>());
        }
    }

    let readiness = crate::vision::feature_readiness(&path, feature_id);
    if let Ok(readiness) = serde_json::from_str::<Value>(&readiness) {
        result["phase"] = readiness.get("phase").cloned().unwrap_or(json!("planned"));
        result["state"] = readiness.get("state").cloned().unwrap_or(json!("planned"));
        result["status"] = readiness.get("status").cloned().unwrap_or(json!("planned"));
        result["title"] = readiness.get("title").cloned().unwrap_or(json!(""));
        result["acceptance_items"] = readiness
            .get("acceptance_items")
            .cloned()
            .unwrap_or(json!([]));
        result["design_options"] = if result.get("design_options").is_some() {
            result["design_options"].clone()
        } else {
            readiness
                .get("design_options")
                .cloned()
                .unwrap_or(json!([]))
        };
        result["readiness"] = readiness.get("readiness").cloned().unwrap_or(json!({}));
    }

    Json(result)
}

/// GET /vision/mockup?project=NAME&feature_id=F-XXX&option_id=MO-XXX — Serve a generated HTML mockup
pub async fn get_vision_mockup(Query(q): Query<VisionMockupQuery>) -> Html<String> {
    let vq = VisionQuery {
        project: q.project.clone(),
        path: None,
    };
    let path = resolve_project_path(&vq);
    let feature_id = q.feature_id.as_deref().unwrap_or("");
    let option_id = q.option_id.as_deref().unwrap_or("");
    if feature_id.is_empty() || option_id.is_empty() {
        return Html("<h1>Missing feature_id or option_id</h1>".to_string());
    }

    match crate::vision::read_mockup_html(&path, feature_id, option_id) {
        Ok(html) => Html(html),
        Err(err) => Html(format!(
            "<h1>Unable to load mockup</h1><p>{}</p>",
            escape_html(&err)
        )),
    }
}

/// POST /api/vision/focus — Persist the operator's active goal/feature focus for auto-continue.
pub async fn set_vision_focus(
    State(app): State<AppState>,
    Json(body): Json<VisionFocusRequest>,
) -> Json<Value> {
    let path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let source = body.source.as_deref().unwrap_or("dashboard");

    let focus = if let Some(feature_id) = body
        .feature_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        crate::vision_focus::upsert_feature_focus(&path, feature_id, Some(source))
    } else {
        crate::vision_focus::upsert_focus(
            &path,
            body.project.as_deref(),
            body.goal_id.as_deref(),
            None,
            Some(source),
        )
    };

    match focus {
        Some(focus) => {
            maybe_emit_focus_change(&app, &focus);
            Json(json!({"status": "focused", "focus": focus}))
        }
        None => Json(json!({"error": "unable_to_set_focus"})),
    }
}

/// POST /api/vision/doc — Create or update a research/discovery doc for a feature
pub async fn upsert_vision_doc(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let doc_type = body["doc_type"]
        .as_str()
        .or_else(|| body["type"].as_str())
        .unwrap_or("");
    let content = body["content"].as_str().unwrap_or("");

    if feature_id.is_empty() || doc_type.is_empty() {
        return Json(json!({"error": "feature_id and doc_type required"}));
    }

    let result = crate::vision::upsert_feature_doc(&path, feature_id, doc_type, content);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/design/mockups/seed — Seed quick client-facing design directions
pub async fn seed_vision_mockups(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let reference = body["reference"].as_str();

    if feature_id.is_empty() {
        return Json(json!({"error": "feature_id required"}));
    }

    let result = crate::vision::seed_mockup_options(&path, feature_id, reference);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/design/review — Approve/reject a design option from the portal
pub async fn review_vision_design(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let feature_id = body["feature_id"].as_str().unwrap_or("");
    let option_id = body["option_id"].as_str().unwrap_or("");
    let status = body["status"].as_str().unwrap_or("approved");
    let note = body["note"].as_str();
    let actor = body["actor"].as_str();

    if feature_id.is_empty() || option_id.is_empty() {
        return Json(json!({"error": "feature_id and option_id required"}));
    }

    let result =
        crate::vision::review_design_option(&path, feature_id, option_id, status, note, actor);
    maybe_emit_vision_change(&app, &path, &result, Some(feature_id));
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

/// POST /api/vision/notify — Best-effort local IPC for hook/external vision mutations
pub async fn notify_vision_change(
    State(app): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let project_path = body["project_path"]
        .as_str()
        .or_else(|| body["path"].as_str())
        .unwrap_or("");
    let result = body["result"].as_str().unwrap_or("");
    let feature_id = body["feature_id"].as_str();

    if project_path.is_empty() || result.is_empty() {
        return Json(json!({"error": "project_path and result required"}));
    }

    maybe_emit_vision_change(&app, project_path, result, feature_id);
    Json(json!({"status": "emitted"}))
}

/// Simple markdown to HTML converter (no external deps)
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let mut list_tag: Option<&'static str> = None;
    let mut in_code = false;
    let mut code_lang = String::new();

    fn close_list(html: &mut String, list_tag: &mut Option<&'static str>) {
        if let Some(tag) = list_tag.take() {
            html.push_str(&format!("</{}>", tag));
        }
    }

    for line in md.lines() {
        // Code blocks
        if line.starts_with("```") {
            if in_code {
                if code_lang.eq_ignore_ascii_case("mermaid") {
                    html.push_str("</div>");
                } else {
                    html.push_str("</code></pre>");
                }
                in_code = false;
                code_lang.clear();
            } else {
                close_list(&mut html, &mut list_tag);
                code_lang = line.trim_start_matches("```").trim().to_string();
                if code_lang.eq_ignore_ascii_case("mermaid") {
                    html.push_str("<div class=\"mermaid\">");
                } else if code_lang.is_empty() {
                    html.push_str("<pre class=\"code-block\"><code>");
                } else {
                    html.push_str(&format!(
                        "<pre class=\"code-block\"><code class=\"language-{}\">",
                        escape_html(&code_lang)
                    ));
                }
                in_code = true;
            }
            continue;
        }
        if in_code {
            html.push_str(&line.replace('<', "&lt;").replace('>', "&gt;"));
            html.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            close_list(&mut html, &mut list_tag);
            continue;
        }

        // Headers
        if trimmed.starts_with("### ") {
            close_list(&mut html, &mut list_tag);
            html.push_str(&format!("<h3>{}</h3>", escape_html(&trimmed[4..])));
        } else if trimmed.starts_with("## ") {
            close_list(&mut html, &mut list_tag);
            html.push_str(&format!("<h2>{}</h2>", escape_html(&trimmed[3..])));
        } else if trimmed.starts_with("# ") {
            close_list(&mut html, &mut list_tag);
            html.push_str(&format!("<h1>{}</h1>", escape_html(&trimmed[2..])));
        }
        // Horizontal rules
        else if trimmed == "---" || trimmed == "***" {
            close_list(&mut html, &mut list_tag);
            html.push_str("<hr>");
        }
        // Blockquotes
        else if trimmed.starts_with("> ") {
            close_list(&mut html, &mut list_tag);
            html.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>",
                inline_md(&trimmed[2..])
            ));
        }
        // List items
        else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if list_tag != Some("ul") {
                close_list(&mut html, &mut list_tag);
                html.push_str("<ul>");
                list_tag = Some("ul");
            }
            html.push_str(&format!("<li>{}</li>", inline_md(&trimmed[2..])));
        }
        // Numbered lists
        else if trimmed.len() > 2
            && trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            && trimmed.contains(". ")
        {
            if let Some(pos) = trimmed.find(". ") {
                if list_tag != Some("ol") {
                    close_list(&mut html, &mut list_tag);
                    html.push_str("<ol>");
                    list_tag = Some("ol");
                }
                html.push_str(&format!("<li>{}</li>", inline_md(&trimmed[pos + 2..])));
            }
        }
        // Regular paragraph
        else {
            close_list(&mut html, &mut list_tag);
            html.push_str(&format!("<p>{}</p>", inline_md(trimmed)));
        }
    }
    close_list(&mut html, &mut list_tag);
    if in_code {
        if code_lang.eq_ignore_ascii_case("mermaid") {
            html.push_str("</div>");
        } else {
            html.push_str("</code></pre>");
        }
    }
    html
}

#[derive(Clone, Debug)]
struct WikiDocEntry {
    id: String,
    title: String,
    summary: String,
    category: String,
    relative_path: String,
    html: String,
}

fn slugify_doc_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn markdown_title_and_summary(md: &str, fallback: &str) -> (String, String) {
    let mut title = fallback.to_string();
    let mut summary = String::new();

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            title = trimmed.trim_start_matches("# ").trim().to_string();
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("```")
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
        {
            continue;
        }
        let looks_numbered = trimmed
            .split_once(". ")
            .map(|(prefix, _)| prefix.chars().all(|ch| ch.is_ascii_digit()))
            .unwrap_or(false);
        if looks_numbered {
            continue;
        }
        summary = trimmed.to_string();
        break;
    }

    (title, summary)
}

fn load_wiki_doc(project_path: &str, relative_path: &str, category: &str) -> Option<WikiDocEntry> {
    let path = FsPath::new(project_path).join(relative_path);
    let content = std::fs::read_to_string(&path).ok()?;
    let fallback = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document");
    let (title, summary) = markdown_title_and_summary(&content, fallback);
    Some(WikiDocEntry {
        id: slugify_doc_id(relative_path),
        title,
        summary,
        category: category.to_string(),
        relative_path: relative_path.to_string(),
        html: markdown_to_html(&content),
    })
}

fn collect_featured_wiki_docs(project_path: &str) -> Vec<WikiDocEntry> {
    let candidates = [
        ("README.md", "overview"),
        ("docs/DXOS_MASTER_ARCHITECTURE.md", "architecture"),
        ("docs/NON_TECH_GUIDE.md", "guide"),
        ("docs/CLIENT_ONBOARDING_BLUEPRINT.md", "client"),
        ("docs/PROJECT_ADOPTION_AND_RECOVERY.md", "recovery"),
        ("docs/CLIENT_PORTAL_PLAYBOOK.md", "client"),
        ("docs/DISCOVERY_AND_DESIGN_PLAYBOOK.md", "design"),
        ("docs/OPERATOR_SYSTEM_GUIDE.md", "operations"),
        ("docs/EXPERIENCE_BLUEPRINT.md", "experience"),
        ("docs/ARCHITECTURE_BLUEPRINT.md", "architecture"),
        ("docs/HOSTED_SYNC_MODEL.md", "sync"),
        ("docs/HISTORY_OF_DX_TERMINAL.md", "history"),
    ];

    candidates
        .into_iter()
        .filter_map(|(relative_path, category)| {
            load_wiki_doc(project_path, relative_path, category)
        })
        .collect()
}

fn collect_scoped_wiki_docs(
    project_path: &str,
    relative_dir: &str,
    category: &str,
) -> Vec<WikiDocEntry> {
    let docs_dir = FsPath::new(project_path).join(relative_dir);
    let Ok(entries) = std::fs::read_dir(&docs_dir) else {
        return Vec::new();
    };

    let mut docs = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
                return None;
            }
            let relative_path = path
                .strip_prefix(project_path)
                .ok()?
                .to_string_lossy()
                .to_string();
            load_wiki_doc(project_path, &relative_path, category)
        })
        .collect::<Vec<_>>();

    docs.sort_by(|left, right| left.title.cmp(&right.title));
    docs
}

fn collect_library_wiki_docs(project_path: &str, exclude: &HashSet<String>) -> Vec<WikiDocEntry> {
    let docs_dir = FsPath::new(project_path).join("docs");
    let Ok(entries) = std::fs::read_dir(&docs_dir) else {
        return Vec::new();
    };

    let mut docs = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
                return None;
            }
            let relative_path = path
                .strip_prefix(project_path)
                .ok()?
                .to_string_lossy()
                .to_string();
            if exclude.contains(&relative_path) {
                return None;
            }
            load_wiki_doc(project_path, &relative_path, "library")
        })
        .collect::<Vec<_>>();

    docs.sort_by(|left, right| left.title.cmp(&right.title));
    docs
}

/// Inline markdown: **bold**, `code`, *italic*
fn inline_md(text: &str) -> String {
    let escaped = escape_html(text);
    // Bold
    let mut result = String::new();
    let mut rest = escaped.as_str();
    while let Some(start) = rest.find("**") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("**") {
            result.push_str("<strong>");
            result.push_str(&rest[..end]);
            result.push_str("</strong>");
            rest = &rest[end + 2..];
        } else {
            result.push_str("**");
        }
    }
    result.push_str(rest);

    // Inline code
    let mut final_result = String::new();
    rest = result.as_str();
    while let Some(start) = rest.find('`') {
        final_result.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('`') {
            final_result.push_str("<code>");
            final_result.push_str(&rest[..end]);
            final_result.push_str("</code>");
            rest = &rest[end + 1..];
        } else {
            final_result.push('`');
        }
    }
    final_result.push_str(rest);
    final_result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── Confluence-Style Wiki ──

/// GET /wiki?project=NAME — Serve a full Confluence-style wiki page for a project's vision
pub async fn wiki_page(Query(q): Query<VisionQuery>) -> Html<String> {
    let path = resolve_project_path(&q);
    let vision_file = std::path::Path::new(&path).join(".vision/vision.json");

    let vision: Value = if vision_file.exists() {
        let content = std::fs::read_to_string(&vision_file).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({"error": "No vision found"})
    };

    let project = vision["project"].as_str().unwrap_or("Unknown");
    let mission = vision["mission"].as_str().unwrap_or("");
    let updated = vision["updated_at"].as_str().unwrap_or("");
    let focus = crate::vision_focus::read_project_focus(&path);
    let featured_docs = collect_featured_wiki_docs(&path);
    let featured_paths = featured_docs
        .iter()
        .map(|doc| doc.relative_path.clone())
        .collect::<HashSet<_>>();
    let library_docs = collect_library_wiki_docs(&path, &featured_paths);
    let research_docs = collect_scoped_wiki_docs(&path, ".vision/research", "research");
    let discovery_docs = collect_scoped_wiki_docs(&path, ".vision/discovery", "discovery");
    let design_docs = collect_scoped_wiki_docs(&path, ".vision/design", "design");

    let phase_tone = |value: &str| -> &'static str {
        match value {
            "discovery" | "specifying" => "discovery",
            "build" | "building" | "in_progress" => "build",
            "test" | "testing" => "test",
            "done" | "achieved" | "verified" | "complete" => "done",
            "blocked" | "failed" | "dropped" => "blocked",
            _ => "planned",
        }
    };
    let doc_tone = |value: &str| -> &'static str {
        match value {
            "guide" | "operations" => "guide",
            "experience" => "experience",
            "architecture" => "architecture",
            "client" => "sync",
            "design" => "design",
            "sync" => "sync",
            "history" => "history",
            "research" => "research",
            "discovery" => "discovery",
            "overview" => "overview",
            _ => "library",
        }
    };

    let principles = vision["principles"].as_array().cloned().unwrap_or_default();
    let goals = vision["goals"].as_array().cloned().unwrap_or_default();
    let features = vision["features"].as_array().cloned().unwrap_or_default();
    let adrs = vision["architecture"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let milestones = vision["milestones"].as_array().cloned().unwrap_or_default();
    let changes = vision["changes"].as_array().cloned().unwrap_or_default();

    let mut phase_counts: HashMap<String, usize> = HashMap::new();
    let mut ready_features = 0usize;
    let mut blocked_features = 0usize;

    let mut goals_html = String::new();
    for goal in &goals {
        let id = goal
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let title = goal
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("Untitled goal");
        let description = goal
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let status = goal
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("planned");
        let priority = goal
            .get("priority")
            .and_then(|value| value.as_u64())
            .unwrap_or(3);
        let goal_metrics = goal
            .get("metrics")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(|metric| {
                        format!("<span class=\"mini-chip\">{}</span>", escape_html(metric))
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        goals_html.push_str(&format!(
            r#"<article class="wiki-card">
                <div class="wiki-card-top">
                    <div class="wiki-heading-group">
                        <span class="wiki-id">{}</span>
                        <h3>{}</h3>
                    </div>
                    <div class="wiki-tag-row">
                        <span class="tone tone-{}">{}</span>
                        <span class="mini-chip">Priority P{}</span>
                    </div>
                </div>
                <p class="wiki-copy">{}</p>
                {}
            </article>"#,
            escape_html(id),
            escape_html(title),
            phase_tone(status),
            escape_html(status),
            priority,
            escape_html(description),
            if goal_metrics.is_empty() {
                String::new()
            } else {
                format!("<div class=\"mini-chip-row\">{}</div>", goal_metrics)
            }
        ));
    }

    let mut features_html = String::new();
    for feature in &features {
        let id = feature
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let title = feature
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("Untitled feature");
        let description = feature
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let goal_id = feature
            .get("goal_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let phase_raw = feature
            .get("phase")
            .or_else(|| feature.get("status"))
            .and_then(|value| value.as_str())
            .unwrap_or("planned");
        let phase = phase_tone(phase_raw);
        let state = feature
            .get("state")
            .and_then(|value| value.as_str())
            .unwrap_or("active");
        *phase_counts.entry(phase.to_string()).or_insert(0) += 1;

        let readiness = feature
            .get("readiness")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let next_gate = match phase {
            "planned" | "discovery" => Some("build"),
            "build" => Some("test"),
            "test" => Some("done"),
            _ => None,
        };
        let ready_for_gate = next_gate
            .map(|gate| {
                let ready_key = format!("ready_for_{}", gate);
                readiness
                    .get(&ready_key)
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let blockers = next_gate
            .and_then(|gate| {
                readiness
                    .get("blockers")
                    .and_then(|value| value.get(gate))
                    .and_then(|value| value.as_array())
                    .cloned()
            })
            .unwrap_or_default();
        if ready_for_gate {
            ready_features += 1;
        } else if !blockers.is_empty() {
            blocked_features += 1;
        }

        let tasks_html = feature
            .get("tasks")
            .and_then(|value| value.as_array())
            .map(|tasks| {
                tasks
                    .iter()
                    .map(|task| {
                        let task_title = task
                            .get("title")
                            .and_then(|value| value.as_str())
                            .unwrap_or("Untitled task");
                        let task_status = task
                            .get("status")
                            .and_then(|value| value.as_str())
                            .unwrap_or("planned");
                        let branch = task
                            .get("branch")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        format!(
                            r#"<li class="detail-row">
                                <span class="tone tone-{}">{}</span>
                                <div>
                                    <div class="detail-title">{}</div>
                                    {}
                                </div>
                            </li>"#,
                            phase_tone(task_status),
                            escape_html(task_status),
                            escape_html(task_title),
                            if branch.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "<div class=\"detail-meta\">branch <code>{}</code></div>",
                                    escape_html(branch)
                                )
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let acceptance_items = if let Some(items) = feature
            .get("acceptance_items")
            .and_then(|value| value.as_array())
        {
            items
                .iter()
                .map(|item| {
                    let text = item
                        .get("text")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Unnamed acceptance criterion");
                    let status = item
                        .get("status")
                        .and_then(|value| value.as_str())
                        .unwrap_or("draft");
                    let meta = [
                        item.get("verification_method")
                            .and_then(|value| value.as_str()),
                        item.get("verification_source")
                            .and_then(|value| value.as_str()),
                        item.get("verified_by").and_then(|value| value.as_str()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                    format!(
                        r#"<li class="detail-row">
                            <span class="tone tone-{}">{}</span>
                            <div>
                                <div class="detail-title">{}</div>
                                {}
                            </div>
                        </li>"#,
                        phase_tone(status),
                        escape_html(status),
                        escape_html(text),
                        if meta.is_empty() {
                            String::new()
                        } else {
                            format!("<div class=\"detail-meta\">{}</div>", escape_html(&meta))
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        } else {
            feature
                .get("acceptance_criteria")
                .and_then(|value| value.as_array())
                .map(|criteria| {
                    criteria
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(|item| {
                            format!(
                                r#"<li class="detail-row">
                                    <span class="tone tone-{}">{}</span>
                                    <div><div class="detail-title">{}</div></div>
                                </li>"#,
                                if phase == "done" { "done" } else { "planned" },
                                if phase == "done" { "verified" } else { "draft" },
                                escape_html(item)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default()
        };

        let questions_html = feature
            .get("questions")
            .and_then(|value| value.as_array())
            .map(|questions| {
                questions
                    .iter()
                    .map(|question| {
                        let text = question
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or("Unnamed question");
                        let status = question
                            .get("status")
                            .and_then(|value| value.as_str())
                            .unwrap_or("open");
                        let blocking = question
                            .get("blocking")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(true);
                        let answer = question
                            .get("answer")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        format!(
                            r#"<li class="detail-row">
                                <span class="tone tone-{}">{}</span>
                                <div>
                                    <div class="detail-title">{}</div>
                                    <div class="detail-meta">{}</div>
                                    {}
                                </div>
                            </li>"#,
                            if status == "answered" {
                                "done"
                            } else {
                                "discovery"
                            },
                            escape_html(status),
                            escape_html(text),
                            if blocking {
                                "blocking question".to_string()
                            } else {
                                "non-blocking question".to_string()
                            },
                            if answer.is_empty() {
                                String::new()
                            } else {
                                format!("<div class=\"detail-meta\">{}</div>", escape_html(answer))
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let decisions_html = feature
            .get("decisions")
            .and_then(|value| value.as_array())
            .map(|decisions| {
                decisions
                    .iter()
                    .map(|decision| {
                        let choice = decision
                            .get("decision")
                            .and_then(|value| value.as_str())
                            .unwrap_or("Unnamed decision");
                        let rationale = decision
                            .get("rationale")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        format!(
                            r#"<li class="detail-row">
                                <span class="tone tone-architecture">decision</span>
                                <div>
                                    <div class="detail-title">{}</div>
                                    {}
                                </div>
                            </li>"#,
                            escape_html(choice),
                            if rationale.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "<div class=\"detail-meta\">{}</div>",
                                    escape_html(rationale)
                                )
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let focus_match = focus
            .as_ref()
            .and_then(|value| value.feature_id.as_deref())
            .map(|feature_id| feature_id == id)
            .unwrap_or(false);
        let blockers_html = if blockers.is_empty() {
            String::new()
        } else {
            format!(
                "<div class=\"mini-chip-row\">{}</div>",
                blockers
                    .iter()
                    .filter_map(|blocker| blocker.as_str())
                    .map(|blocker| format!(
                        "<span class=\"mini-chip mini-chip-alert\">{}</span>",
                        escape_html(blocker)
                    ))
                    .collect::<Vec<_>>()
                    .join("")
            )
        };
        let readiness_html = match next_gate {
            Some(gate) if ready_for_gate => format!(
                "<div class=\"detail-meta\"><strong>Next gate:</strong> ready for {}</div>",
                escape_html(gate)
            ),
            Some(gate) => format!(
                "<div class=\"detail-meta\"><strong>Next gate:</strong> blocked before {}</div>",
                escape_html(gate)
            ),
            None => {
                "<div class=\"detail-meta\"><strong>Lifecycle:</strong> complete</div>".to_string()
            }
        };

        features_html.push_str(&format!(
            r#"<details class="wiki-expandable" id="feature-{}" {}>
                <summary>
                    <div class="wiki-card-top">
                        <div class="wiki-heading-group">
                            <span class="wiki-id">{}</span>
                            <h3>{}</h3>
                        </div>
                        <div class="wiki-tag-row">
                            {}
                            <span class="tone tone-{}">{}</span>
                            <span class="tone tone-{}">{}</span>
                            <span class="mini-chip">goal {}</span>
                            {}
                        </div>
                    </div>
                    <p class="wiki-copy">{}</p>
                    {}
                    {}
                </summary>
                <div class="wiki-detail-grid">
                    {}
                    {}
                    {}
                    {}
                </div>
            </details>"#,
            slugify_doc_id(id),
            if focus_match || phase != "done" { "open" } else { "" },
            escape_html(id),
            escape_html(title),
            if focus_match {
                "<span class=\"tone tone-overview\">focus</span>".to_string()
            } else {
                String::new()
            },
            phase,
            escape_html(phase_raw),
            phase_tone(state),
            escape_html(state),
            escape_html(goal_id),
            if feature
                .get("has_sub_vision")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                "<span class=\"tone tone-architecture\">sub-vision</span>".to_string()
            } else {
                String::new()
            },
            escape_html(description),
            readiness_html,
            blockers_html,
            if tasks_html.is_empty() {
                "<div class=\"detail-block\"><h4>Build Tasks</h4><p class=\"wiki-empty\">No implementation tasks recorded yet.</p></div>".to_string()
            } else {
                format!("<div class=\"detail-block\"><h4>Build Tasks</h4><ul class=\"detail-list\">{}</ul></div>", tasks_html)
            },
            if acceptance_items.is_empty() {
                "<div class=\"detail-block\"><h4>Acceptance</h4><p class=\"wiki-empty\">Acceptance criteria have not been documented yet.</p></div>".to_string()
            } else {
                format!("<div class=\"detail-block\"><h4>Acceptance</h4><ul class=\"detail-list\">{}</ul></div>", acceptance_items)
            },
            if questions_html.is_empty() {
                "<div class=\"detail-block\"><h4>Questions</h4><p class=\"wiki-empty\">No open discovery questions are recorded.</p></div>".to_string()
            } else {
                format!("<div class=\"detail-block\"><h4>Questions</h4><ul class=\"detail-list\">{}</ul></div>", questions_html)
            },
            if decisions_html.is_empty() {
                "<div class=\"detail-block\"><h4>Decisions</h4><p class=\"wiki-empty\">No explicit implementation decisions are logged yet.</p></div>".to_string()
            } else {
                format!("<div class=\"detail-block\"><h4>Decisions</h4><ul class=\"detail-list\">{}</ul></div>", decisions_html)
            }
        ));
    }

    let milestones_html = milestones
        .iter()
        .map(|milestone| {
            let id = milestone
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let title = milestone
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Untitled milestone");
            let description = milestone
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let status = milestone
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("planned");
            let target_date = milestone
                .get("target_date")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let pct = milestone
                .get("progress_pct")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            format!(
                r#"<article class="wiki-card">
                    <div class="wiki-card-top">
                        <div class="wiki-heading-group">
                            <span class="wiki-id">{}</span>
                            <h3>{}</h3>
                        </div>
                        <div class="wiki-tag-row">
                            <span class="tone tone-{}">{}</span>
                            <span class="mini-chip">{}</span>
                            <span class="mini-chip">{}%</span>
                        </div>
                    </div>
                    <p class="wiki-copy">{}</p>
                    <div class="progress-track"><div class="progress-fill tone-fill-{}" style="width:{}%"></div></div>
                </article>"#,
                escape_html(id),
                escape_html(title),
                phase_tone(status),
                escape_html(status),
                if target_date.is_empty() {
                    "No target date".to_string()
                } else {
                    escape_html(target_date)
                },
                pct,
                escape_html(description),
                phase_tone(status),
                pct.min(100)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let adr_html = adrs
        .iter()
        .map(|adr| {
            let id = adr.get("id").and_then(|value| value.as_str()).unwrap_or("");
            let title = adr
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Untitled decision");
            let decision = adr
                .get("decision")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let rationale = adr
                .get("rationale")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let date = adr
                .get("date")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let status = adr
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("active");
            let alternatives = adr
                .get("alternatives_considered")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!(
                r#"<article class="wiki-card">
                    <div class="wiki-card-top">
                        <div class="wiki-heading-group">
                            <span class="wiki-id">{}</span>
                            <h3>{}</h3>
                        </div>
                        <div class="wiki-tag-row">
                            <span class="tone tone-architecture">{}</span>
                            {}
                        </div>
                    </div>
                    <div class="wiki-rich-text">
                        <p><strong>Decision:</strong> {}</p>
                        <p><strong>Rationale:</strong> {}</p>
                        {}
                    </div>
                </article>"#,
                escape_html(id),
                escape_html(title),
                escape_html(status),
                if date.is_empty() {
                    String::new()
                } else {
                    format!("<span class=\"mini-chip\">{}</span>", escape_html(date))
                },
                escape_html(decision),
                escape_html(rationale),
                if alternatives.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<p><strong>Alternatives considered:</strong> {}</p>",
                        escape_html(&alternatives)
                    )
                }
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let changes_html = changes
        .iter()
        .rev()
        .take(25)
        .map(|change| {
            let timestamp = change
                .get("timestamp")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let change_type = change
                .get("change_type")
                .and_then(|value| value.as_str())
                .unwrap_or("updated");
            let field = change
                .get("field")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let reason = change
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let by = change
                .get("triggered_by")
                .and_then(|value| value.as_str())
                .unwrap_or("system");
            format!(
                r#"<div class="timeline-item">
                    <div class="timeline-top">
                        <span class="tone tone-{}">{}</span>
                        <span class="timeline-date">{}</span>
                    </div>
                    <div class="detail-title"><code>{}</code></div>
                    {}
                    <div class="detail-meta">by {}</div>
                </div>"#,
                if change_type == "status_change" {
                    "build"
                } else if change_type == "added" {
                    "done"
                } else {
                    "overview"
                },
                escape_html(change_type),
                escape_html(timestamp.get(..16).unwrap_or(timestamp)),
                escape_html(field),
                if reason.is_empty() {
                    String::new()
                } else {
                    format!("<div class=\"detail-meta\">{}</div>", escape_html(reason))
                },
                escape_html(by)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let featured_cards_html = featured_docs
        .iter()
        .map(|doc| {
            format!(
                r##"<a class="handbook-card" href="#doc-{}">
                    <span class="tone tone-{}">{}</span>
                    <h3>{}</h3>
                    <p>{}</p>
                    <div class="card-path">{}</div>
                </a>"##,
                escape_html(&doc.id),
                doc_tone(&doc.category),
                escape_html(&doc.category),
                escape_html(&doc.title),
                escape_html(if doc.summary.is_empty() {
                    "Open this handbook page for the full narrative."
                } else {
                    &doc.summary
                }),
                escape_html(&doc.relative_path)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let featured_articles_html = featured_docs
        .iter()
        .map(|doc| {
            format!(
                r##"<article class="wiki-doc-article" id="doc-{}">
                    <div class="wiki-card-top">
                        <div class="wiki-heading-group">
                            <span class="tone tone-{}">{}</span>
                            <h3>{}</h3>
                        </div>
                        <a class="text-link" href="#top">Back to top</a>
                    </div>
                    <p class="wiki-copy">{}</p>
                    <div class="card-path">{}</div>
                    <div class="wiki-rich-text">{}</div>
                </article>"##,
                escape_html(&doc.id),
                doc_tone(&doc.category),
                escape_html(&doc.category),
                escape_html(&doc.title),
                escape_html(if doc.summary.is_empty() {
                    "Narrative document"
                } else {
                    &doc.summary
                }),
                escape_html(&doc.relative_path),
                doc.html
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let research_collections = [
        ("Research Notes", "research", &research_docs),
        ("Discovery Notes", "discovery", &discovery_docs),
        ("Design Briefs", "design", &design_docs),
    ];
    let research_html = research_collections
        .iter()
        .map(|(title, tone, docs)| {
            let items = docs
                .iter()
                .map(|doc| {
                    format!(
                        r#"<details class="wiki-expandable">
                            <summary>
                                <div class="wiki-card-top">
                                    <div class="wiki-heading-group">
                                        <span class="tone tone-{}">{}</span>
                                        <h3>{}</h3>
                                    </div>
                                    <span class="mini-chip">{}</span>
                                </div>
                                <p class="wiki-copy">{}</p>
                            </summary>
                            <div class="wiki-rich-text">{}</div>
                        </details>"#,
                        doc_tone(&doc.category),
                        escape_html(&doc.category),
                        escape_html(&doc.title),
                        escape_html(&doc.relative_path),
                        escape_html(if doc.summary.is_empty() {
                            "Operational note"
                        } else {
                            &doc.summary
                        }),
                        doc.html
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<div class="doc-collection">
                    <div class="wiki-card-top">
                        <div class="wiki-heading-group">
                            <span class="tone tone-{}">{}</span>
                            <h3>{}</h3>
                        </div>
                        <span class="mini-chip">{} documents</span>
                    </div>
                    {}
                </div>"#,
                tone,
                escape_html(tone),
                escape_html(title),
                docs.len(),
                if items.is_empty() {
                    "<p class=\"wiki-empty\">No documents in this collection yet.</p>".to_string()
                } else {
                    items
                }
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let library_html = library_docs
        .iter()
        .map(|doc| {
            format!(
                r#"<details class="wiki-expandable">
                    <summary>
                        <div class="wiki-card-top">
                            <div class="wiki-heading-group">
                                <span class="tone tone-{}">{}</span>
                                <h3>{}</h3>
                            </div>
                            <span class="card-path">{}</span>
                        </div>
                        <p class="wiki-copy">{}</p>
                    </summary>
                    <div class="wiki-rich-text">{}</div>
                </details>"#,
                doc_tone(&doc.category),
                escape_html(&doc.category),
                escape_html(&doc.title),
                escape_html(&doc.relative_path),
                escape_html(if doc.summary.is_empty() {
                    "Library document"
                } else {
                    &doc.summary
                }),
                doc.html
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let principles_html = if principles.is_empty() {
        "<p class=\"wiki-empty\">No explicit project principles are recorded yet.</p>".to_string()
    } else {
        format!(
            "<div class=\"principle-grid\">{}</div>",
            principles
                .iter()
                .filter_map(|value| value.as_str())
                .map(|principle| format!(
                    "<div class=\"principle-item\">{}</div>",
                    escape_html(principle)
                ))
                .collect::<Vec<_>>()
                .join("")
        )
    };

    let focus_html = if let Some(entry) = focus {
        let focus_source = entry
            .source
            .as_deref()
            .map(|source| format!("set by {}", source))
            .unwrap_or_else(|| "shared focus".to_string());
        let focus_updated = entry
            .updated_at
            .as_deref()
            .map(|value| format!("updated {}", value))
            .unwrap_or_else(|| "recent".to_string());
        format!(
            r#"<div class="focus-card">
                <span class="tone tone-overview">active focus</span>
                <div class="focus-title">{}</div>
                <div class="detail-meta">{}</div>
                <div class="detail-meta">{}</div>
            </div>"#,
            escape_html(
                entry
                    .feature_id
                    .as_deref()
                    .or(entry.goal_id.as_deref())
                    .unwrap_or(project)
            ),
            escape_html(&focus_source),
            escape_html(&focus_updated)
        )
    } else {
        "<div class=\"focus-card\"><span class=\"tone tone-planned\">no shared focus</span><div class=\"detail-meta\">Set focus from the dashboard, MCP, or hook flow to keep auto-continue and docs aligned.</div></div>".to_string()
    };

    let feature_count = features.len();
    let done_features = phase_counts.get("done").copied().unwrap_or(0);
    let stage_story = if feature_count > 0 && done_features == feature_count {
        "All currently tracked features are marked complete, so this handbook is acting as an execution record and reference surface.".to_string()
    } else if blocked_features > 0 {
        format!(
            "{} feature(s) currently have explicit blockers before their next gate.",
            blocked_features
        )
    } else if ready_features > 0 {
        format!(
            "{} feature(s) are ready to advance to the next delivery gate.",
            ready_features
        )
    } else {
        "The plan is present, but no feature is fully gate-ready yet.".to_string()
    };

    let phase_cards_html = ["planned", "discovery", "build", "test", "done"]
        .iter()
        .map(|phase| {
            format!(
                r#"<div class="stat-card">
                    <div class="stat-label">{}</div>
                    <div class="stat-value">{}</div>
                    <div class="detail-meta">phase count</div>
                </div>"#,
                escape_html(phase),
                phase_counts.get(*phase).copied().unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let sidebar_doc_links = featured_docs
        .iter()
        .map(|doc| {
            format!(
                "<a href=\"#doc-{}\">{}</a>",
                escape_html(&doc.id),
                escape_html(&doc.title)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{project} — DX Handbook</title>
<style>
:root {{
    --paper:#08111d;
    --paper-strong:#0e1b2d;
    --ink:#eef4ff;
    --muted:#9db0cb;
    --line:#20324d;
    --line-strong:#32517c;
    --navy:#06b6d4;
    --teal:#22d3ee;
    --gold:#f59e0b;
    --green:#34d399;
    --coral:#fb7185;
    --slate:#8ea0bb;
    --violet:#a855f7;
    --pink:#ff00aa;
    --shadow:0 24px 70px rgba(2,6,23,.45);
    --radius:24px;
    --radius-sm:12px;
    --serif:'Avenir Next','IBM Plex Sans','Segoe UI','Helvetica Neue',sans-serif;
    --sans:'Avenir Next','IBM Plex Sans','Segoe UI','Helvetica Neue',sans-serif;
    --mono:'SF Mono','JetBrains Mono','Cascadia Code',monospace;
}}
* {{ box-sizing:border-box; }}
html {{ scroll-behavior:smooth; }}
body {{
    margin:0;
    font-family:var(--sans);
    color:var(--ink);
    background:
        radial-gradient(circle at top left, rgba(6,182,212,.14), transparent 30%),
        radial-gradient(circle at top right, rgba(168,85,247,.14), transparent 30%),
        radial-gradient(circle at bottom right, rgba(255,0,170,.1), transparent 26%),
        linear-gradient(180deg, #050b14 0%, var(--paper) 100%);
    line-height:1.7;
}}
a {{ color:var(--navy); text-decoration:none; }}
a:hover {{ text-decoration:underline; }}
code {{ font-family:var(--mono); }}
.wiki-shell {{
    max-width:1480px;
    margin:0 auto;
    padding:28px 24px 48px;
    display:grid;
    grid-template-columns:280px minmax(0, 1fr);
    gap:28px;
}}
.wiki-sidebar {{
    position:sticky;
    top:24px;
    align-self:start;
    display:grid;
    gap:16px;
}}
.sidebar-card,
.section-card,
.wiki-card,
.wiki-doc-article,
.doc-collection {{
    background:linear-gradient(180deg, rgba(255,255,255,.03), rgba(255,255,255,.015));
    border:1px solid var(--line);
    border-radius:var(--radius);
    box-shadow:var(--shadow);
    backdrop-filter:blur(18px);
}}
.sidebar-card {{ padding:18px; }}
.section-card {{ padding:24px; margin-bottom:22px; }}
.brand {{
    display:flex;
    align-items:center;
    gap:12px;
    margin-bottom:10px;
}}
.brand-mark {{
    width:54px;
    height:54px;
    display:flex;
    align-items:center;
    justify-content:center;
    filter:drop-shadow(0 16px 34px rgba(6,182,212,.18));
}}
.brand-mark svg {{
    width:100%;
    height:100%;
    display:block;
}}
.brand-copy h1 {{
    margin:0;
    font-size:21px;
    font-weight:900;
    letter-spacing:-0.04em;
}}
.brand-copy p,
.sidebar-copy,
.wiki-copy,
.detail-meta,
.wiki-empty,
.card-path {{
    color:var(--muted);
}}
.sidebar-copy,
.wiki-copy {{
    font-size:14px;
}}
.sidebar-title,
.section-kicker {{
    font-size:11px;
    text-transform:uppercase;
    letter-spacing:.14em;
    font-weight:800;
    color:var(--navy);
    margin-bottom:10px;
}}
.sidebar-actions {{
    display:flex;
    gap:8px;
    flex-wrap:wrap;
    margin-top:14px;
}}
.sidebar-actions a,
.hero-actions a {{
    display:inline-flex;
    align-items:center;
    justify-content:center;
    padding:10px 14px;
    border-radius:999px;
    border:1px solid var(--line-strong);
    background:rgba(255,255,255,.02);
    font-size:12px;
    font-weight:700;
    color:var(--ink);
}}
.sidebar-actions a.primary,
.hero-actions a.primary {{
    background:linear-gradient(90deg, rgba(6,182,212,.18), rgba(168,85,247,.18));
    color:var(--ink);
    border-color:rgba(79,143,247,.25);
}}
.sidebar-nav {{
    display:grid;
    gap:6px;
}}
.sidebar-nav a {{
    padding:8px 10px;
    border-radius:10px;
    color:var(--ink);
    border:1px solid transparent;
}}
.sidebar-nav a:hover,
.sidebar-nav a.is-active {{
    background:rgba(6,182,212,.08);
    border-color:rgba(79,143,247,.18);
    text-decoration:none;
}}
.focus-card {{
    display:grid;
    gap:6px;
}}
.focus-title {{
    font-size:16px;
    font-weight:800;
}}
.hero {{
    padding:34px;
    background:
        radial-gradient(circle at top right, rgba(168,85,247,.12), transparent 35%),
        linear-gradient(180deg, rgba(255,255,255,.03), rgba(255,255,255,.015));
}}
.hero-top {{
    display:flex;
    align-items:flex-start;
    justify-content:space-between;
    gap:18px;
    flex-wrap:wrap;
}}
.hero h1 {{
    margin:6px 0 8px;
    font-size:48px;
    line-height:1.02;
    font-family:var(--serif);
    font-weight:900;
    letter-spacing:-0.05em;
}}
.hero-mission {{
    font-size:18px;
    max-width:70ch;
    color:var(--muted);
}}
.hero-meta {{
    display:flex;
    gap:12px;
    flex-wrap:wrap;
    margin-top:16px;
}}
.hero-actions {{
    display:flex;
    gap:10px;
    flex-wrap:wrap;
}}
.meta-chip,
.mini-chip {{
    display:inline-flex;
    align-items:center;
    gap:6px;
    padding:6px 10px;
    border-radius:999px;
    border:1px solid var(--line);
    background:rgba(255,255,255,.03);
    font-size:12px;
    color:var(--muted);
}}
.mini-chip {{
    font-size:11px;
    padding:5px 8px;
}}
.mini-chip-alert {{
    color:var(--coral);
    border-color:rgba(160,77,95,.16);
    background:rgba(160,77,95,.08);
}}
.stats-grid,
.handbook-grid,
.principle-grid,
.wiki-detail-grid {{
    display:grid;
    gap:12px;
}}
.stats-grid {{
    grid-template-columns:repeat(6, minmax(0, 1fr));
    margin-top:24px;
}}
.handbook-grid {{
    grid-template-columns:repeat(3, minmax(0, 1fr));
}}
.principle-grid,
.wiki-detail-grid {{
    grid-template-columns:repeat(2, minmax(0, 1fr));
}}
.stat-card,
.handbook-card,
.detail-block,
.principle-item {{
    padding:16px;
    border:1px solid var(--line);
    border-radius:16px;
    background:rgba(255,255,255,.022);
}}
.stat-label {{
    font-size:11px;
    text-transform:uppercase;
    letter-spacing:.12em;
    font-weight:800;
    color:var(--muted);
}}
.stat-value {{
    font-size:28px;
    font-weight:800;
    margin-top:6px;
    letter-spacing:-0.03em;
}}
.section-head {{
    display:flex;
    align-items:flex-end;
    justify-content:space-between;
    gap:16px;
    margin-bottom:18px;
    flex-wrap:wrap;
}}
.section-head h2 {{
    margin:0;
    font-size:30px;
    font-family:var(--serif);
    letter-spacing:-0.04em;
    font-weight:900;
}}
.section-head p {{
    margin:0;
    max-width:68ch;
    color:var(--muted);
}}
.phase-band {{
    display:grid;
    grid-template-columns:repeat(5, minmax(0, 1fr));
    gap:10px;
    margin-bottom:18px;
}}
.phase-band .stat-card {{
    min-height:118px;
}}
.wiki-card,
.wiki-doc-article,
.doc-collection {{
    padding:18px;
}}
.wiki-card + .wiki-card,
.wiki-doc-article + .wiki-doc-article,
.doc-collection + .doc-collection,
.wiki-expandable + .wiki-expandable {{
    margin-top:14px;
}}
.wiki-card-top,
.wiki-heading-group,
.wiki-tag-row,
.timeline-top {{
    display:flex;
    align-items:center;
    gap:10px;
    flex-wrap:wrap;
}}
.wiki-card-top {{
    justify-content:space-between;
    align-items:flex-start;
}}
.wiki-heading-group h3 {{
    margin:0;
    font-size:20px;
    font-weight:800;
    letter-spacing:-0.02em;
}}
.wiki-id {{
    font-family:var(--mono);
    font-size:11px;
    padding:4px 8px;
    border-radius:999px;
    background:rgba(6,182,212,.08);
    color:var(--navy);
    font-weight:700;
}}
.detail-list {{
    list-style:none;
    padding:0;
    margin:12px 0 0;
    display:grid;
    gap:10px;
}}
.detail-row {{
    display:grid;
    grid-template-columns:auto 1fr;
    gap:10px;
    align-items:flex-start;
}}
.detail-title {{
    font-weight:700;
    color:var(--ink);
}}
.wiki-expandable {{
    background:rgba(255,255,255,.018);
    border:1px solid var(--line);
    border-radius:16px;
    padding:0 18px;
}}
.wiki-expandable summary {{
    list-style:none;
    cursor:pointer;
    padding:16px 0;
}}
.wiki-expandable summary::-webkit-details-marker {{
    display:none;
}}
.wiki-expandable[open] summary {{
    border-bottom:1px solid var(--line);
}}
.wiki-expandable > div {{
    padding:16px 0 18px;
}}
.detail-block h4 {{
    margin:0 0 12px;
    font-size:15px;
    font-weight:800;
}}
.timeline-stack {{
    display:grid;
    gap:12px;
}}
.timeline-item {{
    padding:16px;
    border:1px solid var(--line);
    border-radius:16px;
    background:rgba(255,255,255,.022);
}}
.timeline-date {{
    color:var(--muted);
    font-size:12px;
}}
.progress-track {{
    margin-top:12px;
    height:8px;
    border-radius:999px;
    overflow:hidden;
    background:rgba(255,255,255,.05);
}}
.progress-fill {{
    height:100%;
    border-radius:999px;
}}
.tone-fill-planned {{ background:#93a1ad; }}
.tone-fill-discovery {{ background:#b1831f; }}
.tone-fill-build {{ background:#27587e; }}
.tone-fill-test {{ background:#1d6d6b; }}
.tone-fill-done {{ background:#2f7d59; }}
.tone-fill-blocked {{ background:#a04d5f; }}
.tone {{
    display:inline-flex;
    align-items:center;
    justify-content:center;
    padding:5px 10px;
    border-radius:999px;
    font-size:11px;
    font-weight:800;
    text-transform:uppercase;
    letter-spacing:.08em;
    border:1px solid transparent;
    white-space:nowrap;
}}
.tone-planned {{ background:rgba(86,102,121,.1); color:var(--slate); border-color:rgba(86,102,121,.14); }}
.tone-discovery {{ background:rgba(154,107,23,.12); color:var(--gold); border-color:rgba(154,107,23,.18); }}
.tone-build {{ background:rgba(39,88,126,.1); color:var(--navy); border-color:rgba(39,88,126,.16); }}
.tone-test {{ background:rgba(29,109,107,.11); color:var(--teal); border-color:rgba(29,109,107,.18); }}
.tone-done {{ background:rgba(47,125,89,.1); color:var(--green); border-color:rgba(47,125,89,.16); }}
.tone-blocked {{ background:rgba(160,77,95,.11); color:var(--coral); border-color:rgba(160,77,95,.18); }}
.tone-overview {{ background:rgba(39,88,126,.1); color:var(--navy); border-color:rgba(39,88,126,.16); }}
.tone-guide {{ background:rgba(29,109,107,.11); color:var(--teal); border-color:rgba(29,109,107,.18); }}
.tone-experience {{ background:rgba(86,102,121,.1); color:var(--slate); border-color:rgba(86,102,121,.15); }}
.tone-architecture {{ background:rgba(101,84,162,.1); color:var(--violet); border-color:rgba(101,84,162,.15); }}
.tone-sync {{ background:rgba(47,125,89,.1); color:var(--green); border-color:rgba(47,125,89,.16); }}
.tone-history {{ background:rgba(160,77,95,.11); color:var(--coral); border-color:rgba(160,77,95,.18); }}
.tone-research {{ background:rgba(101,84,162,.1); color:var(--violet); border-color:rgba(101,84,162,.15); }}
.tone-discovery.tone {{ }}
.tone-design {{ background:rgba(6,182,212,.1); color:var(--navy); border-color:rgba(39,88,126,.15); }}
.tone-library {{ background:rgba(86,102,121,.08); color:var(--slate); border-color:rgba(86,102,121,.12); }}
.mini-chip-row {{
    display:flex;
    gap:8px;
    flex-wrap:wrap;
    margin-top:12px;
}}
.handbook-card {{
    display:grid;
    gap:10px;
    color:inherit;
    text-decoration:none;
}}
.handbook-card:hover {{
    transform:translateY(-2px);
    box-shadow:0 18px 38px rgba(2,6,23,.28);
    text-decoration:none;
}}
.handbook-card h3 {{
    margin:0;
    font-size:20px;
    font-weight:800;
    color:var(--ink);
}}
.card-path {{
    font-size:12px;
    word-break:break-all;
}}
.wiki-rich-text {{
    margin-top:16px;
    font-size:16px;
}}
.wiki-rich-text h1,
.wiki-rich-text h2,
.wiki-rich-text h3 {{
    font-family:var(--serif);
    color:var(--ink);
    letter-spacing:-0.03em;
}}
.wiki-rich-text h1 {{
    font-size:32px;
    margin:26px 0 12px;
}}
.wiki-rich-text h2 {{
    font-size:24px;
    margin:24px 0 10px;
}}
.wiki-rich-text h3 {{
    font-size:19px;
    margin:18px 0 8px;
}}
.wiki-rich-text p,
.wiki-rich-text li {{
    color:var(--ink);
}}
.wiki-rich-text ul,
.wiki-rich-text ol {{
    padding-left:24px;
}}
.wiki-rich-text pre {{
    padding:16px;
    overflow:auto;
    border-radius:14px;
    background:#07111d;
    color:#f4f7fb;
    border:1px solid rgba(255,255,255,.06);
}}
.wiki-rich-text code {{
    padding:2px 6px;
    border-radius:8px;
    background:rgba(6,182,212,.08);
    color:var(--navy);
}}
.wiki-rich-text pre code {{
    background:none;
    color:inherit;
    padding:0;
}}
.wiki-rich-text hr {{
    border:none;
    border-top:1px solid var(--line);
    margin:24px 0;
}}
.wiki-rich-text blockquote {{
    margin:16px 0;
    padding:12px 16px;
    border-left:4px solid rgba(6,182,212,.3);
    background:rgba(6,182,212,.05);
    color:var(--muted);
}}
.mermaid {{
    margin:18px 0;
    padding:16px;
    overflow:auto;
    border-radius:16px;
    border:1px solid var(--line);
    background:rgba(255,255,255,.02);
    white-space:pre-wrap;
}}
.text-link {{
    font-size:12px;
    font-weight:700;
}}
.wiki-footer {{
    text-align:center;
    color:var(--muted);
    font-size:13px;
    padding:20px 0 8px;
}}
@media (max-width: 1180px) {{
    .stats-grid {{ grid-template-columns:repeat(3, minmax(0, 1fr)); }}
    .handbook-grid {{ grid-template-columns:repeat(2, minmax(0, 1fr)); }}
}}
@media (max-width: 980px) {{
    .wiki-shell {{ grid-template-columns:1fr; }}
    .wiki-sidebar {{ position:static; }}
    .phase-band,
    .principle-grid,
    .wiki-detail-grid {{ grid-template-columns:1fr; }}
}}
@media (max-width: 720px) {{
    .wiki-shell {{ padding:18px 14px 32px; gap:18px; }}
    .hero {{ padding:22px; }}
    .hero h1 {{ font-size:34px; }}
    .stats-grid,
    .handbook-grid {{ grid-template-columns:1fr; }}
    .section-card {{ padding:18px; }}
}}
</style>
</head>
<body>
<div class="wiki-shell" id="top">
    <aside class="wiki-sidebar">
        <div class="sidebar-card">
            <div class="brand">
                <div class="brand-mark" aria-hidden="true">
                    <svg viewBox="0 0 200 200" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <defs>
                            <linearGradient id="wiki-dx-grad" x1="0%" y1="0%" x2="100%" y2="100%">
                                <stop offset="0%" stop-color="#06B6D4"/>
                                <stop offset="50%" stop-color="#A855F7"/>
                                <stop offset="100%" stop-color="#FF00AA"/>
                            </linearGradient>
                        </defs>
                        <polygon points="100,20 170,60 170,140 100,180 30,140 30,60" stroke="url(#wiki-dx-grad)" stroke-width="8" fill="none"/>
                        <circle cx="100" cy="100" r="25" fill="url(#wiki-dx-grad)"/>
                        <circle cx="100" cy="40" r="8" fill="url(#wiki-dx-grad)"/>
                        <circle cx="155" cy="70" r="8" fill="url(#wiki-dx-grad)"/>
                        <circle cx="155" cy="130" r="8" fill="url(#wiki-dx-grad)"/>
                        <circle cx="100" cy="160" r="8" fill="url(#wiki-dx-grad)"/>
                        <circle cx="45" cy="130" r="8" fill="url(#wiki-dx-grad)"/>
                        <circle cx="45" cy="70" r="8" fill="url(#wiki-dx-grad)"/>
                    </svg>
                </div>
                <div class="brand-copy">
                    <h1>DX Program Handbook</h1>
                    <p class="sidebar-copy">One readable record for clients, operators, builders, QA, and stakeholders.</p>
                </div>
            </div>
            <p class="sidebar-copy">This page is generated from the live project mission, delivery state, markdown docs, architecture notes, and change history so the handbook stays close to the running system.</p>
            <div class="sidebar-actions">
                <a class="primary" href="/?project={project_url}">Open Portal</a>
                <a href="#handbook">Read Handbook</a>
            </div>
        </div>
        <div class="sidebar-card">
            <div class="sidebar-title">Jump To</div>
            <nav class="sidebar-nav">
                <a href="#overview">Overview</a>
                <a href="#stages">Stages</a>
                <a href="#handbook">Handbook</a>
                <a href="#goals">Goals</a>
                <a href="#features">Features</a>
                <a href="#architecture">Architecture</a>
                <a href="#research">Research</a>
                <a href="#history">History</a>
                <a href="#library">Library</a>
                {sidebar_doc_links}
            </nav>
        </div>
        <div class="sidebar-card">
            <div class="sidebar-title">Current Focus</div>
            {focus_html}
        </div>
        <div class="sidebar-card">
            <div class="sidebar-title">Hosted Sync Rule</div>
            <p class="sidebar-copy">Local and hosted dashboards should consume the same project brief and event stream, not separate copies of project state.</p>
            <div class="mini-chip-row">
                <span class="tone tone-sync">/api/project/brief</span>
                <span class="tone tone-overview">vision_changed</span>
                <span class="tone tone-overview">focus_changed</span>
            </div>
        </div>
    </aside>

    <main>
        <section class="section-card hero" id="overview">
            <div class="section-kicker">Project Handbook</div>
            <div class="hero-top">
                <div>
                    <h1>{project}</h1>
                    <div class="hero-mission">{mission}</div>
                    <div class="hero-meta">
                        <span class="meta-chip">Last updated {updated}</span>
                        <span class="meta-chip">{path}</span>
                        <span class="meta-chip">{total_docs} handbook documents</span>
                    </div>
                </div>
                <div class="hero-actions">
                    <a class="primary" href="/?project={project_url}">Open cockpit</a>
                    <a href="#library">Browse full library</a>
                </div>
            </div>
            <div class="stats-grid">
                <div class="stat-card"><div class="stat-label">Goals</div><div class="stat-value">{goal_count}</div><div class="detail-meta">program outcomes tracked</div></div>
                <div class="stat-card"><div class="stat-label">Features</div><div class="stat-value">{feature_count}</div><div class="detail-meta">delivery items in the plan</div></div>
                <div class="stat-card"><div class="stat-label">Handbook Docs</div><div class="stat-value">{total_docs}</div><div class="detail-meta">featured, library, research, discovery, design</div></div>
                <div class="stat-card"><div class="stat-label">Design Briefs</div><div class="stat-value">{design_doc_count}</div><div class="detail-meta">client-facing concept artifacts</div></div>
                <div class="stat-card"><div class="stat-label">Milestones</div><div class="stat-value">{milestone_count}</div><div class="detail-meta">program checkpoints</div></div>
                <div class="stat-card"><div class="stat-label">Architecture</div><div class="stat-value">{adr_count}</div><div class="detail-meta">decisions with rationale</div></div>
                <div class="stat-card"><div class="stat-label">History</div><div class="stat-value">{change_count}</div><div class="detail-meta">recorded project changes</div></div>
            </div>
        </section>

        <section class="section-card" id="stages">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Delivery Model</div>
                    <h2>Stages and operating principles</h2>
                </div>
                <p>{stage_story}</p>
            </div>
            <div class="phase-band">{phase_cards}</div>
            {principles_html}
        </section>

        {milestones_section}

        <section class="section-card" id="handbook">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Core Reading</div>
                    <h2>Handbook paths for different audiences</h2>
                </div>
                <p>These documents explain the product from the client, operator, delivery, architecture, hosted sync, and historical perspectives.</p>
            </div>
            <div class="handbook-grid">{featured_cards}</div>
            <div style="margin-top:18px">{featured_articles}</div>
        </section>

        <section class="section-card" id="goals">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Program Intent</div>
                    <h2>Goals and outcomes</h2>
                </div>
                <p>Goals express the business or program outcomes the system is trying to achieve, not just implementation tasks.</p>
            </div>
            {goals_html}
        </section>

        <section class="section-card" id="features">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Delivery Plan</div>
                    <h2>Feature delivery map</h2>
                </div>
                <p>Each feature shows its current stage, readiness, blockers, acceptance coverage, and implementation details.</p>
            </div>
            {features_html}
        </section>

        <section class="section-card" id="architecture">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Architecture</div>
                    <h2>Decisions and tradeoffs</h2>
                </div>
                <p>Architecture records explain why the system looks the way it does and preserve the reasoning behind important choices.</p>
            </div>
            {adr_html}
        </section>

        <section class="section-card" id="research">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Evidence</div>
                    <h2>Research, discovery, and design evidence</h2>
                </div>
                <p>These notes keep delivery grounded in explicit research, design direction, and discovery rather than verbal memory.</p>
            </div>
            {research_html}
        </section>

        <section class="section-card" id="history">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Program Record</div>
                    <h2>Recent change history</h2>
                </div>
                <p>The handbook keeps a readable account of what changed, why it changed, and who or what triggered it.</p>
            </div>
            <div class="timeline-stack">{changes_html}</div>
        </section>

        <section class="section-card" id="library">
            <div class="section-head">
                <div>
                    <div class="section-kicker">Reference Library</div>
                    <h2>Full document library</h2>
                </div>
                <p>Everything under <code>docs/</code> is visible here so the wiki can act as the readable memory of the project.</p>
            </div>
            {library_html}
        </section>

        <div class="wiki-footer">
            Generated from <code>.vision/vision.json</code>, project markdown, and live DX Terminal state.
        </div>
    </main>
</div>
<script type="module">
const navLinks=[...document.querySelectorAll('.sidebar-nav a')];
const sections=[...document.querySelectorAll('main [id]')];
const activate=()=>{{
  const marker=window.scrollY+160;
  let current=sections[0]?.id||'overview';
  sections.forEach(section=>{{ if(section.offsetTop<=marker) current=section.id; }});
  navLinks.forEach(link=>link.classList.toggle('is-active', link.getAttribute('href')===`#${{current}}`));
}};
window.addEventListener('scroll', activate, {{ passive:true }});
activate();

try {{
  const mermaidModule = await import('https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs');
  mermaidModule.default.initialize({{
    startOnLoad:false,
    theme:'base',
    themeVariables:{{
      primaryColor:'#0e1b2d',
      primaryTextColor:'#eef4ff',
      primaryBorderColor:'#32517c',
      lineColor:'#06b6d4',
      secondaryColor:'#132235',
      tertiaryColor:'#08111d',
      fontFamily:'Avenir Next, IBM Plex Sans, Segoe UI, sans-serif',
      background:'#0e1b2d',
      clusterBkg:'#132235',
      clusterBorder:'#32517c'
    }}
  }});
  await mermaidModule.default.run({{ querySelector: '.mermaid' }});
}} catch (_) {{
  document.documentElement.dataset.mermaid='fallback';
}}
</script>
</body>
</html>"##,
        project = escape_html(project),
        project_url = escape_html(&project.replace(' ', "%20")),
        mission = escape_html(if mission.is_empty() {
            "No project mission has been written yet."
        } else {
            mission
        }),
        updated = escape_html(if updated.is_empty() {
            "unknown"
        } else {
            updated
        }),
        path = escape_html(&path),
        total_docs = featured_docs.len()
            + library_docs.len()
            + research_docs.len()
            + discovery_docs.len()
            + design_docs.len(),
        design_doc_count = design_docs.len(),
        goal_count = goals.len(),
        feature_count = features.len(),
        milestone_count = milestones.len(),
        adr_count = adrs.len(),
        change_count = changes.len(),
        stage_story = escape_html(&stage_story),
        phase_cards = phase_cards_html,
        principles_html = principles_html,
        milestones_section = if milestones_html.is_empty() {
            String::new()
        } else {
            format!(
                r#"<section class="section-card" id="milestones">
                    <div class="section-head">
                        <div>
                            <div class="section-kicker">Program Cadence</div>
                            <h2>Milestones and checkpoints</h2>
                        </div>
                        <p>Milestones show the larger program beats the team is trying to reach.</p>
                    </div>
                    {}
                </section>"#,
                milestones_html
            )
        },
        featured_cards = featured_cards_html,
        featured_articles = featured_articles_html,
        goals_html = if goals_html.is_empty() {
            "<p class=\"wiki-empty\">No goals are defined yet.</p>".to_string()
        } else {
            goals_html
        },
        features_html = if features_html.is_empty() {
            "<p class=\"wiki-empty\">No features are defined yet.</p>".to_string()
        } else {
            features_html
        },
        adr_html = if adr_html.is_empty() {
            "<p class=\"wiki-empty\">No architecture decisions are documented yet.</p>".to_string()
        } else {
            adr_html
        },
        research_html = research_html,
        changes_html = if changes_html.is_empty() {
            "<p class=\"wiki-empty\">No project history has been recorded yet.</p>".to_string()
        } else {
            changes_html
        },
        library_html = if library_html.is_empty() {
            "<p class=\"wiki-empty\">No additional library documents were found under <code>docs/</code>.</p>".to_string()
        } else {
            library_html
        },
        sidebar_doc_links = sidebar_doc_links,
        focus_html = focus_html
    );

    Html(html)
}

// ── UI/UX Audit ──

#[derive(Deserialize, Default)]
pub struct FileQuery {
    pub file: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct UrlQuery {
    pub url: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ContrastQuery {
    pub fg: Option<String>,
    pub bg: Option<String>,
}

pub async fn get_audit_ui(Query(q): Query<FileQuery>) -> Json<Value> {
    match q.file {
        Some(path) => Json(crate::ui_audit::audit_ui_file(&path)),
        None => {
            let html = include_str!("../../assets/dashboard.html");
            Json(crate::ui_audit::audit_ui_html(html, "dashboard.html"))
        }
    }
}

async fn playwright_bridge_check(app: &AppState, url: &str) -> Option<Value> {
    let result = tools::gateway_tools::gateway_call(
        app,
        types::GatewayCallRequest {
            mcp: "playwright".to_string(),
            tool: "browser_navigate".to_string(),
            arguments: Some(json!({ "url": url })),
        },
    )
    .await;
    let parsed = parse_mcp(&result);
    let success = parsed.get("status").and_then(|value| value.as_str()) == Some("success");
    let detail = if success {
        "External Playwright MCP navigation succeeded through dx gateway".to_string()
    } else {
        parsed
            .get("error")
            .and_then(|value| value.as_str())
            .map(|value| format!("External Playwright MCP unavailable: {}", value))
            .unwrap_or_else(|| "External Playwright MCP unavailable".to_string())
    };

    Some(json!({
        "name": "playwright_available",
        "category": "setup",
        "passed": success,
        "details": detail,
        "severity": if success { "info" } else { "warning" },
    }))
}

async fn enrich_ux_report_with_bridge(app: &AppState, url: &str, report: Value) -> Value {
    let Some(bridge_check) = playwright_bridge_check(app, url).await else {
        return report;
    };

    let mut checks = report
        .get("checks")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    if let Some(index) = checks.iter().position(|value| {
        value.get("name").and_then(|name| name.as_str()) == Some("playwright_available")
    }) {
        checks[index] = bridge_check;
    } else {
        checks.push(bridge_check);
    }

    crate::ux_audit::rebuild_report(url, checks)
}

pub async fn get_audit_ux(State(app): State<AppState>, Query(q): Query<UrlQuery>) -> Json<Value> {
    let url = q.url.unwrap_or_else(|| "http://localhost:3100".into());
    let ux = crate::ux_audit::audit_ux(&url);
    Json(enrich_ux_report_with_bridge(&app, &url, ux).await)
}

pub async fn get_audit_frontend(
    State(app): State<AppState>,
    Query(q): Query<UrlQuery>,
) -> Json<Value> {
    let html = include_str!("../../assets/dashboard.html");
    let ui = crate::ui_audit::audit_ui_html(html, "dashboard.html");
    let url = q.url.unwrap_or_else(|| "http://localhost:3100".into());
    let ux = enrich_ux_report_with_bridge(
        &app,
        &url,
        crate::ux_audit::audit_ux_with_html(&url, Some(html)),
    )
    .await;
    let tokens = crate::design_tokens::design_tokens();
    let contrasts = crate::design_tokens::check_all_contrasts();

    let ui_score = ui["score"].as_f64().unwrap_or(0.0);
    let ux_score = ux["score"].as_f64().unwrap_or(0.0);
    let combined = (ui_score + ux_score) / 2.0;
    let grade = match combined as u32 {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    Json(json!({
        "ui_audit": ui,
        "ux_audit": ux,
        "design_tokens": tokens,
        "contrast": contrasts,
        "combined_score": (combined * 10.0).round() / 10.0,
        "grade": grade,
    }))
}

pub async fn get_design_tokens() -> Json<Value> {
    Json(crate::design_tokens::design_tokens())
}

pub async fn get_contrast(Query(q): Query<ContrastQuery>) -> Json<Value> {
    match (q.fg, q.bg) {
        (Some(fg), Some(bg)) => Json(crate::design_tokens::check_contrast(&fg, &bg)),
        _ => Json(crate::design_tokens::check_all_contrasts()),
    }
}

/// Sync status endpoint — returns git sync state for dashboard
pub async fn get_sync_status(State(app): State<Arc<crate::app::App>>) -> Json<Value> {
    let sync_mgr = app.sync_manager.read().unwrap();
    match sync_mgr.as_ref() {
        Some(mgr) => {
            let root = &mgr.config.root;
            let git_status = crate::sync::git::status(root).ok();
            Json(json!({
                "active": true,
                "project": mgr.config.project,
                "root": root.display().to_string(),
                "auto_commit": mgr.config.auto_commit,
                "auto_push": mgr.config.auto_push,
                "git": git_status.map(|s| json!({
                    "branch": s.branch,
                    "dirty_files": s.dirty_count,
                    "ahead": s.ahead,
                    "behind": s.behind,
                    "has_remote": s.has_remote,
                })),
                "subscribers": mgr.event_tx.receiver_count(),
            }))
        }
        None => Json(json!({
            "active": false,
            "reason": "no git repository detected"
        })),
    }
}

fn collect_project_runtimes(
    state: &crate::state::types::DxTerminalState,
    live_panes: &[crate::tmux::LivePane],
    project_path: &str,
    project: &str,
) -> Vec<Value> {
    let mut runtimes = Vec::new();
    let mut live_by_target: HashMap<&str, &crate::tmux::LivePane> = HashMap::new();
    let mut seen_targets = HashSet::new();

    for live in live_panes {
        live_by_target.insert(live.target.as_str(), live);
    }

    for (pane_id, pane) in &state.panes {
        let matches = pane.project.eq_ignore_ascii_case(project)
            || matches_project_path(&pane.project_path, project_path)
            || pane
                .workspace_path
                .as_deref()
                .map(|value| matches_project_path(value, project_path))
                .unwrap_or(false);
        if !matches {
            continue;
        }

        let live = pane
            .tmux_target
            .as_deref()
            .and_then(|target| live_by_target.get(target).copied());
        let provider = live
            .map(|entry| {
                provider_json(
                    &entry.command,
                    &entry.window_name,
                    entry.jsonl_path.as_deref(),
                )
            })
            .unwrap_or_else(|| provider_json("", "", None));
        if let Some(target) = pane.tmux_target.as_deref() {
            seen_targets.insert(target.to_string());
        }

        runtimes.push(json!({
            "pane": pane_id.parse::<u8>().ok(),
            "status": pane.status,
            "role": pane.role,
            "task": pane.task,
            "project": pane.project,
            "project_path": pane.project_path,
            "workspace_path": pane.workspace_path,
            "branch_name": pane.branch_name,
            "base_branch": pane.base_branch,
            "tmux_target": pane.tmux_target,
            "browser_port": pane_id
                .parse::<u8>()
                .ok()
                .map(crate::config::pane_browser_port),
            "browser_profile_root": pane_id
                .parse::<u8>()
                .ok()
                .map(crate::config::pane_browser_profile_root)
                .map(|value| value.to_string_lossy().to_string()),
            "browser_artifacts_root": pane_id
                .parse::<u8>()
                .ok()
                .map(crate::config::pane_browser_artifacts_root)
                .map(|value| value.to_string_lossy().to_string()),
            "provider": provider,
            "command": live.map(|entry| entry.command.clone()),
            "window_name": live.map(|entry| entry.window_name.clone()),
            "cwd": live.map(|entry| entry.cwd.clone()).or_else(|| {
                pane.workspace_path.clone().or_else(|| {
                    if pane.project_path.is_empty() {
                        None
                    } else {
                        Some(pane.project_path.clone())
                    }
                })
            }),
            "session_id": live.and_then(|entry| entry.session_id.clone()),
            "jsonl_path": live.and_then(|entry| entry.jsonl_path.clone()),
            "live": live.is_some(),
        }));
    }

    for live in live_panes {
        if seen_targets.contains(&live.target) {
            continue;
        }
        if !(matches_project_path(&live.cwd, project_path)
            || project_name_from_path(&live.cwd).eq_ignore_ascii_case(project))
        {
            continue;
        }

        runtimes.push(json!({
            "pane": Value::Null,
            "status": "live",
            "role": "--",
            "task": format!("{} in {}", crate::tmux::provider_label(crate::tmux::infer_provider(&live.command, &live.window_name, live.jsonl_path.as_deref())), live.target),
            "project": project,
            "project_path": project_path,
            "workspace_path": Value::Null,
            "branch_name": Value::Null,
            "base_branch": Value::Null,
            "tmux_target": live.target,
            "provider": provider_json(&live.command, &live.window_name, live.jsonl_path.as_deref()),
            "command": live.command,
            "window_name": live.window_name,
            "cwd": live.cwd,
            "session_id": live.session_id,
            "jsonl_path": live.jsonl_path,
            "live": true,
        }));
    }

    runtimes.sort_by(|a, b| {
        let a_pane = a
            .get("pane")
            .and_then(|value| value.as_u64())
            .unwrap_or(999);
        let b_pane = b
            .get("pane")
            .and_then(|value| value.as_u64())
            .unwrap_or(999);
        a_pane.cmp(&b_pane)
    });
    runtimes
}

/// GET /api/pane/:id/context — VDD context for a pane's project
/// Returns the vision, active features, tasks, and docs for the project the pane is working on
pub async fn get_pane_context(
    State(app): State<Arc<crate::app::App>>,
    Path(pane_ref): Path<String>,
) -> Json<Value> {
    // Get state data (may be stale)
    let config_result = tools::config_show(
        &app,
        types::ConfigShowRequest {
            pane: Some(pane_ref.clone()),
        },
    )
    .await;
    let pane_data = parse_mcp(&config_result);

    let state_project = pane_data["project"].as_str().unwrap_or("--").to_string();
    let task = pane_data["task"].as_str().unwrap_or("").to_string();
    let state_cwd = pane_data["cwd"].as_str().unwrap_or("").to_string();
    let state_target = pane_data["tmux_target"].as_str().unwrap_or("").to_string();

    // Discover live tmux panes for accurate cwd
    let pane_num: usize = pane_ref.parse().unwrap_or(0);
    let live_panes = tokio::task::spawn_blocking(|| crate::tmux::discover_live_panes())
        .await
        .unwrap_or_default();

    let live_pane = if !state_target.is_empty() {
        live_panes
            .iter()
            .find(|pane| pane.target == state_target)
            .cloned()
    } else if pane_num > 0 && pane_num <= live_panes.len() {
        Some(live_panes[pane_num - 1].clone())
    } else {
        None
    };

    // Get live cwd from the matching pane (prefer target match over positional fallback)
    let live_cwd = live_pane
        .as_ref()
        .map(|pane| pane.cwd.clone())
        .unwrap_or_default();

    // Use live cwd if available, otherwise state cwd
    let cwd = if !live_cwd.is_empty() {
        &live_cwd
    } else {
        &state_cwd
    };

    // Derive project name from cwd (same logic as ws.rs project_from_cwd)
    let project = if !cwd.is_empty() {
        let home = std::env::var("HOME").unwrap_or_default();
        let projects_dir = format!("{}/Projects", home);
        let path = std::path::Path::new(cwd.as_str());
        if cwd.as_str() == projects_dir || cwd.as_str() == home {
            // At root — use state project if available
            if state_project != "--" {
                state_project.clone()
            } else {
                "--".to_string()
            }
        } else if let Ok(rel) = path.strip_prefix(&projects_dir) {
            rel.components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .unwrap_or_else(|| state_project.clone())
        } else {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| state_project.clone())
        }
    } else {
        state_project.clone()
    };

    // Try to find vision.json for this project
    let vision = find_vision_for_project(&project, cwd);

    let project_path = if let Some(ref live) = live_pane {
        find_project_root(FsPath::new(&live.cwd))
            .map(|root| root.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                resolve_project_path(&VisionQuery {
                    project: Some(project.clone()),
                    path: None,
                })
            })
    } else {
        resolve_project_path(&VisionQuery {
            project: Some(project.clone()),
            path: None,
        })
    };
    let guidance_docs = collect_guidance_docs(cwd, &project_path);
    let claude_md = guidance_docs
        .iter()
        .find(|doc| doc.get("name").and_then(|value| value.as_str()) == Some("CLAUDE.md"))
        .and_then(|doc| doc.get("preview").and_then(|value| value.as_str()))
        .map(|value| value.to_string());
    let provider = live_pane
        .as_ref()
        .map(|pane| provider_json(&pane.command, &pane.window_name, pane.jsonl_path.as_deref()))
        .unwrap_or_else(|| provider_json("", "", None));
    let pane_num = pane_ref.parse::<u8>().ok();
    let browser_port = pane_num.map(crate::config::pane_browser_port);
    let browser_profile_root = pane_num
        .map(crate::config::pane_browser_profile_root)
        .map(|value| value.to_string_lossy().to_string());
    let browser_artifacts_root = pane_num
        .map(crate::config::pane_browser_artifacts_root)
        .map(|value| value.to_string_lossy().to_string());

    // Build context response
    let mut ctx = json!({
        "pane": pane_ref,
        "project": project,
        "task": task,
        "cwd": cwd,
        "project_path": project_path,
        "has_claude_md": claude_md.is_some(),
        "guidance_docs": guidance_docs,
        "runtime": {
            "provider": provider,
            "command": live_pane.as_ref().map(|pane| pane.command.clone()),
            "window_name": live_pane.as_ref().map(|pane| pane.window_name.clone()),
            "tmux_target": live_pane
                .as_ref()
                .map(|pane| pane.target.clone())
                .or_else(|| if state_target.is_empty() { None } else { Some(state_target.clone()) }),
            "session_id": live_pane.as_ref().and_then(|pane| pane.session_id.clone()),
            "workspace_path": pane_data.get("workspace_path").cloned().unwrap_or(Value::Null),
            "branch_name": pane_data.get("branch_name").cloned().unwrap_or(Value::Null),
            "base_branch": pane_data.get("base_branch").cloned().unwrap_or(Value::Null),
            "browser_port": browser_port,
            "browser_profile_root": browser_profile_root,
            "browser_artifacts_root": browser_artifacts_root,
        },
    });

    if let Some(ref md) = claude_md {
        // Return first 2000 chars of CLAUDE.md as summary
        let preview: String = md.chars().take(2000).collect();
        ctx["claude_md_preview"] = json!(preview);
    }

    if let Some(v) = vision {
        let mission = v["mission"].as_str().unwrap_or("");
        let goals = v["goals"].as_array().cloned().unwrap_or_default();
        let mut features = Vec::new();
        for goal in &goals {
            if let Some(goal_features) = goal.get("features").and_then(|value| value.as_array()) {
                features.extend(goal_features.iter().cloned());
            }
        }

        ctx["vision"] = json!({
            "project": v["project"],
            "mission": mission,
            "goals": goals,
            "features": features,
            "summary": v["summary"].clone(),
            "wiki_url": format!("/wiki?project={}", project),
        });
    }

    Json(ctx)
}

/// Find vision.json for a project by name or cwd
fn find_vision_for_project(project: &str, cwd: &str) -> Option<Value> {
    let path = if !cwd.is_empty()
        && std::path::Path::new(cwd)
            .join(".vision/vision.json")
            .exists()
    {
        cwd.to_string()
    } else {
        resolve_project_path(&VisionQuery {
            project: if project == "--" || project.is_empty() {
                None
            } else {
                Some(project.to_string())
            },
            path: None,
        })
    };

    let tree = crate::vision::vision_tree(&path);
    serde_json::from_str::<Value>(&tree)
        .ok()
        .filter(|value| value.get("error").is_none())
}
