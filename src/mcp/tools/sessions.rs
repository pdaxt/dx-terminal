//! Sessions micro MCP — discover, spawn, and kill agent sessions.

use serde_json::{json, Value};
use super::MicroMcp;

pub struct SessionsMcp;

impl MicroMcp for SessionsMcp {
    fn namespace(&self) -> &str {
        "sessions"
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "list_sessions",
                "description": "List all active agent sessions with status, project, type, uptime, and token usage.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filter": {
                            "type": "string",
                            "description": "Filter by status: 'all', 'active', 'idle', 'approval'",
                            "default": "all"
                        }
                    }
                }
            }),
            json!({
                "name": "spawn_agent",
                "description": "Start a new AI coding agent in a PTY pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number (0-based)" },
                        "project": { "type": "string", "description": "Project path or name" },
                        "role": { "type": "string", "description": "Agent role", "default": "developer" },
                        "task": { "type": "string", "description": "Task description" },
                        "agent": { "type": "string", "description": "Agent type: claude, opencode, codex, gemini", "default": "claude" },
                        "autonomous": { "type": "boolean", "description": "Skip permission prompts", "default": false }
                    },
                    "required": ["pane", "project", "task"]
                }
            }),
            json!({
                "name": "kill_agent",
                "description": "Stop an agent running in a pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" }
                    },
                    "required": ["pane"]
                }
            }),
            json!({
                "name": "get_status",
                "description": "Get detailed status of a specific agent: type, status, context remaining, subagents, branch.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" }
                    },
                    "required": ["pane"]
                }
            }),
        ]
    }
}
