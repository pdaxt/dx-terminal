use crate::multi_agent::{coordination_db, now_iso};
use rusqlite::params;
use serde_json::{json, Value};

pub fn log_test(
    pane_id: &str,
    project: &str,
    command: Option<&str>,
    success: bool,
    total: Option<i64>,
    passed: Option<i64>,
    failed: Option<i64>,
    skipped: Option<i64>,
    duration_ms: Option<i64>,
    output: Option<&str>,
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
        "INSERT INTO quality_events (session_id, pane_id, project, event_type, command, success, total_count, pass_count, fail_count, skip_count, duration_ms, output, timestamp)
         VALUES (?1, ?2, ?3, 'test', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![session_id, pane_id, project, command.unwrap_or(""), success as i32,
                total.unwrap_or(0), passed.unwrap_or(0), failed.unwrap_or(0), skipped.unwrap_or(0),
                duration_ms.unwrap_or(0), output.unwrap_or(""), now],
    );
    json!({"status": "logged", "event_type": "test", "success": success})
}

pub fn log_build(
    pane_id: &str,
    project: &str,
    command: Option<&str>,
    success: bool,
    duration_ms: Option<i64>,
    output: Option<&str>,
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
        "INSERT INTO quality_events (session_id, pane_id, project, event_type, command, success, duration_ms, output, timestamp)
         VALUES (?1, ?2, ?3, 'build', ?4, ?5, ?6, ?7, ?8)",
        params![session_id, pane_id, project, command.unwrap_or(""), success as i32, duration_ms.unwrap_or(0), output.unwrap_or(""), now],
    );
    json!({"status": "logged", "event_type": "build", "success": success})
}

pub fn log_lint(
    pane_id: &str,
    project: &str,
    command: Option<&str>,
    success: bool,
    total: Option<i64>,
    errors: Option<i64>,
    warnings: Option<i64>,
    output: Option<&str>,
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
        "INSERT INTO quality_events (session_id, pane_id, project, event_type, command, success, total_count, fail_count, skip_count, output, timestamp)
         VALUES (?1, ?2, ?3, 'lint', ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![session_id, pane_id, project, command.unwrap_or(""), success as i32,
                total.unwrap_or(0), errors.unwrap_or(0), warnings.unwrap_or(0), output.unwrap_or(""), now],
    );
    json!({"status": "logged", "event_type": "lint", "success": success})
}

pub fn log_deploy(
    pane_id: &str,
    project: &str,
    target: Option<&str>,
    success: bool,
    duration_ms: Option<i64>,
    output: Option<&str>,
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
        "INSERT INTO quality_events (session_id, pane_id, project, event_type, command, success, duration_ms, output, timestamp)
         VALUES (?1, ?2, ?3, 'deploy', ?4, ?5, ?6, ?7, ?8)",
        params![session_id, pane_id, project, target.unwrap_or(""), success as i32, duration_ms.unwrap_or(0), output.unwrap_or(""), now],
    );
    json!({"status": "logged", "event_type": "deploy", "success": success})
}

pub fn quality_report(project: &str, days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let sql = format!(
        "SELECT event_type, COUNT(*) as cnt, SUM(success) as passes, AVG(duration_ms) as avg_dur
         FROM quality_events WHERE project = ?1 AND timestamp > datetime('now', '-{days} days')
         GROUP BY event_type"
    );
    let mut events = vec![];
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params![project], |r| {
            let cnt: i64 = r.get(1)?;
            let passes: i64 = r.get(2)?;
            Ok(json!({
                "event_type": r.get::<_, String>(0)?,
                "total": cnt, "passed": passes, "failed": cnt - passes,
                "pass_rate": if cnt > 0 { passes as f64 / cnt as f64 * 100.0 } else { 0.0 },
                "avg_duration_ms": r.get::<_, Option<f64>>(3)?,
            }))
        }) {
            for row in rows.flatten() {
                events.push(row);
            }
        }
    }
    json!({"project": project, "period_days": days, "events": events})
}

pub fn quality_gate(project: &str) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    let last_test: Option<(bool, String)> = conn.query_row(
        "SELECT success, timestamp FROM quality_events WHERE project = ?1 AND event_type = 'test' ORDER BY timestamp DESC LIMIT 1",
        params![project], |r| Ok((r.get::<_, i32>(0)? != 0, r.get::<_, String>(1)?)),
    ).ok();

    let last_build: Option<(bool, String)> = conn.query_row(
        "SELECT success, timestamp FROM quality_events WHERE project = ?1 AND event_type = 'build' ORDER BY timestamp DESC LIMIT 1",
        params![project], |r| Ok((r.get::<_, i32>(0)? != 0, r.get::<_, String>(1)?)),
    ).ok();

    let tests_pass = last_test.as_ref().map(|(s, _)| *s).unwrap_or(true);
    let build_pass = last_build.as_ref().map(|(s, _)| *s).unwrap_or(true);
    let gate_pass = tests_pass && build_pass;

    json!({
        "project": project,
        "gate": if gate_pass { "PASS" } else { "FAIL" },
        "tests": { "pass": tests_pass, "last_run": last_test.map(|(_, t)| t) },
        "build": { "pass": build_pass, "last_run": last_build.map(|(_, t)| t) },
    })
}

pub fn regressions(project: &str, days: i64) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };
    let half = days / 2;

    let recent: (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), COALESCE(SUM(success), 0) FROM quality_events WHERE project = ?1 AND timestamp > datetime('now', '-{half} days')"),
        params![project], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    let older: (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), COALESCE(SUM(success), 0) FROM quality_events WHERE project = ?1 AND timestamp BETWEEN datetime('now', '-{days} days') AND datetime('now', '-{half} days')"),
        params![project], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    let recent_rate = if recent.0 > 0 {
        recent.1 as f64 / recent.0 as f64
    } else {
        1.0
    };
    let older_rate = if older.0 > 0 {
        older.1 as f64 / older.0 as f64
    } else {
        1.0
    };
    let regression = older_rate - recent_rate;

    json!({
        "project": project,
        "recent_pass_rate": recent_rate * 100.0,
        "older_pass_rate": older_rate * 100.0,
        "regression": if regression > 0.05 { "DETECTED" } else { "NONE" },
        "delta": regression * 100.0,
    })
}

pub fn project_health(project: &str) -> Value {
    let conn = match coordination_db() {
        Ok(c) => c,
        Err(e) => return json!({"error": e}),
    };

    let test_rate: f64 = conn.query_row(
        "SELECT COALESCE(AVG(CAST(success AS REAL)), 1.0) FROM quality_events WHERE project = ?1 AND event_type = 'test' AND timestamp > datetime('now', '-7 days')",
        params![project], |r| r.get(0),
    ).unwrap_or(1.0);

    let build_rate: f64 = conn.query_row(
        "SELECT COALESCE(AVG(CAST(success AS REAL)), 1.0) FROM quality_events WHERE project = ?1 AND event_type = 'build' AND timestamp > datetime('now', '-7 days')",
        params![project], |r| r.get(0),
    ).unwrap_or(1.0);

    let error_rate: f64 = conn.query_row(
        "SELECT COALESCE(1.0 - AVG(CAST(success AS REAL)), 0.0) FROM tool_calls WHERE project = ?1 AND timestamp > datetime('now', '-7 days')",
        params![project], |r| r.get(0),
    ).unwrap_or(0.0);

    let score = ((test_rate * 40.0) + (build_rate * 40.0) + ((1.0 - error_rate) * 20.0)) as i64;

    json!({
        "project": project,
        "health_score": score,
        "grade": match score { s if s >= 90 => "A", s if s >= 80 => "B", s if s >= 70 => "C", s if s >= 60 => "D", _ => "F" },
        "test_pass_rate": test_rate * 100.0,
        "build_pass_rate": build_rate * 100.0,
        "tool_error_rate": error_rate * 100.0,
    })
}
