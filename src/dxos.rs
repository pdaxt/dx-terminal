use crate::config;
use crate::recovery_planning::SuggestedSessionPlan;
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
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub status: String,
    #[serde(default)]
    pub owner: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub status: String,
    #[serde(default)]
    pub owner: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub status: String,
    #[serde(default)]
    pub owner: Option<String>,
    pub created_at: String,
    pub updated_at: String,
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
    #[serde(default)]
    pub follow_on_suggestions: Vec<SuggestedSessionPlan>,
    #[serde(default)]
    pub follow_on_session_ids: Vec<String>,
    #[serde(default)]
    pub follow_on_work_order_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContractRecord {
    pub id: String,
    pub status: String,
    pub role: String,
    #[serde(default = "default_priority")]
    pub priority: String,
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
    #[serde(default)]
    pub launch_claimed_by: Option<String>,
    #[serde(default)]
    pub launch_claimed_at: Option<String>,
    #[serde(default)]
    pub launch_claim_id: Option<String>,
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
    #[serde(default = "default_priority")]
    pub priority: String,
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
pub struct SchedulerRunRecord {
    pub id: String,
    pub actor: String,
    pub project_name: String,
    pub project_path: String,
    pub outcome: String,
    #[serde(default)]
    pub result: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunRecord {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub kind: String,
    pub scope: String,
    pub summary: String,
    pub status: String,
    #[serde(default)]
    pub source_provider: Option<String>,
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub requested_by: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub work_order_id: Option<String>,
    #[serde(default)]
    pub supervisor_session_id: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub steps: Vec<WorkflowStepRecord>,
    pub created_at: String,
    pub updated_at: String,
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
    pub company_scopes: Vec<String>,
    #[serde(default)]
    pub program_scopes: Vec<String>,
    #[serde(default)]
    pub workspace_scopes: Vec<String>,
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
    #[serde(default)]
    pub workflow_runs: Vec<WorkflowRunRecord>,
    #[serde(default)]
    pub scheduler_runs: Vec<SchedulerRunRecord>,
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
CREATE TABLE IF NOT EXISTS dxos_companies (
    company_id TEXT PRIMARY KEY,
    company_name TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dxos_companies_updated_at
    ON dxos_companies(updated_at DESC, company_name ASC);
CREATE TABLE IF NOT EXISTS dxos_programs (
    program_id TEXT PRIMARY KEY,
    program_name TEXT NOT NULL,
    company_name TEXT,
    updated_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dxos_programs_updated_at
    ON dxos_programs(updated_at DESC, program_name ASC);
CREATE TABLE IF NOT EXISTS dxos_workspaces (
    workspace_id TEXT PRIMARY KEY,
    workspace_name TEXT NOT NULL,
    company_name TEXT,
    program_name TEXT,
    updated_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dxos_workspaces_updated_at
    ON dxos_workspaces(updated_at DESC, workspace_name ASC);
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

fn clean_optional_label(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
}

fn normalized_scope_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn company_record_id(name: &str) -> String {
    normalized_scope_key(name)
}

fn program_record_id(company: Option<&str>, name: &str) -> String {
    format!(
        "{}::{}",
        company
            .map(normalized_scope_key)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "_".to_string()),
        normalized_scope_key(name)
    )
}

fn workspace_record_id(company: Option<&str>, program: Option<&str>, name: &str) -> String {
    format!(
        "{}::{}::{}",
        company
            .map(normalized_scope_key)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "_".to_string()),
        program
            .map(normalized_scope_key)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "_".to_string()),
        normalized_scope_key(name)
    )
}

fn default_state(project_path: &str, project_name: Option<&str>) -> ControlPlaneState {
    ControlPlaneState {
        version: 1,
        project: ProjectDescriptor {
            name: resolved_project_name(project_path, project_name),
            path: project_path.to_string(),
            company: None,
            program: None,
            workspace: None,
        },
        defaults: ControlPlaneDefaults::default(),
        adoptions: Vec::new(),
        debates: Vec::new(),
        sessions: Vec::new(),
        work_orders: Vec::new(),
        workflow_runs: Vec::new(),
        scheduler_runs: Vec::new(),
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

fn registry_projects_for_store_path(registry_path: &Path) -> Result<Vec<Value>, String> {
    let conn = open_control_plane_db(registry_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT project_path, project_name, updated_at, payload_json
             FROM dxos_control_planes
             ORDER BY updated_at DESC, project_name ASC",
        )
        .map_err(|error| format!("prepare: {}", error))?;
    let rows = stmt
        .query_map([], |row| {
            let payload_json: String = row.get(3)?;
            let payload =
                serde_json::from_str::<Value>(&payload_json).unwrap_or_else(|_| json!({}));
            Ok(json!({
                "path": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "updated_at": row.get::<_, String>(2)?,
                "company": payload.get("project").and_then(|value| value.get("company")).cloned().unwrap_or(Value::Null),
                "program": payload.get("project").and_then(|value| value.get("program")).cloned().unwrap_or(Value::Null),
                "workspace": payload.get("project").and_then(|value| value.get("workspace")).cloned().unwrap_or(Value::Null),
            }))
        })
        .map_err(|error| format!("query: {}", error))?;
    Ok(rows.filter_map(Result::ok).collect::<Vec<_>>())
}

fn sorted_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut items = values.into_iter().collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

fn company_groups(projects: &[Value]) -> Vec<Value> {
    let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for project in projects {
        let Some(name) = project
            .get("company")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        grouped
            .entry(name.to_string())
            .or_default()
            .push(project.clone());
    }
    grouped
        .into_iter()
        .map(|(name, grouped_projects)| {
            json!({
                "id": company_record_id(&name),
                "name": name,
                "project_count": grouped_projects.len(),
                "programs": sorted_unique_strings(grouped_projects.iter().filter_map(|project| project.get("program").and_then(Value::as_str)).map(str::trim).filter(|value| !value.is_empty()).map(|value| value.to_string())),
                "workspaces": sorted_unique_strings(grouped_projects.iter().filter_map(|project| project.get("workspace").and_then(Value::as_str)).map(str::trim).filter(|value| !value.is_empty()).map(|value| value.to_string())),
                "projects": grouped_projects,
            })
        })
        .collect::<Vec<_>>()
}

fn program_groups(projects: &[Value]) -> Vec<Value> {
    let mut grouped: BTreeMap<String, (Option<String>, String, Vec<Value>)> = BTreeMap::new();
    for project in projects {
        let Some(name) = project
            .get("program")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let company = clean_optional_label(project.get("company").and_then(Value::as_str));
        let key = program_record_id(company.as_deref(), name);
        let entry = grouped
            .entry(key)
            .or_insert_with(|| (company.clone(), name.to_string(), Vec::new()));
        entry.2.push(project.clone());
    }
    grouped
        .into_iter()
        .map(|(id, (company, name, grouped_projects))| {
            json!({
                "id": id,
                "name": name,
                "company": company,
                "companies": sorted_unique_strings(grouped_projects.iter().filter_map(|project| project.get("company").and_then(Value::as_str)).map(str::trim).filter(|value| !value.is_empty()).map(|value| value.to_string())),
                "project_count": grouped_projects.len(),
                "workspaces": sorted_unique_strings(grouped_projects.iter().filter_map(|project| project.get("workspace").and_then(Value::as_str)).map(str::trim).filter(|value| !value.is_empty()).map(|value| value.to_string())),
                "projects": grouped_projects,
            })
        })
        .collect::<Vec<_>>()
}

fn workspace_groups(projects: &[Value]) -> Vec<Value> {
    let mut grouped: BTreeMap<String, (Option<String>, Option<String>, String, Vec<Value>)> =
        BTreeMap::new();
    for project in projects {
        let Some(name) = project
            .get("workspace")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let company = clean_optional_label(project.get("company").and_then(Value::as_str));
        let program = clean_optional_label(project.get("program").and_then(Value::as_str));
        let key = workspace_record_id(company.as_deref(), program.as_deref(), name);
        let entry = grouped.entry(key).or_insert_with(|| {
            (
                company.clone(),
                program.clone(),
                name.to_string(),
                Vec::new(),
            )
        });
        entry.3.push(project.clone());
    }
    grouped
        .into_iter()
        .map(|(id, (company, program, name, grouped_projects))| {
            json!({
                "id": id,
                "name": name,
                "company": company,
                "program": program,
                "project_count": grouped_projects.len(),
                "projects": grouped_projects,
            })
        })
        .collect::<Vec<_>>()
}

fn merge_company_groups(derived: Vec<Value>, records: Vec<CompanyRecord>) -> Vec<Value> {
    let mut items = derived
        .into_iter()
        .map(|item| {
            let key = item
                .get("name")
                .and_then(Value::as_str)
                .map(company_record_id)
                .unwrap_or_default();
            (key, item)
        })
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let key = record.id.clone();
        let entry = items.entry(key).or_insert_with(|| {
            json!({
                "id": record.id,
                "name": record.name,
                "project_count": 0,
                "programs": [],
                "workspaces": [],
                "projects": [],
            })
        });
        if let Some(object) = entry.as_object_mut() {
            object.insert("id".to_string(), json!(record.id));
            object.insert("name".to_string(), json!(record.name));
            object.insert("summary".to_string(), json!(record.summary));
            object.insert("status".to_string(), json!(record.status));
            object.insert("owner".to_string(), json!(record.owner));
            object.insert("created_at".to_string(), json!(record.created_at));
            object.insert("updated_at".to_string(), json!(record.updated_at));
        }
    }
    let mut values = items.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .get("project_count")
            .and_then(Value::as_u64)
            .cmp(&left.get("project_count").and_then(Value::as_u64))
            .then_with(|| {
                left.get("name")
                    .and_then(Value::as_str)
                    .cmp(&right.get("name").and_then(Value::as_str))
            })
    });
    values
}

fn merge_program_groups(derived: Vec<Value>, records: Vec<ProgramRecord>) -> Vec<Value> {
    let mut items = derived
        .into_iter()
        .map(|item| {
            let key = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (key, item)
        })
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let key = record.id.clone();
        let entry = items.entry(key).or_insert_with(|| {
            json!({
                "id": record.id,
                "name": record.name,
                "company": record.company,
                "companies": record.company.clone().map(|value| vec![value]).unwrap_or_default(),
                "project_count": 0,
                "workspaces": [],
                "projects": [],
            })
        });
        if let Some(object) = entry.as_object_mut() {
            object.insert("id".to_string(), json!(record.id));
            object.insert("name".to_string(), json!(record.name));
            object.insert("company".to_string(), json!(record.company));
            object.insert(
                "companies".to_string(),
                json!(record
                    .company
                    .clone()
                    .map(|value| vec![value])
                    .unwrap_or_default()),
            );
            object.insert("summary".to_string(), json!(record.summary));
            object.insert("status".to_string(), json!(record.status));
            object.insert("owner".to_string(), json!(record.owner));
            object.insert("created_at".to_string(), json!(record.created_at));
            object.insert("updated_at".to_string(), json!(record.updated_at));
        }
    }
    let mut values = items.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .get("project_count")
            .and_then(Value::as_u64)
            .cmp(&left.get("project_count").and_then(Value::as_u64))
            .then_with(|| {
                left.get("name")
                    .and_then(Value::as_str)
                    .cmp(&right.get("name").and_then(Value::as_str))
            })
    });
    values
}

fn merge_workspace_groups(derived: Vec<Value>, records: Vec<WorkspaceRecord>) -> Vec<Value> {
    let mut items = derived
        .into_iter()
        .map(|item| {
            let key = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (key, item)
        })
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let key = record.id.clone();
        let entry = items.entry(key).or_insert_with(|| {
            json!({
                "id": record.id,
                "name": record.name,
                "company": record.company,
                "program": record.program,
                "project_count": 0,
                "projects": [],
            })
        });
        if let Some(object) = entry.as_object_mut() {
            object.insert("id".to_string(), json!(record.id));
            object.insert("name".to_string(), json!(record.name));
            object.insert("company".to_string(), json!(record.company));
            object.insert("program".to_string(), json!(record.program));
            object.insert("summary".to_string(), json!(record.summary));
            object.insert("status".to_string(), json!(record.status));
            object.insert("owner".to_string(), json!(record.owner));
            object.insert("created_at".to_string(), json!(record.created_at));
            object.insert("updated_at".to_string(), json!(record.updated_at));
        }
    }
    let mut values = items.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .get("project_count")
            .and_then(Value::as_u64)
            .cmp(&left.get("project_count").and_then(Value::as_u64))
            .then_with(|| {
                left.get("name")
                    .and_then(Value::as_str)
                    .cmp(&right.get("name").and_then(Value::as_str))
            })
    });
    values
}

fn portfolio_registry_summary(projects: &[Value], registry_path: &Path) -> Value {
    let companies = merge_company_groups(
        company_groups(projects),
        query_company_records_for_store_path(registry_path).unwrap_or_default(),
    );
    let programs = merge_program_groups(
        program_groups(projects),
        query_program_records_for_store_path(registry_path).unwrap_or_default(),
    );
    let workspaces = merge_workspace_groups(
        workspace_groups(projects),
        query_workspace_records_for_store_path(registry_path).unwrap_or_default(),
    );
    json!({
        "company_count": companies.len(),
        "program_count": programs.len(),
        "workspace_count": workspaces.len(),
        "companies": companies,
        "programs": programs,
        "workspaces": workspaces,
    })
}

fn control_plane_registry_value_for_store_path(registry_path: &Path) -> Value {
    let projects = match registry_projects_for_store_path(registry_path) {
        Ok(projects) => projects,
        Err(error) => return json!({"error": error}),
    };
    let portfolio = portfolio_registry_summary(&projects, registry_path);
    json!({
        "backend": "sqlite_with_repo_mirror",
        "database_path": registry_path.to_string_lossy().to_string(),
        "project_count": projects.len(),
        "projects": projects,
        "company_count": portfolio.get("company_count").cloned().unwrap_or_else(|| json!(0)),
        "program_count": portfolio.get("program_count").cloned().unwrap_or_else(|| json!(0)),
        "workspace_count": portfolio.get("workspace_count").cloned().unwrap_or_else(|| json!(0)),
        "companies": portfolio.get("companies").cloned().unwrap_or_else(|| json!([])),
        "programs": portfolio.get("programs").cloned().unwrap_or_else(|| json!([])),
        "workspaces": portfolio.get("workspaces").cloned().unwrap_or_else(|| json!([])),
    })
}

fn control_plane_registry_value_for_project_path(project_path: &str) -> Value {
    control_plane_registry_value_for_store_path(&control_plane_store_path(project_path))
}

fn query_company_records_for_store_path(
    registry_path: &Path,
) -> Result<Vec<CompanyRecord>, String> {
    let conn = open_control_plane_db(registry_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM dxos_companies
             ORDER BY updated_at DESC, company_name ASC",
        )
        .map_err(|error| format!("prepare: {}", error))?;
    let rows = stmt
        .query_map([], |row| {
            let payload_json: String = row.get(0)?;
            Ok(serde_json::from_str::<CompanyRecord>(&payload_json).ok())
        })
        .map_err(|error| format!("query: {}", error))?;
    Ok(rows.filter_map(Result::ok).flatten().collect())
}

fn query_program_records_for_store_path(
    registry_path: &Path,
) -> Result<Vec<ProgramRecord>, String> {
    let conn = open_control_plane_db(registry_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM dxos_programs
             ORDER BY updated_at DESC, program_name ASC",
        )
        .map_err(|error| format!("prepare: {}", error))?;
    let rows = stmt
        .query_map([], |row| {
            let payload_json: String = row.get(0)?;
            Ok(serde_json::from_str::<ProgramRecord>(&payload_json).ok())
        })
        .map_err(|error| format!("query: {}", error))?;
    Ok(rows.filter_map(Result::ok).flatten().collect())
}

fn query_workspace_records_for_store_path(
    registry_path: &Path,
) -> Result<Vec<WorkspaceRecord>, String> {
    let conn = open_control_plane_db(registry_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM dxos_workspaces
             ORDER BY updated_at DESC, workspace_name ASC",
        )
        .map_err(|error| format!("prepare: {}", error))?;
    let rows = stmt
        .query_map([], |row| {
            let payload_json: String = row.get(0)?;
            Ok(serde_json::from_str::<WorkspaceRecord>(&payload_json).ok())
        })
        .map_err(|error| format!("query: {}", error))?;
    Ok(rows.filter_map(Result::ok).flatten().collect())
}

fn store_company_record(project_path: &str, record: &CompanyRecord) -> Result<(), String> {
    let conn = control_plane_db(project_path)?;
    let payload = serde_json::to_string_pretty(record).map_err(|e| format!("serialize: {}", e))?;
    conn.execute(
        "INSERT INTO dxos_companies(company_id, company_name, updated_at, payload_json)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(company_id) DO UPDATE SET
            company_name = excluded.company_name,
            updated_at = excluded.updated_at,
            payload_json = excluded.payload_json",
        params![record.id, record.name, record.updated_at, payload],
    )
    .map_err(|e| format!("upsert: {}", e))?;
    Ok(())
}

fn store_program_record(project_path: &str, record: &ProgramRecord) -> Result<(), String> {
    let conn = control_plane_db(project_path)?;
    let payload = serde_json::to_string_pretty(record).map_err(|e| format!("serialize: {}", e))?;
    conn.execute(
        "INSERT INTO dxos_programs(program_id, program_name, company_name, updated_at, payload_json)
         VALUES(?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(program_id) DO UPDATE SET
            program_name = excluded.program_name,
            company_name = excluded.company_name,
            updated_at = excluded.updated_at,
            payload_json = excluded.payload_json",
        params![record.id, record.name, record.company, record.updated_at, payload],
    )
    .map_err(|e| format!("upsert: {}", e))?;
    Ok(())
}

fn store_workspace_record(project_path: &str, record: &WorkspaceRecord) -> Result<(), String> {
    let conn = control_plane_db(project_path)?;
    let payload = serde_json::to_string_pretty(record).map_err(|e| format!("serialize: {}", e))?;
    conn.execute(
        "INSERT INTO dxos_workspaces(workspace_id, workspace_name, company_name, program_name, updated_at, payload_json)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(workspace_id) DO UPDATE SET
            workspace_name = excluded.workspace_name,
            company_name = excluded.company_name,
            program_name = excluded.program_name,
            updated_at = excluded.updated_at,
            payload_json = excluded.payload_json",
        params![
            record.id,
            record.name,
            record.company,
            record.program,
            record.updated_at,
            payload
        ],
    )
    .map_err(|e| format!("upsert: {}", e))?;
    Ok(())
}

fn company_record_for_store_path(registry_path: &Path, name: &str) -> Option<CompanyRecord> {
    let id = company_record_id(name);
    query_company_records_for_store_path(registry_path)
        .ok()?
        .into_iter()
        .find(|record| record.id == id)
}

fn program_record_for_store_path(
    registry_path: &Path,
    company: Option<&str>,
    name: &str,
) -> Option<ProgramRecord> {
    let id = program_record_id(company, name);
    query_program_records_for_store_path(registry_path)
        .ok()?
        .into_iter()
        .find(|record| record.id == id)
}

fn workspace_record_for_store_path(
    registry_path: &Path,
    company: Option<&str>,
    program: Option<&str>,
    name: &str,
) -> Option<WorkspaceRecord> {
    let id = workspace_record_id(company, program, name);
    query_workspace_records_for_store_path(registry_path)
        .ok()?
        .into_iter()
        .find(|record| record.id == id)
}

fn ensure_portfolio_records_for_project(
    project_path: &str,
    project: &ProjectDescriptor,
) -> Result<(), String> {
    let registry_path = control_plane_store_path(project_path);
    if let Some(company) = project
        .company
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if company_record_for_store_path(&registry_path, company).is_none() {
            let now = crate::state::now();
            store_company_record(
                project_path,
                &CompanyRecord {
                    id: company_record_id(company),
                    name: company.to_string(),
                    summary: None,
                    status: "active".to_string(),
                    owner: None,
                    created_at: now.clone(),
                    updated_at: now,
                },
            )?;
        }
    }
    if let Some(program) = project
        .program
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if program_record_for_store_path(&registry_path, project.company.as_deref(), program)
            .is_none()
        {
            let now = crate::state::now();
            store_program_record(
                project_path,
                &ProgramRecord {
                    id: program_record_id(project.company.as_deref(), program),
                    name: program.to_string(),
                    company: clean_optional_label(project.company.as_deref()),
                    summary: None,
                    status: "active".to_string(),
                    owner: None,
                    created_at: now.clone(),
                    updated_at: now,
                },
            )?;
        }
    }
    if let Some(workspace) = project
        .workspace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if workspace_record_for_store_path(
            &registry_path,
            project.company.as_deref(),
            project.program.as_deref(),
            workspace,
        )
        .is_none()
        {
            let now = crate::state::now();
            store_workspace_record(
                project_path,
                &WorkspaceRecord {
                    id: workspace_record_id(
                        project.company.as_deref(),
                        project.program.as_deref(),
                        workspace,
                    ),
                    name: workspace.to_string(),
                    company: clean_optional_label(project.company.as_deref()),
                    program: clean_optional_label(project.program.as_deref()),
                    summary: None,
                    status: "active".to_string(),
                    owner: None,
                    created_at: now.clone(),
                    updated_at: now,
                },
            )?;
        }
    }
    Ok(())
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
            "portfolio_read".to_string(),
            "project_identity".to_string(),
            "scheduler_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "workflow_*".to_string(),
            "debate_*".to_string(),
            "provider_plugin_*".to_string(),
            "automation_bridge_*".to_string(),
            "pane_talk".to_string(),
            "pane_restart".to_string(),
        ],
        "reviewer" => vec![
            "debate_*".to_string(),
            "portfolio_read".to_string(),
            "work_resolve".to_string(),
            "workflow_*".to_string(),
            "session_block".to_string(),
            "pane_talk".to_string(),
        ],
        "operator" => vec![
            "adoption_*".to_string(),
            "portfolio_read".to_string(),
            "project_identity".to_string(),
            "scheduler_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "workflow_*".to_string(),
            "debate_*".to_string(),
            "provider_plugin_*".to_string(),
            "automation_bridge_*".to_string(),
            "pane_*".to_string(),
        ],
        "observer" => vec!["portfolio_read".to_string()],
        _ => vec![
            "adoption_*".to_string(),
            "portfolio_read".to_string(),
            "scheduler_*".to_string(),
            "session_*".to_string(),
            "work_*".to_string(),
            "workflow_*".to_string(),
            "debate_*".to_string(),
            "provider_plugin_*".to_string(),
            "automation_bridge_*".to_string(),
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
            if operator.company_scopes.is_empty() {
                operator.company_scopes.push("*".to_string());
            }
            if operator.program_scopes.is_empty() {
                operator.program_scopes.push("*".to_string());
            }
            if operator.workspace_scopes.is_empty() {
                operator.workspace_scopes.push("*".to_string());
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

fn metadata_scope_matches(scopes: &[String], value: Option<&str>) -> bool {
    if scopes.is_empty() {
        return true;
    }
    let normalized = value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .unwrap_or("--");
    scopes
        .iter()
        .any(|scope| authorization_pattern_matches(scope, normalized))
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
            "company_scopes": ["*"],
            "program_scopes": ["*"],
            "workspace_scopes": ["*"],
            "allowed_actions": ["*"],
        }));
    }
    let Some(operator) = profiles.iter().find(|profile| profile.id == actor) else {
        return Err(format!(
            "Operator '{}' is not registered for DXOS control.",
            actor
        ));
    };
    let project = load_control_plane(project_path, project_name).project;
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
    if !metadata_scope_matches(&operator.company_scopes, project.company.as_deref()) {
        return Err(format!(
            "Operator '{}' cannot control company '{}'.",
            actor,
            project.company.as_deref().unwrap_or("--")
        ));
    }
    if !metadata_scope_matches(&operator.program_scopes, project.program.as_deref()) {
        return Err(format!(
            "Operator '{}' cannot control program '{}'.",
            actor,
            project.program.as_deref().unwrap_or("--")
        ));
    }
    if !metadata_scope_matches(&operator.workspace_scopes, project.workspace.as_deref()) {
        return Err(format!(
            "Operator '{}' cannot control workspace '{}'.",
            actor,
            project.workspace.as_deref().unwrap_or("--")
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
        "company_scopes": operator.company_scopes,
        "program_scopes": operator.program_scopes,
        "workspace_scopes": operator.workspace_scopes,
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

fn authorize_operator_scope_read_with_profiles(
    profiles: &[ControlOperatorProfile],
    actor: &str,
    action_kind: &str,
    company: Option<&str>,
    program: Option<&str>,
    workspace: Option<&str>,
) -> Result<Value, String> {
    if profiles.is_empty() {
        return Ok(json!({
            "mode": "authenticated_open",
            "id": actor,
            "role": "operator",
            "project_scopes": ["*"],
            "company_scopes": ["*"],
            "program_scopes": ["*"],
            "workspace_scopes": ["*"],
            "allowed_actions": ["*"],
        }));
    }
    let Some(operator) = profiles.iter().find(|profile| profile.id == actor) else {
        return Err(format!(
            "Operator '{}' is not registered for DXOS control.",
            actor
        ));
    };
    if !metadata_scope_matches(&operator.company_scopes, company) {
        return Err(format!(
            "Operator '{}' cannot read company scope '{}'.",
            actor,
            company
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("--")
        ));
    }
    if !metadata_scope_matches(&operator.program_scopes, program) {
        return Err(format!(
            "Operator '{}' cannot read program scope '{}'.",
            actor,
            program
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("--")
        ));
    }
    if !metadata_scope_matches(&operator.workspace_scopes, workspace) {
        return Err(format!(
            "Operator '{}' cannot read workspace scope '{}'.",
            actor,
            workspace
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("--")
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
        "company_scopes": operator.company_scopes,
        "program_scopes": operator.program_scopes,
        "workspace_scopes": operator.workspace_scopes,
        "allowed_actions": operator.allowed_actions,
        "note": operator.note,
    }))
}

pub fn authorize_operator_scope_read(
    actor: &str,
    action_kind: &str,
    company: Option<&str>,
    program: Option<&str>,
    workspace: Option<&str>,
) -> Result<Value, String> {
    authorize_operator_scope_read_with_profiles(
        &load_control_operator_profiles(),
        actor,
        action_kind,
        company,
        program,
        workspace,
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
            "company_scopes": operator.company_scopes,
            "program_scopes": operator.program_scopes,
            "workspace_scopes": operator.workspace_scopes,
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
            "/api/dxos/portfolio/brief",
            "/api/dxos/registry",
            "/api/dxos/project/identity",
            "/api/dxos/company",
            "/api/dxos/program",
            "/api/dxos/workspace",
            "/api/dxos/session/launch",
            "/api/dxos/session/upsert",
            "/api/dxos/session/status",
            "/api/dxos/session/block",
            "/api/dxos/work/delegate",
            "/api/dxos/work/block",
            "/api/dxos/work/resolve",
            "/api/dxos/workflow/start",
            "/api/dxos/workflow/step",
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

pub fn upsert_project_identity(
    project_path: &str,
    project_name: Option<&str>,
    company: Option<&str>,
    program: Option<&str>,
    workspace: Option<&str>,
) -> String {
    let mut state = load_control_plane(project_path, project_name);
    state.project.company = company
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    state.project.program = program
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    state.project.workspace = workspace
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    state.updated_at = crate::state::now();
    match save_control_plane(project_path, &state) {
        Ok(()) => {
            let _ = ensure_portfolio_records_for_project(project_path, &state.project);
            json!({
                "status": "ok",
                "action": "project_identity_updated",
                "project": state.project,
                "portfolio": {
                    "registry": control_plane_registry_value_for_project_path(project_path),
                },
            })
            .to_string()
        }
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn upsert_company_record(
    project_path: &str,
    project_name: Option<&str>,
    name: Option<&str>,
    summary: Option<&str>,
    status: Option<&str>,
    owner: Option<&str>,
) -> String {
    let state = load_control_plane(project_path, project_name);
    let Some(company_name) = clean_optional_label(name)
        .or_else(|| clean_optional_label(state.project.company.as_deref()))
    else {
        return json!({"error": "Company name is required."}).to_string();
    };
    let registry_path = control_plane_store_path(project_path);
    let mut record =
        company_record_for_store_path(&registry_path, &company_name).unwrap_or_else(|| {
            let now = crate::state::now();
            CompanyRecord {
                id: company_record_id(&company_name),
                name: company_name.clone(),
                summary: None,
                status: "active".to_string(),
                owner: None,
                created_at: now.clone(),
                updated_at: now,
            }
        });
    record.name = company_name;
    record.summary = clean_optional_label(summary);
    record.status = clean_optional_label(status).unwrap_or_else(|| "active".to_string());
    record.owner = clean_optional_label(owner);
    record.updated_at = crate::state::now();
    match store_company_record(project_path, &record) {
        Ok(()) => json!({
            "status": "ok",
            "action": "company_record_updated",
            "company": record,
            "portfolio": {
                "registry": control_plane_registry_value_for_project_path(project_path),
            },
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn upsert_program_record(
    project_path: &str,
    project_name: Option<&str>,
    company: Option<&str>,
    name: Option<&str>,
    summary: Option<&str>,
    status: Option<&str>,
    owner: Option<&str>,
) -> String {
    let state = load_control_plane(project_path, project_name);
    let company_name = clean_optional_label(company)
        .or_else(|| clean_optional_label(state.project.company.as_deref()));
    let Some(program_name) = clean_optional_label(name)
        .or_else(|| clean_optional_label(state.project.program.as_deref()))
    else {
        return json!({"error": "Program name is required."}).to_string();
    };
    let registry_path = control_plane_store_path(project_path);
    let mut record =
        program_record_for_store_path(&registry_path, company_name.as_deref(), &program_name)
            .unwrap_or_else(|| {
                let now = crate::state::now();
                ProgramRecord {
                    id: program_record_id(company_name.as_deref(), &program_name),
                    name: program_name.clone(),
                    company: company_name.clone(),
                    summary: None,
                    status: "active".to_string(),
                    owner: None,
                    created_at: now.clone(),
                    updated_at: now,
                }
            });
    record.id = program_record_id(company_name.as_deref(), &program_name);
    record.name = program_name;
    record.company = company_name;
    record.summary = clean_optional_label(summary);
    record.status = clean_optional_label(status).unwrap_or_else(|| "active".to_string());
    record.owner = clean_optional_label(owner);
    record.updated_at = crate::state::now();
    match store_program_record(project_path, &record) {
        Ok(()) => json!({
            "status": "ok",
            "action": "program_record_updated",
            "program": record,
            "portfolio": {
                "registry": control_plane_registry_value_for_project_path(project_path),
            },
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn upsert_workspace_record(
    project_path: &str,
    project_name: Option<&str>,
    company: Option<&str>,
    program: Option<&str>,
    name: Option<&str>,
    summary: Option<&str>,
    status: Option<&str>,
    owner: Option<&str>,
) -> String {
    let state = load_control_plane(project_path, project_name);
    let company_name = clean_optional_label(company)
        .or_else(|| clean_optional_label(state.project.company.as_deref()));
    let program_name = clean_optional_label(program)
        .or_else(|| clean_optional_label(state.project.program.as_deref()));
    let Some(workspace_name) = clean_optional_label(name)
        .or_else(|| clean_optional_label(state.project.workspace.as_deref()))
    else {
        return json!({"error": "Workspace name is required."}).to_string();
    };
    let registry_path = control_plane_store_path(project_path);
    let mut record = workspace_record_for_store_path(
        &registry_path,
        company_name.as_deref(),
        program_name.as_deref(),
        &workspace_name,
    )
    .unwrap_or_else(|| {
        let now = crate::state::now();
        WorkspaceRecord {
            id: workspace_record_id(
                company_name.as_deref(),
                program_name.as_deref(),
                &workspace_name,
            ),
            name: workspace_name.clone(),
            company: company_name.clone(),
            program: program_name.clone(),
            summary: None,
            status: "active".to_string(),
            owner: None,
            created_at: now.clone(),
            updated_at: now,
        }
    });
    record.id = workspace_record_id(
        company_name.as_deref(),
        program_name.as_deref(),
        &workspace_name,
    );
    record.name = workspace_name;
    record.company = company_name;
    record.program = program_name;
    record.summary = clean_optional_label(summary);
    record.status = clean_optional_label(status).unwrap_or_else(|| "active".to_string());
    record.owner = clean_optional_label(owner);
    record.updated_at = crate::state::now();
    match store_workspace_record(project_path, &record) {
        Ok(()) => json!({
            "status": "ok",
            "action": "workspace_record_updated",
            "workspace": record,
            "portfolio": {
                "registry": control_plane_registry_value_for_project_path(project_path),
            },
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
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

fn next_workflow_run_id(state: &ControlPlaneState) -> String {
    format!("WF{:04}", state.workflow_runs.len() + 1)
}

fn default_escalation_target() -> String {
    "lead".to_string()
}

fn default_priority() -> String {
    "medium".to_string()
}

fn normalize_priority(priority: Option<&str>) -> String {
    match priority
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .as_deref()
    {
        Some("high") => "high".to_string(),
        Some("low") => "low".to_string(),
        _ => "medium".to_string(),
    }
}

fn clear_session_launch_claim(session: &mut SessionContractRecord) {
    session.launch_claimed_by = None;
    session.launch_claimed_at = None;
    session.launch_claim_id = None;
}

fn set_session_launch_claim(
    session: &mut SessionContractRecord,
    actor: &str,
    claim_id: Option<&str>,
    claimed_at: &str,
) {
    session.launch_claimed_by = Some(actor.trim().to_string());
    session.launch_claimed_at = Some(claimed_at.to_string());
    session.launch_claim_id = claim_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
}

fn launch_claim_is_stale(session: &SessionContractRecord) -> bool {
    if session.status != "launching" {
        return false;
    }
    let Some(claimed_at) = session.launch_claimed_at.as_deref() else {
        return false;
    };
    let Ok(claimed_at) = chrono::NaiveDateTime::parse_from_str(claimed_at, "%Y-%m-%dT%H:%M:%S")
    else {
        return false;
    };
    let age = chrono::Local::now().naive_local() - claimed_at;
    age.num_seconds() >= crate::config::session_launch_claim_ttl_secs() as i64
}

const SCHEDULER_RUN_HISTORY_LIMIT: usize = 48;

fn scheduler_run_result(state: &ControlPlaneState, run_id: &str) -> Option<Value> {
    state
        .scheduler_runs
        .iter()
        .find(|run| run.id == run_id.trim())
        .map(|run| run.result.clone())
}

fn remember_scheduler_run(
    state: &mut ControlPlaneState,
    actor: &str,
    run_id: &str,
    result: Value,
) -> Value {
    let now = crate::state::now();
    let run_id = run_id.trim();
    let outcome = result
        .get("outcome")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| {
            result
                .get("launch")
                .and_then(|value| value.get("error"))
                .map(|_| "error".to_string())
        })
        .or_else(|| {
            result
                .get("claim")
                .and_then(|value| value.get("error"))
                .map(|_| "blocked".to_string())
        })
        .unwrap_or_else(|| "ok".to_string());

    if let Some(existing) = state.scheduler_runs.iter_mut().find(|run| run.id == run_id) {
        existing.actor = actor.trim().to_string();
        existing.project_name = state.project.name.clone();
        existing.project_path = state.project.path.clone();
        existing.outcome = outcome;
        existing.result = result.clone();
        existing.updated_at = now.clone();
    } else {
        state.scheduler_runs.push(SchedulerRunRecord {
            id: run_id.to_string(),
            actor: actor.trim().to_string(),
            project_name: state.project.name.clone(),
            project_path: state.project.path.clone(),
            outcome,
            result: result.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    if state.scheduler_runs.len() > SCHEDULER_RUN_HISTORY_LIMIT {
        let trim = state.scheduler_runs.len() - SCHEDULER_RUN_HISTORY_LIMIT;
        state.scheduler_runs.drain(0..trim);
    }
    state.updated_at = now;
    result
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

fn capabilities_for_role(role: &str) -> Vec<String> {
    match normalize_role(role).as_str() {
        "discovery" => vec![
            "vision".to_string(),
            "docs".to_string(),
            "analysis".to_string(),
            "research".to_string(),
        ],
        "design" => vec![
            "design".to_string(),
            "docs".to_string(),
            "browser".to_string(),
            "analysis".to_string(),
        ],
        "qa" => vec![
            "qa".to_string(),
            "tests".to_string(),
            "browser".to_string(),
            "analysis".to_string(),
        ],
        "docs" => vec![
            "docs".to_string(),
            "vision".to_string(),
            "git".to_string(),
            "analysis".to_string(),
        ],
        "release" => vec![
            "release".to_string(),
            "git".to_string(),
            "qa".to_string(),
            "analysis".to_string(),
        ],
        "frontend" | "backend" | "developer" => vec![
            "git".to_string(),
            "code".to_string(),
            "tests".to_string(),
            "analysis".to_string(),
        ],
        "lead" => vec![
            "vision".to_string(),
            "docs".to_string(),
            "git".to_string(),
            "analysis".to_string(),
        ],
        _ => vec![
            "analysis".to_string(),
            "docs".to_string(),
            "git".to_string(),
        ],
    }
}

fn expected_outputs_for_role_stage(role: &str, stage: Option<&str>) -> Vec<String> {
    match (
        normalize_role(role).as_str(),
        normalize_stage(stage).as_str(),
    ) {
        ("discovery", _) => vec![
            "recovery_assessment".to_string(),
            "discovery_doc".to_string(),
            "open_questions".to_string(),
        ],
        ("design", _) => vec![
            "design_options".to_string(),
            "approval_packet".to_string(),
            "design_rationale".to_string(),
        ],
        ("qa", _) | (_, "test") => vec![
            "verification_report".to_string(),
            "acceptance_status".to_string(),
            "evidence_bundle".to_string(),
        ],
        ("docs", _) => vec![
            "documentation_sync".to_string(),
            "handbook_update".to_string(),
            "operator_handoff".to_string(),
        ],
        ("release", _) | (_, "done") => vec![
            "release_packet".to_string(),
            "rollout_note".to_string(),
            "handoff_note".to_string(),
        ],
        _ => vec![
            "implementation_artifact".to_string(),
            "linked_diff".to_string(),
            "handoff_note".to_string(),
        ],
    }
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
        "follow_on_suggestions": adoption.follow_on_suggestions,
        "follow_on_session_ids": adoption.follow_on_session_ids,
        "follow_on_work_order_ids": adoption.follow_on_work_order_ids,
        "follow_on_count": adoption.follow_on_suggestions.len(),
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
        "priority": session.priority,
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
        "launch_claimed_by": session.launch_claimed_by,
        "launch_claimed_at": session.launch_claimed_at,
        "launch_claim_id": session.launch_claim_id,
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
        "priority": work_order.priority,
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

fn workflow_step_summary(step: &WorkflowStepRecord) -> Value {
    json!({
        "id": step.id,
        "title": step.title,
        "status": step.status,
        "note": step.note,
        "updated_at": step.updated_at,
    })
}

fn workflow_run_summary(run: &WorkflowRunRecord) -> Value {
    let total_steps = run.steps.len();
    let completed_steps = run
        .steps
        .iter()
        .filter(|step| matches!(step.status.as_str(), "completed" | "skipped"))
        .count();
    let blocked_steps = run
        .steps
        .iter()
        .filter(|step| step.status == "blocked")
        .count();
    json!({
        "id": run.id,
        "workflow_id": run.workflow_id,
        "name": run.name,
        "kind": run.kind,
        "scope": run.scope,
        "summary": run.summary,
        "status": run.status,
        "source_provider": run.source_provider,
        "feature_id": run.feature_id,
        "stage": run.stage,
        "requested_by": run.requested_by,
        "session_id": run.session_id,
        "work_order_id": run.work_order_id,
        "supervisor_session_id": run.supervisor_session_id,
        "sources": run.sources,
        "source_path": run.source_path,
        "sections": run.sections,
        "step_count": total_steps,
        "completed_steps": completed_steps,
        "blocked_steps": blocked_steps,
        "steps": run.steps.iter().map(workflow_step_summary).collect::<Vec<_>>(),
        "created_at": run.created_at,
        "updated_at": run.updated_at,
    })
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        _ => 3,
    }
}

fn scheduler_launch_queue(state: &ControlPlaneState) -> Vec<Value> {
    let mut items = state
        .sessions
        .iter()
        .filter_map(|session| {
            if session.status != "planned" {
                return None;
            }
            let linked_work_orders = state
                .work_orders
                .iter()
                .filter(|work_order| {
                    work_order.worker_session_id.as_deref() == Some(session.id.as_str())
                })
                .collect::<Vec<_>>();
            let work_order = linked_work_orders
                .iter()
                .copied()
                .filter(|work_order| matches!(work_order.status.as_str(), "planned" | "assigned"))
                .max_by(|left, right| {
                    priority_rank(left.priority.as_str())
                        .cmp(&priority_rank(right.priority.as_str()))
                        .reverse()
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                });
            if work_order.is_none() && !linked_work_orders.is_empty() {
                return None;
            }
            let workflow_run = state
                .workflow_runs
                .iter()
                .filter(|run| {
                    run.session_id.as_deref() == Some(session.id.as_str())
                        && matches!(run.status.as_str(), "planned" | "active" | "blocked")
                })
                .max_by(|left, right| {
                    workflow_runtime_rank(left.status.as_str())
                        .cmp(&workflow_runtime_rank(right.status.as_str()))
                        .reverse()
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                });
            let priority = work_order
                .map(|item| item.priority.clone())
                .unwrap_or_else(|| session.priority.clone());
            Some(json!({
                "id": format!("launch:{}", session.id),
                "kind": "launch",
                "priority": priority,
                "ready": session.policy_violations.is_empty(),
                "blocked_by_policy": !session.policy_violations.is_empty(),
                "session_id": session.id,
                "work_order_id": work_order.map(|item| item.id.clone()),
                "workflow_run_id": workflow_run.map(|item| item.id.clone()),
                "role": session.role,
                "provider": session.provider,
                "model": session.model,
                "runtime_adapter": session.runtime_adapter,
                "feature_id": session.feature_id,
                "stage": session.stage,
                "title": work_order.map(|item| item.title.clone()).unwrap_or_else(|| format!("Launch {}", session.role)),
                "objective": work_order.map(|item| item.objective.clone()).unwrap_or_else(|| session.objective.clone()),
                "reason": work_order
                    .map(|item| {
                        if item.status == "assigned" {
                            "A governed work package is assigned to this planned session and ready for a live lane.".to_string()
                        } else {
                            "DXOS has a planned specialist session that is ready to be turned into a live lane.".to_string()
                        }
                    })
                    .unwrap_or_else(|| "DXOS has a planned session contract with no live lane yet.".to_string()),
                "updated_at": session.updated_at,
            }))
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        let left_priority = left
            .get("priority")
            .and_then(Value::as_str)
            .map(priority_rank)
            .unwrap_or(3);
        let right_priority = right
            .get("priority")
            .and_then(Value::as_str)
            .map(priority_rank)
            .unwrap_or(3);
        let left_ready = left.get("ready").and_then(Value::as_bool).unwrap_or(false);
        let right_ready = right.get("ready").and_then(Value::as_bool).unwrap_or(false);
        left_priority
            .cmp(&right_priority)
            .then_with(|| right_ready.cmp(&left_ready))
            .then_with(|| {
                right
                    .get("updated_at")
                    .and_then(Value::as_str)
                    .cmp(&left.get("updated_at").and_then(Value::as_str))
            })
    });
    items
}

fn scheduler_attention_queue(state: &ControlPlaneState) -> Vec<Value> {
    let mut items = state
        .work_orders
        .iter()
        .filter(|work_order| work_order.status == "blocked")
        .map(|work_order| {
            let session = work_order
                .worker_session_id
                .as_deref()
                .and_then(|id| state.sessions.iter().find(|session| session.id == id));
            json!({
                "id": format!("attention:{}", work_order.id),
                "kind": "attention",
                "priority": work_order.priority,
                "status": work_order.status,
                "work_order_id": work_order.id,
                "session_id": work_order.worker_session_id,
                "role": session.map(|item| item.role.clone()),
                "feature_id": work_order.feature_id,
                "stage": work_order.stage,
                "title": work_order.title,
                "objective": work_order.objective,
                "blockers": work_order.blockers,
                "requested_permissions": work_order.requested_permissions,
                "routed_to": if work_order.escalation_target == "human" {
                    "human".to_string()
                } else {
                    work_order.supervisor_session_id.clone()
                },
                "updated_at": work_order.updated_at,
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        let left_priority = left
            .get("priority")
            .and_then(Value::as_str)
            .map(priority_rank)
            .unwrap_or(3);
        let right_priority = right
            .get("priority")
            .and_then(Value::as_str)
            .map(priority_rank)
            .unwrap_or(3);
        left_priority.cmp(&right_priority).then_with(|| {
            right
                .get("updated_at")
                .and_then(Value::as_str)
                .cmp(&left.get("updated_at").and_then(Value::as_str))
        })
    });
    items
}

fn scheduler_active_claims(state: &ControlPlaneState) -> Vec<Value> {
    let mut items = state
        .sessions
        .iter()
        .filter(|session| {
            session.status == "launching"
                && session
                    .launch_claimed_by
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
        })
        .map(|session| {
            json!({
                "session_id": session.id,
                "role": session.role,
                "feature_id": session.feature_id,
                "stage": session.stage,
                "claimed_by": session.launch_claimed_by,
                "claim_id": session.launch_claim_id,
                "claimed_at": session.launch_claimed_at,
                "updated_at": session.updated_at,
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .get("claimed_at")
            .and_then(Value::as_str)
            .cmp(&left.get("claimed_at").and_then(Value::as_str))
    });
    items
}

fn scheduler_recent_runs(state: &ControlPlaneState) -> Vec<Value> {
    let mut items = state
        .scheduler_runs
        .iter()
        .rev()
        .take(8)
        .map(|run| {
            json!({
                "run_id": run.id,
                "actor": run.actor,
                "outcome": run.outcome,
                "action": run.result.get("action").cloned().unwrap_or_else(|| json!(null)),
                "session_id": run.result.get("session_id").cloned().unwrap_or_else(|| json!(null)),
                "created_at": run.created_at,
                "updated_at": run.updated_at,
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .get("updated_at")
            .and_then(Value::as_str)
            .cmp(&left.get("updated_at").and_then(Value::as_str))
    });
    items
}

fn scheduler_summary(state: &ControlPlaneState) -> Value {
    let launch_queue = scheduler_launch_queue(state);
    let attention_queue = scheduler_attention_queue(state);
    let next_launch = launch_queue.first().cloned();
    json!({
        "autorun_enabled": crate::config::scheduler_autorun_enabled(),
        "interval_secs": crate::config::scheduler_interval_secs(),
        "launch_queue": launch_queue,
        "attention_queue": attention_queue,
        "next_launch": next_launch,
        "active_claims": scheduler_active_claims(state),
        "recent_runs": scheduler_recent_runs(state),
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
    let active_workflow_runs = state
        .workflow_runs
        .iter()
        .filter(|run| matches!(run.status.as_str(), "planned" | "active" | "blocked"))
        .count();
    let blocked_workflow_runs = state
        .workflow_runs
        .iter()
        .filter(|run| run.status == "blocked")
        .count();
    let audit_recent = recent_audit_records(project_path, 8);
    let capability_registry = json!({
        "capability_source": "dx_registry",
        "mcp_count": registry.len(),
        "category_counts": categories,
    });
    let control_plane_registry = control_plane_registry_value_for_project_path(project_path);
    let current_company = state.project.company.as_deref().and_then(|company| {
        control_plane_registry
            .get("companies")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|entry| {
                    entry.get("name").and_then(Value::as_str).map(str::trim) == Some(company.trim())
                })
            })
            .cloned()
    });
    let current_program = state.project.program.as_deref().and_then(|program| {
        control_plane_registry
            .get("programs")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|entry| {
                    entry.get("name").and_then(Value::as_str).map(str::trim) == Some(program.trim())
                        && entry.get("company").and_then(Value::as_str).map(str::trim)
                            == state.project.company.as_deref().map(str::trim)
                })
            })
            .cloned()
            .or_else(|| {
                control_plane_registry
                    .get("programs")
                    .and_then(Value::as_array)
                    .and_then(|items| {
                        items.iter().find(|entry| {
                            entry.get("name").and_then(Value::as_str).map(str::trim)
                                == Some(program.trim())
                        })
                    })
                    .cloned()
            })
    });
    let current_workspace = state.project.workspace.as_deref().and_then(|workspace| {
        control_plane_registry
            .get("workspaces")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|entry| {
                    entry.get("name").and_then(Value::as_str).map(str::trim)
                        == Some(workspace.trim())
                        && entry.get("company").and_then(Value::as_str).map(str::trim)
                            == state.project.company.as_deref().map(str::trim)
                        && entry.get("program").and_then(Value::as_str).map(str::trim)
                            == state.project.program.as_deref().map(str::trim)
                })
            })
            .cloned()
            .or_else(|| {
                control_plane_registry
                    .get("workspaces")
                    .and_then(Value::as_array)
                    .and_then(|items| {
                        items.iter().find(|entry| {
                            entry.get("name").and_then(Value::as_str).map(str::trim)
                                == Some(workspace.trim())
                        })
                    })
                    .cloned()
            })
    });

    let portfolio = json!({
        "counts": {
            "projects": control_plane_registry.get("project_count").cloned().unwrap_or_else(|| json!(0)),
            "companies": control_plane_registry.get("company_count").cloned().unwrap_or_else(|| json!(0)),
            "programs": control_plane_registry.get("program_count").cloned().unwrap_or_else(|| json!(0)),
            "workspaces": control_plane_registry.get("workspace_count").cloned().unwrap_or_else(|| json!(0)),
        },
        "company": current_company,
        "program": current_program,
        "workspace": current_workspace,
    });
    let provider_policy = json!({
        "runtime_providers": ["claude", "codex", "gemini", "opencode"],
        "contract_providers": ["shared", "claude", "codex", "gemini", "opencode"],
        "rules": provider_policy_matrix(),
    });
    let runtime_contract = json!({
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
        "scheduler": {
            "autorun_enabled": crate::config::scheduler_autorun_enabled(),
            "interval_secs": crate::config::scheduler_interval_secs(),
            "claim_ttl_secs": crate::config::session_launch_claim_ttl_secs(),
            "supports_run_id": true,
            "idempotent_ticks": true,
        },
        "supervisor": {
            "contract_client": if crate::config::http_supervisor_base_url().is_some() { "remote_http" } else { "in_process_router" },
            "autorun_enabled": crate::config::http_supervisor_autorun_enabled(),
            "interval_secs": crate::config::http_supervisor_interval_secs(),
            "event_driven": true,
            "base_url": crate::config::http_supervisor_base_url(),
            "identity": crate::config::http_supervisor_id(),
        },
        "control_endpoints": {
            "portfolio_brief": "/api/dxos/portfolio/brief",
            "project_identity": "/api/dxos/project/identity",
            "company_record": "/api/dxos/company",
            "program_record": "/api/dxos/program",
            "workspace_record": "/api/dxos/workspace",
            "scheduler_run": "/api/dxos/scheduler/run",
            "session_launch": "/api/dxos/session/launch",
            "provider_plugin_sync": "/api/dxos/provider-plugins/sync",
            "automation_bridge_sync": "/api/dxos/automation-bridges/sync",
            "workflow_list": "/api/dxos/workflows",
            "workflow_start": "/api/dxos/workflow/start",
            "workflow_step": "/api/dxos/workflow/step",
            "pane_talk": "/api/pane/talk",
            "pane_kill": "/api/pane/kill",
            "pane_restart": "/api/pane/restart",
            "pane_output": "/api/pane/{id}/output",
            "event_stream": "/api/events",
            "websocket": "/ws",
        },
    });
    let sessions = json!({
        "total": state.sessions.len(),
        "active": active_sessions,
        "blocked": blocked_sessions,
        "records": state.sessions.iter().map(session_summary).collect::<Vec<_>>(),
    });
    let delegation = json!({
        "total_work_orders": state.work_orders.len(),
        "active_work_orders": active_work_orders,
        "blocked_work_orders": blocked_work_orders,
        "recent": state.work_orders.iter().rev().take(10).map(work_order_summary).collect::<Vec<_>>(),
    });
    let workflow_runner = json!({
        "total_runs": state.workflow_runs.len(),
        "active_runs": active_workflow_runs,
        "blocked_runs": blocked_workflow_runs,
        "recent": state.workflow_runs.iter().rev().take(10).map(workflow_run_summary).collect::<Vec<_>>(),
    });
    let audit = json!({
        "total": audit_record_count(project_path),
        "recent": audit_recent,
    });

    json!({
        "project": state.project,
        "portfolio": portfolio,
        "defaults": state.defaults,
        "provider_policy": provider_policy,
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
        "provider_plugins": crate::provider_plugins::plugin_inventory(),
        "automation_bridges": crate::provider_asset_plugins::plugin_inventory(Some(project_path)),
        "control_plane_registry": control_plane_registry,
        "storage": control_plane_storage_summary(project_path),
        "runtime_contract": runtime_contract,
        "sessions": sessions,
        "delegation": delegation,
        "scheduler": scheduler_summary(&state),
        "workflow_runner": workflow_runner,
        "audit": audit,
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
        "scheduler": scheduler_summary(&state),
        "sessions": state.sessions.iter().map(session_summary).collect::<Vec<_>>(),
        "work_orders": state.work_orders.iter().map(work_order_summary).collect::<Vec<_>>(),
        "workflow_runs": state.workflow_runs.iter().map(workflow_run_summary).collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn scheduler_snapshot(project_path: &str, project_name: Option<&str>) -> String {
    let state = load_control_plane(project_path, project_name);
    json!({
        "project": state.project,
        "scheduler": scheduler_summary(&state),
    })
    .to_string()
}

pub fn scheduler_run_replay(
    project_path: &str,
    project_name: Option<&str>,
    run_id: &str,
) -> Option<Value> {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return None;
    }
    let state = load_control_plane(project_path, project_name);
    scheduler_run_result(&state, run_id)
}

pub fn remember_scheduler_run_result(
    project_path: &str,
    project_name: Option<&str>,
    actor: &str,
    run_id: &str,
    result: Value,
) -> Value {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return result;
    }
    let mut state = load_control_plane(project_path, project_name);
    let stored = remember_scheduler_run(&mut state, actor, run_id, result);
    let _ = save_control_plane(project_path, &state);
    stored
}

fn workflow_step_status(status: &str) -> Option<&'static str> {
    match status.trim().to_lowercase().as_str() {
        "planned" => Some("planned"),
        "in_progress" | "active" => Some("in_progress"),
        "completed" | "done" => Some("completed"),
        "blocked" => Some("blocked"),
        "skipped" => Some("skipped"),
        _ => None,
    }
}

fn recompute_workflow_run_status(steps: &[WorkflowStepRecord]) -> &'static str {
    if steps.iter().any(|step| step.status == "blocked") {
        return "blocked";
    }
    if !steps.is_empty()
        && steps
            .iter()
            .all(|step| matches!(step.status.as_str(), "completed" | "skipped"))
    {
        return "completed";
    }
    if steps.iter().any(|step| {
        matches!(
            step.status.as_str(),
            "in_progress" | "completed" | "skipped"
        )
    }) {
        return "active";
    }
    "planned"
}

#[derive(Clone, Copy)]
enum WorkflowRuntimeSignal {
    Activate,
    Block,
    Complete,
}

struct WorkflowAutoUpdate {
    workflow_run_id: String,
    action: &'static str,
}

fn workflow_runtime_rank(status: &str) -> u8 {
    match status {
        "blocked" => 0,
        "active" => 1,
        "planned" => 2,
        _ => 3,
    }
}

fn find_linked_workflow_run_index(
    state: &ControlPlaneState,
    session_id: Option<&str>,
    work_order_id: Option<&str>,
) -> Option<usize> {
    let mut matches = state
        .workflow_runs
        .iter()
        .enumerate()
        .filter(|(_, run)| {
            !matches!(run.status.as_str(), "completed" | "cancelled")
                && (session_id
                    .map(|value| run.session_id.as_deref() == Some(value))
                    .unwrap_or(false)
                    || work_order_id
                        .map(|value| run.work_order_id.as_deref() == Some(value))
                        .unwrap_or(false))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        let left_rank = workflow_runtime_rank(left.1.status.as_str());
        let right_rank = workflow_runtime_rank(right.1.status.as_str());
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.1.updated_at.cmp(&left.1.updated_at))
    });
    matches.first().map(|(index, _)| *index)
}

fn reconcile_linked_workflow_run(
    state: &mut ControlPlaneState,
    session_id: Option<&str>,
    work_order_id: Option<&str>,
    signal: WorkflowRuntimeSignal,
    note: Option<&str>,
    now: &str,
) -> Option<WorkflowAutoUpdate> {
    let run_index = find_linked_workflow_run_index(state, session_id, work_order_id)?;
    let note = note
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let run = &mut state.workflow_runs[run_index];
    let mut changed = false;
    let action = match signal {
        WorkflowRuntimeSignal::Activate => {
            if let Some(step) = run.steps.iter_mut().find(|step| step.status == "blocked") {
                if step.status != "in_progress" {
                    step.status = "in_progress".to_string();
                    changed = true;
                }
                if note.is_some() && step.note != note {
                    step.note = note.clone();
                    changed = true;
                }
                step.updated_at = now.to_string();
                Some("workflow_run_auto_resumed")
            } else if run.steps.iter().any(|step| step.status == "in_progress") {
                None
            } else if let Some(step) = run.steps.iter_mut().find(|step| step.status == "planned") {
                step.status = "in_progress".to_string();
                if note.is_some() && step.note != note {
                    step.note = note.clone();
                }
                step.updated_at = now.to_string();
                changed = true;
                Some("workflow_run_auto_activated")
            } else if run.steps.is_empty() && run.status != "active" {
                changed = true;
                Some("workflow_run_auto_activated")
            } else {
                None
            }
        }
        WorkflowRuntimeSignal::Block => {
            if let Some(step) = run
                .steps
                .iter_mut()
                .find(|step| matches!(step.status.as_str(), "in_progress" | "planned" | "blocked"))
            {
                if step.status != "blocked" {
                    step.status = "blocked".to_string();
                    changed = true;
                }
                if note.is_some() && step.note != note {
                    step.note = note.clone();
                    changed = true;
                }
                step.updated_at = now.to_string();
                Some("workflow_run_auto_blocked")
            } else if run.steps.is_empty() && run.status != "blocked" {
                changed = true;
                Some("workflow_run_auto_blocked")
            } else {
                None
            }
        }
        WorkflowRuntimeSignal::Complete => {
            for step in run
                .steps
                .iter_mut()
                .filter(|step| !matches!(step.status.as_str(), "completed" | "skipped"))
            {
                step.status = "completed".to_string();
                if note.is_some() {
                    step.note = note.clone();
                }
                step.updated_at = now.to_string();
                changed = true;
            }
            if run.steps.is_empty() && run.status != "completed" {
                changed = true;
            }
            if changed {
                Some("workflow_run_auto_completed")
            } else {
                None
            }
        }
    }?;

    if !changed {
        return None;
    }

    run.status = match signal {
        WorkflowRuntimeSignal::Complete if run.steps.is_empty() => "completed".to_string(),
        WorkflowRuntimeSignal::Activate if run.steps.is_empty() => "active".to_string(),
        WorkflowRuntimeSignal::Block if run.steps.is_empty() => "blocked".to_string(),
        _ => recompute_workflow_run_status(&run.steps).to_string(),
    };
    run.updated_at = now.to_string();
    Some(WorkflowAutoUpdate {
        workflow_run_id: run.id.clone(),
        action,
    })
}

pub fn workflow_run_list(project_path: &str, project_name: Option<&str>) -> String {
    let state = load_control_plane(project_path, project_name);
    json!({
        "project": state.project,
        "catalog": crate::provider_asset_plugins::shared_workflow_catalog(Some(project_path), None),
        "workflow_runs": state.workflow_runs.iter().map(workflow_run_summary).collect::<Vec<_>>(),
    })
    .to_string()
}

#[allow(clippy::too_many_arguments)]
pub fn start_workflow_run(
    project_path: &str,
    project_name: Option<&str>,
    workflow_id: &str,
    requested_by: Option<&str>,
    supervisor_session_id: Option<&str>,
    worker_session_id: Option<&str>,
    feature_id: Option<&str>,
    stage: Option<&str>,
    role: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
) -> String {
    if workflow_id.trim().is_empty() {
        return json!({"error": "workflow_id required"}).to_string();
    }

    let Some(workflow) = crate::provider_asset_plugins::shared_workflow_definition(
        Some(project_path),
        None,
        workflow_id,
    ) else {
        return json!({
            "error": "workflow_not_found",
            "workflow_id": workflow_id.trim(),
        })
        .to_string();
    };

    let mut state = load_control_plane(project_path, project_name);
    let workflow_name = workflow
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(workflow_id)
        .trim()
        .to_string();
    let workflow_kind = workflow
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("workflow")
        .trim()
        .to_string();
    let workflow_scope = workflow
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("project")
        .trim()
        .to_string();
    let workflow_summary_text = workflow
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("Execute the selected DX workflow.")
        .trim()
        .to_string();
    let workflow_sources = workflow
        .get("sources")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let workflow_sections = workflow
        .get("sections")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let step_titles = workflow
        .get("steps")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(|value| value.trim().to_string()))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let normalized_stage = normalize_stage(stage);
    let requested_feature = feature_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let mut chosen_worker_session_id = worker_session_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(worker_id) = chosen_worker_session_id.as_deref() {
        if !state.sessions.iter().any(|session| session.id == worker_id) {
            return json!({"error": "worker_session_not_found"}).to_string();
        }
    }

    let explicit_supervisor = supervisor_session_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(supervisor_id) = explicit_supervisor.as_deref() {
        if !state
            .sessions
            .iter()
            .any(|session| session.id == supervisor_id)
        {
            return json!({"error": "supervisor_session_not_found"}).to_string();
        }
    }

    let now = crate::state::now();
    let mut created_session_id = None;
    if chosen_worker_session_id.is_none() {
        let session_id = next_session_id(&state);
        let chosen_role = normalize_role(role.unwrap_or("workflow_runner"));
        let (normalized_provider, _provider_policy, policy_violations) =
            validate_provider_selection(
                &chosen_role,
                Some(&normalized_stage),
                provider,
                None,
                None,
            );
        let session_status = if policy_violations.is_empty() {
            "planned".to_string()
        } else {
            "blocked".to_string()
        };
        state.sessions.push(SessionContractRecord {
            id: session_id.clone(),
            status: session_status,
            role: chosen_role,
            priority: default_priority(),
            provider: normalized_provider,
            model: model
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            autonomy_level: "guarded_auto".to_string(),
            objective: format!(
                "Execute workflow {}. {}",
                workflow_name, workflow_summary_text
            ),
            expected_outputs: vec![
                "workflow_completion".to_string(),
                "workflow_evidence".to_string(),
                "workflow_handoff".to_string(),
            ],
            allowed_capabilities: vec![
                "automation_bridge".to_string(),
                "dx_workflow_runner".to_string(),
                format!(
                    "workflow:{}",
                    workflow_name.to_lowercase().replace(' ', "_")
                ),
            ],
            allowed_repos: vec![state.project.path.clone()],
            allowed_paths: vec![state.project.path.clone()],
            workspace_path: Some(state.project.path.clone()),
            branch_name: None,
            browser_port: None,
            pane: None,
            runtime_adapter: None,
            tmux_target: None,
            feature_id: requested_feature.clone(),
            stage: Some(normalized_stage.clone()),
            supervisor_session_id: explicit_supervisor.clone(),
            escalation_policy: Some("lead_first_workflow_runner".to_string()),
            policy_violations,
            last_error: None,
            launch_claimed_by: None,
            launch_claimed_at: None,
            launch_claim_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        chosen_worker_session_id = Some(session_id.clone());
        created_session_id = Some(session_id);
    }

    let worker_id = chosen_worker_session_id.clone().unwrap_or_default();
    let inferred_supervisor = explicit_supervisor.or_else(|| {
        state
            .sessions
            .iter()
            .find(|session| session.id == worker_id)
            .and_then(|session| session.supervisor_session_id.clone())
            .filter(|value| !value.trim().is_empty())
    });
    let chosen_supervisor_id = inferred_supervisor.unwrap_or_else(|| worker_id.clone());
    if !state
        .sessions
        .iter()
        .any(|session| session.id == chosen_supervisor_id)
    {
        return json!({"error": "supervisor_session_not_found"}).to_string();
    }

    let work_order_id = next_work_order_id(&state);
    let expected_outputs = if step_titles.is_empty() {
        vec!["workflow_completion".to_string()]
    } else {
        step_titles
            .iter()
            .take(6)
            .map(|step| format!("step: {}", step))
            .collect::<Vec<_>>()
    };
    state.work_orders.push(WorkOrderRecord {
        id: work_order_id.clone(),
        supervisor_session_id: chosen_supervisor_id.clone(),
        worker_session_id: Some(worker_id.clone()),
        status: "assigned".to_string(),
        priority: default_priority(),
        escalation_target: "lead".to_string(),
        title: format!("Execute workflow {}", workflow_name),
        objective: format!(
            "Run workflow {} and complete its governed steps inside DXOS.",
            workflow_name
        ),
        feature_id: requested_feature.clone(),
        stage: Some(normalized_stage.clone()),
        required_capabilities: vec![
            "dx_workflow_runner".to_string(),
            "automation_bridge".to_string(),
        ],
        blockers: Vec::new(),
        requested_permissions: Vec::new(),
        expected_outputs,
        resolution_notes: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });

    let workflow_run_id = next_workflow_run_id(&state);
    state.workflow_runs.push(WorkflowRunRecord {
        id: workflow_run_id.clone(),
        workflow_id: workflow_id.trim().to_string(),
        name: workflow_name.clone(),
        kind: workflow_kind,
        scope: workflow_scope,
        summary: workflow_summary_text,
        status: "planned".to_string(),
        source_provider: workflow
            .get("canonical_provider")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        feature_id: requested_feature,
        stage: Some(normalized_stage),
        requested_by: requested_by
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        session_id: Some(worker_id.clone()),
        work_order_id: Some(work_order_id.clone()),
        supervisor_session_id: Some(chosen_supervisor_id),
        sources: workflow_sources,
        source_path: workflow
            .get("source_path")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        sections: workflow_sections,
        steps: step_titles
            .iter()
            .enumerate()
            .map(|(index, title)| WorkflowStepRecord {
                id: format!("STEP{:02}", index + 1),
                title: title.clone(),
                status: "planned".to_string(),
                note: None,
                updated_at: now.clone(),
            })
            .collect(),
        created_at: now.clone(),
        updated_at: now.clone(),
    });
    let worker_session_is_active = state
        .sessions
        .iter()
        .find(|session| session.id == worker_id)
        .map(|session| session.status == "active")
        .unwrap_or(false);
    let workflow_auto_update = if worker_session_is_active {
        reconcile_linked_workflow_run(
            &mut state,
            Some(worker_id.as_str()),
            Some(work_order_id.as_str()),
            WorkflowRuntimeSignal::Activate,
            Some("Auto-started from linked active session."),
            &now,
        )
    } else {
        None
    };
    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "workflow_run_started",
            "project": state.project.name,
            "project_path": project_path,
            "workflow_run_id": workflow_run_id,
            "session_id": created_session_id.or_else(|| Some(worker_id.clone())),
            "work_order_id": work_order_id,
            "workflow": workflow,
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action).unwrap_or("workflow_run_started"),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .or_else(|| state.workflow_runs.iter().find(|item| item.id == workflow_run_id))
                .map(workflow_run_summary),
            "session": state.sessions.iter().find(|item| item.id == worker_id).map(session_summary),
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id).map(work_order_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn update_workflow_run_step(
    project_path: &str,
    project_name: Option<&str>,
    workflow_run_id: &str,
    step_id: &str,
    status: &str,
    note: Option<&str>,
) -> String {
    if workflow_run_id.trim().is_empty() || step_id.trim().is_empty() {
        return json!({"error": "workflow_run_id and step_id required"}).to_string();
    }
    let Some(normalized_step_status) = workflow_step_status(status) else {
        return json!({"error": "invalid_step_status"}).to_string();
    };

    let mut state = load_control_plane(project_path, project_name);
    let now = crate::state::now();
    let Some(run_index) = state
        .workflow_runs
        .iter()
        .position(|item| item.id == workflow_run_id.trim())
    else {
        return json!({"error": "workflow_run_not_found"}).to_string();
    };

    let session_id = state.workflow_runs[run_index].session_id.clone();
    let work_order_id = state.workflow_runs[run_index].work_order_id.clone();
    let step_found = {
        let run = &mut state.workflow_runs[run_index];
        let Some(step) = run.steps.iter_mut().find(|item| item.id == step_id.trim()) else {
            return json!({"error": "workflow_step_not_found"}).to_string();
        };
        step.status = normalized_step_status.to_string();
        step.note = note
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        step.updated_at = now.clone();
        run.status = recompute_workflow_run_status(&run.steps).to_string();
        run.updated_at = now.clone();
        true
    };
    if !step_found {
        return json!({"error": "workflow_step_not_found"}).to_string();
    }

    let run_status = state.workflow_runs[run_index].status.clone();
    if let Some(work_order_id) = work_order_id.as_deref() {
        if let Some(work_order) = state
            .work_orders
            .iter_mut()
            .find(|item| item.id == work_order_id)
        {
            work_order.status = match run_status.as_str() {
                "blocked" => "blocked".to_string(),
                "completed" => "completed".to_string(),
                "active" => "assigned".to_string(),
                _ => "assigned".to_string(),
            };
            if let Some(message) = note.filter(|value| !value.trim().is_empty()) {
                work_order.resolution_notes.push(WorkResolutionRecord {
                    message: format!(
                        "{} [{}]: {}",
                        step_id.trim(),
                        normalized_step_status,
                        message.trim()
                    ),
                    created_at: now.clone(),
                });
            }
            work_order.updated_at = now.clone();
        }
    }
    if let Some(session_id) = session_id.as_deref() {
        if let Some(session) = state.sessions.iter_mut().find(|item| item.id == session_id) {
            session.status = match run_status.as_str() {
                "blocked" => "blocked".to_string(),
                "completed" => "completed".to_string(),
                "active" => "active".to_string(),
                _ => "planned".to_string(),
            };
            if session.status != "blocked" {
                session.last_error = None;
            }
            if session.status != "launching" {
                clear_session_launch_claim(session);
            }
            session.updated_at = now.clone();
        }
    }
    state.updated_at = now;

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "workflow_run_step_updated",
            "project": state.project.name,
            "project_path": project_path,
            "workflow_run_id": workflow_run_id.trim(),
            "workflow_run": state.workflow_runs.iter().find(|item| item.id == workflow_run_id.trim()).map(workflow_run_summary),
            "session": session_id.as_deref().and_then(|id| state.sessions.iter().find(|item| item.id == id).map(session_summary)),
            "work_order": work_order_id.as_deref().and_then(|id| state.work_orders.iter().find(|item| item.id == id).map(work_order_summary)),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn runtime_launch_context(
    project_path: &str,
    project_name: Option<&str>,
    session_id: &str,
) -> Value {
    let trimmed_session_id = session_id.trim();
    if trimmed_session_id.is_empty() {
        return json!({"error": "session_id_required"});
    }

    let state = load_control_plane(project_path, project_name);
    let Some(session) = state
        .sessions
        .iter()
        .find(|item| item.id == trimmed_session_id)
    else {
        return json!({
            "error": "session_not_found",
            "session_id": trimmed_session_id,
            "project": state.project.name,
        });
    };

    let mut work_orders = state
        .work_orders
        .iter()
        .filter(|work_order| {
            work_order.worker_session_id.as_deref() == Some(trimmed_session_id)
                && !matches!(work_order.status.as_str(), "completed" | "cancelled")
        })
        .collect::<Vec<_>>();
    work_orders.sort_by(|left, right| {
        let left_rank = match left.status.as_str() {
            "assigned" => 0,
            "blocked" => 1,
            "planned" => 2,
            _ => 3,
        };
        let right_rank = match right.status.as_str() {
            "assigned" => 0,
            "blocked" => 1,
            "planned" => 2,
            _ => 3,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    let mut workflow_runs = state
        .workflow_runs
        .iter()
        .filter(|run| {
            run.session_id.as_deref() == Some(trimmed_session_id)
                && !matches!(run.status.as_str(), "completed" | "cancelled")
        })
        .collect::<Vec<_>>();
    workflow_runs.sort_by(|left, right| {
        let left_rank = match left.status.as_str() {
            "blocked" => 0,
            "active" => 1,
            "planned" => 2,
            _ => 3,
        };
        let right_rank = match right.status.as_str() {
            "blocked" => 0,
            "active" => 1,
            "planned" => 2,
            _ => 3,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });

    let adoption = state.adoptions.iter().find(|adoption| {
        adoption.lead_session_id == trimmed_session_id
            && !matches!(adoption.status.as_str(), "completed" | "cancelled")
    });
    let debate = adoption.and_then(|adoption| {
        state
            .debates
            .iter()
            .find(|debate| debate.id == adoption.debate_id)
    });

    json!({
        "project": state.project,
        "session": session_summary(session),
        "primary_work_order": work_orders.first().map(|work_order| work_order_summary(work_order)),
        "work_orders": work_orders.iter().map(|work_order| work_order_summary(work_order)).collect::<Vec<_>>(),
        "primary_workflow_run": workflow_runs.first().map(|run| workflow_run_summary(run)),
        "workflow_runs": workflow_runs.iter().map(|run| workflow_run_summary(run)).collect::<Vec<_>>(),
        "adoption": adoption.map(adoption_summary),
        "debate": debate.map(debate_summary),
    })
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
    start_project_adoption_with_plan(
        project_path,
        project_name,
        summary,
        objective,
        feature_id,
        stage,
        participants,
        requested_by,
        Vec::new(),
    )
}

pub fn start_project_adoption_with_plan(
    project_path: &str,
    project_name: Option<&str>,
    summary: Option<&str>,
    objective: Option<&str>,
    feature_id: Option<&str>,
    stage: Option<&str>,
    participants: Vec<String>,
    requested_by: Option<&str>,
    follow_on_suggestions: Vec<SuggestedSessionPlan>,
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
    let follow_on_suggestions = follow_on_suggestions
        .into_iter()
        .filter(|item| {
            !item.role.trim().is_empty()
                && !item.stage.trim().is_empty()
                && !item.task_prompt.trim().is_empty()
        })
        .collect::<Vec<_>>();

    state.sessions.push(SessionContractRecord {
        id: lead_session_id.clone(),
        status: "planned".to_string(),
        role: "lead".to_string(),
        priority: "high".to_string(),
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
        launch_claimed_by: None,
        launch_claimed_at: None,
        launch_claim_id: None,
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
        worker_session_id: Some(lead_session_id.clone()),
        status: "assigned".to_string(),
        priority: "high".to_string(),
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
        follow_on_suggestions,
        follow_on_session_ids: Vec::new(),
        follow_on_work_order_ids: Vec::new(),
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
    let Some(adoption_index) = state
        .adoptions
        .iter()
        .position(|item| item.id == adoption_id.trim())
    else {
        return json!({"error": "adoption_not_found"}).to_string();
    };

    let updated_at = crate::state::now();
    {
        let adoption = &mut state.adoptions[adoption_index];
        adoption.status = normalized_status.clone();
        adoption.last_note = note
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        adoption.updated_at = updated_at.clone();
    }
    state.updated_at = updated_at.clone();
    let initial_work_order_id = state.adoptions[adoption_index]
        .initial_work_order_id
        .clone();
    let adoption_note = state.adoptions[adoption_index].last_note.clone();

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

    let mut follow_on_sessions = Vec::new();
    let mut follow_on_work_orders = Vec::new();
    if normalized_status == "completed"
        && state.adoptions[adoption_index]
            .follow_on_work_order_ids
            .is_empty()
    {
        let lead_session_id = state.adoptions[adoption_index].lead_session_id.clone();
        let fallback_feature_id = state.adoptions[adoption_index].feature_id.clone();
        let follow_on_suggestions = state.adoptions[adoption_index]
            .follow_on_suggestions
            .clone();
        for suggestion in follow_on_suggestions.into_iter().take(3) {
            let session_id = next_session_id(&state);
            let work_order_id = next_work_order_id(&state);
            let role = normalize_role(&suggestion.role);
            let stage = normalize_stage(Some(&suggestion.stage));
            let feature_id = suggestion
                .feature_id
                .clone()
                .or_else(|| fallback_feature_id.clone());
            let provider_policy = provider_policy_for(&role, Some(&stage));
            let allowed_capabilities = capabilities_for_role(&role);
            let expected_outputs = expected_outputs_for_role_stage(&role, Some(&stage));
            let title = if let Some(feature_id) = feature_id.as_deref() {
                format!("Recovery follow-on · {} · {}", role, feature_id)
            } else {
                format!("Recovery follow-on · {}", role)
            };

            state.sessions.push(SessionContractRecord {
                id: session_id.clone(),
                status: "planned".to_string(),
                role: role.clone(),
                priority: normalize_priority(Some(&suggestion.priority)),
                provider: Some(provider_policy.preferred_provider.clone()),
                model: provider_policy.suggested_models.first().cloned(),
                autonomy_level: "guarded_auto".to_string(),
                objective: suggestion.task_prompt.clone(),
                expected_outputs: expected_outputs.clone(),
                allowed_capabilities: allowed_capabilities.clone(),
                allowed_repos: vec![project_path.to_string()],
                allowed_paths: vec![project_path.to_string()],
                workspace_path: Some(project_path.to_string()),
                branch_name: None,
                browser_port: None,
                pane: None,
                runtime_adapter: Some("pty_native_adapter".to_string()),
                tmux_target: None,
                feature_id: feature_id.clone(),
                stage: Some(stage.clone()),
                supervisor_session_id: Some(lead_session_id.clone()),
                escalation_policy: Some("lead_then_human".to_string()),
                policy_violations: Vec::new(),
                last_error: None,
                launch_claimed_by: None,
                launch_claimed_at: None,
                launch_claim_id: None,
                created_at: updated_at.clone(),
                updated_at: updated_at.clone(),
            });
            state.work_orders.push(WorkOrderRecord {
                id: work_order_id.clone(),
                supervisor_session_id: lead_session_id.clone(),
                worker_session_id: Some(session_id.clone()),
                status: "planned".to_string(),
                priority: normalize_priority(Some(&suggestion.priority)),
                escalation_target: "lead".to_string(),
                title,
                objective: suggestion.task_prompt.clone(),
                feature_id: feature_id.clone(),
                stage: Some(stage.clone()),
                required_capabilities: allowed_capabilities,
                blockers: Vec::new(),
                requested_permissions: Vec::new(),
                expected_outputs,
                resolution_notes: Vec::new(),
                created_at: updated_at.clone(),
                updated_at: updated_at.clone(),
            });
            state.adoptions[adoption_index]
                .follow_on_session_ids
                .push(session_id.clone());
            state.adoptions[adoption_index]
                .follow_on_work_order_ids
                .push(work_order_id.clone());
            follow_on_sessions.push(
                state
                    .sessions
                    .iter()
                    .find(|item| item.id == session_id)
                    .map(session_summary)
                    .unwrap_or_else(|| json!({})),
            );
            follow_on_work_orders.push(
                state
                    .work_orders
                    .iter()
                    .find(|item| item.id == work_order_id)
                    .map(work_order_summary)
                    .unwrap_or_else(|| json!({})),
            );
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
            "follow_on_sessions": follow_on_sessions,
            "follow_on_work_orders": follow_on_work_orders,
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
        existing.status = computed_status.clone();
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
        if existing.status != "launching" {
            clear_session_launch_claim(existing);
        }
        existing.updated_at = now.clone();
        "session_updated"
    } else {
        state.sessions.push(SessionContractRecord {
            id: chosen_id.clone(),
            status: computed_status.clone(),
            role: role.trim().to_string(),
            priority: default_priority(),
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
            launch_claimed_by: None,
            launch_claimed_at: None,
            launch_claim_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        "session_registered"
    };

    let workflow_note_now = now.clone();
    let workflow_auto_update = match computed_status.as_str() {
        "active" => reconcile_linked_workflow_run(
            &mut state,
            Some(chosen_id.as_str()),
            None,
            WorkflowRuntimeSignal::Activate,
            Some("Auto-advanced from session activation."),
            &workflow_note_now,
        ),
        "blocked" | "failed" => reconcile_linked_workflow_run(
            &mut state,
            Some(chosen_id.as_str()),
            None,
            WorkflowRuntimeSignal::Block,
            Some("Auto-blocked from session state."),
            &workflow_note_now,
        ),
        "completed" => reconcile_linked_workflow_run(
            &mut state,
            Some(chosen_id.as_str()),
            None,
            WorkflowRuntimeSignal::Complete,
            Some("Auto-completed from session completion."),
            &workflow_note_now,
        ),
        _ => None,
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
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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
    if session.status != "launching" {
        clear_session_launch_claim(session);
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
    let workflow_updated_at = state.updated_at.clone();
    let workflow_auto_update = match status.trim() {
        "active" => reconcile_linked_workflow_run(
            &mut state,
            Some(session_id.trim()),
            None,
            WorkflowRuntimeSignal::Activate,
            note,
            &workflow_updated_at,
        ),
        "blocked" | "failed" => reconcile_linked_workflow_run(
            &mut state,
            Some(session_id.trim()),
            None,
            WorkflowRuntimeSignal::Block,
            note,
            &workflow_updated_at,
        ),
        "completed" => reconcile_linked_workflow_run(
            &mut state,
            Some(session_id.trim()),
            None,
            WorkflowRuntimeSignal::Complete,
            note,
            &workflow_updated_at,
        ),
        _ => None,
    };

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_status_updated",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
        })
        .to_string(),
        Err(error) => json!({"error": error}).to_string(),
    }
}

pub fn claim_session_launch(
    project_path: &str,
    project_name: Option<&str>,
    session_id: &str,
    actor: Option<&str>,
    claim_id: Option<&str>,
) -> String {
    if session_id.trim().is_empty() {
        return json!({"error": "session_id required"}).to_string();
    }

    let mut state = load_control_plane(project_path, project_name);
    let Some(session) = state
        .sessions
        .iter_mut()
        .find(|item| item.id == session_id.trim())
    else {
        return json!({"error": "session_not_found"}).to_string();
    };

    let claimed_by = actor
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "dxos_scheduler".to_string());
    let claim_id = claim_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let reclaiming = session.status == "launching" && launch_claim_is_stale(session);
    if !reclaiming
        && session.status == "launching"
        && session.launch_claimed_by.as_deref() == Some(claimed_by.as_str())
        && claim_id.is_some()
        && session.launch_claim_id.as_deref() == claim_id.as_deref()
    {
        return json!({
            "status": "ok",
            "action": "session_launch_claim_existing",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id.trim(),
            "claimed_by": claimed_by,
            "claim_id": claim_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
        })
        .to_string();
    }
    if session.status != "planned" && !reclaiming {
        return json!({
            "error": "session_not_launchable",
            "session_id": session_id.trim(),
            "status": session.status,
            "launch_claimed_by": session.launch_claimed_by,
            "launch_claimed_at": session.launch_claimed_at,
            "launch_claim_id": session.launch_claim_id,
        })
        .to_string();
    }

    if !session.policy_violations.is_empty() {
        return json!({
            "error": "session_policy_blocked",
            "session_id": session_id.trim(),
            "policy_violations": session.policy_violations,
        })
        .to_string();
    }

    let now = crate::state::now();
    session.status = "launching".to_string();
    session.last_error = None;
    set_session_launch_claim(session, &claimed_by, claim_id.as_deref(), &now);
    session.updated_at = now;
    state.updated_at = session.updated_at.clone();

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": if reclaiming { "session_launch_reclaimed" } else { "session_launch_claimed" },
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id.trim(),
            "claimed_by": claimed_by,
            "claim_id": claim_id,
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
    clear_session_launch_claim(session);
    session.updated_at = crate::state::now();
    state.updated_at = session.updated_at.clone();
    let workflow_updated_at = state.updated_at.clone();
    let workflow_auto_update = reconcile_linked_workflow_run(
        &mut state,
        Some(session_id.trim()),
        None,
        WorkflowRuntimeSignal::Block,
        Some(error.trim()),
        &workflow_updated_at,
    );

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_launch_failed",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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
        priority: default_priority(),
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
        work_order.priority = "high".to_string();
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
            priority: "high".to_string(),
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
    clear_session_launch_claim(session);
    session.updated_at = now.clone();
    state.updated_at = now.clone();
    let workflow_auto_update = reconcile_linked_workflow_run(
        &mut state,
        Some(worker_session_id.trim()),
        Some(work_order_id.as_str()),
        WorkflowRuntimeSignal::Block,
        Some(blocker.trim()),
        &now,
    );

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
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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
    work_order.priority = "high".to_string();
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

    if let Some(ref worker_session_id) = worker_session_id {
        if let Some(session) = state
            .sessions
            .iter_mut()
            .find(|item| item.id == *worker_session_id)
        {
            session.status = "blocked".to_string();
            clear_session_launch_claim(session);
            session.updated_at = state.updated_at.clone();
        }
    }
    let workflow_updated_at = state.updated_at.clone();
    let workflow_auto_update = reconcile_linked_workflow_run(
        &mut state,
        worker_session_id.as_deref(),
        Some(work_order_id.trim()),
        WorkflowRuntimeSignal::Block,
        Some(blocker.trim()),
        &workflow_updated_at,
    );

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "work_order_blocked",
            "project": state.project.name,
            "project_path": project_path,
            "work_order_id": work_order_id,
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id.trim()).map(work_order_summary),
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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

    if let Some(ref worker_session_id) = worker_session_id {
        if let Some(session) = state
            .sessions
            .iter_mut()
            .find(|item| item.id == *worker_session_id)
        {
            session.status = "active".to_string();
            session.last_error = None;
            clear_session_launch_claim(session);
            session.updated_at = state.updated_at.clone();
        }
    }
    let workflow_updated_at = state.updated_at.clone();
    let workflow_auto_update = reconcile_linked_workflow_run(
        &mut state,
        worker_session_id.as_deref(),
        Some(work_order_id.trim()),
        WorkflowRuntimeSignal::Activate,
        resolution,
        &workflow_updated_at,
    );

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "work_order_resolved",
            "project": state.project.name,
            "project_path": project_path,
            "work_order_id": work_order_id,
            "work_order": state.work_orders.iter().find(|item| item.id == work_order_id.trim()).map(work_order_summary),
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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
    clear_session_launch_claim(session);
    session.updated_at = crate::state::now();
    state.updated_at = session.updated_at.clone();
    let workflow_updated_at = state.updated_at.clone();
    let workflow_auto_update = reconcile_linked_workflow_run(
        &mut state,
        Some(session_id.trim()),
        None,
        WorkflowRuntimeSignal::Block,
        Some(error.trim()),
        &workflow_updated_at,
    );

    match save_control_plane(project_path, &state) {
        Ok(()) => json!({
            "status": "ok",
            "action": "session_delivery_failed",
            "project": state.project.name,
            "project_path": project_path,
            "session_id": session_id,
            "session": state.sessions.iter().find(|item| item.id == session_id.trim()).map(session_summary),
            "workflow_run_id": workflow_auto_update.as_ref().map(|update| update.workflow_run_id.clone()),
            "workflow_action": workflow_auto_update.as_ref().map(|update| update.action),
            "workflow_run": workflow_auto_update
                .as_ref()
                .and_then(|update| state.workflow_runs.iter().find(|item| item.id == update.workflow_run_id))
                .map(workflow_run_summary),
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

pub fn workflow_run_event_from_result(project_path: &str, result: &str) -> Option<StateEvent> {
    let value = serde_json::from_str::<Value>(result).ok()?;
    if value.get("error").is_some() {
        return None;
    }
    let workflow_run = value.get("workflow_run")?;
    let project = value
        .get("project")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| resolved_project_name(project_path, None));
    Some(StateEvent::WorkflowRunChanged {
        project,
        workflow_run_id: value
            .get("workflow_run_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        workflow_id: workflow_run
            .get("workflow_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        status: workflow_run
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        action: value
            .get("workflow_action")
            .and_then(Value::as_str)
            .or_else(|| value.get("action").and_then(Value::as_str))
            .unwrap_or("workflow_run_updated")
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
    fn project_identity_is_persisted_and_exposed_in_registry() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let updated: Value = serde_json::from_str(&upsert_project_identity(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
        ))
        .unwrap();
        assert_eq!(updated["action"], "project_identity_updated");
        assert_eq!(updated["project"]["company"], "DX Ventures");

        let snapshot = control_plane_snapshot(project, Some("demo"));
        assert_eq!(snapshot["project"]["program"], "Agentic Delivery");

        let registry: Value =
            serde_json::from_str(&control_plane_registry_for_project(project)).unwrap();
        let entry = registry["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == project)
            .unwrap();
        assert_eq!(entry["company"], "DX Ventures");
        assert_eq!(entry["workspace"], "core-platform");
    }

    #[test]
    fn control_plane_registry_groups_company_and_program_portfolio() {
        let tmp = tempdir().unwrap();
        let project_a = tmp.path().join("alpha");
        let project_b = tmp.path().join("beta");
        std::fs::create_dir_all(&project_a).unwrap();
        std::fs::create_dir_all(&project_b).unwrap();

        let project_a_str = project_a.to_str().unwrap();
        let project_b_str = project_b.to_str().unwrap();

        let _ = upsert_project_identity(
            project_a_str,
            Some("alpha"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
        );
        let _ = upsert_project_identity(
            project_b_str,
            Some("beta"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("client-portal"),
        );

        let registry: Value =
            serde_json::from_str(&control_plane_registry_for_project(project_a_str)).unwrap();
        assert_eq!(registry["company_count"], 1);
        assert_eq!(registry["program_count"], 1);
        assert_eq!(registry["workspace_count"], 2);
        assert_eq!(registry["companies"][0]["name"], "DX Ventures");
        assert_eq!(registry["companies"][0]["project_count"], 2);
        assert_eq!(registry["programs"][0]["name"], "Agentic Delivery");
        assert_eq!(registry["programs"][0]["project_count"], 2);
    }

    #[test]
    fn project_identity_seeds_first_class_portfolio_records() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let _ = upsert_project_identity(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
        );

        let registry: Value =
            serde_json::from_str(&control_plane_registry_for_project(project)).unwrap();
        assert_eq!(registry["companies"][0]["name"], "DX Ventures");
        assert_eq!(registry["companies"][0]["status"], "active");
        assert_eq!(registry["programs"][0]["company"], "DX Ventures");
        assert_eq!(registry["programs"][0]["status"], "active");
        assert_eq!(registry["workspaces"][0]["program"], "Agentic Delivery");
        assert_eq!(registry["workspaces"][0]["status"], "active");
    }

    #[test]
    fn portfolio_record_metadata_merges_into_registry_and_snapshot() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let _ = upsert_project_identity(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
        );
        let _ = upsert_company_record(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("AI-led company portfolio"),
            Some("active"),
            Some("ops-lead"),
        );
        let _ = upsert_program_record(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("Delivery OS rollout"),
            Some("planning"),
            Some("program-lead"),
        );
        let _ = upsert_workspace_record(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
            Some("Runtime and control plane"),
            Some("active"),
            Some("workspace-lead"),
        );

        let registry: Value =
            serde_json::from_str(&control_plane_registry_for_project(project)).unwrap();
        assert_eq!(registry["companies"][0]["owner"], "ops-lead");
        assert_eq!(registry["programs"][0]["summary"], "Delivery OS rollout");
        assert_eq!(registry["workspaces"][0]["owner"], "workspace-lead");

        let snapshot = control_plane_snapshot(project, Some("demo"));
        assert_eq!(
            snapshot["portfolio"]["company"]["summary"],
            "AI-led company portfolio"
        );
        assert_eq!(snapshot["portfolio"]["program"]["status"], "planning");
        assert_eq!(
            snapshot["portfolio"]["workspace"]["summary"],
            "Runtime and control plane"
        );
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
    fn project_adoption_completion_seeds_follow_on_specialists() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let started: Value = serde_json::from_str(&start_project_adoption_with_plan(
            project,
            Some("demo"),
            Some("Recover the inherited project"),
            Some("Map the current state and create the first governed recovery plan."),
            Some("F1.1"),
            Some("discovery"),
            vec!["lead".to_string(), "qa".to_string()],
            Some("ops-lead"),
            vec![
                SuggestedSessionPlan {
                    role: "design".to_string(),
                    stage: "design".to_string(),
                    priority: "high".to_string(),
                    feature_id: Some("F1.1".to_string()),
                    reason: "Prepare a client-ready direction.".to_string(),
                    task_prompt: "Prepare design options for F1.1.".to_string(),
                },
                SuggestedSessionPlan {
                    role: "qa".to_string(),
                    stage: "test".to_string(),
                    priority: "medium".to_string(),
                    feature_id: Some("F1.1".to_string()),
                    reason: "Backfill verification evidence.".to_string(),
                    task_prompt: "Prepare verification coverage for F1.1.".to_string(),
                },
            ],
        ))
        .unwrap();
        let adoption_id = started["adoption_id"].as_str().unwrap();

        let updated: Value = serde_json::from_str(&update_project_adoption_status(
            project,
            Some("demo"),
            adoption_id,
            "completed",
            Some("Recovery plan accepted."),
        ))
        .unwrap();

        assert_eq!(updated["follow_on_sessions"].as_array().unwrap().len(), 2);
        assert_eq!(
            updated["follow_on_work_orders"].as_array().unwrap().len(),
            2
        );
        assert_eq!(updated["follow_on_sessions"][0]["status"], "planned");
        assert_eq!(updated["follow_on_work_orders"][0]["status"], "planned");

        let sessions: Value = serde_json::from_str(&session_list(project, Some("demo"))).unwrap();
        assert_eq!(sessions["adoptions"][0]["follow_on_count"], 2);
        assert_eq!(
            sessions["adoptions"][0]["follow_on_session_ids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            sessions["adoptions"][0]["follow_on_work_order_ids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(sessions["scheduler"]["launch_queue"][0]["priority"], "high");
        assert_eq!(sessions["scheduler"]["launch_queue"][0]["role"], "design");
        assert_eq!(
            sessions["scheduler"]["launch_queue"][1]["priority"],
            "medium"
        );
    }

    #[test]
    fn stale_launch_claim_can_be_reclaimed() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let session: Value = serde_json::from_str(&upsert_session_contract(
            project,
            Some("demo"),
            None,
            "design",
            Some("claude"),
            Some("claude-opus-4.6"),
            Some("guarded_auto"),
            "Prepare the first concept",
            vec!["mockups".to_string()],
            vec!["docs".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            None,
            None,
            None,
            Some("pty_native_adapter"),
            None,
            Some("F1.1"),
            Some("design"),
            None,
            Some("lead_then_human"),
            Some("planned"),
        ))
        .unwrap();
        let session_id = session["session_id"].as_str().unwrap();

        let initial_claim: Value = serde_json::from_str(&claim_session_launch(
            project,
            Some("demo"),
            session_id,
            Some("scheduler-a"),
            None,
        ))
        .unwrap();
        assert_eq!(initial_claim["action"], "session_launch_claimed");

        let mut state = load_control_plane(project, Some("demo"));
        let stale_at = (chrono::Local::now()
            - chrono::Duration::seconds(crate::config::session_launch_claim_ttl_secs() as i64 + 5))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
        let session = state
            .sessions
            .iter_mut()
            .find(|item| item.id == session_id)
            .unwrap();
        session.status = "launching".to_string();
        session.launch_claimed_by = Some("scheduler-a".to_string());
        session.launch_claimed_at = Some(stale_at);
        session.updated_at = crate::state::now();
        save_control_plane(project, &state).unwrap();

        let reclaimed: Value = serde_json::from_str(&claim_session_launch(
            project,
            Some("demo"),
            session_id,
            Some("scheduler-b"),
            None,
        ))
        .unwrap();
        assert_eq!(reclaimed["action"], "session_launch_reclaimed");
        assert_eq!(reclaimed["claimed_by"], "scheduler-b");
        assert_eq!(reclaimed["session"]["status"], "launching");
        assert_eq!(reclaimed["session"]["launch_claimed_by"], "scheduler-b");
    }

    #[test]
    fn fresh_launch_claim_is_not_reclaimed() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let session: Value = serde_json::from_str(&upsert_session_contract(
            project,
            Some("demo"),
            None,
            "frontend",
            Some("codex"),
            Some("gpt-5.4"),
            Some("guarded_auto"),
            "Build the main surface",
            vec!["ui".to_string()],
            vec!["playwright".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            None,
            None,
            None,
            Some("pty_native_adapter"),
            None,
            Some("F2.1"),
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("planned"),
        ))
        .unwrap();
        let session_id = session["session_id"].as_str().unwrap();

        let _ = claim_session_launch(project, Some("demo"), session_id, Some("scheduler-a"), None);
        let blocked: Value = serde_json::from_str(&claim_session_launch(
            project,
            Some("demo"),
            session_id,
            Some("scheduler-b"),
            None,
        ))
        .unwrap();
        assert_eq!(blocked["error"], "session_not_launchable");
        assert_eq!(blocked["status"], "launching");
        assert_eq!(blocked["launch_claimed_by"], "scheduler-a");
    }

    #[test]
    fn same_run_id_reuses_existing_launch_claim() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let session: Value = serde_json::from_str(&upsert_session_contract(
            project,
            Some("demo"),
            None,
            "frontend",
            Some("codex"),
            Some("gpt-5.4"),
            Some("guarded_auto"),
            "Build the main surface",
            vec!["ui".to_string()],
            vec!["playwright".to_string()],
            vec![project.to_string()],
            vec![project.to_string()],
            Some(project),
            None,
            None,
            None,
            Some("pty_native_adapter"),
            None,
            Some("F2.1"),
            Some("build"),
            None,
            Some("lead_then_human"),
            Some("planned"),
        ))
        .unwrap();
        let session_id = session["session_id"].as_str().unwrap();

        let initial: Value = serde_json::from_str(&claim_session_launch(
            project,
            Some("demo"),
            session_id,
            Some("scheduler-a"),
            Some("run-123"),
        ))
        .unwrap();
        assert_eq!(initial["action"], "session_launch_claimed");

        let replayed: Value = serde_json::from_str(&claim_session_launch(
            project,
            Some("demo"),
            session_id,
            Some("scheduler-a"),
            Some("run-123"),
        ))
        .unwrap();
        assert_eq!(replayed["action"], "session_launch_claim_existing");
        assert_eq!(replayed["claim_id"], "run-123");
        assert_eq!(replayed["session"]["launch_claim_id"], "run-123");
    }

    #[test]
    fn scheduler_run_results_are_replayed_by_run_id() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let stored = remember_scheduler_run_result(
            project,
            Some("demo"),
            "supervisor-a",
            "tick-001",
            json!({
                "project": "demo",
                "project_path": project,
                "actor": "supervisor-a",
                "run_id": "tick-001",
                "action": "no_ready_launch",
                "outcome": "ok",
            }),
        );
        assert_eq!(stored["run_id"], "tick-001");

        let replayed = scheduler_run_replay(project, Some("demo"), "tick-001").unwrap();
        assert_eq!(replayed["action"], "no_ready_launch");
        assert_eq!(replayed["actor"], "supervisor-a");

        let scheduler: Value =
            serde_json::from_str(&scheduler_snapshot(project, Some("demo"))).unwrap();
        assert_eq!(
            scheduler["scheduler"]["recent_runs"][0]["run_id"],
            "tick-001"
        );
    }

    #[test]
    fn runtime_launch_context_includes_adoption_work_package() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();

        let started: Value = serde_json::from_str(&start_project_adoption(
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

        let lead_session_id = started["lead_session_id"].as_str().unwrap();
        let context = runtime_launch_context(project, Some("demo"), lead_session_id);

        assert_eq!(context["session"]["id"], started["lead_session_id"]);
        assert_eq!(
            context["primary_work_order"]["id"],
            started["work_order_id"]
        );
        assert_eq!(context["primary_work_order"]["status"], "assigned");
        assert_eq!(context["adoption"]["id"], started["adoption_id"]);
        assert_eq!(context["debate"]["id"], started["debate_id"]);
    }

    #[test]
    fn operator_policy_authorizes_scoped_actions() {
        let profiles = vec![ControlOperatorProfile {
            id: "ops-lead".to_string(),
            role: "lead".to_string(),
            project_scopes: vec!["demo".to_string()],
            company_scopes: vec!["*".to_string()],
            program_scopes: vec!["*".to_string()],
            workspace_scopes: vec!["*".to_string()],
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
            company_scopes: vec!["*".to_string()],
            program_scopes: vec!["*".to_string()],
            workspace_scopes: vec!["*".to_string()],
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

    #[test]
    fn operator_policy_can_scope_by_company_and_program() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        std::fs::create_dir_all(&project_path).unwrap();
        let project = project_path.to_str().unwrap();
        let _ = upsert_project_identity(
            project,
            Some("demo"),
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            Some("core-platform"),
        );

        let profiles = vec![ControlOperatorProfile {
            id: "ops-lead".to_string(),
            role: "lead".to_string(),
            project_scopes: vec!["*".to_string()],
            company_scopes: vec!["DX Ventures".to_string()],
            program_scopes: vec!["Agentic Delivery".to_string()],
            workspace_scopes: vec!["core-*".to_string()],
            allowed_actions: vec!["session_*".to_string()],
            note: None,
        }];
        let decision = authorize_operator_action_with_profiles(
            &profiles,
            project,
            Some("demo"),
            "ops-lead",
            "session_launch",
        )
        .unwrap();
        assert_eq!(decision["company_scopes"][0], "DX Ventures");

        let denied_profiles = vec![ControlOperatorProfile {
            id: "ops-lead".to_string(),
            role: "lead".to_string(),
            project_scopes: vec!["*".to_string()],
            company_scopes: vec!["Another Co".to_string()],
            program_scopes: vec!["*".to_string()],
            workspace_scopes: vec!["*".to_string()],
            allowed_actions: vec!["session_*".to_string()],
            note: None,
        }];
        let denied = authorize_operator_action_with_profiles(
            &denied_profiles,
            project,
            Some("demo"),
            "ops-lead",
            "session_launch",
        )
        .unwrap_err();
        assert!(denied.contains("cannot control company"));
    }

    #[test]
    fn operator_policy_can_authorize_portfolio_scope_reads() {
        let profiles = vec![ControlOperatorProfile {
            id: "observer-1".to_string(),
            role: "observer".to_string(),
            project_scopes: vec!["*".to_string()],
            company_scopes: vec!["DX Ventures".to_string()],
            program_scopes: vec!["Agentic Delivery".to_string()],
            workspace_scopes: vec!["*".to_string()],
            allowed_actions: vec!["portfolio_read".to_string()],
            note: None,
        }];
        let decision = authorize_operator_scope_read_with_profiles(
            &profiles,
            "observer-1",
            "portfolio_read",
            Some("DX Ventures"),
            Some("Agentic Delivery"),
            None,
        )
        .unwrap();
        assert_eq!(decision["role"], "observer");

        let denied = authorize_operator_scope_read_with_profiles(
            &profiles,
            "observer-1",
            "portfolio_read",
            Some("Another Co"),
            None,
            None,
        )
        .unwrap_err();
        assert!(denied.contains("cannot read company scope"));
    }

    #[test]
    fn workflow_run_start_and_step_updates_roundtrip() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        let project = project_path.to_str().unwrap();
        std::fs::create_dir_all(project_path.join(".claude").join("commands")).unwrap();
        std::fs::write(
            project_path
                .join(".claude")
                .join("commands")
                .join("design-review.md"),
            "# Design Review\n- Capture the goal\n- Compare options\n- Publish the decision",
        )
        .unwrap();

        let started = start_workflow_run(
            project,
            Some("demo"),
            "project:command:design-review",
            Some("operator"),
            None,
            None,
            Some("F1.1"),
            Some("design"),
            Some("design"),
            Some("claude"),
            Some("claude-opus-4.6"),
        );
        let started_value: Value = serde_json::from_str(&started).unwrap();
        assert_eq!(started_value["action"], "workflow_run_started");
        let workflow_run_id = started_value["workflow_run_id"].as_str().unwrap();
        let step_id = started_value["workflow_run"]["steps"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(started_value["session_id"]
            .as_str()
            .unwrap()
            .starts_with("SX"));
        assert!(started_value["work_order_id"]
            .as_str()
            .unwrap()
            .starts_with("WO"));

        let progressed = update_workflow_run_step(
            project,
            Some("demo"),
            workflow_run_id,
            &step_id,
            "in_progress",
            Some("Collecting references now."),
        );
        let progressed_value: Value = serde_json::from_str(&progressed).unwrap();
        assert_eq!(progressed_value["workflow_run"]["status"], "active");
        assert_eq!(progressed_value["session"]["status"], "active");
        assert_eq!(progressed_value["work_order"]["status"], "assigned");

        for step in started_value["workflow_run"]["steps"]
            .as_array()
            .unwrap()
            .iter()
        {
            let _ = update_workflow_run_step(
                project,
                Some("demo"),
                workflow_run_id,
                step["id"].as_str().unwrap(),
                "completed",
                None,
            );
        }

        let listed = workflow_run_list(project, Some("demo"));
        let listed_value: Value = serde_json::from_str(&listed).unwrap();
        assert_eq!(listed_value["workflow_runs"][0]["status"], "completed");
        assert_eq!(listed_value["workflow_runs"][0]["completed_steps"], 3);
    }

    #[test]
    fn workflow_run_auto_reconciles_from_session_and_work_order_state() {
        let tmp = tempdir().unwrap();
        let project_path = tmp.path().join("demo");
        let project = project_path.to_str().unwrap();
        std::fs::create_dir_all(project_path.join(".claude").join("commands")).unwrap();
        std::fs::write(
            project_path
                .join(".claude")
                .join("commands")
                .join("design-review.md"),
            "# Design Review\n- Capture the goal\n- Compare options\n- Publish the decision",
        )
        .unwrap();

        let started = start_workflow_run(
            project,
            Some("demo"),
            "project:command:design-review",
            Some("operator"),
            None,
            None,
            Some("F1.1"),
            Some("design"),
            Some("design"),
            Some("claude"),
            Some("claude-opus-4.6"),
        );
        let started_value: Value = serde_json::from_str(&started).unwrap();
        let session_id = started_value["session_id"].as_str().unwrap().to_string();
        let work_order_id = started_value["work_order_id"].as_str().unwrap().to_string();
        let workflow_run_id = started_value["workflow_run_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(started_value["workflow_run"]["status"], "planned");

        let activated = update_session_status(
            project,
            Some("demo"),
            &session_id,
            "active",
            Some("Lane launched."),
        );
        let activated_value: Value = serde_json::from_str(&activated).unwrap();
        assert_eq!(
            activated_value["workflow_action"],
            "workflow_run_auto_activated"
        );
        assert_eq!(activated_value["workflow_run"]["status"], "active");
        assert_eq!(
            activated_value["workflow_run"]["steps"][0]["status"],
            "in_progress"
        );

        let blocked = raise_session_blocker(
            project,
            Some("demo"),
            &session_id,
            "Need approval to publish",
            Some("publish_post"),
            Some("Ask lead for approval"),
        );
        let blocked_value: Value = serde_json::from_str(&blocked).unwrap();
        assert_eq!(
            blocked_value["workflow_action"],
            "workflow_run_auto_blocked"
        );
        assert_eq!(blocked_value["workflow_run"]["status"], "blocked");
        assert_eq!(
            blocked_value["workflow_run"]["steps"][0]["status"],
            "blocked"
        );

        let resolved = resolve_work_order(
            project,
            Some("demo"),
            &work_order_id,
            Some("Approved by lead."),
        );
        let resolved_value: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(
            resolved_value["workflow_action"],
            "workflow_run_auto_resumed"
        );
        assert_eq!(resolved_value["workflow_run"]["status"], "active");
        assert_eq!(
            resolved_value["workflow_run"]["steps"][0]["status"],
            "in_progress"
        );

        let completed = update_session_status(
            project,
            Some("demo"),
            &session_id,
            "completed",
            Some("Workflow finished."),
        );
        let completed_value: Value = serde_json::from_str(&completed).unwrap();
        assert_eq!(
            completed_value["workflow_action"],
            "workflow_run_auto_completed"
        );
        assert_eq!(completed_value["workflow_run"]["status"], "completed");
        assert!(completed_value["workflow_run"]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|step| step["status"] == "completed"));

        let listed = workflow_run_list(project, Some("demo"));
        let listed_value: Value = serde_json::from_str(&listed).unwrap();
        let run = listed_value["workflow_runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["id"] == workflow_run_id)
            .unwrap();
        assert_eq!(run["status"], "completed");
        assert_eq!(run["completed_steps"], 3);
    }
}
