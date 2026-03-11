//! Tracker tools: issue_create, issue_update, issue_close, feature_to_queue, board_view.
//!
//! Thin wrappers over crate::tracker so TUI/Web/MCP all route through one place.

use super::super::types::*;

/// Create an issue in a tracker space
pub fn issue_create(req: &IssueCreateRequest) -> String {
    let labels = req.labels.clone().unwrap_or_default();
    let result = crate::tracker::issue_create(
        &req.space,
        &req.title,
        req.issue_type.as_deref().unwrap_or("task"),
        req.priority.as_deref().unwrap_or("medium"),
        req.description.as_deref().unwrap_or(""),
        req.assignee.as_deref().unwrap_or(""),
        req.milestone.as_deref().unwrap_or(""),
        &labels,
        req.estimated_acu.unwrap_or(0.0),
        req.role.as_deref().unwrap_or(""),
        req.sprint.as_deref().unwrap_or(""),
        req.parent.as_deref().unwrap_or(""),
    );
    result.to_string()
}

/// Update an issue's fields (status, priority, assignee, etc.)
pub fn issue_update_full(req: &IssueUpdateFullRequest) -> String {
    let result = crate::tracker::issue_update_full(
        &req.space,
        &req.issue_id,
        req.status.as_deref().unwrap_or(""),
        req.priority.as_deref().unwrap_or(""),
        req.assignee.as_deref().unwrap_or(""),
        req.title.as_deref().unwrap_or(""),
        req.description.as_deref().unwrap_or(""),
        req.milestone.as_deref().unwrap_or(""),
        req.add_label.as_deref().unwrap_or(""),
        req.remove_label.as_deref().unwrap_or(""),
        req.estimated_acu.unwrap_or(0.0),
        req.actual_acu.unwrap_or(0.0),
        req.sprint.as_deref().unwrap_or(""),
        req.role.as_deref().unwrap_or(""),
    );
    result.to_string()
}

/// Push issues to execution queue
pub fn feature_to_queue(req: &FeatureToQueueRequest) -> String {
    let result = crate::tracker::feature_to_queue(
        &req.space,
        &req.issue_ids,
        req.sequential.unwrap_or(false),
    );
    result.to_string()
}

/// View the kanban board for a space
pub fn board_view(space: &str) -> String {
    let result = crate::tracker::board_view(space);
    result.to_string()
}

/// Close an issue with resolution
pub fn issue_close(space: &str, issue_id: &str, resolution: &str) -> String {
    let result = crate::tracker::issue_close(space, issue_id, resolution);
    result.to_string()
}

/// View issue details
pub fn issue_view(space: &str, issue_id: &str) -> String {
    let result = crate::tracker::issue_view(space, issue_id);
    result.to_string()
}

/// List/filter issues
pub fn issue_list_filtered(req: &IssueListFilteredRequest) -> String {
    let result = crate::tracker::issue_list_filtered(
        &req.space,
        req.status.as_deref().unwrap_or(""),
        req.issue_type.as_deref().unwrap_or(""),
        req.priority.as_deref().unwrap_or(""),
        req.assignee.as_deref().unwrap_or(""),
        req.milestone.as_deref().unwrap_or(""),
        req.label.as_deref().unwrap_or(""),
        req.sprint.as_deref().unwrap_or(""),
        req.role.as_deref().unwrap_or(""),
    );
    result.to_string()
}

/// Add a comment to an issue
pub fn issue_comment(space: &str, issue_id: &str, content: &str, author: &str) -> String {
    let result = crate::tracker::issue_comment(space, issue_id, content, author);
    result.to_string()
}

/// Link two issues
pub fn issue_link(space: &str, issue_id: &str, link_type: &str, reference: &str) -> String {
    let result = crate::tracker::issue_link(space, issue_id, link_type, reference);
    result.to_string()
}

/// Create a milestone
pub fn milestone_create(req: &MilestoneCreateRequest) -> String {
    let result = crate::tracker::milestone_create(
        &req.space,
        &req.name,
        req.description.as_deref().unwrap_or(""),
        req.due_date.as_deref().unwrap_or(""),
    );
    result.to_string()
}

/// List milestones
pub fn milestone_list(space: &str) -> String {
    let result = crate::tracker::milestone_list(space);
    result.to_string()
}

/// Generate timeline
pub fn timeline_generate(space: &str, milestone: &str) -> String {
    let result = crate::tracker::timeline_generate(space, milestone);
    result.to_string()
}

// ── Process Management ──

/// Start a process from a checklist template
pub fn process_start(req: &ProcessStartRequest) -> String {
    let ctx = req.context.clone().unwrap_or(serde_json::json!({}));
    crate::tracker::process_start(&req.space, &req.template_name, &ctx).to_string()
}

/// Update a process step
pub fn process_update(req: &ProcessUpdateRequest) -> String {
    crate::tracker::process_update(
        &req.space,
        &req.process_id,
        req.step_index,
        req.done.unwrap_or(true),
    )
    .to_string()
}

/// List processes in a space
pub fn process_list(space: &str) -> String {
    crate::tracker::process_list(space).to_string()
}

/// Create a checklist template
pub fn process_template_create(name: &str, content: &str) -> String {
    crate::tracker::process_template_create(name, content).to_string()
}

// ── Feature Management ──

/// List child issues of a parent
pub fn issue_children(space: &str, parent_id: &str) -> String {
    crate::tracker::issue_children(space, parent_id).to_string()
}

/// Decompose a feature into micro-features
pub fn feature_decompose(space: &str, parent_id: &str, children: &[serde_json::Value]) -> String {
    crate::tracker::feature_decompose(space, parent_id, children).to_string()
}

/// Hierarchical feature status
pub fn feature_status(space: &str, feature_id: &str) -> String {
    crate::tracker::feature_status(space, feature_id).to_string()
}
