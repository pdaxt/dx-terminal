#!/bin/bash
# Scripted demo for asciinema recording
# Launches dx-terminal in tmux mode, navigates views, then exits

DX="/Users/pran/Projects/dx-terminal/target/debug/dx"

# Helper: type with realistic delay
type_slow() {
  echo -n "$1" | while IFS= read -r -n1 char; do
    echo -n "$char"
    sleep 0.05
  done
}

clear

# Show what we're about to do
echo ""
echo "  ┌──────────────────────────────────────────────────┐"
echo "  │  DX Terminal — AI-native terminal multiplexer    │"
echo "  │  Monitor AI coding agents from one screen        │"
echo "  └──────────────────────────────────────────────────┘"
echo ""
sleep 1.5

echo "  $ dx --tmux"
sleep 0.8

# Launch dx-terminal in tmux mode
# Use expect-like approach: run dx in background, send keys, then kill
# Actually, let's just run it for a few seconds and let it render

# Run dx with a timeout — it will show the TUI with live agent data
timeout 12 $DX --tmux 2>/dev/null || true

sleep 0.5
clear

echo ""
echo "  ✓ 9,595 lines of Rust"
echo "  ✓ Real-time agent monitoring"
echo "  ✓ 206 MCP tools"
echo "  ✓ Web UI + TUI + CLI"
echo ""
echo "  github.com/pdaxt/dx-terminal"
echo ""
sleep 2
