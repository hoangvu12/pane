# Development mode: build and reload on save

Added for [#12](https://github.com/hoangvu12/pane/issues/12) (Rust) and
[#13](https://github.com/hoangvu12/pane/issues/13) (JavaScript and
TypeScript): US19, US51, US53, T07, T08, G1, G3; contributions, not a claim
that the whole scenario or gate passes. An author edits an extension, saves,
and sees the new behavior while Pane stays open ([ADR 0004](adr/0004-reload-extensions-without-restarting-launcher.md),
Q14). It builds on [reload](../guests/README.md#reloading-a-package-while-pane-stays-open)
(#11) and [pausing](pausing.md) (#16).

## Trying it

The development samples are one command each, "Say hello", which answers
with a `GREETING` constant; each builds in its own folder, as an author's
package would.

**Rust** ([`guests/hello-rust`](../guests/hello-rust)):

1. Build it once, from the repository root:
   `cd guests/hello-rust && cargo build --release --target wasm32-wasip2`.
   (Outside this repository, copy the folder, point `pane-guest`'s `path`
   in `Cargo.toml` at `guests/pane-guest` of a Pane checkout and copy
   `rust-toolchain.toml` beside it.)
2. `cargo run -p pane -- --install guests/hello-rust`, and Enter on
   **Install**.
3. In **Manage extensions…**, choose **Develop Hello Rust** (the list's
   last rows). The status says which command each save runs.
4. Edit `GREETING` in `guests/hello-rust/src/lib.rs` and save. The status
   shows "Building Hello Rust: cargo build --release --target
   wasm32-wasip2…", then "Reloaded Hello Rust"; "Say hello" answers with
   the new text.
5. Save something that does not compile (`const GREETING: &str = 42;`): the
   status says "Hello Rust did not build: error[E0308]: mismatched types.
   It keeps running its installed code; …", and **Why Hello Rust did not
   build** in Manage extensions shows the compiler's output. "Say hello"
   still answers as before. Fix it and save: it is reloaded.
6. **Stop developing Hello Rust** ends it.

**JavaScript and TypeScript** ([`guests/hello-js`](../guests/hello-js),
[`guests/hello-ts`](../guests/hello-ts)) need the JS toolchain
([prerequisites](../guests/README.md#writing-a-javascript-or-typescript-command)):

1. Build once: `python3 tools/componentize-js/pane_js.py build
   guests/hello-ts guests/hello-ts/dist/hello_ts.wasm`.
2. Install `guests/hello-ts`, choose **Develop Hello TypeScript**, edit
   `GREETING` in `src/index.ts` and save. A type error
   (`const GREETING: string = 42;`) is shown as "Hello TypeScript did not
   build: src/index.ts(12,7): error TS2322: …".

The Pane window must be one built from a checkout (`cargo run -p pane`),
which knows where `tools/componentize-js/pane_js.py` is; otherwise set
`PANE_COMPONENTIZE_JS` to that file.

## What Pane does

Development is turned on per installed, enabled package, from its **Develop
<title>** row, and runs in the background:

1. **Watching.** Pane watches the package's source folder (its identity,
   `local:` and the folder) with the system's file watcher ([notify](https://docs.rs/notify/8.2.0)
   8.2: inotify on Linux, FSEvents on macOS, `ReadDirectoryChangesW` on
   Windows). It watches the folder itself and each top-level folder that is
   not the build's (a new one is watched when it appears), so the build's
   own writes never count as saves: not `target/`, `Cargo.lock` (Rust),
   `node_modules/` (JS/TS), the components `pane.json` names or the
   top-level folder they are in (`dist/`), hidden files and folders
   (`.git`, an editor's `.swp`), and `name~` backups. Reading a file is not
   a save.
2. **Building.** After a save, once nothing more is saved for 150 ms (an
   editor's several writes are one save), Pane runs the package's build in
   its folder, one adapter per language, the command the guest README
   documents:

   | Folder has | Build |
   | --- | --- |
   | `Cargo.toml` | `cargo build --release --target wasm32-wasip2` (rustup's `cargo`, so the folder's `rust-toolchain.toml` applies; the `CARGO_*`, `RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTFLAGS` Pane itself was started with are removed, as `pane_js.py` does) |
   | `package.json` | `python3 <pane_js.py> build <folder> <folder>/<component>` for each component `pane.json` names (`PYTHON` names another interpreter; `python` on Windows) |
   | neither | Not developed: "Cannot develop <title>: … has neither Cargo.toml (Rust) nor package.json (JavaScript or TypeScript) …" |

   The status shows "Building <title>: <command>…". The build's output
   (standard output and error) is kept.
3. **A build that fails** replaces nothing: the package keeps running its
   installed code. The status says "<title> did not build: <the first line
   that reports an error>. It keeps running its installed code; the
   diagnostics are under "Why <title> did not build" in Manage extensions."
   That row opens the command, the folder and the last 60 lines of the
   output, with **Build <title> again**; the whole output also goes to
   Pane's standard error. The row goes once a build succeeds.
4. **A build that succeeds** is reloaded exactly as **Reload <title>**
   reloads it: checked as an install checks it (a component that does not
   pass is "not reloaded", the code keeps running), then replacing the
   managed copy and starting each available command. A start that fails
   pauses the package with Retry and diagnostics, and **the earlier code is
   not restored** (Q31, [pausing](pausing.md)); the next save that builds
   reloads it, which ends the pause. Settings are kept; nothing live is
   carried over.
5. **Saving during a build** makes that build obsolete: it runs to its end
   (it is not killed), but is never reloaded, and the folder is built
   again. Builds of one package run one at a time, and each reload ends
   before the next build starts, so an older build never replaces a newer
   one.

Only the developed package is built and reloaded; Pane and every other
package keep running.

## When development ends

When the author chooses **Stop developing <title>**, disables or
uninstalls the package, or Pane quits (and when the launcher is dropped, as
in tests). The watcher is dropped and a running build is stopped with every
process it started: it runs in a process group of its own, killed with
`SIGKILL` on macOS and Linux, and with `taskkill /T /F` on Windows (where it
also runs without a console window). Enabling the package again does not
develop it again.

## Local and published copies

Development applies to one installation, identified by its source folder
([identity](../guests/README.md#packaging-and-installing-a-local-extension)).
Another installed copy of the same package, from another folder (the
"published" copy, until npm and Git sources exist), is never built,
reloaded, disabled or swapped for it (Q30): both stay listed, each with its
own commands, settings and code, and a published copy without sources is
explained ("Cannot develop <title>: …") rather than developed.

## Choices to confirm (provisional)

These are implementation choices of #12/#13, not user decisions:

- Development is turned on per package in Manage extensions and is **not
  recorded**: it ends when Pane quits, and starting it does not build at
  once (the next save does).
- The build is chosen by the folder's `Cargo.toml` or `package.json`, with
  the documented commands; a package cannot declare a build of its own.
- A save during a build lets that build finish and builds again, rather than
  killing it.
- A build failure is shown in the status line and a details row, not as a
  separate notification; the details show the last 60 lines.
- Reloading on save starts every available command, as a manual reload does
  (the provisional exception to lazy activation in
  [current decisions](current-decisions.md)).
- The rows are near the end of Manage extensions, one per enabled
  package, after the hotkey rows and before retained data.

## Checks

- [`crates/pane-core/tests/develop.rs`](../crates/pane-core/tests/develop.rs)
  drives development through the launcher with the system's file watcher
  and a stand-in build that copies a real guest: a save reloads only that
  package (another keeps its code); a build that fails keeps the code and
  shows its diagnostics, which Build again reruns, and a fix reloads it; a
  build that fails to start is paused with Retry and not rolled back, and
  the next save recovers it; a save during a build makes it obsolete (only
  the newer build is reloaded); stopping, disabling, uninstalling and
  dropping the launcher each stop the running build and the watcher; a
  published copy keeps its identity and code; the rows in Manage
  extensions; the window is told of each change.
- [`crates/pane-core/tests/develop_builds.rs`](../crates/pane-core/tests/develop_builds.rs)
  runs the real builds on copies of the samples: an edit is built and
  reloaded, a compile or type error keeps the code and shows the compiler's
  output, a fix reloads it. The Rust test runs in `cargo xtask ci`; the
  JavaScript and TypeScript ones need the JS toolchain and run with
  `PANE_TEST_JS_BUILDS=1` (CI's JS/TS job sets it).
- Unit tests in [`develop.rs`](../crates/pane-core/src/develop.rs): the
  adapters' commands and ignored paths, a folder without a known build,
  the first error line, and stopping a command kills the processes it
  started (Unix).
- [`crates/pane/tests/develop.rs`](../crates/pane/tests/develop.rs): the
  window redraws by itself when a background build fails and when the fix
  is reloaded, and renders the diagnostics.
- The native smokes' development phase (screenshots 66 to 92; see the
  [platform notes](platforms/linux.md#development-mode-12-13)).

## Limits

- A build has no timeout: one that hangs holds the package's development
  until it ends or development stops.
- Pane ended without quitting (a signal such as `SIGTERM` or `SIGKILL`, a
  crash) leaves a running build to finish on its own.
- Only the folder's top level decides what is watched: sources kept inside
  `target/`, `node_modules/` or the components' folder are not watched.
- The JS/TS build needs a Pane checkout's `pane_js.py` (and its toolchain);
  an installed Pane without a checkout cannot build JS/TS on save.
- Native evidence is from Linux (X11) only; the macOS and Windows smokes run
  the same phase, not yet run there. The platform code (process groups,
  kill, the watcher) was compile- and lint-checked for Windows and macOS
  targets from Linux.
