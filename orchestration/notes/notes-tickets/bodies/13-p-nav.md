## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

Backspace with no modifiers, not an auto-repeat, goes back one level — the Back action without clearing text — when the focused search field is empty: an opened command's search field, the command's own list, or a screen where no text field has focus (package previews, confirmations, details; not the hotkey screen while it records). In root search with an empty query it does nothing. Backspace in a form's text field or an argument field never navigates, and a held Backspace that empties a field does not then back out of the command.

Alt+Down and Alt+Up (Option on macOS) move the selection five rows at a time, stopping at the first and last rows, in root search, command lists, command search and the Actions panel. Ctrl+Down and Ctrl+Up (Command on macOS) move to the first row of the next section, or to the last row when there is no next section, and to the first row of the previous section (from inside a section, its own first row first), scrolling the section's label into view — across root search's result sections (in the vertical pinned layout the pinned rows count as the first section; the horizontal strip is not entered), a command list's sections and the Actions panel's groups. The jumps stop at the ends; they never wrap.

The Keyboard page's Emacs and Vim navigation choices already bind Alt+N/Alt+P and Alt+J/Alt+K on Windows and Linux (Ctrl on macOS); they gain Alt+B and Alt+F (Emacs) and Alt+H and Alt+L (Vim) as Left and Right between the query and the argument fields, and the Alt pair keeps working wherever Up and Down do, including Up's recall of recent queries. Backspace-back, Alt+Up/Down and Ctrl+Up/Down join the Keyboard page's rebindable in-app bindings with these defaults, following its rules for conflicts and reset, binding beneath focused controls (a field's own keys keep priority), and refused nowhere the existing bindings use them (Ctrl+Alt+Up and Ctrl+Alt+Down stay the pin move keys).

## Acceptance criteria

- [ ] Backspace backs out of an empty command search and from a details screen; does nothing in root search with an empty query; never fires on auto-repeat, from a form or argument field, or after a held Backspace empties a field
- [ ] Alt+Up and Alt+Down move five rows and clamp at the list's ends, in root search, command lists, command search and the Actions panel
- [ ] Ctrl+Up and Ctrl+Down cross root search's sections (Results, Files, Fallbacks) and a command list's sections, scrolling the label into view, stopping at the ends
- [ ] Alt+N/P (Emacs) and Alt+J/K (Vim) move the selection, Ctrl+K still opens the Actions panel under the Vim choice, and Alt+B/F, Alt+H/L move between the query and argument fields
- [ ] A rebound Backspace-back, Alt+Up/Down or Ctrl+Up/Down takes effect, with the Keyboard page's conflict rules; Ctrl+Alt+Up/Down keep moving the pins
- [ ] Window tests press real keys and cover every case above

## Blocked by

- #251 — Match every chord's modifiers exactly; numpad digits and Ctrl+C with a selection
