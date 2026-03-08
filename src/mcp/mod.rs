//! Micro MCP Architecture — each domain is an independent tool module.
//!
//! DX Terminal exposes itself as an MCP server so other AI agents can:
//!   - Discover and control agent sessions
//!   - Read terminal output from any pane
//!   - Spawn/kill agents
//!   - Query analytics and costs
//!   - Check git status per agent
//!
//! Architecture: Each tool module registers its own tools via the `MicroMcp` trait.
//! The router composes them into a single MCP server.

mod router;
mod server;
pub mod tools;

pub use router::McpRouter;
pub use server::McpServerHandle;
