<div align="center">

# DX Terminal

**The AI-native terminal multiplexer. Orchestrate teams of AI coding agents from one screen.**

[![CI](https://github.com/pdaxt/dx-terminal/actions/workflows/ci.yml/badge.svg)](https://github.com/pdaxt/dx-terminal/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Pure_Rust-orange.svg)](https://github.com/pdaxt/dx-terminal)
[![MCP](https://img.shields.io/badge/MCP-206_tools-purple.svg)](https://github.com/pdaxt/dx-terminal)

Single binary. No login. No telemetry. Open source.

<img src="demo/demo-screenshot.png" alt="DX Terminal showing 16 AI agents with real-time dashboard, task queue, vision tracking, and sync status" width="800">

[Quick Start](#install) · [Operating Model](#operating-model) · [Web Dashboard](#web-dashboard) · [MCP Server](#mcp-server) · [Architecture](#architecture) · [Operator Guide](docs/OPERATOR_SYSTEM_GUIDE.md) · [Experience Blueprint](docs/EXPERIENCE_BLUEPRINT.md) · [Hosted Sync Model](docs/HOSTED_SYNC_MODEL.md) · [Contributing](#contributing)

</div>

---

## The Problem

You're running 16 AI coding runtimes across tmux panes. One needs approval. Another is stuck. A third finished but nobody noticed. You're alt-tabbing constantly, losing context every time, with no idea what's actually getting done.

## The Solution

DX Terminal is a **complete AI agent orchestration platform** — a single Rust binary that monitors, coordinates, and tracks teams of AI coding agents. Real-time TUI dashboard, web dashboard with WebSocket streaming, 206-tool MCP server, vision-driven development tracking, file sync, and build environment management. Built in Rust, <5MB RAM.

## Operating Model

DX Terminal is designed as one control plane for work that usually gets split across:

- documentation in Confluence or ad-hoc markdown
- planning and feature tracking in Jira or tickets
- implementation across tmux panes, CLIs, and worktrees
- QA evidence scattered across terminal history and CI logs

The intended lifecycle is:

1. `planned`: mission exists, feature is known, but discovery has not started.
2. `discovery`: research, questions, architecture notes, and acceptance criteria are being written.
3. `build`: work is assigned to an agent pane and executed in a tracked workspace/worktree.
4. `test`: test evidence and acceptance verification are collected.
5. `done`: implementation, acceptance, and documentation are aligned.

The system should keep these artifacts in sync:

- filesystem docs: `AGENTS.md`, provider overlays, `.vision/research/*`, `.vision/discovery/*`
- git state: branch, worktree, dirty files, ahead/behind
- runtime state: pane, provider, task, browser port, tmux target, live output
- dashboard state: project brief, focus, blockers, readiness, active runtimes
- hosted site state: same snapshot and event contract as the local dashboard

## Install

```bash
# Homebrew (macOS & Linux)
brew install pdaxt/tap/dx-terminal

# Cargo
cargo install dx-terminal

# Shell script
curl -fsSL https://raw.githubusercontent.com/pdaxt/dx-terminal/main/install.sh | bash

# From source
git clone https://github.com/pdaxt/dx-terminal.git && cd dx-terminal && cargo install --path .
```

## Usage

```bash
dx                          # TUI dashboard + web + MCP
dx mcp                      # MCP server mode (stdio, all 206 tools)
dx mcp core                 # Split MCP server (faster tools/list)
dx web --port 3100          # Web dashboard only
```

See [docs/OPERATOR_SYSTEM_GUIDE.md](docs/OPERATOR_SYSTEM_GUIDE.md) for the full operator workflow, architecture diagrams, and documentation contract.

## Features

### Agent Monitoring

| Feature | Description |
|---------|-------------|
| **16+ Agents** | Monitor Claude Code, OpenCode, Codex CLI, Gemini CLI simultaneously |
| **Live Status** | Idle, Working, Awaiting Approval, Error — real-time detection |
| **Task Queue** | Priority-based task routing with auto-cycle across agents |
| **Build Environments** | Themed multi-pane build setups (Bloodstream, Matrix, Ghost Protocol) |
| **Git Sync** | Rust-native file watcher + auto-commit + auto-push via WebSocket |
| **Context Tracking** | Remaining context window percentage per agent |

### Intelligence Layer

| Feature | Description |
|---------|-------------|
| **206 MCP Tools** | Built-in MCP server — agents can query state, manage tasks, coordinate |
| **Vision Tracking** | Vision-Driven Development with goals, features, tasks, acceptance criteria |
| **Wiki** | Auto-generated Confluence-style documentation from vision files |
| **Analytics** | Capacity planning, sprint tracking, burndown charts, role utilization |
| **Quality Gates** | Multi-framework QA engine (build, test, review, verify, ship) |
| **Multi-Agent Coord** | File locks, port allocation, knowledge sharing between agents |

### Dashboards

| Interface | Description |
|-----------|-------------|
| **TUI** | Ratatui terminal dashboard at 60fps — agent tree, queue, analytics |
| **Web** | Real-time web dashboard with WebSocket streaming on any port |
| **SSE** | Server-sent events for external integrations |
| **REST API** | Full JSON API for all state (40+ endpoints) |

## Web Dashboard

The web dashboard runs alongside the MCP server and provides a real-time view of your entire agent fleet:

- **Execution map** with mission, delivery phases, blockers, and ready features
- **Agent grid** with live terminal output, status, and session metadata
- **Runtime lanes** with provider, worktree, and pane-scoped browser test ports
- **Task queue** with priority management and one-click operations
- **Vision cockpit** showing VDD goals, features, and progress
- **Build environments** with themed pane management
- **Documentation sync** showing whether filesystem docs, git, and dashboard state agree
- **Sync status** with git branch, dirty files, ahead/behind indicators
- **Capacity gauges**, role utilization, sprint board, and activity feed

Access at `http://localhost:3100` (configurable).

## MCP Server

DX Terminal includes a built-in MCP server with 206 tools across 5 domains:

| Server | Tools | Purpose |
|--------|-------|---------|
| `core` | Agent lifecycle, PTY management, pane control | Low-level operations |
| `queue` | Task queue, auto-cycle, priority routing | Work management |
| `tracker` | Issues, sprints, milestones, capacity | Project tracking |
| `coord` | File locks, ports, knowledge base, messaging | Multi-agent coordination |
| `intel` | Analytics, monitoring, quality gates, vision | Intelligence & reporting |

Run as monolith (`dx mcp`) or split servers for faster `tools/list` response.

## Key Bindings

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate agents |
| `y`/`n` | Approve/reject pending |
| `a` | Approve ALL |
| `f` | Focus (jump to pane) |
| `i` | Input mode |
| `D` | Dashboard |
| `X` | Analytics |
| `Q` | Task queue |
| `?` | Help |

## Architecture

Detailed diagrams and operating notes live in [docs/OPERATOR_SYSTEM_GUIDE.md](docs/OPERATOR_SYSTEM_GUIDE.md).

```
┌────────────────────────────────────────────────────────────┐
│                       DX Terminal                           │
│                                                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │ TUI      │ │ Web      │ │ MCP      │ │ Sync         │  │
│  │ Ratatui  │ │ Axum+WS  │ │ Server   │ │ notify+git   │  │
│  │ 60fps    │ │ REST+SSE │ │ 206 tools│ │ auto-push    │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └──────┬───────┘  │
│       │             │            │               │          │
│  ┌────┴─────────────┴────────────┴───────────────┴───────┐ │
│  │                    App Core                            │ │
│  │  StateManager · PTY Manager · Queue · Vision · Screen │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  PTY (portable-pty) · Agent Detection · Analytics      │ │
│  │  Knowledge Base · Build Environments · Quality Gates   │ │
│  │  Capacity Planning · Multi-Agent Coordination          │ │
│  └────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

All Rust. Single binary. No external runtime dependencies.

Documentation and hosted UI should not invent their own state. They should consume:

- snapshot: `GET /api/project/brief?project=...`
- live events: websocket `vision_changed`, `focus_changed`, terminal/session updates
- filesystem docs from the project root and `.vision/*`

## Comparison

| Feature | DX Terminal | claude-squad | tmux |
|---------|:-----------:|:------------:|:----:|
| Language | Rust | Go | C |
| Agents monitored | 16+ | 1 | Manual |
| Dashboard views | TUI + Web | 1 | 0 |
| MCP tools | 206 | 0 | 0 |
| Task queue | Priority-based | No | No |
| Vision/VDD tracking | Built-in | No | No |
| File sync | Rust-native | No | No |
| Build environments | Themed multi-pane | No | No |
| Wiki generation | Auto from vision | No | No |
| Memory usage | <5MB | ~20MB | ~3MB |
| Agent types | 4+ | Claude only | N/A |

## Configuration

```bash
dx --init-config          # Generate default config
```

Config at `~/.config/dx-terminal/config.json`:
```json
{
  "web_port": 3100,
  "poll_interval_ms": 500,
  "capture_lines": 100,
  "auto_cycle_interval": 60
}
```

## Contributing

```bash
git clone https://github.com/pdaxt/dx-terminal.git
cd dx-terminal
cargo test
cargo clippy -- -D warnings
cargo fmt
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

**Built for developers who orchestrate AI agents at scale.**

</div>
