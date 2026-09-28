//! Clipboard history, a default extension, through the launcher's public
//! interface, with a fake system clipboard: nothing is kept until the user
//! turns it on in the command; then Pane watches the clipboard and keeps
//! plain text, except what is marked as not to be kept or comes from an
//! excluded program; pausing, disabling and uninstalling stop the watch at
//! once, and a restart watches again only where history is on and the
//! package enabled. The package is the one `cargo xtask guests` assembles
//! in `target/guests/packages/clipboard-history`; since only Windows has a
//! clipboard adapter so far, most tests install a copy whose manifest
//! declares every system, so the same checks run everywhere. The system's
//! real clipboard is never used here.

#[path = "support/platforms.rs"]
mod platforms;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use pane_core::clipboard::{
    ClipboardSystem, Content, MAX_ITEMS, Markers, Observation, Sink, Watch,
};
use pane_core::{Launcher, Runtime, Screen, Status, Unavailable};
use serde_json::Value;
use tempfile::TempDir;

const TITLE: &str = "Clipboard History";
const MANAGE_ROW: &str = "Manage extensions…";
const TURN_ON: &str = "Turn on clipboard history";
const PAUSE: &str = "Pause clipboard history";
const RESUME: &str = "Resume clipboard history";

fn built(path: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests")
        .join(path);
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    path
}

#[derive(Default)]
struct Clipboard {
    /// Where changes go while Pane watches.
    sink: Option<Arc<Sink>>,
    /// How many times Pane started watching.
    started: usize,
    /// What Pane put on the clipboard.
    written: Vec<String>,
}

/// A system clipboard that records what Pane does with it, and reports
/// only the changes a test makes.
#[derive(Clone, Default)]
struct FakeClipboard {
    inner: Arc<Mutex<Clipboard>>,
    unavailable: Option<String>,
}

/// Watching the fake clipboard, until dropped.
struct FakeWatch(Arc<Mutex<Clipboard>>);

impl Drop for FakeWatch {
    fn drop(&mut self) {
        self.0.lock().unwrap().sink = None;
    }
}

impl FakeClipboard {
    fn watching(&self) -> bool {
        self.inner.lock().unwrap().sink.is_some()
    }

    fn started(&self) -> usize {
        self.inner.lock().unwrap().started
    }

    fn written(&self) -> Vec<String> {
        self.inner.lock().unwrap().written.clone()
    }

    /// Reports `observation` as a change of the clipboard, if Pane
    /// watches; returns whether it did.
    fn change(&self, observation: Observation) -> bool {
        let sink = self.inner.lock().unwrap().sink.clone();
        match sink {
            Some(sink) => {
                sink(observation);
                true
            }
            None => false,
        }
    }

    /// Reports `text` copied from `source`.
    fn copy(&self, text: &str, source: Option<&str>) -> bool {
        self.change(Observation {
            content: Content::Text(text.into()),
            markers: Markers::default(),
            source: source.map(str::to_owned),
        })
    }
}

impl ClipboardSystem for FakeClipboard {
    fn unavailable(&self) -> Option<String> {
        self.unavailable.clone()
    }

    fn watch(&self, sink: Sink) -> Result<Watch, String> {
        if let Some(reason) = &self.unavailable {
            return Err(reason.clone());
        }
        let mut inner = self.inner.lock().unwrap();
        assert!(inner.sink.is_none(), "Pane watches the clipboard once");
        inner.sink = Some(Arc::new(sink));
        inner.started += 1;
        Ok(Box::new(FakeWatch(self.inner.clone())))
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        self.inner.lock().unwrap().written.push(text.into());
        // As on a real system, writing is a change Pane sees.
        self.copy(text, Some("pane.exe"));
        Ok(())
    }
}

/// Pane's data location and the package's source folder for one test,
/// which outlive restarts.
struct Pane {
    data: TempDir,
    source: TempDir,
    clipboard: FakeClipboard,
}

impl Pane {
    fn new() -> Pane {
        Pane::with(FakeClipboard::default())
    }

    fn with(clipboard: FakeClipboard) -> Pane {
        let source = tempfile::tempdir().unwrap();
        copy_package(source.path(), true);
        Pane {
            data: tempfile::tempdir().unwrap(),
            source,
            clipboard,
        }
    }

    fn folder(&self) -> &Path {
        self.source.path()
    }

    /// Starts Pane on this data location, as after a restart.
    fn start(&self) -> Launcher {
        Launcher::with_packages(
            Runtime::start(),
            vec![],
            self.data.path().join("extensions"),
        )
        .with_clipboard(Arc::new(self.clipboard.clone()))
    }

    /// Starts Pane with the package installed.
    fn installed(&self) -> Launcher {
        let launcher = self.start();
        block_on(launcher.install_package(self.folder()));
        assert!(
            matches!(launcher.view().status, Status::Result(_)),
            "{:?}",
            launcher.view().status
        );
        launcher.back();
        launcher
    }

    /// Every package's clipboard history as Pane keeps it on disk.
    fn history_file(&self) -> Option<Value> {
        let path = self
            .data
            .path()
            .join("extensions")
            .join("clipboard-history.json");
        let text = fs::read_to_string(path).ok()?;
        Some(serde_json::from_str(&text).unwrap())
    }

    /// The texts kept on disk, for the only package that keeps any.
    fn kept_on_disk(&self) -> Vec<String> {
        let Some(file) = self.history_file() else {
            return Vec::new();
        };
        let packages = file["packages"].as_object().unwrap();
        assert!(packages.len() <= 1, "{packages:?}");
        let Some(values) = packages.values().next() else {
            return Vec::new();
        };
        let mut items: Vec<(&String, &Value)> = values
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with("item:"))
            .collect();
        items.sort_by(|a, b| b.0.cmp(a.0));
        items
            .into_iter()
            .map(|(_, value)| {
                let item: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
                item["text"].as_str().unwrap().to_owned()
            })
            .collect()
    }
}

/// Copies the assembled package into `folder`; with `everywhere`, its
/// command declares every system, so it is available where these tests run.
fn copy_package(folder: &Path, everywhere: bool) {
    let package = built("packages/clipboard-history");
    let component = "clipboard_history.wasm";
    fs::copy(package.join(component), folder.join(component)).unwrap();
    let manifest = fs::read_to_string(package.join("pane.json")).unwrap();
    let mut manifest: Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(
        manifest["commands"][0]["platforms"],
        serde_json::json!(["windows"])
    );
    if everywhere {
        manifest["commands"][0]["platforms"] = serde_json::json!(["windows", "macos", "linux"]);
    }
    fs::write(folder.join("pane.json"), manifest.to_string()).unwrap();
}

fn titles(launcher: &Launcher) -> Vec<String> {
    launcher
        .view()
        .rows
        .into_iter()
        .map(|row| row.title)
        .collect()
}

fn subtitle(launcher: &Launcher, title: &str) -> String {
    launcher
        .view()
        .rows
        .into_iter()
        .find(|row| row.title == title)
        .and_then(|row| row.subtitle)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)))
}

fn select_title(launcher: &Launcher, title: &str) {
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    launcher.select(index);
}

/// Opens the command from root search, showing its view as it is now.
fn open(launcher: &Launcher) {
    launcher.back();
    launcher.back();
    block_on(launcher.set_query("clipboard"));
    select_title(launcher, TITLE);
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().screen,
        Screen::Command,
        "{:?}",
        launcher.view().status
    );
}

/// Runs the item titled `title` of the open command.
fn run(launcher: &Launcher, title: &str) -> Status {
    select_title(launcher, title);
    block_on(launcher.activate_selected());
    launcher.view().status
}

/// The kept texts the command lists, newest first: the rows after its
/// controls.
fn listed(launcher: &Launcher) -> Vec<String> {
    open(launcher);
    launcher
        .view()
        .rows
        .into_iter()
        .filter(|row| {
            row.subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.ends_with("Enter copies it"))
        })
        .map(|row| row.title)
        .collect()
}

fn turn_on(launcher: &Launcher) {
    open(launcher);
    assert_eq!(
        run(launcher, TURN_ON),
        Status::Result("Clipboard history is on".into())
    );
}

fn result(text: &str) -> Status {
    Status::Result(text.into())
}

#[test]
fn nothing_is_watched_or_kept_until_history_is_turned_on() {
    let pane = Pane::new();
    let launcher = pane.installed();
    assert!(!pane.clipboard.watching());

    open(&launcher);
    assert_eq!(titles(&launcher), [TURN_ON, "Exclude a program"]);
    assert_eq!(
        subtitle(&launcher, TURN_ON),
        "Off · Pane keeps nothing you copy until you turn it on. Once on, it keeps the text you \
         copy on this computer; nothing is sent anywhere"
    );
    assert!(!pane.clipboard.copy("before", Some("notepad.exe")));
    assert_eq!(pane.clipboard.started(), 0);

    assert_eq!(run(&launcher, TURN_ON), result("Clipboard history is on"));
    assert!(pane.clipboard.watching());
    // Pressing it again before the view is shown anew changes nothing.
    assert_eq!(run(&launcher, TURN_ON), result("Clipboard history is on"));
    assert_eq!(pane.clipboard.started(), 1);
    assert!(pane.clipboard.copy("hello", Some("notepad.exe")));
    assert!(pane.clipboard.copy("second line\nand more", None));

    assert_eq!(listed(&launcher), ["second line", "hello"]);
    assert_eq!(
        subtitle(&launcher, "hello"),
        "just now · from notepad.exe · Enter copies it"
    );
    assert_eq!(
        subtitle(&launcher, "second line"),
        "just now · 2 lines · Enter copies it"
    );
    assert_eq!(
        subtitle(&launcher, PAUSE),
        "On · 2 items kept · Text you copy is kept on this computer"
    );
    assert_eq!(pane.kept_on_disk(), ["second line\nand more", "hello"]);
    let file = fs::read_to_string(pane.data.path().join("extensions/clipboard-history.json"));
    assert!(!file.unwrap().contains("before"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = pane.data.path().join("extensions/clipboard-history.json");
        let mode = fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn marked_blank_other_and_long_content_is_not_kept() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    let marked = |markers: Markers| Observation {
        content: Content::Text("hunter2".into()),
        markers,
        source: Some("keepassxc.exe".into()),
    };
    assert!(pane.clipboard.change(marked(Markers {
        exclude_from_monitoring: true,
        ..Markers::default()
    })));
    pane.clipboard.change(marked(Markers {
        include_in_history: Some(false),
        ..Markers::default()
    }));
    pane.clipboard.change(marked(Markers {
        upload_to_cloud: Some(false),
        ..Markers::default()
    }));
    pane.clipboard.change(Observation {
        content: Content::Withheld,
        markers: Markers::default(),
        source: None,
    });
    pane.clipboard.change(Observation {
        content: Content::Other,
        markers: Markers::default(),
        source: None,
    });
    pane.clipboard.copy("   \n", None);
    pane.clipboard
        .copy(&"x".repeat(pane_core::clipboard::MAX_TEXT_BYTES + 1), None);
    pane.clipboard.copy("kept", None);

    assert_eq!(listed(&launcher), ["kept"]);
    assert_eq!(pane.kept_on_disk(), ["kept"]);
    let file = fs::read_to_string(pane.data.path().join("extensions/clipboard-history.json"));
    assert!(!file.unwrap().contains("hunter2"));
}

#[test]
fn text_from_an_excluded_program_is_not_kept() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    select_title(&launcher, "Exclude a program");
    block_on(launcher.activate_selected());
    assert!(launcher.view().form().is_some());
    launcher.set_field_value("program", r"C:\KeePass.exe");
    block_on(launcher.submit_form());
    assert!(matches!(launcher.view().status, Status::Error(_)));
    launcher.set_field_value("program", " KeePass.exe ");
    block_on(launcher.submit_form());
    assert_eq!(
        launcher.view().status,
        result("Text copied from KeePass.exe is not kept")
    );

    pane.clipboard.copy("secret", Some("KEEPASS.EXE"));
    pane.clipboard.copy("note", Some("notepad.exe"));
    assert_eq!(listed(&launcher), ["note"]);
    assert_eq!(
        subtitle(&launcher, "Exclude a program"),
        "Text copied from it is never kept · 1 excluded"
    );
    assert_eq!(
        run(&launcher, "Stop excluding keepass.exe"),
        result("Text copied from keepass.exe is kept again")
    );
    pane.clipboard.copy("secret again", Some("keepass.exe"));
    assert_eq!(listed(&launcher), ["secret again", "note"]);
}

#[test]
fn pausing_stops_the_watch_and_resuming_starts_it_again() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    pane.clipboard.copy("one", None);
    open(&launcher);
    assert_eq!(run(&launcher, PAUSE), result("Clipboard history is paused"));
    assert!(!pane.clipboard.watching());
    assert!(!pane.clipboard.copy("while paused", None));

    open(&launcher);
    assert_eq!(
        subtitle(&launcher, RESUME),
        "Paused · 1 item kept · Nothing you copy is kept until you resume"
    );
    // Paused stays paused after a restart.
    drop(launcher);
    let launcher = pane.start();
    assert!(!pane.clipboard.watching());
    open(&launcher);
    assert_eq!(
        run(&launcher, RESUME),
        result("Clipboard history is on again")
    );
    assert!(pane.clipboard.watching());
    pane.clipboard.copy("two", None);
    assert_eq!(listed(&launcher), ["two", "one"]);
}

#[test]
fn disabling_stops_the_watch_and_a_restart_watches_only_while_enabled() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    pane.clipboard.copy("kept", None);
    let identity = launcher.packages()[0].identity.clone();

    block_on(launcher.set_enabled(&identity, false));
    assert!(!pane.clipboard.watching());
    assert!(!pane.clipboard.copy("while disabled", None));
    // Disabled, it stays unwatched after a restart; its history is kept.
    drop(launcher);
    let launcher = pane.start();
    assert!(!pane.clipboard.watching());
    assert_eq!(pane.kept_on_disk(), ["kept"]);

    block_on(launcher.set_enabled(&identity, true));
    assert!(pane.clipboard.watching());
    pane.clipboard.copy("after enabling", None);
    // Enabled and on, a restart watches again at once.
    drop(launcher);
    assert!(!pane.clipboard.watching());
    let launcher = pane.start();
    assert!(pane.clipboard.watching());
    pane.clipboard.copy("after restart", None);
    assert_eq!(
        listed(&launcher),
        ["after restart", "after enabling", "kept"]
    );
}

#[test]
fn enter_copies_an_item_again_and_it_moves_to_the_front() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    pane.clipboard.copy("first", None);
    pane.clipboard.copy("second", None);
    assert_eq!(listed(&launcher), ["second", "first"]);

    assert_eq!(run(&launcher, "first"), result("Copied to the clipboard"));
    assert_eq!(pane.clipboard.written(), ["first"]);
    assert_eq!(listed(&launcher), ["first", "second"]);
    assert_eq!(pane.kept_on_disk(), ["first", "second"]);
}

#[test]
fn at_most_the_newest_items_are_kept_and_clear_deletes_them_all() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    for number in 0..=MAX_ITEMS {
        pane.clipboard.copy(&format!("item {number}"), None);
    }
    let kept = pane.kept_on_disk();
    assert_eq!(kept.len(), MAX_ITEMS);
    assert_eq!(kept[0], format!("item {MAX_ITEMS}"));
    assert!(!kept.contains(&"item 0".to_string()));

    open(&launcher);
    assert_eq!(
        run(&launcher, "Clear clipboard history"),
        result(&format!("Deleted {MAX_ITEMS} kept items"))
    );
    assert!(pane.kept_on_disk().is_empty());
    // History is still on.
    assert!(pane.clipboard.watching());
    pane.clipboard.copy("after clearing", None);
    assert_eq!(listed(&launcher), ["after clearing"]);
}

/// From root search, uninstalls the package with the confirmation row
/// `choice`, returning the confirmation's details.
fn uninstall(launcher: &Launcher, choice: &str) -> Vec<String> {
    launcher.back();
    launcher.back();
    select_title(launcher, MANAGE_ROW);
    block_on(launcher.activate_selected());
    select_title(launcher, &format!("Uninstall {TITLE}"));
    block_on(launcher.activate_selected());
    assert!(matches!(launcher.view().screen, Screen::Confirm { .. }));
    let details = launcher.view().details().to_vec();
    select_title(launcher, choice);
    block_on(launcher.activate_selected());
    assert!(
        matches!(launcher.view().status, Status::Result(_)),
        "{:?}",
        launcher.view().status
    );
    details
}

#[test]
fn uninstalling_stops_the_watch_and_deletes_the_history_if_asked() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    pane.clipboard.copy("one", None);
    pane.clipboard.copy("two", None);

    let details = uninstall(&launcher, "Uninstall and delete saved data");
    assert!(
        details.contains(&"Saved data: 2 clipboard history items".to_string()),
        "{details:?}"
    );
    assert!(!pane.clipboard.watching());
    assert!(pane.kept_on_disk().is_empty());
    assert!(launcher.retained_data().is_empty());
}

#[test]
fn uninstalling_and_keeping_the_history_keeps_it_for_a_reinstall() {
    let pane = Pane::new();
    let launcher = pane.installed();
    turn_on(&launcher);
    pane.clipboard.copy("kept", None);

    uninstall(&launcher, "Uninstall and keep saved data");
    assert!(!pane.clipboard.watching());
    assert_eq!(pane.kept_on_disk(), ["kept"]);
    assert_eq!(launcher.retained_data().len(), 1);
    launcher.back();
    select_title(&launcher, MANAGE_ROW);
    block_on(launcher.activate_selected());
    assert!(
        subtitle(&launcher, &format!("Delete retained data of {TITLE}"))
            .contains("keeps 1 clipboard history item"),
        "{:?}",
        launcher.view().rows
    );
    // Installed again from the same folder, it keeps history as it did.
    drop(launcher);
    let launcher = pane.installed();
    assert!(pane.clipboard.watching());
    assert_eq!(listed(&launcher), ["kept"]);
}

#[test]
fn where_the_clipboard_cannot_be_watched_history_stays_off_and_says_why() {
    let reason = "Not available: the test's clipboard cannot be watched";
    let pane = Pane::with(FakeClipboard {
        unavailable: Some(reason.into()),
        ..FakeClipboard::default()
    });
    let launcher = pane.installed();
    open(&launcher);
    assert!(subtitle(&launcher, TURN_ON).starts_with(&format!("{reason} · Off")));
    assert_eq!(
        run(&launcher, TURN_ON),
        Status::Error(format!("The extension reported an error: {reason}"))
    );
    assert_eq!(pane.clipboard.started(), 0);
    assert!(pane.history_file().is_none());
}

#[test]
fn without_a_clipboard_pane_keeps_nothing_and_says_so() {
    let pane = Pane::new();
    let launcher = Launcher::with_packages(
        Runtime::start(),
        vec![],
        pane.data.path().join("extensions"),
    );
    block_on(launcher.install_package(pane.folder()));
    open(&launcher);
    assert!(
        subtitle(&launcher, TURN_ON)
            .starts_with("Not available: this Pane does not watch the clipboard · Off"),
        "{:?}",
        launcher.view().rows
    );
    assert_eq!(
        run(&launcher, TURN_ON),
        Status::Error(
            "The extension reported an error: Not available: this Pane does not watch the \
             clipboard"
                .into()
        )
    );
}

#[test]
fn the_default_package_is_offered_only_on_windows_so_far() {
    let data = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    copy_package(source.path(), false);
    let launcher =
        Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"))
            .with_clipboard(pane_core::clipboard::none());
    block_on(launcher.install_package(source.path()));
    launcher.back();
    block_on(launcher.set_query("clipboard"));
    let row = launcher
        .view()
        .rows
        .into_iter()
        .find(|row| row.title == TITLE)
        .expect("the command is listed");
    if platforms::this_system() == pane_core::Platform::Windows {
        assert_eq!(row.unavailable, None);
    } else {
        assert_eq!(
            row.unavailable,
            Some(Unavailable::OnThisSystem(platforms::only(
                "this command",
                "Windows"
            )))
        );
    }
}
