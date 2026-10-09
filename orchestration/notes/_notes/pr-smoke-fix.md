#192 writes `clipboard-history.json` compactly (`serde_json::to_string`), but the smokes still grep the pretty form (`"capture": "paused"`, with a space), so every smoke's clipboard phase now times out on main: release run [37901126719](https://github.com/pane-app/pane/actions/runs/37901126719) fails `Smoke (windows-2025)` and `Smoke (ubuntu-24.04)` at `clipboard-history.json: "capture": "paused" is not present`. (Smokes never run on ticket branches, so the verify tier could not catch it.)

This changes the three affected greps in each smoke (`"capture": "paused"`, `"capture": "on"`, `"retentionSeconds": 3600`) to the compact form the file now always has. Plain-text greps (`pane-smoke-second` and friends) and the greps of other files (written pretty, as before) are untouched.

Refs #192
Refs #274

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
