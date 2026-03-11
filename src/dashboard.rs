use crate::multi_agent::coordination_db;
use rusqlite::params;
use serde_json::{json, Value};

pub fn dash_overview(project: Option<&str>) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();

    // Active agents
    let mut agents = vec![];
    if let Ok(mut stmt) = conn.prepare(&format!(
        "SELECT pane_id, project, task, role, status, last_heartbeat FROM agents
         WHERE status IN ('active','idle','busy') {proj_filter} ORDER BY project, pane_id"
    )) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({
                "pane_id": r.get::<_, String>(0)?, "project": r.get::<_, String>(1)?,
                "task": r.get::<_, String>(2)?, "role": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?, "last_heartbeat": r.get::<_, Option<String>>(5)?,
            }))
        }) {
            for row in rows.flatten() {
                agents.push(row);
            }
        }
    }
    let agent_count = agents.len();

    // Pending tasks
    let pending_tasks: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM tasks WHERE status IN ('pending','blocked') {proj_filter}"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Active locks
    let active_locks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM file_locks WHERE expires_at IS NULL OR expires_at > datetime('now')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Recent activity (last hour)
    let recent_tool_calls: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM tool_calls WHERE timestamp > datetime('now', '-1 hour') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let recent_errors: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM tool_calls WHERE success = 0 AND timestamp > datetime('now', '-1 hour') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Unread messages
    let unread_msgs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE read_by = '[]'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Allocated ports
    let mut ports = vec![];
    if let Ok(mut stmt) = conn.prepare("SELECT port, service, pane_id FROM ports ORDER BY port") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({"port": r.get::<_, u16>(0)?, "service": r.get::<_, String>(1)?, "pane_id": r.get::<_, String>(2)?}))
        }) {
            for row in rows.flatten() { ports.push(row); }
        }
    }

    // Quality snapshot
    let last_test_pass: Option<bool> = conn.query_row(
        &format!("SELECT success FROM quality_events WHERE event_type = 'test' {proj_filter} ORDER BY timestamp DESC LIMIT 1"),
        [], |r| Ok(r.get::<_, i32>(0)? != 0),
    ).ok();
    let last_build_pass: Option<bool> = conn.query_row(
        &format!("SELECT success FROM quality_events WHERE event_type = 'build' {proj_filter} ORDER BY timestamp DESC LIMIT 1"),
        [], |r| Ok(r.get::<_, i32>(0)? != 0),
    ).ok();

    json!({
        "agents": agents,
        "agent_count": agent_count,
        "pending_tasks": pending_tasks,
        "active_locks": active_locks,
        "ports": ports,
        "recent": {
            "tool_calls_1h": recent_tool_calls,
            "errors_1h": recent_errors,
            "unread_messages": unread_msgs,
        },
        "quality": {
            "last_test": last_test_pass,
            "last_build": last_build_pass,
        }
    })
}

pub fn dash_agent_detail(pane_id: &str) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    // Agent info
    let agent = match conn.query_row(
        "SELECT pane_id, project, task, role, status, files, session_id, registered_at, last_heartbeat, metadata
         FROM agents WHERE pane_id = ?1",
        params![pane_id],
        |r| Ok(json!({
            "pane_id": r.get::<_, String>(0)?, "project": r.get::<_, String>(1)?,
            "task": r.get::<_, String>(2)?, "role": r.get::<_, String>(3)?,
            "status": r.get::<_, String>(4)?, "files": r.get::<_, String>(5)?,
            "session_id": r.get::<_, Option<String>>(6)?, "registered_at": r.get::<_, String>(7)?,
            "last_heartbeat": r.get::<_, Option<String>>(8)?, "metadata": r.get::<_, String>(9)?,
        })),
    ) {
        Ok(a) => a,
        Err(_) => return json!({"error": "agent not found"}),
    };

    // Recent tool calls
    let mut recent_tools = vec![];
    if let Ok(mut stmt) = conn.prepare(
        "SELECT tool_name, success, timestamp FROM tool_calls WHERE pane_id = ?1 ORDER BY timestamp DESC LIMIT 10"
    ) {
        if let Ok(rows) = stmt.query_map(params![pane_id], |r| {
            Ok(json!({"tool": r.get::<_, String>(0)?, "success": r.get::<_, i32>(1)? != 0, "at": r.get::<_, String>(2)?}))
        }) {
            for row in rows.flatten() { recent_tools.push(row); }
        }
    }

    // Current locks
    let mut locks = vec![];
    if let Ok(mut stmt) =
        conn.prepare("SELECT file_path, reason FROM file_locks WHERE pane_id = ?1")
    {
        if let Ok(rows) = stmt.query_map(params![pane_id], |r| {
            Ok(json!({"file": r.get::<_, String>(0)?, "reason": r.get::<_, String>(1)?}))
        }) {
            for row in rows.flatten() {
                locks.push(row);
            }
        }
    }

    // Session stats
    let session_stats = conn.query_row(
        "SELECT tool_calls, errors, files_touched, commits FROM sessions WHERE pane_id = ?1 AND status = 'active' ORDER BY started_at DESC LIMIT 1",
        params![pane_id],
        |r| Ok(json!({"tool_calls": r.get::<_, i64>(0)?, "errors": r.get::<_, i64>(1)?,
                      "files_touched": r.get::<_, i64>(2)?, "commits": r.get::<_, i64>(3)?})),
    ).unwrap_or(json!({}));

    json!({
        "agent": agent,
        "recent_tools": recent_tools,
        "current_locks": locks,
        "session_stats": session_stats,
    })
}

pub fn dash_project(project: &str) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    // Agents on this project
    let mut agents = vec![];
    if let Ok(mut stmt) = conn.prepare(
        "SELECT pane_id, task, role, status FROM agents WHERE project = ?1 AND status IN ('active','idle','busy')"
    ) {
        if let Ok(rows) = stmt.query_map(params![project], |r| {
            Ok(json!({"pane_id": r.get::<_, String>(0)?, "task": r.get::<_, String>(1)?,
                       "role": r.get::<_, String>(2)?, "status": r.get::<_, String>(3)?}))
        }) {
            for row in rows.flatten() { agents.push(row); }
        }
    }

    // Task breakdown
    let mut task_counts = vec![];
    if let Ok(mut stmt) =
        conn.prepare("SELECT status, COUNT(*) FROM tasks WHERE project = ?1 GROUP BY status")
    {
        if let Ok(rows) = stmt.query_map(params![project], |r| {
            Ok(json!({"status": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)?}))
        }) {
            for row in rows.flatten() {
                task_counts.push(row);
            }
        }
    }

    // Quality (last 7 days)
    let test_rate: f64 = conn.query_row(
        "SELECT COALESCE(AVG(CAST(success AS REAL)), 1.0) FROM quality_events WHERE project = ?1 AND event_type = 'test' AND timestamp > datetime('now', '-7 days')",
        params![project], |r| r.get(0),
    ).unwrap_or(1.0);

    // Recent commits
    let mut commits = vec![];
    if let Ok(mut stmt) = conn.prepare(
        "SELECT commit_hash, branch, message, pane_id, timestamp FROM git_commits WHERE project = ?1 ORDER BY timestamp DESC LIMIT 5"
    ) {
        if let Ok(rows) = stmt.query_map(params![project], |r| {
            Ok(json!({"hash": r.get::<_, String>(0)?, "branch": r.get::<_, String>(1)?,
                       "message": r.get::<_, String>(2)?, "pane_id": r.get::<_, Option<String>>(3)?,
                       "at": r.get::<_, String>(4)?}))
        }) {
            for row in rows.flatten() { commits.push(row); }
        }
    }

    // KB count
    let kb_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kb_entries WHERE project = ?1",
            params![project],
            |r| r.get(0),
        )
        .unwrap_or(0);

    json!({
        "project": project,
        "agents": agents,
        "tasks": task_counts,
        "test_pass_rate": test_rate * 100.0,
        "recent_commits": commits,
        "knowledge_entries": kb_count,
    })
}

pub fn dash_leaderboard(days: i64, project: Option<&str>) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();

    let sql = format!(
        "SELECT pane_id,
                COUNT(*) as total_calls,
                SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as errors,
                COUNT(DISTINCT DATE(timestamp)) as active_days
         FROM tool_calls
         WHERE timestamp > datetime('now', '-{days} days') {proj_filter}
         GROUP BY pane_id
         ORDER BY total_calls DESC
         LIMIT 20"
    );

    let mut rankings = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| {
            let total: i64 = r.get(1)?;
            let errors: i64 = r.get(2)?;
            Ok(json!({
                "pane_id": r.get::<_, String>(0)?,
                "tool_calls": total,
                "errors": errors,
                "success_rate": if total > 0 { (total - errors) as f64 / total as f64 * 100.0 } else { 100.0 },
                "active_days": r.get::<_, i64>(3)?,
            }))
        }) {
            for row in rows.flatten() { rankings.push(row); }
        }
    }
    json!({"period_days": days, "rankings": rankings})
}

pub fn dash_timeline(project: Option<&str>, pane_id: Option<&str>, limit: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    let mut conditions = Vec::new();
    if let Some(p) = project {
        conditions.push(format!("project = '{}'", p.replace('\'', "''")));
    }
    if let Some(a) = pane_id {
        conditions.push(format!("pane_id = '{}'", a.replace('\'', "''")));
    }
    let where_tc = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let pane_filter = pane_id
        .map(|a| format!("WHERE pane_id = '{}'", a.replace('\'', "''")))
        .unwrap_or_default();

    let sql = format!(
        "SELECT 'tool_call' as type, tool_name as detail, pane_id, timestamp FROM tool_calls {where_tc}
         UNION ALL
         SELECT 'commit', message, pane_id, timestamp FROM git_commits {pane_filter}
         ORDER BY timestamp DESC LIMIT ?1"
    );

    let mut events = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params![limit], |r| {
            Ok(json!({
                "type": r.get::<_, String>(0)?, "detail": r.get::<_, String>(1)?,
                "pane_id": r.get::<_, String>(2)?, "at": r.get::<_, String>(3)?,
            }))
        }) {
            for row in rows.flatten() {
                events.push(row);
            }
        }
    }
    let count = events.len();
    json!({"events": events, "count": count})
}

pub fn dash_alerts(project: Option<&str>) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();
    let mut alert_list: Vec<Value> = Vec::new();

    // Dead agents (no heartbeat in 10 min)
    if let Ok(mut stmt) = conn.prepare(&format!(
        "SELECT pane_id, project, last_heartbeat FROM agents
         WHERE status IN ('active','idle') AND last_heartbeat IS NOT NULL AND last_heartbeat < datetime('now', '-10 minutes') {proj_filter}"
    )) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({"level": "critical", "type": "dead_agent",
                       "pane_id": r.get::<_, String>(0)?, "project": r.get::<_, String>(1)?,
                       "last_seen": r.get::<_, String>(2)?}))
        }) {
            for row in rows.flatten() { alert_list.push(row); }
        }
    }

    // High error rate (>20% in last hour)
    let hour_total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM tool_calls WHERE timestamp > datetime('now', '-1 hour') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);
    let hour_errors: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM tool_calls WHERE success = 0 AND timestamp > datetime('now', '-1 hour') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if hour_total > 10 && hour_errors as f64 / hour_total as f64 > 0.2 {
        alert_list.push(json!({
            "level": "warning", "type": "high_error_rate",
            "error_rate": format!("{:.1}%", hour_errors as f64 / hour_total as f64 * 100.0),
            "errors": hour_errors, "total": hour_total,
        }));
    }

    // Failed quality gates
    if let Ok(proj) = conn.query_row::<String, _, _>(
        &format!("SELECT project FROM quality_events WHERE event_type = 'test' AND success = 0 {proj_filter} ORDER BY timestamp DESC LIMIT 1"),
        [], |r| r.get(0),
    ) {
        alert_list.push(json!({"level": "warning", "type": "test_failure", "project": proj}));
    }

    // Expired locks
    let expired_locks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM file_locks WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        [], |r| r.get(0),
    ).unwrap_or(0);
    if expired_locks > 0 {
        alert_list.push(json!({"level": "info", "type": "expired_locks", "count": expired_locks}));
    }

    let count = alert_list.len();
    json!({"alerts": alert_list, "count": count})
}

pub fn dash_daily_digest(project: Option<&str>) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();
    let date_filter = "AND timestamp > datetime('now', '-24 hours')";

    let agents_today: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT pane_id) FROM tool_calls WHERE 1=1 {date_filter} {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let calls_today: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM tool_calls WHERE 1=1 {date_filter} {proj_filter}"),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let errors_today: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM tool_calls WHERE success = 0 {date_filter} {proj_filter}"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let commits_today: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM git_commits WHERE 1=1 {date_filter} {proj_filter}"),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let files_today: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT file_path) FROM file_operations WHERE 1=1 {date_filter} {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let tasks_done: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM tasks WHERE status = 'completed' AND completed_at > datetime('now', '-24 hours') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let kb_added: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM kb_entries WHERE added_at > datetime('now', '-24 hours') {proj_filter}"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    json!({
        "period": "24h",
        "agents_active": agents_today,
        "tool_calls": calls_today,
        "errors": errors_today,
        "error_rate": if calls_today > 0 { format!("{:.1}%", errors_today as f64 / calls_today as f64 * 100.0) } else { "0%".into() },
        "commits": commits_today,
        "files_touched": files_today,
        "tasks_completed": tasks_done,
        "knowledge_added": kb_added,
    })
}

pub fn dash_export(report: &str, project: Option<&str>, days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();

    match report {
        "agents" => {
            let mut data = vec![];
            if let Ok(mut stmt) = conn.prepare(&format!(
                "SELECT pane_id, project, task, role, status, registered_at, last_heartbeat FROM agents WHERE 1=1 {proj_filter}"
            )) {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(json!({"pane_id": r.get::<_, String>(0)?, "project": r.get::<_, String>(1)?,
                              "task": r.get::<_, String>(2)?, "role": r.get::<_, String>(3)?,
                              "status": r.get::<_, String>(4)?, "registered_at": r.get::<_, String>(5)?,
                              "last_heartbeat": r.get::<_, Option<String>>(6)?}))
                }) {
                    for row in rows.flatten() { data.push(row); }
                }
            }
            let count = data.len();
            json!({"report": "agents", "data": data, "count": count})
        }
        "usage" => {
            let mut data = vec![];
            if let Ok(mut stmt) = conn.prepare(&format!(
                "SELECT pane_id, tool_name, success, timestamp FROM tool_calls
                 WHERE timestamp > datetime('now', '-{days} days') {proj_filter}
                 ORDER BY timestamp DESC LIMIT 1000"
            )) {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(
                        json!({"pane_id": r.get::<_, String>(0)?, "tool": r.get::<_, String>(1)?,
                              "success": r.get::<_, i32>(2)? != 0, "at": r.get::<_, String>(3)?}),
                    )
                }) {
                    for row in rows.flatten() {
                        data.push(row);
                    }
                }
            }
            let count = data.len();
            json!({"report": "usage", "period_days": days, "data": data, "count": count})
        }
        "quality" => {
            let mut data = vec![];
            if let Ok(mut stmt) = conn.prepare(&format!(
                "SELECT event_type, success, total_count, pass_count, fail_count, timestamp FROM quality_events
                 WHERE timestamp > datetime('now', '-{days} days') {proj_filter}
                 ORDER BY timestamp DESC"
            )) {
                if let Ok(rows) = stmt.query_map([], |r| {
                    Ok(json!({"type": r.get::<_, String>(0)?, "success": r.get::<_, i32>(1)? != 0,
                              "total": r.get::<_, i64>(2)?, "passed": r.get::<_, i64>(3)?,
                              "failed": r.get::<_, i64>(4)?, "at": r.get::<_, String>(5)?}))
                }) {
                    for row in rows.flatten() { data.push(row); }
                }
            }
            let count = data.len();
            json!({"report": "quality", "period_days": days, "data": data, "count": count})
        }
        _ => {
            json!({"error": format!("Unknown report type: {report}. Use: agents, usage, quality")})
        }
    }
}
