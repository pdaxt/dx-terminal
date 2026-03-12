# DX Terminal — Shared Agent Guide

## Operating Mode

Work autonomously. Prefer execution over asking for permission when the next step is clear.

You are expected to:
- inspect the current workspace before making assumptions
- keep the VDD lifecycle, dashboard, docs, hooks, and runtime state aligned
- run the relevant build and test commands after changes
- leave the repo in a verifiable state

## Session Recovery

On a fresh session:
1. Read `~/.config/dx-terminal/session_state.json` if it exists.
2. Read `~/.config/dx-terminal/queue.json` if it exists.
3. Inspect the current repo state and continue the highest-value unfinished work.
4. If there is no explicit task, verify the repo with `cargo test`.

Before ending a session, update session state with:
- current task
- completed work
- next steps
- blockers
- context needed for the next runtime

## Source Of Truth

Keep these aligned:
- filesystem: `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `GEMINI.md`, `.vision/*`
- git: branch, dirty state, staged/untracked docs
- dashboard: `/api/project/brief` and live websocket events
- runtime state: tmux panes, worktrees, session logs

If they disagree, fix the drift rather than documenting around it.

## Delivery Standard

The target system is provider-neutral. Claude, Codex, Gemini, and future runtimes should all be able to:
- discover the active feature and current phase
- read the same project guidance
- use the same external MCP bridge
- update VDD state without forking the workflow

Do not introduce new provider-specific paths unless there is a neutral shared path first.

## Project Focus

This repo is building a delivery cockpit that replaces fragmented planning and execution tools:
- documentation
- discovery and implementation tracking
- runtime orchestration
- QA and verification visibility
- worktree and branch coordination

The dashboard should read like a product control plane, not a debug console.

## Build And Test

Primary validation:
```bash
cd /Users/pran/Projects/dx-terminal
cargo fmt
cargo test
```

When changing the dashboard or sync model, also verify:
- dashboard HTML/JS parses
- `/api/project/brief` returns coherent documentation state
- frontend audit still improves or stays stable

## File Map

Key areas:
- `src/vision.rs` — VDD lifecycle and evidence model
- `src/bin/vision_hook.rs` — automatic phase-aware hook behavior
- `src/web/api.rs` — canonical dashboard/web contract
- `src/web/ws.rs` — live event transport
- `src/web/replicator.rs` — runtime/doc change detection
- `src/mcp/` — internal MCP surface and gateway bridge
- `assets/dashboard.html` — operator cockpit UI

## Provider Overlays

Provider-specific notes may live in:
- `CLAUDE.md`
- `CODEX.md`
- `GEMINI.md`

Those files extend this guide. They should not contradict it.
