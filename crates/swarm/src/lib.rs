//! # dx-swarm — Multi-Agent Coordination Framework
//!
//! SQLite-backed coordination for concurrent AI agents. Provides:
//! - **Port registry** — allocate/release ports, avoid conflicts
//! - **Agent lifecycle** — register, heartbeat, deregister with session tracking
//! - **File locks** — atomic acquire/release, conflict detection, steal with justification
//! - **Git coordination** — branch claiming, pre-commit safety checks
//! - **Build coordination** — exclusive build access per project
//! - **Task queue** — priority-based inter-agent task distribution
//! - **Knowledge base** — shared discoveries with ring-buffer trimming
//! - **Messaging** — broadcast + direct messages with read tracking
//! - **Signals** — agent-to-control-plane notifications
//!
//! ## Quick Start
//!
//! ```rust
//! use dx_swarm::SwarmDb;
//! use std::path::PathBuf;
//!
//! let db = SwarmDb::open_in_memory().unwrap();
//! let result = db.port_allocate("web-server", "pane1", Some(3001), "Dev server");
//! assert_eq!(result["status"], "allocated");
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

// ============================================================================
// CONSTANTS
// ============================================================================

const DEFAULT_PORT_MIN: u16 = 3001;
const DEFAULT_PORT_MAX: u16 = 3099;
const MAX_KB_ENTRIES: i64 = 500;
const MAX_MESSAGES: i64 = 200;
const MAX_BUILD_HISTORY: i64 = 50;

const COORDINATION_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS ports (
    port         INTEGER PRIMARY KEY,
    service      TEXT NOT NULL UNIQUE,
    pane_id      TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    allocated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ports_pane ON ports(pane_id);

CREATE TABLE IF NOT EXISTS agents (
    pane_id       TEXT PRIMARY KEY,
    project       TEXT NOT NULL,
    task          TEXT NOT NULL DEFAULT '',
    files         TEXT NOT NULL DEFAULT '[]',
    registered_at TEXT NOT NULL,
    last_update   TEXT NOT NULL,
    display_name  TEXT NOT NULL DEFAULT '',
    role          TEXT NOT NULL DEFAULT 'agent',
    status        TEXT NOT NULL DEFAULT 'active',
    session_id    TEXT,
    last_heartbeat TEXT,
    last_tool_call TEXT,
    deregistered_at TEXT,
    metadata      TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_agents_project ON agents(project);

CREATE TABLE IF NOT EXISTS file_locks (
    file_path   TEXT PRIMARY KEY,
    pane_id     TEXT NOT NULL REFERENCES agents(pane_id) ON DELETE CASCADE,
    reason      TEXT NOT NULL DEFAULT '',
    acquired_at TEXT NOT NULL,
    expires_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_locks_pane ON file_locks(pane_id);

CREATE TABLE IF NOT EXISTS git_branches (
    repo_branch TEXT PRIMARY KEY,
    pane_id     TEXT NOT NULL,
    purpose     TEXT NOT NULL DEFAULT '',
    claimed_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_git_pane ON git_branches(pane_id);

CREATE TABLE IF NOT EXISTS builds_active (
    project    TEXT PRIMARY KEY,
    pane_id    TEXT NOT NULL,
    build_type TEXT NOT NULL DEFAULT 'default',
    started_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS builds_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    project      TEXT NOT NULL,
    pane_id      TEXT NOT NULL,
    build_type   TEXT NOT NULL DEFAULT 'default',
    started_at   TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    success      INTEGER NOT NULL DEFAULT 0,
    output       TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_builds_hist_project ON builds_history(project);

CREATE TABLE IF NOT EXISTS tasks (
    id           TEXT PRIMARY KEY,
    project      TEXT NOT NULL,
    title        TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    priority     TEXT NOT NULL DEFAULT 'medium',
    status       TEXT NOT NULL DEFAULT 'pending',
    added_by     TEXT NOT NULL DEFAULT '',
    claimed_by   TEXT,
    added_at     TEXT NOT NULL,
    claimed_at   TEXT,
    completed_at TEXT,
    result       TEXT
);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project);

CREATE TABLE IF NOT EXISTS kb_entries (
    id       TEXT PRIMARY KEY,
    pane_id  TEXT NOT NULL,
    project  TEXT NOT NULL,
    category TEXT NOT NULL,
    title    TEXT NOT NULL,
    content  TEXT NOT NULL,
    files    TEXT NOT NULL DEFAULT '[]',
    added_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kb_project ON kb_entries(project);
CREATE INDEX IF NOT EXISTS idx_kb_added ON kb_entries(added_at);

CREATE TABLE IF NOT EXISTS messages (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    from_pane TEXT NOT NULL,
    to_pane   TEXT NOT NULL,
    message   TEXT NOT NULL,
    priority  TEXT NOT NULL DEFAULT 'info',
    timestamp TEXT NOT NULL,
    read_by   TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_messages_to ON messages(to_pane);
CREATE INDEX IF NOT EXISTS idx_messages_ts ON messages(timestamp);

CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id    TEXT PRIMARY KEY,
    pane_id       TEXT NOT NULL,
    project       TEXT NOT NULL DEFAULT '',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    tool_calls    INTEGER NOT NULL DEFAULT 0,
    errors        INTEGER NOT NULL DEFAULT 0,
    files_touched INTEGER NOT NULL DEFAULT 0,
    commits       INTEGER NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'active',
    summary       TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_sessions_pane ON sessions(pane_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);

CREATE TABLE IF NOT EXISTS tool_calls (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT,
    pane_id      TEXT NOT NULL,
    project      TEXT NOT NULL DEFAULT '',
    tool_name    TEXT NOT NULL,
    mcp_name     TEXT,
    tool_short   TEXT,
    input_size   INTEGER NOT NULL DEFAULT 0,
    output_size  INTEGER NOT NULL DEFAULT 0,
    latency_ms   INTEGER NOT NULL DEFAULT 0,
    success      INTEGER NOT NULL DEFAULT 1,
    error_preview TEXT,
    timestamp    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tc_pane ON tool_calls(pane_id);
CREATE INDEX IF NOT EXISTS idx_tc_ts ON tool_calls(timestamp);
CREATE INDEX IF NOT EXISTS idx_tc_tool ON tool_calls(tool_name);

CREATE TABLE IF NOT EXISTS file_operations (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT,
    pane_id      TEXT NOT NULL,
    project      TEXT NOT NULL DEFAULT '',
    file_path    TEXT NOT NULL,
    operation    TEXT NOT NULL,
    lines_changed INTEGER NOT NULL DEFAULT 0,
    timestamp    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_fops_pane ON file_operations(pane_id);
CREATE INDEX IF NOT EXISTS idx_fops_ts ON file_operations(timestamp);

CREATE TABLE IF NOT EXISTS token_usage (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id    TEXT,
    pane_id       TEXT NOT NULL,
    project       TEXT NOT NULL DEFAULT '',
    model         TEXT NOT NULL,
    input_tokens  INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read    INTEGER NOT NULL DEFAULT 0,
    cache_write   INTEGER NOT NULL DEFAULT 0,
    cost_usd      REAL NOT NULL DEFAULT 0.0,
    timestamp     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tokens_pane ON token_usage(pane_id);
CREATE INDEX IF NOT EXISTS idx_tokens_ts ON token_usage(timestamp);

CREATE TABLE IF NOT EXISTS quality_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT,
    pane_id      TEXT NOT NULL,
    project      TEXT NOT NULL DEFAULT '',
    event_type   TEXT NOT NULL,
    command      TEXT NOT NULL DEFAULT '',
    success      INTEGER NOT NULL DEFAULT 1,
    total_count  INTEGER NOT NULL DEFAULT 0,
    pass_count   INTEGER NOT NULL DEFAULT 0,
    fail_count   INTEGER NOT NULL DEFAULT 0,
    skip_count   INTEGER NOT NULL DEFAULT 0,
    duration_ms  INTEGER NOT NULL DEFAULT 0,
    output       TEXT NOT NULL DEFAULT '',
    timestamp    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_qe_project ON quality_events(project);
CREATE INDEX IF NOT EXISTS idx_qe_type ON quality_events(event_type);

CREATE TABLE IF NOT EXISTS git_commits (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id    TEXT,
    pane_id       TEXT NOT NULL,
    project       TEXT NOT NULL DEFAULT '',
    repo_path     TEXT NOT NULL DEFAULT '',
    commit_hash   TEXT NOT NULL DEFAULT '',
    branch        TEXT NOT NULL DEFAULT '',
    message       TEXT NOT NULL DEFAULT '',
    files_changed INTEGER NOT NULL DEFAULT 0,
    insertions    INTEGER NOT NULL DEFAULT 0,
    deletions     INTEGER NOT NULL DEFAULT 0,
    timestamp     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_gc_project ON git_commits(project);
CREATE INDEX IF NOT EXISTS idx_gc_ts ON git_commits(timestamp);

CREATE TABLE IF NOT EXISTS task_deps (
    task_id    TEXT NOT NULL,
    depends_on TEXT NOT NULL,
    PRIMARY KEY (task_id, depends_on)
);

CREATE TABLE IF NOT EXISTS agent_signals (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    pane_id      TEXT NOT NULL,
    signal_type  TEXT NOT NULL,
    message      TEXT NOT NULL DEFAULT '',
    pipeline_id  TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_signals_ack ON agent_signals(acknowledged);
"#;

// ============================================================================
// HELPERS
// ============================================================================

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn is_port_in_use(port: u16) -> (bool, Option<String>) {
    if let Ok(output) = Command::new("lsof")
        .args(["-i", &format!(":{}", port), "-t"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if output.status.success() && !stdout.is_empty() {
            let pid = stdout.lines().next().unwrap_or("").to_string();
            return (true, Some(pid));
        }
    }
    (false, None)
}

fn is_pane_active(pane_id: &str) -> bool {
    if let Ok(output) = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{session_name}:#{window_index}.#{pane_index}"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.lines().any(|l| l.trim() == pane_id);
    }
    false
}

fn gen_short_id(seed: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    now_iso().hash(&mut h);
    format!("{:08x}", h.finish() as u32)
}

/// Collect all active tmux panes in one call.
fn active_panes() -> Vec<String> {
    if let Ok(output) = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{session_name}:#{window_index}.#{pane_index}"])
        .output()
    {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .collect();
        }
    }
    vec![]
}

// ============================================================================
// SWARM DB — the main entry point
// ============================================================================

/// Multi-agent coordination database.
///
/// All coordination state is stored in a single SQLite database.
/// Create one per project/workspace and share the path across agents.
pub struct SwarmDb {
    conn: Connection,
}

impl SwarmDb {
    /// Open (or create) a coordination database at the given directory.
    /// The database file will be `<dir>/coordination.db`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join("coordination.db");
        Self::open_file(&path)
    }

    /// Open a coordination database from an exact file path.
    pub fn open_file(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("DB open: {}", e))?;
        Self::init_conn(conn)
    }

    /// Create an in-memory coordination database (for testing).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("DB open: {}", e))?;
        Self::init_conn(conn)
    }

    fn init_conn(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;"
        ).map_err(|e| format!("DB pragma: {}", e))?;
        conn.execute_batch(COORDINATION_SCHEMA)
            .map_err(|e| format!("DB schema: {}", e))?;
        Ok(SwarmDb { conn })
    }

    // ========================================================================
    // PORT REGISTRY
    // ========================================================================

    /// Allocate a port for a service. Returns existing allocation if service already has one.
    pub fn port_allocate(&self, service: &str, pane_id: &str, preferred: Option<u16>, description: &str) -> Value {
        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        // Check if service already has a port
        let existing: Option<i64> = tx.query_row(
            "SELECT port FROM ports WHERE service = ?1", params![service], |r| r.get(0)
        ).ok();

        if let Some(port) = existing {
            let (in_use, pid) = is_port_in_use(port as u16);
            if in_use {
                return json!({"status": "exists", "port": port, "pid": pid});
            }
            let _ = tx.execute(
                "UPDATE ports SET pane_id = ?1, description = ?2, allocated_at = ?3 WHERE port = ?4",
                params![pane_id, description, now_iso(), port]
            );
            let _ = tx.commit();
            return json!({"status": "allocated", "port": port, "service": service});
        }

        // Find a free port
        let allocated_ports: Vec<i64> = {
            let mut stmt = match tx.prepare("SELECT port FROM ports") {
                Ok(s) => s,
                Err(e) => return json!({"error": format!("Query: {}", e)}),
            };
            match stmt.query_map([], |r| r.get(0)) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => return json!({"error": format!("Query: {}", e)}),
            }
        };

        let mut port: Option<u16> = None;

        if let Some(pref) = preferred {
            if !allocated_ports.contains(&(pref as i64)) {
                let (in_use, _) = is_port_in_use(pref);
                if !in_use { port = Some(pref); }
            }
        }

        if port.is_none() {
            for p in DEFAULT_PORT_MIN..=DEFAULT_PORT_MAX {
                if !allocated_ports.contains(&(p as i64)) {
                    let (in_use, _) = is_port_in_use(p);
                    if !in_use {
                        port = Some(p);
                        break;
                    }
                }
            }
        }

        let Some(port) = port else {
            return json!({"error": "No free ports available"});
        };

        let _ = tx.execute(
            "INSERT INTO ports (port, service, pane_id, description, allocated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![port as i64, service, pane_id, description, now_iso()]
        );
        let _ = tx.commit();
        json!({"status": "allocated", "port": port, "service": service})
    }

    /// Release a port allocation.
    pub fn port_release(&self, port: u16) -> Value {
        let service: Option<String> = self.conn.query_row(
            "SELECT service FROM ports WHERE port = ?1", params![port as i64], |r| r.get(0)
        ).ok();
        let rows = self.conn.execute("DELETE FROM ports WHERE port = ?1", params![port as i64]).unwrap_or(0);
        if rows > 0 {
            json!({"status": "released", "port": port, "service": service})
        } else {
            json!({"status": "not_found"})
        }
    }

    /// List all allocated ports.
    pub fn port_list(&self) -> Value {
        let mut stmt = match self.conn.prepare("SELECT port, service, pane_id FROM ports") {
            Ok(s) => s,
            Err(e) => return json!({"error": format!("Query: {}", e)}),
        };
        let mut result = vec![];
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        }) {
            for row in rows.flatten() {
                let (port, service, pane_id) = row;
                let (active, pid) = is_port_in_use(port as u16);
                result.push(json!({
                    "port": port, "service": service,
                    "pane_id": pane_id, "active": active, "pid": pid
                }));
            }
        }
        json!({"ports": result})
    }

    /// Look up the port allocated to a service.
    pub fn port_get(&self, service: &str) -> Value {
        let row: Result<i64, _> = self.conn.query_row(
            "SELECT port FROM ports WHERE service = ?1", params![service], |r| r.get(0)
        );
        match row {
            Ok(port) => {
                let (active, pid) = is_port_in_use(port as u16);
                json!({"found": true, "port": port, "active": active, "pid": pid})
            }
            Err(_) => json!({"found": false}),
        }
    }

    // ========================================================================
    // AGENT COORDINATION
    // ========================================================================

    /// Register an agent. Upserts on pane_id. Returns other agents on same project.
    pub fn agent_register(&self, pane_id: &str, project: &str, task: &str, files: &[String]) -> Value {
        let now = now_iso();
        let files_json = serde_json::to_string(files).unwrap_or_else(|_| "[]".into());
        let session_id = uuid::Uuid::new_v4().to_string();

        let _ = self.conn.execute(
            "INSERT INTO agents (pane_id, project, task, files, registered_at, last_update, session_id, last_heartbeat, status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active') \
             ON CONFLICT(pane_id) DO UPDATE SET project=?2, task=?3, files=?4, last_update=?6, session_id=?7, last_heartbeat=?8, status='active', deregistered_at=NULL",
            params![pane_id, project, task, files_json, now, now, session_id, now]
        );

        let _ = self.conn.execute(
            "INSERT INTO sessions (session_id, pane_id, project, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, pane_id, project, now]
        );

        let mut others = vec![];
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT pane_id, task FROM agents WHERE project = ?1 AND pane_id != ?2 AND status = 'active'"
        ) {
            if let Ok(rows) = stmt.query_map(params![project, pane_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    others.push(json!({"pane": row.0, "task": row.1}));
                }
            }
        }
        json!({"status": "registered", "session_id": session_id, "other_agents": others})
    }

    /// Update an agent's task and optionally its file list.
    pub fn agent_update(&self, pane_id: &str, task: &str, files: Option<&[String]>) -> Value {
        let rows = if let Some(f) = files {
            let files_json = serde_json::to_string(f).unwrap_or_else(|_| "[]".into());
            self.conn.execute(
                "UPDATE agents SET task = ?1, files = ?2, last_update = ?3 WHERE pane_id = ?4",
                params![task, files_json, now_iso(), pane_id]
            ).unwrap_or(0)
        } else {
            self.conn.execute(
                "UPDATE agents SET task = ?1, last_update = ?2 WHERE pane_id = ?3",
                params![task, now_iso(), pane_id]
            ).unwrap_or(0)
        };
        if rows > 0 { json!({"status": "updated"}) } else { json!({"status": "not_found"}) }
    }

    /// List all registered agents, optionally filtered by project.
    pub fn agent_list(&self, project: Option<&str>) -> Value {
        let mut result = vec![];
        let query = if project.is_some() {
            "SELECT pane_id, project, task, files, last_update FROM agents WHERE project = ?1"
        } else {
            "SELECT pane_id, project, task, files, last_update FROM agents"
        };

        let mut stmt = match self.conn.prepare(query) {
            Ok(s) => s,
            Err(e) => return json!({"error": format!("Query: {}", e)}),
        };

        let extract = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String, String, String)> {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        };
        let rows_result = if let Some(p) = project {
            stmt.query_map(params![p], extract)
        } else {
            stmt.query_map([], extract)
        };

        if let Ok(rows) = rows_result {
            for row in rows.flatten() {
                let files_val: Value = serde_json::from_str(&row.3).unwrap_or(json!([]));
                result.push(json!({
                    "pane_id": row.0, "project": row.1,
                    "task": row.2, "files": files_val,
                    "active": is_pane_active(&row.0),
                    "last_update": row.4
                }));
            }
        }
        json!({"agents": result})
    }

    /// Deregister an agent. Releases its locks, ports, and branches.
    pub fn agent_deregister(&self, pane_id: &str) -> Value {
        let now = now_iso();

        let session_id: Option<String> = self.conn.query_row(
            "SELECT session_id FROM agents WHERE pane_id = ?1", params![pane_id], |r| r.get(0)
        ).ok();

        if let Some(ref sid) = session_id {
            let _ = self.conn.execute(
                "UPDATE sessions SET ended_at = ?1, status = 'ended', \
                 duration_secs = CAST((julianday(?1) - julianday(started_at)) * 86400 AS INTEGER) \
                 WHERE session_id = ?2 AND status = 'active'",
                params![now, sid]
            );
        }

        let rows = self.conn.execute(
            "UPDATE agents SET status = 'deregistered', deregistered_at = ?1 WHERE pane_id = ?2",
            params![now, pane_id]
        ).unwrap_or(0);

        let _ = self.conn.execute("DELETE FROM file_locks WHERE pane_id = ?1", params![pane_id]);
        let _ = self.conn.execute("DELETE FROM ports WHERE pane_id = ?1", params![pane_id]);
        let _ = self.conn.execute("DELETE FROM git_branches WHERE pane_id = ?1", params![pane_id]);

        if rows > 0 { json!({"status": "deregistered"}) } else { json!({"status": "not_found"}) }
    }

    // ========================================================================
    // LIFECYCLE
    // ========================================================================

    /// Update agent heartbeat.
    pub fn heartbeat(&self, pane_id: &str, task: Option<&str>, status: Option<&str>) -> Value {
        let now = now_iso();
        let rows = if let Some(t) = task {
            self.conn.execute(
                "UPDATE agents SET last_heartbeat = ?1, last_update = ?1, task = ?2 WHERE pane_id = ?3 AND status = 'active'",
                params![now, t, pane_id]
            ).unwrap_or(0)
        } else {
            self.conn.execute(
                "UPDATE agents SET last_heartbeat = ?1, last_update = ?1 WHERE pane_id = ?2 AND status = 'active'",
                params![now, pane_id]
            ).unwrap_or(0)
        };

        if let Some(s) = status {
            let _ = self.conn.execute(
                "UPDATE agents SET status = ?1 WHERE pane_id = ?2",
                params![s, pane_id]
            );
        }

        if rows > 0 { json!({"status": "ok", "heartbeat": now}) } else { json!({"error": "agent not found or not active"}) }
    }

    /// Start a new tracking session for an agent.
    pub fn session_start(&self, pane_id: &str, project: &str) -> Value {
        let now = now_iso();
        let session_id = uuid::Uuid::new_v4().to_string();

        let _ = self.conn.execute(
            "INSERT INTO sessions (session_id, pane_id, project, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, pane_id, project, now]
        );
        let _ = self.conn.execute(
            "UPDATE agents SET session_id = ?1, last_heartbeat = ?2 WHERE pane_id = ?3",
            params![session_id, now, pane_id]
        );

        json!({"session_id": session_id, "started_at": now})
    }

    /// End a tracking session with summary.
    pub fn session_end(&self, session_id: &str, summary: &str) -> Value {
        let now = now_iso();
        let rows = self.conn.execute(
            "UPDATE sessions SET ended_at = ?1, status = 'ended', summary = ?2, \
             duration_secs = CAST((julianday(?1) - julianday(started_at)) * 86400 AS INTEGER) \
             WHERE session_id = ?3 AND status = 'active'",
            params![now, summary, session_id]
        ).unwrap_or(0);

        if rows > 0 { json!({"status": "ended", "ended_at": now}) } else { json!({"error": "session not found or already ended"}) }
    }

    /// List all active agents (simple view).
    pub fn who(&self) -> Value {
        let mut agents = vec![];
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT pane_id, project, task, status, last_heartbeat, session_id FROM agents WHERE status IN ('active', 'busy', 'idle')"
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?, r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?, r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?,
                ))
            }) {
                for row in rows.flatten() {
                    agents.push(json!({
                        "pane_id": row.0, "project": row.1, "task": row.2,
                        "status": row.3, "last_heartbeat": row.4, "session_id": row.5,
                        "tmux_active": is_pane_active(&row.0)
                    }));
                }
            }
        }
        json!({"agents": agents, "count": agents.len()})
    }

    // ========================================================================
    // FILE LOCKS
    // ========================================================================

    /// Force-steal a lock with justification.
    pub fn lock_steal(&self, pane_id: &str, file_path: &str, reason: &str) -> Value {
        let now = now_iso();
        let prev: Option<(String, String)> = self.conn.query_row(
            "SELECT pane_id, reason FROM file_locks WHERE file_path = ?1",
            params![file_path], |r| Ok((r.get(0)?, r.get(1)?))
        ).ok();

        let _ = self.conn.execute("DELETE FROM file_locks WHERE file_path = ?1", params![file_path]);
        let _ = self.conn.execute(
            "INSERT INTO file_locks (file_path, pane_id, reason, acquired_at) VALUES (?1, ?2, ?3, ?4)",
            params![file_path, pane_id, reason, now]
        );

        json!({
            "status": "stolen",
            "file": file_path,
            "previous_holder": prev.as_ref().map(|p| &p.0),
            "previous_reason": prev.as_ref().map(|p| &p.1),
            "reason": reason
        })
    }

    /// Detect concurrent work on same files across agents.
    pub fn conflict_scan(&self, project: Option<&str>) -> Value {
        let mut conflicts = vec![];

        let query = if project.is_some() {
            "SELECT fl.file_path, fl.pane_id, fl.reason, a.project \
             FROM file_locks fl JOIN agents a ON fl.pane_id = a.pane_id \
             WHERE a.project = ?1 AND a.status = 'active'"
        } else {
            "SELECT fl.file_path, fl.pane_id, fl.reason, a.project \
             FROM file_locks fl JOIN agents a ON fl.pane_id = a.pane_id \
             WHERE a.status = 'active'"
        };
        if let Ok(mut stmt) = self.conn.prepare(query) {
            let extract = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String, String)> {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            };
            let rows = if let Some(p) = project {
                stmt.query_map(params![p], extract)
            } else {
                stmt.query_map([], extract)
            };
            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    conflicts.push(json!({"file": row.0, "holder": row.1, "reason": row.2, "project": row.3}));
                }
            }
        }

        let mut file_agents: HashMap<String, Vec<String>> = HashMap::new();
        let fa_query = if project.is_some() {
            "SELECT pane_id, files FROM agents WHERE status = 'active' AND project = ?1"
        } else {
            "SELECT pane_id, files FROM agents WHERE status = 'active'"
        };
        if let Ok(mut stmt) = self.conn.prepare(fa_query) {
            let extract = |r: &rusqlite::Row| -> rusqlite::Result<(String, String)> {
                Ok((r.get(0)?, r.get(1)?))
            };
            let rows = if let Some(p) = project {
                stmt.query_map(params![p], extract)
            } else {
                stmt.query_map([], extract)
            };
            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    if let Ok(files) = serde_json::from_str::<Vec<String>>(&row.1) {
                        for f in files {
                            file_agents.entry(f).or_default().push(row.0.clone());
                        }
                    }
                }
            }
        }

        let overlaps: Vec<Value> = file_agents.iter()
            .filter(|(_, agents)| agents.len() > 1)
            .map(|(f, agents)| json!({"file": f, "agents": agents}))
            .collect();

        json!({"locks": conflicts, "file_overlaps": overlaps})
    }

    /// Acquire file locks atomically. Fails if any file is locked by another agent.
    pub fn lock_acquire(&self, pane_id: &str, files: &[String], reason: &str) -> Value {
        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        let mut blocked = vec![];
        for f in files {
            let conflict: Option<(String, String)> = tx.query_row(
                "SELECT pane_id, reason FROM file_locks WHERE file_path = ?1 AND pane_id != ?2",
                params![f, pane_id], |r| Ok((r.get(0)?, r.get(1)?))
            ).ok();
            if let Some((owner, lock_reason)) = conflict {
                blocked.push(json!({"file": f, "locked_by": owner, "reason": lock_reason}));
            }
        }
        if !blocked.is_empty() {
            return json!({"status": "blocked", "blocked": blocked});
        }

        let now = now_iso();
        for f in files {
            let _ = tx.execute(
                "INSERT INTO file_locks (file_path, pane_id, reason, acquired_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(file_path) DO UPDATE SET pane_id=?2, reason=?3, acquired_at=?4",
                params![f, pane_id, reason, now]
            );
        }
        let _ = tx.commit();
        json!({"status": "acquired", "files": files})
    }

    /// Release file locks. Empty files slice releases all locks for this pane.
    pub fn lock_release(&self, pane_id: &str, files: &[String]) -> Value {
        let mut released = vec![];

        if files.is_empty() {
            if let Ok(mut stmt) = self.conn.prepare("SELECT file_path FROM file_locks WHERE pane_id = ?1") {
                if let Ok(rows) = stmt.query_map(params![pane_id], |r| r.get::<_, String>(0)) {
                    for row in rows.flatten() { released.push(row); }
                }
            }
            let _ = self.conn.execute("DELETE FROM file_locks WHERE pane_id = ?1", params![pane_id]);
        } else {
            for f in files {
                let rows = self.conn.execute(
                    "DELETE FROM file_locks WHERE file_path = ?1 AND pane_id = ?2",
                    params![f, pane_id]
                ).unwrap_or(0);
                if rows > 0 { released.push(f.clone()); }
            }
        }
        json!({"status": "released", "files": released})
    }

    /// Check which files are currently locked.
    pub fn lock_check(&self, files: &[String]) -> Value {
        let mut locked = vec![];
        for f in files {
            let row: Option<(String, String)> = self.conn.query_row(
                "SELECT pane_id, reason FROM file_locks WHERE file_path = ?1",
                params![f], |r| Ok((r.get(0)?, r.get(1)?))
            ).ok();
            if let Some((owner, reason)) = row {
                locked.push(json!({"file": f, "locked_by": owner, "reason": reason}));
            }
        }
        json!({"locked": locked, "clear": locked.is_empty()})
    }

    // ========================================================================
    // GIT COORDINATION
    // ========================================================================

    /// Claim a git branch for exclusive use.
    pub fn git_claim_branch(&self, pane_id: &str, branch: &str, repo: &str, purpose: &str) -> Value {
        let key = format!("{}:{}", repo, branch);

        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        let existing: Option<(String, String)> = tx.query_row(
            "SELECT pane_id, purpose FROM git_branches WHERE repo_branch = ?1",
            params![key], |r| Ok((r.get(0)?, r.get(1)?))
        ).ok();

        if let Some((owner, owner_purpose)) = existing {
            if owner != pane_id && is_pane_active(&owner) {
                return json!({"status": "claimed_by_other", "owner": owner, "purpose": owner_purpose});
            }
        }

        let _ = tx.execute(
            "INSERT INTO git_branches (repo_branch, pane_id, purpose, claimed_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(repo_branch) DO UPDATE SET pane_id=?2, purpose=?3, claimed_at=?4",
            params![key, pane_id, purpose, now_iso()]
        );
        let _ = tx.commit();
        json!({"status": "claimed", "branch": branch})
    }

    /// Release a git branch claim.
    pub fn git_release_branch(&self, pane_id: &str, branch: &str, repo: &str) -> Value {
        let key = format!("{}:{}", repo, branch);
        let owner: Option<String> = self.conn.query_row(
            "SELECT pane_id FROM git_branches WHERE repo_branch = ?1",
            params![key], |r| r.get(0)
        ).ok();

        match owner {
            Some(ref o) if o == pane_id => {
                let _ = self.conn.execute("DELETE FROM git_branches WHERE repo_branch = ?1", params![key]);
                json!({"status": "released"})
            }
            Some(_) => json!({"status": "not_owner"}),
            None => json!({"status": "not_found"}),
        }
    }

    /// List all claimed branches, optionally filtered by repo.
    pub fn git_list_branches(&self, repo: Option<&str>) -> Value {
        let mut result = vec![];
        if let Ok(mut stmt) = self.conn.prepare("SELECT repo_branch, pane_id, purpose FROM git_branches") {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            }) {
                for row in rows.flatten() {
                    let (key, owner, purpose) = row;
                    if let Some((r, b)) = key.rsplit_once(':') {
                        if let Some(filter) = repo {
                            if r != filter { continue; }
                        }
                        result.push(json!({
                            "repo": r, "branch": b,
                            "pane_id": owner, "purpose": purpose,
                            "active": is_pane_active(&owner)
                        }));
                    }
                }
            }
        }
        json!({"branches": result})
    }

    /// Pre-commit safety check: reports file lock conflicts and concurrent edits.
    pub fn git_pre_commit_check(&self, pane_id: &str, _repo: &str, files: &[String]) -> Value {
        let mut conflicts = vec![];

        for f in files {
            let row: Option<String> = self.conn.query_row(
                "SELECT pane_id FROM file_locks WHERE file_path = ?1 AND pane_id != ?2",
                params![f, pane_id], |r| r.get(0)
            ).ok();
            if let Some(owner) = row {
                conflicts.push(json!({"type": "file_lock", "file": f, "owner": owner}));
            }
        }

        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT pane_id, files FROM agents WHERE pane_id != ?1"
        ) {
            if let Ok(rows) = stmt.query_map(params![pane_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    let (other_pane, files_str) = row;
                    let agent_files: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
                    let overlap: Vec<&String> = files.iter()
                        .filter(|f| agent_files.iter().any(|af| af == *f))
                        .collect();
                    if !overlap.is_empty() {
                        conflicts.push(json!({"type": "concurrent_edit", "pane": other_pane, "files": overlap}));
                    }
                }
            }
        }

        json!({"safe": conflicts.is_empty(), "conflicts": conflicts})
    }

    // ========================================================================
    // BUILD COORDINATION
    // ========================================================================

    /// Claim exclusive build access for a project.
    pub fn build_claim(&self, pane_id: &str, project: &str, build_type: &str) -> Value {
        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        let existing: Option<(String, String)> = tx.query_row(
            "SELECT pane_id, started_at FROM builds_active WHERE project = ?1",
            params![project], |r| Ok((r.get(0)?, r.get(1)?))
        ).ok();

        if let Some((owner, started)) = existing {
            if is_pane_active(&owner) {
                return json!({"status": "busy", "owner": owner, "started": started});
            }
            let _ = tx.execute("DELETE FROM builds_active WHERE project = ?1", params![project]);
        }

        let _ = tx.execute(
            "INSERT INTO builds_active (project, pane_id, build_type, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![project, pane_id, build_type, now_iso()]
        );
        let _ = tx.commit();
        json!({"status": "claimed"})
    }

    /// Release build claim, recording result in history.
    pub fn build_release(&self, pane_id: &str, project: &str, success: bool, output: &str) -> Value {
        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        let active: Option<(String, String, String)> = tx.query_row(
            "SELECT pane_id, build_type, started_at FROM builds_active WHERE project = ?1",
            params![project], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        ).ok();

        match active {
            Some((owner, bt, started)) if owner == pane_id => {
                let _ = tx.execute(
                    "INSERT INTO builds_history (project, pane_id, build_type, started_at, completed_at, success, output) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![project, pane_id, bt, started, now_iso(), success as i32, output]
                );
                let _ = tx.execute(
                    "DELETE FROM builds_history WHERE id IN (\
                        SELECT id FROM builds_history ORDER BY id ASC \
                        LIMIT MAX(0, (SELECT COUNT(*) FROM builds_history) - ?1))",
                    params![MAX_BUILD_HISTORY]
                );
                let _ = tx.execute("DELETE FROM builds_active WHERE project = ?1", params![project]);
                let _ = tx.commit();
                json!({"status": "released"})
            }
            Some(_) => json!({"status": "not_owner"}),
            None => json!({"status": "not_found"}),
        }
    }

    /// Check if a project currently has an active build.
    pub fn build_status(&self, project: &str) -> Value {
        let row: Option<(String, String)> = self.conn.query_row(
            "SELECT pane_id, started_at FROM builds_active WHERE project = ?1",
            params![project], |r| Ok((r.get(0)?, r.get(1)?))
        ).ok();
        match row {
            Some((owner, started)) => json!({"building": true, "owner": owner, "started": started}),
            None => json!({"building": false}),
        }
    }

    /// Get the most recent build history entry for a project.
    pub fn build_get_last(&self, project: &str) -> Value {
        let row = self.conn.query_row(
            "SELECT pane_id, build_type, started_at, completed_at, success, output \
             FROM builds_history WHERE project = ?1 ORDER BY id DESC LIMIT 1",
            params![project],
            |r| Ok(json!({
                "pane_id": r.get::<_, String>(0)?,
                "build_type": r.get::<_, String>(1)?,
                "started_at": r.get::<_, String>(2)?,
                "completed_at": r.get::<_, String>(3)?,
                "success": r.get::<_, i32>(4)? != 0,
                "output": r.get::<_, String>(5)?,
                "project": project
            }))
        );
        match row {
            Ok(build) => json!({"found": true, "build": build}),
            Err(_) => json!({"found": false}),
        }
    }

    // ========================================================================
    // TASK QUEUE
    // ========================================================================

    /// Add a task to the inter-agent task queue.
    pub fn task_add(&self, project: &str, title: &str, description: &str, priority: &str, added_by: &str) -> Value {
        let task_id = gen_short_id(title);
        let now = now_iso();
        match self.conn.execute(
            "INSERT INTO tasks (id, project, title, description, priority, status, added_by, added_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)",
            params![task_id, project, title, description, priority, added_by, now]
        ) {
            Ok(_) => json!({"status": "added", "task_id": task_id}),
            Err(e) => json!({"error": format!("Insert: {}", e)}),
        }
    }

    /// Claim the highest-priority pending task.
    pub fn task_claim(&self, pane_id: &str, project: Option<&str>) -> Value {
        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        let query = if project.is_some() {
            "SELECT id, project, title, description, priority, added_by, added_at \
             FROM tasks WHERE status = 'pending' AND project = ?1 \
             ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 2 END, added_at ASC \
             LIMIT 1"
        } else {
            "SELECT id, project, title, description, priority, added_by, added_at \
             FROM tasks WHERE status = 'pending' \
             ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 2 END, added_at ASC \
             LIMIT 1"
        };

        let task_row = if let Some(p) = project {
            tx.query_row(query, params![p], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?))
            })
        } else {
            tx.query_row(query, [], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?))
            })
        };

        match task_row {
            Ok((id, proj, title, desc, priority, added_by, added_at)) => {
                let now = now_iso();
                let _ = tx.execute(
                    "UPDATE tasks SET status = 'claimed', claimed_by = ?1, claimed_at = ?2 WHERE id = ?3",
                    params![pane_id, now, id]
                );
                let _ = tx.commit();
                json!({
                    "status": "claimed",
                    "task": {
                        "id": id, "project": proj, "title": title,
                        "description": desc, "priority": priority,
                        "status": "claimed", "added_by": added_by,
                        "added_at": added_at, "claimed_by": pane_id, "claimed_at": now
                    }
                })
            }
            Err(_) => json!({"status": "empty"}),
        }
    }

    /// Mark a claimed task as completed.
    pub fn task_complete(&self, task_id: &str, pane_id: &str, result: &str) -> Value {
        let owner: Option<String> = self.conn.query_row(
            "SELECT claimed_by FROM tasks WHERE id = ?1",
            params![task_id], |r| r.get(0)
        ).ok().flatten();

        match owner {
            None => json!({"status": "not_found"}),
            Some(ref o) if o != pane_id => json!({"status": "not_owner"}),
            Some(_) => {
                let _ = self.conn.execute(
                    "UPDATE tasks SET status = 'completed', completed_at = ?1, result = ?2 WHERE id = ?3",
                    params![now_iso(), result, task_id]
                );
                json!({"status": "completed"})
            }
        }
    }

    /// List tasks, optionally filtered by status and/or project.
    pub fn task_list(&self, status: Option<&str>, project: Option<&str>) -> Value {
        let mut result = vec![];
        let mut conditions = vec![];
        let mut param_values: Vec<String> = vec![];

        if let Some(s) = status {
            if s != "all" {
                conditions.push(format!("status = ?{}", param_values.len() + 1));
                param_values.push(s.to_string());
            }
        }
        if let Some(p) = project {
            conditions.push(format!("project = ?{}", param_values.len() + 1));
            param_values.push(p.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT id, project, title, description, priority, status, added_by, claimed_by, added_at, claimed_at, completed_at, result FROM tasks{}",
            where_clause
        );

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => return json!({"error": format!("Query: {}", e)}),
        };

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();

        if let Ok(rows) = stmt.query_map(params_refs.as_slice(), |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "project": r.get::<_, String>(1)?,
                "title": r.get::<_, String>(2)?,
                "description": r.get::<_, String>(3)?,
                "priority": r.get::<_, String>(4)?,
                "status": r.get::<_, String>(5)?,
                "added_by": r.get::<_, String>(6)?,
                "claimed_by": r.get::<_, Option<String>>(7)?,
                "added_at": r.get::<_, String>(8)?,
                "claimed_at": r.get::<_, Option<String>>(9)?,
                "completed_at": r.get::<_, Option<String>>(10)?,
                "result": r.get::<_, Option<String>>(11)?
            }))
        }) {
            for row in rows.flatten() { result.push(row); }
        }
        json!({"tasks": result})
    }

    // ========================================================================
    // KNOWLEDGE BASE
    // ========================================================================

    /// Add a knowledge base entry. Ring-buffer trimmed to 500 entries.
    pub fn kb_add(&self, pane_id: &str, project: &str, category: &str, title: &str, content: &str, files: &[String]) -> Value {
        let entry_id = gen_short_id(title);
        let files_json = serde_json::to_string(files).unwrap_or_else(|_| "[]".into());
        let now = now_iso();

        let _ = self.conn.execute(
            "INSERT INTO kb_entries (id, pane_id, project, category, title, content, files, added_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![entry_id, pane_id, project, category, title, content, files_json, now]
        );
        let _ = self.conn.execute(
            "DELETE FROM kb_entries WHERE id IN (\
                SELECT id FROM kb_entries ORDER BY added_at ASC \
                LIMIT MAX(0, (SELECT COUNT(*) FROM kb_entries) - ?1))",
            params![MAX_KB_ENTRIES]
        );
        json!({"status": "added", "entry_id": entry_id})
    }

    /// Search knowledge base by text match.
    pub fn kb_search(&self, query: &str, project: Option<&str>, category: Option<&str>) -> Value {
        let pattern = format!("%{}%", query);
        let mut results = vec![];

        let mut conditions = vec!["(title LIKE ?1 OR content LIKE ?1)".to_string()];
        let mut param_values: Vec<String> = vec![pattern];

        if let Some(p) = project {
            conditions.push(format!("project = ?{}", param_values.len() + 1));
            param_values.push(p.to_string());
        }
        if let Some(c) = category {
            conditions.push(format!("category = ?{}", param_values.len() + 1));
            param_values.push(c.to_string());
        }

        let sql = format!(
            "SELECT id, pane_id, project, category, title, content, files, added_at FROM kb_entries WHERE {} ORDER BY added_at DESC LIMIT 20",
            conditions.join(" AND ")
        );

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => return json!({"error": format!("Query: {}", e)}),
        };
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();

        if let Ok(rows) = stmt.query_map(params_refs.as_slice(), |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "pane_id": r.get::<_, String>(1)?,
                "project": r.get::<_, String>(2)?,
                "category": r.get::<_, String>(3)?,
                "title": r.get::<_, String>(4)?,
                "content": r.get::<_, String>(5)?,
                "files": serde_json::from_str::<Value>(&r.get::<_, String>(6)?).unwrap_or(json!([])),
                "added_at": r.get::<_, String>(7)?
            }))
        }) {
            for row in rows.flatten() { results.push(row); }
        }
        json!({"results": results})
    }

    /// List recent knowledge base entries.
    pub fn kb_list(&self, project: Option<&str>, limit: usize) -> Value {
        let mut entries = vec![];

        let (sql, project_param);
        if let Some(p) = project {
            sql = "SELECT id, pane_id, project, category, title, content, files, added_at FROM kb_entries WHERE project = ?1 ORDER BY added_at DESC LIMIT ?2";
            project_param = Some(p.to_string());
        } else {
            sql = "SELECT id, pane_id, project, category, title, content, files, added_at FROM kb_entries ORDER BY added_at DESC LIMIT ?1";
            project_param = None;
        };

        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(e) => return json!({"error": format!("Query: {}", e)}),
        };

        let extract_kb = |r: &rusqlite::Row| -> rusqlite::Result<Value> {
            Ok(json!({
                "id": r.get::<_, String>(0)?, "pane_id": r.get::<_, String>(1)?,
                "project": r.get::<_, String>(2)?, "category": r.get::<_, String>(3)?,
                "title": r.get::<_, String>(4)?, "content": r.get::<_, String>(5)?,
                "files": serde_json::from_str::<Value>(&r.get::<_, String>(6)?).unwrap_or(json!([])),
                "added_at": r.get::<_, String>(7)?
            }))
        };
        let rows_result = if let Some(ref p) = project_param {
            stmt.query_map(params![p, limit as i64], extract_kb)
        } else {
            stmt.query_map(params![limit as i64], extract_kb)
        };

        if let Ok(rows) = rows_result {
            for row in rows.flatten() { entries.push(row); }
        }
        json!({"entries": entries})
    }

    // ========================================================================
    // MESSAGING
    // ========================================================================

    fn insert_message(&self, from_pane: &str, to_pane: &str, message: &str, priority: &str) {
        let _ = self.conn.execute(
            "INSERT INTO messages (from_pane, to_pane, message, priority, timestamp, read_by) VALUES (?1, ?2, ?3, ?4, ?5, '[]')",
            params![from_pane, to_pane, message, priority, now_iso()]
        );
        let _ = self.conn.execute(
            "DELETE FROM messages WHERE id IN (\
                SELECT id FROM messages ORDER BY id ASC \
                LIMIT MAX(0, (SELECT COUNT(*) FROM messages) - ?1))",
            params![MAX_MESSAGES]
        );
    }

    /// Broadcast a message to all agents.
    pub fn msg_broadcast(&self, from_pane: &str, message: &str, priority: &str) -> Value {
        self.insert_message(from_pane, "all", message, priority);
        json!({"status": "sent"})
    }

    /// Send a direct message to a specific agent.
    pub fn msg_send(&self, from_pane: &str, to_pane: &str, message: &str) -> Value {
        self.insert_message(from_pane, to_pane, message, "info");
        json!({"status": "sent"})
    }

    /// Get unread messages for an agent.
    pub fn msg_get(&self, pane_id: &str, mark_read: bool) -> Value {
        let mut unread = vec![];
        let pane_check = format!("\"{}\"", pane_id);
        let like_pattern = format!("%{}%", pane_check);

        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, from_pane, to_pane, message, priority, timestamp, read_by \
             FROM messages \
             WHERE from_pane != ?1 AND (to_pane = 'all' OR to_pane = ?1) \
             AND read_by NOT LIKE ?2 \
             ORDER BY id ASC"
        ) {
            if let Ok(rows) = stmt.query_map(params![pane_id, like_pattern], |r| {
                Ok((
                    r.get::<_, i64>(0)?, r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?, r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?, r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (id, from, to, msg, prio, ts, read_by_str) = row;
                    unread.push(json!({
                        "from": from, "to": to, "message": msg,
                        "priority": prio, "timestamp": ts
                    }));

                    if mark_read {
                        let mut read_by: Vec<String> = serde_json::from_str(&read_by_str).unwrap_or_default();
                        if !read_by.contains(&pane_id.to_string()) {
                            read_by.push(pane_id.to_string());
                            let new_read_by = serde_json::to_string(&read_by).unwrap_or_else(|_| "[]".into());
                            let _ = self.conn.execute(
                                "UPDATE messages SET read_by = ?1 WHERE id = ?2",
                                params![new_read_by, id]
                            );
                        }
                    }
                }
            }
        }
        json!({"messages": unread})
    }

    // ========================================================================
    // AGENT SIGNALS
    // ========================================================================

    /// Send a signal from an agent to the control plane.
    pub fn signal_send(&self, pane_id: &str, signal_type: &str, message: &str, pipeline_id: Option<&str>) -> Value {
        match self.conn.execute(
            "INSERT INTO agent_signals (pane_id, signal_type, message, pipeline_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pane_id, signal_type, message, pipeline_id, now_iso()],
        ) {
            Ok(_) => json!({"status": "signal_sent", "signal_type": signal_type, "pane_id": pane_id}),
            Err(e) => json!({"error": format!("Signal insert: {}", e)}),
        }
    }

    /// List signals, optionally only unacknowledged.
    pub fn signal_list(&self, unack_only: bool) -> Value {
        let sql = if unack_only {
            "SELECT id, pane_id, signal_type, message, pipeline_id, created_at FROM agent_signals WHERE acknowledged = 0 ORDER BY id DESC LIMIT 50"
        } else {
            "SELECT id, pane_id, signal_type, message, pipeline_id, created_at FROM agent_signals ORDER BY id DESC LIMIT 50"
        };
        let mut signals = vec![];
        if let Ok(mut stmt) = self.conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "pane_id": r.get::<_, String>(1)?,
                    "signal_type": r.get::<_, String>(2)?,
                    "message": r.get::<_, String>(3)?,
                    "pipeline_id": r.get::<_, Option<String>>(4)?,
                    "created_at": r.get::<_, String>(5)?,
                }))
            }) {
                for row in rows.flatten() { signals.push(row); }
            }
        }
        json!({"signals": signals, "count": signals.len()})
    }

    /// Acknowledge a signal.
    pub fn signal_acknowledge(&self, signal_id: i64) -> Value {
        match self.conn.execute("UPDATE agent_signals SET acknowledged = 1 WHERE id = ?1", params![signal_id]) {
            Ok(n) => json!({"status": "acknowledged", "id": signal_id, "updated": n}),
            Err(e) => json!({"error": format!("{}", e)}),
        }
    }

    /// Count unacknowledged signals.
    pub fn signal_count_unack(&self) -> usize {
        self.conn.query_row("SELECT COUNT(*) FROM agent_signals WHERE acknowledged = 0", [], |r| r.get::<_, usize>(0)).unwrap_or(0)
    }

    /// Get unacknowledged signals grouped by pane.
    pub fn signal_by_pane(&self) -> HashMap<u8, Vec<(String, String)>> {
        let mut map = HashMap::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT pane_id, signal_type, message FROM agent_signals WHERE acknowledged = 0 ORDER BY id DESC"
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            }) {
                for row in rows.flatten() {
                    let (pane_str, sig_type, msg) = row;
                    if let Ok(pane_num) = pane_str.parse::<u8>() {
                        map.entry(pane_num).or_insert_with(Vec::new).push((sig_type, msg));
                    }
                }
            }
        }
        map
    }

    // ========================================================================
    // CLEANUP
    // ========================================================================

    /// Remove stale entries where the owning tmux pane no longer exists.
    pub fn cleanup_all(&self) -> Value {
        let active = active_panes();
        let mut cleaned = json!({"ports": 0, "agents": 0, "locks": 0, "branches": 0, "builds": 0});

        let tx = match self.conn.unchecked_transaction() {
            Ok(t) => t,
            Err(e) => return json!({"error": format!("Transaction: {}", e)}),
        };

        // Clean ports
        let mut stale_ports = vec![];
        if let Ok(mut stmt) = tx.prepare("SELECT port, pane_id FROM ports") {
            let collected: Vec<_> = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for (port, pane_id) in collected {
                let (in_use, _) = is_port_in_use(port as u16);
                if !in_use && !active.contains(&pane_id) {
                    stale_ports.push(port);
                }
            }
        }
        for port in &stale_ports {
            let _ = tx.execute("DELETE FROM ports WHERE port = ?1", params![port]);
        }
        cleaned["ports"] = json!(stale_ports.len());

        // Clean agents
        let mut stale_agents = vec![];
        if let Ok(mut stmt) = tx.prepare("SELECT pane_id FROM agents") {
            let collected: Vec<_> = stmt.query_map([], |r| r.get::<_, String>(0))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for row in collected {
                if !active.contains(&row) { stale_agents.push(row); }
            }
        }
        let mut lock_count = 0i64;
        for pane_id in &stale_agents {
            let n: i64 = tx.query_row(
                "SELECT COUNT(*) FROM file_locks WHERE pane_id = ?1", params![pane_id], |r| r.get(0)
            ).unwrap_or(0);
            lock_count += n;
        }
        for pane_id in &stale_agents {
            let _ = tx.execute("DELETE FROM agents WHERE pane_id = ?1", params![pane_id]);
        }
        cleaned["agents"] = json!(stale_agents.len());
        cleaned["locks"] = json!(lock_count);

        let orphan_locks: i64 = tx.query_row(
            "SELECT COUNT(*) FROM file_locks WHERE pane_id NOT IN (SELECT pane_id FROM agents)",
            [], |r| r.get(0)
        ).unwrap_or(0);
        if orphan_locks > 0 {
            let _ = tx.execute(
                "DELETE FROM file_locks WHERE pane_id NOT IN (SELECT pane_id FROM agents)", []
            );
            cleaned["locks"] = json!(lock_count + orphan_locks);
        }

        // Clean git branches
        let mut stale_branches = vec![];
        if let Ok(mut stmt) = tx.prepare("SELECT repo_branch, pane_id FROM git_branches") {
            let collected: Vec<_> = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for row in collected {
                if !active.contains(&row.1) { stale_branches.push(row.0); }
            }
        }
        for key in &stale_branches {
            let _ = tx.execute("DELETE FROM git_branches WHERE repo_branch = ?1", params![key]);
        }
        cleaned["branches"] = json!(stale_branches.len());

        // Clean active builds
        let mut stale_builds = vec![];
        if let Ok(mut stmt) = tx.prepare("SELECT project, pane_id FROM builds_active") {
            let collected: Vec<_> = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for row in collected {
                if !active.contains(&row.1) { stale_builds.push(row.0); }
            }
        }
        for project in &stale_builds {
            let _ = tx.execute("DELETE FROM builds_active WHERE project = ?1", params![project]);
        }
        cleaned["builds"] = json!(stale_builds.len());

        let _ = tx.commit();
        json!({"cleaned": cleaned})
    }

    // ========================================================================
    // STATUS OVERVIEW
    // ========================================================================

    /// Dashboard view: ports, agents, locks, builds, and pending tasks.
    pub fn status_overview(&self, project: Option<&str>) -> Value {
        let mut port_list = vec![];
        if let Ok(mut stmt) = self.conn.prepare("SELECT port, service FROM ports") {
            let collected: Vec<_> = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for row in collected {
                let (active, _) = is_port_in_use(row.0 as u16);
                port_list.push(json!({"port": row.0, "service": row.1, "active": active}));
            }
        }

        let mut agent_list = vec![];
        let query = if project.is_some() {
            "SELECT pane_id, project, task FROM agents WHERE project = ?1"
        } else {
            "SELECT pane_id, project, task FROM agents"
        };
        if let Ok(mut stmt) = self.conn.prepare(query) {
            let extract = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String)> {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            };
            let rows_result = if let Some(p) = project {
                stmt.query_map(params![p], extract)
            } else {
                stmt.query_map([], extract)
            };
            if let Ok(rows) = rows_result {
                for row in rows.flatten() {
                    let short_task: String = row.2.chars().take(50).collect();
                    agent_list.push(json!({
                        "pane": row.0, "project": row.1,
                        "task": short_task, "active": is_pane_active(&row.0)
                    }));
                }
            }
        }

        let lock_count: i64 = self.conn.query_row("SELECT COUNT(*) FROM file_locks", [], |r| r.get(0)).unwrap_or(0);

        let mut active_builds = vec![];
        if let Ok(mut stmt) = self.conn.prepare("SELECT project FROM builds_active") {
            let collected: Vec<_> = stmt.query_map([], |r| r.get::<_, String>(0))
                .into_iter().flat_map(|rows| rows.flatten().collect::<Vec<_>>()).collect();
            for row in collected { active_builds.push(row); }
        }

        let pending_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'pending'", [], |r| r.get(0)
        ).unwrap_or(0);

        json!({
            "ports": port_list, "agents": agent_list,
            "locks": lock_count, "active_builds": active_builds,
            "pending_tasks": pending_tasks
        })
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> SwarmDb {
        SwarmDb::open_in_memory().unwrap()
    }

    #[test]
    fn test_port_allocation() {
        let db = test_db();
        let r = db.port_allocate("web", "p1", Some(3010), "test");
        assert_eq!(r["status"], "allocated");
        assert_eq!(r["port"], 3010);

        // Same service returns existing
        let r2 = db.port_allocate("web", "p2", Some(3011), "test");
        // Port 3010 is not in use (lsof won't find it), so it reclaims
        assert!(r2["status"] == "allocated" || r2["status"] == "exists");
    }

    #[test]
    fn test_port_release() {
        let db = test_db();
        db.port_allocate("api", "p1", Some(3020), "API server");
        let r = db.port_release(3020);
        assert_eq!(r["status"], "released");

        let r2 = db.port_release(9999);
        assert_eq!(r2["status"], "not_found");
    }

    #[test]
    fn test_port_list() {
        let db = test_db();
        db.port_allocate("svc1", "p1", Some(3030), "");
        db.port_allocate("svc2", "p2", Some(3031), "");
        let r = db.port_list();
        assert_eq!(r["ports"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_agent_lifecycle() {
        let db = test_db();
        let r = db.agent_register("p1", "myproject", "coding", &[]);
        assert_eq!(r["status"], "registered");
        assert!(r["session_id"].is_string());

        let r2 = db.agent_update("p1", "testing", None);
        assert_eq!(r2["status"], "updated");

        let r3 = db.agent_deregister("p1");
        assert_eq!(r3["status"], "deregistered");
    }

    #[test]
    fn test_lock_acquire_release() {
        let db = test_db();
        db.agent_register("p1", "proj", "", &[]);
        db.agent_register("p2", "proj", "", &[]);

        let r = db.lock_acquire("p1", &["src/main.rs".into()], "editing");
        assert_eq!(r["status"], "acquired");

        // p2 tries same file — blocked
        let r2 = db.lock_acquire("p2", &["src/main.rs".into()], "also editing");
        assert_eq!(r2["status"], "blocked");

        // p1 releases
        let r3 = db.lock_release("p1", &["src/main.rs".into()]);
        assert_eq!(r3["status"], "released");

        // Now p2 can acquire
        let r4 = db.lock_acquire("p2", &["src/main.rs".into()], "now editing");
        assert_eq!(r4["status"], "acquired");
    }

    #[test]
    fn test_lock_steal() {
        let db = test_db();
        db.agent_register("p1", "proj", "", &[]);
        db.agent_register("p2", "proj", "", &[]);
        db.lock_acquire("p1", &["file.rs".into()], "working");

        let r = db.lock_steal("p2", "file.rs", "urgent fix");
        assert_eq!(r["status"], "stolen");
        assert_eq!(r["previous_holder"], "p1");
    }

    #[test]
    fn test_task_queue() {
        let db = test_db();
        db.task_add("proj", "Low task", "", "low", "user");
        db.task_add("proj", "Urgent task", "", "urgent", "user");

        // Should claim urgent first
        let r = db.task_claim("p1", Some("proj"));
        assert_eq!(r["status"], "claimed");
        assert_eq!(r["task"]["title"], "Urgent task");

        // Complete it
        let task_id = r["task"]["id"].as_str().unwrap().to_string();
        let r2 = db.task_complete(&task_id, "p1", "done");
        assert_eq!(r2["status"], "completed");
    }

    #[test]
    fn test_kb_add_search() {
        let db = test_db();
        db.kb_add("p1", "proj", "code_location", "Auth flow", "OAuth in /lib/auth.ts", &[]);
        db.kb_add("p1", "proj", "bug", "CSS issue", "Flexbox alignment broken", &[]);

        let r = db.kb_search("OAuth", None, None);
        assert_eq!(r["results"].as_array().unwrap().len(), 1);
        assert_eq!(r["results"][0]["title"], "Auth flow");

        let r2 = db.kb_search("issue", Some("proj"), Some("bug"));
        assert_eq!(r2["results"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_messaging() {
        let db = test_db();
        db.msg_broadcast("p1", "deploy starting", "warn");
        db.msg_send("p1", "p2", "please review PR #5");

        // p2 should have 2 unread
        let r = db.msg_get("p2", false);
        assert_eq!(r["messages"].as_array().unwrap().len(), 2);

        // Mark as read
        let r2 = db.msg_get("p2", true);
        assert_eq!(r2["messages"].as_array().unwrap().len(), 2);

        // Now p2 has 0 unread
        let r3 = db.msg_get("p2", false);
        assert_eq!(r3["messages"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_signals() {
        let db = test_db();
        db.signal_send("p1", "task_complete", "finished build", None);
        db.signal_send("p2", "error", "test failed", Some("pipe_123"));

        assert_eq!(db.signal_count_unack(), 2);

        let r = db.signal_list(true);
        assert_eq!(r["count"], 2);

        // Acknowledge first signal
        let sig_id = r["signals"][0]["id"].as_i64().unwrap();
        db.signal_acknowledge(sig_id);
        assert_eq!(db.signal_count_unack(), 1);
    }

    #[test]
    fn test_build_coordination() {
        let db = test_db();
        let r = db.build_claim("p1", "myproj", "release");
        assert_eq!(r["status"], "claimed");

        let r2 = db.build_status("myproj");
        assert_eq!(r2["building"], true);

        let r3 = db.build_release("p1", "myproj", true, "ok");
        assert_eq!(r3["status"], "released");

        let r4 = db.build_get_last("myproj");
        assert_eq!(r4["found"], true);
        assert_eq!(r4["build"]["success"], true);
    }

    #[test]
    fn test_git_branch_claim() {
        let db = test_db();
        let r = db.git_claim_branch("p1", "feat-1", "dx-terminal", "new feature");
        assert_eq!(r["status"], "claimed");

        // Same pane reclaims — ok
        let r2 = db.git_claim_branch("p1", "feat-1", "dx-terminal", "updated purpose");
        assert_eq!(r2["status"], "claimed");

        // Release
        let r3 = db.git_release_branch("p1", "feat-1", "dx-terminal");
        assert_eq!(r3["status"], "released");
    }

    #[test]
    fn test_status_overview() {
        let db = test_db();
        db.port_allocate("web", "p1", Some(3040), "");
        db.agent_register("p1", "proj", "coding", &[]);
        db.task_add("proj", "task 1", "", "medium", "user");

        let r = db.status_overview(None);
        assert!(!r["ports"].as_array().unwrap().is_empty());
        assert!(!r["agents"].as_array().unwrap().is_empty());
        assert_eq!(r["pending_tasks"], 1);
    }

    #[test]
    fn test_who() {
        let db = test_db();
        db.agent_register("p1", "proj", "coding", &[]);
        let r = db.who();
        assert_eq!(r["count"], 1);
        assert_eq!(r["agents"][0]["pane_id"], "p1");
    }
}
