use std::path::PathBuf;
use chrono::{Local, NaiveDateTime};

use crate::app::App;
use crate::config;
use crate::claude;
use crate::tracker;
use crate::capacity;
use crate::state;
use crate::state::types::PaneState;
use crate::workspace;
use crate::queue;
use super::types::*;

/// Execute os_spawn logic — allocates PTY and spawns Claude agent
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

    // Configure project MCPs — auto-route if none explicitly set
    let mut mcps = app.state.get_project_mcps(&project_name).await;
    if mcps.is_empty() {
        // Smart routing: infer MCPs from project + task + role
        let matches = crate::mcp_registry::route_mcps(&project_name, &task, &role);
        mcps = matches.iter()
            .filter(|m| m.score >= 20)
            .map(|m| m.name.clone())
            .collect();
        if !mcps.is_empty() {
            app.state.set_project_mcps(&project_name, mcps.clone()).await;
        }
    }
    if !mcps.is_empty() {
        let _ = claude::set_project_mcps(&project_path, &mcps);
    }

    // Git-first: create worktree for isolation if project is a git repo
    let (spawn_cwd, ws_path, ws_branch) = if workspace::is_git_repo(&project_path) {
        match workspace::create_worktree(&project_path, pane_num, &task) {
            Ok(info) => {
                tracing::info!("Created worktree for pane {}: {} (branch {})", pane_num, info.worktree_path, info.branch_name);
                (info.worktree_path.clone(), Some(info.worktree_path), Some(info.branch_name))
            }
            Err(e) => {
                tracing::warn!("Worktree creation failed for pane {}, using direct path: {}", pane_num, e);
                (project_path.clone(), None, None)
            }
        }
    } else {
        (project_path.clone(), None, None)
    };

    // Generate and write preamble
    let preamble = claude::generate_preamble(pane_num, theme, &project_name, &role, &task, &prompt);
    let _ = claude::write_preamble(pane_num, &preamble);

    // Build env vars
    let config_dir = claude::account_config_dir(pane_num);
    let env_vars = vec![
        ("P".to_string(), pane_num.to_string()),
        ("CLAUDE_CONFIG_DIR".to_string(), config_dir),
    ];

    // Spawn PTY in worktree (isolated) or project dir (fallback)
    let pty_result = {
        let mut pty = app.pty_lock();
        pty.spawn(pane_num, "claude", &["-c"], &spawn_cwd, env_vars)
    };

    let pty_status = match pty_result {
        Ok(()) => "pty_spawned".to_string(),
        Err(e) => format!("pty_error: {}", e),
    };

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
        workspace_path: ws_path.clone(),
        branch_name: ws_branch.clone(),
    };
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(
        pane_num,
        "spawn",
        &format!("Spawned {} on {}: {}", role, project_name, truncate(&task, 40)),
    ).await;

    // Update multi_agent agents.json
    update_agents_json(pane_num, &project_name, &task);

    // Schedule initial prompt delivery after delay
    if !prompt.is_empty() || !task.is_empty() {
        let initial_msg = if !prompt.is_empty() {
            prompt.clone()
        } else {
            task.clone()
        };
        // Send the initial prompt to the PTY after a short delay
        // (Claude CLI needs time to initialize)
        let pty_arc = std::sync::Arc::clone(&app.pty);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let mut pty = pty_arc.lock().unwrap_or_else(|e| e.into_inner());
            let _ = pty.send_line(pane_num, &initial_msg);
        });
    }

    serde_json::json!({
        "status": "spawned",
        "pane": pane_num,
        "theme": theme,
        "project": project_name,
        "role": role,
        "task": task,
        "project_path": project_path,
        "workspace": ws_path,
        "branch": ws_branch,
        "pty": pty_status,
    }).to_string()
}

/// Execute os_kill logic — kills PTY process and cleans up state
pub async fn kill(app: &App, req: KillRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };
    let reason = req.reason.unwrap_or_else(|| "manual".into());

    // Get workspace info before clearing state
    let pane_data = app.state.get_pane(pane_num).await;
    let ws_path = pane_data.workspace_path.clone();
    let project_path = pane_data.project_path.clone();

    // Kill PTY
    let pty_result = {
        let mut pty = app.pty_lock();
        pty.kill(pane_num)
    };
    let pty_status = match pty_result {
        Ok(()) => "killed",
        Err(_) => "no_pty",
    };

    // Git-first: save WIP and cleanup worktree
    let mut git_info = serde_json::Value::Null;
    if let Some(ws) = &ws_path {
        let commit_result = workspace::commit_all(ws, &format!("WIP: killed ({})", reason));
        let wt_result = workspace::remove_worktree(&project_path, ws);
        git_info = serde_json::json!({
            "wip_commit": commit_result.unwrap_or_else(|e| e.to_string()),
            "worktree_removed": wt_result.is_ok(),
        });
    }

    // Update state
    let mut pane_state = pane_data;
    pane_state.status = "idle".into();
    pane_state.task = String::new();
    pane_state.workspace_path = None;
    pane_state.branch_name = None;
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(pane_num, "kill", &format!("Killed: {}", reason)).await;

    // Remove from multi_agent
    remove_from_agents_json(pane_num);

    serde_json::json!({
        "status": "killed",
        "pane": pane_num,
        "reason": reason,
        "pty": pty_status,
        "git": git_info,
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

/// Execute os_reassign logic — sends new task to running agent via PTY
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

    // Send new task to the running agent via PTY
    if let Some(task) = &req.task {
        let msg = format!(
            "NEW TASK: {}\nRole: {}\nProject: {}\nPlease acknowledge and begin working on this new task.",
            task, pane_data.role, pane_data.project
        );
        let send_result = {
            let mut pty = app.pty_lock();
            pty.send_line(pane_num, &msg)
        };
        if let Err(e) = send_result {
            tracing::warn!("Failed to send reassign message to pane {}: {}", pane_num, e);
        }
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

/// Execute os_collect logic — reads real PTY output
pub async fn collect(app: &App, req: CollectRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    // Fetch state first (async), then PTY (sync) — never hold MutexGuard across await
    let pane_data = app.state.get_pane(pane_num).await;
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    // Collect PTY info under lock, then drop immediately
    let pty_info = {
        let pty = app.pty_lock();
        if pty.has_agent(pane_num) {
            let output = pty.last_output(pane_num, 50).unwrap_or_default();
            let screen = pty.screen_text(pane_num).unwrap_or_default();
            let running = pty.is_running(pane_num);
            let health = pty.check_health(pane_num, &markers);
            let line_count = pty.line_count(pane_num);
            Some((output, screen, running, health, line_count))
        } else {
            None
        }
    };

    // Git-first: include workspace git info if available
    let git_info = if let Some(ws) = &pane_data.workspace_path {
        let status = workspace::git_status(ws).unwrap_or_default();
        let diff = workspace::git_diff(ws).unwrap_or_default();
        serde_json::json!({
            "branch": pane_data.branch_name,
            "status": status,
            "diff_stat": diff,
        })
    } else {
        serde_json::json!(null)
    };

    if let Some((output, screen, running, health, line_count)) = pty_info {
        let display_output = if !screen.trim().is_empty() {
            truncate(&screen, 3000)
        } else {
            truncate(&output, 3000)
        };

        // Auto-update state if agent has finished
        if health.done && pane_data.status == "active" {
            app.state.update_pane_status(pane_num, "done").await;
        }

        serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": pane_data.status,
            "branch": pane_data.branch_name,
            "running": running,
            "done": health.done,
            "error": health.error,
            "done_marker": health.done_marker,
            "output": display_output,
            "line_count": line_count,
            "git": git_info,
        }).to_string()
    } else {
        let done = pane_data.status == "done" || pane_data.status == "idle";
        serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": pane_data.status,
            "branch": pane_data.branch_name,
            "running": false,
            "done": done,
            "error": serde_json::Value::Null,
            "output": format!("[No PTY] Pane {} - Status: {}", pane_num, pane_data.status),
            "line_count": 0,
            "git": git_info,
        }).to_string()
    }
}

/// Execute os_complete logic
pub async fn complete(app: &App, req: CompleteRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let mut pane_data = app.state.get_pane(pane_num).await;
    let summary = req.summary.clone().unwrap_or_default();

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

    // Git-first: commit, push, create PR, cleanup worktree
    let mut git_info = serde_json::json!(null);
    if let (Some(ws), Some(branch)) = (&pane_data.workspace_path, &pane_data.branch_name) {
        let commit_msg = if summary.is_empty() {
            format!("Pane {}: {}", pane_num, truncate(&pane_data.task, 60))
        } else {
            summary.clone()
        };
        let commit_result = workspace::commit_all(ws, &commit_msg);
        let push_result = workspace::push_branch(ws, branch);
        let pr_title = format!("[Pane {}] {}", pane_num, truncate(&pane_data.task, 50));
        let pr_body = format!(
            "## Task\n{}\n\n## Summary\n{}\n\n## ACU\n{:.2}\n\nAutomated PR from AgentOS pane {}",
            pane_data.task, if summary.is_empty() { "completed" } else { &summary }, acu, pane_num
        );
        let pr_result = workspace::create_pr(ws, &pr_title, &pr_body);
        let remove_result = workspace::remove_worktree(&pane_data.project_path, ws);

        git_info = serde_json::json!({
            "commit": commit_result.unwrap_or_else(|e| e.to_string()),
            "push": push_result.unwrap_or_else(|e| e.to_string()),
            "pr": pr_result.unwrap_or_else(|e| e.to_string()),
            "worktree_removed": remove_result.is_ok(),
            "branch": branch,
        });
    }

    // Kill the PTY process
    {
        let mut pty = app.pty_lock();
        let _ = pty.kill(pane_num);
    }

    // Update pane state
    pane_data.status = "idle".into();
    pane_data.acu_spent = acu;
    pane_data.workspace_path = None;
    pane_data.branch_name = None;
    let task_display = truncate(&pane_data.task, 30);
    app.state.set_pane(pane_num, pane_data.clone()).await;
    app.state.log_activity(pane_num, "complete", &format!("Done: {} ({} ACU)", task_display, acu)).await;

    serde_json::json!({
        "status": "completed",
        "pane": pane_num,
        "acu_spent": acu,
        "issue_id": pane_data.issue_id,
        "summary": summary,
        "git": git_info,
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
            }).to_string();
        }
    }

    // Fetch all pane state first (async)
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    // Then check PTY (sync)
    let pty = app.pty_lock();
    let mut result = serde_json::Map::new();
    for (i, pd) in &pane_states {
        result.insert(i.to_string(), serde_json::json!({
            "theme": config::theme_name(*i),
            "project": pd.project,
            "role": pd.role,
            "task": pd.task,
            "status": pd.status,
            "pty_active": pty.has_agent(*i),
        }));
    }
    drop(pty);
    serde_json::Value::Object(result).to_string()
}

/// Execute os_status logic
pub async fn status(app: &App) -> String {
    // Fetch state first (async), then PTY (sync)
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let pty = app.pty_lock();
    let mut panes = Vec::new();
    for (i, pd) in &pane_states {
        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "task": truncate(&pd.task, 40),
            "acu": pd.acu_spent,
            "status": pd.status,
            "issue_id": pd.issue_id,
            "branch": pd.branch_name,
            "pty_running": pty.is_running(*i),
        }));
    }
    drop(pty);

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

    // Fetch all state first (async)
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }
    let state_snap = app.state.get_state_snapshot().await;
    let log: Vec<_> = state_snap.activity_log.iter().take(8).cloned().collect();

    // Then PTY info (sync)
    let pty = app.pty_lock();
    let mut panes = Vec::new();
    for (i, pd) in &pane_states {
        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "project": pd.project,
            "task": truncate(&pd.task, 30),
            "role": config::role_short(&pd.role),
            "status": pd.status,
            "pty": pty.is_running(*i),
        }));
    }
    drop(pty);

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

    let now_str = state::now();
    let display_ts = now_str.get(..16).unwrap_or(&now_str);
    let mut lines = vec![
        format!("AgentOS Dashboard — {}", display_ts),
        format!("ACU: {}/{} ({}%)  Reviews: {}/{}  Bottleneck: {}",
            cap.acu_used, cap.acu_total, acu_pct, cap.reviews_used, cap.reviews_total, bn),
        String::new(),
        " #  Theme   Project        Task                          Role  Status  PTY".into(),
        " -  ------  -------------- ------------------------------ ----  ------  ---".into(),
    ];
    for p in &panes {
        lines.push(format!(" {}  {:<7} {:<14} {:<30} {:<5} {:<7} {}",
            p["pane"], p["theme"].as_str().unwrap_or(""),
            p["project"].as_str().unwrap_or("--"),
            p["task"].as_str().unwrap_or("--"),
            p["role"].as_str().unwrap_or("--"),
            p["status"].as_str().unwrap_or("idle"),
            if p["pty"].as_bool().unwrap_or(false) { "Y" } else { "-" },
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
            let ts = entry.ts.get(11..16).unwrap_or(&entry.ts);
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

/// Execute os_health logic — real PTY health checks
pub async fn health(app: &App) -> String {
    let state = app.state.get_state_snapshot().await;
    let stuck_mins = state.config.stuck_threshold_minutes;
    let markers = state.config.completion_markers.clone();

    // Fetch all pane state first (async)
    let mut pane_states = Vec::new();
    for i in 1..=9u8 {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    // Then collect PTY health info (sync)
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

            // Check for stuck
            if pd.status == "active" && health.running && !health.done {
                if let Some(started) = &pd.started_at {
                    if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                        let now = Local::now().naive_local();
                        let mins = (now - start_dt).num_minutes();
                        if mins > (stuck_mins * 10) as i64 {
                            health_status = "stuck";
                        }
                    }
                }
            }

            results.push(serde_json::json!({
                "pane": *i,
                "theme": config::theme_name(*i),
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

            results.push(serde_json::json!({
                "pane": *i,
                "theme": config::theme_name(*i),
                "status": pd.status,
                "health": health_status,
                "pty_running": false,
                "has_output": false,
                "error": serde_json::Value::Null,
                "done_marker": serde_json::Value::Null,
                "line_count": 0,
            }));
        }
    }
    drop(pty);

    let active = results.iter().filter(|r| r["status"] == "active").count();
    let stuck = results.iter().filter(|r| r["health"] == "stuck").count();
    let errors = results.iter().filter(|r| r["health"] == "error").count();
    let pty_count = results.iter().filter(|r| r["pty_running"].as_bool().unwrap_or(false)).count();

    serde_json::json!({
        "panes": results,
        "summary": {
            "active": active,
            "stuck": stuck,
            "errors": errors,
            "idle": 9 - active,
            "pty_running": pty_count,
        }
    }).to_string()
}

/// Execute os_mcp_list logic — list available MCPs with metadata
pub async fn mcp_list(_app: &App, req: McpListRequest) -> String {
    let registry = crate::mcp_registry::load_registry();

    let filtered: Vec<_> = registry.into_iter().filter(|mcp| {
        if let Some(cat) = &req.category {
            if !mcp.category.eq_ignore_ascii_case(cat) {
                return false;
            }
        }
        if let Some(proj) = &req.project {
            if !mcp.projects.iter().any(|p| p.eq_ignore_ascii_case(proj)) {
                return false;
            }
        }
        true
    }).collect();

    let items: Vec<serde_json::Value> = filtered.iter().map(|mcp| {
        serde_json::json!({
            "name": mcp.name,
            "description": mcp.description,
            "category": mcp.category,
            "capabilities": mcp.capabilities,
            "projects": mcp.projects,
        })
    }).collect();

    serde_json::json!({
        "count": items.len(),
        "mcps": items,
    }).to_string()
}

/// Execute os_mcp_route logic — smart MCP routing based on project+task+role
pub async fn mcp_route(app: &App, req: McpRouteRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let matches = crate::mcp_registry::route_mcps(&req.project, &req.task, &role);

    // Top matches (score > 0)
    let suggested: Vec<String> = matches.iter()
        .filter(|m| m.score >= 20) // Meaningful match threshold
        .map(|m| m.name.clone())
        .collect();

    let details: Vec<serde_json::Value> = matches.iter().take(15).map(|m| {
        serde_json::json!({
            "name": m.name,
            "score": m.score,
            "reasons": m.reasons,
            "description": m.description,
        })
    }).collect();

    // Auto-apply if requested
    if req.apply.unwrap_or(false) && !suggested.is_empty() {
        app.state.set_project_mcps(&req.project, suggested.clone()).await;
        let project_path = config::resolve_project_path(&req.project);
        let _ = claude::set_project_mcps(&project_path, &suggested);
    }

    serde_json::json!({
        "project": req.project,
        "task": req.task,
        "role": role,
        "suggested_mcps": suggested,
        "applied": req.apply.unwrap_or(false),
        "details": details,
    }).to_string()
}

/// Execute os_mcp_search logic — search MCPs by capability or keyword
pub async fn mcp_search(_app: &App, req: McpSearchRequest) -> String {
    let results = crate::mcp_registry::search(&req.query);

    let items: Vec<serde_json::Value> = results.iter().map(|mcp| {
        serde_json::json!({
            "name": mcp.name,
            "description": mcp.description,
            "category": mcp.category,
            "capabilities": mcp.capabilities,
            "projects": mcp.projects,
            "keywords": mcp.keywords,
        })
    }).collect();

    serde_json::json!({
        "query": req.query,
        "count": items.len(),
        "results": items,
    }).to_string()
}

// === GIT TOOLS ===

/// Execute os_git_sync — pull latest from base branch into agent's worktree
pub async fn git_sync(app: &App, req: GitSyncRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    // Determine base branch from branch name (pane-N/slug was branched from base)
    let base = workspace::git_status(&ws)
        .map(|_| "main".to_string()) // simplified — we stored base_branch at create time but not in state
        .unwrap_or_else(|_| "main".into());

    let result = workspace::sync_from_main(&ws, &base);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "base_branch": base,
        "result": result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

/// Execute os_git_status — show git status/diff for agent's worktree
pub async fn git_status_tool(app: &App, req: GitStatusRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let ws = match &pane_data.workspace_path {
        Some(ws) => ws.clone(),
        None => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let status = workspace::git_status(&ws).unwrap_or_default();
    let diff = if req.verbose.unwrap_or(false) {
        // Full diff
        std::process::Command::new("git")
            .args(["diff"])
            .current_dir(&ws)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    } else {
        workspace::git_diff(&ws).unwrap_or_default()
    };

    serde_json::json!({
        "pane": pane_num,
        "branch": pane_data.branch_name,
        "status": status,
        "diff": truncate(&diff, 5000),
    }).to_string()
}

/// Execute os_git_push — commit and push agent's current work
pub async fn git_push(app: &App, req: GitPushRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    let msg = req.message.unwrap_or_else(|| {
        format!("Pane {}: {}", pane_num, truncate(&pane_data.task, 60))
    });

    let commit_result = workspace::commit_all(&ws, &msg);
    let push_result = workspace::push_branch(&ws, &branch);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "commit": commit_result.unwrap_or_else(|e| e.to_string()),
        "push": push_result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

/// Execute os_git_pr — create a PR from agent's branch
pub async fn git_pr(app: &App, req: GitPrRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let (ws, branch) = match (&pane_data.workspace_path, &pane_data.branch_name) {
        (Some(ws), Some(br)) => (ws.clone(), br.clone()),
        _ => return json_err(&format!("Pane {} has no git workspace", pane_num)),
    };

    // Commit any outstanding changes first
    let _ = workspace::commit_all(&ws, &format!("Pane {}: pre-PR commit", pane_num));

    // Push
    let push_result = workspace::push_branch(&ws, &branch);

    // Create PR
    let title = req.title.unwrap_or_else(|| {
        format!("[Pane {}] {}", pane_num, truncate(&pane_data.task, 50))
    });
    let body = req.body.unwrap_or_else(|| {
        format!("## Task\n{}\n\nAutomated PR from AgentOS pane {}", pane_data.task, pane_num)
    });
    let pr_result = workspace::create_pr(&ws, &title, &body);

    serde_json::json!({
        "pane": pane_num,
        "branch": branch,
        "push": push_result.unwrap_or_else(|e| e.to_string()),
        "pr": pr_result.unwrap_or_else(|e| e.to_string()),
    }).to_string()
}

// === QUEUE / AUTO-CYCLE ===

/// Add a task to the queue
pub async fn queue_add(_app: &App, req: QueueAddRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let prompt = req.prompt.unwrap_or_else(|| req.task.clone());
    let priority = req.priority.unwrap_or(3);
    let depends_on = req.depends_on.unwrap_or_default();

    match queue::add_task(&req.project, &role, &req.task, &prompt, priority, depends_on) {
        Ok(task) => serde_json::json!({
            "status": "queued",
            "task_id": task.id,
            "project": task.project,
            "task": task.task,
            "priority": task.priority,
            "depends_on": task.depends_on,
        }).to_string(),
        Err(e) => json_err(&format!("Failed to add task: {}", e)),
    }
}

/// List queue tasks
pub async fn queue_list(_app: &App, req: QueueListRequest) -> String {
    let q = queue::load_queue();

    let filtered: Vec<&queue::QueueTask> = q.tasks.iter().filter(|t| {
        if let Some(status) = &req.status {
            let s = format!("{:?}", t.status).to_lowercase();
            s == status.to_lowercase()
        } else {
            true
        }
    }).collect();

    let items: Vec<serde_json::Value> = filtered.iter().map(|t| {
        serde_json::json!({
            "id": t.id,
            "project": t.project,
            "task": truncate(&t.task, 50),
            "role": t.role,
            "priority": t.priority,
            "status": format!("{:?}", t.status).to_lowercase(),
            "pane": t.pane,
            "depends_on": t.depends_on,
        })
    }).collect();

    let pending = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Pending).count();
    let running = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Running).count();
    let done = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Done).count();

    serde_json::json!({
        "tasks": items,
        "summary": { "pending": pending, "running": running, "done": done, "total": q.tasks.len() },
    }).to_string()
}

/// Mark a queue task as done
pub async fn queue_done(_app: &App, req: QueueDoneRequest) -> String {
    let result = req.result.unwrap_or_else(|| "completed".into());
    match queue::mark_done(&req.task_id, &result) {
        Ok(()) => {
            let next = queue::next_task();
            serde_json::json!({
                "status": "done",
                "task_id": req.task_id,
                "next_pending": next.map(|t| serde_json::json!({"id": t.id, "task": t.task, "project": t.project})),
            }).to_string()
        }
        Err(e) => json_err(&format!("Failed to mark done: {}", e)),
    }
}

/// Auto-cycle: scan all panes, complete finished agents, spawn next tasks
pub async fn auto_cycle(app: &App) -> String {
    let cfg = queue::load_auto_config();
    let mut actions = Vec::new();
    let mut occupied_panes = Vec::new();

    // Phase 1: Collect status of all running panes
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    for i in 1..=9u8 {
        let pd = app.state.get_pane(i).await;
        if pd.status != "active" { continue; }
        occupied_panes.push(i);

        // Check if this agent has finished
        let health = {
            let pty = app.pty_lock();
            if pty.has_agent(i) {
                Some(pty.check_health(i, &markers))
            } else {
                None
            }
        };

        if let Some(h) = health {
            if h.done && cfg.auto_complete {
                // Auto-complete this pane
                let _result = complete(app, super::types::CompleteRequest {
                    pane: i.to_string(),
                    summary: Some(format!("Auto-completed by cycle")),
                }).await;

                // Mark queue task as done
                if let Some(qt) = queue::task_for_pane(i) {
                    let _ = queue::mark_done(&qt.id, "auto-completed");
                }

                occupied_panes.retain(|&p| p != i);
                actions.push(serde_json::json!({
                    "action": "auto_complete",
                    "pane": i,
                    "project": pd.project,
                }));
            } else if h.error.is_some() {
                // Mark as failed, free pane
                if let Some(qt) = queue::task_for_pane(i) {
                    let _ = queue::mark_failed(&qt.id, h.error.as_deref().unwrap_or("unknown error"));
                }
                let _ = kill(app, super::types::KillRequest {
                    pane: i.to_string(),
                    reason: Some(format!("error: {}", h.error.unwrap_or_default())),
                }).await;
                occupied_panes.retain(|&p| p != i);
                actions.push(serde_json::json!({
                    "action": "error_kill",
                    "pane": i,
                    "project": pd.project,
                }));
            }
        }
    }

    // Phase 2: Spawn next tasks on free panes
    if cfg.auto_assign {
        loop {
            let free_pane = queue::find_free_pane(&cfg, &occupied_panes);
            let next_task = queue::next_task();

            match (free_pane, next_task) {
                (Some(pane), Some(task)) => {
                    // Mark running
                    let _ = queue::mark_running(&task.id, pane);
                    occupied_panes.push(pane);

                    // Spawn
                    let _result = spawn(app, super::types::SpawnRequest {
                        pane: pane.to_string(),
                        project: task.project.clone(),
                        role: Some(task.role.clone()),
                        task: Some(task.task.clone()),
                        prompt: Some(task.prompt.clone()),
                    }).await;

                    actions.push(serde_json::json!({
                        "action": "auto_spawn",
                        "pane": pane,
                        "task_id": task.id,
                        "project": task.project,
                        "task": truncate(&task.task, 40),
                    }));
                }
                _ => break,
            }
        }
    }

    // Summary
    let q = queue::load_queue();
    let pending = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Pending).count();
    let running = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Running).count();

    serde_json::json!({
        "actions": actions,
        "queue": { "pending": pending, "running": running },
        "occupied_panes": occupied_panes,
        "config": {
            "max_parallel": cfg.max_parallel,
            "auto_complete": cfg.auto_complete,
            "auto_assign": cfg.auto_assign,
        },
        "instruction": if pending > 0 || running > 0 {
            "Call os_auto again in 30-60 seconds to continue the cycle."
        } else {
            "Queue empty. Add tasks with os_queue_add or wait."
        },
    }).to_string()
}

/// Update auto-cycle config
pub async fn auto_config(_app: &App, req: AutoConfigRequest) -> String {
    let mut cfg = queue::load_auto_config();
    if let Some(mp) = req.max_parallel { cfg.max_parallel = mp.clamp(1, 9); }
    if let Some(rp) = req.reserved_panes { cfg.reserved_panes = rp; }
    if let Some(ac) = req.auto_complete { cfg.auto_complete = ac; }
    if let Some(aa) = req.auto_assign { cfg.auto_assign = aa; }
    if let Some(ci) = req.cycle_interval_secs { cfg.cycle_interval_secs = ci; }

    match queue::save_auto_config(&cfg) {
        Ok(()) => serde_json::json!({
            "status": "updated",
            "config": {
                "max_parallel": cfg.max_parallel,
                "reserved_panes": cfg.reserved_panes,
                "auto_complete": cfg.auto_complete,
                "auto_assign": cfg.auto_assign,
                "cycle_interval_secs": cfg.cycle_interval_secs,
            }
        }).to_string(),
        Err(e) => json_err(&format!("Failed to save config: {}", e)),
    }
}

// --- Helpers ---

fn json_err(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", end)
    }
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
