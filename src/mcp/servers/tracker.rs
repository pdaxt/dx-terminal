//! DX Terminal Tracker: Issue tracking, features, capacity planning, code audits.
//! 32 tools.

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
pub struct DxTrackerService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DxTrackerService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === TRACKER TOOLS (15 tools) ===

    #[tool(description = "Create a new issue in a tracker space. Returns issue ID.")]
    async fn issue_create(
        &self,
        Parameters(req): Parameters<IssueCreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_create(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update an issue's fields: status, priority, assignee, labels, ACU, etc.")]
    async fn issue_update_full(
        &self,
        Parameters(req): Parameters<IssueUpdateFullRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_update_full(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "List issues with filters: status, type, priority, assignee, milestone, label, sprint, role."
    )]
    async fn issue_list_filtered(
        &self,
        Parameters(req): Parameters<IssueListFilteredRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_list_filtered(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "View full details of a single issue including comments and links.")]
    async fn issue_view(
        &self,
        Parameters(req): Parameters<IssueViewRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_view(&req.space, &req.issue_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add a comment to an issue.")]
    async fn issue_comment(
        &self,
        Parameters(req): Parameters<IssueCommentRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_comment(
            &req.space,
            &req.issue_id,
            &req.text,
            &req.author.clone().unwrap_or_else(|| "agent".into()),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Link a doc, commit, or PR to an issue.")]
    async fn issue_link(
        &self,
        Parameters(req): Parameters<IssueLinkRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_link(
            &req.space,
            &req.issue_id,
            &req.link_type,
            &req.reference,
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Close an issue with a resolution note.")]
    async fn issue_close(
        &self,
        Parameters(req): Parameters<IssueCloseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_close(
            &req.space,
            &req.issue_id,
            req.resolution.as_deref().unwrap_or(""),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Create a milestone for a space with optional due date.")]
    async fn milestone_create(
        &self,
        Parameters(req): Parameters<MilestoneCreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::milestone_create(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List milestones with progress for a space.")]
    async fn milestone_list(
        &self,
        Parameters(req): Parameters<MilestoneListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::milestone_list(&req.space);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Generate a Mermaid Gantt timeline from open issues.")]
    async fn timeline_generate(
        &self,
        Parameters(req): Parameters<TimelineGenerateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::timeline_generate(
            &req.space,
            &req.milestone.clone().unwrap_or_default(),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Start a process from a checklist template. Context vars substitute {{var}} placeholders."
    )]
    async fn process_start(
        &self,
        Parameters(req): Parameters<ProcessStartRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::process_start(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update a process step as done or undone.")]
    async fn process_update(
        &self,
        Parameters(req): Parameters<ProcessUpdateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::process_update(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all processes in a space with progress.")]
    async fn process_list(
        &self,
        Parameters(req): Parameters<ProcessListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::process_list(&req.space);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Create a checklist template from markdown with - [ ] items.")]
    async fn process_template_create(
        &self,
        Parameters(req): Parameters<ProcessTemplateCreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::process_template_create(&req.name, &req.content);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Kanban board view of all issues in a space grouped by status.")]
    async fn board_view(
        &self,
        Parameters(req): Parameters<BoardViewRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::board_view(&req.space);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === FEATURE MANAGEMENT TOOLS (4 tools) ===

    #[tool(
        description = "List child issues (micro-features) of a parent feature/epic. Shows progress."
    )]
    async fn issue_children(
        &self,
        Parameters(req): Parameters<IssueChildrenRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_children(&req.space, &req.parent_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Decompose a feature/epic into micro-feature child issues. Creates task issues linked to parent. Children: [{title, description?, priority?, role?, estimated_acu?}]"
    )]
    async fn feature_decompose(
        &self,
        Parameters(req): Parameters<FeatureDecomposeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result =
            tools::tracker_tools::feature_decompose(&req.space, &req.parent_id, &req.children);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Push tracker issues into the execution queue. Links queue tasks back to issues for auto-status updates on completion. Set sequential=true for ordered execution."
    )]
    async fn feature_to_queue(
        &self,
        Parameters(req): Parameters<FeatureToQueueRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::feature_to_queue(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Hierarchical feature status: parent feature → child micro-features → queue task status. Shows overall progress."
    )]
    async fn feature_status(
        &self,
        Parameters(req): Parameters<FeatureStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::feature_status(&req.space, &req.feature_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === CAPACITY TOOLS (8 tools) ===

    #[tool(
        description = "Configure capacity: pane count, hours, availability factor, review bandwidth, build slots."
    )]
    async fn cap_configure(
        &self,
        Parameters(req): Parameters<CapConfigureRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_configure(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Estimate ACU for a task based on type, complexity, and role.")]
    async fn cap_estimate(
        &self,
        Parameters(req): Parameters<CapEstimateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_estimate(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Log work done: ACU spent on an issue with role and review tracking.")]
    async fn cap_log_work(
        &self,
        Parameters(req): Parameters<CapLogWorkRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_log_work(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Plan a sprint: assign issues, calculate capacity vs load, detect bottlenecks."
    )]
    async fn cap_plan_sprint(
        &self,
        Parameters(req): Parameters<CapPlanSprintRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_plan_sprint(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Capacity dashboard: today's ACU usage, review load, active sprint progress."
    )]
    async fn cap_dashboard(
        &self,
        Parameters(req): Parameters<CapDashboardRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_dashboard(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Sprint burndown chart: ideal vs actual progress with projection.")]
    async fn cap_burndown(
        &self,
        Parameters(req): Parameters<CapBurndownRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result =
            tools::capacity_tools::cap_burndown(&req.sprint_id.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Sprint velocity: historical throughput across sprints with accuracy tracking."
    )]
    async fn cap_velocity(
        &self,
        Parameters(req): Parameters<CapVelocityRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_velocity(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all roles with definitions and today's utilization per role.")]
    async fn cap_roles(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_roles();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === AUDIT TOOLS (5 tools) ===

    #[tool(
        description = "Audit code quality: find dead code, fragmentation, loose ends (TODO/FIXME/HACK), empty impls, and incomplete patterns. Works on any project in the registry or by absolute path."
    )]
    async fn audit_code(
        &self,
        Parameters(req): Parameters<AuditCodeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_code(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Security audit: scan for hardcoded secrets, unsafe code, command injection vectors, path traversal, and dependency CVEs (via cargo audit). Returns findings by severity."
    )]
    async fn audit_security(
        &self,
        Parameters(req): Parameters<AuditSecurityRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_security(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Intent verification: check if code matches its purpose. Finds stub functions, untested modules, missing module files, and compares README claims against actual source. Optionally provide a description of intended functionality."
    )]
    async fn audit_intent(
        &self,
        Parameters(req): Parameters<AuditIntentRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_intent(
            &req.project,
            req.description.as_deref().unwrap_or(""),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Dependency health audit: check for wildcard versions, excessive dependencies, duplicate crate versions, and known vulnerabilities in Cargo.lock/package.json."
    )]
    async fn audit_deps(
        &self,
        Parameters(req): Parameters<AuditDepsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_deps(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(
        description = "Full audit: runs code, security, intent, and dependency audits. Returns aggregate grade (A-F), findings by severity, and stores results for trend tracking. Use this for production readiness checks."
    )]
    async fn audit_full(
        &self,
        Parameters(req): Parameters<AuditFullRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_full(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for DxTrackerService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DX Terminal Tracker: Issue tracking, features, capacity planning, code audits."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run(app: Arc<App>) -> anyhow::Result<()> {
    // Yellow banner for tracker server
    eprintln!("\x1b[33m━━━ DX Tracker ━━━ 32 tools ━━━\x1b[0m");
    tracing::info!("Starting tracker MCP server (32 tools)");
    let server = DxTrackerService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
