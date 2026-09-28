# Clipboard history

Added for [#35](https://github.com/hoangvu12/pane/issues/35) (Windows):
US65, US66, US70, US71; T10, T21, T22; contributions to G5 and G7, not
claims that they pass. Once the user turns it on, Pane keeps the text they
copy on this computer, and the **Clipboard History** default extension lists
it, newest first; Enter on an item copies it again. It starts off, can be
paused and resumed, and disabling the extension stops it too. Expiry and the
remaining deletion controls are [#36](https://github.com/hoangvu12/pane/issues/36);
macOS and Linux observation are [#37](https://github.com/hoangvu12/pane/issues/37)
and [#38](https://github.com/hoangvu12/pane/issues/38). The architecture is
recorded in [ADR 0017](adr/0017-host-keeps-clipboard-history-for-an-extension.md)
(proposed).

## Where it lives

- **Host capability**, in the core: `pane:extension/clipboard-history`
  ([`wit/clipboard.wit`](../wit/clipboard.wit)), a host import any command
  of an installed package may use: `status`, `set-capture` (off, on,
  paused), `set-excluded`, `entries`, `copy` and `clear`. The host watches
  the clipboard and keeps the history itself, so nothing of the extension
  runs while the clipboard changes, and the history is the package's
  [extension data](extension-data.md) whatever the extension does.
- **Default extension**, [`guests/clipboard-history`](../guests/clipboard-history)
  (Rust), package [`guests/packages/clipboard-history`](../guests/packages/clipboard-history):
  its command, "Clipboard History", shows the controls and the kept items.
  It declares Windows only (`"platforms": ["windows"]`), since only Windows
  has an adapter, so on macOS and Linux it is listed as unavailable with
  #19's reason ("Not available on Linux: this command supports only
  Windows") and never runs. Rust commands use the import through
  `pane_guest::clipboard_history`; JavaScript and TypeScript commands cannot
  yet (it is not in their world, so their prebuilt samples are unchanged).
- **System adapter** behind one small trait
  ([`pane_core::clipboard`](../crates/pane-core/src/clipboard.rs)), chosen
  by `clipboard::native()`: the Windows listener, or on other systems one
  that says clipboard history is unavailable there.

Acquiring the package automatically at setup is
[#51](https://github.com/hoangvu12/pane/issues/51) to
[#53](https://github.com/hoangvu12/pane/issues/53); until then it is
installed from its folder (`pane --install target/guests/packages/clipboard-history`).

## Behavior

The command's rows, in order:

| Row | Enter |
| --- | --- |
| "Turn on clipboard history" (off), "Pause clipboard history" (on) or "Resume clipboard history" (paused), subtitled with the state, the number kept and, if Pane cannot watch the clipboard, why | turns it on, pauses or resumes it; each row does only that, so pressing it again before the command is opened anew changes nothing more |
| "Exclude a program" | a form taking a program's file name, such as `KeePass.exe` |
| "Stop excluding keepass.exe", one per excluded program | removes the exclusion |
| "Clear clipboard history", while items are kept | deletes every kept item; whether history is kept does not change |
| One row per kept item, newest first: its first line with content (at most 80 characters), subtitled "5 min ago · from notepad.exe · 2 lines · Enter copies it" | puts its text on the clipboard again ("Copied to the clipboard"); the copy is a change like any other, so it moves to the front |
| "Nothing kept yet", while on and empty | nothing |

The rows are the command's view when it opens: after an action the status
line answers, and the rows change the next time the command is opened.

- **Off until turned on.** A new package, and one never turned on, keeps
  nothing and Pane does not watch the clipboard at all: no listener is
  registered with the system. Turning it on starts the watch at once.
- **Watched exactly while kept.** Pane watches the clipboard while at least
  one installed package's history is on and the package runs (it is enabled
  and not [paused](pausing.md) after a failure). Pausing the history,
  disabling the package, Pane pausing it, uninstalling it, or turning
  another package's history off when it was the last one drops the
  listener at once (on Windows the listener's thread has removed itself and
  ended before the change returns). Enabling the package, resuming or
  turning it on starts it again. A change that arrives while the package's
  code may not run is not kept, even if the listener has not stopped yet.
- **Across restarts.** The capture state is kept with the history: after a
  restart Pane watches again, before any command opens, only where history
  is on and the package enabled; paused stays paused, disabled stays
  disabled and keeps its history (retention while disabled is #36).
- **What is kept** ([`clipboard::accept`](../crates/pane-core/src/clipboard.rs)),
  the same on every system:
  - plain text only (Windows `CF_UNICODETEXT`); a copy with text and other
    formats keeps the text; images, files and rich formats alone keep
    nothing;
  - at most 32 KiB of UTF-8 (`MAX_TEXT_BYTES`); longer text is not kept at
    all rather than cut short;
  - not empty or white space only;
  - not marked by the copying application as not to be kept (below);
  - not copied from an excluded program, matched by the owning process's
    file name, ignoring case, with or without its extension (`KeePass`
    excludes `KeePass.exe`); at most 64 programs;
  - one item per text: copying a kept text again moves it to the front with
    its new time;
  - at most 100 items per package (`MAX_ITEMS`); beyond that the oldest go.
    This bounds the file; it is not the retention policy (#36).
- **Local only.** The history stays in Pane's data folder; Pane sends none
  of it anywhere and no other extension can read it through Pane (only the
  package that keeps it). This is not a boundary against trusted extensions
  or other programs running as the user
  ([policy](extension-policy-proposal.md#clipboard-history)).

## Sensitive markers

Before reading the text, Pane's Windows listener reads the formats
applications use to say that what they copied must not be kept, and reads
the text only if none of them does:

| Format | Pane keeps the text |
| --- | --- |
| `ExcludeClipboardContentFromMonitorProcessing` present | never |
| `Clipboard Viewer Ignore` present (older password managers) | never |
| `CanIncludeInClipboardHistory` = 0 | never |
| `CanUploadToCloudClipboard` = 0 | never, although Pane uploads nothing: an application refusing the cloud is taken to mark the text sensitive |
| `CanIncludeInClipboardHistory` or `CanUploadToCloudClipboard` = 1, or absent | yes, unless another rule refuses it |

These are the formats Microsoft documents for clipboard monitors, Windows'
clipboard history and its cloud clipboard; password managers such as KeePass
and KeePassXC set one or more of them when they copy a password (which ones
depends on the application and its version, and was not checked here).
Detection is only as good as what applications declare: an application that
sets none of them is kept like any other, and Pane does not try to detect
secrets in the text itself. A program can also be excluded by name, but the
owning process is the one whose window owns the clipboard, which is
sometimes a helper process or none at all (a copy made without a window has
no owner, so its program is unknown and never excluded).

## Ownership and deletion

- The history is the package's extension data of a kind of its own,
  **clipboard history**, in `clipboard-history.json` beside the other kinds,
  under the package identity's key, written by Pane only (never by the
  extension directly), atomically, and readable only by the user on macOS
  and Linux (mode 0600; on Windows the data folder's permissions). Its
  values: `capture` ("on" or "paused"; missing is off), `excluded` (a list
  of program names) and one `item:<id>` per item (`text`, `copiedAt` in
  milliseconds since the Unix epoch, `source`).
- It is **saved data**, like settings and content: Clear cache keeps it;
  uninstalling asks, "Saved data: 12 clipboard history items", and
  "Uninstall and delete saved data" removes it, while "keep" keeps it as
  [retained data](extension-data.md#retained-data) ("keeps 12 clipboard
  history items", or "clipboard history settings" when only the state and
  exclusions are kept), which "Delete retained data" removes. Reinstalling
  the same source after keeping it keeps history as it was, on if it was on.
- "Clear clipboard history" in the command deletes every item of the
  package and keeps its state and exclusions. Per-item deletion, "Disable
  and delete history" and expiry are #36. Deleting is not forensic erasure,
  and never changes what is on the system's clipboard.

## Per platform

| | Windows (#35) | macOS (#37) | Linux (#38) |
| --- | --- | --- | --- |
| Observed with | `AddClipboardFormatListener` on a message-only window of a thread of Pane's own (`WM_CLIPBOARDUPDATE`), reading the markers first, then `CF_UNICODETEXT`, and the owner through `GetClipboardOwner`, `GetWindowThreadProcessId` and `QueryFullProcessImageNameW` | not yet | not yet |
| Written back with | `SetClipboardData(CF_UNICODETEXT)` | | |
| Permission | none | | |
| Unavailable | | the command is listed as "Not available on macOS: this command supports only Windows" | "Not available on Linux: this command supports only Windows" |
| Baseline | Windows 10/11; CI `windows-2025` | | |

## Checks

- Capture rules ([`clipboard.rs`](../crates/pane-core/src/clipboard.rs) unit
  tests): plain, marked, withheld, other, blank and long content; excluded
  programs; program names; newest first, one per text and at most 100;
  state, exclusions and clearing kept apart. The package's generation
  ([`extension_data.rs`](../crates/pane-core/src/extension_data.rs)):
  nothing kept while off, paused by Pane or uninstalled, and each change
  starting or stopping the watch.
- Launcher public interface ([`crates/pane-core/tests/clipboard.rs`](../crates/pane-core/tests/clipboard.rs)),
  with the real Clipboard History guest and a fake system clipboard (a copy
  of the package declaring every system): nothing watched or kept until
  turned on, then kept, on disk too with mode 0600; markers, blank, other and
  long content; excluding and including a program through the form; pause
  and resume, also across a restart; disable stopping the watch, a restart
  while disabled not watching, enable and a restart watching again; Enter
  copying an item again; the 100-item bound and Clear; uninstall deleting
  or keeping (retained, and kept on for a reinstall); a system that cannot
  watch, and a launcher without a clipboard; the real package unavailable on
  macOS and Linux with its reason.
- Windows adapter ([`crates/pane-core/tests/clipboard_adapter.rs`](../crates/pane-core/tests/clipboard_adapter.rs),
  Windows only) against the real clipboard, with text only the test puts
  there: plain text reported with its owner (the test's own process), each
  of the four markers withholding the text, markers of 1 allowing it, a
  written text reported, and nothing once the watch is dropped. It saves the
  clipboard's formats in memory first and puts them back at the end.
- Native GUI smokes, screenshots 280 to 285: on Windows (with a data folder
  of its own, copying only its own `pane-smoke-...` text through the
  clipboard API and putting back what was on the clipboard, in memory only)
  off, turned on, kept without the four marked texts, paused, resumed,
  Enter copying an item again, disabled, disabled across a restart,
  enabled and kept again across a restart, with `clipboard-history.json`
  checked at each step; on macOS and Linux (280 and 281) the command listed
  as unavailable and explained, with no history file. See the
  [Windows](platforms/windows.md#clipboard-history-35),
  [macOS](platforms/macos.md#clipboard-history-35) and
  [Linux](platforms/linux.md#clipboard-history-35) notes for where they have
  run.

## Limits

- Windows only; macOS (#37) and Linux (#38) observation are not built.
- Text only; no images, files or rich text, and no text longer than 32 KiB.
- Detection of sensitive content is only what applications declare
  (markers) and the programs the user excludes; the owning process can be a
  helper or unknown.
- No finite retention yet: items stay until cleared, pushed out by newer
  ones (100), or deleted with the package's saved data (#36).
- The command's rows are read when it opens; they do not change while it is
  open, even as text is copied.
- JavaScript and TypeScript commands cannot use the import yet.
- More than one package may keep history; each keeps its own, and Pane
  watches once for all of them.
- The files are replaced atomically but not locked (as every kind of
  extension data): two Pane processes on one data folder can lose each
  other's last write.
