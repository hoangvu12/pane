#!/usr/bin/env bash
# Native GUI smoke on X11 (Xvfb): launches Pane, drives it with real key
# events and captures screenshots. Requires xvfb-run, xdotool and ImageMagick.
# Usage: scripts/smoke-linux.sh <output-dir>
set -euo pipefail
out=${1:-smoke}
mkdir -p "$out"
export out
xvfb-run -a -s "-screen 0 1280x800x24" bash -c '
  set -euo pipefail
  target/debug/pane 2>"$out/stderr.log" &
  pid=$!
  for _ in $(seq 50); do
    window=$(xdotool search --pid "$pid" 2>/dev/null | head -1) && [ -n "$window" ] && break
    sleep 0.2
  done
  [ -n "${window:-}" ] || { echo "Pane window did not appear"; exit 1; }
  sleep 2
  import -window root "$out/1-root.png"
  xdotool windowfocus --sync "$window" key Return; sleep 2
  import -window root "$out/2-command.png"
  xdotool key Down key Return; sleep 2
  import -window root "$out/3-action-result.png"
  xdotool key Escape; sleep 1
  import -window root "$out/4-back-to-root.png"
  kill -0 "$pid" || { echo "Pane exited during the smoke"; exit 1; }
  kill "$pid"
'
echo "screenshots in $out"
