## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

Once the hook recognizes the Windows key tapped alone, a **fresh install's Open Pane hotkey on Windows is the Windows key alone**, as Raycast's is — Pane replaces the Start menu as the place the user starts everything. An existing installation keeps the Open Pane hotkey it has; only a fresh data folder gets the new default. If the hook cannot be installed, Pane registers Ctrl+Alt+Space instead and shows why. macOS (Option+Space) and Linux (Ctrl+Alt+Space) keep their defaults.

Beside the Open Pane recorder, shown while another hotkey is set, a **"Use the Windows key"** choice (Raycast's "Replace Start Menu") sets it in one step and says the Start menu stays reachable from the taskbar's Start button and Ctrl+Esc, so only the lone tap changes: Win+E, Win+D, Win+L and the rest keep Windows' meaning. A General-page setting, **"Show the taskbar when Pane opens"**, shows an auto-hiding taskbar while the launcher is open, so the Start button stays one click away. The hotkeys document and the first-run text change with this slice.

## Acceptance criteria

- [ ] A fresh data folder on Windows gets the Windows key alone as the Open Pane hotkey; an existing settings record keeps its hotkey
- [ ] When the fake hook cannot be installed, Ctrl+Alt+Space is registered instead, with the reason shown
- [ ] "Use the Windows key" appears beside the Open Pane recorder while another hotkey is set, sets it in one step, and says where the Start menu remains
- [ ] "Show the taskbar when Pane opens" appears on the General page and shows an auto-hiding taskbar while the launcher is open (fake adapter in tests)
- [ ] The hotkeys document and the first-run text describe the fresh-install default, building on #182
- [ ] The launcher's public-interface tests, with the fake adapter, cover the fresh default, the kept hotkey and the hook-missing fallback

## Blocked by

- #259 — Show each hotkey's dispatch and the keyboard hook's health in Settings
- #260 — Bind lone and double modifier taps, sides and the extended key set
- #182 — Describe the Open Pane hotkey and Settings' Shortcuts page in the hotkeys document and the decision index (https://github.com/pane-app/pane/issues/182)
