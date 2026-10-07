# Files

File search: the user types a file's or a folder's name into
[root search](root-search.md), or into the **Search Files** command (#150),
and opens, reveals, copies or recycles it through the file actions Pane
performs itself. Pane's host keeps an index of the names of the files and
folders under the user's home folder (the **file index**), caught up at
start from what the file system recorded while Pane was not running and
kept current while it runs, so a file is found as quickly as a command
([#126](https://github.com/hoangvu12/pane/issues/126),
[ADR 0034](adr/0034-file-search-indexes-the-users-home-folder.md); built by
[#174](https://github.com/hoangvu12/pane/issues/174) and
[#175](https://github.com/hoangvu12/pane/issues/175)). The feature is a
**default extension**, Files, which the user can disable like any package;
while no enabled package uses the index, Pane neither indexes nor watches
anything.

Until #175, file search found only the files of one folder the user granted
Files (#29, [ADR 0017](adr/0017-host-lists-a-granted-folder-for-an-extension.md)).
That capability stays for other packages, unchanged
([The granted folder](#the-granted-folder)); Files no longer uses it.

Not yet (later tickets of #126 and #161): the File search page in Settings
and the safety valves (churn quarantine, the low-disk floor, pausing for
sleep, folders that hang) are [#176](https://github.com/hoangvu12/pane/issues/176);
Search Files' recent files, kind dropdown, detail and paging are
[#177](https://github.com/hoangvu12/pane/issues/177).

## Where it lives

A pure WASI 0.3 guest cannot read the user's folders (its WASI context
preopens none) or open a file, and the walk and watching must never run on
the extension runtime's thread, so the index is the host's, in `pane-core`,
and one index serves every package that uses it:

- **The index**, [`pane_core::file_index`](../crates/pane-core/src/file_index.rs):
  the walker, the engine, the scope and the NTFS change journal (#174,
  [below](#the-engine-and-the-walker)); the reconciling walk
  (`file_index/reconcile.rs`); one change source per system behind the
  `ChangeSource` trait (`file_index/changes.rs` and `changes/{ntfs,macos,linux}.rs`);
  the coordinator, which opens, catches up, walks and watches the index,
  gives each entry an id and checks it again (`file_index/indexer.rs`,
  `Indexer`); and the host side of `pane:extension/file-index`
  (`file_index/host.rs`).
- **The launcher's side**, [`launcher/file_search.rs`](../crates/pane-core/src/launcher/file_search.rs):
  which packages use the index, root search's Files section and its
  "Search Files for “…”" row, the system icons, deleting the index with the
  last package that used it, and the calls the File search page (#176) and
  Search Files (#177) build on (`Launcher::file_indexer`,
  `file_index_status`, `file_search_rules`, `set_file_search_rules`,
  `rebuild_file_index`). The rows and their actions are
  [`launcher/files.rs`](../crates/pane-core/src/launcher/files.rs) and
  [`launcher/own_actions.rs`](../crates/pane-core/src/launcher/own_actions.rs).
- **The host interface**, `pane:extension/file-index`
  ([`wit/file-index.wit`](../wit/file-index.wit)): `search` and `status`,
  for any package that declares `"fileIndex": true` ([For authors](#for-authors)).
- **The default extension**, [`guests/files`](../guests/files) (Rust),
  package [`guests/packages/files`](../guests/packages/files) (0.7.0,
  `"fileIndex": true`): its one command, Search Files (id `files`,
  `"search": true` and `"rootResults": true`), asks the index for the
  query and answers with the entries' ids. It is not a
  [root provider](root-search.md#root-providers): it has a row and a
  screen of its own.
- **The window**, [`crates/pane/src/main.rs`](../crates/pane/src/main.rs):
  `Launcher::with_file_index(IndexerConfig::native(cache, home, own))`
  with Pane's cache folder, the home folder (`USERPROFILE` on Windows,
  `HOME` elsewhere) and Pane's data folder, which is never indexed.

Acquiring the package automatically at setup is
[#51](https://github.com/hoangvu12/pane/issues/51) to
[#53](https://github.com/hoangvu12/pane/issues/53); until then it is
installed from its folder like the other default extensions
(`pane --install target/guests/packages/files`).

## The file index

### When it runs

A package declares in `pane.json` that it uses the index:
`"fileIndex": true`. The index is opened, caught up and watched only while
at least one such package is enabled and not paused
(`Launcher::sync_file_index`, after every change of the packages). When the
last one is disabled or paused, watching stops at once and the index stays
on disk; enabling one again catches it up from where it stopped. Uninstalling
the last one deletes the index. This keeps lazy activation
([ADR 0005](adr/0005-lazy-activation-and-managed-dependencies.md)) without
running guest code to learn it.

At start Pane catches the index up (it is cheap); a first full walk, or a
reconciling walk the catch-up asks for, waits until the launcher is first
shown (`WindowPresence::Shown`) or 60 seconds after start
(`file_index::FIRST_WALK_DELAY`), whichever comes first. Meanwhile, and
while any walk runs, searches answer from what is indexed so far.

One thread of its own per activation ("pane-file-index"), at background
priority (background mode and EcoQoS on Windows, the background QoS class
on macOS, nice 19 and the idle I/O class on Linux), does all the writing;
the walker's threads run at the same priority. A query never runs at
background priority and never waits for the coordinator.

### Where it is kept

In Pane's cache folder, since it can always be rebuilt:
`%LOCALAPPDATA%\Pane\cache\file-index` on Windows,
`~/Library/Caches/Pane/file-index` on macOS and
`$XDG_CACHE_HOME/pane/file-index` on Linux (`file_index::INDEX_DIR`). The
folder is readable by the user only: mode 0700 (files 0600) on macOS and
Linux, and on Windows a protected DACL for the user and SYSTEM, inherited
by its files, as `credentials.json` is written. It carries a format version;
an index of another version, or one that cannot be read, is deleted and
rebuilt, never read. The folder is locked: a second Pane on the same cache
folder does not index, and its status says "Another Pane is using file
search on this computer". It is never sent anywhere.

The index records with itself whether a first walk finished, each volume's
cursor into the system's change records, and the roots and rules it was
built under, so that changing the rules (even while file search is off) is
applied the next time it opens.

### The index scope

The **roots** are the home folder (`%USERPROFILE%` on Windows, `$HOME` on
macOS and Linux) and the folders the user adds. The **rules**, applied by
the walker, the reconciling walk and every live change alike
([the walker's rules](#the-engine-and-the-walker)):

- Always left out: Pane's own data and cache folders and the index itself;
  the system's recycle and setup folders (`$RECYCLE.BIN`,
  `System Volume Information` and the like); folders tagged with
  `CACHEDIR.TAG`.
- Left out by default, each a switch: hidden entries (a leading `.`; the
  hidden or system attribute on Windows); what `.gitignore` (inside a Git
  repository), `.ignore`, `.git/info/exclude` and the global Git ignore file
  exclude, read as Git reads them without running `git`; `node_modules`,
  folders named `tmp`, `temp`, `cache` or `caches`, `*.tmp` and `*.temp`;
  the home folder's `AppData` (Windows) or `Library` (macOS); network and
  removable volumes mounted under a root.
- The user's own: added roots, excluded folders and excluded patterns (in
  `.gitignore` syntax).

The user's rules are Pane's own record, not extension data:
`file-search.json` beside `installed.json`
(`{"version": 1, "rules": {"addedRoots": [], "excludedFolders": [],
"excludedPatterns": [], "includeHidden": false, "useIgnoreFiles": true,
"defaultExclusions": true, "includeOtherVolumes": false}}`,
`file_index::UserRules`). `Launcher::set_file_search_rules` records and
applies them without a restart: a removed root's entries go and an added
root alone is walked; any other change walks every root again. The File
search page that changes them is #176.

Entries are files and folders, named as the system names them (a name that
is not valid Unicode is shown with replacement characters and opened by
its exact name). Links and junctions are indexed as entries and never
followed. Online-only files (OneDrive, iCloud Drive, File Provider) are
indexed from their folder's listing alone, so nothing is downloaded. A
folder that cannot be read is indexed and counted, its contents not. A
root the user added that is away (an unplugged drive) keeps its entries,
hidden from searches until it is back (looked at no more than every two
seconds).

### Catching up and watching

| | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Catch-up at start | The NTFS change journal of each root's volume, read without administrator rights from the saved cursor ([below](#the-ntfs-change-journal-without-administrator-rights)), resolved through the folder ids the index holds | FSEvents' history, replayed by one stream over the roots from the saved event id, per volume (by its FSEvents UUID) | A reconciling walk |
| Live changes | `ReadDirectoryChangesW` on each root (the `notify` crate) | The same FSEvents stream | inotify, one watch per indexed folder, shallowest first |
| When the records cannot be used | A recreated journal, discarded records, more than a million records, a volume without a journal (FAT, exFAT, a network share) or a refused read: the volume's roots are reconciled | A new volume UUID: its roots are reconciled; a folder FSEvents asks to rescan (history purged or coalesced, events dropped) is reconciled alone; wrapped event ids reconcile every root | The folders past the watch limit (`fs.inotify.max_user_watches`) are reconciled every 5 minutes, and counted in the status |

A **reconciling walk** (`file_index::reconcile`) compares the index's own
folders with the disk and reads again only a folder whose modified time
changed (a folder's time changes when an entry is added, removed or renamed
in it); a folder new to the index is walked whole, and a folder gone takes
everything under it out. A watcher's overflow reconciles the root it
concerns.

Live changes are gathered for about 100 ms (`file_index::SETTLE`) and
applied together: each path reported is looked at again (indexed if it
exists and the rules admit it, removed otherwise, a new folder walked
whole), so a change is visible to queries well within a second of the
system reporting it. While nothing changes, nothing runs, except Linux's
reconciliation of the folders it cannot watch. The status
(`Indexer::status`, `Launcher::file_index_status`, and
`pane:extension/file-index`'s `status`) says whether the index is off,
building (and how many entries the walk found so far), current or stopped
and why, how it last caught up (`CaughtUpBy`: the journal, the event
history, a reconciling walk or a full walk) and when, and how many folders
could not be read or are not watched.

## In root search

Files answers root search through `root-results`, now from the index: its
call returns at once from the host and never waits for a walk, so a busy
disk holds up no other result. A query of one character or more lists:

- at most **5 file rows** per command (`ROOT_FILE_ROWS`), the index's best
  by its own scoring: an exact name above an exact stem, a name prefix, a
  word start and then a match in the folders; case and accents ignored
  ("resume" finds `Résumé.pdf`); the folders' words count ("invoices march"
  finds `Invoices 2026/march.pdf`); newer entries a little higher; folders
  found as well as files;
- then **"Search Files for “<query>”"**, which opens Search Files with the
  query typed in its own field and searched at once.

They are listed **after** the commands, applications and quicklinks found
by title, under the section "Files". Each row is the host's, whatever the
extension's result says: its title is the entry's own name, its subtitle
its folder (below the home folder as `~/…`), its icon the system's icon for
the path (#142, a document's or folder's outline until it is loaded), and
its kind **File**, or **Folder** for a folder. A blank query lists no
files. A result naming an id the index did not give the package is not
listed. No use of a file row is recorded for learning (ADR 0030).

## Search Files

Search Files (#150) is a view command whose search field is the
launcher's own ([command search](command-search.md)): Enter on its row in
root search opens it with the field empty above its own list ("What is
searched", whose subtitle says what the index is doing: "Indexing your
files… 1204 found so far", "53210 files and folders of your home folder
are indexed", "File search is off (…)"), and typing lists what the index
finds (at most 50), each titled with its own name and folder. Opened from
root search's "Search Files for “plan”" row, the field holds "plan" and
lists its results at once. A command may set both `"search"` and
`"rootResults"` (#150): root search asks it, and so does its own field.
Recent files before typing, the kind dropdown, the detail and paging
are #177's.

## The file actions

Each file, in Search Files and in root search's file results alike, has
actions Pane performs itself, without calling the extension, as an item of
a command's list has them: Enter runs the first, Ctrl+Enter the second,
Ctrl+Shift+Enter the third, and the Actions panel (Ctrl+K) lists them all.

| A document | A program or script | A folder |
| --- | --- | --- |
| **Open** (Enter): the system's handler for its type | **Show in Explorer** (Enter) | **Open** (Enter): the file manager |
| **Show in Explorer** (Ctrl+Enter): selected in the file manager | **Open With…** (Ctrl+Enter) | **Show in Explorer** (Ctrl+Enter) |
| **Open With…**: a submenu of the installed applications, by name | **Run** (Ctrl+Shift+Enter): the system's handler, which runs it | **Copy Path** |
| **Copy Path**: its path, as text | **Copy Path** | **Copy File** |
| **Copy File**: the file, as the file manager copies it | **Copy File** | **Move to Recycle Bin** |
| **Move to Recycle Bin** (destructive): after a confirmation | **Move to Recycle Bin** | |

File search's own Enter never runs a program by accident
([ADR 0037](adr/0037-a-command-declares-its-mode-and-host-functions-decide-what-happens-after-it-runs.md)):
a file that would run a program when opened ([below](#opening)) is shown
in the file manager, and only its explicit **Run** runs it; choosing Run is
the confirmation, so nothing more is asked. (On macOS the file manager is
Finder, so the action is "Show in Finder", elsewhere "Show in File
Manager"; the Recycle Bin is the Trash outside Windows.)

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

## Opening

A row names its entry by the id the index gave the package that found it
(`i<generation>-<n>`, the newest 10,000 kept per package); an id of an
earlier index (closed, rebuilt or deleted since) or of another package is
not known ("Pane no longer knows it; search again"). Before every action,
off the window's thread, the host checks the entry again
(`Indexer::checked`):

1. The path is not a network path (Windows, before any file system call).
2. `symlink_metadata`: still there ("it no longer exists"), of the kind
   indexed ("it is now a folder", "it is now a file"), not a link where a
   file or folder was indexed ("it is now a link"; a link itself is never
   opened: "it is a link, which Pane does not follow").
3. It is still in the index scope ("it is no longer in the folders file
   search covers"), and its canonical path is under a root's (a folder
   above it replaced by a link outside: "it is no longer inside the folders
   file search covers").
4. Whether it is a **program**: any entry of the types below, the same on
   every system, or on macOS and Linux a file with an executable bit
   ([`files::runs_as_program`](../crates/pane-core/src/files.rs)): the
   Windows types `exe bat cmd com lnk js jse vbs vbe wsf wsh hta msi msp
   scr pif ps1 cpl reg url`, the macOS types `app command tool terminal
   workflow` and anything inside an `.app` bundle, and `.desktop` files.
   A row already knows from the name whether it is one; the executable bit
   is told at this check, so a document that became a program since it was
   found is shown in the file manager rather than opened.

Only then is the checked canonical path acted on: Enter hands a document or
a folder to the system's handler, and shows a program in the file manager;
Run, Show in Explorer, Open With…, the copies and the Recycle Bin act on a
program as on any file.

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

A package that searches the index sets `"fileIndex": true` in `pane.json`;
its commands call `search(query, options)` and `status()` of
`pane:extension/file-index`, and answer `open-file` results with the
entries' ids, or command search results whose `file` is the id
(`SearchResult { file: Some(id), .. }` in Rust, `{ id, title, file }` in
JavaScript and TypeScript) ([author guide](../guests/README.md#panes-file-index)):

- `search` answers at once from what is indexed, never waiting for a walk:
  the entries matching the query (a blank query: the most recently
  modified), filtered by kind (file, folder, link) and category
  (documents, images, audio, video, archives, applications, told from the
  name's extension by one table on every system), sorted by relevance or
  modified time, paged by `limit` (at most 200 per call) and `offset`. Each
  entry carries its id, absolute path (text, for showing and copying),
  name, folder for people, kind, whether opening it would run a program,
  size, modified time and volume. A package that does not declare
  `"fileIndex": true` is refused.
- `status` answers the state (off, building, current, stopped), the
  entries indexed, the entries the walk in progress found and why.

Rust: `pane_guest::file_index::{search, status}` and
`RootAction::OpenFile(entry.id)`; JavaScript and TypeScript: `search` and
`status` from `"pane:extension/file-index@0.1.0"`
([`guests/js/file-index.d.ts`](../guests/js/file-index.d.ts); WIT's `u64`
numbers are `bigint`), whose `package.json` sets
`"pane": { "fileIndex": true }` so that only such a component imports the
interface, and `{ tag: "open-file", val: entry.id }`. The samples
[`guests/sample-files`](../guests/sample-files),
[`guests/sample-files-js`](../guests/sample-files-js) and
[`guests/sample-files-ts`](../guests/sample-files-ts) do what Files does,
in root search and in their own field, and give the same answers. A command
that chooses to open or run an entry explicitly may use the host's open
functions of ADR 0037 with the path; that is its own explicit action.

## The granted folder

The capability file search used before #175 stays, unchanged, for a
package that wants an exhaustive listing of one folder the user chooses,
including a folder the index leaves out
([ADR 0017](adr/0017-host-lists-a-granted-folder-for-an-extension.md),
superseded for file search by ADR 0034). Files no longer declares it; the
test fixture [`guests/fixtures/folder-files`](../guests/fixtures/folder-files),
what Files was before, keeps it covered. Its own `open-file` results keep
ADR 0017's refusal to open programs and scripts, since such a package never
offered a Run action.

- **The grant**, in the core: a package declaring `"folderAccess": true` in
  `pane.json` gets Pane's own "Choose folder…" row; Pane checks the folder
  and records it in its own `folders.json`
  ([`pane_core::files`](../crates/pane-core/src/files.rs),
  [`launcher/files.rs`](../crates/pane-core/src/launcher/files.rs)).
- **The listing**, in the core: `pane:extension/files`
  ([`wit/files.wit`](../wit/files.wit)), `list-folder()` without a path,
  answered at once from the listing Pane makes on the package's own worker.
- **The file actions** are the ones above, checked again against the grant
  (`FileAccess::checked_file`): the id in the package's latest listing,
  that listing's folder still its grant, not a network path, a regular
  file and not a link, its canonical path inside the grant's, and for Open
  not a program ("it is a program or script, which opening would run").

### Granting the folder

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

### The scan policy

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

#### When it is listed

`list-folder()` never waits. The first call in a visit of root search
starts a listing on the package's **own worker thread** (one per package,
started with its first listing) and answers `listing`; the extension
answers no files yet. The worker waits 100 ms (`files::DEBOUNCE`) before it
starts, and takes only the newest request. The listing is then **kept for
the visit**: every later keystroke gets it at once, and only filters it. It
is dropped when root search is left (a command, Settings › Extensions, a
preview, a restart of the visit) and when the grant changes, so the next
visit lists the folder again; this listing keeps no index and watches
nothing.

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
waiting (the package is then asked again at once); a listing of a visit already
left that stops late does not end that wait.

**A hung folder** (an unresponsive disk or a network mount the host could
not tell apart) holds only its package's worker: no new thread is started
for later requests, which wait (only the newest is kept), other extensions
are unaffected, and the package lists nothing until the listing returns. With
UNC paths refused this should be rare; mapped network drives on Windows and
network mounts on macOS and Linux are not detected.

## The engine and the walker

The measured first slice of #126 ([#174](https://github.com/hoangvu12/pane/issues/174)),
which the coordinator above builds on:

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
  Pane's record: whether a first walk finished, each volume's journal
  cursor, and the roots and rules it was built under), `<n>.seg`, `<n>.wal`, and `lock`, which the index holds locked,
  so a second Pane on the same cache folder gets `IndexError::InUse`. The
  folder is mode 0700 and the files 0600 on macOS and Linux; on Windows
  the folder is made with a protected DACL for the user and SYSTEM, which
  its files inherit (#175). An index of another `FORMAT_VERSION`,
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

The other systems' equivalents are FSEvents' history on macOS (no
permission beyond reading the folders, and the privacy prompts for Desktop,
Documents and Downloads) and, on Linux, where no history is readable
without privileges (fanotify needs `CAP_SYS_ADMIN`), the reconciling walk;
see [Catching up and watching](#catching-up-and-watching).

## Checks

Written with #175; none has run yet (tests run once every ticket of the
milestone is merged).

- **The index through the launcher**
  ([`crates/pane-core/tests/file_index.rs`](../crates/pane-core/tests/file_index.rs)),
  with the real Files guest, the real index and the system's own change
  source over a fixture folder standing for the home folder (the test names
  it) and a cache folder of the test's own, a recording opener and system,
  waiting on `Launcher::wait_for_file_index`: typing a file's name lists it
  under "Files", exact name first, at most five, then "Search Files for
  “plan”", with its folder `~/Documents`, and Enter opens it; hidden,
  ignored, `node_modules` and cache-tagged entries absent; a row from the
  first character and none for a blank query; case, accents and folder
  words; a folder found, reading Folder; file rows after a command found
  by title; a program, a script and a shortcut each revealed by Enter and
  never opened; an entry replaced by a folder since it was found explained,
  not opened; the first walk waiting until the launcher is shown;
  disabling Files stopping the index (kept on disk), a file written then
  found once it is enabled again, and uninstalling deleting the index; a
  restart catching up what changed while Pane was stopped by the system's
  records (`CaughtUpBy::Journal` on Windows, `EventHistory` on macOS,
  `ReconcilingWalk` on Linux), not a full walk; a second Pane on the same
  cache folder saying file search is in use; the user's rules recorded in
  `file-search.json` and applied without a restart.
- **The coordinator through the change source's seam**
  (`file_index::indexer` unit tests): a fake source the test scripts (what
  a catch-up finds) and drives (the live changes it reports), with the real
  index and walker: the first walk waiting to be shown; only a package
  that uses the index searching it, ids its own and checked again
  (removed, replaced by a folder, by a link); kinds, categories, sorting
  and pages; created, moved-in, renamed and deleted entries applied, a
  hidden one not; an overflow reconciling its folder; disabling stopping
  the watch and enabling again catching up without walking; records that
  are gone leading to a reconciling walk with the same result; a restart
  over the same cache folder; a second index refused; deleting the index
  forgetting its ids; the user's rules (hidden entries, an added root, an
  excluded folder, a removed root); an added root away and back; the
  folders a source cannot watch counted and reconciled every few minutes.
  `file_index::reconcile` and `store` unit tests cover reading only changed
  folders, a missing root keeping its entries, and a folder's children
  across segments and memory.
- **Per system**: the NTFS journal read without administrator rights
  (`file_index::journal` tests, #174); inotify reporting a change in a
  watched folder and a folder added later, and stopping when dropped
  (`changes::linux` tests); FSEvents replaying a change made before the
  stream opened from a saved event id, saying the history is done, then
  reporting a live change (`changes::macos` tests). The watch limit cannot
  be lowered without root; its fallback is driven through the seam above.
- **Search Files and the file actions**
  ([`crates/pane-core/tests/file_actions.rs`](../crates/pane-core/tests/file_actions.rs)),
  for Files and the Rust, JavaScript and TypeScript samples alike: the
  entries listed in the command's own field with their folders, a folder
  found; a document's six actions acting through the fakes and closing the
  window; Move to Recycle Bin confirmed first; a program revealed by Enter,
  Ctrl+Enter its Open With… submenu, only Run running it, in Search Files
  and in root search; root search's Files section with the system's icon
  and the row opening the command with the query typed; a folder opened in
  the file manager; a file created while Pane runs found through the
  system's watcher, and explained at Enter once deleted; the command
  keeping its id. In the window
  ([`crates/pane/tests/file_actions.rs`](../crates/pane/tests/file_actions.rs)),
  with real keys over the index: Enter and Ctrl+Enter on a document and on
  a program.
- **The granted folder**
  ([`crates/pane-core/tests/files.rs`](../crates/pane-core/tests/files.rs)),
  with the `folder-files` fixture: the grant, its record and its refusals,
  the scan policy, the listing per visit and its cancellation, the checks
  at Enter, as since #29.
- **Native GUI smokes**, one phase per system (screenshots 220 to 224): a
  debug build's `PANE_TEST_FILE_INDEX_HOME` names a fixture folder for the
  index to cover instead of the home folder (keeping the index in the
  phase's data folder); install Files, type "plan" and open the file found,
  then start Pane again, type "runner" and Enter, which reveals the script
  and runs nothing. Rewritten for the index with #175; not run yet.

## Limits

- The File search page, the safety valves (churn quarantine, the low-disk
  floor, pausing for sleep, a folder that hangs) and the migration of a
  folder granted to Files under #29 into the roots are #176; until then a
  folder that hangs holds up the walk, and nothing stops indexing on a
  full disk.
- Search Files has no recent files, kind dropdown, detail or paging yet
  (#177); `pane:extension/file-index` already offers them to extensions.
- Matching is by word prefix; a query inside a word ("port" in
  "report") is not found yet, and a query with `/` or `\` is matched word
  by word, not as path segments in order.
- macOS asks before Pane reads Desktop, Documents and Downloads; Pane does
  not explain it first yet, nor list a refused folder (#176's page).
- Linux's catch-up compares folders' modified times in whole seconds: a
  change within the second a folder was indexed is not seen until the
  folder changes again, and a file changed in place while Pane was stopped
  keeps its old size and time until it changes again while Pane runs.
- The roots and rules an index was built under are recorded as JSON; a
  root whose path is not valid Unicode cannot be recorded.
- The benchmark's numbers against Raycast's are not measured yet (#174).
- A handler slow to fail (over three seconds) is reported as having opened
  the file. Screen reader behaviour is unverified, as for all of root
  search.
