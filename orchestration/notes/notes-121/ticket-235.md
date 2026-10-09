=== #235 Draw extension views as a tree of rows, columns, text and buttons, written in JSX or a Rust builder [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 1 of the parent, with the first cut of slice 5 (the JSX runtime and the Rust builder), so that authors can write designed views from the start. Size L.

- Extend the typed envelope #135 introduced: the `view` resource with `render(context)` answering `rendered { tree, refresh-after-ms }` and `handle-event(ui-event)` answering an `outcome`, and `open-view(command, launch)`. Only the envelope is type-checked; the tree and the context are JSON.
- A strict host-side node type parsed from the tree, with the parent's limits (nodes, depth, bytes, text per node). Over a limit, or an `Err`, is the extension's error and the last good tree stays.
- The first UI components: `column` and `row` (gap, padding, align, justify, wrap), `text` (text style and text level) and `button` (tones), with the space, tone, text-style and text-level tokens mapped onto Pane's theme in light and dark.
- `press` events by per-render callback id: Pane sends the render number, node key and id, then calls `render` again. Events of one view are delivered one at a time, in order; a late answer never replaces a newer tree, and an answer for a view that has left is dropped.
- The UI component set's own `MAJOR.MINOR` version in each document. Unknown properties are ignored, `fallback` and `requires` are honoured, an unknown node without a fallback renders its children, and another major is refused, naming both versions.
- **JS/TS:** a JSX runtime in `@pane-app/extension` (`jsxImportSource`), so the TS samples type-check every property. It provides function components, `useState`, `useRef` and `useMemo`, and the callback table. It does not use React (see the parent's Authoring APIs).
- **Rust:** in `pane-extension`, a GPUI-like builder (`View`, `Cx`, `cx.listener(...)`, `column()`, `row()`, `text()`, `button()`) that emits the document.
- A designed-view sample in Rust, JS and TS that behaves the same in all three (a counter: buttons that change state shown in text).
- Glossary entries for the tree and the UI component set (through `/domain-modeling`).

## Acceptance criteria

- [ ] Opening the sample's view shows its first tree; pressing a button re-renders with the new state, in Rust, JS and TS (`Launcher` seam).
- [ ] Errors and over-limit trees keep the last good tree; unknown nodes with and without a fallback, a newer minor and another major behave as described.
- [ ] Window tests: the four UI components resolve their tokens to the theme in light and dark, and announce their roles and names.
- [ ] The TS sample fails `tsc` on a misspelt property.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

None - can start immediately

