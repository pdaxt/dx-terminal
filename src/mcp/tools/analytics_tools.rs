//! Analytics tools: tool calls, file ops, tokens, git commits, usage reports, rankings, health, trends.
//!
//! Thin wrappers over crate::analytics so all layers route through one place.

use super::super::types::*;

/// Log a tool call for analytics
pub fn log_tool_call(req: &LogToolCallRequest) -> String {
    crate::analytics::log_tool_call(
        &req.pane_id,
        &req.tool_name,
        req.input_size.unwrap_or(0),
        req.output_size.unwrap_or(0),
        req.latency_ms,
        req.success.unwrap_or(true),
        req.error_preview.as_deref(),
    )
    .to_string()
}

/// Log a file operation
pub fn log_file_op(req: &LogFileOpRequest) -> String {
    crate::analytics::log_file_op(
        &req.pane_id,
        &req.file_path,
        &req.operation,
        req.lines_changed,
    )
    .to_string()
}

/// Log token usage
pub fn log_tokens(req: &LogTokensRequest) -> String {
    crate::analytics::log_tokens(
        &req.pane_id,
        &req.model,
        req.input_tokens,
        req.output_tokens,
        req.cache_read.unwrap_or(0),
        req.cache_write.unwrap_or(0),
    )
    .to_string()
}

/// Log a git commit
pub fn log_git_commit(req: &LogGitCommitRequest) -> String {
    crate::analytics::log_git_commit(
        &req.pane_id,
        &req.project,
        &req.repo_path.clone().unwrap_or_default(),
        &req.commit_hash,
        &req.branch.clone().unwrap_or_default(),
        &req.message,
        req.files_changed.unwrap_or(0),
        req.insertions.unwrap_or(0),
        req.deletions.unwrap_or(0),
    )
    .to_string()
}

/// Usage report over N days
pub fn usage_report(req: &UsageReportRequest) -> String {
    crate::analytics::usage_report(
        req.pane_id.as_deref(),
        req.project.as_deref(),
        req.days.unwrap_or(7),
    )
    .to_string()
}

/// Tool ranking by usage and error rate
pub fn tool_ranking(req: &ToolRankingRequest) -> String {
    crate::analytics::tool_ranking(
        req.project.as_deref(),
        req.days.unwrap_or(7),
        req.limit.unwrap_or(20),
    )
    .to_string()
}

/// MCP server health by error rates
pub fn mcp_health(days: i64) -> String {
    crate::analytics::mcp_health(days).to_string()
}

/// Agent activity feed
pub fn agent_activity(pane_id: &str, limit: i64) -> String {
    crate::analytics::agent_activity(pane_id, limit).to_string()
}

/// Token cost report
pub fn cost_report(req: &CostReportRequest) -> String {
    crate::analytics::cost_report(req.project.as_deref(), req.days.unwrap_or(30)).to_string()
}

/// Time-series trends
pub fn trends(req: &TrendsRequest) -> String {
    crate::analytics::trends(
        &req.metric,
        req.project.as_deref(),
        &req.granularity.clone().unwrap_or_else(|| "daily".into()),
        req.periods.unwrap_or(30),
    )
    .to_string()
}
