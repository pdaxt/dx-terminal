//! Vision-Driven Development MCP tools.
//! Thin wrappers around vision.rs CRUD functions.

use crate::vision;

fn resolve_project(project: Option<&str>) -> String {
    project.unwrap_or(".").to_string()
}

pub fn vision_tree(project: Option<&str>) -> String {
    vision::vision_tree(&resolve_project(project))
}

pub fn vision_drill(project: Option<&str>, goal_id: &str) -> String {
    vision::drill_down(&resolve_project(project), goal_id)
}

pub fn vision_work(project: Option<&str>, description: &str) -> String {
    vision::assess_work(&resolve_project(project), description)
}

pub fn vision_add_feature(
    project: Option<&str>, goal_id: &str, title: &str, description: &str,
    acceptance_criteria: Vec<String>,
) -> String {
    vision::add_feature(&resolve_project(project), goal_id, title, description, acceptance_criteria)
}

pub fn vision_add_question(project: Option<&str>, feature_id: &str, question: &str) -> String {
    vision::add_question(&resolve_project(project), feature_id, question)
}

pub fn vision_research_doc_upsert(project: Option<&str>, feature_id: &str, content: &str) -> String {
    vision::upsert_feature_doc(&resolve_project(project), feature_id, "research", content)
}

pub fn vision_discovery_doc_upsert(project: Option<&str>, feature_id: &str, content: &str) -> String {
    vision::upsert_feature_doc(&resolve_project(project), feature_id, "discovery", content)
}

pub fn vision_answer(
    project: Option<&str>, feature_id: &str, question_id: &str,
    answer: &str, rationale: &str, alternatives: Vec<String>,
) -> String {
    vision::answer_question(&resolve_project(project), feature_id, question_id, answer, rationale, alternatives)
}

pub fn vision_add_task(
    project: Option<&str>, feature_id: &str, title: &str, description: &str,
    branch: Option<&str>,
) -> String {
    vision::add_task(&resolve_project(project), feature_id, title, description, branch)
}

pub fn vision_update_task(
    project: Option<&str>, feature_id: &str, task_id: &str,
    status: &str, branch: Option<&str>, pr: Option<&str>, commit: Option<&str>,
) -> String {
    vision::update_task_status(&resolve_project(project), feature_id, task_id, status, branch, pr, commit)
}

pub fn vision_update_feature(project: Option<&str>, feature_id: &str, status: &str) -> String {
    vision::update_feature_status(&resolve_project(project), feature_id, status)
}

pub fn vision_feature_readiness(project: Option<&str>, feature_id: &str) -> String {
    vision::feature_readiness(&resolve_project(project), feature_id)
}

pub fn vision_sync(project: Option<&str>) -> String {
    vision::sync_git_status(&resolve_project(project))
}

pub fn vision_init(project: &str, name: &str, mission: &str, repo: &str) -> String {
    vision::init_vision(project, name, mission, repo)
}

pub fn vision_add_goal(project: Option<&str>, id: &str, title: &str, description: &str, priority: u8) -> String {
    vision::add_goal(&resolve_project(project), id, title, description, priority)
}

pub fn vision_update_goal(project: Option<&str>, goal_id: &str, status: &str) -> String {
    vision::update_goal_status(&resolve_project(project), goal_id, status, &format!("Status changed to {}", status))
}
