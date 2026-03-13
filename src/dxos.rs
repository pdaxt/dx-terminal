use crate::config;
use crate::state::events::StateEvent;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDescriptor {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneDefaults {
    pub deployment_model: String,
    pub autonomy_mode: String,
    pub runtime_substrate: String,
    pub runtime_adapter: String,
    pub capability_source: String,
    pub governance_model: String,
    pub research_mode: String,
    pub docs_required: bool,
    pub stages: Vec<String>,
    pub v1_domains: Vec<String>,
}

impl Default for ControlPlaneDefaults {
    fn default() -> Self {
        Self {
            deployment_model: "hybrid_saas".to_string(),
            autonomy_mode: "guarded_auto".to_string(),
            runtime_substrate: "custom_pty".to_string(),
            runtime_adapter: "pty_native_adapter".to_string(),
            capability_source: "dx_registry".to_string(),
            governance_model: "structured_council".to_string(),
            research_mode: "formal_debate".to_string(),
            docs_required: true,
            stages: vec![
                "planned".to_string(),
                "discovery".to_string(),
                "design".to_string(),
                "build".to_string(),
                "test".to_string(),
                "done".to_string(),
            ],
            v1_domains: vec![
                "delivery".to_string(),
                "design".to_string(),
                "documentation".to_string(),
                "qa".to_string(),
                "security".to_string(),
                "compliance".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPolicyRule {
    pub role: String,
    pub stage: String,
    pub preferred_provider: String,
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    #[serde(default)]
    pub suggested_models: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalRecord {
    pub id: String,
    pub author: String,
    #[serde(default)]
    pub model: Option<String>,
    pub summary: String,
    pub rationale: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionRecord {
    pub id: String,
    pub proposal_id: String,
    pub author: String,
    #[serde(default)]
    pub model: Option<String>,
    pub rationale: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRecord {
    pub id: String,
    pub proposal_id: String,
    pub voter: String,
    #[serde(default)]
    pub model: Option<String>,
    pub stance: String,
    pub rationale: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub chosen_proposal_id: String,
    pub decided_by: String,
    pub summary: String,
    pub rationale: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateRecord {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub status: String,
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub proposals: Vec<ProposalRecord>,
    #[serde(default)]
    pub contradictions: Vec<ContradictionRecord>,
    #[serde(default)]
    pub votes: Vec<VoteRecord>,
    #[serde(default)]
    pub decision: Option<DecisionRecord>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAdoptionRecord {
    pub id: String,
    pub status: String,
    pub mode: String,
    pub summary: String,
    pub objective: String,
    #[serde(default)]
    pub last_note: Option<String>,
    #[serde(default)]
    pub initial_work_order_id: Option<String>,
    #[serde(default)]
    pub feature_id: Option<String>,
    pub stage: String,
    pub lead_session_id: String,
    pub debate_id: String,
    #[serde(default)]
    pub requested_by: Option<String>,
    #[serde(default)]
    pub participants: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContractRecord {
    pub id: String,
    pub status: String,
    pub role: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub autonomy_level: String,
    pub objective: String,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub allowed_capabilities: Vec<String>,
    #[serde(default)]
    pub allowed_repos: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub browser_port: Option<u16>,
    #[serde(default)]
    pub pane: Option<u8>,
    #[serde(default)]
    pub runtime_adapter: Option<String>,
    #[serde(default)]
    pub tmux_target: Option<String>,
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub supervisor_session_id: Option<String>,
    #[serde(default)]
    pub escalation_policy: Option<String>,
    #[serde(default)]
    pub policy_violations: Vec<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrderRecord {
    pub id: String,
    pub supervisor_session_id: String,
    #[serde(default)]
    pub worker_session_id: Option<String>,
    pub status: String,
    #[serde(default = "default_escalation_target")]
    pub escalation_target: String,
    pub title: String,
    pub objective: String,
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub requested_permissions: Vec<String>,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub resolution_notes: Vec<WorkResolutionRecord>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResolutionRecord {
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: String,
    pub project_path: String,
    pub project_name: String,
    pub actor: String,
    pub action_kind: String,
    pub target: String,
    pub outcome: String,
    pub summary: String,
    #[serde(default)]
    pub details: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlOperatorProfile {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub project_scopes: Vec<String>,
    #[serde(default)]
    pub allowed_actions: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneState {
    pub version: u32,
    pub project: ProjectDescriptor,
    pub defaults: ControlPlaneDefaults,
    #[serde(default)]
    pub adoptions: Vec<ProjectAdoptionRecord>,
    #[serde(default)]
    pub debates: Vec<DebateRecord>,
    #[serde(default)]
    pub sessions: Vec<SessionContractRecord>,
    #[serde(default)]
    pub work_orders: Vec<WorkOrderRecord>,
    pub updated_at: String,
}

fn resolved_project_name(project_path: &str, project_name: Option<&str>) -> String {
    project_name
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            Path::new(project_path)
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "project".to_string())
}

fn control_plane_dir(project_path: &str) -> PathBuf {
    Path::new(project_path).join(".dxos")
}

fn control_plane_file(project_path: &str) -> PathBuf {
    control_plane_dir(project_path).join("control-plane.json")
}

fn control_plane_store_dir(_project_path: &str) -> PathBuf {
    #[cfg(test)]
    {
        let path = Path::new(_project_path);
        if path.starts_with(std::env::temp_dir()) {
            return path.parent().unwrap_or(path).join(".dxos-store");
        }
    }
    config::dx_root().join("dxos")
}

fn control_plane_store_path(project_path: &str) -> PathBuf {
    control_plane_store_dir(project_path).join("control-plane.sqlite3")
}

const CONTROL_PLANE_STORE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS dxos_control_planes (
    project_path TEXT PRIMARY KEY,
    project_name TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dxos_control_planes_updated_at
    ON dxos_control_planes(updated_at DESC);
CREATE TABLE IF NOT EXISTS dxos_audit_log (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    project_name TEXT NOT NULL,
    actor TEXT NOT NULL,
    action_kind TEXT NOT NULL,
    target TEXT NOT NULL,
    outcome TEXT NOT NULL,
    summary TEXT NOT NULL,
    details_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dxos_audit_log_project_created_at
    ON dxos_audit_log(project_path, created_at DESC);
"#;

fn open_control_plane_db(path: &Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {}", e))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("open: {}", e))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| format!("pragma: {}", e))?;
    conn.execute_batch(CONTROL_PLANE_STORE_SCHEMA)
        .map_err(|e| format!("schema: {}", e))?;
    Ok(conn)
}

fn control_plane_db(project_path: &str) -> Result<Connection, String> {
    open_control_plane_db(&control_plane_store_path(project_path))
}

fn global_control_plane_store_path() -> PathBuf {
    config::dx_root().join("dxos").join("control-plane.sqlite3")
}

fn load_control_plane_from_store(project_path: &str) -> Option<ControlPlaneState> {
    let conn = control_plane_db(project_path).ok()?;
    let payload: String = conn
        .query_row(
            "SELECT payload_json FROM dxos_control_planes WHERE project_path = ?1",
            params![project_path],
            |row| row.get(0),
        )
        .ok()?;
    serde_json::from_str::<ControlPlaneState>(&payload).ok()
}

fn store_control_plane_in_sqlite(state: &ControlPlaneState) -> Result<(), String> {
    let conn = control_plane_db(&state.project.path)?;
    let payload = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {}", e))?;
    conn.execute(
        "INSERT INTO dxos_control_planes(project_path, project_name, updated_at, payload_json)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(project_path) DO UPDATE SET
            project_name = excluded.project_name,
            updated_at = excluded.updated_at,
            payload_json = excluded.payload_json",
        params![
            &state.project.path,
            &state.project.name,
            &state.updated_at,
            payload
        ],
    )
    .map_err(|e| format!("upsert: {}", e))?;
    Ok(())
}

fn save_control_plane_mirror(project_path: &str, state: &ControlPlaneState) -> Result<(), String> {
    let dir = control_plane_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {}", e))?;
    std::fs::write(control_plane_file(project_path), json).map_err(|e| format!("write: {}", e))
}

fn control_plane_store_project_count(project_path: &str) -> usize {
    control_plane_db(project_path)
        .ok()
        .and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM dxos_control_planes", [], |row| {
                row.get::<_, i64>(0)
            })
            .ok()
        })
        .unwrap_or(0)
        .max(0) as usize
}

fn control_plane_storage_summary(project_path: &str) -> Value {
    json!({
        "backend": "sqlite_with_repo_mirror",
        "canonical": "dx_root_sqlite",
        "mirror": "repo_local_json",
        "database_path": control_plane_store_path(project_path).to_string_lossy().to_string(),
        "mirror_path": control_plane_file(project_path).to_string_lossy().to_string(),
        "registered_projects": control_plane_store_project_count(project_path),
    })
}

fn default_state(project_path: &str, project_name: Option<&str>) -> ControlPlaneState {
    ControlPlaneState {
        version: 1,
        project: ProjectDescriptor {
            name: resolved_project_name(project_path, project_name),
            path: project_path.to_string(),
        },
        defaults: ControlPlaneDefaults::default(),
        adoptions: Vec::new(),
        debates: Vec::new(),
        sessions: Vec::new(),
        work_orders: Vec::new(),
        updated_at: crate::state::now(),
    }
}

pub fn load_control_plane(project_path: &str, project_name: Option<&str>) -> ControlPlaneState {
    if let Some(mut state) = load_control_plane_from_store(project_path) {
        if state.project.name.trim().is_empty() {
            state.project.name = resolved_project_name(project_path, project_name);
        }
        if state.project.path.trim().is_empty() {
            state.project.path = project_path.to_string();
        }
        let _ = save_control_plane_mirror(project_path, &state);
        return state;
    }

    let path = control_plane_file(project_path);
    if path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(mut state) = serde_json::from_str::<ControlPlaneState>(&contents) {
                if state.project.name.trim().is_empty() {
                    state.project.name = resolved_project_name(project_path, project_name);
                }
                if state.project.path.trim().is_empty() {
                    state.project.path = project_path.to_string();
                }
                let _ = store_control_plane_in_sqlite(&state);
                return state;
            }
        }
    }
    default_state(project_path, project_name)
}

fn save_control_plane(project_path: &str, state: &ControlPlaneState) -> Result<(), String> {
    store_control_plane_in_sqlite(state)?;
    save_control_plane_mirror(project_path, state)
}

fn control_plane_registry_value_for_store_path(registry_path: &Path) -> Value {
    let conn = match open_control_plane_db(&registry_path) {
        Ok(conn) => conn,
        Err(error) => return json!({"error": error}),
    };
    let mut stmt = match conn.prepare(
        "SELECT project_path, project_name, updated_at
         FROM dxos_control_planes
         ORDER BY updated_at DESC, project_name ASC",
    ) {
        Ok(stmt) => stmt,
        Err(error) => return json!({"error": format!("prepare: {}", error)}),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(json!({
            "path": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "updated_at": row.get::<_, String>(2)?,
        }))
    }) {
        Ok(rows) => rows,
        Err(error) => return json!({"error": format!("query: {}", error)}),
    };
    let projects = rows.filter_map(Result::ok).collect::<Vec<_>>();
    json!({
        "backend": "sqlite_with_repo_mirror",
        "database_path": registry_path.to_string_lossy().to_string(),
        "project_count": projects.len(),
        "projects": projects,
    })
}

fn control_plane_registry_value_for_project_path(project_path: &str) -> Value {
    control_plane_registry_value_for_store_path(&control_plane_store_path(project_path))
}

fn next_audit_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let seq = AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("AU{}-{}", millis, seq)
}

fn audit_summary(record: &AuditRecord) -> Value {
    json!({
        "id": record.id,
        "actor": record.actor,
        "action_kind": record.action_kind,
        "target": record.target,
        "outcome": record.outcome,
        "summary": record.summary,
        "details": record.details,
        "created_at": record.created_at,
    })
}

fn recent_audit_records(project_path: &str, limit: usize) -> Vec<Value> {
    let conn = match control_plane_db(project_path) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare(
        "SELECT id, project_path, project_name, actor, action_kind, target, outcome, summary, details_json, created_at
         FROM dxos_audit_log
         WHERE project_path = ?1
         ORDER BY created_at DESC
         LIMIT ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(params![project_path, limit as i64], |row| {
        let details_json: String = row.get(8)?;
        Ok(AuditRecord {
            id: row.get(0)?,
            project_path: row.get(1)?,
            project_name: row.get(2)?,
            actor: row.get(3)?,
            action_kind: row.get(4)?,
            target: row.get(5)?,
            outcome: row.get(6)?,
            summary: row.get(7)?,
            details: serde_json::from_str(&details_json).unwrap_or_else(|_| json!({})),
            created_at: row.get(9)?,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(Result::ok)
        .map(|record| audit_summary(&record))
        .collect()
}

fn audit_record_count(project_path: &str) -> usize {
    control_plane_db(project_path)
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM dxos_audit_log WHERE project_path = ?1",
                params![project_path],
                |row| row.get::<_, i64>(0),
            )
            .ok()
        })
        .unwrap_or(0)
        .max(0) as usize
}

pub fn append_audit_record(
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    action_kind: &str,
    target: &str,
    outcome: &str,
    summary: &str,
    details: Value,
) -> Result<AuditRecord, String> {
    if project_path.trim().is_empty() {
        return Err("project_path required".to_string());
    }
    let record = AuditRecord {
        id: next_audit_id(),
        project_path: project_path.to_string(),
        project_name: resolved_project_name(project_path, project_name),
        actor: actor.trim().to_string(),
        action_kind: action_kind.trim().to_string(),
        target: target.trim().to_string(),
        outcome: outcome.trim().to_string(),
        summary: summary.trim().to_string(),
        details,
        created_at: crate::state::now(),
    };
    let conn = control_plane_db(project_path)?;
    conn.execute(
        "INSERT INTO dxos_audit_log(id, project_path, project_name, actor, action_kind, target, outcome, summary, details_json, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            &record.id,
            &record.project_path,
            &record.project_name,
            &record.actor,
            &record.action_kind,
            &record.target,
            &record.outcome,
            &record.summary,
            serde_json::to_string(&record.details).map_err(|e| format!("serialize: {}", e))?,
            &record.created_at,
        ],
    )
    .map_err(|e| format!("insert audit: {}", e))?;
    Ok(record)
}

pub fn audit_list(project_path: &str, project_name: Option<&str>, limit: usize) -> String {
    json!({
        "project": {
            "name": resolved_project_name(project_path, project_name),
            "path": project_path,
        },
        "audit": {
            "total": audit_record_count(project_path),
            "recent": recent_audit_records(project_path, limit),
        }
    })
    .to_string()
}

fn default_allowed_actions_for_role(role: &str) -> Vec<String> {
    match role {
        "admin" => vec!["*".to_string()],
        "lead" => vec![
            "adoption_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "debate_*".to_string(),
            "pane_talk".to_string(),
            "pane_restart".to_string(),
        ],
        "reviewer" => vec![
            "debate_*".to_string(),
            "work_resolve".to_string(),
            "session_block".to_string(),
            "pane_talk".to_string(),
        ],
        "operator" => vec![
            "adoption_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "debate_*".to_string(),
            "pane_*".to_string(),
        ],
        "observer" => Vec::new(),
        _ => vec![
            "adoption_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "debate_*".to_string(),
            "pane_talk".to_string(),
        ],
    }
}

fn parse_control_operator_registry_value(raw: &str) -> Vec<ControlOperatorProfile> {
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let entries = parsed
        .get("operators")
        .cloned()
        .unwrap_or(parsed)
        .as_array()
        .cloned()
        .unwrap_or_default();
    entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<ControlOperatorProfile>(entry).ok())
        .filter_map(|mut operator| {
            operator.id = operator.id.trim().to_string();
            operator.role = operator.role.trim().to_lowercase();
            if operator.id.is_empty() {
                return None;
            }
            if operator.role.is_empty() {
                operator.role = "operator".to_string();
            }
            if operator.project_scopes.is_empty() {
                operator.project_scopes.push("*".to_string());
            }
            if operator.allowed_actions.is_empty() {
                operator.allowed_actions = default_allowed_actions_for_role(&operator.role);
            }
            Some(operator)
        })
        .collect()
}

fn load_control_operator_profiles() -> Vec<ControlOperatorProfile> {
    config::control_operators_json()
        .map(|raw| parse_control_operator_registry_value(&raw))
        .unwrap_or_default()
}

fn authorization_pattern_matches(pattern: &str, value: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "*" {
        return true;
    }
    if let Some(prefix) = trimmed.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    trimmed == value
}

fn project_scope_matches(pattern: &str, project_path: &str, project_name: Option<&str>) -> bool {
    let normalized_name = resolved_project_name(project_path, project_name);
    authorization_pattern_matches(pattern, project_path)
        || authorization_pattern_matches(pattern, &normalized_name)
        || (project_path.trim().is_empty()
            && (authorization_pattern_matches(pattern, "global")
                || authorization_pattern_matches(pattern, "--")))
}

fn authorize_operator_action_with_profiles(
    profiles: &[ControlOperatorProfile],
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    action_kind: &str,
) -> Result<Value, String> {
    if profiles.is_empty() {
        return Ok(json!({
            "mode": "authenticated_open",
            "id": actor,
            "role": "operator",
            "project_scopes": ["*"],
            "allowed_actions": ["*"],
        }));
    }
    let Some(operator) = profiles.iter().find(|profile| profile.id == actor) else {
        return Err(format!(
            "Operator '{}' is not registered for DXOS control.",
            actor
        ));
    };
    if !operator
        .project_scopes
        .iter()
        .any(|scope| project_scope_matches(scope, project_path, project_name))
    {
        return Err(format!(
            "Operator '{}' cannot control project '{}'.",
            actor,
            resolved_project_name(project_path, project_name)
        ));
    }
    if !operator
        .allowed_actions
        .iter()
        .any(|pattern| authorization_pattern_matches(pattern, action_kind))
    {
        return Err(format!(
            "Operator '{}' with role '{}' is not allowed to perform '{}'.",
            actor, operator.role, action_kind
        ));
    }
    Ok(json!({
        "mode": "operator_policy",
        "id": operator.id,
        "role": operator.role,
        "project_scopes": operator.project_scopes,
        "allowed_actions": operator.allowed_actions,
        "note": operator.note,
    }))
}

pub fn authorize_operator_action(
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    action_kind: &str,
) -> Result<Value, String> {
    authorize_operator_action_with_profiles(
        &load_control_operator_profiles(),
        project_path,
        project_name,
        actor,
        action_kind,
    )
}

pub fn control_operator_registry() -> Value {
    let operators = load_control_operator_profiles();
    json!({
        "configured": !operators.is_empty(),
        "count": operators.len(),
        "operators": operators.iter().map(|operator| json!({
            "id": operator.id,
            "role": operator.role,
            "project_scopes": operator.project_scopes,
            "allowed_actions": operator.allowed_actions,
            "note": operator.note,
        })).collect::<Vec<_>>(),
    })
}

pub fn control_auth_contract() -> Value {
    let token_configured = config::control_token().is_some();
    let operator_registry = control_operator_registry();
    let operator_policy_configured = operator_registry
        .get("configured")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    json!({
        "mode": if token_configured && operator_policy_configured {
            "token_and_operator_policy"
        } else if token_configured {
            "control_token"
        } else if operator_policy_configured {
            "local_role_policy"
        } else {
            "local_trusted"
        },
        "configured": token_configured,
        "operator_policy_configured": operator_policy_configured,
        "required_header": "x-dx-control-token",
        "authorization_scheme": "Bearer",
        "operators": operator_registry["operators"].clone(),
        "required_for": [
            "/api/dxos/session/launch",
            "/api/dxos/session/upsert",
            "/api/dxos/session/status",
            "/api/dxos/session/block",
            "/api/dxos/work/delegate",
            "/api/dxos/work/block",
            "/api/dxos/work/resolve",
            "/api/dxos/debate/start",
            "/api/dxos/debate/proposal",
            "/api/dxos/debate/contradiction",
            "/api/dxos/debate/vote",
            "/api/dxos/debate/decision",
            "/api/dxos/adoption/start",
            "/api/dxos/adoption/status",
            "/api/pane/talk",
            "/api/pane/kill",
            "/api/pane/restart"
        ]
    })
}

pub fn control_plane_registry() -> String {
    control_plane_registry_value_for_store_path(&global_control_plane_store_path()).to_string()
}

#[cfg(test)]
fn control_plane_registry_for_project(project_path: &str) -> String {
    control_plane_registry_value_for_project_path(project_path).to_string()
}

fn next_debate_id(state: &ControlPlaneState) -> String {
    format!("DB{:04}", state.debates.len() + 1)
}

fn next_adoption_id(state: &ControlPlaneState) -> String {
    format!("AD{:04}", state.adoptions.len() + 1)
}

fn next_child_id(prefix: &str, len: usize) -> String {
    format!("{}{:04}", prefix, len + 1)
}

fn next_session_id(state: &ControlPlaneState) -> String {
    format!("SX{:04}", state.sessions.len() + 1)
}

fn next_work_order_id(state: &ControlPlaneState) -> String {
    format!("WO{:04}", state.work_orders.len() + 1)
}

fn default_escalation_target() -> String {
    "lead".to_string()
}

fn normalize_stage(stage: Option<&str>) -> String {
    stage
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "build".to_string())
}

fn normalize_role(role: &str) -> String {
    let normalized = role.trim().to_lowercase();
    if normalized.is_empty() {
        "developer".to_string()
    } else {
        normalized
    }
}

fn normalize_provider(provider: Option<&str>) -> Option<String> {
    provider
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .map(|value| match value.as_str() {
            "openai" => "codex".to_string(),
            "google" => "gemini".to_string(),
            "open-code" => "opencode".to_string(),
            other => other.to_string(),
        })
}

fn provider_policy_for(role: &str, stage: Option<&str>) -> ProviderPolicyRule {
    let role = normalize_role(role);
    let stage = normalize_stage(stage);

    match (role.as_str(), stage.as_str()) {
        ("design", _) | ("discovery", _) | ("lead", "design") | ("lead", "discovery") => {
            ProviderPolicyRule {
                role,
                stage,
                preferred_provider: "claude".to_string(),
                allowed_providers: vec![
                    "claude".to_string(),
                    "gemini".to_string(),
                    "codex".to_string(),
                    "opencode".to_string(),
                ],
                suggested_models: vec![
                    "claude-opus-4.6".to_string(),
                    "gemini-2.5-pro".to_string(),
                    "gpt-5.4".to_string(),
                ],
                rationale: "Discovery and design lanes need stronger long-context reasoning and multimodal critique before implementation accelerates.".to_string(),
            }
        }
        ("frontend", _)
        | ("backend", _)
        | ("developer", _)
        | (_, "build") => ProviderPolicyRule {
            role,
            stage,
            preferred_provider: "codex".to_string(),
            allowed_providers: vec![
                "codex".to_string(),
                "claude".to_string(),
                "gemini".to_string(),
                "opencode".to_string(),
            ],
            suggested_models: vec![
                "gpt-5.4".to_string(),
                "claude-opus-4.6".to_string(),
                "gemini-2.5-pro".to_string(),
            ],
            rationale: "Build lanes should prefer tool-stable coding runtimes while keeping higher-reasoning providers available for harder codegen and review turns.".to_string(),
        },
        ("qa", _)
        | ("security", _)
        | ("review", _)
        | ("release", _)
        | (_, "test") => ProviderPolicyRule {
            role,
            stage,
            preferred_provider: "claude".to_string(),
            allowed_providers: vec![
                "claude".to_string(),
                "codex".to_string(),
                "gemini".to_string(),
            ],
            suggested_models: vec![
                "claude-opus-4.6".to_string(),
                "gpt-5.4".to_string(),
                "gemini-2.5-pro".to_string(),
            ],
            rationale: "Verification, review, and release lanes should bias toward stronger critical evaluation and reliable tool traces over raw generation speed.".to_string(),
        },
        ("docs", _) => ProviderPolicyRule {
            role,
            stage,
            preferred_provider: "claude".to_string(),
            allowed_providers: vec![
                "claude".to_string(),
                "gemini".to_string(),
                "codex".to_string(),
                "opencode".to_string(),
            ],
            suggested_models: vec![
                "claude-opus-4.6".to_string(),
                "gemini-2.5-pro".to_string(),
            ],
            rationale: "Documentation lanes should preserve narrative quality and consistency while staying provider-neutral.".to_string(),
        },
        _ => ProviderPolicyRule {
            role,
            stage,
            preferred_provider: "claude".to_string(),
            allowed_providers: vec![
                "claude".to_string(),
                "codex".to_string(),
                "gemini".to_string(),
                "opencode".to_string(),
            ],
            suggested_models: vec![
                "claude-opus-4.6".to_string(),
                "gpt-5.4".to_string(),
                "gemini-2.5-pro".to_string(),
            ],
            rationale: "Default DXOS policy keeps all governed runtimes available while preferring Claude for general orchestration and research-heavy work.".to_string(),
        },
    }
}

fn provider_policy_matrix() -> Vec<ProviderPolicyRule> {
    let roles = [
        "lead",
        "discovery",
        "design",
        "frontend",
        "backend",
        "qa",
        "docs",
        "security",
        "review",
        "release",
    ];
    let stages = ["planned", "discovery", "design", "build", "test", "done"];
    let mut rules = Vec::new();
    for role in roles {
        for stage in stages {
            rules.push(provider_policy_for(role, Some(stage)));
        }
    }
    rules
}

fn validate_provider_selection(
    role: &str,
    stage: Option<&str>,
    provider: Option<&str>,
    pane: Option<u8>,
    tmux_target: Option<&str>,
) -> (Option<String>, ProviderPolicyRule, Vec<String>) {
    let policy = provider_policy_for(role, stage);
    let normalized_provider = normalize_provider(provider);
    let mut violations = Vec::new();
    let runtime_bound = pane.is_some() || tmux_target.is_some();

    if matches!(normalized_provider.as_deref(), Some("shared")) {
        if runtime_bound {
            violations.push(
                "Provider 'shared' is only valid for unbound DXOS contracts; runtime-bound sessions need a concrete provider."
                    .to_string(),
            );
        }
    } else if let Some(selected) = normalized_provider.as_deref() {
        if !policy.allowed_providers.iter().any(|item| item == selected) {
            violations.push(format!(
                "Provider '{}' is outside DXOS policy for role '{}' at stage '{}'. Allowed: {}.",
                selected,
                policy.role,
                policy.stage,
                policy.allowed_providers.join(", ")
            ));
        }
    }

    (normalized_provider, policy, violations)
}

fn debate_summary(debate: &DebateRecord) -> Value {
    let mut tallies: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
    for vote in &debate.votes {
        let proposal_entry = tallies.entry(vote.proposal_id.clone()).or_default();
        *proposal_entry.entry(vote.stance.clone()).or_insert(0) += 1;
    }

    json!({
        "id": debate.id,
        "title": debate.title,
        "objective": debate.objective,
        "status": debate.status,
        "feature_id": debate.feature_id,
        "stage": debate.stage,
        "participants": debate.participants,
        "proposal_count": debate.proposals.len(),
        "contradiction_count": debate.contradictions.len(),
        "vote_count": debate.votes.len(),
        "decision": debate.decision,
        "tallies": tallies,
        "created_at": debate.created_at,
        "updated_at": debate.updated_at,
    })
}

fn adoption_summary(adoption: &ProjectAdoptionRecord) -> Value {
    json!({
        "id": adoption.id,
        "status": adoption.status,
        "mode": adoption.mode,
        "summary": adoption.summary,
        "objective": adoption.objective,
        "last_note": adoption.last_note,
        "initial_work_order_id": adoption.initial_work_order_id,
        "feature_id": adoption.feature_id,
        "stage": adoption.stage,
        "lead_session_id": adoption.lead_session_id,
        "debate_id": adoption.debate_id,
        "requested_by": adoption.requested_by,
        "participants": adoption.participants,
        "created_at": adoption.created_at,
        "updated_at": adoption.updated_at,
    })
}

fn session_summary(session: &SessionContractRecord) -> Value {
    let provider_policy = provider_policy_for(&session.role, session.stage.as_deref());
    json!({
        "id": session.id,
        "status": session.status,
        "role": session.role,
        "provider": session.provider,
        "model": session.model,
        "autonomy_level": session.autonomy_level,
        "objective": session.objective,
        "expected_outputs": session.expected_outputs,
        "allowed_capabilities": session.allowed_capabilities,
        "allowed_repos": session.allowed_repos,
        "allowed_paths": session.allowed_paths,
        "workspace_path": session.workspace_path,
        "branch_name": session.branch_name,
        "browser_port": session.browser_port,
        "pane": session.pane,
        "runtime_adapter": session.runtime_adapter,
        "tmux_target": session.tmux_target,
        "feature_id": session.feature_id,
        "stage": session.stage,
        "supervisor_session_id": session.supervisor_session_id,
        "escalation_policy": session.escalation_policy,
        "policy_violations": session.policy_violations,
        "last_error": session.last_error,
        "provider_policy": provider_policy,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
    })
}

fn work_order_summary(work_order: &WorkOrderRecord) -> Value {
    let routed_to = if work_order.escalation_target == "human" {
        "human".to_string()
    } else {
        work_order.supervisor_session_id.clone()
    };
    json!({
        "id": work_order.id,
        "supervisor_session_id": work_order.supervisor_session_id,
        "worker_session_id": work_order.worker_session_id,
        "status": work_order.status,
        "escalation_target": work_order.escalation_target,
        "routed_to": routed_to,
        "title": work_order.title,
        "objective": work_order.objective,
        "feature_id": work_order.feature_id,
        "stage": work_order.stage,
        "required_capabilities": work_order.required_capabilities,
        "blockers": work_order.blockers,
        "requested_permissions": work_order.requested_permissions,
        "expected_outputs": work_order.expected_outputs,
        "last_resolution": work_order.resolution_notes.last().map(|note| note.message.clone()),
        "resolution_notes": work_order.resolution_notes.iter().rev().take(5).collect::<Vec<_>>(),
        "created_at": work_order.created_at,
        "updated_at": work_order.updated_at,
    })
}

pub fn control_plane_snapshot(project_path: &str, project_name: Option<&str>) -> Value {
    let state = load_control_plane(project_path, project_name);
    let registry = crate::mcp_registry::load_registry();
    let mut categories: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &registry {
        *categories.entry(entry.category.clone()).or_insert(0) += 1;
    }

    let open_debates = state
        .debates
        .iter()
        .filter(|debate| debate.status == "open")
        .count();
    let decided_debates = state
        .debates
        .iter()
        .filter(|debate| debate.status == "decided")
        .count();
    let active_adoptions = state
        .adoptions
        .iter()
        .filter(|adoption| matches!(adoption.status.as_str(), "active" | "planned"))
        .count();
    let recent_adoptions = state
        .adoptions
        .iter()
        .rev()
        .take(5)
        .map(adoption_summary)
        .collect::<Vec<_>>();
    let recent = state
        .debates
        .iter()
        .rev()
        .take(5)
        .map(debate_summary)
        .collect::<Vec<_>>();
    let active_sessions = state
        .sessions
        .iter()
        .filter(|session| matches!(session.status.as_str(), "planned" | "launching" | "active"))
        .count();
    let blocked_sessions = state
        .sessions
        .iter()
        .filter(|session| session.status == "blocked")
        .count();
    let active_work_orders = state
        .work_orders
        .iter()
        .filter(|work_order| matches!(work_order.status.as_str(), "assigned" | "blocked"))
        .count();
    let blocked_work_orders = state
        .work_orders
        .iter()
        .filter(|work_order| work_order.status == "blocked")
        .count();
    let audit_recent = recent_audit_records(project_path, 8);
    let capability_registry = json!({
        "capability_source": "dx_registry",
        "mcp_count": registry.len(),
        "category_counts": categories,
    });
    let control_plane_registry = control_plane_registry_value_for_project_path(project_path);

    json!({
        "project": state.project,
        "defaults": state.defaults,
        "provider_policy": {
            "runtime_providers": ["claude", "codex", "gemini", "opencode"],
            "contract_providers": ["shared", "claude", "codex", "gemini", "opencode"],
            "rules": provider_policy_matrix(),
        },
        "adoptions": {
            "total": state.adoptions.len(),
            "active": active_adoptions,
            "recent": recent_adoptions,
        },
        "debates": {
            "total": state.debates.len(),
            "open": open_debates,
            "decided": decided_debates,
            "recent": recent,
        },
        "registry": capability_registry.clone(),
        "capability_registry": capability_registry,
        "control_plane_registry": control_plane_registry,
        "storage": control_plane_storage_summary(project_path),
        "runtime_contract": {
            "auth": control_auth_contract(),
            "launch_broker": {
                "name": "dx_runtime_broker",
                "adapters": crate::runtime_broker::adapter_inventory(),
                "providers": crate::runtime_broker::provider_inventory(),
            },
            "runtime_substrate": "custom_pty_target",
            "runtime_adapter": "pty_native_adapter",
            "runtime_adapters": ["pty_native_adapter", "tmux_migration_adapter"],
            "provider_native_launch": true,
            "runtime_providers": ["claude", "codex", "gemini", "opencode"],
            "browser_port_base": crate::config::browser_port_base(),
            "browser_port_formula": "browser_port_base + pane",
            "control_endpoints": {
                "session_launch": "/api/dxos/session/launch",
                "pane_talk": "/api/pane/talk",
                "pane_kill": "/api/pane/kill",
                "pane_restart": "/api/pane/restart",
                "pane_output": "/api/pane/{id}/output",
                "event_stream": "/api/events",
                "websocket": "/ws",
            },
        },
        "sessions": {
            "total": state.sessions.len(),
            "active": active_sessions,
            "blocked": blocked_sessions,
            "records": state.sessions.iter().map(session_summary).collect::<Vec<_>>(),
        },
        "delegation": {
            "total_work_orders": state.work_orders.len(),
            "active_work_orders": active_work_orders,
            "blocked_work_orders": blocked_work_orders,
            "recent": state.work_orders.iter().rev().take(10).map(work_order_summary).collect::<Vec<_>>(),
        },
        "audit": {
            "total": audit_record_count(project_path),
            "recent": audit_recent,
        },
        "updated_at": state.updated_at,
    })
}

pub fn debate_list(project_path: &str, project_name: Option<&str>) -> String {
    let state = load_control_plane(project_path, project_name);
    json!({
        "project": state.project,
        "defaults": state.defaults,
        "debates": state.debates.iter().map(debate_summary).collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn session_list(project_path: &str, project_name: Option<&str>) -> String {
    let state = load_control_plane(project_path, project_name);
    json!({
        "project": state.project,
        "adoptions": state.adoptions.iter().map(adoption_summary).collect::<Vec<_>>(),
        "provider_policy": {
            "runtime_providers": ["claude", "codex", "gemini", "opencode"],
            "contract_providers": ["shared", "claude", "codex", "gemini", "opencode"],
            "rules": provider_policy_matrix(),
        },
        "sessions": state.sessions.iter().map(session_summary).collect::<Vec<_>>(),
        "work_orders": state.work_orders.iter().map(work_order_summary).collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn start_project_adoption(
    project_path: &str,
    project_name: Option<&str>,
    summary: Option<&str>,
    objective: Option<&str>,
    feature_id: Option<&str>,
    stage: Option<&str>,
    participants: Vec<String>,
    requested_by: Option<&str>,
) -> String {
    let mut state = load_control_plane(project_path, project_name);
    if let Some(existing) = state
        .adoptions
        .iter()
        .find(|adoption| matches!(adoption.status.as_str(), "active" | "planned"))
    {
        return json!({
            "error": "adoption_in_progress",
            "adoption": adoption_summary(existing),
        })
        .to_string();
    }

    let now = crate::state::now();
    let project = state.project.name.clone();
    let normalized_stage = normalize_stage(stage);
    let feature_id = feature_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let provider_policy = provider_policy_for("lead", Some(&normalized_stage));
    let adoption_id = next_adoption_id(&state);
    let lead_session_id = next_session_id(&state);
    let debate_id = next_debate_id(&state);
    let work_order_id = next_work_order_id(&state);
    let resolved_summary = summary
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Adopt {} into DXOS, reconstruct the current truth, and create the first governed recovery plan.",
                project
            )
        });
    let resolved_objective = objective
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Inventory the existing project state for {}, map active features and stages, identify missing docs and approvals, and produce the first recovery plan with evidence.",
                project
            )
        });
    let participants = if participants.is_empty() {
        vec![
            "recovery-lead".to_string(),
            "design-review".to_string(),
            "qa-review".to_string(),
            "docs-steward".to_string(),
        ]
    } else {
        participants
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
    };

    state.sessions.push(SessionContractRecord {
        id: lead_session_id.clone(),
        status: "planned".to_string(),
        role: "lead".to_string(),
        provider: Some(provider_policy.preferred_provider.clone()),
        model: provider_policy.suggested_models.first().cloned(),
        autonomy_level: "guarded_auto".to_string(),
        objective: resolved_objective.clone(),
        expected_outputs: vec![
            "recovery_assessment".to_string(),
            "feature_map".to_string(),
            "stage_map".to_string(),
            "handoff_plan".to_string(),
        ],
        allowed_capabilities: vec![
            "vision".to_string(),
            "docs".to_string(),
            "git".to_string(),
            "analysis".to_string(),
        ],
        allowed_repos: vec![project_path.to_string()],
        allowed_paths: vec![project_path.to_string()],
        workspace_path: Some(project_path.to_string()),
        branch_name: None,
        browser_port: None,
        pane: None,
        runtime_adapter: Some("pty_native_adapter".to_string()),
        tmux_target: None,
        feature_id: feature_id.clone(),
        stage: Some(normalized_stage.clone()),
        supervisor_session_id: None,
        escalation_policy: Some("human".to_string()),
        policy_violations: Vec::new(),
        last_error: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    state.debates.push(DebateRecord {
        id: debate_id.clone(),
        title: format!("Project adoption council · {}", project),
        objective: resolved_objective.clone(),
        status: "open".to_string(),
        feature_id: feature_id.clone(),
        stage: Some(normalized_stage.clone()),
        participants: participants.clone(),
        proposals: Vec::new(),
        contradictions: Vec::new(),
        votes: Vec::new(),
        decision: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    state.work_orders.push(WorkOrderRecord {
        id: work_order_id.clone(),
        supervisor_session_id: lead_session_id.clone(),
        worker_session_id: None,
        status: "planned".to_string(),
        escalation_target: "lead".to_string(),
        title: format!("Initial recovery work package · {}", project),
        objective: resolved_objective.clone(),
        feature_id: feature_id.clone(),
        stage: Some(normalized_stage.clone()),
        required_capabilities: vec![
            "vision".to_string(),
            "docs".to_string(),
            "git".to_string(),
            "analysis".to_string(),
        ],
        blockers: Vec::new(),
        requested_permissions: Vec::new(),
        expected_outputs: vec![
            "recovery_assessment".to_string(),
            "feature_map".to_string(),
            "stage_map".to_string(),
            "handoff_plan".to_string(),
        ],
        resolution_notes: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    state.adoptions.push(ProjectAdoptionRecord {
        id: adoption_id.clone(),
        status: "active".to_string(),
        mode: "recovery".to_string(),
        summary: resolved_summary.clone(),
        objective: resolved_objective.clone(),
        last_note: None,
        initial_work_order_id: Some(work_order_id.clone()),
        feature_id: feature_id.clone(),
        stage: normalized_stage.clone(),
        lead_session_id: lead_session_id.clone(),
        debate_id: debate_id.clone(),
        requested_by: requested_by
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        participants: participants.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "adoption_started",
            "project": state.project.name,
            "project_path": project_path,
            "adoption_id": adoption_id,
            "lead_session_id": lead_session_id,
            "debate_id": debate_id,
            "work_order_id": work_order_id,
            "adoption": state.adoptions.iter().find(|item| item.id == adoption_id).map(adoption_summary),
            "session": state.sessions.iter().find(|item| item.id == lead_session_id).map(session_summary),
            "debate": state.debates.iter().find(|item| item.id == debate_id).map(debate_summary),
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn update_project_adoption_status(
    project_path: &str,
    project_name: Option<&str>,
    adoption_id: &str,
    status: &str,
    note: Option<&str>,
) -> String {
    if adoption_id.trim().is_empty() || status.trim().is_empty() {
        return json!({"error": "adoption_id and status required"}).to_string();
    }

    let normalized_status = status.trim().to_lowercase();
    if !matches!(
        normalized_status.as_str(),
        "planned" | "active" | "completed" | "cancelled"
    ) {
        return json!({"error": "status must be planned/active/completed/cancelled"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(adoption) = state
        .adoptions
        .iter_mut()
        .find(|item| item.id == adoption_id.trim())
    else {
        return json!({"error": "adoption_not_found"}).to_string();
    };

    adoption.status = normalized_status.clone();
    adoption.last_note = note
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    adoption.updated_at = crate::state::now();
    state.updated_at = adoption.updated_at.clone();
    let initial_work_order_id = adoption.initial_work_order_id.clone();
    let adoption_note = adoption.last_note.clone();

    if let Some(work_order_id) = initial_work_order_id.as_deref() {
        if let Some(work_order) = state
            .work_orders
            .iter_mut()
            .find(|item| item.id == work_order_id)
        {
            if matches!(normalized_status.as_str(), "completed" | "cancelled") {
                work_order.status = normalized_status.clone();
                if let Some(note) = adoption_note.clone() {
                    work_order.resolution_notes.push(WorkResolutionRecord {
                        message: note,
                        created_at: state.updated_at.clone(),
                    });
                }
                work_order.updated_at = state.updated_at.clone();
            }
        }
    }

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "adoption_status_updated",
            "project": state.project.name,
            "project_path": project_path,
            "adoption_id": adoption_id,
            "adoption": state.adoptions.iter().find(|item| item.id == adoption_id.trim()).map(adoption_summary),
            "work_order": initial_work_order_id.and_then(|id| {
                state
                    .work_orders
                    .iter()
                    .find(|item| item.id == id)
                    .map(work_order_summary)
            }),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_start(
    project_path: &str,
    project_name: Option<&str>,
    title: &str,
    objective: &str,
    stage: Option<&str>,
    feature_id: Option<&str>,
    participants: Vec<String>,
    requested_by: Option<&str>,
) -> String {
    if title.trim().is_empty() || objective.trim().is_empty() {
        return json!({"error": "title and objective required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let now = crate::state::now();
    let debate = DebateRecord {
        id: next_debate_id(&state),
        title: title.trim().to_string(),
        objective: objective.trim().to_string(),
        status: "open".to_string(),
        feature_id: feature_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        stage: stage
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        participants: participants
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect(),
        proposals: Vec::new(),
        contradictions: Vec::new(),
        votes: Vec::new(),
        decision: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    state.updated_at = now.clone();
    let debate_id = debate.id.clone();
    state.debates.push(debate);

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "started",
            "action": "started",
            "project": state.project.name,
            "project_path": project_path,
            "debate_id": debate_id,
            "requested_by": requested_by,
            "debate": state.debates.iter().find(|debate| debate.id == debate_id).map(debate_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_add_proposal(
    project_path: &str,
    project_name: Option<&str>,
    debate_id: &str,
    author: &str,
    model: Option<&str>,
    summary: &str,
    rationale: &str,
    evidence: Vec<String>,
) -> String {
    if debate_id.trim().is_empty()
        || author.trim().is_empty()
        || summary.trim().is_empty()
        || rationale.trim().is_empty()
    {
        return json!({"error": "debate_id, author, summary, and rationale required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(debate) = state
        .debates
        .iter_mut()
        .find(|debate| debate.id == debate_id)
    else {
        return json!({"error": "debate_not_found"}).to_string();
    };
    if debate.status != "open" {
        return json!({"error": "debate_closed"}).to_string();
    }

    if !debate.participants.iter().any(|value| value == author) {
        debate.participants.push(author.to_string());
    }

    let proposal = ProposalRecord {
        id: next_child_id("P", debate.proposals.len()),
        author: author.trim().to_string(),
        model: model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        summary: summary.trim().to_string(),
        rationale: rationale.trim().to_string(),
        evidence: evidence
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect(),
        created_at: crate::state::now(),
    };
    let proposal_id = proposal.id.clone();
    debate.updated_at = crate::state::now();
    debate.proposals.push(proposal);
    state.updated_at = debate.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "proposal_added",
            "project": state.project.name,
            "project_path": project_path,
            "debate_id": debate_id,
            "proposal_id": proposal_id,
            "debate": state.debates.iter().find(|debate| debate.id == debate_id).map(debate_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_add_contradiction(
    project_path: &str,
    project_name: Option<&str>,
    debate_id: &str,
    proposal_id: &str,
    author: &str,
    model: Option<&str>,
    rationale: &str,
) -> String {
    if debate_id.trim().is_empty()
        || proposal_id.trim().is_empty()
        || author.trim().is_empty()
        || rationale.trim().is_empty()
    {
        return json!({"error": "debate_id, proposal_id, author, and rationale required"})
            .to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(debate) = state
        .debates
        .iter_mut()
        .find(|debate| debate.id == debate_id)
    else {
        return json!({"error": "debate_not_found"}).to_string();
    };
    if debate.status != "open" {
        return json!({"error": "debate_closed"}).to_string();
    }
    if !debate
        .proposals
        .iter()
        .any(|proposal| proposal.id == proposal_id)
    {
        return json!({"error": "proposal_not_found"}).to_string();
    }

    if !debate.participants.iter().any(|value| value == author) {
        debate.participants.push(author.to_string());
    }

    let contradiction = ContradictionRecord {
        id: next_child_id("C", debate.contradictions.len()),
        proposal_id: proposal_id.trim().to_string(),
        author: author.trim().to_string(),
        model: model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        rationale: rationale.trim().to_string(),
        created_at: crate::state::now(),
    };
    let contradiction_id = contradiction.id.clone();
    debate.updated_at = crate::state::now();
    debate.contradictions.push(contradiction);
    state.updated_at = debate.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "contradiction_added",
            "project": state.project.name,
            "project_path": project_path,
            "debate_id": debate_id,
            "contradiction_id": contradiction_id,
            "debate": state.debates.iter().find(|debate| debate.id == debate_id).map(debate_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_cast_vote(
    project_path: &str,
    project_name: Option<&str>,
    debate_id: &str,
    proposal_id: &str,
    voter: &str,
    model: Option<&str>,
    stance: &str,
    rationale: &str,
) -> String {
    if debate_id.trim().is_empty()
        || proposal_id.trim().is_empty()
        || voter.trim().is_empty()
        || stance.trim().is_empty()
    {
        return json!({"error": "debate_id, proposal_id, voter, and stance required"}).to_string();
    }

    let stance = stance.trim().to_lowercase();
    if !matches!(stance.as_str(), "support" | "oppose" | "abstain") {
        return json!({"error": "stance must be support/oppose/abstain"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(debate) = state
        .debates
        .iter_mut()
        .find(|debate| debate.id == debate_id)
    else {
        return json!({"error": "debate_not_found"}).to_string();
    };
    if debate.status != "open" {
        return json!({"error": "debate_closed"}).to_string();
    }
    if !debate
        .proposals
        .iter()
        .any(|proposal| proposal.id == proposal_id)
    {
        return json!({"error": "proposal_not_found"}).to_string();
    }

    if !debate.participants.iter().any(|value| value == voter) {
        debate.participants.push(voter.to_string());
    }

    debate.votes.retain(|vote| vote.voter != voter);
    let vote = VoteRecord {
        id: next_child_id("V", debate.votes.len()),
        proposal_id: proposal_id.trim().to_string(),
        voter: voter.trim().to_string(),
        model: model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        stance,
        rationale: rationale.trim().to_string(),
        created_at: crate::state::now(),
    };
    debate.updated_at = crate::state::now();
    debate.votes.push(vote);
    state.updated_at = debate.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "vote_cast",
            "project": state.project.name,
            "project_path": project_path,
            "debate_id": debate_id,
            "debate": state.debates.iter().find(|debate| debate.id == debate_id).map(debate_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_finalize(
    project_path: &str,
    project_name: Option<&str>,
    debate_id: &str,
    chosen_proposal_id: &str,
    decided_by: &str,
    summary: &str,
    rationale: &str,
) -> String {
    if debate_id.trim().is_empty()
        || chosen_proposal_id.trim().is_empty()
        || decided_by.trim().is_empty()
        || summary.trim().is_empty()
        || rationale.trim().is_empty()
    {
        return json!({
            "error": "debate_id, chosen_proposal_id, decided_by, summary, and rationale required"
        })
        .to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(debate) = state
        .debates
        .iter_mut()
        .find(|debate| debate.id == debate_id)
    else {
        return json!({"error": "debate_not_found"}).to_string();
    };
    if debate.status != "open" {
        return json!({"error": "debate_closed"}).to_string();
    }
    if !debate
        .proposals
        .iter()
        .any(|proposal| proposal.id == chosen_proposal_id)
    {
        return json!({"error": "proposal_not_found"}).to_string();
    }

    let now = crate::state::now();
    debate.status = "decided".to_string();
    debate.updated_at = now.clone();
    debate.decision = Some(DecisionRecord {
        chosen_proposal_id: chosen_proposal_id.trim().to_string(),
        decided_by: decided_by.trim().to_string(),
        summary: summary.trim().to_string(),
        rationale: rationale.trim().to_string(),
        created_at: now.clone(),
    });
    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "decision_finalized",
            "project": state.project.name,
            "project_path": project_path,
            "debate_id": debate_id,
            "debate": state.debates.iter().find(|debate| debate.id == debate_id).map(debate_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn upsert_session_contract(
    project_path: &str,
    project_name: Option<&str>,
    session_id: Option<&str>,
    role: &str,
    provider: Option<&str>,
    model: Option<&str>,
    autonomy_level: Option<&str>,
    objective: &str,
    expected_outputs: Vec<String>,
    allowed_capabilities: Vec<String>,
    allowed_repos: Vec<String>,
    allowed_paths: Vec<String>,
    workspace_path: Option<&str>,
    branch_name: Option<&str>,
    browser_port: Option<u16>,
    pane: Option<u8>,
    runtime_adapter: Option<&str>,
    tmux_target: Option<&str>,
    feature_id: Option<&str>,
    stage: Option<&str>,
    supervisor_session_id: Option<&str>,
    escalation_policy: Option<&str>,
    status: Option<&str>,
) -> String {
    if role.trim().is_empty() || objective.trim().is_empty() {
        return json!({"error": "role and objective required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let now = crate::state::now();
    let chosen_id = session_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| next_session_id(&state));
    let (normalized_provider, provider_policy, mut policy_violations) =
        validate_provider_selection(role, stage, provider, pane, tmux_target);
    let desired_status = status.unwrap_or("active").trim().to_string();
    let computed_status = if policy_violations.is_empty() {
        desired_status.clone()
    } else if matches!(desired_status.as_str(), "idle" | "completed") {
        desired_status.clone()
    } else {
        "blocked".to_string()
    };

    let action = if let Some(existing) = state.sessions.iter_mut().find(|item| item.id == chosen_id)
    {
        existing.status = computed_status;
        existing.role = role.trim().to_string();
        existing.provider = normalized_provider.clone();
        existing.model = model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.autonomy_level = autonomy_level.unwrap_or("guarded_auto").trim().to_string();
        existing.objective = objective.trim().to_string();
        existing.expected_outputs = expected_outputs
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect();
        existing.allowed_capabilities = allowed_capabilities
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect();
        existing.allowed_repos = allowed_repos
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect();
        existing.allowed_paths = allowed_paths
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect();
        existing.workspace_path = workspace_path
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.branch_name = branch_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.browser_port = browser_port;
        existing.pane = pane;
        existing.runtime_adapter = runtime_adapter
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.tmux_target = tmux_target
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.feature_id = feature_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.stage = stage
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.supervisor_session_id = supervisor_session_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.escalation_policy = escalation_policy
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        existing.policy_violations.clear();
        existing.policy_violations.append(&mut policy_violations);
        existing.last_error = None;
        existing.updated_at = now.clone();
        "session_updated"
    } else {
        state.sessions.push(SessionContractRecord {
            id: chosen_id.clone(),
            status: computed_status,
            role: role.trim().to_string(),
            provider: normalized_provider.clone(),
            model: model
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            autonomy_level: autonomy_level.unwrap_or("guarded_auto").trim().to_string(),
            objective: objective.trim().to_string(),
            expected_outputs: expected_outputs
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect(),
            allowed_capabilities: allowed_capabilities
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect(),
            allowed_repos: allowed_repos
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect(),
            allowed_paths: allowed_paths
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect(),
            workspace_path: workspace_path
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            branch_name: branch_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            browser_port,
            pane,
            runtime_adapter: runtime_adapter
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            tmux_target: tmux_target
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            feature_id: feature_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            stage: stage
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            supervisor_session_id: supervisor_session_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            escalation_policy: escalation_policy
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            policy_violations,
            last_error: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        "session_registered"
    };

    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": action,
            "project": state.project.name,
            "project_path": project_path,
            "session_id": chosen_id,
            "provider_policy": provider_policy,
            "session": state.sessions.iter().find(|item| item.id == chosen_id).map(session_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn update_session_status(
    project_path: &str,
    project_name: Option<&str>,
    session_id: &str,
    status: &str,
    note: Option<&str>,
) -> String {
    if session_id.trim().is_empty() || status.trim().is_empty() {
        return json!({"error": "session_id and status required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(session) = state
        .sessions
        .iter_mut()
        .find(|item| item.id == session_id.trim())
    else {
        return json!({"error": "session_not_found"}).to_string();
    };

    session.status = status.trim().to_string();
    if !matches!(session.status.as_str(), "blocked" | "failed") {
        session.last_error = None;
    }
    if let Some(note) = note.filter(|value| !value.trim().is_empty()) {
        session.objective = format!(
            "{}\n\nStatus note: {}",
            session.objective.trim(),
            note.trim()
        );
    }
    session.updated_at = crate::state::now();
    state.updated_at = session.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_status_updated",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn record_session_launch_failure(
    project_path: &str,
    project_name: Option<&str>,
    session_id: &str,
    error: &str,
) -> String {
    if session_id.trim().is_empty() || error.trim().is_empty() {
        return json!({"error": "session_id and error required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(session) = state
        .sessions
        .iter_mut()
        .find(|item| item.id == session_id.trim())
    else {
        return json!({"error": "session_not_found"}).to_string();
    };

    session.status = "blocked".to_string();
    session.last_error = Some(error.trim().to_string());
    session.updated_at = crate::state::now();
    state.updated_at = session.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_launch_failed",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn delegate_work_order(
    project_path: &str,
    project_name: Option<&str>,
    supervisor_session_id: &str,
    worker_session_id: Option<&str>,
    title: &str,
    objective: &str,
    feature_id: Option<&str>,
    stage: Option<&str>,
    required_capabilities: Vec<String>,
    expected_outputs: Vec<String>,
) -> String {
    if supervisor_session_id.trim().is_empty()
        || title.trim().is_empty()
        || objective.trim().is_empty()
    {
        return json!({
            "error": "supervisor_session_id, title, and objective required"
        })
        .to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    if !state
        .sessions
        .iter()
        .any(|session| session.id == supervisor_session_id.trim())
    {
        return json!({"error": "supervisor_session_not_found"}).to_string();
    }
    if let Some(worker) = worker_session_id.filter(|value| !value.trim().is_empty()) {
        if !state
            .sessions
            .iter()
            .any(|session| session.id == worker.trim())
        {
            return json!({"error": "worker_session_not_found"}).to_string();
        }
    }

    let now = crate::state::now();
    let work_order_id = next_work_order_id(&state);
    state.work_orders.push(WorkOrderRecord {
        id: work_order_id.clone(),
        supervisor_session_id: supervisor_session_id.trim().to_string(),
        worker_session_id: worker_session_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        status: if worker_session_id.is_some() {
            "assigned".to_string()
        } else {
            "planned".to_string()
        },
        escalation_target: "lead".to_string(),
        title: title.trim().to_string(),
        objective: objective.trim().to_string(),
        feature_id: feature_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        stage: stage
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        required_capabilities: required_capabilities
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect(),
        blockers: Vec::new(),
        requested_permissions: Vec::new(),
        expected_outputs: expected_outputs
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect(),
        resolution_notes: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });
    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "work_order_delegated",
            "project": state.project.name,
            "project_path": project_path,
            "work_order_id": work_order_id,
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn raise_session_blocker(
    project_path: &str,
    project_name: Option<&str>,
    worker_session_id: &str,
    blocker: &str,
    requested_permission: Option<&str>,
    resolution_hint: Option<&str>,
) -> String {
    if worker_session_id.trim().is_empty() || blocker.trim().is_empty() {
        return json!({"error": "worker_session_id and blocker required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(session_index) = state
        .sessions
        .iter()
        .position(|item| item.id == worker_session_id.trim())
    else {
        return json!({"error": "session_not_found"}).to_string();
    };

    let worker_session = state.sessions[session_index].clone();
    let routed_lead = worker_session
        .supervisor_session_id
        .clone()
        .filter(|value| !value.trim().is_empty() && value != worker_session_id.trim())
        .filter(|lead_id| state.sessions.iter().any(|session| &session.id == lead_id));
    let escalation_target = if routed_lead.is_some() {
        "lead".to_string()
    } else {
        "human".to_string()
    };
    let broker_session_id = routed_lead.unwrap_or_else(|| worker_session.id.clone());
    let now = crate::state::now();

    let existing_index = state.work_orders.iter().rposition(|item| {
        item.worker_session_id.as_deref() == Some(worker_session_id.trim())
            && matches!(item.status.as_str(), "planned" | "assigned" | "blocked")
    });

    let work_order_id = if let Some(index) = existing_index {
        let work_order = &mut state.work_orders[index];
        work_order.status = "blocked".to_string();
        work_order.supervisor_session_id = broker_session_id.clone();
        work_order.escalation_target = escalation_target.clone();
        if !work_order
            .blockers
            .iter()
            .any(|item| item == blocker.trim())
        {
            work_order.blockers.push(blocker.trim().to_string());
        }
        if let Some(permission) = requested_permission.filter(|value| !value.trim().is_empty()) {
            if !work_order
                .requested_permissions
                .iter()
                .any(|item| item == permission.trim())
            {
                work_order
                    .requested_permissions
                    .push(permission.trim().to_string());
            }
        }
        if let Some(hint) = resolution_hint.filter(|value| !value.trim().is_empty()) {
            let hint_line = format!("Resolution hint: {}", hint.trim());
            if !work_order
                .expected_outputs
                .iter()
                .any(|item| item == &hint_line)
            {
                work_order.expected_outputs.push(hint_line);
            }
        }
        work_order.updated_at = now.clone();
        work_order.id.clone()
    } else {
        let work_order_id = next_work_order_id(&state);
        let mut expected_outputs = vec!["lead_guidance".to_string()];
        if let Some(permission) = requested_permission.filter(|value| !value.trim().is_empty()) {
            expected_outputs.push(format!("permission: {}", permission.trim()));
        }
        if let Some(hint) = resolution_hint.filter(|value| !value.trim().is_empty()) {
            expected_outputs.push(format!("resolution_hint: {}", hint.trim()));
        }
        state.work_orders.push(WorkOrderRecord {
            id: work_order_id.clone(),
            supervisor_session_id: broker_session_id.clone(),
            worker_session_id: Some(worker_session.id.clone()),
            status: "blocked".to_string(),
            escalation_target: escalation_target.clone(),
            title: format!("Broker: {} blocked", worker_session.role),
            objective: format!(
                "Unblock {} on {} and route the decision through DXOS.",
                worker_session.id,
                worker_session
                    .feature_id
                    .clone()
                    .unwrap_or_else(|| "current work".to_string())
            ),
            feature_id: worker_session.feature_id.clone(),
            stage: worker_session.stage.clone(),
            required_capabilities: requested_permission
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .into_iter()
                .collect(),
            blockers: vec![blocker.trim().to_string()],
            requested_permissions: requested_permission
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .into_iter()
                .collect(),
            expected_outputs,
            resolution_notes: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        work_order_id
    };

    let session = &mut state.sessions[session_index];
    session.status = "blocked".to_string();
    session.updated_at = now.clone();
    state.updated_at = now.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_blocker_raised",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": worker_session_id,
            "work_order_id": work_order_id,
            "escalation_target": escalation_target,
            "routed_to": if broker_session_id == worker_session_id.trim() { "human".to_string() } else { broker_session_id.clone() },
            "session": state.sessions.iter().find(|item| item.id == worker_session_id.trim()).map(session_summary),
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn work_order_block(
    project_path: &str,
    project_name: Option<&str>,
    work_order_id: &str,
    blocker: &str,
    requested_permission: Option<&str>,
) -> String {
    if work_order_id.trim().is_empty() || blocker.trim().is_empty() {
        return json!({"error": "work_order_id and blocker required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(work_order) = state
        .work_orders
        .iter_mut()
        .find(|item| item.id == work_order_id.trim())
    else {
        return json!({"error": "work_order_not_found"}).to_string();
    };

    work_order.status = "blocked".to_string();
    if !work_order
        .blockers
        .iter()
        .any(|item| item == blocker.trim())
    {
        work_order.blockers.push(blocker.trim().to_string());
    }
    if let Some(permission) = requested_permission.filter(|value| !value.trim().is_empty()) {
        if !work_order
            .requested_permissions
            .iter()
            .any(|item| item == permission.trim())
        {
            work_order
                .requested_permissions
                .push(permission.trim().to_string());
        }
    }
    work_order.updated_at = crate::state::now();
    state.updated_at = work_order.updated_at.clone();
    let worker_session_id = work_order.worker_session_id.clone();

    if let Some(worker_session_id) = worker_session_id {
        if let Some(session) = state
            .sessions
            .iter_mut()
            .find(|item| item.id == worker_session_id)
        {
            session.status = "blocked".to_string();
            session.updated_at = state.updated_at.clone();
        }
    }

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "work_order_blocked",
            "project": state.project.name,
            "project_path": project_path,
            "work_order_id": work_order_id,
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id.trim()).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn resolve_work_order(
    project_path: &str,
    project_name: Option<&str>,
    work_order_id: &str,
    resolution: Option<&str>,
) -> String {
    if work_order_id.trim().is_empty() {
        return json!({"error": "work_order_id required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(work_order) = state
        .work_orders
        .iter_mut()
        .find(|item| item.id == work_order_id.trim())
    else {
        return json!({"error": "work_order_not_found"}).to_string();
    };

    work_order.status = "assigned".to_string();
    work_order.blockers.clear();
    work_order.requested_permissions.clear();
    if let Some(resolution) = resolution.filter(|value| !value.trim().is_empty()) {
        work_order.resolution_notes.push(WorkResolutionRecord {
            message: resolution.trim().to_string(),
            created_at: crate::state::now(),
        });
    }
    work_order.updated_at = crate::state::now();
    state.updated_at = work_order.updated_at.clone();
    let worker_session_id = work_order.worker_session_id.clone();

    if let Some(worker_session_id) = worker_session_id {
        if let Some(session) = state
            .sessions
            .iter_mut()
            .find(|item| item.id == worker_session_id)
        {
            session.status = "active".to_string();
            session.last_error = None;
            session.updated_at = state.updated_at.clone();
        }
    }

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "work_order_resolved",
            "project": state.project.name,
            "project_path": project_path,
            "work_order_id": work_order_id,
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id.trim()).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn record_session_delivery_failure(
    project_path: &str,
    project_name: Option<&str>,
    session_id: &str,
    error: &str,
) -> String {
    if session_id.trim().is_empty() || error.trim().is_empty() {
        return json!({"error": "session_id and error required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(session) = state
        .sessions
        .iter_mut()
        .find(|item| item.id == session_id.trim())
    else {
        return json!({"error": "session_not_found"}).to_string();
    };

    session.status = "blocked".to_string();
    session.last_error = Some(error.trim().to_string());
    session.updated_at = crate::state::now();
    state.updated_at = session.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_delivery_failed",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn debate_event_from_result(project_path: &str, result: &str) -> Option<StateEvent> {
    let value = serde_json::from_str::<Value>(result).ok()?;
    if value.get("error").is_some() {
        return None;
    }
    let debate = value.get("debate")?;
    let project = value
        .get("project")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| resolved_project_name(project_path, None));
    Some(StateEvent::DebateChanged {
        project,
        debate_id: value
            .get("debate_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        title: debate
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        status: debate
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        action: value
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("updated")
            .to_string(),
    })
}

pub fn session_event_from_result(project_path: &str, result: &str) -> Option<StateEvent> {
    let value = serde_json::from_str::<Value>(result).ok()?;
    if value.get("error").is_some() {
        return None;
    }
    let project = value
        .get("project")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| resolved_project_name(project_path, None));

    if let Some(session) = value.get("session") {
        return Some(StateEvent::SessionContractChanged {
            project,
            session_id: value
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            role: session
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            status: session
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            action: value
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("session_updated")
                .to_string(),
        });
    }

    let work_order = value.get("work_order")?;
    Some(StateEvent::SessionContractChanged {
        project,
        session_id: work_order
            .get("worker_session_id")
            .and_then(Value::as_str)
            .or_else(|| {
                work_order
                    .get("supervisor_session_id")
                    .and_then(Value::as_str)
            })
            .unwrap_or("")
            .to_string(),
        role: "delegation".to_string(),
        status: work_order
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        action: value
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("work_order_updated")
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn control_plane_defaults_are_project_scoped() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let state = load_control_plane(project_path.to_str().unwrap(), Some("demo"));
        assert_eq!(state.project.name, "demo");
        assert_eq!(state.defaults.capability_source, "dx_registry");
        assert_eq!(state.defaults.runtime_substrate, "custom_pty");
    }

    #[test]
    fn debate_roundtrip_persists_and_tallies_votes() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let started = debate_start(
            project,
            Some("demo"),
            "Choose runtime substrate",
            "Pick the first serious runtime layer",
            Some("design"),
            Some("F1.1"),
            vec!["lead".to_string(), "gpt-5.4".to_string()],
            Some("lead"),
        );
        let started_value: Value = serde_json::from_str(&started).unwrap();
        let debate_id = started_value["debate_id"].as_str().unwrap();

        let proposal_a = debate_add_proposal(
            project,
            Some("demo"),
            debate_id,
            "claude-opus",
            Some("claude-opus-4.6"),
            "Use custom PTY runtime",
            "Better long-term substrate",
            vec!["bench/runtime.md".to_string()],
        );
        let proposal_a_value: Value = serde_json::from_str(&proposal_a).unwrap();
        let proposal_a_id = proposal_a_value["proposal_id"].as_str().unwrap();

        let proposal_b = debate_add_proposal(
            project,
            Some("demo"),
            debate_id,
            "gpt-5.4",
            Some("gpt-5.4"),
            "Keep tmux primary",
            "Ship faster immediately",
            vec![],
        );
        let proposal_b_value: Value = serde_json::from_str(&proposal_b).unwrap();
        let proposal_b_id = proposal_b_value["proposal_id"].as_str().unwrap();

        let contradiction = debate_add_contradiction(
            project,
            Some("demo"),
            debate_id,
            proposal_b_id,
            "codex",
            Some("codex"),
            "Tmux should be migration-only or architecture stays provider-bound.",
        );
        assert!(contradiction.contains("contradiction_added"));

        let vote_one = debate_cast_vote(
            project,
            Some("demo"),
            debate_id,
            proposal_a_id,
            "lead",
            Some("claude-opus-4.6"),
            "support",
            "Best long-term path",
        );
        assert!(vote_one.contains("vote_cast"));

        let vote_two = debate_cast_vote(
            project,
            Some("demo"),
            debate_id,
            proposal_b_id,
            "lead",
            Some("claude-opus-4.6"),
            "oppose",
            "Overwrites prior vote for same voter",
        );
        let vote_two_value: Value = serde_json::from_str(&vote_two).unwrap();
        let tallies = &vote_two_value["debate"]["tallies"];
        assert_eq!(tallies[proposal_a_id]["support"].as_u64().unwrap_or(0), 0);
        assert_eq!(tallies[proposal_b_id]["oppose"].as_u64().unwrap_or(0), 1);

        let decided = debate_finalize(
            project,
            Some("demo"),
            debate_id,
            proposal_a_id,
            "lead",
            "Custom PTY wins",
            "It aligns with the long-term control-plane design.",
        );
        let decided_value: Value = serde_json::from_str(&decided).unwrap();
        assert_eq!(decided_value["debate"]["status"], "decided");

        let listed: Value = serde_json::from_str(&debate_list(project, Some("demo"))).unwrap();
        assert_eq!(
            listed["debates"][0]["decision"]["chosen_proposal_id"],
            proposal_a_id
        );
    }

    #[test]
    fn session_contract_and_work_order_roundtrip() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let lead = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "architect",
            Some("claude"),
            Some("claude-opus-4.6"),
            Some("guarded_auto"),
            "Lead the runtime redesign",
            vec!["decision record".to_string()],
            vec!["playwright".to_string(), "git".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/runtime"),
            Some(46001),
            Some(1),
            Some("tmux_migration_adapter"),
            Some("dx:1.1"),
            Some("F1.1"),
            Some("design"),
            None,
            Some("lead_then_human"),
            Some("active"),
        );
        let lead_value: Value = serde_json::from_str(&lead).unwrap();
        let lead_id = lead_value["session_id"].as_str().unwrap();

        let worker = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "frontend",
            Some("openai"),
            Some("gpt-5.4"),
            Some("guarded_auto"),
            "Build the shell glass layer",
            vec!["prototype".to_string()],
            vec!["playwright".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/glass"),
            Some(46002),
            Some(2),
            Some("tmux_migration_adapter"),
            Some("dx:2.1"),
            Some("F1.1"),
            Some("build"),
            Some(lead_id),
            Some("lead_then_human"),
            Some("active"),
        );
        let worker_value: Value = serde_json::from_str(&worker).unwrap();
        let worker_id = worker_value["session_id"].as_str().unwrap();

        let work = delegate_work_order(
            project,
            Some("demo"),
            lead_id,
            Some(worker_id),
            "Prototype glass shell",
            "Produce a modern operator shell prototype",
            Some("F1.1"),
            Some("build"),
            vec!["playwright".to_string()],
            vec!["prototype".to_string(), "screenshots".to_string()],
        );
        let work_value: Value = serde_json::from_str(&work).unwrap();
        let work_order_id = work_value["work_order_id"].as_str().unwrap();

        let blocked = work_order_block(
            project,
            Some("demo"),
            work_order_id,
            "Needs browser permission",
            Some("browser_control"),
        );
        let blocked_value: Value = serde_json::from_str(&blocked).unwrap();
        assert_eq!(blocked_value["work_order"]["status"], "blocked");
        assert_eq!(
            blocked_value["work_order"]["requested_permissions"][0],
            "browser_control"
        );
        let listed_after_block: Value =
            serde_json::from_str(&session_list(project, Some("demo"))).unwrap();
        let worker_after_block = listed_after_block["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == worker_id)
            .unwrap();
        assert_eq!(worker_after_block["status"], "blocked");

        let resolved = resolve_work_order(
            project,
            Some("demo"),
            work_order_id,
            Some("Permission granted by lead"),
        );
        let resolved_value: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(resolved_value["work_order"]["status"], "assigned");
        assert_eq!(
            resolved_value["work_order"]["last_resolution"],
            "Permission granted by lead"
        );
        assert_eq!(
            resolved_value["work_order"]["resolution_notes"][0]["message"],
            "Permission granted by lead"
        );

        let listed_after_resolve: Value =
            serde_json::from_str(&session_list(project, Some("demo"))).unwrap();
        let worker_after_resolve = listed_after_resolve["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == worker_id)
            .unwrap();
        assert_eq!(worker_after_resolve["status"], "active");

        let completed = update_session_status(
            project,
            Some("demo"),
            worker_id,
            "completed",
            Some("Prototype shipped"),
        );
        let completed_value: Value = serde_json::from_str(&completed).unwrap();
        assert_eq!(completed_value["session"]["status"], "completed");

        let listed: Value = serde_json::from_str(&session_list(project, Some("demo"))).unwrap();
        assert_eq!(listed["sessions"].as_array().unwrap().len(), 2);
        assert_eq!(listed["work_orders"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn provider_policy_blocks_invalid_runtime_choice() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let result = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "security",
            Some("opencode"),
            Some("local-security-model"),
            Some("guarded_auto"),
            "Review production hardening",
            vec!["security review".to_string()],
            vec!["security_scan".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/security"),
            Some(46003),
            Some(3),
            Some("pty_native_adapter"),
            Some("dx:3.1"),
            Some("F2.4"),
            Some("test"),
            None,
            Some("lead_then_human"),
            Some("launching"),
        );
        let value: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["session"]["status"], "blocked");
        assert_eq!(value["provider_policy"]["preferred_provider"], "claude");
        assert!(value["session"]["policy_violations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap_or("").contains("outside DXOS policy")));
    }

    #[test]
    fn launch_failure_persists_session_error() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let session = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "frontend",
            Some("codex"),
            Some("gpt-5.4"),
            Some("guarded_auto"),
            "Build glass shell",
            vec!["prototype".to_string()],
            vec!["playwright".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/glass"),
            Some(46002),
            Some(2),
            Some("pty_native_adapter"),
            None,
            Some("F1.2"),
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("launching"),
        );
        let session_value: Value = serde_json::from_str(&session).unwrap();
        let session_id = session_value["session_id"].as_str().unwrap();

        let failed = record_session_launch_failure(
            project,
            Some("demo"),
            session_id,
            "Runtime launch failed: codex binary missing",
        );
        let failed_value: Value = serde_json::from_str(&failed).unwrap();
        assert_eq!(failed_value["session"]["status"], "blocked");
        assert_eq!(
            failed_value["session"]["last_error"],
            "Runtime launch failed: codex binary missing"
        );

        let listed: Value = serde_json::from_str(&session_list(project, Some("demo"))).unwrap();
        assert_eq!(
            listed["sessions"][0]["last_error"],
            "Runtime launch failed: codex binary missing"
        );
    }

    #[test]
    fn session_blocker_routes_to_lead_before_human() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let lead = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "lead",
            Some("claude"),
            Some("claude-opus-4.6"),
            Some("guarded_auto"),
            "Coordinate delivery",
            vec!["guidance".to_string()],
            vec!["docs".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/main"),
            Some(46001),
            Some(1),
            Some("tmux_migration_adapter"),
            Some("dx:1.1"),
            Some("F3.1"),
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("active"),
        );
        let lead_value: Value = serde_json::from_str(&lead).unwrap();
        let lead_id = lead_value["session_id"].as_str().unwrap();

        let worker = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "frontend",
            Some("codex"),
            Some("gpt-5.4"),
            Some("guarded_auto"),
            "Build the approval modal",
            vec!["prototype".to_string()],
            vec!["playwright".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/modal"),
            Some(46002),
            Some(2),
            Some("tmux_migration_adapter"),
            Some("dx:2.1"),
            Some("F3.1"),
            Some("build"),
            Some(lead_id),
            Some("lead_then_human"),
            Some("active"),
        );
        let worker_value: Value = serde_json::from_str(&worker).unwrap();
        let worker_id = worker_value["session_id"].as_str().unwrap();

        let blocked = raise_session_blocker(
            project,
            Some("demo"),
            worker_id,
            "Needs browser login approval",
            Some("browser_login"),
            Some("Approve login and continue"),
        );
        let blocked_value: Value = serde_json::from_str(&blocked).unwrap();
        assert_eq!(blocked_value["session"]["status"], "blocked");
        assert_eq!(blocked_value["work_order"]["status"], "blocked");
        assert_eq!(blocked_value["escalation_target"], "lead");
        assert_eq!(blocked_value["routed_to"], lead_id);
        assert_eq!(
            blocked_value["work_order"]["requested_permissions"][0],
            "browser_login"
        );
    }

    #[test]
    fn sqlite_store_becomes_canonical_and_registry_lists_project() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let result = upsert_session_contract(
            project,
            Some("demo"),
            None,
            "lead",
            Some("claude"),
            Some("claude-opus-4.6"),
            Some("guarded_auto"),
            "Coordinate delivery",
            vec!["brief".to_string()],
            vec!["docs".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            Some("feat/main"),
            Some(46001),
            Some(1),
            Some("pty_native_adapter"),
            None,
            Some("F4.1"),
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("active"),
        );
        let value: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["session"]["status"], "active");

        let db_path = control_plane_store_path(project);
        assert!(db_path.exists());
        let registry: Value =
            serde_json::from_str(&control_plane_registry_for_project(project)).unwrap();
        assert_eq!(registry["backend"], "sqlite_with_repo_mirror");
        assert!(registry["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == project));
    }

    #[test]
    fn legacy_repo_json_is_imported_into_sqlite_store() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(control_plane_dir(project_path.to_str().unwrap())).unwrap();
        let state = default_state(project_path.to_str().unwrap(), Some("demo"));
        let legacy_json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(
            control_plane_file(project_path.to_str().unwrap()),
            legacy_json,
        )
        .unwrap();

        let loaded = load_control_plane(project_path.to_str().unwrap(), Some("demo"));
        assert_eq!(loaded.project.name, "demo");
        assert!(control_plane_store_path(project_path.to_str().unwrap()).exists());

        let registry: Value = serde_json::from_str(&control_plane_registry_for_project(
            project_path.to_str().unwrap(),
        ))
        .unwrap();
        assert!(registry["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == project_path.to_str().unwrap()));
    }

    #[test]
    fn audit_records_are_persisted_in_sqlite_store() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let record = append_audit_record(
            project,
            Some("demo"),
            "portal-operator",
            "session_launch",
            "auto",
            "ok",
            "session_launch auto",
            json!({"pane": 2}),
        )
        .unwrap();
        assert_eq!(record.project_name, "demo");

        let audit: Value = serde_json::from_str(&audit_list(project, Some("demo"), 10)).unwrap();
        assert_eq!(audit["audit"]["total"], 1);
        assert_eq!(audit["audit"]["recent"][0]["action_kind"], "session_launch");
    }

    #[test]
    fn control_plane_snapshot_includes_recent_audit_records() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        append_audit_record(
            project,
            Some("demo"),
            "portal-operator",
            "pane_restart",
            "pane:3",
            "error",
            "pane_restart failed for pane:3",
            json!({"error": "runtime unavailable"}),
        )
        .unwrap();

        let snapshot = control_plane_snapshot(project, Some("demo"));
        assert_eq!(snapshot["audit"]["total"], 1);
        assert_eq!(snapshot["audit"]["recent"][0]["target"], "pane:3");
        assert_eq!(snapshot["audit"]["recent"][0]["outcome"], "error");
    }

    #[test]
    fn project_adoption_seeds_recovery_session_and_council() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let value: Value = serde_json::from_str(&start_project_adoption(
            project,
            Some("demo"),
            Some("Recover the inherited project"),
            Some("Map the current state and create the first governed recovery plan."),
            Some("F1.1"),
            Some("discovery"),
            vec!["lead".to_string(), "qa".to_string()],
            Some("ops-lead"),
        ))
        .unwrap();

        assert_eq!(value["action"], "adoption_started");
        assert!(value["adoption_id"].as_str().unwrap().starts_with("AD"));
        assert!(value["lead_session_id"].as_str().unwrap().starts_with("SX"));
        assert!(value["debate_id"].as_str().unwrap().starts_with("DB"));
        assert!(value["work_order_id"].as_str().unwrap().starts_with("WO"));

        let snapshot = control_plane_snapshot(project, Some("demo"));
        assert_eq!(snapshot["adoptions"]["total"], 1);
        assert_eq!(snapshot["adoptions"]["active"], 1);
        assert_eq!(snapshot["sessions"]["total"], 1);
        assert_eq!(snapshot["debates"]["total"], 1);
        assert_eq!(snapshot["delegation"]["total_work_orders"], 1);
        assert_eq!(
            snapshot["adoptions"]["recent"][0]["lead_session_id"],
            value["lead_session_id"]
        );
        assert_eq!(
            snapshot["adoptions"]["recent"][0]["initial_work_order_id"],
            value["work_order_id"]
        );
    }

    #[test]
    fn project_adoption_status_can_be_completed() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let started: Value = serde_json::from_str(&start_project_adoption(
            project,
            Some("demo"),
            None,
            None,
            None,
            Some("discovery"),
            Vec::new(),
            Some("ops-lead"),
        ))
        .unwrap();
        let adoption_id = started["adoption_id"].as_str().unwrap();
        let work_order_id = started["work_order_id"].as_str().unwrap();
        let updated: Value = serde_json::from_str(&update_project_adoption_status(
            project,
            Some("demo"),
            adoption_id,
            "completed",
            Some("Recovery plan accepted."),
        ))
        .unwrap();

        assert_eq!(updated["action"], "adoption_status_updated");
        assert_eq!(updated["adoption"]["status"], "completed");
        assert_eq!(updated["adoption"]["last_note"], "Recovery plan accepted.");
        assert_eq!(updated["work_order"]["id"], work_order_id);
        assert_eq!(updated["work_order"]["status"], "completed");
    }

    #[test]
    fn operator_policy_authorizes_scoped_actions() {
        let profiles = vec![ControlOperatorProfile {
            id: "ops-lead".to_string(),
            role: "lead".to_string(),
            project_scopes: vec!["demo".to_string()],
            allowed_actions: vec!["session_*".to_string(), "work_*".to_string()],
            note: None,
        }];
        let decision = authorize_operator_action_with_profiles(
            &profiles,
            "/tmp/demo",
            Some("demo"),
            "ops-lead",
            "session_launch",
        )
        .unwrap();
        assert_eq!(decision["role"], "lead");
        assert_eq!(decision["id"], "ops-lead");
    }

    #[test]
    fn operator_policy_denies_actions_outside_role_scope() {
        let profiles = vec![ControlOperatorProfile {
            id: "reviewer-1".to_string(),
            role: "reviewer".to_string(),
            project_scopes: vec!["demo".to_string()],
            allowed_actions: vec!["debate_*".to_string()],
            note: None,
        }];
        let denied = authorize_operator_action_with_profiles(
            &profiles,
            "/tmp/demo",
            Some("demo"),
            "reviewer-1",
            "session_launch",
        )
        .unwrap_err();
        assert!(denied.contains("not allowed"));
    }
}
