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
    pub field: String,      // what changed: "mission", "goal:G1", "milestone:M2", etc.
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
    pub repo: String,          // "owner/repo"
    pub sync_enabled: bool,
    pub wiki_page: Option<String>,
    pub project_board: Option<u64>,
    #[serde(default)]
    pub labels: Vec<String>,   // labels to apply to vision-related issues
}

// ─── VDD: Feature/Question/Decision/Task ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,                        // "F1.1"
    pub goal_id: String,                   // links to parent goal
    pub title: String,
    pub description: String,
    pub status: FeatureStatus,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub decisions: Vec<VisionDecision>,
    #[serde(default)]
    pub tasks: Vec<VisionTask>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub sub_vision: Option<String>,        // path to sub-vision file (recursive)
    #[serde(default)]
    pub parent_vision: Option<String>,     // link up the tree
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,                        // "Q1.1.1"
    pub text: String,
    pub status: QuestionStatus,
    #[serde(default)]
    pub answer: Option<String>,
    pub asked_at: String,
    #[serde(default)]
    pub answered_at: Option<String>,
    #[serde(default)]
    pub decision_id: Option<String>,       // links to the decision it produced
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
    pub id: String,                        // "D1.1.1"
    #[serde(default)]
    pub question_id: Option<String>,       // which question this answers
    pub decision: String,
    pub rationale: String,
    pub date: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionTask {
    pub id: String,                        // "T1.1.1"
    pub feature_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub branch: Option<String>,            // Git branch name
    #[serde(default)]
    pub pr: Option<String>,                // PR number/URL
    #[serde(default)]
    pub commit: Option<String>,            // merge commit
    #[serde(default)]
    pub assignee: Option<String>,          // pane ID or agent name
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
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("vision: parse error for {}: {}", path.display(), e);
            None
        }
    }
}

pub fn save_vision(project_path: &str, vision: &Vision) -> Result<(), String> {
    let dir = vision_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {}", e))?;

    let json = serde_json::to_string_pretty(vision)
        .map_err(|e| format!("serialize: {}", e))?;
    std::fs::write(vision_file(project_path), json)
        .map_err(|e| format!("write: {}", e))?;

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
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
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
        }).to_string();
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
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

pub fn get_vision(project_path: &str) -> String {
    match load_vision(project_path) {
        Some(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string()),
        None => serde_json::json!({
            "error": "no_vision",
            "hint": "Run vision_init to create a vision for this project"
        }).to_string(),
    }
}

pub fn add_goal(project_path: &str, id: &str, title: &str, description: &str, priority: u8) -> String {
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
    project_path: &str, id: &str, title: &str, description: &str,
    target_date: Option<&str>, goal_ids: Vec<String>,
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
    project_path: &str, id: &str, title: &str, decision: &str,
    rationale: &str, alternatives: Vec<String>,
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

pub fn update_goal_status(project_path: &str, goal_id: &str, new_status: &str, reason: &str) -> String {
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
        Ok(()) => serde_json::json!({"status": "updated", "goal": goal_id, "new_status": new_status}).to_string(),
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
        change_type: if reason.contains("pivot") { ChangeType::Pivot } else { ChangeType::Modified },
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
    let active_milestones: Vec<_> = vision.milestones.iter()
        .filter(|m| m.status == MilestoneStatus::Active)
        .map(|m| serde_json::json!({
            "id": m.id, "title": m.title, "progress": m.progress_pct,
            "target": m.target_date,
        }))
        .collect();

    let recent_changes: Vec<_> = vision.changes.iter().rev().take(5)
        .map(|c| serde_json::json!({
            "time": c.timestamp, "field": c.field,
            "type": c.change_type, "reason": c.reason,
        }))
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
    }).to_string()
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
    }).to_string()
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
        }).to_string();
    }

    let repo = &vision.github.repo;
    let mut results = vec![];

    // Sync milestones
    for ms in &vision.milestones {
        if ms.github_milestone.is_none() {
            let due = ms.target_date.as_deref().unwrap_or("");
            let cmd = format!(
                "gh api repos/{}/milestones -f title='{}' -f description='{}' -f state=open {}",
                repo, ms.title.replace('\'', "'\\''"),
                ms.description.replace('\'', "'\\''"),
                if due.is_empty() { String::new() } else { format!("-f due_on='{}T00:00:00Z'", due) }
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
                repo, goal.title.replace('\'', "'\\''"),
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
    }).to_string()
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
        md.push_str(&format!("| {} | {} | P{} | {:?} |\n", g.id, g.title, g.priority, g.status));
    }

    md.push_str("\n## Milestones\n\n");
    for m in &vision.milestones {
        md.push_str(&format!("### {} — {} ({:?})\n\n{}\n\nProgress: {}%\n\n",
            m.id, m.title, m.status, m.description, m.progress_pct));
    }

    if !vision.architecture.is_empty() {
        md.push_str("## Architecture Decisions\n\n");
        for a in &vision.architecture {
            md.push_str(&format!("### ADR-{}: {}\n\n**Decision:** {}\n\n**Rationale:** {}\n\n**Status:** {:?}\n\n",
                a.id, a.title, a.decision, a.rationale, a.status));
        }
    }

    if !vision.changes.is_empty() {
        md.push_str("## Recent Changes\n\n");
        for c in vision.changes.iter().rev().take(10) {
            md.push_str(&format!("- **{}** `{}` {:?}: {} → {} ({})\n",
                c.timestamp, c.field, c.change_type, c.old_value, c.new_value, c.reason));
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
    }).to_string()
}

// ─── VDD: Feature CRUD ──────────────────────────────────────────────────────

fn features_dir(project_path: &str) -> PathBuf {
    vision_dir(project_path).join("features")
}

/// Add a feature under a goal.
pub fn add_feature(
    project_path: &str, goal_id: &str, title: &str, description: &str,
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
    let existing = vision.features.iter().filter(|f| f.goal_id == goal_id).count();
    let feature_num = existing + 1;
    let id = format!("F{}.{}", goal_id.trim_start_matches('G'), feature_num);

    let feature = Feature {
        id: id.clone(),
        goal_id: goal_id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        status: FeatureStatus::Planned,
        questions: vec![],
        decisions: vec![],
        tasks: vec![],
        acceptance_criteria,
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
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Add a question to a feature.
pub fn add_question(project_path: &str, feature_id: &str, text: &str) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
    };

    let q_num = feature.questions.len() + 1;
    let id = format!("Q{}.{}", feature_id.trim_start_matches('F'), q_num);

    let question = Question {
        id: id.clone(),
        text: text.to_string(),
        status: QuestionStatus::Open,
        answer: None,
        asked_at: now(),
        answered_at: None,
        decision_id: None,
    };

    feature.questions.push(question);

    // Move to specifying if it was planned
    if feature.status == FeatureStatus::Planned {
        feature.status = FeatureStatus::Specifying;
    }
    feature.updated_at = now();
    vision.updated_at = now();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "question": id,
            "feature": feature_id,
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Answer a question and record a decision.
pub fn answer_question(
    project_path: &str, feature_id: &str, question_id: &str,
    answer: &str, rationale: &str, alternatives: Vec<String>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
    };

    let question = match feature.questions.iter_mut().find(|q| q.id == question_id) {
        Some(q) => q,
        None => return serde_json::json!({"error": "question_not_found", "id": question_id}).to_string(),
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
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Add a task to a feature.
pub fn add_task(
    project_path: &str, feature_id: &str, title: &str, description: &str,
    branch: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
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
    let all_answered = feature.questions.iter().all(|q| q.status == QuestionStatus::Answered);
    if (feature.status == FeatureStatus::Specifying || feature.status == FeatureStatus::Planned) && all_answered {
        feature.status = FeatureStatus::Building;
    }
    feature.updated_at = now();
    vision.updated_at = now();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "added",
            "task": id,
            "feature": feature_id,
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

/// Update task status, optionally linking a branch or PR.
pub fn update_task_status(
    project_path: &str, feature_id: &str, task_id: &str,
    new_status: &str, branch: Option<&str>, pr: Option<&str>, commit: Option<&str>,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
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
    if let Some(b) = branch { task.branch = Some(b.to_string()); }
    if let Some(p) = pr { task.pr = Some(p.to_string()); }
    if let Some(c) = commit { task.commit = Some(c.to_string()); }
    task.updated_at = now();

    // Auto-update feature status based on task completion
    let all_done = feature.tasks.iter().all(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified);
    let any_in_progress = feature.tasks.iter().any(|t| t.status == TaskStatus::InProgress);
    if all_done && !feature.tasks.is_empty() {
        feature.status = FeatureStatus::Testing;
    } else if any_in_progress && feature.status != FeatureStatus::Building {
        feature.status = FeatureStatus::Building;
    }
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
        }).to_string(),
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
                "sub_vision": f.sub_vision,
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
    }).to_string()
}

/// Create a sub-vision file for a feature (recursive vision).
pub fn create_sub_vision(
    project_path: &str, feature_id: &str, mission: &str,
) -> String {
    let mut vision = match load_vision(project_path) {
        Some(v) => v,
        None => return serde_json::json!({"error": "no_vision"}).to_string(),
    };

    let feature = match vision.features.iter_mut().find(|f| f.id == feature_id) {
        Some(f) => f,
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
    };

    if feature.sub_vision.is_some() {
        return serde_json::json!({"error": "sub_vision_exists", "feature": feature_id}).to_string();
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
        }).to_string(),
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
                    "open_questions": open_questions,
                    "tasks_done": done_tasks,
                    "tasks_total": total_tasks,
                    "progress": if total_tasks > 0 { (done_tasks as f64 / total_tasks as f64 * 100.0) as u8 } else { 0 },
                    "has_sub_vision": f.sub_vision.is_some(),
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
    let done_features = vision.features.iter().filter(|f| f.status == FeatureStatus::Done).count();
    let total_tasks: usize = vision.features.iter().map(|f| f.tasks.len()).sum();
    let done_tasks: usize = vision.features.iter()
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
    let mut scored: Vec<(&Goal, usize)> = vision.goals.iter().map(|g| {
        let goal_text = format!("{} {} {:?}", g.title, g.description, g.metrics).to_lowercase();
        let score = words.iter().filter(|w| goal_text.contains(*w)).count();
        (g, score)
    }).collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let best = scored.first();

    if let Some((goal, score)) = best {
        if *score > 0 {
            // Find existing features for this goal
            let existing_features: Vec<_> = vision.features.iter()
                .filter(|f| f.goal_id == goal.id)
                .map(|f| serde_json::json!({
                    "id": f.id, "title": f.title, "status": f.status,
                }))
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
            }).to_string();
        }
    }

    serde_json::json!({
        "matched": false,
        "suggestion": "No matching goal found. Consider creating a new goal first.",
        "goals": vision.goals.iter().map(|g| serde_json::json!({
            "id": g.id, "title": g.title, "status": g.status,
        })).collect::<Vec<_>>(),
        "description": description,
    }).to_string()
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
                    "gh api repos/{}/branches/{} --jq '.name' 2>/dev/null", repo, branch
                ));
                let branch_exists = !branch_check.trim().is_empty() && !branch_check.contains("error");

                // Check for open PR
                let pr_check = run_gh(&format!(
                    "gh pr list -R {} --head {} --json number,state --jq '.[0]' 2>/dev/null", repo, branch
                ));

                if pr_check.contains("\"state\":\"MERGED\"") || pr_check.contains("\"state\":\"merged\"") {
                    if task.status != TaskStatus::Done && task.status != TaskStatus::Verified {
                        task.status = TaskStatus::Done;
                        changed = true;
                    }
                } else if !pr_check.trim().is_empty() && !pr_check.contains("error") && pr_check.contains("number") {
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
            let all_done = feature.tasks.iter().all(|t| t.status == TaskStatus::Done || t.status == TaskStatus::Verified);
            let any_in_progress = feature.tasks.iter().any(|t| t.status == TaskStatus::InProgress);
            if all_done {
                feature.status = FeatureStatus::Testing;
            } else if any_in_progress {
                feature.status = FeatureStatus::Building;
            }
        }
    }

    vision.updated_at = now();
    let update_count = updates.len();

    match save_vision(project_path, &vision) {
        Ok(()) => serde_json::json!({
            "status": "synced",
            "updates": update_count,
            "details": updates,
        }).to_string(),
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
        None => return serde_json::json!({"error": "feature_not_found", "id": feature_id}).to_string(),
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
    feature.status = parsed;
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
        }).to_string(),
        Err(e) => serde_json::json!({"error": e}).to_string(),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
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
