use axum::{
    extract::{Path, Query, State},
    response::{Html, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::app::App;
use crate::capacity;
use crate::config;
use crate::mcp::{tools, types};
use crate::queue;

type AppState = Arc<App>;

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
pub async fn assess_vision_work(Json(body): Json<Value>) -> Json<Value> {
    let project = body["project"].as_str().unwrap_or("").to_string();
    let path = resolve_project_path(&VisionQuery {
        project: Some(project.clone()),
        path: None,
    });
    let description = body["description"].as_str().unwrap_or("");
    let result = crate::vision::assess_work(&path, description);
    Json(serde_json::from_str(&result).unwrap_or(json!({"raw": result})))
}

// ── VDD Research & Discovery Docs ──

#[derive(Deserialize, Default)]
pub struct VisionDocQuery {
    pub project: Option<String>,
    pub feature_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct VisionFocusRequest {
    pub project: Option<String>,
    pub path: Option<String>,
    pub goal_id: Option<String>,
    pub feature_id: Option<String>,
    pub source: Option<String>,
}

/// GET /api/vision/docs?project=NAME — List all research/discovery docs
pub async fn list_vision_docs(Query(q): Query<VisionQuery>) -> Json<Value> {
    let path = resolve_project_path(&q);
    let base = std::path::Path::new(&path).join(".vision");

    let mut docs = vec![];
    for subdir in &["research", "discovery"] {
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
                        "preview": preview,
                        "size": content.len(),
                    }));
                }
            }
        }
    }

    Json(json!({ "project": q.project, "docs": docs }))
}

/// GET /api/vision/doc?project=NAME&feature_id=F-XXX — Get a specific research or discovery doc
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
        result["readiness"] = readiness.get("readiness").cloned().unwrap_or(json!({}));
    }

    Json(result)
}

/// POST /api/vision/focus — Persist the operator's active goal/feature focus for auto-continue.
pub async fn set_vision_focus(Json(body): Json<VisionFocusRequest>) -> Json<Value> {
    let path = resolve_project_path(&VisionQuery {
        project: body.project.clone(),
        path: body.path.clone(),
    });
    let source = body.source.as_deref().unwrap_or("dashboard");

    let focus = if let Some(feature_id) = body.feature_id.as_deref().filter(|value| !value.trim().is_empty()) {
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
        Some(focus) => Json(json!({"status": "focused", "focus": focus})),
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
    let mut in_list = false;
    let mut in_code = false;

    for line in md.lines() {
        // Code blocks
        if line.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>");
                in_code = false;
            } else {
                if in_list {
                    html.push_str("</ul>");
                    in_list = false;
                }
                html.push_str("<pre><code>");
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
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            continue;
        }

        // Headers
        if trimmed.starts_with("### ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str(&format!("<h3>{}</h3>", escape_html(&trimmed[4..])));
        } else if trimmed.starts_with("## ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str(&format!("<h2>{}</h2>", escape_html(&trimmed[3..])));
        } else if trimmed.starts_with("# ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str(&format!("<h1>{}</h1>", escape_html(&trimmed[2..])));
        }
        // Horizontal rules
        else if trimmed == "---" || trimmed == "***" {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<hr>");
        }
        // List items
        else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                html.push_str("<ul>");
                in_list = true;
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
                if !in_list {
                    html.push_str("<ol>");
                    in_list = true;
                }
                html.push_str(&format!("<li>{}</li>", inline_md(&trimmed[pos + 2..])));
            }
        }
        // Regular paragraph
        else {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str(&format!("<p>{}</p>", inline_md(trimmed)));
        }
    }
    if in_list {
        html.push_str("</ul>");
    }
    if in_code {
        html.push_str("</code></pre>");
    }
    html
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

    // Build goals HTML
    let mut goals_html = String::new();
    if let Some(goals) = vision["goals"].as_array() {
        for g in goals {
            let id = g["id"].as_str().unwrap_or("");
            let title = g["title"].as_str().unwrap_or("");
            let desc = g["description"].as_str().unwrap_or("");
            let status = g["status"].as_str().unwrap_or("planned");
            let priority = g["priority"].as_u64().unwrap_or(3);
            let (badge_color, badge_bg) = match status {
                "achieved" => ("#10b981", "rgba(16,185,129,0.1)"),
                "in_progress" | "build" => ("#3b82f6", "rgba(59,130,246,0.1)"),
                _ => ("#6b7280", "rgba(107,114,128,0.1)"),
            };
            let metrics_html = if let Some(metrics) = g["metrics"].as_array() {
                let items: Vec<String> = metrics
                    .iter()
                    .filter_map(|m| m.as_str())
                    .map(|m| format!("<span class='wiki-metric'>{}</span>", escape_html(m)))
                    .collect();
                format!("<div class='wiki-metrics'>{}</div>", items.join(""))
            } else {
                String::new()
            };

            goals_html.push_str(&format!(r#"
                <div class="wiki-goal">
                    <div class="wiki-goal-header">
                        <span class="wiki-id">{id}</span>
                        <span class="wiki-goal-title">{title}</span>
                        <span class="wiki-badge" style="color:{badge_color};background:{badge_bg}">{status}</span>
                        <span class="wiki-priority">P{priority}</span>
                    </div>
                    <div class="wiki-desc">{desc}</div>
                    {metrics_html}
                </div>
            "#, id=escape_html(id), title=escape_html(title), desc=escape_html(desc),
               status=status, badge_color=badge_color, badge_bg=badge_bg,
               priority=priority, metrics_html=metrics_html));
        }
    }

    // Build features HTML
    let mut features_html = String::new();
    if let Some(features) = vision["features"].as_array() {
        for f in features {
            let id = f["id"].as_str().unwrap_or("");
            let title = f["title"].as_str().unwrap_or("");
            let desc = f["description"].as_str().unwrap_or("");
            let status = f
                .get("phase")
                .or(f.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("planned");
            let goal_id = f["goal_id"].as_str().unwrap_or("");

            let (badge_color, badge_bg) = match status {
                "done" => ("#10b981", "rgba(16,185,129,0.1)"),
                "test" | "testing" => ("#f59e0b", "rgba(245,158,11,0.1)"),
                "build" | "building" | "in_progress" => ("#3b82f6", "rgba(59,130,246,0.1)"),
                "specifying" => ("#8b5cf6", "rgba(139,92,246,0.1)"),
                _ => ("#6b7280", "rgba(107,114,128,0.1)"),
            };

            // Tasks
            let mut tasks_html = String::new();
            if let Some(tasks) = f["tasks"].as_array() {
                for t in tasks {
                    let t_title = t["title"].as_str().unwrap_or("");
                    let t_status = t["status"].as_str().unwrap_or("planned");
                    let icon = match t_status {
                        "done" => "&#x2705;",
                        "in_progress" => "&#x1F6E0;",
                        _ => "&#x25CB;",
                    };
                    let branch = t["branch"].as_str().unwrap_or("");
                    let branch_tag = if !branch.is_empty() {
                        format!("<code class='wiki-branch'>{}</code>", escape_html(branch))
                    } else {
                        String::new()
                    };
                    tasks_html.push_str(&format!(
                        "<div class='wiki-task'><span>{icon}</span> <span>{title}</span> {branch}</div>",
                        icon=icon, title=escape_html(t_title), branch=branch_tag
                    ));
                }
            }

            // Questions
            let mut questions_html = String::new();
            if let Some(questions) = f["questions"].as_array() {
                for q in questions {
                    let q_text = q["text"].as_str().unwrap_or("");
                    let q_status = q["status"].as_str().unwrap_or("open");
                    let q_answer = q["answer"].as_str().unwrap_or("");
                    let icon = if q_status == "answered" {
                        "&#x2705;"
                    } else {
                        "&#x2753;"
                    };
                    questions_html.push_str(&format!(
                        "<div class='wiki-question'><span>{icon}</span> <span>{text}</span>{answer}</div>",
                        icon=icon, text=escape_html(q_text),
                        answer=if !q_answer.is_empty() {
                            format!("<div class='wiki-answer'>{}</div>", escape_html(q_answer))
                        } else { String::new() }
                    ));
                }
            }

            // Decisions
            let mut decisions_html = String::new();
            if let Some(decisions) = f["decisions"].as_array() {
                for d in decisions {
                    let d_decision = d["decision"].as_str().unwrap_or("");
                    let d_rationale = d["rationale"].as_str().unwrap_or("");
                    decisions_html.push_str(&format!(
                        "<div class='wiki-decision'><strong>Decision:</strong> {decision}<br><em>Rationale:</em> {rationale}</div>",
                        decision=escape_html(d_decision), rationale=escape_html(d_rationale)
                    ));
                }
            }

            // Acceptance criteria
            let mut criteria_html = String::new();
            if let Some(criteria) = f["acceptance_criteria"].as_array() {
                for c in criteria {
                    if let Some(text) = c.as_str() {
                        let check = if status == "done" {
                            "&#x2705;"
                        } else {
                            "&#x25CB;"
                        };
                        criteria_html.push_str(&format!(
                            "<div class='wiki-criterion'><span>{check}</span> {text}</div>",
                            check = check,
                            text = escape_html(text)
                        ));
                    }
                }
            }

            features_html.push_str(&format!(r#"
                <div class="wiki-feature" id="feature-{id}">
                    <div class="wiki-feature-header">
                        <span class="wiki-id">{id}</span>
                        <span class="wiki-feature-title">{title}</span>
                        <span class="wiki-badge" style="color:{badge_color};background:{badge_bg}">{status}</span>
                        <span class="wiki-goal-ref">&#x2192; {goal_id}</span>
                    </div>
                    <div class="wiki-desc">{desc}</div>
                    {tasks_section}
                    {criteria_section}
                    {questions_section}
                    {decisions_section}
                </div>
            "#,
                id=escape_html(id), title=escape_html(title), desc=escape_html(desc),
                status=status, badge_color=badge_color, badge_bg=badge_bg,
                goal_id=escape_html(goal_id),
                tasks_section=if !tasks_html.is_empty() {
                    format!("<div class='wiki-section-title'>Tasks</div>{}", tasks_html)
                } else { String::new() },
                criteria_section=if !criteria_html.is_empty() {
                    format!("<div class='wiki-section-title'>Acceptance Criteria</div>{}", criteria_html)
                } else { String::new() },
                questions_section=if !questions_html.is_empty() {
                    format!("<div class='wiki-section-title'>Questions</div>{}", questions_html)
                } else { String::new() },
                decisions_section=if !decisions_html.is_empty() {
                    format!("<div class='wiki-section-title'>Decisions</div>{}", decisions_html)
                } else { String::new() },
            ));
        }
    }

    // Architecture Decision Records
    let mut adr_html = String::new();
    if let Some(adrs) = vision["architecture"].as_array() {
        for a in adrs {
            let id = a["id"].as_str().unwrap_or("");
            let title = a["title"].as_str().unwrap_or("");
            let decision = a["decision"].as_str().unwrap_or("");
            let rationale = a["rationale"].as_str().unwrap_or("");
            let date = a["date"].as_str().unwrap_or("");
            let status = a["status"].as_str().unwrap_or("active");
            let alts = a["alternatives_considered"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            adr_html.push_str(&format!(r#"
                <div class="wiki-adr">
                    <div class="wiki-adr-header">
                        <span class="wiki-id">{id}</span>
                        <span class="wiki-adr-title">{title}</span>
                        <span class="wiki-badge" style="color:#10b981;background:rgba(16,185,129,0.1)">{status}</span>
                        <span class="wiki-date">{date}</span>
                    </div>
                    <div class="wiki-adr-body">
                        <p><strong>Decision:</strong> {decision}</p>
                        <p><strong>Rationale:</strong> {rationale}</p>
                        {alts_html}
                    </div>
                </div>
            "#,
                id=escape_html(id), title=escape_html(title), decision=escape_html(decision),
                rationale=escape_html(rationale), date=escape_html(date), status=escape_html(status),
                alts_html=if !alts.is_empty() {
                    format!("<p><strong>Alternatives considered:</strong> {}</p>", escape_html(&alts))
                } else { String::new() }
            ));
        }
    }

    // Milestones
    let mut milestones_html = String::new();
    if let Some(milestones) = vision["milestones"].as_array() {
        for m in milestones {
            let id = m["id"].as_str().unwrap_or("");
            let title = m["title"].as_str().unwrap_or("");
            let desc = m["description"].as_str().unwrap_or("");
            let status = m["status"].as_str().unwrap_or("upcoming");
            let target = m["target_date"].as_str().unwrap_or("");
            let pct = m["progress_pct"].as_u64().unwrap_or(0);
            let (badge_color, badge_bg) = match status {
                "complete" => ("#10b981", "rgba(16,185,129,0.1)"),
                "active" | "in_progress" => ("#3b82f6", "rgba(59,130,246,0.1)"),
                _ => ("#6b7280", "rgba(107,114,128,0.1)"),
            };

            milestones_html.push_str(&format!(r#"
                <div class="wiki-milestone">
                    <div class="wiki-milestone-header">
                        <span class="wiki-id">{id}</span>
                        <span>{title}</span>
                        <span class="wiki-badge" style="color:{badge_color};background:{badge_bg}">{status}</span>
                        <span class="wiki-date">{target}</span>
                        <span class="wiki-pct">{pct}%</span>
                    </div>
                    <div class="wiki-desc">{desc}</div>
                    <div class="wiki-progress-bar"><div class="wiki-progress-fill" style="width:{pct}%"></div></div>
                </div>
            "#,
                id=escape_html(id), title=escape_html(title), desc=escape_html(desc),
                status=escape_html(status), target=escape_html(target), pct=pct,
                badge_color=badge_color, badge_bg=badge_bg,
            ));
        }
    }

    // Recent changes
    let mut changes_html = String::new();
    if let Some(changes) = vision["changes"].as_array() {
        let recent: Vec<&Value> = changes.iter().rev().take(20).collect();
        for c in recent {
            let ts = c["timestamp"].as_str().unwrap_or("");
            let change_type = c["change_type"].as_str().unwrap_or("");
            let field = c["field"].as_str().unwrap_or("");
            let reason = c["reason"].as_str().unwrap_or("");
            let triggered_by = c["triggered_by"].as_str().unwrap_or("");
            let icon = match change_type {
                "added" => "&#x2795;",
                "modified" => "&#x270F;",
                "status_change" => "&#x1F504;",
                _ => "&#x2022;",
            };
            changes_html.push_str(&format!(
                "<tr><td>{icon}</td><td><code>{field}</code></td><td>{reason}</td><td>{by}</td><td class='wiki-date'>{ts}</td></tr>",
                icon=icon, field=escape_html(field), reason=escape_html(reason),
                by=escape_html(triggered_by), ts=escape_html(&ts.get(..16).unwrap_or(ts))
            ));
        }
    }

    // Research/Discovery docs
    let mut docs_html = String::new();
    let base = std::path::Path::new(&path).join(".vision");
    for subdir in &["research", "discovery"] {
        let dir = base.join(subdir);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".md") {
                    let feature_id = fname.trim_end_matches(".md");
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let html = markdown_to_html(&content);
                    docs_html.push_str(&format!(r#"
                        <div class="wiki-doc">
                            <div class="wiki-doc-header">
                                <span class="wiki-badge" style="color:#8b5cf6;background:rgba(139,92,246,0.1)">{subdir}</span>
                                <span>{feature_id}</span>
                            </div>
                            <div class="wiki-doc-content">{html}</div>
                        </div>
                    "#, subdir=subdir, feature_id=escape_html(feature_id), html=html));
                }
            }
        }
    }

    // Principles
    let principles_html = vision["principles"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|p| format!("<li>{}</li>", escape_html(p)))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    // Assemble full page
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{project} — Project Wiki</title>
<style>
:root {{
    --bg: #0d1117; --surface: #161b22; --surface2: #1c2333; --border: #30363d;
    --text: #e6edf3; --muted: #8b949e; --dim: #484f58;
    --blue: #58a6ff; --green: #3fb950; --yellow: #d29922; --red: #f85149;
    --purple: #bc8cff; --teal: #39d2c0;
}}
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ background:var(--bg); color:var(--text); font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif; line-height:1.6; }}
.wiki-container {{ max-width:960px; margin:0 auto; padding:32px 24px; }}
.wiki-header {{ border-bottom:1px solid var(--border); padding-bottom:24px; margin-bottom:32px; }}
.wiki-title {{ font-size:28px; font-weight:700; color:var(--text); margin-bottom:4px; }}
.wiki-mission {{ font-size:14px; color:var(--muted); margin-bottom:12px; }}
.wiki-meta {{ font-size:11px; color:var(--dim); display:flex; gap:16px; }}
.wiki-nav {{ display:flex; gap:8px; margin-bottom:32px; flex-wrap:wrap; }}
.wiki-nav a {{ padding:6px 14px; border-radius:6px; background:var(--surface); color:var(--muted); text-decoration:none; font-size:12px; font-weight:500; border:1px solid var(--border); transition:all 0.15s; }}
.wiki-nav a:hover, .wiki-nav a.active {{ background:var(--surface2); color:var(--blue); border-color:var(--blue); }}
.wiki-section {{ margin-bottom:40px; }}
.wiki-section > h2 {{ font-size:20px; font-weight:600; color:var(--text); border-bottom:2px solid var(--border); padding-bottom:8px; margin-bottom:16px; }}
.wiki-section > h2 span {{ font-size:12px; color:var(--dim); font-weight:400; margin-left:8px; }}
.wiki-id {{ font-size:10px; font-weight:700; color:var(--purple); background:rgba(139,92,246,0.1); padding:2px 6px; border-radius:4px; font-family:monospace; }}
.wiki-badge {{ font-size:10px; font-weight:600; padding:2px 8px; border-radius:10px; text-transform:uppercase; letter-spacing:0.3px; }}
.wiki-date {{ font-size:11px; color:var(--dim); }}
.wiki-priority {{ font-size:10px; color:var(--yellow); font-weight:600; }}
.wiki-pct {{ font-size:11px; color:var(--blue); font-weight:600; }}
.wiki-desc {{ font-size:13px; color:var(--muted); margin:6px 0 10px; }}
.wiki-goal, .wiki-feature, .wiki-adr, .wiki-milestone, .wiki-doc {{ background:var(--surface); border:1px solid var(--border); border-radius:8px; padding:16px; margin-bottom:12px; }}
.wiki-goal-header, .wiki-feature-header, .wiki-adr-header, .wiki-milestone-header {{ display:flex; align-items:center; gap:8px; flex-wrap:wrap; }}
.wiki-goal-title, .wiki-feature-title, .wiki-adr-title {{ font-size:15px; font-weight:600; }}
.wiki-goal-ref {{ font-size:10px; color:var(--dim); }}
.wiki-metrics {{ display:flex; gap:6px; flex-wrap:wrap; margin-top:8px; }}
.wiki-metric {{ font-size:10px; padding:3px 8px; border-radius:4px; background:var(--surface2); color:var(--teal); border:1px solid rgba(57,210,192,0.15); }}
.wiki-section-title {{ font-size:11px; font-weight:700; color:var(--dim); text-transform:uppercase; letter-spacing:0.5px; margin:12px 0 6px; }}
.wiki-task {{ display:flex; align-items:center; gap:6px; font-size:12px; padding:3px 0; color:var(--muted); }}
.wiki-branch {{ font-size:10px; padding:1px 5px; background:var(--surface2); border-radius:3px; color:var(--blue); }}
.wiki-question {{ font-size:12px; padding:4px 0; color:var(--muted); }}
.wiki-answer {{ margin-left:22px; padding:4px 8px; border-left:2px solid var(--green); color:var(--green); font-size:12px; }}
.wiki-decision {{ font-size:12px; padding:8px; background:var(--surface2); border-radius:4px; margin:4px 0; color:var(--muted); }}
.wiki-criterion {{ display:flex; align-items:center; gap:6px; font-size:12px; padding:3px 0; color:var(--muted); }}
.wiki-adr-body {{ font-size:13px; color:var(--muted); margin-top:8px; }}
.wiki-adr-body p {{ margin:4px 0; }}
.wiki-progress-bar {{ height:4px; background:var(--surface2); border-radius:2px; margin-top:8px; overflow:hidden; }}
.wiki-progress-fill {{ height:100%; background:var(--green); border-radius:2px; transition:width 0.3s; }}
.wiki-doc-header {{ display:flex; align-items:center; gap:8px; margin-bottom:8px; }}
.wiki-doc-content {{ font-size:13px; color:var(--muted); }}
.wiki-doc-content h1,.wiki-doc-content h2,.wiki-doc-content h3 {{ color:var(--text); margin:12px 0 6px; }}
.wiki-doc-content pre {{ background:var(--surface2); padding:10px; border-radius:6px; overflow-x:auto; font-size:12px; }}
.wiki-doc-content code {{ font-size:12px; padding:1px 4px; background:var(--surface2); border-radius:3px; }}
.wiki-doc-content ul,.wiki-doc-content ol {{ margin-left:20px; margin-bottom:8px; }}
.wiki-doc-content li {{ margin:2px 0; }}
.wiki-changelog {{ width:100%; border-collapse:collapse; font-size:12px; }}
.wiki-changelog td {{ padding:6px 8px; border-bottom:1px solid var(--border); color:var(--muted); }}
.wiki-changelog code {{ font-size:11px; background:var(--surface2); padding:1px 5px; border-radius:3px; color:var(--purple); }}
.wiki-principles {{ list-style:none; }}
.wiki-principles li {{ padding:4px 0; font-size:13px; color:var(--muted); }}
.wiki-principles li::before {{ content:"→ "; color:var(--blue); font-weight:700; }}
.wiki-footer {{ border-top:1px solid var(--border); padding-top:16px; margin-top:40px; text-align:center; font-size:11px; color:var(--dim); }}
@media (max-width:768px) {{ .wiki-container {{ padding:16px 12px; }} .wiki-title {{ font-size:22px; }} }}
</style>
</head>
<body>
<div class="wiki-container">
    <div class="wiki-header">
        <div class="wiki-title">{project}</div>
        <div class="wiki-mission">{mission}</div>
        <div class="wiki-meta">
            <span>Last updated: {updated}</span>
            <span><a href="/" style="color:var(--blue);text-decoration:none">&#x2190; Dashboard</a></span>
        </div>
    </div>

    <nav class="wiki-nav">
        <a href="#principles">Principles</a>
        <a href="#milestones">Milestones</a>
        <a href="#goals">Goals</a>
        <a href="#features">Features</a>
        <a href="#architecture">Architecture</a>
        <a href="#docs">Docs</a>
        <a href="#changelog">Changelog</a>
    </nav>

    <div class="wiki-section" id="principles">
        <h2>Principles</h2>
        <ul class="wiki-principles">{principles}</ul>
    </div>

    <div class="wiki-section" id="milestones">
        <h2>Milestones <span>{milestone_count} milestones</span></h2>
        {milestones}
    </div>

    <div class="wiki-section" id="goals">
        <h2>Goals <span>{goal_count} goals</span></h2>
        {goals}
    </div>

    <div class="wiki-section" id="features">
        <h2>Features <span>{feature_count} features</span></h2>
        {features}
    </div>

    <div class="wiki-section" id="architecture">
        <h2>Architecture Decision Records <span>{adr_count} ADRs</span></h2>
        {adrs}
    </div>

    {docs_section}

    <div class="wiki-section" id="changelog">
        <h2>Changelog <span>{change_count} changes</span></h2>
        <table class="wiki-changelog">{changes}</table>
    </div>

    <div class="wiki-footer">
        Generated from <code>.vision/vision.json</code> &mdash; DX Terminal
    </div>
</div>
<script>
document.querySelectorAll('.wiki-nav a').forEach(a => {{
    a.addEventListener('click', e => {{
        document.querySelectorAll('.wiki-nav a').forEach(x => x.classList.remove('active'));
        a.classList.add('active');
    }});
}});
</script>
</body>
</html>"##,
        project = escape_html(project),
        mission = escape_html(mission),
        updated = escape_html(updated),
        principles = principles_html,
        goals = goals_html,
        features = features_html,
        adrs = adr_html,
        milestones = milestones_html,
        changes = changes_html,
        milestone_count = vision["milestones"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0),
        goal_count = vision["goals"].as_array().map(|a| a.len()).unwrap_or(0),
        feature_count = vision["features"].as_array().map(|a| a.len()).unwrap_or(0),
        adr_count = vision["architecture"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0),
        change_count = vision["changes"].as_array().map(|a| a.len()).unwrap_or(0),
        docs_section = if !docs_html.is_empty() {
            format!(
                r#"<div class="wiki-section" id="docs"><h2>Research &amp; Discovery Docs</h2>{}</div>"#,
                docs_html
            )
        } else {
            String::new()
        },
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

pub async fn get_audit_ux(Query(q): Query<UrlQuery>) -> Json<Value> {
    let url = q.url.unwrap_or_else(|| "http://localhost:3100".into());
    Json(crate::ux_audit::audit_ux(&url))
}

pub async fn get_audit_frontend(Query(q): Query<UrlQuery>) -> Json<Value> {
    let html = include_str!("../../assets/dashboard.html");
    let ui = crate::ui_audit::audit_ui_html(html, "dashboard.html");
    let url = q.url.unwrap_or_else(|| "http://localhost:3100".into());
    let ux = crate::ux_audit::audit_ux(&url);
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

    // Discover live tmux panes for accurate cwd
    let pane_num: usize = pane_ref.parse().unwrap_or(0);
    let live_panes = tokio::task::spawn_blocking(|| crate::tmux::discover_live_panes())
        .await
        .unwrap_or_default();

    // Get live cwd from the matching pane (0-indexed)
    let live_cwd = if pane_num > 0 && pane_num <= live_panes.len() {
        live_panes[pane_num - 1].cwd.clone()
    } else {
        String::new()
    };

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

    // Also look for CLAUDE.md as project documentation
    let claude_md = find_claude_md(cwd, &project);

    // Build context response
    let mut ctx = json!({
        "pane": pane_ref,
        "project": project,
        "task": task,
        "cwd": cwd,
        "has_claude_md": claude_md.is_some(),
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

/// Find CLAUDE.md for a project
fn find_claude_md(cwd: &str, project: &str) -> Option<String> {
    // Try cwd first
    if !cwd.is_empty() {
        let p = std::path::Path::new(cwd).join("CLAUDE.md");
        if p.exists() {
            return std::fs::read_to_string(&p).ok();
        }
    }
    // Try common project locations
    let home = std::env::var("HOME").unwrap_or_default();
    for base in &[format!("{}/Projects", home), format!("{}", home)] {
        let p = std::path::Path::new(base).join(project).join("CLAUDE.md");
        if p.exists() {
            return std::fs::read_to_string(&p).ok();
        }
    }
    None
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
