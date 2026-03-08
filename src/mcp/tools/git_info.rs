//! Git micro MCP — branch, PR, and commit info per agent.

use serde_json::{json, Value};
use super::MicroMcp;

pub struct GitInfoMcp;

impl MicroMcp for GitInfoMcp {
    fn namespace(&self) -> &str {
        "git"
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "get_branch",
                "description": "Get the current git branch for an agent's working directory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" }
                    },
                    "required": ["pane"]
                }
            }),
            json!({
                "name": "get_diff",
                "description": "Get uncommitted changes in an agent's working directory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "staged": { "type": "boolean", "description": "Show only staged changes", "default": false }
                    },
                    "required": ["pane"]
                }
            }),
            json!({
                "name": "get_log",
                "description": "Get recent git commits for an agent's project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "count": { "type": "integer", "description": "Number of commits", "default": 10 }
                    },
                    "required": ["pane"]
                }
            }),
        ]
    }
}
