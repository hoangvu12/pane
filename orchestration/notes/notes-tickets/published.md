# Published tickets for #123, #125, #127 (2026-10-12 session)

Published with the to-tickets process; bodies in `bodies/`, creation log in `created.txt`.
All attached as native sub-issues of their spec with native blocked_by edges.
W-human (#270) carries `ready-for-human`, not `ready-for-agent` (repo triage vocabulary; the spec
calls for a human check). All others: `implementation` + `ready-for-agent`.

## #123 Launcher polish (https://github.com/pane-app/pane/issues/123)
- #245 P-sel   Selected row a wash without an edge; fainter hover wash          — frontier
- #246 P-text  Text levels by alpha with contrast floors                        — frontier
- #247 P-icon  Built-in glyphs extensions name on Pane's neutral tile (narrowed: bare app/file/image icons, placeholders, pins/compact/Actions-header rules already shipped with #139/#142/#172) — frontier
- #248 P-load  Loading bar after 300 ms, replaces Running…                      — frontier
- #249 P-toast Toast close button, Escape, details popover (3 s timing/pause/pending already from #141) — frontier
- #250 P-hud   HUD icon+message, 150 DIP placement, fade, pending, announcements (window exists from #141) — frontier
- #251 P-mods  Exact modifiers, AltGr, Ctrl+C with selection, numpad, hint tests — frontier
- #258 P-nav   Backspace back, Alt/Ctrl row+section moves, rebindable            — blocked by #251

## #125 Windows power features (https://github.com/pane-app/pane/issues/125)
- #252 W-hook   Hook fallback + safeguards, record v2, warnings                — frontier
- #253 W-paste  Front application tracking + paste on Windows                   — frontier
- #254 W-run1   Run: what Win+R runs, shared history                            — frontier
- #255 W-sys1   System Commands: session and power                              — frontier
- #259 W-guard  Dispatch + hook health in Settings                              — by #252
- #260 W-kinds  Lone/double taps, sides, extended keys, recording, Start mask   — by #252
- #261 W-game   Game mode                                                       — by #252
- #262 W-sel    Selected text (UIA worker + simulated copy)                     — by #253
- #263 W-switch Switch Windows                                                  — by #253
- #264 W-run2   Run completions + Run in terminal                               — by #254
- #265 W-sys2   Volume and microphone                                          — by #255
- #266 W-sys3   Recycle Bin, appearance, devices                                — by #255
- #268 W-winkey Windows key fresh-install default, Use the Windows key, taskbar, docs — by #259, #260, #182
- #270 W-human  Windows 11 desktop check (ready-for-human)                      — by #268, #253, #262, #261, #263

## #127 Extension updates (https://github.com/pane-app/pane/issues/127)
- #256 U-res  Update results record + view + background failure announcement + 1-minute first check — frontier
- #267 U-now  Check for Extension Updates + Settings button, Update all        — by #256
- #269 U-def  Default extensions update from release tags (ADR 0045 shape)      — by #256, #207

## Deliberately left out
- UI Access helper (#125): per-machine install + code-signing prerequisites absent (spec's flagged later slice).
- Clipboard History's paste-primary action (#125): spec lists it as that extension's follow-up.
- Window actions on Switch Windows rows (#125): spec's later slice with the action panel.
- macOS/Linux/X11 follow-ups for hotkey kinds, paste, selected text, Switch Windows, System Commands, Run (#125): spec's proposed follow-ups; unavailable-row explanations cover them.
- Nothing from #123 or #127 was dropped; #123's P-icon was narrowed to the one unshipped rule.
