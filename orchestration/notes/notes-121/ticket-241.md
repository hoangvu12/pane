=== #241 Rebuild Form on the tree with the full field set, and move every sample and extension off the typed form [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

The Form part of slice 7 of the parent. Size L.

- **Fields:**
  - text, password, text area, checkbox and toggle
  - date and date-time picker
  - dropdown (sections, search, extension-handled search)
  - tag picker
  - file and folder picker (system dialog)
  - description, separator and link

  Each takes `title`, `placeholder`, `info`, `default`, `error`, `auto-focus` and `remember`, and fields may sit inside the author's own layout.
- Submission is an action, and validation errors come from the next render. In a text area, Enter inserts a newline and Ctrl+Enter submits.
- `Form` and `Form.*` as JSX components and as Rust builders.
- Retire the typed `form` record from the WIT. Move the samples, fixtures and default extensions (Quicklinks' Create Quicklink among them) to the tree in the same change. The refusal of components built for the older shape names the change.
- A revised glossary entry for Form.

## Acceptance criteria

- [ ] `Launcher` seam: every field type, validation errors and `remember`, in Rust, JS and TS samples.
- [ ] Window tests: Form keyboard behaviour, pickers and accessibility.
- [ ] No sample, fixture or default extension uses the typed form any more.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/238
https://github.com/pane-app/pane/issues/239

