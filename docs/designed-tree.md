# A designed view as a tree

A command whose `pane.json` entry says `"mode": "designed"` opens a
designed view: a screen the extension describes as a **tree of layout
nodes and UI components that Pane renders**, in Pane's design, with
Pane's keyboard behaviour and accessibility (#235, part of #121). The tree
reaches Pane through the typed envelope of
[ADR 0036](https://github.com/hoangvu12/pane/blob/main/docs/adr/0036-extension-ui-is-a-tree-pane-renders-written-with-a-gpui-like-api.md),
which extends the one the list tree uses (`render` and `handle-event`,
[list-tree.md](list-tree.md)) without redefining it. `pane:extension/command`
(`wit/extension.wit`) has what a designed view needs:

- **`open-view(command, launch) -> result<view, string>`** opens the
  designed view of the command `command` (its id in `pane.json`, so one
  component can serve several commands), passing its launch record, and
  answers a `view` resource. Pane calls this at the command's launch —
  Enter on its row, its alias, a fallback, its hotkey, its quick slot,
  another command — and never calls the command's `render`. A component
  that serves no designed command answers with an error; Pane never calls
  it then. The custom view's opener is `open-custom-view(item-id)`
  (retired when the canvas lands, #242).
- **`view.render(context) -> result<rendered, string>`** draws the view:
  `rendered` carries the tree as JSON, naming the version of the UI
  component set it uses, and `refresh-after-ms`, how long Pane waits
  before asking again (read and carried, but nothing acts on it until
  timers land, #236). `context` is JSON too: the sequence number of this
  render (`{"render": 1, "ui": "1.0"}`), so the extension can name each
  render's callbacks, and the version of the component set Pane
  supports; it can grow without WIT changes.
- **`view.handle-event(event) -> result<outcome, string>`** handles the
  user's input: `ui-event` carries the callback id the tree named
  (`callback`), the sequence number of the render whose tree the user saw
  (`render`), the key of the node the event was raised on (`key`, `""`
  when the tree gave it none) and the event's details as JSON
  (`payload`, `"{}"` for a press). Pane then calls `view.render` again
  and draws the answer. `outcome` (`push`, `replace`, `pop`) is ignored
  until the navigation stack lands (#239).

Events of one view are delivered one at a time, in order; an answer that
arrives after its view left the screen is discarded, and a tree is shown
only if no later event's tree is on screen yet. An error the extension
answers with — from `open-view`, `render` or `handle-event` — is shown on
the screen while the view stays open with its last good tree; a crash
closes it, as a crash closes a custom view.

The envelope is the only part an extension's component is type-checked
against, so a UI component, a property or an event can be added with no
WIT change, and a component built against an older component set keeps
working.

Authors never see the JSON or the callback ids. In Rust
(`pane-extension`) the command's `open_designed_view` answers with its
view's state, a type implementing `pane_extension::view::View`, whose
`render` builds the tree with `column()`, `row()`, `text()` and
`button()`, naming each button's closure with
`cx.listener(|this: &mut V| ...)`; the SDK writes the tree. In
JavaScript and TypeScript (`@pane-app/extension`) the exported command's
`openView` resolves with `createView(Component)` from
`@pane-app/extension/view`, the component written as JSX (`Column`,
`Row`, `Text`, `Button`, every property type-checked; the `jsx-runtime`
module is the JSX import source) or as the elements `jsxs(...)` builds
directly, with `useState`, `useRef` and `useMemo` for state; the SDK's
runtime writes the tree. Both SDKs keep the callbacks of the last two
renders, so a press of what the user could see is delivered even if the
view has rendered since, and an older event is dropped.

The sample is a counter, in [Rust](../guests/sample-view/src/lib.rs),
[JavaScript](../guests/sample-view-js/src/index.js) and
[TypeScript](../guests/sample-view-ts/src/index.tsx), held alike by
`crates/pane-core/tests/designed_views.rs`.

## The document

A document is one JSON object: its version and its root node.

```json
{
  "version": "1.0",
  "root": {
    "type": "column",
    "key": "main",
    "gap": "m",
    "padding": { "x": "l", "y": "s" },
    "align": "center",
    "justify": "space-between",
    "children": [
      { "type": "text", "text": "Count: 3", "style": "title", "level": "secondary" },
      { "type": "row", "gap": "s", "wrap": true, "children": [
        { "type": "button", "key": "up", "label": "Increment", "onPress": 1 },
        { "type": "button", "key": "down", "label": "Reset", "tone": "destructive", "onPress": 2 }
      ] }
    ]
  }
}
```

- `version`: the version of the UI component set the document targets, as
  `"<major>.<minor>"`. A document of Pane's major and any minor is read;
  one of another major is refused as the extension's error, naming both
  versions. Additive changes (a component, a property, an event) bump the
  minor.
- Every node has a `type`, and may have:
  - `key`: the node's stable identity among its siblings, which Pane
    keeps node state under (the keyed reconciler, #238; the focus of a
    button survives a re-render that still draws it);
  - `name`: what assistive technology reads the node by, when the node's
    own content does not name it (a column's or row's);
  - `requires`: the minimum minor version of the component set the node
    needs; a node Pane cannot satisfy degrades as an unknown node does;
  - `fallback`: the node drawn instead when Pane does not know the node;
  - `children`: the node's children, in order.

### `column`, `row`

A `column` lays its children out below each other; a `row` beside each
other. Both take:

- `gap`: a space token, the gap between children;
- `padding`: a space token for all four sides, or `{ "x": ..., "y": ... }`
  naming the left and right, top and bottom sides, or `top`, `right`,
  `bottom`, `left` naming each one;
- `align`: `start`, `center`, `end`, `stretch` or `baseline` — how
  children are laid out along the cross axis;
- `justify`: `start`, `center`, `end`, `space-between` or `space-around`
  — how children share the main axis;
- `wrap`: whether children wrap onto further lines.

### `text`

One line of text. Its children (in JSX, its content) spell its `text`.
It takes:

- `style`: `heading`, `title`, `body` (the default), `caption`, `mono` or
  `small-mono` — its size, weight and family;
- `level`: `primary` (the default), `secondary`, `tertiary` or
  `quaternary` — its colour, through the alpha of the text colour.

### `button`

A button. Its children spell its `label`. It takes:

- `tone`: `default` (the plain pill), `secondary`, `ghost`, `accent` or
  `destructive` — how it is drawn;
- `onPress`: the callback id a press of it runs. A button without one
  cannot be pressed: its label is drawn as a text.

Buttons are focusable, in tree order: Tab and Shift+Tab move through
them, and Enter and Space press the focused one. Escape stays with Pane,
as it does for a custom view: it leaves the screen.

### Tokens

The properties above name **tokens**, resolved onto Pane's theme (in
light and dark): space (`xs`, `s`, `m`, `l`, `xl`, `xxl` — the named
distances), text style and text level as above, and tone as above. The
public token layer (#237) extracts the mappings; raw values (hex colours,
pixel sizes) and the tokens yet to come (radius, icon size) land with it.

## Reading a document

Reading is strict in the document's shape and lenient exactly where
versioning asks for it, as the list tree's is:

- A property Pane does not know is ignored, at every level.
- A token this version does not know is left out, as a newer minor
  version may add one.
- A node whose `type` Pane does not know, or whose `requires` it does not
  meet, draws the `fallback` the tree gave, else its children if it has
  any, and nothing without either.
- A document that is not JSON, lacks `version` or `root`, or a node's
  `type`, a text's `text` or a button's `label`, or gives one of the
  wrong type, is **unreadable**: the command's failure, which Pane
  reports as its own reading of what the extension answered ("Pane could
  not read what the extension answered: ..."), never a crash.
- A document over one of the limits, or of another major version, is the
  **extension's error** ("The extension reported an error: ..."): the view
  keeps its last good tree, which an unreadable tree leaves too.

### Limits

One document may hold at most 10,000 nodes (counting `fallback`
subtrees), be at most 64 levels deep and at most 4 MiB of JSON, and each
text node at most 64 KiB. Provisional (#121), as the spec's proposed
defaults.
