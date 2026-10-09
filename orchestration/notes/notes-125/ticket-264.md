## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

Run gains **completions while typing**: Run history first, then programs from App Paths and the registry search path, Control Panel applets, management consoles, registered URL schemes and environment variables, each matching the typed text, so the user types less.

And **Run in terminal**: the command line runs through the run-program host function — in a new Windows Terminal tab when Windows Terminal is installed, otherwise in the shell's own window — with the shell (PowerShell, Command Prompt) chosen in the command's preferences, so console tools stay open to read.

## Acceptance criteria

- [ ] Completions appear while typing, drawn from history, App Paths, the registry search path, applets, consoles, registered schemes and environment variables, each matching the typed text
- [ ] Run in terminal opens a new Windows Terminal tab when it is installed, otherwise the shell's own window, with the shell chosen in the command's preferences
- [ ] The launcher's public-interface tests, with a fake `run` adapter and the real default extension, cover completions and the terminal run

## Blocked by

- #254 — Add Run: run what Win+R runs, sharing its history

