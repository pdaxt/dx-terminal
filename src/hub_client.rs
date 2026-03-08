//! DX Hub client — reads state from hub API. Pure API consumer.

use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use serde::Deserialize;
use serde_json::Value;

use crate::agents::{AgentStatus, AgentType, MonitoredAgent};
use crate::state_reader::{
    ActivityEntry, AutoCycleConfig, BoardData, CapacityData, DashboardData, McpServer,
    MilestoneData, MultiAgentEntry, ProcessData, SessionData, SprintData,
};

const DEFAULT_API_URL: &str = "http://localhost:3100";

// =============================================================================
// Hub API response types (match hub_mcp JSON exactly)
// =============================================================================

/// Hub pane state from the /api/status endpoint
#[derive(Debug, Deserialize)]
pub struct HubPane {
    #[serde(default)]
    pub pane: u8,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub theme_color: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub role_full: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub pty_active: bool,
    #[serde(default)]
    pub pty_running: bool,
    #[serde(default)]
    pub line_count: usize,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub acu: f64,
    #[serde(default)]
    pub space: Option<String>,
    #[serde(default)]
    pub issue_id: Option<String>,
}

/// Queue task from /api/queue endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct QueueTask {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub pane: Option<u8>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub added_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
}

/// Factory pipeline request from /api/factory/status
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FactoryRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub request: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub classification: FactoryClassification,
    #[serde(default)]
    pub tasks: Vec<FactoryTask>,
    #[serde(default)]
    pub created_at: String,
}

/// Classification result for a factory request
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FactoryClassification {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub role: String,
    #[serde(rename = "type", default)]
    pub req_type: String,
}

/// A single task in a factory pipeline
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FactoryTask {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub pane: Option<u8>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Factory inbox from /api/factory/status
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FactoryInbox {
    #[serde(default)]
    pub requests: Vec<FactoryRequest>,
}

/// Factory submit response
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FactorySubmitResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub factory_id: String,
    #[serde(default)]
    pub message: String,
}

/// 24h daily digest from /api/analytics/digest
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AnalyticsDigest {
    #[serde(default)]
    pub tool_calls: i64,
    #[serde(default)]
    pub errors: i64,
    #[serde(default)]
    pub error_rate: String,
    #[serde(default)]
    pub commits: i64,
    #[serde(default)]
    pub files_touched: i64,
    #[serde(default)]
    pub agents_active: i64,
    #[serde(default)]
    pub tasks_completed: i64,
}

/// Single alert from /api/analytics/alerts
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Alert {
    #[serde(default)]
    pub level: String,
    #[serde(rename = "type", default)]
    pub alert_type: String,
    #[serde(default)]
    pub pane_id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub error_rate: Option<String>,
}

/// Alerts response from /api/analytics/alerts
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AlertsResponse {
    #[serde(default)]
    pub alerts: Vec<Alert>,
    #[serde(default)]
    pub count: i64,
}

// =============================================================================
// Internal API response wrappers (for deserialization)
// =============================================================================

#[derive(Debug, Deserialize)]
struct StatusResponse {
    panes: Vec<HubPane>,
}

#[derive(Debug, Deserialize)]
struct QueueResponse {
    tasks: Vec<QueueTask>,
}

/// Aggregate /api/dashboard response
#[derive(Debug, Deserialize, Default)]
struct DashboardApiResponse {
    #[serde(default)]
    capacity: CapacityData,
    #[serde(default)]
    sprints: Vec<Value>,
    #[serde(default)]
    board_summary: Vec<BoardSummaryEntry>,
    #[serde(default)]
    mcps: Vec<ApiMcpEntry>,
    #[serde(default)]
    activity: Vec<ActivityEntry>,
    #[serde(default)]
    auto_config: AutoCycleConfig,
    #[serde(default)]
    session: SessionData,
    #[serde(default)]
    milestones: Vec<MilestoneData>,
    #[serde(default)]
    processes: Vec<ProcessData>,
    #[serde(default)]
    agents: Vec<ApiMultiAgentEntry>,
    #[serde(default)]
    digest: AnalyticsDigest,
    #[serde(default)]
    alerts: AlertsResponse,
}

#[derive(Debug, Deserialize, Default)]
struct BoardSummaryEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    counts: HashMap<String, usize>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiMcpEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    tools: i64,
    #[serde(default)]
    is_rust: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ApiMultiAgentEntry {
    #[serde(default)]
    pane: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    task: String,
}

// =============================================================================
// Full dashboard result (dashboard data + analytics in one fetch)
// =============================================================================

pub struct FullDashboardResult {
    pub dashboard: DashboardData,
    pub digest: AnalyticsDigest,
    pub alerts: AlertsResponse,
}

// =============================================================================
// Hub Client
// =============================================================================

pub struct HubClient {
    api_url: String,
    client: reqwest::Client,
}

impl HubClient {
    pub fn new(api_url: Option<String>) -> Self {
        Self {
            api_url: api_url.unwrap_or_else(|| DEFAULT_API_URL.to_string()),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("failed to create HTTP client"),
        }
    }

    /// Fetch pane states from hub API
    pub async fn fetch_panes(&self) -> anyhow::Result<Vec<HubPane>> {
        let url = format!("{}/api/status", self.api_url);
        let resp: StatusResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp.panes)
    }

    /// Fetch queue tasks from hub API
    pub async fn fetch_queue(&self) -> anyhow::Result<Vec<QueueTask>> {
        let url = format!("{}/api/queue", self.api_url);
        let resp: QueueResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp.tasks)
    }

    /// Fetch 24h analytics digest
    pub async fn fetch_digest(&self) -> anyhow::Result<AnalyticsDigest> {
        let url = format!("{}/api/analytics/digest", self.api_url);
        let resp: AnalyticsDigest = self.client.get(&url).send().await?.json().await?;
        Ok(resp)
    }

    /// Fetch active alerts
    pub async fn fetch_alerts(&self) -> anyhow::Result<AlertsResponse> {
        let url = format!("{}/api/analytics/alerts", self.api_url);
        let resp: AlertsResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp)
    }

    /// Fetch ALL dashboard data + analytics in one HTTP call
    pub async fn fetch_dashboard(&self) -> anyhow::Result<FullDashboardResult> {
        let url = format!("{}/api/dashboard", self.api_url);
        let resp: DashboardApiResponse = self.client.get(&url).send().await?.json().await?;

        // Convert sprints → SprintData
        let sprint = Self::parse_sprint(&resp.sprints);

        // Convert board_summary → BoardData
        let board = BoardData {
            spaces: resp
                .board_summary
                .into_iter()
                .map(|b| (b.name, b.counts))
                .collect(),
        };

        // Convert MCPs
        let mcps = resp
            .mcps
            .into_iter()
            .map(|m| McpServer {
                name: m.name,
                tools: m.tools.to_string(),
                is_rust: m.is_rust,
            })
            .collect();

        // Convert agents
        let multi_agent = resp
            .agents
            .into_iter()
            .map(|a| MultiAgentEntry {
                pane_id: a.pane,
                project: a.project,
                task: a.task,
                last_update: String::new(),
            })
            .collect();

        Ok(FullDashboardResult {
            dashboard: DashboardData {
                capacity: resp.capacity,
                sprint,
                board,
                mcps,
                activity: resp.activity,
                auto_config: resp.auto_config,
                session: resp.session,
                multi_agent,
                milestones: resp.milestones,
                processes: resp.processes,
            },
            digest: resp.digest,
            alerts: resp.alerts,
        })
    }

    /// Parse raw sprint JSON into SprintData (picks active or latest)
    fn parse_sprint(sprints: &[Value]) -> Option<SprintData> {
        let s = sprints
            .iter()
            .find(|s| s.get("status").and_then(|v| v.as_str()) == Some("active"))
            .or(sprints.last())?;

        let name = s
            .get("name")
            .or(s.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let space = s
            .get("space")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let planned = s.get("planned").and_then(|v| v.as_object())?;
        let issues = planned.get("issues").and_then(|v| v.as_array());

        let total_issues = issues.map(|i| i.len()).unwrap_or(0);
        let done_issues = issues
            .map(|arr| {
                arr.iter()
                    .filter(|i| {
                        i.get("status").and_then(|v| v.as_str()) == Some("done")
                            || i.get("actual_acu").is_some()
                    })
                    .count()
            })
            .unwrap_or(0);

        let total_acu = planned
            .get("total_acu")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| {
                issues
                    .map(|arr| {
                        arr.iter()
                            .map(|i| i.get("estimated_acu").and_then(|v| v.as_f64()).unwrap_or(0.0))
                            .sum()
                    })
                    .unwrap_or(0.0)
            });

        let used_acu: f64 = issues
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| i.get("actual_acu").and_then(|v| v.as_f64()))
                    .sum()
            })
            .unwrap_or(0.0);

        let end_date = s
            .get("end_date")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (days_left, ended) =
            if let Ok(end) = NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
                let today = Local::now().date_naive();
                let days = (end - today).num_days() + 1;
                (days.max(0), days < 0)
            } else {
                (0, false)
            };

        Some(SprintData {
            name,
            space,
            total_issues,
            done_issues,
            total_acu: (total_acu * 10.0).round() / 10.0,
            used_acu: (used_acu * 10.0).round() / 10.0,
            days_left,
            ended,
        })
    }

    /// Submit a factory request
    pub async fn submit_factory(&self, request: &str) -> anyhow::Result<FactorySubmitResponse> {
        let url = format!("{}/api/factory/submit", self.api_url);
        let body = serde_json::json!({ "request": request });
        let resp: FactorySubmitResponse = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    /// Fetch factory pipeline status
    pub async fn fetch_factory_status(&self) -> anyhow::Result<Vec<FactoryRequest>> {
        let url = format!("{}/api/factory/status", self.api_url);
        let resp: FactoryInbox = self.client.get(&url).send().await?.json().await?;
        Ok(resp.requests)
    }

    /// Convert hub pane to MonitoredAgent
    pub fn pane_to_agent(pane: &HubPane) -> MonitoredAgent {
        let status = match pane.status.as_str() {
            "active" if pane.pty_running => AgentStatus::Processing {
                activity: pane.task.clone(),
            },
            "active" => AgentStatus::Idle,
            "idle" => AgentStatus::Idle,
            _ => AgentStatus::Unknown,
        };

        let session_name = format!("dx-{}", pane.theme.to_lowercase());
        let window_name = if pane.project != "--" {
            pane.project.clone()
        } else {
            format!("pane-{}", pane.pane)
        };

        let mut agent = MonitoredAgent::new(
            format!("dx-{}", pane.pane),
            format!("dx:{}:{}", pane.pane, pane.theme.to_lowercase()),
            session_name,
            0,
            window_name,
            pane.pane as u32,
            pane.workspace
                .clone()
                .unwrap_or_else(|| pane.project.clone()),
            AgentType::ClaudeCode,
            0, // PID unknown from API
        );
        agent.status = status;
        agent.branch = pane.branch.clone();
        agent.touch();

        agent
    }
}
