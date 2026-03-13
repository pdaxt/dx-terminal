use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxTerminalState {
    #[serde(default)]
    pub panes: HashMap<String, PaneState>,
    #[serde(default)]
    pub project_mcps: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub space_project_map: HashMap<String, String>,
    #[serde(default)]
    pub activity_log: VecDeque<LogEntry>,
    #[serde(default)]
    pub config: DxTerminalConfig,
}

impl Default for DxTerminalState {
    fn default() -> Self {
        Self {
            panes: HashMap::new(),
            project_mcps: HashMap::new(),
            space_project_map: HashMap::new(),
            activity_log: VecDeque::new(),
            config: DxTerminalConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneState {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub project_path: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub runtime_adapter: Option<String>,
    #[serde(default)]
    pub dxos_session_id: Option<String>,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub space: Option<String>,
    #[serde(default = "default_idle")]
    pub status: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub acu_spent: f64,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub machine_ip: Option<String>,
    #[serde(default)]
    pub machine_hostname: Option<String>,
    #[serde(default)]
    pub machine_mac: Option<String>,
    /// tmux target pane (e.g. "claude6:11.1") — where the agent actually lives
    #[serde(default)]
    pub tmux_target: Option<String>,
}

fn default_idle() -> String {
    "idle".into()
}

impl Default for PaneState {
    fn default() -> Self {
        Self {
            theme: String::new(),
            project: "--".into(),
            project_path: String::new(),
            role: String::new(),
            provider: None,
            model: None,
            runtime_adapter: None,
            dxos_session_id: None,
            task: String::new(),
            issue_id: None,
            space: None,
            status: "idle".into(),
            started_at: None,
            acu_spent: 0.0,
            workspace_path: None,
            branch_name: None,
            base_branch: None,
            machine_ip: None,
            machine_hostname: None,
            machine_mac: None,
            tmux_target: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub pane: u8,
    pub event: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxTerminalConfig {
    #[serde(default = "default_markers")]
    pub completion_markers: Vec<String>,
    #[serde(default = "default_stuck")]
    pub stuck_threshold_minutes: u64,
    #[serde(default = "default_role")]
    pub default_role: String,
}

fn default_markers() -> Vec<String> {
    vec!["---DONE---".into(), "TASK COMPLETE".into()]
}
fn default_stuck() -> u64 {
    5
}
fn default_role() -> String {
    "developer".into()
}

impl Default for DxTerminalConfig {
    fn default() -> Self {
        Self {
            completion_markers: default_markers(),
            stuck_threshold_minutes: 5,
            default_role: "developer".into(),
        }
    }
}
