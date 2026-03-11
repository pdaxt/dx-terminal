//! DX Terminal Queue: Task queue, auto-cycle, factory pipelines, orchestration, project intelligence.
//! 29 tools.

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
pub struct DxQueueService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DxQueueService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === QUEUE / AUTO-CYCLE ===

    #[tool(
        description = "Add a task to the queue. Tasks are auto-assigned to free panes when os_auto is called."
    )]
    async fn os_queue_add(
        &self,
        Parameters(req): Parameters<QueueAddRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_add(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Decompose a high-level goal into sub-tasks with auto-wired dependencies. Use numbered steps (1. 2. 3.) for sequential tasks, prefix with || for parallel."
    )]
    async fn os_queue_decompose(
        &self,
        Parameters(req): Parameters<DecomposeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_decompose(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List all queued tasks with status. Filter by: pending, running, done, failed."
    )]
    async fn os_queue_list(
        &self,
        Parameters(req): Parameters<QueueListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_list(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Mark a queued task as done. Unblocks dependent tasks.")]
    async fn os_queue_done(
        &self,
        Parameters(req): Parameters<QueueDoneRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_done(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Run one auto-cycle: complete finished agents, spawn next queued tasks on free panes. Call repeatedly (every 30-60s) for continuous operation."
    )]
    async fn os_auto(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::auto_cycle(&self.app).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Configure auto-cycle behavior: max parallel panes, reserved panes, auto-complete, auto-assign."
    )]
    async fn os_auto_config(
        &self,
        Parameters(req): Parameters<AutoConfigRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::auto_config(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Cancel a specific queue task. Marks it as failed and cascades failure to dependent tasks."
    )]
    async fn os_queue_cancel(
        &self,
        Parameters(req): Parameters<QueueCancelRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_cancel(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Retry a failed queue task. Resets to pending and increments retry count. Must be under max_retries."
    )]
    async fn os_queue_retry(
        &self,
        Parameters(req): Parameters<QueueRetryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_retry(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Clear completed and/or failed tasks from the queue. Filter: done (default), failed, or all."
    )]
    async fn os_queue_clear(
        &self,
        Parameters(req): Parameters<QueueClearRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_clear(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Detect which project a natural language description refers to. Returns project name and confidence score."
    )]
    async fn factory_detect(
        &self,
        Parameters(req): Parameters<FactoryDetectRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_detect(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Get saved quality gate results (build/test/lint) for a pipeline. Shows pass/fail and command output."
    )]
    async fn factory_gate_result(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_gate_result(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Retry failed stages in a pipeline. Resets failed tasks to pending.")]
    async fn factory_retry(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_retry(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get pipeline events log. Shows all state transitions and actions.")]
    async fn factory_events(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_events(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Pause a factory pipeline. Stops new stages from spawning. Running agents continue but no new ones start. Use :resume to unpause."
    )]
    async fn factory_pause(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_pause(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Resume a paused factory pipeline. Queued stages will spawn on next auto-cycle."
    )]
    async fn factory_resume(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_resume(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Retry a specific stage in a pipeline by name (e.g., 'dev', 'qa'). Resets that stage and all cascade-failed dependents back to pending."
    )]
    async fn factory_retry_stage(
        &self,
        Parameters(req): Parameters<FactoryRetryStageRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_retry_stage(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === FACTORY (TRACKED PIPELINE) ===

    #[tool(
        description = "Factory mode: natural language request → classifies project + intent → creates tracked dev+QA+security pipeline → monitors end-to-end. Returns factory_id to track progress. Use 'factory_status' to check pipeline state."
    )]
    async fn factory_run(
        &self,
        Parameters(req): Parameters<FactoryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_run(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Get status of a factory pipeline run: stage, pane assignments, agent progress."
    )]
    async fn factory_status(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_status(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all factory pipeline runs: active, completed, and failed.")]
    async fn factory_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_list();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Run quality gates (build, test, lint) on a factory pipeline. Returns pass/fail for each check. Auto-runs after dev stage completes."
    )]
    async fn factory_gate(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_gate(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Scan for git conflicts in a factory pipeline's project: uncommitted changes, overlapping edits between pipeline agents."
    )]
    async fn pipeline_conflict_scan(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::conflict_scan(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Cancel a factory pipeline. Marks all pending/blocked stages as failed and kills any running agents. Use when a pipeline is broken or no longer needed."
    )]
    async fn factory_cancel(
        &self,
        Parameters(req): Parameters<FactoryStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_cancel(&self.app, &req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "View the factory inbox: TUI-submitted requests, their classification, pipeline mapping, and status. Shows pending/running/complete/failed requests from the command bar."
    )]
    async fn factory_inbox(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::factory_tools::factory_inbox();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === ORCHESTRATION ===

    #[tool(
        description = "Orchestrate: say what you want in natural language. DX Terminal identifies the project, decomposes into dev + QA + security tasks, spawns agents on free panes, monitors to completion. The 'machine that builds machines' command."
    )]
    async fn orchestrate(
        &self,
        Parameters(req): Parameters<OrchestrateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::orchestrate::orchestrate(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === PROJECT INTELLIGENCE (5 tools) ===

    #[tool(
        description = "Scan ~/Projects for git repos. Auto-detects tech stacks, test/build commands, git status. Returns count of discovered projects."
    )]
    async fn project_scan(
        &self,
        Parameters(_req): Parameters<ProjectScanRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_scan();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List all discovered projects with tech stack, health grade, git status. Filter by tech (e.g. 'rust', 'node')."
    )]
    async fn project_list(
        &self,
        Parameters(req): Parameters<ProjectListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_list(req.tech.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Full detail for one project: tech, commands, git status, health, open issues, active agents. The single source of truth for a project."
    )]
    async fn project_detail(
        &self,
        Parameters(req): Parameters<ProjectDetailRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_detail(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Run tests for a project NOW and return pass/fail with output. Logs result to quality system."
    )]
    async fn project_test(
        &self,
        Parameters(req): Parameters<ProjectTestRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_test(&req.project).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Show dependency graph between local projects. Shows which projects depend on each other."
    )]
    async fn project_deps(
        &self,
        Parameters(req): Parameters<ProjectDepsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_deps(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for DxQueueService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DX Terminal Queue: Task queue, auto-cycle, factory pipelines, orchestration, project intelligence.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run(app: Arc<App>) -> anyhow::Result<()> {
    // Green banner for queue server
    eprintln!("\x1b[32m━━━ DX Queue ━━━ 29 tools ━━━\x1b[0m");
    tracing::info!("Starting queue MCP server (29 tools)");
    let server = DxQueueService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
