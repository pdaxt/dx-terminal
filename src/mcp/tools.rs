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
use crate::machine;
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
    let (spawn_cwd, ws_path, ws_branch, ws_base) = if workspace::is_git_repo(&project_path) {
        match workspace::create_worktree(&project_path, pane_num, &task) {
            Ok(info) => {
                tracing::info!("Created worktree for pane {}: {} (branch {})", pane_num, info.worktree_path, info.branch_name);
                (info.worktree_path.clone(), Some(info.worktree_path), Some(info.branch_name), Some(info.base_branch))
            }
            Err(e) => {
                tracing::warn!("Worktree creation failed for pane {}, using direct path: {}", pane_num, e);
                (project_path.clone(), None, None, None)
            }
        }
    } else {
        (project_path.clone(), None, None, None)
    };

    // Generate preamble and write as CLAUDE.md in workspace for auto-load
    let preamble = claude::generate_preamble(pane_num, theme, &project_name, &role, &task, &prompt);
    let _ = claude::write_preamble(pane_num, &preamble);
    // Also write to workspace CLAUDE.md so claude reads it on start
    let claude_md_path = format!("{}/CLAUDE.md", spawn_cwd);
    let _ = std::fs::write(&claude_md_path, &preamble);

    // Register machine identity
    let machine_id = machine::register(pane_num);

    // Build env vars (includes machine identity)
    let config_dir = claude::account_config_dir(pane_num);
    let env_vars = vec![
        ("P".to_string(), pane_num.to_string()),
        ("CLAUDE_CONFIG_DIR".to_string(), config_dir),
        ("MACHINE_IP".to_string(), machine_id.ip.clone()),
        ("MACHINE_HOSTNAME".to_string(), machine_id.hostname.clone()),
        ("MACHINE_MAC".to_string(), machine_id.mac.clone()),
        ("MACHINE_PANE".to_string(), pane_num.to_string()),
    ];

    // Build claude args: skip permissions (trust dialog) + provide task as prompt
    let task_prompt = format!("{}\n\n{}", task, if prompt.is_empty() { "" } else { &prompt });
    let claude_args = vec![
        "--dangerously-skip-permissions",
        "-p",
        &task_prompt,
    ];

    // Spawn PTY in worktree (isolated) or project dir (fallback)
    let pty_result = {
        let mut pty = app.pty_lock();
        pty.spawn(pane_num, "claude", &claude_args, &spawn_cwd, env_vars)
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
        base_branch: ws_base.clone(),
        machine_ip: Some(machine_id.ip.clone()),
        machine_hostname: Some(machine_id.hostname.clone()),
        machine_mac: Some(machine_id.mac.clone()),
    };
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(
        pane_num,
        "spawn",
        &format!("Spawned {} on {}: {}", role, project_name, truncate(&task, 40)),
    ).await;

    // Register agent in coordination DB
    update_agents_json(pane_num, &project_name, &task);

    // Auto-claim git branch in coordination DB so other agents see it
    if let Some(ref branch) = ws_branch {
        let window = (pane_num as u32 - 1) / 3 + 1;
        let pane = (pane_num as u32 - 1) % 3 + 1;
        let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
        let _ = crate::multi_agent::git_claim_branch(&pane_id, branch, &project_name, &task);
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
        "machine_ip": machine_id.ip,
        "machine_hostname": machine_id.hostname,
        "machine_mac": machine_id.mac,
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

    // Save output before killing
    let output_log = save_agent_output(app, pane_num, &reason);

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
    let branch_name = pane_data.branch_name.clone();
    let project_name = pane_data.project.clone();
    if let Some(ws) = &ws_path {
        let commit_result = workspace::commit_all(ws, &format!("WIP: killed ({})", reason));
        let wt_result = workspace::remove_worktree(&project_path, ws);
        git_info = serde_json::json!({
            "wip_commit": commit_result.unwrap_or_else(|e| e.to_string()),
            "worktree_removed": wt_result.is_ok(),
        });
    }

    // Release git branch claim in coordination DB
    if let Some(ref branch) = branch_name {
        let window = (pane_num as u32 - 1) / 3 + 1;
        let pane = (pane_num as u32 - 1) % 3 + 1;
        let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
        let _ = crate::multi_agent::git_release_branch(&pane_id, branch, &project_name);
    }

    // Deregister machine identity
    machine::deregister(pane_num);

    // Update state — fully reset to idle
    let mut pane_state = pane_data;
    pane_state.status = "idle".into();
    pane_state.task = String::new();
    pane_state.project = "--".into();
    pane_state.project_path = String::new();
    pane_state.role = "--".into();
    pane_state.started_at = None;
    pane_state.acu_spent = 0.0;
    pane_state.issue_id = None;
    pane_state.space = None;
    pane_state.workspace_path = None;
    pane_state.branch_name = None;
    pane_state.base_branch = None;
    pane_state.machine_ip = None;
    pane_state.machine_hostname = None;
    pane_state.machine_mac = None;
    app.state.set_pane(pane_num, pane_state).await;
    app.state.log_activity(pane_num, "kill", &format!("Killed: {}", reason)).await;

    // Remove from multi_agent (CASCADE deletes file locks too)
    remove_from_agents_json(pane_num);

    serde_json::json!({
        "status": "killed",
        "pane": pane_num,
        "reason": reason,
        "pty": pty_status,
        "git": git_info,
        "output_log": output_log,
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

    // Update coordination DB so other agents see the new project/task
    update_agents_json(pane_num, &pane_data.project, &pane_data.task);

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
            "exit_code": health.exit_code,
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
    // Auto-extract result from agent output if no summary provided
    let summary = req.summary.clone().unwrap_or_else(|| extract_result(app, pane_num));

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
        let pr_url = pr_result.as_ref().map(|s| s.as_str()).unwrap_or("");
        let auto_merge = workspace::auto_merge_pr(ws, pr_url).unwrap_or_else(|e| e.to_string());
        let remove_result = workspace::remove_worktree(&pane_data.project_path, ws);

        git_info = serde_json::json!({
            "commit": commit_result.unwrap_or_else(|e| e.to_string()),
            "push": push_result.unwrap_or_else(|e| e.to_string()),
            "pr": pr_result.unwrap_or_else(|e| e.to_string()),
            "auto_merge": auto_merge,
            "worktree_removed": remove_result.is_ok(),
            "branch": branch,
        });
    }

    // Save output before killing PTY
    let _output_log = save_agent_output(app, pane_num, "completed");

    // Save handoff context to KB for dependent tasks
    let result_text = extract_result(app, pane_num);
    if let Some(qt) = queue::task_for_pane(pane_num) {
        let window = (pane_num as u32 - 1) / 3 + 1;
        let pane_pos = (pane_num as u32 - 1) % 3 + 1;
        let pane_id = format!("{}:{}.{}", config::session_name(), window, pane_pos);
        let handoff_content = format!(
            "Task: {}\nResult: {}\nSummary: {}\nBranch: {}\nPR: {}",
            qt.task,
            result_text,
            summary,
            pane_data.branch_name.as_deref().unwrap_or("none"),
            git_info.get("pr").and_then(|v| v.as_str()).unwrap_or("none"),
        );
        let _ = crate::multi_agent::kb_add(
            &pane_id, &pane_data.project, "agent_handoff",
            &qt.id, &handoff_content, &[],
        );
    }

    // Kill the PTY process
    {
        let mut pty = app.pty_lock();
        let _ = pty.kill(pane_num);
    }

    // Release git branch claim in coordination DB
    if let Some(ref branch) = pane_data.branch_name {
        let window = (pane_num as u32 - 1) / 3 + 1;
        let pane = (pane_num as u32 - 1) % 3 + 1;
        let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
        let _ = crate::multi_agent::git_release_branch(&pane_id, branch, &pane_data.project);
    }

    // Deregister from coordination DB (CASCADE deletes file locks)
    remove_from_agents_json(pane_num);

    // Update pane state — preserve acu_spent for reporting, reset everything else
    let task_display = truncate(&pane_data.task, 30);
    pane_data.status = "idle".into();
    pane_data.acu_spent = acu;
    pane_data.task = String::new();
    pane_data.project = "--".into();
    pane_data.project_path = String::new();
    pane_data.role = "--".into();
    pane_data.started_at = None;
    pane_data.issue_id = None;
    pane_data.space = None;
    pane_data.workspace_path = None;
    pane_data.branch_name = None;
    pane_data.base_branch = None;
    pane_data.machine_ip = None;
    pane_data.machine_hostname = None;
    pane_data.machine_mac = None;
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
    for i in 1..=config::pane_count() {
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
    for i in 1..=config::pane_count() {
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
            "exit_code": pty.exit_code(*i),
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
        "summary": {"active": active, "idle": idle, "total": config::pane_count()}
    }).to_string()
}

/// Execute os_dashboard logic
pub async fn dashboard(app: &App, req: DashboardRequest) -> String {
    let cap = capacity::load_capacity();
    let board = tracker::load_board_summary();

    // Fetch all state first (async)
    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
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
    for i in 1..=config::pane_count() {
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
                "exit_code": health.exit_code,
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
            "idle": config::pane_count() as usize - active,
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

    // Use stored base_branch from worktree creation, fall back to detecting from remote
    let base = pane_data.base_branch.clone().unwrap_or_else(|| {
        // Detect default branch from remote
        std::process::Command::new("git")
            .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
            .current_dir(&ws)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    // strip "origin/" prefix
                    Some(s.strip_prefix("origin/").unwrap_or(&s).to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "main".into())
    });

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

/// Merge an agent's branch back into the base branch
pub async fn git_merge(app: &App, req: GitMergeRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;
    let project_path = &pane_data.project_path;
    if project_path.is_empty() {
        return json_err(&format!("Pane {} has no project", pane_num));
    }

    let branch = req.branch.or(pane_data.branch_name.clone())
        .unwrap_or_default();
    if branch.is_empty() {
        return json_err(&format!("Pane {} has no branch to merge", pane_num));
    }

    let base = pane_data.base_branch.clone().unwrap_or_else(|| "main".into());
    let project_name = &pane_data.project;

    match workspace::merge_branch(project_path, &branch, &base) {
        Ok(result) => {
            // Release branch claim in coordination DB after successful merge
            let window = (pane_num as u32 - 1) / 3 + 1;
            let pane = (pane_num as u32 - 1) % 3 + 1;
            let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
            let _ = crate::multi_agent::git_release_branch(&pane_id, &branch, project_name);

            serde_json::json!({
                "status": "merged",
                "pane": pane_num,
                "branch": branch,
                "base": base,
                "result": result,
            }).to_string()
        }
        Err(e) => serde_json::json!({
            "status": "failed",
            "pane": pane_num,
            "branch": branch,
            "base": base,
            "error": e.to_string(),
        }).to_string(),
    }
}

// === QUEUE / AUTO-CYCLE ===

/// Add a task to the queue
pub async fn queue_add(_app: &App, req: QueueAddRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let prompt = req.prompt.unwrap_or_else(|| req.task.clone());
    let priority = req.priority.unwrap_or(3);
    let depends_on = req.depends_on.unwrap_or_default();

    match queue::add_task(&req.project, &role, &req.task, &prompt, priority, depends_on) {
        Ok(task) => {
            // Apply max_retries if specified
            if let Some(mr) = req.max_retries {
                let mut q = queue::load_queue();
                if let Some(t) = q.tasks.iter_mut().find(|t| t.id == task.id) {
                    t.max_retries = mr;
                }
                let _ = queue::save_queue(&q);
            }
            serde_json::json!({
                "status": "queued",
                "task_id": task.id,
                "project": task.project,
                "task": task.task,
                "priority": task.priority,
                "depends_on": task.depends_on,
                "max_retries": req.max_retries.unwrap_or(2),
            }).to_string()
        }
        Err(e) => json_err(&format!("Failed to add task: {}", e)),
    }
}

/// Decompose a high-level goal into sub-tasks with auto-wired dependencies
pub async fn queue_decompose(_app: &App, req: DecomposeRequest) -> String {
    let max = req.max_subtasks.unwrap_or(5) as usize;
    let role = req.role.unwrap_or_else(|| "developer".into());
    let priority = req.priority.unwrap_or(3);

    // Parse goal into sub-tasks: split on numbered items, bullets, or double newlines
    let mut subtasks: Vec<(String, bool)> = Vec::new(); // (task_text, is_parallel)
    let lines: Vec<&str> = req.goal.lines().collect();

    let mut current = String::new();
    let mut is_parallel = false;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.trim().is_empty() {
                subtasks.push((current.trim().to_string(), is_parallel));
                current.clear();
                is_parallel = false;
            }
            continue;
        }

        // Check for list markers: "1.", "2.", "- ", "* "
        let is_new_item = trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || (trimmed.len() > 2 && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.'));

        if is_new_item {
            if !current.trim().is_empty() {
                subtasks.push((current.trim().to_string(), is_parallel));
            }
            // Strip the marker
            let text = trimmed
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '*')
                .trim();
            // Check for parallel prefix "||"
            is_parallel = text.starts_with("||");
            let clean = text.trim_start_matches("||").trim();
            current = clean.to_string();
        } else {
            if current.is_empty() {
                current = trimmed.to_string();
            } else {
                current.push(' ');
                current.push_str(trimmed);
            }
        }
    }
    if !current.trim().is_empty() {
        subtasks.push((current.trim().to_string(), is_parallel));
    }

    // Cap at max
    subtasks.truncate(max);

    if subtasks.is_empty() {
        return json_err("No sub-tasks found. Use numbered steps (1. 2. 3.) or bullets (- *).");
    }

    // Create queue tasks with sequential dependencies (unless parallel)
    let mut created_ids: Vec<String> = Vec::new();
    let mut task_infos: Vec<serde_json::Value> = Vec::new();

    for (i, (task_text, parallel)) in subtasks.iter().enumerate() {
        let deps = if *parallel || i == 0 {
            vec![] // First task or parallel tasks have no deps
        } else {
            vec![created_ids[i - 1].clone()] // Depends on previous
        };

        match queue::add_task(&req.project, &role, task_text, task_text, priority, deps.clone()) {
            Ok(task) => {
                created_ids.push(task.id.clone());
                task_infos.push(serde_json::json!({
                    "id": task.id,
                    "task": truncate(task_text, 60),
                    "depends_on": deps,
                    "parallel": parallel,
                }));
            }
            Err(e) => {
                return json_err(&format!("Failed creating sub-task {}: {}", i + 1, e));
            }
        }
    }

    serde_json::json!({
        "status": "decomposed",
        "project": req.project,
        "subtasks": task_infos,
        "count": created_ids.len(),
        "task_ids": created_ids,
    }).to_string()
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

    for i in 1..=config::pane_count() {
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
                // Extract result before completing
                let result = extract_result(app, i);

                let _result = complete(app, super::types::CompleteRequest {
                    pane: i.to_string(),
                    summary: Some("Auto-completed by cycle".into()),
                }).await;

                if let Some(qt) = queue::task_for_pane(i) {
                    let _ = queue::mark_done(&qt.id, &result);
                }

                occupied_panes.retain(|&p| p != i);
                actions.push(serde_json::json!({
                    "action": "auto_complete",
                    "pane": i,
                    "project": pd.project,
                    "exit_code": h.exit_code,
                    "result": result,
                }));
            } else if h.error.is_some() {
                let _output_log = save_agent_output(app, i, &format!("error: {}", h.error.as_deref().unwrap_or("unknown")));

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
                    "exit_code": h.exit_code,
                }));
            }
        }
    }

    // Phase 1.5: Kill stuck agents (running too long without completion)
    let stuck_threshold = state_snap.config.stuck_threshold_minutes;
    for i in 1..=config::pane_count() {
        if !occupied_panes.contains(&i) { continue; }
        let pd = app.state.get_pane(i).await;
        if pd.status != "active" { continue; }
        if let Some(started) = &pd.started_at {
            if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                let now = Local::now().naive_local();
                let mins = (now - start_dt).num_minutes();
                if mins > (stuck_threshold * 10) as i64 {
                    let is_done = {
                        let pty = app.pty_lock();
                        if pty.has_agent(i) { pty.check_health(i, &markers).done } else { true }
                    };
                    if !is_done {
                        let _output_log = save_agent_output(app, i, &format!("stuck: {} minutes", mins));
                        if let Some(qt) = queue::task_for_pane(i) {
                            let _ = queue::mark_failed(&qt.id, &format!("stuck for {} minutes", mins));
                        }
                        let _ = kill(app, super::types::KillRequest {
                            pane: i.to_string(),
                            reason: Some(format!("stuck: {} minutes", mins)),
                        }).await;
                        occupied_panes.retain(|&p| p != i);
                        actions.push(serde_json::json!({
                            "action": "stuck_kill", "pane": i,
                            "project": pd.project, "minutes": mins,
                        }));
                    }
                }
            }
        }
    }

    // Phase 1.7: Auto-retry failed tasks with retries remaining
    {
        let q = queue::load_queue();
        let retryable: Vec<String> = q.tasks.iter()
            .filter(|t| t.status == queue::QueueStatus::Failed && t.retry_count < t.max_retries)
            .map(|t| t.id.clone())
            .collect();
        for task_id in retryable {
            if let Ok(true) = queue::requeue_failed(&task_id) {
                actions.push(serde_json::json!({ "action": "auto_retry", "task_id": task_id }));
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

                    // Build handoff context from predecessor tasks
                    let mut prompt = task.prompt.clone();
                    if !task.depends_on.is_empty() {
                        let mut handoff_parts = Vec::new();
                        for dep_id in &task.depends_on {
                            // Get predecessor result from queue
                            if let Some(dep_task) = queue::task_by_id(dep_id) {
                                let result = dep_task.result.as_deref().unwrap_or("completed");
                                handoff_parts.push(format!("- {} ({}): {}", dep_task.task, dep_id, result));
                            }
                            // Get richer context from KB
                            let kb = crate::multi_agent::kb_search(dep_id, Some(&task.project), Some("agent_handoff"));
                            if let Some(entries) = kb.get("entries").and_then(|v| v.as_array()) {
                                for entry in entries {
                                    if let Some(content) = entry.get("content").and_then(|v| v.as_str()) {
                                        handoff_parts.push(format!("  KB: {}", content));
                                    }
                                }
                            }
                        }
                        if !handoff_parts.is_empty() {
                            prompt = format!("{}\n\n## Predecessor Results\n{}", prompt, handoff_parts.join("\n"));
                        }
                    }

                    // Spawn
                    let _result = spawn(app, super::types::SpawnRequest {
                        pane: pane.to_string(),
                        project: task.project.clone(),
                        role: Some(task.role.clone()),
                        task: Some(task.task.clone()),
                        prompt: Some(prompt),
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

// === MONITORING TOOLS ===

/// os_monitor — Single-call "what's happening right now" overview
pub async fn monitor(app: &App, req: MonitorRequest) -> String {
    let include_output = req.include_output.unwrap_or(false);
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    // Collect all pane states
    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    // PTY health check
    let pty = app.pty_lock();
    let mut panes = Vec::new();
    let mut alerts = Vec::new();
    let mut active_count = 0u32;
    let mut idle_count = 0u32;
    let mut done_count = 0u32;
    let mut error_count = 0u32;
    let mut stuck_count = 0u32;

    for (i, pd) in &pane_states {
        let has_pty = pty.has_agent(*i);
        let running = pty.is_running(*i);
        let line_count = pty.line_count(*i);

        let mut health_status = match pd.status.as_str() {
            "active" => { active_count += 1; "active" },
            "done" => { done_count += 1; "done" },
            "error" => { error_count += 1; "error" },
            _ => { idle_count += 1; "idle" },
        };

        let mut error_msg: Option<String> = None;
        let mut done_marker: Option<String> = None;
        let mut exit_code: Option<i32> = None;
        let mut output_snippet = String::new();

        if has_pty {
            let h = pty.check_health(*i, &markers);
            exit_code = h.exit_code;
            if h.error.is_some() {
                health_status = "error";
                error_msg = h.error.clone();
                if pd.status == "active" { error_count += 1; active_count = active_count.saturating_sub(1); }
                alerts.push(serde_json::json!({
                    "level": "error",
                    "pane": i,
                    "theme": config::theme_name(*i),
                    "message": format!("Error detected: {}", h.error.as_deref().unwrap_or("unknown")),
                }));
            } else if h.done {
                health_status = "done";
                done_marker = h.done_marker.clone();
                if pd.status == "active" {
                    alerts.push(serde_json::json!({
                        "level": "info",
                        "pane": i,
                        "theme": config::theme_name(*i),
                        "message": "Agent finished — ready for completion",
                    }));
                }
            }

            // Stuck detection
            if pd.status == "active" && running && !h.done && h.error.is_none() {
                if let Some(started) = &pd.started_at {
                    if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                        let now = Local::now().naive_local();
                        let mins = (now - start_dt).num_minutes();
                        if mins > (state_snap.config.stuck_threshold_minutes * 10) as i64 {
                            health_status = "stuck";
                            stuck_count += 1;
                            active_count = active_count.saturating_sub(1);
                            alerts.push(serde_json::json!({
                                "level": "warning",
                                "pane": i,
                                "theme": config::theme_name(*i),
                                "message": format!("Agent stuck for {} minutes", mins),
                            }));
                        }
                    }
                }
            }

            if include_output && has_pty {
                let screen = pty.screen_text(*i).unwrap_or_default();
                output_snippet = truncate(&screen, 500);
            }
        }

        let mut pane_info = serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "task": truncate(&pd.task, 50),
            "health": health_status,
            "pty": running,
            "lines": line_count,
            "branch": pd.branch_name,
        });

        if let Some(e) = &error_msg {
            pane_info["error"] = serde_json::json!(e);
        }
        if let Some(c) = exit_code {
            pane_info["exit_code"] = serde_json::json!(c);
        }
        if let Some(d) = &done_marker {
            pane_info["done_marker"] = serde_json::json!(d);
        }
        if !output_snippet.is_empty() {
            pane_info["output"] = serde_json::json!(output_snippet);
        }
        // Only show runtime for active (non-idle) panes
        if pd.status != "idle" {
            if let Some(started) = &pd.started_at {
                if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                    let now = Local::now().naive_local();
                    let mins = (now - start_dt).num_minutes();
                    pane_info["runtime_mins"] = serde_json::json!(mins);
                }
            }
        }

        panes.push(pane_info);
    }
    drop(pty);

    // Queue summary
    let q = queue::load_queue();
    let q_pending = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Pending).count();
    let q_running = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Running).count();
    let q_done = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Done).count();
    let q_failed = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Failed).count();

    // Capacity
    let cap = capacity::load_capacity();
    let acu_pct = if cap.acu_total > 0.0 { (cap.acu_used / cap.acu_total * 100.0) as i32 } else { 0 };

    // Recent activity (last 5)
    let recent: Vec<_> = state_snap.activity_log.iter().take(5).map(|e| {
        serde_json::json!({
            "time": e.ts.get(11..16).unwrap_or(&e.ts),
            "pane": e.pane,
            "event": e.event,
            "summary": truncate(&e.summary, 60),
        })
    }).collect();

    // Build status text header
    let urgency = if error_count > 0 || stuck_count > 0 {
        "ATTENTION NEEDED"
    } else if alerts.is_empty() {
        "ALL CLEAR"
    } else {
        "OK"
    };

    serde_json::json!({
        "status": urgency,
        "panes": panes,
        "summary": {
            "active": active_count,
            "idle": idle_count,
            "done": done_count,
            "errors": error_count,
            "stuck": stuck_count,
            "total": config::pane_count(),
        },
        "alerts": alerts,
        "queue": {
            "pending": q_pending,
            "running": q_running,
            "done": q_done,
            "failed": q_failed,
        },
        "capacity": {
            "acu_used": cap.acu_used,
            "acu_total": cap.acu_total,
            "acu_pct": acu_pct,
        },
        "recent": recent,
    }).to_string()
}

/// os_project_status — Everything about one project across all panes, issues, git, capacity
pub async fn project_status(app: &App, req: ProjectStatusRequest) -> String {
    let include_issues = req.include_issues.unwrap_or(true);
    let include_git = req.include_git.unwrap_or(true);
    let project_lower = req.project.to_lowercase();

    // Find all panes working on this project
    let mut project_panes = Vec::new();
    for i in 1..=config::pane_count() {
        let pd = app.state.get_pane(i).await;
        if pd.project.to_lowercase().contains(&project_lower) || pd.project_path.to_lowercase().contains(&project_lower) {
            let pty = app.pty_lock();
            let running = pty.is_running(i);
            let lines = pty.line_count(i);
            drop(pty);

            project_panes.push(serde_json::json!({
                "pane": i,
                "theme": config::theme_name(i),
                "role": pd.role,
                "task": truncate(&pd.task, 60),
                "status": pd.status,
                "branch": pd.branch_name,
                "pty_running": running,
                "lines": lines,
                "acu": pd.acu_spent,
            }));
        }
    }

    // Find matching tracker spaces and issues
    let mut issues_data = serde_json::json!(null);
    let mut board_data = serde_json::json!(null);
    if include_issues {
        let spaces_dir = config::collab_root().join("spaces");
        let mut matched_space = String::new();
        if let Ok(entries) = std::fs::read_dir(&spaces_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains(&project_lower) {
                    matched_space = entry.file_name().to_string_lossy().to_string();
                    break;
                }
            }
        }
        if !matched_space.is_empty() {
            let board = tracker::board_view(&matched_space);
            board_data = serde_json::from_str(&board.to_string()).unwrap_or(serde_json::json!(null));

            // Count by status
            let issues_dir = config::collab_root().join("spaces").join(&matched_space).join("issues");
            let mut counts = std::collections::HashMap::new();
            if let Ok(entries) = std::fs::read_dir(&issues_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            if let Ok(issue) = serde_json::from_str::<serde_json::Value>(&content) {
                                let status = issue.get("status").and_then(|v| v.as_str()).unwrap_or("backlog").to_string();
                                *counts.entry(status).or_insert(0u32) += 1;
                            }
                        }
                    }
                }
            }
            issues_data = serde_json::json!({
                "space": matched_space,
                "counts": counts,
            });
        }
    }

    // Git activity — recent commits on project branches
    let mut git_data = serde_json::json!(null);
    if include_git {
        let project_path = config::resolve_project_path(&req.project);
        if std::path::Path::new(&project_path).join(".git").exists() {
            let log_output = std::process::Command::new("git")
                .args(["log", "--oneline", "--all", "-20", "--format=%h %s (%ar)"])
                .current_dir(&project_path)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            let branch_output = std::process::Command::new("git")
                .args(["branch", "-a", "--format=%(refname:short) %(upstream:track)"])
                .current_dir(&project_path)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            let status_output = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&project_path)
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();

            git_data = serde_json::json!({
                "recent_commits": log_output.lines().take(10).collect::<Vec<_>>(),
                "branches": branch_output.lines().take(20).collect::<Vec<_>>(),
                "dirty_files": status_output.lines().count(),
            });
        }
    }

    // Capacity: work log entries for this project
    let log_path = config::capacity_root().join("work_log.json");
    let log = crate::state::persistence::read_json(&log_path);
    let entries = log.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let project_acu: f64 = entries.iter()
        .filter(|e| {
            e.get("space").and_then(|v| v.as_str())
                .map_or(false, |s| s.to_lowercase().contains(&project_lower))
        })
        .filter_map(|e| e.get("acu_spent").and_then(|v| v.as_f64()))
        .sum();

    // Project MCPs
    let mcps = app.state.get_project_mcps(&req.project).await;

    serde_json::json!({
        "project": req.project,
        "panes": project_panes,
        "pane_count": project_panes.len(),
        "issues": issues_data,
        "board": board_data,
        "git": git_data,
        "total_acu": (project_acu * 100.0).round() / 100.0,
        "mcps": mcps,
    }).to_string()
}

/// os_digest — Daily/weekly summary of team output
pub async fn digest(app: &App, req: DigestRequest) -> String {
    let period = req.period.as_deref().unwrap_or("today");
    let project_filter = req.project.as_deref().unwrap_or("");

    // Determine time window
    let now = Local::now();
    let start = match period {
        "yesterday" => (now - chrono::Duration::days(1)).format("%Y-%m-%dT00:00:00").to_string(),
        "week" => (now - chrono::Duration::days(7)).format("%Y-%m-%dT00:00:00").to_string(),
        "month" => (now - chrono::Duration::days(30)).format("%Y-%m-%dT00:00:00").to_string(),
        _ => now.format("%Y-%m-%dT00:00:00").to_string(), // today
    };
    let end = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    // Activity log analysis
    let state_snap = app.state.get_state_snapshot().await;
    let mut spawns = 0u32;
    let mut completions = 0u32;
    let mut kills = 0u32;
    let mut errors = 0u32;
    let mut projects_seen = std::collections::HashSet::new();
    let mut log_entries = Vec::new();

    for entry in &state_snap.activity_log {
        if entry.ts < start { continue; }
        if !project_filter.is_empty() && !entry.summary.to_lowercase().contains(&project_filter.to_lowercase()) {
            continue;
        }
        match entry.event.as_str() {
            "spawn" => spawns += 1,
            "complete" => completions += 1,
            "kill" => kills += 1,
            _ => {}
        }
        if entry.summary.to_lowercase().contains("error") { errors += 1; }
        // Extract project name from spawn entries
        if entry.event == "spawn" {
            if let Some(proj) = entry.summary.split(" on ").nth(1) {
                if let Some(name) = proj.split(':').next() {
                    projects_seen.insert(name.trim().to_string());
                }
            }
        }
        log_entries.push(entry.clone());
    }

    // Work log analysis
    let log_path = config::capacity_root().join("work_log.json");
    let work_log = crate::state::persistence::read_json(&log_path);
    let work_entries = work_log.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut total_acu = 0.0f64;
    let mut acu_by_role = std::collections::HashMap::<String, f64>::new();
    let mut acu_by_project = std::collections::HashMap::<String, f64>::new();
    let mut reviews_needed = 0u32;
    let mut issues_worked = std::collections::HashSet::new();

    for entry in &work_entries {
        let logged_at = entry.get("logged_at").and_then(|v| v.as_str()).unwrap_or("");
        if logged_at < start.as_str() { continue; }

        let space = entry.get("space").and_then(|v| v.as_str()).unwrap_or("");
        if !project_filter.is_empty() && !space.to_lowercase().contains(&project_filter.to_lowercase()) {
            continue;
        }

        let acu = entry.get("acu_spent").and_then(|v| v.as_f64()).unwrap_or(0.0);
        total_acu += acu;

        let role = entry.get("role").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        *acu_by_role.entry(role).or_default() += acu;

        if !space.is_empty() {
            *acu_by_project.entry(space.to_string()).or_default() += acu;
        }

        if entry.get("review_needed").and_then(|v| v.as_bool()).unwrap_or(false) {
            reviews_needed += 1;
        }

        let issue_id = entry.get("issue_id").and_then(|v| v.as_str()).unwrap_or("");
        if !issue_id.is_empty() && issue_id != "adhoc" {
            issues_worked.insert(issue_id.to_string());
        }
    }

    // Queue stats
    let q = queue::load_queue();
    let q_done = q.tasks.iter().filter(|t| {
        t.status == queue::QueueStatus::Done && t.completed_at.as_deref().unwrap_or("") >= start.as_str()
    }).count();
    let q_failed = q.tasks.iter().filter(|t| {
        t.status == queue::QueueStatus::Failed && t.completed_at.as_deref().unwrap_or("") >= start.as_str()
    }).count();
    let q_pending = q.tasks.iter().filter(|t| t.status == queue::QueueStatus::Pending).count();

    // Round ACU values
    let round_map = |m: &std::collections::HashMap<String, f64>| -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for (k, v) in m {
            result.insert(k.clone(), serde_json::json!((*v * 100.0).round() / 100.0));
        }
        serde_json::Value::Object(result)
    };

    // Build recommendations
    let mut recommendations = Vec::new();
    if q_pending > 5 {
        recommendations.push(format!("{} tasks queued — consider increasing max_parallel panes", q_pending));
    }
    if q_failed > 0 {
        recommendations.push(format!("{} tasks failed — review and retry or fix", q_failed));
    }
    if errors > 2 {
        recommendations.push(format!("{} errors in period — investigate recurring failures", errors));
    }
    if reviews_needed > 3 {
        recommendations.push(format!("{} items need review — review bottleneck risk", reviews_needed));
    }
    if completions == 0 && spawns > 0 {
        recommendations.push("Agents spawned but nothing completed — check if tasks are stuck".into());
    }

    serde_json::json!({
        "period": period,
        "window": { "start": start, "end": end },
        "activity": {
            "agents_spawned": spawns,
            "tasks_completed": completions,
            "agents_killed": kills,
            "errors_detected": errors,
        },
        "work": {
            "total_acu": (total_acu * 100.0).round() / 100.0,
            "by_role": round_map(&acu_by_role),
            "by_project": round_map(&acu_by_project),
            "issues_worked": issues_worked.len(),
            "reviews_pending": reviews_needed,
        },
        "queue": {
            "completed": q_done,
            "failed": q_failed,
            "still_pending": q_pending,
        },
        "projects_active": projects_seen.into_iter().collect::<Vec<_>>(),
        "recommendations": recommendations,
    }).to_string()
}

/// os_watch — Tail a pane's PTY output with error analysis
pub async fn watch(app: &App, req: WatchRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };
    let tail_lines = req.tail.unwrap_or(30);
    let analyze = req.analyze_errors.unwrap_or(true);

    let pd = app.state.get_pane(pane_num).await;
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    let pty = app.pty_lock();
    if !pty.has_agent(pane_num) {
        drop(pty);
        return serde_json::json!({
            "pane": pane_num,
            "theme": config::theme_name(pane_num),
            "status": pd.status,
            "project": pd.project,
            "task": pd.task,
            "pty_active": false,
            "output": format!("[No PTY] Pane {} is {}", pane_num, pd.status),
        }).to_string();
    }

    let screen = pty.screen_text(pane_num).unwrap_or_default();
    let output = pty.last_output(pane_num, tail_lines).unwrap_or_default();
    let running = pty.is_running(pane_num);
    let health = pty.check_health(pane_num, &markers);
    let line_count = pty.line_count(pane_num);
    drop(pty);

    // Use screen if available, fall back to output buffer
    let display = if !screen.trim().is_empty() { &screen } else { &output };

    // Error analysis
    let mut errors_found = Vec::new();
    let mut warnings_found = Vec::new();
    if analyze {
        let error_patterns = ["error", "Error", "ERROR", "panic", "PANIC", "failed", "FAILED", "fatal", "FATAL"];
        let warning_patterns = ["warning", "Warning", "WARN", "deprecated", "timeout"];

        for (line_num, line) in display.lines().enumerate() {
            for pat in &error_patterns {
                if line.contains(pat) && !line.contains("error_count") && !line.contains("no_error") {
                    errors_found.push(serde_json::json!({
                        "line": line_num + 1,
                        "text": truncate(line.trim(), 120),
                    }));
                    break;
                }
            }
            for pat in &warning_patterns {
                if line.contains(pat) {
                    warnings_found.push(serde_json::json!({
                        "line": line_num + 1,
                        "text": truncate(line.trim(), 120),
                    }));
                    break;
                }
            }
        }
    }

    // Runtime calculation — only for active panes
    let runtime_mins = if pd.status != "idle" {
        if let Some(started) = &pd.started_at {
            if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                let now = Local::now().naive_local();
                Some((now - start_dt).num_minutes())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Determine agent phase from output
    let phase = if health.done {
        "completed"
    } else if display.contains("Thinking") || display.contains("thinking") {
        "thinking"
    } else if display.contains("Writing") || display.contains("Editing") || display.contains("Creating") {
        "writing"
    } else if display.contains("Reading") || display.contains("Searching") {
        "reading"
    } else if display.contains("Running") || display.contains("testing") || display.contains("cargo") || display.contains("npm") {
        "running_commands"
    } else if running {
        "working"
    } else {
        "idle"
    };

    serde_json::json!({
        "pane": pane_num,
        "theme": config::theme_name(pane_num),
        "project": pd.project,
        "role": pd.role,
        "task": truncate(&pd.task, 80),
        "status": pd.status,
        "branch": pd.branch_name,
        "pty_running": running,
        "phase": phase,
        "runtime_mins": runtime_mins,
        "line_count": line_count,
        "done": health.done,
        "done_marker": health.done_marker,
        "exit_code": health.exit_code,
        "output": truncate(display, 4000),
        "errors": errors_found,
        "warnings": warnings_found,
        "error_count": errors_found.len(),
        "warning_count": warnings_found.len(),
    }).to_string()
}

// --- Helpers ---

/// Save agent output to file before killing PTY (prevents irreversible output loss)
fn save_agent_output(app: &App, pane_num: u8, reason: &str) -> Option<String> {
    let pty = app.pty_lock();
    if !pty.has_agent(pane_num) {
        return None;
    }
    let output = pty.last_output(pane_num, 200).unwrap_or_default();
    let screen = pty.screen_text(pane_num).unwrap_or_default();
    drop(pty);

    if output.is_empty() && screen.is_empty() {
        return None;
    }

    let dir = config::output_logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    let filename = format!("pane{}_{}.log", pane_num, Local::now().format("%Y%m%d_%H%M%S"));
    let path = dir.join(&filename);

    let content = format!(
        "=== AgentOS Output Log ===\nPane: {}\nReason: {}\nTimestamp: {}\n\n=== Screen ===\n{}\n\n=== Last 200 Lines ===\n{}\n",
        pane_num, reason, state::now(), screen, output
    );
    let _ = std::fs::write(&path, &content);
    Some(path.to_string_lossy().to_string())
}

/// Extract meaningful result from agent output (PR URL, commit hash, etc.)
fn extract_result(app: &App, pane_num: u8) -> String {
    let pty = app.pty_lock();
    let output = pty.last_output(pane_num, 50).unwrap_or_default();
    let screen = pty.screen_text(pane_num).unwrap_or_default();
    drop(pty);

    let text = if !screen.trim().is_empty() { &screen } else { &output };
    let mut results = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // PR URL
        if trimmed.contains("github.com") && trimmed.contains("/pull/") {
            for word in trimmed.split_whitespace() {
                if word.contains("github.com") && word.contains("/pull/") {
                    results.push(format!("PR: {}", word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != ':' && c != '.' && c != '-' && c != '_')));
                    break;
                }
            }
        }
        // Git commit hash
        if trimmed.starts_with('[') && trimmed.contains(']') {
            for word in trimmed.split_whitespace() {
                let clean = word.trim_matches(|c: char| !c.is_ascii_hexdigit());
                if clean.len() >= 7 && clean.len() <= 40 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                    results.push(format!("commit: {}", &clean[..7.min(clean.len())]));
                    break;
                }
            }
        }
    }

    if results.is_empty() {
        text.lines().rev()
            .find(|l| !l.trim().is_empty() && !l.contains('$') && !l.contains('%'))
            .map(|l| truncate(l.trim(), 200))
            .unwrap_or_else(|| "auto-completed".into())
    } else {
        results.join("; ")
    }
}

/// MCP tool: machine identity for one or all panes
pub fn machine_info_tool(req: &MachineInfoRequest) -> String {
    let pane = req.pane.as_ref().and_then(|p| config::resolve_pane(p));
    machine::machine_info(pane).to_string()
}

/// MCP tool: list all registered machines with network info
pub fn machine_list_tool() -> String {
    machine::machine_list().to_string()
}

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
    let window = (pane_num as u32 - 1) / 3 + 1;
    let pane = (pane_num as u32 - 1) % 3 + 1;
    let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
    let _ = crate::multi_agent::agent_register(&pane_id, project, task, &[]);
}

fn remove_from_agents_json(pane_num: u8) {
    let window = (pane_num as u32 - 1) / 3 + 1;
    let pane = (pane_num as u32 - 1) % 3 + 1;
    let pane_id = format!("{}:{}.{}", config::session_name(), window, pane);
    let _ = crate::multi_agent::agent_deregister(&pane_id);
}
