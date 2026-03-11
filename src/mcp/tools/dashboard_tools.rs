//! Dashboard tools: overview, agent detail, project view, leaderboard, timeline, alerts, digest, export.
//!
//! Thin wrappers over crate::dashboard so all layers route through one place.

use super::super::types::*;

/// God view: agents, tasks, locks, ports, quality, recent activity
pub fn dash_overview(project: Option<&str>) -> String {
    crate::dashboard::dash_overview(project).to_string()
}

/// Deep dive on one agent
pub fn dash_agent_detail(pane_id: &str) -> String {
    crate::dashboard::dash_agent_detail(pane_id).to_string()
}

/// Project view: agents, tasks, quality, commits, knowledge
pub fn dash_project(project: &str) -> String {
    crate::dashboard::dash_project(project).to_string()
}

/// Agent leaderboard
pub fn dash_leaderboard(days: i64, project: Option<&str>) -> String {
    crate::dashboard::dash_leaderboard(days, project).to_string()
}

/// Chronological event stream
pub fn dash_timeline(req: &DashTimelineRequest) -> String {
    crate::dashboard::dash_timeline(
        req.project.as_deref(),
        req.pane_id.as_deref(),
        req.limit.unwrap_or(50),
    )
    .to_string()
}

/// Alerts: dead agents, high error rates, failed tests
pub fn dash_alerts(project: Option<&str>) -> String {
    crate::dashboard::dash_alerts(project).to_string()
}

/// 24h summary
pub fn dash_daily_digest(project: Option<&str>) -> String {
    crate::dashboard::dash_daily_digest(project).to_string()
}

/// JSON data export
pub fn dash_export(report: &str, project: Option<&str>, days: i64) -> String {
    crate::dashboard::dash_export(report, project, days).to_string()
}
