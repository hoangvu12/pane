# Slicing plan (draft)

Format: ## Parent / ## What to build / ## Acceptance criteria / ## Blocked by. Size S/M/L. CI line.

## #123 Launcher polish
- P-sel  Selection without an edge + hover wash (all lists, Actions panel, Pane menu, calculator card)
- P-text Text levels by alpha with contrast floors
- P-icon Icon rules remaining after #172 (placeholder, files bare, pins/compact/Actions header, ext image vs glyph)  [check]
- P-load Loading bar after 300 ms (replaces Running…)
- P-toast Toasts leave after 3 s (pause, close, Escape, Ctrl+T details popover)
- P-hud  HUD window (Windows first, mac/X11, Wayland fallback)   [check what #141 did]
- P-mods Exact modifiers, AltGr, Ctrl+C with selection, numpad digits, number-hint tests
- P-nav  Backspace back, Alt+Up/Down, Ctrl+Up/Down, rebindable, Emacs/Vim to Alt   (blocked by P-mods)

## #125 Windows power
- W-hook   record v2 + refused chord falls back to LL hook (thread, tag, resync, only-while-needed), warnings list, smoke phase
- W-guard  watchdog, locked pages, Keyboard page diagnostics      (by W-hook)
- W-kinds  lone/double taps, sides, extended keys, hook recording, Start mask, coexistence, unavailable rows (by W-hook)
- W-winkey fresh-install Windows key default, Use the Windows key, Show taskbar, Ctrl+Alt+Space fallback, docs (by W-guard, W-kinds, #182)
- W-paste  front application tracking + paste on Windows
- W-sel    selected text (by W-paste)
- W-switch Switch Windows (by W-paste)
- W-run1   Run: runs, history shared, elevation, fallback/alias
- W-run2   Run: completions + Run in terminal (by W-run1)
- W-sys1   System Commands: capability + extension + session/power
- W-sys2   audio + mic (by W-sys1)
- W-sys3   Recycle Bin, appearance, HDR, desktop, hidden files, eject, Bluetooth (by W-sys1)
- W-game   game mode (by W-hook)
- W-human  ready-for-human Windows 11 desktop check (by W-winkey, W-paste, W-sel, W-game, W-switch)
- left out: UI Access helper (prereqs absent), Clipboard History paste-primary (spec says follow-up)

## #127 Extension updates
- U-res    update results record + view + background failure announcement + 1-minute first check
- U-now    Check for Extension Updates + Settings button, user-asked pass, progress toast, Retry/Update Now (by U-res)
- U-def    default extensions update from release tags (by U-res, #207)
