//! Vision Framework — Living product vision with GitHub sync and change tracking.
//!
//! Each project gets a `.vision/vision.json` that tracks:
//! - Mission, goals, architecture decisions
//! - Milestones and their status
//! - Vision change history (what changed, why, when)
//! - GitHub issue/PR links for traceability

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ─── Data Model ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vision {
    pub project: String,
    pub mission: String,
    pub principles: Vec<String>,
    pub goals: Vec<Goal>,
    pub milestones: Vec<Milestone>,
    pub architecture: Vec<ArchDecision>,
    pub changes: Vec<VisionChange>,
    #[serde(default)]
    pub features: Vec<Feature>,
    #[serde(default)]
    pub github: GitHubConfig,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub priority: u8, // 1=critical, 2=high, 3=medium
    #[serde(default)]
    pub linked_issues: Vec<String>, // GitHub issue numbers
    #[serde(default)]
    pub metrics: Vec<String>, // success metrics
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Planned,
    InProgress,
    Achieved,
    Deferred,
    Dropped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: MilestoneStatus,
    pub target_date: Option<String>,
    pub goals: Vec<String>, // goal IDs
    #[serde(default)]
    pub github_milestone: Option<u64>, // GitHub milestone number
    #[serde(default)]
    pub progress_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Upcoming,
    Active,
    InProgress,
    Complete,
    Missed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchDecision {
    pub id: String,
    pub title: String,
    pub decision: String,
    pub rationale: String,
    pub date: String,
    #[serde(default)]
    pub alternatives_considered: Vec<String>,
    #[serde(default)]
    pub linked_pr: Option<String>,
    pub status: ArchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArchStatus {
    Active,
    Superseded,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionChange {
    pub timestamp: String,
    pub change_type: ChangeType,
    pub field: String, // what changed: "mission", "goal:G1", "milestone:M2", etc.
    pub old_value: String,
    pub new_value: String,
    pub reason: String,
    pub triggered_by: String, // "user request", "task completion", "pivot"
    #[serde(default)]
    pub github_issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
    StatusChange,
    Pivot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubConfig {
    pub repo: String, // "owner/repo"
    pub sync_enabled: bool,
    pub wiki_page: Option<String>,
    pub project_board: Option<u64>,
    #[serde(default)]
    pub labels: Vec<String>, // labels to apply to vision-related issues
}

// ─── VDD: Feature/Question/Decision/Task ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,      // "F1.1"
    pub goal_id: String, // links to parent goal
    pub title: String,
    pub description: String,
    pub status: FeatureStatus,
    #[serde(default)]
    pub phase: FeaturePhase,
    #[serde(default)]
    pub state: FeatureState,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub decisions: Vec<VisionDecision>,
    #[serde(default)]
    pub tasks: Vec<VisionTask>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub acceptance_items: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub design_options: Vec<DesignOption>,
    #[serde(default)]
    pub sub_vision: Option<String>, // path to sub-vision file (recursive)
    #[serde(default)]
    pub parent_vision: Option<String>, // link up the tree
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    Planned,
    Specifying,
    Building,
    Testing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeaturePhase {
    Planned,
    Discovery,
    Build,
    Test,
    Done,
}

impl Default for FeaturePhase {
    fn default() -> Self {
        Self::Planned
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    Planned,
    Active,
    Blocked,
    Complete,
}

impl Default for FeatureState {
    fn default() -> Self {
        Self::Planned
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub status: AcceptanceStatus,
    #[serde(default)]
    pub verification_method: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub verified_at: Option<String>,
    #[serde(default)]
    pub verified_by: Option<String>,
    #[serde(default)]
    pub verification_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStatus {
    Draft,
    Mapped,
    Verified,
    Failed,
}

impl Default for AcceptanceStatus {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignOption {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: DesignOptionStatus,
    #[serde(default)]
    pub relative_path: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub approved_by: Option<String>,
    #[serde(default)]
    pub approved_at: Option<String>,
    #[serde(default)]
    pub review_notes: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DesignOptionStatus {
    Draft,
    Proposed,
    Approved,
    Rejected,
}

impl Default for DesignOptionStatus {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String, // "Q1.1.1"
    pub text: String,
    pub status: QuestionStatus,
    #[serde(default = "default_true")]
    pub blocking: bool,
    #[serde(default)]
    pub answer: Option<String>,
    pub asked_at: String,
    #[serde(default)]
    pub answered_at: Option<String>,
    #[serde(default)]
    pub decision_id: Option<String>, // links to the decision it produced
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Open,
    Answered,
    Revised,
}

/// Named VisionDecision to avoid conflict with ArchDecision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionDecision {
    pub id: String, // "D1.1.1"
    #[serde(default)]
    pub question_id: Option<String>, // which question this answers
    pub decision: String,
    pub rationale: String,
    pub date: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionTask {
    pub id: String, // "T1.1.1"
    pub feature_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub branch: Option<String>, // Git branch name
    #[serde(default)]
    pub pr: Option<String>, // PR number/URL
    #[serde(default)]
    pub commit: Option<String>, // merge commit
    #[serde(default)]
    pub assignee: Option<String>, // pane ID or agent name
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Planned,
    InProgress,
    Done,
    Verified,
    Blocked,
}

fn default_true() -> bool {
    true
}

fn feature_phase_from_status(status: &FeatureStatus) -> FeaturePhase {
    match status {
        FeatureStatus::Planned => FeaturePhase::Planned,
        FeatureStatus::Specifying => FeaturePhase::Discovery,
        FeatureStatus::Building => FeaturePhase::Build,
        FeatureStatus::Testing => FeaturePhase::Test,
        FeatureStatus::Done => FeaturePhase::Done,
    }
}

fn feature_state_from_status(status: &FeatureStatus) -> FeatureState {
    match status {
        FeatureStatus::Planned => FeatureState::Planned,
        FeatureStatus::Done => FeatureState::Complete,
        FeatureStatus::Specifying | FeatureStatus::Building | FeatureStatus::Testing => {
            FeatureState::Active
        }
    }
}

fn feature_status_from_phase(phase: &FeaturePhase) -> FeatureStatus {
    match phase {
        FeaturePhase::Planned => FeatureStatus::Planned,
        FeaturePhase::Discovery => FeatureStatus::Specifying,
        FeaturePhase::Build => FeatureStatus::Building,
        FeaturePhase::Test => FeatureStatus::Testing,
        FeaturePhase::Done => FeatureStatus::Done,
    }
}

fn acceptance_id(feature_id: &str, idx: usize) -> String {
    format!("AC{}.{}", feature_id.trim_start_matches('F'), idx + 1)
}

fn design_option_id(feature_id: &str, idx: usize) -> String {
    format!("MO{}.{}", feature_id.trim_start_matches('F'), idx + 1)
}

fn slugify_fragment(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_dash {
            last_dash = true;
            Some('-')
        } else {
            None
        };
        if let Some(next) = next {
            slug.push(next);
        }
    }
    slug.trim_matches('-').to_string()
}

fn feature_requires_design(feature: &Feature) -> bool {
    if !feature.design_options.is_empty() {
        return true;
    }

    let haystack = format!(
        "{} {}",
        feature.title.to_lowercase(),
        feature.description.to_lowercase()
    );
    [
        "ui",
        "ux",
        "frontend",
        "front-end",
        "website",
        "landing",
        "portal",
        "dashboard",
        "client",
        "customer",
        "shopify",
        "design",
        "brand",
        "onboarding",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn approved_design_count(feature: &Feature) -> usize {
    feature
        .design_options
        .iter()
        .filter(|option| option.status == DesignOptionStatus::Approved)
        .count()
}

fn proposed_design_count(feature: &Feature) -> usize {
    feature
        .design_options
        .iter()
        .filter(|option| {
            matches!(
                option.status,
                DesignOptionStatus::Draft
                    | DesignOptionStatus::Proposed
                    | DesignOptionStatus::Approved
            )
        })
        .count()
}

fn sync_acceptance_items(feature: &mut Feature) {
    if feature.acceptance_items.is_empty() && !feature.acceptance_criteria.is_empty() {
        feature.acceptance_items = feature
            .acceptance_criteria
            .iter()
            .enumerate()
            .map(|(idx, text)| AcceptanceCriterion {
                id: acceptance_id(&feature.id, idx),
                text: text.clone(),
                status: AcceptanceStatus::Draft,
                verification_method: None,
                evidence: vec![],
                verified_at: None,
                verified_by: None,
                verification_source: None,
            })
            .collect();
    }

    if !feature.acceptance_items.is_empty() {
        feature.acceptance_criteria = feature
            .acceptance_items
            .iter()
            .map(|item| item.text.clone())
            .collect();
    }
}

fn normalize_feature(feature: &mut Feature) {
    sync_acceptance_items(feature);

    let phase_was_default = feature.phase == FeaturePhase::Planned;
    let state_was_default = feature.state == FeatureState::Planned;

    if phase_was_default && state_was_default {
        feature.phase = feature_phase_from_status(&feature.status);
        feature.state = feature_state_from_status(&feature.status);
        return;
    }

    if feature.phase == FeaturePhase::Planned && feature.state != FeatureState::Planned {
        feature.state = FeatureState::Planned;
    } else if feature.phase == FeaturePhase::Done && feature.state == FeatureState::Planned {
        feature.state = FeatureState::Complete;
    } else if feature.phase != FeaturePhase::Planned
        && feature.phase != FeaturePhase::Done
        && feature.state == FeatureState::Planned
    {
        feature.state = FeatureState::Active;
    }

    feature.status = feature_status_from_phase(&feature.phase);
}

fn set_feature_lifecycle(feature: &mut Feature, phase: FeaturePhase, state: FeatureState) {
    feature.phase = phase.clone();
    feature.state = state;
    feature.status = feature_status_from_phase(&phase);
}

fn task_counts(feature: &Feature) -> (usize, usize, usize) {
    let total = feature.tasks.len();
    let complete = feature
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified)
        .count();
    let verified = feature
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Verified)
        .count();
    (total, complete, verified)
}

fn feature_doc_exists(project_path: &str, feature_id: &str, doc_type: &str) -> bool {
    feature_doc_path(project_path, doc_type, feature_id)
        .map(|(path, _)| path.exists())
        .unwrap_or(false)
}

fn feature_readiness_value(project_path: &str, feature: &Feature) -> serde_json::Value {
    let open_questions = feature
        .questions
        .iter()
        .filter(|q| q.status == QuestionStatus::Open)
        .count();
    let blocking_open_questions = feature
        .questions
        .iter()
        .filter(|q| q.status == QuestionStatus::Open && q.blocking)
        .count();
    let non_blocking_open_questions = open_questions.saturating_sub(blocking_open_questions);
    let has_research_doc = feature_doc_exists(project_path, &feature.id, "research");
    let has_discovery_doc = feature_doc_exists(project_path, &feature.id, "discovery");
    let has_design_doc = feature_doc_exists(project_path, &feature.id, "design");
    let design_required = has_design_doc || feature_requires_design(feature);
    let design_option_count = proposed_design_count(feature);
    let approved_design_options = approved_design_count(feature);
    let has_discovery_artifact =
        has_research_doc || has_discovery_doc || has_design_doc || design_option_count > 0;
    let acceptance_count = feature.acceptance_items.len();
    let acceptance_verified = feature
        .acceptance_items
        .iter()
        .filter(|item| item.status == AcceptanceStatus::Verified)
        .count();
    let acceptance_failed = feature
        .acceptance_items
        .iter()
        .filter(|item| item.status == AcceptanceStatus::Failed)
        .count();
    let (task_total, task_complete, task_verified) = task_counts(feature);

    let mut build_blockers = Vec::new();
    if !has_discovery_artifact {
        build_blockers.push("discovery artifact missing".to_string());
    }
    if blocking_open_questions > 0 {
        build_blockers.push(format!(
            "{} blocking discovery question(s)",
            blocking_open_questions
        ));
    }
    if acceptance_count == 0 {
        build_blockers.push("acceptance criteria missing".to_string());
    }
    if design_required && !has_design_doc {
        build_blockers.push("design brief missing".to_string());
    }
    if design_required && design_option_count == 0 {
        build_blockers.push("client mockup missing".to_string());
    }
    if design_required && approved_design_options == 0 {
        build_blockers.push("client approval missing".to_string());
    }

    let mut test_blockers = Vec::new();
    if task_total == 0 {
        test_blockers.push("build tasks missing".to_string());
    } else if task_complete < task_total {
        test_blockers.push(format!(
            "{} build task(s) still incomplete",
            task_total - task_complete
        ));
    }

    let mut done_blockers = test_blockers.clone();
    if task_total > 0 && task_verified < task_total {
        done_blockers.push(format!(
            "{} build task(s) not yet verified",
            task_total - task_verified
        ));
    }
    if acceptance_count == 0 {
        done_blockers.push("acceptance criteria missing".to_string());
    } else if acceptance_verified < acceptance_count {
        done_blockers.push(format!(
            "{} acceptance criterion/criteria not verified",
            acceptance_count - acceptance_verified
        ));
    }

    let mut blockers = build_blockers.clone();
    for blocker in test_blockers.iter().chain(done_blockers.iter()) {
        if !blockers.contains(blocker) {
            blockers.push(blocker.clone());
        }
    }

    serde_json::json!({
        "ready_for_build": build_blockers.is_empty(),
        "ready_for_test": test_blockers.is_empty(),
        "ready_for_done": done_blockers.is_empty(),
        "blockers": {
            "build": build_blockers,
            "test": test_blockers,
            "done": done_blockers,
        },
        "counts": {
            "open_questions": open_questions,
            "blocking_open_questions": blocking_open_questions,
            "non_blocking_open_questions": non_blocking_open_questions,
            "acceptance_criteria": acceptance_count,
            "acceptance_verified": acceptance_verified,
            "acceptance_failed": acceptance_failed,
            "tasks_total": task_total,
            "tasks_complete": task_complete,
            "tasks_verified": task_verified,
            "has_research_doc": has_research_doc,
            "has_discovery_doc": has_discovery_doc,
            "has_design_doc": has_design_doc,
            "has_discovery_artifact": has_discovery_artifact,
            "design_required": design_required,
            "design_options": design_option_count,
            "design_approved": approved_design_options,
        },
        "discovery": {
            "has_research_doc": has_research_doc,
            "has_discovery_doc": has_discovery_doc,
            "has_design_doc": has_design_doc,
            "design_required": design_required,
            "design_options": design_option_count,
            "design_approved": approved_design_options,
            "blocking_open_questions": blocking_open_questions,
            "non_blocking_open_questions": non_blocking_open_questions,
            "acceptance_criteria": acceptance_count,
            "acceptance_verified": acceptance_verified,
            "acceptance_failed": acceptance_failed,
        }
    })
}

fn reconcile_feature_lifecycle(project_path: &str, feature: &mut Feature) {
    sync_acceptance_items(feature);
    let readiness = feature_readiness_value(project_path, feature);
    let ready_for_build = readiness["ready_for_build"].as_bool().unwrap_or(false);
    let ready_for_test = readiness["ready_for_test"].as_bool().unwrap_or(false);
    let ready_for_done = readiness["ready_for_done"].as_bool().unwrap_or(false);

    if ready_for_done && feature.phase != FeaturePhase::Done {
        set_feature_lifecycle(feature, FeaturePhase::Done, FeatureState::Complete);
        return;
    }

    if ready_for_test && matches!(feature.phase, FeaturePhase::Build | FeaturePhase::Test) {
        set_feature_lifecycle(feature, FeaturePhase::Test, FeatureState::Active);
        return;
    }

    if ready_for_build
        && matches!(
            feature.phase,
            FeaturePhase::Planned | FeaturePhase::Discovery
        )
    {
        set_feature_lifecycle(feature, FeaturePhase::Build, FeatureState::Active);
    }
}

// ─── Storage ────────────────────────────────────────────────────────────────

fn vision_dir(project_path: &str) -> PathBuf {
    Path::new(project_path).join(".vision")
}

fn vision_file(project_path: &str) -> PathBuf {
    vision_dir(project_path).join("vision.json")
}

fn history_file(project_path: &str) -> PathBuf {
    vision_dir(project_path).join("history.jsonl")
}

pub fn load_vision(project_path: &str) -> Option<Vision> {
    let path = vision_file(project_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("vision: cannot read {}: {}", path.display(), e);
            return None;
        }
    };
    match serde_json::from_str::<Vision>(&content) {
        Ok(mut v) => {
            for feature in &mut v.features {
                normalize_feature(feature);
            }
            Some(v)
        }
        Err(e) => {
            tracing::warn!("vision: parse error for {}: {}", path.display(), e);
            None
        }
    }
}

pub fn save_vision(project_path: &str, vision: &Vision) -> Result<(), String> {
    let dir = vision_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;

    let mut normalized = vision.clone();
    for feature in &mut normalized.features {
        normalize_feature(feature);
    }

    let json =
        serde_json::to_string_pretty(&normalized).map_err(|e| format!("serialize: {}", e))?;
    std::fs::write(vision_file(project_path), json).map_err(|e| format!("write: {}", e))?;

    // Also write .gitignore to NOT ignore vision
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        let _ = std::fs::write(&gitignore, "# Vision files are tracked in git\n!*\n");
    }

    Ok(())
}

/// Append a change to the history JSONL for auditing.
fn append_history(project_path: &str, change: &VisionChange) {
    let path = history_file(project_path);
    let _ = std::fs::create_dir_all(vision_dir(project_path));
    if let Ok(json) = serde_json::to_string(change) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{}", json);
        }
    }
}

// ─── Operations ─────────────────────────────────────────────────────────────

pub fn init_vision(project_path: &str, project_name: &str, mission: &str, repo: &str) -> String {
    if load_vision(project_path).is_some() {
        return serde_json::json!({
            "status": "exists",
            "message": "Vision already exists. Use vision_update to modify."
        })
        .to_string();
    }

    let vision = Vision {
        project: project_name.to_string(),
        mission: mission.to_string(),
        principles: vec![],
        goals: vec![],
        milestones: vec![],
        architecture: vec![],
        changes: vec![],
        features: vec![],
        github: GitHubConfig {
            repo: repo.to_string(),
            sync_enabled: !repo.is_empty(),
            wiki_page: None,
            project_board: None,
            labels: vec!["vision".to_string()],
        },
        updated_at: now(),
    };

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "created",
            "path": vision_file(project_path).display().to_string(),
            "project": project_name,
            "mission": mission,
            "github_repo": repo,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn get_vision(project_path: &str) -> String {
    match load_vision(project_path) {
        Some(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string()),
        None => serde_json::json!({
            "error": "no_vision",
            "hint": "Run vision_init to create a vision for this project"
        })
        .to_string(),
    }
}

pub fn add_goal(
    project_path: &str,
    id: &str,
    title: &str,
    description: &str,
    priority: u8,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    if vision.goals.iter().any(|g| g.id == id) {
        return serde_json::json!({"error": "goal_exists", "id": id}).to_string();
    }

    let goal = Goal {
        id: id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        status: GoalStatus::Planned,
        priority,
        linked_issues: vec![],
        metrics: vec![],
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("goal:{}", id),
        old_value: String::new(),
        new_value: title.to_string(),
        reason: "New goal added".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    vision.goals.push(goal);
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({"status": "added", "goal": id}).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn add_milestone(
    project_path: &str,
    id: &str,
    title: &str,
    description: &str,
    target_date: Option<&str>,
    goal_ids: Vec<String>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    if vision.milestones.iter().any(|m| m.id == id) {
        return serde_json::json!({"error": "milestone_exists", "id": id}).to_string();
    }

    let ms = Milestone {
        id: id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        status: MilestoneStatus::Upcoming,
        target_date: target_date.map(|s| s.to_string()),
        goals: goal_ids,
        github_milestone: None,
        progress_pct: 0,
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("milestone:{}", id),
        old_value: String::new(),
        new_value: title.to_string(),
        reason: "New milestone added".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    vision.milestones.push(ms);
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({"status": "added", "milestone": id}).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn add_arch_decision(
    project_path: &str,
    id: &str,
    title: &str,
    decision: &str,
    rationale: &str,
    alternatives: Vec<String>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let ad = ArchDecision {
        id: id.to_string(),
        title: title.to_string(),
        decision: decision.to_string(),
        rationale: rationale.to_string(),
        date: now(),
        alternatives_considered: alternatives,
        linked_pr: None,
        status: ArchStatus::Active,
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("arch:{}", id),
        old_value: String::new(),
        new_value: format!("{}: {}", title, decision),
        reason: rationale.to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    vision.architecture.push(ad);
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({"status": "added", "decision": id}).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn update_goal_status(
    project_path: &str,
    goal_id: &str,
    new_status: &str,
    reason: &str,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let goal = match vision.goals.iter_mut().find(|g| g.id == goal_id) {
        Some(g) => g,
        None => return serde_json::json!({"error": "goal_not_found", "id": goal_id}).to_string(),
    };

    let old_status = serde_json::to_string(&goal.status).unwrap_or_default();
    let parsed: GoalStatus = match new_status {
        "planned" => GoalStatus::Planned,
        "in_progress" => GoalStatus::InProgress,
        "achieved" => GoalStatus::Achieved,
        "deferred" => GoalStatus::Deferred,
        "dropped" => GoalStatus::Dropped,
        _ => return serde_json::json!({"error": "invalid_status", "options": ["planned","in_progress","achieved","deferred","dropped"]}).to_string(),
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("goal:{}", goal_id),
        old_value: old_status,
        new_value: new_status.to_string(),
        reason: reason.to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    goal.status = parsed;
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => {
            serde_json::json!({"status": "updated", "goal": goal_id, "new_status": new_status})
                .to_string()
        }
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn update_mission(project_path: &str, new_mission: &str, reason: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: if reason.contains("pivot") {
            ChangeType::Pivot
        } else {
            ChangeType::Modified
        },
        field: "mission".to_string(),
        old_value: vision.mission.clone(),
        new_value: new_mission.to_string(),
        reason: reason.to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    vision.mission = new_mission.to_string();
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({"status": "updated", "field": "mission"}).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Get a summary suitable for the dashboard widget.
pub fn vision_summary(project_path: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let goals_by_status = |s: &GoalStatus| vision.goals.iter().filter(|g| &g.status == s).count();
    let active_milestones: Vec<_> = vision
        .milestones
        .iter()
        .filter(|m| m.status == MilestoneStatus::Active)
        .map(|m| {
            serde_json::json!({
                "id": m.id, "title": m.title, "progress": m.progress_pct,
                "target": m.target_date,
            })
        })
        .collect();

    let recent_changes: Vec<_> = vision
        .changes
        .iter()
        .rev()
        .take(5)
        .map(|c| {
            serde_json::json!({
                "time": c.timestamp, "field": c.field,
                "type": c.change_type, "reason": c.reason,
            })
        })
        .collect();

    serde_json::json!({
        "project": vision.project,
        "mission": vision.mission,
        "goals": {
            "total": vision.goals.len(),
            "planned": goals_by_status(&GoalStatus::Planned),
            "in_progress": goals_by_status(&GoalStatus::InProgress),
            "achieved": goals_by_status(&GoalStatus::Achieved),
            "deferred": goals_by_status(&GoalStatus::Deferred),
        },
        "milestones": {
            "total": vision.milestones.len(),
            "active": active_milestones,
        },
        "arch_decisions": vision.architecture.len(),
        "principles": vision.principles,
        "recent_changes": recent_changes,
        "github": {
            "repo": vision.github.repo,
            "sync": vision.github.sync_enabled,
        },
        "updated_at": vision.updated_at,
    })
    .to_string()
}

/// Get recent changes as a diff-style view.
pub fn vision_diff(project_path: &str, last_n: usize) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let changes: Vec<_> = vision.changes.iter().rev().take(last_n).collect();
    serde_json::json!({
        "project": vision.project,
        "change_count": changes.len(),
        "changes": changes,
    })
    .to_string()
}

// ─── GitHub Sync ────────────────────────────────────────────────────────────

/// Sync vision to GitHub: create/update issues for goals, milestones.
pub fn github_sync(project_path: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    if !vision.github.sync_enabled || vision.github.repo.is_empty() {
        return serde_json::json!({
            "error": "github_not_configured",
            "hint": "Set github.repo and github.sync_enabled in vision"
        })
        .to_string();
    }

    let repo = &vision.github.repo;
    let mut results = vec![];

    // Sync milestones
    for ms in &vision.milestones {
        if ms.github_milestone.is_none() {
            let due = ms.target_date.as_deref().unwrap_or("");
            let cmd = format!(
                "gh api repos/{}/milestones -f title='{}' -f description='{}' -f state=open {}",
                repo,
                ms.title.replace('\'', "'\\''"),
                ms.description.replace('\'', "'\\''"),
                if due.is_empty() {
                    String::new()
                } else {
                    format!("-f due_on='{}T00:00:00Z'", due)
                }
            );
            let output = run_gh(&cmd);
            results.push(serde_json::json!({
                "type": "milestone", "id": ms.id, "action": "create",
                "result": output.trim_end(),
            }));
        }
    }

    // Sync goals as issues
    for goal in &vision.goals {
        if goal.linked_issues.is_empty() && goal.status != GoalStatus::Dropped {
            let labels = vision.github.labels.join(",");
            let status_label = match goal.status {
                GoalStatus::Planned => "planned",
                GoalStatus::InProgress => "in-progress",
                GoalStatus::Achieved => "achieved",
                GoalStatus::Deferred => "deferred",
                GoalStatus::Dropped => "dropped",
            };
            let body = format!(
                "## Vision Goal: {}\n\n{}\n\n**Priority:** {}\n**Status:** {}\n\n---\n_Auto-synced from .vision/vision.json_",
                goal.title, goal.description, goal.priority, status_label
            );
            let cmd = format!(
                "gh issue create -R {} --title '[Vision] {}' --body '{}' --label '{},vision-goal'",
                repo,
                goal.title.replace('\'', "'\\''"),
                body.replace('\'', "'\\''"),
                labels,
            );
            let output = run_gh(&cmd);
            results.push(serde_json::json!({
                "type": "goal_issue", "id": goal.id, "action": "create",
                "result": output.trim_end(),
            }));
        }
    }

    // Create/update wiki page if configured
    if vision.github.wiki_page.is_some() {
        let wiki_md = generate_wiki_markdown(&vision);
        results.push(serde_json::json!({
            "type": "wiki", "action": "generate",
            "content_length": wiki_md.len(),
        }));
    }

    serde_json::json!({
        "status": "synced",
        "repo": repo,
        "actions": results,
    })
    .to_string()
}

fn generate_wiki_markdown(vision: &Vision) -> String {
    let mut md = format!("# {} — Product Vision\n\n", vision.project);
    md.push_str(&format!("## Mission\n\n{}\n\n", vision.mission));

    if !vision.principles.is_empty() {
        md.push_str("## Principles\n\n");
        for p in &vision.principles {
            md.push_str(&format!("- {}\n", p));
        }
        md.push('\n');
    }

    md.push_str("## Goals\n\n");
    md.push_str("| ID | Goal | Priority | Status |\n|---|---|---|---|\n");
    for g in &vision.goals {
        md.push_str(&format!(
            "| {} | {} | P{} | {:?} |\n",
            g.id, g.title, g.priority, g.status
        ));
    }

    md.push_str("\n## Milestones\n\n");
    for m in &vision.milestones {
        md.push_str(&format!(
            "### {} — {} ({:?})\n\n{}\n\nProgress: {}%\n\n",
            m.id, m.title, m.status, m.description, m.progress_pct
        ));
    }

    if !vision.architecture.is_empty() {
        md.push_str("## Architecture Decisions\n\n");
        for a in &vision.architecture {
            md.push_str(&format!(
                "### ADR-{}: {}\n\n**Decision:** {}\n\n**Rationale:** {}\n\n**Status:** {:?}\n\n",
                a.id, a.title, a.decision, a.rationale, a.status
            ));
        }
    }

    if !vision.changes.is_empty() {
        md.push_str("## Recent Changes\n\n");
        for c in vision.changes.iter().rev().take(10) {
            md.push_str(&format!(
                "- **{}** `{}` {:?}: {} → {} ({})\n",
                c.timestamp, c.field, c.change_type, c.old_value, c.new_value, c.reason
            ));
        }
    }

    md.push_str(&format!("\n---\n_Last updated: {}_\n", vision.updated_at));
    md
}

/// Get all visions across known projects.
pub fn list_visions() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/pran".to_string());
    let projects_dir = format!("{}/Projects", home);
    let mut visions = vec![];

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let vision_path = path.join(".vision/vision.json");
                if vision_path.exists() {
                    if let Some(v) = load_vision(path.to_str().unwrap_or("")) {
                        visions.push(serde_json::json!({
                            "project": v.project,
                            "mission": v.mission,
                            "goals": v.goals.len(),
                            "milestones": v.milestones.len(),
                            "path": path.display().to_string(),
                            "updated_at": v.updated_at,
                        }));
                    }
                }
            }
        }
    }

    serde_json::json!({
        "visions": visions,
        "count": visions.len(),
    })
    .to_string()
}

// ─── VDD: Feature CRUD ──────────────────────────────────────────────────────

fn features_dir(project_path: &str) -> PathBuf {
    vision_dir(project_path).join("features")
}

fn mockups_dir(project_path: &str, feature_id: &str) -> PathBuf {
    vision_dir(project_path).join("mockups").join(feature_id)
}

fn feature_doc_path(
    project_path: &str,
    doc_type: &str,
    feature_id: &str,
) -> Result<(PathBuf, String), String> {
    match doc_type {
        "research" | "discovery" | "design" => {
            let relative = format!(".vision/{}/{}.md", doc_type, feature_id);
            Ok((
                vision_dir(project_path).join(format!("{}/{}.md", doc_type, feature_id)),
                relative,
            ))
        }
        _ => Err(format!("invalid_doc_type: {}", doc_type)),
    }
}

/// Add a feature under a goal.
pub fn add_feature(
    project_path: &str,
    goal_id: &str,
    title: &str,
    description: &str,
    acceptance_criteria: Vec<String>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    // Verify goal exists
    if !vision.goals.iter().any(|g| g.id == goal_id) {
        return serde_json::json!({"error": "goal_not_found", "id": goal_id}).to_string();
    }

    // Generate feature ID: count existing features for this goal + 1
    let existing = vision
        .features
        .iter()
        .filter(|f| f.goal_id == goal_id)
        .count();
    let feature_num = existing + 1;
    let id = format!("F{}.{}", goal_id.trim_start_matches('G'), feature_num);
    let has_acceptance = !acceptance_criteria.is_empty();

    let feature = Feature {
        id: id.clone(),
        goal_id: goal_id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        status: if has_acceptance {
            FeatureStatus::Specifying
        } else {
            FeatureStatus::Planned
        },
        phase: if has_acceptance {
            FeaturePhase::Discovery
        } else {
            FeaturePhase::Planned
        },
        state: if has_acceptance {
            FeatureState::Active
        } else {
            FeatureState::Planned
        },
        questions: vec![],
        decisions: vec![],
        tasks: vec![],
        acceptance_items: acceptance_criteria
            .iter()
            .enumerate()
            .map(|(idx, text)| AcceptanceCriterion {
                id: acceptance_id(&id, idx),
                text: text.clone(),
                status: AcceptanceStatus::Draft,
                verification_method: None,
                evidence: vec![],
                verified_at: None,
                verified_by: None,
                verification_source: None,
            })
            .collect(),
        acceptance_criteria,
        design_options: vec![],
        sub_vision: None,
        parent_vision: None,
        created_at: now(),
        updated_at: now(),
    };

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("feature:{}", id),
        old_value: String::new(),
        new_value: title.to_string(),
        reason: format!("Feature added under goal {}", goal_id),
        triggered_by: "user".to_string(),
        github_issue: None,
    };

    vision.features.push(feature);
    vision.changes.push(change.clone());
    vision.updated_at = now();
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "feature": id,
            "goal": goal_id,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn upsert_feature_doc(
    project_path: &str,
    feature_id: &str,
    doc_type: &str,
    content: &str,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let (doc_path, relative_path) = match feature_doc_path(project_path, doc_type, feature_id) {
        Ok(paths) => paths,
        Err(e) => {
            return serde_json::json!({"error": e, "options": ["research", "discovery", "design"]})
                .to_string()
        }
    };

    if let Some(parent) = doc_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return serde_json::json!({"error": format!("mkdir: {}", e)}).to_string();
        }
    }

    let existed = doc_path.exists();
    if let Err(e) = std::fs::write(&doc_path, content) {
        return serde_json::json!({"error": format!("write: {}", e)}).to_string();
    }

    if feature.phase == FeaturePhase::Planned {
        set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let feature_phase = feature.phase.clone();
    let feature_state = feature.state.clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: if existed {
            ChangeType::Modified
        } else {
            ChangeType::Added
        },
        field: format!("{}_doc:{}", doc_type, feature_id),
        old_value: if existed {
            "existing".to_string()
        } else {
            String::new()
        },
        new_value: relative_path.clone(),
        reason: format!("{} doc upserted", doc_type),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": if existed { "updated" } else { "created" },
            "feature": feature_id,
            "doc_type": doc_type,
            "path": relative_path,
            "phase": feature_phase,
            "state": feature_state,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

fn dataxlr8_hex_logo_svg() -> &'static str {
    r#"<svg width="72" height="72" viewBox="0 0 200 200" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="dx-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#06B6D4;stop-opacity:1" />
      <stop offset="50%" style="stop-color:#A855F7;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#FF00AA;stop-opacity:1" />
    </linearGradient>
    <filter id="dx-glow">
      <feGaussianBlur stdDeviation="3" result="coloredBlur"/>
      <feMerge>
        <feMergeNode in="coloredBlur"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
    </filter>
  </defs>
  <polygon points="100,20 170,60 170,140 100,180 30,140 30,60" stroke="url(#dx-grad)" stroke-width="6" fill="none" filter="url(#dx-glow)"/>
  <circle cx="100" cy="100" r="25" fill="url(#dx-grad)"/>
  <circle cx="100" cy="40" r="8" fill="url(#dx-grad)"/>
  <circle cx="155" cy="70" r="8" fill="url(#dx-grad)"/>
  <circle cx="155" cy="130" r="8" fill="url(#dx-grad)"/>
  <circle cx="100" cy="160" r="8" fill="url(#dx-grad)"/>
  <circle cx="45" cy="130" r="8" fill="url(#dx-grad)"/>
  <circle cx="45" cy="70" r="8" fill="url(#dx-grad)"/>
  <line x1="100" y1="40" x2="100" y2="75" stroke="url(#dx-grad)" stroke-width="3"/>
  <line x1="155" y1="70" x2="120" y2="85" stroke="url(#dx-grad)" stroke-width="3"/>
  <line x1="155" y1="130" x2="120" y2="115" stroke="url(#dx-grad)" stroke-width="3"/>
  <line x1="100" y1="160" x2="100" y2="125" stroke="url(#dx-grad)" stroke-width="3"/>
  <line x1="45" y1="130" x2="80" y2="115" stroke="url(#dx-grad)" stroke-width="3"/>
  <line x1="45" y1="70" x2="80" y2="85" stroke="url(#dx-grad)" stroke-width="3"/>
  <path d="M90 95 L110 100 L90 105 L95 100 Z" fill="white"/>
</svg>"#
}

fn mockup_variant_specs(reference: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    let reference = reference.to_lowercase();
    if reference.contains("shopify") || reference.contains("commerce") {
        vec![
            (
                "merchant-command",
                "Merchant Command",
                "A commerce-first landing page with a bold hero, proof points, and merchant operations story.",
            ),
            (
                "growth-story",
                "Growth Story",
                "A calmer narrative route focused on outcomes, trust, and guided onboarding for non-technical buyers.",
            ),
            (
                "operator-console",
                "Operator Console",
                "A product-led version that leads with the operating experience and the system behind the storefront.",
            ),
        ]
    } else {
        vec![
            (
                "future-brief",
                "Future Brief",
                "A concise executive overview that explains the offer, the process, and the outcome in one pass.",
            ),
            (
                "guided-journey",
                "Guided Journey",
                "A client-friendly flow with discovery, design options, approval, build, and test made explicit.",
            ),
            (
                "product-proof",
                "Product Proof",
                "A stronger product-led route that pairs a live system preview with trust and implementation evidence.",
            ),
        ]
    }
}

fn render_mockup_html(
    feature: &Feature,
    option_title: &str,
    option_summary: &str,
    reference: &str,
) -> String {
    let feature_title = &feature.title;
    let feature_description = if feature.description.trim().is_empty() {
        "A client-facing digital experience designed to be approved quickly and then executed through DX Terminal."
    } else {
        feature.description.as_str()
    };
    let reference_label = if reference.trim().is_empty() {
        "Client reference"
    } else {
        reference
    };
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{feature_title} — {option_title}</title>
  <style>
    :root {{
      --bg:#0f0f1a;
      --surface:#17172a;
      --surface-2:#1d1d33;
      --text:#f8fafc;
      --muted:#a3aed0;
      --cyan:#06b6d4;
      --violet:#a855f7;
      --pink:#ff00aa;
      --line:rgba(255,255,255,.1);
      --shadow:0 30px 80px rgba(3,7,18,.55);
    }}
    * {{ box-sizing:border-box; }}
    body {{
      margin:0;
      font-family:"SF Pro Display","Segoe UI",system-ui,sans-serif;
      color:var(--text);
      background:
        radial-gradient(circle at 10% 10%, rgba(6,182,212,.14), transparent 28%),
        radial-gradient(circle at 85% 18%, rgba(168,85,247,.18), transparent 24%),
        radial-gradient(circle at 60% 80%, rgba(255,0,170,.12), transparent 22%),
        linear-gradient(180deg,#0b0b14 0%, #0f0f1a 100%);
      min-height:100vh;
    }}
    .shell {{
      max-width:1240px;
      margin:0 auto;
      padding:32px 24px 56px;
    }}
    .nav {{
      display:flex;
      align-items:center;
      justify-content:space-between;
      gap:16px;
      padding:12px 16px;
      border:1px solid var(--line);
      border-radius:24px;
      background:rgba(15,15,26,.72);
      backdrop-filter:blur(16px);
      box-shadow:var(--shadow);
    }}
    .brand {{
      display:flex;
      align-items:center;
      gap:14px;
      font-weight:700;
      letter-spacing:.08em;
      text-transform:uppercase;
    }}
    .brand small {{
      display:block;
      color:var(--muted);
      font-size:11px;
      letter-spacing:.18em;
    }}
    .brand strong {{
      display:block;
      font-size:15px;
    }}
    .nav-pill {{
      border:1px solid rgba(6,182,212,.28);
      color:#c4f6ff;
      padding:8px 12px;
      border-radius:999px;
      font-size:12px;
      background:rgba(6,182,212,.08);
    }}
    .hero {{
      display:grid;
      grid-template-columns:1.2fr .95fr;
      gap:28px;
      align-items:stretch;
      margin-top:26px;
    }}
    .hero-copy,
    .hero-card,
    .section-card,
    .lane {{
      background:linear-gradient(180deg, rgba(255,255,255,.04), rgba(255,255,255,.02));
      border:1px solid var(--line);
      border-radius:28px;
      box-shadow:var(--shadow);
      position:relative;
      overflow:hidden;
    }}
    .hero-copy {{
      padding:36px;
    }}
    .hero-copy::before,
    .hero-card::before,
    .section-card::before {{
      content:"";
      position:absolute;
      inset:0;
      background:linear-gradient(135deg, rgba(6,182,212,.08), rgba(168,85,247,.08), rgba(255,0,170,.05));
      pointer-events:none;
    }}
    .eyebrow {{
      display:inline-flex;
      gap:8px;
      align-items:center;
      border:1px solid rgba(168,85,247,.26);
      background:rgba(168,85,247,.1);
      color:#edd4ff;
      padding:8px 12px;
      border-radius:999px;
      font-size:12px;
      text-transform:uppercase;
      letter-spacing:.14em;
    }}
    h1 {{
      font-size:clamp(42px, 5vw, 76px);
      line-height:.96;
      margin:18px 0 14px;
      max-width:12ch;
    }}
    .lede {{
      font-size:18px;
      line-height:1.7;
      color:#d6def8;
      max-width:58ch;
    }}
    .cta-row {{
      display:flex;
      gap:12px;
      flex-wrap:wrap;
      margin-top:26px;
    }}
    .cta-primary,
    .cta-secondary {{
      display:inline-flex;
      align-items:center;
      justify-content:center;
      padding:13px 18px;
      border-radius:16px;
      text-decoration:none;
      font-weight:700;
    }}
    .cta-primary {{
      color:white;
      background:linear-gradient(135deg,var(--cyan),var(--violet));
      box-shadow:0 20px 40px rgba(6,182,212,.22);
    }}
    .cta-secondary {{
      color:#d6def8;
      border:1px solid var(--line);
      background:rgba(255,255,255,.04);
    }}
    .hero-card {{
      padding:24px;
      display:flex;
      flex-direction:column;
      gap:16px;
    }}
    .preview-window {{
      border:1px solid rgba(255,255,255,.12);
      background:#0c0d17;
      border-radius:22px;
      overflow:hidden;
    }}
    .window-head {{
      display:flex;
      align-items:center;
      gap:8px;
      padding:14px 16px;
      border-bottom:1px solid rgba(255,255,255,.06);
      background:rgba(255,255,255,.03);
    }}
    .window-dot {{ width:10px; height:10px; border-radius:999px; }}
    .window-dot.red {{ background:#ff5f57; }}
    .window-dot.yellow {{ background:#febc2e; }}
    .window-dot.green {{ background:#28c840; }}
    .preview-body {{
      padding:18px;
      display:grid;
      grid-template-columns:1fr 1fr;
      gap:14px;
    }}
    .panel {{
      background:var(--surface);
      border:1px solid rgba(255,255,255,.06);
      border-radius:18px;
      padding:16px;
      min-height:120px;
    }}
    .panel h3 {{
      margin:0 0 8px;
      font-size:14px;
      letter-spacing:.08em;
      text-transform:uppercase;
      color:#c4f6ff;
    }}
    .panel p,
    .panel li {{
      color:#b8c3e3;
      font-size:14px;
      line-height:1.65;
    }}
    .grid {{
      display:grid;
      grid-template-columns:repeat(3,minmax(0,1fr));
      gap:18px;
      margin-top:22px;
    }}
    .section-card {{
      padding:24px;
    }}
    .section-card h2 {{
      margin:0 0 12px;
      font-size:22px;
    }}
    .mini {{
      color:var(--muted);
      font-size:14px;
      line-height:1.6;
    }}
    .process {{
      display:grid;
      grid-template-columns:repeat(5,minmax(0,1fr));
      gap:14px;
      margin-top:18px;
    }}
    .lane {{
      padding:18px;
      background:linear-gradient(180deg, rgba(255,255,255,.05), rgba(255,255,255,.02));
      border:1px solid var(--line);
      border-radius:18px;
    }}
    .lane .step {{
      font-size:11px;
      text-transform:uppercase;
      letter-spacing:.16em;
      color:#c4f6ff;
    }}
    .lane strong {{
      display:block;
      margin-top:10px;
      font-size:17px;
    }}
    .lane p {{
      color:#b8c3e3;
      font-size:13px;
      line-height:1.6;
    }}
    .proof-row {{
      display:grid;
      grid-template-columns:repeat(4,minmax(0,1fr));
      gap:12px;
      margin-top:18px;
    }}
    .proof {{
      border:1px solid var(--line);
      background:rgba(255,255,255,.03);
      padding:16px;
      border-radius:18px;
    }}
    .proof strong {{
      display:block;
      font-size:28px;
      margin-bottom:6px;
    }}
    .foot {{
      display:flex;
      justify-content:space-between;
      gap:18px;
      margin-top:24px;
      color:var(--muted);
      font-size:13px;
    }}
    @media (max-width: 980px) {{
      .hero,
      .grid,
      .process,
      .proof-row,
      .preview-body {{
        grid-template-columns:1fr;
      }}
      h1 {{ max-width:none; }}
      .shell {{ padding:18px 14px 32px; }}
    }}
  </style>
</head>
<body>
  <div class="shell">
    <div class="nav">
      <div class="brand">
        {logo}
        <div>
          <small>DataXLR8</small>
          <strong>{option_title}</strong>
        </div>
      </div>
      <div class="nav-pill">Reference: {reference_label}</div>
    </div>

    <section class="hero">
      <div class="hero-copy">
        <span class="eyebrow">Quick discovery + design direction</span>
        <h1>{feature_title}</h1>
        <p class="lede">{feature_description}</p>
        <div class="cta-row">
          <a class="cta-primary" href="#process">Review the concept flow</a>
          <a class="cta-secondary" href="#proof">See proof and trust signals</a>
        </div>
      </div>

      <div class="hero-card">
        <div class="preview-window">
          <div class="window-head">
            <span class="window-dot red"></span>
            <span class="window-dot yellow"></span>
            <span class="window-dot green"></span>
          </div>
          <div class="preview-body">
            <div class="panel">
              <h3>Client Promise</h3>
              <p>{option_summary}</p>
            </div>
            <div class="panel">
              <h3>Delivery Engine</h3>
              <ul>
                <li>Portal-led discovery and approval</li>
                <li>Parallel branch execution behind the scenes</li>
                <li>Shared documentation, git, and runtime sync</li>
              </ul>
            </div>
            <div class="panel">
              <h3>What the client sees</h3>
              <p>A guided journey, mockup options, and clear approval points instead of raw terminal activity.</p>
            </div>
            <div class="panel">
              <h3>What the team sees</h3>
              <p>Live branches, panes, test evidence, and delivery gates coordinated by DX Terminal.</p>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="grid" id="proof">
      <article class="section-card">
        <h2>Positioning</h2>
        <p class="mini">This concept keeps the client in the portal while the implementation team works in isolated panes and worktrees. Approval happens here. Execution happens underneath.</p>
      </article>
      <article class="section-card">
        <h2>Process clarity</h2>
        <p class="mini">The customer gets a readable journey: discovery questions, design options, approval, build progress, verification, and release readiness.</p>
      </article>
      <article class="section-card">
        <h2>Brand feel</h2>
        <p class="mini">Dark future-facing palette, hexagonal DX signature, cyan-violet glow, and terminal-derived product credibility without exposing internal complexity.</p>
      </article>
    </section>

    <section class="section-card" id="process" style="margin-top:22px">
      <h2>Client Journey</h2>
      <p class="mini">A single flow from problem framing to approved direction and delivery.</p>
      <div class="process">
        <div class="lane"><span class="step">01</span><strong>Discovery</strong><p>Clarify goals, audience, references, and constraints.</p></div>
        <div class="lane"><span class="step">02</span><strong>Design</strong><p>Show 2-3 quick concepts so the client can react early.</p></div>
        <div class="lane"><span class="step">03</span><strong>Approval</strong><p>Approve one direction in the portal and unlock implementation.</p></div>
        <div class="lane"><span class="step">04</span><strong>Build</strong><p>Parallel panes and branches execute the approved work.</p></div>
        <div class="lane"><span class="step">05</span><strong>Test</strong><p>Evidence, docs, and release readiness stay synced in one place.</p></div>
      </div>
    </section>

    <section class="section-card" style="margin-top:22px">
      <h2>Proof Layer</h2>
      <div class="proof-row">
        <div class="proof"><strong>3x</strong>Parallel design directions before build.</div>
        <div class="proof"><strong>1</strong>Approved source of truth for implementation.</div>
        <div class="proof"><strong>Live</strong>Docs, git, and runtime state kept in sync.</div>
        <div class="proof"><strong>Zero</strong>Need for the client to watch terminals.</div>
      </div>
      <div class="foot">
        <span>Feature: {feature_title}</span>
        <span>Direction: {option_title}</span>
        <span>Reference: {reference_label}</span>
      </div>
    </section>
  </div>
</body>
</html>"##,
        feature_title = feature_title,
        option_title = option_title,
        feature_description = feature_description,
        option_summary = option_summary,
        reference_label = reference_label,
        logo = dataxlr8_hex_logo_svg(),
    )
}

pub fn seed_mockup_options(
    project_path: &str,
    feature_id: &str,
    reference: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let reference = reference.unwrap_or("").trim();
    let variants = mockup_variant_specs(reference);
    let mockup_root = mockups_dir(project_path, feature_id);
    if let Err(e) = std::fs::create_dir_all(&mockup_root) {
        return serde_json::json!({"error": format!("mkdir: {}", e)}).to_string();
    }

    let mut created = Vec::new();
    for (slug, title, summary) in variants {
        let option_id = design_option_id(feature_id, feature.design_options.len());
        let file_name = format!(
            "{}-{}.html",
            option_id.to_lowercase(),
            slugify_fragment(slug)
        );
        let relative_path = format!(".vision/mockups/{}/{}", feature_id, file_name);
        let html = render_mockup_html(feature, title, summary, reference);
        let path = mockup_root.join(&file_name);
        if let Err(e) = std::fs::write(&path, html) {
            return serde_json::json!({"error": format!("write: {}", e)}).to_string();
        }

        let option = DesignOption {
            id: option_id.clone(),
            title: title.to_string(),
            summary: summary.to_string(),
            kind: "mockup".to_string(),
            status: DesignOptionStatus::Proposed,
            relative_path: Some(relative_path.clone()),
            reference: if reference.is_empty() {
                None
            } else {
                Some(reference.to_string())
            },
            approved_by: None,
            approved_at: None,
            review_notes: vec![],
            created_at: now(),
            updated_at: now(),
        };
        feature.design_options.push(option.clone());
        created.push(serde_json::json!({
            "id": option.id,
            "title": option.title,
            "summary": option.summary,
            "status": option.status,
            "path": relative_path,
        }));
    }

    if feature.phase == FeaturePhase::Planned {
        set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let feature_phase = feature.phase.clone();
    let feature_state = feature.state.clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("mockups:{}", feature_id),
        old_value: String::new(),
        new_value: format!("{} seeded option(s)", created.len()),
        reason: "Design mockup options seeded".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "seeded",
            "feature": feature_id,
            "count": created.len(),
            "options": created,
            "phase": feature_phase,
            "state": feature_state,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn review_design_option(
    project_path: &str,
    feature_id: &str,
    option_id: &str,
    status: &str,
    note: Option<&str>,
    actor: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let parsed_status = match status {
        "approved" => DesignOptionStatus::Approved,
        "rejected" => DesignOptionStatus::Rejected,
        "proposed" => DesignOptionStatus::Proposed,
        "draft" => DesignOptionStatus::Draft,
        _ => {
            return serde_json::json!({
                "error": "invalid_status",
                "options": ["draft", "proposed", "approved", "rejected"]
            })
            .to_string()
        }
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let Some(selected_idx) = feature
        .design_options
        .iter()
        .position(|option| option.id == option_id)
    else {
        return serde_json::json!({"error": "design_option_not_found", "id": option_id})
            .to_string();
    };

    if parsed_status == DesignOptionStatus::Approved {
        for (idx, option) in feature.design_options.iter_mut().enumerate() {
            if idx != selected_idx && option.status == DesignOptionStatus::Approved {
                option.status = DesignOptionStatus::Proposed;
                option.updated_at = now();
            }
        }
    }

    let option = &mut feature.design_options[selected_idx];
    option.status = parsed_status.clone();
    if let Some(note) = note.filter(|value| !value.trim().is_empty()) {
        option.review_notes.push(note.to_string());
    }
    if parsed_status == DesignOptionStatus::Approved {
        option.approved_by = actor
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        option.approved_at = Some(now());
    }
    option.updated_at = now();

    if feature.phase == FeaturePhase::Planned {
        set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let feature_phase = feature.phase.clone();
    let feature_state = feature.state.clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("design_option:{}", option_id),
        old_value: String::new(),
        new_value: status.to_string(),
        reason: "Design option review updated".to_string(),
        triggered_by: actor
            .map(|value| value.to_string())
            .unwrap_or_else(|| "user".to_string()),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "reviewed",
            "feature": feature_id,
            "option_id": option_id,
            "option_status": parsed_status,
            "phase": feature_phase,
            "state": feature_state,
            "readiness": feature_readiness_value(project_path, feature),
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn read_mockup_html(
    project_path: &str,
    feature_id: &str,
    option_id: &str,
) -> Result<String, String> {
    let vision = load_vision(project_path).ok_or_else(|| "no_vision".to_string())?;
    let feature = vision
        .features
        .iter()
        .find(|f| f.id == feature_id)
        .ok_or_else(|| "feature_not_found".to_string())?;
    let option = feature
        .design_options
        .iter()
        .find(|design| design.id == option_id)
        .ok_or_else(|| "design_option_not_found".to_string())?;
    let relative = option
        .relative_path
        .as_deref()
        .ok_or_else(|| "design_option_missing_path".to_string())?;
    std::fs::read_to_string(Path::new(project_path).join(relative))
        .map_err(|e| format!("read: {}", e))
}

pub fn start_discovery(project_path: &str, feature_id: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    if feature.phase == FeaturePhase::Discovery {
        return serde_json::json!({
            "status": "noop",
            "feature": feature_id,
            "phase": feature.phase,
            "state": feature.state,
        })
        .to_string();
    }

    if feature.phase != FeaturePhase::Planned {
        return serde_json::json!({
            "status": "blocked",
            "feature": feature_id,
            "phase": feature.phase,
            "state": feature.state,
            "reason": "feature_not_in_planned_phase",
        })
        .to_string();
    }

    let old_phase = serde_json::to_string(&feature.phase).unwrap_or_default();
    set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("feature_phase:{}", feature_id),
        old_value: old_phase,
        new_value: "discovery".to_string(),
        reason: "Discovery started".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "started",
            "feature": feature_id,
            "phase": "discovery",
            "state": "active",
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn add_acceptance_criterion(project_path: &str, feature_id: &str, criterion: &str) -> String {
    let criterion = criterion.trim();
    if criterion.is_empty() {
        return serde_json::json!({"error": "criterion_required"}).to_string();
    }

    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    if feature.acceptance_items.iter().any(|c| c.text == criterion) {
        return serde_json::json!({
            "status": "noop",
            "feature": feature_id,
            "criterion": criterion,
            "reason": "criterion_exists",
        })
        .to_string();
    }

    if feature.phase == FeaturePhase::Planned {
        set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    }

    let criterion_id = acceptance_id(feature_id, feature.acceptance_items.len());
    feature.acceptance_items.push(AcceptanceCriterion {
        id: criterion_id.clone(),
        text: criterion.to_string(),
        status: AcceptanceStatus::Draft,
        verification_method: None,
        evidence: vec![],
        verified_at: None,
        verified_by: None,
        verification_source: None,
    });
    sync_acceptance_items(feature);
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let acceptance_count = feature.acceptance_criteria.len();
    let feature_phase = feature.phase.clone();
    let feature_state = feature.state.clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Added,
        field: format!("acceptance:{}", feature_id),
        old_value: String::new(),
        new_value: criterion.to_string(),
        reason: "Acceptance criterion added".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "feature": feature_id,
            "criterion_id": criterion_id,
            "criterion": criterion,
            "count": acceptance_count,
            "phase": feature_phase,
            "state": feature_state,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn update_acceptance_criterion(
    project_path: &str,
    feature_id: &str,
    criterion_id: &str,
    text: Option<&str>,
    verification_method: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let item_idx = match feature
        .acceptance_items
        .iter()
        .position(|item| item.id == criterion_id)
    {
        Some(idx) => idx,
        None => {
            return serde_json::json!({"error": "criterion_not_found", "id": criterion_id})
                .to_string()
        }
    };

    if let Some(next_text) = text.map(str::trim).filter(|text| !text.is_empty()) {
        feature.acceptance_items[item_idx].text = next_text.to_string();
    }
    if let Some(method) = verification_method.map(str::trim) {
        feature.acceptance_items[item_idx].verification_method = if method.is_empty() {
            None
        } else {
            Some(method.to_string())
        };
        if feature.acceptance_items[item_idx].status == AcceptanceStatus::Draft
            && feature.acceptance_items[item_idx]
                .verification_method
                .is_some()
        {
            feature.acceptance_items[item_idx].status = AcceptanceStatus::Mapped;
        }
    }

    sync_acceptance_items(feature);
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let item_text = feature.acceptance_items[item_idx].text.clone();
    let item_status = feature.acceptance_items[item_idx].status.clone();
    let item_method = feature.acceptance_items[item_idx]
        .verification_method
        .clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Modified,
        field: format!("acceptance:{}", criterion_id),
        old_value: String::new(),
        new_value: item_text.clone(),
        reason: "Acceptance criterion updated".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "updated",
            "feature": feature_id,
            "criterion_id": criterion_id,
            "criterion": item_text,
            "criterion_status": item_status,
            "verification_method": item_method,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn verify_acceptance_criterion(
    project_path: &str,
    feature_id: &str,
    criterion_id: &str,
    status: &str,
    evidence: Vec<String>,
    verified_by: Option<&str>,
    verification_source: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let parsed_status = match status {
        "mapped" => AcceptanceStatus::Mapped,
        "verified" => AcceptanceStatus::Verified,
        "failed" => AcceptanceStatus::Failed,
        "draft" => AcceptanceStatus::Draft,
        _ => {
            return serde_json::json!({
                "error": "invalid_status",
                "options": ["draft", "mapped", "verified", "failed"]
            })
            .to_string()
        }
    };

    let item_idx = match feature
        .acceptance_items
        .iter()
        .position(|item| item.id == criterion_id)
    {
        Some(idx) => idx,
        None => {
            return serde_json::json!({"error": "criterion_not_found", "id": criterion_id})
                .to_string()
        }
    };

    feature.acceptance_items[item_idx].status = parsed_status.clone();
    feature.acceptance_items[item_idx].evidence = evidence;
    feature.acceptance_items[item_idx].verified_at = if parsed_status == AcceptanceStatus::Verified
        || parsed_status == AcceptanceStatus::Failed
    {
        Some(now())
    } else {
        None
    };
    feature.acceptance_items[item_idx].verified_by = verified_by
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    feature.acceptance_items[item_idx].verification_source = verification_source
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());

    sync_acceptance_items(feature);
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();
    let item_text = feature.acceptance_items[item_idx].text.clone();
    let item_evidence = feature.acceptance_items[item_idx].evidence.clone();
    let item_verified_at = feature.acceptance_items[item_idx].verified_at.clone();
    let item_verified_by = feature.acceptance_items[item_idx].verified_by.clone();
    let item_verification_source = feature.acceptance_items[item_idx]
        .verification_source
        .clone();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("acceptance:{}", criterion_id),
        old_value: String::new(),
        new_value: status.to_string(),
        reason: "Acceptance criterion verification updated".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "updated",
            "feature": feature_id,
            "criterion_id": criterion_id,
            "criterion": item_text,
            "criterion_status": parsed_status,
            "evidence": item_evidence,
            "verified_at": item_verified_at,
            "verified_by": item_verified_by,
            "verification_source": item_verification_source,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Add a question to a feature.
pub fn add_question(project_path: &str, feature_id: &str, text: &str) -> String {
    add_question_with_blocking(project_path, feature_id, text, true)
}

pub fn add_question_with_blocking(
    project_path: &str,
    feature_id: &str,
    text: &str,
    blocking: bool,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let q_num = feature.questions.len() + 1;
    let id = format!("Q{}.{}", feature_id.trim_start_matches('F'), q_num);

    let question = Question {
        id: id.clone(),
        text: text.to_string(),
        status: QuestionStatus::Open,
        blocking,
        answer: None,
        asked_at: now(),
        answered_at: None,
        decision_id: None,
    };

    feature.questions.push(question);

    // Move to specifying if it was planned
    if feature.status == FeatureStatus::Planned {
        set_feature_lifecycle(feature, FeaturePhase::Discovery, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "question": id,
            "feature": feature_id,
            "blocking": blocking,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Answer a question and record a decision.
pub fn answer_question(
    project_path: &str,
    feature_id: &str,
    question_id: &str,
    answer: &str,
    rationale: &str,
    alternatives: Vec<String>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let question = match feature.questions.iter_mut().find(|q| q.id == question_id) {
        Some(q) => q,
        None => {
            return serde_json::json!({"error": "question_not_found", "id": question_id})
                .to_string()
        }
    };

    // Create decision
    let d_num = feature.decisions.len() + 1;
    let decision_id = format!("D{}.{}", feature_id.trim_start_matches('F'), d_num);

    let decision = VisionDecision {
        id: decision_id.clone(),
        question_id: Some(question_id.to_string()),
        decision: answer.to_string(),
        rationale: rationale.to_string(),
        date: now(),
        alternatives,
    };

    // Update question
    question.status = QuestionStatus::Answered;
    question.answer = Some(answer.to_string());
    question.answered_at = Some(now());
    question.decision_id = Some(decision_id.clone());

    feature.decisions.push(decision);
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::Modified,
        field: format!("question:{}", question_id),
        old_value: "open".to_string(),
        new_value: format!("answered: {}", answer),
        reason: rationale.to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "answered",
            "question": question_id,
            "decision": decision_id,
            "feature": feature_id,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Add a task to a feature.
pub fn add_task(
    project_path: &str,
    feature_id: &str,
    title: &str,
    description: &str,
    branch: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let t_num = feature.tasks.len() + 1;
    let id = format!("T{}.{}", feature_id.trim_start_matches('F'), t_num);

    let task = VisionTask {
        id: id.clone(),
        feature_id: feature_id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        status: TaskStatus::Planned,
        branch: branch.map(|s| s.to_string()),
        pr: None,
        commit: None,
        assignee: None,
        created_at: now(),
        updated_at: now(),
    };

    feature.tasks.push(task);

    // Move to building if it was specifying and all questions answered
    let all_answered = feature
        .questions
        .iter()
        .all(|q| q.status == QuestionStatus::Answered);
    if (feature.status == FeatureStatus::Specifying || feature.status == FeatureStatus::Planned)
        && all_answered
    {
        set_feature_lifecycle(feature, FeaturePhase::Build, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "task": id,
            "feature": feature_id,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Update task status, optionally linking a branch or PR.
pub fn update_task_status(
    project_path: &str,
    feature_id: &str,
    task_id: &str,
    new_status: &str,
    branch: Option<&str>,
    pr: Option<&str>,
    commit: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let task = match feature.tasks.iter_mut().find(|t| t.id == task_id) {
        Some(t) => t,
        None => return serde_json::json!({"error": "task_not_found", "id": task_id}).to_string(),
    };

    let parsed: TaskStatus = match new_status {
        "planned" => TaskStatus::Planned,
        "in_progress" => TaskStatus::InProgress,
        "done" => TaskStatus::Done,
        "verified" => TaskStatus::Verified,
        "blocked" => TaskStatus::Blocked,
        _ => return serde_json::json!({"error": "invalid_status", "options": ["planned","in_progress","done","verified","blocked"]}).to_string(),
    };

    let old_status = serde_json::to_string(&task.status).unwrap_or_default();
    task.status = parsed;
    if let Some(b) = branch {
        task.branch = Some(b.to_string());
    }
    if let Some(p) = pr {
        task.pr = Some(p.to_string());
    }
    if let Some(c) = commit {
        task.commit = Some(c.to_string());
    }
    task.updated_at = now();

    // Auto-update feature status based on task completion
    let all_done = feature
        .tasks
        .iter()
        .all(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified);
    let any_in_progress = feature
        .tasks
        .iter()
        .any(|t| t.status == TaskStatus::InProgress);
    if all_done && !feature.tasks.is_empty() {
        set_feature_lifecycle(feature, FeaturePhase::Test, FeatureState::Active);
    } else if any_in_progress && feature.status != FeatureStatus::Building {
        set_feature_lifecycle(feature, FeaturePhase::Build, FeatureState::Active);
    }
    reconcile_feature_lifecycle(project_path, feature);
    feature.updated_at = now();
    vision.updated_at = now();

    let feature_status = serde_json::to_string(&feature.status).unwrap_or_default();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("task:{}", task_id),
        old_value: old_status,
        new_value: new_status.to_string(),
        reason: "Task status updated".to_string(),
        triggered_by: "agent".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "updated",
            "task": task_id,
            "task_status": new_status,
            "feature": feature_id,
            "feature_status": feature_status,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Drill down into a goal — returns all features with questions/tasks.
pub fn drill_down(project_path: &str, goal_id: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let goal = match vision.goals.iter().find(|g| g.id == goal_id) {
        Some(g) => g,
        None => return serde_json::json!({"error": "goal_not_found", "id": goal_id}).to_string(),
    };

    let features: Vec<_> = vision.features.iter()
        .filter(|f| f.goal_id == goal_id)
        .map(|f| {
            let open_qs = f.questions.iter().filter(|q| q.status == QuestionStatus::Open).count();
            let total_tasks = f.tasks.len();
            let done_tasks = f.tasks.iter().filter(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified).count();
            serde_json::json!({
                "id": f.id,
                "title": f.title,
                "description": f.description,
                "status": f.status,
                "phase": f.phase,
                "state": f.state,
                "questions": {
                    "total": f.questions.len(),
                    "open": open_qs,
                    "items": f.questions,
                },
                "decisions": f.decisions,
                "tasks": {
                    "total": total_tasks,
                    "done": done_tasks,
                    "items": f.tasks,
                },
                "acceptance_criteria": f.acceptance_criteria,
                "acceptance_items": f.acceptance_items,
                "design_options": f.design_options,
                "sub_vision": f.sub_vision,
                "readiness": feature_readiness_value(project_path, f),
                "progress": if total_tasks > 0 { (done_tasks as f64 / total_tasks as f64 * 100.0) as u8 } else { 0 },
            })
        })
        .collect();

    serde_json::json!({
        "goal": {
            "id": goal.id,
            "title": goal.title,
            "description": goal.description,
            "status": goal.status,
            "priority": goal.priority,
        },
        "features": features,
        "feature_count": features.len(),
    })
    .to_string()
}

/// Create a sub-vision file for a feature (recursive vision).
pub fn create_sub_vision(project_path: &str, feature_id: &str, mission: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    if feature.sub_vision.is_some() {
        return serde_json::json!({"error": "sub_vision_exists", "feature": feature_id})
            .to_string();
    }

    // Create the sub-vision file
    let dir = features_dir(project_path);
    let _ = std::fs::create_dir_all(&dir);
    let filename = format!("{}.json", feature_id);
    let sub_path = dir.join(&filename);
    let relative_path = format!(".vision/features/{}", filename);

    let sub_vision = Vision {
        project: format!("{} — {}", vision.project, feature.title),
        mission: mission.to_string(),
        principles: vec![],
        goals: vec![],
        milestones: vec![],
        architecture: vec![],
        changes: vec![],
        features: vec![],
        github: vision.github.clone(),
        updated_at: now(),
    };

    let json = match serde_json::to_string_pretty(&sub_vision) {
        Ok(j) => j,
        Err(e) => return serde_json::json!({"error": format!("serialize: {}", e)}).to_string(),
    };

    if let Err(e) = std::fs::write(&sub_path, json) {
        return serde_json::json!({"error": format!("write: {}", e)}).to_string();
    }

    // Link it
    feature.sub_vision = Some(relative_path.clone());
    feature.updated_at = now();
    vision.updated_at = now();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "created",
            "sub_vision": relative_path,
            "feature": feature_id,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Get full vision tree: vision → goals → features → tasks with progress rollup.
pub fn vision_tree(project_path: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let goals: Vec<_> = vision.goals.iter().map(|g| {
        let features: Vec<_> = vision.features.iter()
            .filter(|f| f.goal_id == g.id)
            .map(|f| {
                let total_tasks = f.tasks.len();
                let done_tasks = f.tasks.iter()
                    .filter(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified)
                    .count();
                let open_questions = f.questions.iter()
                    .filter(|q| q.status == QuestionStatus::Open)
                    .count();

                serde_json::json!({
                    "id": f.id,
                    "title": f.title,
                    "status": f.status,
                    "phase": f.phase,
                    "state": f.state,
                    "open_questions": open_questions,
                    "tasks_done": done_tasks,
                    "tasks_total": total_tasks,
                    "progress": if total_tasks > 0 { (done_tasks as f64 / total_tasks as f64 * 100.0) as u8 } else { 0 },
                    "has_sub_vision": f.sub_vision.is_some(),
                    "acceptance_items": f.acceptance_items,
                    "design_options": f.design_options,
                    "readiness": feature_readiness_value(project_path, f),
                    "tasks": f.tasks.iter().map(|t| serde_json::json!({
                        "id": t.id,
                        "title": t.title,
                        "status": t.status,
                        "branch": t.branch,
                        "pr": t.pr,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();

        let total_features = features.len();
        let done_features = vision.features.iter()
            .filter(|f| f.goal_id == g.id && f.status == FeatureStatus::Done)
            .count();

        serde_json::json!({
            "id": g.id,
            "title": g.title,
            "status": g.status,
            "priority": g.priority,
            "features": features,
            "features_done": done_features,
            "features_total": total_features,
            "progress": if total_features > 0 { (done_features as f64 / total_features as f64 * 100.0) as u8 } else { 0 },
        })
    }).collect();

    let total_features = vision.features.len();
    let done_features = vision
        .features
        .iter()
        .filter(|f| f.status == FeatureStatus::Done)
        .count();
    let total_tasks: usize = vision.features.iter().map(|f| f.tasks.len()).sum();
    let done_tasks: usize = vision
        .features
        .iter()
        .flat_map(|f| f.tasks.iter())
        .filter(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified)
        .count();

    serde_json::json!({
        "project": vision.project,
        "mission": vision.mission,
        "goals": goals,
        "summary": {
            "goals_total": vision.goals.len(),
            "goals_achieved": vision.goals.iter().filter(|g| g.status == GoalStatus::Achieved).count(),
            "features_total": total_features,
            "features_done": done_features,
            "tasks_total": total_tasks,
            "tasks_done": done_tasks,
            "overall_progress": if total_tasks > 0 { (done_tasks as f64 / total_tasks as f64 * 100.0) as u8 } else { 0 },
        },
        "github": {
            "repo": vision.github.repo,
            "sync": vision.github.sync_enabled,
        },
        "updated_at": vision.updated_at,
    }).to_string()
}

pub fn feature_readiness(project_path: &str, feature_id: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    serde_json::json!({
        "feature_id": feature.id,
        "goal_id": feature.goal_id,
        "title": feature.title,
        "status": feature.status,
        "phase": feature.phase,
        "state": feature.state,
        "acceptance_items": feature.acceptance_items,
        "design_options": feature.design_options,
        "readiness": feature_readiness_value(project_path, feature),
    })
    .to_string()
}

pub fn discovery_ready_check(project_path: &str, feature_id: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let readiness = feature_readiness_value(project_path, feature);
    let ready = readiness["ready_for_build"].as_bool().unwrap_or(false);

    serde_json::json!({
        "feature_id": feature.id,
        "goal_id": feature.goal_id,
        "title": feature.title,
        "phase": feature.phase,
        "state": feature.state,
        "ready": ready,
        "checks": readiness["discovery"].clone(),
        "blockers": readiness["blockers"]["build"].clone(),
    })
    .to_string()
}

pub fn complete_discovery(project_path: &str, feature_id: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature_idx = match vision.features.iter().position(|f| f.id == feature_id) {
        Some(idx) => idx,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let readiness = feature_readiness_value(project_path, &vision.features[feature_idx]);
    let blockers = readiness["blockers"]["build"].clone();
    let ready = readiness["ready_for_build"].as_bool().unwrap_or(false);
    if !ready {
        return serde_json::json!({
            "status": "blocked",
            "feature": feature_id,
            "phase": vision.features[feature_idx].phase,
            "blockers": blockers,
            "checks": readiness["discovery"].clone(),
        })
        .to_string();
    }

    let feature = &mut vision.features[feature_idx];
    if feature.phase == FeaturePhase::Build
        || feature.phase == FeaturePhase::Test
        || feature.phase == FeaturePhase::Done
    {
        return serde_json::json!({
            "status": "noop",
            "feature": feature_id,
            "phase": feature.phase,
            "state": feature.state,
        })
        .to_string();
    }

    let old_phase = serde_json::to_string(&feature.phase).unwrap_or_default();
    set_feature_lifecycle(feature, FeaturePhase::Build, FeatureState::Active);
    feature.updated_at = now();
    vision.updated_at = now();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("feature_phase:{}", feature_id),
        old_value: old_phase,
        new_value: "build".to_string(),
        reason: "Discovery completed".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "advanced",
            "feature": feature_id,
            "phase": "build",
            "state": "active",
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

// ─── VDD: Work Assessment ───────────────────────────────────────────────────

/// Assess a work description against the vision — find matching goal, suggest feature.
pub fn assess_work(project_path: &str, description: &str) -> String {
    let vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let desc_lower = description.to_lowercase();
    let words: Vec<&str> = desc_lower.split_whitespace().collect();

    // Score each goal by keyword overlap
    let mut scored: Vec<(&Goal, usize)> = vision
        .goals
        .iter()
        .map(|g| {
            let goal_text = format!("{} {} {:?}", g.title, g.description, g.metrics).to_lowercase();
            let score = words.iter().filter(|w| goal_text.contains(*w)).count();
            (g, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let best = scored.first();

    if let Some((goal, score)) = best {
        if *score > 0 {
            // Find existing features for this goal
            let existing_features: Vec<_> = vision
                .features
                .iter()
                .filter(|f| f.goal_id == goal.id)
                .map(|f| {
                    serde_json::json!({
                        "id": f.id, "title": f.title, "status": f.status,
                    })
                })
                .collect();

            // Check if any existing feature matches
            let matching_feature = vision.features.iter().find(|f| {
                f.goal_id == goal.id && {
                    let ft = format!("{} {}", f.title, f.description).to_lowercase();
                    words.iter().filter(|w| ft.contains(*w)).count() > words.len() / 3
                }
            });

            return serde_json::json!({
                "matched": true,
                "goal": {
                    "id": goal.id,
                    "title": goal.title,
                    "status": goal.status,
                    "confidence": score,
                },
                "existing_features": existing_features,
                "matching_feature": matching_feature.map(|f| serde_json::json!({
                    "id": f.id, "title": f.title, "status": f.status,
                })),
                "suggested_action": if matching_feature.is_some() {
                    "Continue existing feature"
                } else {
                    "Create new feature under this goal"
                },
                "description": description,
            })
            .to_string();
        }
    }

    serde_json::json!({
        "matched": false,
        "suggestion": "No matching goal found. Consider creating a new goal first.",
        "goals": vision.goals.iter().map(|g| serde_json::json!({
            "id": g.id, "title": g.title, "status": g.status,
        })).collect::<Vec<_>>(),
        "description": description,
    })
    .to_string()
}

/// Sync task statuses from Git — check branch/PR status via gh CLI.
pub fn sync_git_status(project_path: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    if vision.github.repo.is_empty() {
        return serde_json::json!({"error": "no_github_repo"}).to_string();
    }

    let repo = vision.github.repo.clone();
    let mut updates = vec![];

    for feature in &mut vision.features {
        for task in &mut feature.tasks {
            let mut changed = false;

            // Check branch existence
            if let Some(ref branch) = task.branch {
                let branch_check = run_gh(&format!(
                    "gh api repos/{}/branches/{} --jq '.name' 2>/dev/null",
                    repo, branch
                ));
                let branch_exists =
                    !branch_check.trim().is_empty() && !branch_check.contains("error");

                // Check for open PR
                let pr_check = run_gh(&format!(
                    "gh pr list -R {} --head {} --state all --json number,state --jq '.[0]' 2>/dev/null", repo, branch
                ));

                if pr_check.contains("\"state\":\"MERGED\"")
                    || pr_check.contains("\"state\":\"merged\"")
                {
                    if task.status != TaskStatus::Done && task.status != TaskStatus::Verified {
                        task.status = TaskStatus::Done;
                        changed = true;
                    }
                } else if !pr_check.trim().is_empty()
                    && !pr_check.contains("error")
                    && pr_check.contains("number")
                {
                    // PR exists and is open — in progress
                    if task.status == TaskStatus::Planned {
                        task.status = TaskStatus::InProgress;
                        changed = true;
                    }
                    // Extract PR number
                    if task.pr.is_none() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&pr_check) {
                            if let Some(num) = v.get("number").and_then(|n| n.as_u64()) {
                                task.pr = Some(format!("#{}", num));
                                changed = true;
                            }
                        }
                    }
                } else if branch_exists && task.status == TaskStatus::Planned {
                    task.status = TaskStatus::InProgress;
                    changed = true;
                }
            }

            if changed {
                task.updated_at = now();
                updates.push(serde_json::json!({
                    "task": task.id,
                    "feature": task.feature_id,
                    "new_status": task.status,
                    "branch": task.branch,
                    "pr": task.pr,
                }));
            }
        }

        // Cascade: update feature status
        if !feature.tasks.is_empty() {
            let all_done = feature
                .tasks
                .iter()
                .all(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified);
            let any_in_progress = feature
                .tasks
                .iter()
                .any(|t| t.status == TaskStatus::InProgress);
            if all_done {
                set_feature_lifecycle(feature, FeaturePhase::Test, FeatureState::Active);
            } else if any_in_progress {
                set_feature_lifecycle(feature, FeaturePhase::Build, FeatureState::Active);
            }
        }
        reconcile_feature_lifecycle(project_path, feature);
    }

    vision.updated_at = now();
    let update_count = updates.len();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "synced",
            "updates": update_count,
            "details": updates,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Update feature status directly.
pub fn update_feature_status(project_path: &str, feature_id: &str, new_status: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => {
            return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string()
        }
    };

    let parsed: FeatureStatus = match new_status {
        "planned" => FeatureStatus::Planned,
        "specifying" => FeatureStatus::Specifying,
        "building" => FeatureStatus::Building,
        "testing" => FeatureStatus::Testing,
        "done" => FeatureStatus::Done,
        _ => return serde_json::json!({"error": "invalid_status", "options": ["planned","specifying","building","testing","done"]}).to_string(),
    };

    let old_status = serde_json::to_string(&feature.status).unwrap_or_default();
    let next_state = match parsed {
        FeatureStatus::Planned => FeatureState::Planned,
        FeatureStatus::Done => FeatureState::Complete,
        FeatureStatus::Specifying | FeatureStatus::Building | FeatureStatus::Testing => {
            FeatureState::Active
        }
    };
    set_feature_lifecycle(feature, feature_phase_from_status(&parsed), next_state);
    feature.updated_at = now();
    vision.updated_at = now();

    let change = VisionChange {
        timestamp: now(),
        change_type: ChangeType::StatusChange,
        field: format!("feature:{}", feature_id),
        old_value: old_status,
        new_value: new_status.to_string(),
        reason: "Feature status updated".to_string(),
        triggered_by: "user".to_string(),
        github_issue: None,
    };
    vision.changes.push(change.clone());
    append_history(project_path, &change);

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "updated",
            "feature": feature_id,
            "new_status": new_status,
        })
        .to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let vision_dir = dir.path().join(".vision");
        std::fs::create_dir_all(&vision_dir).unwrap();
        dir
    }

    fn init_test_vision(dir: &std::path::Path) {
        let path = dir.to_str().unwrap();
        init_vision(path, "test-proj", "Test mission", "user/repo");
    }

    #[test]
    fn test_sub_vision_uses_feature_id_not_goal_id() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal one", "desc", 1);
        add_feature(path, "G1", "Feature A", "desc", vec![]);
        add_feature(path, "G1", "Feature B", "desc", vec![]);

        let vision = load_vision(path).unwrap();
        let f1_id = &vision.features[0].id;
        let f2_id = &vision.features[1].id;

        // Create sub-visions for both features under same goal
        let r1 = create_sub_vision(path, f1_id, "Sub A mission");
        let r2 = create_sub_vision(path, f2_id, "Sub B mission");

        // Both should succeed (no overwrite)
        assert!(!r1.contains("error"), "First sub-vision failed: {}", r1);
        assert!(!r2.contains("error"), "Second sub-vision failed: {}", r2);

        // Files should be named by feature_id, not goal_id
        let features_dir = dir.path().join(".vision/features");
        assert!(
            features_dir.join(format!("{}.json", f1_id)).exists(),
            "Missing {}.json",
            f1_id
        );
        assert!(
            features_dir.join(format!("{}.json", f2_id)).exists(),
            "Missing {}.json",
            f2_id
        );
    }

    #[test]
    fn test_update_feature_status_all_phases() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec![]);

        let vision = load_vision(path).unwrap();
        let fid = &vision.features[0].id;

        for status in &["specifying", "building", "testing", "done"] {
            let result = update_feature_status(path, fid, status);
            assert!(
                !result.contains("error"),
                "Failed setting {}: {}",
                status,
                result
            );
            let v = load_vision(path).unwrap();
            let f = v.features.iter().find(|f| f.id == *fid).unwrap();
            let expected: FeatureStatus = match *status {
                "specifying" => FeatureStatus::Specifying,
                "building" => FeatureStatus::Building,
                "testing" => FeatureStatus::Testing,
                "done" => FeatureStatus::Done,
                _ => unreachable!(),
            };
            assert_eq!(f.status, expected);
            let expected_phase = match *status {
                "specifying" => FeaturePhase::Discovery,
                "building" => FeaturePhase::Build,
                "testing" => FeaturePhase::Test,
                "done" => FeaturePhase::Done,
                _ => unreachable!(),
            };
            assert_eq!(f.phase, expected_phase);
        }
    }

    #[test]
    fn test_update_feature_status_invalid() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec![]);

        let vision = load_vision(path).unwrap();
        let fid = &vision.features[0].id;

        let result = update_feature_status(path, fid, "banana");
        assert!(result.contains("invalid_status"));
    }

    #[test]
    fn test_legacy_feature_status_backfills_phase_and_state() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();

        let legacy = serde_json::json!({
            "project": "legacy-proj",
            "mission": "Legacy mission",
            "principles": [],
            "goals": [{
                "id": "G1",
                "title": "Goal",
                "description": "desc",
                "status": "planned",
                "priority": 1,
                "linked_issues": [],
                "metrics": []
            }],
            "milestones": [],
            "architecture": [],
            "changes": [],
            "features": [{
                "id": "F1.1",
                "goal_id": "G1",
                "title": "Legacy feature",
                "description": "desc",
                "status": "building",
                "questions": [],
                "decisions": [],
                "tasks": [],
                "acceptance_criteria": [],
                "sub_vision": null,
                "parent_vision": null,
                "created_at": "2026-03-12T00:00:00Z",
                "updated_at": "2026-03-12T00:00:00Z"
            }],
            "github": {
                "repo": "",
                "sync_enabled": false,
                "wiki_page": null,
                "project_board": null,
                "labels": []
            },
            "updated_at": "2026-03-12T00:00:00Z"
        });

        std::fs::write(
            vision_file(path),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let vision = load_vision(path).unwrap();
        let feature = &vision.features[0];
        assert_eq!(feature.status, FeatureStatus::Building);
        assert_eq!(feature.phase, FeaturePhase::Build);
        assert_eq!(feature.state, FeatureState::Active);
        assert_eq!(feature.acceptance_items.len(), 0);
    }

    #[test]
    fn test_feature_readiness_tracks_phase_and_blockers() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);

        add_question(path, "F1.1", "What protocol?");

        let discovery: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(discovery["phase"], "discovery");
        assert_eq!(discovery["state"], "active");
        assert_eq!(discovery["readiness"]["ready_for_build"], false);
        assert_eq!(
            discovery["readiness"]["counts"]["has_discovery_artifact"],
            false
        );

        answer_question(
            path,
            "F1.1",
            "Q1.1.1",
            "WebSocket",
            "Need bidirectional",
            vec![],
        );
        upsert_feature_doc(path, "F1.1", "discovery", "# Discovery");
        add_task(path, "F1.1", "Build it", "Implement feature", None);

        let build: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(build["phase"], "build");
        assert_eq!(build["readiness"]["ready_for_build"], true);
        assert_eq!(build["readiness"]["ready_for_test"], false);

        update_task_status(path, "F1.1", "T1.1.1", "verified", None, None, None);

        let test_ready: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(test_ready["phase"], "test");
        assert_eq!(test_ready["readiness"]["ready_for_test"], true);
        assert_eq!(test_ready["readiness"]["ready_for_done"], false);

        let criterion_id = test_ready["acceptance_items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        verify_acceptance_criterion(
            path,
            "F1.1",
            &criterion_id,
            "verified",
            vec!["cargo test".to_string()],
            Some("qa-pane"),
            Some("agent"),
        );

        let done_ready: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(done_ready["phase"], "done");
        assert_eq!(done_ready["readiness"]["ready_for_done"], true);
    }

    #[test]
    fn test_feature_with_acceptance_starts_in_discovery() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);

        let result = add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);
        assert!(result.contains("F1.1"));

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.phase, FeaturePhase::Discovery);
        assert_eq!(feature.state, FeatureState::Active);
    }

    #[test]
    fn test_discovery_artifacts_auto_advance_to_build() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);

        add_question(path, "F1.1", "Blocking question?");
        upsert_feature_doc(path, "F1.1", "discovery", "# Discovery");

        let before: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(before["phase"], "discovery");

        answer_question(path, "F1.1", "Q1.1.1", "Answer", "Resolved", vec![]);

        let after: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(after["phase"], "build");
        assert_eq!(after["readiness"]["ready_for_build"], true);
    }

    #[test]
    fn test_discovery_ready_check_ignores_non_blocking_open_questions() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);

        upsert_feature_doc(path, "F1.1", "research", "# Research");
        add_question_with_blocking(path, "F1.1", "Optional follow-up?", false);

        let check: serde_json::Value =
            serde_json::from_str(&discovery_ready_check(path, "F1.1")).unwrap();
        assert_eq!(check["ready"], true);
        assert_eq!(check["checks"]["blocking_open_questions"], 0);
        assert_eq!(check["checks"]["non_blocking_open_questions"], 1);
        assert_eq!(check["blockers"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_discovery_ready_check_requires_artifact_and_blocking_resolution() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);

        add_question(path, "F1.1", "Blocking question?");
        let initial: serde_json::Value =
            serde_json::from_str(&discovery_ready_check(path, "F1.1")).unwrap();
        assert_eq!(initial["ready"], false);
        assert!(initial["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str() == Some("discovery artifact missing")));

        upsert_feature_doc(path, "F1.1", "discovery", "# Discovery");
        let with_doc: serde_json::Value =
            serde_json::from_str(&discovery_ready_check(path, "F1.1")).unwrap();
        assert_eq!(with_doc["ready"], false);
        assert_eq!(with_doc["checks"]["blocking_open_questions"], 1);

        answer_question(path, "F1.1", "Q1.1.1", "Answer", "Resolved", vec![]);
        let resolved: serde_json::Value =
            serde_json::from_str(&discovery_ready_check(path, "F1.1")).unwrap();
        assert_eq!(resolved["ready"], true);
    }

    #[test]
    fn test_complete_discovery_advances_only_when_ready() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec!["criterion".to_string()]);

        let blocked: serde_json::Value =
            serde_json::from_str(&complete_discovery(path, "F1.1")).unwrap();
        assert_eq!(blocked["status"], "blocked");

        upsert_feature_doc(path, "F1.1", "discovery", "# Discovery");
        let auto: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(auto["phase"], "build");

        let advanced: serde_json::Value =
            serde_json::from_str(&complete_discovery(path, "F1.1")).unwrap();
        assert_eq!(advanced["status"], "noop");
        assert_eq!(advanced["phase"], "build");

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.phase, FeaturePhase::Build);
        assert_eq!(feature.status, FeatureStatus::Building);
    }

    #[test]
    fn test_start_discovery_moves_planned_feature_once() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec![]);

        let started: serde_json::Value =
            serde_json::from_str(&start_discovery(path, "F1.1")).unwrap();
        assert_eq!(started["status"], "started");
        assert_eq!(started["phase"], "discovery");

        let again: serde_json::Value =
            serde_json::from_str(&start_discovery(path, "F1.1")).unwrap();
        assert_eq!(again["status"], "noop");

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.phase, FeaturePhase::Discovery);
        assert_eq!(feature.state, FeatureState::Active);
    }

    #[test]
    fn test_add_acceptance_criterion_starts_discovery_and_dedupes() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec![]);

        let added: serde_json::Value = serde_json::from_str(&add_acceptance_criterion(
            path,
            "F1.1",
            "Sub-second updates",
        ))
        .unwrap();
        assert_eq!(added["status"], "added");
        assert_eq!(added["phase"], "discovery");
        assert_eq!(added["count"], 1);

        let duplicate: serde_json::Value = serde_json::from_str(&add_acceptance_criterion(
            path,
            "F1.1",
            "Sub-second updates",
        ))
        .unwrap();
        assert_eq!(duplicate["status"], "noop");

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.acceptance_criteria.len(), 1);
        assert_eq!(feature.acceptance_items.len(), 1);
        assert_eq!(feature.phase, FeaturePhase::Discovery);
    }

    #[test]
    fn test_legacy_acceptance_strings_backfill_acceptance_items() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();

        let legacy = serde_json::json!({
            "project": "legacy-proj",
            "mission": "Legacy mission",
            "principles": [],
            "goals": [{
                "id": "G1",
                "title": "Goal",
                "description": "desc",
                "status": "planned",
                "priority": 1,
                "linked_issues": [],
                "metrics": []
            }],
            "milestones": [],
            "architecture": [],
            "changes": [],
            "features": [{
                "id": "F1.1",
                "goal_id": "G1",
                "title": "Legacy feature",
                "description": "desc",
                "status": "planned",
                "questions": [],
                "decisions": [],
                "tasks": [],
                "acceptance_criteria": ["Criterion A", "Criterion B"],
                "sub_vision": null,
                "parent_vision": null,
                "created_at": "2026-03-12T00:00:00Z",
                "updated_at": "2026-03-12T00:00:00Z"
            }],
            "github": {
                "repo": "",
                "sync_enabled": false,
                "wiki_page": null,
                "project_board": null,
                "labels": []
            },
            "updated_at": "2026-03-12T00:00:00Z"
        });

        std::fs::write(
            vision_file(path),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.acceptance_items.len(), 2);
        assert_eq!(feature.acceptance_items[0].id, "AC1.1.1");
        assert_eq!(feature.acceptance_items[0].text, "Criterion A");
    }

    #[test]
    fn test_acceptance_update_and_verify_are_provider_neutral() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(
            path,
            "G1",
            "Feature",
            "desc",
            vec!["Initial criterion".to_string()],
        );

        let updated: serde_json::Value = serde_json::from_str(&update_acceptance_criterion(
            path,
            "F1.1",
            "AC1.1.1",
            Some("Updated criterion"),
            Some("integration_test"),
        ))
        .unwrap();
        assert_eq!(updated["status"], "updated");
        assert_eq!(updated["criterion_status"], "mapped");

        let verified: serde_json::Value = serde_json::from_str(&verify_acceptance_criterion(
            path,
            "F1.1",
            "AC1.1.1",
            "verified",
            vec!["tests::integration::ws_streaming".to_string()],
            Some("gemini-worker"),
            Some("agent"),
        ))
        .unwrap();
        assert_eq!(verified["criterion_status"], "verified");
        assert_eq!(verified["verified_by"], "gemini-worker");
        assert_eq!(verified["verification_source"], "agent");

        let feature: serde_json::Value =
            serde_json::from_str(&feature_readiness(path, "F1.1")).unwrap();
        assert_eq!(feature["acceptance_items"][0]["text"], "Updated criterion");
        assert_eq!(feature["acceptance_items"][0]["status"], "verified");
    }

    #[test]
    fn test_upsert_feature_doc_creates_file_and_starts_discovery() {
        let dir = temp_project();
        let path = dir.path().to_str().unwrap();
        init_test_vision(dir.path());
        add_goal(path, "G1", "Goal", "desc", 1);
        add_feature(path, "G1", "Feature", "desc", vec![]);

        let result = upsert_feature_doc(path, "F1.1", "research", "# Notes");
        let json: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(json["status"], "created");
        assert_eq!(json["phase"], "discovery");
        assert_eq!(json["state"], "active");

        let doc_path = dir.path().join(".vision/research/F1.1.md");
        assert!(doc_path.exists());
        assert_eq!(std::fs::read_to_string(doc_path).unwrap(), "# Notes");

        let vision = load_vision(path).unwrap();
        let feature = vision.features.iter().find(|f| f.id == "F1.1").unwrap();
        assert_eq!(feature.phase, FeaturePhase::Discovery);
        assert_eq!(feature.status, FeatureStatus::Specifying);
    }

    #[test]
    fn test_milestone_in_progress_parses() {
        // This was the root cause bug — InProgress missing from MilestoneStatus
        let json = r#"{"status":"in_progress","id":"M1","title":"Test","description":"","target_date":"2026-01-01","goals":[]}"#;
        let m: Result<Milestone, _> = serde_json::from_str(json);
        assert!(
            m.is_ok(),
            "InProgress milestone should parse: {:?}",
            m.err()
        );
        assert!(matches!(m.unwrap().status, MilestoneStatus::InProgress));
    }
}

fn run_gh(cmd: &str) -> String {
    std::process::Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).to_string()
            } else {
                format!("error: {}", String::from_utf8_lossy(&o.stderr))
            }
        })
        .unwrap_or_else(|e| format!("exec error: {}", e))
}
