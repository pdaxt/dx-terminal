//! Dashboard data types used by the TUI.
//! Data is fetched from hub API by HubClient — zero file reads.

use serde::Deserialize;
use std::collections::HashMap;

// =============================================================================
// Capacity
// =============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CapacityData {
    #[serde(default)]
    pub acu_used: f64,
    #[serde(default)]
    pub acu_total: f64,
    #[serde(default)]
    pub reviews_used: u32,
    #[serde(default)]
    pub reviews_total: u32,
}

impl CapacityData {
    pub fn acu_pct(&self) -> f64 {
        if self.acu_total > 0.0 {
            self.acu_used / self.acu_total * 100.0
        } else {
            0.0
        }
    }

    pub fn bottleneck(&self) -> &'static str {
        let rev_pct = if self.reviews_total > 0 {
            self.reviews_used as f64 / self.reviews_total as f64 * 100.0
        } else {
            0.0
        };
        if rev_pct > 80.0 {
            "REVIEW"
        } else if self.acu_pct() > 90.0 {
            "COMPUTE"
        } else {
            "BALANCED"
        }
    }
}

// =============================================================================
// Sprint
// =============================================================================

#[derive(Debug, Clone, Default)]
pub struct SprintData {
    pub name: String,
    pub space: String,
    pub total_issues: usize,
    pub done_issues: usize,
    pub total_acu: f64,
    pub used_acu: f64,
    pub days_left: i64,
    pub ended: bool,
}

impl SprintData {
    pub fn pct(&self) -> f64 {
        if self.total_issues > 0 {
            self.done_issues as f64 / self.total_issues as f64 * 100.0
        } else {
            0.0
        }
    }
}

// =============================================================================
// Board (issues by space)
// =============================================================================

#[derive(Debug, Clone, Default)]
pub struct BoardData {
    pub spaces: Vec<(String, HashMap<String, usize>)>,
}

impl BoardData {
    pub fn total_issues(&self) -> usize {
        self.spaces
            .iter()
            .map(|(_, counts)| counts.values().sum::<usize>())
            .sum()
    }
}

// =============================================================================
// MCP Servers
// =============================================================================

#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub tools: String,
    pub is_rust: bool,
}

// =============================================================================
// Activity Log
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ActivityEntry {
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub pane: u8,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub summary: String,
}

// =============================================================================
// Auto-Cycle Config
// =============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AutoCycleConfig {
    #[serde(default)]
    pub max_parallel: u8,
    #[serde(default)]
    pub reserved_panes: Vec<u8>,
    #[serde(default)]
    pub auto_assign: bool,
    #[serde(default)]
    pub auto_complete: bool,
    #[serde(default)]
    pub default_role: String,
    #[serde(default, alias = "cycle_interval_secs")]
    pub cycle_interval: u32,
}

// =============================================================================
// Session State
// =============================================================================

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionData {
    #[serde(default)]
    pub current_task: String,
    #[serde(default)]
    pub completed: Vec<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub blocked_on: Option<String>,
}

// =============================================================================
// Multi-Agent Coordination
// =============================================================================

#[derive(Debug, Clone)]
pub struct MultiAgentEntry {
    pub pane_id: String,
    pub project: String,
    pub task: String,
    pub last_update: String,
}

// =============================================================================
// Milestones
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct MilestoneData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub space: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub due_date: Option<String>,
}

// =============================================================================
// Processes (Active Workflows)
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub space: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub total_steps: usize,
    #[serde(default)]
    pub completed_steps: usize,
}

// =============================================================================
// Combined Dashboard Data
// =============================================================================

#[derive(Debug, Clone, Default)]
pub struct DashboardData {
    pub capacity: CapacityData,
    pub sprint: Option<SprintData>,
    pub board: BoardData,
    pub mcps: Vec<McpServer>,
    pub activity: Vec<ActivityEntry>,
    pub auto_config: AutoCycleConfig,
    pub session: SessionData,
    pub multi_agent: Vec<MultiAgentEntry>,
    pub milestones: Vec<MilestoneData>,
    pub processes: Vec<ProcessData>,
}

impl DashboardData {
    pub fn total_mcp_tools(&self) -> u32 {
        self.mcps
            .iter()
            .filter_map(|m| m.tools.parse::<u32>().ok())
            .sum()
    }
}
