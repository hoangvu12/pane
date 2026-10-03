//! The Launcher settings page (#78): the display the launcher opens on
//! and what reopening it starts from, driven through the real windows on
//! GPUI's test platform. The system is a fake placement — the display
//! layout it reports is chosen by the test, and every move it is asked
//! for is recorded — so the placement is exercised at the same boundary a
//! user sees it, the launcher window's opening, without owning this
//! machine's real displays. The hotkey system is the same fake the Open
//! Pane tests use, so the Open Pane hotkey can be pressed and a command's
//! global hotkey can be recorded ahead of the run. The real platform
//! halves (the GDI, RandR and CoreGraphics display lists, and the moves
//! through `SetWindowPos`, a configure request and `setFrameTopLeftPoint`)
//! are recorded natively in `docs/evidence/settings-78/`.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::executor::block_on;
use gpui::{
    AnyWindowHandle, Modifiers, TestAppContext, VisualTestContext, WindowHandle, prelude::*,
};
use pane::placement::Placement;
use pane::{LauncherWindow, SettingsWindow};
use pane_core::hotkeys::{HotkeyError, Hotkeys, Shortcut};
use pane_core::placement::{Display, DisplayId, DisplayLayout, Point, Rect, Size};
use pane_core::{Launcher, PackageIdentity, Runtime, Screen};
use tempfile::TempDir;

#[path = "support/settle.rs"]
mod settle;

use settle::settle;

/// The fake system for global hotkeys, as the Open Pane tests' one: what
/// Pane registered, and no shortcut another application has.
#[derive(Default)]
struct FakeSystem {
    registered: Mutex<Vec<Shortcut>>,
}

impl Hotkeys for FakeSystem {
    fn unavailable(&self) -> Option<String> {
        None
    }

    fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError> {
        self.registered.lock().unwrap().push(shortcut.clone());
        Ok(())
    }

    fn unregister(&self, shortcut: &Shortcut) {
        self.registered
            .lock()
            .unwrap()
            .retain(|kept| kept != shortcut);
    }
}

/// The fake placement: the display layout the test chooses, what it
/// refuses to tell, and a record of every move the launcher window was
/// placed with — the boundary the placement's decisions are observed at,
/// as the fake hotkey system's registrations are.
#[derive(Default)]
struct FakePlacement {
    layout: RefCell<DisplayLayout>,
    unavailable: RefCell<Option<String>>,
    refuse: RefCell<Option<String>>,
    moves: RefCell<Vec<Rect>>,
}

impl FakePlacement {
    /// The two-display layout the placement tests use: a primary display
    /// on the left and a second one to its right, with the pointer and
    /// the active window where the test puts them, or neither told.
    fn layout(&self, pointer: Option<Point>, active: Option<DisplayId>) {
        *self.layout.borrow_mut() = DisplayLayout {
            displays: vec![
                display(1, 0., 0., 1920., 1080., 40.),
                display(2, 1920., 0., 2560., 1440., 60.),
            ],
            primary: Some(DisplayId(1)),
            pointer,
            active,
        };
    }

    /// The origin of the move number `index`, the part of a placement a
    /// platform applies: the bounds it was asked for, at the top left.
    fn origin(&self, index: usize) -> Point {
        self.moves.borrow()[index].origin
    }
}

impl Placement for FakePlacement {
    fn unavailable(&self) -> Option<String> {
        self.unavailable.borrow().clone()
    }

    fn layout(&self) -> DisplayLayout {
        self.layout.borrow().clone()
    }

    fn place(&self, _window: &mut gpui::Window, bounds: Rect) -> Result<(), String> {
        if let Some(why) = self.refuse.borrow().clone() {
            return Err(why);
        }
        self.moves.borrow_mut().push(bounds);
        Ok(())
    }
}

/// A display with identity `id`, covering the square from (`x`, `y`) of
/// the given size, whose usable area is inset by the same amount on every
/// side.
fn display(id: u64, x: f32, y: f32, width: f32, height: f32, inset: f32) -> Display {
    Display {
        id: DisplayId(id),
        bounds: Rect {
            origin: Point { x, y },
            size: Size { width, height },
        },
        usable: Rect {
            origin: Point {
                x: x + inset,
                y: y + inset,
            },
            size: Size {
                width: width - 2. * inset,
                height: height - 2. * inset,
            },
        },
    }
}

/// Initializes the settings record of `data` in `cx`, as the binary does
/// before its first window opens.
fn init_settings(data: Option<&Path>, cx: &mut TestAppContext) {
    cx.update(|cx| {
        pane::settings::init_with_overrides(
            data.map(|data| data.to_owned()),
            pane::settings::Overrides::default(),
            cx,
        )
    });
}

/// The keystroke that opens Settings on this platform.
fn settings_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-,"
    } else {
        "ctrl-,"
    }
}

/// Opens the Settings window and moves it to the Launcher page by
/// clicking its sidebar row, ready for the page's own controls.
fn open_launcher_page(
    cx: &mut VisualTestContext,
) -> (WindowHandle<SettingsWindow>, VisualTestContext) {
    cx.simulate_keystrokes(settings_shortcut());
    cx.run_until_parked();
    let settings = cx
        .cx
        .update(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<SettingsWindow>())
                .next()
        })
        .expect("Settings opened");
    let mut settings_cx = VisualTestContext::from_window(AnyWindowHandle::from(settings), &cx.cx);
    click(&mut settings_cx, "section-Launcher");
    settings_cx.run_until_parked();
    assert!(
        settings_cx.debug_bounds("launcher-title").is_some(),
        "the Launcher page is drawn"
    );
    (settings, settings_cx)
}

/// Presses the Open Pane hotkey `shortcut`, as the system's adapter would
/// report it while any application has focus, moved past the window's
/// repeat guard so two of these are two genuine presses.
fn press(window: &gpui::Entity<LauncherWindow>, shortcut: &Shortcut, cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(700));
    window.update_in(cx, |window, w, cx| window.hotkey_pressed(shortcut, w, cx));
}

/// Whether the launcher window is hidden — hidden, not closed: Pane keeps
/// running, and the same live window answers the next opening.
fn hidden(window: &gpui::Entity<LauncherWindow>, cx: &VisualTestContext) -> bool {
    cx.read_entity(window, |window, _| window.hidden())
}

/// Hides the launcher with the Open Pane hotkey, as a dismissal does,
/// saying how many moves the placement has been asked for so far: hiding
/// moves nothing.
fn dismiss(
    window: &gpui::Entity<LauncherWindow>,
    shortcut: &Shortcut,
    cx: &mut VisualTestContext,
    placement: &FakePlacement,
) -> usize {
    let before = placement.moves.borrow().len();
    press(window, shortcut, cx);
    cx.run_until_parked();
    assert!(hidden(window, cx), "the launcher hid");
    assert_eq!(
        placement.moves.borrow().len(),
        before,
        "hiding moves nothing"
    );
    before
}

/// Clicks the element whose debug selector is `selector` in `cx`.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not drawn"));
    cx.simulate_click(bounds.center(), Modifiers::none());
}

/// The accessibility tree of the window `cx` drives, as raw JSON, forced
/// on so the tree is built regardless of platform accessibility.
fn a11y(cx: &mut VisualTestContext) -> String {
    cx.update(|window, _| window.set_a11y_forced(true));
    cx.run_until_parked();
    cx.update(|window, _| window.debug_a11y_tree_json())
        .expect("an accessibility tree")
}

/// Runs `cx` until the settings record in `data` holds `text`: the save
/// the page started is written off the window's thread.
fn until_record(cx: &mut VisualTestContext, data: &Path, text: &str) {
    let record = data.join("settings.json");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        if fs::read_to_string(&record)
            .ok()
            .is_some_and(|held| held.contains(text))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {text} in the record"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Writes a package folder whose one command is the Rust sample, as the
/// hotkey tests' fixture does.
fn package(folder: &Path) -> PathBuf {
    let guest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/guests/sample_rust.wasm");
    assert!(
        guest.exists(),
        "{} is missing; run `cargo xtask guests`",
        guest.display()
    );
    fs::create_dir_all(folder).unwrap();
    fs::write(
        folder.join("pane.json"),
        r#"{
  "manifestVersion": 1,
  "title": "Hello",
  "version": "1.0.0",
  "apiVersion": "0.1",
  "commands": [{ "id": "hello", "title": "Say hello", "component": "hello.wasm" }]
}"#,
    )
    .unwrap();
    fs::copy(guest, folder.join("hello.wasm")).unwrap();
    folder.to_path_buf()
}

/// The command id of the package at `folder`: its identity's key and its
/// manifest's command id.
fn command_id(folder: &Path) -> String {
    format!("{}#hello", PackageIdentity::local(folder).unwrap().key())
}

/// Records the hotkey `shortcut` for the package at `folder`'s command
/// into `data` before a launcher is made of it, as a Pane that ran before
/// would have.
fn seed_hotkey(data: &TempDir, folder: &Path, shortcut: &str) {
    let dir = data.path().join("extensions");
    fs::create_dir_all(&dir).unwrap();
    let hotkeys = [(command_id(folder), serde_json::json!(shortcut))]
        .into_iter()
        .collect::<serde_json::Map<String, serde_json::Value>>();
    fs::write(
        dir.join("hotkeys.json"),
        serde_json::json!({ "version": 1, "hotkeys": hotkeys }).to_string(),
    )
    .unwrap();
}

/// The launcher window over a launcher whose hotkey system and placement
/// are the fakes, with the settings record of `data`, and the placement
/// the window places through.
fn open<'a>(
    cx: &'a mut TestAppContext,
    data: Option<&Path>,
    placement: &Rc<FakePlacement>,
) -> (gpui::Entity<LauncherWindow>, &'a mut VisualTestContext) {
    cx.update(|cx| pane::placement::init(placement.clone() as Rc<dyn Placement>, cx));
    init_settings(data, cx);
    let launcher =
        Launcher::new(Runtime::start(), Vec::new()).with_hotkeys(Arc::new(FakeSystem::default()));
    cx.executor().allow_parking();
    cx.update(pane::bind_keys);
    let (window, cx) = cx.add_window_view(|window, cx| LauncherWindow::new(launcher, window, cx));
    (window, cx)
}

#[gpui::test]
fn the_launcher_opens_on_the_chosen_display_and_never_moves_settings(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 2500., y: 700. }), Some(DisplayId(2)));
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // The record holds nothing yet: the provisional default is the primary
    // display, and the window was placed on it as it opened — centered in
    // its usable area, which the first recorded move's origin names.
    let default = Shortcut::open_pane_default();
    assert_eq!(
        placement.origin(0),
        Point { x: 40., y: 40. },
        "the primary display's usable area"
    );

    // Reopening the launcher, with the pointer on the second display,
    // keeps the primary placement until the choice says otherwise.
    let before = dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    assert!(!hidden(window, cx));
    assert_eq!(placement.moves.borrow().len(), before + 1);
    assert_eq!(
        placement.origin(before),
        Point { x: 40., y: 40. },
        "the default still opens on the primary display"
    );

    // The choice, taken through the Launcher page's own control: the
    // pointer's display.
    let (settings, mut settings_cx) = open_launcher_page(cx);
    click(&mut settings_cx, "launcher-monitor-Pointer");
    settings_cx.run_until_parked();
    until_record(
        &mut settings_cx,
        data.path(),
        "\"openingMonitor\": \"pointer\"",
    );
    assert!(
        a11y(&mut settings_cx).contains("Primary display"),
        "the page shows the choices"
    );

    // The Settings window sits where it was opened; the launcher's next
    // opening moves the launcher only.
    let settings_before = settings_cx.update(|window, _| window.bounds());
    let before = dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    assert_eq!(
        placement.moves.borrow().len(),
        before + 1,
        "the launcher was placed"
    );
    assert_eq!(
        placement.origin(before),
        Point { x: 1980., y: 60. },
        "the pointer's display's usable area"
    );
    // The Settings window is exactly where it was: the launcher's
    // placement never moves it.
    let mut settings_cx = VisualTestContext::from_window(AnyWindowHandle::from(settings), &cx.cx);
    let still = settings_cx.update(|window, _| window.bounds());
    assert_eq!(still, settings_before, "the Settings window did not move");
}

#[gpui::test]
fn choices_the_system_does_not_answer_are_explained_not_offered(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    // Nothing is told: no pointer, no active window.
    placement.layout(None, None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    let (_settings, mut settings_cx) = open_launcher_page(cx);
    let tree = a11y(&mut settings_cx);
    // The two choices the system cannot answer are shown with their
    // reason, not offered.
    assert!(
        tree.contains("This system does not tell Pane where the pointer is"),
        "the pointer's display is explained, {tree}"
    );
    assert!(
        tree.contains("This system does not tell Pane which window is active"),
        "the active window's display is explained, {tree}"
    );
    // The primary display is always offered, and the default named as
    // provisional.
    assert!(tree.contains("Primary display"));
    assert!(tree.contains("provisional default"));
    // Choosing a choice that cannot be answered does nothing: the row is
    // not clickable, and nothing is saved.
    click(&mut settings_cx, "launcher-monitor-Pointer");
    settings_cx.run_until_parked();
    assert!(
        !data.path().join("settings.json").exists(),
        "nothing was kept"
    );
    let _ = window;
}

#[gpui::test]
fn a_disconnected_or_unanswered_choice_falls_back_and_says_so(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 2500., y: 700. }), None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // The active window's display is not told: the record can still hold
    // the choice (written on a system that could answer it), and the page
    // says what the launcher would open on instead.
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    click(&mut settings_cx, "launcher-monitor-ActiveWindow");
    settings_cx.run_until_parked();
    until_record(
        &mut settings_cx,
        data.path(),
        "\"openingMonitor\": \"active-window\"",
    );
    let tree = a11y(&mut settings_cx);
    assert!(
        tree.contains("cannot open on the display of the window you are working in"),
        "the fallback is explained, {tree}"
    );
    assert!(
        tree.contains("opens on the primary display instead"),
        "the fallback names what happens, {tree}"
    );

    // The launcher still opens on an available display: the fallback, not
    // nowhere.
    let default = Shortcut::open_pane_default();
    let before = dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    assert_eq!(placement.moves.borrow().len(), before + 1);
    assert_eq!(
        placement.origin(before),
        Point { x: 40., y: 40. },
        "the fallback placed the launcher on the primary display"
    );

    // A display that is gone: the choice falls back the same way, because
    // the layout no longer lists it.
    *placement.layout.borrow_mut() = DisplayLayout {
        displays: vec![display(2, 0., 0., 2560., 1440., 60.)],
        primary: Some(DisplayId(2)),
        pointer: Some(Point { x: 2500., y: 700. }),
        active: Some(DisplayId(1)),
    };
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    assert!(
        a11y(&mut settings_cx).contains("it is not connected"),
        "the disconnected display is explained"
    );
}

#[gpui::test]
fn a_platform_that_cannot_choose_the_display_explains_and_offers_nothing(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    *placement.unavailable.borrow_mut() =
        Some("Not available on Linux with Wayland: the compositor places windows itself".into());
    let (window, cx) = open(cx, Some(data.path()), &placement);

    let (_settings, mut settings_cx) = open_launcher_page(cx);
    let tree = a11y(&mut settings_cx);
    assert!(
        tree.contains("Not available on Linux with Wayland"),
        "the platform's reason is shown, {tree}"
    );
    // No opening-monitor choice is offered at all.
    assert!(
        !tree.contains("Primary display"),
        "no monitor choice is offered, {tree}"
    );
    // The reopening choice is unaffected: it is no platform integration.
    assert!(
        tree.contains("Restore the current view"),
        "reopening is offered, {tree}"
    );
    // The launcher still opens: nothing is placed, and nothing fails.
    let default = Shortcut::open_pane_default();
    let before = placement.moves.borrow().len();
    press(window, &default, cx);
    cx.run_until_parked();
    assert_eq!(placement.moves.borrow().len(), before, "nothing was placed");
}

#[gpui::test]
fn reopening_restores_the_view_and_focuses_its_search_by_default(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 100., y: 100. }), None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // A query typed into root search, then a dismissal: the default — the
    // parent specification's provisional one — restores the view the
    // launcher was left on, with its search focused, so typing lands in
    // the query. Nothing is dispatched by the reopening.
    cx.simulate_input("zz");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some("zz"));
    let default = Shortcut::open_pane_default();
    dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    assert!(!hidden(window, cx));
    let view = cx.read_entity(window, |window, _| window.launcher().view());
    assert_eq!(
        view.query(),
        Some("zz"),
        "the restored view kept what was typed"
    );
    cx.simulate_input("q");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some("zzq"), "the search has focus");
    assert!(matches!(view.screen, Screen::Root { .. }));
    assert_eq!(
        view.status,
        pane_core::Status::Idle,
        "nothing was dispatched"
    );
}

#[gpui::test]
fn choosing_root_search_starts_the_reopening_from_root_search(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(None, None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // The root-search choice, taken through the page's own control.
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    click(&mut settings_cx, "launcher-reopening-RootSearch");
    settings_cx.run_until_parked();
    until_record(
        &mut settings_cx,
        data.path(),
        "\"reopening\": \"root-search\"",
    );

    // A query typed, then a dismissal: the reopening starts from root
    // search with an empty query, whatever was left.
    cx.simulate_input("zz");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some("zz"));
    let default = Shortcut::open_pane_default();
    dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    let view = settle(window, cx);
    assert_eq!(view.query(), Some(""), "the query was left behind");
    assert!(matches!(view.screen, Screen::Root { .. }));
    cx.simulate_input("q");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some("q"), "root search's query has focus");
}

#[gpui::test]
fn a_view_whose_command_is_gone_returns_safely_to_root_search(cx: &mut TestAppContext) {
    let (sources, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let folder = package(&sources.path().join("hello"));
    let placement = Rc::new(FakePlacement::default());
    placement.layout(None, None);
    cx.update(|cx| pane::placement::init(placement.clone() as Rc<dyn Placement>, cx));
    init_settings(Some(data.path()), cx);
    let launcher =
        Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"))
            .with_hotkeys(Arc::new(FakeSystem::default()));
    cx.executor().allow_parking();
    cx.update(pane::bind_keys);
    let (window, cx) =
        cx.add_window_view(|window, cx| LauncherWindow::new(launcher.clone(), window, cx));

    // Install the package and open its command: the view is a command's,
    // which the default reopening restores while its package is installed
    // and enabled.
    let installing = launcher.install_package(&folder);
    cx.foreground_executor().block_on(installing);
    let view = settle(window, cx);
    assert!(
        view.rows.iter().any(|row| row.title == "Say hello"),
        "the package's command is listed"
    );
    cx.simulate_input("hello");
    let view = settle(window, cx);
    assert_eq!(view.selected, Some(0), "the command is the selected row");
    cx.simulate_keystrokes("enter");
    let view = settle(window, cx);
    assert!(matches!(view.screen, Screen::Command), "{:?}", view.screen);

    // The reopening restores the command's view.
    let default = Shortcut::open_pane_default();
    dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    let view = cx.read_entity(window, |window, _| window.launcher().view());
    assert!(
        matches!(view.screen, Screen::Command),
        "the command's view was restored, {:?}",
        view.screen
    );

    // The package disabled: its commands offer none, so the view is not
    // one to return to, and the reopening goes safely to root search.
    press(window, &default, cx);
    cx.run_until_parked();
    assert!(hidden(window, cx));
    let identity = launcher
        .packages()
        .first()
        .expect("the installed package")
        .identity
        .clone();
    block_on(launcher.set_enabled(&identity, false));
    press(window, &default, cx);
    cx.run_until_parked();
    let view = settle(window, cx);
    assert!(
        matches!(view.screen, Screen::Root { .. }),
        "an invalid view returned safely to root search, {:?}",
        view.screen
    );
}

#[gpui::test]
fn a_commands_hotkey_still_opens_its_command_with_the_root_preference(cx: &mut TestAppContext) {
    let (sources, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let folder = package(&sources.path().join("hello"));
    // A command hotkey recorded by a Pane that ran before.
    let hello_hotkey = Shortcut::parse("ctrl+alt+h").unwrap();
    seed_hotkey(&data, &folder, "ctrl+alt+h");
    let placement = Rc::new(FakePlacement::default());
    placement.layout(None, None);
    cx.update(|cx| pane::placement::init(placement.clone() as Rc<dyn Placement>, cx));
    // The root-search preference, written ahead of the run as a Pane that
    // ran before would have.
    fs::write(
        data.path().join("settings.json"),
        "{ \"version\": 1, \"reopening\": \"root-search\" }",
    )
    .unwrap();
    let launcher =
        Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"))
            .with_hotkeys(Arc::new(FakeSystem::default()));
    cx.executor().allow_parking();
    cx.update(pane::bind_keys);
    let (window, cx) = cx.add_window_view(|window, cx| LauncherWindow::new(launcher, window, cx));

    // Install the package and open its command, then leave it open.
    let installing = launcher.install_package(&folder);
    cx.foreground_executor().block_on(installing);
    cx.simulate_input("hello");
    let view = settle(window, cx);
    assert_eq!(view.selected, Some(0));
    cx.simulate_keystrokes("enter");
    let view = settle(window, cx);
    assert!(matches!(view.screen, Screen::Command), "{:?}", view.screen);

    // Dismiss, then press the command's own global hotkey: it opens its
    // named command, not root search — the reopening preference governs
    // the launcher's opening, never a command's binding.
    let default = Shortcut::open_pane_default();
    dismiss(window, &default, cx, &placement);
    press(window, &hello_hotkey, cx);
    cx.run_until_parked();
    let view = settle(window, cx);
    assert!(
        matches!(view.screen, Screen::Command),
        "the command's hotkey opened its command, {:?}",
        view.screen
    );
}

#[gpui::test]
fn the_recorded_choices_are_applied_by_a_fresh_application(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 2500., y: 700. }), None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // Both choices, taken through the page's own controls.
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    click(&mut settings_cx, "launcher-monitor-Pointer");
    settings_cx.run_until_parked();
    click(&mut settings_cx, "launcher-reopening-RootSearch");
    settings_cx.run_until_parked();
    until_record(
        &mut settings_cx,
        data.path(),
        "\"openingMonitor\": \"pointer\"",
    );
    until_record(
        &mut settings_cx,
        data.path(),
        "\"reopening\": \"root-search\"",
    );

    // A fresh application over the same data folder: a new app, nothing
    // carried over but the executors; the settings the record alone, and
    // a fresh placement whose layout still reports the same displays.
    let mut fresh = cx.cx.new_app();
    let fresh_placement = Rc::new(FakePlacement::default());
    fresh_placement.layout(Some(Point { x: 2500., y: 700. }), None);
    fresh.update(|cx| pane::placement::init(fresh_placement.clone() as Rc<dyn Placement>, cx));
    fresh.update(|cx| {
        pane::settings::init_with_overrides(
            Some(data.path().to_owned()),
            pane::settings::Overrides::default(),
            cx,
        )
    });
    let launcher =
        Launcher::new(Runtime::start(), Vec::new()).with_hotkeys(Arc::new(FakeSystem::default()));
    let (window, fresh_cx) =
        fresh.add_window_view(|window, cx| LauncherWindow::new(launcher, window, cx));
    fresh_cx.run_until_parked();

    // The fresh window opened on the display the record names.
    assert_eq!(
        fresh_placement.origin(0),
        Point { x: 1980., y: 60. },
        "the recorded choice placed the fresh launcher"
    );
    // And the recorded reopening starts from root search.
    fresh_cx.simulate_input("zz");
    let view = settle(&window, fresh_cx);
    assert_eq!(view.query(), Some("zz"));
    let default = Shortcut::open_pane_default();
    dismiss(&window, &default, fresh_cx, &fresh_placement);
    press(&window, &default, fresh_cx);
    fresh_cx.run_until_parked();
    let view = settle(&window, fresh_cx);
    assert_eq!(view.query(), Some(""), "root search, as recorded");
}

#[gpui::test]
fn a_save_that_fails_is_reported_and_the_shown_choice_stays_what_was_saved(
    cx: &mut TestAppContext,
) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 2500., y: 700. }), None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    let default = Shortcut::open_pane_default();

    // A change that lands and is saved: the pointer's display.
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    click(&mut settings_cx, "launcher-monitor-Pointer");
    settings_cx.run_until_parked();
    until_record(
        &mut settings_cx,
        data.path(),
        "\"openingMonitor\": \"pointer\"",
    );

    // Break the record's replacement: a folder where the record belongs,
    // so the atomic write cannot rename over it.
    fs::remove_file(data.path().join("settings.json")).unwrap();
    fs::create_dir(data.path().join("settings.json")).unwrap();

    // Another change: it cannot be saved, and the failure is reported.
    click(&mut settings_cx, "launcher-reopening-RootSearch");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        settings_cx.run_until_parked();
        let tree = a11y(&mut settings_cx);
        if tree.contains("Pane could not save your choice") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: the page shows {tree}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // The shown choice is what the record last held: the save's failure
    // rolled the reopening choice back to the default.
    let tree = a11y(&mut settings_cx);
    assert!(
        tree.contains("Restore the current view"),
        "the shown choice is the one that was saved, {tree}"
    );

    // The choice that could not be saved never took effect: the launcher's
    // next opening places as the record holds.
    let before = dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    cx.run_until_parked();
    assert_eq!(placement.moves.borrow().len(), before + 1);
    assert_eq!(
        placement.origin(before),
        Point { x: 1980., y: 60. },
        "the saved choice is the one that works"
    );
}

#[gpui::test]
fn a_move_that_fails_is_said_not_hidden(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(Some(Point { x: 2500., y: 700. }), None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // The platform refuses the move: the launcher's own status line says
    // so, so an opening that did not go where the choice says is never
    // mistaken for one that did.
    *placement.refuse.borrow_mut() = Some("the window manager refused the move".into());
    let default = Shortcut::open_pane_default();
    let before = dismiss(window, &default, cx, &placement);
    press(window, &default, cx);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        let view = cx.read_entity(window, |window, _| window.launcher().view());
        if matches!(view.status, pane_core::Status::Error(_)) {
            let message = match view.status {
                pane_core::Status::Error(message) => message,
                _ => unreachable!(),
            };
            assert!(
                message.contains("the window manager refused the move"),
                "the failure is said: {message}"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: the launcher shows {view:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        placement.moves.borrow().len(),
        before,
        "the refused move recorded nothing"
    );
}

#[gpui::test]
fn escape_at_root_search_with_an_empty_query_hides_the_launcher(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(None, None);
    let (window, cx) = open(cx, Some(data.path()), &placement);

    // A query typed: Escape clears it, as it always has, and hides
    // nothing.
    cx.simulate_input("zz");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some("zz"));
    cx.simulate_keystrokes("escape");
    let view = settle(window, cx);
    assert_eq!(view.query(), Some(""));
    assert!(!hidden(window, cx), "the query was cleared, nothing hid");

    // Root search, an empty query: the end of the Escape chain — nothing
    // is left to back out of — dismisses the launcher. Hidden, not
    // closed: the same live window answers the next opening, and a
    // placement is applied to it as to any opening.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(hidden(window, cx), "the launcher hid");
    let default = Shortcut::open_pane_default();
    press(window, &default, cx);
    cx.run_until_parked();
    assert!(!hidden(window, cx), "the hidden launcher was shown again");
}

#[gpui::test]
fn the_page_registers_its_settings_in_the_host_page_catalog(cx: &mut TestAppContext) {
    let data = tempfile::tempdir().unwrap();
    let placement = Rc::new(FakePlacement::default());
    placement.layout(None, None);
    let (_window, cx) = open(cx, Some(data.path()), &placement);

    // The Launcher page's settings are registered under their searchable
    // labels, each with the page it is on — the catalog the Settings
    // search (#83) filters.
    let (_settings, mut settings_cx) = open_launcher_page(cx);
    let labels: Vec<(String, String)> = settings_cx.update(|_, cx| {
        let settings = cx
            .windows()
            .into_iter()
            .filter_map(|window| window.downcast::<SettingsWindow>())
            .next()
            .expect("the Settings window");
        settings
            .update(cx, |window, _, _| window.searchable_labels())
            .expect("the window answers")
            .into_iter()
            .map(|(page, label)| (page.to_owned(), label.to_owned()))
            .collect()
    });
    assert!(
        labels.contains(&("Launcher".into(), "opening monitor".into())),
        "the opening monitor is registered, {labels:?}"
    );
    assert!(
        labels.contains(&("Launcher".into(), "reopening".into())),
        "reopening is registered, {labels:?}"
    );
    assert!(
        labels.contains(&("Launcher".into(), "pointer's display".into())),
        "the choices are registered by the words a user would look for, {labels:?}"
    );
}
