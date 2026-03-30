#!/usr/bin/env bash
#
# Install git hooks from hooks/ into .git/hooks/
# Safe to run multiple times (overwrites with latest version).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_SRC="$REPO_ROOT/hooks"
HOOKS_DST="$REPO_ROOT/.git/hooks"

if [ ! -d "$HOOKS_DST" ]; then
    echo "Error: not a git repository (no .git/hooks/)"
    exit 1
fi

installed=0
for hook in "$HOOKS_SRC"/*; do
    [ -f "$hook" ] || continue
    name="$(basename "$hook")"
    cp "$hook" "$HOOKS_DST/$name"
    chmod +x "$HOOKS_DST/$name"
    installed=$((installed + 1))
done

echo "Installed $installed hook(s) into .git/hooks/"
