# DX Terminal

**AI-native terminal multiplexer for AI agent teams.**

Monitor, manage, and orchestrate multiple Claude Code agents from one screen. Real-time terminal streaming, task queuing, and 206 MCP tools — all in a single Rust binary.

## Features

- **TUI Dashboard** — Ratatui-based operator console with 10 view modes
- **Web Dashboard** — Real-time WebSocket streaming at `localhost:3100`
- **206 MCP Tools** — Full agent lifecycle, queue management, project scanning, code auditing
- **Native PTY** — Direct terminal management, no tmux dependency required
- **Screen Management** — Dynamic multi-screen layouts (up to 48 panes)
- **Task Queue** — Priority-based auto-cycling with dependency tracking
- **MCP Gateway** — Spawn and route micro MCPs on-demand
- **Project Scanner** — Auto-discover projects with tech stack detection
- **Real-time Streaming** — Bidirectional WebSocket for live terminal output

## Quick Start

```bash
cargo build --release
./target/release/dx          # TUI dashboard (default)
./target/release/dx web      # Web dashboard only
./target/release/dx mcp      # MCP server mode (stdio)
```

## Architecture

```
dx (single binary)
├── TUI        Ratatui dashboard with keybind-driven navigation
├── Web        Axum REST + SSE + WebSocket server
├── MCP        rmcp-based MCP server (206 tools)
├── PTY        Native pseudo-terminal management
├── Engine     Background tasks: reaper, retention, health checks
├── Queue      Priority task queue with auto-cycle
├── Gateway    Micro MCP spawning and routing
├── Scanner    Project discovery and tech detection
└── State      Persistent state with broadcast event bus
```

## Config

`~/.config/dx-terminal/config.json`

```json
{
  "pane_count": 9,
  "session_name": "dx",
  "web_port": 3100
}
```

## License

MIT
