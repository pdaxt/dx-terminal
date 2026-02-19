use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpawnRequest {
    #[schemars(description = "Pane reference (1-9, theme name like 'cyan', or shortcut like 'c')")]
    pub pane: String,
    #[schemars(description = "Project name or path (fuzzy matched against ~/Projects)")]
    pub project: String,
    #[schemars(description = "Agent role: pm/architect/frontend/backend/qa/security/devops/developer")]
    pub role: Option<String>,
    #[schemars(description = "Task description for the agent")]
    pub task: Option<String>,
    #[schemars(description = "Optional initial prompt to send after launch")]
    pub prompt: Option<String>,
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
    #[schemars(description = "Number of lines to capture (default 20)")]
    pub lines: Option<usize>,
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
