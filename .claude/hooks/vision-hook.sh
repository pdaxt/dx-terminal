#!/bin/sh
set -eu

REPO_DIR=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
BIN="$REPO_DIR/target/debug/vision-hook"

if [ -x "$BIN" ]; then
  exec "$BIN"
fi

exec cargo run --quiet --manifest-path "$REPO_DIR/Cargo.toml" --bin vision-hook
