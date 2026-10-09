## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0039)

## What to build

Pane follows which application was in front before it: a system foreground event hook, out of process and cheap, records the last window in front that is not one of Pane's own windows and not a shell surface (the taskbars and their overflow, the desktop, Alt+Tab and task view, Start, Search, the lock screen, input and notification hosts, Widgets), starting when Pane starts. A host function every command has answers the **front application**: its name and icon, and whether a target exists.

Paste — ADR 0037's host function, already declared with copying as the fallback — is implemented on Windows: Pane hides its window, puts the text on the clipboard tagged as not to be kept (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory` 0 and `CanUploadToCloudClipboard` 0, so neither Pane's clipboard history nor Windows' own keeps it), brings the target to the front on a worker with a timeout (skipping a window that is not responding; the plain foreground call first, then attaching to the foreground thread's input and trying again; a newer request supersedes an older one), waits until the target really is in front (up to about 500 ms), sends a tagged Ctrl+V, and restores the clipboard's earlier contents after a short delay (about 500 ms) unless the clipboard changed meanwhile. The restore is tagged too, and copies back every format held in ordinary memory (text, rich text, HTML, images as device-independent bitmaps, file lists); formats rendered on demand or held as graphics handles cannot be restored and are documented as lost. Text only in this slice; images and files are a follow-up.

Failures are honest: no target, a target gone or not responding, a target that did not come to the front, or a target running as administrator (detected from its process's elevation before injecting, since Windows would silently drop the keys) each answer the command with a failure saying why; the text stays on the clipboard, still tagged so history stays clean, and the earlier contents are not restored, so the user can paste by hand. On macOS and Linux the front application and paste answer "not available on this system yet", and the SDK's paste helpers fall back to copying and saying so, as already declared.

## Acceptance criteria

- [ ] The front application host function answers name, icon and whether a target exists, exercised through samples in Rust, JavaScript and TypeScript
- [ ] Paste into a test-owned edit control arrives; the clipboard is restored; a clipboard listener sees only tagged writes
- [ ] The pasted text is kept out of Pane's clipboard history and Windows' own
- [ ] A hung, gone, never-front or elevated target fails with a clear message, the text staying on the clipboard, tagged
- [ ] Shell surfaces and Pane's own windows are never tracked as the target
- [ ] On other systems the functions answer "not available" and the SDK helper copies instead, saying so
- [ ] Launcher public-interface tests run against a fake system; opt-in real-input adapter tests (the Windows runner) cover the real path

