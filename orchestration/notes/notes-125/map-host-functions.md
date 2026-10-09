# Pane codebase map — extension host functions / WIT contract / SDK side

Repo: `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\spec-125` (branch `spec/125-windows-power`, at `origin/main`). Read‑only exploration; no builds run.

## 1. `wit/system.wit` — the system interface

File: `wit/system.wit` (package `pane:extension@0.1.0`). Every command imports it, whatever its mode, through the `extension` world (`wit/extension.wit` line ~230: `world extension { import commands; import window; import feedback; import system; export command; }`).

Declared, exactly:

```wit
interface system {
  variant clip { text(string), file(string) }
  enum host-system { windows, macos, linux, other }
  record not-trashed { path: string, reason: string }
  running-on: func() -> host-system;
  copy: func(content: clip, concealed: bool) -> result<_, string>;
  read-clipboard: func() -> result<option<clip>, string>;
  open: func(target: string, application: option<string>) -> result<_, string>;
  reveal: func(path: string) -> result<_, string>;
  trash: func(paths: list<string>) -> result<_, list<not-trashed>>;
  variant system-error { not-available(string), failed(string) }
  record front-app { name: string, icon: option<string> }
  paste: func(content: clip) -> result<_, system-error>;
  front-application: func() -> result<option<front-app>, system-error>;
  selected-text: func() -> result<option<string>, system-error>;
}
```

Doc comments in the WIT are explicit that `paste`/`front-application`/`selected-text` answer `not-available` "on macOS and Linux (X11 included), and on Windows until Pane's Windows power features land", and that `not-available` "is never a reason to pause the extension" — the SDKs' Paste copies instead.

**Host implementation.** Trait `System` in `crates/pane-core/src/system.rs`:

```rust
pub trait System: Send + Sync + 'static {
    fn copy(&self, clip: &Clip, concealed: bool) -> Result<(), String>;
    fn read_clipboard(&self) -> Result<Option<Clip>, String>;
    fn open(&self, target: &str, application: Option<&str>) -> Result<(), String>;
    fn reveal(&self, path: &Path) -> Result<(), String>;
    fn trash(&self, paths: &[PathBuf]) -> Vec<NotTrashed>;
    // The seam of the Windows power features (#125): defaults say "not yet".
    fn can_paste(&self) -> Result<(), SystemError> { Err(not_yet(PASTE)) }
    fn paste_clipboard(&self) -> Result<(), SystemError> { Err(not_yet(PASTE)) }
    fn front_application(&self) -> Result<Option<FrontApplication>, SystemError> { Err(not_yet(FRONT_APPLICATION)) }
    fn selected_text(&self) -> Result<Option<String>, SystemError> { Err(not_yet(SELECTED_TEXT)) }
}
```

- `native()` (system.rs ~line 245) returns `windows::WindowsSystem` / `macos::MacosSystem` / `linux::LinuxSystem::new()` / `Unavailable` elsewhere; `none()` returns `Unavailable("Not available: this Pane does not reach the system's clipboard, open things or recycle them")`.
- `not_yet(what)` (system.rs ~180) formats: `"{what} is not available on {Windows|macOS|Linux} yet"` with:
  - `PASTE = "Pasting into another application"`
  - `FRONT_APPLICATION = "Reading the application in front"`
  - `SELECTED_TEXT = "Reading the selected text"`
  - `PASTE_FALLBACK = "Copied — paste is not available here yet"` (pub const, shared with Clipboard History, computed answers and both SDKs).

**Per-adapter status:**

- **Windows** (`crates/pane-core/src/system/windows.rs`, `struct WindowsSystem`): implements **only** `copy`, `read_clipboard`, `open`, `reveal`, `trash`. It does *not* override `can_paste`/`paste_clipboard`/`front_application`/`selected_text`, so all four answer `SystemError::NotAvailable("Pasting into another application is not available on Windows yet")` etc. (asserted by system.rs test `this_systems_adapter_says_paste_front_application_and_selected_text_are_not_available_yet`). `copy` → `put_clip`, `read_clipboard` → `read_clip(MAX_CLIPBOARD_TEXT)` from `crate::clipboard::windows`; `open` → `shell_execute` (`ShellExecuteExW` with `SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI`, an application given the target as its one quoted argument); `reveal` → `ILCreateFromPathW` + `SHOpenFolderAndSelectItems`; `trash` → `SHFileOperationW` (`FO_DELETE` with `FOF_ALLOWUNDO|FOF_NOCONFIRMATION|FOF_NOERRORUI|FOF_SILENT|FOF_WANTNUKEWARNING`).
- **macOS** (`system/macos.rs`): pasteboard via `crate::clipboard::macos` (concealed = `org.nspasteboard.ConcealedType`), `/usr/bin/open` (`-a`, `-R`), `NSFileManager::trashItemAtURL`. Also does not override the seam.
- **Linux** (`system/linux.rs`): text via the X11 clipboard adapter; `Clip::File` copy → `"Copying a file is not available on Linux yet"`; `read_clipboard` → `"Reading the clipboard is not available on Linux yet"`; `xdg-open` / `gio launch` (`.desktop`), reveal via `dbus-send … org.freedesktop.FileManager1.ShowItems` with folder fallback, `gio trash`. X11 has no "do not record" marker, so a concealed copy is a plain one.
- **`system/programs.rs`** (`#[cfg(unix)]`): `run(program, args)` / `start(program, args)` helpers for the system's own programs (`open`, `xdg-open`, `gio`, `dbus-send`), never through a shell; `start` waits a `SETTLE` of 3 s for failure.
- `MAX_CLIPBOARD_TEXT: usize = 4 * 1024 * 1024` (system.rs) bounds `read-clipboard`/`selected-text`.

Guest-side dispatch: `crates/pane-core/src/runtime/system_functions.rs` implements `system_host::Host for GuestState` (see §9), running each adapter call off the runtime thread (`off_thread`, thread name `"pane-system"`).

## 2. `commands.wit`, `extension.wit`, `feedback.wit`, `programs.wit`, arguments

**`wit/extension.wit`** — the `command` interface the guest exports and the minimal world:

- `render: async func(launch: launch-record) -> result<string, string>` — the JSON tree (ADR 0036 envelope, `docs/list-tree.md`).
- `run: async func(command: string, launch: launch-record) -> result<string, string>` — the **no‑view** entry point: "one whose `pane.json` entry says `"mode": "no-view"`… Pane calls it, never `render`… opens no screen. The answer is a JSON object… Pane shows nothing of it."
- `handle-event`, `submit-form`, `open-view`, the `custom-view` resource, forms and custom-view records.

**`wit/commands.wit`** — `launch-record` (`launch-type` user‑initiated/background; `source` root‑search/alias/fallback/hotkey/quick‑slot/command/schedule; `arguments: list<argument-value>`; `fallback-text`; `context`; `command`), `command-ref`, and host imports `launch` and `set-subtitle`. World `commands-user { import commands; }` for guest bindings.

**`wit/feedback.wit`** — two interfaces every command imports:

- `window`: `close(clear-root-search: bool, pop: pop-to-root-type) -> bool`, `pop-to-root(clear-search: bool) -> bool`, `clear-search() -> bool`.
- `feedback`: `toast-style` (animated/success/failure), `toast-action`, `toast`, `show-toast/update-toast/hide-toast`, **`show-hud: func(title: string, style: toast-style)`** ("Closes the launcher, then shows `title` in a small window of its own… which never takes the focus: for 1.2 seconds, or 3 for the failure style"), and **`confirm: async func(confirmation: confirmation) -> result<bool, string>`** with `record confirmation { title, message, primary, destructive, dismiss, remember }` — `remember` offers "Don't ask again"; Pane remembers under that key for the package, `confirm` answers `true` at once thereafter; a dismissal is never remembered.

**`wit/programs.wit`** — `interface programs` (ADR 0033): `run` (waits, answers `output { exit-code, stdout, stderr }`), `spawn` + `write-input`/`close-input`/`read-output`/`read-error`/`wait`/`kill`; `record options { folder, environment, timeout-ms, show-window, elevated }` — "elevated … on Windows through the system's elevation prompt, which the user may decline (`declined`)… an elevated program's streams cannot be reached"; `program-error-kind` = not‑found, unavailable, declined, timed‑out, too‑much‑output (16 MiB), refused, failed. **No terminal run exists** — the only "terminal" mention in the module is a doc comment; ADR 0033 says "Script commands, which the user wants later, are not decided here." World `programs-user { import programs; }`.

**Declaring no‑view in `pane.json`** — `crates/pane-core/src/packages.rs` (~line 484–517, ~905–918): `"mode"` on a command entry: `"view"` (default), `"no-view"` (`CommandMode::NoView`), `"provider"` (`CommandMode::Provider`, a root provider, #164). An unknown mode is refused: `command `{}` has the mode "{}"; a command's `mode` is "view" …, "no-view" … or "provider"`. A no‑view schedule names no `item`; its hotkey runs it without showing the window (`crates/pane-core/src/launcher/launching.rs`, `hotkey_shows_window`).

**Typed arguments** — `crates/pane-core/src/arguments.rs` (no `wit/arguments` file; arguments are strings in the launch record). `MAX_ARGUMENTS = 3`; `ManifestArgument { name, kind, placeholder, required }` with `ArgumentKind::Text | Password | Dropdown(Vec<DropdownOption>)`. Install refuses a fourth argument, a repeated name, an unknown type, a dropdown without options. **There is no numeric/range type**: a "Set Volume 0–100" command can only use a `text` argument (validated by the command itself) or a `dropdown` of fixed values; every value reaches the guest as `argument-value { name: string, value: string }`. Fallback text fills the first text/password argument (`fill`); a required empty one triggers Pane's argument form (`launcher/argument_form.rs`); values another command passes are checked by `check_given` (dropdown must be among its options). Samples: `guests/packages/sample-arguments/pane.json` (Greet: text + password + dropdown; Stamp; Relay).

## 3. How a NEW host capability/import is added end‑to‑end (the ADR 0015 `applications` pattern)

`pane:extension/applications` is the reference example. Every file that must change to add a new interface (say `pane:extension/windows` or `pane:extension/run`):

1. **ADR** — `docs/adr/00NN-<decision>.md` (0015 for applications, 0033 for programs, 0020/0042 for clipboard history…). Also follow `docs/agents/domain.md` (CONTEXT.md glossary + ADRs).
2. **New WIT file** — `wit/applications.wit`: the interface itself, a guest‑bindings world `world applications-user { import applications; }`, and a composed hosted world chaining the ladder: `extension` (extension.wit) → `extension-with-data` (data.wit) → `extension-with-applications` (applications.wit) → `extension-with-helpers` (helpers.wit) → `extension-with-files` (files.wit) → `extension-with-clipboard` (clipboard.wit) → **`extension-with-file-index`** (file‑index.wit, the world Pane hosts today).
3. **Host implementation crate** — `crates/pane-core/src/applications.rs` + `applications/` (`app_bundles`, `cached`, `desktop_entries`, `icons/`, `identity`, `names`, `plist`, `start_menu`, `watching`), behind a trait (`Applications`) with `native()` per‑system adapters. For a simple import, `programs.rs`/`system.rs` are the closer templates.
4. **Runtime registration** — `crates/pane-core/src/runtime.rs`:
   - `pub(crate) mod bindings` (~line 99): `wasmtime::component::bindgen!({ path: "../../wit", world: "extension-with-file-index", imports: { … } })` — add the interface (choose `store` vs `async` handling; programs is `store`, system is `async`);
   - `Code::new` (~lines 2721–2792): one more `<interface>::add_to_linker::<_, …>(&mut linker, |state| state)` (e.g. `bindings::pane::extension::programs::add_to_linker::<_, crate::programs::Calls>`);
   - an `impl … Host for GuestState` (like `runtime/system_functions.rs`) or a `Calls` struct with `impl HasData` (like `programs::Calls`);
   - if use should be visible (as ADR 0033 decided for programs), a constant like `pub(crate) const PROGRAMS_INTERFACE: &str = "pane:extension/programs@";` recorded at install/update/reload by `packages.rs` (it already records `wasi:http` imports for "Network use").
5. **Opt‑in in package config** — there is **no** `pane.json` "permissions"/"capabilities" field. The opt‑in is the component's *import set*:
   - Rust: what the component imports (built against a world that imports it — pane‑extension's per‑module worlds);
   - JS/TS: `package.json` `"pane": { … }` options, mapped in `tools/componentize-js/pane_js.py` `command_world()` — `EXPORT_OPTIONS` (rootResults, indexedResults, operations, search, service) and `IMPORT_OPTIONS` (files → `pane:extension/files@0.1.0`, fileIndex, clipboardHistory); plus automatic bundle scanning for `wasi:http` (`uses_http`) and `pane:extension/programs@0.1.0` (`uses_programs`). The `pane.json` entry flags exports (`"indexedResults": true`, `"mode": "no-view"`, `"arguments"`, `"schedule"`, `"service"`, `"search"`, `"takesQuery"`, `"folderAccess"`, `"fileIndex"`).
   - `pane.json` flags *checked* by the host: e.g. a provider must declare `rootResults`/`indexedResults` (packages.rs ~1072).
6. **Rust SDK** — `guests/pane-extension/wit/` is a checked copy of `wit/` (`cargo xtask sdks` → `same_files(&root/wit, &guests/pane-extension/wit)`). `guests/pane-extension/src/lib.rs` adds a module per interface: `pub mod applications { wit_bindgen::generate!({ path: "wit", world: "applications-user", default_bindings_module: "pane_extension::applications", }); pub use pane::extension::applications::{…}; }` (same pattern for clipboard_history, helpers, files, file_index, programs, publish, root, indexed, search, service).
7. **JS/TS SDK** — `guests/js/*.d.ts` + `*.js` per module (`applications.d.ts`/`applications.js`, `system.d.ts`/`system.js`, `programs-host.d.ts`, …), npm package `@pane-app/extension` (`guests/js/package.json`, packed by `cargo xtask sdks` into `target/sdks/`). The guest world `guests/js/wit/world.wit` (`js-extension`) imports `pane:extension/extension-with-data@0.1.0`, `pane:extension/applications@0.1.0`, `pane:extension/helpers@0.1.0`, `wasi:clocks/monotonic-clock@0.3.0`; the build writes a per‑command world `js-command` from it (`command_world()`).
8. **Guest WIT copies** — `wit/` must be mirrored to `guests/pane-extension/wit/` and `guests/js/wit/deps/pane-extension/` (the latter copies `PANE_WIT` — `tools/componentize-js/pane_js.py` ~line 111: the list of wit/*.wit filenames — a new file must be added there), plus WASI deps from `wit/deps`.
9. **Prebuilt rebuild flow** — `cargo xtask js-guests` → `python tools/componentize-js/pane_js.py samples` rebuilds `guests/prebuilt/*.wasm` + `guests/prebuilt/manifest.json` (digests); `cargo xtask guests` builds Rust guests (`--target wasm32-wasip2`) into `target/guests`, copies `PREBUILT` there, assembles `SAMPLE_PACKAGES` into `target/guests/packages/`; `cargo xtask sdks` checks the WIT copy, `cargo publish --dry-run -p pane-extension`, `npm pack`; `cargo xtask ci-lints` runs fmt, `pane_js.py check`, `sdks`, clippy. New samples must be added to the samples list in `pane_js.py` (~line 100, e.g. `("sample_programs_ts.wasm", "guests/sample-programs-ts")`), to `PREBUILT` and `SAMPLE_PACKAGES` in `xtask/src/main.rs`.
10. **Docs/samples** — `guests/README.md` (author guide section), `docs/` spec pages, `guests/packages/<sample>/pane.json`.

## 4. ADR 0037's paste host function

- **Declared** in `wit/system.wit`: `paste: func(content: clip) -> result<_, system-error>` — "Closes the window (as `window.close` does…), brings the application that was in front before Pane back to the front and pastes `content` into it, then puts back what the clipboard held before, unless something else was copied meanwhile… Both copies are concealed… `not-available` leaves the window and the clipboard as they were; the SDKs' Paste then copies instead and says so." Part of the `system` interface every command imports.
- **Implemented (host orchestration)**: `crate::system::paste(system: &dyn System, clip: &Clip)` in `crates/pane-core/src/system.rs` — saves `read_clipboard`, `copy(clip, concealed=true)`, calls `paste_clipboard()`, puts the old content back concealed only if the clipboard still holds ours; failures of the restore never fail the paste. Guest side: `runtime/system_functions.rs::paste` — asks `can_paste()` **first** (so a system that cannot paste changes nothing and leaves the window open), then `close_for_paste()` (closes the window), then runs `host_system::paste` off‑thread. `launcher/own_actions.rs::paste_or_copy` and `launcher/clipboard_view.rs::paste_clipboard_record` are Pane's own paste call sites (Clipboard History's Enter, Paste answer for computed answers), both falling back to copy + `PASTE_FALLBACK` HUD.
- **What the Windows implementation replaces**: the default trait bodies of `System::can_paste` and `System::paste_clipboard` (system.rs), by overriding them in `crates/pane-core/src/system/windows.rs`. The copying fallback already exists on every system (it is what runs today on Windows, macOS, Linux — see `guests/clipboard-history/src/lib.rs::paste`, `guests/pane-extension/src/actions.rs::Does::Paste`, `guests/js/system.js::pasteAction`, and the smokes' "Paste: not available yet, so it copies it again" steps); implementing paste makes those take the paste path with no code change.
- **Concealed copy marker — already implemented**:
  - `crates/pane-core/src/system.rs` `CONCEALED_MARKERS: [(&str, u32); 3] = [("ExcludeClipboardContentFromMonitorProcessing", 0), ("CanIncludeInClipboardHistory", 0), ("CanUploadToCloudClipboard", 0)]`.
  - Applied on write by `put_clip(clip, concealed)` in `crates/pane-core/src/clipboard/windows.rs` (writes `CF_UNICODETEXT` or `CF_HDROP` + `Preferred DropEffect` for files, plus each marker as a registered format DWORD, owned by the writer thread's `WRITER_CLASS` message window).
  - Read back by the clipboard listener on each `WM_CLIPBOARDUPDATE`: `Formats::register()` registers `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore` (older), `CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard`, `PNG`; `read()` builds `Markers { exclude_from_monitoring: available(exclude) || available(viewer_ignore), include_in_history: flag(history), upload_to_cloud: flag(cloud) }` and, if `!markers.allow()`, answers `Content::Withheld` without reading the content.
  - Pane's history ignores tagged writes through `clipboard::accept`/`accept_any` (crates/pane-core/src/clipboard.rs): `if !observation.markers.allow() { return Err(Skip::Marked) }` — the capture sink never stores it. Test `a_concealed_copy_is_one_pane_s_history_skips` (system.rs) pins the marker list to the adapter's `Markers`.
  - macOS concealed type: `org.nspasteboard.ConcealedType` (`clipboard/macos.rs`); Linux: plain copy, no marker.

## 5. ADR 0033 programs (`pane:extension/programs`)

- `crates/pane-core/src/programs.rs`: host struct `Calls` (`impl HasData { type Data<'a> = &'a mut GuestState; }`, `impl wit::HostWithStore<T> for Calls` for run/spawn/write‑input/close‑input/read‑output/read‑error/wait/kill); `pub(crate) const PROGRAMS_INTERFACE: &str = "pane:extension/programs@";`; `Processes` map of spawned ids in `GuestState` (ids die with the call; "a spawned program ends with the call that started it"). Waiting runs through `crate::runtime::deadlines::hosted` — not the guest's computing.
- `programs/runner.rs`: `pub(crate) fn run(request: Request, input: Vec<u8>, owner: Owner) -> impl Future<Output = Result<Output, ProgramError>> + Send + 'static` and `spawn`, on a `"pane-program"` supervisor thread; process‑tree containment (Job Object on Windows, process group on Unix) from `crate::process_tree` (re‑exported from pane‑build, `lib.rs:94`); ownership via `helpers::runner::Helpers::register(Kind::Program, …)` and the generation's undo list; limits `MAX_PROGRAM_OUTPUT = 16 << 20`, `MAX_PROGRAM_INPUT`, `MAX_ARGS = 4096`, `MAX_ENVIRONMENT = 1024`, `DRAIN = 2 s` after exit.
- `programs/elevated.rs`: Windows `run(request, owner, watching)` — `ShellExecuteExW` with `lpVerb = "runas"`, `SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI`, `WaitForSingleObject` in 10 ms ticks, `Job::contain(process.0)`; declined (ERROR_CANCELLED) → `"the user declined to run {} as an administrator"`; non‑Windows → `"running a program elevated is not available on {here} yet"`; elevated **spawn** refused up front ("…cannot be spawned elevated: an elevated program's streams cannot be reached; run it instead"). `command_line(args)` quotes for `CommandLineToArgvW` semantics.
- `programs/search.rs`: `system_search_path()` reads the **registry on every call** — machine `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` `Path` then user `HKCU\Environment` `Path` via `RegGetValueW` (RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ, expanded), falling back to Pane's own PATH; macOS adds Homebrew folders. `resolve(program, search_path)`: absolute path must be a file; bare name resolved with PATHEXT (default `.COM;.EXE;.BAT;.CMD`); relative path refused ("`{}` is a relative path: name a program by an absolute path, or by a bare name Pane finds on the search path"); not found ("there is no program `{}` on the search path" / "there is no program at {}"). **App Paths is not consulted** — resolution is PATH‑only; App‑Paths/shortcut resolution lives only in the applications discovery.
- No terminal run; ADR 0033 explicitly defers script commands.

## 6. ADR 0038 / ADR 0015 applications

`crates/pane-core/src/applications.rs` (+ `applications/`):

- `trait Applications { installed() -> Result<Vec<Application>, String>; open(id); source(id); current_id(id); on_change(...) }`, `native()` picks the adapter; discovery is plain filesystem work compiled everywhere, opening is per‑system.
- **Windows** `start_menu.rs`: Start‑menu Programs folders, user/all‑users Desktops (`SHGetKnownFolderPath(FOLDERID_Desktop/FOLDERID_PublicDesktop)`), taskbar pins, packaged (AppX/MSIX) apps of `shell:AppsFolder` (`const APPS_FOLDER: &str = r"shell:AppsFolder\"`); shortcuts `.lnk`/`.url`/`.appref-ms` (ClickOnce); shell links read via the shortcut interface on single‑threaded COM apartments in parallel, including each shortcut's **AppUserModelID** property; MSI‑advertised shortcuts resolved through the Windows Installer; `is_app_user_model_id(parsing)` (contains `!`, no `\ / : {`).
- **Identity** `identity.rs` (ADR 0038): `enum Key { Program { target, arguments }, Link { .. }, Package { family, app: Option<String> } }` — a packaged app's key is its **package family** (lowercase) plus **AppUserModelID** when not the package's first app (`packaged_keys(aumids)`); the application's id is a digest of the key, stable across updates; `Catalog::find(id)` also accepts pre‑identity path ids and `shell:AppsFolder\<AppUserModelID>`.
- **Live list & icon cache** `cached.rs`: `DEBOUNCE`/`GRACE` (watchers debounce ~0.5 s, 5 s grace before removal), rescan on watcher overflow/sleep/period; built lazily while an enabled package asks for it, dropped when none can run (ADR 0005). `icon_reference`: a packaged app also found by a shortcut uses `shell:AppsFolder\<AppUserModelID>`.
- **Icons** `icons.rs`/`icons/` + `crate::system_icons` (`windows_shell::shell_image` → `IShellItemImageFactory`): extracted at 256 px in the background, cache in Pane's cache folder keyed by id + fingerprint, low‑priority refresh worker, light/dark packaged variants. `icons.rs::system_path` keeps `shell:AppsFolder\…` names as‑is for the system to find.
- **Window enumeration today: none.** Grep for `EnumWindows|GetForegroundWindow|SetForegroundWindow|AttachThreadInput|GetGUIThreadInfo|SendInput|keybd_event|SetWindowsHookEx|IUIAutomation|UiaGetRoot` over `crates/**/*.rs`: **zero matches**. The only window/thread machinery in pane‑core is: the clipboard listener and hotkey threads' message‑only windows (`crates/pane-core/src/threads/windows.rs` — `MessageThread`, `WindowClass::message_window`/`hidden_window`, `WM_WAKE`/`WM_STOP`), the tray's hidden top‑level window (`tray/windows.rs`, also the only user of `Win32_UI_Accessibility` — `HIGHCONTRASTW`), the shell calls of system/windows.rs, and `GetWindowThreadProcessId`+`QueryFullProcessImageNameW` to name the clipboard's owner program (`clipboard/windows.rs::owner_program`). Window‑to‑application identity is entirely shortcut/AUMID based, never live windows. (Scripts use PowerShell's `GetForegroundWindow` and `System.Windows.Automation` to *drive* Pane in smokes; `crates/pane/src/features/announcer.rs` mentions Narrator/NVDA via UIA for the announcer — GPUI‑level, not pane‑core.)

## 7. Samples

- **Rust samples** live in `guests/`: `sample-actions` (the host‑functions/paste sample), `sample-programs`, `sample-arguments`, `sample-no-view`, `sample-settings`, `sample-clipboard-js/-ts`, `sample-applications-js/-ts`, `sample-query`, `sample-search`, `sample-helper`, `sample-files`, `sample-icons`, `sample-preferences`, `sample-schedule`, `sample-service`, `sample-operations`, `sample-dependencies`, plus default extensions `calculator`, `applications`, `quicklinks`, `files`, `clipboard-history` and development samples `hello-rust/js/ts`. **JS/TS twins** `-js`/`-ts` of each. Package manifests under `guests/packages/<name>/pane.json`; components named per package.
- **Rust SDK**: `guests/pane-extension` (crate `pane-extension`, `wit-bindgen =0.62.0`, `#![no_std]`, per‑module `wit_bindgen::generate!` + re‑exports; `actions.rs` holds the standard actions incl. `paste` with the copy fallback; `system.rs` re‑exports the WIT functions and `FrontApp::icon()`).
- **JS/TS SDK**: `guests/js` (`@pane-app/extension`): `pane.d.ts`, `feedback.d.ts/.js`, `system.d.ts/.js` (incl. `NotAvailableError`, `PASTE_FALLBACK`, `pasteAction`, `frontApplication`, `selectedText`), `programs.d.ts/.js`, `applications.d.ts`, `clipboard.d.ts`, `helpers.d.ts`, `file-index.d.ts`, `files.d.ts`, `operations.d.ts`, `*-host.d.ts` (raw host shapes), `adapt.js`, `wit/world.wit`.
- **Prebuilt**: `guests/prebuilt/*.wasm` + `manifest.json` (committed digests; `NOTICE.md`), rebuilt only by `cargo xtask js-guests` (`pane_js.py samples`) with the pinned componentize‑qjs toolchain (`tools/componentize-js/`, patches + `pins.json`); `pane_js.py check` verifies without a toolchain. `cargo xtask guests` copies `PREBUILT` (xtask list) into `target/guests/` and assembles `SAMPLE_PACKAGES` (59 entries) into `target/guests/packages/`.
- **How tests use samples**: `crates/pane-core/tests/support/guests.rs` (`guest("sample_rust")` → `target/guests/<name>.wasm`, panics with "run `cargo xtask guests`"); packages installed from `target/guests/packages/<pkg>`. Notable files: `tests/paste.rs` (the **actions sample** in Rust/JS/TS + `support/system.rs` `RecordingSystem` + `support/feedback.rs` `RecordingWindow`; item actions PASTE/PASTE_TO/PASTE_DIRECTLY/FRONT_APPLICATION/SEARCH_SELECTION), `tests/samples.rs` (language‑parity contract checks over `sample_rust/js/ts`), `tests/no_view.rs`, `tests/programs.rs`, `tests/arguments.rs`, `tests/system.rs`, `tests/feedback.rs`, `tests/confirmations.rs`; "the settings sample" (`sample-settings`) drives `develop.rs`, `runtime_crash.rs`, `unresponsive.rs`, `pausing.rs`, `retained`/`uninstall` phases (and the smokes' Settings/hotkey/alias phases).
- **Adding a sample for new imports**: create `guests/sample-<name>{,-js,-ts}` (source + `guests/packages/sample-<name>/pane.json`); for Rust add the component name to the `workspaces` list in `xtask/src/main.rs::guests()`; add the package+component to `SAMPLE_PACKAGES`; for JS/TS add `(“sample_<name>_js.wasm”, "guests/sample-<name>-js")` entries to the samples list in `tools/componentize-js/pane_js.py` (~line 100–109) **and** to `PREBUILT` in `xtask/src/main.rs`; run `cargo xtask js-guests` (rebuilds `guests/prebuilt/` + manifest), then `cargo xtask guests`; add a test module under `crates/pane-core/tests/` and declare it in `tests/main.rs`.

## 8. Windows‑specific crates

- Workspace `Cargo.toml`: `windows = "0.62"` (in `[workspace.dependencies]`), `x11rb = "0.13"` (Linux), `objc2 = "0.6"` (macOS).
- `crates/pane-core/Cargo.toml` `[target.'cfg(windows)'.dependencies] windows = { workspace = true, features = ["Win32_Foundation", "Win32_Graphics_Gdi", "Win32_Security", "Win32_Security_Authorization", "Win32_Security_Cryptography", "Win32_Storage_FileSystem", "Win32_Storage_Packaging_Appx", "Win32_System_ApplicationInstallationAndServicing", "Win32_System_Com", "Win32_System_Com_StructuredStorage", "Win32_System_DataExchange", "Win32_System_IO", "Win32_System_Ioctl", "Win32_System_LibraryLoader", "Win32_System_Memory", "Win32_System_ProcessStatus", "Win32_System_Registry", "Win32_System_Threading", "Win32_System_Variant", "Win32_System_WindowsProgramming", "Win32_UI_Accessibility", "Win32_UI_HiDpi", "Win32_UI_Input_KeyboardAndMouse", "Win32_UI_Shell", "Win32_UI_Shell_Common", "Win32_UI_Shell_PropertiesSystem", "Win32_UI_WindowsAndMessaging"] }` (the doc comment above it lists what each group is for).
- **Where UI Automation / raw input / `SetWindowsHookEx` would be added**:
  - UI Automation COM (`IUIAutomation`, `UiaGetRootNode`…): feature **`Win32_UI_Accessibility` is already enabled** (today used only for `HIGHCONTRASTW` in `tray/windows.rs`); also needs `Win32_System_Com` (already) and COM on a thread with a message pump.
  - Raw input (`RegisterRawInputDevices`, `GetRawInputData` — ADR 0039's hook watchdog): `Win32_UI_Input_KeyboardAndMouse` (already enabled; hotkeys' `RegisterHotKey` is there).
  - `SetWindowsHookEx` (WH_KEYBOARD_LL), `EnumWindows`, `GetForegroundWindow`, `SetForegroundWindow`, `AttachThreadInput`, `GetGUIThreadInfo`, `SendInput`: all `Win32_UI_WindowsAndMessaging` (already enabled; the clipboard listener/hotkey/tray message windows already live there).
  - So no Cargo.toml change is strictly required for the Windows power features; only new code and, at most, a message‑pumping thread.
- Other Windows deps in‑tree via GPUI CE and pane‑build (Job Objects, EcoQoS, DPAPI via `Win32_Security_Cryptography`).

## 9. How host functions are surfaced to guests (runtime dispatch)

`crates/pane-core/src/runtime.rs` (one Wasmtime engine on a dedicated thread serving many calls, #136):

- **Bindings**: `pub(crate) mod bindings` (~line 99) — `wasmtime::component::bindgen!({ path: "../../wit", world: "extension-with-file-index", imports: { "pane:extension/operations": store, "pane:extension/helpers": store, "pane:extension/programs": store, "pane:extension/settings.set": async, "pane:extension/content.set": async, "pane:extension/cache.set": async, "pane:extension/credentials.set": async, "pane:extension/system": async }, exports: { default: async | store } })`. Separate bindgen modules for export worlds: `root_bindings` (root‑results‑provider), `indexed_bindings`, `search_bindings`, `service_bindings`, `operations_bindings`.
- **Linker**: `Code::new` (~2721–2792) registers, in order: `wasmtime_wasi::p3::add_to_linker` (only WASI 0.3 — no P2 linker, no unknown‑import stubs, so mixed components cannot instantiate), `wasmtime_wasi_http::p3`, `settings`/`content`/`cache`/`credentials` (`HasSelf`), `operations::Calls`, `applications`, `clipboard_history`, `helpers::Runs`, `programs::Calls`, `files`, `file_index`, `launching` (commands), `window_host`, `feedback_host` with `host_functions::Confirms` (the async `confirm`), `system_host`, `preference_values`.
- **Host impls by interface**: `runtime/system_functions.rs` (`system_host::Host for GuestState`, everything off‑thread via `off_thread`/`"pane-system"` threads), `runtime/host_functions.rs` (window/toasts/HUD/`Confirms::confirm`, `set-subtitle`), `runtime/application_list.rs` (applications + indexed results), `crates/pane-core/src/programs.rs` (`Calls`), plus inline impls in runtime.rs for data/preferences/launching.
- **Errors** flow as the WIT `result<_, E>` payload: e.g. `ProgramError` → `wit::ProgramError { kind, message }` (`From` impl in programs.rs), `SystemError` → `system_host::SystemError::NotAvailable/Failed` (`wire_error` in system_functions.rs); stopped code answers `stopped_code(end)` ("this code was stopped…"); a runtime with no launcher answers as `system::none()` does. Host time is never charged to the guest: waits go through `crate::runtime::deadlines::hosted` (`GuestState::hosted`) and `runtime/deadlines.rs` metering; "an error is an answer, never a reason to pause".
- Interface constants in runtime.rs: `COMMAND_INTERFACE = "pane:extension/command@0.1.0"`, `ROOT_RESULTS_INTERFACE`, `INDEXED_RESULTS_INTERFACE`, `WASI_VERSION = "@0.3."`; `compile()` rejects components importing non‑0.3 WASI.

## 10. CONTEXT.md glossary — terms about host functions / modes / HUD / confirm

- **Host function** — "A function Pane gives every command, whatever its mode, through which the command decides what happens after it runs: close the window, pop to root, clear the search field, show a HUD or a toast, ask for confirmation, use the clipboard, paste into the previous application, open or reveal a path or URL, move to the Recycle Bin, launch another command or run a system program (ADR 0037, ADR 0033). Pane does nothing after a run that the command did not ask for." (_Avoid_: built‑in outcome, convenience action.)
- **Command mode** — view / no‑view declared in the manifest (ADR 0037). **No‑view command** — runs its run entry point, opens no screen; its hotkey runs without the window.
- **Toast** — footer message, animated/success/failure, updatable, up to two actions; shown as a HUD while the window is hidden. **HUD** — small unfocused window over other applications, 1.2 s (3 s failure), closes the launcher first.
- **Action** (an item's, incl. toast actions) — callback or submenu, primary/secondary. **Confirmation** is described under Host function and the extension page's "Reset Confirmations" (remembered answers kept per package).
- Related: **Launch record**, **Argument** ("up to three typed fields (text, password or dropdown)… a password's is never recorded"), **Root provider** (`"mode": "provider"`), **Clipboard history** ("A copy its application marks as concealed (as password managers do)… is not kept"), **Disabled application**, **Retention**, **Installed application** (ADR 0038 identity), **Native helper**, **Helper target**, **Generation**, **Extension runtime**, **Unresponsive call** ("time inside Pane's host calls … not counted"), **Waiting command**, **Capability** (ADR 0041 operations brokering — *not* an import permission), **Setup**, **Preference** (typed: text, password, checkbox, dropdown, file, folder, application — note preferences have more kinds than arguments).

---
