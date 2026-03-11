//! Capacity tools: configure, estimate, log work, sprint planning, dashboard, burndown, velocity, roles.
//!
//! Thin wrappers over crate::capacity so all layers route through one place.

use super::super::types::*;

/// Configure capacity parameters
pub fn cap_configure(req: &CapConfigureRequest) -> String {
    crate::capacity::cap_configure(
        req.pane_count,
        req.hours_per_day,
        req.availability_factor,
        req.review_bandwidth,
        req.build_slots,
    )
    .to_string()
}

/// Estimate ACU for a task
pub fn cap_estimate(req: &CapEstimateRequest) -> String {
    crate::capacity::cap_estimate(
        &req.description,
        &req.complexity.clone().unwrap_or_default(),
        &req.task_type.clone().unwrap_or_default(),
        &req.role.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Log work done on an issue
pub fn cap_log_work(req: &CapLogWorkRequest) -> String {
    crate::capacity::cap_log_work_full(
        &req.issue_id,
        &req.space,
        &req.role,
        &req.pane_id.clone().unwrap_or_default(),
        req.acu_spent,
        req.review_needed.unwrap_or(false),
        &req.notes.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Plan a sprint
pub fn cap_plan_sprint(req: &CapPlanSprintRequest) -> String {
    crate::capacity::cap_plan_sprint(
        &req.space,
        &req.name.clone().unwrap_or_default(),
        &req.start_date.clone().unwrap_or_default(),
        req.days.unwrap_or(5),
        &req.issue_ids.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Capacity dashboard
pub fn cap_dashboard(req: &CapDashboardRequest) -> String {
    crate::capacity::cap_dashboard(
        &req.space.clone().unwrap_or_default(),
        &req.sprint_id.clone().unwrap_or_default(),
    )
    .to_string()
}

/// Sprint burndown chart
pub fn cap_burndown(sprint_id: &str) -> String {
    crate::capacity::cap_burndown(sprint_id).to_string()
}

/// Sprint velocity
pub fn cap_velocity(req: &CapVelocityRequest) -> String {
    crate::capacity::cap_velocity(
        &req.space.clone().unwrap_or_default(),
        req.count.unwrap_or(5),
    )
    .to_string()
}

/// List roles with utilization
pub fn cap_roles() -> String {
    crate::capacity::cap_roles().to_string()
}
