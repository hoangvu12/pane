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
key() {  # macOS virtual key codes: 36 Return, 125 Down, 53 Escape, 48 Tab
  osascript -e "tell application \"System Events\" to key code $1"
}
type_text() { osascript -e "tell application \"System Events\" to keystroke \"$1\""; }

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

# The Rust command's form (its fifth item): submitting it empty is rejected
# and focus returns to the name, so typing there and choosing a greeting with
# Tab and Down makes the guest answer.
key 36; sleep 3
for _ in 1 2 3 4; do key 125; done
key 36; sleep 1
capture 6-form.png
key 36; sleep 2
capture 7-form-error.png
check 7-form-error.png f08c8c   # the rejected field's message
type_text Ada
key 48; key 125; key 36; sleep 2
capture 8-form-result.png
check 8-form-result.png 9fd8a8   # the guest's answer
key 53; key 53; sleep 1
stop_pane

# Install the assembled Rust sample package (the folder the picker would
# return), then run its command. Root lists the three samples, the installed
# command, then the install and Manage extensions… rows.
start_pane --install target/guests/packages/sample-rust
capture 9-package.png
check 9-package.png aab4c0   # the package's identity and compatibility lines
key 36; sleep 2
capture 10-installed.png
check 10-installed.png 9fd8a8   # "Installed Rust sample"
key 36; sleep 3
key 36; sleep 2
capture 11-installed-result.png
check 11-installed-result.png 9fd8a8   # the installed guest's answer
stop_pane

# The installed command is still listed after a restart.
start_pane
capture 12-restarted.png
check 12-restarted.png 8a96a3
[ -f "$out/data/extensions/installed.json" ] || { echo "no install record"; exit 1; }
stop_pane

# Install the settings sample, save a choice with it, then disable it in
# Manage extensions. Root lists the three samples, Rust sample, Greeting, the
# install row, then Manage extensions… last; the extension list holds Rust
# sample, then Settings sample.
start_pane --install target/guests/packages/sample-settings
key 36; sleep 2   # Install; Greeting is selected
key 36; sleep 3   # open Greeting
key 36; sleep 2   # "Use a formal greeting"
capture 13-setting-saved.png
check 13-setting-saved.png 9fd8a8   # "Saved the formal greeting"
key 53; sleep 1
for ((i = 0; i < 10; i++)); do key 125; done   # the last row
key 36; sleep 1
key 125; key 36; sleep 2
capture 14-disabled.png
check 14-disabled.png 9fd8a8   # "Disabled Settings sample"
stop_pane
grep -q '"disabled": true' "$out/data/extensions/installed.json" || { echo "disabled state not recorded"; exit 1; }
grep -q '"greeting-style": "formal"' "$out/data/extensions/settings.json" || { echo "setting not saved"; exit 1; }

# After a restart Greeting is no longer in root search. Enabling the package
# again brings it back with its setting: "Greet me" answers in the saved
# formal style, where without a saved style it reports an error.
start_pane
capture 15-restarted-disabled.png
check 15-restarted-disabled.png 8a96a3
for ((i = 0; i < 10; i++)); do key 125; done
key 36; sleep 1
key 125; key 36; sleep 2
capture 16-enabled.png
check 16-enabled.png 9fd8a8   # "Enabled Settings sample"
key 53; sleep 1
for ((i = 0; i < 4; i++)); do key 125; done   # Greeting
key 36; sleep 3
key 125; key 125; key 36; sleep 2   # "Greet me"
capture 17-greeted.png
check 17-greeted.png 9fd8a8   # "Good day to you"
stop_pane
echo "screenshots in $out"
