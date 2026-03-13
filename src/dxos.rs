use crate::state::events::StateEvent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
            runtime_adapter: "tmux_migration_adapter".to_string(),
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
    pub tmux_target: Option<String>,
    #[serde(default)]
    pub feature_id: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub supervisor_session_id: Option<String>,
    #[serde(default)]
    pub escalation_policy: Option<String>,
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneState {
    pub version: u32,
    pub project: ProjectDescriptor,
    pub defaults: ControlPlaneDefaults,
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

fn default_state(project_path: &str, project_name: Option<&str>) -> ControlPlaneState {
    ControlPlaneState {
        version: 1,
        project: ProjectDescriptor {
            name: resolved_project_name(project_path, project_name),
            path: project_path.to_string(),
        },
        defaults: ControlPlaneDefaults::default(),
        debates: Vec::new(),
        sessions: Vec::new(),
        work_orders: Vec::new(),
        updated_at: crate::state::now(),
    }
}

pub fn load_control_plane(project_path: &str, project_name: Option<&str>) -> ControlPlaneState {
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
                return state;
            }
        }
    }
    default_state(project_path, project_name)
}

fn save_control_plane(project_path: &str, state: &ControlPlaneState) -> Result<(), String> {
    let dir = control_plane_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {}", e))?;
    std::fs::write(control_plane_file(project_path), json).map_err(|e| format!("write: {}", e))
}

fn next_debate_id(state: &ControlPlaneState) -> String {
    format!("DB{:04}", state.debates.len() + 1)
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

fn session_summary(session: &SessionContractRecord) -> Value {
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
        "tmux_target": session.tmux_target,
        "feature_id": session.feature_id,
        "stage": session.stage,
        "supervisor_session_id": session.supervisor_session_id,
        "escalation_policy": session.escalation_policy,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
    })
}

fn work_order_summary(work_order: &WorkOrderRecord) -> Value {
    json!({
        "id": work_order.id,
        "supervisor_session_id": work_order.supervisor_session_id,
        "worker_session_id": work_order.worker_session_id,
        "status": work_order.status,
        "title": work_order.title,
        "objective": work_order.objective,
        "feature_id": work_order.feature_id,
        "stage": work_order.stage,
        "required_capabilities": work_order.required_capabilities,
        "blockers": work_order.blockers,
        "requested_permissions": work_order.requested_permissions,
        "expected_outputs": work_order.expected_outputs,
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
        .filter(|session| matches!(session.status.as_str(), "planned" | "active" | "blocked"))
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

    json!({
        "project": state.project,
        "defaults": state.defaults,
        "debates": {
            "total": state.debates.len(),
            "open": open_debates,
            "decided": decided_debates,
            "recent": recent,
        },
        "registry": {
            "capability_source": "dx_registry",
            "mcp_count": registry.len(),
            "category_counts": categories,
        },
        "runtime_contract": {
            "runtime_substrate": "custom_pty_target",
            "runtime_adapter": "tmux_migration_adapter",
            "browser_port_base": crate::config::browser_port_base(),
            "browser_port_formula": "browser_port_base + pane",
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
        "sessions": state.sessions.iter().map(session_summary).collect::<Vec<_>>(),
        "work_orders": state.work_orders.iter().map(work_order_summary).collect::<Vec<_>>(),
    })
    .to_string()
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

    let action = if let Some(existing) = state.sessions.iter_mut().find(|item| item.id == chosen_id) {
        existing.status = status.unwrap_or("active").trim().to_string();
        existing.role = role.trim().to_string();
        existing.provider = provider
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
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
        existing.updated_at = now.clone();
        "session_updated"
    } else {
        state.sessions.push(SessionContractRecord {
            id: chosen_id.clone(),
            status: status.unwrap_or("active").trim().to_string(),
            role: role.trim().to_string(),
            provider: provider
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
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
    if let Some(note) = note.filter(|value| !value.trim().is_empty()) {
        session.objective = format!("{}\n\nStatus note: {}", session.objective.trim(), note.trim());
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
    if supervisor_session_id.trim().is_empty() || title.trim().is_empty() || objective.trim().is_empty()
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
        if !state.sessions.iter().any(|session| session.id == worker.trim()) {
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
    if !work_order.blockers.iter().any(|item| item == blocker.trim()) {
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
        work_order.expected_outputs.push(format!("Resolution: {}", resolution.trim()));
    }
    work_order.updated_at = crate::state::now();
    state.updated_at = work_order.updated_at.clone();

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

        let resolved = resolve_work_order(
            project,
            Some("demo"),
            work_order_id,
            Some("Permission granted by lead"),
        );
        let resolved_value: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(resolved_value["work_order"]["status"], "assigned");

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
}
