#!/bin/bash
# Demo recording script for dx-terminal
# Records a scripted asciinema session showing key features

set -e

CAST_FILE="/Users/pran/Projects/dx-terminal/demo/demo.cast"
GIF_FILE="/Users/pran/Projects/dx-terminal/demo/demo.gif"
DX="/Users/pran/Projects/dx-terminal/target/debug/dx"

echo "Recording dx-terminal demo..."

# Use script to record a controlled session
# We'll launch dx in tmux mode so it picks up running sessions
asciinema rec "$CAST_FILE" \
  --cols 140 \
  --rows 40 \
  --idle-time-limit 2 \
  --title "DX Terminal — AI-native terminal multiplexer" \
  --command "/Users/pran/Projects/dx-terminal/demo/demo-expect.sh"

echo "Converting to GIF..."
agg "$CAST_FILE" "$GIF_FILE" \
  --theme monokai \
  --font-size 14 \
  --speed 1.5

echo "Done! GIF at: $GIF_FILE"
echo "Cast at: $CAST_FILE"
