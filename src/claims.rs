//! Claims Registry — prevents multiple agents from working on the same issue.
//!
//! Backed by SQLite at `~/.config/dx-terminal/claims.db`.
//! Atomic INSERT OR IGNORE ensures only one agent can claim an issue.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS claims (
    repo       TEXT NOT NULL,
    issue      INTEGER NOT NULL,
    agent_id   TEXT NOT NULL,
    claimed_at TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'active',
    released_at TEXT,
    PRIMARY KEY (repo, issue)
);
CREATE INDEX IF NOT EXISTS idx_claims_agent ON claims(agent_id);
CREATE INDEX IF NOT EXISTS idx_claims_status ON claims(status);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub repo: String,
    pub issue: u32,
    pub agent_id: String,
    pub claimed_at: String,
    pub status: String,
    pub released_at: Option<String>,
}

fn db_path() -> std::path::PathBuf {
    config::dx_root().join("claims.db")
}

fn open_db() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create claims db directory")?;
    }
    let conn = Connection::open(&path).context("open claims database")?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .context("set pragmas")?;
    conn.execute_batch(SCHEMA)
        .context("initialize claims schema")?;
    Ok(conn)
}

/// Try to claim an issue for an agent. Returns true if the claim succeeded,
/// false if another agent already holds it.
pub fn try_claim(repo: &str, issue: u32, agent_id: &str) -> Result<bool> {
    let conn = open_db()?;
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // Only block if there's an active claim by a *different* agent
    let existing: Option<String> = conn
        .query_row(
            "SELECT agent_id FROM claims WHERE repo = ?1 AND issue = ?2 AND status = 'active'",
            params![repo, issue],
            |row| row.get(0),
        )
        .ok();

    if let Some(ref holder) = existing {
        if holder == agent_id {
            return Ok(true); // idempotent: already ours
        }
        return Ok(false); // someone else holds it
    }

    // Upsert: insert new or replace a released/completed claim
    conn.execute(
        "INSERT INTO claims (repo, issue, agent_id, claimed_at, status)
         VALUES (?1, ?2, ?3, ?4, 'active')
         ON CONFLICT(repo, issue) DO UPDATE SET
           agent_id = excluded.agent_id,
           claimed_at = excluded.claimed_at,
           status = 'active',
           released_at = NULL
         WHERE claims.status != 'active'",
        params![repo, issue, agent_id, now],
    )
    .context("upsert claim")?;

    // Verify we actually hold the claim (race guard)
    let holder: String = conn
        .query_row(
            "SELECT agent_id FROM claims WHERE repo = ?1 AND issue = ?2 AND status = 'active'",
            params![repo, issue],
            |row| row.get(0),
        )
        .context("verify claim")?;

    Ok(holder == agent_id)
}

/// Release a claim (agent finished or gave up).
pub fn release(repo: &str, issue: u32, status: &str) -> Result<bool> {
    let conn = open_db()?;
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let final_status = match status {
        "completed" | "failed" | "released" => status,
        _ => "released",
    };
    let changed = conn
        .execute(
            "UPDATE claims SET status = ?1, released_at = ?2
             WHERE repo = ?3 AND issue = ?4 AND status = 'active'",
            params![final_status, now, repo, issue],
        )
        .context("release claim")?;
    Ok(changed > 0)
}

/// List all claims for a repo (or all repos if None).
pub fn list(repo: Option<&str>, active_only: bool) -> Result<Vec<Claim>> {
    let conn = open_db()?;
    let mut claims = Vec::new();

    let (sql, use_repo) = match (repo, active_only) {
        (Some(_), true) => (
            "SELECT repo, issue, agent_id, claimed_at, status, released_at
             FROM claims WHERE repo = ?1 AND status = 'active' ORDER BY claimed_at DESC",
            true,
        ),
        (Some(_), false) => (
            "SELECT repo, issue, agent_id, claimed_at, status, released_at
             FROM claims WHERE repo = ?1 ORDER BY claimed_at DESC",
            true,
        ),
        (None, true) => (
            "SELECT repo, issue, agent_id, claimed_at, status, released_at
             FROM claims WHERE status = 'active' ORDER BY claimed_at DESC",
            false,
        ),
        (None, false) => (
            "SELECT repo, issue, agent_id, claimed_at, status, released_at
             FROM claims ORDER BY claimed_at DESC",
            false,
        ),
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = if use_repo {
        stmt.query_map(params![repo.unwrap_or("")], read_claim)?
    } else {
        stmt.query_map([], read_claim)?
    };

    for row in rows {
        claims.push(row?);
    }
    Ok(claims)
}

/// Check if an issue is currently claimed.
pub fn is_claimed(repo: &str, issue: u32) -> Result<Option<Claim>> {
    let conn = open_db()?;
    conn.query_row(
        "SELECT repo, issue, agent_id, claimed_at, status, released_at
         FROM claims WHERE repo = ?1 AND issue = ?2 AND status = 'active'",
        params![repo, issue],
        read_claim,
    )
    .ok()
    .map(Ok)
    .transpose()
}

/// Force-release all active claims for a given agent (e.g., agent crashed).
pub fn release_agent(agent_id: &str) -> Result<u32> {
    let conn = open_db()?;
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let changed = conn
        .execute(
            "UPDATE claims SET status = 'released', released_at = ?1
             WHERE agent_id = ?2 AND status = 'active'",
            params![now, agent_id],
        )
        .context("release agent claims")?;
    Ok(changed as u32)
}

/// Purge old non-active claims older than N days.
pub fn purge(days: u32) -> Result<u32> {
    let conn = open_db()?;
    let cutoff = (Utc::now() - chrono::Duration::days(days as i64))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let changed = conn
        .execute(
            "DELETE FROM claims WHERE status != 'active' AND claimed_at <= ?1",
            params![cutoff],
        )
        .context("purge old claims")?;
    Ok(changed as u32)
}

fn read_claim(row: &rusqlite::Row) -> rusqlite::Result<Claim> {
    Ok(Claim {
        repo: row.get(0)?,
        issue: row.get::<_, u32>(1)?,
        agent_id: row.get(2)?,
        claimed_at: row.get(3)?,
        status: row.get(4)?,
        released_at: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let guard = crate::queue::tests::env_lock();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("DX_ROOT", tmp.path());
        std::fs::create_dir_all(tmp.path()).unwrap();
        (guard, tmp)
    }

    #[test]
    fn claim_and_release() {
        let (_g, _d) = setup();
        assert!(try_claim("pdaxt/dx-terminal", 4, "agent-1").unwrap());
        // Same agent can re-claim (idempotent)
        assert!(try_claim("pdaxt/dx-terminal", 4, "agent-1").unwrap());
        // Different agent cannot claim
        assert!(!try_claim("pdaxt/dx-terminal", 4, "agent-2").unwrap());

        // Release
        assert!(release("pdaxt/dx-terminal", 4, "completed").unwrap());
        // Now agent-2 can claim
        assert!(try_claim("pdaxt/dx-terminal", 4, "agent-2").unwrap());
    }

    #[test]
    fn list_filters_correctly() {
        let (_g, _d) = setup();
        try_claim("owner/repo", 1, "a1").unwrap();
        try_claim("owner/repo", 2, "a2").unwrap();
        try_claim("owner/other", 3, "a3").unwrap();
        release("owner/repo", 1, "completed").unwrap();

        let active = list(Some("owner/repo"), true).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].issue, 2);

        let all = list(Some("owner/repo"), false).unwrap();
        assert_eq!(all.len(), 2);

        let global = list(None, true).unwrap();
        assert_eq!(global.len(), 2); // issue 2 + issue 3
    }

    #[test]
    fn is_claimed_check() {
        let (_g, _d) = setup();
        assert!(is_claimed("r/r", 1).unwrap().is_none());
        try_claim("r/r", 1, "a").unwrap();
        let claim = is_claimed("r/r", 1).unwrap().unwrap();
        assert_eq!(claim.agent_id, "a");
    }

    #[test]
    fn release_agent_clears_all() {
        let (_g, _d) = setup();
        try_claim("r/a", 1, "agent-x").unwrap();
        try_claim("r/b", 2, "agent-x").unwrap();
        try_claim("r/a", 3, "agent-y").unwrap();

        let released = release_agent("agent-x").unwrap();
        assert_eq!(released, 2);

        // agent-y's claim is untouched
        assert!(is_claimed("r/a", 3).unwrap().is_some());
    }

    #[test]
    fn purge_removes_old_claims() {
        let (_g, _d) = setup();
        try_claim("r/r", 1, "a").unwrap();
        release("r/r", 1, "completed").unwrap();
        // Purging with 0 days should remove it (it was just claimed/released)
        let removed = purge(0).unwrap();
        assert_eq!(removed, 1);
    }
}
