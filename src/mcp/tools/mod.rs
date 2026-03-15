//! Micro MCP tool modules — independent, efficient, unified.
//!
//! Each sub-module owns a domain; all re-exported here so callers
//! continue to use `tools::spawn`, `tools::dashboard`, etc.
//!
//! ## Module Map
//! - `helpers` — shared utilities (json_err, truncate, workspace prep, git finalize)
//! - `panes` — agent lifecycle (spawn, kill, restart, reassign, assign, collect, complete)
//! - `config_tools` — configuration (set_mcps, set_preamble, config_show)
//! - `routing` — MCP discovery (mcp_list, mcp_route, mcp_search)
//! - `git_tools` — git isolation (sync, status, push, pr, merge)
//! - `queue_tools` — task queue + auto-cycle
//! - `monitoring` — observability (status, dashboard, logs, health, monitor, watch, digest)
//! - `tracker_tools` — issue tracking (CRUD, milestones, processes, features)
//! - `multi_agent_tools` — coordination (ports, agents, locks, branches, builds, tasks, KB, messaging)
//! - `collab_tools` — collaboration (spaces, docs CRUD, proposals, comments)
//! - `knowledge_tools` — knowledge graph, session replay, TruthGuard facts
//! - `capacity_tools` — sprint planning (configure, estimate, log work, burndown, velocity)
//! - `analytics_tools` — usage tracking (tool calls, file ops, tokens, commits, reports)
//! - `quality_tools` — quality gates (test, build, lint, deploy logging; regressions, health)
//! - `dashboard_tools` — rich dashboards (overview, agent detail, leaderboard, timeline, alerts)
//! - `scanner_tools` — project intelligence (scan, list, detail, test, deps)
//! - `audit_tools` — code audit (code, security, intent, deps, full)

pub mod analytics_tools;
pub mod audit_tools;
pub mod build_tools;
pub mod capacity_tools;
pub mod collab_tools;
pub mod config_tools;
pub mod dashboard_tools;
pub mod dxos_tools;
pub mod factory_tools;
pub mod gateway_tools;
pub mod git_tools;
pub mod helpers;
pub mod knowledge_tools;
pub mod monitoring;
pub mod multi_agent_tools;
pub mod orchestrate;
pub mod panes;
pub mod quality_tools;
pub mod queue_tools;
pub mod routing;
pub mod scanner_tools;
pub mod screen_tools;
pub mod session_control_tools;
pub mod tracker_tools;
pub mod ui_audit_tools;
pub mod vision_tools;

// ── Re-exports (flat namespace for backward compat) ──

pub use config_tools::{config_show, set_mcps, set_preamble};
pub use git_tools::{git_merge, git_pr, git_push, git_status_tool, git_sync};
pub use helpers::{machine_info_tool, machine_list_tool};
pub use monitoring::{dashboard, digest, health, logs, monitor, project_status, status, watch};
pub use panes::{assign, assign_adhoc, collect, complete, kill, reassign, restart, spawn};
pub use queue_tools::{
    auto_config, auto_cycle, queue_add, queue_cancel, queue_clear, queue_decompose, queue_done,
    queue_list, queue_retry,
};
pub use routing::{mcp_list, mcp_route, mcp_search};
