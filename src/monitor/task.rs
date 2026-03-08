use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::hub_client::{HubClient, QueueTask, AlertsResponse, AnalyticsDigest, FactoryRequest};
use crate::agents::{AgentStatus, MonitoredAgent};
use crate::app::AgentTree;
use crate::parsers::ParserRegistry;
use crate::state_reader::DashboardData;
use crate::tmux::{refresh_process_cache, TmuxClient};

/// Hysteresis duration - keep "Processing" status for this long after last active detection
const STATUS_HYSTERESIS_MS: u64 = 2000;

/// Command sent from TUI to monitor for async execution
#[derive(Debug)]
pub enum FactoryCommand {
    Submit { request: String },
}

/// Update message sent from monitor to UI
#[derive(Debug, Clone)]
pub struct MonitorUpdate {
    pub agents: AgentTree,
    pub queue_tasks: Vec<QueueTask>,
    pub hub_connected: bool,
    /// Flash message for connection state changes
    pub flash: Option<String>,
    /// 24h analytics digest (fetched on slow cadence)
    pub digest: Option<AnalyticsDigest>,
    /// Active alerts (fetched on slow cadence)
    pub alerts: Option<AlertsResponse>,
    /// Dashboard data (fetched on slow cadence via /api/dashboard)
    pub dashboard: Option<DashboardData>,
    /// Factory pipeline requests (fetched on slow cadence)
    pub factory_requests: Option<Vec<FactoryRequest>>,
}

/// Background task that monitors tmux panes and hub API for AI agents
pub struct MonitorTask {
    tmux_client: Arc<TmuxClient>,
    parser_registry: Arc<ParserRegistry>,
    hub_client: Option<HubClient>,
    tx: mpsc::Sender<MonitorUpdate>,
    factory_rx: mpsc::Receiver<FactoryCommand>,
    poll_interval: Duration,
    /// Track when each agent was last seen as "active" (Processing/AwaitingApproval)
    /// Key: agent target string
    last_active: HashMap<String, Instant>,
    /// Consecutive API failures for exponential backoff
    api_fail_count: u32,
    /// Whether API was connected last poll (for detecting transitions)
    was_connected: bool,
    /// Counter for slow-cadence analytics polling
    analytics_counter: u32,
}

impl MonitorTask {
    pub fn new(
        tmux_client: Arc<TmuxClient>,
        parser_registry: Arc<ParserRegistry>,
        hub_client: Option<HubClient>,
        tx: mpsc::Sender<MonitorUpdate>,
        factory_rx: mpsc::Receiver<FactoryCommand>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            tmux_client,
            parser_registry,
            hub_client,
            tx,
            factory_rx,
            poll_interval,
            last_active: HashMap::new(),
            api_fail_count: 0,
            was_connected: false,
            analytics_counter: 0,
        }
    }

    /// Runs the monitoring loop
    pub async fn run(mut self) {
        loop {
            // Process any pending factory commands (non-blocking drain)
            let mut flash_from_factory: Option<String> = None;
            while let Ok(cmd) = self.factory_rx.try_recv() {
                match cmd {
                    FactoryCommand::Submit { request } => {
                        if let Some(ref client) = self.hub_client {
                            match client.submit_factory(&request).await {
                                Ok(resp) => {
                                    flash_from_factory = Some(format!(
                                        "Factory: {} ({})",
                                        resp.message, resp.factory_id
                                    ));
                                }
                                Err(e) => {
                                    flash_from_factory =
                                        Some(format!("Factory error: {}", e));
                                }
                            }
                        } else {
                            flash_from_factory =
                                Some("Factory: Hub not connected".to_string());
                        }
                    }
                }
            }

            let (tree, queue_tasks, connected) = match self.poll_all().await {
                Ok(result) => result,
                Err(e) => {
                    warn!("Monitor poll error: {}", e);
                    (AgentTree::new(), Vec::new(), false)
                }
            };

            // Detect connection state transitions
            let flash = if let Some(msg) = flash_from_factory {
                Some(msg)
            } else if connected && !self.was_connected {
                self.api_fail_count = 0;
                Some("Hub connected".to_string())
            } else if !connected && self.was_connected {
                Some("Hub disconnected".to_string())
            } else {
                None
            };
            self.was_connected = connected;

            // Fetch dashboard + analytics + factory on slow cadence (~5s at 500ms poll = every 10th poll)
            self.analytics_counter += 1;
            let mut digest = None;
            let mut alerts = None;
            let mut dashboard = None;
            let mut factory_requests = None;
            if connected && self.analytics_counter % 10 == 0 {
                if let Some(ref client) = self.hub_client {
                    // Single /api/dashboard call returns everything including digest + alerts
                    match client.fetch_dashboard().await {
                        Ok(result) => {
                            dashboard = Some(result.dashboard);
                            digest = Some(result.digest);
                            alerts = Some(result.alerts);
                        }
                        Err(e) => {
                            debug!("Dashboard fetch failed: {}", e);
                        }
                    }
                    // Fetch factory pipeline status
                    match client.fetch_factory_status().await {
                        Ok(reqs) => {
                            factory_requests = Some(reqs);
                        }
                        Err(e) => {
                            debug!("Factory status fetch failed: {}", e);
                        }
                    }
                }
            }

            let update = MonitorUpdate {
                agents: tree,
                queue_tasks,
                hub_connected: connected,
                flash,
                digest,
                alerts,
                dashboard,
                factory_requests,
            };
            if self.tx.send(update).await.is_err() {
                debug!("Monitor channel closed, stopping");
                break;
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn poll_all(&mut self) -> anyhow::Result<(AgentTree, Vec<QueueTask>, bool)> {
        // Poll tmux agents
        let mut tree = self.poll_tmux_agents().await?;

        // Poll Hub (if configured)
        let mut queue_tasks = Vec::new();
        let mut connected = false;

        if let Some(ref client) = self.hub_client {
            // Exponential backoff: skip API calls if failing repeatedly
            // After N failures, only try every 2^N polls (max 32 = ~16s at 500ms)
            let backoff_polls = (1u32 << self.api_fail_count.min(5)) as u64;
            let should_try_api = self.api_fail_count == 0
                || (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
                    / self.poll_interval.as_millis() as u64)
                    .is_multiple_of(backoff_polls);

            if !should_try_api {
                return Ok((tree, queue_tasks, false));
            }

            // Fetch panes from Hub
            match client.fetch_panes().await {
                Ok(panes) => {
                    connected = true;
                    for pane in &panes {
                        // Show panes that have a real project or are actively running
                        let has_project = pane.project != "--" && !pane.project.is_empty();
                        let is_active = pane.pty_running || pane.status == "active";
                        if has_project || is_active {
                            let agent = HubClient::pane_to_agent(pane);
                            // Only add if not already detected via tmux
                            let already_exists = tree
                                .root_agents
                                .iter()
                                .any(|a| a.path == agent.path && a.agent_type == agent.agent_type);
                            if !already_exists {
                                tree.root_agents.push(agent);
                            }
                        }
                    }
                }
                Err(e) => {
                    self.api_fail_count = self.api_fail_count.saturating_add(1);
                    debug!(
                        "Hub API unavailable (fail #{}): {}",
                        self.api_fail_count, e
                    );
                }
            }

            // Fetch queue
            match client.fetch_queue().await {
                Ok(tasks) => {
                    queue_tasks = tasks;
                }
                Err(e) => {
                    debug!("Hub queue unavailable: {}", e);
                }
            }
        }

        // Sort agents by target for consistent ordering
        tree.root_agents.sort_by(|a, b| a.target.cmp(&b.target));

        Ok((tree, queue_tasks, connected))
    }

    async fn poll_tmux_agents(&mut self) -> anyhow::Result<AgentTree> {
        // Refresh process cache once per poll cycle (much faster than per-pane)
        refresh_process_cache();

        let panes = self.tmux_client.list_panes()?;
        let mut tree = AgentTree::new();

        for pane in panes {
            // Try to find a matching parser for the pane (checks command, title, cmdline)
            if let Some(parser) = self.parser_registry.find_parser_for_pane(&pane) {
                let target = pane.target();

                // Capture pane content
                let content = match self.tmux_client.capture_pane(&target) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("Failed to capture pane {}: {}", target, e);
                        continue;
                    }
                };

                // Parse status from content
                let mut status = parser.parse_status(&content);

                // Check pane title for spinner (Claude Code specific)
                let title_has_spinner = pane.title.chars().any(|c| {
                    matches!(
                        c,
                        '⠿' | '⠇'
                            | '⠋'
                            | '⠙'
                            | '⠸'
                            | '⠴'
                            | '⠦'
                            | '⠧'
                            | '⠖'
                            | '⠏'
                            | '⠹'
                            | '⠼'
                            | '⠷'
                            | '⠾'
                            | '⠽'
                            | '⠻'
                            | '⠐'
                            | '⠑'
                            | '⠒'
                            | '⠓'
                    )
                });

                // If title has spinner, override to Processing
                if title_has_spinner && matches!(status, AgentStatus::Idle | AgentStatus::Unknown) {
                    status = AgentStatus::Processing {
                        activity: "Working...".to_string(),
                    };
                }

                // Apply hysteresis
                let now = Instant::now();
                let is_active = matches!(
                    status,
                    AgentStatus::Processing { .. } | AgentStatus::AwaitingApproval { .. }
                );

                if is_active {
                    self.last_active.insert(target.clone(), now);
                } else if matches!(status, AgentStatus::Idle) {
                    if let Some(last) = self.last_active.get(&target) {
                        if now.duration_since(*last) < Duration::from_millis(STATUS_HYSTERESIS_MS) {
                            status = AgentStatus::Processing {
                                activity: "Working...".to_string(),
                            };
                        }
                    }
                }

                // Parse subagents
                let subagents = parser.parse_subagents(&content);

                // Parse context remaining
                let context_remaining = parser.parse_context_remaining(&content);

                // Create monitored agent
                let mut agent = MonitoredAgent::new(
                    format!("{}-{}", target, pane.pid),
                    target,
                    pane.session.clone(),
                    pane.window,
                    pane.window_name.clone(),
                    pane.pane,
                    pane.path.clone(),
                    parser.agent_type(),
                    pane.pid,
                );
                agent.status = status;
                agent.subagents = subagents;
                agent.last_content = content;
                agent.context_remaining = context_remaining;
                agent.touch();

                tree.root_agents.push(agent);
            }
        }

        Ok(tree)
    }
}
