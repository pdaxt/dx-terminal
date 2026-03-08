//! Built-in MCP server — the terminal IS the MCP.
//!
//! Other Claude agents can control this terminal via MCP tools:
//!   - list_sessions: See all running agent sessions
//!   - spawn_agent: Start a new agent in a pane
//!   - send_input: Send text to a pane
//!   - get_content: Read pane output
//!   - get_analytics: Token/cost metrics
//!   - kill_agent: Stop an agent
//!
//! Built natively in Rust — no external dependencies.

mod server;

pub use server::McpServerHandle;
