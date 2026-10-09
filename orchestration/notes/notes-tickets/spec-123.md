# #123 Launcher polish: Raycast's selection, icons, loading and feedback timing, and keyboard extras within the accepted UI
['ready-for-agent', 'specification']

## Problem Statement

The user is happy with Pane's launcher as built (ADR 0029), but next to Raycast, which Pane learns from, it feels heavier in small ways that add up over a day of use:

- The selected row carries a border as well as a wash, so the list looks busier than it needs to, and the selection's look is tied to one shade rather than to the text colour, so it reads differently on glass, on solid and over a background image.
- Every result sits on a tile, applications included. An application's own icon, once Pane has it, would be boxed into a tile meant for Pane's commands, and applications would look like commands.
- "Running…" flashes in the footer for actions that finish in a few milliseconds, which reads as flicker rather than progress; and nothing shows that root search is still waiting for a slow provider.
- A result or an error stays in the footer until the next action, so an old "Copied 42 to the clipboard" lingers; and when the launcher is hidden there is nowhere for a short confirmation to appear at all.
- Text levels are separate fixed greys rather than one text colour at a few strengths, so secondary text does not follow the surface beneath it.
- Several keys a keyboard user expects do nothing: Backspace in an empty search field does not go back, there is no way to move several rows at once or jump between sections, and nothing guarantees that a chord with an extra modifier (Ctrl+Shift+K, Shift+Enter) never triggers the plainer chord. Once results carry their own copy actions, Ctrl+C in a field with selected text could copy the row instead of the text.

## Solution

Keep Pane's accepted design (proportions, type, tiles, footer, Actions panel, background image and frost) and borrow Raycast's polish where the user decided it (the user's decision 2, recorded in ADR 0035):

- **Selection without an edge.** The selected row is a wash of the text colour at about 10%, with no inset edge, and the calculator card loses its accent ring; where hovering does not select, a fainter 5% wash shows it. Selection changes at once; focus rings on controls that take the keyboard stay.
- **Bare application icons.** An application's own icon is drawn as it is, with no tile behind it; Pane's commands and built-in rows keep their tiles.
- **A loading bar that waits 300 ms.** A one-pixel sweep along the line under the search field shows that work is pending, but only once it has lasted 300 ms; quick actions show nothing.
- **Toasts that leave after 3 seconds.** A result or failure in the footer is a toast that dismisses itself after three seconds, paused while it is hovered or focused; a long one opens in full in a details popover.
- **A HUD for when the launcher is hidden.** A small, click-through message near the bottom of the screen shows for 1.2 seconds (3 seconds for a failure). On Wayland, where Pane cannot place such a window, a toast-like message takes its place.
- **Number hints after a 400 ms hold**, as today, guarded by tests.
- **Text through alpha.** Text levels are the text colour at 100, 60, 40 and 20%, wherever that keeps text legible on the surface beneath it.
- **Keyboard extras.** Backspace on an empty field goes back one level; Alt+Up and Alt+Down move five rows; Ctrl+Up and Ctrl+Down jump between sections; the Emacs and Vim navigation bindings move to Alt (Alt+N/P, Alt+J/K), as Raycast's do on Windows; numpad digits work as Ctrl+digit; every chord matches its modifiers exactly; Ctrl+C with text selected copies the text; and the new keys can be rebound on the Keyboard page like Pane's other in-app bindings.

## User Stories

### Selection and hover

1. As a launcher user, I want the selected row marked by a soft wash without a border, so that the list looks calm and the selection still stands out.
2. As a launcher user, I want the selection's wash to be a strength of the text colour, so that it reads the same on glass, on solid and over my background image, in light and dark.
3. As a launcher user, I want a hover wash, where hovering does not select, to be lighter than the selection, so that I never confuse where the pointer is with what Enter will run.
4. As a launcher user, I want the selection to move instantly when I press a key, so that navigation never lags behind my fingers.
5. As a pointer user, I want a hover wash to fade out quickly when I leave its item, so that moving across the surface feels smooth rather than blinking.
6. As a launcher user, I want the same selection look in root search, a command's list, command search, Manage extensions and the Actions panel, so that the launcher feels like one surface.
7. As a user who reduces motion, I want hover changes without fades, so that nothing animates against my preference.
8. As a launcher user, I want a selected calculator answer card marked by the same wash rather than an accent ring, so that every selected row looks alike.
9. As a keyboard user, I want fields and controls that take my keys to keep their focus ring, so that I always see where my typing goes.

### Icons

10. As a launcher user, I want an application's own icon shown as it is, without a tile behind it, so that applications look like themselves.
11. As a launcher user, I want Pane's commands and built-in rows to keep their tiles, so that I can tell commands from applications at a glance.
12. As a launcher user, I want an application whose icon is not available yet to keep a stable placeholder of the same size, so that the row does not jump when the icon arrives.
13. As a launcher user, I want my pinned slots and the compact window's pins to follow the same icon rules, so that a pinned application looks like it does in the results.
14. As an extension author, I want my command's own image icon drawn as I made it, and the built-in glyphs I name drawn on Pane's tile, so that my extension looks deliberate either way.
15. As a launcher user, I want a file result to show the system's icon for its type without a tile, so that files look like files.

### Loading

16. As a launcher user, I want no loading indicator for work that finishes quickly, so that ordinary actions feel instant.
17. As a launcher user, I want a thin moving line under the search field when an action or search takes longer than 300 ms, so that I know Pane is still working.
18. As a launcher user, I want that line to show while root search waits for a slow provider, so that I know more results may come.
19. As a launcher user, I want the line to fade away when the work ends, so that it does not snap off distractingly.
20. As a user who reduces motion, I want a still line instead of a sweep, so that I am told without animation.
21. As a screen-reader user, I want to be told that Pane is working only when it has been working for a moment, so that quick actions are not announced as busy.
22. As a launcher user, I want background work Pane reports in words (such as a development build) to keep its words in the footer, so that I still know what is happening.

### Toasts

23. As a launcher user, I want a confirmation such as "Copied 42 to the clipboard" to leave by itself after three seconds, so that the footer returns to its actions.
24. As a launcher user, I want a failure to leave after three seconds too, unless I am looking at it, so that old errors do not linger.
25. As a pointer user, I want a toast to stay while my pointer rests on it, so that I can finish reading it.
26. As a launcher user, I want to dismiss a toast at once, by its close button or a key, so that I can clear it when I have read it.
27. As a launcher user, I want a toast too long for the footer to show its first line and open in full on request, so that long errors stay readable without crowding the list.
28. As a launcher user, I want a new toast to replace the current one, so that I always see the latest outcome.
29. As a launcher user, I want a toast still running (pending) to stay until its work ends, so that progress never disappears early.
30. As a screen-reader user, I want each toast announced once when it appears, so that I hear outcomes without looking.
31. As a keyboard user, I want the toast's details and its close button reachable by keys, so that I never need the pointer for feedback.

### HUD

32. As a user, I want a short message near the bottom of my screen when an action finishes after the launcher has closed, so that I know it worked.
33. As a user, I want that message gone after about a second, or three seconds for a failure, so that it confirms without interrupting.
34. As a user, I want the HUD never to take focus or catch clicks, so that the application I returned to keeps working undisturbed.
35. As a user, I want the HUD on the monitor where I used the launcher, so that I see it where I am looking.
36. As a user, I want a toast shown while the launcher is hidden or compact to appear as a HUD, so that I never miss it.
37. As a screen-reader user, I want the HUD's message announced, so that a hidden launcher's confirmation still reaches me.

### Number hints and text

38. As a keyboard user, I want the Ctrl+digit numbers to appear only after I hold Ctrl alone for 400 ms, so that ordinary chords never flash them.
39. As a keyboard user, I want the numpad's digits to work as Ctrl+digit too, so that either row of digits picks a result.
40. As a launcher user, I want secondary text, section labels and placeholders to be the text colour at lower strengths, so that the hierarchy follows the surface beneath it.
41. As a user with low vision, I want every text level to stay legible against its actual backdrop, so that alpha-based text never fades below what I can read.

### Keyboard extras

42. As a keyboard user, I want Backspace in an empty search field to go back one level, so that I can leave a command the way I left the text.
43. As a keyboard user, I want holding Backspace to delete my text without then backing out of the command, so that a long press never surprises me.
44. As a keyboard user, I want Backspace to go back from a screen with no text field focused (a detail, a confirmation), so that the same key backs out everywhere.
45. As a keyboard user, I want Backspace in an empty form field or argument field to do nothing more, so that clearing a field never leaves the form.
46. As a keyboard user, I want Alt+Down and Alt+Up to move five rows at a time, so that I can cross long lists quickly.
47. As a keyboard user, I want Ctrl+Down and Ctrl+Up to jump to the next and previous section, so that I can reach Fallbacks or Files directly.
48. As a keyboard user, I want those jumps to stop at the list's ends rather than wrap, so that I always know where I am.
49. As a keyboard user, I want Ctrl+Shift+K never to open Actions and Shift+Enter never to invoke the row, so that a chord only does what it says.
50. As a user of a European layout, I want characters typed with AltGr to reach the field, never a Ctrl+Alt chord, so that I can type my language.
51. As a keyboard user, I want Ctrl+C with text selected in a field to copy that text, so that a row's own copy action never takes over ordinary copying.
52. As a keyboard user, I want Ctrl+C with no text selected to run the selected row's copy action where it has one, so that copying a result stays one chord.
53. As a macOS user, I want the same keys with Command and Option where Pane already maps Ctrl and Alt that way, so that the extras feel native.
54. As a keyboard user who chose Emacs navigation, I want Alt+N and Alt+P to move the selection down and up, so that the binding never collides with Pane's Ctrl chords.
55. As a keyboard user who chose Vim navigation, I want Alt+J and Alt+K to move the selection down and up, so that Ctrl+K stays Open actions.
56. As a keyboard user, I want to rebind Backspace back, Alt+Up/Down and Ctrl+Up/Down on the Keyboard page, so that the new keys follow the same rules as Pane's other in-app bindings.

## Implementation Decisions

### Scope and decision provenance

- **The user's decision 2 (2026-10-06), recorded in ADR 0035,** which amends ADR 0029 without reopening it: keep Pane's accepted design and borrow Raycast's polish: selection without an edge (alpha-based selection, a fainter hover where hovering does not select; the selected row's inset edge and the computed answer card's accent ring go; focus rings on controls that take the keyboard stay; the frosted selected row over a background image stays); bare application icons once Pane has them (built-in commands keep tiles); the loading indicator shown only after 300 ms; toasts hiding after 3 seconds, an animated in-progress toast staying until updated; a HUD for 1.2 seconds (3 seconds for failures); number hints after a 400 ms hold (as ADR 0027 already has it); text levels by alpha where it fits. Raycast's proportions and palette are not taken.
- **Scope set by the user's list of specifications:** these keyboard extras: Backspace on an empty query goes back one level; Alt+Up and Alt+Down move five rows; Ctrl+Up and Ctrl+Down jump sections; exact modifier matching; Ctrl+C with selected text copies text.
- **Proposed defaults the user delegated ("like Raycast, as flexible as possible"):** success and failure toasts both hide after 3 s like Raycast's, pausing while hovered or focused, with long text in a details popover; the new keys (Backspace back, Alt+Up/Down, Ctrl+Up/Down) are rebindable on the Keyboard page, like the other in-app bindings (#77); numpad digits work as Ctrl+digit; the Alt-based Emacs and Vim navigation bindings (Alt+N/P, Alt+J/K) are included; the HUD behaves the same on macOS and Linux where the system allows a non-activating window, with a toast-like fallback on Wayland; an application row whose icon has not arrived shows a neutral placeholder of the same size.
- **Proposed defaults**, not separately confirmed: the exact alphas (10% selection, 5% hover, 70 ms hover fade-out), the loading bar's look and fade, the toast's details affordance and its key, the HUD's placement, size and fade, where each key extra applies, and the contrast floors for alpha text. They are Raycast for Windows 2.6's measured values where Raycast has one.
- **What stays as accepted (ADR 0029):** the launcher's size and proportions, row height and radius, Geist and Geist Mono, the accent, tiles for commands, the footer's layout and buttons, the Actions panel's place and layout, the pinned home, the compact window, the background image and frost (ADR 0028), and the existing motion policy. Raycast's proportions, fonts and colours are not adopted.
- **Relation to sibling specifications.** The "Extension commands like Raycast" specification defines the extension-facing host functions that show toasts and HUDs (styles, updates, actions, the toast-to-HUD fallback when the window is hidden) and the icon model extensions use; this specification defines how the launcher draws and times them, and builds the HUD surface they need. Application icons themselves (extraction, cache, light and dark variants) are the "Applications done properly" specification (#124)'s; this specification draws them. Narrator announcement of the selected row is the "Quick fixes" specification (#129)'s.

### Selection and hover

- As ADR 0035 decides: the selected row's wash is the theme's text colour at a low alpha (proposed 10%, Raycast's; white in dark, black in light), with **no inset edge or ring**. The selected row's 1px inset edge goes, and so does the computed answer card's accent ring while it is selected; the card shows its selection by the same wash. Over a background image the selected row stays frosted as ADR 0028 draws it, without the edge.
- **Hover** gets a fainter wash (proposed 5%) only where hovering does not select. In root search and the launcher's lists the pointer moving over a row selects it (as today), so those rows show the selection wash, not a separate hover wash; the hover wash applies where the pointer does not move the selection (for example while an overlay holds the target, and on surfaces such as pinned slots and footer buttons that have their own hover today). Where a hover wash is drawn, it fades out over 70 ms when the pointer leaves; selection changes are always instant.
- The same tokens serve root search's rows, a command's list, command search, Manage extensions and confirmation rows, and the Actions panel's and Pane menu's entries (replacing their separate 8.5%, 3.5% and 11% values). A control that takes the keyboard (a field, a Settings control, a focused slot) keeps its focus ring. The Settings window's sidebar and controls keep their own tokens.
- Reduced motion: no hover fade.

### Icons in rows

- **Applications** draw their own icon (from the "Applications done properly" specification (#124)) bare at the row's icon size, with no tile, edge, highlight or drop shadow. Until the icon is available, or when there is none, the row shows a neutral placeholder of the same size (the generic application glyph, faded, without a tile; proposed default, delegated), so the layout never shifts. **Files** draw the system's icon for the file bare, with the same fallback.
- **Pane's own commands, default extensions' commands and built-in rows keep their tiles** as accepted. An extension command's **image** icon (the "Extension commands like Raycast" specification (#120)'s icon model) is drawn as supplied, with its mask; a **built-in glyph** an extension names is drawn on Pane's neutral tile; a package without an icon gets the generated first-letter tile the "Extension commands like Raycast" specification (#120) defines.
- Pinned slots, compact pins and the Actions panel's header follow the same rules at their own sizes.

### The loading bar

- A one-pixel line along the rule under the search field shows a soft highlight sweeping across it (one pass every 1.5 seconds) while work the user is waiting for is pending: an invoked action or command call, an opened command's search, or root search's providers for the current query (the "Root search like Raycast" specification (#122)'s publishing). It appears only when that work has lasted 300 ms, fading in and out over about 300 ms; work that ends sooner shows nothing.
- It replaces the footer's "Running…" text for such work. Background work Pane describes in words (development builds, acquisition, updates) keeps its words in the footer, as today.
- Reduced motion: the line shows at partial strength without sweeping.
- Accessibility: the "busy" state is announced through the existing status announcement only once the 300 ms threshold has passed.

### Toasts in the footer

- The footer's outcome messages (a result or a failure) become **toasts**: drawn in the footer's left slot as today, one at a time, a new one replacing the current one.
- **Timing (proposed default, delegated, as Raycast does):** every toast that is not pending, success and failure alike, dismisses itself 3 seconds after it appears; if the launcher is active it fades out over 200 ms, otherwise it is removed at once. The timer pauses while the pointer is over the toast or its details are open, or while it has keyboard focus, and restarts the full 3 seconds when that ends. A pending toast (work still running, such as the "Extension commands like Raycast" specification (#120)'s animated toast) never times out; it ends when its work ends, or is updated, and an extension's pending toast is closed when the launcher deactivates.
- **Dismissing:** a close button appears on hover or focus; Escape while the toast has focus dismisses it.
- **Long text:** a toast whose text does not fit on one line shows its first line, truncated, and a details affordance; Ctrl+T (Command+T on macOS) or a click opens the full text, wrapped and scrollable, in a popover above the footer, which also lists the toast's actions when the "Extension commands like Raycast" specification (#120) gives it some. This replaces the footer growing to show a long status (the #64 behaviour) with an on-request view; the full text stays reachable. (Proposed default, delegated.)
- **Where outcomes go:** the launcher's own results and failures keep their wording ("Copied 42 to the clipboard", "Could not open …"). The core keeps the status; the window owns the timing and clears an expired toast through the launcher, so a later screen never shows a stale one.
- Accessibility: each toast is announced once when it appears, as statuses are today; its close button and details are focusable and named.

### The HUD

- A small, borderless, topmost window that never takes focus and lets clicks through: content-sized, 46 logical pixels tall (56 with a second line), at most 500 wide, centred horizontally on the monitor the launcher last showed on, its bottom edge 150 logical pixels above that monitor's bottom. It uses Pane's popover material and type: an optional icon, a one-line title and an optional one-line message.
- **Timing:** a default or success HUD shows for 1.2 seconds, a failure for 3 seconds, then fades out over about a second; a pending HUD stays until it is updated or the launcher becomes active again. One at a time; a new one replaces the current one. Reduced motion: no fade.
- **Use:** the "Extension commands like Raycast" specification (#120)'s HUD host function (which closes the launcher first) and its toast-to-HUD fallback when the launcher is hidden or in the compact window mode both draw here. Pane's own actions keep the launcher open as today, so they use toasts.
- Accessibility: the HUD's text is announced through the platform's notification mechanism for an unfocused window (on Windows, a UI Automation notification).
- Platforms (proposed default, delegated): Windows first. macOS and Linux X11 get the same behaviour where the system allows a non-activating window; on Wayland, where a client cannot place such a window, a toast-like message takes the HUD's place (in the footer when the launcher is shown, otherwise as the platform allows), and that limitation is documented.

### Number hints

- Kept as ADR 0027 already has it (ADR 0035 confirms it) and guarded by tests: the numbers appear after Ctrl (Command on macOS) is held alone for 400 ms; any other key, a key release, a scroll, focus leaving or the window deactivating hides them; a chord never shows them.
- The numpad's digits act as the digit row's for Ctrl+digit chords, like Raycast (proposed default, delegated).

### Text through alpha

- As ADR 0035 decides, where it fits, secondary and tertiary text use the primary text colour at a lower alpha, and where a level's contrast would suffer the accepted colour stays. Proposed strengths, Raycast's: primary (titles, the query) 100%; secondary (subtitles, kind labels, footer labels, keycap labels) 60%; tertiary (section labels, the placeholder, disabled text) 40%; quaternary (faint marks) 20%; separators 10%.
- **Where it fits:** for each theme, material and background-image case Pane supports, a role keeps its present opaque value wherever the composited contrast of its alpha form against that surface would fall below a floor (proposed: 4.5:1 for primary and secondary text, 3:1 for tertiary text and placeholders). The check is made against the surfaces' own tints, so glass over an unknown desktop is judged on its tint alone, as the existing legibility floor for the glass tint is.
- The Settings window's text roles are unchanged.

### Keyboard extras

- **Backspace goes back.** Backspace with no modifiers, not an auto-repeat, goes back one level (the Back action without clearing text) when the focused search field is empty: an opened command's search field, the command's own list, or a screen where no text field has focus (package previews, confirmations, details, the hotkey screen excepted while it records). In root search with an empty query it does nothing. Backspace in a form's text field or an argument field never navigates. A held Backspace that empties a field does not then go back.
- **Alt+Down and Alt+Up** (Option on macOS) move the selection five rows down or up, stopping at the first and last rows, in root search, command lists, command search and the Actions panel.
- **Ctrl+Down and Ctrl+Up** (Command on macOS) move to the first row of the next section, or to the last row when there is no next section; and to the first row of the previous section (from inside a section, its own first row first), scrolling the section's label into view. Sections are root search's result sections (in the vertical pinned layout the pinned rows count as the first section; the horizontal strip is not entered this way), a command list's sections (the "Extension UI you can design" specification (#121)) and the Actions panel's groups. They stop at the ends; they never wrap.
- **Alt-based Emacs and Vim navigation** (proposed default, delegated, like Raycast for Windows): the Keyboard page's Emacs and Vim navigation choices move from Ctrl to Alt on Windows and Linux: Emacs Alt+N and Alt+P move down and up (Alt+B and Alt+F move between the query and argument fields at their edges, as Left and Right do), Vim Alt+J and Alt+K move down and up (Alt+H and Alt+L as Left and Right). This ends the Vim binding's collision with Ctrl+K (Open actions). macOS keeps Ctrl, as Raycast does there. The Alt pair works wherever Up and Down do, including Up's recall of recent queries (the "Root search like Raycast" specification (#122)).
- **Rebindable** (proposed default, delegated): Backspace back, Alt+Up/Down and Ctrl+Up/Down join the Keyboard page's rebindable in-app bindings (#77), with these defaults, and follow its rules for conflicts and reset. They bind beneath focused controls, so a field's own keys keep priority, and they are refused nowhere the existing bindings use them (Ctrl+Alt+Up and Down stay the pin move keys).
- **Exact modifiers.** Every launcher key and chord fires only when Shift, Ctrl, Alt and the Windows or Command key are each held exactly as it declares: the Keyboard page's bindings, Ctrl+digit, the pin keys, the new extras, and the "Extension commands like Raycast" specification (#120)'s action shortcuts and Enter variants. Ctrl+Shift+K does not open Actions, Shift+Enter does not invoke, Ctrl+Alt+digit picks nothing. A character the layout produces with AltGr (reported as Ctrl and Alt on Windows) is typed into the focused field and never matches a Ctrl+Alt chord. Letter chords follow the key the current layout labels with that letter.
- **Ctrl+C with selected text** (Command+C on macOS): when the focused field (the query, an argument, command search, a form field) has a non-empty selection, Ctrl+C copies the selected text, even if the selected row has an action bound to Ctrl+C; otherwise the row's action runs. Ctrl+Shift+C is unaffected.

### Motion

- New motion uses the shared motion policy and its reduced-motion handling: the hover fade, the loading bar's sweep and fades, the toast's fade and the HUD's fade all stop under reduced motion. Nothing here adds motion to navigation or selection, which stay instant.

### Modules

- **The theme** gains the alpha-based selection, hover and text-role tokens and the contrast check that decides where each applies.
- **The result row and its siblings** (Actions entries, slots, compact pins) drop the selection edge and apply the icon rules.
- **The footer** gains the toast slot's timing, close button and details popover; **the launcher shell** gains the loading bar on the rule under the search field.
- **A new HUD window module** in the window layer, with a placement adapter per platform.
- **The window's key handling** gains the extras, the exact-modifier rule and the Ctrl+C rule; the core gains nothing for keys. The core exposes whether work the user is waiting for is pending and since when (for the 300 ms threshold) and lets the window clear an expired status.

## Testing Decisions

- A good test presses real keys, moves the simulated pointer and advances the harness's controlled animation time, then checks what is drawn and what ran; it never asserts token values for their own sake or private state.
- **The window tests** (the launcher-window integration harness, with real sample extensions in Rust, JavaScript and TypeScript and controlled time, as the motion work of #84 to #88 introduced) are the primary seam:
  - selection: the selected row has no edge and the selection wash, a selected computed answer card has no accent ring, a focused field keeps its focus ring, the selected row over a background image stays frosted, a hover wash (where hovering does not select) is lighter and its fade finishes within its time and is absent under reduced motion; the same in a command list, Manage extensions and the Actions panel;
  - icons: an application row with an icon has no tile and one without keeps a same-size placeholder; a command row keeps its tile;
  - loading: an action shorter than 300 ms never shows the bar; a slow one (the existing slow fixture) shows it after 300 ms and hides it when it ends; "Running…" no longer appears for it; background progress text still does;
  - toasts: dismissed after 3 s, paused while hovered or focused and restarted after, replaced by a newer one, a pending one kept, a long one truncated and opened in full with Ctrl+T, closed by its button and by Escape, announced once;
  - number hints: shown after 400 ms of Ctrl alone, never on a chord, numpad digits picking rows;
  - navigation bindings: Alt+N/P (Emacs) and Alt+J/K (Vim) moving the selection, Ctrl+K opening Actions under the Vim choice; a rebound Backspace-back, Alt+Up/Down or Ctrl+Up/Down taking effect, with the Keyboard page's conflict rules;
  - keys: Backspace back from an empty command search and from a details screen, not from root, not on auto-repeat, not from an empty form field or argument; Alt+Up and Down moving five rows and clamping; Ctrl+Up and Down across root search's sections (Results, Files, Fallbacks) and stopping at the ends; Ctrl+Shift+K, Shift+Enter and Ctrl+Alt+digit doing nothing; an AltGr character typed into the query; Ctrl+C with a selection copying the text (read through the window's clipboard) and without one running the row's copy action.
  Prior art: the window, keyboard, compact pins and command-search window suites.
- **Theme tests** check the contrast rule: for each theme, material and background-image case, each text role's chosen form meets its floor.
- **The HUD** is tested in the window harness for timing, replacement, focus never taken and the toast-to-HUD route when the launcher is hidden or compact; its placement and click-through are checked natively on Windows by the smoke scripts (a screenshot phase with the launcher hidden), and on other systems where they run.
- **The core's launcher tests** cover only what the core exposes: pending work's start time and clearing an expired status.
- No native screen-reader run is claimed; announcements are checked through the accessibility tree and the harness's announcement capture, as today.

## Out of Scope

- Raycast's proportions (750 by 475, 38-pixel rows, a 66-pixel search bar, a 42-pixel footer), Inter and JetBrains Mono, following the system accent, a seven-colour semantic palette and contrast-corrected tag colours (the "Extension commands like Raycast" specification (#120)'s accessories and the "Extension UI you can design" specification (#121)'s tokens), and the coloured glow behind toasts.
- Moving the Actions panel's search to its bottom, its opening overshoot, removing the dimmer, or any other change to its layout.
- Removing or changing the existing view-enter, section and popup motion.
- Pointer extras: double-click to invoke and right-click to open the Actions panel on rows.
- Page-navigation keys (Ctrl+[ and Ctrl+]), and Escape closing the window outright when a command was opened by its own hotkey.
- Interface size or density, theme editing, and the Settings window's styling.
- Narrator announcement of the selected row (the "Quick fixes" specification (#129)), application icon extraction (the "Applications done properly" specification (#124)), and the toast and HUD host functions (the "Extension commands like Raycast" specification (#120)).

## Further Notes

- Research: `docs/research/raycast-deep-dive.md` (look and motion, window and feedback, keyboard), with the earlier `docs/research/raycast-inspiration.md`. Raycast facts used here: selection is the selection colour at 10% and hover at 5%, with no border, selection instant and hover fading out over 70 ms; rows draw application icons bare while built-in commands ship coloured tiles; the loading sweep sits on the one-pixel rule under the search bar and appears only after 300 ms; every non-pending toast hides after 3 s with a 200 ms fade, pauses on hover, has a close button, and opens its actions with Ctrl+T; pending toasts close when the window deactivates; the HUD shows for 1.2 s (3 s for failures), sits 150 DIP above the monitor's bottom, is click-through and fades out over 1 s; number hints use a 400 ms hold; text is the foreground at 100, 60, 40 and 20%; Backspace on an empty search field goes back, never on auto-repeat; Alt+Up and Down move five rows; Ctrl+Up and Down jump sections; its Emacs and Vim bindings use Alt on Windows (Alt+N/P, Alt+J/K); numpad digits pick results; shortcuts match each modifier exactly; Ctrl+C with selected text copies the text.
- Pane already matches two of the user's points: number hints after a 400 ms hold, and the selection changing instantly. This specification keeps them and guards them with tests.
- Glossary updates to make with ADR 0035 (through domain modeling): "Toast" and "HUD" (shared with the "Extension commands like Raycast" specification (#120)), and the Number hints entry for numpad digits.
- **Settled since the first draft (proposed defaults the user delegated, "like Raycast, as flexible as possible"):**
  1. Success and failure toasts both hide after 3 s, pausing while hovered or focused; long text is available through the details popover.
  2. An application row before its icon arrives shows a neutral placeholder of the same size.
  3. The new keys are rebindable on the Keyboard page, like the other in-app bindings (#77).
  4. Numpad digits work as Ctrl+digit.
  5. Alt-based Emacs and Vim navigation (Alt+N/P, Alt+J/K) is included here.
  6. The HUD behaves the same on macOS and Linux where the system allows a non-activating window, with a toast-like fallback on Wayland.


---

Decisions this specification relies on are recorded in [ADRs 0030–0040](https://github.com/hoangvu12/pane/tree/main/docs/adr) (added by [#119](https://github.com/hoangvu12/pane/pull/119)); the evidence is [docs/research/raycast-deep-dive.md](https://github.com/hoangvu12/pane/blob/main/docs/research/raycast-deep-dive.md).


## COMMENTS
