# A designed view as a tree

A command whose `pane.json` entry says `"mode": "designed"` opens a
designed view: a screen the extension describes as a **tree of layout
nodes and UI components that Pane renders**, in Pane's design, with
Pane's keyboard behaviour and accessibility (#235, #237, part of #121).
The tree reaches Pane through the typed envelope of
[ADR 0036](https://github.com/hoangvu12/pane/blob/main/docs/adr/0036-extension-ui-is-a-tree-pane-renders-written-with-a-gpui-like-api.md),
which extends the one the list tree uses (`render` and `handle-event`,
[list-tree.md](list-tree.md)) without redefining it.
`pane:extension/command` (`wit/extension.wit`) has what a designed view
needs:

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
  before asking again (#236, see [Refreshes](#refreshes)). `context` is
  JSON too: the sequence number of this render (`{"render": 1, "ui":
  "1.1"}`), so the extension can name each render's callbacks, and the
  version of the component set Pane supports; it can grow without WIT
  changes.
- **`view.handle-event(event) -> result<outcome, string>`** handles the
  user's input: `ui-event` carries the callback id the tree named
  (`callback`), the sequence number of the render whose tree the user saw
  (`render`), the key of the node the event was raised on (`key`, `""`
  when the tree gave it none) and the event's details as JSON
  (`payload`: `"{}"` for a press, `{"value": …}` for a change — the
  value the user chose, a boolean, a string or a number). Pane then calls
  `view.render` again and draws the answer. `outcome` (`push`, `replace`,
  `pop`) changes the view's navigation stack, below.

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
`render` builds the tree with `column()`, `row()`, `stack()`, `text()`,
`button()` and every other constructor of `pane_extension::view`, naming
each control's closure with `cx.listener(|this: &mut V| ...)` and its
style through the same-named methods every builder carries; the SDK
writes the tree. In JavaScript and TypeScript (`@pane-app/extension`) the
exported command's `openView` resolves with `createView(Component)` from
`@pane-app/extension/view`, the component written as JSX (`Column`,
`Row`, `Stack`, `Text`, `Button`, every property type-checked; the
`jsx-runtime` module is the JSX import source) or as the elements
`jsxs(...)` builds directly, with `useState`, `useRef` and `useMemo` for
state; the SDK's runtime writes the tree. Both SDKs keep the callbacks of
the last two renders, so a press of what the user could see is delivered
even if the view has rendered since, and an older event is dropped.

The samples are a counter and a components gallery, in
[Rust](../guests/sample-view/src/lib.rs),
[JavaScript](../guests/sample-view-js/src/index.js) and
[TypeScript](../guests/sample-view-ts/src/index.tsx), held alike by
`crates/pane-core/tests/designed_views.rs`.

## The navigation stack

A designed view's screen holds a **stack** of views: the root view the
command opened, and each view a view's answer pushed above it, the top of
the stack on screen (#239). The stack is Pane's, the extension's state
living in each view's resource; a popped or replaced view's resource is
dropped, never used again. One command's stack holds at most 32 views: a
push that would go beyond it is the extension's error ("the navigation
stack already holds 32 views; a further push is refused"), the view
keeping its last good tree, as it does for any error.

What `handle-event` answers changes the stack, at most one field of the
`outcome` acted on — a `pop` first, then a `replace`, then a `push` (the
SDKs answer exactly one):

- **`push`**: the view the answer names is pushed above the answering
  one, and drawn — its first render. The answering view stays below,
  keeping its state and its tree, which shows again when the pushed view
  pops.
- **`replace`**: the view the answer names takes the answering one's
  place, at its depth; the replaced view is dropped.
- **`pop`**: the answering view pops itself, dropped with its resource,
  answering the string as the pop's result to the view below. Popping
  the root view drops the whole stack, leaving the command as the back
  key's own pop of the root does. A pop that carried no result answers
  the empty string.

The back key (Escape, whatever the Keyboard page binds back to) pops the
top view without asking: the view below's last tree shows **at once**
first — no guest call — and then the view below is told the view above
it popped, re-rendering as after any event. Before popping, the back key
does what it does on every screen: it clears a non-empty search field,
closes an open dropdown or menu, and cancels an input method's
composition, in that order (the "Launcher polish" specification, #123,
owns that order; a designed view shows no search field of its own today,
the List's arriving with #240). Backspace pops the top view too, but not
on key repeat. When the stack holds only the root view, the back key
leaves the command, as it leaves a list command. `window.pop-to-root`
(wit/feedback.wit, the host function) and Shift+Esc drop the whole stack,
leaving for root search at once.

The event that tells a view the one above it popped is the **pop event**:
callback id 0, an id no tree's `onPress` names (the SDKs' ids start at
1), with the pop's result as its payload — `{"pop": "…"}` for a pop that
answered one, `{"pop": null}` for a pop that carried none (the back
key's, so no view's result reached the one below). An answer to it is an
answer like any other: it may itself push, replace or pop.

Authors write the stack through the SDKs: in Rust `cx.push(open)` (the
`on_pop` of `cx.push_with` answering the pushed view's pop),
`cx.replace(open)`, `cx.pop()` and `cx.pop_with(result)`; in JavaScript
and TypeScript `push(target, onPop)`, `replace(target)`, `pop(result)`
and the Raycast-style `Push(target, onPop)` as a button's `onClick`. The
`onPop` handler lives on the pushing side and is never sent over the
wire.

The sample is a navigation sample, in
[Rust](../guests/sample-nav/src/lib.rs),
[JavaScript](../guests/sample-nav-js/src/index.js) and
[TypeScript](../guests/sample-nav-ts/src/index.tsx), held alike by
`crates/pane-core/tests/navigation_stack.rs`.

## Refreshes

A render's answer may ask Pane to draw the view again after some
milliseconds (`refresh-after-ms`), which is how a screen changes by
itself — a timer, a clock, a progress bar, a poll of a service — with no
change to the extension runtime (#236). The ask is clamped to a floor of
100 ms and a ceiling of 24 hours; each refresh is an ordinary guest call
(`render` again, with no event), so it holds other extensions' calls
while it computes, exactly as any call does. Pane sends it numbered with
the view's events — at most one refresh in flight at a time, and a late
answer never replaces a newer tree — and wakes the window for each
answer through the change channel the continuing services use.

- Each answer's own ask paces the view: the refresh happens the delay
  after the answer that asked for it (the period is the delay plus the
  render's own time, as "draw me again after this" reads). Time that
  passes while a refresh runs, or in one jump of the clock, is served by
  the next refresh, not replayed.
- Refreshes run only while the view is on top of its stack — the view
  the screen shows; a view below another, or one whose screen the user
  left, refreshes nothing — and while the launcher's window is shown
  expanded, not
  hidden or collapsed to its search field, where no view is drawn: a
  hidden screen refreshes nothing, so it uses no CPU or battery. A
  refresh that fell due otherwise waits for the next showing, which runs
  it at once — one refresh from the clock's current time, not the ticks
  it missed. Leaving the view cancels its refresh; a view opened afresh
  asks anew.
- An answer that asks for no refresh — one that failed, or the view's
  done — ends any asked before it: the ask belongs to the answer, and
  every answer rules.

The SDKs build their asynchronous helpers on this until the real-push
slice (#243) lands. The JavaScript and TypeScript SDK has `useInterval`
(run once each time `ms` passes while the view is open) and `usePending`
(data awaited in the prompt refresh a loading state asks for); the Rust
SDK has `.refresh_after(Duration)` on the render's root, `loading(tree)`
for the loading state, `Pending` for the data, and `cx.refreshed()`, true
on the render that answers the view's own ask, where an interval's work
runs. A view with pending data renders its loading state at once, asks
for a prompt refresh, and in it awaits the pending work — one refresh's
worth of loading state, so fast screens never flicker; the await is
bounded by the call's limits, as any guest compute is. The sample is a
timer, in [Rust](../guests/sample-timer/src/lib.rs),
[JavaScript](../guests/sample-timer-js/src/index.js) and
[TypeScript](../guests/sample-timer-ts/src/index.tsx), held by
`crates/pane-core/tests/view_refresh.rs`.

## The document

A document is one JSON object: its version and its root node.

```json
{
  "version": "1.1",
  "root": {
    "type": "column",
    "key": "main",
    "gap": "m",
    "padding": { "x": "l", "y": "s" },
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
  minor; 1.1 added the layout primitives, the shared UI components, the
  tokens' colour grammar and Markdown (#237).
- Every node has a `type`, and may have:
  - `key`: the node's stable identity among its siblings, which Pane
    keeps node state under (the keyed reconciler, #238; the focus of a
    control survives a re-render that still draws it);
  - `name`: what assistive technology reads the node by, when the node's
    own content does not name it (a column's or row's);
  - `navigationTitle`: what names the view, read from the root node only
    — the screen's title, shown where a screen's title is (the footer's
    left names it as it names a custom view's; the search header, once
    the List's search field owns it, #240). Ignored on any other node;
  - `requires`: the minimum minor version of the component set the node
    needs; a node Pane cannot satisfy degrades as an unknown node does;
  - `fallback`: the node drawn instead when Pane does not know the node;
  - `children`: the node's children, in order;
  - the **style** every node carries, below.

### The style every node carries

Any node, whichever its type, may set how it takes space and the surface
it draws — and Pane applies the `hover` and `pressed` variants of that
surface itself, with no call into the extension (Figma-style):

- **Sizing**: `grow` and `shrink` (numbers), `basis`, `width`, `height`,
  `minWidth`, `maxWidth`, `minHeight` and `maxHeight` (a length), and
  `aspectRatio` (a number, its width over its height; a non-positive one
  is left out).
- **Surface**: `background` (a colour), `border`
  (`{ "width": <length>, "color": <colour> }`, either alone allowed),
  `radius` (a radius) and `opacity` (0 to 1).
- **Variants**: `hover` and `pressed`, each an object restating any of
  the surface's properties, drawn while the pointer is over the node or
  the node is pressed.
- **In a stack**: `place` (one of the nine places) and `offset`
  (a length, or `{ "x": ..., "y": ... }`), which move the node from its
  place.

A **length** is a space token (`"xs"`–`"xxl"`), a number of pixels, a
string of pixels (`"12px"`), or a fraction of the parent (`"1/2"` or
`"50%"`). A **radius** is a radius token (`"s"`, `"m"`, `"l"`, `"full"`)
or pixels. Raw lengths are clamped to 0–4096 px; raw opacity to 0–1.

### Tokens and raw values

A **colour** is accepted wherever a token is, in any of its forms:

- a **tone token** by its name: `neutral` (the primary ink), `accent`,
  `success`, `warning`, `danger`, `primary`, `secondary`, and the seven
  palette colours `red`, `orange`, `yellow`, `green`, `blue`, `purple`,
  `magenta`;
- a **raw colour**: `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`, `rgba()`,
  `hsl()` or `hsla()`, channels as numbers or percentages;
- a **pair**: `{ "light": <colour>, "dark": <colour> }`, one per
  appearance;
- an **exact** colour: `{ "raw": <colour or pair> }`, drawn as it is.

**Contrast correction:** a colour used for *text or an icon* — a text's
`color`, a span's, a link's, a tag's, a tint — is corrected against the
surface it is drawn on, by moving its lightness until it reads at **2.5:1**
(the ratio the spec chose, deliberately below Pane's own icon 3.0 and
text 4.5), unless it is given as `raw`. A colour used for a *background*
is drawn as it is; the text and icons on it are corrected against it,
composited over what is behind.

The token set also names the **space** tokens (`xs`–`xxl`), the **text
style** (`heading`, `title`, `body`, `caption`, `mono`, `small-mono`) and
**text level** (`primary`, `secondary`, `tertiary`, `quaternary`) of #235,
the **radius** tokens (`s`, `m`, `l`, `full`), and the **icon size**
tokens (`s`, `m`, `l`, `xl`) — a stable mapping layer over Pane's
private theme, resolving per appearance (and over the background image),
so the theme can change without renaming a token.

Buttons are focusable, in tree order: Tab and Shift+Tab move through
them, and Enter and Space press the focused one. Escape stays with Pane,
as it does for a custom view: it pops the navigation stack (above), and
leaves the screen when only the root view is on it.

### Layout primitives

- `column`, `row`: children below or beside each other, with `gap`,
  `padding` (as #235), `align`, `justify` and `wrap`.
- `stack`: children drawn over each other, each placed at one of the nine
  places — `place` on the child, the stack's `align` the default — with
  an optional `offset` from it.
- `scroll`: a scrolling region, `orientation` `"vertical"` (the default)
  or `"horizontal"`, whose position Pane keeps by key.
- `spacer`: space, growing to fill what it is given unless its style says
  otherwise.
- `divider`: a hairline rule, `orientation` as a scroll's.
- `card`: children on Pane's own card surface, with a column's layout.

Layout is computed by GPUI's flex layout; nothing is laid out by the
extension.

### Text

One text: `text` (a string), or `spans` — an array of
`{ "text", "style", "level", "color", "code", "onPress" }` — where a span
with `onPress` is a **link**, its callback run by a press. A text takes
`style`, `level`, `color` (a colour), `size` (pixels), `weight`
(100–900) and `truncate` (one line with an ellipsis). In the SDKs the
children spell the `text`, and `Span` children make the spans.

### Shared UI components

Pane's own, drawn from Pane's own families; each has one accessibility
mapping (role, name, value, state) owned by the host. All are focusable
where they name a callback, in tree order: Tab and Shift+Tab move through
them, Enter and Space press or change the focused one, and the segmented
control's and slider's arrows move them. Escape stays with Pane, as it
does for a custom view: it leaves the screen.

- **`button`** — `label`, `tone` (`default`, `secondary`, `ghost`,
  `accent`, `destructive`), `icon`, `keys` (a key sequence drawn after
  its label), `enabled` and `onPress`. Without `onPress` it draws its
  label as a text.
- **`link`** — `label`, `color`, `onPress`: its label underlined in the
  accent.
- **`icon`**, **`icon-tile`** — the icon model below, `size`; the tile
  draws it on Pane's own tile.
- **`image`** — the icon model below as `image`, `size`, `fit`
  (`contain`, the default, `cover`, `fill`); its children are its
  placeholder, drawn while it loads or cannot be read.
- **`rich-row`** — `title`, `subtitle`, `icon`,
  `accessories` (`{ "text", "tag", "color" }`), `onPress`: the
  launcher's own result row as a component.
- **`keycap`** — `key`: the key its cap shows.
- **`key-sequence`** — `keys`: one cap per key, drawn as the launcher's
  shortcuts are.
- **`tag`**, **`badge`** — `text`, `color`: a short label in a chip, a
  short count in a filled one.
- **`toggle`** — `on`, `label`, `onChange`: the Settings board's switch.
  A change tells the extension `{"value": true|false}`.
- **`checkbox`** — `checked`, `label`, `onChange`: alike, a box with a
  check.
- **`segmented`** — `options` (`{ "value", "label" }`), `value`,
  `label`, `onChange`: the Settings board's track; its arrows move the
  choice, a change telling the extension `{"value": "…"}`.
- **`select`** — `options`, `value`, `label`, `onChange`: a well showing
  the chosen option. Enter and a click step through its options until
  #238's keyed state opens the searchable select.
- **`slider`** — `value`, `min` (0), `max` (1), `step` (0.1), `label`,
  `onChange`: its arrows adjust it by its step, a click moves it to
  where its rail was clicked, and a change tells the extension
  `{"value": <number>}`.
- **`progress`** — `value` (0 to 1), `label`.
- **`loading`** — `label`: an indeterminate bar, its highlight sweeping
  it.
- **`markdown`** — `markdown` (the source; the children spell it): a
  CommonMark subset with GitHub's tables and task lists, drawn in the
  theme's own typography. LaTeX and images are out of scope here.
- **`section-header`** — `title` (the children spell it), `note`.
- **`metadata-list`** — `items`:
  `{ "label", "value", "onPress" (making the value a link), "tags",
  "separator" }`.
- **`empty-state`** — `title`, `description`, `icon`, its children its
  actions: the notice the launcher's own empty board draws.
- **`text-input`**, **`password-input`**, **`text-area`** — `value`,
  `placeholder`, `label`, `onChange`: a well holding the value the tree
  named, focusable, Enter committing it. The value drawn is the value
  the tree names and a commit tells the extension that value; the
  editing state that survives a re-render — the caret, the typing —
  arrives with #238's keyed reconciler.

### Icons and images

`icon`, `icon-tile`, `image`, a button's `icon`, a rich row's and an
empty state's use the icon model the list tree uses
([list-tree.md](list-tree.md), ADR 0036): a reicon builtin by name, a
packaged PNG or SVG by path (with its `@dark` and `@light` variants, or a
light and dark pair), a web image by URL through the existing bounded
download and cache, a system file or application icon, or bounded inline
`data:` — each with `tint` (a colour, corrected), `mask`
(`circle`, `rounded-rectangle`), `fallback` (four deep) and `tooltip`,
which also names it to assistive technology; an icon without one is
decoration. Web images load as rows' icons do: the fallback shows until
one arrives, and on failure.

## Reading a document

Reading is strict in the document's shape and lenient exactly where
versioning asks for it, as the list tree's is:

- A property Pane does not know is ignored, at every level.
- A token this version does not know is left out, as a newer minor
  version may add one.
- A node whose `type` Pane does not know, or whose `requires` it does not
  meet, draws the `fallback` the tree gave, else its children if it has
  any, and nothing without either.
- An icon Pane cannot read is not drawn (its image node draws its
  placeholder).
- A document that is not JSON, lacks `version` or `root`, or a node's
  `type`, a text's `text` or `spans`, a button's or link's `label`, a
  markdown's `markdown`, a rich row's or section header's `title`, an
  empty state's `title`, a segmented's or select's `options`, a slider's
  or progress's `value`, or gives one of the wrong type, is
  **unreadable**: the command's failure, which Pane reports as its own
  reading of what the extension answered ("Pane could not read what the
  extension answered: ..."), never a crash.
- A document over one of the limits, or of another major version, is the
  **extension's error** ("The extension reported an error: ..."): the view
  keeps its last good tree, which an unreadable tree leaves too.

### Limits

One document may hold at most 10,000 nodes (counting `fallback`
subtrees), be at most 64 levels deep and at most 4 MiB of JSON; each text
node at most 64 KiB; each markdown source at most 1 MiB; each inline
`data:` image at most 1 MiB. Raw lengths are clamped to 0–4096 px, raw
opacity to 0–1. One command's navigation stack holds at most 32 views.
Provisional (#121), as the spec's proposed defaults.

The parse and reconcile cost of a 500- and a 5,000-node tree is measured
and recorded with the change (`crates/pane-core/tests/designed_cost.rs`
prints it), not gated.
