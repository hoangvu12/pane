=== #243 Let a view push a re-render the moment its data arrives (flagged) [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 9 of the parent, flagged: the largest and riskiest change, which nothing else depends on. Size L. It builds on #120's runtime slice (#136, done).

- A view gets a push channel, either a host import that asks for a render or a WASI 0.3 stream of dirty signals that Pane drops when the view closes. The instance therefore runs between Pane's calls.
- Push answers share the view's event numbering, so a late push never overwrites a newer answer. Pane coalesces requests to at most one render per frame.
- The SDKs' loading helpers (from the refresh ticket) switch from prompt refreshes to push.
- Update the glossary's entries for the extension runtime.

## Acceptance criteria

- [ ] `Launcher` seam: push renders are ordered against event answers, and a view that pushes while another extension waits on a slow fixture service is drawn without waiting for it.
- [ ] The unresponsive, pausing and runtime-crash suites pass unchanged.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/236

