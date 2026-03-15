//! DX Terminal Core: Agent lifecycle, monitoring, configuration, git isolation, screen management.
//! 43 tools.

use crate::app::App;
use crate::mcp::tools;
use crate::mcp::types::*;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct DxCoreService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DxCoreService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === AGENT LIFECYCLE ===

    #[tool(
        description = "Spawn a Claude agent in a pane with full auto-config. Resolves project path, sets MCPs, generates role preamble."
    )]
    async fn os_spawn(
        &self,
        Parameters(req): Parameters<SpawnRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::spawn(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Gracefully stop an agent and clean up state.")]
    async fn os_kill(
        &self,
        Parameters(req): Parameters<KillRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::kill(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Kill and re-spawn an agent with same config.")]
    async fn os_restart(
        &self,
        Parameters(req): Parameters<RestartRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::restart(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update an agent's task/project/role without full restart.")]
    async fn os_reassign(
        &self,
        Parameters(req): Parameters<ReassignRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::reassign(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === TASK ASSIGNMENT ===

    #[tool(description = "Pull a tracker issue and assign it to a pane with auto-config.")]
    async fn os_assign(
        &self,
        Parameters(req): Parameters<AssignRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::assign(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Assign an ad-hoc task (not from tracker) to a pane.")]
    async fn os_assign_adhoc(
        &self,
        Parameters(req): Parameters<AssignAdhocRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::assign_adhoc(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Capture agent output and detect completion/errors.")]
    async fn os_collect(
        &self,
        Parameters(req): Parameters<CollectRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collect(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Mark task complete, log ACU, update tracker, free pane.")]
    async fn os_complete(
        &self,
        Parameters(req): Parameters<CompleteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::complete(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === CONFIGURATION ===

    #[tool(description = "Set project-level MCPs in claude config.")]
    async fn os_set_mcps(
        &self,
        Parameters(req): Parameters<SetMcpsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::set_mcps(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Write a custom preamble for an agent.")]
    async fn os_set_preamble(
        &self,
        Parameters(req): Parameters<SetPreambleRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::set_preamble(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show configuration for a pane or all panes.")]
    async fn os_config_show(
        &self,
        Parameters(req): Parameters<ConfigShowRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::config_show(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === MONITORING ===

    #[tool(description = "Full status of all panes: project, role, task, ACU, status.")]
    async fn os_status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::status(&self.app).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Rich dashboard with capacity gauges, agent list, kanban summary.")]
    async fn os_dashboard(
        &self,
        Parameters(req): Parameters<DashboardRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::dashboard(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get recent activity log, optionally filtered by pane.")]
    async fn os_logs(
        &self,
        Parameters(req): Parameters<LogsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::logs(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Health check: stuck agents, idle panes, error detection.")]
    async fn os_health(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::health(&self.app).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === MONITORING (ENHANCED) ===

    #[tool(
        description = "Single-call overview of everything happening right now: all pane health, queue status, alerts, capacity, recent activity. Use this first when checking in."
    )]
    async fn os_monitor(
        &self,
        Parameters(req): Parameters<MonitorRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::monitor(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Everything about one project: which panes work on it, open issues, git activity, capacity spent, assigned MCPs."
    )]
    async fn os_project_status(
        &self,
        Parameters(req): Parameters<ProjectStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::project_status(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Daily/weekly digest: tasks completed, ACU spent, errors, queue throughput, recommendations. Period: today, yesterday, week, month."
    )]
    async fn os_digest(
        &self,
        Parameters(req): Parameters<DigestRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::digest(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Watch a pane's live output with error highlighting. Shows last N lines, detects errors/warnings, identifies agent phase (thinking/writing/running)."
    )]
    async fn os_watch(
        &self,
        Parameters(req): Parameters<WatchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::watch(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === MCP ROUTING ===

    #[tool(
        description = "List all available MCPs with descriptions, capabilities, and categories. Filter by category or project."
    )]
    async fn os_mcp_list(
        &self,
        Parameters(req): Parameters<McpListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::mcp_list(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Smart MCP routing: given a project, task, and role, suggests the best MCPs to enable. Set apply=true to auto-configure."
    )]
    async fn os_mcp_route(
        &self,
        Parameters(req): Parameters<McpRouteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::mcp_route(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search MCPs by name, description, capability, or keyword.")]
    async fn os_mcp_search(
        &self,
        Parameters(req): Parameters<McpSearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::mcp_search(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Route a task to the best provider based on task type, language, historical success, and estimated cost."
    )]
    async fn route_task(
        &self,
        Parameters(req): Parameters<RouteTaskRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::agent_router_tools::route_task(req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show provider usage statistics and cost-per-task history.")]
    async fn agent_stats(
        &self,
        Parameters(req): Parameters<AgentStatsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::agent_router_tools::agent_stats(req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add a custom regex routing rule for provider selection.")]
    async fn add_routing_rule(
        &self,
        Parameters(req): Parameters<AddRoutingRuleRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::agent_router_tools::add_routing_rule(req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === GIT ISOLATION ===

    #[tool(description = "Sync agent's worktree with latest from base branch (fetch + rebase).")]
    async fn os_git_sync(
        &self,
        Parameters(req): Parameters<GitSyncRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_sync(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show git status and diff for an agent's isolated worktree.")]
    async fn os_git_status(
        &self,
        Parameters(req): Parameters<GitStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_status_tool(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Commit and push an agent's current work to its branch.")]
    async fn os_git_push(
        &self,
        Parameters(req): Parameters<GitPushRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_push(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Create a pull request from an agent's branch. Commits and pushes first.")]
    async fn os_git_pr(
        &self,
        Parameters(req): Parameters<GitPrRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_pr(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Merge an agent's branch back into the base branch (rebase + merge). Cleans up the branch after merge."
    )]
    async fn os_git_merge(
        &self,
        Parameters(req): Parameters<GitMergeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_merge(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === MACHINE IDENTITY ===

    #[tool(
        description = "Get machine identity (IP, hostname, MAC) for a pane. Omit pane to list all registered machines."
    )]
    async fn os_machine_info(
        &self,
        Parameters(req): Parameters<MachineInfoRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::machine_info_tool(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List all registered machines with network identities, subnet info, and IP range."
    )]
    async fn os_machine_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::machine_list_tool();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === LIFECYCLE (heartbeat, sessions, who, lock_steal, conflict_scan) ===

    #[tool(description = "Send heartbeat to keep agent alive. Optionally update task/status.")]
    async fn heartbeat(
        &self,
        Parameters(req): Parameters<HeartbeatRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::heartbeat(
            &req.pane_id,
            req.task.as_deref(),
            req.status.as_deref(),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Start a new tracking session for an agent.")]
    async fn session_start(
        &self,
        Parameters(req): Parameters<SessionStartRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::session_start(&req.pane_id, &req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "End a tracking session with summary.")]
    async fn session_end(
        &self,
        Parameters(req): Parameters<SessionEndRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::session_end(
            &req.session_id,
            &req.summary.clone().unwrap_or_default(),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all active agents (simple view with heartbeat status).")]
    async fn who(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::who();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Force-steal a file lock with justification.")]
    async fn lock_steal(
        &self,
        Parameters(req): Parameters<LockStealRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result =
            tools::multi_agent_tools::lock_steal(&req.pane_id, &req.file_path, &req.reason);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Detect concurrent work on same files across agents.")]
    async fn conflict_scan(
        &self,
        Parameters(req): Parameters<ConflictScanRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::conflict_scan(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === DATA RETENTION ===

    #[tool(description = "Manually prune old data according to retention policies.")]
    async fn prune_data(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = crate::engine::retention::prune_manual();
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    // === SESSION CONTROL ===

    #[tool(
        description = "Start supervising a tmux pane. Watches agent output, auto-approves safe commands, nudges idle agents, and sends recovery instructions on errors."
    )]
    async fn session_control_start(
        &self,
        Parameters(req): Parameters<SessionControlStartRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::session_control_tools::session_control_start(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Stop supervising a tmux pane.")]
    async fn session_control_stop(
        &self,
        Parameters(req): Parameters<SessionControlStopRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::session_control_tools::session_control_stop(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get the current supervision state for one tmux pane.")]
    async fn session_control_status(
        &self,
        Parameters(req): Parameters<SessionControlStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::session_control_tools::session_control_status(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all supervised tmux panes and their control-loop state.")]
    async fn session_control_list(
        &self,
        Parameters(_req): Parameters<SessionControlListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::session_control_tools::session_control_list(&self.app).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Send a manual instruction into a supervised tmux pane.")]
    async fn session_control_send(
        &self,
        Parameters(req): Parameters<SessionControlSendRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::session_control_tools::session_control_send(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === SCREEN MANAGEMENT ===

    #[tool(
        description = "Add a new screen with configurable layout. Creates a tmux window with N panes. Layouts: single (1), split2 (2), horizontal (3, default), vertical (3), grid2x2 (4)."
    )]
    async fn dx_add_screen(
        &self,
        Parameters(req): Parameters<AddScreenRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::screen_tools::add_screen(&self.app, req.name, req.layout, req.panes);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Remove a screen and its panes. Fails if agents are active unless force=true. Cannot remove the last screen."
    )]
    async fn dx_remove_screen(
        &self,
        Parameters(req): Parameters<RemoveScreenRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result =
            tools::screen_tools::remove_screen(&self.app, req.screen, req.force.unwrap_or(false));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List all screens with their panes, agent status, and layout. Shows active/idle counts per screen."
    )]
    async fn dx_list_screens(
        &self,
        Parameters(_req): Parameters<ListScreensRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::screen_tools::list_screens(&self.app);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get screen layout summary: total screens, total panes, session name.")]
    async fn dx_screen_summary(
        &self,
        Parameters(_req): Parameters<ScreenSummaryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::screen_tools::screen_summary(&self.app);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === GATEWAY (MICRO MCP MANAGEMENT) ===

    #[tool(
        description = "Discover micro MCPs matching a capability keyword. Optionally auto-start them. Use this to find composable building blocks."
    )]
    async fn mcp_discover(
        &self,
        Parameters(req): Parameters<GatewayDiscoverRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_discover(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Call a tool on a micro MCP. Auto-starts the MCP if not running. Routes through the gateway for lifecycle management."
    )]
    async fn mcp_call(
        &self,
        Parameters(req): Parameters<GatewayCallRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_call(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List all registered and running micro MCPs. Shows tool counts, uptime, and last-used timestamps."
    )]
    async fn mcp_gateway_list(
        &self,
        Parameters(req): Parameters<GatewayListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_list(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Inspect the tools exposed by one gateway MCP. Auto-starts the MCP by default and returns tool schemas."
    )]
    async fn mcp_gateway_tools(
        &self,
        Parameters(req): Parameters<GatewayToolsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_tools(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for DxCoreService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DX Terminal Core: Agent lifecycle, monitoring, configuration, git isolation, screen management.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run(app: Arc<App>) -> anyhow::Result<()> {
    // Cyan banner for core server
    eprintln!("\x1b[36m━━━ DX Core ━━━ 43 tools ━━━\x1b[0m");
    tracing::info!("Starting core MCP server (43 tools)");
    let server = DxCoreService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
