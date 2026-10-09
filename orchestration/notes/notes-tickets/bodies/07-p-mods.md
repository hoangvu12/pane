## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

Every launcher key and chord fires only when Shift, Ctrl, Alt and the Windows or Command key are each held exactly as it declares — the Keyboard page's bindings, the Ctrl+digit chords, the pin keys, action shortcuts and Enter variants — so a chord with an extra modifier never triggers the plainer one: Ctrl+Shift+K does not open the Actions panel, Shift+Enter does not invoke the row, Ctrl+Alt+digit picks nothing. A character the current layout produces with AltGr (reported as Ctrl and Alt on Windows) is typed into the focused field and never matches a Ctrl+Alt chord. Ctrl+C (Command+C on macOS) with a non-empty selection in a focused field — the query, an argument, command search, a form field — copies the selected text, even when the selected row has an action bound to Ctrl+C; with no selection, the row's action runs. Ctrl+Shift+C is unaffected. The numpad's digits act as the digit row's for Ctrl+digit chords. The number hints, which already show after Ctrl is held alone for 400 ms, are guarded by tests: shown after 400 ms of Ctrl alone, never on a chord, hidden by any other key, a key release, a scroll, focus leaving or the window deactivating.

## Acceptance criteria

- [ ] Ctrl+Shift+K, Shift+Enter and Ctrl+Alt+digit do nothing they do not declare, across the launcher's chords
- [ ] An AltGr character typed into the query reaches the field
- [ ] Ctrl+C with a selection copies the text (read through the window's clipboard); without one it runs the selected row's copy action; Ctrl+Shift+C is unaffected
- [ ] Numpad digits pick rows as Ctrl+digit, and the number hints show after 400 ms of Ctrl alone and never on a chord
- [ ] Keyboard tests press real keys and check what ran, with no assertion on token values or private state
