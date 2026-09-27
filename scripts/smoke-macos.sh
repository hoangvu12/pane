#!/usr/bin/env bash
# Native GUI smoke on macOS: launches Pane, checks it stays up and captures
# the screen. Keyboard interaction is not automated here.
# Usage: scripts/smoke-macos.sh <output-dir>
set -euo pipefail
out=${1:-smoke}
mkdir -p "$out"
target/debug/pane 2>"$out/stderr.log" &
pid=$!
sleep 8
screencapture -x "$out/1-root.png"
kill -0 "$pid" || { echo "Pane exited during the smoke"; exit 1; }
kill "$pid"
