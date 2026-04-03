<div align="center">

# DX Terminal

**AI agent OS — code, orchestrate, ship. One binary to rule them all.**

[![CI](https://github.com/pdaxt/dx-terminal/actions/workflows/ci.yml/badge.svg)](https://github.com/pdaxt/dx-terminal/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Pure_Rust-orange.svg)](https://github.com/pdaxt/dx-terminal)
[![MCP](https://img.shields.io/badge/MCP-206_tools-purple.svg)](https://github.com/pdaxt/dx-terminal)

Single binary. No login. No telemetry. Open source.

`dx` merges two products into one: an **AI coding agent** (chat, fix, review, commit) and an **agent orchestration platform** (MCP server, TUI dashboard, swarm, router). Use it standalone to code, or at scale to run 16+ agents from one screen.

<img src="demo/demo.gif" alt="DX Terminal — dx go launching agents, dashboard, and task queue in a single command" width="800">

[Quick Start](#quick-start) · [Install](#install) · [Agent Commands](#agent-commands) · [Orchestration](#orchestration) · [Web Dashboard](#web-dashboard) · [MCP Server](#mcp-server) · [Microservices](#cli-microservices-architecture) · [Architecture](#architecture) · [Contributing](#contributing)

</div>

---

## Why

Most AI coding tools do one thing: either they're an agent that writes code, or they're a dashboard that watches agents. `dx` does both. Use `dx chat` to pair-program with a local model, `dx fix` to auto-repair your codebase, or `dx go` to launch a full fleet of agents on your open issues — all from the same binary, with the same state, the same MCP server, the same dashboard.

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

### Code with AI

```bash
dx chat                     # Interactive AI coding REPL (local Ollama models)
dx fix                      # Find and fix issues automatically
dx review                   # Review uncommitted changes (read-only)
dx explain                  # Explain the current codebase
dx test                     # Run tests and fix failures
dx commit                   # Generate commit message and commit
dx pr                       # Generate PR description and create PR
dx run "add error handling" # Single-prompt agent run
dx setup                    # Download and configure a local model
```

### Orchestrate agents

```bash
dx go                       # Zero-config launch: tmux + agents + dashboard
dx swarm start --repo .     # Assign agents to open GitHub issues
dx swarm status             # Check swarm progress
dx tui                      # TUI dashboard (standalone operator console)
dx web --port 3100          # Web dashboard only
dx mcp                      # MCP server (stdio, all 206 tools)
dx ci                       # Local CI gate (check + test + clippy)
```

### Route and discover

```bash
dx router route "fix rust bug"    # Best provider for this task
dx router stats                   # Provider usage + cost history
dx services list                  # 8 internal + 39 external services
dx services topology              # Service dependency graph
dx external list                  # Imported tool servers from Claude/Codex
dx external run playwright click_text --args '{"text":"Login"}'
```

See [docs/OPERATOR_SYSTEM_GUIDE.md](docs/OPERATOR_SYSTEM_GUIDE.md) for the full operator workflow.

## Agent Commands

`dx` ships a built-in AI coding agent powered by local models (Ollama). No API keys, no cloud — runs entirely on your machine.

| Command | Permission | What it does |
|---------|-----------|--------------|
| `dx chat` | workspace-write | Interactive REPL — pair-program with AI |
| `dx run "<prompt>"` | workspace-write | Single-prompt agent run (configurable: `--permission`, `--max-turns`) |
| `dx fix` | workspace-write | Scan codebase, find issues, fix them |
| `dx review` | read-only | Review uncommitted changes without modifying anything |
| `dx explain` | read-only | Explain what the codebase does |
| `dx test` | full-access | Run tests, analyze failures, fix them |
| `dx commit` | full-access | Generate a commit message from the diff and commit |
| `dx pr` | full-access | Generate PR description and create a pull request |
| `dx setup` | — | Detect system capabilities and download a local model |

Permission levels: `read-only` (can only read files), `workspace-write` (can edit project files), `full-access` (can run any command).

## Orchestration

### Swarm — Issue-to-PR at scale

```bash
dx swarm start --repo pdaxt/dx-terminal --max-agents 5 --label bug
dx swarm status
dx swarm stop
```

Assigns agents to open GitHub issues, creates worktrees, and drives each issue to a PR. One command to parallelize your backlog.

### Router — Provider-neutral agent routing

```bash
dx router route "fix a bug in rust"
# → {"provider": "claude", "score": 89.0, "reasons": ["matched strength 'rust'"]}

dx router stats    # Usage per provider
dx router cost     # Cost-per-provider breakdown
dx router add-rule "(?i)\\bpython\\b" aider "Python defaults to aider"
```

Routes tasks to the best provider (Claude, Codex, Gemini, Aider) based on language, task type, cost, and historical success rate. Regex rules for custom routing.

### Go — Zero-config project launch

```bash
dx go                                    # Launch with defaults
dx go --agents 5 --issues 10            # Customize fleet size
dx go --dry-run                          # Preview the plan
dx go --auto-approve                     # Auto-approve permission prompts
```

Detects your project, pulls open issues, launches a tmux session with agent panes and a dashboard — one command from clone to shipping.

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

## CLI Microservices Architecture

DX Terminal now exposes its service architecture directly through the CLI. The CLI is the control plane: internal split MCP domains are treated as services, and external micro-MCPs are managed through the embedded gateway.

```bash
dx services list --kind internal
dx services topology
dx services inspect core
dx services serve intel --no-web
dx services inspect pqvault
dx services call pqvault vault_status
```

Internal services:

- `mcp` is the API facade over the split domains.
- `core`, `queue`, `tracker`, `coord`, and `intel` are the internal microservices.
- `web` is the HTTP edge service.
- `gateway` is the embedded integration gateway for external microservices.

## External Tool Commands

Use `dx external ...` for imported tool servers from Claude, Codex, or shared dx catalogs. This is the operator-first surface for external integrations; the internal gateway/MCP bridge remains under the hood.

```bash
dx external list
dx external discover browser --auto-start
dx external inspect playwright
dx external run filesystem read_file --args '{"path":"README.md"}'
```

`dx tools ...` is a shortcut alias for the same command family.

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
┌──────────────────────────────────────────────────────────────────┐
│                          dx (single binary)                       │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  Agent Layer (chat, fix, review, test, commit, pr, run)     │ │
│  │  Ollama · Provider Router · Swarm · Permission Sandbox      │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                              │                                    │
│  ┌──────────┐ ┌──────────┐ ┌┴─────────┐ ┌──────────┐ ┌───────┐ │
│  │ TUI      │ │ Web      │ │ MCP      │ │ Sync     │ │ CI    │ │
│  │ Ratatui  │ │ Axum+WS  │ │ Server   │ │ notify   │ │ Gate  │ │
│  │ 60fps    │ │ REST+SSE │ │ 206 tools│ │ +git     │ │       │ │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └───┬───┘ │
│       └─────────────┴────────────┴────────────┴───────────┘     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  App Core: StateManager · PTY · Queue · Vision · Screen    │ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │  Services: core · queue · tracker · coord · intel · gateway│ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │  External: 39 micro-MCPs (playwright, pqvault, mailforge…) │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

All Rust. Single binary. No external runtime dependencies.

Documentation and hosted UI should not invent their own state. They should consume:

- snapshot: `GET /api/project/brief?project=...`
- live events: websocket `vision_changed`, `focus_changed`, terminal/session updates
- filesystem docs from the project root and `.vision/*`

## Comparison

| Feature | DX Terminal | Claude Code | Codex CLI | claude-squad | tmux |
|---------|:-----------:|:-----------:|:---------:|:------------:|:----:|
| Language | Rust | TypeScript | TypeScript | Go | C |
| Built-in AI agent | Local models | Cloud API | Cloud API | No | No |
| Agent orchestration | 16+ agents | 1 | 1 | 1 | Manual |
| Dashboard | TUI + Web | No | No | TUI | No |
| MCP server | 206 tools | N/A | N/A | No | No |
| Swarm (issue→PR) | Built-in | No | No | No | No |
| Provider routing | 4 providers | Claude only | OpenAI only | Claude only | N/A |
| Task queue | Priority-based | No | No | No | No |
| CI gate | Built-in | No | No | No | No |
| Service catalog | 8 internal + 39 ext | No | No | No | No |
| Memory usage | <5MB | ~100MB | ~50MB | ~20MB | ~3MB |
| Requires API key | No (local) | Yes | Yes | Yes | N/A |

## Local CI Gate

```bash
dx ci                     # Full gate: cargo check + test + clippy
dx ci --no-clippy          # Skip clippy
dx ci --no-test            # Skip tests
dx ci --no-fail-fast       # Run all steps even if one fails
```

Blocks `git push` on failure when used as a pre-push hook. Live output streaming shows each step's progress.

## Configuration

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

**One binary. Code with AI, orchestrate agents, ship faster.**

</div>
