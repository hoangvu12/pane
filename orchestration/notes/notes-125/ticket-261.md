## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

An optional **game mode** (Windows only, off by default): while it is on, Pane checks on each foreground change — a system foreground event, not a timer — whether the window in front is a game: Windows reports a full-screen Direct3D application in front through its documented notification state, or the program is one the user listed in game mode's settings, so windowed games are covered too. While a game is in front, every Pane hotkey (the Open Pane hotkey as well) is released and the hook passes everything through, so the game gets every key; Pane's hotkeys come back by themselves when the game leaves the front. The tray icon's tooltip says hotkeys are paused while they are.

## Acceptance criteria

- [ ] Game mode is off by default and has its settings (programs to treat as games)
- [ ] Hotkeys pause when a fake foreground source reports a full-screen game or a listed program, and return when it leaves the front, without a timer
- [ ] The tray icon's tooltip says hotkeys are paused
- [ ] The launcher's public-interface tests drive game mode through a fake foreground source

## Blocked by

- #252 — Fall back to a low-level keyboard hook when Windows refuses a hotkey

