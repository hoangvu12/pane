import sys

ROOT = sys.argv[1]


def edit(p, pairs):
    p = ROOT + "/" + p
    s = open(p, encoding="utf-8").read()
    for a, b in pairs:
        assert s.count(a) == 1, (p, a, s.count(a))
        s = s.replace(a, b)
    open(p, "w", encoding="utf-8", newline="\n").write(s)


SECTION = r"""## From the terminal: `pane-ext dev`

`pane-ext dev [folder]` ([#217](https://github.com/pane-app/pane/issues/217),
[ADR 0047](adr/0047-extensions-are-built-from-the-app-first-with-pane-ext-beside-it.md))
develops the package in `folder` (the current folder by default) from the
author's terminal. It runs the same session as Pane's own development (the
`pane-build` crate's), so the build command, the obsolete builds and the
copying of components into the source folder are the same; the difference
is where the builds run and print:

1. It reaches the running Pane over the **local channel**, a per-user
   endpoint only the same user can open: the named pipe
   `\\.\pipe\pane-<the user's SID>` on Windows, whose protected security
   descriptor grants this user alone and which refuses remote clients; the
   socket `channel` in `$XDG_RUNTIME_DIR/pane`, else in `pane-<uid>` in the
   temporary folder, on macOS and Linux, in a folder of mode 0700 (Pane
   does not listen in one open to others), itself mode 0600, closing
   connections from other users unanswered. `PANE_CHANNEL` names another
   endpoint (to `pane-ext` always; to Pane only in a development build).
2. If no Pane answers, it starts one: the program `PANE_APP` names, else
   `pane` beside `pane-ext`, else where Pane's packages install it
   (`%LOCALAPPDATA%\Pane\pane.exe`, `Pane.app` in `/Applications` or
   `~/Applications`, `~/.local/bin/pane`), else `pane` on the search path,
   and waits up to a minute for it to listen. Found nowhere, it says where
   it looked and exits, before building anything.
3. It builds the package in the terminal, printing what the build prints,
   cargo's errors included, and stages it in a folder of its own in the
   user's cache folder (`pane-ext/<hash of the folder>`).
4. It hands the first build that succeeds to Pane. A folder Pane has not
   installed is first shown in Pane's ordinary install preview, with this
   build (its components are copied into the folder), and the author
   chooses **Install** there; leaving the preview refuses the build, and
   `pane-ext` exits. Pane then develops the package without watching or
   building it: the rows, the status line, **Why <title> did not build**,
   the extension log and its file are as for its own development, the
   status saying "Developing <title> with pane-ext". An installed folder is
   developed at once, from this build; a development already going on,
   Pane's own or another `pane-ext`'s, ends first.
5. Each save builds the package again in the terminal; Pane reloads each
   build that succeeds, and a build that fails leaves Pane running the
   working code, with the failure shown in Pane as its own are. **Build
   <title> again** asks `pane-ext` to build.
6. Pane's messages about the package and what the package prints, its
   extension log, are printed in the terminal as they come ("Pane: Reloaded
   Hello Rust", "info [hello] …").
7. Ctrl+C, or the connection closing, stops the development as **Stop
   developing** does (the package stays installed). Stopping it in Pane,
   disabling or uninstalling the package, or quitting Pane ends
   `pane-ext dev`.

The channel's requests are JSON, one a line, each naming the channel's
version (1), as `pane_core::local_channel` documents: `subscribe`,
`develop` (a folder and a staged build), `building` and `failed` (later
builds) and `stop`. Pane answers with `previewing`, `developing`,
`refused`, `build`, `log` and `ended` events. A request of another version
is refused, saying so.

Rust packages work end to end. A JavaScript or TypeScript package builds
with `pane_js.py`, as Pane's own development does, which `pane-ext` finds
only through `PANE_COMPONENTIZE_JS`; the componentizer slice of #128
replaces it.

## Local and published copies
"""

CHOICES = r"""- `pane-ext dev` (#217): the request set above; the endpoint's locations;
  where `pane-ext` looks for a Pane to start, and the minute it waits; that
  Pane waits up to 30 seconds for its window to show the install preview it
  asked for, and then for as long as the author takes to answer it; that
  the first `pane-ext dev` of a folder not installed copies the build's
  components into the folder, so that the preview and the install are of
  that build; that a second `pane-ext dev` of the same package takes the
  development over; and that `pane-ext` reaches Pane before its first
  build, so that a missing Pane is reported at once.

## Checks
"""

CHECKS_AFTER = r"""- [`crates/pane/tests/develop.rs`](../crates/pane/tests/develop.rs): the
  window redraws by itself when a background build fails and when the fix
  is reloaded, and renders the diagnostics.
- [`crates/pane-core/tests/local_channel.rs`](../crates/pane-core/tests/local_channel.rs)
  speaks the local channel as `pane-ext` does, against a launcher listening
  on an endpoint of its own, with guests staged as builds: a folder not
  installed is previewed with the build and developed once installed (its
  log, Pane's messages, a failure kept as Pane's own, a later build
  reloaded), and closing the connection stops the development; a preview
  left refuses the build; an installed folder is developed at once, and
  stopping it in Pane ends the connection's; an unknown folder is refused.
  Unit tests in [`local_channel.rs`](../crates/pane-core/src/local_channel.rs):
  the requests' and events' JSON, other versions refused, a second Pane
  cannot listen on the endpoint, and the endpoint is open to this user only
  (the socket's and its folder's modes, or the pipe's DACL).
- [`crates/pane-ext/tests/dev.rs`](../crates/pane-ext/tests/dev.rs) runs
  `pane-ext dev` on a copy of `guests/hello-rust` against a launcher
  listening as the running Pane: the build's output, the install preview,
  Pane's messages and the package's log line in the terminal; a compile
  error printed there while Pane keeps the working code; a fix reloaded;
  the development stopped when `pane-ext` is killed. With no Pane listening
  and none to start, it says where it looked, before building. Its unit
  tests: waiting for a Pane that starts listening, and giving up on one
  that does not."""

LIMITS_AFTER = r"""- The JS/TS build needs a Pane checkout's `pane_js.py` (and its toolchain);
  an installed Pane without a checkout cannot build JS/TS on save.
- On macOS and Linux, Ctrl+C in `pane-ext dev`'s terminal ends `pane-ext`
  but not a build it is running, which is in a process group of its own:
  the build runs to its end, and Pane is told nothing of it. On Windows the
  console's Ctrl+C reaches the build too.
- That another user cannot open the endpoint is checked in CI through its
  permissions (the socket's and its folder's modes, the pipe's DACL), not
  by connecting as another user."""

edit(
    "docs/development-mode.md",
    [
        ("## Local and published copies\n", SECTION),
        ("## Checks\n", CHOICES),
        (
            """- [`crates/pane/tests/develop.rs`](../crates/pane/tests/develop.rs): the
  window redraws by itself when a background build fails and when the fix
  is reloaded, and renders the diagnostics.""",
            CHECKS_AFTER,
        ),
        (
            """- The JS/TS build needs a Pane checkout's `pane_js.py` (and its toolchain);
  an installed Pane without a checkout cannot build JS/TS on save.""",
            LIMITS_AFTER,
        ),
    ],
)

edit(
    "README.md",
    [
        (
            """- `crates/pane-target`: the operating-system and processor names shared by the core, `xtask` and native helpers.""",
            """- `crates/pane-build`: how a package is built from its source folder, once or after each save; development mode and `pane-ext` both build with it.
- `crates/pane-ext`: `pane-ext`, the command-line tool beside the app; `pane-ext dev` builds a package in the terminal and hands each build to the running Pane ([development mode](docs/development-mode.md#from-the-terminal-pane-ext-dev)).
- `crates/pane-target`: the operating-system and processor names shared by the core, `xtask` and native helpers.""",
        )
    ],
)

edit(
    "CONTEXT.md",
    [
        (
            """**pane-ext**:
The command-line tool authors use beside the app to create, develop, check and pack an extension package; it builds with the same code as development mode and hands the result to the running Pane. Distinct from `pane`, the application's executable.
_Avoid_: Pane CLI, `pane` (the application)
""",
            """**pane-ext**:
The command-line tool authors use beside the app to create, develop, check and pack an extension package; it builds with the same code as development mode and hands the result to the running Pane. Distinct from `pane`, the application's executable.
_Avoid_: Pane CLI, `pane` (the application)

**Local channel**:
The endpoint the running Pane listens on for `pane-ext`, which only the same user can open: a named pipe on Windows, a Unix-domain socket in a folder of the user's own elsewhere. `pane-ext dev` hands its builds over it and receives the package's development status and extension log back; closing it stops the development.
_Avoid_: IPC, socket (one of its forms), single-instance channel
""",
        )
    ],
)
