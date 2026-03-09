//! DX Terminal Intel: Analytics, knowledge graph, session replay, truthguard, quality, dashboards.
//! 49 tools.

use std::sync::Arc;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use crate::app::App;
use crate::mcp::types::*;
use crate::mcp::tools;

#[derive(Clone)]
pub struct DxIntelService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DxIntelService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === KNOWLEDGE GRAPH TOOLS (8 tools) ===

    #[tool(description = "Add an entity to the knowledge graph. Upserts by ID. Types: project, file, tool, pattern, error, person, concept, mcp, library, platform, config, service, database.")]
    async fn kgraph_add_entity(
        &self,
        Parameters(req): Parameters<KgraphAddEntityRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_add_entity(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add a typed edge between two entities. Relations: uses, depends_on, causes, fixes, part_of, related_to, etc.")]
    async fn kgraph_add_edge(
        &self,
        Parameters(req): Parameters<KgraphAddEdgeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_add_edge(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Record an observation on an edge. Auto-creates entities and edges. Adjusts weight by impact.")]
    async fn kgraph_observe(
        &self,
        Parameters(req): Parameters<KgraphObserveRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_observe(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Query neighbors of an entity via BFS traversal. Returns subgraph with nodes and edges.")]
    async fn kgraph_query_neighbors(
        &self,
        Parameters(req): Parameters<KgraphQueryNeighborsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_query_neighbors(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Find shortest path between two entities in the knowledge graph.")]
    async fn kgraph_query_path(
        &self,
        Parameters(req): Parameters<KgraphQueryPathRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_query_path(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search entities by name or properties. Filter by type.")]
    async fn kgraph_search(
        &self,
        Parameters(req): Parameters<KgraphSearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_search(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Delete an entity (cascades edges) or a specific edge.")]
    async fn kgraph_delete(
        &self,
        Parameters(req): Parameters<KgraphDeleteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_delete(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Knowledge graph statistics: entity count, edge count, observations, breakdowns by type and relation.")]
    async fn kgraph_stats(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::kgraph_stats();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === SESSION REPLAY TOOLS (7 tools) ===

    #[tool(description = "Index Claude Code session JSONL files into searchable database. Incremental by default.")]
    async fn replay_index(
        &self,
        Parameters(req): Parameters<ReplayIndexRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_index(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search across all indexed sessions for content matches. Filter by project, tool, time range.")]
    async fn replay_search(
        &self,
        Parameters(req): Parameters<ReplaySearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_search(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Retrieve full session turns. Filter tool results and errors.")]
    async fn replay_session(
        &self,
        Parameters(req): Parameters<ReplaySessionRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_session(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List indexed sessions. Filter by project and time range.")]
    async fn replay_list_sessions(
        &self,
        Parameters(req): Parameters<ReplayListSessionsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_list_sessions(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show usage history for a specific tool across sessions.")]
    async fn replay_tool_history(
        &self,
        Parameters(req): Parameters<ReplayToolHistoryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_tool_history(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List recent errors across sessions. Filter by project and time range.")]
    async fn replay_errors(
        &self,
        Parameters(req): Parameters<ReplayErrorsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_errors(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Session replay index status: session count, messages, errors, unindexed files.")]
    async fn replay_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::replay_status();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === TRUTHGUARD TOOLS (8 tools) ===

    #[tool(description = "Add an immutable fact to the registry. Categories: identity, project, business, technical, preference.")]
    async fn fact_add(
        &self,
        Parameters(req): Parameters<FactAddRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_add(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get a fact by ID, key, or category+key.")]
    async fn fact_get(
        &self,
        Parameters(req): Parameters<FactGetRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_get(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search facts by text match on key, value, or aliases. Filter by category and confidence.")]
    async fn fact_search(
        &self,
        Parameters(req): Parameters<FactSearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_search(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check a claim against known facts. Returns matches, contradictions, and verdicts.")]
    async fn fact_check(
        &self,
        Parameters(req): Parameters<FactCheckRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_check(&req.claim);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check an entire response for factual contradictions. Splits into sentences and checks each.")]
    async fn fact_check_response(
        &self,
        Parameters(req): Parameters<FactCheckResponseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_check_response(&req.response_text);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update a fact's value, confidence, aliases, source, or tags. Logged in audit trail.")]
    async fn fact_update(
        &self,
        Parameters(req): Parameters<FactUpdateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_update(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Delete a fact with audit logging. Irreversible.")]
    async fn fact_delete(
        &self,
        Parameters(req): Parameters<FactDeleteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::fact_delete(&req.fact_id, &req.reason.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "TruthGuard status: fact count by category, total checks, contradictions found.")]
    async fn truthguard_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::knowledge_tools::truthguard_status();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === ANALYTICS (10 tools) ===

    #[tool(description = "Log a tool call for analytics tracking. Auto-parses MCP name from tool_name.")]
    async fn log_tool_call(&self, Parameters(req): Parameters<LogToolCallRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::log_tool_call(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log a file operation (read/write/edit/delete) for tracking.")]
    async fn log_file_op(&self, Parameters(req): Parameters<LogFileOpRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::log_file_op(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log token usage and costs for a model interaction.")]
    async fn log_tokens(&self, Parameters(req): Parameters<LogTokensRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::log_tokens(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log a git commit with stats (files changed, insertions, deletions).")]
    async fn log_git_commit(&self, Parameters(req): Parameters<LogGitCommitRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::log_git_commit(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get usage report: tool calls, errors, file ops over N days.")]
    async fn usage_report(&self, Parameters(req): Parameters<UsageReportRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::usage_report(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Rank tools by usage count, error rate, and average latency.")]
    async fn tool_ranking(&self, Parameters(req): Parameters<ToolRankingRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::tool_ranking(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check MCP server health: error rates grouped by MCP server.")]
    async fn mcp_health(&self, Parameters(req): Parameters<McpHealthRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::mcp_health(req.days.unwrap_or(7));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get chronological activity feed for an agent (tool calls, file ops, commits).")]
    async fn agent_activity(&self, Parameters(req): Parameters<AgentActivityRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::agent_activity(&req.pane_id, req.limit.unwrap_or(50));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get token cost report broken down by model with cache analysis.")]
    async fn cost_report(&self, Parameters(req): Parameters<CostReportRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::cost_report(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get time-series metrics with daily/weekly/monthly granularity.")]
    async fn trends(&self, Parameters(req): Parameters<TrendsRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::analytics_tools::trends(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === QUALITY (8 tools) ===

    #[tool(description = "Log test results (total, passed, failed, skipped, duration).")]
    async fn log_test(&self, Parameters(req): Parameters<LogTestRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::log_test(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log build result (success, duration, output).")]
    async fn log_build(&self, Parameters(req): Parameters<LogBuildRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::log_build(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log lint results (errors, warnings).")]
    async fn log_lint(&self, Parameters(req): Parameters<LogLintRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::log_lint(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log deployment result (target, success, duration).")]
    async fn log_deploy(&self, Parameters(req): Parameters<LogDeployRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::log_deploy(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get quality report: pass rates by event type over N days.")]
    async fn quality_report(&self, Parameters(req): Parameters<QualityReportRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::quality_report(&req.project, req.days.unwrap_or(7));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Quality gate: PASS/FAIL based on latest test + build results.")]
    async fn quality_gate(&self, Parameters(req): Parameters<QualityGateRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::quality_gate(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Detect regressions: compare recent vs older pass rates, flag >5% drops.")]
    async fn regressions(&self, Parameters(req): Parameters<RegressionsRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::regressions(&req.project, req.days.unwrap_or(14));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Project health score (0-100): test_rate*40 + build_rate*40 + (1-error_rate)*20.")]
    async fn project_health(&self, Parameters(req): Parameters<ProjectHealthRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::quality_tools::project_health(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === DASHBOARD (8 tools) ===

    #[tool(description = "God view: agents, tasks, locks, ports, quality, recent activity.")]
    async fn dash_overview(&self, Parameters(req): Parameters<DashOverviewRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_overview(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Deep dive on one agent: status, recent tools, locks, session stats.")]
    async fn dash_agent_detail(&self, Parameters(req): Parameters<DashAgentDetailRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_agent_detail(&req.pane_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Project view: agents, tasks, quality, commits, knowledge.")]
    async fn dash_project(&self, Parameters(req): Parameters<DashProjectRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_project(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Agent leaderboard: ranked by tool_calls, success_rate, active_days.")]
    async fn dash_leaderboard(&self, Parameters(req): Parameters<DashLeaderboardRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_leaderboard(req.days.unwrap_or(7), req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Chronological event stream (tool calls + commits).")]
    async fn dash_timeline(&self, Parameters(req): Parameters<DashTimelineRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_timeline(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Alerts: dead agents, high error rates, failed tests, expired locks.")]
    async fn dash_alerts(&self, Parameters(req): Parameters<DashAlertsRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_alerts(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "24h summary: tool_calls, errors, commits, files_touched.")]
    async fn dash_daily_digest(&self, Parameters(req): Parameters<DashDailyDigestRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_daily_digest(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "JSON data export: agents, usage, quality reports.")]
    async fn dash_export(&self, Parameters(req): Parameters<DashExportRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard_tools::dash_export(&req.report, req.project.as_deref(), req.days.unwrap_or(30));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

}

#[tool_handler]
impl ServerHandler for DxIntelService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DX Terminal Intel: Analytics, knowledge graph, session replay, truthguard, quality, dashboards.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run(app: Arc<App>) -> anyhow::Result<()> {
    // Red banner for intel server
    eprintln!("\x1b[31m━━━ DX Intel ━━━ 49 tools ━━━\x1b[0m");
    tracing::info!("Starting intel MCP server (49 tools)");
    let server = DxIntelService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
