//! Pane lifecycle: spawn, kill, restart, reassign, assign, assign_adhoc, collect, complete.

use std::path::PathBuf;
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
use crate::tmux;
use super::super::types::*;
use super::helpers::*;

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

    // Pre-spawn cleanup: kill any stale processes owned by this pane
    cleanup_pane_resources(pane_num);

    // Micro-helpers: workspace setup + MCP selection
    let ws = prepare_workspace(&req.project, pane_num, &task);
    let _mcps = select_mcps(app, &ws.project_name, &ws.project_path, &task, &role).await;

    let project_path = ws.project_path;
    let project_name = ws.project_name;
    let mut spawn_cwd = ws.spawn_cwd;
    let ws_path = ws.ws_path;
    let ws_branch = ws.ws_branch;
    let ws_base = ws.ws_base;

    // Validate CWD exists — fall back to project_path to avoid posix_spawn ENOENT
    if !std::path::Path::new(&spawn_cwd).exists() {
        tracing::warn!("spawn_cwd does not exist: {}, falling back to project_path: {}", spawn_cwd, project_path);
        spawn_cwd = project_path.clone();
        // If project_path also doesn't exist, fail early with clear error
        if !std::path::Path::new(&spawn_cwd).exists() {
            return json_err(&format!("Neither workspace nor project path exists: {}", spawn_cwd));
        }
    }

    // Generate preamble and write as CLAUDE.md in workspace for auto-load
    let preamble = claude::generate_preamble(pane_num, theme, &project_name, &role, &task, &prompt);
    let _ = claude::write_preamble(pane_num, &preamble);
    let claude_md_path = format!("{}/CLAUDE.md", spawn_cwd);
    let _ = std::fs::write(&claude_md_path, &preamble);

    // Register machine identity
    let machine_id = machine::register(pane_num);

    // Environment variables for the agent
    let env_vars = vec![
        ("P".to_string(), pane_num.to_string()),
        ("DX_PANE".to_string(), pane_num.to_string()),
        ("DX_THEME".to_string(), theme.to_string()),
        ("DX_PROJECT".to_string(), project_name.clone()),
        ("DX_ROLE".to_string(), role.clone()),
        ("MACHINE_IP".to_string(), machine_id.ip.clone()),
        ("MACHINE_HOSTNAME".to_string(), machine_id.hostname.clone()),
        ("MACHINE_MAC".to_string(), machine_id.mac.clone()),
    ];

    let task_prompt = format!("{}\n\n{}", task, if prompt.is_empty() { "" } else { &prompt });
    let autonomous = req.autonomous.unwrap_or(true);

    // Spawn via tmux — creates a visible window the user can attach to
    let window_name = format!("dx-{}-{}", pane_num, config::theme_name(pane_num).to_lowercase());
    let tmux_result = tmux::spawn_agent(&window_name, &spawn_cwd, &task_prompt, &env_vars, autonomous);

    let (tmux_status, tmux_target) = match &tmux_result {
        Ok(agent) => ("tmux_spawned".to_string(), Some(agent.target.clone())),
        Err(e) => (format!("tmux_error: {}", e), None),
    };

    if tmux_result.is_err() {
        if let Some(ref ws) = ws_path {
            let _ = workspace::remove_worktree(&project_path, ws);
        }
        return format!("{{\"error\": \"Tmux spawn failed: {}\"}}", tmux_status);
    }

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
        tmux_target: tmux_target.clone(),
    };
    app.state.set_pane(pane_num, pane_state).await;
    app.state.event_bus.send(crate::state::events::StateEvent::PaneSpawned {
        pane: pane_num,
        project: project_name.clone(),
        role: role.clone(),
    });
    app.state.log_activity(
        pane_num,
        "spawn",
        &format!("Spawned {} on {}: {}", role, project_name, truncate(&task, 40)),
    ).await;

    update_agents_json(pane_num, &project_name, &task);

    // Auto-register agent with multi_agent coordination system
    let _ = crate::multi_agent::agent_register(
        &pane_id_str(pane_num),
        &project_name,
        &task,
        &[], // files will be claimed via lock_acquire as agent works
    );

    if let Some(ref branch) = ws_branch {
        let _ = crate::multi_agent::git_claim_branch(&pane_id_str(pane_num), branch, &project_name, &task);
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
        "tmux": tmux_status,
        "tmux_target": tmux_target,
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

    let pane_data = app.state.get_pane(pane_num).await;
    let ws_path = pane_data.workspace_path.clone();
    let project_path = pane_data.project_path.clone();

    let output_log = save_agent_output(app, pane_num, &reason);

    // Kill via tmux if we have a target, otherwise try PTY fallback
    let kill_status = if let Some(ref target) = pane_data.tmux_target {
        match tmux::kill_window(target) {
            Ok(()) => "tmux_killed",
            Err(_) => "tmux_no_window",
        }
    } else {
        // Fallback: try PTY kill for legacy agents
        let mut pty = app.pty_lock();
        match pty.kill(pane_num) {
            Ok(()) => "pty_killed",
            Err(_) => "no_process",
        }
    };

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

    if let Some(ref branch) = branch_name {
        let _ = crate::multi_agent::git_release_branch(&pane_id_str(pane_num), branch, &project_name);
    }

    // Deregister from coordination system + release all file locks
    let _ = crate::multi_agent::agent_deregister(&pane_id_str(pane_num));
    machine::deregister(pane_num);

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
    pane_state.tmux_target = None;
    app.state.set_pane(pane_num, pane_state).await;
    app.state.event_bus.send(crate::state::events::StateEvent::PaneKilled {
        pane: pane_num,
        reason: reason.clone(),
    });
    app.state.log_activity(pane_num, "kill", &format!("Killed: {}", reason)).await;

    remove_from_agents_json(pane_num);

    serde_json::json!({
        "status": "killed",
        "pane": pane_num,
        "reason": reason,
        "kill_method": kill_status,
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

    let _ = kill(app, KillRequest {
        pane: pane_num.to_string(),
        reason: Some("restart".into()),
    }).await;

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
        autonomous: None,
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
            .map(|n: &std::ffi::OsStr| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project.clone());
        pane_data.project_path = path;
    }
    if let Some(role) = &req.role {
        pane_data.role = role.clone();
    }
    if let Some(task) = &req.task {
        pane_data.task = task.clone();
    }

    if let Some(task) = &req.task {
        let msg = format!(
            "NEW TASK: {}\nRole: {}\nProject: {}\nPlease acknowledge and begin working on this new task.",
            task, pane_data.role, pane_data.project
        );
        // Send via tmux if available, otherwise PTY fallback
        if let Some(ref target) = pane_data.tmux_target {
            if let Err(e) = tmux::send_command(target, &msg) {
                tracing::warn!("Failed to send reassign via tmux to pane {}: {}", pane_num, e);
            }
        } else {
            let send_result = {
                let mut pty = app.pty_lock();
                pty.send_line(pane_num, &msg)
            };
            if let Err(e) = send_result {
                tracing::warn!("Failed to send reassign message to pane {}: {}", pane_num, e);
            }
        }
    }

    app.state.set_pane(pane_num, pane_data.clone()).await;
    app.state.log_activity(
        pane_num,
        "reassign",
        &format!("Reassigned: {}", truncate(req.task.as_deref().unwrap_or("config change"), 40)),
    ).await;

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

    let theme = config::theme_name(pane_num);
    let _ = tracker::update_issue(&req.space, &req.issue_id, &serde_json::json!({
        "status": "in_progress",
        "assignee": theme.to_lowercase(),
        "updated_at": state::now(),
    }));

    let _result = spawn(app, SpawnRequest {
        pane: pane_num.to_string(),
        project: project_path,
        role: Some(role.clone()),
        task: Some(task),
        prompt: Some(prompt),
        autonomous: None,
    }).await;

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
        autonomous: None,
    }).await
}

/// Execute os_collect logic — reads tmux output (or PTY fallback)
pub async fn collect(app: &App, req: CollectRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return json_err(&format!("Invalid pane: {}", req.pane)),
    };

    let pane_data = app.state.get_pane(pane_num).await;

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

    // Prefer tmux capture if we have a target
    if let Some(ref target) = pane_data.tmux_target {
        let t = target.clone();
        let output = tokio::task::spawn_blocking(move || tmux::capture_output(&t))
            .await.unwrap_or_default();
        let t2 = target.clone();
        let done = tokio::task::spawn_blocking(move || tmux::check_done(&t2))
            .await.unwrap_or(false);
        let t3 = target.clone();
        let error = tokio::task::spawn_blocking(move || tmux::check_error(&t3))
            .await.unwrap_or(None);

        let line_count = output.lines().count();
        let display_output = truncate(&output, 3000);

        if done && pane_data.status == "active" {
            app.state.update_pane_status(pane_num, "done").await;
        }

        return serde_json::json!({
            "pane": pane_num,
            "theme": pane_data.theme,
            "project": pane_data.project,
            "task": truncate(&pane_data.task, 60),
            "status": if done && pane_data.status == "active" { "done" } else { &pane_data.status },
            "branch": pane_data.branch_name,
            "tmux_target": target,
            "running": !done,
            "done": done,
            "error": error,
            "output": display_output,
            "line_count": line_count,
            "git": git_info,
        }).to_string();
    }

    // Fallback: try PTY
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();
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

    if let Some((output, screen, running, health, line_count)) = pty_info {
        let display_output = if !screen.trim().is_empty() {
            truncate(&screen, 3000)
        } else {
            truncate(&output, 3000)
        };

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
            "output": format!("[No agent] Pane {} - Status: {}", pane_num, pane_data.status),
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
    let summary = req.summary.clone().unwrap_or_else(|| extract_result(app, pane_num));

    // Micro-helper: calculate ACU spent
    let acu = pane_data.started_at.as_deref().map(calculate_acu).unwrap_or(0.0);

    if let (Some(issue_id), Some(space)) = (&pane_data.issue_id, &pane_data.space) {
        let _ = tracker::update_issue(space, issue_id, &serde_json::json!({
            "status": "done",
            "actual_acu": acu,
            "updated_at": state::now(),
        }));
    }

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

    // Micro-helpers: git finalization + feature-to-code bridge
    let mut git_info = serde_json::json!(null);
    if let (Some(ws), Some(branch)) = (&pane_data.workspace_path, &pane_data.branch_name) {
        if let (Some(issue_id), Some(space)) = (&pane_data.issue_id, &pane_data.space) {
            let base = pane_data.base_branch.as_deref().unwrap_or("main");
            let started = pane_data.started_at.as_deref().unwrap_or("");
            attach_code_to_issue(space, issue_id, ws, base, started);
        }
        let result = finalize_git(ws, branch, &pane_data.project_path, pane_num, &pane_data.task, &summary, acu);
        git_info = result.info;
    }

    let _output_log = save_agent_output(app, pane_num, "completed");

    // Save handoff context to KB for dependent tasks
    let result_text = extract_result(app, pane_num);
    if let Some(qt) = queue::task_for_pane(pane_num) {
        let pid = pane_id_str(pane_num);
        let handoff_content = format!(
            "Task: {}\nResult: {}\nSummary: {}\nBranch: {}\nPR: {}",
            qt.task,
            result_text,
            summary,
            pane_data.branch_name.as_deref().unwrap_or("none"),
            git_info.get("pr").and_then(|v| v.as_str()).unwrap_or("none"),
        );
        let _ = crate::multi_agent::kb_add(
            &pid, &pane_data.project, "agent_handoff",
            &qt.id, &handoff_content, &[],
        );
    }

    // Kill the agent process (tmux or PTY)
    if let Some(ref target) = pane_data.tmux_target {
        let _ = tmux::kill_window(target);
    } else {
        let mut pty = app.pty_lock();
        let _ = pty.kill(pane_num);
    }

    if let Some(ref branch) = pane_data.branch_name {
        let _ = crate::multi_agent::git_release_branch(&pane_id_str(pane_num), branch, &pane_data.project);
    }

    // Deregister from coordination system + release all file locks
    let _ = crate::multi_agent::agent_deregister(&pane_id_str(pane_num));

    remove_from_agents_json(pane_num);

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
    pane_data.tmux_target = None;
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

/// Resolve "claude" to an absolute path. Checks common locations + which.
pub fn resolve_claude_binary() -> String {
    // Check common locations first (fastest)
    let candidates = [
        "/opt/homebrew/bin/claude",
        "/usr/local/bin/claude",
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    // Fall back to `which claude`
    if let Ok(output) = std::process::Command::new("which").arg("claude").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    // Last resort — let PATH resolve it
    "claude".to_string()
}
