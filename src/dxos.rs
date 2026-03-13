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
}
