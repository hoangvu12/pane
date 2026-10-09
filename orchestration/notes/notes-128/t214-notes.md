# #214 — Error overlay for a developed extension's crash (working notes)

Branch `pi-subagent/128-214-error-overlay`, worktree `spec-128-t214`, base 26c8e224.
This file lives outside the repo. Decisions, test approach, and exactly what
the merge must port onto #218's rewritten JS build.

## What I set out to build

Slice 1 of spec #128: a developed package's command that crashes, traps,
throws (JS/TS) or fails to start shows the message and stack trace over the
command's view, with Open Logs / Copy / Retry. Not-developed packages keep
today's presentation exactly. JS/TS development builds keep a source map
beside the staged component; Pane maps JS stack positions back to source
files on the overlay and in the extension log.

## Design decisions

### 1. Placement: a pane-core screen (`Screen::Crash`), not window state

Chose the screen route (the task guidance's preferred option), like
`Screen::Confirm`/`Screen::PauseDetails`/`Screen::BuildDetails`:

- Works headless in pane-core's `LauncherView` tests: the core seam asserts
  `view().screen == Screen::Crash { .. }`, rows, details, retry, the same way
  `develop.rs` asserts `BuildDetails`.
- Rows + details + selection + Enter/Escape come free from the existing
  generic screen machinery (rows are the three actions; details lines are the
  message and the trace). The window needs only two one-line arms (`empty`
  text, scrollable details share); no new feature module, no new key context,
  no focus juggling.
- "Over the command's view, which it must not destroy": the state the crash
  left behind (`state.open`, `state.open_command`, `state.launch`,
  `state.searching`, `state.form`, `state.custom_view`) is untouched while the
  overlay shows; `State::error_overlay` snapshots the covered
  `LauncherView` + `entries` and `back()` (Escape) puts them back.
  `Screen::Confirm` is the precedent for "a screen that stands in for the one
  under it and returns to it".
- A stale snapshot can never be restored over a newer screen: it is only
  reachable while the screen is `Screen::Crash`, and only the overlay itself
  ever sets that screen (any other flow replaces the screen first, making the
  snapshot unreachable garbage).

Escape = leave the overlay, back to what was under it. Enter = the selected
row; rows in ticket order (Open Logs first, then Copy, then Retry).

### 2. Trigger sites (all gated on `Launcher::development(&identity).is_some()`, and only `CallError::Trap` / `CallError::Guest`)

| Site | What it covers | Today (not developed) — unchanged | Retry does |
|---|---|---|---|
| `run_action` (item chosen in the command's list) | Rust panic on an action (`sample-settings` "Crash"); JS/TS throw on an action ("Fail") | status line / failure toast | relaunch the command |
| `open_command` (opening a view command) | trap or throw during the first render | status line | relaunch the command |
| `run_no_view` (no-view run, user-initiated) | crash/throw of a no-view run | status line / failure toast | re-run the command |
| `reload.rs carry_out` failed start (development reload) | a reload whose replacement could not start; the pause and its "Why X is paused" screen stay as they are | pause + status line | `retry_start` (the pause's Retry) |

Not in scope (kept as today, for developed packages too): `Unresponsive`
(not in the ticket's trigger list), `Unreadable` (the command's own failure),
`RuntimeUnavailable` (Pane's own), form errors (`CallError::Form`), the 3rd
crash that pauses (the pause pre-empts the answer: `note_health` runs before
the call's answer and `show_root` bumps the screen epoch, so the answer is
discarded — today's pause presentation shows, by design).

Retry semantics ("the same path as activating the row again"): the overlay
rebuilds the command's `Opening` and goes through `launch_opening` — a view
command reopens, a no-view command re-runs; the failed-start overlay's Retry
goes through `retry_start` exactly as the PauseDetails row does.

The overlay also ends when a new development event arrives for the package
(`show_development` with Progress/Result — "Building…" / "Reloaded X"): what
it was about is over. A failed build or another failed start leaves it.

### 3. The message and the stack trace

- Rust trap: `CallError::Trap`'s reason is wasmtime's `{trap:#}` Display,
  which contains the wasm backtrace on its later lines. The overlay's title is
  "<title> crashed", the message is the Display's first line, the trace is the
  remaining lines. (The panic line itself is already in the extension log —
  the SDK's panic handler writes it before trapping; no new logging was
  needed there.)
- JS/TS throw: `CallError::Guest` carries only the message; the thrown
  error's stack reaches Pane through `guests/js/adapt.js`'s `logThrown`, which
  writes it to the extension log at error level (this is what the ticket
  means by "carries message+stack through adapt.js" — the guest error answer
  itself stays message-only, so not-developed failure toasts are unchanged).
  The overlay takes the trailing run of the package's stderr error lines
  (logThrown's block, already source-mapped at capture — below) as the trace.
  Title "<title> failed".
- Failed start: title "<title> failed to start", the error's Display.

Copy copies the screen's details lines joined by newlines (message + trace).

### 4. Per-crash log lines — already landed, left alone

The task asked to "ALSO write each crash/trap of a DEVELOPED package into the
extension log as a Pane line at Error level" and to keep not-developed
packages without per-crash lines. That premise is stale: #212 already writes
each crash/unresponsive/failed-start as a Pane Error line for **every**
installed package (`runtime.rs`'s `Host::report` → `ExtensionLogs::pane`;
comment: "The package's log has each failure, with a crash's backtrace"), and
a JS/TS throw is logged with its stack by `logThrown`. Both kinds of package
keep exactly that behavior; I changed nothing on that path and my tests
assert the lines are there. `PANE_LINE_LIMIT` (16 KiB) already covers
backtraces.

### 5. Source maps

**Build side (minimal, clearly commented — #218 is rewriting this path):**

- `tools/componentize-js/bundle.mjs`: esbuild gains `sourcemap: "external"`
  and `sourcesContent: false` (the host needs names and positions only; no
  sources content is embedded, so the committed maps stay small). esbuild
  writes `work/bundle.mjs.map` beside the bundle.
- `tools/componentize-js/pane_js.py` `build()`: after componentizing, copies
  the bundle's map to `<out.wasm>.map` (the same path plus `.map`), so the
  staging folder holds the component and its map side by side.
- `xtask/src/main.rs` `guests()`: copies `guests/prebuilt/<name>.wasm.map`
  beside the prebuilt component and into each assembled sample package, when
  it exists. `pane_js.py samples` now writes those maps.

**Host side (pane-core, all mine; #218 only needs the build-side bits above):**

- New module `crates/pane-core/src/source_map.rs`: parses a source map
  (sources + mappings; VLQ decoding by hand, ~40 lines, no new crates —
  Cargo.lock untouched), and rewrites stack frames
  `at fn (bundle.mjs:line:col)` → `at fn (src/index.ts:line:col)`.
- The map is looked up **beside the running component** — the managed copy
  under `data/extensions`, not the transient development staging folder: that
  is where the component the guest runs actually lives, and it is the path
  the runtime already knows. To get it there:
  - `packages.rs copy_package` (the install/reload copy into the managed
    copy) also copies `<component>.map` beside each named component when it
    exists;
  - `pane-build session.rs copy_components` (the copy back into the source
    folder after a successful build) does the same, so a later Reload of the
    same build keeps the map.
  So a map travels staging → managed copy → source folder, following exactly
  the paths the reload already uses for components (the task's "follow
  whatever the reload already copies, or extend it").
- **Mapping happens at capture time, in the runtime**: `start_instance`
  reads the map beside the component once per instance and hands it to the
  WASI output streams; every line the package writes is passed through the
  frame rewriter before it becomes a `LogLine`. That is what makes the trace
  mapped "in the extension log" — the Logs screen, the development log file,
  the followers and the overlay all show mapped lines, and the overlay
  (which takes the JS trace from the log) needs no mapping of its own.
  Pane's own lines need none (a JS crash has no JS stack; a Rust trap's
  backtrace is wasm).
- Line/column bases: sourcemap generated lines are 1-based and columns
  0-based; QuickJS stack frames are line 1-based, so the lookup uses
  `line-1` and `col-1`, taking the nearest mapping segment at or before the
  column on that generated line. The frame matcher does not assume the
  "at fn (file:line:col)" shape: it finds any `bundle.mjs:<line>:<col>`
  substring (the runtime names the module `!/bundle.mjs` — verified in a
  prebuilt component's string table) and replaces it, so shape variations
  still map. Mapped lines no longer match, so re-mapping is a no-op.

A not-developed package whose managed copy still has a map beside its
component (development stopped) still gets its frames mapped; that changes no
presentation, and the log is the author's either way.

### 6. Screen/entry plumbing (pane-core)

- `Screen::Crash { identity: PackageIdentity, details: Vec<String> }` — the
  details are the message and trace lines; title "<title> crashed/failed/
  failed to start".
- `State::error_overlay: Option<error_overlay::Shown>` — the retry plan and
  the covered view + entries.
- New entries: `Entry::CrashLogs(identity)` (also clears the overlay, then
  the existing `show_extension_log`), `Entry::CrashCopy` (the window writes
  the clipboard through an extended `selected_copy`; the status line says
  "Copied the message and trace"), `Entry::CrashRetry` (runs the snapshot's
  retry plan).
- Arms added everywhere `Screen` is matched exhaustively: `back()`,
  `LauncherView::details()`, `refresh()` (no-op), `actions.rs`
  `SelectedAction` ("Show logs" / "Copy" / "Run again"), the announcer
  (Titled, like the other details screens), app.rs `empty` text and the
  scrollable-details condition, and `show_development`'s leave-on-new-event.

## Tests

- `crates/pane-core/tests/error_overlay.rs` (Launcher seam, CopyBuilder
  pattern from `develop.rs`/`extension_log.rs`): the settings sample's
  "Crash" (Rust trap) and "Fail" (JS and TS throws) show the overlay with
  the message and trace over the still-open command view; Escape returns to
  the command; Retry relaunches (the item list is drawn again, a second
  crash shows the overlay again); Open Logs opens `Screen::ExtensionLog`;
  Copy's `selected_copy` is the message + trace; the crash's Pane Error line
  is in the log; a not-developed package that crashes shows the status line
  and no overlay; a no-view run that fails shows the overlay and its Retry
  re-runs it; a development reload that fails to start shows the overlay
  with "Retry starting" and the pause is still there; a fixed save reloads
  and the overlay leaves.
- Source-mapped stacks without the JS toolchain: unit tests in
  `source_map.rs` over a hand-written fixture map and stack text (VLQ
  decode, frame rewrite, unmapped frame left alone). The TS sample's real
  mapped frame is asserted in the core test above — it needs the rebuilt
  `guests/prebuilt` with maps (committed from CI's `js-guests` artifact
  before any verify run; locally unrunnable by rule).
- `crates/pane/tests/error_overlay.rs` (window, real key events): the
  developed settings sample's "Crash" item shows the overlay
  (debug selectors `row-Retry`-style rows + `detail-` lines), Open Logs
  opens the Logs screen, Copy puts the message and trace on the clipboard
  (`cx.read_from_clipboard`), Retry runs the command again, Escape returns
  to the command's list; the TS "Fail" item shows the overlay with a
  source-named frame; a not-developed crash shows the status line only.
- Existing tests updated where the overlay replaces the old presentation
  *for developed packages*: `extension_log.rs` (core) `pane.run("Crash")` /
  `pane.run("Fail")` now show the overlay (asserted) instead of a status
  error; `develop.rs` (core) `a_build_that_fails_to_start_is_paused_with_…`
  asserts the overlay and its "Retry starting Dev" row, then the fixed save
  reloads (the overlay leaves, "Reloaded Dev" shows).

## What the merge must port onto #218 (JS build rewritten into pane-build)

Exact files and options:

1. `tools/componentize-js/bundle.mjs` — esbuild call gains
   `sourcemap: "external"` and `sourcesContent: false`. If #218 deletes this
   file, the same two options go wherever it runs esbuild.
2. `tools/componentize-js/pane_js.py` `build()` — after componentize, copy
   the bundle's map to `<out>.map` beside the staged component (the
   search: `bundle.with_extension("mjs.map")`, hmm — see the code: the map
   esbuild writes beside the bundle, copied to `out.with_extension` … the
   exact lines are marked with a `#214` comment).
3. `xtask/src/main.rs` `guests()` — copy `<name>.wasm.map` beside each
   prebuilt component and into each assembled sample package when present
   (marked with a `#214` comment).
4. Everything else is host-side and conflict-free: `source_map.rs`,
   `extension_log.rs`, `runtime.rs`, `packages.rs` `copy_package`,
   `pane-build/src/session.rs` `copy_components` (this one may conflict
   textually with #218's build-crate work but is two lines + comment).

The contract #218 must keep: **the map file's path is the component's path
plus `.map`, in the same folder, named `sources`/`mappings` per the source
map v3 spec; the bundle's own module name ends with `bundle.mjs`.** If #218's
esbuild invocation changes the bundle file name, the frame matcher
(`source_map.rs`, matches `bundle.mjs:<line>:<col>`) needs the new name.

## CI plan

Plain pushes only (fast tier: fmt + check on Linux+Windows). The first push
makes `guests/prebuilt` stale (bundle.mjs is a tool input), so the rebuild
job rebuilds the JS/TS samples on Linux and uploads the `js-guests` artifact
(now including `<name>.wasm.map` files); I download and commit it
(`gh run download <run> -n js-guests -D guests/prebuilt`) so the verify
tier's prebuilt-samples check passes. The orchestrator dispatches exactly one
verify at the end.
