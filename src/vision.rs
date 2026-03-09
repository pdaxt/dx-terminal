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
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
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
