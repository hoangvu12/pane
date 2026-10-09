## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

The host capability `pane:extension/run` (Windows only) and the default extension **Run**: a query-taking command that runs what Win+R runs — programs by name or path with arguments, Control Panel applets, management consoles, folders, network paths, `shell:` and `ms-settings:` addresses and other registered schemes, with environment variables expanded in paths.

History is Win+R's own, in both directions: read in Explorer's format (the lettered values in the order its list names, Explorer's trailing marker stripped, duplicates removed ignoring case); after each successful run, written back the same way with the new entry first, case-insensitive duplicates removed and at most 26 entries, so Win+R sees Pane's runs and Pane sees Win+R's. Deleting an entry rewrites the key without it. A failed run, or one declined at the elevation prompt, is not recorded. Parsing: a quoted head is taken as written; a head ending in a program, applet or console extension splits there; otherwise, for a rooted path with spaces, the longest space-separated prefix that exists on disk is the program (so `C:\Program Files\Tool\tool.exe -a` runs without quotes); otherwise the first space splits. Rooted paths are normalized (drive letter case, real casing, separators, network paths kept). Running: a `.cpl` applet through the Control Panel program; a bare name resolved on the search path read from the registry (machine and user), not Pane's own environment, and App Paths, so tools installed after Pane started are found; everything else through the system's open, which also opens a folder in Explorer. "Run as administrator" uses the elevation verb, showing Windows' own prompt (ADR 0033). Run is offered as a fallback for any text and usable through an alias, so a command line typed in root search can run it. The extension declares `windows` alone, joins the Windows default set enabled by default, and is disableable on its own on its page in Settings.

## Acceptance criteria

- [ ] Run's parsing, normalizer and classifier are pure functions tested on every system, including spaced unquoted paths and environment variables
- [ ] Run reads and writes Explorer's format in a test-owned registry key given to the adapter, sharing history both ways, and deletes an entry from it
- [ ] A marker-writing test program runs with arguments and unquoted spaced paths, resolved from a test search path
- [ ] A program installed after Pane started is found (the search path is read from the registry, not Pane's environment)
- [ ] An elevated run shows Windows' prompt; a declined elevation is not recorded and says why
- [ ] The launcher's public-interface tests, with a fake `run` adapter and the real default extension, cover running, recording and history deletion
- [ ] The extension appears in the Windows default set, enabled by default and disableable on its own
