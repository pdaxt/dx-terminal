#!/usr/bin/expect -f
# Automated dx-terminal demo — captures TUI with live agents

set timeout 30
set dx "/Users/pran/Projects/dx-terminal/target/debug/dx"

# Launch dx in tmux mode (sees all attached tmux sessions)
spawn $dx --tmux

# Wait for TUI + agent detection
sleep 5

# Navigate down through agents
send "j"; sleep 0.5
send "j"; sleep 0.5
send "j"; sleep 0.5
send "j"; sleep 0.5
send "k"; sleep 0.5
send "k"; sleep 1

# Toggle Dashboard
send "D"; sleep 2.5

# Close Dashboard
send "D"; sleep 0.8

# Toggle Analytics
send "X"; sleep 2.5

# Close Analytics
send "X"; sleep 0.8

# Toggle Queue
send "Q"; sleep 2

# Close Queue
send "Q"; sleep 0.8

# Show Help
send "?"; sleep 2.5

# Close Help
send "?"; sleep 1

# Quit
send "q"
expect eof
