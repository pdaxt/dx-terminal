use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpawnRequest {
    #[schemars(
        description = "Pane reference (1-9, theme name like 'cyan', shortcut like 'c', or 'auto' for the next free lane)"
    )]
    pub pane: String,
    #[schemars(description = "Project name or path (fuzzy matched against ~/Projects)")]
    pub project: String,
    #[schemars(
        description = "Agent role: pm/architect/frontend/backend/qa/security/devops/developer"
    )]
    pub role: Option<String>,
    #[serde(default)]
    #[schemars(description = "Runtime provider: claude, codex, gemini, opencode")]
    pub provider: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional model name to pass through to the provider CLI")]
    pub model: Option<String>,
    #[serde(default)]
    #[schemars(description = "Runtime adapter: pty_native_adapter or tmux_migration_adapter")]
    pub runtime_adapter: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Optional client-generated request id to correlate async launch results"
    )]
    pub client_request_id: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional existing DXOS session contract ID to bind this launch to")]
    pub session_id: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional linked feature ID for DXOS session registration")]
    pub feature_id: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional delivery stage for DXOS policy and session registration")]
    pub stage: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional supervisor session ID for DXOS delegation mapping")]
    pub supervisor_session_id: Option<String>,
    #[schemars(description = "Task description for the agent")]
    pub task: Option<String>,
    #[schemars(description = "Optional initial prompt to send after launch")]
    pub prompt: Option<String>,
    #[serde(default)]
    #[schemars(description = "Run with --dangerously-skip-permissions (default: true)")]
    pub autonomous: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KillRequest {
    #[schemars(description = "Pane reference (1-9, theme name, or shortcut)")]
    pub pane: String,
    #[schemars(description = "Optional reason for killing")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestartRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReassignRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "New project (optional)")]
    pub project: Option<String>,
    #[schemars(description = "New role (optional)")]
    pub role: Option<String>,
    #[schemars(description = "New task description (optional)")]
    pub task: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Issue ID from tracker (e.g. 'MAIL-5')")]
    pub issue_id: String,
    #[schemars(description = "Tracker space name (e.g. 'mailforge')")]
    pub space: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignAdhocRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Task description")]
    pub task: String,
    #[schemars(description = "Agent role (default: developer)")]
    pub role: Option<String>,
    #[schemars(description = "Project name or path")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CollectRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Completion summary")]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetMcpsRequest {
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "List of MCP names to enable")]
    pub mcps: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPreambleRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Preamble markdown content")]
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConfigShowRequest {
    #[schemars(description = "Pane reference (optional, shows all if empty)")]
    pub pane: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashboardRequest {
    #[schemars(description = "Output format: 'text' or 'json'")]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogsRequest {
    #[schemars(description = "Pane reference (optional)")]
    pub pane: Option<String>,
    #[schemars(description = "Number of entries (default 20)")]
    pub lines: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpListRequest {
    #[schemars(description = "Filter by category (e.g. 'data', 'infrastructure', 'monitoring')")]
    pub category: Option<String>,
    #[schemars(description = "Filter by project name")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpRouteRequest {
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Task description to route MCPs for")]
    pub task: String,
    #[schemars(description = "Agent role (helps refine MCP selection)")]
    pub role: Option<String>,
    #[schemars(description = "If true, auto-apply the routed MCPs to the project config")]
    pub apply: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpSearchRequest {
    #[schemars(description = "Search query (matches name, description, capabilities, keywords)")]
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitSyncRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitStatusRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Include full diff output")]
    pub verbose: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitPushRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "Commit message (default: auto-generated)")]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitPrRequest {
    #[schemars(description = "Pane reference")]
    pub pane: String,
    #[schemars(description = "PR title (default: task description)")]
    pub title: Option<String>,
    #[schemars(description = "PR body/description")]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitMergeRequest {
    #[schemars(description = "Pane reference (1-9, theme name, or pane-N/branch)")]
    pub pane: String,
    #[schemars(description = "Branch to merge (default: pane's current branch)")]
    pub branch: Option<String>,
}

// === QUEUE / AUTO-CYCLE ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueAddRequest {
    #[schemars(description = "Project name or path")]
    pub project: String,
    #[schemars(description = "Task description")]
    pub task: String,
    #[schemars(description = "Full prompt to send to the agent")]
    pub prompt: Option<String>,
    #[schemars(description = "Agent role (default: developer)")]
    pub role: Option<String>,
    #[schemars(description = "Priority 1-5 (1=highest, default=3)")]
    pub priority: Option<u8>,
    #[schemars(description = "Task IDs this depends on (must complete first)")]
    pub depends_on: Option<Vec<String>>,
    #[schemars(description = "Max retries on failure (default 2, 0=no retry)")]
    pub max_retries: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DecomposeRequest {
    #[schemars(description = "Project name or path")]
    pub project: String,
    #[schemars(
        description = "High-level goal — use numbered steps (1. 2. 3.) or bullet points (- *) to define sub-tasks. Sequential by default; prefix with || for parallel tasks."
    )]
    pub goal: String,
    #[schemars(description = "Max sub-tasks to create (default 5)")]
    pub max_subtasks: Option<u8>,
    #[schemars(description = "Default role for sub-tasks (default: developer)")]
    pub role: Option<String>,
    #[schemars(description = "Priority 1-5 for all sub-tasks (default 3)")]
    pub priority: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueListRequest {
    #[schemars(description = "Filter by status: pending, running, done, failed")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueDoneRequest {
    #[schemars(description = "Task ID to mark done")]
    pub task_id: String,
    #[schemars(description = "Result summary")]
    pub result: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueCancelRequest {
    #[schemars(description = "Task ID to cancel (marks as failed and cascades to dependents)")]
    pub task_id: String,
    #[schemars(description = "Reason for cancellation")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueRetryRequest {
    #[schemars(description = "Task ID to retry (must be failed and under max_retries)")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueClearRequest {
    #[schemars(description = "Clear tasks with this status: done, failed, or all (default: done)")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactoryDetectRequest {
    #[schemars(
        description = "Natural language description — auto-detects which project it refers to"
    )]
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AutoConfigRequest {
    #[schemars(description = "Max parallel panes (1-9)")]
    pub max_parallel: Option<u8>,
    #[schemars(description = "Reserved panes (never auto-assigned)")]
    pub reserved_panes: Option<Vec<u8>>,
    #[schemars(description = "Auto-complete when agent finishes")]
    pub auto_complete: Option<bool>,
    #[schemars(description = "Auto-assign next task when pane frees")]
    pub auto_assign: Option<bool>,
    #[schemars(
        description = "Background auto-cycle interval in seconds (0 = disabled, default 30)"
    )]
    pub cycle_interval_secs: Option<u64>,
}

// === MULTI-AGENT COORDINATION ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PortAllocateRequest {
    #[schemars(description = "Service name (e.g. 'dataxlr8-web')")]
    pub service: String,
    #[schemars(description = "Pane ID (e.g. 'claude6:1.0')")]
    pub pane_id: String,
    #[schemars(description = "Preferred port number")]
    pub preferred: Option<u16>,
    #[schemars(description = "Description of what the port is for")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PortReleaseRequest {
    #[schemars(description = "Port number to release")]
    pub port: u16,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PortGetRequest {
    #[schemars(description = "Service name to look up")]
    pub service: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentRegisterRequest {
    #[schemars(description = "Pane ID (e.g. 'claude6:1.0')")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Current task description")]
    pub task: String,
    #[schemars(description = "Files being worked on")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentUpdateRequest {
    #[schemars(description = "Pane ID")]
    pub pane_id: String,
    #[schemars(description = "Updated task description")]
    pub task: String,
    #[schemars(description = "Updated file list")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentListRequest {
    #[schemars(description = "Filter by project name")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentDeregisterRequest {
    #[schemars(description = "Pane ID to deregister")]
    pub pane_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LockAcquireRequest {
    #[schemars(description = "Pane ID requesting lock")]
    pub pane_id: String,
    #[schemars(description = "File paths to lock")]
    pub files: Vec<String>,
    #[schemars(description = "Reason for locking")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LockReleaseRequest {
    #[schemars(description = "Pane ID releasing locks")]
    pub pane_id: String,
    #[schemars(description = "Specific files to release (empty = release all)")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LockCheckRequest {
    #[schemars(description = "File paths to check")]
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitClaimBranchRequest {
    #[schemars(description = "Pane ID claiming the branch")]
    pub pane_id: String,
    #[schemars(description = "Branch name")]
    pub branch: String,
    #[schemars(description = "Repository name")]
    pub repo: String,
    #[schemars(description = "Purpose of the branch")]
    pub purpose: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitReleaseBranchRequest {
    #[schemars(description = "Pane ID releasing the branch")]
    pub pane_id: String,
    #[schemars(description = "Branch name")]
    pub branch: String,
    #[schemars(description = "Repository name")]
    pub repo: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitListBranchesRequest {
    #[schemars(description = "Filter by repository name")]
    pub repo: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitPreCommitCheckRequest {
    #[schemars(description = "Pane ID doing the commit")]
    pub pane_id: String,
    #[schemars(description = "Repository name")]
    pub repo: String,
    #[schemars(description = "Files being committed")]
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildClaimRequest {
    #[schemars(description = "Pane ID claiming the build")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Build type (e.g. 'cargo', 'npm', 'docker')")]
    pub build_type: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildReleaseRequest {
    #[schemars(description = "Pane ID releasing the build")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Whether the build succeeded")]
    pub success: bool,
    #[schemars(description = "Build output or summary")]
    pub output: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildStatusRequest {
    #[schemars(description = "Project name to check")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildGetLastRequest {
    #[schemars(description = "Project name")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MaTaskAddRequest {
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Task title")]
    pub title: String,
    #[schemars(description = "Task description")]
    pub description: Option<String>,
    #[schemars(description = "Priority: urgent, high, medium, low")]
    pub priority: Option<String>,
    #[schemars(description = "Pane ID adding the task")]
    pub added_by: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MaTaskClaimRequest {
    #[schemars(description = "Pane ID claiming a task")]
    pub pane_id: String,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MaTaskCompleteRequest {
    #[schemars(description = "Task ID to complete")]
    pub task_id: String,
    #[schemars(description = "Pane ID completing the task")]
    pub pane_id: String,
    #[schemars(description = "Result summary")]
    pub result: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MaTaskListRequest {
    #[schemars(description = "Filter by status: pending, claimed, completed, all")]
    pub status: Option<String>,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbAddRequest {
    #[schemars(description = "Pane ID adding knowledge")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Category (e.g. 'code_location', 'bug', 'architecture')")]
    pub category: String,
    #[schemars(description = "Title/summary")]
    pub title: String,
    #[schemars(description = "Full content")]
    pub content: String,
    #[schemars(description = "Related file paths")]
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbSearchRequest {
    #[schemars(description = "Search query")]
    pub query: String,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Filter by category")]
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbListRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Max entries to return (default 20)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MsgBroadcastRequest {
    #[schemars(description = "Sender pane ID")]
    pub from_pane: String,
    #[schemars(description = "Message content")]
    pub message: String,
    #[schemars(description = "Priority: info, warning, urgent")]
    pub priority: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MsgSendRequest {
    #[schemars(description = "Sender pane ID")]
    pub from_pane: String,
    #[schemars(description = "Recipient pane ID")]
    pub to_pane: String,
    #[schemars(description = "Message content")]
    pub message: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MsgGetRequest {
    #[schemars(description = "Pane ID to get messages for")]
    pub pane_id: String,
    #[schemars(description = "Mark messages as read")]
    pub mark_read: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatusOverviewRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

// === TRACKER TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueCreateRequest {
    #[schemars(description = "Collab space name (e.g. 'mailforge')")]
    pub space: String,
    #[schemars(description = "Issue title")]
    pub title: String,
    #[schemars(description = "Type: bug, feature, task, improvement, epic")]
    pub issue_type: Option<String>,
    #[schemars(description = "Priority: critical, high, medium, low")]
    pub priority: Option<String>,
    #[schemars(description = "Markdown description")]
    pub description: Option<String>,
    #[schemars(description = "Assignee")]
    pub assignee: Option<String>,
    #[schemars(description = "Milestone name")]
    pub milestone: Option<String>,
    #[schemars(description = "Labels for categorization")]
    pub labels: Option<Vec<String>>,
    #[schemars(description = "Agent Capacity Units estimate")]
    pub estimated_acu: Option<f64>,
    #[schemars(description = "Agent role (pm/architect/developer/qa/devops)")]
    pub role: Option<String>,
    #[schemars(description = "Sprint assignment")]
    pub sprint: Option<String>,
    #[schemars(description = "Parent issue ID for micro-features (e.g. 'DX-1')")]
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueUpdateFullRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue ID (e.g. 'MAIL-1')")]
    pub issue_id: String,
    #[schemars(description = "New status")]
    pub status: Option<String>,
    #[schemars(description = "New priority")]
    pub priority: Option<String>,
    #[schemars(description = "New assignee")]
    pub assignee: Option<String>,
    #[schemars(description = "New title")]
    pub title: Option<String>,
    #[schemars(description = "New description")]
    pub description: Option<String>,
    #[schemars(description = "New milestone")]
    pub milestone: Option<String>,
    #[schemars(description = "Add a label")]
    pub add_label: Option<String>,
    #[schemars(description = "Remove a label")]
    pub remove_label: Option<String>,
    #[schemars(description = "Estimated ACU")]
    pub estimated_acu: Option<f64>,
    #[schemars(description = "Actual ACU consumed")]
    pub actual_acu: Option<f64>,
    #[schemars(description = "Agent role")]
    pub role: Option<String>,
    #[schemars(description = "Sprint")]
    pub sprint: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueListFilteredRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Filter by status")]
    pub status: Option<String>,
    #[schemars(description = "Filter by type")]
    pub issue_type: Option<String>,
    #[schemars(description = "Filter by priority")]
    pub priority: Option<String>,
    #[schemars(description = "Filter by assignee")]
    pub assignee: Option<String>,
    #[schemars(description = "Filter by milestone")]
    pub milestone: Option<String>,
    #[schemars(description = "Filter by label")]
    pub label: Option<String>,
    #[schemars(description = "Filter by sprint")]
    pub sprint: Option<String>,
    #[schemars(description = "Filter by role")]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueViewRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue ID")]
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueCommentRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue ID")]
    pub issue_id: String,
    #[schemars(description = "Comment text (markdown)")]
    pub text: String,
    #[schemars(description = "Author name")]
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueLinkRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue ID")]
    pub issue_id: String,
    #[schemars(description = "Link type: doc, commit, or pr")]
    pub link_type: String,
    #[schemars(description = "Reference (doc name, commit hash, PR number)")]
    pub reference: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueCloseRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue ID")]
    pub issue_id: String,
    #[schemars(description = "Resolution note")]
    pub resolution: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MilestoneCreateRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Milestone name")]
    pub name: String,
    #[schemars(description = "Description")]
    pub description: Option<String>,
    #[schemars(description = "Due date (YYYY-MM-DD)")]
    pub due_date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MilestoneListRequest {
    #[schemars(description = "Space name")]
    pub space: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineGenerateRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Filter by milestone (empty = all)")]
    pub milestone: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessStartRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Template name")]
    pub template_name: String,
    #[schemars(description = "Context variables as JSON object")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessUpdateRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Process ID")]
    pub process_id: String,
    #[schemars(description = "Step index (0-based)")]
    pub step_index: usize,
    #[schemars(description = "Mark done (true) or undone (false)")]
    pub done: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessListRequest {
    #[schemars(description = "Space name")]
    pub space: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProcessTemplateCreateRequest {
    #[schemars(description = "Template name")]
    pub name: String,
    #[schemars(description = "Markdown checklist with - [ ] items")]
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BoardViewRequest {
    #[schemars(description = "Space name")]
    pub space: String,
}

// === FEATURE MANAGEMENT TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IssueChildrenRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Parent issue ID (e.g. 'DX-1')")]
    pub parent_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeatureDecomposeRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Parent feature/epic issue ID")]
    pub parent_id: String,
    #[schemars(
        description = "Array of micro-features: [{title, description?, priority?, role?, estimated_acu?}]"
    )]
    pub children: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeatureToQueueRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Issue IDs to push to the execution queue")]
    pub issue_ids: Vec<String>,
    #[schemars(
        description = "If true, tasks run sequentially (each depends on previous). If false, all run in parallel."
    )]
    pub sequential: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeatureStatusRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Feature/epic issue ID to show hierarchical status for")]
    pub feature_id: String,
}

// === CAPACITY TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapConfigureRequest {
    #[schemars(description = "Number of parallel panes")]
    pub pane_count: Option<u32>,
    #[schemars(description = "Working hours per day")]
    pub hours_per_day: Option<f64>,
    #[schemars(description = "Productivity factor 0.0-1.0")]
    pub availability_factor: Option<f64>,
    #[schemars(description = "Max human reviews per day")]
    pub review_bandwidth: Option<u32>,
    #[schemars(description = "Max concurrent builds")]
    pub build_slots: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapEstimateRequest {
    #[schemars(description = "Task description")]
    pub description: String,
    #[schemars(description = "Complexity: low, medium, high, very_high")]
    pub complexity: Option<String>,
    #[schemars(description = "Type: bug, task, feature, improvement, epic")]
    pub task_type: Option<String>,
    #[schemars(description = "Role: pm, architect, developer, qa, devops")]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapLogWorkRequest {
    #[schemars(description = "Issue ID")]
    pub issue_id: String,
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Agent role")]
    pub role: String,
    #[schemars(description = "Pane ID")]
    pub pane_id: Option<String>,
    #[schemars(description = "ACU consumed")]
    pub acu_spent: f64,
    #[schemars(description = "Whether human review needed")]
    pub review_needed: Option<bool>,
    #[schemars(description = "Notes")]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapPlanSprintRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Sprint name (auto if empty)")]
    pub name: Option<String>,
    #[schemars(description = "Start date YYYY-MM-DD (default today)")]
    pub start_date: Option<String>,
    #[schemars(description = "Working days (default 5)")]
    pub days: Option<u32>,
    #[schemars(description = "Comma-separated issue IDs")]
    pub issue_ids: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapDashboardRequest {
    #[schemars(description = "Filter by space")]
    pub space: Option<String>,
    #[schemars(description = "Specific sprint ID")]
    pub sprint_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapBurndownRequest {
    #[schemars(description = "Sprint ID (empty = latest active)")]
    pub sprint_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapVelocityRequest {
    #[schemars(description = "Filter by space")]
    pub space: Option<String>,
    #[schemars(description = "Number of sprints to analyze")]
    pub count: Option<usize>,
}

// === COLLAB TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpaceCreateRequest {
    #[schemars(description = "Space name (lowercase, hyphens ok)")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocListRequest {
    #[schemars(description = "Filter by space (empty = all)")]
    pub space: Option<String>,
    #[schemars(description = "Filter by status: draft, review, approved, locked")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocReadRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name (without .md)")]
    pub name: String,
    #[schemars(description = "Include metadata, comments, directives (default true)")]
    pub include_meta: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocCreateRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name (no .md extension)")]
    pub name: String,
    #[schemars(description = "Initial markdown content")]
    pub content: Option<String>,
    #[schemars(description = "Initial status: draft, review, approved")]
    pub status: Option<String>,
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocEditRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "New full markdown content")]
    pub content: String,
    #[schemars(description = "Your agent/pane ID for lock checking")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocProposeRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Full proposed markdown content")]
    pub content: String,
    #[schemars(description = "Brief description of changes")]
    pub summary: Option<String>,
    #[schemars(description = "Your agent/pane ID")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocApproveRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Proposal ID or 'latest'")]
    pub proposal_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocRejectRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Proposal ID to reject")]
    pub proposal_id: String,
    #[schemars(description = "Rejection reason")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocLockRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Who is locking (default: 'human')")]
    pub locked_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocUnlockRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocCommentRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Comment text")]
    pub text: String,
    #[schemars(description = "Author (default: 'claude')")]
    pub author: Option<String>,
    #[schemars(description = "Line number reference (0 = general)")]
    pub line: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocCommentsRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocStatusRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "New status: draft, review, approved, archived")]
    pub status: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocSearchRequest {
    #[schemars(description = "Search text (case-insensitive)")]
    pub query: String,
    #[schemars(description = "Limit to specific space (empty = all)")]
    pub space: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocDirectivesRequest {
    #[schemars(description = "Limit to specific space (empty = all)")]
    pub space: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocHistoryRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Max history entries (default 10)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocDeleteRequest {
    #[schemars(description = "Space name")]
    pub space: String,
    #[schemars(description = "Document name")]
    pub name: String,
    #[schemars(description = "Must be true to delete")]
    pub confirm: Option<bool>,
}

// === MONITORING TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MonitorRequest {
    #[schemars(
        description = "Include PTY output snippets for active panes (default false, saves tokens)"
    )]
    pub include_output: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectStatusRequest {
    #[schemars(description = "Project name (fuzzy matched)")]
    pub project: String,
    #[schemars(description = "Include open issues list (default true)")]
    pub include_issues: Option<bool>,
    #[schemars(description = "Include recent git activity (default true)")]
    pub include_git: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DigestRequest {
    #[schemars(description = "Period: 'today', 'yesterday', 'week', 'month' (default 'today')")]
    pub period: Option<String>,
    #[schemars(description = "Filter by project (empty = all projects)")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchRequest {
    #[schemars(description = "Pane reference (1-9, theme name, or shortcut)")]
    pub pane: String,
    #[schemars(description = "Number of lines to tail (default 30)")]
    pub tail: Option<usize>,
    #[schemars(description = "Include error analysis (default true)")]
    pub analyze_errors: Option<bool>,
}

// === KNOWLEDGE: KGRAPH TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphAddEntityRequest {
    #[schemars(description = "Entity name")]
    pub name: String,
    #[schemars(
        description = "Entity type: project, file, tool, pattern, error, person, concept, mcp, library, platform, config, service, database"
    )]
    pub entity_type: String,
    #[schemars(description = "JSON properties object")]
    pub properties: Option<String>,
    #[schemars(description = "Custom ID (auto-generated if empty)")]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphAddEdgeRequest {
    #[schemars(description = "Source entity (name or ID)")]
    pub source: String,
    #[schemars(description = "Target entity (name or ID)")]
    pub target: String,
    #[schemars(
        description = "Relation: uses, depends_on, causes, fixes, part_of, related_to, conflicts_with, replaced_by, about, solved_by, creates, configures, tests, deploys, documents"
    )]
    pub relation: String,
    #[schemars(description = "Edge weight 0.0-10.0 (default 1.0)")]
    pub weight: Option<f64>,
    #[schemars(description = "JSON properties for the edge")]
    pub properties: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphObserveRequest {
    #[schemars(description = "Source entity (auto-created if new)")]
    pub source: String,
    #[schemars(description = "Target entity (auto-created if new)")]
    pub target: String,
    #[schemars(description = "Relation type")]
    pub relation: String,
    #[schemars(description = "What was observed")]
    pub observation: String,
    #[schemars(description = "Impact -1.0 to 1.0 (adjusts edge weight)")]
    pub impact: Option<f64>,
    #[schemars(description = "Session ID for provenance")]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphQueryNeighborsRequest {
    #[schemars(description = "Entity name or ID")]
    pub entity: String,
    #[schemars(description = "Filter by relation type (empty = all)")]
    pub relation: Option<String>,
    #[schemars(description = "Direction: outgoing, incoming, both (default both)")]
    pub direction: Option<String>,
    #[schemars(description = "Max traversal depth 1-4 (default 1)")]
    pub depth: Option<u32>,
    #[schemars(description = "Max nodes to return (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphQueryPathRequest {
    #[schemars(description = "Source entity")]
    pub source: String,
    #[schemars(description = "Target entity")]
    pub target: String,
    #[schemars(description = "Max hops 1-6 (default 4)")]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphSearchRequest {
    #[schemars(description = "Search query (matches name and properties)")]
    pub query: String,
    #[schemars(description = "Filter by entity type")]
    pub entity_type: Option<String>,
    #[schemars(description = "Max results (default 20)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KgraphDeleteRequest {
    #[schemars(description = "Entity ID to delete (cascades edges)")]
    pub entity_id: Option<String>,
    #[schemars(description = "Edge source (for edge deletion)")]
    pub edge_source: Option<String>,
    #[schemars(description = "Edge target (for edge deletion)")]
    pub edge_target: Option<String>,
    #[schemars(description = "Edge relation (optional, deletes all if empty)")]
    pub edge_relation: Option<String>,
}

// === KNOWLEDGE: SESSION REPLAY TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayIndexRequest {
    #[schemars(description = "Force re-index all sessions (default: incremental)")]
    pub force: Option<bool>,
    #[schemars(description = "Filter by project path substring")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplaySearchRequest {
    #[schemars(description = "Search text in message content")]
    pub query: String,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Filter by tool name")]
    pub tool: Option<String>,
    #[schemars(description = "Max results (default 20)")]
    pub limit: Option<u32>,
    #[schemars(description = "Only search last N days (0 = all)")]
    pub days: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplaySessionRequest {
    #[schemars(description = "Session ID or file path")]
    pub session_id: String,
    #[schemars(description = "Include tool results (default true)")]
    pub include_tools: Option<bool>,
    #[schemars(description = "Include error messages (default true)")]
    pub include_errors: Option<bool>,
    #[schemars(description = "Max messages to return (default 100)")]
    pub max_messages: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayListSessionsRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Only last N days (default 30)")]
    pub days: Option<u32>,
    #[schemars(description = "Max results (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayToolHistoryRequest {
    #[schemars(description = "Tool name to search for")]
    pub tool_name: String,
    #[schemars(description = "Max results (default 20)")]
    pub limit: Option<u32>,
    #[schemars(description = "Only last N days (0 = all)")]
    pub days: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayErrorsRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Only last N days (default 7)")]
    pub days: Option<u32>,
    #[schemars(description = "Max results (default 50)")]
    pub limit: Option<u32>,
}

// === KNOWLEDGE: TRUTHGUARD TOOLS ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactAddRequest {
    #[schemars(description = "Category: identity, project, business, technical, preference")]
    pub category: String,
    #[schemars(description = "Fact key (unique within category)")]
    pub key: String,
    #[schemars(description = "Fact value")]
    pub value: String,
    #[schemars(description = "Confidence 0.0-1.0 (default 1.0)")]
    pub confidence: Option<f64>,
    #[schemars(description = "Source of fact")]
    pub source: Option<String>,
    #[schemars(description = "Alternative names/spellings")]
    pub aliases: Option<Vec<String>>,
    #[schemars(description = "Tags for grouping")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactGetRequest {
    #[schemars(description = "Fact ID (direct lookup)")]
    pub fact_id: Option<String>,
    #[schemars(description = "Fact key")]
    pub key: Option<String>,
    #[schemars(description = "Category (helps narrow key lookup)")]
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactSearchRequest {
    #[schemars(description = "Search text (matches key, value, aliases)")]
    pub query: Option<String>,
    #[schemars(description = "Filter by category")]
    pub category: Option<String>,
    #[schemars(description = "Minimum confidence threshold (default 0.0)")]
    pub min_confidence: Option<f64>,
    #[schemars(description = "Max results (default 20)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactCheckRequest {
    #[schemars(description = "Claim text to verify against known facts")]
    pub claim: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactCheckResponseRequest {
    #[schemars(description = "Full response text to check for contradictions")]
    pub response_text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactUpdateRequest {
    #[schemars(description = "Fact ID")]
    pub fact_id: Option<String>,
    #[schemars(description = "Category (for key-based lookup)")]
    pub category: Option<String>,
    #[schemars(description = "Key (for key-based lookup)")]
    pub key: Option<String>,
    #[schemars(description = "New value")]
    pub value: Option<String>,
    #[schemars(description = "New confidence")]
    pub confidence: Option<f64>,
    #[schemars(description = "New aliases")]
    pub aliases: Option<Vec<String>>,
    #[schemars(description = "Source of update")]
    pub source: Option<String>,
    #[schemars(description = "New tags")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactDeleteRequest {
    #[schemars(description = "Fact ID to delete")]
    pub fact_id: String,
    #[schemars(description = "Reason for deletion (for audit log)")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MachineInfoRequest {
    #[schemars(description = "Pane reference (number or theme name). Omit to list all machines.")]
    pub pane: Option<String>,
}

// ============================================================================
// ANALYTICS TYPES
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogToolCallRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Full tool name (e.g. mcp__google-cloud__sheets_read)")]
    pub tool_name: String,
    #[schemars(description = "Input size in bytes")]
    pub input_size: Option<i64>,
    #[schemars(description = "Output size in bytes")]
    pub output_size: Option<i64>,
    #[schemars(description = "Latency in milliseconds")]
    pub latency_ms: Option<i64>,
    #[schemars(description = "Whether the tool call succeeded")]
    pub success: Option<bool>,
    #[schemars(description = "First 200 chars of error if failed")]
    pub error_preview: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogFileOpRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "File path operated on")]
    pub file_path: String,
    #[schemars(description = "Operation type: read, write, edit, delete")]
    pub operation: String,
    #[schemars(description = "Number of lines changed")]
    pub lines_changed: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogTokensRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Model name (e.g. claude-sonnet-4-5-20250929)")]
    pub model: String,
    #[schemars(description = "Input tokens")]
    pub input_tokens: i64,
    #[schemars(description = "Output tokens")]
    pub output_tokens: i64,
    #[schemars(description = "Cache read tokens")]
    pub cache_read: Option<i64>,
    #[schemars(description = "Cache write tokens")]
    pub cache_write: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogGitCommitRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Repository path")]
    pub repo_path: Option<String>,
    #[schemars(description = "Commit hash")]
    pub commit_hash: String,
    #[schemars(description = "Branch name")]
    pub branch: Option<String>,
    #[schemars(description = "Commit message")]
    pub message: String,
    #[schemars(description = "Files changed count")]
    pub files_changed: Option<i64>,
    #[schemars(description = "Lines inserted")]
    pub insertions: Option<i64>,
    #[schemars(description = "Lines deleted")]
    pub deletions: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UsageReportRequest {
    #[schemars(description = "Filter by pane_id")]
    pub pane_id: Option<String>,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Number of days to look back (default 7)")]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ToolRankingRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Number of days to look back (default 7)")]
    pub days: Option<i64>,
    #[schemars(description = "Max tools to return (default 20)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpHealthRequest {
    #[schemars(description = "Number of days to look back (default 7)")]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentActivityRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Max events to return (default 50)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CostReportRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Number of days (default 30)")]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrendsRequest {
    #[schemars(description = "Metric: tool_calls, tokens, errors, files, commits")]
    pub metric: String,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Granularity: daily, weekly, monthly (default daily)")]
    pub granularity: Option<String>,
    #[schemars(description = "Number of days of data (default 30)")]
    pub periods: Option<i64>,
}

// ============================================================================
// QUALITY TYPES
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogTestRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Test command run")]
    pub command: Option<String>,
    #[schemars(description = "Whether tests passed")]
    pub success: bool,
    #[schemars(description = "Total test count")]
    pub total: Option<i64>,
    #[schemars(description = "Passed count")]
    pub passed: Option<i64>,
    #[schemars(description = "Failed count")]
    pub failed: Option<i64>,
    #[schemars(description = "Skipped count")]
    pub skipped: Option<i64>,
    #[schemars(description = "Duration in milliseconds")]
    pub duration_ms: Option<i64>,
    #[schemars(description = "Test output (truncated)")]
    pub output: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogBuildRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Build command")]
    pub command: Option<String>,
    #[schemars(description = "Whether build succeeded")]
    pub success: bool,
    #[schemars(description = "Duration in milliseconds")]
    pub duration_ms: Option<i64>,
    #[schemars(description = "Build output (truncated)")]
    pub output: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogLintRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Lint command")]
    pub command: Option<String>,
    #[schemars(description = "Whether lint passed")]
    pub success: bool,
    #[schemars(description = "Total issues")]
    pub total: Option<i64>,
    #[schemars(description = "Error count")]
    pub errors: Option<i64>,
    #[schemars(description = "Warning count")]
    pub warnings: Option<i64>,
    #[schemars(description = "Lint output")]
    pub output: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogDeployRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Deploy target (production, staging, etc)")]
    pub target: Option<String>,
    #[schemars(description = "Whether deploy succeeded")]
    pub success: bool,
    #[schemars(description = "Duration in milliseconds")]
    pub duration_ms: Option<i64>,
    #[schemars(description = "Deploy output")]
    pub output: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QualityReportRequest {
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Number of days (default 7)")]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QualityGateRequest {
    #[schemars(description = "Project name")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegressionsRequest {
    #[schemars(description = "Project name")]
    pub project: String,
    #[schemars(description = "Number of days to compare (default 14)")]
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectHealthRequest {
    #[schemars(description = "Project name")]
    pub project: String,
}

// ============================================================================
// DASHBOARD TYPES
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashOverviewRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashAgentDetailRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashProjectRequest {
    #[schemars(description = "Project name")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashLeaderboardRequest {
    #[schemars(description = "Number of days (default 7)")]
    pub days: Option<i64>,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashTimelineRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Filter by pane_id")]
    pub pane_id: Option<String>,
    #[schemars(description = "Max events (default 50)")]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashAlertsRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashDailyDigestRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashExportRequest {
    #[schemars(description = "Report type: agents, usage, quality")]
    pub report: String,
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
    #[schemars(description = "Number of days (default 30)")]
    pub days: Option<i64>,
}

// ============================================================================
// LIFECYCLE TYPES
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HeartbeatRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Current task (optional update)")]
    pub task: Option<String>,
    #[schemars(description = "Status: active, idle, busy")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionStartRequest {
    #[schemars(description = "Agent pane_id")]
    pub pane_id: String,
    #[schemars(description = "Project name")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionEndRequest {
    #[schemars(description = "Session ID to end")]
    pub session_id: String,
    #[schemars(description = "Summary of what was done")]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LockStealRequest {
    #[schemars(description = "Agent pane_id stealing the lock")]
    pub pane_id: String,
    #[schemars(description = "File path to steal lock for")]
    pub file_path: String,
    #[schemars(description = "Justification for stealing")]
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConflictScanRequest {
    #[schemars(description = "Filter by project")]
    pub project: Option<String>,
}

// --- Project Intelligence ---

#[derive(Debug, Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct ProjectScanRequest {
    #[schemars(description = "Force a full rescan even if recently scanned")]
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectListRequest {
    #[schemars(description = "Filter by tech stack (e.g. 'rust', 'node', 'python')")]
    pub tech: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectDetailRequest {
    #[schemars(description = "Project name (fuzzy matched)")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectTestRequest {
    #[schemars(description = "Project name to run tests for")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectDepsRequest {
    #[schemars(description = "Project name (omit for full dep graph)")]
    pub project: Option<String>,
}

// === ORCHESTRATE TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrchestrateRequest {
    #[schemars(
        description = "Natural language request: what you want built/done. DX Terminal will identify the project, decompose into tasks, spawn developer + QA + security agents."
    )]
    pub request: String,
    #[schemars(description = "Explicit project name (auto-detected from request if empty)")]
    pub project: Option<String>,
    #[schemars(
        description = "Run QA agent concurrently with developer (default true). If false, QA runs after developer completes."
    )]
    pub concurrent_qa: Option<bool>,
    #[schemars(
        description = "Run security audit concurrently (default false). If true, security agent watches in real-time."
    )]
    pub concurrent_security: Option<bool>,
    #[schemars(
        description = "Max panes to use for this orchestration (default 3: dev + qa + security)"
    )]
    pub max_panes: Option<u8>,
}

// === FACTORY TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactoryRequest {
    #[schemars(
        description = "What to build/fix/test (natural language). Project auto-detected if not specified."
    )]
    pub request: String,
    #[schemars(
        description = "Project name (optional — auto-detected from request text if omitted)"
    )]
    pub project: Option<String>,
    #[schemars(description = "Pipeline template: full, quick, secure (default: full)")]
    pub template: Option<String>,
    #[schemars(description = "Priority 1-5 (default: 1)")]
    pub priority: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FactoryStatusRequest {
    #[schemars(description = "Pipeline ID (omit to list all pipelines)")]
    pub pipeline_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FactoryRetryStageRequest {
    #[schemars(description = "Pipeline ID")]
    pub pipeline_id: String,
    #[schemars(description = "Stage name to retry (e.g., 'dev', 'qa', 'security')")]
    pub stage: String,
}

// === SIGNAL TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SignalRequest {
    #[schemars(description = "Your pane ID (e.g. '3' or 'claude6:3.0')")]
    pub pane_id: String,
    #[schemars(description = "Signal type: need_help, blocked, found_issue, completed, failed")]
    pub signal_type: String,
    #[schemars(description = "Details about what you need or found")]
    pub message: String,
    #[schemars(description = "Pipeline ID if this relates to a factory pipeline")]
    pub pipeline_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SignalAckRequest {
    #[schemars(description = "Signal ID to acknowledge")]
    pub signal_id: i64,
}

// === GATEWAY TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GatewayDiscoverRequest {
    #[schemars(
        description = "Capability keyword to search for (e.g. 'knowledge', 'testing', 'email')"
    )]
    pub capability: String,
    #[schemars(description = "Auto-start matching MCPs (default false)")]
    pub auto_start: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GatewayCallRequest {
    #[schemars(description = "MCP name to call")]
    pub mcp: String,
    #[schemars(description = "Tool name on that MCP")]
    pub tool: String,
    #[schemars(description = "Arguments as JSON object")]
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GatewayListRequest {
    #[schemars(description = "Show only running MCPs (default: show all)")]
    pub running_only: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GatewayToolsRequest {
    #[schemars(description = "MCP name to inspect")]
    pub mcp: String,
    #[schemars(description = "Auto-start the MCP if needed (default true)")]
    pub auto_start: Option<bool>,
}

// === AUDIT TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditCodeRequest {
    #[schemars(description = "Project name or absolute path to audit")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditSecurityRequest {
    #[schemars(description = "Project name or absolute path to audit")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditIntentRequest {
    #[schemars(description = "Project name or absolute path to audit")]
    pub project: String,
    #[schemars(
        description = "Description of what the project should do (for intent verification)"
    )]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditDepsRequest {
    #[schemars(description = "Project name or absolute path to audit")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditFullRequest {
    #[schemars(description = "Project name or absolute path to audit")]
    pub project: String,
}

// === UI/UX AUDIT TYPES ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UiAuditRequest {
    #[schemars(description = "File path to audit (HTML/CSS). Defaults to embedded dashboard.")]
    pub file: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UxAuditRequest {
    #[schemars(description = "URL to audit for UX issues (e.g. http://localhost:3100)")]
    pub url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContrastCheckRequest {
    #[schemars(description = "Foreground color as hex (e.g. '#d4dce8')")]
    pub fg: String,
    #[schemars(description = "Background color as hex (e.g. '#0a0e14')")]
    pub bg: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DesignTokensRequest {}

// ── Vision-Driven Development ──

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionTreeRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionDrillRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Goal ID to drill into (e.g. 'G1')")]
    pub goal_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionWorkRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Work description — matched against vision goals")]
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionFeatureRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Goal ID to add feature under")]
    pub goal_id: String,
    #[schemars(description = "Feature title")]
    pub title: String,
    #[schemars(description = "Feature description")]
    pub description: String,
    #[schemars(description = "Acceptance criteria")]
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionQuestionRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Question text")]
    pub question: String,
    #[schemars(description = "Whether this question blocks discovery completion (default: true)")]
    #[serde(default)]
    pub blocking: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionAcceptanceRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Acceptance criterion text")]
    pub criterion: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionAcceptanceUpdateRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Acceptance criterion ID (e.g. 'AC1.1.1')")]
    pub criterion_id: String,
    #[schemars(description = "Updated criterion text")]
    pub text: Option<String>,
    #[schemars(
        description = "Verification method, e.g. integration_test/manual_review/pipeline_gate"
    )]
    pub verification_method: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionAcceptanceVerifyRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Acceptance criterion ID (e.g. 'AC1.1.1')")]
    pub criterion_id: String,
    #[schemars(description = "New criterion status: draft/mapped/verified/failed")]
    pub status: String,
    #[schemars(description = "Evidence refs, e.g. test IDs, artifact paths, URLs")]
    #[serde(default)]
    pub evidence: Vec<String>,
    #[schemars(description = "Actor who recorded the verification, provider-neutral")]
    pub verified_by: Option<String>,
    #[schemars(description = "Source of verification, e.g. human/agent/pipeline")]
    pub verification_source: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionDocUpsertRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Markdown document content")]
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionMockupSeedRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Reference or style hint, e.g. 'website like Shopify'")]
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionDesignReviewRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. 'F1.1')")]
    pub feature_id: String,
    #[schemars(description = "Design option ID (e.g. 'MO1.1.1')")]
    pub option_id: String,
    #[schemars(description = "Review status: draft/proposed/approved/rejected")]
    pub status: String,
    #[schemars(description = "Optional review note")]
    pub note: Option<String>,
    #[schemars(description = "Reviewer identity, provider-neutral")]
    pub actor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionAnswerRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID")]
    pub feature_id: String,
    #[schemars(description = "Question ID to answer")]
    pub question_id: String,
    #[schemars(description = "Answer text")]
    pub answer: String,
    #[schemars(description = "Rationale for the decision")]
    pub rationale: String,
    #[schemars(description = "Alternatives considered")]
    #[serde(default)]
    pub alternatives: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionTaskRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID")]
    pub feature_id: String,
    #[schemars(description = "Task title")]
    pub title: String,
    #[schemars(description = "Task description")]
    #[serde(default)]
    pub description: Option<String>,
    #[schemars(description = "Git branch name")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionFeatureStatusRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. F3.1)")]
    pub feature_id: String,
    #[schemars(description = "New status: planned/specifying/building/testing/done")]
    pub status: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionFeatureReadinessRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID (e.g. F3.1)")]
    pub feature_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionTaskStatusRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Feature ID")]
    pub feature_id: String,
    #[schemars(description = "Task ID")]
    pub task_id: String,
    #[schemars(description = "New status: planned/in_progress/done/verified/blocked")]
    pub status: String,
    #[schemars(description = "Git branch name")]
    pub branch: Option<String>,
    #[schemars(description = "PR number or URL")]
    pub pr: Option<String>,
    #[schemars(description = "Merge commit hash")]
    pub commit: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionSyncRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
}

// ── Vision Init / Goal Management ──

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionInitRequest {
    #[schemars(description = "Project path (e.g. '/Users/pran/Projects/dataxlr8')")]
    pub project: String,
    #[schemars(description = "Project name (e.g. 'dataxlr8')")]
    pub name: String,
    #[schemars(description = "Project mission statement")]
    pub mission: String,
    #[schemars(description = "GitHub repo (e.g. 'pdaxt/dataxlr8')")]
    #[serde(default)]
    pub repo: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionAddGoalRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Goal ID (e.g. 'G-AUTH')")]
    pub id: String,
    #[schemars(description = "Goal title")]
    pub title: String,
    #[schemars(description = "Goal description")]
    pub description: String,
    #[schemars(description = "Priority: 1=critical, 2=high, 3=medium")]
    #[serde(default = "default_priority")]
    pub priority: u8,
}

fn default_priority() -> u8 {
    2
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VisionUpdateGoalRequest {
    #[schemars(description = "Project path")]
    pub project: Option<String>,
    #[schemars(description = "Goal ID")]
    pub goal_id: String,
    #[schemars(description = "New status: planned/in_progress/achieved/deferred/dropped")]
    pub status: String,
}

// ── DXOS Control Plane / Debate Engine ──

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosControlPlaneRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosProviderPluginsRequest {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosProviderPluginSyncRequest {
    #[schemars(
        description = "Optional source provider to convert from: claude, codex/openai/gpt, gemini"
    )]
    pub source_provider: Option<String>,
    #[schemars(description = "Target provider bridge to export: claude, codex/openai/gpt, gemini")]
    pub target_provider: String,
    #[schemars(
        description = "If true, return the translated payload without writing the provider bridge file"
    )]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateListRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosAdoptionStartRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Short adoption or recovery summary shown in the portal")]
    pub summary: Option<String>,
    #[schemars(
        description = "Detailed adoption objective for the seeded recovery lead and council"
    )]
    pub objective: Option<String>,
    #[schemars(
        description = "Optional linked feature ID if adoption should start around one feature"
    )]
    pub feature_id: Option<String>,
    #[schemars(description = "Starting stage, usually discovery")]
    pub stage: Option<String>,
    #[schemars(description = "Initial council or participant identities")]
    #[serde(default)]
    pub participants: Vec<String>,
    #[schemars(description = "Actor requesting the adoption workflow")]
    pub requested_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosAdoptionStatusRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Adoption workflow ID")]
    pub adoption_id: String,
    #[schemars(description = "New adoption status: planned/active/completed/cancelled")]
    pub status: String,
    #[schemars(description = "Optional handoff or closure note")]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateStartRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Debate title")]
    pub title: String,
    #[schemars(description = "Decision objective or question to resolve")]
    pub objective: String,
    #[schemars(description = "Optional delivery stage, e.g. discovery/design/build")]
    pub stage: Option<String>,
    #[schemars(description = "Optional linked feature ID")]
    pub feature_id: Option<String>,
    #[schemars(description = "Participant identities, models, or roles")]
    #[serde(default)]
    pub participants: Vec<String>,
    #[schemars(description = "Actor requesting the debate")]
    pub requested_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateProposalRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Debate ID")]
    pub debate_id: String,
    #[schemars(description = "Author identity")]
    pub author: String,
    #[schemars(description = "Optional model identifier")]
    pub model: Option<String>,
    #[schemars(description = "Short proposal summary")]
    pub summary: String,
    #[schemars(description = "Detailed reasoning for the proposal")]
    pub rationale: String,
    #[schemars(description = "Evidence refs, artifact paths, URLs, or citations")]
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateContradictionRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Debate ID")]
    pub debate_id: String,
    #[schemars(description = "Proposal ID being challenged")]
    pub proposal_id: String,
    #[schemars(description = "Author identity")]
    pub author: String,
    #[schemars(description = "Optional model identifier")]
    pub model: Option<String>,
    #[schemars(description = "Contradicting rationale")]
    pub rationale: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateVoteRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Debate ID")]
    pub debate_id: String,
    #[schemars(description = "Proposal ID being voted on")]
    pub proposal_id: String,
    #[schemars(description = "Voter identity")]
    pub voter: String,
    #[schemars(description = "Optional model identifier")]
    pub model: Option<String>,
    #[schemars(description = "Vote stance: support/oppose/abstain")]
    pub stance: String,
    #[schemars(description = "Vote rationale")]
    pub rationale: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosDebateFinalizeRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Debate ID")]
    pub debate_id: String,
    #[schemars(description = "Chosen proposal ID")]
    pub chosen_proposal_id: String,
    #[schemars(description = "Actor finalizing the decision")]
    pub decided_by: String,
    #[schemars(description = "Final decision summary")]
    pub summary: String,
    #[schemars(description = "Decision rationale")]
    pub rationale: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosSessionUpsertRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Existing session ID to update; omitted to create a new contract")]
    pub session_id: Option<String>,
    #[schemars(description = "Session role, e.g. design/frontend/qa/security")]
    pub role: String,
    #[schemars(description = "Provider ID, e.g. claude/openai/gemini")]
    pub provider: Option<String>,
    #[schemars(description = "Model ID, e.g. claude-opus-4.6")]
    pub model: Option<String>,
    #[schemars(description = "Autonomy level, default guarded_auto")]
    pub autonomy_level: Option<String>,
    #[schemars(description = "Primary objective for the session")]
    pub objective: String,
    #[schemars(description = "Expected outputs or artifacts")]
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[schemars(description = "Capabilities the session is allowed to use")]
    #[serde(default)]
    pub allowed_capabilities: Vec<String>,
    #[schemars(description = "Repos this session can operate in")]
    #[serde(default)]
    pub allowed_repos: Vec<String>,
    #[schemars(description = "Paths this session can operate in")]
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[schemars(description = "Workspace path")]
    pub workspace_path: Option<String>,
    #[schemars(description = "Branch name")]
    pub branch_name: Option<String>,
    #[schemars(description = "Owned browser port")]
    pub browser_port: Option<u16>,
    #[schemars(description = "Owned pane number")]
    pub pane: Option<u8>,
    #[schemars(description = "Runtime adapter ID")]
    pub runtime_adapter: Option<String>,
    #[schemars(description = "tmux target or runtime target")]
    pub tmux_target: Option<String>,
    #[schemars(description = "Linked feature ID")]
    pub feature_id: Option<String>,
    #[schemars(description = "Delivery stage")]
    pub stage: Option<String>,
    #[schemars(description = "Supervisor session ID if this is a child session")]
    pub supervisor_session_id: Option<String>,
    #[schemars(description = "Escalation policy text")]
    pub escalation_policy: Option<String>,
    #[schemars(description = "Initial status, default active")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosSessionListRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosSessionStatusRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Session ID")]
    pub session_id: String,
    #[schemars(description = "New status, e.g. active/blocked/completed/idle")]
    pub status: String,
    #[schemars(description = "Optional status note")]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosWorkDelegateRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Supervisor session ID")]
    pub supervisor_session_id: String,
    #[schemars(description = "Optional worker session ID")]
    pub worker_session_id: Option<String>,
    #[schemars(description = "Work order title")]
    pub title: String,
    #[schemars(description = "Delegated objective")]
    pub objective: String,
    #[schemars(description = "Linked feature ID")]
    pub feature_id: Option<String>,
    #[schemars(description = "Delivery stage")]
    pub stage: Option<String>,
    #[schemars(description = "Required capabilities")]
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[schemars(description = "Expected outputs")]
    #[serde(default)]
    pub expected_outputs: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosWorkBlockRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Work order ID")]
    pub work_order_id: String,
    #[schemars(description = "Blocking reason")]
    pub blocker: String,
    #[schemars(description = "Requested permission or approval")]
    pub requested_permission: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosWorkResolveRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Work order ID")]
    pub work_order_id: String,
    #[schemars(description = "Optional resolution summary")]
    pub resolution: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DxosSessionRaiseBlockerRequest {
    #[schemars(description = "Project path (default: current directory)")]
    pub project: Option<String>,
    #[schemars(description = "Worker session ID raising the blocker")]
    pub worker_session_id: String,
    #[schemars(description = "Blocking reason or question")]
    pub blocker: String,
    #[schemars(description = "Requested permission or approval, if any")]
    pub requested_permission: Option<String>,
    #[schemars(description = "Optional hint about the guidance or resolution needed")]
    pub resolution_hint: Option<String>,
}

// ── Screen Management ──

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddScreenRequest {
    #[schemars(
        description = "Screen name (e.g., 'Dev Screen', 'QA Screen'). Auto-generated if not provided."
    )]
    pub name: Option<String>,
    #[schemars(
        description = "Layout: single, split2, horizontal (default, 3 panes), vertical, grid2x2"
    )]
    pub layout: Option<String>,
    #[schemars(description = "Override number of panes (default: based on layout)")]
    pub panes: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveScreenRequest {
    #[schemars(description = "Screen ID (number) or name to remove")]
    pub screen: String,
    #[schemars(description = "Force remove even if agents are active (default: false)")]
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListScreensRequest {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenSummaryRequest {}

// === Build Environment Types ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildEnvStatusRequest {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildEnvCreateRequest {
    #[schemars(description = "Build number 1-5 (auto-assigns next if omitted)")]
    pub number: Option<u8>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildEnvRestyleRequest {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildEnvSendRequest {
    #[schemars(description = "Build number (1-5)")]
    pub build: u8,
    #[schemars(description = "Pane number within the build (1-3)")]
    pub pane: u8,
    #[schemars(description = "Shell command to execute in the pane")]
    pub command: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildEnvRenameRequest {
    #[schemars(description = "Build number to rename (1-5)")]
    pub build: u8,
    #[schemars(description = "New window name")]
    pub name: String,
}
