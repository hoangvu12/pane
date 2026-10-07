# Files

Added for [#29](https://github.com/hoangvu12/pane/issues/29) (US03, US08,
US12, US40, US44, US57; T01, T03, T09, T22; G3, G7, as contributions, not
claims that they pass), and reworked after its security review. A user
grants one folder, finds its files by typing their names into
[root search](root-search.md) or into its command, **Search Files** (#150),
and opens, reveals, copies or recycles one through the file actions Pane
performs itself. The feature is a **default extension**, Files, which the
user can disable like any package.

## Where it lives

A pure WASI 0.3 guest cannot read the user's folders (its WASI context
preopens none) or open a file, so Pane's host does both, and owns every step
that decides what is reached, as recorded (proposed) in
[ADR 0017](adr/0017-host-lists-a-granted-folder-for-an-extension.md):

- **The grant**, in the core: a package declaring `"folderAccess": true` in
  `pane.json` gets Pane's own "Choose folder…" row; Pane checks the folder
  and records it in its own `folders.json`
  ([`pane_core::files`](../crates/pane-core/src/files.rs),
  [`launcher/files.rs`](../crates/pane-core/src/launcher/files.rs)).
- **The listing**, in the core: `pane:extension/files`
  ([`wit/files.wit`](../wit/files.wit)), `list-folder()` without a path,
  answered at once from the listing Pane makes on the package's own worker.
- **The file actions**, in the core: a computed root result's action
  `open-file(id)` ([`wit/root-results.wit`](../wit/root-results.wit)), or a
  command search result's `file` ([`wit/search.wit`](../wit/search.wit)),
  names a file by the id Pane gave it; Pane shows its own name for it,
  gives it its own actions, and checks it again before each
  ([`launcher/own_actions.rs`](../crates/pane-core/src/launcher/own_actions.rs)).
- **Default extension**, [`guests/files`](../guests/files) (Rust), package
  [`guests/packages/files`](../guests/packages/files): its one command,
  Search Files (id `files`, `"search": true` and `"rootResults": true`),
  only matches the listing Pane gives it against the text typed and answers
  with the files' ids.

Acquiring the package automatically at setup is
[#51](https://github.com/hoangvu12/pane/issues/51) to
[#53](https://github.com/hoangvu12/pane/issues/53); until then it is
installed from its folder like the other default extensions
(`pane --install target/guests/packages/files`).

## Granting the folder

Every command of a package with `"folderAccess": true` starts with Pane's
own rows, above the extension's items:

- **Choose folder…**, subtitled with the folder granted now ("Pane lets
  Files list only /…/Documents") or that none is. Enter opens the system's
  folder picker (`Launcher::folder_to_choose`, then the window calls
  `Launcher::grant_folder`); cancelling changes nothing.
- **Stop sharing the folder with <title>**, once one is granted: the grant
  is taken back and recorded; the folder itself is not changed.

Pane checks the chosen folder before granting it
([`files::check_grant`](../crates/pane-core/src/files.rs)) and says why not
("Pane did not grant the folder: …"), keeping the earlier grant:

| Chosen | Refused because |
| --- | --- |
| `\\server\share`, `\\?\UNC\…`, `//server/share` (Windows) | a network location, told from the text before any file system call |
| `/`, `C:\` | the whole disk |
| the home folder itself (`HOME`, `USERPROFILE`) | choose a folder inside it |
| a folder whose name starts with `.` (or hidden on Windows) | a hidden folder |
| a file, a missing path, a folder the user may not read | as it says |

A granted folder is recorded **canonically** (links resolved; on Windows
without the `\\?\` prefix) in `folders.json` beside `installed.json`, by
package identity key: `{"version": 1, "folders": {"<identity>": "<folder>"}}`.
It is Pane's record, not extension data: the extension never supplies,
saves or sees the path. It survives restarts and disabling; uninstalling
the package forgets it, whether or not its saved data is kept.

## The scan policy

The same on every system, enforced by the host
([`files::walk`](../crates/pane-core/src/files.rs)); the limits are defined
once, as `pane_core::files::MAX_DEPTH`, `MAX_FILES` and `MAX_ENTRIES`, and
extensions read them with `files.limits()`:

- **Regular files only**, from the granted folder and its subfolders,
  **breadth first** (a folder's own files before its subfolders'), each
  folder's entries **in name order** (by the bytes of their names).
- **At most 8 folders deep**, **5,000 files** and **20,000 entries** looked
  at (files, folders, links, anything). Entries are counted, and
  cancellation checked, *while* each folder is read, so a huge folder stops
  at the entry limit rather than being read whole first; only the paths of
  the folders still to list are queued, not open directory handles.
- **Skipped, neither listed nor entered**, at every level: hidden entries (a
  name starting with `.`, and on Windows the hidden and system attributes),
  symbolic links and other links (Windows junctions included) and names
  that are not Unicode.
- A subfolder (or an entry) that cannot be read is skipped and makes the
  listing **partial**: `truncated` is set, as when a limit is reached.

### When it is listed

`list-folder()` never waits. The first call in a visit of root search
starts a listing on the package's **own worker thread** (one per package,
started with its first listing) and answers `listing`; the extension
answers no files yet. The worker waits 100 ms (`files::DEBOUNCE`) before it
starts, and takes only the newest request. The listing is then **kept for
the visit**: every later keystroke gets it at once, and only filters it. It
is dropped when root search is left (a command, Manage extensions, a
preview, a restart of the visit) and when the grant changes, so the next
visit lists the folder again; there is no index and no file watching.

Once a listing ends, the commands whose answer waited for it are asked
again for the query then on screen, **after** every other result of that
query was shown: computed results (the calculator, quicklinks), and the
[indexed results](root-search.md#results-supplied-ahead-of-the-query) (the
applications). A slow or huge folder therefore holds up no other
extension's results; only its own files arrive later.

A listing stops (its answer is dropped) when root search is left, when the
grant changes, and when the package's generation ends (disable, reload,
update, pause, uninstall). A new query does not restart it: the new search
waits for the same listing, and the older search's wait is cancelled. A
search whose extension was told the folder is listing waits for its own
visit's listing (or a newer one), even if it ended before the search began
waiting (Files is then asked again at once); a listing of a visit already
left that stops late does not end that wait.

**A hung folder** (an unresponsive disk or a network mount the host could
not tell apart) holds only its package's worker: no new thread is started
for later requests, which wait (only the newest is kept), other extensions
are unaffected, and Files lists nothing until the listing returns. With
UNC paths refused this should be rare; mapped network drives on Windows and
network mounts on macOS and Linux are not detected.

## In root search

Files are found by name: a file is listed when each word of the query is in
its name, and then, after those, when each word is in its name or the
folders below the granted one ("notes todo" finds `notes/todo.txt`),
ignoring letter case (Unicode lowercasing, no accent folding), at most 20.
That matching is the extension's.

Each row is a
[computed result](root-search.md#results-computed-from-the-query) whose
title and subtitle are **the host's**, not the extension's: the file's own
name, and "File in <granted folder's name>/<subfolders>". A result naming an
id the host did not give in the package's latest listing is not listed.
File results are listed **after the results found by title** (commands and
applications), unlike other computed results. A blank query lists no files,
and none are listed while no folder is granted.

## Search Files

Search Files (#150) is a view command whose search field is the
launcher's own ([command search](command-search.md)): Enter on its row in
root search opens it with the field empty above its own list (Pane's
folder rows, then "What is searched"), and typing lists the granted
folder's files as root search does (by name, then by folder, at most 20),
each titled with its own name and "File in <folder>". A newer text stops
the search before it. Opening the command is a new visit, so the folder is
listed again then; a search answered while it is still being listed shows
"Running…" and is asked again once the listing ends, unless a newer text
(or leaving the command) stopped it first.

A command may set both `"search"` and `"rootResults"` (#150): root search
asks it, and so does its own field. Root search still never asks a
command that searches unless it says `rootResults` too.

## The file actions

Each file, in Search Files and in root search's file results alike, has
actions Pane performs itself, without calling the extension, as an item of
a command's list has them: Enter runs the first, Ctrl+Enter the second,
Ctrl+Shift+Enter the third, and the Actions panel (Ctrl+K) lists them all.

| A document | A program or script |
| --- | --- |
| **Open** (Enter): the system's handler for its type | **Show in Explorer** (Enter) |
| **Show in Explorer** (Ctrl+Enter): selected in the file manager | **Open With…** (Ctrl+Enter) |
| **Open With…**: a submenu of the installed applications, by name | **Run** (Ctrl+Shift+Enter): the system's handler, which runs it |
| **Copy Path**: its path, as text | **Copy Path** |
| **Copy File**: the file, as the file manager copies it | **Copy File** |
| **Move to Recycle Bin** (destructive): after a confirmation | **Move to Recycle Bin** |

File search's own Enter never runs a program by accident (ADR 0037's
exception, keeping ADR 0017's intent): a file that would run a program
when opened (below) is shown in the file manager, and only its explicit
**Run** runs it.
Whether a file is one is told on the listing's worker, with the listing, so
a row knows at once what Enter does. (On macOS the file manager is Finder,
so the action is "Show in Finder", elsewhere "Show in File Manager"; the
Recycle Bin is the Trash outside Windows.)

Each action closes the window after it acts and says what it did in a
HUD, as the standard actions do: Open, Show in Explorer, Open With… and
Run ("Opened plan.md", "Showed run.bat in Explorer", "Opened plan.md with
Notepad", "Ran run.bat"), Copy Path and Copy File ("Copied to
Clipboard"), and Move to Recycle Bin, once the user confirmed "Move
“plan.md” to the Recycle Bin?" (never remembered) ("Moved to Recycle
Bin"). What fails stays on screen in the status line ("Could not
open todo.txt: it no longer exists"). Opening and running go through the
launcher's link opener (`LinkOpener::open_file`), the others through its
system ([`crate::system`](../crates/pane-core/src/system.rs): reveal, open
with an application, the clipboard, the Recycle Bin), so tests record
them all.

## Checking a file again

Before every action, off the window's thread, the host checks the file
again (`FileAccess::checked_file`):

1. The id is in the package's latest listing, and that listing's folder is
   still the package's grant.
2. The path is not a network path (Windows, before any file system call).
3. `symlink_metadata`: a regular file, not a link ("it is now a link"),
   still there ("it no longer exists").
4. Its canonical path is inside the grant's canonical path (a folder above
   it replaced by a link outside: "it is no longer inside the granted
   folder").
5. For Open (Enter on a document) only: it is not a program or script
   ([`files::runs_as_program`](../crates/pane-core/src/files.rs)), on
   every system: the Windows types `exe bat cmd com lnk js jse vbs vbe
   wsf wsh hta msi msp scr pif ps1 cpl reg url`, the macOS types `app
   command tool terminal workflow` and anything inside an `.app` bundle,
   `.desktop` files, and on macOS and Linux any file with an executable
   bit ("it is a program or script, which opening would run"). A document
   that became a program since it was listed is refused so. Run, Show in Explorer,
   Open With…, the copies and the Recycle Bin act on a program as on any
   file.

Only then is the checked canonical path acted on, with the host's name for
the file in what the status says; root search keeps its query.

The window's opener, `pane::SystemLinks`, runs the handler the `open` crate
(5.4.4) names for the system, without a shell:

| System | Handler | A missing handler |
| --- | --- | --- |
| Linux | `xdg-open <path>`, else `gio open`, `gnome-open`, `kde-open` | none installed: "no program to open this kind of file is installed"; xdg-open finding none (status 3): "no program to open this kind of file is set up"; the program failing (status 4): "the program for this kind of file refused or failed to open it" |
| macOS | `/usr/bin/open -- <path>` (Launch Services) | its failure status |
| Windows | PowerShell with the path in an environment variable (not on its command line), which opens an existing path with `Invoke-Item -LiteralPath`; else `explorer.exe` | its failure status; an unassociated type may show the system's "Open with" dialog |

A handler still running after three seconds counts as having opened the
file. Tests replace the opener with a recording fake; a launcher given no
opener says "this Pane has no handler for files".

## For authors

A package that lists a granted folder sets `"folderAccess": true` in
`pane.json`, and its commands call `list-folder()` and answer `open-file`
results with the ids, or command search results whose `file` is the id
(`SearchResult { file: Some(id), .. }` in Rust, `{ id, title, file }` in
JavaScript and TypeScript), in Rust, JavaScript and TypeScript alike
([author guide](../guests/README.md#files-of-a-granted-folder)):
`pane_guest::files::list_folder()` and `RootAction::OpenFile(id)` in Rust;
`listFolder()` from `"pane:extension/files@0.1.0"` and
`{ tag: "open-file", val: id }` in JavaScript and TypeScript, whose
`package.json` sets `"pane": { "files": true }` so that only such a
component imports the interface. The samples
[`guests/sample-files-js`](../guests/sample-files-js) and
[`guests/sample-files-ts`](../guests/sample-files-ts) do what Files does,
in root search and in their own field (`"pane": { "search": true }`), with
a simpler match (every word in the name), and give the same answers.

## Checks

- Launcher public interface
  ([`crates/pane-core/tests/files.rs`](../crates/pane-core/tests/files.rs)),
  with the real Files guest, a recording opener and a controlled fixture
  folder named "Pane files — ñ" holding "Résumé plan ü.txt", a subfolder, a
  hidden file and a hidden folder: nothing found before a folder is
  granted; granting through Pane's row; the grant recorded in
  `folders.json` by identity (parsed; both paths canonicalized) and not in
  the extension's settings; a hidden folder refused, keeping the grant;
  "Stop sharing" removing the files; the files found by name, then by
  subfolder, case ignored, hidden ones not; Enter opening the Unicode file
  (the opener's path and the fixture's, both resolved); file results after
  a command whose title matches; a folder gone since it was granted
  explained as a row; at Enter, a `.bat` file and an executable script
  revealed (through a recording system) rather than opened, a removed file, a file replaced by a link and a folder above it
  replaced by a link outside the grant each refused, and nothing reaching
  the opener; the grant kept across a restart, hidden while disabled, and
  forgotten by uninstalling; the JavaScript and TypeScript samples; and
  the `faulty` fixture naming `/etc/hosts` itself (not listed) and giving
  each listed file the title "harmless.txt" (shown with the real names,
  and opened as such).
- Grants and the policy, on fixture folders: the root, the home folder, a
  hidden folder, a file, a missing and a relative path refused, and UNC
  forms on Windows; breadth-first name order; links not listed or followed
  (macOS and Linux); hidden attributes and junctions (Windows, written, not
  run); each limit; entries counted while a 50-file folder is read (11
  checks for a limit of 10); a cancelled listing; an unreadable subfolder
  making the listing partial (macOS and Linux, unless run as root); program
  and script types told from documents.
- The listing, with a folder lister the test holds up (same file): while
  the Files folder is listing, the applications' result and the
  calculator's answer are shown, then the files once it is released; one
  listing per visit, later keystrokes filtering it; a new query waiting for
  the same listing, the older search ending at once and only the newer
  query's file shown; leaving root search stopping the listing and the next
  visit listing again; disabling stopping it, with nothing arriving once it
  has returned (waited on, not slept); a new grant stopping the old
  listing. Unit tests in
  [`pane_core::files`](../crates/pane-core/src/files.rs) hold a left
  visit's listing until the next visit's is queued and waited for (its late
  end must not end that wait), and let a listing end before the wait for it
  is made (the wait must end at once); each race failed a run of the tests
  above once (#29).
- Search Files and the file actions (#150), through the launcher
  ([`crates/pane-core/tests/file_actions.rs`](../crates/pane-core/tests/file_actions.rs)),
  for Files and the JavaScript and TypeScript samples alike, with a
  recording opener, system and window: the files listed in the command's
  own field, a newer text stopping the search waiting for the listing; a
  document's six actions, each acting through the fakes and closing the
  window (Copy Path and Copy File with their HUD), Open With… listing the
  installed applications by name, Move to Recycle Bin confirmed first; a
  program revealed by Enter, Ctrl+Enter its Open With… submenu, only Run
  running it, in Search Files and in root search; the command keeping its
  id. In the window
  ([`crates/pane/tests/file_actions.rs`](../crates/pane/tests/file_actions.rs)),
  with real keys: Enter and Ctrl+Enter on a document and on a program.
- Native GUI smokes, one phase per system (screenshots 220 to 223), with a
  data folder of its own and a fixture folder "Pane smoke files" (spaces)
  holding "Résumé plan ü.txt" (non-ASCII) and an executable script or batch
  file: install Files, Enter on "Choose folder…" (the debug build takes
  the folder from `PANE_TEST_CHOOSE_FOLDER` instead of showing the picker),
  type "plan", check the selected row, Enter; then type "runner" and Enter,
  which must be refused, with the script neither handed over nor run. On
  Linux the file opens through the real `xdg-open` outside any desktop
  session, whose only handler for plain text is a script of the smoke's
  that records the path (XDG_CONFIG_HOME, XDG_DATA_HOME and BROWSER of the
  smoke's own), and the recorded path, resolved, must be the fixture
  file's; it ran on Linux X11 on 2026-09-28
  ([evidence](platforms/linux.md#files-29)). On macOS and Windows the debug
  build's `PANE_TEST_OPEN_FILE_LOG` makes the opener record the path
  instead of running `open` or `Invoke-Item` (which could open the user's
  own program or show the "Open with" dialog), and the recorded path must
  be the fixture file's; that is written but has not run yet.

## Limits

- One folder per package; no whole-disk index, other scopes, content search,
  file watching, icons, previews, recent files or ranking beyond name then
  path.
- Each visit of root search lists the folder again; a folder at the limits
  costs up to 20,000 entries per visit, and files beyond the limits are not
  found.
- Mapped network drives (Windows) and network mounts (macOS, Linux) are not
  refused; a hung one holds up only its package's worker.
- Programs and scripts are revealed by Enter; only their Run action runs
  them, and it asks nothing more ("Instant file search", #126, settles the
  rest).
- A handler slow to fail (over three seconds) is reported as having opened
  the file.
- The positive native open ran only on Linux X11 (xdg-open with a recording
  handler); on macOS and Windows the real handler is not run by the smoke.
- Screen reader behaviour is unverified, as for all of root search.

## The file index (#126, in progress)

"Instant file search" ([#126](https://github.com/hoangvu12/pane/issues/126),
[ADR 0034](adr/0034-file-search-indexes-the-users-home-folder.md)) replaces
the granted folder's listing for file search with an index of the home
folder that the host keeps. Its first slice
([#174](https://github.com/hoangvu12/pane/issues/174)) is in
[`pane_core::file_index`](../crates/pane-core/src/file_index.rs); nothing
uses it yet (the coordinator, the change sources and root search's rows are
[#175](https://github.com/hoangvu12/pane/issues/175)), so everything above
still describes what Files does.

- **The walker** (`file_index/walker.rs`): every folder under the roots the
  rules admit is listed once, by up to 8 threads of its own at background
  priority (`file_index/priority.rs`: background mode and EcoQoS on
  Windows, the background QoS class on macOS, nice 19 and the idle I/O
  class on Linux). On Windows a folder is read with
  `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`, which gives each
  entry's attributes, size, times and NTFS file id in one call per 64 KB;
  elsewhere with `read_dir` and each entry's own metadata. Links and
  junctions (reparse points whose tag names another file) are indexed as
  entries and never followed; cloud-file placeholders are listed like any
  entry, so nothing is downloaded. A folder that cannot be read is indexed,
  counted and named; a walk stops at 5 million entries.
- **The rules** (`file_index/scope.rs`, `ScopeRules::for_home`): hidden
  entries (a leading `.`; the hidden or system attribute on Windows);
  `.gitignore` inside a Git repository, `.ignore` anywhere, the
  repository's `.git/info/exclude` and the global Git ignore file, matched
  by ripgrep's `ignore` crate as Git matches them; `node_modules`, folders
  named `tmp`, `temp`, `cache` or `caches`, `*.tmp` and `*.temp`; the home
  folder's `AppData` (Windows) or `Library` (macOS); network and FAT or
  exFAT volumes mounted under a root (Linux); and always the system's
  recycle and setup folders, folders tagged with `CACHEDIR.TAG` and Pane's
  own folders. Each but the last is a switch, and the user's folders and
  `.gitignore`-style patterns add to them. `Scope::admits` applies the same
  rules to one path, reading the ignore files above it, for changes.
- **The engine** (`file_index/store.rs`), in the shape of `minidex`: a
  memory table of recent changes, logged first to a write-ahead log
  (`file_index/wal.rs`, records with a CRC, a torn tail dropped); immutable
  segments (`file_index/segment.rs`) holding the entries sorted by path,
  front-coded in blocks of 16, each with a fixed 4-byte hint (day modified,
  depth, kind) and the postings of its terms, whose dictionary is an `fst`
  map read through a memory map; prefix tombstones hiding a deleted or
  renamed folder's entries in older segments; and a merge of all segments
  into one, which drops superseded versions, deletions and what tombstones
  hide, once there are more than 8. A first walk writes segments directly,
  without the log, and merges them at the end. Terms are the folded words
  (case and accents ignored, split at camel case and digits) of the name
  and, separately, of the folders below the root. A query reads, per
  segment, at most 1,000 candidates matching every word, those with every
  word in the name first, ranked by their hints, then scores them as #126's
  "Matching and ranking" describes (`file_index/text.rs`). Keys are the
  path's exact bytes (WTF-8 on Windows), so a name that is not valid
  Unicode is shown with replacement characters and still opened exactly.
  Each entry keeps its kind, size, modified time, file id and volume.
- **Its folder**: `index.json` (the format version, the live segments, and
  Pane's record: whether a first walk finished, and each volume's journal
  cursor), `<n>.seg`, `<n>.wal`, and `lock`, which the index holds locked,
  so a second Pane on the same cache folder gets `IndexError::InUse`. The
  folder is mode 0700 and the files 0600 on macOS and Linux; on Windows
  they inherit `%LOCALAPPDATA%`'s permissions (a protected DACL as
  `credentials.json` has is #175's). An index of another `FORMAT_VERSION`,
  or one that cannot be read, is deleted and rebuilt, never read. The log
  is handed to the system after each batch but not flushed to the disk: a
  power loss can lose the last changes, which the catch-up finds again from
  the file system's records, since the cursors are saved only with
  segments.

### The engine: Pane's own, not the `minidex` crate

Decided on reading `minidex` 0.38.0's source (MIT, by Joao Neves, the
crate Raycast uses), before the measurement, which is to confirm it:

- **Format.** #126 requires Pane's index format to be Pane's, versioned,
  and rebuilt on any other version. `minidex` writes its own files, and at
  0.38 it is pre-1.0 and changing quickly (each release may change them);
  Pane would version a format it does not control.
- **What an entry keeps.** `minidex` keys an entry by
  `path.to_string_lossy()`, so a name that is not valid Unicode cannot be
  opened from the index; and it keeps kind, modified and accessed times, a
  category and a volume type, but no size and no file id. The NTFS
  catch-up resolves journal records by file id and parent folder id, and
  Search Files shows sizes, so both would need a second store beside it.
- **Tombstones.** Its prefix tombstones compare paths in ASCII lower case,
  so deleting `~/Docs` would also hide `~/docs` on a case-sensitive file
  system (Linux, case-sensitive APFS).
- **Threads and priority.** It starts its own flush, compaction and
  recovery threads with its own priority policy; #126 wants Pane's
  coordinator to own background priority, pausing for sleep and "no
  periodic work while nothing changes".
- **Matching.** Its tokenizer and candidate pruning are fixed; #126 wants
  folding consistent with root search's and weights tuned against Pane's
  fixtures, with name and folder matches told apart.
- **Dependencies.** It brings `zstd` (a C library built by `cc`), `fs4`,
  `arc-swap`, `thiserror` and `log`; Pane's own engine adds only `fst`
  (no dependencies of its own), `memmap2` (already in the tree) and, for
  the rules rather than the engine, `ignore`.
- **The first index.** Its inserts all go through its log; Pane's first
  walk writes segments directly.

What Pane takes from it is the shape: segments with an FST dictionary, a
memory table and log, tombstones, compaction, and a compact per-entry hint
for pruning candidates before reading them.

**The measurement that confirms it** is the benchmark below on the machine
where Raycast was measured, against the home folder: a first index in less
than 12.9 s (aiming for half), an index smaller than 61.8 MB, a query's
95th percentile under 10 ms, and a changed file visible within 10 ms. If
Pane's engine misses the size or query target, `minidex` 0.38 is measured
on the same tree behind the same `FileIndex` calls before Pane's is tuned
further; the result and the decision are recorded in #174's results
comment.

### The benchmark

`cargo xtask file-index-bench [options]` builds
[`crates/pane-core/examples/file_index_bench.rs`](../crates/pane-core/examples/file_index_bench.rs)
in release and runs it; it runs on demand, never in CI. By default it
generates a home-shaped tree of about 450,000 indexable entries (9 files
per folder, paths about 8 folders deep, accented names, and, left out by
the rules, hidden folders, `node_modules` and Git repositories with ignored
`build` folders and logs) in the system's temporary folder, reused by later
runs; `--home` indexes the real home folder instead (it writes nothing
there), `--root <folder>` another folder. It prints, as a table against
#126's targets and Raycast's numbers:

- the first index, run 1 and the median of the warm runs (`--runs`,
  default 3), with the walk's own time; run 1 is cold only when it is the
  first since a restart, or after `--drop-caches` where the system allows
  it (Linux as root, macOS with `sudo purge`; Windows has no way without
  administrator rights);
- the index on disk, and per entry;
- query latency over a fixed set (`--queries`, default 1,000: whole names,
  3-letter prefixes, words, folder and name words, one letter), the first
  pass after opening and a warm pass, with the 50th, 95th and 99th
  percentiles and per kind;
- a changed file re-indexed: written, read, applied and found by a query,
  100 times;
- on Windows with the generated tree, the catch-up after 10,000 files
  created: the journal read from a saved cursor, resolved, looked at,
  applied and found;
- the time to open the index at start, the private memory it adds while
  idle, and the peak memory while indexing.

### The NTFS change journal without administrator rights

`file_index::read_journal` opens the volume's root folder (`C:\`) for
reading attributes only, asks for the journal with
`FSCTL_QUERY_USN_JOURNAL`, and reads it from a saved cursor with
`FSCTL_READ_UNPRIVILEGED_USN_JOURNAL` (Windows 10 1709 and later), never
creating or resizing a journal. It answers the records and the new cursor,
or why a walk is needed: the journal was recreated (another id), the
records were already discarded, there are more than a walk would cost, the
volume keeps none (FAT, exFAT, network shares), or the system refused.
`file_index::resolve` turns the records into paths through the folder ids
the index holds (each folder's NTFS file id, read by the walk), following
renamed folders; a record in a folder the index does not hold resolves to
nothing. `catch_up_changes` then looks at each path and applies the scope.
The test
`file_index::journal::tests::the_journal_is_read_without_administrator_rights_and_resolves_to_paths`
indexes a temporary folder, makes changes (a file created, one deleted, a
folder renamed, a file in it created), and reads them back from the
cursor; it has not run yet, and is the evidence #174 asks for when run
without administrator rights (CI's Windows runner is an administrator).

The other systems' equivalents and their limits, for #175:

- **macOS**: FSEvents replays a volume's history from a saved event id
  without any permission beyond reading the folders (and the privacy
  prompts for Desktop, Documents and Downloads); history can be purged or
  coalesced, which FSEvents reports per folder (`MustScanSubDirs`), and a
  changed volume UUID means walking again.
- **Linux**: no history is readable without privileges (fanotify needs
  `CAP_SYS_ADMIN`); the catch-up is a walk that reads only folders whose
  modified time changed, and live changes come from inotify, one watch per
  folder, up to `fs.inotify.max_user_watches`.
