=== #236 Re-render a view after some milliseconds, with interval and loading helpers in both SDKs [OPEN] assignees=
## Parent

https://github.com/pane-app/pane/issues/121

## What to build

Slice 2 of the parent. Size M.

- Implement `refresh-after-ms` on a render answer: Pane calls `render` again after that delay, clamped to the 100 ms floor and the 24 h ceiling, through the view's event numbering. It wakes the window through the change channel the services use.
- Refreshes run only while the view is on top and the launcher is shown. One that fell due while hidden runs as soon as the view is shown again, and leaving the view cancels it.
- **JS/TS:** `useInterval` and a refresh hook.
- **Rust:** `.refresh_after(Duration)`.
- Both SDKs present pending data as ordinary loading state: render the loading state at once, ask for a prompt refresh, and in it await the pending work (bounded) and render the result.
- A timer sample in all three languages.

## Acceptance criteria

- [ ] With the launcher's controlled clock: refreshes at the asked time, the clamping, pausing while hidden or not on top, and catching up on return (`Launcher` seam).
- [ ] A view with pending data shows its loading state first, then its data.
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch.

## Blocked by

https://github.com/pane-app/pane/issues/235

