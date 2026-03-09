use rusqlite::params;
use crate::multi_agent::coordination_db;

/// Mark agents as dead if no heartbeat in 10 minutes, release their resources.
pub fn reap_dead_agents() -> Result<(), String> {
    let conn = coordination_db()?;

    let mut stmt = conn.prepare(
        "SELECT pane_id FROM agents
         WHERE status IN ('active','idle')
         AND last_heartbeat IS NOT NULL
         AND last_heartbeat < datetime('now', '-10 minutes')"
    ).map_err(|e| e.to_string())?;

    let dead_agents: Vec<String> = stmt.query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for pane_id in &dead_agents {
        tracing::info!("Reaping dead agent: {pane_id}");

        // Release file locks
        let _ = conn.execute("DELETE FROM file_locks WHERE pane_id = ?1", params![pane_id]);
        // Release ports
        let _ = conn.execute("DELETE FROM ports WHERE pane_id = ?1", params![pane_id]);
        // Release git branches
        let _ = conn.execute("DELETE FROM git_branches WHERE pane_id = ?1", params![pane_id]);
        // Release build locks
        let _ = conn.execute("DELETE FROM builds_active WHERE pane_id = ?1", params![pane_id]);

        // End active session
        let _ = conn.execute(
            "UPDATE sessions SET ended_at = datetime('now'), status = 'timeout',
             duration_secs = CAST((julianday(datetime('now')) - julianday(started_at)) * 86400 AS INTEGER)
             WHERE pane_id = ?1 AND status = 'active'",
            params![pane_id],
        );

        // Mark agent dead
        let _ = conn.execute(
            "UPDATE agents SET status = 'dead' WHERE pane_id = ?1",
            params![pane_id],
        );
    }

    if !dead_agents.is_empty() {
        tracing::info!("Reaped {} dead agents", dead_agents.len());
    }

    Ok(())
}

/// Clean up expired file locks.
pub fn expire_locks() -> Result<(), String> {
    let conn = coordination_db()?;
    let deleted = conn.execute(
        "DELETE FROM file_locks WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        [],
    ).map_err(|e| e.to_string())?;

    if deleted > 0 {
        tracing::info!("Expired {deleted} file locks");
    }
    Ok(())
}
