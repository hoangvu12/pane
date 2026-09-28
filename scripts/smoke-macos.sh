#!/usr/bin/env bash
# Native GUI smoke on macOS: launches Pane, drives it with real key events
# (System Events via osascript, which needs the Accessibility permission the
# GitHub macOS runners grant) and captures and checks the screen.
# Requires Python 3 with Pillow for the screenshot checks. Pane keeps
# installed packages in <output-dir>/data, not the user's data folder.
# Usage: scripts/smoke-macos.sh <output-dir> [pane-binary]
set -euo pipefail
out=${1:-smoke}
pane=${2:-target/debug/pane}
mkdir -p "$out"
rm -rf "$out/data"
export PANE_DATA_DIR=$out/data
{ sw_vers; uname -m; } >"$out/system.txt"   # the tested OS version and architecture

pid=
trap '[ -n "$pid" ] && kill "$pid" 2>/dev/null || true' EXIT

capture() { screencapture -x "$out/$1"; }
check() { python3 "$(dirname "$0")/check_screenshot.py" "$out/$1" "$2"; }
key() {  # macOS virtual key codes: 36 Return, 125 Down, 53 Escape
  osascript -e "tell application \"System Events\" to key code $1"
}

# Starts Pane with the given arguments and brings it to the front.
start_pane() {
  "$pane" "$@" 2>>"$out/stderr.log" &
  pid=$!
  sleep 8
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true"
  sleep 1
}

stop_pane() {
  kill -0 "$pid" || { echo "Pane exited during the smoke"; exit 1; }
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  pid=
}

start_pane
capture 1-root.png
check 1-root.png 8a96a3   # the hint line: text renders

# Open each sample command (Rust, JavaScript, TypeScript) and run an item.
for index in 0 1 2; do
  for ((i = 0; i < index; i++)); do key 125; done   # not seq: BSD "seq 0" prints 1 0
  key 36; sleep 3
  capture "$((index + 2))-command-$index.png"
  key 125; key 36; sleep 2
  capture "$((index + 2))-result-$index.png"
  check "$((index + 2))-result-$index.png" 9fd8a8   # the guest's answer
  key 53; sleep 1
done
capture 5-back-to-root.png
# Each command must have answered from its own guest, not the same view twice.
python3 "$(dirname "$0")/check_screenshot.py" --distinct "$out"/{2,3,4}-result-*.png
stop_pane

# Install the assembled Rust sample package (the folder the picker would
# return), then run its command. Root lists the three samples, the installed
# command, then the install row.
start_pane --install target/guests/packages/sample-rust
capture 6-package.png
check 6-package.png aab4c0   # the package's identity and compatibility lines
key 36; sleep 2
capture 7-installed.png
check 7-installed.png 9fd8a8   # "Installed Rust sample"
key 36; sleep 3
key 36; sleep 2
capture 8-installed-result.png
check 8-installed-result.png 9fd8a8   # the installed guest's answer
stop_pane

# The installed command is still listed after a restart.
start_pane
capture 9-restarted.png
check 9-restarted.png 8a96a3
[ -f "$out/data/extensions/installed.json" ] || { echo "no install record"; exit 1; }
stop_pane
echo "screenshots in $out"
