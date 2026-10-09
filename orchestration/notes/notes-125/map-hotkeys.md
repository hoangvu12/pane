# GLOBAL HOTKEYS — codebase map (branch `spec/125-windows-power`)

Repo root: `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\spec-125`. Line numbers marked `~` are counted from reads; unmarked numbers were confirmed by search. Files are under `crates/…` relative to the root.

Governing docs: `docs/hotkeys.md` (feature spec), `docs/adr/0016-host-registers-global-hotkeys-for-commands.md` (original architecture, amended), `docs/adr/0037-…-a-command-declares-its-mode…` (no‑view hotkey runs hidden), **`docs/adr/0039-hotkeys-fall-back-to-a-keyboard-hook-and-the-windows-key-opens-pane.md`** (this branch's ADR: `WH_KEYBOARD_LL` hook fallback, refusal-is-not-an-error, Windows-key Open Pane default on fresh install, game mode). `docs/agents/domain.md` requires reading `CONTEXT.md` + ADRs before design work — done.

## 1. `crates/pane-core/src/hotkeys.rs` — core model (428 lines)

Module docs (lines 1–20): launcher decides bindings, system reached through one trait `Hotkeys`, one adapter per system chosen by `native`.

- Re-exports: `pub use x11::X11Hotkeys` (30), `pub use windows::WindowsHotkeys` (35), `pub use macos::MacHotkeys` (40).
- `const FUNCTION_KEYS: u8 = 12;` (43)
- `pub struct Shortcut { control: bool, alt: bool, shift: bool, super_key: bool, key: String }` (48), `#[derive(Clone, Debug, PartialEq, Eq, Hash)]` — "A key combination: modifier keys held while one other key is pressed."
- `pub fn new(control: bool, alt: bool, shift: bool, super_key: bool, key: &str) -> Result<Shortcut, String>` (61) — lowercases the key; accepts `a`–`z`, `0`–`9`, `f1`–`f12`, `space`; otherwise `"Pane cannot use {key} in a hotkey: end it with a letter, a digit, F1 to F12 or Space"` (74). **The key set an implementer extends for an "extended key set."**
- `pub fn open_pane_default() -> Shortcut` (94) — `alt+space` on macOS, `ctrl+alt+space` otherwise (provisional synthesis default; ADR 0039 changes the Windows fresh‑install default once the hook exists).
- `pub fn parse(text: &str) -> Result<Shortcut, String>` (104) — reads `id()` text; modifier synonyms `ctrl|control`, `alt|option`, `shift`, `super|win|cmd|command`.
- `pub fn id(&self) -> String` (125) — fixed modifier order `ctrl+alt+shift+super` + key, "The same text on every system, for records".
- Accessors `control()` (136), `alt()` (140), `shift()` (144), `super_key()` (150), `key()` (154); `fn modifiers() -> [(bool, &'static str); 4]` (159).
- `pub fn refusal(&self) -> Option<String>` (171) — refuses: no Ctrl/Alt/Super ("… needs Ctrl, Alt or the Windows key, so that it does not take over typing"), reserved combos, then `editing_refusal()`.
- `fn editing_refusal(&self)` (192) — primary modifier + single character ("Ctrl+C is used by applications for their own commands…; add Alt or Shift").
- `fn reserved(platform: Option<Platform>) -> &'static [(&'static str, &'static str)]` (210) — Windows: `alt+f4`, `super+l`, `super+d`, `super+e`, `super+r`, `alt+space`; macOS: `super+space`, `ctrl+space`, `super+alt+space`, `shift+super+3/4/5`, `ctrl+super+q`; Linux: `alt+f4`, `super+l`, `ctrl+alt+f1`…`f12`. Reserved lists are Pane's, not read from the system.
- `fn key_name(key: &str) -> String` (251); `impl fmt::Display for Shortcut` (268) — "Ctrl+Alt+P" / Win / "Control+Option+Command+P".
- `pub enum HotkeyError { Taken, Refused(String) }` (282) with `Display` (289–295): "another application or the system already uses it" / "the system refused it: …". Under ADR 0039 a `Taken` on Windows stops being an error (hook takes it) — the refusal-handling seam is `HotkeyError` + `Launcher::set_hotkey_of`'s `Err` branch (§5).
- `pub trait Hotkeys: Send + Sync + 'static` (307):
  - `fn unavailable(&self) -> Option<String>` — why hotkeys cannot be used here at all (Wayland).
  - `fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError>` (313)
  - `fn unregister(&self, shortcut: &Shortcut)`
  - Trait contract: called on the window's thread (macOS run loop); one adapter per process; dropping it releases every shortcut and ends its thread.
- `pub struct PressSender` (~327) with `pub fn send(&self, shortcut: Shortcut)` (~330); `pub struct Presses` (~336) with `pub async fn next(&mut self) -> Option<Shortcut>` and `pub fn try_next(&mut self) -> Option<Shortcut>`; `pub fn channel() -> (PressSender, Presses)` (~349). **A dispatch‑route/press‑metadata change (registered vs hook, lone tap, double tap) would flow through `PressSender::send`'s payload.**
- `pub fn native(presses: PressSender) -> Arc<dyn Hotkeys>` (355) — Windows: `WindowsHotkeys::start(presses)` (382) → `Unavailable("Not available: …")` on error; Linux: Wayland check, `X11Hotkeys::connect(&display, presses)` (369); macOS: `MacHotkeys::start`.
- `pub struct Unavailable(pub String)` (410) + `impl Hotkeys` (413); `pub(crate) fn none() -> Arc<dyn Hotkeys>` (426) — launcher with no adapter.

Sibling adapters: `hotkeys/macos.rs` (Carbon via `global-hotkey` crate), `hotkeys/x11.rs` (`XGrabKey` on root, `X11Hotkeys::connect` at 278, per‑layout keys, `MappingNotify` re‑grab; `presses` are matched in the X event loop).

## 2. `crates/pane-core/src/hotkeys/windows.rs` — `RegisterHotKey` adapter (208 lines)

- Doc (1–12): `RegisterHotKey` on a thread of Pane's own whose message loop receives `WM_HOTKEY`; conflict = `ERROR_HOTKEY_ALREADY_REGISTERED`; no permission; hotkeys belong to the registering thread; thread is a `threads::windows::MessageThread` "like the clipboard listener's".
- `enum Request { Register(Shortcut, mpsc::Sender<Result<(), HotkeyError>>), Unregister(Shortcut, mpsc::Sender<()>) }` (29); `type Requests = Arc<Mutex<VecDeque<Request>>>` (34).
- `pub struct WindowsHotkeys { thread: MessageThread, requests: Requests }` (38).
- `fn virtual_key(key: &str) -> Option<u32>` (44) — `a`–`z` → `VK_A..`, `0`–`9`, `space` → `0x20`, `f1..f12` → `0x70+`. **Extend for an extended key set.**
- `fn modifiers(shortcut: &Shortcut) -> HOT_KEY_MODIFIERS` (57) — always includes **`MOD_NOREPEAT`** ("Holding the keys reports one press, not one per repeat"), `MOD_CONTROL/MOD_ALT/MOD_SHIFT/MOD_WIN`. **Side‑specific modifiers (LWIN/RWIN, L/R Ctrl‑Alt) have no expression here — hook territory.**
- `pub fn start(presses: PressSender) -> Result<WindowsHotkeys, String>` (77) — `MessageThread::spawn("pane-hotkeys", || Ok((Registered::default(), None)), move |registered, message| registered.serve(message, &served, &presses), Registered::release_all)`.
- `fn send(&self, request: Request) -> bool` (90) — queues, wakes with `WM_WAKE`.
- `impl Hotkeys for WindowsHotkeys` (99): `unavailable()` → `None`; `register` (104) blocks on the thread's mpsc answer (thread‑stopped → `Refused`); `unregister` (114).
- `impl Drop` (122) — `self.thread.stop(None)`; thread releases all hotkeys as it ends.
- `struct Registered { shortcuts: HashMap<i32, Shortcut>, last_id: i32 }` (132) — hotkeys by thread‑local id.
- `Registered::release(id)` (138) — `unsafe { UnregisterHotKey(None, id) }`; `release_all(mut self)` (144).
- `fn serve(&mut self, message: &MSG, requests: &Mutex<VecDeque<Request>>, presses: &PressSender)` (152) — `WM_HOTKEY` → `presses.send(shortcut.clone())` looked up by `wParam` id; `WM_WAKE` → drains queued Register/Unregister.
- `fn register(&mut self, shortcut: Shortcut) -> Result<(), HotkeyError>` (~186–208) — de‑dupes; `virtual_key` miss → `Refused("{shortcut} has no Windows key")` (192); `unsafe { RegisterHotKey(None, id, modifiers(&shortcut), key) }` with no window (hotkey belongs to this thread); `ERROR_HOTKEY_ALREADY_REGISTERED` → **`HotkeyError::Taken`** (207 other errors → `Refused(error.message())`).
- **Where a `WH_KEYBOARD_LL` hook fallback slots in**: a new sibling (e.g. `hotkeys/windows.rs` grown, or `hotkeys/windows_hook.rs`) implementing the same `Hotkeys` trait; the register path would try `RegisterHotKey` first and, on `Taken`/inexpressible binding, fall back to the hook (ADR 0039). The `Request`/`Registered` seam, the `MessageThread` infra in `crates/pane-core/src/threads/windows.rs` (`MessageThread`, `WM_WAKE`, plus `WindowClass` for hidden windows as the tray uses), and `PressSender::send` are the integration points. Safeguards named by ADR 0039 (fast callback, high‑priority dedicated thread, locked pages, watchdog reinstall, modifier‑state resync, injected‑key tag, admin‑foreground limitation, game mode) have no code yet.
- Tests: `crates/pane-core/tests/hotkey_adapters.rs` `mod windows` (lines ~157–208): `a_registered_shortcut_is_refused_to_others_and_released` (uses `ctrl+alt+shift+f9`, second `WindowsHotkeys::start` gets `Err(HotkeyError::Taken)`, after `unregister` it registers) and `dropping_the_adapter_releases_its_hotkeys` (`ctrl+alt+shift+f10`, `drop(pane)` frees it). No press test here (GUI smokes do that).

## 3. The fake adapter used by tests

There is no shared fake in the core crate; each test file defines its own:

- `crates/pane-core/tests/hotkeys.rs` — `struct FakeSystem { registered: Mutex<Vec<Shortcut>>, taken: Mutex<Vec<Shortcut>>, unavailable: Option<String> }` (line 43); `impl Hotkeys` (72–99): `take(&str)` pushes into `taken` so `register` answers `Err(HotkeyError::Taken)` — this is how "another application uses it" is faked; `unavailable: Some(reason)` makes `register` answer `Refused(reason)`; asserts no double registration. Helper `press(&self, launcher, shortcut) -> bool` (59) only opens if Pane registered it, then `block_on`s `Launcher::press_hotkey`'s future.
- `crates/pane/tests/hotkeys.rs` — minimal `FakeSystem { registered }` (25–41).
- `crates/pane/tests/open_pane.rs` — `FakeSystem { registered, taken }` (41–65) plus `UnavailableSystem(String)` (67–80) for Wayland‑style refusals.
- `crates/pane/tests/shortcuts.rs` — `FakeHotkeys { registered, taken }` (67–90).
- Injection: `Launcher::with_packages(Runtime::start(), vec![], data/extensions).with_hotkeys(system.clone())` (e.g. pane‑core/tests/hotkeys.rs:139–146, pane/tests/open_pane.rs:186–187, pane/tests/shortcuts.rs:242). At startup the window applies the Open Pane default through the same fake (pane/tests/hotkeys.rs:105–111 asserts `vec![Shortcut::open_pane_default(), command_shortcut]`).

## 4. `crates/pane-core/src/host_settings.rs` — settings.json

- `const FILE: &str = "settings.json"` (33), `const VERSION: u64 = 1` (36). House rules: versioned, validated, atomic (`write_atomically`), missing field → default, unreadable record never repaired (docs 1–20).
- `pub struct HostSettings` (~233) incl. `pub open_pane: Shortcut` (~241) — "the global shortcut that opens Pane itself … a host setting, not a command's hotkey".
- `impl Default` (~299): `open_pane: Shortcut::open_pane_default()`.
- `pub fn open(dir: &Path) -> Result<HostSettings, String>` (~318): no file (NotFound) → defaults (**this is the fresh‑data‑folder decision: `settings.json` absent ⇒ default Open Pane hotkey**); wrong version → "`{file}` has version {v}, which this Pane does not read"; `open_pane` parsed with `Shortcut::parse`, missing → default, invalid fails the whole record.
- `pub fn save(&self, dir: &Path) -> Result<(), String>` (~390) — writes `Recorded` atomically.
- `struct Recorded` (serde, camelCase) with `#[serde(default, rename = "open_pane")] open_pane: Option<String>` (~452) — "The field keeps the name it was first recorded with" (record grammar `"open_pane": "ctrl+alt+space"`). Tests inside the file (lines ~505–700): defaults, round trip, `the_open_pane_field_defaults_and_a_value_that_is_not_a_shortcut_fails_the_record`, failed write leaves the record whole, etc.
- Data folder decision itself: `crates/pane/src/lib.rs` `pub fn data_dir() -> Option<PathBuf>` (~350) — `PANE_DATA_DIR` env, else `%LOCALAPPDATA%\Pane\data` (Windows), `~/Library/Application Support/Pane` (macOS), `$XDG_DATA_HOME/pane`. The binary uses it in `crates/pane/src/main.rs` (~120): `Launcher::with_packages(..., dir.join("extensions"))`; `HostSettings::open` happens in `pane::settings::init` (`configure_visuals`, lib.rs ~142).

## 5. Launcher-level wiring (core owns registration)

`crates/pane-core/src/launcher.rs`:
- `mod hotkeys;` (52); `use crate::hotkeys::{self as system_hotkeys, Hotkeys};` (71); `pub use hotkeys::HotkeyOutcome;` (121); `use hotkeys::{Bindings, OpenPane};` (122).
- `Launcher` field `hotkeys: Arc<dyn Hotkeys>` (609–610); `State` fields `bindings: Bindings` (777) and `open_pane: OpenPane` (779–782).
- `pub fn with_hotkeys(self, hotkeys: Arc<dyn Hotkeys>) -> Self` (1755) — **registers recorded hotkeys at start** (`sync_hotkeys` + `report_failures`). Without it, `system_hotkeys::none()` (1551) explains "this Pane has no global hotkeys".
- Screen/entry plumbing: `Screen::Hotkey { command, details }` (used at 2524, 557); `Entry::AskHotkey(String)` (1242), `Entry::RemoveHotkey(String)` (1244); `Pending::HotkeyChange` handled at 2694; entries dispatched at 2835–2841; `hotkey_mark`/sync on disable (3111–3112), reload (3169), uninstall (3193).

`crates/pane-core/src/launcher/hotkeys.rs` (898 lines) — the registration brain:
- `pub(super) type HotkeyChoices = BTreeMap<String, Shortcut>` (~63) — "kept in `hotkeys.json` beside `installed.json`, by the command's id".
- `impl Choices for HotkeyChoices` (69): `const FILE: &'static str = "hotkeys.json"; const VERSION: u64 = 1; const WHAT: &'static str = "hotkeys";` `read` (~75: `{ "version": 1, "hotkeys": { "<command id>": "ctrl+alt+g" } }`, non‑shortcuts left out), `write` (~86: `shortcut.id()`), `restore`, `retain`, `of(state)`. **Record v2 = new const + `read`/`write` here + the generic record machinery in `launcher/choices.rs`** (`pub(super) trait Choices` at choices.rs:39 — `FILE`/`VERSION`/`WHAT`/`read`/`write`/`restore`/`retain`/`of`; `Record::open` (choices.rs ~105) refuses other versions and never overwrites an unreadable record; `Launcher::save` serializes writes).
- `pub(super) struct Bindings { record: Record<HotkeyChoices>, registered: HashMap<String, Registered>, problems: HashMap<String, String> }` (116) — `problems` is why a chosen hotkey is not active ("Not active: …"). **Dispatch‑route reporting would extend `Registered`/`problems`.**
- `struct Registered { shortcut: Shortcut, generation: EndMark }` (~170) — undo list of the package's generation ("hotkey", see `crate::generation`).
- `pub(super) struct OpenPane { registered: Option<Shortcut>, problem: Option<String> }` (185); `fn taken_by(&self, shortcut)` (~197).
- `fn offered(packages: &[InstalledPackage]) -> Vec<(CommandRegistration, Option<String>)>` (209) — enabled packages' launchable commands with per‑system unavailability; root providers never offered.
- `pub(super) fn sync_hotkeys(&self, state: &mut State)` (221) — registers exactly the chosen hotkeys whose commands are offered and available, releases stale ones, records refusals in `problems`, keeps Open Pane's keys unregistered for commands ("the Open Pane hotkey uses it").
- `fn hotkey_mark(&self, state, command) -> EndMark` (313); `pub(super) fn forget_hotkeys_of(...)` (334) (uninstall); `pub(super) fn hotkey_opening(...)` (349); `pub fn press_hotkey(&self, shortcut: &Shortcut) -> Option<impl Future<Output = ()> + Send + 'static>` (381) — view command: show root + `Status::Running`; no‑view: `Launcher::begin_run` and no window (`hotkey_shows_window` asks first).
- `pub(super) fn hotkey_rows(...)` (405) — extension‑list rows: `"{shortcut} · Opens it from any application"` / `"{shortcut} · Not active: {problem}"` / `"None · Choose keys…"`; unavailable → `Row { unavailable: Some(Unavailable::OnThisSystem(...)), .. }` with `Entry::Unavailable(reason)`.
- `pub(super) fn show_hotkey(...)` (448) — builds `Screen::Hotkey` details ("Press the keys that should open {title} from any application, such as Ctrl+Alt+G." etc.) + "Remove hotkey" row.
- `pub fn record_hotkey(&self, shortcut: Shortcut) -> impl Future<Output = ()> + Send + 'static` (500) — hotkey‑screen entry (`assign_hotkey` ~512, `finish_hotkey_change` ~735: writes via `save::<HotkeyChoices>`; failed write rolls back in‑memory + registration).
- `pub fn set_hotkey(&self, command: &str, shortcut: Option<Shortcut>) -> Result<impl Future<Output = HotkeyOutcome> + Send + 'static, String>` (558) — Settings Shortcuts page entry; `fn set_hotkey_of(...)` (577) is the shared checks (offered+available, root provider refusal #164, `shortcut.refusal()`, collision with another command, `open_pane.taken_by`, adapter `unavailable`, **register new before releasing old**); `fn finish_set_hotkey` (~665) → `HotkeyOutcome`; `pub(super) fn remove_hotkey` (~705); `fn leave_hotkey` (~750).
- `pub fn open_pane_problem(&self) -> Option<String>` (774) — General page reads it; `pub fn opens_pane(&self, shortcut: &Shortcut) -> bool` (781) — window dispatches Open Pane presses; `pub fn set_open_pane(&self, shortcut) -> Result<(), String>` (792); `pub fn sync_open_pane(&self, chosen: Shortcut) -> Result<(), String>` (803) — startup/rollback path, **keeps a refused choice recorded with its problem**; `pub fn release_hotkeys(&self)` (820) — quit path (drains `bindings.registered` + Open Pane, clears problem); `fn assign_open_pane` (~837) — validate → register new → release old.
- `pub(super) struct HotkeyChange { command, done, epoch }` (870); `pub(super) struct HotkeySet { command, done, write }` (879); `pub enum HotkeyOutcome { Saved(String), NotKept(String) }` (892).

`crates/pane-core/src/launcher/launching.rs`: `pub fn hotkey_shows_window(&self, shortcut: &crate::hotkeys::Shortcut) -> bool` (235) — **ADR 0037 answer: a no‑view command's hotkey already runs it without showing the window**, except when it needs setup or asks for its arguments first (argument form). Verified by `crates/pane-core/tests/no_view.rs` ~300–330 (press runs it, screen stays `Screen::Root`, `hotkey_shows_window` false).

`crates/pane-core/src/launcher/shortcuts.rs` — the Shortcuts catalog: `pub struct ShortcutCatalog { groups: Vec<ShortcutGroup>, hotkeys_unavailable: Option<String> }` (36); `pub struct ShortcutGroup` (48); `pub struct ShortcutCommand { id, title, subtitle, takes_query, alias, alias_inactive, editable, hotkey: Option<Shortcut>, hotkey_inactive: Option<String>, hotkey_editable: bool }` (66); `pub fn shortcut_catalog(&self)` (112) / `pub(super) fn catalog(launcher, state)` (119); per‑command wording built in `fn command(...)` (229) — `hotkey_inactive` from unavailability / disabled package / `bindings.problem_of`; `fn missing(...)` (279) for commands gone from a package; "Not installed" group (204).

## 6. Launcher-level wiring in the app (pane crate)

`crates/pane/src/main.rs`:
- 222–223: `let (press_sender, mut presses) = pane_core::hotkeys::channel(); let launcher = launcher.with_hotkeys(pane_core::hotkeys::native(press_sender));` — "the system's adapter is made on the main thread, whose run loop receives the presses on macOS".
- 240–249: quit hook — tray hidden + `quitting.release_hotkeys()` + `quit_cleanly()` (`WM_ENDSESSION` too).
- 342–348: `cx.spawn(async … while let Some(shortcut) = presses.next().await { window.update(... launcher.hotkey_pressed(&shortcut, window, cx)) })` — **every press lands in `LauncherWindow::hotkey_pressed`**.

`crates/pane/src/app.rs`:
- `LauncherWindow::new` (~169): calls `crate::settings::attach_launcher(&launcher, cx)` (~183–184) — startup application of the recorded Open Pane hotkey.
- `pub fn hotkey_pressed(&mut self, shortcut: &Shortcut, window, cx)` (822): if `launcher.opens_pane(shortcut)` → `open_pane_pressed` (962); else `hotkey_shows_window` → `press_hotkey`; when the window is shown: `unhide` (846) + `activate_window()` + `motion.land_at_once()`; `show_until_done(pending, …)`.
- `fn open_pane_pressed` (962) — `presence.press(now, active)` → `Repeat | Hide | Summon`; `fn summon` (~1000) — reopening choice, `pops_to_root`, `unhide`, activate, focus query.
- `fn key_down` (1053) — **the launcher hotkey screen's recorder**: on `Screen::Hotkey`, builds `Shortcut::new(control, alt, shift, platform, &keystroke.key)`, records or shows the error.
- `tray_selected` (1040) — tray Open Pane → `summon` (never hides).
- Root rows show the command's hotkey as keycaps: `shown.hotkey.as_ref().map(crate::keyboard::hotkey_keys)` (1403).

`crates/pane/src/app/presence.rs`:
- `const OPEN_PANE_REPEAT: Duration = Duration::from_millis(600)` (22) — the 600 ms repeat guard (Windows/X11 stop repeat at source; macOS Carbon re‑reports).
- `pub(crate) enum Press { Repeat, Hide, Summon }` (~62); `pub(crate) fn press(&mut self, now: Instant, active: bool) -> Press` (~95); `hide`/`show`/`pops_to_root`.

`crates/pane/src/settings.rs` (the host‑settings entity):
- `launcher: Option<Launcher>` field (239) — "applies the Open Pane hotkey to the system, once a launcher window has attached it".
- `pub(crate) fn set_open_pane(&mut self, shortcut: Shortcut, cx) -> Result<(), String>` (835) — register first via `launcher.set_open_pane`, then `commit` (964) / `written` (1062) off‑thread; failed save rolls the registration back to the record.
- `pub fn init_with_overrides(dir, overrides, cx)` (1338); `ensure` (1370); `shared` (1380).
- `pub(crate) fn attach_launcher(launcher: &Launcher, cx: &mut App)` (1411) — reads the record's `open_pane()` and calls `launcher.sync_open_pane(recorded)`; `pub fn attach_tray(...)` (1435).

## 7. UI — Settings pages

Settings window: `crates/pane/src/features/settings/mod.rs` — pages registered in `SettingsWindow::new`; `pub(crate) fn captured_while_recording(context) -> [KeyBinding; 4]` (~98) binds Up/Down/close/find to `NoAction` in a listening recorder's context. Pages: `general.rs`, `appearance.rs`, `extensions.rs`, `file_search.rs`, `keyboard.rs`, `launcher.rs`, `about.rs`, `search.rs`, `shortcuts.rs`.

**General page** — `crates/pane/src/features/settings/general.rs`:
- `pub(crate) const TITLE: &str = "General"`; `RECORDER`/`RECORDER_IDLE` key contexts (~108); `actions!(general, [ActivateRecorder, CancelRecording])`; `bind_keys` (~120): Enter/Space activate, Escape/Tab/shift‑Tab cancel recording.
- `pub(crate) struct State { recording: bool, rejection: Option<String>, tray_refusal, focus }` (~181).
- `entries()` (199) — search entries "Open Pane hotkey" (unavailable = `launcher.open_pane_problem()`), "Reset the Open Pane hotkey", login, tray.
- `struct GeneralView { keys: KeySequence, binding, default, recording, resettable, problem, rejection, … }` (~258); `render` (287); `recorder_row` (~524) — label `"Recording; Open Pane with {binding}"`, note `"Not active: {problem}"` in warning tone; Reset via `apply_open_pane(Shortcut::open_pane_default(), …)`.
- Recorder behavior on `SettingsWindow`: `activate_recorder`/`cancel_recording`/`start_recorder`/`stop_recording`/`recorder_key_down` (~668–750) and `apply_open_pane` (~756) — goes through `crate::settings::shared(cx).update(|s,cx| s.set_open_pane(shortcut, cx))`.

**Shortcuts page** — `crates/pane/src/features/shortcuts.rs` (`crates/pane/src/features/settings/shortcuts.rs`):
- Key contexts `PAGE/GROUP/CELL/EDITOR/HOTKEY_CELL/HOTKEY_RECORDER/HOTKEY_CLEAR` (84–99); `actions!(shortcuts, [EditAlias, CommitAlias, CancelAlias, ToggleGroup, RecordHotkey, CancelHotkeyRecording, ClearHotkey])` (105); `bind_keys` (118).
- `struct State { query, collapsed, disclosures, editing: Option<Editing>, recording: Option<Recording>, alias_cells/hotkey_cells/hotkey_clears/group_cells: HashMap<String, FocusHandle>, drawn: Option<ShortcutCatalog>, status, … }` (~255); `struct Recording { command, rejection }` (~264).
- Watcher: `pub(crate) const WATCH: Duration = Duration::from_millis(500)` (113); `shortcuts_watched` (~737) redraws on catalog change.
- `render` (~750) reads `launcher.shortcut_catalog()` fresh; `columns_header(hotkeys_unavailable, …)` (924) — whole‑column note `"Hotkeys are unavailable here: {why}"` (selector `shortcut-hotkeys-unavailable`); group headers show `"Not active: {why}"` for disabled/paused packages; `hotkey_cell` (~1507) — recorder with Clear button, `"Recording; Hotkey for {title}: …"` a11y label, notes `"Not active: {why}"` (`shortcut-hotkey-inactive-{id}`) and rejection (`shortcut-hotkey-error`); non‑editable commands draw at disabled opacity.
- Behavior: `shortcuts_record_hotkey` (587), `shortcuts_cancel_hotkey`/`shortcuts_cancel_recording`, `shortcuts_hotkey_key_down` (~625, builds `Shortcut::new`), `shortcuts_apply_hotkey` (~652 → `launcher.set_hotkey`), `shortcuts_hotkey_recorded` (outcome → status line `shortcut-status`).

**Keyboard page** — `crates/pane/src/features/settings/keyboard.rs` — yes, it exists: the *in‑app* navigation bindings (not global hotkeys). Bounded action set from `crates/pane-core/src/keyboard.rs` (`Keyboard`, `Binding`, `KeyboardAction::ALL`), recorded in settings.json's `keyboard` field; recorder is the same Discord‑style widget with its own `KeyboardRecorder` context.

**Recorder widget** — `crates/pane/src/ui/controls.rs`: `pub(crate) const RECORDING_TEXT: &str = "Press a shortcut…"` (525); `pub(crate) fn binding_text(keys: &KeySequence) -> String` (529) — caps joined by " + "; `pub(crate) fn recorder(text, recording, trailing, theme) -> Div` (547) — 36‑high well, record mark, danger ring while recording; `icon_button` (661). Keycap conversion: `crates/pane/src/keyboard.rs` `pub(crate) fn binding_keys(binding: &Binding) -> KeySequence` (190) and `pub(crate) fn hotkey_keys(shortcut: &Shortcut) -> KeySequence` (231) (converts a `Shortcut` to a `Binding`; Win/Ctrl/Alt/Shift order on Windows, Control/Option/Shift/Command on macOS).

**Tray icon (Windows)** — `crates/pane-core/src/tray.rs` (`pub trait Tray` (100), `native` (151), `TrayAction { OpenPane, Settings, Quit }` (59), `channel()` (145)) + `crates/pane-core/src/tray/windows.rs`: `Shell_NotifyIcon` on a hidden‑window `MessageThread`; `const TIP: &str = "Pane"` (~91) is the tooltip, copied into `data.szTip` with `NIF_TIP` in `NotificationArea::notify` (614–619); `trait Shell` seam (174) with a `FakeShell` for unit tests (1157); GUID identity, `TaskbarCreated` re‑add, taskbar‑theme variants (`MARK_DARK`/`MARK_LIGHT`). Menu items Open Pane / Settings / Exit. Left click summons.

**"Not active: <reason>" rows are rendered at**: launcher extension list (`launcher/hotkeys.rs::hotkey_rows` 405), Shortcuts page group headers and hotkey cells (shortcuts.rs ~955/~1507), General page Open Pane row (general.rs ~524). Root search rows show the hotkey as keycap hints (app.rs 1403).

**Platform‑availability (#19) mechanism**: `crates/pane-core/src/launcher.rs` — `pub struct Row { id, title, subtitle, unavailable: Option<Unavailable> }` (322–329); `pub enum Unavailable { OnThisSystem(String), Paused(String) }` (331–338); `impl Unavailable { pub fn reason(&self) -> &str }` (~401). Unavailable rows stay listed/selectable; activating shows the reason (`Entry::Unavailable`). `hotkey_rows` maps adapter `unavailable()` into `Unavailable::OnThisSystem`. Documented in `docs/platform-availability.md` and referenced from `docs/hotkeys.md`.

## 8. Tests

- `crates/pane-core/tests/hotkeys.rs` (659 lines) — public‑interface coverage with the settings‑sample guests and `FakeSystem`. Helpers: `key()` (102), `package()` (107, copies from `target/guests/packages`, needs `cargo xtask guests`), `struct Dirs { sources, data }` (129) with `packages_dir() = data/extensions`, `launcher()` (139 — restart = new launcher over same folder), `install()` (147), `manage()` (179), `assign()` (189 — Manage extensions → hotkey row → `record_hotkey`), `error()` (197). 21 tests, including: assign registers + press opens from another app and from an open command; kept & re‑registered after restart (`hotkeys.json` checked); changing releases the old; Remove hotkey; disable releases / enable restores (across restart); paused extension's hotkey explains the pause; taken by another application (explained, old kept); taken hotkey keeps the one it would replace; shortcut of another command refused; no Ctrl/Alt/Super and reserved refused; unavailable everywhere (rows say why, `Unavailable::OnThisSystem`); taken‑meanwhile explained after restart; Escape changes nothing; JavaScript command; uninstall releases and forgets (even with saved data kept; exactly its own ids — "x" vs "x#y"); update keeps / dropped command releases; a press that opens nothing leaves Pane as it was (`press_hotkey` → `None`, window not raised); removed‑right‑after‑assigned stays removed; unrecordable change undone in Pane and on disk.
- `crates/pane-core/tests/hotkey_adapters.rs` — real adapters: X11 on Xvfb (`PANE_XVFB`, `PANE_XDOTOOL`; skipped without Xvfb) — grab reported / refused as Taken to a second client / released; a real `xdotool key ctrl+alt+p` reported once; dropping releases grabs; unreachable display explained. Windows (mod windows, ~157): the two tests in §2. macOS: smoke‑only (main run loop).
- `crates/pane-core/tests/no_view.rs` — no‑view hotkey behavior (§5). There is **no** `open_pane.rs` or `shortcuts.rs` in `crates/pane-core/tests/` — those live in the `pane` crate.
- `crates/pane/tests/hotkeys.rs` — GPUI test platform, `FakeSystem`, support `settle.rs` (`enter_flow`, `settle`, `settle_shown`) and `packages.rs` (`package`). Tests: pressing keys on the hotkey screen assigns them (`p` explained; `ctrl-alt-p` assigned; Open Pane default registered beside it at startup; press with a typed query opens the command, list focused, Escape back to root) and `a_command_hotkey_cannot_take_the_open_pane_keys` (refusal wording, nothing recorded).
- `crates/pane/tests/open_pane.rs` (831 lines) — `FakeSystem`/`UnavailableSystem`, helpers `open_settings` (96, opens Settings via `settings_shortcut()` from `support/setup.rs`), `press` (128, advances the clock 700 ms past the repeat guard then `window.hotkey_pressed`), `hidden` (136, `window.hidden()` test support). 10 `#[gpui::test]`s: default registered at start and press toggles (shown/no‑focus → Summon, focused → Hide, hidden → Summon); held key doesn't re‑toggle (`a_held_key_does_not_toggle_the_launcher_repeatedly`, 218); Settings' focus doesn't count and it stays open; recording swaps registration + record (`until_record_holds`); taken shortcut refused, keeps binding, another try lands; collision with a command hotkey refused; failed save rolls registration back; fresh app registers the record (`the_recorded_hotkey_is_registered_by_a_fresh_application`, uses `cx.new_app()`); Escape/Tab/click cancel and captured keys don't act; reset through the same checks; hotkey stays available while the runtime has failed (`Err(CallError::RuntimeUnavailable)` launcher); Wayland explanation on the General page.
- `crates/pane/tests/shortcuts.rs` (2129 lines) — Shortcuts page: `FakeHotkeys` (67), `query_package`/`hello_package` fixtures (93/118), `seed` (writes `aliases.json`/`hotkeys.json` by hand), `record_hotkey(settings, cx, id)` (419, clicks `shortcut-hotkey-{id}` cell and asserts "Recording; "), `press` (441). Coverage: catalog listing, inline hotkey recorded and cleared, collisions with another command and with Open Pane, taken‑by‑another‑app refusal, rollback when the record cannot be written, opened from another application, package lifecycle (disable/enable/uninstall/update) and restart, filter, group disclosure, watcher redraw, virtualized rows.
- Related: `crates/pane/tests/keyboard.rs` (Keyboard page + Open Pane presses), `tray.rs` (tray + Open Pane), `arguments.rs`/`no_view.rs` (hotkey‑run commands asking/hidden), `launcher_settings.rs`, `settings.rs`. Native smokes: `scripts/smoke-windows.ps1` 779–848 (hotkey phase, screenshots 52–56), `scripts/smoke-linux.sh` 861–909, `scripts/smoke-macos.sh` 752–810; `docs/evidence/settings-74/native-validation.md` for #74.

## 9. Diagnostics — `crates/pane-core/src/diagnostics.rs`

- One path: `pub fn report(site: &str, message: &str)` (70) — stderr + log; `report_line` (82); `diagnostic!` macro (47); `pub fn redacted(text: &str) -> String` (92) — same redaction as the log (home → `~`, user → `<user>`, computer → `<computer>`).
- Log: `pane.log` in `logs_dir` (see `pane/src/lib.rs::logs_dir`), rotated, rate‑limited, redacted; `start(folder, …)` (~140).
- **Copy Diagnostics**: `crates/pane/src/features/settings/about.rs` `fn diagnostics(update: &ApplicationUpdate, log: Option<&pane_core::LogNotice>) -> String` (~583) — composes "Pane {version} / Built for / Data folder / Log folder (+ quit unexpectedly) / Update check: …", all folder paths through `pane_core::diagnostics::redacted`. **Hook health joins this report here** (plus, if surfaced, the Shortcuts/General "Not active" plumbing of §5/§7).

## 10. Recorder UI flow (today)

Three recorders share one widget and one capture discipline, each with its own state:
1. Launcher hotkey screen — `Screen::Hotkey` + `LauncherWindow::key_down` (app.rs 1053) + `Launcher::record_hotkey` (launcher/hotkeys.rs 500). Keys Enter/Escape/arrows/Tab are bound by the launcher's keymap and never reach it; a modifier alone is not a key press; a bad key shows the `Shortcut::new` error as status.
2. General page Open Pane recorder — `general.rs` `State.recording`, contexts `RECORDER`/`RECORDER_IDLE`; Enter/Space start, Escape/Tab/click‑outside/second click cancel; captured keys go to `apply_open_pane` → `settings.set_open_pane` (register first).
3. Shortcuts page hotkey cells — `shortcuts.rs` `State.recording: Option<Recording>`; same keys via `HOTKEY_RECORDER` context plus `captured_while_recrowning`‑style `NoAction` bindings for Up/Down/close/find; Clear button; rejection shown under the cell.
4. Keyboard page recorders — `keyboard.rs`, same widget, per‑action.
**Where a recording‑session API would go**: today the "listening" state and rejection are duplicated per page and per window; a unified session (e.g. "recorder holds keys back from Windows while listening", ADR 0039) would be modeled in `pane-core` beside `hotkeys`/`keyboard` and surfaced through `Launcher`, with the pane crate's three UIs reduced to views over it. The hook's "hold keys back" part belongs in the Windows adapter (§2).

## 11. docs/hotkeys.md cross‑references and CONTEXT.md glossary

`docs/hotkeys.md` links out to: ADR 0016; ADR 0039/#125 (Defaults section: "#125 (ADR 0039) will change the Windows default on a fresh install later, and this document changes with it"); `docs/platform-availability.md#an-actions-supported-systems` (#19 unavailable rows); `docs/platforms/linux.md#global-hotkeys-32-33-34`, `macos.md#global-hotkeys-33`, `windows.md#global-hotkeys-32`; test files (pane-core/tests/hotkeys.rs, pane/tests/hotkeys.rs, open_pane.rs, shortcuts.rs, hotkey_adapters.rs); `docs/root-search.md` links in (`root-search.md:19`); "The record is kept as the [aliases](aliases.md)' is"; reserved lists "per system in `pane_core::hotkeys`". Its refusal‑wording table is the contract the core reproduces verbatim.

CONTEXT.md glossary entries touching hotkeys (just the terms): **Global hotkey** (239); **No-view command** (267 — "its hotkey runs it without showing Pane's window"); **Disabled extension** (35) / **Disabled command** (39) — hotkey kept, not active; **Extension page** (43) — commands each with alias, hotkey and switch; **Root provider** (207) — no alias/hotkey; **Actions panel** (139) — alias/hotkey configuration of an installed command; **Frecency** (119) — "a global hotkey's use … earns none"; **Launch record** (259) / **Command mode** (263). There is no dedicated "Open Pane hotkey" glossary entry.

WIT surface: `wit/commands.wit` `launch-source` includes `hotkey` (36–37); SDK side `guests/pane-extension/src/lib.rs:210 LaunchSource::Hotkey => "hotkey"`. Extensions cannot assign, read or declare hotkeys (no WIT/manifest change — ADR 0016/0039).

Game‑mode/foreground precedent: the only foreground‑window reading today is display placement — `crates/pane/src/placement/windows.rs` (`GetForegroundWindow` + `MonitorFromWindow`, read fresh per open) and `crates/pane-core/src/placement.rs` (renderer‑independent model). No persistent foreground watcher exists yet; a game mode would add one (e.g. `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`) plus a release/re‑register path through `sync_hotkeys`.

## Where new work goes

- **Hook fallback (WH_KEYBOARD_LL, ADR 0039)**: `crates/pane-core/src/hotkeys/windows.rs` (register‑first‑then‑hook in `Registered::register`; new `Request` variants; watchdog/thread priority/locked pages; injected‑key tag; admin‑foreground note) + `crates/pane-core/src/threads/windows.rs` (`MessageThread` infra) + `crates/pane-core/src/hotkeys.rs` (`HotkeyError::Taken` stops being fatal — adjust `Launcher::set_hotkey_of`/`sync_hotkeys` in `crates/pane-core/src/launcher/hotkeys.rs` 577/221 so a refused chord falls back rather than erroring) + adapter tests in `crates/pane-core/tests/hotkey_adapters.rs` (windows module).
- **New binding kinds (lone tap, double tap, side‑specific modifiers, extended key set)**: `crates/pane-core/src/hotkeys.rs` — `Shortcut` (48), `new` (61), `parse` (104), `id` (125), `refusal`/`reserved`/`editing_refusal` (171/210/192), `Display` (268); `PressSender::send` payload (~330) for tap semantics; adapters `hotkeys/windows.rs::virtual_key`/`modifiers` (44/57), `hotkeys/macos.rs`, `hotkeys/x11.rs`; display `crates/pane/src/keyboard.rs::hotkey_keys` (231); record grammar in `crates/pane-core/src/launcher/hotkeys.rs::HotkeyChoices::read/write` (69) and `host_settings.rs::Recorded::open_pane` (~452); recorder capture sites (§10).
- **Record v2 (`hotkeys.json`)**: `crates/pane-core/src/launcher/hotkeys.rs` (const VERSION, `read`, `write`) + the generic machinery `crates/pane-core/src/launcher/choices.rs` (`Choices` 39, `Record::open` ~105 — needs a migration/read‑both path; today another version is simply not read) + tests in `crates/pane-core/tests/hotkeys.rs` (restart tests read the file directly).
- **Dispatch/health UI (registered vs hook, "Not active", hook health)**: model — `crates/pane-core/src/launcher/hotkeys.rs` (`Registered` ~170, `problems` 126, `OpenPane.problem` 185, `shortcut_catalog` wording in `launcher/shortcuts.rs::command` 229); rows — `crates/pane/src/features/settings/shortcuts.rs` (`hotkey_cell` ~1507, `columns_header` 924) and `general.rs` (`recorder_row` ~524); extension list rows — `launcher/hotkeys.rs::hotkey_rows` (405); diagnostics — `crates/pane/src/features/settings/about.rs::diagnostics` (~583).
- **Recorder sessions (incl. holding keys back while listening)**: a new core‑level session beside `crates/pane-core/src/hotkeys.rs`/`keyboard.rs`, surfaced via `Launcher`; UI seams at `crates/pane/src/features/settings/general.rs` (`State.recording` ~181), `shortcuts.rs` (`State.recording` ~255/`Recording` ~264), `keyboard.rs`, and `crates/pane/src/app.rs::key_down` (1053); widget stays `crates/pane/src/ui/controls.rs::recorder` (547).
- **Game mode (foreground events)**: new foreground watcher (Windows `SetWinEventHook`; reuse precedent in `crates/pane/src/placement/windows.rs`), feeding a release/re‑register path in `crates/pane-core/src/launcher/hotkeys.rs::sync_hotkeys` (221) and the adapter's `unregister` (windows.rs 114).
- **Fresh‑install Windows‑key Open Pane default (ADR 0039)**: `crates/pane-core/src/hotkeys.rs::Shortcut::open_pane_default` (94) + `crates/pane-core/src/host_settings.rs` (`Default` ~299, `open` ~318 — "fresh install" today == missing `settings.json`; an explicit fresh‑vs‑existing distinction, and the Ctrl+Alt+Space fallback when the hook cannot install, need new decision here) + `crates/pane/src/features/settings/general.rs` (reset button/aria labels, the proposed "Use the Windows key" choice) + tests `crates/pane/tests/open_pane.rs` + update `docs/hotkeys.md` (its Defaults section already promises the change).

---
