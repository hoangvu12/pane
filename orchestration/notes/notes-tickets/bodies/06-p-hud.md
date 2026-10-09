## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

The HUD window that extension commands already show (#141) is shaped to the specification: content-sized, 46 logical pixels tall (56 with a second line), at most 500 wide, centred horizontally on the monitor the launcher last showed on, its bottom edge 150 logical pixels above that monitor's bottom. It draws an optional icon, a one-line title and an optional one-line message, in Pane's popover material and type. A default or success HUD shows for 1.2 seconds, a failure for 3 seconds, then fades out over about a second; a pending HUD stays until it is updated or the launcher becomes active again; one at a time, a new one replacing the current. Under reduced motion there is no fade. The HUD never takes focus and lets clicks through. Its text is announced through the platform's notification mechanism for an unfocused window (a UI Automation notification on Windows) and through the harness's announcement capture. The toast-to-HUD route when the launcher is hidden or in the compact window mode keeps working, and the Wayland limitation — where a client cannot place such a window, a toast-like message takes its place — stays documented.

## Acceptance criteria

- [ ] The HUD places centred, its bottom edge 150 logical pixels above the monitor's bottom, 46 (or 56) pixels tall, at most 500 wide, with icon, title and message
- [ ] 1.2-second and 3-second durations, fading out over about a second; no fade under reduced motion
- [ ] A pending HUD stays until updated or the launcher becomes active; a newer HUD replaces it
- [ ] The HUD never takes focus, is click-through, and shows on the monitor the launcher used
- [ ] The message is announced (UI Automation on Windows; announcement capture in the harness)
- [ ] Window-harness tests cover timing, replacement, focus never taken and the toast-to-HUD route while hidden or compact; the smoke scripts check placement and click-through on Windows
