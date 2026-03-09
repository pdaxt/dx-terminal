use crate::multi_agent::coordination_db;

/// Prune old data according to retention policies.
pub fn prune() -> Result<(), String> {
    let conn = coordination_db()?;

    // tool_calls: 30 days
    let tc = conn.execute(
        "DELETE FROM tool_calls WHERE timestamp < datetime('now', '-30 days')", [],
    ).map_err(|e| e.to_string())?;

    // file_operations: 14 days
    let fo = conn.execute(
        "DELETE FROM file_operations WHERE timestamp < datetime('now', '-14 days')", [],
    ).map_err(|e| e.to_string())?;

    // token_usage: 90 days
    let tu = conn.execute(
        "DELETE FROM token_usage WHERE timestamp < datetime('now', '-90 days')", [],
    ).map_err(|e| e.to_string())?;

    // messages: 7 days old
    let msg = conn.execute(
        "DELETE FROM messages WHERE timestamp < datetime('now', '-7 days')", [],
    ).map_err(|e| e.to_string())?;

    // quality_events: 90 days
    let qe = conn.execute(
        "DELETE FROM quality_events WHERE timestamp < datetime('now', '-90 days')", [],
    ).map_err(|e| e.to_string())?;

    // git_commits: 180 days
    let gc = conn.execute(
        "DELETE FROM git_commits WHERE timestamp < datetime('now', '-180 days')", [],
    ).map_err(|e| e.to_string())?;

    // Dead/deregistered agents: 30 days
    let da = conn.execute(
        "DELETE FROM agents WHERE status IN ('dead','deregistered') AND last_heartbeat < datetime('now', '-30 days')", [],
    ).map_err(|e| e.to_string())?;

    // Ended sessions: 30 days
    let cs = conn.execute(
        "DELETE FROM sessions WHERE status IN ('ended','timeout') AND ended_at < datetime('now', '-30 days')", [],
    ).map_err(|e| e.to_string())?;

    let total = tc + fo + tu + msg + qe + gc + da + cs;
    if total > 0 {
        tracing::info!("Retention: pruned {total} rows (tc={tc} fo={fo} tu={tu} msg={msg} qe={qe} gc={gc} agents={da} sessions={cs})");
    }

    Ok(())
}

/// Manual prune callable as MCP tool.
pub fn prune_manual() -> serde_json::Value {
    match prune() {
        Ok(()) => serde_json::json!({"status": "pruned"}),
        Err(e) => serde_json::json!({"error": e}),
    }
}
