## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

The footer's outcome messages — Pane's own results and failures, today's status-line Result and Error texts — become toasts drawn through the toast controls the launcher already has (a 3-second self-dismiss that pauses while the toast is hovered or has the focus, a pending toast that stays until its work ends, a new toast replacing the current one). What this ticket adds: a **close button** that appears on hover or focus and dismisses the toast at once; **Escape** dismissing it while it has the focus; and **long text** — a toast whose text does not fit on one line shows its first line, truncated, with a details affordance, and Ctrl+T (Command+T on macOS) or a click opens the full text, wrapped and scrollable, in a popover above the footer, which also lists the toast's actions. Ctrl+T opens the details popover (this supersedes today's Ctrl+T-moves-focus-to-the-action binding; the actions remain reachable by keys inside the popover). Each toast is announced once when it appears, as statuses are today; its close button and details are focusable and named. The core keeps the status; the window owns the timing and clears an expired toast through the launcher, so a later screen never shows a stale one.

## Acceptance criteria

- [ ] A result or failure toast is dismissed 3 seconds after it appears unless the pointer rests on it or it has the focus, then restarts the full time
- [ ] The close button and Escape dismiss it at once
- [ ] A long toast shows its first line truncated and opens in full in the details popover, by Ctrl+T and by click, with its actions reachable by keys
- [ ] A new toast replaces the current one; a pending toast stays until its work ends or is updated
- [ ] Each toast is announced once when it appears
- [ ] Window tests: dismissal and pause/restart, replacement, a pending one kept, long text opened with Ctrl+T, closed by button and by Escape, announced once
