Implements the launcher polish specification #123 (ADR 0035): Raycast's selection, icons, loading and feedback timing, and keyboard extras, within the accepted UI of ADR 0029.

## What it does

- **Selection without an edge (#245):** the selected row in every launcher surface is a wash of the text colour (~10% alpha) with no inset edge or ring; the computed answer card loses its accent ring; a fainter (~5%) hover wash, fading out over ~70 ms, appears only where hovering does not select; focus rings on controls that take the keyboard stay.
- **Text levels by alpha (#246):** secondary, tertiary, faint and separator text become the text colour at 60/40/20/10%, with contrast floors (4.5:1 primary/secondary, 3:1 tertiary) that keep the accepted opaque colour where alpha would fall below them.
- **Built-in glyph tiles (#247):** a built-in glyph an extension names draws on Pane's neutral command tile; image icons stay bare; guard tests keep the bare/tile split true across rows, pins and the Actions header.
- **A loading bar after 300 ms (#248):** a one-pixel sweep along the rule under the search field replaces the footer's "Running…" text, appearing only once waited-for work has lasted 300 ms and fading in and out; still at partial strength under reduced motion.
- **Toasts that leave (#249):** outcome messages in the footer self-dismiss after 3 s (paused while hovered or focused, restarted after), with a close button, Escape dismissal, and a details popover (Ctrl+T) for long text.
- **The HUD shaped (#250):** content-sized, centred, its bottom 150 logical pixels above the monitor's bottom, 46/56 px tall, at most 500 wide; 1.2 s (3 s for failures) then fading out over about a second; never taking focus, click-through, announced.
- **Exact chord modifiers (#251):** every key and chord fires only when Shift, Ctrl, Alt and Windows/Command are held exactly as declared; AltGr characters reach the field; Ctrl+C with a selection copies the text; numpad digits act as Ctrl+digit.
- **Backspace back, Alt+arrows, rebindable (#258):** Backspace on an empty field goes back one level; Alt+Up/Down move five rows; Ctrl+Up/Down jump sections; the keys join the Keyboard page's rebindable bindings.

Closes #123
Closes #245
Closes #246
Closes #247
Closes #248
Closes #249
Closes #250
Closes #251
Closes #258

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
