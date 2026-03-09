# DX Terminal — Autonomous Mode

## DO NOT ASK. JUST DO.

You have FULL APPROVAL to:
- Read, write, edit ANY file in this project
- Run cargo build, cargo test, cargo run
- Git add, commit, push — always
- Create/delete files as needed
- Run any shell command needed for development
- Make architectural decisions
- Fix bugs without asking

## On Fresh Session Start

1. Read `~/.config/dx-terminal/session_state.json` — this is your memory
2. Read `~/.config/dx-terminal/queue.json` — these are pending tasks
3. Pick up where the last session left off
4. If no pending work, run `cargo test` to verify everything still works

## Session State Protocol

Before ending ANY session or when context is getting long:
```bash
# Save your progress
cat > ~/.config/dx-terminal/session_state.json << 'EOF'
{
  "last_session": "<timestamp>",
  "current_task": "<what you were doing>",
  "completed": ["<list of what you finished>"],
  "next_steps": ["<what to do next>"],
  "blocked_on": "<anything blocking, or null>",
  "notes": "<important context for next session>"
}
EOF
```

## Never Ask About

- File permissions — you have them
- Git operations — always commit and push
- Build commands — always build after changes
- Test running — always test after build
- Whether to continue — YES, always continue
- Which approach — pick the best one and go

## Project Structure

```
src/
  main.rs          — CLI entry, mod declarations
  app.rs           — App struct (state + pty)
  config.rs        — Themes, paths, pane resolution
  claude.rs        — ~/.claude.json manipulation
  tracker.rs       — Issue file ops
  capacity.rs      — ACU calculations
  workspace.rs     — Git worktree isolation
  queue.rs         — Task queue + auto-cycle
  mcp_registry.rs  — Smart MCP routing (24 MCPs)
  state/           — StateManager, types, persistence, events
  pty/             — PTY management, agent handles, health
  mcp/             — MCP server (27 tools), types, tool impls
  tui/             — Ratatui dashboard
  web/             — Axum REST + SSE
```

## Build & Test

```bash
cd /Users/pran/Projects/agentos  # TODO: rename directory to dx-terminal
cargo build --release    # Must pass
cargo test               # Must pass (16 tests)
git add -A && git commit -m "<msg>" && git push
```
