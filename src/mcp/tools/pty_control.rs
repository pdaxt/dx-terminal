//! PTY Control micro MCP — read/write terminal panes.

use serde_json::{json, Value};
use super::MicroMcp;

pub struct PtyControlMcp;

impl MicroMcp for PtyControlMcp {
    fn namespace(&self) -> &str {
        "pty"
    }

    fn tools(&self) -> Vec<Value> {
        vec![
            json!({
                "name": "send_input",
                "description": "Send text input to an agent's terminal pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "input": { "type": "string", "description": "Text to send" },
                        "enter": { "type": "boolean", "description": "Press enter after input", "default": true }
                    },
                    "required": ["pane", "input"]
                }
            }),
            json!({
                "name": "get_content",
                "description": "Read terminal output from an agent's pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "lines": { "type": "integer", "description": "Number of lines to capture", "default": 50 },
                        "from_bottom": { "type": "boolean", "description": "Capture from bottom of scrollback", "default": true }
                    },
                    "required": ["pane"]
                }
            }),
            json!({
                "name": "send_approval",
                "description": "Send approval (y/n) to an agent waiting for permission.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "approve": { "type": "boolean", "description": "true=approve, false=reject" }
                    },
                    "required": ["pane", "approve"]
                }
            }),
            json!({
                "name": "send_choice",
                "description": "Send a numbered choice to an agent asking a question.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "choice": { "type": "integer", "description": "Choice number (1-9)" }
                    },
                    "required": ["pane", "choice"]
                }
            }),
            json!({
                "name": "resize_pane",
                "description": "Resize a PTY pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": { "type": "integer", "description": "Pane number" },
                        "cols": { "type": "integer", "description": "Column width" },
                        "rows": { "type": "integer", "description": "Row height" }
                    },
                    "required": ["pane", "cols", "rows"]
                }
            }),
        ]
    }
}
