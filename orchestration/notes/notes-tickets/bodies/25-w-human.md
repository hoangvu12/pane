## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features)

## What to build

What the Windows power features cannot prove in CI, checked by hand on a real Windows 11 desktop, as the specification's research open questions list: Start-menu suppression on a real desktop, game mode against a real game, the feel of lone and double taps, paste into elevated or hung applications and foreground reliability. Record what was checked as comments on this issue; file anything broken as its own issue and link it here; close this one when every check passes or is accounted for.

## Acceptance criteria

- [ ] The Windows key opens Pane on a fresh install without the Start menu opening as well; Win+E, Win+D, Win+L and the rest keep Windows' meaning; the Start menu is still reachable from the taskbar's Start button and Ctrl+Esc
- [ ] Lone and double-tap hotkeys feel right to use against the Start menu (tap and double-tap windows)
- [ ] Game mode pauses Pane's hotkeys while a real game is in front, full-screen and windowed, and they return when it leaves
- [ ] Paste reaches ordinary applications reliably, and fails honestly into elevated and hung ones, leaving the text on the clipboard
- [ ] Switch Windows brings the chosen window to the front reliably across virtual desktops
- [ ] Findings recorded as comments; anything broken filed and linked

## Blocked by

- #268 — Open Pane with the Windows key alone on a fresh install
- #253 — Track the front application and paste into it on Windows
- #262 — Read the text selected in the front application on Windows
- #261 — Add game mode: pause Pane's hotkeys while a game is in front
- #263 — Add Switch Windows: list the open windows and bring one to the front
