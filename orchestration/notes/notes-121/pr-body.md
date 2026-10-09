## What this does

Implements spec #121: extensions describe their UI as a versioned JSON tree of UI components that Pane renders natively with GPUI, in Pane's design.

Authors write it the way natural to their language — a GPUI-like builder in Rust (`column().gap(Space::M).children([...])`) and JSX in JavaScript/TypeScript (`<Column gap="m">…</Column>`) — and the SDKs hide the wire format, which is a versioned JSON document carried through the small typed WIT envelope #135 introduced.

On top of the tree:

- layout primitives (row, column, stack, scroll, spacer, divider; sizing and surfaces with hover/pressed variants), tokens and raw values with contrast correction, icons and images
- Pane's own shared UI components as first-class nodes, extracted into data-driven form and used by Pane's own screens too
- a keyed host reconciler that keeps focus, typing, selection and scroll across re-renders, with partially controlled inputs
- the standard views: List with sections, filtering, pagination, empty view, detail and a search-bar dropdown; Detail with Markdown and metadata; Grid; Form with the full field set — replacing the typed form and `search: true` command search
- a navigation stack (push, replace, pop; Escape pops)
- `refresh-after-ms` for timers and polling, and (flagged slice) real push re-render on arriving data
- a canvas leaf for drawing, replacing the frame custom view

Tickets landed as one commit each:

- Closes #235
- Closes #236
- Closes #237
- Closes #238
- Closes #239
- Closes #240
- Closes #241
- Closes #242
- Closes #243

Closes #121

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
