# Pane — TESTS, CI AND SMOKES codebase map (branch `spec/125-windows-power`)

Repo: `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\spec-125`. Workspace members: `crates/pane`, `crates/pane-core`, `crates/pane-build`, `crates/pane-ext`, `crates/pane-target`, `xtask`. Guests (`guests/`) are a separate wasm workspace, excluded. No cargo/rustc/xtask was run; everything below is from reading files.

Key fact up front: **`PANE_TEST_REAL_INPUT` does not exist yet** (zero matches in the repo). Existing opt-ins: `PANE_TEST_REAL_CLIPBOARD`, `PANE_TEST_SYSTEM_ICONS`, `PANE_TEST_PROCESS_TREES`, `PANE_TEST_JS_BUILDS`, plus smoke-only vars (`PANE_TEST_SHOW_HUD`, `PANE_TEST_REVEAL`, `PANE_TEST_TRASH`, `PANE_TEST_SYSTEM_LOG`, `PANE_TEST_RUNTIME_FAULTS`).

## 1. `crates/pane-core/tests/` — the launcher's integration tests

`autotests = false` in `crates/pane-core/Cargo.toml` (line 8). One binary, `tests/main.rs`, declares every file as `mod name;` — **every new test file must be declared there or it is never compiled**, enforced by the test `every_test_file_is_compiled` in `crates/pane-core/tests/main.rs`. Run one file's tests as `cargo test -p pane-core --test integration` + `hotkeys::` filter. Each file pulls support helpers through its own `#[path = "support/…"] mod` (deliberately duplicated per file; `#![allow(clippy::duplicate_mod)]` at the top of main.rs).

Test files (path → coverage, from each file's module docs):
- `aliases.rs` — aliases and fallbacks in root search, Echo query sample.
- `application_adapters.rs` — each system's application adapter; **finding tested on every system with fixture folders; opening only on its own system**; `#[cfg(target_os = "linux")] mod linux`, `#[cfg(target_os = "macos")] mod macos`, `#[cfg(windows)] mod windows` (the Windows module makes real `.lnk` shortcuts via `powershell … (New-Object -ComObject WScript.Shell).CreateShortcut(...)`).
- `application_cache.rs` — host's live `Cached` list over a fake system (ADR 0038).
- `application_changes.rs` — live list changes through the real Applications guest.
- `application_icon_adapters.rs` — real-system icon extraction (#172), Windows part in `#[cfg(windows)] mod windows` (line 99).
- `application_icons.rs` — icons through the launcher with fake extraction.
- `application_identity.rs`, `application_names.rs`, `application_update.rs` (#54).
- `arguments.rs` — argument form for required arguments.
- `calculator.rs`, `quicklinks.rs`, `files.rs`, `clipboard.rs`, `search_files.rs` — default extensions with fakes.
- `clipboard_adapter.rs` / `clipboard_adapter_linux.rs` / `clipboard_adapter_macos.rs` — separate real-clipboard targets.
- `clipboard_view.rs` — Clipboard History split view projection (#102, #166).
- `command_search.rs`, `confirmations.rs` (#146), `credentials.rs` (#130), `dependencies.rs`, `develop.rs`, `develop_builds.rs` (gated by `PANE_TEST_JS_BUILDS=1`), `disable.rs`, `disable_dependents.rs`, `extension_log.rs`, `extension_pages.rs` (#168), `feedback.rs` (#141), `file_actions.rs` (#150), `file_index.rs`, `helpers.rs`, `hotkeys.rs` (fake `Hotkeys`), `icons.rs` (#139), `installer.rs` (default extensions from a local artifact source), `item_actions.rs` (#137), `launcher.rs`, `list_tree.rs`, `local_channel.rs` (#217), `memory.rs` (#120), `no_view.rs` (ADR 0037), `npm.rs`, `operations.rs`, `packages.rs`, `paste.rs` (#148), `pausing.rs` (#18), `preferences.rs` (#143), `program_adapters.rs` (gated by `PANE_TEST_PROCESS_TREES=1`), `programs.rs` (#147), `quick_slots.rs`, `reload.rs`, `repositories.rs`, `result_actions.rs`, `root_providers.rs` (#164), `runtime_cache.rs`, `runtime_crash.rs`, `runtime_timers.rs` (#190), `samples.rs`, `schedules.rs`, `search.rs`, `stopping.rs`, `submenus.rs` (#140), `system.rs` (**the model for fake-adapter launcher tests**), `system_icon_adapters.rs` (`PANE_TEST_SYSTEM_ICONS == "1"`, `opted_in()` lines 20–23), `uninstall.rs`, `uninstall_dependents.rs`, `unresponsive.rs`, `update.rs`, `web_icons.rs` (#142).

### `crates/pane-core/tests/support/` — the doubles
- `system.rs` — **`RecordingSystem`**: implements `pane_core::system::System` (copy/read_clipboard/open/reveal/trash/can_paste/paste_clipboard/front_application/selected_text) and `Applications`. Records `Done::{Copied, Opened, Revealed, Trashed, Pasted}` in a `Mutex<Vec<Done>>`, read with `take()`. Own clipboard, `support_paste()`, `fail_paste(why)`, `copy_while_pasting`, `set_front_application`, `set_selected_text`. Every path not told otherwise answers `SystemError::NotAvailable(NOT_YET)` = `"Not available in this test"`. `KEPT = ("Keep me.txt", "It is open in another program")` is the one file it refuses to trash. `installed_applications()` = Zed, Notepad, code editor.
- `feedback.rs` — **`RecordingWindow`**: implements `pane_core::WindowControl` (hide/show_hud/confirmation), records `WindowRequest`s; `RecordingWindow::attach(&launcher)`; helpers `huds()`, `confirmations()`, `hides()`. `shown(launcher)` reads the toast as the old status line.
- `guests.rs` — where the guests live: `target/guests` (`CARGO_MANIFEST_DIR/../../target/guests`), `guest(name)` → `<name>.wasm`, panics `"{} is missing; run `cargo xtask guests`"`.
- `rows.rs` — `titles(launcher)` (row titles in order), `select_title(launcher, title)`, `to_root`, `manage` (opens extension manager from root search), `MANAGE_ROW = "Manage Extensions"`.
- `platforms.rs` — the current platform, the two others, and the exact "Not available on …" wording the samples use; shared by pane-core and pane tests.
- `artifacts.rs` — a local artifact source (default extensions) on 127.0.0.1, with scripted `Status/Drop/Stall` behaviors.
- `npm_registry.rs`, `repo_server.rs`, `image_server.rs`, `service.rs`, `unreachable.rs`.

### How a Launcher is constructed in tests (`crates/pane-core/tests/system.rs` lines 167–220)
```rust
let sources = tempfile::tempdir().unwrap();
let data = tempfile::tempdir().unwrap();
let system = Arc::new(RecordingSystem::default());
let runtime = Runtime::start().unwrap();
runtime.set_applications(system.clone());
let mut launcher = Launcher::with_packages(Ok(runtime), vec![], data.path().join("extensions"));
if with_system { launcher = launcher.with_system(system.clone()); }
let window = RecordingWindow::attach(&launcher);
let folder = copy(fixture.package, &sources.path().join(fixture.package)); // copies target/guests/packages/<name>
block_on(launcher.install_package(&folder));
block_on(launcher.set_query(fixture.command));
select_title(&launcher, fixture.command);
block_on(launcher.activate_selected());
```
Hotkeys fake in `crates/pane-core/tests/hotkeys.rs`: `Launcher::with_packages(Runtime::start(), vec![], self.packages_dir()).with_hotkeys(system.clone())` (line 136), and the fake's `press()` drives `launcher.press_hotkey(&shortcut)` + `block_on(opening)` (lines 60–70). `crates/pane-core/tests/launcher.rs` uses the plain `Launcher::new(Runtime::start(), commands)` with `CommandRegistration`s pointing at `target/guests/*.wasm`.

### Every-system vs opt-in
- The `integration` binary is pure logic + real wasm guests + local servers; it runs on **every OS** of every tests matrix (Linux, Windows, macOS in `ci.yml`; Linux+Windows at ci-fast verify; Linux-only per-push in ci-branch).
- Real-system adapter tests are their own binaries in `crates/pane-core/Cargo.toml` (`[[test]]` entries at the end, lines ~110–150): `integration`, `clipboard_adapter` (Windows-only file: `#![cfg(target_os = "windows")]` at line 26), `clipboard_adapter_linux`, `clipboard_adapter_macos`, `hotkey_adapters`, `program_adapters`. Gating:
  - `clipboard_adapter.rs`: `if std::env::var("PANE_TEST_REAL_CLIPBOARD").as_deref() != Ok("1") { eprintln!("skipped: set PANE_TEST_REAL_CLIPBOARD=1 to let it replace the clipboard"); return; }`; plus a `static SERIAL: Mutex<()>` so its tests run one at a time, and `.config/nextest.toml` puts the whole binary in a `real-clipboard` test-group `max-threads = 1`.
  - `clipboard_adapter_linux.rs`: same env var and needs a display; CI runs it under `xvfb-run -a cargo nextest run --locked --workspace --retries 2 -E 'binary(clipboard_adapter_linux)'` in one shard.
  - `hotkey_adapters.rs`: platform modules, no env opt-in on Windows (see §8).
  - `program_adapters.rs`: `PANE_TEST_PROCESS_TREES=1`.
  - `system_icon_adapters.rs`: `PANE_TEST_SYSTEM_ICONS=1`.
  - `develop_builds.rs`: `PANE_TEST_JS_BUILDS=1` (set only in `ci.yml`'s `js-guests-tests` job).
- `.cargo/config.toml` sets `PANE_TEST_CODE_CACHE = target/test-code-cache` for every cargo run.
- `.config/nextest.toml`: `fail-fast = false`, priority override for `test(/memory_peaks/)`, `success-output = "immediate"` for memory tests, `real-clipboard` group.

### How new opt-in real-input Windows tests would be gated in CI
The opt-ins live in the composite action `.github/actions/setup/action.yml` "Opt in to the tests that need a CI runner" step, so every test job that uses `with: tests: true` (ci-fast verify `tests`, ci-fast `chosen`, ci-branch `tests`, ci.yml `tests` and `js-guests-tests`) gets them:
```yaml
    - name: Opt in to the tests that need a CI runner
      if: inputs.tests == 'true'
      shell: bash
      run: |
        {
          echo "PANE_TEST_PROCESS_TREES=1"
          if [ "$RUNNER_OS" != Linux ]; then echo "PANE_TEST_REAL_CLIPBOARD=1"; fi
          if [ "$RUNNER_OS" = Windows ]; then echo "PANE_TEST_SYSTEM_ICONS=1"; fi
        } >> "$GITHUB_ENV"
```
A `PANE_TEST_REAL_INPUT` would follow `PANE_TEST_SYSTEM_ICONS`'s line shape: `if [ "$RUNNER_OS" = Windows ]; then echo "PANE_TEST_REAL_INPUT=1"; fi`. The alternative pattern is a dedicated Windows step like ci-fast.yml's (lines 105–112) / ci-branch.yml's (lines 115–123) / ci.yml's (lines 220–228):
```yaml
      - name: Run the Linux clipboard adapter test (Xvfb)
        if: runner.os == 'Linux' && matrix.shard == '1/3'
        run: xvfb-run -a cargo nextest run --locked --workspace --retries 2 -E 'binary(clipboard_adapter_linux)'
        env:
          PANE_TEST_REAL_CLIPBOARD: "1"
```
Windows test jobs also get `TMP`/`TEMP` moved to a faster drive (`.github/actions/setup/fast-temp.ps1`: D: or a Dev Drive, with a USN change journal, appended to `GITHUB_ENV`).

## 2. `crates/pane/tests/` — the native windows on GPUI's test platform

`crates/pane/tests/main.rs` mirrors pane-core: one `integration` binary, `mod name;` per file, guarded by `every_test_file_is_compiled`. Files: aliases, announcements, application_icons, arguments, command_search, compact_pins, confirmations, crash_record, default_icons, develop, extension_log, feedback, file_actions, file_search_settings, hotkeys, icons, install, item_actions, keyboard, launcher_settings, no_view, npm, open_pane, preferences, quicklinks, repositories, runtime_crash, search_files, settings, settings_search, shortcuts, submenus, system, tray, unresponsive, update, virtual_lists, web_icons, window.

### How a window is built (`crates/pane/tests/window.rs` lines 117–131, `keyboard.rs` 92–106)
```rust
cx.executor().allow_parking();        // guest replies arrive from real runtime threads
cx.update(pane::bind_keys);           // the app's key bindings
cx.add_window_view(|window, cx| LauncherWindow::new(launcher, window, cx))
```
Settings are initialized first, as the binary does: `init_settings(data, cx)` from `support/setup.rs` → `pane::settings::init_with_overrides(...)` (before `bind_keys`, so recorded rebinds are in force). Tests are `#[gpui::test] fn name(cx: &mut TestAppContext)`, then work through `VisualTestContext`. The launcher is given a fake hotkeys system via `Launcher::new(...).with_hotkeys(system.clone())` or `Launcher::with_packages(Runtime::start(), vec![], data.join("extensions")).with_hotkeys(...)`.

### Fake hotkeys systems
Every window test defines its own small fake implementing `pane_core::hotkeys::{Hotkeys, HotkeyError, Shortcut}`:
- `hotkeys.rs`/`keyboard.rs`/`settings.rs`: `FakeSystem { registered: Mutex<Vec<Shortcut>> }` — `register` pushes, `unregister` retains.
- `open_pane.rs`/`shortcuts.rs`: adds `taken: Mutex<Vec<Shortcut>>` → `register` returns `Err(HotkeyError::Taken)`; plus `UnavailableSystem(String)` whose `unavailable()` returns `Some(reason)` and `register` returns `HotkeyError::Refused`.
- `tray.rs`: same shapes (`FakeHotkeys`, `UnavailableHotkeys`).

### Driving input
- Keys: `cx.simulate_keystrokes("enter")`, `"ctrl-alt-p"`, `"down down down down enter"`; text: `cx.simulate_input("zzz")`; clicks: `cx.simulate_click(bounds.center(), Modifiers::none())`.
- A **global hotkey press** is driven directly: `window.update_in(cx, |window, w, cx| window.hotkey_pressed(&shortcut, w, cx))` (`open_pane.rs` line 209), with `press()` first doing `cx.executor().advance_clock(Duration::from_millis(700))` to step past the repeat guard.
- Focus checks: `handle.is_active(cx)`; visibility: `window.hidden()`.
- Settings window: `cx.simulate_keystrokes(settings_shortcut())` (ctrl-/cmd-comma), then find the `SettingsWindow` among `cx.windows()`.
- Drawn content: `cx.debug_bounds("selector")` (elements tag themselves with `.debug_selector(...)`), `window.drawn_view()`, `window.drawn_over()` in `support/settle.rs`; painted quads in `support/paint.rs`.
- Accessibility: `support/a11y.rs` — `a11y(cx)` forces the tree and returns `window.debug_a11y_tree_json()`; `accessibility()` → (focused label, JSON); `announcement(cx)`.
- Waiting: `support/settle.rs` — `settle`, `settle_bare`, `until(view -> bool)`, `settle_shown`. 60 s deadlines; 5 ms sleeps. `support/wait.rs` — `until(cx, done)`, `until_record_holds(cx, data, "\"open_pane\": \"…\"")`.

### `hotkeys.rs` structure (2 tests)
1. `pressing_keys_on_the_hotkey_screen_assigns_them_and_the_hotkey_opens_the_command` — install the "hello" package, Enter to install, `enter_flow`, `down down down down enter` → `Screen::Hotkey`, title `"Hotkey for Say hello"`; a bare key `"p"` is explained and the screen stays; `"ctrl-alt-p"` records; registered list is `[Shortcut::open_pane_default(), shortcut]`; then `window.hotkey_pressed(...)` opens the command, Enter runs "Hello from the Rust guest", Escape returns to `Root { query: "" }`.
2. `a_command_hotkey_cannot_take_the_open_pane_keys` — `"ctrl-alt-space"` refused with `"{open_pane} opens Pane itself: choose another shortcut for Say hello, or change Pane's hotkey in Settings."`; nothing written to `data/extensions/hotkeys.json`.

### `open_pane.rs` structure (12 tests, ~700 lines)
Helpers: `press`, `hidden`, `handle_of`, `click(selector)`, `until_diag`, `until_record`, `open_settings`, `is_active`. Tests: default registers at startup and toggles focus/hidden/shown; held key repeats ignored (`advance_clock` past the 700 ms guard); Settings' focus doesn't count; recording swaps registration and persists; a shortcut another application has is refused and keeps the binding; collision with a command hotkey refused; a save that fails rolls the registration back; a fresh application registers the recorded hotkey; Escape cancels the recorder; Reset; the hotkey stays available with a failed runtime; the unavailable Wayland-like system explains on the General page.

### `shortcuts.rs` structure (~2100 lines)
Fake: `FakeHotkeys { registered, taken }`. `open(cx, &data, &[&query, &hello])` builds the launcher + Settings over the query (Echo) and hello (Rust sample) packages; `seed(&data, aliases, hotkeys)` writes the record. Asserts with `debug_bounds("shortcut-filter"/"shortcut-columns"/"shortcut-group-<key>"/"shortcut-row-<command-id>")` and a11y labels `"Alias for Echo: ec"`, `"Hotkey for Echo: <keys>"`, then inline alias editing, inline hotkey recording with collisions/rollbacks/refusals, the filter, the catalog following package lifecycle and a restart. `open_pane_keystrokes()` = `"alt-space"` on macOS, `"ctrl-alt-space"` elsewhere.

## 3. The registry-test pattern — does any test touch the registry?

**No test reads or writes the Windows registry today.** Source uses:
- `crates/pane-core/src/autostart/windows.rs` — the Run key adapter: `const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";`, value `NAME: &str = "Pane"`, `command(exe)` = the path in double quotes; `enable` writes `REG_SZ` UTF-16 bytes via `RegSetValueExW` through `open(KEY_SET_VALUE, …)`; its only tests are pure in-file: `#[cfg(test)] mod tests` → `the_command_quotes_the_programs_path` and `the_value_is_the_per_user_run_keys_slot` — compiled only on Windows.
- `crates/pane-core/src/applications/start_menu.rs:810` — reads `HKEY_CLASSES_ROOT` (`has_handler`, `#[cfg(windows)]`).
- `crates/pane-core/src/programs/search.rs:166,181` — reads `HKEY_CURRENT_USER\Environment` (the search path).
- `crates/pane-core/src/tray/windows.rs:65,444` — reads `HKEY_CURRENT_USER` taskbar settings.

The intended pattern for Run's RunMRU tests:
- The closest structural precedents are (a) `hotkey_adapters.rs`'s Windows module (registers real hotkeys with the session, using an unlikely combination `ctrl+alt+shift+f9/f10` so the session's own shortcuts are not disturbed) and (b) `application_adapters.rs`'s Windows module (makes real `.lnk` files in a temp folder and reads them back through the adapter). A RunMRU adapter test that writes a **test-owned key** under `HKEY_CURRENT_USER` with a Pane-test-specific name would be the first of its kind; everything format-shaped goes in the cross-platform pure half first (`#[cfg(any(windows, test))]` if needed). `windows-registry`-style APIs are reached through the `windows` crate (`Win32_System_Registry` feature, `crates/pane-core/Cargo.toml` target dep).

## 4. Native GUI smokes

- **Phases are defined inline in the scripts** — there is no phase table and no xtask smoke task: `scripts/smoke-windows.ps1` (~2540 lines), `scripts/smoke-linux.sh` (~2400), `scripts/smoke-macos.sh` (~2300). A phase: creates its own data folder (`$data = Join-Path $OutDir "hotkeys-data"`, `Remove-Item` first, `$env:PANE_DATA_DIR = $data`), starts Pane (`Start-Pane "stderr-<phase>.log" @("--install", "target/guests/packages/<sample>")`), drives it, captures numbered screenshots (`Capture "52-hotkey-screen.png"`), checks them (`Check <name> <color>`, `check_screenshot.py`), and `Stop-Pane $process`. Numbering is sequential per script early on (1–77) and grouped later (200s runtime, 220s files, 240s unresponsive, 260s npm, 280s/400s clipboard, 300s git).
- **The hotkeys phase** (`smoke-windows.ps1` lines 779–848): `Open-Extension "Settings sample"` → `Press-Named "Hotkey for Greeting:" -Prefix` → `Wait-Shown "Recording; Hotkey for Greeting"` → `Capture "52-hotkey-screen.png"` → `Send "^%g"` → `Wait-For … extensions/hotkeys.json '"ctrl+alt+g"'` → restart → disable the extension → `--same` checks. Hotkey presses use **PowerShell SendKeys**: `function Send($keys) { [System.Windows.Forms.SendKeys]::SendWait($keys) }`. Linux presses hotkeys with `xdotool key ctrl+alt+space` inside the script's own Xvfb (`PANE_XVFB`/`PANEXDOTOOL`).
- **Registering a shortcut from another process: nothing in the repo does this today.** The smokes only press keys; the only cross-registration precedent is in-process (`hotkey_adapters.rs` starts a second `WindowsHotkeys::start(other_sender)` in the same test process). A hook-dispatched binding smoke would need a new helper — the smoke scripts already embed C# via `Add-Type @" … "@` (`Win` class with `SetForegroundWindow`, `mouse_event`, etc.), so that is the house style.
- **Settings is driven through UI Automation**, not keys (`Press-Named`/`Find-Named`/`Wait-Shown`/`Type-Field`/`Manage-Extensions`/`Open-Extension`/`Extension-Action`/`Close-Settings`, lines 137–250): finds elements by `AutomationElement.NameProperty` in the "Settings" window of Pane's process.
- **Output**: everything into `-OutDir` (default `"smoke"`), including screenshots, per-phase `data/` folders, `stderr-*.log`, `system.txt`.
- **Opt-in standalone Windows smokes** (the model for a new one): `scripts/smoke-windows-hud.ps1` (#141) and `scripts/smoke-windows-system.ps1` (reveal/Recycle Bin via `PANE_TEST_SYSTEM_LOG`, `PANE_TEST_REVEAL`, `PANE_TEST_TRASH`). The binary side of those hooks is `crates/pane/src/main.rs` (`#[cfg(debug_assertions)] fn smoke_system(log: PathBuf)`; `PANE_TEST_SHOW_HUD`; `PANE_TEST_RUNTIME_FAULTS` `runtime.watch_fault_file`).
- **How CI runs smokes — release matrix only** (`ci.yml`): the `smoke` job builds `cargo build --locked -p pane` + `cargo xtask guests`, installs Pillow, runs `python -m unittest discover -s scripts -p test_check_screenshot.py`, then per OS runs the smoke script. Artifacts: `gui-smoke-${{ runner.os }}`. Manual narrowing: `gh workflow run ci.yml --ref <branch> -f only=smoke [-f smoke_os=windows-2025]`. Neither ci-fast.yml nor ci-branch.yml runs any smoke.

## 5. CI workflows (quoted)

`ci-fast.yml` quick tier (pushes to `pi-subagent/**`):
```yaml
  check:
    name: Check (${{ matrix.os }})
    if: ${{ !inputs.verify && !inputs.tests }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-24.04, windows-2025]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup
      - name: Quick compile check
        run: |
          cargo fmt --all --check
          cargo check --locked --workspace --all-targets
```
Verify tier (manual `gh workflow run ci-fast.yml --ref <branch> -f verify=true`): `lints` (ubuntu+windows, `cargo xtask ci-lints`), `tests` (ubuntu+windows, shards `1/3..3/3`, `cargo xtask ci-tests --partition hash:N/3`, plus the Linux Xvfb clipboard step), `macos-check` (macos-15: fmt+check and `cargo test --locked -p pane-core --test clipboard_adapter_macos` with `PANE_TEST_REAL_CLIPBOARD: "1"`), `chosen` (`-f tests='test(/file_index/)'` on all three OS), `js-guests` rebuild, and the `passed` summary job.

`ci-branch.yml` (every non-main, non-pi-subagent push; fork PRs): a `changes` job decides `verify`; then `lints` on **ubuntu+windows+macos** (`cargo xtask ci-lints`), and `tests` on **ubuntu-24.04 only**, three shards, shard 1 running the Xvfb clipboard step, shard 2 running `cargo xtask file-index-guard`. Ends in `CI passed`.

`ci.yml` (release matrix, main + dispatch): `windows-renderer`, `macos-renderer`, `licenses`, `lints` (all 3 OS; on main also builds tests for the cache), `tests` (all 3 OS × 3 shards), `smoke`, `package`, `js-guests` + `js-guests-tests` (`PANE_TEST_JS_BUILDS: "1"`), `passed`.

`.github/actions/setup/action.yml`: Linux deps (incl. `xvfb xdotool imagemagick`), mold link flags, pinned toolchain only, one rust-cache per OS (`shared-key: pane`, saved only by main's lints), `cargo-nextest@0.9.146` when `tests: true`, the opt-in env step, Windows `fast-temp.ps1`.

## 6. rustfmt

There is **no `.rustfmt.toml` or `rustfmt.toml` anywhere** and no formatting keys in the root `Cargo.toml` `[workspace.package]` (which holds only `edition = "2024"`, `publish = false`, `rust-version = "1.98"`, `license`). The toolchain is pinned by `rust-toolchain.toml`: `channel = "1.98.1"`, `components = ["rustfmt", "clippy"]`. CI enforces `cargo fmt --all --check` (and `ci-lints` also runs it in `guests`, `guests/fixtures/mixed-p2`, `guests/helpers/echo`, `guests/hello-rust`). Formatting is therefore **rustfmt's defaults for edition 2024 on the pinned toolchain** — the numbers you must hand-format to are the defaults: `max_width = 100`, `fn_call_width = 60`, `chain_width = 60`, `struct_lit_width = 18`, `single_line_if_else_max_width = 50`, 4-space indent, no hard tabs. House style: doc comments wrapped around column 80; long string literals broken with `\` continuation and aligned continuation lines; imports grouped and sorted with `use …::{A, B}` braces folded.

## 7. How Windows-gated code is structured so the other legs still compile

Three patterns, all verified:
1. **Cross-platform pure half + cfg-gated adapter modules** (`hotkeys.rs` lines 29–40): `Shortcut` (parse/new/id/display, `FUNCTION_KEYS`, key validation) lives in the shared file and is compiled everywhere; the adapters are:
   ```rust
   #[cfg(target_os = "linux")]  mod x11;     pub use x11::X11Hotkeys;
   #[cfg(target_os = "windows")] mod windows; pub use windows::WindowsHotkeys;
   #[cfg(target_os = "macos")]   mod macos;   pub use macos::MacHotkeys;
   ```
   So `crates/pane-core/src/hotkeys/windows.rs` **is `#[cfg(target_os = "windows")]`-gated** (by its parent's `mod` declaration; the file itself has no inner cfg). Same shape: `system.rs`, `autostart.rs`, `tray.rs`, `system_icons.rs`, `clipboard.rs`, `threads.rs`.
2. **The whole adapter file compiled everywhere, with `#[cfg(windows)]` only on the API-touching functions** — `applications.rs` declares `mod start_menu;` with no cfg: "Finding is plain file system work and is compiled on every system, so each adapter's discovery is tested everywhere with fixture folders; opening uses the system's own launcher and works only on its system." Windows-API functions inside `start_menu.rs` (e.g. `has_handler`) carry their own `#[cfg(windows)]`.
3. **`#[cfg(any(windows, test))]`** — `crates/pane-core/src/applications/icons.rs` line 59: `#[cfg(any(windows, test))] mod click_once;` whose doc says: "Plain file system reading, so the tests build a store of their own on any system." This is the established way to make a **pure Windows-format parser testable on Linux and macOS too** (the module compiles in every test build).
- Windows-only deps are target-gated in `crates/pane-core/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows = { workspace = true, features = [… "Win32_System_Registry" …] }`). The macOS/Linux legs never compile the `#[cfg(windows)]` modules; macOS-gated code is compiled by ci-branch's lints, ci-fast's `macos-check`, and the release matrix.

## 8. `crates/pane-core/tests/hotkey_adapters.rs` in detail

Separate `[[test]] name = "hotkey_adapters"` binary in Cargo.toml. Module doc: "Each system's global hotkey adapter against the real system: a shortcut registers, a second registration of it (as another application would make) is refused as taken, and releasing it, or dropping the adapter, frees it for others. … On Windows the hotkeys are registered with the session the tests run in; a press is made by the GUI smoke, not here. macOS registers hotkeys with the main thread's run loop, which a test thread does not run, so the macOS adapter is checked by the GUI smoke only."
- Imports: `#[cfg(any(target_os = "linux", target_os = "windows"))] use pane_core::hotkeys::{HotkeyError, Hotkeys, Shortcut, channel};`
- `#[cfg(target_os = "linux")] mod x11 { … }` — starts its own Xvfb on a random display, `X11Hotkeys::connect(&server.display, sender)`; tests: `a_grabbed_shortcut_is_reported_refused_to_others_and_released` (xdotool press reported once), `dropping_the_adapter_releases_its_grabs`, `a_display_that_cannot_be_reached_is_explained`. Env vars: `PANE_XVFB`, `PANE_XDOTOOL`.
- `#[cfg(target_os = "windows")] mod windows { … }` (line 180) — `use pane_core::hotkeys::WindowsHotkeys;`:
  - `a_registered_shortcut_is_refused_to_others_and_released`: `Shortcut::parse("ctrl+alt+shift+f9")` ("An unlikely combination, so the session's own shortcuts are not disturbed"), `WindowsHotkeys::start(sender)` (twice, two adapters in one process), second registration → `Err(HotkeyError::Taken)`, `unregister` frees it.
  - `dropping_the_adapter_releases_its_hotkeys` (`ctrl+alt+shift+f10`).
  - **No env gating on Windows** — these run on every Windows test job (ci-fast verify, ci.yml tests) because CI's Windows runner has an interactive session. A new real-input test would gate itself with an env var, as `system_icon_adapters.rs` does.

## 9. `diagnostics.rs` and Copy Diagnostics (for #259 hook health)

`crates/pane-core/src/diagnostics.rs` is Pane's one diagnostic path (`report`, the `diagnostic!` macro, `report_line`), the size-capped/redacted log, the panic hook, and the crash record; `pub fn redacted(text)` redacts home/`<user>`/`<computer>` for user-facing copies. The **Copy Diagnostics text** is assembled in `crates/pane/src/features/settings/about.rs`:
- `fn diagnostics(update: &ApplicationUpdate, log: Option<&pane_core::LogNotice>) -> String` (lines 588-608): `"Pane {APP_VERSION}"`, `"\nBuilt for {target.id()}"`, `"\nData folder: {redacted}"`, `"\nLog folder: {redacted}"` + `"\nLast run: Pane quit unexpectedly"`, `"\nUpdate check: {update_line(update)}"`.
- The button: `AboutControl::Diagnostics` → builds the report at click time, `cx.write_to_clipboard(ClipboardItem::new_string(report))`. Label `"Copy diagnostics"`. A hook-health line for #259 would be added to this `diagnostics(...)` assembly (and its test is in `crates/pane/tests/settings.rs`'s About-page tests).

## 10. xtask tasks (`xtask/src/main.rs`)

- `guests` — builds the two guest workspaces for `wasm32-wasip2` release, copies 30 components to `target/guests`, copies the 34 `PREBUILT` JS/TS components from `guests/prebuilt`, assembles the 59 `SAMPLE_PACKAGES` into `target/guests/packages`, builds the `pane-echo` native helper per target, packs the npm sample tarball, assembles the Git sample.
- `js-guests` — rebuilds `guests/prebuilt` with the pinned componentize-js toolchain, then `guests`.
- `ci` — `ci_lints` then `ci_tests`.
- `ci-lints` — `cargo fmt --all --check` (root + 4 guest dirs), `pane_js.py check`, `sdks`, `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- `sdks` — `cargo publish --dry-run` for `pane-extension`, WIT-copy check, `npm pack` into `target/sdks`.
- `ci-tests` — `guests()` then `cargo nextest run --locked --workspace --retries 2` + options (`--partition hash:N/3`, `-E <filter>`, `--no-run`).
- `file-index-bench`, `file-index-guard` (#183), `package-linux`/`package-windows`/`package-macos`.
- **There is no smoke task** — smokes are the `scripts/smoke-*` files driven directly by ci.yml's smoke job.

## Where new work goes

**Pure-logic tests (recognizer, window predicate, Run parser, system-command decisions).** New file `crates/pane-core/tests/<name>.rs` (or an existing one) + `mod <name>;` in `crates/pane-core/tests/main.rs` — `every_test_file_is_compiled` fails otherwise. These run on all three OSes in every tests matrix. The code they test must be compiled everywhere: put pure functions in the shared module and cfg-gate only the Windows-API half — or, for Windows-format parsing that should be tested on every system, use the `#[cfg(any(windows, test))] mod …;` trick from `applications/icons.rs`/`icons/click_once.rs` ("Plain file system reading, so the tests build a store of their own on any system") — exactly the pattern for RunMRU's Explorer format. Pure halves of `#[cfg(windows)]` files can also be unit-tested in a `#[cfg(test)] mod tests` inside the file, as `autostart/windows.rs` does — but those then run on Windows only.

**Fake-adapter launcher tests.** pane-core level: `crates/pane-core/tests/<feature>.rs` with `#[path = "support/…"] mod` imports, `Launcher::with_packages(Ok(Runtime::start()), vec![], data.join("extensions"))` (+ `.with_system(Arc<RecordingSystem>)`, `.with_hotkeys(Arc<FakeSystem>)`), `RecordingWindow::attach`, `runtime.set_applications(...)`, guests from `target/guests[/packages]`, driven with `block_on(launcher.activate_selected())`, `launcher.set_query(...)`, `rows::{titles, select_title}` and `feedback::shown` (model: `system.rs`, `hotkeys.rs`). GPUI window level: `crates/pane/tests/<feature>.rs` + `mod` in its main.rs, `init_settings` + `bind_keys` + `cx.add_window_view(LauncherWindow::new …)`, a local `FakeHotkeys` (model: `hotkeys.rs`, `open_pane.rs`, `shortcuts.rs`, `settings.rs`'s `FakeLogin`, `launcher_settings.rs`).

**Opt-in real-input Windows tests.** Follow `hotkey_adapters.rs`/`clipboard_adapter.rs`: either a new `[[test]]` target in `crates/pane-core/Cargo.toml` ("The adapters against the real clipboard and hotkeys keep a process of their own, apart from the tests running beside them") or a `#[cfg(windows)] mod windows { … }` in an existing adapter test. Gate the body on a new env var — `PANE_TEST_REAL_INPUT=1` does not exist yet; copy `system_icon_adapters.rs`'s `opted_in()` + skip-with-eprintln shape. Wire CI once in `.github/actions/setup/action.yml`'s opt-in step (`if [ "$RUNNER_OS" = Windows ]; then echo "PANE_TEST_REAL_INPUT=1"; fi`) so ci-fast verify `Tests (windows-2025, N/3)`, ci.yml's `Tests (windows-2025, N/3)` and the `chosen` job all get it; or add a dedicated Windows step with `if: runner.os == 'Windows'` and `env: PANE_TEST_REAL_INPUT: "1"` like the Xvfb clipboard step. Add the job to the workflow's `passed` `needs` if you add a job. A nextest `test-group` in `.config/nextest.toml` if the tests can't run in parallel.

**Registry-reading adapter tests (Run's RunMRU).** Create the pattern: a `#[cfg(windows)] mod windows` that creates a key under `HKEY_CURRENT_USER` with a Pane-test-specific name (as `hotkey_adapters.rs` uses unlikely hotkey combinations and `application_adapters.rs` makes its own `.lnk` files), writes the Explorer RunMRU format through the adapter, and reads it back; everything format-shaped goes in the cross-platform pure half first.

**A smoke phase.** Inline in `scripts/smoke-windows.ps1` (Linux: `smoke-linux.sh`): own `$OutDir` data folder, `Start-Pane "stderr-<phase>.log" @("--install", …)`, numbered `Capture`/`Check`/`Capture-Until` screenshots (next free numbers; hotkeys used 52–58), assertions via `Check-Pane-In-Front`/`Wait-For`/`Select-String` on the data records, `Stop-Pane`. Pressing hotkeys = SendKeys; driving Settings = UI Automation `Press-Named`. Registering a shortcut **from another process** needs a new helper — embed C# via `Add-Type` (the scripts' `Win` class style) or spawn a helper binary; no existing precedent. If the phase needs Pane to do one thing at startup, add a `#[cfg(debug_assertions)]` env hook in `crates/pane/src/main.rs` like `PANE_TEST_SHOW_HUD`/`PANE_TEST_REVEAL`, and consider a standalone opt-in `scripts/smoke-windows-<thing>.ps1` ("run it by hand, or from a ticket's validation") if it must not join the release matrix. Smokes run only in `ci.yml`'s `smoke` job; evidence lands in the `gui-smoke-<os>` artifact.

**Hand-formatting rules.** No rustfmt config file exists; the style is rustfmt defaults on the pinned 1.98.1 toolchain with edition 2024: `max_width = 100`, `fn_call_width = 60` (break argument lists past 60 columns), `chain_width = 60` (break method chains past 60), 4-space indent, wrapped doc comments (~80 cols), long string literals split with `\` and aligned continuations. `cargo fmt --all --check` (run over the root and the four guest workspaces by `cargo xtask ci-lints`) is what CI enforces — match what the existing files show, since you cannot run rustfmt locally.
