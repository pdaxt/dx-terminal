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

type AppState = Arc<App>;

/// GET / — Serve dashboard HTML
pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../assets/dashboard.html"))
}

// === Query parameter structs ===

#[derive(Deserialize, Default)]
pub struct SpaceQuery {
    pub space: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct SprintQuery {
    pub sprint: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct PaneQuery {
    pub lines: Option<usize>,
}

// === AgentOS state endpoints (powered by in-memory state + PTY) ===

/// GET /api/status — All 9 panes with PTY state
pub async fn get_status(State(app): State<AppState>) -> Json<Value> {
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let pty = app.pty_lock();
    let mut panes = Vec::new();
    for (i, pd) in &pane_states {
        panes.push(json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "theme_color": config::theme_fg(*i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "role_full": pd.role,
            "task": pd.task,
            "acu": pd.acu_spent,
            "status": pd.status,
            "issue_id": pd.issue_id,
            "space": pd.space,
            "branch": pd.branch_name,
            "workspace": pd.workspace_path,
            "started_at": pd.started_at,
            "pty_running": pty.is_running(*i),
            "pty_active": pty.has_agent(*i),
            "line_count": pty.line_count(*i),
        }));
    }
    drop(pty);

    let active = panes.iter().filter(|p| p["status"] == "active").count();
    let idle = panes.iter().filter(|p| {
        let s = p["status"].as_str().unwrap_or("");
        s == "idle" || s.is_empty()
    }).count();
    let pty_count = panes.iter().filter(|p| p["pty_running"].as_bool().unwrap_or(false)).count();

    Json(json!({
        "panes": panes,
        "summary": {
            "active": active,
            "idle": idle,
            "total": 9,
            "pty_running": pty_count,
        }
    }))
}

/// GET /api/pane/:id — Single pane detail
pub async fn get_pane(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
) -> Json<Value> {
    let pane_num = match config::resolve_pane(&pane_ref) {
        Some(n) => n,
        None => return Json(json!({"error": format!("Invalid pane: {}", pane_ref)})),
    };

    let pd = app.state.get_pane(pane_num).await;
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    let pty_info = {
        let pty = app.pty_lock();
        if pty.has_agent(pane_num) {
            let screen = pty.screen_text(pane_num).unwrap_or_default();
            let output = pty.last_output(pane_num, 100).unwrap_or_default();
            let running = pty.is_running(pane_num);
            let health = pty.check_health(pane_num, &markers);
            let line_count = pty.line_count(pane_num);
            Some((screen, output, running, health, line_count))
        } else {
            None
        }
    };

    if let Some((screen, output, running, health, line_count)) = pty_info {
        Json(json!({
            "pane": pane_num,
            "theme": config::theme_name(pane_num),
            "theme_color": config::theme_fg(pane_num),
            "project": pd.project,
            "project_path": pd.project_path,
            "role": pd.role,
            "task": pd.task,
            "status": pd.status,
            "started_at": pd.started_at,
            "issue_id": pd.issue_id,
            "space": pd.space,
            "branch": pd.branch_name,
            "workspace": pd.workspace_path,
            "acu_spent": pd.acu_spent,
            "pty_running": running,
            "pty_done": health.done,
            "pty_error": health.error,
            "pty_done_marker": health.done_marker,
            "line_count": line_count,
            "screen": screen,
            "output": output,
        }))
    } else {
        Json(json!({
            "pane": pane_num,
            "theme": config::theme_name(pane_num),
            "theme_color": config::theme_fg(pane_num),
            "project": pd.project,
            "project_path": pd.project_path,
            "role": pd.role,
            "task": pd.task,
            "status": pd.status,
            "started_at": pd.started_at,
            "issue_id": pd.issue_id,
            "space": pd.space,
            "branch": pd.branch_name,
            "workspace": pd.workspace_path,
            "acu_spent": pd.acu_spent,
            "pty_running": false,
            "pty_active": false,
            "line_count": 0,
            "screen": "",
            "output": "",
        }))
    }
}

/// GET /api/pane/:id/output — PTY output for a pane
pub async fn get_pane_output(
    State(app): State<AppState>,
    Path(pane_ref): Path<String>,
    Query(params): Query<PaneQuery>,
) -> Json<Value> {
    let pane_num = match config::resolve_pane(&pane_ref) {
        Some(n) => n,
        None => return Json(json!({"error": format!("Invalid pane: {}", pane_ref)})),
    };

    let lines = params.lines.unwrap_or(50);
    let pty = app.pty_lock();
    let output = pty.last_output(pane_num, lines).unwrap_or_default();
    let screen = pty.screen_text(pane_num).unwrap_or_default();
    let running = pty.is_running(pane_num);
    let line_count = pty.line_count(pane_num);
    drop(pty);

    Json(json!({
        "pane": pane_num,
        "running": running,
        "line_count": line_count,
        "output": output,
        "screen": screen,
    }))
}

/// GET /api/health — PTY health for all panes
pub async fn get_health(State(app): State<AppState>) -> Json<Value> {
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();
    let stuck_mins = state_snap.config.stuck_threshold_minutes;

    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let pty = app.pty_lock();
    let mut results = Vec::new();
    for (i, pd) in &pane_states {
        let has_pty = pty.has_agent(*i);
        if has_pty {
            let health = pty.check_health(*i, &markers);
            let mut health_status = if health.error.is_some() {
                "error"
            } else if health.done {
                "done"
            } else if health.running {
                "ok"
            } else {
                "stopped"
            };

            if pd.status == "active" && health.running && !health.done {
                if let Some(started) = &pd.started_at {
                    if let Ok(start_dt) = chrono::NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                        let now = chrono::Local::now().naive_local();
                        let mins = (now - start_dt).num_minutes();
                        if mins > (stuck_mins * 10) as i64 {
                            health_status = "stuck";
                        }
                    }
                }
            }

            results.push(json!({
                "pane": i,
                "theme": config::theme_name(*i),
                "theme_color": config::theme_fg(*i),
                "status": pd.status,
                "health": health_status,
                "pty_running": health.running,
                "has_output": health.has_output,
                "error": health.error,
                "done_marker": health.done_marker,
                "line_count": pty.line_count(*i),
            }));
        } else {
            let health_status = match pd.status.as_str() {
                "idle" | "" => "idle",
                "active" => "no_pty",
                "done" => "done",
                "error" => "error",
                _ => "unknown",
            };
            results.push(json!({
                "pane": i,
                "theme": config::theme_name(*i),
                "theme_color": config::theme_fg(*i),
                "status": pd.status,
                "health": health_status,
                "pty_running": false,
                "has_output": false,
                "error": Value::Null,
                "done_marker": Value::Null,
                "line_count": 0,
            }));
        }
    }
    drop(pty);

    let active = results.iter().filter(|r| r["status"] == "active").count();
    let stuck = results.iter().filter(|r| r["health"] == "stuck").count();
    let errors = results.iter().filter(|r| r["health"] == "error").count();

    Json(json!({
        "panes": results,
        "summary": {
            "active": active,
            "stuck": stuck,
            "errors": errors,
            "idle": 9 - active,
        }
    }))
}

/// GET /api/logs — Activity log
pub async fn get_logs(
    State(app): State<AppState>,
    Query(_params): Query<SpaceQuery>,
) -> Json<Value> {
    let state = app.state.get_state_snapshot().await;
    let log: Vec<_> = state.activity_log.into_iter().collect();
    Json(json!(log))
}

// === Tracker / capacity endpoints (backward compat with hub_mcp dashboard) ===

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

/// GET /api/agents — Agent list (from in-memory state, not agents.json)
pub async fn get_agents(State(app): State<AppState>) -> Json<Value> {
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let pty = app.pty_lock();
    let mut agents = Vec::new();
    for (i, pd) in &pane_states {
        if pd.status == "active" || pty.has_agent(*i) {
            let window = (*i as u32 - 1) / 3 + 1;
            let pane = (*i as u32 - 1) % 3 + 1;
            agents.push(json!({
                "pane": format!("{}:{}.{}", config::SESSION_NAME, window, pane),
                "pane_num": i,
                "theme": config::theme_name(*i),
                "theme_color": config::theme_fg(*i),
                "project": pd.project,
                "task": pd.task,
                "role": pd.role,
                "status": pd.status,
                "branch": pd.branch_name,
                "workspace": pd.workspace_path,
                "pty_running": pty.is_running(*i),
                "files": [],
            }));
        }
    }
    drop(pty);

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

    // Role utilization from capacity config
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

/// GET /api/mcps — List all available MCPs
pub async fn get_mcps(Query(params): Query<SpaceQuery>) -> Json<Value> {
    let registry = crate::mcp_registry::load_registry();
    let filtered: Vec<_> = if let Some(project) = &params.space {
        registry.into_iter().filter(|mcp| {
            mcp.projects.iter().any(|p| p.eq_ignore_ascii_case(project))
        }).collect()
    } else {
        registry
    };

    let items: Vec<Value> = filtered.iter().map(|mcp| {
        json!({
            "name": mcp.name,
            "description": mcp.description,
            "category": mcp.category,
            "capabilities": mcp.capabilities,
            "projects": mcp.projects,
        })
    }).collect();
    Json(json!(items))
}

/// GET /api/mcps/route — Smart route MCPs for a project+task
#[derive(Deserialize, Default)]
pub struct McpRouteQuery {
    pub project: Option<String>,
    pub task: Option<String>,
    pub role: Option<String>,
}

pub async fn get_mcp_route(Query(params): Query<McpRouteQuery>) -> Json<Value> {
    let project = params.project.unwrap_or_default();
    let task = params.task.unwrap_or_default();
    let role = params.role.unwrap_or_else(|| "developer".into());

    if project.is_empty() {
        return Json(json!({"error": "missing project parameter"}));
    }

    let matches = crate::mcp_registry::route_mcps(&project, &task, &role);
    let suggestions: Vec<Value> = matches.iter().take(10).map(|m| {
        json!({
            "name": m.name,
            "score": m.score,
            "reasons": m.reasons,
            "description": m.description,
        })
    }).collect();

    Json(json!({
        "project": project,
        "task": task,
        "role": role,
        "suggestions": suggestions,
    }))
}

/// GET /api/roles — Role config
pub async fn get_roles() -> Json<Value> {
    let cfg_path = config::capacity_root().join("config.json");
    let cfg = crate::state::persistence::read_json(&cfg_path);
    Json(cfg.get("roles").cloned().unwrap_or_else(|| json!({})))
}

/// POST /api/queue/add — Add a task from web UI
pub async fn post_queue_add(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let project = body.get("project").and_then(|v| v.as_str()).unwrap_or("");
    let task = body.get("task").and_then(|v| v.as_str()).unwrap_or("");
    let role = body.get("role").and_then(|v| v.as_str()).unwrap_or("developer");
    let priority = body.get("priority").and_then(|v| v.as_u64()).unwrap_or(3) as u8;
    let prompt = body.get("prompt").and_then(|v| v.as_str()).unwrap_or(task);

    if project.is_empty() || task.is_empty() {
        return Json(json!({"error": "project and task are required"}));
    }

    let deps: Vec<String> = body.get("depends_on")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    match queue::add_task(project, role, task, prompt, priority, deps) {
        Ok(t) => {
            app.state.event_bus.send(crate::state::events::StateEvent::QueueChanged {
                action: "added".into(),
                task_id: t.id.clone(),
                task: t.task.clone(),
            });
            Json(json!({"status": "added", "task_id": t.id}))
        }
        Err(e) => Json(json!({"error": format!("{}", e)})),
    }
}

/// POST /api/queue/done — Mark a task done from web UI
pub async fn post_queue_done(State(app): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let task_id = body.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    let result = body.get("result").and_then(|v| v.as_str()).unwrap_or("done");
    if task_id.is_empty() {
        return Json(json!({"error": "task_id required"}));
    }
    match queue::mark_done(task_id, result) {
        Ok(()) => {
            app.state.event_bus.send(crate::state::events::StateEvent::QueueChanged {
                action: "done".into(),
                task_id: task_id.to_string(),
                task: String::new(),
            });
            Json(json!({"status": "done", "task_id": task_id}))
        }
        Err(e) => Json(json!({"error": format!("{}", e)})),
    }
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

/// GET /api/queue — Task queue with status counts
pub async fn get_queue() -> Json<Value> {
    let q = queue::load_queue();
    let cfg = queue::load_auto_config();

    let mut pending = 0usize;
    let mut running = 0usize;
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut blocked = 0usize;

    let tasks: Vec<Value> = q.tasks.iter().map(|t| {
        match t.status {
            queue::QueueStatus::Pending => pending += 1,
            queue::QueueStatus::Running => running += 1,
            queue::QueueStatus::Done => done += 1,
            queue::QueueStatus::Failed => failed += 1,
            queue::QueueStatus::Blocked => blocked += 1,
        }
        json!({
            "id": t.id,
            "project": t.project,
            "role": t.role,
            "task": t.task,
            "priority": t.priority,
            "status": t.status,
            "pane": t.pane,
            "added_at": t.added_at,
            "started_at": t.started_at,
            "completed_at": t.completed_at,
            "result": t.result,
            "depends_on": t.depends_on,
        })
    }).collect();

    Json(json!({
        "tasks": tasks,
        "summary": {
            "pending": pending,
            "running": running,
            "done": done,
            "failed": failed,
            "blocked": blocked,
            "total": tasks.len(),
        },
        "config": {
            "max_parallel": cfg.max_parallel,
            "reserved_panes": cfg.reserved_panes,
            "auto_complete": cfg.auto_complete,
            "auto_assign": cfg.auto_assign,
        }
    }))
}

// === Helpers ===

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

    // Only include non-empty columns
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

    // Ideal burndown
    let mut ideal = Vec::new();
    for d in 0..=days {
        let date = start + chrono::Duration::days(d as i64);
        ideal.push(json!({
            "day": d,
            "date": date.format("%Y-%m-%d").to_string(),
            "remaining": ((planned_acu - daily_burn * d as f64) * 100.0).round() / 100.0,
        }));
    }

    // Actual burndown from work log
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
