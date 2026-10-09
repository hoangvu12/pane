=== #240 Build List's additions, Detail with Markdown and Grid on the tree [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

The List, Detail and Grid part of slice 7 of the parent. Size L.

- **List** (on #135's List document):
  - sections
  - keywords
  - host fuzzy filtering with the root-search matcher, unless the view handles the search-text event (throttled, 250 ms by default)
  - controlled `search-text` and `selected-key`, and a selection-change event
  - `is-loading` with the 300 ms loading bar, and `search-placeholder`
  - a search-bar dropdown
  - pagination (`has-more`, `page-size`, load-more near the end)
  - an empty view
  - a detail pane

  Any row, the empty view or the detail may be the author's own subtree.
- **Detail:** Markdown with images and a metadata panel, a loading state and actions.
- **Grid:** List's behaviour with cells of image, colour, icon or a subtree; 1–8 columns per section; aspect ratio, fit and inset; and Raycast's keyboard movement.
- `List`, `List.Section`, `List.Item`, `Detail` and `Grid` as JSX components and as Rust builders.
- These replace `search: true` command search and its export.

## Acceptance criteria

- [ ] `Launcher` seam: filtering, controlled search and selection, pagination, the empty view and the detail pane, in Rust, JS and TS.
- [ ] Window tests: List and Grid keyboard behaviour, the loading bar's 300 ms threshold, and Markdown and metadata rendering with accessibility.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/237
https://github.com/pane-app/pane/issues/238
https://github.com/pane-app/pane/issues/239

