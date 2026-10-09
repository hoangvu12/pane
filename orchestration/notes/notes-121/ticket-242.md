=== #242 Draw on a canvas node, and move the colour picker and every custom view onto it [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 8 of the parent. Size L.

- `canvas`: a keyed leaf, fixed or filling its space (reported in the render context, with a resize event). Its drawing operations are paths with fill and stroke, rectangles, rounded rectangles, circles, images from the icon model, text runs, clip and transform, with colours as tokens, pairs or raw values. Limit: 20,000 operations.
- A host import measures text while rendering.
- Input: keys with modifiers (Tab, Enter and Escape stay with Pane), pointer down, move, up, enter and leave, wheel, double-click and secondary button. Moves are coalesced.
- Accessibility: one node with a role from the widened set, a label, a value, and increment, decrement and activate actions.
- It is drawn with GPUI's canvas and path builder.
- Retire the `frame` custom view from the WIT. Move the colour-picker samples and fixtures onto a canvas inside a layout.
- A revised glossary entry for Custom view and Canvas.

## Acceptance criteria

- [ ] `Launcher` seam: drawing operations and canvas events in Rust, JS and TS samples.
- [ ] Window tests: pointer and key input, resize, and the canvas's accessibility.
- [ ] The native smokes open the Rust sample's designed view, a List with detail and a canvas.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/237

