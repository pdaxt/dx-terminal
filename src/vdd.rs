//! VDD Kernel: Vision-Driven Development stage tracking with SQLite persistence.
//!
//! Tracks features through the VDD lifecycle:
//!   planned -> discovery -> design -> build -> test -> done
//!
//! Each transition is recorded with a timestamp, so we can answer:
//! - How long did feature X spend in each stage?
//! - Which features are stuck in design?
//! - What's the team velocity across projects?

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::vision::FeaturePhase;

// ─── Schema ─────────────────────────────────────────────────────────────────

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS vdd_features (
    id          TEXT NOT NULL,
    project     TEXT NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    goal_id     TEXT NOT NULL DEFAULT '',
    phase       TEXT NOT NULL DEFAULT 'planned',
    state       TEXT NOT NULL DEFAULT 'planned',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (id, project)
);

CREATE TABLE IF NOT EXISTS vdd_transitions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    feature_id  TEXT NOT NULL,
    project     TEXT NOT NULL,
    from_phase  TEXT NOT NULL,
    to_phase    TEXT NOT NULL,
    triggered_by TEXT NOT NULL DEFAULT 'system',
    reason      TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_transitions_feature
    ON vdd_transitions(feature_id, project);
CREATE INDEX IF NOT EXISTS idx_transitions_time
    ON vdd_transitions(created_at);
CREATE INDEX IF NOT EXISTS idx_features_phase
    ON vdd_features(phase);
"#;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VddFeature {
    pub id: String,
    pub project: String,
    pub title: String,
    pub goal_id: String,
    pub phase: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VddTransition {
    pub id: i64,
    pub feature_id: String,
    pub project: String,
    pub from_phase: String,
    pub to_phase: String,
    pub triggered_by: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTime {
    pub phase: String,
    pub entered_at: String,
    pub exited_at: Option<String>,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VddSummary {
    pub total: usize,
    pub by_phase: Vec<(String, usize)>,
    pub features: Vec<VddFeature>,
}

// ─── DB ─────────────────────────────────────────────────────────────────────

fn db_path() -> std::path::PathBuf {
    crate::config::dx_root().join("vdd.db")
}

fn open_db() -> rusqlite::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

// ─── Core Operations ────────────────────────────────────────────────────────

/// Register or update a feature in the VDD tracker.
/// If the feature already exists, updates title/goal_id but does NOT change phase
/// (use `advance` for that).
pub fn upsert_feature(
    project: &str,
    feature_id: &str,
    title: &str,
    goal_id: &str,
) -> Result<VddFeature, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO vdd_features (id, project, title, goal_id)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id, project) DO UPDATE SET
            title = excluded.title,
            goal_id = excluded.goal_id,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        params![feature_id, project, title, goal_id],
    )
    .map_err(|e| e.to_string())?;

    get_feature(&conn, project, feature_id)
}

/// Advance a feature to a new phase. Records the transition.
/// Returns error if the transition is a backward move (unless force=true).
pub fn advance(
    project: &str,
    feature_id: &str,
    to_phase: &FeaturePhase,
    triggered_by: &str,
    reason: &str,
    force: bool,
) -> Result<VddFeature, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let current = get_feature(&conn, project, feature_id)?;
    let current_phase =
        FeaturePhase::from_str_loose(&current.phase).unwrap_or(FeaturePhase::Planned);

    if !force && to_phase.ordinal() <= current_phase.ordinal() && *to_phase != current_phase {
        return Err(format!(
            "backward transition {} -> {} not allowed (use force=true)",
            current.phase,
            to_phase.as_str()
        ));
    }

    if current_phase == *to_phase {
        return Ok(current);
    }

    let new_state = match to_phase {
        FeaturePhase::Planned => "planned",
        FeaturePhase::Done => "complete",
        _ => "active",
    };

    conn.execute(
        "UPDATE vdd_features SET phase = ?1, state = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?3 AND project = ?4",
        params![to_phase.as_str(), new_state, feature_id, project],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO vdd_transitions (feature_id, project, from_phase, to_phase, triggered_by, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            feature_id,
            project,
            current_phase.as_str(),
            to_phase.as_str(),
            triggered_by,
            reason
        ],
    )
    .map_err(|e| e.to_string())?;

    get_feature(&conn, project, feature_id)
}

/// Get a single feature by project + id.
fn get_feature(conn: &Connection, project: &str, feature_id: &str) -> Result<VddFeature, String> {
    conn.query_row(
        "SELECT id, project, title, goal_id, phase, state, created_at, updated_at
         FROM vdd_features WHERE id = ?1 AND project = ?2",
        params![feature_id, project],
        |row| {
            Ok(VddFeature {
                id: row.get(0)?,
                project: row.get(1)?,
                title: row.get(2)?,
                goal_id: row.get(3)?,
                phase: row.get(4)?,
                state: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| format!("feature not found: {}", e))
}

/// List all features for a project, optionally filtered by phase.
pub fn list_features(project: &str, phase_filter: Option<&str>) -> Result<Vec<VddFeature>, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    if let Some(phase) = phase_filter {
        let mut stmt = conn
            .prepare(
                "SELECT id, project, title, goal_id, phase, state, created_at, updated_at
                 FROM vdd_features WHERE project = ?1 AND phase = ?2
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![project, phase], row_to_feature)
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, project, title, goal_id, phase, state, created_at, updated_at
                 FROM vdd_features WHERE project = ?1
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![project], row_to_feature)
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

/// Get transition history for a feature.
pub fn transitions(
    project: &str,
    feature_id: &str,
) -> Result<Vec<VddTransition>, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, feature_id, project, from_phase, to_phase, triggered_by, reason, created_at
             FROM vdd_transitions WHERE feature_id = ?1 AND project = ?2
             ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![feature_id, project], |row| {
            Ok(VddTransition {
                id: row.get(0)?,
                feature_id: row.get(1)?,
                project: row.get(2)?,
                from_phase: row.get(3)?,
                to_phase: row.get(4)?,
                triggered_by: row.get(5)?,
                reason: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Compute time-in-stage for a feature based on its transition history.
pub fn stage_times(project: &str, feature_id: &str) -> Result<Vec<StageTime>, String> {
    let txns = transitions(project, feature_id)?;
    let mut stages: Vec<StageTime> = Vec::new();

    if txns.is_empty() {
        // Feature exists but never transitioned -- it's in its initial phase
        let conn = open_db().map_err(|e| e.to_string())?;
        let feature = get_feature(&conn, project, feature_id)?;
        stages.push(StageTime {
            phase: feature.phase,
            entered_at: feature.created_at,
            exited_at: None,
            duration_secs: None,
        });
        return Ok(stages);
    }

    // First stage: from creation to first transition
    stages.push(StageTime {
        phase: txns[0].from_phase.clone(),
        entered_at: {
            let conn = open_db().map_err(|e| e.to_string())?;
            get_feature(&conn, project, feature_id)?.created_at
        },
        exited_at: Some(txns[0].created_at.clone()),
        duration_secs: duration_between_iso(
            &{
                let conn = open_db().map_err(|e| e.to_string())?;
                get_feature(&conn, project, feature_id)?.created_at
            },
            &txns[0].created_at,
        ),
    });

    // Remaining stages from transitions
    for i in 0..txns.len() {
        let entered = &txns[i].created_at;
        let exited = txns.get(i + 1).map(|t| t.created_at.clone());
        let duration = exited
            .as_ref()
            .and_then(|exit| duration_between_iso(entered, exit));

        stages.push(StageTime {
            phase: txns[i].to_phase.clone(),
            entered_at: entered.clone(),
            exited_at: exited,
            duration_secs: duration,
        });
    }

    Ok(stages)
}

/// Summary across all features for a project (or all projects if project is empty).
pub fn summary(project: Option<&str>) -> Result<VddSummary, String> {
    let conn = open_db().map_err(|e| e.to_string())?;

    let (query, features) = if let Some(proj) = project {
        let mut stmt = conn
            .prepare(
                "SELECT id, project, title, goal_id, phase, state, created_at, updated_at
                 FROM vdd_features WHERE project = ?1 ORDER BY phase, id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![proj], row_to_feature)
            .map_err(|e| e.to_string())?;
        let features: Vec<VddFeature> = rows.filter_map(|r| r.ok()).collect();
        ("project".to_string(), features)
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, project, title, goal_id, phase, state, created_at, updated_at
                 FROM vdd_features ORDER BY project, phase, id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], row_to_feature)
            .map_err(|e| e.to_string())?;
        let features: Vec<VddFeature> = rows.filter_map(|r| r.ok()).collect();
        ("all".to_string(), features)
    };
    let _ = query;

    let total = features.len();
    let phases = ["planned", "discovery", "design", "build", "test", "done"];
    let by_phase: Vec<(String, usize)> = phases
        .iter()
        .map(|p| {
            let count = features.iter().filter(|f| f.phase == *p).count();
            (p.to_string(), count)
        })
        .collect();

    Ok(VddSummary {
        total,
        by_phase,
        features,
    })
}

// ─── Sync: bridge vision.rs JSON <-> VDD SQLite ─────────────────────────────

/// Sync features from a vision.json file into the VDD SQLite store.
/// Called after any vision mutation so SQLite stays in sync.
pub fn sync_from_vision(project_path: &str) {
    let project = project_name_from_path(project_path);
    let vision = match crate::vision::load_vision(project_path) {
        Some(v) => v,
        None => return,
    };

    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return,
    };

    for feature in &vision.features {
        let phase = feature.phase.as_str();
        let state = match feature.state {
            crate::vision::FeatureState::Planned => "planned",
            crate::vision::FeatureState::Active => "active",
            crate::vision::FeatureState::Blocked => "blocked",
            crate::vision::FeatureState::Complete => "complete",
        };

        // Upsert feature
        let _ = conn.execute(
            "INSERT INTO vdd_features (id, project, title, goal_id, phase, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id, project) DO UPDATE SET
                title = excluded.title,
                goal_id = excluded.goal_id,
                phase = excluded.phase,
                state = excluded.state,
                updated_at = excluded.updated_at",
            params![
                feature.id,
                project,
                feature.title,
                feature.goal_id,
                phase,
                state,
                feature.created_at,
                feature.updated_at,
            ],
        );
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn row_to_feature(row: &rusqlite::Row<'_>) -> rusqlite::Result<VddFeature> {
    Ok(VddFeature {
        id: row.get(0)?,
        project: row.get(1)?,
        title: row.get(2)?,
        goal_id: row.get(3)?,
        phase: row.get(4)?,
        state: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// Extract project name from path (last component).
fn project_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn duration_between_iso(from: &str, to: &str) -> Option<i64> {
    let from_dt = chrono::NaiveDateTime::parse_from_str(from, "%Y-%m-%dT%H:%M:%SZ").ok()?;
    let to_dt = chrono::NaiveDateTime::parse_from_str(to, "%Y-%m-%dT%H:%M:%SZ").ok()?;
    Some((to_dt - from_dt).num_seconds())
}


// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::FeaturePhase;

    #[test]
    fn test_upsert_and_get() {
        // Use temp dir so tests don't collide
        let _dir = tempfile::tempdir().unwrap();
        let feature = upsert_feature("test-project", "F1.1", "Login flow", "G1").unwrap();
        assert_eq!(feature.id, "F1.1");
        assert_eq!(feature.project, "test-project");
        assert_eq!(feature.phase, "planned");
        assert_eq!(feature.state, "planned");
    }

    #[test]
    fn test_advance_lifecycle() {
        let f = upsert_feature("lifecycle-test", "F2.1", "Auth", "G1").unwrap();
        assert_eq!(f.phase, "planned");

        let f = advance(
            "lifecycle-test",
            "F2.1",
            &FeaturePhase::Discovery,
            "agent-1",
            "starting research",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "discovery");
        assert_eq!(f.state, "active");

        let f = advance(
            "lifecycle-test",
            "F2.1",
            &FeaturePhase::Design,
            "agent-1",
            "research complete, designing",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "design");

        let f = advance(
            "lifecycle-test",
            "F2.1",
            &FeaturePhase::Build,
            "agent-2",
            "design approved",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "build");

        let f = advance(
            "lifecycle-test",
            "F2.1",
            &FeaturePhase::Test,
            "agent-2",
            "implementation complete",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "test");

        let f = advance(
            "lifecycle-test",
            "F2.1",
            &FeaturePhase::Done,
            "qa",
            "all tests pass",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "done");
        assert_eq!(f.state, "complete");
    }

    #[test]
    fn test_backward_transition_blocked() {
        upsert_feature("back-test", "F3.1", "Feature", "G1").unwrap();
        advance(
            "back-test",
            "F3.1",
            &FeaturePhase::Build,
            "user",
            "",
            false,
        )
        .unwrap();

        let err = advance(
            "back-test",
            "F3.1",
            &FeaturePhase::Discovery,
            "user",
            "",
            false,
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("backward"));
    }

    #[test]
    fn test_backward_transition_forced() {
        upsert_feature("force-test", "F4.1", "Feature", "G1").unwrap();
        advance(
            "force-test",
            "F4.1",
            &FeaturePhase::Build,
            "user",
            "",
            false,
        )
        .unwrap();

        let f = advance(
            "force-test",
            "F4.1",
            &FeaturePhase::Discovery,
            "user",
            "rework needed",
            true,
        )
        .unwrap();
        assert_eq!(f.phase, "discovery");
    }

    #[test]
    fn test_transitions_history() {
        upsert_feature("hist-test", "F5.1", "Feature", "G1").unwrap();
        advance(
            "hist-test",
            "F5.1",
            &FeaturePhase::Discovery,
            "a",
            "",
            false,
        )
        .unwrap();
        advance(
            "hist-test",
            "F5.1",
            &FeaturePhase::Design,
            "b",
            "",
            false,
        )
        .unwrap();
        advance(
            "hist-test",
            "F5.1",
            &FeaturePhase::Build,
            "c",
            "",
            false,
        )
        .unwrap();

        let txns = transitions("hist-test", "F5.1").unwrap();
        assert_eq!(txns.len(), 3);
        assert_eq!(txns[0].from_phase, "planned");
        assert_eq!(txns[0].to_phase, "discovery");
        assert_eq!(txns[1].from_phase, "discovery");
        assert_eq!(txns[1].to_phase, "design");
        assert_eq!(txns[2].from_phase, "design");
        assert_eq!(txns[2].to_phase, "build");
    }

    #[test]
    fn test_summary() {
        upsert_feature("sum-test", "F6.1", "A", "G1").unwrap();
        upsert_feature("sum-test", "F6.2", "B", "G1").unwrap();
        advance(
            "sum-test",
            "F6.2",
            &FeaturePhase::Build,
            "x",
            "",
            false,
        )
        .unwrap();

        let s = summary(Some("sum-test")).unwrap();
        assert_eq!(s.total, 2);

        let planned = s.by_phase.iter().find(|(p, _)| p == "planned").unwrap().1;
        let build = s.by_phase.iter().find(|(p, _)| p == "build").unwrap().1;
        assert_eq!(planned, 1);
        assert_eq!(build, 1);
    }

    #[test]
    fn test_idempotent_advance() {
        upsert_feature("idem-test", "F7.1", "Feature", "G1").unwrap();
        advance(
            "idem-test",
            "F7.1",
            &FeaturePhase::Build,
            "a",
            "",
            false,
        )
        .unwrap();

        // Advancing to same phase is a no-op
        let f = advance(
            "idem-test",
            "F7.1",
            &FeaturePhase::Build,
            "a",
            "",
            false,
        )
        .unwrap();
        assert_eq!(f.phase, "build");

        // Should not create duplicate transition
        let txns = transitions("idem-test", "F7.1").unwrap();
        assert_eq!(txns.len(), 1);
    }
}
