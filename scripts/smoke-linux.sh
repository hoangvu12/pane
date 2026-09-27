#!/usr/bin/env bash
# Native GUI smoke on X11: starts a virtual X server (Xvfb), launches Pane,
# drives it with real key events and captures screenshots.
#
# Requires Xvfb, xdotool and either ImageMagick (`import`) or Python 3 with
# Pillow for screenshots, plus a Vulkan driver (Mesa's lavapipe works without
# a GPU). Set PANE_XVFB / PANE_XDOTOOL to use binaries outside PATH.
# Usage: scripts/smoke-linux.sh <output-dir> [pane-binary]
set -euo pipefail
out=${1:-smoke}
pane=${2:-target/debug/pane}
xvfb=${PANE_XVFB:-Xvfb}
xdotool=${PANE_XDOTOOL:-xdotool}
mkdir -p "$out"

display=:$((90 + RANDOM % 100))
"$xvfb" "$display" -screen 0 1280x800x24 -nolisten tcp 2>"$out/xvfb.log" &
xvfb_pid=$!
pane_pid=
cleanup() {
  [ -n "$pane_pid" ] && kill "$pane_pid" 2>/dev/null || true
  kill "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT
export DISPLAY=$display
unset WAYLAND_DISPLAY
sleep 1

capture() {
  if command -v import >/dev/null; then
    import -window root "$out/$1"
  else
    python3 -c 'import sys; from PIL import ImageGrab; ImageGrab.grab(xdisplay=sys.argv[1]).save(sys.argv[2])' \
      "$display" "$out/$1"
  fi
}

"$pane" 2>"$out/stderr.log" &
pane_pid=$!
window=
for _ in $(seq 100); do
  window=$("$xdotool" search --pid "$pane_pid" 2>/dev/null | head -1) && [ -n "$window" ] && break
  sleep 0.2
done
[ -n "$window" ] || { echo "Pane window did not appear"; exit 1; }
sleep 2
capture 1-root.png
"$xdotool" windowfocus --sync "$window"

# Open each sample command (Rust, JavaScript, TypeScript) and run an item.
for index in 0 1 2; do
  for _ in $(seq "$index"); do "$xdotool" key Down; done
  "$xdotool" key Return; sleep 3
  capture "$((index + 2))-command-$index.png"
  "$xdotool" key Down key Return; sleep 2
  capture "$((index + 2))-result-$index.png"
  "$xdotool" key Escape; sleep 1
done
capture 5-back-to-root.png

kill -0 "$pane_pid" || { echo "Pane exited during the smoke"; exit 1; }
echo "screenshots in $out"
