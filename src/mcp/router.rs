//! MCP Router — composes micro MCP modules into a single server.
//!
//! Each micro MCP registers tools under its namespace.
//! The router dispatches incoming tool calls to the right module.

use serde_json::Value;
use super::tools::{
    MicroMcp,
    sessions::SessionsMcp,
    pty_control::PtyControlMcp,
    analytics::AnalyticsMcp,
    git_info::GitInfoMcp,
};

/// Composes multiple micro MCP modules into a single tool registry.
pub struct McpRouter {
    modules: Vec<Box<dyn MicroMcp>>,
}

impl McpRouter {
    /// Create a router with all built-in micro MCP modules.
    pub fn new() -> Self {
        Self {
            modules: vec![
                Box::new(SessionsMcp),
                Box::new(PtyControlMcp),
                Box::new(AnalyticsMcp),
                Box::new(GitInfoMcp),
            ],
        }
    }

    /// Get all tool definitions from all modules.
    pub fn all_tools(&self) -> Vec<Value> {
        self.modules
            .iter()
            .flat_map(|m| m.tools())
            .collect()
    }

    /// Get tool definitions grouped by namespace.
    pub fn tools_by_namespace(&self) -> Vec<(&str, Vec<Value>)> {
        self.modules
            .iter()
            .map(|m| (m.namespace(), m.tools()))
            .collect()
    }

    /// Find which namespace a tool belongs to.
    pub fn find_namespace(&self, tool_name: &str) -> Option<&str> {
        for module in &self.modules {
            for tool in module.tools() {
                if tool.get("name").and_then(|n| n.as_str()) == Some(tool_name) {
                    return Some(module.namespace());
                }
            }
        }
        None
    }

    /// Total number of tools across all modules.
    pub fn tool_count(&self) -> usize {
        self.modules.iter().map(|m| m.tools().len()).sum()
    }

    /// List all module namespaces.
    pub fn namespaces(&self) -> Vec<&str> {
        self.modules.iter().map(|m| m.namespace()).collect()
    }
}

impl Default for McpRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_has_all_modules() {
        let router = McpRouter::new();
        let ns = router.namespaces();
        assert!(ns.contains(&"sessions"));
        assert!(ns.contains(&"pty"));
        assert!(ns.contains(&"analytics"));
        assert!(ns.contains(&"git"));
    }

    #[test]
    fn test_router_tool_count() {
        let router = McpRouter::new();
        // sessions: 4, pty: 5, analytics: 3, git: 3 = 15
        assert_eq!(router.tool_count(), 15);
    }

    #[test]
    fn test_find_namespace() {
        let router = McpRouter::new();
        assert_eq!(router.find_namespace("list_sessions"), Some("sessions"));
        assert_eq!(router.find_namespace("send_input"), Some("pty"));
        assert_eq!(router.find_namespace("get_usage"), Some("analytics"));
        assert_eq!(router.find_namespace("get_branch"), Some("git"));
        assert_eq!(router.find_namespace("nonexistent"), None);
    }
}
