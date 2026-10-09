## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

System Commands gains the remaining catalog, all through documented Windows APIs, each a no-view command answering with a HUD of the state it ended in (or why nothing changed):

- **Open Recycle Bin** and **Empty Recycle Bin** — asking first, destructive in style with "don't ask again"; an already empty bin is a success
- **Toggle System Appearance** — the personalization values for apps and system, followed by the setting-change broadcasts given a hang timeout
- **Toggle HDR** — the display configuration's advanced colour state: if any capable display is off, turn all on, otherwise all off; none capable is explained
- **Show Desktop**; **Toggle Hidden Files** (refreshing open Explorer windows); **Eject Removable Drives** — each drive locked, dismounted and ejected, per-drive failures reported; **Toggle Bluetooth** (the documented radios API)

## Acceptance criteria

- [ ] Each command calls the capability and answers with the state it ended in, or explains why nothing changed
- [ ] Empty Recycle Bin confirms first and treats an already empty bin as a success
- [ ] Fake-capability tests cover no HDR-capable display, and per-drive ejection failures
- [ ] The system adapter is exercised only for reversible reads in the opt-in real-input tests (Recycle Bin size)
- [ ] The launcher's public-interface tests drive every command through a fake system adapter

## Blocked by

- #255 — Add System Commands: session and power
