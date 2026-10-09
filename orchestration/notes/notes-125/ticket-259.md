## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

Settings' Keyboard page shows, per binding, how it is dispatched — through Windows' registration or through Pane's keyboard hook — and the hook's health: whether it is installed, how many times it has been reinstalled, and whether its pages are pinned. Copy Diagnostics includes them. Rows of hook-based bindings also say there that they do nothing while an elevated application is in front.

## Acceptance criteria

- [ ] Each hotkey's row in Settings shows its dispatch route
- [ ] The hook's health — installed, reinstall count, pages pinned — shows on the Keyboard page
- [ ] Copy Diagnostics includes the hook's state
- [ ] Window tests show a hotkey row's route with the fake adapter

## Blocked by

- #252 — Fall back to a low-level keyboard hook when Windows refuses a hotkey

