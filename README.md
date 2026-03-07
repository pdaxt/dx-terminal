<p align="center">
  <h1 align="center">AgentOS</h1>
</p>

<p align="center">
  <strong>The operating system for AI agent fleets.</strong>
</p>

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-compatible-brightgreen)](https://modelcontextprotocol.io)

**Spawn, orchestrate, and monitor dozens of AI coding agents working simultaneously across multiple projects. One runtime. Zero chaos.**

---

## What Is AgentOS?

AgentOS is a Rust runtime that turns your machine into a multi-agent development platform. Instead of one AI assistant doing one thing at a time, AgentOS manages fleets of autonomous agents — each in its own isolated environment — building software in parallel.

Think of it as **Kubernetes for AI agents**. Each agent gets its own PTY, git worktree, and task queue. AgentOS handles orchestration, conflict resolution, quality gates, and the entire dev pipeline from code to merge.

Built by [Pranjal Gupta](https://github.com/pdaxt) at [DataXLR8](https://dataxlr8.ai) — part of the DataXLR8 AI infrastructure ecosystem.

### The Problem

You have an AI coding agent. It's great — for one task at a time. But real projects need:
- Multiple agents working on different features simultaneously
- Git branch isolation so agents don't stomp on each other
- Quality gates before code gets merged
- Resource coordination (ports, files, databases)
- Visibility into what every agent is doing

### The Solution

```
┌─────────────────────────────────────────────────────────┐
│                    AgentOS Runtime                        │
│                                                          │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐   │
│  │ Agent 1 │  │ Agent 2 │  │ Agent 3 │  │ Agent N │   │
│  │ Feature │  │ Bug Fix │  │  Tests  │  │  Docs   │   │
│  │ branch/1│  │ branch/2│  │ branch/3│  │ branch/N│   │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘   │
│       │            │            │            │          │
│  ┌────┴────────────┴────────────┴────────────┴────┐    │
│  │         Orchestration Layer                      │    │
│  │  • Git worktree isolation                        │    │
│  │  • Port allocation                               │    │
│  │  • File lock coordination                        │    │
│  │  • Quality gates (build → test → lint → review)  │    │
│  │  • Auto-merge pipeline                           │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ TUI      │  │ Web API  │  │ MCP      │              │
│  │ Dashboard│  │ + SSE    │  │ Server   │              │
│  └──────────┘  └──────────┘  └──────────┘              │
└─────────────────────────────────────────────────────────┘
```

## Features

### Agent Management
- **Spawn agents** into isolated PTY sessions with project context
- **Role-based assignment** — developer, QA, security, devops, PM, architect
- **Auto-cycle** — agents automatically pick up the next task when done
- **Health monitoring** — detect and restart crashed agents
- **Graceful shutdown** — all PTY children cleaned up on exit

### Git Isolation
- **Worktree per agent** — each agent works on its own git worktree
- **Branch coordination** — claim/release branches to prevent conflicts
- **Pre-commit checks** — verify no file lock conflicts before commit
- **Auto-merge pipeline** — dev → gate → QA → review → merge

### Factory Pipeline
- **Natural language → pipeline** — describe what you want, get a full CI pipeline
- **Stages**: dev → build/test/lint gate → QA → code review → merge
- **Templates**: full, quick, secure, hotfix
- **Auto-retry** on transient failures

### Multi-Agent Coordination
- **Port allocation** — no two agents fight over the same port
- **File locking** — coordinate edits to shared files
- **Knowledge base** — agents share discoveries across sessions
- **Message passing** — direct agent-to-agent communication
- **Machine registry** — cross-machine agent coordination

### Observability
- **TUI dashboard** — Ratatui-based terminal UI showing all agents
- **Web dashboard** — Axum REST API + SSE for real-time updates
- **Analytics** — tool usage, file operations, token consumption, commit tracking
- **Audit trail** — every action logged with timestamps

### Quality & Intelligence
- **Quality gates** — automated build, test, lint checks before merge
- **Code auditing** — security scanning, pattern detection, dependency analysis
- **Project scanner** — auto-discover project structure, dependencies, test suites
- **Capacity planning** — sprint estimation, velocity tracking, burndown charts
- **Session replay** — full tool-by-tool replay of any agent session

## 100+ MCP Tools

AgentOS exposes its entire API as an MCP server, making it controllable from any MCP-compatible client.

| Domain | Tools | Purpose |
|--------|-------|---------|
| **Panes** | spawn, kill, restart, assign, collect | Agent lifecycle |
| **Git** | sync, status, push, pr, merge | Isolated git workflows |
| **Queue** | add, decompose, list, auto_cycle | Task management |
| **Monitor** | status, dashboard, logs, health, digest | Observability |
| **Tracker** | issues, milestones, processes | Project tracking |
| **Multi-Agent** | ports, locks, branches, KB, messages | Coordination |
| **Collab** | spaces, docs, proposals, comments | Team collaboration |
| **Knowledge** | graph, replay, facts | Persistent intelligence |
| **Capacity** | estimate, log_work, burndown, velocity | Sprint planning |
| **Analytics** | tool_calls, file_ops, tokens, commits | Usage tracking |
| **Quality** | test, build, lint, deploy, regressions | Quality gates |
| **Dashboard** | overview, detail, leaderboard, alerts | Rich visualization |
| **Scanner** | scan, list, detail, test, deps | Project intelligence |
| **Audit** | code, security, intent, deps, full | Code auditing |
| **Factory** | run, status, gate, retry, cancel | CI/CD pipeline |
| **Gateway** | route to 25+ micro MCP servers | MCP multiplexer |
| **Config** | set_mcps, set_preamble, config_show | Runtime config |

## Quick Start

### Prerequisites
- Rust 1.75+
- tmux (optional, for multi-screen setup)

### Build

```bash
git clone https://github.com/pdaxt/agentos.git
cd agentos
cargo build --release
```

### Run as MCP Server

```bash
# Default: MCP server on stdio + web dashboard on port 4200
./target/release/agentos

# MCP server only (no web)
./target/release/agentos mcp --no-web

# Custom web port
./target/release/agentos mcp --web-port 8080
```

### Run TUI Dashboard

```bash
./target/release/agentos tui
```

### Run Web Dashboard Only

```bash
./target/release/agentos web --port 4200
```

### Add to Any MCP Client

```json
{
  "mcpServers": {
    "agentos": {
      "command": "/path/to/agentos",
      "args": ["mcp"]
    }
  }
}
```

Works with any MCP-compatible client.

## Architecture

```
src/
├── main.rs              # CLI entry (MCP, TUI, Web modes)
├── app.rs               # Core state (PTY manager + shared state)
├── config.rs            # Themes, paths, pane resolution
├── claude.rs            # AI agent config management
├── factory.rs           # CI/CD pipeline engine
├── multi_agent.rs       # Cross-agent coordination
├── workspace.rs         # Git worktree isolation
├── queue.rs             # Task queue with auto-cycle
├── knowledge.rs         # Knowledge graph + session replay
├── machine.rs           # Cross-machine registry
├── analytics.rs         # Usage tracking
├── quality.rs           # Quality gate engine
├── capacity.rs          # Sprint planning
├── scanner.rs           # Project auto-discovery
├── audit.rs             # Code + security auditing
├── mcp_registry.rs      # Smart MCP routing (25+ MCPs)
├── collab.rs            # Team collaboration
├── dashboard.rs         # Rich dashboard generation
├── tracker.rs           # Issue tracking
├── state/               # State management + persistence
├── pty/                 # PTY process management
├── mcp/                 # MCP server (100+ tools)
├── tui/                 # Ratatui terminal UI
├── web/                 # Axum REST API + SSE
└── engine/              # Background tasks (reaper, retention)
```

### Workspace Crates

| Crate | Purpose |
|-------|---------|
| `agentos-types` | Shared types across crates |
| `agentos-gateway` | Micro MCP gateway with lifecycle management |

## How It Works

1. **You describe a task** — "Add dark mode to the settings page"
2. **AgentOS creates a pipeline** — spawns a developer agent on an isolated git worktree
3. **The agent works autonomously** — writes code, runs tests, commits
4. **Quality gates run automatically** — build, test, lint must pass
5. **QA agent verifies** — runs E2E tests, visual regression
6. **Review agent checks** — scans for secrets, patterns, security
7. **Auto-merge** — PR created, merged, branch cleaned up
8. **You get the result** — merged code, test report, and a summary

All while other agents work on other tasks in parallel.

## Background Services

AgentOS runs several background services:

- **Auto-cycle timer** — periodically checks queue and spawns/completes agents
- **Dead agent reaper** — detects and cleans up crashed PTY sessions
- **Lock expiry** — automatically releases stale file locks
- **Data retention** — prunes old analytics and audit data
- **Gateway GC** — shuts down idle micro MCP processes (5-min TTL)

## Configuration

Config lives at `~/.config/agentos/config.json`:

```json
{
  "web_port": 4200,
  "theme_colors": ["cyan", "green", "purple", "orange", "red", "yellow", "silver", "teal", "pink"],
  "cycle_interval_secs": 300,
  "max_agents": 9
}
```

## License

MIT

## Contributing

PRs welcome. Run `cargo test` before submitting. See the architecture section for where to start.

---

**AgentOS: One runtime. Many agents. Zero chaos.**
