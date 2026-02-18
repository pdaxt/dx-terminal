use std::path::PathBuf;
use chrono::{Local, NaiveDateTime};

use crate::app::App;
use crate::config;
use crate::claude;
use crate::tracker;
use crate::capacity;
use crate::state;
use crate::state::types::PaneState;
use super::types::*;

/// Execute os_spawn logic
pub async fn spawn(app: &App, req: SpawnRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}. Use 1-9 or theme name.", req.pane)),
    };

    let role = req.role.unwrap_or_else(|| "developer".into());
    let task = req.task.unwrap_or_default();
    let prompt = req.prompt.unwrap_or_default();
    let theme = config::theme_name(pane_num);
    let project_path = config::resolve_project_path(&req.project);
    let project_name = PathBuf::from(&project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| req.project.clone());

    // Configure project MCPs
    let mcps = app.state.get_project_mcps(&project_name).await;
    if !mcps.is_empty() {
        let _ = claude::set_project_mcps(&project_path, &mcps);
    }

    // Generate and write preamble
    let preamble = claude::generate_preamble(pane_num, theme, &project_name, &role, &task, &prompt);
    let _ = claude::write_preamble(pane_num, &preamble);

    // Update state
    let pane_state = PaneState {
        theme: theme.to_string(),
        project: project_name.clone(),
        project_path: project_path.clone(),
        role: role.clone(),
        task: task.clone(),
        issue_id: None,
        space: None,
        status: "active".into(),
        started_at: Some(state::now()),
        acu_spent: 0.0,
    };
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(
        pane_num,
        "spawn",
        &format!("Spawned {} on {}: {}", role, project_name, truncate(&task, 40)),
    ).await;

    // Update multi_agent agents.json
    update_agents_json(pane_num, &project_name, &task);

    serde_json::json!({
        "status": "spawned",
        "pane": pane_num,
        "theme": theme,
        "project": project_name,
        "role": role,
        "task": task,
        "project_path": project_path,
        "note": "PTY spawn will be available in Phase 2. State updated."
    }).to_string()
}

/// Execute os_kill logic
pub async fn kill(app: &App, req: KillRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };
    let reason = req.reason.unwrap_or_else(|| "manual".into());

    // Update state
    let mut pane_state = app.state.get_pane(pane_num).await;
    pane_state.status = "idle".into();
    pane_state.task = String::new();
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(pane_num, "kill", &format!("Killed: {}", reason)).await;

    // Remove from multi_agent
    remove_from_agents_json(pane_num);

    serde_json::json!({
        "status": "killed",
        "pane": pane_num,
        "reason": reason
    }).to_string()
}

/// Execute os_restart logic
pub async fn restart(app: &App, req: RestartRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    if pane_data.project == "--" || pane_data.project.is_empty() {
        return json_err(&format!("Pane {} has no previous config to restart", pane_num));
    }

    // Kill first
    let _ = kill(app, KillRequest {
        pane: pane_num.to_string(),
        reason: Some("restart".into()),
    }).await;

    // Re-spawn with previous config
    spawn(app, SpawnRequest {
        pane: pane_num.to_string(),
        project: if pane_data.project_path.is_empty() {
            pane_data.project
        } else {
            pane_data.project_path
        },
        role: Some(pane_data.role),
        task: Some(pane_data.task),
        prompt: None,
    }).await
}

/// Execute os_reassign logic
pub async fn reassign(app: &App, req: ReassignRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let mut pane_data = app.state.get_pane(pane_num).await;
    if pane_data.status != "active" {
        return json_err(&format!("Pane {} is not active", pane_num));
    }

    if let Some(project) = &req.project {
        let path = config::resolve_project_path(project);
        pane_data.project = PathBuf::from(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project.clone());
        pane_data.project_path = path;
    }
    if let Some(role) = &req.role {
        pane_data.role = role.clone();
    }
    if let Some(task) = &req.task {
        pane_data.task = task.clone();
    }

    app.state.set_pane(pane_num, pane_data.clone()).await;
    app.state.log_activity(
        pane_num,
        "reassign",
        &format!("Reassigned: {}", truncate(req.task.as_deref().unwrap_or("config change"), 40)),
    ).await;

    serde_json::json!({
        "status": "reassigned",
        "pane": pane_num,
        "updates": {
            "project": pane_data.project,
            "role": pane_data.role,
            "task": pane_data.task,
        }
    }).to_string()
}

/// Execute os_assign logic
pub async fn assign(app: &App, req: AssignRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let issue = match tracker::find_issue(&req.space, &req.issue_id) {
        Some(i) => i,
        None => return json_err(&format!("Issue {} not found in space {}", req.issue_id, req.space)),
    };

    let project_path = app.state.get_space_project_path(&req.space).await
        .unwrap_or_else(|| format!("{}/Projects/{}", config::home_dir().display(), req.space));

    let state_snap = app.state.get_state_snapshot().await;
    let role = issue.get("role").and_then(|v| v.as_str())
        .unwrap_or(&state_snap.config.default_role)
        .to_string();

    let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let task = format!("[{}] {}", req.issue_id, title);
    let description = issue.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let priority = issue.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
    let issue_type = issue.get("type").and_then(|v| v.as_str()).unwrap_or("task");
    let est_acu = issue.get("estimated_acu").map(|v| v.to_string()).unwrap_or("not set".into());

    let prompt = format!(
        "You have been assigned issue {}: {}\n\nPriority: {}\nType: {}\n\nDescription:\n{}\n\nAcceptance criteria: Complete this issue and update its status when done.\nEstimated ACU: {}",
        req.issue_id, title, priority, issue_type, description, est_acu
    );

    // Update issue status
    let theme = config::theme_name(pane_num);
    let _ = tracker::update_issue(&req.space, &req.issue_id, &serde_json::json!({
        "status": "in_progress",
        "assignee": theme.to_lowercase(),
        "updated_at": state::now(),
    }));

    // Spawn agent
    let _result = spawn(app, SpawnRequest {
        pane: pane_num.to_string(),
        project: project_path,
        role: Some(role.clone()),
        task: Some(task),
        prompt: Some(prompt),
    }).await;

    // Update state with issue info
    let mut pane_data = app.state.get_pane(pane_num).await;
    pane_data.issue_id = Some(req.issue_id.clone());
    pane_data.space = Some(req.space.clone());
    app.state.set_pane(pane_num, pane_data).await;

    serde_json::json!({
        "status": "assigned",
        "pane": pane_num,
        "issue": req.issue_id,
        "title": title,
        "role": role,
    }).to_string()
}

/// Execute os_assign_adhoc logic
pub async fn assign_adhoc(app: &App, req: AssignAdhocRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let project = match &req.project {
        Some(p) if !p.is_empty() => p.clone(),
        _ => {
            let existing = app.state.get_pane(pane_num).await;
            if !existing.project_path.is_empty() {
                existing.project_path
            } else if existing.project != "--" {
                existing.project
            } else {
                "Projects".into()
            }
        }
    };

    spawn(app, SpawnRequest {
        pane: pane_num.to_string(),
        project,
        role: req.role.or(Some("developer".into())),
        task: Some(req.task),
        prompt: None,
    }).await
}

/// Execute os_collect logic
pub async fn collect(app: &App, req: CollectRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    // In Phase 2 this will read from PTY output buffer.
    // For now, return state-based info.
    let pane_data = app.state.get_pane(pane_num).await;
    let output = format!(
        "[Phase 2: PTY output capture not yet available]\nPane {} ({}) - Status: {} - Task: {}",
        pane_num, pane_data.theme, pane_data.status, pane_data.task
    );

    let done = pane_data.status == "done" || pane_data.status == "idle";

    serde_json::json!({
        "pane": pane_num,
        "done": done,
        "error": false,
        "output": output,
        "note": "PTY output capture available in Phase 2"
    }).to_string()
}

/// Execute os_complete logic
pub async fn complete(app: &App, req: CompleteRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let mut pane_data = app.state.get_pane(pane_num).await;
    let summary = req.summary.unwrap_or_default();

    // Calculate ACU spent
    let acu = if let Some(started) = &pane_data.started_at {
        if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
            let now = Local::now().naive_local();
            let hours = (now - start_dt).num_seconds() as f64 / 3600.0;
            (hours * 100.0).round() / 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Update tracker issue if assigned
    if let (Some(issue_id), Some(space)) = (&pane_data.issue_id, &pane_data.space) {
        let _ = tracker::update_issue(space, issue_id, &serde_json::json!({
            "status": "done",
            "actual_acu": acu,
            "updated_at": state::now(),
        }));
    }

    // Log to capacity work_log
    let review_needed = matches!(pane_data.role.as_str(), "frontend" | "backend" | "devops");
    let _ = capacity::log_work_entry(serde_json::json!({
        "issue_id": pane_data.issue_id.as_deref().unwrap_or("adhoc"),
        "space": pane_data.space.as_deref().unwrap_or(""),
        "role": pane_data.role,
        "pane_id": pane_num.to_string(),
        "acu_spent": acu,
        "review_needed": review_needed,
        "logged_at": state::now(),
        "summary": summary,
    }));

    // Update pane state
    pane_data.status = "idle".into();
    pane_data.acu_spent = acu;
    let task_display = truncate(&pane_data.task, 30);
    app.state.set_pane(pane_num, pane_data.clone()).await;
    app.state.log_activity(pane_num, "complete", &format!("Done: {} ({} ACU)", task_display, acu)).await;

    serde_json::json!({
        "status": "completed",
        "pane": pane_num,
        "acu_spent": acu,
        "issue_id": pane_data.issue_id,
        "summary": summary,
    }).to_string()
}

/// Execute os_set_mcps logic
pub async fn set_mcps(app: &App, req: SetMcpsRequest) -> String {
    app.state.set_project_mcps(&req.project, req.mcps.clone()).await;

    let project_path = config::resolve_project_path(&req.project);
    match claude::set_project_mcps(&project_path, &req.mcps) {
        Ok(()) => serde_json::json!({
            "status": "ok",
            "project": req.project,
            "mcps": req.mcps,
            "project_path": project_path,
        }).to_string(),
        Err(e) => serde_json::json!({
            "status": "partial",
            "state_updated": true,
            "claude_json_error": e.to_string(),
        }).to_string(),
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
        }).to_string(),
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
            return serde_json::json!({
                "pane": pane_num,
                "theme": config::theme_name(pane_num),
                "project": pane_data.project,
                "project_path": pane_data.project_path,
                "role": pane_data.role,
                "task": pane_data.task,
                "status": pane_data.status,
                "preamble_exists": claude::preamble_exists(pane_num),
                "project_mcps": mcps,
            }).to_string();
        }
    }

    // All panes
    let mut result = serde_json::Map::new();
    for i in 1..=9u8 {
        let pd = app.state.get_pane(i).await;
        result.insert(i.to_string(), serde_json::json!({
            "theme": config::theme_name(i),
            "project": pd.project,
            "role": pd.role,
            "task": pd.task,
            "status": pd.status,
        }));
    }
    serde_json::Value::Object(result).to_string()
}

/// Execute os_status logic
pub async fn status(app: &App) -> String {
    let mut panes = Vec::new();
    for i in 1..=9u8 {
        let pd = app.state.get_pane(i).await;
        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "task": truncate(&pd.task, 40),
            "acu": pd.acu_spent,
            "status": pd.status,
            "issue_id": pd.issue_id,
        }));
    }

    let active = panes.iter().filter(|p| p["status"] == "active").count();
    let idle = panes.iter().filter(|p| {
        let s = p["status"].as_str().unwrap_or("");
        s == "idle" || s.is_empty()
    }).count();

    serde_json::json!({
        "panes": panes,
        "summary": {"active": active, "idle": idle, "total": 9}
    }).to_string()
}

/// Execute os_dashboard logic
pub async fn dashboard(app: &App, req: DashboardRequest) -> String {
    let cap = capacity::load_capacity();
    let board = tracker::load_board_summary();

    let mut panes = Vec::new();
    for i in 1..=9u8 {
        let pd = app.state.get_pane(i).await;
        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(i),
            "project": pd.project,
            "task": truncate(&pd.task, 30),
            "role": config::role_short(&pd.role),
            "status": pd.status,
        }));
    }

    let state_snap = app.state.get_state_snapshot().await;
    let log: Vec<_> = state_snap.activity_log.iter().take(8).cloned().collect();

    let format = req.format.unwrap_or_else(|| "text".into());
    if format == "json" {
        return serde_json::json!({
            "capacity": {
                "acu_used": cap.acu_used,
                "acu_total": cap.acu_total,
                "reviews_used": cap.reviews_used,
                "reviews_total": cap.reviews_total,
            },
            "panes": panes,
            "board": board,
            "log": log,
        }).to_string();
    }

    // Text format
    let acu_pct = if cap.acu_total > 0.0 {
        (cap.acu_used / cap.acu_total * 100.0) as i32
    } else { 0 };
    let rev_pct = if cap.reviews_total > 0 {
        (cap.reviews_used as f64 / cap.reviews_total as f64 * 100.0) as i32
    } else { 0 };
    let bn = if rev_pct > 80 { "REVIEW" } else if acu_pct > 90 { "COMPUTE" } else { "BALANCED" };

    let mut lines = vec![
        format!("AgentOS Dashboard — {}", &state::now()[..16]),
        format!("ACU: {}/{} ({}%)  Reviews: {}/{}  Bottleneck: {}",
            cap.acu_used, cap.acu_total, acu_pct, cap.reviews_used, cap.reviews_total, bn),
        String::new(),
        " #  Theme   Project        Task                          Role  Status".into(),
        " -  ------  -------------- ------------------------------ ----  ------".into(),
    ];
    for p in &panes {
        lines.push(format!(" {}  {:<7} {:<14} {:<30} {:<5} {}",
            p["pane"], p["theme"].as_str().unwrap_or(""),
            p["project"].as_str().unwrap_or("--"),
            p["task"].as_str().unwrap_or("--"),
            p["role"].as_str().unwrap_or("--"),
            p["status"].as_str().unwrap_or("idle"),
        ));
    }

    lines.push(String::new());
    let board_str: Vec<String> = board.iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect();
    lines.push(format!("Board: {}", board_str.join("  ")));

    if !log.is_empty() {
        lines.push(String::new());
        lines.push("Recent:".into());
        for entry in log.iter().take(5) {
            let ts = if entry.ts.len() >= 16 { &entry.ts[11..16] } else { &entry.ts };
            lines.push(format!("  {} P{} {}", ts, entry.pane, truncate(&entry.summary, 50)));
        }
    }

    lines.join("\n")
}

/// Execute os_logs logic
pub async fn logs(app: &App, req: LogsRequest) -> String {
    let state = app.state.get_state_snapshot().await;
    let mut log: Vec<_> = state.activity_log.into_iter().collect();

    if let Some(pane_ref) = &req.pane {
        if let Some(pane_num) = config::resolve_pane(pane_ref) {
            log.retain(|e| e.pane == pane_num);
        }
    }

    let lines = req.lines.unwrap_or(20);
    log.truncate(lines);
    serde_json::to_string(&log).unwrap_or_else(|_| "[]".into())
}

/// Execute os_health logic
pub async fn health(app: &App) -> String {
    let state = app.state.get_state_snapshot().await;
    let stuck_mins = state.config.stuck_threshold_minutes;

    let mut results = Vec::new();
    for i in 1..=9u8 {
        let pd = app.state.get_pane(i).await;
        let status = &pd.status;
        let mut health = "ok";
        let mut detail = String::new();

        if status == "active" {
            // Check if stuck
            if let Some(started) = &pd.started_at {
                if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                    let now = Local::now().naive_local();
                    let mins = (now - start_dt).num_minutes();
                    if mins > (stuck_mins * 10) as i64 {
                        health = "stuck";
                        detail = format!("running {} minutes", mins);
                    }
                }
            }
        }

        results.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(i),
            "status": status,
            "health": health,
            "detail": detail,
        }));
    }

    let active = results.iter().filter(|r| r["status"] == "active").count();
    let stuck = results.iter().filter(|r| r["health"] == "stuck").count();
    let errors = results.iter().filter(|r| r["health"] == "error").count();

    serde_json::json!({
        "panes": results,
        "summary": {"active": active, "stuck": stuck, "errors": errors, "idle": 9 - active}
    }).to_string()
}

// --- Helpers ---

fn json_err(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}...", &s[..max-3]) }
}

fn update_agents_json(pane_num: u8, project: &str, task: &str) {
    let agents_file = config::multi_agent_root().join("agents.json");
    let mut agents = crate::state::persistence::read_json(&agents_file);
    let window = (pane_num as u32 - 1) / 3 + 1;
    let pane = (pane_num as u32 - 1) % 3 + 1;
    let pane_id = format!("{}:{}.{}", config::SESSION_NAME, window, pane);
    if let Some(obj) = agents.as_object_mut() {
        obj.insert(pane_id, serde_json::json!({
            "project": project,
            "task": task,
            "files": [],
            "registered_at": state::now(),
            "last_update": state::now(),
        }));
    }
    let _ = crate::state::persistence::write_json(&agents_file, &agents);
}

fn remove_from_agents_json(pane_num: u8) {
    let agents_file = config::multi_agent_root().join("agents.json");
    let mut agents = crate::state::persistence::read_json(&agents_file);
    let window = (pane_num as u32 - 1) / 3 + 1;
    let pane = (pane_num as u32 - 1) % 3 + 1;
    let pane_id = format!("{}:{}.{}", config::SESSION_NAME, window, pane);
    if let Some(obj) = agents.as_object_mut() {
        obj.remove(&pane_id);
    }
    let _ = crate::state::persistence::write_json(&agents_file, &agents);
}
