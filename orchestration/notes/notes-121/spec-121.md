## Problem Statement

An extension author who wants to show anything beyond a plain list hits a wall. A command can return one list of items with a title and a subtitle, open a form with a single-line text field and a single choice, or draw a custom view out of filled rectangles and one-line text in a fixed-size area. There is no way to lay out content in rows and columns, show an icon, a tag, a keycap, a button or Markdown, put a detail pane beside a list, show a grid of images, ask for a checkbox, a date, a password, several tags or a file, or go from one screen to the next and back. A screen cannot change by itself: a timer, a clock, a progress bar or a poll of a web service can only be shown again when the user presses a key.

Authors coming from Raycast expect List, Detail, Grid and Form with sections, accessories, an empty view, a loading state, a search-bar dropdown, pagination, a navigation stack and about ten form field types; Raycast's extension corpus uses them heavily (isLoading 77% of extensions, List 75%, accessories 54%, Markdown detail 48%, EmptyView 46%, push navigation 45%, sections 44%, Form 39%, search-bar dropdown 30%, Grid 9%). Raycast, however, gives authors only that fixed set: no free layout, no drawing, nothing between "standard view" and "nothing". Pane's users asked for more: an author should be able to design the screen, compose Pane's own UI components freely, and still get Pane's look, keyboard behaviour and accessibility without re-implementing them.

Today's contract also cannot grow without breaking every extension. Each new field on a WIT record changes the shape of an export, and Pane refuses every component built against the earlier shape until it is rebuilt. Adding the dozens of UI components and properties Raycast-level UI needs that way would break every published extension on every addition.

## Solution

Extensions describe their UI as a tree of UI components that Pane renders natively with GPUI, in Pane's design. The author writes it in the way natural to their language: a GPUI-like builder in Rust and JSX in JavaScript and TypeScript. The tree is made of layout primitives (row, column, stack, scroll, spacer, grow and shrink, wrap) and Pane's own shared UI components (rich row, icon and icon tile, image, keycap, tag, badge, button, text input, text area, select, toggle, checkbox, Markdown, and more). Styling uses Pane's theme tokens by default (tone, text style, text level, space) and accepts raw values (hex colours, pixel sizes) when an author needs them, with Pane correcting the contrast of raw colours so text stays legible in light and dark.

Pane owns everything that must react within a frame: text editing and input methods, focus, hover and pressed states, scroll position, list selection and filtering. Pane keeps that state by the stable keys the author gives the tree's nodes, so re-rendering a screen never loses the caret, the selection or what the user is typing. Callbacks such as a button's click are ordinary closures in the author's code; Pane calls them back by id.

On top of the tree sit the standard views authors know from Raycast — List (sections, accessories, empty view, loading state, search-bar dropdown, pagination, a detail pane), Detail (Markdown with metadata), Grid and Form (with many more field types than today) — so the common case needs no layout work at all, and any standard view can contain designed parts. A navigation stack lets a command push one view on another; Escape pops it and the pushing view hears about it. A view can ask to be rendered again after some milliseconds, which covers timers, clocks, progress and polling with no change to the extension runtime. A canvas is a leaf of the tree for what UI components cannot show: today's custom view becomes one, with paths, images, styled text and richer input.

The wire format between guest and host is hidden in the SDKs: a versioned JSON document carried through a small typed WIT envelope, so a new UI component or property needs no WIT change, a component built against an older SDK keeps working, and a node Pane does not know degrades gracefully instead of failing the screen. The "Extension commands like Raycast" specification introduces that envelope and the JSON tree for the List view (its items, actions, icons and accessories); this specification extends the same envelope with layout primitives, UI components, Detail, Grid, Form, navigation and the canvas, and does not redefine what that specification introduced.

Later in this specification, flagged as its own large slice, a view learns to push a re-render the moment its data arrives. It builds on the "Extension commands like Raycast" specification (#120)'s runtime slice, in which a call waiting on the user, a program or the network no longer holds other packages' calls.

## User Stories

### Designing a screen

1. As an extension author, I want to lay out my screen in rows and columns with gaps, padding and alignment, so that I can design it rather than squeeze it into a list.
2. As an extension author, I want to let children grow, shrink and wrap, so that my layout adapts to the launcher's width and to long text.
3. As an extension author, I want a scrolling region inside my screen, so that long content does not push the rest of the screen away.
4. As an extension author, I want to stack elements on top of each other with an alignment, so that I can place a badge over an image or a label over a progress bar.
5. As an extension author, I want to use Pane's own rich row, icon tile, keycap, tag, badge and button, so that my extension looks like part of Pane without copying its styles.
6. As an extension author, I want text with styles (title, body, caption, monospace), weights, colours, truncation and wrapping, so that I can set hierarchy without guessing pixel values.
7. As an extension author, I want styled spans inside one text, including links that call my code, so that I can write "Accept the [terms]" in one sentence.
8. As an extension author, I want to name spacing, tones and text levels as tokens, so that my screen follows Pane's light and dark themes and any later polish automatically.
9. As an extension author who needs an exact brand colour or size, I want to give a hex colour or a pixel size, so that I am never blocked by the token set.
10. As a user, I want text an extension colours itself to stay readable in both light and dark appearance, so that a raw colour never makes a screen illegible.
11. As an extension author, I want to give a colour as a light and dark pair, so that I control both appearances when contrast correction is not what I want.
12. As an extension author, I want to turn contrast correction off for a specific colour, so that a brand swatch shows its exact value.
13. As an extension author, I want icons from Pane's built-in icon set, my package's images, web images and system file icons, with tint, mask and fallback, anywhere in my tree, so that I use one icon model everywhere.
14. As an extension author, I want a tooltip on any node, so that I can explain a compact control.

### Writing it naturally

15. As a Rust author, I want to build my tree with a GPUI-like builder (`column().gap(Space::M).children([...])`), so that it reads like the Rust UI code I already know.
16. As a Rust author, I want event handlers that receive my view's state mutably (`cx.listener(...)`), so that a click updates my state without shared-ownership boilerplate.
17. As a JavaScript or TypeScript author, I want to write JSX (`<Column gap="m">…</Column>`) with full type checking of every UI component's properties, so that mistakes are caught when I build.
18. As a JavaScript or TypeScript author, I want function UI components with state, so that I can write the same patterns I use with React.
19. As an extension author, I want my screen re-rendered whole after each event while Pane keeps what the user is doing, so that I never write diffing or patching code.
20. As an extension author, I want the SDK to hide the wire format, so that I never build JSON by hand or depend on its shape.

### Interaction and host-owned state

21. As a user, I want typing in an extension's text field to be as fast and IME-friendly as in Pane's own fields, so that input never lags behind a busy extension.
22. As an extension author, I want to hear when a field's value is committed, and optionally as the user types (throttled), so that I can validate or search without owning every keystroke.
23. As an extension author, I want to set a field's value from my code, so that "Clear" or "Use suggestion" buttons work, while re-rendering the field's current value never fights the user's typing.
24. As a user, I want hover and pressed feedback to appear instantly, so that an extension's controls feel native.
25. As a user, I want my scroll position, selection, expanded sections and caret to survive when an extension re-renders, so that a refresh never jumps the screen.
26. As an extension author, I want to give nodes stable keys, so that Pane keeps each node's state even when the list reorders.
27. As an extension author, I want to be told in development when two siblings share a key or a stateful node has none, so that I fix it before users see state jump.
28. As a user, I want a click on a button I could see to do what that button said even if the extension re-rendered just before, so that I am not punished for timing.
29. As a keyboard user, I want Tab and Shift+Tab to move through an extension's focusable controls in a sensible order, so that I can operate every screen without a pointer.
30. As an extension author, I want to choose which control has focus first and move focus from my code, so that the screen opens where the user will type.

### Re-rendering by itself

31. As an extension author, I want to ask Pane to render my view again after some milliseconds, so that I can show a clock, a countdown or a progress bar.
32. As an extension author, I want to poll a service on an interval while my view is open, so that a status screen stays current.
33. As a user, I want an extension's timers to stop working while its view is not shown, so that hidden screens do not use my CPU or battery.
34. As a user, I want a screen that was hidden to catch up as soon as I return to it, so that I never see stale numbers.
35. As an extension author, I want to show a loading state and fill the screen when my data arrives, so that the screen opens immediately.
36. As a user, I want the loading indicator to appear only if loading takes longer than a moment, so that fast screens do not flicker.

### Navigation

37. As an extension author, I want to push a new view onto the current one, so that I can go from a list to an item's detail or an edit form.
38. As a user, I want Escape to go back one view, so that I can always retrace my steps.
39. As a user, I want the previous view to reappear instantly when I go back, as I left it, so that going back never waits on the extension.
40. As an extension author, I want to be told when a view I pushed is popped, with an optional result, so that I can refresh my list after an edit.
41. As an extension author, I want to pop from my code (after a successful submit), so that a flow returns where it started.
42. As an extension author, I want to replace the current view instead of pushing, so that a wizard's steps do not pile up on the stack.
43. As a user, I want Backspace in an empty search field, and Escape in an empty one, to go back one level, so that keyboard navigation matches Raycast.
44. As a user, I want "pop to root" to take me straight back to root search from any depth, so that I can abandon a flow at once.
45. As a user, I want each view's title shown, so that I know where I am in the stack.

### Standard views

46. As an extension author, I want a List with sections, so that I can group items under headings.
47. As an extension author, I want list items with an icon, title, subtitle, keywords and accessories (text, relative date, coloured tag, icon, each with a tooltip), so that rows carry the information Raycast rows carry.
48. As a user, I want a command's list filtered by Pane as I type, using the same fuzzy matching as root search, so that every extension's search behaves alike.
49. As an extension author, I want to take over the search text myself, with a throttle, so that I can search a web service instead of filtering locally.
50. As an extension author, I want to control the search text and the selected item from my code, so that I can restore a previous state or select a new item after creating it.
51. As an extension author, I want to hear when the selection changes, so that I can load the selected item's details lazily.
52. As an extension author, I want an empty view with an icon, title, description and its own actions, so that "Nothing found — Create one" is one keystroke away.
53. As an extension author, I want a dropdown beside the search field, so that the user can switch between filters or accounts.
54. As an extension author, I want pagination that asks me for more items as the user nears the end, so that I can show long remote lists without loading everything.
55. As a user, I want a list with a detail pane beside it, so that I can read about the selected item without opening it.
56. As an extension author, I want an item's detail built only when that item is selected, so that I do not compute details for hundreds of rows.
57. As an extension author, I want a Detail view with Markdown and a metadata panel (labels, links, tag lists, separators), so that I can show an article, a record or a README.
58. As an extension author, I want Markdown images from my package, the web or files, sized and tinted, so that documentation renders properly.
59. As an extension author, I want a Grid with 1 to 8 columns, an aspect ratio, a fit and an inset, per section, so that I can show images, colours, emoji or icons.
60. As a keyboard user, I want arrow keys to move by cell and row in a grid, and Ctrl+Up/Down to jump sections, so that grids are as fast as lists.
61. As an extension author, I want a Form with text, password, text area, checkbox, toggle, date and date-time, dropdown with sections and search, tag picker, file and folder picker, description, separator and link fields, so that I can ask for anything Raycast forms can.
62. As an extension author, I want per-field placeholder, info, default value, error and auto-focus, so that forms explain themselves.
63. As an extension author, I want a field to remember its last value, so that repeated forms are prefilled.
64. As an extension author, I want to place form fields inside my own layout, so that a form can be designed and not only stacked.
65. As an extension author, I want to customise a standard row, cell or empty view with my own subtree while Pane still selects, filters and activates it, so that standard views never limit my design.
66. As an extension author, I want items and views to carry the actions of Pane's action panel (from the "Extension commands like Raycast" specification (#120)), so that every view has a primary action, secondary actions and shortcuts.

### Canvas

67. As an extension author, I want a canvas node inside my layout, so that I can draw a chart, a colour wheel or a game board next to ordinary controls.
68. As an extension author, I want to draw paths (lines, curves, arcs), fills, strokes, rounded rectangles, images and styled text on the canvas, so that I am not limited to rectangles.
69. As an extension author, I want hover, wheel, double-click, modifiers and secondary-button input on the canvas, so that I can build real interactive controls.
70. As an extension author, I want the canvas to fill the space the layout gives it and be told its size, so that my drawing adapts to the window.
71. As a screen reader user, I want a canvas to be one named control with a role and a value, so that I know what it is and what it shows.

### Reliability, compatibility and accessibility

72. As an extension author, I want my extension built against an older UI component set to keep working on a newer Pane, so that I am not forced to rebuild for every Pane release.
73. As an extension author using a newer UI component than the user's Pane knows, I want to give a fallback, so that my screen degrades instead of breaking.
74. As a user, I want a screen whose tree is malformed or too large to show the extension's error and keep the last good screen, so that one bad render does not blank the launcher.
75. As a screen reader user, I want every UI component Pane renders for an extension to have the right role, name, value and state, so that designed screens are as accessible as Pane's own.
76. As a user, I want an extension's screen to close as soon as its package is disabled, updated or reloaded, so that no stale code keeps running behind it.
77. As a user, I want one slow extension not to freeze another extension's screen, so that a stuck network call stays that extension's problem (the "Extension commands like Raycast" specification (#120)'s runtime slice, which this specification relies on).
78. As an extension author, I want my view to re-render the moment my data arrives instead of on a timer, so that loading feels immediate (the real-push slice).
79. As a contributor, I want Pane's own screens and extension screens to share the same UI components, so that the design stays one system.

## Implementation Decisions

### Decision provenance

- **Decided by the user, recorded in ADR 0036** ("Extension UI is a tree Pane renders, written with a GPUI-like API", accepted 2026-10-06): the tree, its layout primitives and first-class shared UI components; the GPUI-like Rust builder and JSX; versioned JSON in a typed WIT envelope (`render`, and `handle-event` by callback id) with graceful degradation of unknown nodes; tokens as the default and raw values allowed, with contrast correction; the icon model; host-owned state by stable keys; partially controlled inputs where a value the extension sets wins; `refresh-after-ms`; the navigation stack; the canvas as a leaf; real push as a later slice; and that the command's list, the forms of #20 and the custom view of #21 are rebuilt on the tree (a list becomes the standard List, a form the standard Form, a custom view's shapes a canvas). This spec implements ADR 0036 and does not re-argue it. Where in doubt, the user asked for the more flexible option for authors.
- **Decided by the user, recorded in ADR 0037:** a command declares `"mode"` (`view` or `no-view`); a view command renders its UI on the tree; host functions (close the window, pop to root, HUD, toast, confirm, clipboard, open, launch another command) decide what happens after a command runs.
- **Decided by the user (decision 2):** the launcher keeps its accepted design (ADR 0029) with Raycast's polish (ADR 0035): alpha-based selection and hover without a border, a loading bar shown only after 300 ms, colour-through-alpha text levels. Extension views follow it.
- **Proposed defaults the user delegated ("like Raycast, as flexible as possible"):** the refresh floor is 100 ms and the navigation depth limit is 32; in a form's text area Enter inserts a newline and Ctrl+Enter submits, as Raycast's forms do.
- **Coordination calls (made by the coordinator, not the user):** the "Extension commands like Raycast" specification (#120) introduces ADR 0036's typed WIT envelope and the JSON tree for the List view (items, actions, icons, accessories); this specification extends that envelope and does not redefine it. That specification also owns the runtime slice in which a waiting call does not hold other packages' calls; this specification's real-push slice builds on it.
- **Proposed defaults in this spec** (not individually confirmed by the user): the extension of the WIT envelope, the JSON vocabulary, limits and numbers, the event set, the stale-event rule, how an extension-set value is told apart from an echo, the UI component set versioning details, the form field set beyond Raycast's, and the slice order. They follow the research in `docs/research/raycast-deep-dive.md` (Extension UI bridge) and its prior-art survey (Shopify remote-dom, Figma Widgets, VS Code tree views, Adaptive Cards, Slack and Discord components, Flutter's rfw, Zed's discussions).
- ADR 0036 settles, within ADR 0003, the shared view, event and drawing API that Q13 left open. Breaking the pre-release extension API 0.1 is allowed under ADR 0010.

### Relationship to the other Raycast-parity specifications

- The **"Extension commands like Raycast" specification** owns the action model (several actions per item, the Ctrl+K action panel with sections, submenus, shortcuts and destructive style, Enter primary and Ctrl+Enter secondary), the host functions and command `mode` of ADR 0037, launch record, declared preferences, and the extension's `icon` field. This spec does not redefine them: actions are attached to tree nodes (items, views, the empty view, any focusable node) as an action-panel subtree, and the host maps them into that specification's panel; icons in the tree use that icon model.
- **Order (coordinator's call):** the "Extension commands like Raycast" specification (#120) introduces the typed WIT envelope and the JSON tree for the List view (items, actions, icons, accessories) first; this spec extends the same envelope with layout primitives, UI components, Detail, Grid, Form, navigation and the canvas. It also owns the runtime slice in which a waiting call does not hold other packages' calls, on which this spec's real-push slice builds.
- The **"Launcher polish" specification** owns the keyboard extras (Backspace on an empty query goes back, Alt+Up/Down by five, Ctrl+Up/Down by section, Alt-based Emacs and Vim navigation); extension Lists and Grids get them by being host-rendered.
- The **"Root search like Raycast" specification** owns the fuzzy matcher; host filtering of extension Lists uses the same matcher and sensitivity.

### The contract: a typed envelope carrying a versioned JSON tree

- The envelope is the one the "Extension commands like Raycast" specification (#120) introduces for the List view; this spec extends it with what navigation and the further views need. A view is a guest **resource**, as today's custom view is, so its state lives in the extension and Pane drops it when the view leaves the stack. Its shape once extended (proposed; the decision-rich parts):

  ```wit
  resource view {
    render: async func(context: string) -> result<rendered, string>;
    handle-event: async func(event: ui-event) -> result<outcome, string>;
  }
  record rendered { tree: string, refresh-after-ms: option<u32> }
  record ui-event { render: u64, key: string, callback: u32, payload: string }
  record outcome { push: option<view>, replace: option<view>, pop: option<string> }
  open-view: async func(command: string, launch: string) -> result<view, string>;
  ```

- `tree` is a JSON document: a UI component set version and a root node. A node has a type, an optional key, properties and children. Callbacks appear in the document as numeric ids. `context` is JSON too (the render sequence number, the appearance in effect, the selected item's key in a List or Grid, the space available to a canvas, the UI component set version Pane supports), so the context can grow without WIT changes. `launch` is the launch record of the "Extension commands like Raycast" specification (#120), as JSON.
- After `handle-event`, Pane calls `render` and shows the result. An `Err` from either is the extension's error, shown as today ("The extension reported an error: …"); the view keeps its last good tree and stays open. A trap is a crash and closes the view, as for every call.
- The host parses the JSON into a strict host-side node type with explicit limits. The envelope's WIT shape is the only part an extension's component is type-checked against, so adding a UI component, a property or a context field never makes Pane refuse a built component.
- Today's three UI shapes (the typed `view` list with `item`, the `form` record and the `frame` custom view) are rebuilt on the tree, as ADR 0036 decides: the List (moved to the tree by the "Extension commands like Raycast" specification (#120)), Form and canvas replace them and there is one way to describe extension UI. The samples, fixtures and default extensions move to the tree in the same change; the refusal of components built for an older API shape stays and names the change.

### The tree: layout primitives

- `row`, `column`: flex direction with `gap`, `padding` (per side), `align` (start, centre, end, stretch, baseline), `justify` (start, centre, end, space-between, space-around), `wrap`.
- `stack`: children drawn over each other, each aligned (nine positions) with an optional offset; absolute coordinates beyond that are the canvas's job.
- `scroll`: vertical or horizontal scrolling region whose position Pane keeps by key.
- `spacer` and `divider`.
- Sizing on any node: `grow`, `shrink`, `basis`, `width`/`height` and `min`/`max` in space tokens, pixels or a fraction of the parent, and `aspect-ratio`.
- Surface on any node: `background` (tone or raw colour), `border` (width, tone or raw colour), `radius` (token or pixels), `opacity`, and declarative `hover` and `pressed` variants of those properties that Pane applies without calling the extension.
- Layout is computed by GPUI's flex layout; nothing is laid out by the extension.

### Shared UI components

- First-class UI components are Pane's own, extracted where needed into data-driven form and used by Pane's own screens too: text (with styled spans and link spans), icon, icon tile, image, rich row, keycap and key sequence, tag, badge, button (default, secondary, ghost, accent, destructive; with icon and keycap), text input, password input, text area, select (Pane's searchable select), toggle, checkbox, segmented control, slider, progress bar, loading indicator, Markdown, card, section header, metadata list, empty state and link.
- A standalone tag and badge are extracted from the row's alias chip; Markdown is new (a CommonMark parser with GitHub tables and task lists; LaTeX is out of scope); image and icon nodes need an image path from bytes and from SVG that the row and tile do not have today.
- Each UI component has one accessibility mapping (role, name, value, state) owned by the host.

### Tokens and raw values

- The public token set is a stable mapping layer over Pane's private theme, so the theme can change (as ADR 0035 changes it) without renaming tokens, and tokens follow the user's appearance and background image (ADR 0028's frost) as Pane's own screens do: **tone** (neutral, accent, success, warning, danger, and the seven palette colours red, orange, yellow, green, blue, purple, magenta that Raycast authors know from tags), **text style** (heading, title, body, caption, mono and small mono), **text level** (primary, secondary, tertiary, quaternary, through alpha of the text colour as ADR 0035 describes), **space** (xs, s, m, l, xl, xxl), **radius** (s, m, l, full) and **icon size** (s, m, l, xl).
- Raw values are accepted everywhere a token is: colours as `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `rgb()`/`rgba()` and `hsl()`/`hsla()`, or a `{ light, dark }` pair of any of these; lengths as pixels. Raycast accepts the same colour forms (hex, rgb, hsl, CSS keywords; CSS keywords are not proposed for Pane).
- **Contrast correction:** a raw or palette colour used for text or an icon is corrected against the surface it is drawn on, by moving its lightness until it reaches a minimum contrast ratio (Raycast uses 2.5 for this; Pane starts there, tunable), unless the author turns correction off for that colour. Raw backgrounds are not corrected; foregrounds drawn on them are corrected against them.
- Fonts other than Pane's UI and mono families are out of scope; sizes and weights are free.

### Icons and images

- The icon and image nodes use the icon model of ADR 0036 (shared with the "Extension commands like Raycast" specification (#120)'s row icons): the full vendored reicon set by name, packaged PNG/SVG with automatic `@dark`/`@light` variants and light/dark source pairs, web images by URL that the host downloads and caches through the existing bounded HTTP limits with a fallback while loading and on failure, system file and application icons by path, and inline image data (bounded); each with tint (token, pair or raw, contrast-corrected), mask (circle, rounded rectangle) and fallback.
- Image nodes add size, fit (contain, cover, fill) and a loading placeholder.

### Keys, reconciliation and host-owned state

- Every stateful or interactive node (inputs, selects, toggles, scroll regions, lists and their items, grid cells, canvases, anything with a callback) takes a key unique among its siblings; the host composes path keys. Static nodes may omit keys and are matched by position.
- The host reconciles each new tree against the previous one by path key and keeps per key: text, caret, selection and IME composition (GPUI CE's editable text state), select and dropdown state, focus, hover, pressed, scroll position, list and grid selection, expanded or collapsed sections, and stable accessibility ids. A node whose key disappears loses its state; a node of a different type under the same key is new.
- In development mode, duplicate sibling keys and stateful nodes without keys are reported in the extension's log; Pane then falls back to positional matching for those nodes rather than failing the screen.
- Generalises the form's existing reconciliation of text fields and focus handles.

### Events and callbacks

- Callbacks are closures in the author's code. The SDK assigns each a numeric id per render and keeps the table of the latest render and the one before it; the host sends the render sequence, the node key and the id with each event. The event set: press (buttons, rows, cells, links), change (committed value), input (as typed, only if the node asks for it, coalesced and throttled), focus and blur, selection change, search-text change, load-more, dropdown change, submit, pop of a pushed view, key (on focusable nodes that ask for keys), and the canvas's pointer, wheel and key events.
- **Stale events:** an event raised on a node the user could see is delivered even if the extension has rendered since, provided the node with that key still has a handler for it ("the user clicked what they saw"); otherwise it is dropped and, in development, logged.
- Events of one view are delivered one at a time in order, numbered as custom-view events are today, so a late older answer never replaces a newer tree; input events and pointer moves are coalesced to the latest while one is in flight.
- An answer that arrives after its view left the screen is discarded; a view opened after the user left is dropped at once (today's custom-view rules, kept).

### Inputs are partially controlled

- The host owns editing: typing, IME composition, selection, undo and clipboard never wait for the extension. Pane edits the text at once and tells the extension: `input` as the user types (coalesced to the latest while one is in flight, and throttled if the node asks) and `change` when a value is committed (blur, Enter, choice made).
- A value the extension sets wins (ADR 0036). So that echoing the field's own value back never fights fast typing, a node's value counts as set only when it differs from the value that node had in the extension's previous render; an unchanged value is no instruction, and the field keeps the user's newer text. A new value replaces the text and moves the caret to its end.

### Navigation stack

- The host owns the stack of views per opened command. `outcome.push` pushes a new view resource; `replace` swaps the top; `pop` pops the top with an optional result string. The SDKs expose `push`, `replace` and `pop` (and the Raycast-style `Action.Push`) and an `onPop` handler on the pushing side.
- Escape pops at once, showing the previous view's last tree immediately, then delivers a pop event (with the result) to it, which re-renders it. A popped view's resource is dropped.
- Escape first clears a non-empty search field, closes an open dropdown or menu, or cancels IME composition, in that order, before popping (Raycast's order). Backspace on an empty search field pops, but not on key repeat (the "Launcher polish" specification (#123)). Pop to root (host function of the "Extension commands like Raycast" specification (#120), Shift+Esc) drops the whole stack.
- A depth limit of 32 (proposed default, delegated) refuses a further push as the extension's error.
- Each view's title is its tree's `navigation-title`, shown in the search header as today's screen title is.

### Re-rendering after some milliseconds (no runtime change)

- `refresh-after-ms` on a render answer asks Pane to call `render` again after that time. Pane schedules it through the view's event numbering and wakes the window through the existing change channel the continuing services use.
- Refreshes run only while the view is the top of the stack and the launcher is shown; a refresh that fell due while hidden runs as soon as the view is shown again. Leaving the view cancels it.
- The floor is 100 ms (proposed default, delegated) and the ceiling 24 hours (proposed), clamped. Each refresh is an ordinary guest call, so it holds other extensions' calls while it computes, exactly as any call does; once the "Extension commands like Raycast" specification (#120)'s runtime slice lands, a refresh that waits on the network no longer holds them.
- The SDKs build their asynchronous helpers on it until the real-push slice lands: a view whose data is pending renders its loading state at once and asks for a prompt refresh, in which the SDK awaits the pending work (bounded) and renders the result. The JS/TS SDK's state hooks and the Rust builder's async helpers present this as ordinary loading state.

### Standard views built on the tree

The standard views are UI components of the same tree whose behaviour the host owns. Any of their parts (a row, a cell, the empty view, the detail) may be replaced by an author's subtree while the host still selects, filters and activates it.

- **List:** builds on the List document the "Extension commands like Raycast" specification (#120) introduces (items with key, icon, title, subtitle, accessories — text, relative date, coloured tag, icon, each with a tooltip — and actions) and adds: sections with titles and subtitles; item keywords; host fuzzy filtering on title, subtitle and keywords with the root-search matcher unless the view handles `search-text change` itself (then with a throttle, default 250 ms as Raycast's); controlled `search-text` and `selected-key`; a selection-change event; `is-loading` (the 300 ms loading bar); `search-placeholder`; a search-bar dropdown (sections, items, its own search); pagination (`has-more`, `page-size`, load-more raised as the selection nears the end); an empty view (icon, title, description, actions; Raycast's default is "No Results"); and a detail pane (`is-showing-detail`) whose content is built only for the selected item, using the selected key in the render context. Long lists are virtualised.
- **Detail:** Markdown with images (sized, tinted, from package, web or file) and a metadata panel (label, link, tag list, separator), loading state and actions.
- **Grid:** List's behaviour with cells of image, colour, icon or subtree content, 1–8 columns (default 5), aspect ratio, fit and inset, per section; cell title and subtitle; keyboard movement by cell and row, Ctrl+Up/Down by section, no wrap, as Raycast's grid.
- **Form:** fields text, password, text area, checkbox, toggle, date and date-time picker, dropdown (sections, search, extension-handled search), tag picker (multi-select), file and folder picker (system dialog, one or many), description, separator and link; per field `title`, `placeholder`, `info`, `default`, `error`, `auto-focus` and `remember` (the last submitted value is kept as the package's settings and prefilled next time); fields may sit inside the author's own layout. Submission is an action (the "Extension commands like Raycast" specification (#120)), with validation errors set by the extension's next render; the form helpers in the utilities library (the "Making extensions easier to build" specification (#128)) add validate-on-blur and focus-first-invalid. Raycast's Enter never submits a form (submission is Ctrl+Enter); Pane's forms submit on Enter today. In a text area, Enter inserts a newline and Ctrl+Enter submits, running the form's primary action instead of the secondary one (Raycast's form behaviour; proposed default, delegated). Proposed: Enter in a single-line field runs the form's primary action (submit), as today.
- These replace today's `search: true` command search and its export: a command's search is the List's search-text event.

### Canvas

- `canvas` is a leaf with a key, a size (fixed, or filling the space layout gives it, reported in the render context with a resize event) and drawing operations: paths (move, line, quadratic and cubic curves, arcs, close) with fill and stroke (width, caps, joins), rectangles and rounded rectangles, circles, images (any image source of the icon model), text runs in token styles or raw sizes, clip and transform, all colours as tokens, pairs or raw values.
- Text measurement is a host import the canvas code may call while rendering, so text can be laid out exactly.
- Input: keys (all non-reserved keys with modifiers; Tab, Enter and Escape stay with Pane as today), pointer down, move, up, hover enter and leave, wheel, double-click, secondary button, with modifiers; moves coalesced as today.
- Accessibility: one node with a role from a widened set (colour well, slider, image, figure, group, generic), a label, a value, and optional increment, decrement and activate actions.
- Rendered on GPUI's canvas and path builder, not one element per shape. Today's colour-picker sample becomes a canvas inside a layout.

### Authoring APIs

- The Rust SDK adds a GPUI-like builder (shape below; trimmed to the decision-rich parts). A view is a type implementing `render`; handlers get the view's state mutably through a context listener; the builder emits the JSON document.

  ```rust
  impl View for Timer {
      fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoNode {
          column().gap(Space::M).children([
              text(format!("{}s", self.elapsed)).style(TextStyle::Title),
              row().gap(Space::S).children([
                  button("Pause").on_click(cx.listener(|this, _, _| this.paused = true)),
                  button("Reset").tone(Tone::Danger).on_click(cx.listener(|this, _, _| this.elapsed = 0)),
              ]),
          ])
          .refresh_after(Duration::from_secs(1))
      }
  }
  ```

- The JS/TS SDK adds a JSX runtime (`jsxImportSource` set to the SDK; the build already bundles and type-checks) producing plain objects, function UI components with a small hook set (state, ref, memo, and interval/refresh), typed properties for every UI component, and the per-render callback table. React and a React reconciler are not used (their scheduler needs timers that run only inside calls).

  ```tsx
  function Timer() {
    const [elapsed, setElapsed] = useState(0);
    useInterval(1000, () => setElapsed((s) => s + 1));
    return (
      <Column gap="m">
        <Text style="title">{elapsed}s</Text>
        <Row gap="s">
          <Button onClick={() => pause()}>Pause</Button>
          <Button tone="danger" onClick={() => setElapsed(0)}>Reset</Button>
        </Row>
      </Column>
    );
  }
  ```

- Both SDKs expose the standard views as UI components (`List`, `List.Section`, `List.Item`, `Detail`, `Grid`, `Form`, `Form.TextField`… in JSX; equivalent builders in Rust), so Raycast authors recognise them.

### Versioning and graceful degradation

- The UI component set has its own `MAJOR.MINOR` version, written into each document by the SDK. Additive changes (a UI component, a property, an event) bump the minor version; Pane renders a document of its major and any minor, ignoring unknown properties and treating unknown UI components by their declared fallback.
- Any node may carry a `fallback` subtree (shown when Pane does not know the node) and a `requires` minimum minor version; an unknown node without a fallback renders its children if it has any, otherwise nothing; in development it is logged.
- A document of another major version is refused as the extension's error, naming the versions. `pane.json` may declare the minimum UI component set version a package needs, so installing it on an older Pane explains that a newer Pane is needed.
- Pane's own SDK packages each target one UI component set version; the extension API version (`apiVersion`) changes only when the envelope does.

### Limits (proposed, provisional)

- Per document: at most 10,000 nodes, 64 levels deep, 4 MiB of JSON, 64 KiB of text per text node, 1 MiB of Markdown, 1 MiB of inline image data per image; per canvas, 20,000 drawing operations. A document over a limit is the extension's error ("The extension reported an error: the view has 10,001 nodes; at most 10,000 are drawn") and the last good tree stays.
- Raw lengths are clamped to 0–4096 px, raw opacity to 0–1, list page sizes and grid columns to their ranges.
- The parse and reconcile cost of a 500- and a 5,000-node tree is measured and recorded with the change (not a gate), as the research asked when the wire format was still open.

### Real push (a later slice, size L, flagged)

- Today the runtime serves one call at a time across all extensions, a guest runs only inside a host call, and Wasmtime 49 cannot cancel a guest task without dropping its instance. The "Extension commands like Raycast" specification owns the runtime slice that lets a call waiting on the user, a program or the network stop holding other packages' calls (coordinator's call), with compute time still attributed per instance (the unresponsive-call limit unchanged) and generations still stopping their instances. This slice builds on that one and does not rebuild it; if that slice has not landed, this one waits for it.
- A view then gets a push channel (a host import to request a render, or a WASI 0.3 stream or future of "dirty" signals that the host drops when the view closes), which needs an instance to run between Pane's calls; push answers share the view's event numbering, so a late push cannot overwrite a newer answer; Pane coalesces requests to at most one render per frame.
- The glossary's descriptions of the extension runtime and the unresponsive call change with the runtime slice; push adds what a view may do by itself.
- It is flagged because it is the largest and riskiest change here and the rest of the spec does not depend on it.

### Slices (proposed order)

1. On the envelope and List document the "Extension commands like Raycast" specification (#120) introduces: the extended view resource, the JSON node type with limits, `column`/`row`/`text`/`button` with tokens and callback events, rendering and reconciliation basics; samples in Rust, JS and TS.
2. `refresh-after-ms`.
3. Shared UI components, icons and images, raw values and contrast correction.
4. Keyed reconciler for inputs, selects, focus stops and scroll; partially controlled inputs.
5. JSX runtime and Rust builder (may begin with slice 1 and grow with each slice).
6. Navigation stack.
7. List's additions (sections, empty view, dropdown, pagination, detail pane), Detail with Markdown, Grid, Form; retirement of the typed form and custom-view shapes and migration of samples, fixtures and default extensions.
8. Canvas.
9. Real push (flagged, L), on the "Extension commands like Raycast" specification (#120)'s runtime slice.

## Testing Decisions

- A good test drives what an author or user can observe: the tree an extension's real component renders, as Pane's launcher holds it, and what the window draws and announces in response to real keys and pointer events. Tests do not assert the JSON wire format, the reconciler's internal tables or GPUI element structure.
- **Primary seam: pane-core's `Launcher`**, driven by real sample extensions in Rust, JavaScript and TypeScript, as the existing samples and launcher tests do for forms and custom views. Covered there: opening a view and its first tree; callback events and the re-rendered tree; stale events (delivered while the key exists, dropped after it goes); event ordering and coalescing; errors keeping the last tree; limits; unknown nodes with and without fallbacks; a newer minor and a different major version; push, replace, pop with a result, Escape order and depth bound; refresh-after-ms with a controlled clock (the launcher's clock seam), paused while hidden or not on top and caught up on return; pagination load-more; host filtering against the root-search matcher; List selection and lazy detail through the render context; Form values, remembered values and validation; generation ending (disable, reload, update) closing the stack. The three languages must render the same trees and give the same answers, as the samples are held to today.
- **Window seam: the window tests** in the pane crate, with real key and mouse events through GPUI's test platform, for what is drawn: layout and tokens resolving to the theme in light and dark; contrast-corrected colours; keyed state surviving re-renders (caret, IME composition, selection, scroll, focus, hover); Tab order; Escape and Backspace navigation; List, Grid and Form keyboard behaviour; the loading bar's 300 ms threshold; and the accessibility tree (roles, names, values, states) for every UI component, as the form and custom-view checks do today. Input-method composition is proven as the form tests prove it today, with the same stated limits.
- **Native smokes:** extend the existing GUI smoke scripts to open the Rust sample's designed view, a List with detail and a canvas, as the colour picker is driven today; only the Rust sample is driven natively, the JS and TS views resting on the contract and window tests.
- The real-push slice is tested at the `Launcher` seam: push renders are tested for ordering against event answers, and a view that pushes while another extension waits on a slow fixture service is drawn without waiting for it (that a waiting call holds no other call is the "Extension commands like Raycast" specification (#120)'s test); the existing unresponsive, pausing and runtime-crash suites must pass unchanged.
- Prior art: the forms and custom-views contract and host tests, the samples suite held across Rust, JS and TS, the window tests for forms and custom views, the command-search tests with the fixture service, and the launcher-clock seam used by schedules, services and clipboard expiry.

## Out of Scope

- Webviews, HTML, CSS or any embedded browser; direct access to GPUI objects from extensions; shaders or GPU access.
- Extension-defined animation and motion; Pane's own motion policy applies to extension views as to its screens.
- Fonts other than Pane's UI and mono families; CSS colour keywords.
- Menu-bar commands (Raycast's `menu-bar` mode; it does not exist on Windows).
- Form drafts offered again from root search after leaving an unsubmitted form (Raycast keeps them); a later slice once root search has a place for them.
- LaTeX in Markdown; Quick Look previews.
- A mutation (patch) protocol instead of whole-tree renders; it can be added beside `render` if measured payloads demand it.
- The action panel, host functions, command modes, preferences, arguments and the icon model themselves (the "Extension commands like Raycast" specification (#120)); keyboard extras (the "Launcher polish" specification (#123)); the fuzzy matcher (the "Root search like Raycast" specification (#122)).
- Compatibility with Raycast's API or React components as such; the shapes are familiar, not identical.

## Further Notes

- Research: `docs/research/raycast-deep-dive.md` (Extension UI bridge, the corpus census and the Raycast facts cited here), and the earlier research notes on the GPUI extension bridge and Raycast's extension UI. Raycast serialises the whole tree as JSON on each commit (compressed above 30 KB), passes callbacks as string ids and serialises only the selected item's detail and actions; Pane follows the whole-tree model with host diffing by key.
- Prior art that shaped specific rules: Shopify's "partially controlled" fields (the host owns in-progress input; the guest gets commits), Figma Widgets' declarative hover styles, VS Code's warning that label-derived ids lose state (hence required keys), Adaptive Cards' `fallback`/`requires` (hence graceful degradation), Flutter rfw's warning against moving a whole application into the remote model (hence a small canvas escape hatch rather than a second UI framework).
- Pane is already ahead of Raycast on custom-drawn views; this spec keeps that lead (the canvas) while closing the standard-view gap.
- The glossary needs terms for the tree, the UI component set, the navigation stack and the canvas, and revised entries for Custom view (whose "avoid: canvas" changes, since a custom view becomes a canvas node) and Form; they are recorded through domain modelling with the first slice.
- Settled since the first draft: the order between this spec and the "Extension commands like Raycast" specification (#120) (that specification introduces the envelope and the List's tree and owns the runtime slice; coordinator's call); the refresh floor (100 ms) and the navigation depth limit (32), proposed defaults the user delegated; and a form's text area (Enter inserts a newline, Ctrl+Enter submits, like Raycast; proposed default, delegated).
- Open questions, not resolved here: whether `remember` values are the package's settings or a Pane-owned record; the document limits other than the refresh floor and depth limit.


---

Decisions this specification relies on are recorded in [ADRs 0030–0040](https://github.com/hoangvu12/pane/tree/main/docs/adr) (added by [#119](https://github.com/hoangvu12/pane/pull/119)); the evidence is [docs/research/raycast-deep-dive.md](https://github.com/hoangvu12/pane/blob/main/docs/research/raycast-deep-dive.md).

