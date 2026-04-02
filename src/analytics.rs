use crate::multi_agent::{coordination_db, now_iso};
use rusqlite::params;
use serde_json::{json, Value};

fn parse_tool_name(name: &str) -> (Option<String>, Option<String>) {
    if name.starts_with("mcp__") {
        let parts: Vec<&str> = name.splitn(3, "__").collect();
        if parts.len() == 3 {
            return (Some(parts[1].to_string()), Some(parts[2].to_string()));
        }
    }
    (None, Some(name.to_string()))
}

/// Look up session_id and project for an agent (pane_id).
fn agent_context(conn: &rusqlite::Connection, pane_id: &str) -> (Option<String>, String) {
    let session_id: Option<String> = conn
        .query_row(
            "SELECT session_id FROM agents WHERE pane_id = ?1",
            params![pane_id],
            |r| r.get(0),
        )
        .ok();
    let project: String = conn
        .query_row(
            "SELECT project FROM agents WHERE pane_id = ?1",
            params![pane_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    (session_id, project)
}

pub fn log_tool_call(
    pane_id: &str,
    tool_name: &str,
    input_size: i64,
    output_size: i64,
    latency_ms: Option<i64>,
    success: bool,
    error_preview: Option<&str>,
) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let (mcp_name, tool_short) = parse_tool_name(tool_name);
    let (session_id, project) = agent_context(&conn, pane_id);
    let now = now_iso();

    let _ = conn.execute(
        "INSERT INTO tool_calls (session_id, pane_id, project, tool_name, mcp_name, tool_short, input_size, output_size, latency_ms, success, error_preview, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![session_id, pane_id, project, tool_name, mcp_name, tool_short,
                input_size, output_size, latency_ms, success as i32, error_preview, now],
    );

    // Update session counters
    if let Some(sid) = &session_id {
        let _ = conn.execute(
            "UPDATE sessions SET tool_calls = tool_calls + 1 WHERE session_id = ?1",
            params![sid],
        );
        if !success {
            let _ = conn.execute(
                "UPDATE sessions SET errors = errors + 1 WHERE session_id = ?1",
                params![sid],
            );
        }
    }
    let _ = conn.execute(
        "UPDATE agents SET last_tool_call = ?1 WHERE pane_id = ?2",
        params![now, pane_id],
    );

    json!({"status": "logged"})
}

pub fn log_file_op(
    pane_id: &str,
    file_path: &str,
    operation: &str,
    lines_changed: Option<i64>,
) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let (session_id, project) = agent_context(&conn, pane_id);
    let now = now_iso();

    let _ = conn.execute(
        "INSERT INTO file_operations (session_id, pane_id, project, file_path, operation, lines_changed, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, pane_id, project, file_path, operation, lines_changed.unwrap_or(0), now],
    );

    if let Some(sid) = &session_id {
        let _ = conn.execute(
            "UPDATE sessions SET files_touched = files_touched + 1 WHERE session_id = ?1",
            params![sid],
        );
    }
    json!({"status": "logged"})
}

pub fn log_tokens(
    pane_id: &str,
    model: &str,
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let (session_id, project) = agent_context(&conn, pane_id);
    let now = now_iso();

    let _ = conn.execute(
        "INSERT INTO token_usage (session_id, pane_id, project, model, input_tokens, output_tokens, cache_read, cache_write, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![session_id, pane_id, project, model, input, output, cache_read, cache_write, now],
    );
    json!({"status": "logged"})
}

#[allow(clippy::too_many_arguments)]
pub fn log_git_commit(
    pane_id: &str,
    project: &str,
    repo_path: &str,
    commit_hash: &str,
    branch: &str,
    message: &str,
    files_changed: i64,
    insertions: i64,
    deletions: i64,
) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let session_id: Option<String> = conn
        .query_row(
            "SELECT session_id FROM agents WHERE pane_id = ?1",
            params![pane_id],
            |r| r.get(0),
        )
        .ok();
    let now = now_iso();

    let _ = conn.execute(
        "INSERT INTO git_commits (session_id, pane_id, project, repo_path, commit_hash, branch, message, files_changed, insertions, deletions, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![session_id, pane_id, project, repo_path, commit_hash, branch, message, files_changed, insertions, deletions, now],
    );

    if let Some(sid) = &session_id {
        let _ = conn.execute(
            "UPDATE sessions SET commits = commits + 1 WHERE session_id = ?1",
            params![sid],
        );
    }
    json!({"status": "logged", "commit_hash": commit_hash})
}

pub fn usage_report(pane_id: Option<&str>, project: Option<&str>, days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    let mut conditions = vec![format!("timestamp > datetime('now', '-{days} days')")];
    let mut param_values: Vec<String> = vec![];

    if let Some(a) = pane_id {
        conditions.push(format!("pane_id = ?{}", param_values.len() + 1));
        param_values.push(a.to_string());
    }
    if let Some(p) = project {
        conditions.push(format!("project = ?{}", param_values.len() + 1));
        param_values.push(p.to_string());
    }

    let where_clause = conditions.join(" AND ");
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values
        .iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();

    let total_calls: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM tool_calls WHERE {where_clause}"),
            params_ref.as_slice(),
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total_errors: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM tool_calls WHERE success = 0 AND {where_clause}"),
            params_ref.as_slice(),
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total_files: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM file_operations WHERE {where_clause}"),
            params_ref.as_slice(),
            |r| r.get(0),
        )
        .unwrap_or(0);

    json!({
        "period_days": days,
        "total_tool_calls": total_calls,
        "total_errors": total_errors,
        "error_rate": if total_calls > 0 { total_errors as f64 / total_calls as f64 * 100.0 } else { 0.0 },
        "total_file_operations": total_files,
    })
}

pub fn tool_ranking(project: Option<&str>, days: i64, limit: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();
    let sql = format!(
        "SELECT tool_name, COUNT(*) as cnt, SUM(CASE WHEN success=0 THEN 1 ELSE 0 END) as errors,
                AVG(latency_ms) as avg_latency
         FROM tool_calls WHERE timestamp > datetime('now', '-{days} days') {proj_filter}
         GROUP BY tool_name ORDER BY cnt DESC LIMIT {limit}"
    );
    let mut tools = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({
                "tool": r.get::<_, String>(0)?,
                "calls": r.get::<_, i64>(1)?,
                "errors": r.get::<_, i64>(2)?,
                "avg_latency_ms": r.get::<_, Option<f64>>(3)?,
            }))
        }) {
            for row in rows.flatten() {
                tools.push(row);
            }
        }
    }
    json!({"tools": tools})
}

pub fn mcp_health(days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let sql = format!(
        "SELECT mcp_name, COUNT(*) as cnt, SUM(CASE WHEN success=0 THEN 1 ELSE 0 END) as errors,
                AVG(latency_ms) as avg_latency
         FROM tool_calls WHERE mcp_name IS NOT NULL AND timestamp > datetime('now', '-{days} days')
         GROUP BY mcp_name ORDER BY cnt DESC"
    );
    let mut mcps = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| {
            let cnt: i64 = r.get(1)?;
            let errors: i64 = r.get(2)?;
            Ok(json!({
                "mcp": r.get::<_, String>(0)?,
                "calls": cnt, "errors": errors,
                "error_rate": if cnt > 0 { errors as f64 / cnt as f64 * 100.0 } else { 0.0 },
                "avg_latency_ms": r.get::<_, Option<f64>>(3)?,
            }))
        }) {
            for row in rows.flatten() {
                mcps.push(row);
            }
        }
    }
    json!({"mcps": mcps})
}

pub fn agent_activity(pane_id: &str, limit: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let mut events = vec![];
    if let Ok(mut stmt) = conn.prepare(
        "SELECT 'tool_call' as type, tool_name as detail, timestamp FROM tool_calls WHERE pane_id = ?1
         UNION ALL
         SELECT 'file_op', file_path || ' (' || operation || ')', timestamp FROM file_operations WHERE pane_id = ?1
         UNION ALL
         SELECT 'commit', message, timestamp FROM git_commits WHERE pane_id = ?1
         ORDER BY timestamp DESC LIMIT ?2"
    ) {
        if let Ok(rows) = stmt.query_map(params![pane_id, limit], |r| {
            Ok(json!({"type": r.get::<_, String>(0)?, "detail": r.get::<_, String>(1)?, "timestamp": r.get::<_, String>(2)?}))
        }) {
            for row in rows.flatten() { events.push(row); }
        }
    }
    json!({"pane_id": pane_id, "events": events})
}

pub fn cost_report(project: Option<&str>, days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();
    let sql = format!(
        "SELECT model, SUM(input_tokens) as inp, SUM(output_tokens) as out,
                SUM(cache_read) as cr, SUM(cache_write) as cw
         FROM token_usage WHERE timestamp > datetime('now', '-{days} days') {proj_filter}
         GROUP BY model ORDER BY (inp + out) DESC"
    );
    let mut models = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({
                "model": r.get::<_, String>(0)?,
                "input_tokens": r.get::<_, i64>(1)?,
                "output_tokens": r.get::<_, i64>(2)?,
                "cache_read": r.get::<_, i64>(3)?,
                "cache_write": r.get::<_, i64>(4)?,
            }))
        }) {
            for row in rows.flatten() {
                models.push(row);
            }
        }
    }
    json!({"period_days": days, "models": models})
}

pub fn trends(metric: &str, project: Option<&str>, granularity: &str, periods: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let date_format = match granularity {
        "weekly" => "%Y-W%W",
        "monthly" => "%Y-%m",
        _ => "%Y-%m-%d",
    };
    let proj_filter = project
        .map(|p| format!("AND project = '{}'", p.replace('\'', "''")))
        .unwrap_or_default();
    let (table, count_expr) = match metric {
        "tokens" => ("token_usage", "SUM(input_tokens + output_tokens)"),
        "errors" => ("tool_calls", "SUM(CASE WHEN success=0 THEN 1 ELSE 0 END)"),
        "files" => ("file_operations", "COUNT(*)"),
        "commits" => ("git_commits", "COUNT(*)"),
        _ => ("tool_calls", "COUNT(*)"),
    };
    let sql = format!(
        "SELECT strftime('{date_format}', timestamp) as period, {count_expr} as val
         FROM {table} WHERE timestamp > datetime('now', '-{periods} days') {proj_filter}
         GROUP BY period ORDER BY period"
    );
    let mut data = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({"period": r.get::<_, String>(0)?, "value": r.get::<_, i64>(1)?}))
        }) {
            for row in rows.flatten() {
                data.push(row);
            }
        }
    }
    json!({"metric": metric, "granularity": granularity, "data": data})
}
