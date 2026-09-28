# Files

Added for [#29](https://github.com/hoangvu12/pane/issues/29) (US03, US08,
US12, US40, US44, US57; T01, T03, T09, T22; G3, G7, as contributions, not
claims that they pass). A user chooses one folder, finds its files by
typing their names into [root search](root-search.md), and opens one with
the system's handler for its type. The feature is a **default extension**,
Files, which the user can disable like any package.

## Where it lives

As for [applications](applications.md), a pure WASI 0.3 guest cannot read
the user's folders (its WASI context preopens none) or open a file, so the
split, recorded as proposed in
[ADR 0017](adr/0017-host-lists-a-chosen-folder-for-the-files-extension.md), is:

- **Host capability**, in the core: `pane:extension/files`
  ([`wit/files.wit`](../wit/files.wit)), a host import any command may use:
  `list-folder(folder)` lists a folder under the [scan policy](#the-scan-policy).
  It holds no state, keeps no index and does nothing until a guest asks.
  The listing runs on a thread of its own, never on the extension thread,
  and stops as soon as the guest's call is dropped
  ([`pane_core::files`](../crates/pane-core/src/files.rs)).
- **Opening a file**, in the core: a computed root result's new action
  `open-file(path)` ([`wit/root-results.wit`](../wit/root-results.wit)),
  which Pane performs with the system's handler, as it performs `open-url`.
- **Default extension**, [`guests/files`](../guests/files) (Rust), package
  [`guests/packages/files`](../guests/packages/files): its command, "Files",
  chooses the folder in a [form](forms.md), keeps it in its
  [settings](extension-data.md), and answers root search's query with the
  matching files as `open-file` results.
- **Not a native helper** ([ADR 0014](adr/0014-optional-native-extension-helpers.md)):
  the host already lists folders and opens files natively on every system.
- **Not a preopened folder**: giving the guest a WASI preopen of the chosen
  folder would let it walk the folder itself, but its walk would run on the
  extension thread, where a large folder holds up every other extension's
  calls, and it could not be stopped while it runs (see
  [cancelling](#cancelling-a-pending-search)).

Acquiring the package automatically at setup is
[#51](https://github.com/hoangvu12/pane/issues/51) to
[#53](https://github.com/hoangvu12/pane/issues/53); until then it is
installed from its folder like the other default extensions
(`pane --install target/guests/packages/files`).

## Choosing the folder

The command **Files** lists:

- **Choose folder**, a form with one field, *Folder*, and a "Search this
  folder" button. Its subtitle says which folder is searched, or that none
  is chosen yet. Submitting an empty field keeps the current folder (forms
  have no initial values yet); with none chosen, it asks for one.
- **Stop searching the folder**, once one is chosen: root search then finds
  no files; the folder itself is not touched.
- **What is searched**, which states the scan policy.

The form lists the folder before saving it, so every problem is shown on
the field and the form stays open ("Folder: …"):

| Typed | Message |
| --- | --- |
| nothing, none chosen | Enter the folder's full path |
| a relative path | “notes” is not a full path: enter the folder's whole path, such as /home/you/Documents |
| a missing path | /…/missing does not exist |
| a file | /…/todo.txt is a file, not a folder |
| a folder the user may not read | Pane may not read /…/private |

Saved, the status says "Searching “<folder name>”: 4 files", with a note
when Pane stopped at its limits. The folder is the package's settings, so
it belongs to its identity, survives restarts, updates and disabling, and
is not removed with its cache.

## The scan policy

The same on every system, enforced by the host whatever the extension asks
([`files::walk`](../crates/pane-core/src/files.rs)):

- **Regular files only**, from the chosen folder and its subfolders,
  **breadth first** (a folder's own files before its subfolders'), each
  folder's entries **in name order** (by the bytes of their names, so
  capitals first on Linux and macOS).
- **At most 8 folders deep** below the chosen one, **5,000 files** and
  **20,000 entries** looked at (files, folders, links, anything). Reaching a
  limit stops the listing, which says so (`truncated`): the Files form's
  status and "What is searched" mention it.
- **Skipped, neither listed nor entered**: hidden entries (a name starting
  with `.`, and on Windows the hidden and system attributes), symbolic links
  and other links (Windows junctions included), names that are not Unicode,
  and a subfolder that cannot be read.
- The chosen folder itself must be an absolute path to a folder the user
  can read; it may be reached through a link.
- **Every query lists the folder again**: no index, no file watching and no
  cache, so a file saved a moment ago is found at the next keystroke. For a
  folder near the limits each keystroke reads up to 20,000 entries; the
  listing is cancelled as soon as the query changes.

Files in the chosen folder are found by name: a file is listed when each
word of the query is in its name, and then, after those, when each word is
in its name or the folders below the chosen one ("notes todo" finds
`notes/todo.txt`), ignoring letter case (Unicode lowercasing, no accent
folding), at most 20, in listing order. Each is a
[computed result](root-search.md#results-computed-from-the-query) titled
with the file's name and subtitled "File in <folder name>/<subfolders>".

Unlike other computed results, **file results are listed after the results
found by title** (commands and applications), before the rows explaining
failures and the fallbacks: a folder can hold many files matching a short
query, which should not push commands down. A blank query lists no files,
and none are listed while no folder is chosen.

## Opening a file

Enter (or a click) on a file result opens it. `Launcher::activate_selected`
refuses a relative path at once; otherwise it shows "Running…" and, off the
window's thread, checks that the path is still a file (a folder is refused:
it would open a file manager) and hands it to the launcher's `LinkOpener`
(`open_file`). The status then reads "Opened <file name>" or "Could not open
<file name>: <reason>" ("it no longer exists", "it is a folder; Pane opens
only files", the handler's own failure); root search keeps its query. The
status names only the file, not its folders. Pane opens any file the
extension offers, whatever its type: opening a program file runs it, as
the file manager would (Q9's trust model).

The window's opener, `pane::SystemLinks`, runs the handler the `open` crate
names for the system, without a shell, as for [links](quicklinks.md#opening-a-link):

| System | Handler | A missing handler |
| --- | --- | --- |
| Linux | `xdg-open <path>`, else `gio open`, `gnome-open`, `kde-open` | none installed: "no program to open this kind of file is installed"; xdg-open finding none (status 3): "no program to open this kind of file is set up"; the program failing (status 4): "the program for this kind of file refused or failed to open it" |
| macOS | `/usr/bin/open -- <path>` (Launch Services) | its failure status |
| Windows | PowerShell `Start-Process` (ShellExecute), the path passed in an environment variable, not on the command line; else `explorer.exe` | its failure status |

A handler still running after three seconds counts as having opened the
file. Tests replace the opener with a recording fake; a launcher given no
opener, or an opener that opens no files, says "this Pane has no handler
for files".

## Cancelling a pending search

A search owns its calls for computed results (root search's own calls,
not those of commands, forms or views, which navigation only discards):

- **The query changes**: the calls still pending for the older query are
  cancelled. A call not started is never started; one waiting inside the
  extension (here, on `list-folder`) is dropped where it waits, and so is the
  extension's instance, since Wasmtime 49 cannot cancel a task the host
  called ([generations](generations.md#what-stopping-costs)); the host's
  listing thread is told to stop and its answer, if it still comes, goes
  nowhere. The next query starts a fresh instance. A cancelled call is not a
  failure: it never counts towards [pausing](pausing.md).
- **Root search is left** (a command, Manage extensions, a package preview,
  Escape back to an empty query): the same.
- **The extension is disabled** (or reloaded, updated, paused or
  uninstalled): its generation ends, which stops the call as before (#14).
- **Delayed results never replace current ones**: an answer for an older
  search is dropped, as before; and since the listing runs off the
  extension thread, a listing that ignores cancelling holds up no other
  extension's call.

This applies to every command that computes root results (the calculator,
quicklinks), not only to Files; their calls rarely wait, so only a queued
call of theirs is ever skipped.

## For authors

Any command can list a folder and answer `open-file` results, in Rust,
JavaScript and TypeScript alike ([author guide](../guests/README.md#files-of-a-folder)):
`pane_guest::files::list_folder` in Rust; `listFolder` from
`"pane:extension/files@0.1.0"` in JavaScript and TypeScript, which rejects
with `payload` as the reason; and `RootAction::OpenFile(path)` or
`{ tag: "open-file", val: path }`. The samples
[`guests/sample-files-js`](../guests/sample-files-js) and
[`guests/sample-files-ts`](../guests/sample-files-ts) do what Files does,
with a simpler match (every word in the name).

## Checks

- Launcher public interface ([`crates/pane-core/tests/files.rs`](../crates/pane-core/tests/files.rs)),
  with the real Files guest, a recording opener and a controlled fixture
  folder named "Pane files — ñ" holding "Résumé plan ü.txt", a subfolder,
  a hidden file and a hidden folder: nothing found and nothing failing
  before a folder is chosen; a relative, blank, missing and file path each
  marked on the field with the form open and nothing saved; the chosen
  folder's files found by name, then by subfolder, case ignored, hidden ones
  not; Enter opening the Unicode file (the path the opener got, resolved,
  is the fixture's, resolved); file results after a command whose title
  matches; a folder removed since it was chosen explained as a row; a file
  removed after it was found explained on Enter; the folder kept across a
  restart; disabling removing the results and enabling bringing them back;
  and the JavaScript and TypeScript samples choosing the folder (a relative
  one refused), finding and opening the same file.
- Scan policy, on fixture folders (same file): breadth-first name order and
  absolute paths; hidden entries skipped; links to a file and a folder not
  listed or followed (macOS and Linux); each limit (depth, files, entries)
  stopping the listing and saying so; a cancelled listing stopping; and the
  errors for a relative path, a missing folder and a file.
- Cancelling, with a folder lister the test holds up (same file): a new
  query cancels the pending listing, its search ends at once and the next
  query's results are shown, never the older ones; a listing that ignores
  cancelling holds up nothing and its answer never shows; leaving root
  search (a package preview) cancels it and drops the instance, and the next
  query starts afresh; disabling Files cancels it, lists nothing and runs
  nothing afterwards.
- Native GUI smokes, one phase per system (screenshots 220 to 222), with a
  data folder of its own and a fixture folder "Pane smoke files" (spaces)
  holding "Résumé plan ü" (non-ASCII): install Files, choose the folder in
  its form with real key events, type "plan", check the selected row, and
  Enter. On Linux the file opens through the real `xdg-open` outside any
  desktop session, whose only handler for plain text is a script of the
  smoke's that records the path (XDG_CONFIG_HOME and XDG_DATA_HOME of the
  smoke's own, BROWSER the same script), and the recorded path, resolved,
  must be the fixture file's; it ran on Linux X11 on 2026-09-28
  ([evidence](platforms/linux.md#files-29)). On macOS and Windows the file
  has a type no application claims (`.panesmoke`), so the real handler
  (`open`, `Start-Process`) runs and opens nothing, and Pane must report
  that it could not open it; that is written but has not run yet.

## Limits

- One folder; no whole-disk index, other scopes, content search, file
  watching, icons, previews, recent files or ranking beyond name then path.
- Every keystroke lists the folder again; a folder at the limits costs up to
  20,000 entries per query, and results beyond the limits are not found.
- Cancelling a waiting call drops the extension's instance, losing what it
  keeps in memory (Files keeps nothing there).
- A handler slow to fail (over three seconds) is reported as having opened
  the file; opening a program file runs it.
- The positive native open ran only on Linux X11 (xdg-open with a recording
  handler); macOS and Windows open through the same code and are checked
  natively only for the handler reporting that no program opens the file,
  and that phase has not run yet.
- Screen reader behaviour is unverified, as for all of root search.
