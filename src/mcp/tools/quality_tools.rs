//! Quality tools: test, build, lint, deploy logging; reports, gates, regressions, health scores.
//!
//! Thin wrappers over crate::quality so all layers route through one place.

use super::super::types::*;

/// Log test results
pub fn log_test(req: &LogTestRequest) -> String {
    crate::quality::log_test(
        &req.pane_id, &req.project, req.command.as_deref(), req.success,
        req.total, req.passed, req.failed, req.skipped, req.duration_ms, req.output.as_deref(),
    ).to_string()
}

/// Log build result
pub fn log_build(req: &LogBuildRequest) -> String {
    crate::quality::log_build(
        &req.pane_id, &req.project, req.command.as_deref(), req.success,
        req.duration_ms, req.output.as_deref(),
    ).to_string()
}

/// Log lint results
pub fn log_lint(req: &LogLintRequest) -> String {
    crate::quality::log_lint(
        &req.pane_id, &req.project, req.command.as_deref(), req.success,
        req.total, req.errors, req.warnings, req.output.as_deref(),
    ).to_string()
}

/// Log deployment result
pub fn log_deploy(req: &LogDeployRequest) -> String {
    crate::quality::log_deploy(
        &req.pane_id, &req.project, req.target.as_deref(), req.success,
        req.duration_ms, req.output.as_deref(),
    ).to_string()
}

/// Quality report over N days
pub fn quality_report(project: &str, days: i64) -> String {
    crate::quality::quality_report(project, days).to_string()
}

/// Quality gate: PASS/FAIL
pub fn quality_gate(project: &str) -> String {
    crate::quality::quality_gate(project).to_string()
}

/// Detect regressions
pub fn regressions(project: &str, days: i64) -> String {
    crate::quality::regressions(project, days).to_string()
}

/// Project health score (0-100)
pub fn project_health(project: &str) -> String {
    crate::quality::project_health(project).to_string()
}
