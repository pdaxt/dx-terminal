//! Split MCP servers for fast tools/list response.
//!
//! Instead of one monolithic server with 206 tools (which times out on tools/list),
//! we split into 5 focused servers, each with 30-50 tools.
//!
//! Usage: `dx mcp core`, `dx mcp queue`, `dx mcp tracker`, `dx mcp coord`, `dx mcp intel`

pub mod coord;
pub mod core_server;
pub mod intel;
pub mod queue;
pub mod tracker;
