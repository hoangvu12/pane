//! Automatic updates of installed npm packages through the launcher's
//! public interface, from a local registry on 127.0.0.1 that each test
//! fills (`support/npm_registry.rs`): nothing here reaches the network or
//! the real npm registry. The package is the assembled JavaScript
//! settings sample (`target/guests/packages/sample-settings-js`) packed
//! as an npm package: its Greeting command saves settings and has "Save
//! after waiting", which waits ten seconds, so that a command still
//! running can stand in the update's way.
//!
//! What is checked: a newer version updates the package by itself once
//! no command of it runs, keeping its identity, its settings and its
//! disabled state, ending the old code's generation; a command that is
//! running finishes first, the update waiting until the screen the user
//! is on closes; a pinned, disabled, or turned-off package is never
//! replaced; the global and per-extension controls work through Manage
//! extensions; an incompatible version, a dependency that cannot be
//! installed and an unreachable registry explain and leave the installed
//! copy alone; and the check repeats on its cadence, and at Pane's start.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::clipboard::{Clock as _, ManualClock, SystemClock};
use pane_core::npm::Registry as NpmRegistry;
use pane_core::{Launcher, PackageIdentity, Runtime, Screen, Status};
use tempfile::TempDir;

#[path = "support/npm_registry.rs"]
mod npm_registry;
#[path = "support/unreachable.rs"]
mod unreachable;

use npm_registry::{Registry, pack};

/// The npm name of the package every test here installs.
const NAME: &str = "@pane-tests/settings";

fn guests() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/guests")
}

fn guest(file: &str) -> PathBuf {
    let path = guests().join(file);
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    path
}

/// The tarball of the settings sample at `version`, with the API version
/// `api` and the dependency declarations `dependencies` (JSON, already
/// quoted) in its `pane.json`.
fn settings_files(version: &str, api: &str, dependencies: &str) -> Vec<(&'static str, Vec<u8>)> {
    let manifest = format!(
        r#"{{ "manifestVersion": 1, "title": "Settings from npm", "version": "{version}",
             "apiVersion": "{api}",
             "commands": [{{ "id": "greeting", "title": "Greeting",
                             "component": "sample_settings_js.wasm" }}]{dependencies} }}"#
    );
    let package = format!(r#"{{ "name": "{NAME}", "version": "{version}" }}"#);
    vec![
        ("package.json", package.into_bytes()),
        ("pane.json", manifest.into_bytes()),
        (
            "sample_settings_js.wasm",
            fs::read(guest("packages/sample-settings-js/sample_settings_js.wasm")).unwrap(),
        ),
    ]
}

struct Dirs {
    data: TempDir,
    runtime: Runtime,
    registry: Registry,
    clock: Arc<ManualClock>,
}

impl Dirs {
    fn new() -> Dirs {
        let clock = ManualClock::at(SystemClock.now());
        Dirs {
            data: tempfile::tempdir().unwrap(),
            runtime: Runtime::start().unwrap(),
            registry: Registry::start(),
            clock: clock.clone(),
        }
    }

    fn packages_dir(&self) -> PathBuf {
        self.data.path().join("extensions")
    }

    /// A launcher on this data folder downloading from the local registry,
    /// whose checks the test's clock moves along: advancing it past a
    /// second brings the first check, and past a day the next one.
    fn launcher(&self) -> Launcher {
        Launcher::with_packages(Ok(self.runtime.clone()), vec![], self.packages_dir())
            .with_npm_registry(NpmRegistry::local(self.registry.url()).unwrap())
            .with_clock(self.clock.clone())
    }

    /// A launcher on the system's clock, whose first check comes by
    /// itself about a second after it starts, as Pane's does at its start.
    fn launcher_by_the_system_clock(&self) -> Launcher {
        Launcher::with_packages(Ok(self.runtime.clone()), vec![], self.packages_dir())
            .with_npm_registry(NpmRegistry::local(self.registry.url()).unwrap())
    }

    /// Publishes the sample at `version`, tagged latest, asking for the
    /// API version `api` (the version this Pane provides is 0.1).
    fn publish(&self, version: &str, api: &str) {
        self.publish_with(version, api, "");
    }

    /// Publishes the sample at `version` with the dependency declarations
    /// `dependencies` (JSON, already quoted) in its `pane.json`.
    fn publish_with(&self, version: &str, api: &str, dependencies: &str) {
        self.registry.publish(
            NAME,
            version,
            pack(&settings_files(version, api, dependencies)),
        );
    }

    /// Installs the sample at `version`, unpinned.
    fn install(&self, launcher: &Launcher, version: &str) {
        self.publish(version, "0.1");
        block_on(launcher.install_npm(NAME));
        assert_eq!(
            launcher.view().status,
            Status::Result("Installed Settings from npm".into())
        );
    }

    /// The record of the sample in `installed.json`.
    fn record(&self) -> serde_json::Value {
        let text = fs::read_to_string(self.packages_dir().join("installed.json")).unwrap();
        let registry: serde_json::Value = serde_json::from_str(&text).unwrap();
        registry["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["npm"] == NAME)
            .cloned()
            .unwrap_or_else(|| panic!("no record of {NAME} in {registry:#}"))
    }

    /// The version the sample is installed at, from its record.
    fn installed_version(&self) -> String {
        self.record()["npmVersion"].as_str().unwrap().to_owned()
    }

    /// What the Greeting command saved in its settings, as the whole
    /// settings file's text.
    fn settings(&self) -> String {
        fs::read_to_string(self.packages_dir().join("settings.json")).unwrap_or_default()
    }

    /// What "Save after waiting" has saved: "started", "finished" or
    /// nothing.
    fn slow_save(&self) -> Option<&str> {
        ["finished", "started"].into_iter().find(|progress| {
            self.settings()
                .contains(&format!("\"slow-save\": \"{progress}\""))
        })
    }

    /// Asks the launcher for a check: the clock moves past when the next
    /// one is due. Waits until the check completed and whatever it staged
    /// was applied or deferred.
    fn check(&self, launcher: &Launcher) {
        self.clock.advance(Duration::from_secs(2));
        assert!(
            launcher.wait_for_updates(Duration::from_secs(30)),
            "the updater did not settle"
        );
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

fn activate(launcher: &Launcher, title: &str) {
    select_title(launcher, title);
    block_on(launcher.activate_selected());
}

/// Runs the item titled `item` of the Greeting command, returning the
/// status; the command's screen stays open, as it does for a user.
fn run(launcher: &Launcher, item: &str) -> Status {
    open_greeting(launcher);
    activate(launcher, item);
    launcher.view().status
}

/// Opens the Greeting command from root search.
fn open_greeting(launcher: &Launcher) {
    to_root(launcher);
    activate(launcher, "Greeting");
    assert_eq!(launcher.view().screen, Screen::Command);
}

fn to_root(launcher: &Launcher) {
    while !matches!(launcher.view().screen, Screen::Root { .. }) {
        launcher.back();
    }
}

/// Opens Manage extensions from root search.
fn manage(launcher: &Launcher) {
    to_root(launcher);
    activate(launcher, "Manage extensions…");
    assert!(matches!(launcher.view().screen, Screen::Extensions { .. }));
}

fn error_of(launcher: &Launcher) -> String {
    match launcher.view().status {
        Status::Error(text) => text,
        other => panic!("not an error: {other:?}"),
    }
}

/// Waits until `what` holds, asserting it did within `limit`; `what`'s
/// name says what it was in the panic.
fn wait_until(what: &str, limit: Duration, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !check() {
        assert!(
            Instant::now() < deadline,
            "{what} did not happen within {limit:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// The component of the Greeting command of the installed copy at
/// `version`, by the folder it is in.
fn component_of(launcher: &Launcher) -> PathBuf {
    launcher
        .packages()
        .into_iter()
        .find(|package| package.identity == PackageIdentity::npm(NAME))
        .expect("installed")
        .commands()[0]
        .component
        .clone()
}

#[test]
fn a_newer_version_updates_the_package_by_itself_keeping_its_data() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    // A setting saved through the old code: an update keeps it.
    assert_eq!(
        run(&launcher, "Use a casual greeting"),
        Status::Result("Saved the casual greeting".into())
    );
    assert!(dirs.settings().contains("casual"));
    // Leave the command: its instance stays alive for its generation, and
    // the update ends that.
    to_root(&launcher);
    let old = component_of(&launcher);
    assert!(block_on(dirs.runtime.running()).contains(&old));

    dirs.publish("0.2.0", "0.1");
    dirs.check(&launcher);

    assert_eq!(
        launcher.view().status,
        Status::Result("Updated Settings from npm to 0.2.0".into())
    );
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(dirs.record().get("pinned"), None);
    // The old code's generation ended: its instance is gone.
    assert!(!block_on(dirs.runtime.running()).contains(&old));
    // The new code runs, and the saved setting is still there.
    assert_eq!(
        run(&launcher, "Use a casual greeting"),
        Status::Result("Saved the casual greeting".into())
    );
    assert!(dirs.settings().contains("casual"));
}

#[test]
fn a_command_that_is_running_finishes_before_the_update_replaces_it() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    // "Save after waiting", run on another thread: its call is pending,
    // with the command's screen open.
    open_greeting(&launcher);
    select_title(&launcher, "Save after waiting");
    let saving = launcher.activate_selected();
    let thread = thread::spawn(move || {
        block_on(saving);
    });
    wait_until("the slow save started", Duration::from_secs(10), || {
        dirs.slow_save() == Some("started")
    });

    dirs.publish("0.2.0", "0.1");
    dirs.check(&launcher);

    // The update is staged and deferred: the installed copy is unchanged,
    // the user was not interrupted and the command is still running.
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_ne!(
        launcher.view().status,
        Status::Result("Updated Settings from npm to 0.2.0".into())
    );
    assert_eq!(dirs.slow_save(), Some("started"));

    // The running command finishes: nothing replaced it mid-command.
    thread.join().unwrap();
    assert_eq!(dirs.slow_save(), Some("finished"));
    // The screen the answer is on is still the user's: the update keeps
    // waiting while it is open.
    assert_eq!(dirs.installed_version(), "0.1.0");

    // Leaving the command is the boundary: the update applies, and says
    // so where the user is.
    to_root(&launcher);
    wait_until(
        "the deferred update applied",
        Duration::from_secs(30),
        || launcher.view().status == Status::Result("Updated Settings from npm to 0.2.0".into()),
    );
    assert_eq!(dirs.installed_version(), "0.2.0");
    // What the command saved is kept.
    assert!(dirs.settings().contains("finished"));
}

#[test]
fn a_pinned_package_is_never_updated_automatically() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.publish("0.1.0", "0.1");
    block_on(launcher.install_npm(&format!("{NAME}@0.1.0")));
    assert_eq!(dirs.record()["pinned"], serde_json::json!(true));
    let asked = dirs.registry.requests().len();

    dirs.publish("0.2.0", "0.1");
    dirs.check(&launcher);

    // Not even asked about: a pinned version is not a candidate. The
    // status line still says what happened before, not that anything was
    // updated.
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(dirs.registry.requests().len(), asked);
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Settings from npm".into())
    );
}

#[test]
fn an_opted_out_package_is_not_updated_until_the_user_turns_updates_back_on() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    // The row in Manage extensions, and what it says before and after.
    manage(&launcher);
    activate(&launcher, "Update Settings from npm automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of Settings from npm are off".into())
    );
    assert!(titles(&launcher).contains(&"Update Settings from npm automatically".to_owned()));

    dirs.publish("0.2.0", "0.1");
    let asked = dirs.registry.requests().len();
    dirs.check(&launcher);
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(dirs.registry.requests().len(), asked);

    // Turning it back on checks at once, and the update applies.
    activate(&launcher, "Update Settings from npm automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of Settings from npm are on".into())
    );
    wait_until("the update applied", Duration::from_secs(30), || {
        dirs.installed_version() == "0.2.0"
    });
}

#[test]
fn turning_updates_off_everywhere_stops_them_all() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    manage(&launcher);
    activate(&launcher, "Update extensions automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of extensions are off".into())
    );

    dirs.publish("0.2.0", "0.1");
    let asked = dirs.registry.requests().len();
    dirs.check(&launcher);
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(dirs.registry.requests().len(), asked);

    // Back on: the update applies.
    activate(&launcher, "Update extensions automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of extensions are on".into())
    );
    wait_until("the update applied", Duration::from_secs(30), || {
        dirs.installed_version() == "0.2.0"
    });
}

#[test]
fn an_incompatible_new_version_is_refused_with_its_explanation() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    // The new version needs an API this Pane does not provide.
    dirs.publish("0.2.0", "0.2");
    dirs.check(&launcher);

    let error = error_of(&launcher);
    assert!(
        error.starts_with(
            "Settings from npm was not updated: Incompatible package: it needs Pane extension \
             API 0.2, but this Pane provides 0.1."
        ),
        "{error}"
    );
    assert!(
        error.contains("It keeps running its installed code."),
        "{error}"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");
    // The installed copy still runs.
    assert_eq!(
        run(&launcher, "Use a casual greeting"),
        Status::Result("Saved the casual greeting".into())
    );
}

#[test]
fn a_new_version_whose_dependency_cannot_be_installed_is_refused() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    dirs.publish_with(
        "0.2.0",
        "0.1",
        r#", "dependencies": [{ "id": "nobody", "source": "npm:nobody",
                                  "operations": [{ "id": "nothing", "version": 1 }] }]"#,
    );
    dirs.check(&launcher);

    let error = error_of(&launcher);
    assert!(
        error.starts_with("Settings from npm was not updated to 0.2.0: "),
        "{error}"
    );
    assert!(
        error.contains("npm package nobody was not found in the registry"),
        "{error}"
    );
    assert!(
        error.contains("It keeps running its installed code."),
        "{error}"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");
}

#[test]
fn an_unreachable_registry_is_explained_and_leaves_the_installed_copy_alone() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    drop(launcher);

    // A registry that refuses connections, as one that is down does.
    let closed = unreachable::ClosedPort::new();
    let url = format!("{}/", closed.url());
    let launcher = Launcher::with_packages(Ok(dirs.runtime.clone()), vec![], dirs.packages_dir())
        .with_npm_registry(NpmRegistry::local(&url).unwrap())
        .with_clock(dirs.clock.clone());
    dirs.check(&launcher);

    let error = error_of(&launcher);
    assert!(
        error.starts_with(&format!(
            "Settings from npm was not checked for a newer version: Could not reach the npm \
             registry {url} for {NAME}:"
        )),
        "{error}"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");
    // The installed copy still runs.
    assert_eq!(
        run(&launcher, "Use a casual greeting"),
        Status::Result("Saved the casual greeting".into())
    );
}

#[test]
fn the_check_repeats_on_its_cadence() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    // The first check: up to date, nothing happens. The status line still
    // says what happened before, not that anything was updated.
    dirs.check(&launcher);
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Settings from npm".into())
    );

    // A newer version is published; a day passes on the clock, and the
    // next check finds it.
    dirs.publish("0.2.0", "0.1");
    dirs.clock.advance(Duration::from_secs(24 * 3600 + 2));
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );

    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(
        launcher.view().status,
        Status::Result("Updated Settings from npm to 0.2.0".into())
    );
}

#[test]
fn a_disabled_package_is_not_updated() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    block_on(launcher.set_enabled(&PackageIdentity::npm(NAME), false));

    dirs.publish("0.2.0", "0.1");
    let asked = dirs.registry.requests().len();
    dirs.check(&launcher);

    // The user switched it off: Pane does not touch its code.
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(dirs.registry.requests().len(), asked);
}

#[test]
fn after_a_restart_the_first_check_updates_the_package() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    drop(launcher);

    dirs.publish("0.2.0", "0.1");
    // A new Pane, by the system's clock: its first check comes about a
    // second after it starts, by itself.
    let launcher = dirs.launcher_by_the_system_clock();
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );

    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(
        launcher.view().status,
        Status::Result("Updated Settings from npm to 0.2.0".into())
    );
}
