//! Monitoring tools: status, dashboard, logs, health, monitor, watch, digest, project_status.

use chrono::{Local, NaiveDateTime};

use crate::app::App;
use crate::capacity;
use crate::config;
use crate::state;
use crate::tracker;

use super::super::types::*;
use super::helpers::truncate;
use crate::queue;

/// Execute os_status logic
pub async fn status(app: &App) -> String {
    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let mut panes = Vec::new();
    for (i, pd) in &pane_states {
        // Check liveness via tmux if available, otherwise PTY
        let (is_running, line_count) = if let Some(ref target) = pd.tmux_target {
            let output = crate::tmux::capture_output(target);
            let done = crate::tmux::check_done(target);
            (!done, output.lines().count())
        } else {
            let pty = app.pty_lock();
            (pty.is_running(*i), pty.line_count(*i))
        };

        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "theme_color": config::theme_fg(*i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "role_full": pd.role,
            "task": truncate(&pd.task, 40),
            "acu": pd.acu_spent,
            "status": pd.status,
            "issue_id": pd.issue_id,
            "space": pd.space,
            "branch": pd.branch_name,
            "workspace": pd.workspace_path,
            "started_at": pd.started_at,
            "tmux_target": pd.tmux_target,
            "running": is_running,
            "line_count": line_count,
        }));
    }

    let active = panes.iter().filter(|p| p["status"] == "active").count();
    let idle = panes
        .iter()
        .filter(|p| {
            let s = p["status"].as_str().unwrap_or("");
            s == "idle" || s.is_empty()
        })
        .count();
    let running = panes
        .iter()
        .filter(|p| p["running"].as_bool().unwrap_or(false))
        .count();

    serde_json::json!({
        "panes": panes,
        "summary": {"active": active, "idle": idle, "total": config::pane_count(), "running": running}
    }).to_string()
}

/// Execute os_dashboard logic
pub async fn dashboard(app: &App, req: DashboardRequest) -> String {
    let cap = capacity::load_capacity();
    let board = tracker::load_board_summary();

    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }
    let state_snap = app.state.get_state_snapshot().await;
    let log: Vec<_> = state_snap.activity_log.iter().take(8).cloned().collect();

    let mut panes = Vec::new();
    for (i, pd) in &pane_states {
        let is_running = if let Some(ref target) = pd.tmux_target {
            !crate::tmux::check_done(target)
        } else {
            let pty = app.pty_lock();
            pty.is_running(*i)
        };
        panes.push(serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "project": pd.project,
            "task": truncate(&pd.task, 30),
            "role": config::role_short(&pd.role),
            "status": pd.status,
            "running": is_running,
        }));
    }

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
        })
        .to_string();
    }

    let acu_pct = if cap.acu_total > 0.0 {
        (cap.acu_used / cap.acu_total * 100.0) as i32
    } else {
        0
    };
    let rev_pct = if cap.reviews_total > 0 {
        (cap.reviews_used as f64 / cap.reviews_total as f64 * 100.0) as i32
    } else {
        0
    };
    let bn = if rev_pct > 80 {
        "REVIEW"
    } else if acu_pct > 90 {
        "COMPUTE"
    } else {
        "BALANCED"
    };

    let now_str = state::now();
    let display_ts = now_str.get(..16).unwrap_or(&now_str);
    let mut lines = vec![
        format!("DX Terminal Dashboard — {}", display_ts),
        format!(
            "ACU: {}/{} ({}%)  Reviews: {}/{}  Bottleneck: {}",
            cap.acu_used, cap.acu_total, acu_pct, cap.reviews_used, cap.reviews_total, bn
        ),
        String::new(),
        " #  Theme   Project        Task                          Role  Status  Run".into(),
        " -  ------  -------------- ------------------------------ ----  ------  ---".into(),
    ];
    for p in &panes {
        lines.push(format!(
            " {}  {:<7} {:<14} {:<30} {:<5} {:<7} {}",
            p["pane"],
            p["theme"].as_str().unwrap_or(""),
            p["project"].as_str().unwrap_or("--"),
            p["task"].as_str().unwrap_or("--"),
            p["role"].as_str().unwrap_or("--"),
            p["status"].as_str().unwrap_or("idle"),
            if p["running"].as_bool().unwrap_or(false) {
                "Y"
            } else {
                "-"
            },
        ));
    }

    lines.push(String::new());
    let board_str: Vec<String> = board.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
    lines.push(format!("Board: {}", board_str.join("  ")));

    if !log.is_empty() {
        lines.push(String::new());
        lines.push("Recent:".into());
        for entry in log.iter().take(5) {
            let ts = entry.ts.get(11..16).unwrap_or(&entry.ts);
            lines.push(format!(
                "  {} P{} {}",
                ts,
                entry.pane,
                truncate(&entry.summary, 50)
            ));
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

/// Execute os_health logic — checks tmux or PTY health
pub async fn health(app: &App) -> String {
    let state = app.state.get_state_snapshot().await;
    let stuck_mins = state.config.stuck_threshold_minutes;
    let markers = state.config.completion_markers.clone();

    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let mut results = Vec::new();
    for (i, pd) in &pane_states {
        // Check via tmux if target exists
        if let Some(ref target) = pd.tmux_target {
            let done = crate::tmux::check_done(target);
            let error = crate::tmux::check_error(target);
            let output = crate::tmux::capture_output(target);
            let line_count = output.lines().count();

            let mut health_status = if error.is_some() {
                "error"
            } else if done {
                "done"
            } else {
                "ok"
            };

            if pd.status == "active" && !done && error.is_none() {
                if let Some(started) = &pd.started_at {
                    if let Ok(start_dt) =
                        NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S")
                    {
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
                "theme_color": config::theme_fg(*i),
                "status": pd.status,
                "health": health_status,
                "running": !done,
                "has_output": !output.trim().is_empty(),
                "error": error,
                "tmux_target": target,
                "line_count": line_count,
            }));
        } else {
            // PTY fallback or no agent
            let pty = app.pty_lock();
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
                        if let Ok(start_dt) =
                            NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S")
                        {
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
                    "theme_color": config::theme_fg(*i),
                    "status": pd.status,
                    "health": health_status,
                    "running": health.running,
                    "has_output": health.has_output,
                    "error": health.error,
                    "exit_code": health.exit_code,
                    "done_marker": health.done_marker,
                    "line_count": pty.line_count(*i),
                }));
            } else {
                let health_status = match pd.status.as_str() {
                    "idle" | "" => "idle",
                    "active" => "no_agent",
                    "done" => "done",
                    "error" => "error",
                    _ => "unknown",
                };

                results.push(serde_json::json!({
                    "pane": *i,
                    "theme": config::theme_name(*i),
                    "theme_color": config::theme_fg(*i),
                    "status": pd.status,
                    "health": health_status,
                    "running": false,
                    "has_output": false,
                    "error": serde_json::Value::Null,
                    "line_count": 0,
                }));
            }
        }
    }

    let active = results.iter().filter(|r| r["status"] == "active").count();
    let stuck = results.iter().filter(|r| r["health"] == "stuck").count();
    let errors = results.iter().filter(|r| r["health"] == "error").count();
    let running = results
        .iter()
        .filter(|r| r["running"].as_bool().unwrap_or(false))
        .count();

    serde_json::json!({
        "panes": results,
        "summary": {
            "active": active,
            "stuck": stuck,
            "errors": errors,
            "idle": config::pane_count() as usize - active,
            "running": running,
        }
    })
    .to_string()
}

/// os_monitor — Single-call "what's happening right now" overview
pub async fn monitor(app: &App, req: MonitorRequest) -> String {
    let include_output = req.include_output.unwrap_or(false);
    let state_snap = app.state.get_state_snapshot().await;
    let markers = state_snap.config.completion_markers.clone();

    let mut pane_states = Vec::new();
    for i in 1..=config::pane_count() {
        pane_states.push((i, app.state.get_pane(i).await));
    }

    let mut panes = Vec::new();
    let mut alerts = Vec::new();
    let mut active_count = 0u32;
    let mut idle_count = 0u32;
    let mut done_count = 0u32;
    let mut error_count = 0u32;
    let mut stuck_count = 0u32;

    for (i, pd) in &pane_states {
        let mut health_status = match pd.status.as_str() {
            "active" => {
                active_count += 1;
                "active"
            }
            "done" => {
                done_count += 1;
                "done"
            }
            "error" => {
                error_count += 1;
                "error"
            }
            _ => {
                idle_count += 1;
                "idle"
            }
        };

        let mut error_msg: Option<String> = None;
        let mut done_marker: Option<String> = None;
        let mut output_snippet = String::new();
        let mut is_running = false;
        let mut line_count = 0usize;

        // Check via tmux if target exists, otherwise PTY fallback
        if let Some(ref target) = pd.tmux_target {
            let tmux_done = crate::tmux::check_done(target);
            let tmux_error = crate::tmux::check_error(target);
            let tmux_output = crate::tmux::capture_output(target);
            line_count = tmux_output.lines().count();
            is_running = !tmux_done;

            if let Some(err) = tmux_error {
                health_status = "error";
                error_msg = Some(err.clone());
                if pd.status == "active" {
                    error_count += 1;
                    active_count = active_count.saturating_sub(1);
                }
                alerts.push(serde_json::json!({
                    "level": "error",
                    "pane": i,
                    "theme": config::theme_name(*i),
                    "message": format!("Error detected: {}", err),
                }));
            } else if tmux_done && pd.status == "active" {
                health_status = "done";
                alerts.push(serde_json::json!({
                    "level": "info",
                    "pane": i,
                    "theme": config::theme_name(*i),
                    "message": "Agent finished — ready for completion",
                }));
            }

            if pd.status == "active" && !tmux_done && error_msg.is_none() {
                if let Some(started) = &pd.started_at {
                    if let Ok(start_dt) =
                        NaiveDateTime::parse_from_str(started, "%Y-%m-%dT%H:%M:%S")
                    {
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

            if include_output {
                output_snippet = truncate(&tmux_output, 500);
            }
        } else {
            // PTY fallback
            let pty = app.pty_lock();
            let has_pty = pty.has_agent(*i);
            is_running = pty.is_running(*i);
            line_count = pty.line_count(*i);

            if has_pty {
                let h = pty.check_health(*i, &markers);
                if h.error.is_some() {
                    health_status = "error";
                    error_msg = h.error.clone();
                    if pd.status == "active" {
                        error_count += 1;
                        active_count = active_count.saturating_sub(1);
                    }
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

                if include_output {
                    let screen = pty.screen_text(*i).unwrap_or_default();
                    output_snippet = truncate(&screen, 500);
                }
            }
        }

        let mut pane_info = serde_json::json!({
            "pane": i,
            "theme": config::theme_name(*i),
            "project": pd.project,
            "role": config::role_short(&pd.role),
            "task": truncate(&pd.task, 50),
            "health": health_status,
            "running": is_running,
            "lines": line_count,
            "branch": pd.branch_name,
            "tmux_target": pd.tmux_target,
        });

        if let Some(e) = &error_msg {
            pane_info["error"] = serde_json::json!(e);
        }
        if let Some(d) = &done_marker {
            pane_info["done_marker"] = serde_json::json!(d);
        }
        if !output_snippet.is_empty() {
            pane_info["output"] = serde_json::json!(output_snippet);
        }
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

    let q = queue::load_queue();
    let q_pending = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Pending)
        .count();
    let q_running = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Running)
        .count();
    let q_done = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Done)
        .count();
    let q_failed = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Failed)
        .count();

    let cap = capacity::load_capacity();
    let acu_pct = if cap.acu_total > 0.0 {
        (cap.acu_used / cap.acu_total * 100.0) as i32
    } else {
        0
    };

    let recent: Vec<_> = state_snap
        .activity_log
        .iter()
        .take(5)
        .map(|e| {
            serde_json::json!({
                "time": e.ts.get(11..16).unwrap_or(&e.ts),
                "pane": e.pane,
                "event": e.event,
                "summary": truncate(&e.summary, 60),
            })
        })
        .collect();

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
    })
    .to_string()
}

/// os_watch — Tail a pane's output with error analysis (tmux or PTY)
pub async fn watch(app: &App, req: WatchRequest) -> String {
    let pane_num = match config::resolve_pane(&req.pane) {
        Some(n) => n,
        None => return super::helpers::json_err(&format!("Invalid pane: {}", req.pane)),
    };
    let _tail_lines = req.tail.unwrap_or(30);
    let analyze = req.analyze_errors.unwrap_or(true);

    let pd = app.state.get_pane(pane_num).await;

    // Get output from tmux or PTY
    let (display, running, done) = if let Some(ref target) = pd.tmux_target {
        let output = crate::tmux::capture_output(target);
        let is_done = crate::tmux::check_done(target);
        (output, !is_done, is_done)
    } else {
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
                "branch": pd.branch_name,
                "running": false,
                "phase": if pd.status == "idle" { "idle" } else { "unknown" },
                "line_count": 0,
                "done": false,
                "error_count": 0,
                "warning_count": 0,
                "errors": [],
                "warnings": [],
                "output": format!("[No agent] Pane {} is {}", pane_num, pd.status),
            })
            .to_string();
        }
        let screen = pty.screen_text(pane_num).unwrap_or_default();
        let output = pty.last_output(pane_num, _tail_lines).unwrap_or_default();
        let r = pty.is_running(pane_num);
        let h = pty.check_health(pane_num, &markers);
        drop(pty);
        let d = if !screen.trim().is_empty() {
            screen
        } else {
            output
        };
        (d, r, h.done)
    };

    let line_count = display.lines().count();

    let mut errors_found = Vec::new();
    let mut warnings_found = Vec::new();
    if analyze {
        let error_patterns = [
            "error", "Error", "ERROR", "panic", "PANIC", "failed", "FAILED", "fatal", "FATAL",
        ];
        let warning_patterns = ["warning", "Warning", "WARN", "deprecated", "timeout"];

        for (line_num, line) in display.lines().enumerate() {
            for pat in &error_patterns {
                if line.contains(pat) && !line.contains("error_count") && !line.contains("no_error")
                {
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

    let phase = if done {
        "completed"
    } else if display.contains("Thinking") || display.contains("thinking") {
        "thinking"
    } else if display.contains("Writing")
        || display.contains("Editing")
        || display.contains("Creating")
    {
        "writing"
    } else if display.contains("Reading") || display.contains("Searching") {
        "reading"
    } else if display.contains("Running")
        || display.contains("testing")
        || display.contains("cargo")
        || display.contains("npm")
    {
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
        "tmux_target": pd.tmux_target,
        "running": running,
        "phase": phase,
        "runtime_mins": runtime_mins,
        "line_count": line_count,
        "done": done,
        "output": truncate(&display, 4000),
        "errors": errors_found,
        "warnings": warnings_found,
        "error_count": errors_found.len(),
        "warning_count": warnings_found.len(),
    })
    .to_string()
}

/// os_project_status — Everything about one project across all panes, issues, git, capacity
pub async fn project_status(app: &App, req: ProjectStatusRequest) -> String {
    let include_issues = req.include_issues.unwrap_or(true);
    let include_git = req.include_git.unwrap_or(true);
    let project_lower = req.project.to_lowercase();

    let mut project_panes = Vec::new();
    for i in 1..=config::pane_count() {
        let pd = app.state.get_pane(i).await;
        if pd.project.to_lowercase().contains(&project_lower)
            || pd.project_path.to_lowercase().contains(&project_lower)
        {
            let (running, lines) = if let Some(ref target) = pd.tmux_target {
                let output = crate::tmux::capture_output(target);
                (!crate::tmux::check_done(target), output.lines().count())
            } else {
                let pty = app.pty_lock();
                (pty.is_running(i), pty.line_count(i))
            };

            project_panes.push(serde_json::json!({
                "pane": i,
                "theme": config::theme_name(i),
                "role": pd.role,
                "task": truncate(&pd.task, 60),
                "status": pd.status,
                "branch": pd.branch_name,
                "tmux_target": pd.tmux_target,
                "running": running,
                "lines": lines,
                "acu": pd.acu_spent,
            }));
        }
    }

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
            board_data =
                serde_json::from_str(&board.to_string()).unwrap_or(serde_json::json!(null));

            let issues_dir = config::collab_root()
                .join("spaces")
                .join(&matched_space)
                .join("issues");
            let mut counts = std::collections::HashMap::new();
            if let Ok(entries) = std::fs::read_dir(&issues_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            if let Ok(issue) = serde_json::from_str::<serde_json::Value>(&content) {
                                let status = issue
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("backlog")
                                    .to_string();
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
                .args([
                    "branch",
                    "-a",
                    "--format=%(refname:short) %(upstream:track)",
                ])
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

    let log_path = config::capacity_root().join("work_log.json");
    let log = crate::state::persistence::read_json(&log_path);
    let entries = log
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let project_acu: f64 = entries
        .iter()
        .filter(|e| {
            e.get("space")
                .and_then(|v| v.as_str())
                .map_or(false, |s| s.to_lowercase().contains(&project_lower))
        })
        .filter_map(|e| e.get("acu_spent").and_then(|v| v.as_f64()))
        .sum();

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
    })
    .to_string()
}

/// os_digest — Daily/weekly summary of team output
pub async fn digest(app: &App, req: DigestRequest) -> String {
    let period = req.period.as_deref().unwrap_or("today");
    let project_filter = req.project.as_deref().unwrap_or("");

    let now = Local::now();
    let start = match period {
        "yesterday" => (now - chrono::Duration::days(1))
            .format("%Y-%m-%dT00:00:00")
            .to_string(),
        "week" => (now - chrono::Duration::days(7))
            .format("%Y-%m-%dT00:00:00")
            .to_string(),
        "month" => (now - chrono::Duration::days(30))
            .format("%Y-%m-%dT00:00:00")
            .to_string(),
        _ => now.format("%Y-%m-%dT00:00:00").to_string(),
    };
    let end = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    let state_snap = app.state.get_state_snapshot().await;
    let mut spawns = 0u32;
    let mut completions = 0u32;
    let mut kills = 0u32;
    let mut errors = 0u32;
    let mut projects_seen = std::collections::HashSet::new();

    for entry in &state_snap.activity_log {
        if entry.ts < start {
            continue;
        }
        if !project_filter.is_empty()
            && !entry
                .summary
                .to_lowercase()
                .contains(&project_filter.to_lowercase())
        {
            continue;
        }
        match entry.event.as_str() {
            "spawn" => spawns += 1,
            "complete" => completions += 1,
            "kill" => kills += 1,
            other => {
                tracing::trace!("Unknown activity event: {}", other);
            }
        }
        if entry.summary.to_lowercase().contains("error") {
            errors += 1;
        }
        if entry.event == "spawn" {
            if let Some(proj) = entry.summary.split(" on ").nth(1) {
                if let Some(name) = proj.split(':').next() {
                    projects_seen.insert(name.trim().to_string());
                }
            }
        }
    }

    let log_path = config::capacity_root().join("work_log.json");
    let work_log = crate::state::persistence::read_json(&log_path);
    let work_entries = work_log
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut total_acu = 0.0f64;
    let mut acu_by_role = std::collections::HashMap::<String, f64>::new();
    let mut acu_by_project = std::collections::HashMap::<String, f64>::new();
    let mut reviews_needed = 0u32;
    let mut issues_worked = std::collections::HashSet::new();

    for entry in &work_entries {
        let logged_at = entry
            .get("logged_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if logged_at < start.as_str() {
            continue;
        }

        let space = entry.get("space").and_then(|v| v.as_str()).unwrap_or("");
        if !project_filter.is_empty()
            && !space
                .to_lowercase()
                .contains(&project_filter.to_lowercase())
        {
            continue;
        }

        let acu = entry
            .get("acu_spent")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        total_acu += acu;

        let role = entry
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        *acu_by_role.entry(role).or_default() += acu;

        if !space.is_empty() {
            *acu_by_project.entry(space.to_string()).or_default() += acu;
        }

        if entry
            .get("review_needed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            reviews_needed += 1;
        }

        let issue_id = entry.get("issue_id").and_then(|v| v.as_str()).unwrap_or("");
        if !issue_id.is_empty() && issue_id != "adhoc" {
            issues_worked.insert(issue_id.to_string());
        }
    }

    let q = queue::load_queue();
    let q_done = q
        .tasks
        .iter()
        .filter(|t| {
            t.status == queue::QueueStatus::Done
                && t.completed_at.as_deref().unwrap_or("") >= start.as_str()
        })
        .count();
    let q_failed = q
        .tasks
        .iter()
        .filter(|t| {
            t.status == queue::QueueStatus::Failed
                && t.completed_at.as_deref().unwrap_or("") >= start.as_str()
        })
        .count();
    let q_pending = q
        .tasks
        .iter()
        .filter(|t| t.status == queue::QueueStatus::Pending)
        .count();

    let round_map = |m: &std::collections::HashMap<String, f64>| -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for (k, v) in m {
            result.insert(k.clone(), serde_json::json!((*v * 100.0).round() / 100.0));
        }
        serde_json::Value::Object(result)
    };

    let mut recommendations = Vec::new();
    if q_pending > 5 {
        recommendations.push(format!(
            "{} tasks queued — consider increasing max_parallel panes",
            q_pending
        ));
    }
    if q_failed > 0 {
        recommendations.push(format!(
            "{} tasks failed — review and retry or fix",
            q_failed
        ));
    }
    if errors > 2 {
        recommendations.push(format!(
            "{} errors in period — investigate recurring failures",
            errors
        ));
    }
    if reviews_needed > 3 {
        recommendations.push(format!(
            "{} items need review — review bottleneck risk",
            reviews_needed
        ));
    }
    if completions == 0 && spawns > 0 {
        recommendations
            .push("Agents spawned but nothing completed — check if tasks are stuck".into());
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
    })
    .to_string()
}
