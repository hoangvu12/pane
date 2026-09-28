//! Development mode through the launcher's public interface: a local
//! package whose source folder is watched is built after each save and, when
//! the build succeeds, reloaded, while Pane and other packages keep
//! running. A build that fails keeps the working code and shows its
//! diagnostics; a save during a build makes that build obsolete; ending
//! development stops the watcher and the build.
//!
//! The file watcher is the system's; the build is a stand-in that copies the
//! real guest named in the folder's `source.txt` (from `cargo xtask guests`)
//! to the package's component, so these tests need no compiler. The real
//! Rust and JavaScript builds are exercised in `develop_builds.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::develop::{Build, BuildStop, Builder};
use pane_core::{Launcher, PackageIdentity, Runtime, SavedData, Screen, Status};
use tempfile::TempDir;

const MANAGE_ROW: &str = "Manage extensions…";

fn guest(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests")
        .join(format!("{name}.wasm"));
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    path
}

/// The stand-in build's shared state: how often it ran, how often it was
/// stopped, and a gate it waits at while closed.
#[derive(Default)]
struct Probe {
    runs: AtomicUsize,
    stopped: AtomicUsize,
    /// Whether builds wait; `Condvar` wakes them when it opens.
    closed: Mutex<bool>,
    opened: Condvar,
}

impl Probe {
    fn close(&self) {
        *self.closed.lock().unwrap() = true;
    }

    fn open(&self) {
        *self.closed.lock().unwrap() = false;
        self.opened.notify_all();
    }

    fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }

    fn stopped(&self) -> usize {
        self.stopped.load(Ordering::SeqCst)
    }
}

/// Builds a folder holding `source.txt` by copying the guest it names to
/// `command.wasm`, or fails with its text when that starts with "error".
struct FakeBuilder(Arc<Probe>);

struct FakeBuild {
    folder: PathBuf,
    probe: Arc<Probe>,
}

impl Builder for FakeBuilder {
    fn build_for(&self, folder: &Path) -> Result<Arc<dyn Build>, String> {
        if !folder.join("source.txt").exists() {
            return Err("it has no source.txt".into());
        }
        Ok(Arc::new(FakeBuild {
            folder: folder.to_path_buf(),
            probe: self.0.clone(),
        }))
    }
}

impl Build for FakeBuild {
    fn command(&self) -> String {
        "fake build".into()
    }

    fn ignores(&self, path: &Path) -> bool {
        path == Path::new("command.wasm")
    }

    fn run(&self, stop: &BuildStop) -> Result<String, String> {
        // What was saved when the build started.
        let source = fs::read_to_string(self.folder.join("source.txt")).unwrap();
        self.probe.runs.fetch_add(1, Ordering::SeqCst);
        let mut closed = self.probe.closed.lock().unwrap();
        while *closed {
            if stop.is_stopped() {
                self.probe.stopped.fetch_add(1, Ordering::SeqCst);
                return Err("stopped".into());
            }
            closed = self
                .probe
                .opened
                .wait_timeout(closed, Duration::from_millis(20))
                .unwrap()
                .0;
        }
        drop(closed);
        let source = source.trim();
        if source.starts_with("error") {
            return Err(format!("   Compiling dev\n{source}\nfake build failed"));
        }
        fs::copy(guest(source), self.folder.join("command.wasm")).unwrap();
        Ok("built".into())
    }
}

/// Writes a package folder titled `title` whose source is the guest
/// `source`, built.
fn package(folder: &Path, title: &str, source: &str) -> PathBuf {
    fs::create_dir_all(folder).unwrap();
    fs::write(
        folder.join("pane.json"),
        format!(
            r#"{{
  "manifestVersion": 1,
  "title": "{title}",
  "apiVersion": "0.1",
  "commands": [{{ "id": "open", "title": "Open {title}", "component": "command.wasm" }}]
}}"#
        ),
    )
    .unwrap();
    fs::write(folder.join("source.txt"), source).unwrap();
    fs::copy(guest(source), folder.join("command.wasm")).unwrap();
    folder.to_path_buf()
}

/// Saves `source` in the package's source, as an author's editor would.
fn save(folder: &Path, source: &str) {
    fs::write(folder.join("source.txt"), source).unwrap();
}

struct Dev {
    _sources: TempDir,
    _data: TempDir,
    sources: PathBuf,
    launcher: Launcher,
    probe: Arc<Probe>,
}

impl Dev {
    fn new() -> Dev {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let probe = Arc::new(Probe::default());
        let (changes, _) = pane_core::develop::changes();
        let launcher = Launcher::with_packages(
            Ok(Runtime::start().unwrap()),
            vec![],
            data.path().join("extensions"),
        )
        .with_development(Arc::new(FakeBuilder(probe.clone())), changes);
        Dev {
            sources: sources.path().to_path_buf(),
            _sources: sources,
            _data: data,
            launcher,
            probe,
        }
    }

    /// Installs the package `title` built from the guest `source`.
    fn install(&self, title: &str, source: &str) -> (PathBuf, PackageIdentity) {
        let folder = package(&self.sources.join(title), title, source);
        block_on(self.launcher.install_package(&folder));
        assert!(
            matches!(self.launcher.view().status, Status::Result(_)),
            "{:?}",
            self.launcher.view().status
        );
        (folder.clone(), PackageIdentity::local(&folder).unwrap())
    }

    /// Installs `title` and develops it.
    fn developing(&self, title: &str, source: &str) -> (PathBuf, PackageIdentity) {
        let (folder, identity) = self.install(title, source);
        block_on(self.launcher.start_developing(&identity));
        assert!(
            self.launcher.development(&identity).is_some(),
            "{:?}",
            self.launcher.view().status
        );
        (folder, identity)
    }

    /// Waits until the package's builds have ended `handled` times.
    fn handled(&self, identity: &PackageIdentity, handled: u64) {
        wait_until(&format!("{handled} builds handled"), || {
            self.launcher
                .development(identity)
                .is_some_and(|development| development.handled >= handled)
        });
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn titles(launcher: &Launcher) -> Vec<String> {
    launcher
        .view()
        .rows
        .into_iter()
        .map(|row| row.title)
        .collect()
}

fn select_title(launcher: &Launcher, title: &str) {
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    launcher.select(index);
}

fn to_root(launcher: &Launcher) {
    for _ in 0..3 {
        launcher.back();
    }
}

/// From root search, opens the command titled `command` and runs its item
/// titled `item`, returning the outcome.
fn run(launcher: &Launcher, command: &str, item: &str) -> Status {
    to_root(launcher);
    select_title(launcher, command);
    block_on(launcher.activate_selected());
    select_title(launcher, item);
    block_on(launcher.activate_selected());
    launcher.view().status
}

fn manage(launcher: &Launcher) {
    to_root(launcher);
    select_title(launcher, MANAGE_ROW);
    block_on(launcher.activate_selected());
    assert!(matches!(launcher.view().screen, Screen::Extensions { .. }));
}

/// Activates the extension manager's row titled `row`, returning the
/// outcome.
fn press(launcher: &Launcher, row: &str) -> Status {
    manage(launcher);
    select_title(launcher, row);
    block_on(launcher.activate_selected());
    launcher.view().status
}

fn error(status: Status) -> String {
    match status {
        Status::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

const RUST: &str = "Hello from the Rust guest";
const JAVASCRIPT: &str = "Hello from the JavaScript guest";
const TYPESCRIPT: &str = "Hello from the TypeScript guest";

#[test]
fn saving_builds_and_reloads_only_that_package() {
    let dev = Dev::new();
    dev.install("Other", "sample_js");
    let (folder, identity) = dev.developing("Dev", "sample_rust");
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(RUST.into())
    );

    save(&folder, "sample_ts");
    dev.handled(&identity, 1);
    assert_eq!(
        dev.launcher.view().status,
        Status::Result("Reloaded Dev".into())
    );
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(TYPESCRIPT.into())
    );
    assert_eq!(
        run(&dev.launcher, "Open Other", "Say hello"),
        Status::Result(JAVASCRIPT.into())
    );

    // Each save builds again.
    save(&folder, "sample_js");
    dev.handled(&identity, 2);
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(JAVASCRIPT.into())
    );
    assert_eq!(dev.probe.runs(), 2);
}

#[test]
fn a_build_that_fails_keeps_the_working_code_and_shows_its_diagnostics() {
    let dev = Dev::new();
    let (folder, identity) = dev.developing("Dev", "sample_rust");

    save(&folder, "error[E0308]: mismatched types");
    dev.handled(&identity, 1);
    let message = error(dev.launcher.view().status);
    assert_eq!(
        message,
        "Dev did not build: error[E0308]: mismatched types. It keeps running its installed \
         code; the diagnostics are under \"Why Dev did not build\" in Manage extensions."
    );
    assert_eq!(
        dev.launcher
            .development(&identity)
            .unwrap()
            .failure
            .as_deref(),
        Some("   Compiling dev\nerror[E0308]: mismatched types\nfake build failed")
    );
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(RUST.into())
    );

    // The diagnostics, and a row that builds again.
    press(&dev.launcher, "Why Dev did not build");
    let view = dev.launcher.view();
    assert!(matches!(view.screen, Screen::BuildDetails { .. }));
    assert_eq!(view.title, "Why Dev did not build");
    let details = view.details().to_vec();
    assert!(
        details.contains(&"Build command: fake build".to_string()),
        "{details:?}"
    );
    assert!(
        details.contains(&"error[E0308]: mismatched types".to_string()),
        "{details:?}"
    );
    assert_eq!(titles(&dev.launcher), ["Build Dev again"]);
    block_on(dev.launcher.activate_selected());
    dev.handled(&identity, 2);
    assert_eq!(dev.probe.runs(), 2);

    // Fixing it reloads, and the failure is gone.
    save(&folder, "sample_ts");
    dev.handled(&identity, 3);
    assert_eq!(dev.launcher.development(&identity).unwrap().failure, None);
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(TYPESCRIPT.into())
    );
    manage(&dev.launcher);
    assert!(!titles(&dev.launcher).contains(&"Why Dev did not build".to_string()));
}

#[test]
fn a_build_that_fails_to_start_is_paused_with_retry_and_not_rolled_back() {
    let dev = Dev::new();
    let (folder, identity) = dev.developing("Dev", "sample_rust");

    save(&folder, "failing_start");
    dev.handled(&identity, 1);
    let message = error(dev.launcher.view().status);
    assert!(
        message
            .starts_with("Reloaded Dev, but it failed to start; its earlier code is not restored."),
        "{message}"
    );
    manage(&dev.launcher);
    assert!(titles(&dev.launcher).contains(&"Retry starting Dev".to_string()));

    // A fixed save reloads it, which ends the pause.
    save(&folder, "sample_rust");
    dev.handled(&identity, 2);
    assert_eq!(
        dev.launcher.view().status,
        Status::Result("Reloaded Dev".into())
    );
    assert!(!titles(&dev.launcher).iter().any(|t| t.starts_with("Retry")));
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(RUST.into())
    );
}

#[test]
fn a_save_during_a_build_makes_that_build_obsolete() {
    let dev = Dev::new();
    let (folder, identity) = dev.developing("Dev", "sample_rust");
    dev.probe.close();

    save(&folder, "sample_js");
    wait_until("the first build", || dev.probe.runs() == 1);
    assert!(dev.launcher.development(&identity).unwrap().building);
    save(&folder, "sample_ts");
    // Let the first build finish: it built the older save.
    std::thread::sleep(Duration::from_millis(300));
    dev.probe.open();

    dev.handled(&identity, 1);
    let development = dev.launcher.development(&identity).unwrap();
    assert_eq!((development.obsolete, development.handled), (1, 1));
    assert_eq!(dev.probe.runs(), 2);
    // Only the newer build was reloaded.
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(TYPESCRIPT.into())
    );
    std::thread::sleep(Duration::from_millis(500));
    let development = dev.launcher.development(&identity).unwrap();
    assert_eq!((development.obsolete, development.handled), (1, 1));
}

#[test]
fn stopping_development_stops_its_build_and_its_watcher() {
    let dev = Dev::new();
    let (folder, identity) = dev.developing("Dev", "sample_rust");
    dev.probe.close();
    save(&folder, "sample_js");
    wait_until("the build", || dev.probe.runs() == 1);

    assert_eq!(
        press(&dev.launcher, "Stop developing Dev"),
        Status::Result("Stopped developing Dev".into())
    );
    wait_until("the build to stop", || dev.probe.stopped() == 1);
    assert!(dev.launcher.development(&identity).is_none());
    assert!(titles(&dev.launcher).contains(&"Develop Dev".to_string()));

    // Nothing watches the folder any more.
    dev.probe.open();
    save(&folder, "sample_ts");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(dev.probe.runs(), 1);
    assert_eq!(
        run(&dev.launcher, "Open Dev", "Say hello"),
        Status::Result(RUST.into())
    );
}

#[test]
fn disabling_or_uninstalling_a_package_ends_its_development() {
    let dev = Dev::new();
    let (folder, identity) = dev.developing("Dev", "sample_rust");
    dev.probe.close();
    save(&folder, "sample_js");
    wait_until("the build", || dev.probe.runs() == 1);

    block_on(dev.launcher.set_enabled(&identity, false));
    wait_until("the build to stop", || dev.probe.stopped() == 1);
    assert!(dev.launcher.development(&identity).is_none());
    // Enabling it again does not develop it again.
    block_on(dev.launcher.set_enabled(&identity, true));
    assert!(dev.launcher.development(&identity).is_none());
    dev.probe.open();

    block_on(dev.launcher.start_developing(&identity));
    assert!(dev.launcher.development(&identity).is_some());
    block_on(dev.launcher.uninstall(&identity, SavedData::Keep));
    assert!(dev.launcher.development(&identity).is_none());
    save(&folder, "sample_ts");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(dev.probe.runs(), 1);
}

#[test]
fn dropping_the_launcher_stops_development() {
    let dev = Dev::new();
    let (folder, _) = dev.developing("Dev", "sample_rust");
    dev.probe.close();
    save(&folder, "sample_js");
    wait_until("the build", || dev.probe.runs() == 1);

    let Dev {
        launcher,
        probe,
        _sources,
        _data,
        ..
    } = dev;
    drop(launcher);
    wait_until("the build to stop", || probe.stopped() == 1);
}

#[test]
fn a_published_copy_keeps_its_own_identity_and_code() {
    let dev = Dev::new();
    // The published copy: built components only, as a package is shipped.
    let published = dev.sources.join("published");
    package(&published, "Dev", "sample_rust");
    fs::remove_file(published.join("source.txt")).unwrap();
    block_on(dev.launcher.install_package(&published));
    let published = PackageIdentity::local(&published).unwrap();
    let (folder, identity) = dev.developing("Dev", "sample_rust");
    assert_ne!(published, identity);

    save(&folder, "sample_js");
    dev.handled(&identity, 1);
    // Both are titled Dev: the first command is the published copy's.
    to_root(&dev.launcher);
    let answers: Vec<Status> = (0..2)
        .map(|index| {
            to_root(&dev.launcher);
            dev.launcher.select(index);
            block_on(dev.launcher.activate_selected());
            select_title(&dev.launcher, "Say hello");
            block_on(dev.launcher.activate_selected());
            dev.launcher.view().status
        })
        .collect();
    assert_eq!(
        answers,
        [
            Status::Result(RUST.into()),
            Status::Result(JAVASCRIPT.into())
        ]
    );
    assert!(dev.launcher.development(&published).is_none());

    // The published copy cannot be developed: it has no source to build.
    block_on(dev.launcher.start_developing(&published));
    assert_eq!(
        dev.launcher.view().status,
        Status::Error("Cannot develop Dev: it has no source.txt".into())
    );
    assert!(dev.launcher.development(&published).is_none());
}

#[test]
fn development_is_started_and_stopped_in_manage_extensions() {
    let dev = Dev::new();
    let (folder, identity) = dev.install("Dev", "sample_rust");
    let status = press(&dev.launcher, "Develop Dev");
    assert_eq!(
        status,
        Status::Result(format!(
            "Developing Dev: each save in {} runs `fake build`, then reloads it",
            folder.display()
        ))
    );
    let view = dev.launcher.view();
    assert!(
        view.rows[0]
            .subtitle
            .as_deref()
            .is_some_and(|s| s.starts_with("Enabled · Developing · ")),
        "{:?}",
        view.rows[0]
    );
    assert!(titles(&dev.launcher).contains(&"Stop developing Dev".to_string()));
    let development = dev.launcher.development(&identity).unwrap();
    assert_eq!(development.folder, folder.canonicalize().unwrap());
    assert_eq!(development.command, "fake build");

    // Developing it again does nothing more.
    block_on(dev.launcher.start_developing(&identity));
    save(&folder, "sample_js");
    dev.handled(&identity, 1);
    assert_eq!(dev.probe.runs(), 1);

    press(&dev.launcher, "Stop developing Dev");
    assert!(titles(&dev.launcher).contains(&"Develop Dev".to_string()));
}

#[test]
fn the_window_is_told_of_each_change() {
    let sources = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let probe = Arc::new(Probe::default());
    let (sender, mut changes) = pane_core::develop::changes();
    let launcher = Launcher::with_packages(
        Ok(Runtime::start().unwrap()),
        vec![],
        data.path().join("extensions"),
    )
    .with_development(Arc::new(FakeBuilder(probe)), sender);
    let folder = package(&sources.path().join("Dev"), "Dev", "sample_rust");
    block_on(launcher.install_package(&folder));
    let identity = PackageIdentity::local(&folder).unwrap();
    block_on(launcher.start_developing(&identity));

    save(&folder, "sample_js");
    // Building, then reloaded.
    block_on(changes.next()).unwrap();
    wait_until("the reload", || {
        launcher.view().status == Status::Result("Reloaded Dev".into())
    });
    block_on(changes.next()).unwrap();
}
