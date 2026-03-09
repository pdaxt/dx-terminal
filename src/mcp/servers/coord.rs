//! DX Terminal Coord: Multi-agent coordination, file locks, messaging, knowledge base, collaboration.
//! 53 tools.

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
pub struct DxCoordService {
    app: Arc<App>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DxCoordService {
    pub fn new(app: Arc<App>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    // === MULTI-AGENT COORDINATION (37 tools) ===

    #[tool(description = "Allocate a port for a service. Finds free port in 3001-3099 range, checks for conflicts.")]
    async fn port_allocate(
        &self,
        Parameters(req): Parameters<PortAllocateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::port_allocate(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Release an allocated port back to the pool.")]
    async fn port_release(
        &self,
        Parameters(req): Parameters<PortReleaseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::port_release(req.port);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all allocated ports with active/inactive status.")]
    async fn port_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::port_list();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get the port allocated for a service by name.")]
    async fn port_get(
        &self,
        Parameters(req): Parameters<PortGetRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::port_get(&req.service);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Register an agent in a pane. Returns other agents on same project for coordination.")]
    async fn agent_register(
        &self,
        Parameters(req): Parameters<AgentRegisterRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::agent_register(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update an agent's current task and file list.")]
    async fn agent_update(
        &self,
        Parameters(req): Parameters<AgentUpdateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::agent_update(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all registered agents, optionally filtered by project.")]
    async fn agent_list(
        &self,
        Parameters(req): Parameters<AgentListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::agent_list(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Deregister an agent and release its locks.")]
    async fn agent_deregister(
        &self,
        Parameters(req): Parameters<AgentDeregisterRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::agent_deregister(&req.pane_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Acquire file locks to prevent concurrent edits. Returns blocked status if files locked by others.")]
    async fn lock_acquire(
        &self,
        Parameters(req): Parameters<LockAcquireRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::lock_acquire(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Release file locks. Empty files list releases all locks for this pane.")]
    async fn lock_release(
        &self,
        Parameters(req): Parameters<LockReleaseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::lock_release(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check if files are locked and by whom.")]
    async fn lock_check(
        &self,
        Parameters(req): Parameters<LockCheckRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::lock_check(&req.files);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Claim a git branch for exclusive use. Prevents other agents from using the same branch.")]
    async fn git_claim_branch(
        &self,
        Parameters(req): Parameters<GitClaimBranchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::git_claim_branch(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Release a claimed git branch.")]
    async fn git_release_branch(
        &self,
        Parameters(req): Parameters<GitReleaseBranchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::git_release_branch(&req.pane_id, &req.branch, &req.repo);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all claimed git branches, optionally filtered by repo.")]
    async fn git_list_branches(
        &self,
        Parameters(req): Parameters<GitListBranchesRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::git_list_branches(req.repo.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check for conflicts before committing: file locks and concurrent edits.")]
    async fn git_pre_commit_check(
        &self,
        Parameters(req): Parameters<GitPreCommitCheckRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::git_pre_commit_check(&req.pane_id, &req.repo, &req.files);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Claim exclusive build access for a project. Prevents concurrent builds.")]
    async fn build_claim(
        &self,
        Parameters(req): Parameters<BuildClaimRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::build_claim(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Release build claim and record result in history.")]
    async fn build_release(
        &self,
        Parameters(req): Parameters<BuildReleaseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::build_release(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Check if a project is currently being built.")]
    async fn build_status(
        &self,
        Parameters(req): Parameters<BuildStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::build_status(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get the last build result for a project.")]
    async fn build_get_last(
        &self,
        Parameters(req): Parameters<BuildGetLastRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::build_get_last(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add an inter-agent task to the shared queue (not the DX Terminal auto-cycle queue).")]
    async fn ma_task_add(
        &self,
        Parameters(req): Parameters<MaTaskAddRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::task_add(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Claim the next pending inter-agent task by priority.")]
    async fn ma_task_claim(
        &self,
        Parameters(req): Parameters<MaTaskClaimRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::task_claim(&req.pane_id, req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Mark an inter-agent task as completed.")]
    async fn ma_task_complete(
        &self,
        Parameters(req): Parameters<MaTaskCompleteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::task_complete(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List inter-agent tasks, optionally filtered by status and project.")]
    async fn ma_task_list(
        &self,
        Parameters(req): Parameters<MaTaskListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::task_list(req.status.as_deref(), req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add a knowledge base entry for cross-agent learning.")]
    async fn kb_add(
        &self,
        Parameters(req): Parameters<KbAddRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::kb_add(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search the knowledge base by query, optionally filtered by project and category.")]
    async fn kb_search(
        &self,
        Parameters(req): Parameters<KbSearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::kb_search(&req.query, req.project.as_deref(), req.category.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List recent knowledge base entries.")]
    async fn kb_list(
        &self,
        Parameters(req): Parameters<KbListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::kb_list(req.project.as_deref(), req.limit.unwrap_or(20));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Broadcast a message to all agents.")]
    async fn msg_broadcast(
        &self,
        Parameters(req): Parameters<MsgBroadcastRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::msg_broadcast(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Send a direct message to a specific agent. Message is pushed to their PTY in real-time.")]
    async fn msg_send(
        &self,
        Parameters(req): Parameters<MsgSendRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::msg_send(&req.from_pane, &req.to_pane, &req.message);
        // Push to target agent's PTY for real-time delivery
        if let Ok(pane_num) = req.to_pane.parse::<u8>() {
            let formatted = format!("[MSG from {}]: {}", req.from_pane, req.message);
            let mut pty = self.app.pty_lock();
            let _ = pty.send_line(pane_num, &formatted);
        }
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Get unread messages for this agent. Marks as read by default.")]
    async fn msg_get(
        &self,
        Parameters(req): Parameters<MsgGetRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::msg_get(&req.pane_id, req.mark_read.unwrap_or(true));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Signal the control pane that you need attention. Types: need_help, blocked, found_issue, completed, failed. Appears as alert badge in TUI.")]
    async fn os_signal(
        &self,
        Parameters(req): Parameters<SignalRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = crate::multi_agent::signal_send(&req.pane_id, &req.signal_type, &req.message, req.pipeline_id.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }

    #[tool(description = "List agent signals (alerts). Shows unacknowledged by default.")]
    async fn os_signal_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = crate::multi_agent::signal_list(true);
        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }

    #[tool(description = "Acknowledge (dismiss) a signal by ID.")]
    async fn os_signal_ack(
        &self,
        Parameters(req): Parameters<SignalAckRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = crate::multi_agent::signal_acknowledge(req.signal_id);
        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }

    #[tool(description = "Clean up stale entries: ports, agents, locks, branches, builds from inactive panes.")]
    async fn cleanup_all(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::cleanup_all();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Full status overview: ports, agents, locks, builds, pending tasks.")]
    async fn status_overview(
        &self,
        Parameters(req): Parameters<StatusOverviewRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::status_overview(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === COLLAB TOOLS (19 tools) ===

    #[tool(description = "List all collaboration spaces with document counts.")]
    async fn space_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::space_list();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Create a new collaboration space for organizing docs by project.")]
    async fn space_create(
        &self,
        Parameters(req): Parameters<SpaceCreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::space_create(&req.name);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List documents. Filter by space and/or status (draft, review, approved, locked).")]
    async fn doc_list(
        &self,
        Parameters(req): Parameters<DocListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_list(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Read a document and its metadata, comments, directives, and proposals.")]
    async fn doc_read(
        &self,
        Parameters(req): Parameters<DocReadRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_read(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Create a new markdown document in a space.")]
    async fn doc_create(
        &self,
        Parameters(req): Parameters<DocCreateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_create(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Edit a document. Fails if locked by another agent — use doc_propose instead.")]
    async fn doc_edit(
        &self,
        Parameters(req): Parameters<DocEditRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_edit(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Propose changes to a document for human review. Use when doc is locked or review is wanted.")]
    async fn doc_propose(
        &self,
        Parameters(req): Parameters<DocProposeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_propose(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Approve a proposal and merge it into the document.")]
    async fn doc_approve(
        &self,
        Parameters(req): Parameters<DocApproveRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_approve(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Reject a proposal with a reason.")]
    async fn doc_reject(
        &self,
        Parameters(req): Parameters<DocRejectRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_reject(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Lock a document. Prevents direct editing — agents must use doc_propose. Auto-expires after 30 min.")]
    async fn doc_lock(
        &self,
        Parameters(req): Parameters<DocLockRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_lock(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Unlock a document. Allows direct editing again.")]
    async fn doc_unlock(
        &self,
        Parameters(req): Parameters<DocUnlockRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_unlock(&req.space, &req.name);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Add a comment to a document. For feedback, questions, or directive responses.")]
    async fn doc_comment(
        &self,
        Parameters(req): Parameters<DocCommentRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_comment(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Read all comments on a document.")]
    async fn doc_comments(
        &self,
        Parameters(req): Parameters<DocCommentsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_comments(&req.space, &req.name);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Update document status: draft, review, approved, archived.")]
    async fn doc_status(
        &self,
        Parameters(req): Parameters<DocStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_status(&req.space, &req.name, &req.status);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Search across all documents for text matches (case-insensitive).")]
    async fn doc_search(
        &self,
        Parameters(req): Parameters<DocSearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_search(&req.query, &req.space.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Find all <!-- @claude: ... --> directives — tasks/questions from humans for Claude.")]
    async fn doc_directives(
        &self,
        Parameters(req): Parameters<DocDirectivesRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_directives(&req.space.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show git version history of a document.")]
    async fn doc_history(
        &self,
        Parameters(req): Parameters<DocHistoryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_history(&req.space, &req.name, req.limit.unwrap_or(10));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Delete a document and its metadata/proposals. Requires confirm=true.")]
    async fn doc_delete(
        &self,
        Parameters(req): Parameters<DocDeleteRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::doc_delete(&req.space, &req.name, req.confirm.unwrap_or(false));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Initialize the collab workspace. Creates directories and sets up git.")]
    async fn collab_init(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::collab_tools::collab_init();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

}

#[tool_handler]
impl ServerHandler for DxCoordService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DX Terminal Coord: Multi-agent coordination, file locks, messaging, knowledge base, collaboration.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}

pub async fn run(app: Arc<App>) -> anyhow::Result<()> {
    tracing::info!("Starting coord MCP server (53 tools)");
    let server = DxCoordService::new(app);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("MCP serve error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}
