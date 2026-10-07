# Applications

Added for [#24](https://github.com/hoangvu12/pane/issues/24) (Windows),
[#25](https://github.com/hoangvu12/pane/issues/25) (macOS) and
[#26](https://github.com/hoangvu12/pane/issues/26) (Linux): US03, US05, US12,
US44; T01, T03, T22; contributions to G2 and G7, not claims that they pass.
Stable identities were added for [#169](https://github.com/hoangvu12/pane/issues/169),
the first slice of "Applications done properly"
([#124](https://github.com/hoangvu12/pane/issues/124)), the live list
for [#171](https://github.com/hoangvu12/pane/issues/171), its third, and the
Desktops, taskbar pins, internet and ClickOnce shortcuts as sources on
Windows for [#173](https://github.com/hoangvu12/pane/issues/173), its fifth.
Typing an installed application's name into root search lists it, ranked
with commands by title, and Enter (or a click) opens it. The feature is a
**default extension**, Applications, which the user can disable like any
package; the three systems share one design and differ only in where Pane
looks for applications and how it opens them.

## Where it lives

The spec and [ADR 0001](adr/0001-small-core.md) make app launching a
disableable default extension, not core; [ADR 0006](adr/0006-raycast-style-search-with-extension-providers.md)
keeps matching, ranking and dispatch in the core. A pure WASI 0.3 guest can
neither read the system's application folders (its WASI context has no
preopened folders) nor start a program. So the split, recorded in
[ADR 0015](adr/0015-host-finds-and-opens-applications-for-an-extension.md), is:

- **Host capability**, in the core: `pane:extension/applications`
  ([`wit/applications.wit`](../wit/applications.wit)), a host import any
  command may use: `installed()` lists the applications, `open(id)` opens one.
  Each system has an adapter behind one small trait
  ([`pane_core::applications`](../crates/pane-core/src/applications.rs),
  `Discovery`), which reports the shortcuts, bundles or desktop entries it
  finds; the host turns them into applications with a stable
  [identity](#identity) and keeps that list with the map from each id to
  what opens it, [current by itself](#live-list) while a package that asked
  for it runs. ADR 0038 ("The host keeps a live application list and an
  icon cache") amends ADR 0015's stateless adapter for this. Nothing is
  looked for until a guest asks.
- **Default extension**, [`guests/applications`](../guests/applications)
  (Rust), package [`guests/packages/applications`](../guests/packages/applications):
  its command, "Applications", is a
  [root provider](root-search.md#root-providers) (#164): it has no row of
  its own and supplies the applications to root search as
  [indexed results](root-search.md#results-supplied-ahead-of-the-query),
  each found by its name and opened by Enter.
- **Not a native helper** ([ADR 0014](adr/0014-optional-native-extension-helpers.md), #15): listing folders and asking the
  system to open a file are what the host process already does for every
  system; a separately packaged per-OS helper binary would add distribution
  and lifecycle work (#15 is not built) for no capability the host lacks.

Acquiring the package automatically at setup is
[#51](https://github.com/hoangvu12/pane/issues/51) to
[#53](https://github.com/hoangvu12/pane/issues/53); until then it is
installed from its folder like the calculator
(`pane --install target/guests/packages/applications`).

## Behavior

- Nothing runs at start, install or for a blank query. The first query that
  is not blank starts the extension, which asks the host for the installed
  applications; they are kept, and every later query ranks them at once, so
  typing never waits for them (the first query lists the commands at once
  and the applications as soon as they are found).
- They are asked for again on the first query after each return to root
  search (at start, after Escape from a command, after an install), keeping
  the earlier list until the new one arrives, and whenever the host's
  [live list](#live-list) changes. The host keeps its own list
  ([`Cached`](../crates/pane-core/src/applications/cached.rs)): only the
  very first request scans the system's folders, on the runtime thread;
  later ones get the kept list at once, which watchers keep current. So an
  application installed while Pane is open appears within about a second,
  even while root search is on screen.
- A blank query lists no application, so root search's empty list stays the
  commands and Pane's own rows.
- Each application is a root result titled with its name and subtitled
  "Application", matched and ranked like a command's title
  ([root search](root-search.md#matching-and-ranking)): "firefox" finds
  Firefox; an exact or prefix title beats a word inside another title; on
  the same rank commands come first.
- Enter opens it, off the window's thread, and the status says "Opened
  Firefox". A failure stays visible as the status error: "Could not open
  Firefox: cannot find the program firefox", "... not allowed to run ...",
  the system's own message from `open` or `ShellExecuteEx`, or "... it is
  no longer installed" (an id no application has) or "... no longer
  exists" (an old path) for an application removed since it was listed.
- If the applications cannot be listed at all, a row "Applications — Could
  not list: ..." is listed for every query that is not blank; Enter shows
  the whole error. A missing or unreadable location only adds nothing.
- **Disabled** (Settings › Extensions), its applications leave root search at
  once, the kept list is dropped, every watcher stops, its instance is
  stopped, nothing looks for applications any more, and a list still on its
  way is discarded; other results (commands, the calculator) are
  untouched. Enabled again, the next query looks, and watches, again.

## Live list

The host's list stays current by itself (ADR 0038), replacing the earlier
10-second rescan. The rules are in
[`cached`](../crates/pane-core/src/applications/cached.rs) and
[`watching`](../crates/pane-core/src/applications/watching.rs):

- **Lazy.** The list is built, and its watchers start, when a running
  package first asks for the installed applications (`installed()`; for
  the Applications extension, the first query that is not blank). Each
  component that asked is noted on its package's generation; once no
  package that asked can run any more (disabled, uninstalled, paused, its
  code replaced), the list is dropped and every watcher stops
  (`Applications::release`), until a package asks again. Opening an
  application by id while no list is kept scans once without keeping or
  watching anything.
- **Watchers**, the native file watcher development mode uses (`notify`):
  on Windows ReadDirectoryChangesW on each shortcut folder (the Start
  menus with their subfolders; the Desktops and taskbar pins flat), where
  any file changing counts, `.lnk`, `.url` and `.appref-ms` alike, and, for packaged apps, on
  `%LOCALAPPDATA%\Packages`, where Windows makes a folder for each package
  registered for the user: only a package's folder appearing or going
  counts there, and since Windows completes the registration afterwards,
  the list is rescanned again five seconds later. On macOS FSEvents on the
  Applications folders, and on Linux inotify on the `applications` data
  folders, with their subfolders. A folder that does not exist yet is
  watched for from the nearest folder above it (only changes on the way to
  it count), and the watch is made again after each rescan until the
  folder is watched itself.
- **Debounce.** Reported changes are rescanned once they settle: half a
  second after the last one, at most two seconds after the first.
- **Grace.** A rescan replaces the list by identity. An application whose
  last source went stays listed (and opens) for five seconds, by the
  launcher's clock, and leaves only if nothing with its identity came back
  meanwhile, so an update that removes and reinstalls it never makes it
  flicker out. A shortcut that now opens another program is that program
  at once, not a leaving application. A renamed shortcut keeps its
  application, its id and its pins, with the new title.
- **Reconciling.** A watcher that lost changes (its buffer overflowed) or
  failed, a wait that lasted far longer than it could (the system slept, or
  its clock jumped) and every 30 minutes while the list is kept trigger a
  full rescan at once. A failed rescan keeps the list.
- **Telling the launcher.** When the listed applications change, every
  enabled command with indexed results that asked for the installed
  applications (the Applications extension, the JavaScript and TypeScript
  samples, any extension using the import) has its results marked stale.
  While root search is on screen with a query they are asked for again at
  once and listed in place (a command being asked is waited for first, its
  answer possibly predating the change); otherwise at the next query. The
  selected row stays on the same result wherever it moved, or at the same
  position if that result left, so the list never jumps under the user. No
  guest export is added: the guest's `results()` reads the host's current
  list, which answers at once.

## Identity

An application is identified by what it is, not by where its shortcut
lies, so its pins (and, once root search learns from choices, ADR 0030,
what it learned) survive its updates. The rules are pure functions in
[`identity`](../crates/pane-core/src/applications/identity.rs), tested on
every system:

- Each system's adapter reports **sources** (a shortcut, a packaged app, a
  bundle, a desktop entry), each with the **key** of what it opens:
  - Windows desktop program: the shortcut's target, read through the
    shell's shortcut interface (`IShellLinkW`, on single-threaded COM
    apartments, up to eight threads at once, each shortcut read again only
    when its file changed), in lowercase with its arguments. Every path
    segment that is a version (digits separated by one to three dots,
    optionally after a prefix of up to eight letters and one separator:
    `app-1.0.9003`, `1.2.3.4`, `v2.0`) becomes a wildcard keeping its
    prefix (`app-*`), so Discord or Slack moving into a new version folder
    stays the same application; different arguments (two browser profiles,
    two web apps) are different applications. An MSI-advertised shortcut
    (Office's, for instance), which names the product it installs rather
    than a file, is resolved through the Windows Installer
    (`MsiGetShortcutTargetW`, `MsiGetComponentPathW`) to the program that
    product installed, so it is one application with any plain shortcut to
    that program; one whose product is not installed is left out.
  - Windows internet shortcut (`.url`) or ClickOnce application reference
    (`.appref-ms`): the URL, or the deployment it names, in lowercase, so a
    game's `steam://rungameid/...` link on the Desktop and in the Start menu
    is one application.
  - Windows packaged app: its package family name; a package with several
    apps adds the AppUserModelID for every app after the first (by app id).
    A shortcut whose AppUserModelID is a packaged app's is that app.
  - macOS: the bundle identifier (`CFBundleIdentifier` of the bundle's
    `Info.plist`, XML or binary), so moving or renaming the bundle keeps it.
  - Linux: the desktop file id, as before.
  - A source with nothing better (a shortcut the shell cannot read, a bundle
    without an identifier) is keyed by its own path.
- Sources with one key are **one application**: two shortcuts to one
  program are one result. Its **primary** source, whose name is its title
  and which Pane opens, is the one in the most preferred place (on Windows
  the user's Desktop, every user's Desktop, the user's Start menu, every
  user's, the taskbar pins, then the Apps folder, as Explorer prefers a
  shortcut of the user's own; on macOS `/Applications`,
  `/System/Applications`, `~/Applications`), then the shorter path.
- Its **id** is the first 128 bits of a SHA-256 digest of the typed key, as
  32 hexadecimal digits: the same on every start and every machine with the
  same installation. Extensions receive it through the applications import
  in Rust, JavaScript and TypeScript and must not parse it.
- The host keeps the map from each id, and from each source's path, to the
  application, so `open(id)` and opening a target with an application
  (`system.open`, Open With…, an indexed result's `open`) work for any id.
- **Ids from before identities** were the source's path (the shortcut, the
  bundle, the desktop entry, `shell:AppsFolder\<AppUserModelID>`). `open`
  still accepts them, opening the application that source belongs to. A
  quick slot holding one is carried over when the command's results arrive:
  it resolves to the result now listed for that application, and
  `quick-slots.json` is rewritten with the new id. One whose source is gone
  keeps its slot and says that Applications no longer lists it.

## Per platform

| | Windows ([#24](https://github.com/hoangvu12/pane/issues/24)) | macOS ([#25](https://github.com/hoangvu12/pane/issues/25)) | Linux ([#26](https://github.com/hoangvu12/pane/issues/26)) |
| --- | --- | --- | --- |
| Found in | Shortcuts: shell links (`.lnk`), internet shortcuts (`.url`) whose scheme has a registered handler (its key under `HKEY_CLASSES_ROOT` is marked `URL Protocol` and has a `shell` key: a game launcher's `steam://`, `com.epicgames.launcher://`) and ClickOnce application references (`.appref-ms`), in the Start menu's `%APPDATA%\Microsoft\Windows\Start Menu\Programs` then `%ProgramData%\...\Programs`, with subfolders, and without subfolders on the user's Desktop and every user's (`FOLDERID_Desktop`, `FOLDERID_PublicDesktop`, wherever the shell keeps them) and among the taskbar pins (`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar`); then the packaged (AppX/MSIX) apps of the shell's Apps folder (`FOLDERID_AppsFolder`), such as Calculator on Windows 11 | Application bundles (`.app`) in `/Applications`, `/System/Applications` and `~/Applications`, and their subfolders two deep (such as `Utilities`), not inside bundles | Desktop entries (`.desktop`) in `$XDG_DATA_HOME/applications` (default `~/.local/share/applications`) then `applications` in each of `$XDG_DATA_DIRS` (default `/usr/local/share:/usr/share`), with subfolders (Flatpak and Snap add their folders to `XDG_DATA_DIRS`) |
| Name | The shortcut's file name; a packaged app's display name | The bundle's folder name | The entry's `Name` (not localized `Name[..]`) |
| Identified by | A shell link's target and arguments, version folders wildcarded (an MSI-advertised one's installed program); an internet shortcut's URL; a ClickOnce reference's deployment; a packaged app's package family | The bundle identifier | The desktop file id |
| Left out | The Startup folders (`Startup` in either Start menu's Programs folder); uninstallers: a shortcut whose name contains "Uninstall" or whose program's name starts with `unins` (`unins000.exe`, `uninstall.exe`); a shell link whose target is missing (broken), empty (a shell item, or an advertised product that is not installed), a folder, or a document rather than a program (a program is `.exe`, `.com`, `.bat`, `.cmd`, `.msc`, `.cpl`, `.vbs`, `.vbe`, `.wsf` or `.wsh`); an internet shortcut to a web page or a document (`http`, `https`, `ftp`, `file`, `mailto`, `news`) or to a scheme nothing handles; folders that are symbolic links or junctions, which are not walked; a shortcut at the same place in the all-users menu (or Desktop) as in the user's; Apps folder items that are not packaged apps (desktop programs, found by their shortcuts) or that have a shortcut's name | Nothing | `Type` other than `Application`, `NoDisplay` or `Hidden` (a hidden entry also hides a lower one with the same desktop file id), no `Exec`, `OnlyShowIn`/`NotShowIn` against `$XDG_CURRENT_DESKTOP`, a `TryExec` program that is missing, an `Exec` line using field codes against the spec (`%i`, `%F` or `%U` inside an argument, more than one of `%f %u %F %U`, an unknown code or a lone `%`: skipped with a line on standard error, not guessed) |
| Opened by (the primary source) | `ShellExecuteEx` on the shortcut, or on `shell:AppsFolder\<AppUserModelID>` for a packaged app, as Explorer opens them (errors returned, no dialog), with COM initialized for the call and uninitialized after | `/usr/bin/open` on the bundle (Launch Services); its error message is shown | Running the `Exec` program directly (quoting and field codes per the Desktop Entry spec; file and URL codes dropped; `Path` as working folder), in its own process group; a `Terminal=true` entry runs in `$TERMINAL -e`, else the first installed of `x-terminal-emulator -e`, `gnome-terminal --`, `konsole -e`, `xfce4-terminal -x`, `alacritty -e`, `kitty`, `foot`, `xterm -e`, and is refused with an explanation when there is none |
| Not supported yet | Windows Settings pages and Control Panel items (a later slice of #124) | Localized names (`CFBundleDisplayName`), Spotlight-only locations | D-Bus activation, localized names, desktop actions |
| Not looked for | Programs known only to their uninstall records (winget installs), game launchers' libraries beyond the `.url` shortcuts they make, folders the user chooses for portable programs | The Desktop, the Dock, internet shortcuts (`.webloc`) and aliases, Spotlight-only locations and System Settings panes: only the bundles in the three Applications folders | The Desktop (`~/Desktop` launchers), panel or dock pins, internet shortcuts (`Type=Link` entries are left out), and D-Bus activation: only the desktop entries in the `applications` data folders |
| Desktop baseline | Windows 10/11 desktop; CI runs Windows Server 2025 (`windows-2025`) | macOS 15 (`macos-15`, arm64) | freedesktop Desktop Entry 1.5 on any desktop; run on X11 (Xvfb) only, Wayland untested |

The same author-facing contract serves all three: an extension receives
`application` records (`id`, its stable identity; `name` and `location`,
its primary source's) and returns `open-application(id)`; only the adapter
differs.

## Checks

- Launcher public interface ([`crates/pane-core/tests/applications.rs`](../crates/pane-core/tests/applications.rs)),
  with the real Applications guest and a fake system (so which applications
  exist and what opening does are deterministic): typing a name lists it and
  Enter opens it; ranking with a command by title; a blank query lists none
  and starts nothing; looked for once per visit of root search and found
  again after coming back; typing lists commands while the system is still
  being asked; opening and listing failures explained; disabling removes the
  applications, stops the instance and asking while the calculator still
  answers, and enabling brings them back; an answer arriving after
  disabling discarded; no "Applications" row, each application still found
  ([`root_providers.rs`](../crates/pane-core/tests/root_providers.rs) covers
  providers as such); a package declaring
  `indexedResults` without the interface refused at install; and the
  JavaScript and TypeScript author examples
  ([`guests/sample-applications-js`](../guests/sample-applications-js),
  [`-ts`](../guests/sample-applications-ts)), which supply "Launch <name>"
  for each application and whose commands list and open them through the
  import, errors included.
- Identity through the launcher ([`crates/pane-core/tests/application_identity.rs`](../crates/pane-core/tests/application_identity.rs)),
  with the real guest and the host's list over a fake system's sources: two
  shortcuts to one program are one result, opened by the preferred one;
  different arguments stay two results; an application updated into a new
  version folder keeps its id and its pin across a restart; pins made
  before identities (a data folder an earlier Pane wrote) resolve, are
  rewritten with the new ids, and a gone one keeps its slot and says why;
  the host opens an application by its old path; and the JavaScript and
  TypeScript samples receive the stable id and open by it. The pure rules
  (version folders and what is not one, the key and its digest, packaged
  apps' keys, primary election, legacy lookup) are unit tests of
  `identity`, and reading `Info.plist` (XML and binary) of `plist`.
- Live list ([`crates/pane-core/tests/application_cache.rs`](../crates/pane-core/tests/application_cache.rs)),
  over a fake system whose sources and watcher the test drives, on a
  manual clock: nothing is scanned or watched until asked, then once; a
  burst of reported changes is rescanned once after the debounce and told
  once, and a rescan finding the same applications tells nothing; a
  removed application stays for its grace by the clock and then leaves,
  and one reinstalled within it never does; lost changes rescan at once, a
  completing change again later, and the period reconciles what was never
  reported; a failed rescan keeps the list and a failed first scan is an
  error; releasing or dropping the list stops watching, and the next
  request scans and watches again; a partial watch is made again after a
  rescan; opening finds the application by its id or its old path (a scan
  that is not kept when nothing is), and an id no application has is
  explained. The pure rules (the grace from the first miss, a retargeted
  shortcut, a renamed one, telling a sleep, judging watcher events, the
  folder watched for a missing one) are unit tests of `cached` and
  `watching`.
- Live list through the launcher ([`crates/pane-core/tests/application_changes.rs`](../crates/pane-core/tests/application_changes.rs)),
  with the real guest and the host's list over a fake system: nothing is
  watched until root search is first used; an application installed (a
  program, or a packaged app whose change completes later) while root
  search is on screen appears within about a second without leaving it;
  an uninstalled one leaves after its grace on the launcher's clock, and
  one reinstalled within it never does; a renamed shortcut changes the
  title and keeps the pin; a lost change and the period reconcile; the
  selected row stays on its application, or at its position when it left;
  a change while the query is blank is listed by the next query of the
  same visit; and disabling Applications stops every watcher and drops the
  list, enabling it looking and watching again.
- Window ([`crates/pane/tests/window.rs`](../crates/pane/tests/window.rs)):
  typing a name renders the application's row, which assistive technology
  sees as the selected `ListBoxOption`, and Enter opens it with the field
  keeping focus.
- Adapters ([`crates/pane-core/tests/application_adapters.rs`](../crates/pane-core/tests/application_adapters.rs)):
  discovery of each system's fixtures runs on every system (precedence,
  hidden and filtered entries, subfolders, uninstallers, bundles inside
  bundles, `Exec` lines against the spec); `Exec` parsing, field-code
  checks, choosing a terminal emulator and adding the Apps folder's
  packaged apps without duplicating shortcuts have unit tests. Identity on
  every system: shortcuts (read by a fake reader) to one program in two
  version folders are one application, opened by the user's own; different
  arguments two; a shortcut on the Desktop and in the Start menu to one
  program is one application, opened by the Desktop's, every user's
  Desktop and the taskbar pins are found, and a Desktop's subfolder is not
  looked into; internet shortcuts to a scheme with a (fake) handler are
  found, one link on the Desktop and in the Start menu once, while those to
  a scheme nothing handles or to a web page are not, and a ClickOnce
  reference in UTF-16 is found by its deployment; the Startup folders,
  uninstallers by name and by program, broken shortcuts (a fake disk),
  shortcuts to folders and to documents are left out; the macOS and Linux
  sources Pane does not have are stated in this page's "Not looked for"
  row; an MSI-advertised shortcut the installer resolved is one application
  with a plain shortcut to its program, and one whose product is not
  installed is left out (unit tests, with the parsing of `.url` and
  `.appref-ms` files and the program, uninstaller and scheme rules); an unreadable shortcut keyed by its path; the host's list
  finds an application by its id and by a source's old path; bundles by
  their identifier (a moved copy is the same application) and desktop
  entries by their desktop file id; a shortcut to a packaged app is that
  app (unit test). On Windows, shortcuts made with `WScript.Shell` to a
  copied program in two version folders are read by the shell as one
  application, and inbox packaged apps are keyed by their package family;
  the native list looks in both Desktops and the taskbar pins without
  their subfolders; a `.url` to a scheme the test registers under the
  user's classes is found and one to an unregistered scheme is not; an
  `.appref-ms` is found; and shortcuts the shell makes to a removed
  program, a folder, a text file and an uninstaller copy are left out;
  on macOS, Calculator is identified by `com.apple.calculator`. On its own system each adapter
  opens a harmless application the test makes, which writes a marker file:
  a desktop entry (Linux), a bundle whose program is a shell script (macOS),
  a shortcut to `cmd.exe` made with `WScript.Shell` (Windows). The native
  lists are also read on each system: on macOS they must include
  Calculator, on Windows an inbox packaged app (Calculator or Settings)
  from the Apps folder. Each adapter's watcher runs on every system with its native watcher: a
  shortcut made in a watched Start menu subfolder or Desktop folder, a
  desktop entry, or a bundle in a temporary folder given to the adapter is
  reported within two seconds, and an `applications` folder that does not
  exist yet is reported when it appears.
- Native GUI smokes, one identical phase on all three systems (screenshots
  44 and 45): install the package, type "pane smoke", check the selected
  row, Enter, check "Opened Pane Smoke App" and that the application the
  smoke added (in a data folder, HOME or APPDATA of Pane's own, so the
  system's are searched too) wrote its marker file. See the
  [Linux](platforms/linux.md#applications-24-25-26), [macOS](platforms/macos.md#applications-25)
  and [Windows](platforms/windows.md#applications-24) notes for where it has run.

## Limits

- No icons, no localized names, no keywords or aliases (the user's aliases,
  [#31](aliases.md), are for installed commands only), no frequency
  ranking; applications are not ranked against commands beyond the title
  rank.
- The host imports are synchronous: the very first scan runs on the
  runtime thread, so a guest call made meanwhile waits for it (later scans
  run on the list's own thread; #29 owns cancellation). A change between
  the first scan and the moment the watchers start is found by the next
  rescan (a later change, or the 30-minute period).
- Packaged apps are followed through the `Packages` folder, not the
  system's packaged-app catalog events (`PackageCatalog`), which the
  specification asked to check first for an unpackaged process and which
  Pane does not use yet; a Store install appears once its folder is made
  and is looked at again five seconds later. A registration that takes
  longer is found by the next change or the period.
- On Windows a scan reads every shortcut through the shell, which takes longer than listing the
  folders did (Raycast measured 0.6 to 1.5 s for about 115 applications);
  later scans read only the shortcuts that changed.
- A shell link the shell cannot read is keyed by its own path, as before,
  and listed. A pin made before identities is carried over only while its
  source still exists. Opening a ClickOnce reference starts the ClickOnce
  installer's own checks, which may show its dialog; a `.url` opens through
  the handler the system has for its scheme.
- Pane does not hide or reset after opening an application; root search
  stays as it was.
- The adapters trust the host's own listing: `open` accepts any existing
  shortcut, bundle or desktop entry path, which any trusted extension could
  pass (Q9's trust model).
