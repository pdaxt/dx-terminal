//! Session streaming: Parse Claude Code JSONL session logs for rich terminal output.
//!
//! Claude Code writes structured JSONL to ~/.claude/projects/<project>/<session>.jsonl
//! Each line is a JSON object with type, role, message content, tool calls, etc.
//! This module tails those files and extracts structured events for the web dashboard.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::Path;

/// A structured event extracted from a Claude JSONL session.
#[derive(Clone, Debug, Serialize)]
pub struct SessionEvent {
    /// Event type: text, tool_use, tool_result, thinking, progress, result
    pub kind: String,
    /// Role: assistant, user, system
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Tool name (for tool_use events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Tool input (for tool_use: command, file_path, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    /// Text content (for text events, tool_result content)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Cost in USD (for result events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Token counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
}

/// Tail the last N events from a JSONL session file.
/// Skips hook_progress entries to reduce noise.
pub fn tail_session_events(jsonl_path: &str, max_events: usize) -> Vec<SessionEvent> {
    let path = Path::new(jsonl_path);
    if !path.exists() {
        return vec![];
    }

    // Read from end of file — seek back to find last N meaningful lines
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    // Read from end of file for large files
    let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
    let seek_pos = if file_size > 500_000 {
        file_size - 500_000
    } else {
        0
    };

    let file2 = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut reader2 = std::io::BufReader::new(file2);
    if seek_pos > 0 {
        let _ = reader2.seek(SeekFrom::Start(seek_pos));
        // Skip partial first line
        let mut _skip = String::new();
        let _ = reader2.read_line(&mut _skip);
    }

    let mut raw_events = Vec::new();
    for line in reader2.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Parse based on type
        let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let role = v
            .get("role")
            .or_else(|| v.get("message").and_then(|m| m.get("role")))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string());
        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        match entry_type {
            "progress" => {
                let data = v.get("data").cloned().unwrap_or(Value::Null);
                let data_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
                // Skip noisy hook_progress
                if data_type == "hook_progress" {
                    continue;
                }

                match data_type {
                    "bash_progress" => {
                        let output = data
                            .get("output")
                            .and_then(|o| o.as_str())
                            .map(|s| truncate(s, 500));
                        raw_events.push(SessionEvent {
                            kind: "bash_output".into(),
                            role: None,
                            tool: None,
                            input: None,
                            text: output,
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        });
                    }
                    "waiting_for_task" => {
                        let desc = data
                            .get("taskDescription")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string());
                        raw_events.push(SessionEvent {
                            kind: "waiting".into(),
                            role: None,
                            tool: None,
                            input: None,
                            text: desc,
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        });
                    }
                    "agent_progress" => {
                        raw_events.push(SessionEvent {
                            kind: "agent_progress".into(),
                            role: None,
                            tool: None,
                            input: None,
                            text: Some(serde_json::to_string(&data).unwrap_or_default()),
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        });
                    }
                    _ => {} // skip other progress types
                }
            }
            "assistant" | "user" | "system" => {
                // Parse message content
                let msg = v.get("message").cloned().unwrap_or(Value::Null);
                let content = msg.get("content").cloned().unwrap_or(Value::Null);

                if let Value::Array(items) = content {
                    for item in items {
                        let ct = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match ct {
                            "tool_use" => {
                                let name = item
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let input = item.get("input").cloned();
                                // Extract a summary for display
                                let summary = tool_input_summary(&name, &input);
                                raw_events.push(SessionEvent {
                                    kind: "tool_use".into(),
                                    role: role.clone(),
                                    tool: Some(name),
                                    input,
                                    text: Some(summary),
                                    timestamp: timestamp.clone(),
                                    cost_usd: None,
                                    tokens_in: None,
                                    tokens_out: None,
                                });
                            }
                            "tool_result" => {
                                let result_text = item
                                    .get("content")
                                    .and_then(|c| {
                                        if let Value::String(s) = c {
                                            Some(s.clone())
                                        } else if let Value::Array(arr) = c {
                                            arr.first()
                                                .and_then(|a| a.get("text"))
                                                .and_then(|t| t.as_str())
                                                .map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .map(|s| truncate(&s, 1000));
                                raw_events.push(SessionEvent {
                                    kind: "tool_result".into(),
                                    role: role.clone(),
                                    tool: None,
                                    input: None,
                                    text: result_text,
                                    timestamp: timestamp.clone(),
                                    cost_usd: None,
                                    tokens_in: None,
                                    tokens_out: None,
                                });
                            }
                            "text" => {
                                let text = item
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .map(|s| truncate(s, 2000));
                                if text.as_ref().map(|t| !t.is_empty()).unwrap_or(false) {
                                    raw_events.push(SessionEvent {
                                        kind: "text".into(),
                                        role: role.clone(),
                                        tool: None,
                                        input: None,
                                        text,
                                        timestamp: timestamp.clone(),
                                        cost_usd: None,
                                        tokens_in: None,
                                        tokens_out: None,
                                    });
                                }
                            }
                            "thinking" => {
                                let thought = item
                                    .get("thinking")
                                    .and_then(|t| t.as_str())
                                    .map(|s| truncate(s, 500));
                                if thought.is_some() {
                                    raw_events.push(SessionEvent {
                                        kind: "thinking".into(),
                                        role: role.clone(),
                                        tool: None,
                                        input: None,
                                        text: thought,
                                        timestamp: timestamp.clone(),
                                        cost_usd: None,
                                        tokens_in: None,
                                        tokens_out: None,
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "result" => {
                raw_events.push(SessionEvent {
                    kind: "result".into(),
                    role: None,
                    tool: None,
                    input: None,
                    text: None,
                    timestamp,
                    cost_usd: v.get("costUSD").and_then(|c| c.as_f64()),
                    tokens_in: v.get("inputTokens").and_then(|t| t.as_u64()),
                    tokens_out: v.get("outputTokens").and_then(|t| t.as_u64()),
                });
            }
            _ => {}
        }
    }

    // Return last N events
    let start = raw_events.len().saturating_sub(max_events);
    raw_events[start..].to_vec()
}

/// Track file position for incremental JSONL tailing.
pub struct SessionTailer {
    positions: HashMap<String, u64>,
}

impl SessionTailer {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Get new events since last call for a given JSONL file.
    pub fn poll_new_events(&mut self, jsonl_path: &str, max_events: usize) -> Vec<SessionEvent> {
        let path = Path::new(jsonl_path);
        if !path.exists() {
            return vec![];
        }

        let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let prev_pos = self.positions.get(jsonl_path).copied().unwrap_or(0);

        if file_size <= prev_pos {
            // No new data (or file was truncated)
            if file_size < prev_pos {
                self.positions.insert(jsonl_path.to_string(), 0);
            }
            return vec![];
        }

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut reader = std::io::BufReader::new(file);
        let _ = reader.seek(SeekFrom::Start(prev_pos));

        // If prev_pos > 0, skip partial first line
        if prev_pos > 0 {
            let mut skip = String::new();
            let _ = reader.read_line(&mut skip);
        }

        let mut events = Vec::new();
        let mut current_pos = if prev_pos > 0 { prev_pos } else { 0 };

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            current_pos += line.len() as u64 + 1; // +1 for newline

            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Quick filter — skip hook_progress
            let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if entry_type == "progress" {
                let dt = v
                    .get("data")
                    .and_then(|d| d.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if dt == "hook_progress" {
                    continue;
                }
            }

            // Reuse the full parser via tail_session_events would be redundant,
            // so we inline quick parsing here
            let role = v
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());
            let timestamp = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            if let Some(evt) = parse_single_entry(&v, role, timestamp) {
                events.push(evt);
            }
        }

        self.positions.insert(jsonl_path.to_string(), current_pos);

        // Return at most max_events
        let start = events.len().saturating_sub(max_events);
        events[start..].to_vec()
    }
}

/// Parse a single JSONL entry into a SessionEvent.
fn parse_single_entry(
    v: &Value,
    role: Option<String>,
    timestamp: Option<String>,
) -> Option<SessionEvent> {
    let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match entry_type {
        "progress" => {
            let data = v.get("data")?;
            let dt = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match dt {
                "bash_progress" => Some(SessionEvent {
                    kind: "bash_output".into(),
                    role: None,
                    tool: None,
                    input: None,
                    text: data
                        .get("output")
                        .and_then(|o| o.as_str())
                        .map(|s| truncate(s, 500)),
                    timestamp,
                    cost_usd: None,
                    tokens_in: None,
                    tokens_out: None,
                }),
                "waiting_for_task" => Some(SessionEvent {
                    kind: "waiting".into(),
                    role: None,
                    tool: None,
                    input: None,
                    text: data
                        .get("taskDescription")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string()),
                    timestamp,
                    cost_usd: None,
                    tokens_in: None,
                    tokens_out: None,
                }),
                _ => None,
            }
        }
        "assistant" | "user" | "system" => {
            let content = v.get("message")?.get("content")?;
            if let Value::Array(items) = content {
                let item = items.first()?;
                let ct = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match ct {
                    "tool_use" => {
                        let name = item
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let input = item.get("input").cloned();
                        let summary = tool_input_summary(&name, &input);
                        Some(SessionEvent {
                            kind: "tool_use".into(),
                            role,
                            tool: Some(name),
                            input,
                            text: Some(summary),
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        })
                    }
                    "tool_result" => {
                        let text = item.get("content").and_then(|c| {
                            if let Value::String(s) = c {
                                Some(truncate(s, 500))
                            } else {
                                None
                            }
                        });
                        Some(SessionEvent {
                            kind: "tool_result".into(),
                            role,
                            tool: None,
                            input: None,
                            text,
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        })
                    }
                    "text" => {
                        let text = item
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| truncate(s, 2000));
                        Some(SessionEvent {
                            kind: "text".into(),
                            role,
                            tool: None,
                            input: None,
                            text,
                            timestamp,
                            cost_usd: None,
                            tokens_in: None,
                            tokens_out: None,
                        })
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        "result" => Some(SessionEvent {
            kind: "result".into(),
            role: None,
            tool: None,
            input: None,
            text: None,
            timestamp,
            cost_usd: v.get("costUSD").and_then(|c| c.as_f64()),
            tokens_in: v.get("inputTokens").and_then(|t| t.as_u64()),
            tokens_out: v.get("outputTokens").and_then(|t| t.as_u64()),
        }),
        _ => None,
    }
}

/// Generate a human-readable summary of tool input.
fn tool_input_summary(name: &str, input: &Option<Value>) -> String {
    let input = match input {
        Some(v) => v,
        None => return format!("{}()", name),
    };

    match name {
        "Bash" => {
            let cmd = input
                .get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("...");
            format!("$ {}", truncate(cmd, 120))
        }
        "Read" => {
            let path = input
                .get("file_path")
                .and_then(|p| p.as_str())
                .unwrap_or("?");
            // Show just filename
            let short = path.rsplit('/').next().unwrap_or(path);
            format!("Read {}", short)
        }
        "Write" => {
            let path = input
                .get("file_path")
                .and_then(|p| p.as_str())
                .unwrap_or("?");
            let short = path.rsplit('/').next().unwrap_or(path);
            format!("Write {}", short)
        }
        "Edit" => {
            let path = input
                .get("file_path")
                .and_then(|p| p.as_str())
                .unwrap_or("?");
            let short = path.rsplit('/').next().unwrap_or(path);
            format!("Edit {}", short)
        }
        "Glob" => {
            let pat = input.get("pattern").and_then(|p| p.as_str()).unwrap_or("*");
            format!("Glob {}", pat)
        }
        "Grep" => {
            let pat = input.get("pattern").and_then(|p| p.as_str()).unwrap_or("?");
            format!("Grep /{}/", truncate(pat, 40))
        }
        "Agent" => {
            let desc = input
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("...");
            format!("Agent: {}", truncate(desc, 80))
        }
        _ => {
            // MCP tools
            if name.starts_with("mcp__") {
                let parts: Vec<&str> = name.split("__").collect();
                if parts.len() >= 3 {
                    format!("{}:{}", parts[1], parts[2])
                } else {
                    name.to_string()
                }
            } else {
                name.to_string()
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find a valid char boundary at or before max
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}
