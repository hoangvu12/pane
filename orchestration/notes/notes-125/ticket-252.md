## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

For each binding Pane wants active, the Windows adapter still calls `RegisterHotKey` when the binding is a plain chord it can express and Windows accepts. When Windows refuses the registration — another application has it, or Windows keeps it — the binding is recognized through a low-level keyboard hook (`WH_KEYBOARD_LL`) of Pane's own instead, so **a refused shortcut is never an error**: the binding works, and its row says it is dispatched through Pane's keyboard hook, and for a shortcut Windows or a known application uses, what that shortcut does there. The Windows reserved list (Alt+F4, Alt+Space, Win+D, Win+E, Win+R and the rest) becomes warnings rather than refusals. Still refused, with the reason: shortcuts no program can intercept (Win+L, Ctrl+Alt+Delete), and a binding another of Pane's bindings already has (naming the command, as today).

The hook is installed only while a binding needs it and removed when none does. Its callback runs on a dedicated thread at the highest thread priority (not a raised process priority) and only updates a small state machine and posts a message: it never allocates on the hot path, takes no lock the window or runtime holds, and calls nothing in GPUI or the extension runtime. Its code and data pages are locked after raising the process's minimum working set by what they need; if Windows refuses, the diagnostics say the pages are not pinned. A liveness watchdog — a message-only window receiving raw keyboard input — notices when several raw key events arrive without the hook seeing them, installs the hook again, counts attempts and gives up with a diagnostic after repeated failures within a short time. The state machine re-reads the modifiers' real state from the system when a key event contradicts it, on session unlock and on resume, so a missed release never leaves a modifier stuck down for Pane. Every key Pane injects carries Pane's own tag in its extra information; the hook passes tagged events through untouched, and keys other tools inject are passed through without counting as the user's.

The hotkey record and the host settings' Open Pane field move to a new version that still reads version 1 (chords only in this slice; the new kinds come in a later ticket). Rows of hook-based bindings say they do nothing while an application running as administrator is in front, since Windows does not deliver those keys to a hook of a normal process.

## Acceptance criteria

- [ ] A registration Windows refuses falls back to the hook and the binding fires; the row reports its dispatch route and it is not an error to the user
- [ ] Windows-reserved shortcuts become warnings saying what Windows does with them; Win+L and Ctrl+Alt+Delete are still refused with the reason; a Pane-internal clash still names the other command
- [ ] The hook installs when the first binding needs it and is removed when none does
- [ ] The binding recognizer, driven by synthetic key-event sequences in tests that run on every system, covers chords, keys between, stuck modifiers resynchronized, tagged events passed through, and injected keys not counted as the user's
- [ ] A settings record of version 1 is still read; the new version round-trips
- [ ] The launcher's public-interface hotkey tests (fake adapter, real sample guests) cover the refusal-falls-back rule and the still-refused explanations
- [ ] Opt-in real-input adapter tests (the Windows runner) cover the hook reporting a chord injected with a test tag, and the watchdog detecting and repairing a hook removed behind the adapter's back
- [ ] A native smoke phase covers a hook-dispatched binding: a shortcut the smoke registers first from another process, then assigns in Pane, opens the test command

