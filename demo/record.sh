#!/bin/bash
# Record dx-terminal demo inside an ATTACHED tmux session
# This ensures dx-terminal sees all running agents
set -e

DEMO_DIR="/Users/pran/Projects/dx-terminal/demo"
CAST="$DEMO_DIR/demo.cast"
GIF="$DEMO_DIR/demo.gif"
DX="/Users/pran/Projects/dx-terminal/target/debug/dx"

# Use an existing attached session + create a temporary window
SESSION="dx-build"
WINDOW="demo-rec"

echo "Creating recording window in attached session '$SESSION'..."

# Create a new window in the attached session
tmux new-window -t "$SESSION" -n "$WINDOW"

# Resize the window/pane for optimal recording
tmux resize-pane -t "$SESSION:$WINDOW" -x 120 -y 35 2>/dev/null || true

# Start asciinema recording inside the attached session
tmux send-keys -t "$SESSION:$WINDOW" "asciinema rec '$CAST' --cols 120 --rows 35 --idle-time-limit 1.5 --overwrite --title 'DX Terminal — AI-native terminal multiplexer' --command '$DX --tmux'" Enter

# Wait for TUI to start and detect agents
echo "Waiting for TUI + agent detection..."
sleep 8

# === DEMO SEQUENCE ===

echo "Navigating agents..."
tmux send-keys -t "$SESSION:$WINDOW" j; sleep 0.5
tmux send-keys -t "$SESSION:$WINDOW" j; sleep 0.5
tmux send-keys -t "$SESSION:$WINDOW" j; sleep 0.5
tmux send-keys -t "$SESSION:$WINDOW" j; sleep 0.5
tmux send-keys -t "$SESSION:$WINDOW" k; sleep 0.5
tmux send-keys -t "$SESSION:$WINDOW" k; sleep 1.5

echo "Dashboard..."
tmux send-keys -t "$SESSION:$WINDOW" D; sleep 3

echo "Close dashboard..."
tmux send-keys -t "$SESSION:$WINDOW" D; sleep 1

echo "Analytics..."
tmux send-keys -t "$SESSION:$WINDOW" X; sleep 3

echo "Close analytics..."
tmux send-keys -t "$SESSION:$WINDOW" X; sleep 1

echo "Queue..."
tmux send-keys -t "$SESSION:$WINDOW" Q; sleep 2.5

echo "Close queue..."
tmux send-keys -t "$SESSION:$WINDOW" Q; sleep 1

echo "Help..."
tmux send-keys -t "$SESSION:$WINDOW" '?'; sleep 3

echo "Close help..."
tmux send-keys -t "$SESSION:$WINDOW" '?'; sleep 1.5

echo "Quitting..."
tmux send-keys -t "$SESSION:$WINDOW" q
sleep 3

# Kill the recording window
tmux kill-window -t "$SESSION:$WINDOW" 2>/dev/null || true

echo ""
echo "Recording saved: $CAST"

# Check recording
FRAMES=$(wc -l < "$CAST")
echo "Frames captured: $((FRAMES - 1))"

# Trim startup (remove everything before TUI renders)
echo "Trimming startup..."
python3 << 'PYEOF'
import json

with open("/Users/pran/Projects/dx-terminal/demo/demo.cast") as f:
    lines = f.readlines()

header = json.loads(lines[0])
events = [json.loads(l) for l in lines[1:]]

# Find first frame with TUI + agents
start_time = None
for ev in events:
    text = ev[2]
    if "agents" in text and "╭" in text and "DX" in text:
        start_time = ev[0]
        break

# Fallback: first TUI frame
if start_time is None:
    for ev in events:
        if "╭" in ev[2] and "╰" in ev[2]:
            start_time = ev[0]
            break

if start_time is None:
    start_time = 0
else:
    start_time = max(0, start_time - 0.05)

# Shift timestamps
trimmed = []
for ev in events:
    if ev[0] >= start_time:
        new_time = round(ev[0] - start_time, 6)
        trimmed.append([new_time, ev[1], ev[2]])

with open("/Users/pran/Projects/dx-terminal/demo/demo.cast", "w") as f:
    f.write(json.dumps(header) + "\n")
    for ev in trimmed:
        f.write(json.dumps(ev) + "\n")

print(f"Trimmed {start_time:.1f}s. {len(trimmed)} frames remaining.")
PYEOF

echo "Converting to GIF..."
agg "$CAST" "$GIF" \
  --theme monokai \
  --font-size 14 \
  --speed 1.8 \
  --last-frame-duration 3

echo ""
echo "Done!"
ls -lh "$GIF"
