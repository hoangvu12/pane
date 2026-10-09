=== #239 Push, replace and pop views on a navigation stack [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 6 of the parent. Size M.

- Pane owns a stack of view resources per opened command. `outcome.push`, `replace` and `pop` (with an optional result) change it. The SDKs expose `push`, `replace`, `pop`, the Raycast-style `Action.Push` and an `onPop` handler.
- Escape pops at once, shows the previous view's last tree, then delivers the pop event to it. Before popping, Escape clears a non-empty search field, closes an open dropdown or menu, or cancels composition, in that order. Backspace on an empty search field pops, but not on key repeat.
- Pop to root (Shift+Esc) drops the whole stack. A depth limit of 32 refuses a further push as the extension's error.
- Each view's `navigation-title` is shown in the search header.
- Popped views' resources are dropped.
- A glossary entry for the navigation stack.

## Acceptance criteria

- [ ] `Launcher` seam: push, replace, pop with a result, the Escape order, pop to root and the depth bound, in Rust, JS and TS samples.
- [ ] Window tests for Escape and Backspace navigation.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/235

