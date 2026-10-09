=== #237 Give extension views the full layout primitives, Pane's shared UI components, tokens and raw values, icons and images [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 3 of the parent. Size L.

- **Layout:** `stack`, `scroll`, `spacer` and `divider`. Sizing on any node (grow, shrink, basis, width and height with min and max, as tokens, pixels or fractions, and aspect ratio). A surface on any node (background, border, radius, opacity), with declarative `hover` and `pressed` variants that Pane applies without calling the extension.
- **Shared UI components:** Pane's own, extracted into data-driven form where needed and used by Pane's own screens too: text with styled and link spans, icon, icon tile, image, rich row, keycap and key sequence, tag, badge, button variants with icon and keycap, toggle, checkbox, segmented control, slider, progress bar, loading indicator, Markdown, card, section header, metadata list, empty state and link. Text input, password input, text area and select are drawn here; keeping their editing state across renders is the next ticket's job.
- Each UI component has one accessibility mapping.
- **Tokens:** the full public token set (tone with the seven palette colours, text style, text level, space, radius, icon size) over Pane's private theme.
- **Raw values:** accepted wherever a token is (hex, `rgb()`, `hsl()`, `{ light, dark }` pairs, pixels), clamped. Text and icon colours are contrast-corrected against their surface (ratio 2.5), and an author can turn that off per colour.
- **Icons and images:** the icon model of ADR 0036 (reicon names, packaged files with `@dark` and `@light`, web images through the existing cache, system icons, bounded inline data), with tint, mask, fallback, size, fit and a loading placeholder.
- Typed properties for every new UI component in the JSX runtime and the Rust builder.
- The sample grows to use them.

## Acceptance criteria

- [ ] Window tests: layout, tokens and raw values in light and dark, contrast correction, and hover and pressed variants without an extension call.
- [ ] Every UI component's accessibility role, name, value and state.
- [ ] Parse and reconcile cost of a 500- and a 5,000-node tree, measured and recorded (not a gate).
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/235

