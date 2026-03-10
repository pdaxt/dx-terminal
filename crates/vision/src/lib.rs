//! # dx-vision — Vision-Driven Development Framework
//!
//! Recursive vision trees for any project. Goals → Features → Questions → Tasks.
//! Everything traces back to the vision. Features can have sub-visions (infinite depth).
//!
//! ## Quick Start
//! ```rust,no_run
//! use dx_vision::{Vision, VisionStore};
//!
//! let store = VisionStore::new("/path/to/project");
//! let mut vision = store.load().unwrap_or_else(|_| Vision::new("my-project", "Build amazing things"));
//!
//! vision.add_goal("G1", "Core Engine", "Build the core", 1);
//! vision.add_feature("G1", "F1.1", "REST API", "HTTP endpoints", vec!["All CRUD ops work".into()]);
//! vision.add_question("F1.1", "Q1", "REST or GraphQL?");
//! vision.answer_question("F1.1", "Q1", "REST", "Simpler", vec!["GraphQL".into()]);
//! vision.add_task("F1.1", "T1", "Implement GET /items", "", Some("feat/get-items"));
//! vision.update_task_status("F1.1", "T1", "done", None, None, None);
//! store.save(&vision).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ─── Core Types ─────────────────────────────────────────────────────────────

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
    pub priority: u8,
    #[serde(default)]
    pub linked_issues: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus { Planned, InProgress, Achieved, Deferred, Dropped }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub goal_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
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
    pub sub_vision: Option<String>,
    #[serde(default)]
    pub parent_vision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus { #[default] Planned, Specifying, Building, Testing, Done }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub status: QuestionStatus,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub asked_at: String,
    #[serde(default)]
    pub answered_at: Option<String>,
    #[serde(default)]
    pub decision_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus { #[default] Open, Answered, Revised }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionDecision {
    pub id: String,
    #[serde(default)]
    pub question_id: Option<String>,
    pub decision: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionTask {
    pub id: String,
    pub feature_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub pr: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { #[default] Planned, InProgress, Done, Verified, Blocked }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: MilestoneStatus,
    #[serde(default)]
    pub target_date: Option<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub github_milestone: Option<u64>,
    #[serde(default)]
    pub progress_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus { Upcoming, Active, Complete, Missed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchDecision {
    pub id: String,
    pub title: String,
    pub decision: String,
    pub rationale: String,
    pub date: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionChange {
    #[serde(rename = "type")]
    pub change_type: String,
    pub field: String,
    pub reason: String,
    pub time: String,
    #[serde(default)]
    pub old_value: Option<String>,
    #[serde(default)]
    pub new_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubConfig {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub sync: bool,
    #[serde(default)]
    pub issues_label: String,
}

#[derive(Debug, Clone)]
pub struct AssessResult {
    pub goal_id: String,
    pub goal_title: String,
    pub features: Vec<String>,
    pub score: usize,
}

// ─── Vision Methods ─────────────────────────────────────────────────────────

impl Vision {
    pub fn new(project: &str, mission: &str) -> Self {
        Self {
            project: project.into(), mission: mission.into(),
            principles: vec![], goals: vec![], milestones: vec![],
            architecture: vec![], changes: vec![], features: vec![],
            github: GitHubConfig::default(), updated_at: now(),
        }
    }

    pub fn add_goal(&mut self, id: &str, title: &str, description: &str, priority: u8) {
        self.goals.push(Goal {
            id: id.into(), title: title.into(), description: description.into(),
            status: GoalStatus::Planned, priority, linked_issues: vec![], metrics: vec![],
        });
        self.log_change("added", "goal", &format!("Added goal {}: {}", id, title));
    }

    pub fn add_feature(&mut self, goal_id: &str, id: &str, title: &str, description: &str, criteria: Vec<String>) -> Result<(), String> {
        if !self.goals.iter().any(|g| g.id == goal_id) { return Err(format!("Goal {} not found", goal_id)); }
        if self.features.iter().any(|f| f.id == id) { return Err(format!("Feature {} exists", id)); }
        self.features.push(Feature {
            id: id.into(), goal_id: goal_id.into(), title: title.into(),
            description: description.into(), status: FeatureStatus::Planned,
            questions: vec![], decisions: vec![], tasks: vec![],
            acceptance_criteria: criteria, sub_vision: None, parent_vision: None,
        });
        self.log_change("added", "feature", &format!("{}: {}", id, title));
        Ok(())
    }

    pub fn add_question(&mut self, feature_id: &str, id: &str, text: &str) -> Result<(), String> {
        let f = self.features.iter_mut().find(|f| f.id == feature_id)
            .ok_or_else(|| format!("Feature {} not found", feature_id))?;
        f.questions.push(Question {
            id: id.into(), text: text.into(), status: QuestionStatus::Open,
            answer: None, asked_at: now(), answered_at: None, decision_id: None,
        });
        f.status = FeatureStatus::Specifying;
        self.log_change("added", "question", &format!("{} on {}", id, feature_id));
        Ok(())
    }

    pub fn answer_question(&mut self, feature_id: &str, qid: &str, answer: &str, rationale: &str, alts: Vec<String>) -> Result<(), String> {
        let f = self.features.iter_mut().find(|f| f.id == feature_id)
            .ok_or_else(|| format!("Feature {} not found", feature_id))?;
        let q = f.questions.iter_mut().find(|q| q.id == qid)
            .ok_or_else(|| format!("Question {} not found", qid))?;
        let did = format!("D{}", qid.trim_start_matches('Q'));
        q.status = QuestionStatus::Answered;
        q.answer = Some(answer.into());
        q.answered_at = Some(now());
        q.decision_id = Some(did.clone());
        f.decisions.push(VisionDecision {
            id: did, question_id: Some(qid.into()), decision: answer.into(),
            rationale: rationale.into(), date: now(), alternatives: alts,
        });
        if f.questions.iter().all(|q| q.status == QuestionStatus::Answered) && !f.tasks.is_empty() {
            f.status = FeatureStatus::Building;
        }
        self.log_change("answered", "question", &format!("{} on {}", qid, feature_id));
        Ok(())
    }

    pub fn add_task(&mut self, feature_id: &str, id: &str, title: &str, desc: &str, branch: Option<&str>) -> Result<(), String> {
        let f = self.features.iter_mut().find(|f| f.id == feature_id)
            .ok_or_else(|| format!("Feature {} not found", feature_id))?;
        f.tasks.push(VisionTask {
            id: id.into(), feature_id: feature_id.into(), title: title.into(),
            description: desc.into(), status: TaskStatus::Planned,
            branch: branch.map(Into::into), pr: None, commit: None, assignee: None,
        });
        if f.questions.is_empty() || f.questions.iter().all(|q| q.status == QuestionStatus::Answered) {
            f.status = FeatureStatus::Building;
        }
        self.log_change("added", "task", &format!("{}: {}", id, title));
        Ok(())
    }

    pub fn update_task_status(&mut self, feature_id: &str, task_id: &str, status: &str, branch: Option<&str>, pr: Option<&str>, commit: Option<&str>) -> Result<(), String> {
        let f = self.features.iter_mut().find(|f| f.id == feature_id)
            .ok_or_else(|| format!("Feature {} not found", feature_id))?;
        let t = f.tasks.iter_mut().find(|t| t.id == task_id)
            .ok_or_else(|| format!("Task {} not found", task_id))?;
        t.status = match status {
            "planned" => TaskStatus::Planned, "in_progress" => TaskStatus::InProgress,
            "done" => TaskStatus::Done, "verified" => TaskStatus::Verified,
            "blocked" => TaskStatus::Blocked, _ => return Err(format!("Invalid status: {}", status)),
        };
        if let Some(b) = branch { t.branch = Some(b.into()); }
        if let Some(p) = pr { t.pr = Some(p.into()); }
        if let Some(c) = commit { t.commit = Some(c.into()); }
        // Cascade
        let all_done = f.tasks.iter().all(|t| matches!(t.status, TaskStatus::Done | TaskStatus::Verified));
        let any_ip = f.tasks.iter().any(|t| t.status == TaskStatus::InProgress);
        if all_done && !f.tasks.is_empty() { f.status = FeatureStatus::Testing; }
        else if any_ip { f.status = FeatureStatus::Building; }
        self.log_change("status_change", "task", &format!("{} → {}", task_id, status));
        Ok(())
    }

    pub fn tree(&self) -> serde_json::Value {
        let goals_json: Vec<serde_json::Value> = self.goals.iter().map(|goal| {
            let features: Vec<&Feature> = self.features.iter().filter(|f| f.goal_id == goal.id).collect();
            let total: usize = features.iter().map(|f| f.tasks.len()).sum();
            let done: usize = features.iter().map(|f| f.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Done | TaskStatus::Verified)).count()).sum();
            let pct = if total > 0 { (done * 100) / total } else { 0 };
            let fj: Vec<serde_json::Value> = features.iter().map(|f| {
                let fd = f.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Done | TaskStatus::Verified)).count();
                let ft = f.tasks.len();
                let fp = if ft > 0 { (fd * 100) / ft } else { 0 };
                let oq = f.questions.iter().filter(|q| q.status == QuestionStatus::Open).count();
                serde_json::json!({"id":f.id,"title":f.title,"status":f.status,"tasks_done":fd,"tasks_total":ft,"progress":fp,"open_questions":oq,"has_sub_vision":f.sub_vision.is_some(),"tasks":f.tasks})
            }).collect();
            serde_json::json!({"id":goal.id,"title":goal.title,"status":goal.status,"progress":pct,"features":fj})
        }).collect();
        let total: usize = self.features.iter().map(|f| f.tasks.len()).sum();
        let done: usize = self.features.iter().map(|f| f.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Done | TaskStatus::Verified)).count()).sum();
        let overall = if total > 0 { (done * 100) / total } else { 0 };
        serde_json::json!({"project":self.project,"mission":self.mission,"goals":goals_json,"github":self.github,
            "summary":{"goals_total":self.goals.len(),"goals_achieved":self.goals.iter().filter(|g|g.status==GoalStatus::Achieved).count(),
            "features_total":self.features.len(),"tasks_total":total,"tasks_done":done,"overall_progress":overall}})
    }

    pub fn assess(&self, description: &str) -> Option<AssessResult> {
        let dl = description.to_lowercase();
        let words: Vec<&str> = dl.split_whitespace().collect();
        let mut best: Option<(&Goal, usize)> = None;
        for g in &self.goals {
            let text = format!("{} {} {}", g.title, g.description, g.metrics.join(" ")).to_lowercase();
            let score: usize = words.iter().filter(|w| w.len() > 3 && text.contains(*w)).count();
            if score > best.map(|(_, s)| s).unwrap_or(0) { best = Some((g, score)); }
        }
        best.map(|(g, s)| AssessResult {
            goal_id: g.id.clone(), goal_title: g.title.clone(),
            features: self.features.iter().filter(|f| f.goal_id == g.id).map(|f| f.id.clone()).collect(),
            score: s,
        })
    }

    pub fn drill(&self, goal_id: &str) -> Option<serde_json::Value> {
        let g = self.goals.iter().find(|g| g.id == goal_id)?;
        let fs: Vec<&Feature> = self.features.iter().filter(|f| f.goal_id == goal_id).collect();
        Some(serde_json::json!({"goal": g, "features": fs}))
    }

    fn log_change(&mut self, ct: &str, field: &str, reason: &str) {
        self.changes.push(VisionChange {
            change_type: ct.into(), field: field.into(), reason: reason.into(),
            time: now(), old_value: None, new_value: None,
        });
        self.updated_at = now();
    }
}

// ─── Vision Store ───────────────────────────────────────────────────────────

pub struct VisionStore { root: PathBuf }

impl VisionStore {
    pub fn new(root: impl AsRef<Path>) -> Self { Self { root: root.as_ref().into() } }
    pub fn vision_dir(&self) -> PathBuf { self.root.join(".vision") }
    pub fn vision_file(&self) -> PathBuf { self.vision_dir().join("vision.json") }

    pub fn load(&self) -> Result<Vision, String> {
        let p = self.vision_file();
        let c = fs::read_to_string(&p).map_err(|e| format!("Read {}: {}", p.display(), e))?;
        serde_json::from_str(&c).map_err(|e| format!("Parse: {}", e))
    }

    pub fn save(&self, v: &Vision) -> Result<(), String> {
        let d = self.vision_dir();
        fs::create_dir_all(&d).map_err(|e| format!("Mkdir {}: {}", d.display(), e))?;
        let c = serde_json::to_string_pretty(v).map_err(|e| format!("Serialize: {}", e))?;
        fs::write(self.vision_file(), c).map_err(|e| format!("Write: {}", e))
    }

    pub fn init(&self, project: &str, mission: &str) -> Result<Vision, String> {
        if self.vision_file().exists() { return Err("Vision already exists".into()); }
        let v = Vision::new(project, mission);
        self.save(&v)?;
        Ok(v)
    }

    pub fn create_sub_vision(&self, vision: &mut Vision, feature_id: &str, mission: &str) -> Result<(), String> {
        let f = vision.features.iter_mut().find(|f| f.id == feature_id)
            .ok_or_else(|| format!("Feature {} not found", feature_id))?;
        let sub_path = format!("features/{}.json", f.goal_id);
        let full = self.vision_dir().join(&sub_path);
        fs::create_dir_all(full.parent().unwrap()).map_err(|e| format!("Mkdir: {}", e))?;
        let sv = Vision::new(&vision.project, mission);
        fs::write(&full, serde_json::to_string_pretty(&sv).unwrap()).map_err(|e| format!("Write: {}", e))?;
        f.sub_vision = Some(sub_path);
        f.parent_vision = Some("vision.json".into());
        Ok(())
    }

    pub fn scan_projects(dir: impl AsRef<Path>) -> Vec<(PathBuf, Vision)> {
        let mut r = vec![];
        if let Ok(entries) = fs::read_dir(dir.as_ref()) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let s = VisionStore::new(&p);
                    if let Ok(v) = s.load() { r.push((p, v)); }
                }
            }
        }
        r
    }
}

fn now() -> String { chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_lifecycle() {
        let dir = std::env::temp_dir().join(format!("dx-vision-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let store = VisionStore::new(&dir);
        let mut v = store.init("test", "Build great things").unwrap();
        v.add_goal("G1", "Core", "Core engine", 1);
        v.add_feature("G1", "F1", "API", "REST API", vec!["CRUD".into()]).unwrap();
        v.add_question("F1", "Q1", "REST or GraphQL?").unwrap();
        assert_eq!(v.features[0].status, FeatureStatus::Specifying);
        v.answer_question("F1", "Q1", "REST", "Simple", vec![]).unwrap();
        v.add_task("F1", "T1", "GET", "", Some("feat/get")).unwrap();
        assert_eq!(v.features[0].status, FeatureStatus::Building);
        v.add_task("F1", "T2", "POST", "", None).unwrap();
        v.update_task_status("F1", "T1", "done", None, None, None).unwrap();
        assert_eq!(v.features[0].status, FeatureStatus::Building); // T2 still pending
        v.update_task_status("F1", "T2", "done", None, None, None).unwrap();
        assert_eq!(v.features[0].status, FeatureStatus::Testing);
        store.save(&v).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.features[0].tasks.len(), 2);
        let tree = loaded.tree();
        assert_eq!(tree["summary"]["overall_progress"], 100);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backward_compat() {
        let json = r#"{"project":"old","mission":"test","principles":[],"goals":[],"milestones":[],"architecture":[],"changes":[]}"#;
        let v: Vision = serde_json::from_str(json).unwrap();
        assert!(v.features.is_empty());
    }
}
