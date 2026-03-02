//! Queue & auto-cycle: queue_add, queue_decompose, queue_list, queue_done, auto_cycle, auto_config.

use chrono::{Local, NaiveDateTime};

use crate::app::App;
use crate::config;
use crate::tracker;
use crate::queue;
use super::super::types::*;
use super::helpers::*;
use super::panes;

/// Add a task to the queue
pub async fn queue_add(_app: &App, req: QueueAddRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let prompt = req.prompt.unwrap_or_else(|| req.task.clone());
    let priority = req.priority.unwrap_or(3);
    let depends_on = req.depends_on.unwrap_or_default();

    match queue::add_task(&req.project, &role, &req.task, &prompt, priority, depends_on) {
        Ok(task) => {
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

    let mut subtasks: Vec<(String, bool)> = Vec::new();
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

        let is_new_item = trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || (trimmed.len() > 2 && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.'));

        if is_new_item {
            if !current.trim().is_empty() {
                subtasks.push((current.trim().to_string(), is_parallel));
            }
            let text = trimmed
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '*')
                .trim();
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

    subtasks.truncate(max);

    if subtasks.is_empty() {
        return json_err("No sub-tasks found. Use numbered steps (1. 2. 3.) or bullets (- *).");
    }

    let mut created_ids: Vec<String> = Vec::new();
    let mut task_infos: Vec<serde_json::Value> = Vec::new();

    for (i, (task_text, parallel)) in subtasks.iter().enumerate() {
        let deps = if *parallel || i == 0 {
            vec![]
        } else {
            vec![created_ids[i - 1].clone()]
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
    // Process-level lock
    let lock_path = config::agentos_root().join("auto_cycle.lock");
    let lock_file = match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(_) => return "lock_error".into(),
    };
    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    let lock_result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if lock_result != 0 {
        return "lock_held_by_another_instance".into();
    }
    let _ = std::fs::write(&lock_path, format!("{}", std::process::id()));

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
                let result = extract_result(app, i);

                let _result = panes::complete(app, CompleteRequest {
                    pane: i.to_string(),
                    summary: Some("Auto-completed by cycle".into()),
                }).await;

                if let Some(qt) = queue::task_for_pane(i) {
                    let _ = queue::mark_done(&qt.id, &result);

                    if let (Some(issue_id), Some(space)) = (&qt.issue_id, &qt.space) {
                        let _ = tracker::issue_update_full(
                            space, issue_id, "done", "", "", "", "", "",
                            "", "", 0.0, 0.0, "", "",
                        );
                        let _ = tracker::issue_comment(
                            space, issue_id,
                            &format!("Auto-completed by AgentOS queue task {}", qt.id),
                            "agentos",
                        );
                        // Micro-helper: check if all siblings done → close parent
                        check_feature_closure(space, issue_id);
                    }
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
                    if let (Some(issue_id), Some(space)) = (&qt.issue_id, &qt.space) {
                        let _ = tracker::issue_update_full(
                            space, issue_id, "blocked", "", "", "", "", "",
                            "", "", 0.0, 0.0, "", "",
                        );
                        let _ = tracker::issue_comment(
                            space, issue_id,
                            &format!("Queue task {} failed: {}", qt.id, h.error.as_deref().unwrap_or("unknown")),
                            "agentos",
                        );
                    }
                }
                let _ = panes::kill(app, KillRequest {
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

    // Phase 1.5: Kill stuck agents
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
                        let _ = panes::kill(app, KillRequest {
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

    // Phase 1.7: Auto-retry failed tasks
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
                    let _ = queue::mark_running(&task.id, pane);
                    occupied_panes.push(pane);

                    let mut prompt = task.prompt.clone();
                    if !task.depends_on.is_empty() {
                        let mut handoff_parts = Vec::new();
                        for dep_id in &task.depends_on {
                            if let Some(dep_task) = queue::task_by_id(dep_id) {
                                let result = dep_task.result.as_deref().unwrap_or("completed");
                                handoff_parts.push(format!("- {} ({}): {}", dep_task.task, dep_id, result));
                            }
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

                    let _result = panes::spawn(app, SpawnRequest {
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
