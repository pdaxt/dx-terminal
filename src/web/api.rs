use std::sync::Arc;
use axum::{
    extract::{Query, State, Path},
    response::{Html, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app::App;
use crate::config;
use crate::capacity;
use crate::queue;
use crate::mcp::{tools, types};

type AppState = Arc<App>;

/// Parse MCP tool JSON string result into Value, logging failures
fn parse_mcp(result: &str) -> Value {
    serde_json::from_str(result).unwrap_or_else(|e| {
        tracing::warn!("MCP tool returned unparseable JSON: {} (input: {})", e, &result[..result.len().min(200)]);
        json!({"error": "parse failed", "raw": &result[..result.len().min(500)]})
    })
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
pub async fn get_pane(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
) -> Json<Value> {
    // Get config data
    let config_result = tools::config_show(&app, types::ConfigShowRequest {
        pane: Some(pane_ref.clone()),
    }).await;
    let mut data = parse_mcp(&config_result);

    // Get PTY data via watch
    let watch_result = tools::watch(&app, types::WatchRequest {
        pane: pane_ref,
        tail: Some(100),
        analyze_errors: Some(true),
    }).await;
    let watch_data = parse_mcp(&watch_result);

    // Merge watch data into config data
    if let Some(obj) = data.as_object_mut() {
        if let Some(w) = watch_data.as_object() {
            for key in ["screen", "output", "pty_running", "line_count",
                        "pty_done", "pty_error", "pty_done_marker", "health",
                        "error_analysis", "progress"] {
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
    let result = tools::watch(&app, types::WatchRequest {
        pane: pane_ref,
        tail: params.lines.map(|l| l),
        analyze_errors: Some(false),
    }).await;
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
    let result = tools::logs(&app, types::LogsRequest {
        pane: None,
        lines: None,
    }).await;
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

    let agents: Vec<Value> = data["panes"].as_array()
        .map(|panes| panes.iter().filter(|p| {
            p["status"].as_str().map_or(false, |s| s == "active")
                || p["pty_active"].as_bool().unwrap_or(false)
        }).cloned().collect())
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
            roles.insert(key.clone(), json!({
                "name": name,
                "used": 0,
                "pct": 0,
            }));
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
pub async fn get_mcps(State(app): State<AppState>, Query(params): Query<SpaceQuery>) -> Json<Value> {
    let result = tools::mcp_list(&app, types::McpListRequest {
        category: None,
        project: params.space,
    }).await;
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

pub async fn get_mcp_route(State(app): State<AppState>, Query(params): Query<McpRouteQuery>) -> Json<Value> {
    let project = params.project.unwrap_or_default();
    if project.is_empty() {
        return Json(json!({"error": "missing project parameter"}));
    }
    let result = tools::mcp_route(&app, types::McpRouteRequest {
        project,
        task: params.task.unwrap_or_default(),
        role: params.role,
        apply: Some(false),
    }).await;
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
    let project = body.get("project").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let task = body.get("task").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let role = body.get("role").and_then(|v| v.as_str()).map(|s| s.to_string());
    let priority = body.get("priority").and_then(|v| v.as_u64()).map(|p| p as u8);
    let prompt = body.get("prompt").and_then(|v| v.as_str()).map(|s| s.to_string());
    let depends_on: Option<Vec<String>> = body.get("depends_on")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());

    if project.is_empty() || task.is_empty() {
        return Json(json!({"error": "project and task are required"}));
    }

    let result = tools::queue_add(&app, types::QueueAddRequest {
        project,
        task,
        role,
        prompt,
        priority,
        depends_on,
        max_retries: None,
    }).await;
    Json(parse_mcp(&result))
}

/// POST /api/queue/done — Mark task done (via tools::queue_done)
pub async fn post_queue_done(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let task_id = body.get("task_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let result_str = body.get("result").and_then(|v| v.as_str()).unwrap_or("done").to_string();
    if task_id.is_empty() {
        return Json(json!({"error": "task_id required"}));
    }
    let result = tools::queue_done(&app, types::QueueDoneRequest {
        task_id,
        result: Some(result_str),
    }).await;
    Json(parse_mcp(&result))
}

/// POST /api/queue/delete — Remove a task from queue
pub async fn post_queue_delete(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
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
            app.state.event_bus.send(crate::state::events::StateEvent::QueueChanged {
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
            app.state.event_bus.send(crate::state::events::StateEvent::QueueChanged {
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
    let result = tools::queue_list(&app, types::QueueListRequest {
        status: None,
    }).await;
    Json(parse_mcp(&result))
}

// === Enhanced monitoring endpoints ===

/// GET /api/monitor — Full monitoring overview (via tools::monitor)
pub async fn get_monitor(State(app): State<AppState>) -> Json<Value> {
    let result = tools::monitor(&app, types::MonitorRequest { include_output: Some(false) }).await;
    Json(parse_mcp(&result))
}

/// GET /api/pane/:id/watch — Watch pane output (via tools::watch)
pub async fn get_watch(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
    Query(params): Query<PaneQuery>,
) -> Json<Value> {
    let result = tools::watch(&app, types::WatchRequest {
        pane: pane_ref,
        tail: params.lines.or(Some(50)),
        analyze_errors: Some(true),
    }).await;
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

    let build_list: Vec<Value> = builds.iter().map(|b| {
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
    }).collect();

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
    let command = body.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if build == 0 || pane == 0 || command.is_empty() {
        return Json(json!({"error": "build (1-5), pane (1-3), and command are required"}));
    }

    let result = crate::mcp::tools::build_tools::build_send(build, pane, command);
    Json(parse_mcp(&result))
}

/// POST /api/builds/rename — Rename a build window
pub async fn post_build_rename(Json(body): Json<Value>) -> Json<Value> {
    let build = body.get("build").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if build == 0 || name.is_empty() {
        return Json(json!({"error": "build (1-5) and name are required"}));
    }

    let result = crate::mcp::tools::build_tools::build_rename(build, name);
    Json(parse_mcp(&result))
}

// === Helpers (file-based tracker data) ===

fn load_all_issues(space: &str) -> Vec<Value> {
    let dir = config::collab_root().join("spaces").join(space).join("issues");
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
    let statuses = ["backlog", "todo", "in_progress", "review", "done", "closed", "blocked"];
    let mut columns: std::collections::HashMap<&str, Vec<Value>> = statuses.iter().map(|s| (*s, Vec::new())).collect();

    for issue in load_all_issues(space) {
        let status = issue.get("status").and_then(|v| v.as_str()).unwrap_or("backlog");
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
                        if space.is_empty() || sprint.get("space").and_then(|v| v.as_str()) == Some(space) {
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
    let path = config::capacity_root().join("sprints").join(format!("{}.json", sprint_id));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return json!({"error": "sprint not found"}),
    };
    let sprint: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return json!({"error": "invalid sprint data"}),
    };

    let planned_acu = sprint.get("planned").and_then(|p| p.get("total_acu")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let days = sprint.get("days").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let start_date = sprint.get("start_date").and_then(|v| v.as_str()).unwrap_or("");

    let start = match chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return json!({"error": "invalid start_date"}),
    };

    let daily_burn = if days > 0 { planned_acu / days as f64 } else { 0.0 };

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
    let entries = log.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut actual = Vec::new();
    let mut cumulative = 0.0;
    let today = chrono::Local::now().date_naive();
    for d in 0..=days {
        let date = start + chrono::Duration::days(d as i64);
        if date > today {
            break;
        }
        let date_str = date.format("%Y-%m-%d").to_string();
        let day_acu: f64 = entries.iter()
            .filter(|e| e.get("logged_at").and_then(|v| v.as_str()).map_or(false, |s| s.starts_with(&date_str)))
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
