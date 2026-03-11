//! Queue & auto-cycle: queue_add, queue_decompose, queue_list, queue_done, auto_cycle, auto_config.

use chrono::{Local, NaiveDateTime};

use super::super::types::*;
use super::helpers::*;
use super::panes;
use crate::app::App;
use crate::config;
use crate::queue;
use crate::tracker;

/// Add a task to the queue
pub async fn queue_add(_app: &App, req: QueueAddRequest) -> String {
    let role = req.role.unwrap_or_else(|| "developer".into());
    let prompt = req.prompt.unwrap_or_else(|| req.task.clone());
    let priority = req.priority.unwrap_or(3);
    let depends_on = req.depends_on.unwrap_or_default();

    match queue::add_task(
        &req.project,
        &role,
        &req.task,
        &prompt,
        priority,
        depends_on,
    ) {
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
            })
            .to_string()
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
            || (trimmed.len() > 2
                && trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.'));

        if is_new_item {
            if !current.trim().is_empty() {
                subtasks.push((current.trim().to_string(), is_parallel));
            }
            let text = trimmed
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c == '.' || c == '-' || c == '*'
                })
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

        match queue::add_task(
            &req.project,
            &role,
            task_text,
            task_text,
            priority,
            deps.clone(),
        ) {
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
    })
    .to_string()
}

/// List queue tasks
pub async fn queue_list(_app: &App, req: QueueListRequest) -> String {
    let q = queue::load_queue();

    let filtered: Vec<&queue::QueueTask> = q
        .tasks
        .iter()
        .filter(|t| {
            if let Some(status) = &req.status {
                let s = format!("{:?}", t.status).to_lowercase();
                s == status.to_lowercase()
            } else {
                true
            }
        })
        .collect();

    let items: Vec<serde_json::Value> = filtered
        .iter()
        .map(|t| {
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
        })
        .collect();

    let pending = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Pending)
        .count();
    let running = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Running)
        .count();
    let done = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Done)
        .count();

    serde_json::json!({
        "tasks": items,
        "summary": { "pending": pending, "running": running, "done": done, "total": q.tasks.len() },
    })
    .to_string()
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

/// Cancel a specific queue task (marks failed, cascades to dependents)
pub async fn queue_cancel(_app: &App, req: QueueCancelRequest) -> String {
    let reason = req.reason.unwrap_or_else(|| "Manually cancelled".into());
    match queue::mark_failed(&req.task_id, &reason) {
        Ok(()) => serde_json::json!({
            "status": "cancelled",
            "task_id": req.task_id,
            "reason": reason,
        })
        .to_string(),
        Err(e) => json_err(&format!("Failed to cancel: {}", e)),
    }
}

/// Retry a failed queue task (resets to pending, increments retry count)
pub async fn queue_retry(_app: &App, req: QueueRetryRequest) -> String {
    match queue::requeue_failed(&req.task_id) {
        Ok(true) => serde_json::json!({
            "status": "requeued",
            "task_id": req.task_id,
        })
        .to_string(),
        Ok(false) => json_err("Task not eligible for retry (not failed or max retries reached)"),
        Err(e) => json_err(&format!("Failed to retry: {}", e)),
    }
}

/// Clear completed/failed tasks from the queue
pub async fn queue_clear(_app: &App, req: QueueClearRequest) -> String {
    let status = req.status.as_deref().unwrap_or("done");
    match queue::clear_tasks(status) {
        Ok(removed) => serde_json::json!({
            "status": "cleared",
            "removed": removed,
            "filter": status,
        })
        .to_string(),
        Err(e) => json_err(&format!("Failed to clear: {}", e)),
    }
}

/// Auto-cycle: scan all panes, complete finished agents, spawn next tasks
pub async fn auto_cycle(app: &App) -> String {
    // Process-level lock
    let lock_path = config::dx_root().join("auto_cycle.lock");
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
    // SAFETY: flock() is a POSIX syscall. fd is valid (from lock_file.as_raw_fd() on an open File).
    // LOCK_EX|LOCK_NB = exclusive non-blocking. Lock released when lock_file is dropped.
    let lock_result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if lock_result != 0 {
        return "lock_held_by_another_instance".into();
    }
    let _ = std::fs::write(&lock_path, format!("{}", std::process::id()));

    let cfg = queue::load_auto_config();
    let mut actions = Vec::new();
    let mut occupied_panes = Vec::new();

    // Phase 0: Process factory inbox — convert pending requests into pipelines
    let inbox_actions = crate::factory::process_inbox();
    actions.extend(inbox_actions);

    // Phase 1: Collect status of all running panes
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    for i in 1..=config::pane_count() {
        let pd = app.state.get_pane(i).await;
        if pd.status != "active" {
            continue;
        }
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

                let _result = panes::complete(
                    app,
                    CompleteRequest {
                        pane: i.to_string(),
                        summary: Some("Auto-completed by cycle".into()),
                    },
                )
                .await;

                if let Some(qt) = queue::task_for_pane(i) {
                    let _ = queue::mark_done(&qt.id, &result);

                    // Quality gate: if this task belongs to a pipeline dev stage, run checks
                    if let Some(ref pid) = qt.pipeline_id {
                        if qt.role == "developer" {
                            match crate::factory::run_gate(pid) {
                                Ok(gate) => {
                                    if !gate.passed {
                                        // Gate failed — try auto-retry if retries remain
                                        if qt.retry_count < qt.max_retries {
                                            // Circuit breaker: reset dev task to pending for re-run
                                            // First mark as failed (so retry_stage can reset it)
                                            let _ =
                                                queue::mark_failed(&qt.id, "Quality gate failed");
                                            let retried =
                                                crate::factory::retry_stage(pid, "dev").is_ok();
                                            crate::factory::log_pipeline_event(
                                                pid,
                                                "gate_retry",
                                                &format!(
                                                    "Auto-retrying dev (attempt {}/{})",
                                                    qt.retry_count + 1,
                                                    qt.max_retries
                                                ),
                                            );
                                            actions.push(serde_json::json!({
                                                "action": "quality_gate_retry",
                                                "pipeline_id": pid,
                                                "retry_count": qt.retry_count + 1,
                                                "max_retries": qt.max_retries,
                                                "retried": retried,
                                                "build": gate.build.as_ref().map(|c| c.success),
                                                "test": gate.test.as_ref().map(|c| c.success),
                                            }));
                                        } else {
                                            // Max retries exhausted — fail dependents
                                            let q = queue::load_queue();
                                            let dep_ids: Vec<String> = q
                                                .tasks
                                                .iter()
                                                .filter(|t| {
                                                    t.pipeline_id.as_deref() == Some(pid.as_str())
                                                        && t.depends_on.contains(&qt.id)
                                                        && (t.status == queue::QueueStatus::Pending
                                                            || t.status
                                                                == queue::QueueStatus::Blocked)
                                                })
                                                .map(|t| t.id.clone())
                                                .collect();
                                            for dep_id in &dep_ids {
                                                let _ = queue::mark_failed(
                                                    dep_id,
                                                    &format!(
                                                        "Quality gate failed after {} retries",
                                                        qt.max_retries
                                                    ),
                                                );
                                            }
                                            crate::factory::log_pipeline_event(pid, "gate_failed",
                                                &format!("Gate failed after {} retries, blocking {} dependents", qt.max_retries, dep_ids.len()));
                                            actions.push(serde_json::json!({
                                                "action": "quality_gate_block",
                                                "pipeline_id": pid,
                                                "blocked_tasks": dep_ids.len(),
                                                "retries_exhausted": true,
                                                "build": gate.build.as_ref().map(|c| c.success),
                                                "test": gate.test.as_ref().map(|c| c.success),
                                            }));
                                        }
                                    } else {
                                        actions.push(serde_json::json!({
                                            "action": "quality_gate",
                                            "pipeline_id": pid,
                                            "passed": true,
                                            "build": gate.build.as_ref().map(|c| c.success),
                                            "test": gate.test.as_ref().map(|c| c.success),
                                        }));
                                    }
                                }
                                Err(e) => {
                                    actions.push(serde_json::json!({
                                        "action": "quality_gate_error",
                                        "pipeline_id": pid,
                                        "error": e.to_string(),
                                    }));
                                }
                            }
                        }
                    }

                    if let (Some(issue_id), Some(space)) = (&qt.issue_id, &qt.space) {
                        let _ = tracker::issue_update_full(
                            space, issue_id, "done", "", "", "", "", "", "", "", 0.0, 0.0, "", "",
                        );
                        let _ = tracker::issue_comment(
                            space,
                            issue_id,
                            &format!("Auto-completed by DX Terminal queue task {}", qt.id),
                            "dx-terminal",
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
                let _output_log = save_agent_output(
                    app,
                    i,
                    &format!("error: {}", h.error.as_deref().unwrap_or("unknown")),
                );

                if let Some(qt) = queue::task_for_pane(i) {
                    let _ =
                        queue::mark_failed(&qt.id, h.error.as_deref().unwrap_or("unknown error"));
                    if let (Some(issue_id), Some(space)) = (&qt.issue_id, &qt.space) {
                        let _ = tracker::issue_update_full(
                            space, issue_id, "blocked", "", "", "", "", "", "", "", 0.0, 0.0, "",
                            "",
                        );
                        let _ = tracker::issue_comment(
                            space,
                            issue_id,
                            &format!(
                                "Queue task {} failed: {}",
                                qt.id,
                                h.error.as_deref().unwrap_or("unknown")
                            ),
                            "dx-terminal",
                        );
                    }
                }
                let _ = panes::kill(
                    app,
                    KillRequest {
                        pane: i.to_string(),
                        reason: Some(format!("error: {}", h.error.unwrap_or_default())),
                    },
                )
                .await;
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
        if !occupied_panes.contains(&i) {
            continue;
        }
        let pd = app.state.get_pane(i).await;
        if pd.status != "active" {
            continue;
        }
        if let Some(started) = &pd.started_at {
            if let Ok(start_dt) = NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S") {
                let now = Local::now().naive_local();
                let mins = (now - start_dt).num_minutes();
                if mins > (stuck_threshold * 10) as i64 {
                    let is_done = {
                        let pty = app.pty_lock();
                        if pty.has_agent(i) {
                            pty.check_health(i, &markers).done
                        } else {
                            true
                        }
                    };
                    if !is_done {
                        let _output_log =
                            save_agent_output(app, i, &format!("stuck: {} minutes", mins));
                        if let Some(qt) = queue::task_for_pane(i) {
                            let _ =
                                queue::mark_failed(&qt.id, &format!("stuck for {} minutes", mins));
                        }
                        let _ = panes::kill(
                            app,
                            KillRequest {
                                pane: i.to_string(),
                                reason: Some(format!("stuck: {} minutes", mins)),
                            },
                        )
                        .await;
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
        let retryable: Vec<String> = q
            .tasks
            .iter()
            .filter(|t| t.status == queue::QueueStatus::Failed && t.retry_count < t.max_retries)
            .map(|t| t.id.clone())
            .collect();
        for task_id in retryable {
            if let Ok(true) = queue::requeue_failed(&task_id) {
                actions.push(serde_json::json!({ "action": "auto_retry", "task_id": task_id }));
            }
        }
    }

    // Phase 1.8: Check tmux-based tasks for completion
    {
        let q = queue::load_queue();
        let tmux_running: Vec<(String, String)> = q
            .tasks
            .iter()
            .filter(|t| t.status == queue::QueueStatus::Running && t.tmux_target.is_some())
            .map(|t| (t.id.clone(), t.tmux_target.clone().unwrap()))
            .collect();

        for (task_id, target) in tmux_running {
            if crate::tmux::check_done(&target) {
                // Agent finished — capture output as result
                let output = crate::tmux::capture_output(&target);
                let result_text = output
                    .lines()
                    .rev()
                    .find(|l: &&str| !l.trim().is_empty() && !l.contains('$') && !l.contains('%'))
                    .map(|l: &str| truncate(l.trim(), 200))
                    .unwrap_or_else(|| "tmux-completed".into());

                let _ = queue::mark_done(&task_id, &result_text);

                // Run quality gate for dev stage of pipeline
                if let Some(qt) = queue::task_by_id(&task_id) {
                    if let Some(ref pid) = qt.pipeline_id {
                        if qt.role == "developer" {
                            if let Ok(gate) = crate::factory::run_gate(pid) {
                                if !gate.passed {
                                    if qt.retry_count < qt.max_retries {
                                        // Circuit breaker: mark failed then retry stage
                                        let _ = queue::mark_failed(&task_id, "Quality gate failed");
                                        let _ = crate::factory::retry_stage(pid, "dev");
                                        crate::factory::log_pipeline_event(
                                            pid,
                                            "gate_retry",
                                            &format!(
                                                "Auto-retrying dev via tmux (attempt {}/{})",
                                                qt.retry_count + 1,
                                                qt.max_retries
                                            ),
                                        );
                                    } else {
                                        // Max retries exhausted — fail dependents
                                        let q2 = queue::load_queue();
                                        let dep_ids: Vec<String> = q2
                                            .tasks
                                            .iter()
                                            .filter(|t| {
                                                t.pipeline_id.as_deref() == Some(pid.as_str())
                                                    && t.depends_on.contains(&task_id)
                                                    && (t.status == queue::QueueStatus::Pending
                                                        || t.status == queue::QueueStatus::Blocked)
                                            })
                                            .map(|t| t.id.clone())
                                            .collect();
                                        for dep_id in &dep_ids {
                                            let _ = queue::mark_failed(
                                                dep_id,
                                                &format!(
                                                    "Quality gate failed after {} retries",
                                                    qt.max_retries
                                                ),
                                            );
                                        }
                                        crate::factory::log_pipeline_event(pid, "gate_failed",
                                            &format!("Gate failed after {} retries, blocking {} dependents", qt.max_retries, dep_ids.len()));
                                    }
                                }
                            }
                        }
                    }
                }

                // Clean up tmux window
                let _ = crate::tmux::kill_window(&target);
                actions.push(serde_json::json!({
                    "action": "tmux_complete",
                    "task_id": task_id,
                    "tmux_target": target,
                }));
            } else if let Some(error) = crate::tmux::check_error(&target) {
                let _ = queue::mark_failed(&task_id, &format!("tmux error: {}", error));
                let _ = crate::tmux::kill_window(&target);
                actions.push(serde_json::json!({
                    "action": "tmux_error",
                    "task_id": task_id,
                    "error": error,
                }));
            }
        }
    }

    // Phase 2: Spawn next tasks — use tmux for pipeline tasks, PTY for standalone
    if cfg.auto_assign {
        loop {
            let next_task = queue::next_task();
            let task = match next_task {
                Some(t) => t,
                None => break,
            };

            // Skip tasks from paused pipelines
            if let Some(ref pid) = task.pipeline_id {
                if crate::factory::is_pipeline_paused(pid) {
                    continue;
                }
            }

            // Build enriched prompt with predecessor results
            let mut prompt = task.prompt.clone();
            if !task.depends_on.is_empty() {
                let mut handoff_parts = Vec::new();
                for dep_id in &task.depends_on {
                    if let Some(dep_task) = queue::task_by_id(dep_id) {
                        let result = dep_task.result.as_deref().unwrap_or("completed");
                        handoff_parts.push(format!("- {} ({}): {}", dep_task.task, dep_id, result));
                    }
                    let kb = crate::multi_agent::kb_search(
                        dep_id,
                        Some(&task.project),
                        Some("agent_handoff"),
                    );
                    if let Some(entries) = kb.get("entries").and_then(|v| v.as_array()) {
                        for entry in entries {
                            if let Some(content) = entry.get("content").and_then(|v| v.as_str()) {
                                handoff_parts.push(format!("  KB: {}", content));
                            }
                        }
                    }
                }
                if !handoff_parts.is_empty() {
                    prompt = format!(
                        "{}\n\n## Predecessor Results\n{}",
                        prompt,
                        handoff_parts.join("\n")
                    );
                }
            }

            // Pipeline-specific context: gate results + coordination
            let pane_for_coord = 0u8; // tmux doesn't use pane numbers
            if let Some(ref pid) = task.pipeline_id {
                if let Some(gate) = crate::factory::get_gate_result(pid) {
                    let mut gate_lines = vec![format!(
                        "Quality gate: {}",
                        if gate.passed { "PASSED" } else { "FAILED" }
                    )];
                    if let Some(ref b) = gate.build {
                        gate_lines.push(format!(
                            "  Build ({}): {}",
                            b.command,
                            if b.success { "PASS" } else { "FAIL" }
                        ));
                    }
                    if let Some(ref t) = gate.test {
                        gate_lines.push(format!(
                            "  Test ({}): {}",
                            t.command,
                            if t.success { "PASS" } else { "FAIL" }
                        ));
                        if !t.success {
                            gate_lines.push(format!(
                                "  Test output: {}",
                                &t.output.chars().take(500).collect::<String>()
                            ));
                        }
                    }
                    if let Some(ref l) = gate.lint {
                        gate_lines.push(format!(
                            "  Lint ({}): {}",
                            l.command,
                            if l.success { "PASS" } else { "WARN" }
                        ));
                    }
                    prompt = format!(
                        "{}\n\n## Quality Gate Results\n{}",
                        prompt,
                        gate_lines.join("\n")
                    );
                }
                let coord = crate::factory::coordination_context(pid, pane_for_coord, &task.role);
                if !coord.is_empty() {
                    prompt = format!("{}\n\n{}", prompt, coord);
                }
            }

            // Spawn via tmux (pipeline tasks) or PTY (standalone)
            if task.pipeline_id.is_some() {
                // TMUX MODE: visible, autonomous
                let project_path = crate::config::resolve_project_path(&task.project);
                let window_name = format!(
                    "{}-{}",
                    task.role,
                    task.id
                        .chars()
                        .rev()
                        .take(6)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                );

                match crate::tmux::spawn_agent(&window_name, &project_path, &prompt, &[], true) {
                    Ok(agent) => {
                        let _ = queue::mark_running(&task.id, 0); // 0 = tmux mode
                        let _ = queue::set_tmux_target(&task.id, &agent.target);
                        actions.push(serde_json::json!({
                            "action": "tmux_spawn",
                            "tmux_target": agent.target,
                            "task_id": task.id,
                            "project": task.project,
                            "role": task.role,
                            "task": truncate(&task.task, 40),
                        }));
                    }
                    Err(e) => {
                        let err_msg = format!("tmux spawn failed: {}", e);
                        tracing::error!("Tmux spawn failed for {}: {}", task.id, err_msg);
                        let _ = queue::mark_failed(&task.id, &err_msg);
                        actions.push(serde_json::json!({
                            "action": "tmux_spawn_error",
                            "task_id": task.id,
                            "error": err_msg,
                        }));
                    }
                }
            } else {
                // PTY MODE: internal (legacy)
                let free_pane = queue::find_free_pane(&cfg, &occupied_panes);
                match free_pane {
                    Some(pane) => {
                        let _ = queue::mark_running(&task.id, pane);
                        occupied_panes.push(pane);

                        let _result = panes::spawn(
                            app,
                            SpawnRequest {
                                pane: pane.to_string(),
                                project: task.project.clone(),
                                role: Some(task.role.clone()),
                                task: Some(task.task.clone()),
                                prompt: Some(prompt),
                                autonomous: None,
                            },
                        )
                        .await;

                        actions.push(serde_json::json!({
                            "action": "auto_spawn",
                            "pane": pane,
                            "task_id": task.id,
                            "project": task.project,
                            "task": truncate(&task.task, 40),
                        }));
                    }
                    None => break, // No free panes
                }
            }
        }
    }

    // Summary
    let q = queue::load_queue();
    let pending = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Pending)
        .count();
    let running = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Running)
        .count();

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
    })
    .to_string()
}

/// Update auto-cycle config
pub async fn auto_config(_app: &App, req: AutoConfigRequest) -> String {
    let mut cfg = queue::load_auto_config();
    if let Some(mp) = req.max_parallel {
        cfg.max_parallel = mp.clamp(1, 9);
    }
    if let Some(rp) = req.reserved_panes {
        cfg.reserved_panes = rp;
    }
    if let Some(ac) = req.auto_complete {
        cfg.auto_complete = ac;
    }
    if let Some(aa) = req.auto_assign {
        cfg.auto_assign = aa;
    }
    if let Some(ci) = req.cycle_interval_secs {
        cfg.cycle_interval_secs = ci;
    }

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
        })
        .to_string(),
        Err(e) => json_err(&format!("Failed to save config: {}", e)),
    }
}
