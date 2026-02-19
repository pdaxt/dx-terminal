pub mod types;
pub mod tools;

use std::sync::Arc;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use crate::app::App;
use self::types::*;

#[derive(Clone)]
pub struct AgentOSService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AgentOSService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === AGENT LIFECYCLE ===

    #[tool(description = "Spawn a Claude agent in a pane with full auto-config. Resolves project path, sets MCPs, generates role preamble.")]
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

    #[tool(description = "Full status of all 9 panes: project, role, task, ACU, status.")]
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

    // === MCP ROUTING ===

    #[tool(description = "List all available MCPs with descriptions, capabilities, and categories. Filter by category or project.")]
    async fn os_mcp_list(
        &self,
        Parameters(req): Parameters<McpListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::mcp_list(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Smart MCP routing: given a project, task, and role, suggests the best MCPs to enable. Set apply=true to auto-configure.")]
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
}

#[tool_handler]
impl ServerHandler for AgentOSService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "AgentOS: Terminal orchestrator for AI agent teams. \
                 Spawns, assigns, monitors Claude agents across 9 panes \
                 from a single control plane.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run_mcp_server(app: Arc<App>) -> anyhow::Result<()> {
    tracing::info!("Starting AgentOS MCP server");

    let server = AgentOSService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;

    service.waiting().await?;
    tracing::info!("AgentOS MCP server stopped");
    Ok(())
}
