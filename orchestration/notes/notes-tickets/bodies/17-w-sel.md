## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features)

## What to build

The selected-text host function, already declared with the front application, implemented on Windows. First **UI Automation**, in a worker process of Pane's own — Pane's program started with an internal argument, owned like a helper and ended when Pane quits — asked with a timeout (about 2.5 seconds) and replaced when it times out; three failures within ten minutes pause UI Automation reads for ten minutes. The worker reads the focused element's text selection, waking a Chromium-based application's accessibility tree on first contact and retrying shortly after, so it works in browsers and editors built on Chromium as well as in native Windows controls.

If UI Automation gives nothing, a **simulated copy**: Pane saves the clipboard, sends a tagged Ctrl+C to the target, waits for the clipboard's sequence number to change (up to about 500 ms), reads the text and restores the clipboard; the restore is tagged, and Pane's clipboard history ignores clipboard changes during that window (Windows' own history may keep the target's copy, since Pane cannot mark another application's copy — documented). Reading is quick and never freezes Pane even when the application in front is slow. No selection answers "nothing is selected", distinct from a failure, so a command can explain rather than receive an empty string silently.

## Acceptance criteria

- [ ] Selected text is read through UI Automation from a test-owned edit control, and through the simulated copy from a test window without a text pattern
- [ ] Chromium-based accessibility trees are woken on first contact and retried shortly after (fake target in tests)
- [ ] The clipboard is unchanged after a simulated-copy read, and Pane's clipboard history ignores the window's changes
- [ ] "Nothing is selected" is answered distinctly from a failure
- [ ] A slow target never freezes Pane: the worker times out and is replaced; repeated failures pause UI Automation reads
- [ ] Samples in Rust, JavaScript and TypeScript exercise the function against a fake system; opt-in real-input adapter tests cover both paths

## Blocked by

- #253 — Track the front application and paste into it on Windows
