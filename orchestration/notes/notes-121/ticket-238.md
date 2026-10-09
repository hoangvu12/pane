=== #238 Keep focus, typing, selection and scroll across re-renders by key, with partially controlled inputs [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 4 of the parent. Size L.

- Pane reconciles each new tree against the previous one by path key. Per key it keeps:
  - text, caret, selection and IME composition
  - select and dropdown state
  - focus, hover and pressed
  - scroll position
  - expanded or collapsed sections
  - stable accessibility ids

  A key that disappears loses its state, and a different type under the same key is a new node. This generalises the form's existing reconciliation.
- Interactive nodes take a key. In development mode, duplicate sibling keys and stateful nodes without one are logged in the extension's log (#212), and Pane falls back to matching by position.
- **Partially controlled inputs:** Pane edits at once and sends `input` (coalesced, and throttled if asked) and `change` on commit. A value the extension sets wins only when it differs from that node's value in the previous render.
- **Stale events:** an event on a node the user could see is delivered while that key still has a handler; otherwise it is dropped and logged in development.
- The remaining event set: focus, blur and key on focusable nodes.
- `useRef`, and the inputs' typed properties, in both SDKs.

## Acceptance criteria

- [ ] Window tests: caret, IME composition, selection, scroll, focus and hover survive re-renders and reorders; Tab order.
- [ ] Typing fast while the extension echoes the value loses nothing; a value set by the extension replaces the text.
- [ ] Stale events are delivered or dropped as described; development logs duplicate and missing keys.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/237

