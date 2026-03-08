//! Micro MCP tool modules.
//!
//! Each module is a self-contained domain with its own tool definitions
//! and handlers. The router composes them into a single MCP server.

pub mod sessions;
pub mod pty_control;
pub mod analytics;
pub mod git_info;

use serde_json::Value;

/// Every micro MCP module implements this trait.
pub trait MicroMcp: Send + Sync {
    /// Tool definitions for this module (JSON Schema).
    fn tools(&self) -> Vec<Value>;

    /// Module name prefix (e.g., "sessions", "analytics").
    fn namespace(&self) -> &str;
}
