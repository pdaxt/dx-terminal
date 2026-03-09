use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Descriptor for a micro MCP that can be spawned by the gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPDescriptor {
    pub name: String,
    pub command: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub description: String,
}

/// Status of a running micro MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPStatus {
    pub name: String,
    pub running: bool,
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub uptime_secs: u64,
    pub last_used_secs_ago: u64,
}

/// Result of an MCP call routed through the gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPCallResult {
    pub mcp: String,
    pub tool: String,
    pub success: bool,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Build context for multi-agent application builds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContext {
    pub project: String,
    pub spec: String,
    pub features: Vec<BuildFeature>,
    pub artifacts: Vec<BuildArtifact>,
    #[serde(default)]
    pub status: BuildStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildFeature {
    pub id: String,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: FeatureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArtifact {
    pub feature_id: String,
    pub pane: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub api_endpoints: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BuildStatus {
    #[default]
    Planning,
    Building,
    Merging,
    Testing,
    Deploying,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    #[default]
    Pending,
    Queued,
    Building,
    GatePassing,
    GateFailed,
    ReadyToMerge,
    Merged,
    Done,
}

/// Gate check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub feature_id: String,
    pub checks: Vec<GateCheck>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheck {
    pub name: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
