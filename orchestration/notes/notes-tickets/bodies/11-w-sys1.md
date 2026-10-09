## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

The host capability `pane:extension/system` (any command may use it; Windows implementations in this ticket) and the default extension **System Commands** (ADR 0040's set): each command a no-view command (ADR 0037), so its hotkey runs it without showing Pane's window. This slice holds the session and power commands:

- **Lock Screen**; **Log Out** (planned shutdown reason, not forced); **Restart** and **Shut Down** (the shutdown privilege enabled for the call only, forcing applications closed after the confirmation, as the user decided in ADR 0040)
- **Sleep**: on a Modern Standby computer, turns the displays off so Windows enters its standby, otherwise suspends; **Hibernate**: only with a hibernation file, else explained
- **Turn Off Displays**: a monitor-power message to a window of Pane's own with a timeout, never a broadcast a hung application could block; **Start Screen Saver**

Restart, Shut Down and Log Out ask first, with a destructive confirmation offering "don't ask again" (ADR 0037's confirm host function). Each command answers with a HUD of the state it ended in — 1.2 seconds, or 3 when it failed (ADR 0035). Every command is a root result the user can give an alias or a global hotkey. The same operations are available to any extension through the host import, in Rust, JavaScript and TypeScript alike.

## Acceptance criteria

- [ ] Each command calls the capability and answers with the expected HUD text, through the launcher's public interface with a fake system adapter
- [ ] Restart and Shut Down ask the adapter to force applications closed; Log Out does not
- [ ] The destructive ones confirm first, with "don't ask again" remembered
- [ ] A no-view command's hotkey runs it without showing the window
- [ ] Sleep on Modern Standby turns the displays off, otherwise suspends; Hibernate without a hibernation file is explained (fake capabilities)
- [ ] The commands' decisions are pure functions tested on every system (fake capabilities for Modern Standby, no hibernation file)
- [ ] The extension appears in the Windows default set, enabled by default and disableable on its own on its page in Settings
