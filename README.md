<div align="center">

# DX Terminal

**The AI-native terminal multiplexer.**

Monitor, manage, and orchestrate AI coding agents from a single Rust binary.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MCP Tools](https://img.shields.io/badge/MCP_tools-206-green)](src/mcp/tools/)

</div>

---

DX Terminal replaces tmux + bash scripts + glue code with a purpose-built operator console for AI agent teams. It provides native PTY management, a real-time TUI dashboard, a WebSocket-powered web UI, and 206 MCP tools — all compiled into a single `dx` binary.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    dx (binary)                      │
├──────────┬──────────┬──────────┬────────────────────┤
│   TUI    │   Web    │   MCP    │      Engine        │
│ Ratatui  │  Axum    │  rmcp    │                    │
│ 10 views │ REST+WS  │ stdio    │  Reaper · Health   │
│ keybinds │ SSE+HTML │ 206 tools│  Retention · Scan  │
├──────────┴──────────┴──────────┴────────────────────┤
│                  Shared State                       │
│   PTY Pool · Task Queue · Event Bus · SQLite        │
├─────────────────────────────────────────────────────┤
│              Native PTY Layer                       │
│   portable-pty · vt100 · up to 48 panes             │
└─────────────────────────────────────────────────────┘
```

## Quick Start

```bash
git clone https://github.com/pdaxt/dx-terminal.git
cd dx-terminal
git checkout rust-rewrite
cargo build --release

# Three modes of operation:
./target/release/dx mcp          # MCP server (stdio) — connect from Claude Code
./target/release/dx tui          # TUI operator console
./target/release/dx web          # Web dashboard at localhost:3100
```

### Connect to Claude Code

Add to your `~/.claude.json`:

```json
{
  "mcpServers": {
    "dx-terminal": {
      "command": "/path/to/dx",
      "args": ["mcp"]
    }
  }
}
```

## Features

### TUI Dashboard
Ratatui-based operator console with 10 view modes: agents, queues, logs, projects, analytics, quality gates, factory pipelines, knowledge graph, capacity planning, and screen management. Navigate with keyboard shortcuts, no mouse needed.

### Web Dashboard
Axum server at `:3100` with REST API, Server-Sent Events for live updates, and full WebSocket streaming for bidirectional terminal I/O. Drop-in web UI for remote monitoring.

### 206 MCP Tools
Expose the entire agent lifecycle over MCP stdio. Tools are organized into domains:

| Domain | Tools | What It Does |
|--------|-------|-------------|
| **Queue** | `os_queue_add`, `os_queue_list`, `os_queue_done`, ... | Priority task queue with auto-cycling and dependency tracking |
| **Orchestration** | `os_spawn`, `os_assign`, `os_kill`, `os_status`, ... | Spawn, assign, monitor, and terminate agent sessions |
| **Git** | `git_claim_branch`, `git_pre_commit_check`, ... | Branch locking, pre-commit validation, release management |
| **Scanner** | `project_scan`, `project_health`, `project_deps`, ... | Auto-discover repos, detect tech stacks, map dependencies |
| **Analytics** | `trends`, `usage_report`, `tool_ranking`, ... | Token usage, tool frequency, cost tracking |
| **Quality** | `quality_gate`, `quality_report`, `regressions`, ... | Automated quality checks, regression detection |
| **Factory** | `factory_run`, `factory_gate`, `factory_status`, ... | Multi-stage CI/CD pipelines with gate approvals |
| **Knowledge** | `kb_add`, `kb_search`, `kgraph_add_entity`, ... | Shared knowledge base + graph with SQLite persistence |
| **Multi-Agent** | `lock_acquire`, `msg_send`, `agent_register`, ... | File locking, message passing, agent coordination |
| **Capacity** | `cap_estimate`, `cap_velocity`, `cap_burndown`, ... | Sprint planning, velocity tracking, workload estimation |
| **Audit** | `audit_code`, `audit_security`, `audit_deps`, ... | Code review, dependency scanning, security analysis |
| **Dashboard** | `dash_overview`, `dash_timeline`, `dash_export`, ... | Aggregated views, digests, exportable reports |
| **Screen** | `dx_add_screen`, `dx_list_screens`, ... | Dynamic multi-screen management up to 48 panes |
| **Gateway** | `mcp_discover`, `mcp_call`, `mcp_health`, ... | Spawn and route to micro-MCPs on demand |

### Native PTY Management
Direct pseudo-terminal allocation via `portable-pty`. No tmux dependency. Each agent gets its own PTY with VT100 parsing, scroll history, and process lifecycle management.

### MCP Gateway
Spawn micro-MCP servers on-demand, discover their tools, and route calls through a unified gateway. Compose specialized MCPs into larger workflows without manual wiring.

### Project Scanner
Walk your filesystem, detect project types (Rust, Node, Python, Go, etc.), parse dependency files, and build a project graph. Used by orchestration tools to auto-assign agents to the right codebase.

## Workspace Structure

```
dx-terminal/
├── src/
│   ├── main.rs          # CLI entrypoint (clap)
│   ├── app.rs           # Core application state
│   ├── mcp/             # MCP server + 206 tool implementations
│   │   ├── tools/       # Tool modules by domain
│   │   └── servers/     # MCP server configurations
│   ├── tui/             # Ratatui dashboard
│   │   ├── dashboard.rs # Main render loop
│   │   ├── widgets.rs   # Custom TUI components
│   │   └── overlays.rs  # Modal overlays
│   ├── web/             # Axum web server
│   │   ├── api.rs       # REST endpoints
│   │   ├── ws.rs        # WebSocket handlers
│   │   └── sse.rs       # Server-Sent Events
│   ├── pty/             # Native PTY management
│   ├── state/           # Shared state + event bus
│   ├── engine/          # Background workers
│   ├── queue.rs         # Priority task queue
│   └── scanner.rs       # Project discovery
├── crates/
│   ├── types/           # Shared type definitions
│   └── gateway/         # MCP gateway logic
└── Cargo.toml
```

## Configuration

Config lives at `~/.config/dx-terminal/config.json`:

```json
{
  "pane_count": 9,
  "session_name": "dx",
  "web_port": 3100,
  "data_dir": "~/.local/share/dx-terminal"
}
```

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DX_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `DX_WEB_PORT` | `3100` | Web dashboard port |
| `DX_DATA_DIR` | `~/.local/share/dx-terminal` | Persistent data directory |

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [rmcp](https://github.com/anthropics/rmcp) | MCP server framework (stdio transport) |
| [ratatui](https://ratatui.rs) | Terminal UI framework |
| [axum](https://github.com/tokio-rs/axum) | Web server (REST + WebSocket + SSE) |
| [tokio](https://tokio.rs) | Async runtime |
| [portable-pty](https://docs.rs/portable-pty) | Cross-platform PTY allocation |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite for knowledge graph + session replay |
| [clap](https://clap.rs) | CLI argument parsing |

## Building from Source

Requires Rust 1.75+ (2021 edition).

```bash
# Debug build
cargo build

# Release build (LTO + stripped)
cargo build --release

# Run tests
cargo test

# The binary is at:
./target/release/dx
```

## Contributing

1. Fork and clone
2. Create a feature branch from `rust-rewrite`
3. Make changes with tests
4. `cargo test && cargo clippy`
5. Open a PR

## License

MIT
