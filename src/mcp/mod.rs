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

    #[tool(description = "Single-call overview of everything happening right now: all pane health, queue status, alerts, capacity, recent activity. Use this first when checking in.")]
    async fn os_monitor(
        &self,
        Parameters(req): Parameters<MonitorRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::monitor(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Everything about one project: which panes work on it, open issues, git activity, capacity spent, assigned MCPs.")]
    async fn os_project_status(
        &self,
        Parameters(req): Parameters<ProjectStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::project_status(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Daily/weekly digest: tasks completed, ACU spent, errors, queue throughput, recommendations. Period: today, yesterday, week, month.")]
    async fn os_digest(
        &self,
        Parameters(req): Parameters<DigestRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::digest(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Watch a pane's live output with error highlighting. Shows last N lines, detects errors/warnings, identifies agent phase (thinking/writing/running).")]
    async fn os_watch(
        &self,
        Parameters(req): Parameters<WatchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::watch(&self.app, req).await;
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

    #[tool(description = "Merge an agent's branch back into the base branch (rebase + merge). Cleans up the branch after merge.")]
    async fn os_git_merge(
        &self,
        Parameters(req): Parameters<GitMergeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::git_merge(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === QUEUE / AUTO-CYCLE ===

    #[tool(description = "Add a task to the queue. Tasks are auto-assigned to free panes when os_auto is called.")]
    async fn os_queue_add(
        &self,
        Parameters(req): Parameters<QueueAddRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_add(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Decompose a high-level goal into sub-tasks with auto-wired dependencies. Use numbered steps (1. 2. 3.) for sequential tasks, prefix with || for parallel.")]
    async fn os_queue_decompose(
        &self,
        Parameters(req): Parameters<DecomposeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::queue_decompose(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all queued tasks with status. Filter by: pending, running, done, failed.")]
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

    #[tool(description = "Run one auto-cycle: complete finished agents, spawn next queued tasks on free panes. Call repeatedly (every 30-60s) for continuous operation.")]
    async fn os_auto(
        &self,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::auto_cycle(&self.app).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Configure auto-cycle behavior: max parallel panes, reserved panes, auto-complete, auto-assign.")]
    async fn os_auto_config(
        &self,
        Parameters(req): Parameters<AutoConfigRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::auto_config(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
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

    #[tool(description = "Add an inter-agent task to the shared queue (not the AgentOS auto-cycle queue).")]
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

    #[tool(description = "Send a direct message to a specific agent.")]
    async fn msg_send(
        &self,
        Parameters(req): Parameters<MsgSendRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::msg_send(&req.from_pane, &req.to_pane, &req.message);
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

    #[tool(description = "List issues with filters: status, type, priority, assignee, milestone, label, sprint, role.")]
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
            &req.space, &req.issue_id, &req.text, &req.author.clone().unwrap_or_else(|| "agent".into()),
        );
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Link a doc, commit, or PR to an issue.")]
    async fn issue_link(
        &self,
        Parameters(req): Parameters<IssueLinkRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_link(&req.space, &req.issue_id, &req.link_type, &req.reference);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Close an issue with a resolution note.")]
    async fn issue_close(
        &self,
        Parameters(req): Parameters<IssueCloseRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_close(
            &req.space, &req.issue_id, req.resolution.as_deref().unwrap_or(""),
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
        let result = tools::tracker_tools::timeline_generate(&req.space, &req.milestone.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Start a process from a checklist template. Context vars substitute {{var}} placeholders.")]
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

    #[tool(description = "List child issues (micro-features) of a parent feature/epic. Shows progress.")]
    async fn issue_children(
        &self,
        Parameters(req): Parameters<IssueChildrenRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::issue_children(&req.space, &req.parent_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Decompose a feature/epic into micro-feature child issues. Creates task issues linked to parent. Children: [{title, description?, priority?, role?, estimated_acu?}]")]
    async fn feature_decompose(
        &self,
        Parameters(req): Parameters<FeatureDecomposeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::feature_decompose(&req.space, &req.parent_id, &req.children);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Push tracker issues into the execution queue. Links queue tasks back to issues for auto-status updates on completion. Set sequential=true for ordered execution.")]
    async fn feature_to_queue(
        &self,
        Parameters(req): Parameters<FeatureToQueueRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::feature_to_queue(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Hierarchical feature status: parent feature → child micro-features → queue task status. Shows overall progress.")]
    async fn feature_status(
        &self,
        Parameters(req): Parameters<FeatureStatusRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::tracker_tools::feature_status(&req.space, &req.feature_id);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === CAPACITY TOOLS (8 tools) ===

    #[tool(description = "Configure capacity: pane count, hours, availability factor, review bandwidth, build slots.")]
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

    #[tool(description = "Plan a sprint: assign issues, calculate capacity vs load, detect bottlenecks.")]
    async fn cap_plan_sprint(
        &self,
        Parameters(req): Parameters<CapPlanSprintRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::capacity_tools::cap_plan_sprint(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Capacity dashboard: today's ACU usage, review load, active sprint progress.")]
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
        let result = tools::capacity_tools::cap_burndown(&req.sprint_id.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Sprint velocity: historical throughput across sprints with accuracy tracking.")]
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

    // === MACHINE IDENTITY ===

    #[tool(description = "Get machine identity (IP, hostname, MAC) for a pane. Omit pane to list all registered machines.")]
    async fn os_machine_info(
        &self,
        Parameters(req): Parameters<MachineInfoRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::machine_info_tool(&req);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all registered machines with network identities, subnet info, and IP range.")]
    async fn os_machine_list(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::machine_list_tool();
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

    // === LIFECYCLE (heartbeat, sessions, who, lock_steal, conflict_scan) ===

    #[tool(description = "Send heartbeat to keep agent alive. Optionally update task/status.")]
    async fn heartbeat(&self, Parameters(req): Parameters<HeartbeatRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::heartbeat(&req.pane_id, req.task.as_deref(), req.status.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Start a new tracking session for an agent.")]
    async fn session_start(&self, Parameters(req): Parameters<SessionStartRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::session_start(&req.pane_id, &req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "End a tracking session with summary.")]
    async fn session_end(&self, Parameters(req): Parameters<SessionEndRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::session_end(&req.session_id, &req.summary.clone().unwrap_or_default());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all active agents (simple view with heartbeat status).")]
    async fn who(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::who();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Force-steal a file lock with justification.")]
    async fn lock_steal(&self, Parameters(req): Parameters<LockStealRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::lock_steal(&req.pane_id, &req.file_path, &req.reason);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Detect concurrent work on same files across agents.")]
    async fn conflict_scan(&self, Parameters(req): Parameters<ConflictScanRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::multi_agent_tools::conflict_scan(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === DATA RETENTION ===

    #[tool(description = "Manually prune old data according to retention policies.")]
    async fn prune_data(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = crate::engine::retention::prune_manual();
        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }

    // === PROJECT INTELLIGENCE (5 tools) ===

    #[tool(description = "Scan ~/Projects for git repos. Auto-detects tech stacks, test/build commands, git status. Returns count of discovered projects.")]
    async fn project_scan(
        &self,
        Parameters(_req): Parameters<types::ProjectScanRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_scan();
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all discovered projects with tech stack, health grade, git status. Filter by tech (e.g. 'rust', 'node').")]
    async fn project_list(
        &self,
        Parameters(req): Parameters<types::ProjectListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_list(req.tech.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Full detail for one project: tech, commands, git status, health, open issues, active agents. The single source of truth for a project.")]
    async fn project_detail(
        &self,
        Parameters(req): Parameters<types::ProjectDetailRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_detail(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Run tests for a project NOW and return pass/fail with output. Logs result to quality system.")]
    async fn project_test(
        &self,
        Parameters(req): Parameters<types::ProjectTestRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_test(&req.project).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Show dependency graph between local projects. Shows which projects depend on each other.")]
    async fn project_deps(
        &self,
        Parameters(req): Parameters<types::ProjectDepsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::scanner_tools::project_deps(req.project.as_deref());
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === AUDIT TOOLS (5 tools) ===

    #[tool(description = "Audit code quality: find dead code, fragmentation, loose ends (TODO/FIXME/HACK), empty impls, and incomplete patterns. Works on any project in the registry or by absolute path.")]
    async fn audit_code(
        &self,
        Parameters(req): Parameters<types::AuditCodeRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_code(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Security audit: scan for hardcoded secrets, unsafe code, command injection vectors, path traversal, and dependency CVEs (via cargo audit). Returns findings by severity.")]
    async fn audit_security(
        &self,
        Parameters(req): Parameters<types::AuditSecurityRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_security(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Intent verification: check if code matches its purpose. Finds stub functions, untested modules, missing module files, and compares README claims against actual source. Optionally provide a description of intended functionality.")]
    async fn audit_intent(
        &self,
        Parameters(req): Parameters<types::AuditIntentRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_intent(&req.project, req.description.as_deref().unwrap_or(""));
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Dependency health audit: check for wildcard versions, excessive dependencies, duplicate crate versions, and known vulnerabilities in Cargo.lock/package.json.")]
    async fn audit_deps(
        &self,
        Parameters(req): Parameters<types::AuditDepsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_deps(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Full audit: runs code, security, intent, and dependency audits. Returns aggregate grade (A-F), findings by severity, and stores results for trend tracking. Use this for production readiness checks.")]
    async fn audit_full(
        &self,
        Parameters(req): Parameters<types::AuditFullRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::audit_tools::audit_full(&req.project);
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === ORCHESTRATION ===

    #[tool(description = "Orchestrate: say what you want in natural language. AgentOS identifies the project, decomposes into dev + QA + security tasks, spawns agents on free panes, monitors to completion. The 'machine that builds machines' command.")]
    async fn orchestrate(
        &self,
        Parameters(req): Parameters<types::OrchestrateRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::orchestrate::orchestrate(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    // === GATEWAY (MICRO MCP MANAGEMENT) ===

    #[tool(description = "Discover micro MCPs matching a capability keyword. Optionally auto-start them. Use this to find composable building blocks.")]
    async fn mcp_discover(
        &self,
        Parameters(req): Parameters<types::GatewayDiscoverRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_discover(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "Call a tool on a micro MCP. Auto-starts the MCP if not running. Routes through the gateway for lifecycle management.")]
    async fn mcp_call(
        &self,
        Parameters(req): Parameters<types::GatewayCallRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_call(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "List all registered and running micro MCPs. Shows tool counts, uptime, and last-used timestamps.")]
    async fn mcp_gateway_list(
        &self,
        Parameters(req): Parameters<types::GatewayListRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = tools::gateway_tools::gateway_list(&self.app, req).await;
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

#[tool_handler]
impl ServerHandler for AgentOSService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "AgentOS: Terminal orchestrator for AI agent teams. \
                 Spawns, assigns, monitors Claude agents across configurable panes \
                 from a single control plane. Fully autonomous with auto-cycle.".into()
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
