## Parent

https://github.com/pane-app/pane/issues/125 (Windows power features; ADR 0040)

## What to build

The host capability `pane:extension/windows` (Windows only; every call elsewhere answers "not available on this system") and the default extension **Switch Windows** (ADR 0040's set): a command listing the windows Alt+Tab would show, so the user can find a window by typing and switch to it with Enter.

**Which windows count** (documented calls only): visible, unowned top-level windows that are not "no activate", not tool windows unless forced onto the taskbar, not removed from the taskbar by their application, not a Store app's inner core window (its frame is kept), not the shell's desktop windows; windows the application cloaks are dropped; windows the shell cloaks because they are on another virtual desktop are kept only when the user's Alt+Tab setting shows all desktops and the documented virtual desktop manager says they are elsewhere. Pane's own windows and windows with an empty title are dropped.

**Records**: an opaque id valid for the session, title, application name and icon — the window's own AppUserModelID first (which separates web apps from their browser and finds Store apps hosted by the frame process), then the program's path matched against installed applications, then the process's packaged identity — minimized, maximized, on another desktop, and whether it is elevated. Ordered by z-order with the front application first, so switching back is one keystroke.

**Activate**: restore if minimized, then the same foreground path as paste. A window on another virtual desktop is activated as Windows activates it from Alt+Tab. Typing filters by title and application name. The extension declares `windows` alone, joins the Windows default set enabled by default, and is disableable on its own on its page in Settings. Window actions (close, minimize, maximize, restore, keep on top) are a later slice; an action refused because the window is elevated says so.

## Acceptance criteria

- [ ] The window predicate keeps an ordinary window and drops tool, owned, no-activate and cloaked ones — a pure function over synthetic window attributes, tested on every system
- [ ] Window-to-application resolution works over fake applications, including PWAs under their own app identity
- [ ] The extension lists and activates windows through the launcher's public interface with a fake `windows` adapter; the front application's window is listed first
- [ ] A native smoke phase switches to a window the smoke opened
- [ ] The extension appears in the Windows default set, enabled by default and disableable on its own

## Blocked by

- #253 — Track the front application and paste into it on Windows

