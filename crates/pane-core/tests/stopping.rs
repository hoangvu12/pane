//! Calls still pending when their package's code stops, through the
//! launcher's public interface: disabling, reloading or updating a package
//! stops its pending calls at once, and their late results never reach the
//! screen or the package's data. Every check runs against the settings
//! sample's "Save after waiting" in Rust, JavaScript and TypeScript, real
//! guests from `cargo xtask guests`: it saves "started", waits ten seconds,
//! then saves "finished".

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::{Launcher, PackageIdentity, Runtime, Screen, Status};
use tempfile::TempDir;

/// Well under the ten seconds "Save after waiting" waits: a call that is
/// stopped ends within it, one that is not takes longer.
const STOPPED_WITHIN: Duration = Duration::from_secs(6);

/// A settings sample package: the same command in each language.
struct Fixture {
    /// The assembled package under `target/guests/packages`.
    package: &'static str,
    component: &'static str,
    title: &'static str,
}

const RUST: Fixture = Fixture {
    package: "sample-settings",
    component: "sample_settings.wasm",
    title: "Settings sample",
};
const JAVASCRIPT: Fixture = Fixture {
    package: "sample-settings-js",
    component: "sample_settings_js.wasm",
    title: "JavaScript settings sample",
};
const TYPESCRIPT: Fixture = Fixture {
    package: "sample-settings-ts",
    component: "sample_settings_ts.wasm",
    title: "TypeScript settings sample",
};

/// Copies the assembled settings sample package of `fixture` into `folder`,
/// titled "Settings sample" whatever its language.
fn settings_package(fixture: &Fixture, folder: &Path) -> PathBuf {
    let assembled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests/packages")
        .join(fixture.package);
    assert!(
        assembled.exists(),
        "{} is missing; run `cargo xtask guests`",
        assembled.display()
    );
    fs::create_dir_all(folder).unwrap();
    let manifest = fs::read_to_string(assembled.join("pane.json")).unwrap();
    let original = format!("\"{}\"", fixture.title);
    assert!(manifest.contains(&original), "{manifest}");
    let manifest = manifest.replace(&original, "\"Settings sample\"");
    fs::write(folder.join("pane.json"), manifest).unwrap();
    fs::copy(
        assembled.join(fixture.component),
        folder.join(fixture.component),
    )
    .unwrap();
    folder.to_path_buf()
}

/// A launcher with the settings sample of one language installed.
struct Installed {
    _sources: TempDir,
    data: TempDir,
    runtime: Runtime,
    launcher: Launcher,
    folder: PathBuf,
    identity: PackageIdentity,
}

impl Installed {
    fn new(fixture: &Fixture) -> Installed {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let runtime = Runtime::start().unwrap();
        let launcher = Launcher::with_packages(
            Ok(runtime.clone()),
            vec![],
            data.path().join("extensions"),
        );
        let folder = settings_package(fixture, &sources.path().join("settings"));
        block_on(launcher.install_package(&folder));
        let identity = PackageIdentity::local(&folder).unwrap();
        Installed {
            _sources: sources,
            data,
            runtime,
            launcher,
            folder,
            identity,
        }
    }

    /// What "Save after waiting" has saved: "started", "finished" or
    /// nothing.
    fn slow_save(&self) -> Option<String> {
        let path = self.data.path().join("extensions/settings.json");
        let text = fs::read_to_string(path).unwrap_or_default();
        ["finished", "started"]
            .into_iter()
            .find(|progress| text.contains(&format!("\"slow-save\": \"{progress}\"")))
            .map(str::to_owned)
    }

    /// Opens the Greeting command and runs "Save after waiting" on another
    /// thread, returning once the guest has saved "started": its call is
    /// then waiting inside the guest.
    fn start_slow_save(&self) -> Pending {
        open_greeting_at(&self.launcher, "Save after waiting");
        let saving = self.launcher.activate_selected();
        let started = Instant::now();
        let thread = thread::spawn(move || block_on(saving));
        while self.slow_save().as_deref() != Some("started") {
            assert!(
                started.elapsed() < STOPPED_WITHIN,
                "the call did not start: {:?}",
                self.launcher.view().status
            );
            thread::sleep(Duration::from_millis(10));
        }
        Pending { thread, started }
    }
}

/// A call waiting inside the guest.
struct Pending {
    thread: thread::JoinHandle<()>,
    started: Instant,
}

impl Pending {
    /// Waits for the call to end, asserting it was stopped rather than
    /// finishing its wait.
    fn assert_stopped(self) {
        self.thread.join().unwrap();
        let took = self.started.elapsed();
        assert!(took < STOPPED_WITHIN, "the call ran for {took:?}");
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

/// Opens the installed Greeting command from root search and selects its
/// item titled `item`.
fn open_greeting_at(launcher: &Launcher, item: &str) {
    launcher.back();
    launcher.back();
    select_title(launcher, "Greeting");
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Command);
    select_title(launcher, item);
}

fn disabling_stops_a_pending_call_and_discards_its_result(fixture: &Fixture) {
    let installed = Installed::new(fixture);
    let pending = installed.start_slow_save();

    block_on(installed.launcher.set_enabled(&installed.identity, false));
    pending.assert_stopped();

    let view = installed.launcher.view();
    assert!(
        matches!(view.screen, Screen::Root { .. }),
        "{:?}",
        view.screen
    );
    assert_eq!(
        view.status,
        Status::Result("Disabled Settings sample".into())
    );
    assert_eq!(installed.slow_save().as_deref(), Some("started"));
    assert_eq!(block_on(installed.runtime.running()), Vec::<PathBuf>::new());
}

fn reloading_stops_a_pending_call_and_the_new_code_runs(fixture: &Fixture) {
    let installed = Installed::new(fixture);
    let pending = installed.start_slow_save();

    block_on(installed.launcher.reload(&installed.identity));
    pending.assert_stopped();

    // The command of the old code closed; its answer never shows, not even
    // over the new code's screens.
    let view = installed.launcher.view();
    assert!(
        matches!(view.screen, Screen::Root { .. }),
        "{:?}",
        view.screen
    );
    assert_eq!(view.status, Status::Result("Reloaded Settings sample".into()));
    assert_eq!(installed.slow_save().as_deref(), Some("started"));
    open_greeting_at(&installed.launcher, "Use a casual greeting");
    block_on(installed.launcher.activate_selected());
    assert_eq!(
        installed.launcher.view().status,
        Status::Result("Saved the casual greeting".into())
    );
    assert_eq!(installed.slow_save().as_deref(), Some("started"));
}

/// Declares one test per check for each language's settings sample.
macro_rules! contract {
    ($($check:ident),* $(,)?) => {
        mod rust {
            $(#[test] fn $check() { super::$check(&super::RUST) })*
        }
        mod javascript {
            $(#[test] fn $check() { super::$check(&super::JAVASCRIPT) })*
        }
        mod typescript {
            $(#[test] fn $check() { super::$check(&super::TYPESCRIPT) })*
        }
    };
}

contract!(
    disabling_stops_a_pending_call_and_discards_its_result,
    reloading_stops_a_pending_call_and_the_new_code_runs,
);
