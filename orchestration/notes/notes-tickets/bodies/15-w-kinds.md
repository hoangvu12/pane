## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

A binding can be a **lone tap** of one modifier, a **double tap** of one modifier (two presses of the same modifier with nothing between, within about 400 ms; the first press passes through to applications), a chord with **side-specific modifiers** (Right Ctrl, Right Alt, and sides inside chords), or use the **extended key set**: F13 to F24, punctuation by its US-layout name, the arrows, Home, End, Page Up, Page Down, Insert, Delete, Enter, Tab, Space, and numpad keys distinct from their counterparts. Display names follow Windows: "Win", "Right Ctrl", "Ctrl Ctrl". The binding record gains a textual form for these — a side prefix such as `rctrl`, and `tap:` and `double:` kinds — and the same kinds serve the Open Pane hotkey and every command's global hotkey alike. A single tap and a double tap of the same modifier bound together are refused: "cannot coexist".

**Recording**: on Windows the recorder (the Open Pane recorder, the Shortcuts page and the hotkey screen) asks the hook adapter for a recording session while it listens — the hook reports the raw key state and holds the keys back from Windows, so the Windows key alone, double taps and sides can be recorded without the Start menu opening. Escape and Tab still cancel. The session ends when the recorder stops listening, the window loses focus or Pane quits. On macOS and Linux the recorder is unchanged and offers none of the new kinds, which are explained there through the unavailable-row mechanism ("Not available on macOS: …").

**The Windows key alone**, while bound, is recognized as a press and release with no other key between, within about 500 ms; so that Explorer does not open the Start menu, Pane injects a tagged neutral key before the release reaches Explorer. Any other key pressed with the Windows key passes through untouched, so Win+E, Win+R and the rest keep Windows' meaning.

## Acceptance criteria

- [ ] Lone taps, double taps, side-specific modifiers and the extended key set bind and fire through the hook
- [ ] Numpad keys bind distinctly from their counterparts; the record round-trips its new textual forms and display names
- [ ] The recorder shows "Win", "Right Ctrl" and "Ctrl Ctrl" from a fake recording session; Escape still cancels; the Start menu does not open while recording
- [ ] A single and a double tap of the same modifier bound together are refused with "cannot coexist"
- [ ] While the Windows key alone is bound, other Win+<key> chords keep Windows' meaning and the Start-menu mask is emitted (adapter tests; the smoke presses the bound tap)
- [ ] Kinds unavailable on the test binary's own system are explained on their rows
- [ ] The recognizer is a pure function driven by synthetic key sequences in tests that run on every system: taps, double taps within and beyond the window, sides, keys between, coexistence

## Blocked by

- #252 — Fall back to a low-level keyboard hook when Windows refuses a hotkey
