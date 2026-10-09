## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

System Commands gains the audio and microphone commands, on the default output device:

- **Volume Up**, **Volume Down**, **Toggle Mute** and **Set Volume** — one argument, 0 to 100, through typed arguments — each answering with a HUD of the volume it ended at.
- **Toggle Microphone Mute**: if any capture device is unmuted, mute all, otherwise unmute them all, and say which it did; devices that vanish meanwhile are skipped; no microphone is explained.

Each remains a no-view command answering through ADR 0037's host functions, usable through an alias or a global hotkey, and available to any extension through the host import.

## Acceptance criteria

- [ ] Each volume command answers with the volume it ended at, through the launcher's public interface with a fake system adapter
- [ ] Set Volume takes its argument through typed arguments
- [ ] Toggle Microphone Mute mutes all when any is on, unmutes all otherwise, and says which it did
- [ ] Fake-capability tests cover two microphones with one muted, devices vanishing mid-toggle, and no microphone at all

## Blocked by

- #255 — Add System Commands: session and power

