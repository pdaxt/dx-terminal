//! Micro MCP tool modules — independent, efficient, unified.
//!
//! Each sub-module owns a domain; all re-exported here so callers
//! continue to use `tools::spawn`, `tools::dashboard`, etc.

pub mod helpers;
pub mod panes;
pub mod config_tools;
pub mod routing;
pub mod git_tools;
pub mod queue_tools;
pub mod monitoring;

// ── Re-exports (flat namespace for backward compat) ──

pub use panes::{spawn, kill, restart, reassign, assign, assign_adhoc, collect, complete};
pub use config_tools::{set_mcps, set_preamble, config_show};
pub use monitoring::{status, dashboard, logs, health, monitor, project_status, digest, watch};
pub use routing::{mcp_list, mcp_route, mcp_search};
pub use git_tools::{git_sync, git_status_tool, git_push, git_pr, git_merge};
pub use queue_tools::{queue_add, queue_decompose, queue_list, queue_done, auto_cycle, auto_config};
pub use helpers::{machine_info_tool, machine_list_tool};
