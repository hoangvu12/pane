Implements spec #125 — the Windows power features — on one branch.

## What it does

- **Hotkeys that never fail for a reason Windows gives.** A binding Windows refuses through `RegisterHotKey` — or one `RegisterHotKey` cannot express — is recognized through a low-level keyboard hook (`WH_KEYBOARD_LL`) of Pane's own, guarded like Raycast guards its own: installed only while a binding needs it, a fast high-priority thread, locked pages, a raw-input liveness watchdog that reinstalls it, stuck-modifier recovery, and tagged injected input. Settings shows each hotkey's dispatch route and the hook's health; an optional game mode pauses every hotkey while a game is in front; lone and double modifier taps, side-specific modifiers and an extended key set bind alike; and a fresh install opens Pane with the Windows key alone (ADR 0039).
- **Paste and selected text.** Pane tracks the application that was in front before it, pastes into it with a tagged Ctrl+V and a restored clipboard, and reads the text selected there through UI Automation in a worker process, falling back to a simulated copy (ADR 0037's host functions, implemented on Windows).
- **Switch Windows**, **System Commands** and **Run** join the default extensions, Windows-only, enabled by default and each disableable on its own (ADR 0040): window switching like Alt+Tab, session/power/volume/mic/appearance commands through documented Windows APIs, and Run sharing Win+R's own history in both directions.

Closes #125.

- Closes #252 — Fall back to a low-level keyboard hook when Windows refuses a hotkey
- Closes #253 — Track the front application and paste into it on Windows
- Closes #254 — Add Run: run what Win+R runs, sharing its history
- Closes #255 — Add System Commands: session and power
- Closes #259 — Show each hotkey's dispatch and the keyboard hook's health in Settings
- Closes #260 — Bind lone and double modifier taps, sides and the extended key set
- Closes #261 — Add game mode: pause Pane's hotkeys while a game is in front
- Closes #262 — Read the text selected in the front application on Windows
- Closes #263 — Add Switch Windows: list the open windows and bring one to the front
- Closes #264 — Complete Run while typing and run in a terminal
- Closes #265 — Add System Commands: volume and the microphone
- Closes #266 — Add System Commands: the Recycle Bin, appearance and devices
- Closes #268 — Open Pane with the Windows key alone on a fresh install

#270 (check the Windows power features by hand on a Windows 11 desktop) stays open for the user's hand validation.

Signed-off-by: hoangvu12 &lt;hggaming91@gmail.com&gt;
