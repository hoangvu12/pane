//! Automatic updates of installed npm and Git packages through the
//! launcher's public interface: the npm ones from a local registry on
//! 127.0.0.1 that each test fills (`support/npm_registry.rs`), the Git
//! ones from a repository each test makes with the `git` program and
//! serves over Git's smart HTTP protocol from 127.0.0.1
//! (`support/repo_server.rs`) — nothing here reaches the network, the
//! real npm registry or a real Git host. The npm package is the assembled
//! JavaScript settings sample
//! (`target/guests/packages/sample-settings-js`) packed as an npm
//! package: its Greeting command saves settings and has "Save after
//! waiting", which waits ten seconds, so that a command still running can
//! stand in the update's way. The Git package is the assembled Git sample
//! (`target/guests/git/greeter`), whose tracked `release` branch moves to
//! a newer commit.
//!
//! What is checked: a newer version updates the npm package by itself
//! once no command of it runs, keeping its identity, its settings and its
//! disabled state, ending the old code's generation, and a tracked
//! branch that has moved updates the Git package the same way, keeping
//! its identity and its tracked reference, while a pinned revision never
//! moves; a command that is running finishes first, the update waiting
//! until the screen the user is on closes; a pinned, disabled, or
//! turned-off package is never replaced, and neither is an installed
//! local folder's copy; the global and per-extension controls work
//! through Manage extensions, a Git package's row among them; an
//! incompatible version, a dependency that cannot be installed, an
//! unreachable registry and a tracked branch that has moved to a
//! source-only revision explain and leave the installed copy alone; an
//! action or an opening asked in the moment the replacement is being
//! applied is refused rather than started and stopped by it; a new
//! version that fails to start is not rolled back; and the check repeats
//! on its cadence, and at Pane's start.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::clipboard::{Clock as _, ManualClock, SystemClock};
use pane_core::npm::Registry as NpmRegistry;
use pane_core::{
    Launcher, OperationKind, PackageIdentity, Runtime, Screen, SettingsTarget, Status, ToastStyle,
    UpdateResultsAction, WindowPresence,
};
use tempfile::TempDir;

#[path = "support/feedback.rs"]
mod feedback;
#[path = "support/npm_registry.rs"]
mod npm_registry;
#[path = "support/repo_server.rs"]
mod repo_server;
#[path = "support/unreachable.rs"]
mod unreachable;

use feedback::shown;
use npm_registry::{Registry, pack};
use repo_server::{Repo, Server, greeter_files};

#[path = "support/guests.rs"]
mod guests;
#[path = "support/rows.rs"]
mod rows;

use guests::{guest_file as guest, guests};
use rows::{select_title, titles};

/// The npm name of the package every test here installs.
const NAME: &str = "@pane-tests/settings";

/// The tarball of the settings sample at `version`, with the API version
/// `api` and the dependency declarations `dependencies` (JSON, already
/// quoted) in its `pane.json`.
fn settings_files(version: &str, api: &str, dependencies: &str) -> Vec<(&'static str, Vec<u8>)> {
    settings_files_of(version, api, dependencies, sample_component())
}

/// The tarball of the sample at `version`, with `component` in place of
/// its built one, so a test can publish a version whose code behaves
/// differently, such as one that fails to start.
fn settings_files_of(
    version: &str,
    api: &str,
    dependencies: &str,
    component: Vec<u8>,
) -> Vec<(&'static str, Vec<u8>)> {
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
        ("sample_settings_js.wasm", component),
    ]
}

/// The settings sample's built component.
fn sample_component() -> Vec<u8> {
    fs::read(guest("packages/sample-settings-js/sample_settings_js.wasm")).unwrap()
}

struct Dirs {
    data: TempDir,
    runtime: Runtime,
    registry: Registry,
    /// Where each test's Git repositories are made, served by `server`.
    repos: TempDir,
    server: Server,
    clock: Arc<ManualClock>,
}

impl Dirs {
    fn new() -> Dirs {
        let clock = ManualClock::at(SystemClock.now());
        Dirs {
            data: tempfile::tempdir().unwrap(),
            runtime: Runtime::start().unwrap(),
            registry: Registry::start(),
            repos: tempfile::tempdir().unwrap(),
            server: Server::start(),
            clock: clock.clone(),
        }
    }

    fn packages_dir(&self) -> PathBuf {
        self.data.path().join("extensions")
    }

    /// A launcher on this data folder downloading from the local registry,
    /// whose checks the test's clock moves along: advancing it past a
    /// minute brings the first check, and past a day the next one.
    fn launcher(&self) -> Launcher {
        Launcher::with_packages(Ok(self.runtime.clone()), vec![], self.packages_dir())
            .with_npm_registry(NpmRegistry::local(self.registry.url()).unwrap())
            .with_clock(self.clock.clone())
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

    /// Publishes the sample at `version` with `component` in place of its
    /// built one, tagged latest.
    fn publish_component(&self, version: &str, component: Vec<u8>) {
        self.registry.publish(
            NAME,
            version,
            pack(&settings_files_of(version, "0.1", "", component)),
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
        self.record_of(NAME)
    }

    /// The record of the package `name` in `installed.json`.
    fn record_of(&self, name: &str) -> serde_json::Value {
        let text = fs::read_to_string(self.packages_dir().join("installed.json")).unwrap();
        let registry: serde_json::Value = serde_json::from_str(&text).unwrap();
        registry["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["npm"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("no record of {name} in {registry:#}"))
    }

    /// The record of the local package installed from `folder` in
    /// `installed.json`, by its resolved folder.
    fn local_record(&self, folder: &Path) -> serde_json::Value {
        let identity = PackageIdentity::local(folder).unwrap();
        let text = fs::read_to_string(self.packages_dir().join("installed.json")).unwrap();
        let registry: serde_json::Value = serde_json::from_str(&text).unwrap();
        registry["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["local"] == identity.local_folder().unwrap().to_str().unwrap())
            .cloned()
            .unwrap_or_else(|| panic!("no local record in {registry:#}"))
    }

    /// The version the sample is installed at, from its record.
    fn installed_version(&self) -> String {
        self.version_of(NAME)
    }

    /// The version the package `name` is installed at, from its record.
    fn version_of(&self, name: &str) -> String {
        self.record_of(name)["npmVersion"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Publishes the sample as a package named `name`, titled `title`, at
    /// `version`, tagged latest.
    fn publish_as(&self, name: &str, title: &str, version: &str) {
        self.registry.publish(
            name,
            version,
            pack(&files_as(name, title, version, sample_component())),
        );
    }

    /// A launcher on this data folder that also develops, as the app's
    /// does: development mode needs the wiring (its builder and the
    /// channel that tells the window of changes).
    fn developing_launcher(&self) -> Launcher {
        let (changes, _) = pane_core::changes::channel();
        self.launcher().with_development(
            Arc::new(pane_core::develop::Toolchains::from_env(None)),
            changes,
        )
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
    /// one is due — the first check comes a minute after Pane starts.
    /// Waits until the check completed and whatever it staged was
    /// applied or deferred.
    fn check(&self, launcher: &Launcher) {
        self.clock.advance(Duration::from_secs(62));
        assert!(
            launcher.wait_for_updates(Duration::from_secs(30)),
            "the updater did not settle"
        );
    }

    /// The Git repository served as `name`, as an installed record's
    /// `git` field names it: its host and path, without the `git:` an
    /// identity key adds.
    fn git_identity(&self, name: &str) -> String {
        format!(
            "{}{}",
            self.server.url().trim_start_matches("http://"),
            name
        )
    }

    /// A new Git repository served as `name`, the controlled repository
    /// of the Git sample: its source alone on `main`, its built component
    /// on the branch `release`, tagged `v0.1.0`.
    fn git_repository(&self, name: &str) -> GitGreeter {
        let repo = Repo::init(&self.repos.path().join(name), self.server.home());
        let url = self.server.serve(name, &repo);
        repo.commit(&greeter_files(&guests(), false), "Greeter 0.1.0 source");
        repo.git(&["switch", "--quiet", "-c", "release"]);
        let release = repo.commit(&greeter_files(&guests(), true), "Release 0.1.0");
        repo.tag("v0.1.0");
        repo.git(&["switch", "--quiet", "main"]);
        GitGreeter { repo, url, release }
    }

    /// The record of the package installed from the Git repository served
    /// as `name`, in `installed.json`.
    fn git_record(&self, name: &str) -> serde_json::Value {
        let git = self.git_identity(name);
        let text = fs::read_to_string(self.packages_dir().join("installed.json")).unwrap();
        let registry: serde_json::Value = serde_json::from_str(&text).unwrap();
        registry["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["git"] == git.as_str())
            .cloned()
            .unwrap_or_else(|| panic!("no record of {git} in {registry:#}"))
    }

    /// The commit the package installed from the Git repository served as
    /// `name` is at, from its record.
    fn git_commit(&self, name: &str) -> String {
        self.git_record(name)["gitCommit"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Waits until the downloads folder is empty, as an ended install or
    /// update leaves it.
    fn wait_for_no_downloads(&self) {
        let downloads = self.packages_dir().join("downloads");
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left: Vec<_> = fs::read_dir(&downloads)
                .map(|entries| entries.map(|e| e.unwrap().file_name()).collect())
                .unwrap_or_default();
            if left.is_empty() {
                return;
            }
            assert!(Instant::now() < deadline, "downloads left: {left:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// The controlled Git repository of the Git sample, as
/// [`Dirs::git_repository`] makes it: `main` holding the source only,
/// `release` its built component (tagged `v0.1.0`), so that a tracked
/// branch can move while Pane is not looking.
struct GitGreeter {
    repo: Repo,
    /// The address it is served at, `http://127.0.0.1:<port>/<name>.git`.
    url: String,
    /// `release`, tagged `v0.1.0`: with the built component.
    release: String,
}

impl GitGreeter {
    /// Moves the release branch to a new release of the sample at
    /// `version`, a commit that changes only the manifest's version: what
    /// a tracked branch moving looks like to the updater. `main` and the
    /// tag stay where they were, pointing at the old release.
    fn move_release(&self, version: &str) -> String {
        self.repo.git(&["switch", "--quiet", "release"]);
        let mut files = greeter_files(&guests(), true);
        for (path, contents) in &mut files {
            if *path == "pane.json" {
                let manifest = String::from_utf8(std::mem::take(contents)).unwrap();
                *contents = manifest
                    .replace(
                        "\"version\": \"0.1.0\"",
                        &format!("\"version\": \"{version}\""),
                    )
                    .into_bytes();
            }
        }
        let moved = self.repo.commit(&files, &format!("Release {version}"));
        self.repo.git(&["switch", "--quiet", "main"]);
        moved
    }

    /// Moves the release branch to a revision holding the source only,
    /// without the built component: an unrunnable revision the updater
    /// must refuse, as a preview would.
    fn move_release_to_source(&self) -> String {
        self.repo.git(&["switch", "--quiet", "release"]);
        self.repo.git(&["rm", "--quiet", "-r", "dist"]);
        let moved = self.repo.commit(&[], "Source only");
        self.repo.git(&["switch", "--quiet", "main"]);
        moved
    }
}

fn activate(launcher: &Launcher, title: &str) {
    select_title(launcher, title);
    block_on(launcher.activate_selected());
}

/// Runs the item titled `item` of the Greeting command, returning what it
/// showed (its toast, or the status line); the command's screen stays
/// open, as it does for a user.
fn run(launcher: &Launcher, item: &str) -> Status {
    open_greeting(launcher);
    activate(launcher, item);
    shown(launcher)
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
    activate(launcher, "Manage Extensions");
    assert!(matches!(launcher.view().screen, Screen::Extensions { .. }));
}

fn error_of(launcher: &Launcher) -> String {
    match launcher.view().status {
        Status::Error(text) => text,
        other => panic!("not an error: {other:?}"),
    }
}

/// A Git commit id as the update results say it: its first 12 digits.
fn short_commit(commit: &str) -> String {
    commit[..commit.len().min(12)].to_owned()
}

/// The settings sample's files as a package named `name` at `version`
/// publishes them: the `package.json` renamed to match the name the
/// registry holds, so the tarball holds the package asked for.
fn named_files(name: &str, version: &str) -> Vec<(&'static str, Vec<u8>)> {
    let mut files = settings_files(version, "0.1", "");
    for (path, contents) in &mut files {
        if *path == "package.json" {
            let package = format!("{{ \"name\": \"{name}\", \"version\": \"{version}\" }}");
            *contents = package.into_bytes();
        }
    }
    files
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

/// Writes a local package folder titled "Local settings", whose Greeting
/// command runs the settings sample's component: an installed local-source
/// copy of the same code, as a user's own folder is.
fn local_package(sources: &Path) -> PathBuf {
    local_package_as(sources, "settings", "Local settings", "Local greeting")
}

/// Writes a local package folder under `sources`'s `name`, titled `title`,
/// whose Greeting command is titled `command`: the settings sample's
/// component, as a user's own folder is.
fn local_package_as(sources: &Path, name: &str, title: &str, command: &str) -> PathBuf {
    let folder = sources.join(name);
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("pane.json"),
        format!(
            r#"{{ "manifestVersion": 1, "title": "{title}", "version": "0.1.0",
             "apiVersion": "0.1",
             "commands": [{{ "id": "greeting", "title": "{command}",
                             "component": "sample_settings_js.wasm" }}] }}"#
        ),
    )
    .unwrap();
    fs::copy(
        guest("packages/sample-settings-js/sample_settings_js.wasm"),
        folder.join("sample_settings_js.wasm"),
    )
    .unwrap();
    folder
}

/// The settings sample's files as a package named `name`, titled `title`,
/// at `version`, with `component` in place of its built one (the sample's
/// by default): a second and third package told apart from the first by
/// its name and title, whose tarball holds the package asked for.
fn files_as(
    name: &str,
    title: &str,
    version: &str,
    component: Vec<u8>,
) -> Vec<(&'static str, Vec<u8>)> {
    let manifest = format!(
        r#"{{ "manifestVersion": 1, "title": "{title}", "version": "{version}",
             "apiVersion": "0.1",
             "commands": [{{ "id": "greeting", "title": "Greeting",
                             "component": "sample_settings_js.wasm" }}] }}"#
    );
    let package = format!(r#"{{ "name": "{name}", "version": "{version}" }}"#);
    vec![
        ("package.json", package.into_bytes()),
        ("pane.json", manifest.into_bytes()),
        ("sample_settings_js.wasm", component),
    ]
}

/// The metadata path the registry is asked for the package `name`.
fn metadata_path(name: &str) -> String {
    format!("/{}", name.replace('/', "%2f"))
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

    // The update is recorded as its outcome, and the status line stays
    // at rest: a successful background update is quiet.
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.updated[0].title, "Settings from npm");
    assert_eq!(recorded.updated[0].detail, "0.1.0 → 0.2.0");
    assert_eq!(launcher.view().status, Status::Idle);
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
    // Published before the command starts: packing the tarball is the
    // test's own work, and the save below runs for a fixed ten seconds,
    // which the check that finds this must fit inside.
    dirs.publish("0.2.0", "0.1");

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
    dirs.check(&launcher);

    // The update is staged and deferred: the installed copy is unchanged,
    // the user was not interrupted and the command is still running.
    assert_eq!(dirs.installed_version(), "0.1.0");
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
        || dirs.installed_version() == "0.2.0" && launcher.update_results().updated.len() == 1,
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

    // Not even asked about: a pinned version is not a candidate. Nothing
    // is recorded either — the pass found nothing new — and the status
    // line still says what happened before, not that anything was
    // updated.
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(dirs.registry.requests().len(), asked);
    assert!(launcher.update_results().is_empty());
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

    // The new version needs an API this Pane does not provide: skipped
    // with that reason, so skipping never looks like a fault — nothing
    // failed, and the record holds the row under Skipped.
    dirs.publish("0.2.0", "0.2");
    dirs.check(&launcher);

    let recorded = launcher.update_results();
    assert!(recorded.updated.is_empty() && recorded.failed.is_empty());
    let detail = &recorded.skipped[0].detail;
    assert!(
        detail.starts_with(
            "Incompatible package: it needs Pane extension API 0.2, but this Pane provides 0.1."
        ),
        "{detail}"
    );
    assert!(
        detail.ends_with("It keeps running its installed code."),
        "{detail}"
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

    let recorded = launcher.update_results();
    assert_eq!(recorded.failed.len(), 1);
    let detail = &recorded.failed[0].detail;
    assert!(
        detail.starts_with("It was not updated to 0.2.0: "),
        "{detail}"
    );
    assert!(
        detail.contains("npm package nobody was not found in the registry"),
        "{detail}"
    );
    assert!(
        detail.ends_with("It keeps running its installed code."),
        "{detail}"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");
}

#[test]
fn an_unreachable_registry_is_recorded_and_leaves_the_installed_copy_alone() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    drop(launcher);

    // A registry that refuses connections, as one that is down does. The
    // launcher is hidden while the pass runs, so the failure is announced
    // only when it is next shown (the announcement test).
    let closed = unreachable::ClosedPort::new();
    let url = format!("{}/", closed.url());
    let launcher = Launcher::with_packages(Ok(dirs.runtime.clone()), vec![], dirs.packages_dir())
        .with_npm_registry(NpmRegistry::local(&url).unwrap())
        .with_clock(dirs.clock.clone());
    launcher.set_window_presence(WindowPresence::Hidden);
    dirs.check(&launcher);

    let recorded = launcher.update_results();
    assert_eq!(recorded.failed.len(), 1);
    let detail = &recorded.failed[0].detail;
    assert!(
        detail.starts_with(&format!(
            "It was not checked for a newer version: Could not reach the npm \
             registry {url} for {NAME}:"
        )),
        "{detail}"
    );
    assert!(
        detail.ends_with("It keeps running its installed code."),
        "{detail}"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");
    // Nothing is announced while the launcher is hidden.
    assert_eq!(launcher.toast(), None);
    // The installed copy still runs, shown again as its user is.
    launcher.set_window_presence(WindowPresence::Shown);
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
    dirs.clock.advance(Duration::from_secs(24 * 3600 + 10));
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );

    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(launcher.update_results().updated.len(), 1);
    assert_eq!(launcher.update_results().updated[0].detail, "0.1.0 → 0.2.0");
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
    // A new Pane, on the same data: its first check comes a minute after
    // it starts, by the clock it follows, and updates the package —
    // quietly, as a successful background update is.
    let launcher = dirs.launcher();
    dirs.clock.advance(Duration::from_secs(62));
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );

    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(launcher.view().status, Status::Idle);
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.updated[0].detail, "0.1.0 → 0.2.0");
}

#[test]
fn a_local_folder_package_is_never_updated_automatically() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    // An installed local-source copy of the sample, as a user's own folder
    // is: it has no registry to check against. An npm package is installed
    // beside it, so the check demonstrably runs.
    let sources = tempfile::tempdir().unwrap();
    let folder = local_package(sources.path());
    block_on(launcher.install_package(&folder));
    let identity = PackageIdentity::local(&folder).unwrap();
    let location = launcher
        .packages()
        .into_iter()
        .find(|package| package.identity == identity)
        .expect("the local package is installed")
        .location
        .clone();
    dirs.install(&launcher, "0.1.0");

    // What the check must leave alone: the record, the managed copy's
    // files, and the local package never being asked about (the only
    // request the check makes is the npm package's metadata).
    let record = dirs.local_record(&folder);
    let manifest = fs::read(location.join("pane.json")).unwrap();
    let component = fs::read(location.join("sample_settings_js.wasm")).unwrap();
    let asked = dirs.registry.requests().len();

    dirs.check(&launcher);

    // The check ran, asking only about the npm package, and nothing it
    // found changed the local copy.
    assert_eq!(dirs.registry.requests().len(), asked + 1);
    assert_eq!(
        dirs.registry.requests()[asked],
        format!("/{}", NAME.replace('/', "%2f"))
    );
    assert_eq!(dirs.local_record(&folder), record);
    assert_eq!(
        fs::read(location.join("pane.json")).unwrap(),
        manifest,
        "the local package's manifest was touched"
    );
    assert_eq!(
        fs::read(location.join("sample_settings_js.wasm")).unwrap(),
        component,
        "the local package's code was touched"
    );
    // Its command still runs, and its settings are its own.
    to_root(&launcher);
    activate(&launcher, "Local greeting");
    assert_eq!(launcher.view().screen, Screen::Command);
    activate(&launcher, "Use a casual greeting");
    assert_eq!(
        shown(&launcher),
        Status::Result("Saved the casual greeting".into())
    );
}

#[test]
fn an_action_asked_while_the_update_applies_is_refused_not_stopped() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");

    dirs.publish("0.2.0", "0.1");

    // An action of the package's command, asked for but not sent yet: as
    // the deferral test holds a command running by not resolving it, this
    // holds its call back by not polling the future that makes it, so the
    // package is quiet and the update may apply. Leaving the command is
    // the boundary.
    open_greeting(&launcher);
    select_title(&launcher, "Use a casual greeting");
    let action = launcher.activate_selected();
    to_root(&launcher);

    // The check: the update is staged, and applying it claims the package
    // while the replacement is written. That window lasts as long as the
    // copy, which is no time at all where the copy is a clone (APFS): run
    // 37721686998's macOS leg never saw it by polling. So the test holds
    // the update once it has claimed the package, before the replacement
    // is written, asks its things inside the claim, and lets it go on.
    let hold = launcher.hold_update_applies();
    dirs.clock.advance(Duration::from_secs(62));
    let component = component_of(&launcher);
    {
        let deadline = Instant::now() + Duration::from_secs(120);
        while !launcher.package_being_updated(&component) {
            // Explains which way it failed: a check or stage failure (the
            // status line says why), or the apply deferred or never begun
            // (idle at 0.1.0, no claim).
            assert!(
                Instant::now() < deadline,
                "the update never claimed the package: the status is {:?}, the record has {}, \
                 the claims are {:?}",
                launcher.view().status,
                dirs.installed_version(),
                launcher.claims_now()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    assert_eq!(dirs.installed_version(), "0.1.0", "held before the copy");

    // The action asked of the updating package now, while the replacement
    // is being applied, is refused with the update's explanation rather
    // than started and then stopped by the replacement: its call is never
    // made. Opening the command is refused the same way, which is what
    // tells the moment was the apply window.
    block_on(action);
    activate(&launcher, "Greeting");
    assert_eq!(
        error_of(&launcher),
        "Settings from npm is updating; open it again once that is done"
    );

    // The update lands, and the refused action never ran.
    drop(hold);
    wait_until("the update applied", Duration::from_secs(120), || {
        dirs.installed_version() == "0.2.0"
    });
    assert!(!dirs.settings().contains("casual"));
}

#[test]
fn a_new_version_that_fails_to_start_is_not_rolled_back() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    // A setting saved through the old code: the replacement keeps it.
    assert_eq!(
        run(&launcher, "Use a casual greeting"),
        Status::Result("Saved the casual greeting".into())
    );
    to_root(&launcher);

    // 0.2.0's component is the failing-start fixture, which traps the
    // first time it is asked for its view and starts on every later one.
    dirs.publish_component("0.2.0", fs::read(guest("failing_start.wasm")).unwrap());
    dirs.check(&launcher);

    // The update applied and is recorded: the row shows the old and the
    // new version — the earlier code is not restored — and the saved
    // setting is kept.
    assert_eq!(launcher.view().status, Status::Idle);
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.updated[0].detail, "0.1.0 → 0.2.0");
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert!(dirs.settings().contains("casual"));

    // An update starts no code itself, as an install does (a reload, which
    // does start what it replaced, would pause a failing start with
    // Retry): the new code runs from the next call, the first of which
    // fails as the component does, without rolling anything back.
    activate(&launcher, "Greeting");
    let error = error_of(&launcher);
    assert!(error.starts_with("The extension crashed: "), "{error}");
    assert!(matches!(launcher.view().screen, Screen::Root { .. }));
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert!(dirs.settings().contains("casual"));
    manage(&launcher);
    assert!(!titles(&launcher).iter().any(|t| t.starts_with("Retry")));

    // The next start is the fixture's later one: the new code runs.
    to_root(&launcher);
    open_greeting(&launcher);
    assert_eq!(launcher.view().title, "Started");
    activate(&launcher, "Started on a later attempt");
    assert_eq!(shown(&launcher), Status::Result("ran started".into()));
    assert_eq!(dirs.installed_version(), "0.2.0");
}

/// What the Greeter from Git command's "Say hello" shows in its toast.
const GIT_HELLO: &str = "Hello from the Git repository";

/// Opens the Greeter from Git command from root search and runs `item`,
/// returning what it showed (its toast, or the status line); the
/// command's screen stays open, as it does for a user.
fn run_greeter(launcher: &Launcher, item: &str) -> Status {
    to_root(launcher);
    activate(launcher, "Greeter from Git");
    assert_eq!(launcher.view().screen, Screen::Command);
    activate(launcher, item);
    shown(launcher)
}

#[test]
fn a_moved_tracked_branch_updates_the_package_by_itself() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    let greeter = dirs.git_repository("greeter");

    // Installed from its tracked release branch; its command runs.
    block_on(launcher.install_git(&format!("{}@release", greeter.url)));
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Greeter from Git".into())
    );
    assert_eq!(
        run_greeter(&launcher, "Say hello"),
        Status::Result(GIT_HELLO.into())
    );
    let record = dirs.git_record("greeter");
    assert_eq!(record["gitRef"], "refs/heads/release");
    assert_eq!(record["gitCommit"], greeter.release.as_str());
    assert_eq!(record.get("pinned"), None);

    // Leave the command, so that no screen of the package is open when
    // the branch moves.
    to_root(&launcher);

    // The branch moves to a new release while no command runs.
    let moved = greeter.move_release("0.2.0");
    dirs.check(&launcher);

    // The update applied by itself: the record keeps the identity, the
    // tracked branch and the pin, at the branch's new commit, and the
    // row says the old and the new commit.
    let record = dirs.git_record("greeter");
    assert_eq!(record["git"], dirs.git_identity("greeter").as_str());
    assert_eq!(record["gitRef"], "refs/heads/release");
    assert_eq!(record["gitCommit"], moved.as_str());
    assert_eq!(record.get("pinned"), None);
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(
        recorded.updated[0].detail,
        format!(
            "{} → {}",
            short_commit(&greeter.release),
            short_commit(&moved)
        )
    );
    dirs.wait_for_no_downloads();

    // The new copy runs, and a check that finds the branch at its new
    // commit fetches nothing: no download appears and the record stays.
    assert_eq!(
        run_greeter(&launcher, "Say hello"),
        Status::Result(GIT_HELLO.into())
    );
    dirs.check(&launcher);
    assert_eq!(dirs.git_commit("greeter"), moved);
    dirs.wait_for_no_downloads();
}

#[test]
fn a_pinned_revision_is_never_updated_automatically() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    let greeter = dirs.git_repository("greeter");

    // Installed from its release tag: pinned, whatever the branch does.
    block_on(launcher.install_git(&format!("{}@v0.1.0", greeter.url)));
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Greeter from Git".into())
    );
    let record = dirs.git_record("greeter");
    assert_eq!(record["gitRef"], "refs/tags/v0.1.0");
    assert_eq!(record["pinned"], serde_json::json!(true));
    let asked = dirs.server.requests().len();

    // Both the tag and the branch move; a check runs. Not even the
    // repository's listing is asked for: a pinned revision is not a
    // candidate.
    greeter.move_release("0.2.0");
    dirs.check(&launcher);

    assert_eq!(dirs.git_commit("greeter"), greeter.release);
    assert_eq!(dirs.server.requests().len(), asked);
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Greeter from Git".into())
    );
}

#[test]
fn an_opted_out_git_package_is_not_updated_until_the_user_turns_updates_back_on() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    let greeter = dirs.git_repository("greeter");
    block_on(launcher.install_git(&format!("{}@release", greeter.url)));

    // The row in Manage extensions, and what it says before and after.
    manage(&launcher);
    activate(&launcher, "Update Greeter from Git automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of Greeter from Git are off".into())
    );
    assert!(titles(&launcher).contains(&"Update Greeter from Git automatically".to_owned()));

    greeter.move_release("0.2.0");
    let asked = dirs.server.requests().len();
    dirs.check(&launcher);
    assert_eq!(dirs.git_commit("greeter"), greeter.release);
    assert_eq!(dirs.server.requests().len(), asked);

    // Turning it back on checks at once, and the update applies.
    activate(&launcher, "Update Greeter from Git automatically");
    assert_eq!(
        launcher.view().status,
        Status::Result("Automatic updates of Greeter from Git are on".into())
    );
    wait_until("the update applied", Duration::from_secs(30), || {
        dirs.git_commit("greeter") != greeter.release
    });
}

#[test]
fn a_tracked_branch_now_holding_only_the_source_is_refused() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    let greeter = dirs.git_repository("greeter");
    block_on(launcher.install_git(&format!("{}@release", greeter.url)));
    assert_eq!(
        run_greeter(&launcher, "Say hello"),
        Status::Result(GIT_HELLO.into())
    );

    // Leave the command, so that the check's explanation shows in root
    // search's status line, where a background check explains itself.
    to_root(&launcher);

    // The branch moves to a revision without the built component: not a
    // runnable release revision, refused as a preview would refuse it,
    // with the installed copy untouched and still running.
    greeter.move_release_to_source();
    dirs.check(&launcher);

    let recorded = launcher.update_results();
    assert_eq!(recorded.failed.len(), 1);
    let detail = &recorded.failed[0].detail;
    assert!(
        detail.starts_with("It was not updated: Branch release (commit ")
            && detail.contains("holds only the source of")
            && detail.ends_with("It keeps running its installed code."),
        "{detail}"
    );
    assert_eq!(dirs.git_commit("greeter"), greeter.release);
    assert_eq!(
        run_greeter(&launcher, "Say hello"),
        Status::Result(GIT_HELLO.into())
    );
    dirs.wait_for_no_downloads();
}

/// The npm name of the second package the group test installs, pinned: a
/// package the pass does not look at, whatever the registry holds.
const PINNED: &str = "@pane-tests/pinned";

#[test]
fn a_pass_records_what_it_updated_skipped_and_failed_in_order() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    // An unpinned npm package: the pass updates it.
    dirs.install(&launcher, "0.1.0");
    // A pinned npm package: the pass does not look at it. Its tarball's
    // `package.json` names it, as the registry's does.
    dirs.registry
        .publish(PINNED, "0.1.0", pack(&named_files(PINNED, "0.1.0")));
    block_on(launcher.install_npm(&format!("{PINNED}@0.1.0")));
    // A local folder package: a local copy, never replaced by a pass.
    let sources = tempfile::tempdir().unwrap();
    let folder = local_package(sources.path());
    block_on(launcher.install_package(&folder));
    // A Git package whose tracked branch moved to a revision holding only
    // the source: the pass fails it.
    let greeter = dirs.git_repository("greeter");
    block_on(launcher.install_git(&format!("{}@release", greeter.url)));
    to_root(&launcher);

    dirs.publish("0.2.0", "0.1");
    greeter.move_release_to_source();
    dirs.check(&launcher);

    // The record: one Updated row, two Skipped with their reasons, one
    // Failed with its explanation, in that order (the view hides the
    // empty groups, and lists these in the groups' order).
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.updated[0].title, "Settings from npm");
    assert_eq!(recorded.updated[0].detail, "0.1.0 → 0.2.0");
    assert_eq!(recorded.skipped.len(), 2);
    let skipped = |key: String| {
        recorded
            .skipped
            .iter()
            .find(|row| row.identity.key() == key)
            .unwrap_or_else(|| panic!("no row for {key}"))
    };
    assert_eq!(
        skipped(PackageIdentity::npm(PINNED).key()).detail,
        "Its version is pinned"
    );
    assert_eq!(
        skipped(PackageIdentity::local(&folder).unwrap().key()).detail,
        "It is a local copy, from a folder"
    );
    assert_eq!(recorded.failed.len(), 1);
    let detail = &recorded.failed[0].detail;
    assert!(
        detail.starts_with("It was not updated: Branch release (commit ")
            && detail.ends_with("It keeps running its installed code."),
        "{detail}"
    );
    // The updated package really is, and the others are as they were.
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(dirs.git_commit("greeter"), greeter.release);
}

#[test]
fn the_record_persists_across_a_restart_and_a_quiet_pass_keeps_it() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    let sources = tempfile::tempdir().unwrap();
    let folder = local_package(sources.path());
    block_on(launcher.install_package(&folder));

    dirs.publish("0.2.0", "0.1");
    dirs.check(&launcher);
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.skipped.len(), 1);
    drop(launcher);

    // A new Pane on the same data: the record is read again, before any
    // check of its own has run.
    let launcher = dirs.launcher();
    assert_eq!(launcher.update_results(), recorded);

    // Its first check finds nothing new — everything is up to date — and
    // a pass that found nothing new does not replace the record.
    dirs.check(&launcher);
    assert_eq!(launcher.update_results(), recorded, "the record stays");
}

#[test]
fn a_failed_pass_is_announced_once_the_next_time_the_launcher_is_shown() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    drop(launcher);

    // A registry that refuses connections, as one that is down does. The
    // launcher is hidden while the pass runs: the failure waits for the
    // next time the launcher is shown.
    let closed = unreachable::ClosedPort::new();
    let url = format!("{}/", closed.url());
    let launcher = Launcher::with_packages(Ok(dirs.runtime.clone()), vec![], dirs.packages_dir())
        .with_npm_registry(NpmRegistry::local(&url).unwrap())
        .with_clock(dirs.clock.clone());
    launcher.set_window_presence(WindowPresence::Hidden);
    dirs.check(&launcher);
    assert_eq!(dirs.installed_version(), "0.1.0");
    assert_eq!(launcher.toast(), None, "nothing announced while hidden");

    // Shown: the failure is announced, once, with View Details.
    launcher.set_window_presence(WindowPresence::Shown);
    let toast = launcher.toast().expect("the failure is announced");
    assert_eq!(toast.toast.style, ToastStyle::Failure);
    assert_eq!(toast.toast.title, "1 extension update failed");
    assert_eq!(
        toast
            .toast
            .primary
            .as_ref()
            .map(|action| action.title.as_str()),
        Some("View Details")
    );

    // The toast's time runs out, and showing the launcher again says
    // nothing more for the same failure.
    launcher.toast_left(toast.id, toast.revision);
    launcher.set_window_presence(WindowPresence::Hidden);
    launcher.set_window_presence(WindowPresence::Shown);
    assert_eq!(launcher.toast(), None, "not repeated for the same failure");

    // A day passes and the next pass fails again, hidden: a new failing
    // pass re-arms the announcement.
    launcher.set_window_presence(WindowPresence::Hidden);
    dirs.clock.advance(Duration::from_secs(24 * 3600 + 10));
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );
    assert_eq!(launcher.toast(), None, "hidden: nothing announced yet");
    launcher.set_window_presence(WindowPresence::Shown);
    let again = launcher.toast().expect("the new failure is announced");
    assert_ne!(again.id, toast.id, "another toast");
    assert_eq!(again.toast.title, "1 extension update failed");
}

#[test]
fn the_first_check_comes_a_minute_after_pane_starts() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish("0.2.0", "0.1");
    let asked = dirs.registry.requests().len();

    // Before the minute is up, nothing is checked: no request is made and
    // the installed copy stays.
    dirs.clock.advance(Duration::from_secs(59));
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        dirs.registry.requests().len(),
        asked,
        "nothing checked before a minute"
    );
    assert_eq!(dirs.installed_version(), "0.1.0");

    // Past the minute, the first check runs and applies what it finds.
    dirs.clock.advance(Duration::from_secs(3));
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the updater did not settle"
    );
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(launcher.update_results().updated.len(), 1);
}

#[test]
fn the_update_results_screen_shows_the_groups_and_settings_opens_it() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish("0.2.0", "0.1");
    dirs.check(&launcher);

    // The extension list offers the record as an operation of the
    // launcher's, which Settings runs without moving the user off their
    // screen; the row is there once anything is recorded.
    manage(&launcher);
    assert!(
        titles(&launcher)
            .iter()
            .any(|title| title == "Update Results")
    );
    let operation = launcher
        .extension_operations()
        .into_iter()
        .find(|operation| operation.kind == OperationKind::UpdateResults)
        .expect("the operation is offered");
    assert_eq!(operation.owner, None);
    block_on(launcher.run_extension_operation(&operation));

    // The screen: its rows one per result, each opening its extension's
    // page in Settings, which the window does with the selected row.
    let view = launcher.view();
    assert!(matches!(view.screen, Screen::UpdateResults { query } if query.is_empty()));
    assert_eq!(view.title, "Update Results");
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].title, "Settings from npm");
    assert_eq!(view.rows[0].subtitle.as_deref(), Some("0.1.0 → 0.2.0"));
    assert!(matches!(
        launcher.selected_settings_target(),
        Some(SettingsTarget::Extension(_))
    ));

    // The search field filters the rows, and the selection follows what
    // is listed.
    block_on(launcher.set_query("settings"));
    assert_eq!(launcher.view().rows.len(), 1);
    assert_eq!(launcher.view().selected, Some(0));
    block_on(launcher.set_query("nowhere"));
    assert_eq!(launcher.view().rows.len(), 0);
    assert_eq!(launcher.view().selected, None);
    block_on(launcher.set_query(""));
    assert_eq!(launcher.view().rows.len(), 1);

    // Back returns to the extension list the flow was entered from.
    launcher.back();
    assert!(matches!(launcher.view().screen, Screen::Extensions { .. }));
}

/// The npm name of the packages the asked-pass tests install beside the
/// first: one whose automatic updates are turned off, one disabled, one
/// paused, one pinned, and a second plain one.
const SECOND_NAME: &str = "@pane-tests/second";
const OFF: &str = "@pane-tests/off";
const DISABLED: &str = "@pane-tests/disabled";
const PAUSED: &str = "@pane-tests/paused";

/// Writes a local package folder titled "Developed settings" that also
/// builds: a `Cargo.toml` beside its manifest makes development mode watch
/// the folder, so an installed copy of it is a development copy while it
/// is developed. No file is saved, so no build runs.
fn developed_package(sources: &Path) -> PathBuf {
    let folder = local_package_as(
        sources,
        "developed",
        "Developed settings",
        "Developed greeting",
    );
    fs::write(folder.join("Cargo.toml"), "").unwrap();
    folder
}

/// Activates the Greeting command of the package with the identity key
/// `key`, among the other packages' commands of the same title.
fn activate_greeting_of(launcher: &Launcher, key: &str) {
    to_root(launcher);
    let id = format!("{key}#greeting");
    let index = launcher
        .view()
        .rows
        .iter()
        .position(|row| row.id == id)
        .unwrap_or_else(|| panic!("no row {id}"));
    launcher.select(index);
    block_on(launcher.activate_selected());
}

#[test]
fn the_root_search_row_and_the_public_call_start_a_pass_at_once() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish("0.2.0", "0.1");

    // The row is listed among Pane's own rows; no check has run yet (the
    // first automatic one comes a minute after Pane starts).
    to_root(&launcher);
    assert!(
        titles(&launcher)
            .iter()
            .any(|title| title == "Check for Extension Updates")
    );
    assert_eq!(launcher.last_extension_check(), None);

    // Activating the row starts the pass at once, whatever the cadence:
    // the clock never moves, and the wait is for the pass itself.
    activate(&launcher, "Check for Extension Updates");
    assert_eq!(dirs.installed_version(), "0.2.0");
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1);
    assert_eq!(recorded.updated[0].detail, "0.1.0 → 0.2.0");
    let toast = launcher.toast().expect("the ending toast");
    assert_eq!(
        (toast.toast.style, toast.toast.title.as_str()),
        (ToastStyle::Success, "Updated 1 extension")
    );
    assert_eq!(
        toast
            .toast
            .primary
            .as_ref()
            .map(|action| action.title.as_str()),
        Some("View Details")
    );
    assert_eq!(launcher.last_extension_check(), Some(dirs.clock.now()));

    // The public call both entry points run (root search's row through
    // the same one, the Settings Extensions group's button by hand here):
    // a pass over what is now up to date, which answers even so — the
    // user asked — and says when it checked.
    let asked = dirs.registry.requests().len();
    block_on(launcher.check_extension_updates());
    assert_eq!(dirs.registry.requests().len(), asked + 1);
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        Some("Extensions are up to date".into())
    );
    // A pass that found nothing new keeps the record.
    assert_eq!(launcher.update_results(), recorded);
    drop(launcher);

    // When the check last ran survives a restart with the record.
    let checked = dirs.clock.now();
    let launcher = dirs.launcher();
    assert_eq!(launcher.last_extension_check(), Some(checked));
}

#[test]
fn a_pass_the_user_asked_for_includes_off_disabled_and_paused_extensions() {
    let dirs = Dirs::new();
    let launcher = dirs.developing_launcher();
    dirs.install(&launcher, "0.1.0");
    // Whose automatic updates are turned off.
    dirs.publish_as(OFF, "Off settings", "0.1.0");
    block_on(launcher.install_npm(OFF));
    // Disabled.
    dirs.publish_as(DISABLED, "Disabled settings", "0.1.0");
    block_on(launcher.install_npm(DISABLED));
    block_on(launcher.set_enabled(&PackageIdentity::npm(DISABLED), false));
    // Paused: its command crashes until Pane pauses it after the third.
    dirs.publish_as(PAUSED, "Paused settings", "0.1.0");
    block_on(launcher.install_npm(PAUSED));
    for _ in 0..3 {
        activate_greeting_of(&launcher, &PackageIdentity::npm(PAUSED).key());
        activate(&launcher, "Crash");
    }
    assert!(matches!(
        launcher.extension_mark(&PackageIdentity::npm(PAUSED)),
        Some(pane_core::ExtensionMark::Paused(_))
    ));
    // A pinned package, a local folder's copy and a development copy.
    dirs.registry.publish(
        PINNED,
        "0.1.0",
        pack(&files_as(
            PINNED,
            "Pinned settings",
            "0.1.0",
            sample_component(),
        )),
    );
    block_on(launcher.install_npm(&format!("{PINNED}@0.1.0")));
    let sources = tempfile::tempdir().unwrap();
    let folder = local_package(sources.path());
    block_on(launcher.install_package(&folder));
    let developed = developed_package(sources.path());
    block_on(launcher.install_package(&developed));
    block_on(launcher.start_developing(&PackageIdentity::local(&developed).unwrap()));
    to_root(&launcher);

    // A newer version of every updatable one is published, and the OFF
    // package's updates are turned off — the toggle checks at once, so
    // the automatic pass that follows runs over everything as it stands:
    // it updates none but the plain one, and skips the rest with their
    // reasons.
    dirs.publish("0.2.0", "0.1");
    dirs.publish_as(OFF, "Off settings", "0.2.0");
    dirs.publish_as(DISABLED, "Disabled settings", "0.2.0");
    dirs.publish_as(PAUSED, "Paused settings", "0.2.0");
    manage(&launcher);
    activate(&launcher, "Update Off settings automatically");
    to_root(&launcher);
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the toggle's check settled"
    );
    assert_eq!(dirs.installed_version(), "0.2.0");
    for name in [OFF, DISABLED, PAUSED] {
        assert_eq!(dirs.version_of(name), "0.1.0", "{name} not updated");
    }

    // The pass the user asks for: everything Pane could update is
    // updated — turned off, disabled and paused ones too — and the
    // pinned, local and development copies are skipped with their
    // reasons.
    dirs.publish("0.3.0", "0.1");
    block_on(launcher.check_extension_updates());
    assert_eq!(dirs.installed_version(), "0.3.0");
    assert_eq!(dirs.version_of(OFF), "0.2.0");
    // An update keeps a disabled package disabled.
    assert_eq!(dirs.version_of(DISABLED), "0.2.0");
    assert!(
        !launcher
            .packages()
            .into_iter()
            .any(|package| package.identity == PackageIdentity::npm(DISABLED) && package.enabled)
    );
    // An update of a paused one unpauses it, as the preview's Update row
    // does: its code runs again.
    assert_eq!(dirs.version_of(PAUSED), "0.2.0");
    assert_eq!(launcher.extension_mark(&PackageIdentity::npm(PAUSED)), None);
    activate_greeting_of(&launcher, &PackageIdentity::npm(PAUSED).key());
    activate(&launcher, "Use a casual greeting");
    assert_eq!(
        shown(&launcher),
        Status::Result("Saved the casual greeting".into())
    );

    // The record: every updatable one updated, the rest skipped with why.
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 4, "{recorded:#?}");
    assert!(recorded.waiting.is_empty(), "{recorded:#?}");
    let skipped = |key: String| {
        recorded
            .skipped
            .iter()
            .find(|row| row.identity.key() == key)
            .unwrap_or_else(|| panic!("no row for {key}"))
    };
    assert_eq!(
        skipped(PackageIdentity::npm(PINNED).key()).detail,
        "Its version is pinned"
    );
    assert_eq!(
        skipped(PackageIdentity::local(&folder).unwrap().key()).detail,
        "It is a local copy, from a folder"
    );
    assert_eq!(
        skipped(PackageIdentity::local(&developed).unwrap().key()).detail,
        "It is a development copy"
    );
    // The rows keep the installed list's order, whatever order the pass's
    // checks completed in (they run a few at a time).
    assert_eq!(
        recorded
            .updated
            .iter()
            .map(|row| row.identity.key())
            .collect::<Vec<_>>(),
        vec![
            PackageIdentity::npm(NAME).key(),
            PackageIdentity::npm(OFF).key(),
            PackageIdentity::npm(DISABLED).key(),
            PackageIdentity::npm(PAUSED).key(),
        ]
    );
}

#[test]
fn a_pass_the_user_asked_for_shows_its_progress_and_ends_with_its_summary() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish("0.2.0", "0.1");

    // The registry holds the metadata answer back: the pass stays in its
    // checking phase, its toast saying so.
    let held = dirs.registry.hold(&metadata_path(NAME));
    let _pass = launcher.check_extension_updates();
    wait_until("the checking toast", Duration::from_secs(30), || {
        feedback::toast_title(&launcher).as_deref() == Some("Checking for extension updates…")
    });
    drop(held);

    // The applies are held: the toast says how far they have got.
    let held = launcher.hold_update_applies();
    wait_until("the updating toast", Duration::from_secs(30), || {
        feedback::toast_title(&launcher).as_deref() == Some("Updating 1 of 1…")
    });
    let updating = launcher.toast().expect("the toast");
    drop(held);
    wait_until("the ending toast", Duration::from_secs(30), || {
        feedback::toast_title(&launcher).as_deref() == Some("Updated 1 extension")
    });

    // One toast through the pass, updated by id: not replaced.
    let ended = launcher.toast().expect("the ending toast");
    assert_eq!(ended.id, updating.id);
    assert!(ended.revision > updating.revision, "updated, not replaced");
    assert_eq!(ended.toast.style, ToastStyle::Success);
    assert_eq!(dirs.installed_version(), "0.2.0");

    // The toast's View Details opens the results view.
    block_on(launcher.run_toast_action(ended.id, pane_core::ToastSlot::Primary));
    assert!(matches!(
        launcher.view().screen,
        Screen::UpdateResults { .. }
    ));
    assert_eq!(launcher.view().rows.len(), 1);
}

#[test]
fn an_asked_pass_that_failed_something_ends_with_its_failures() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.1.0");
    block_on(launcher.install_npm(SECOND_NAME));
    // A Git package whose tracked branch has moved to a revision holding
    // only the source: the pass fails it, as a preview would refuse it.
    let greeter = dirs.git_repository("greeter");
    block_on(launcher.install_git(&format!("{}@release", greeter.url)));
    to_root(&launcher);

    dirs.publish("0.2.0", "0.1");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.2.0");
    greeter.move_release_to_source();
    block_on(launcher.check_extension_updates());

    // The ending toast says what was updated and what failed, carrying
    // View Details; the failures are not announced again on the next
    // showing — this toast was the announcement.
    let toast = launcher.toast().expect("the ending toast");
    assert_eq!(
        (toast.toast.style, toast.toast.title.as_str()),
        (ToastStyle::Failure, "Updated 2 extensions, 1 failed")
    );
    launcher.toast_left(toast.id, toast.revision);
    launcher.set_window_presence(WindowPresence::Hidden);
    launcher.set_window_presence(WindowPresence::Shown);
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        None,
        "the asked pass's ending toast was the announcement"
    );
    // What was updated and failed really was.
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(dirs.version_of(SECOND_NAME), "0.2.0");
    assert_eq!(dirs.git_commit("greeter"), greeter.release);
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 2);
    assert_eq!(recorded.failed.len(), 1);
}

#[test]
fn a_package_in_use_waits_and_is_listed_until_it_is_quiet() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish("0.2.0", "0.1");

    // "Save after waiting", run on another thread with the command's
    // screen open: the package is in use (the deferral test's shape).
    open_greeting(&launcher);
    select_title(&launcher, "Save after waiting");
    let saving = launcher.activate_selected();
    let thread = thread::spawn(move || {
        block_on(saving);
    });
    wait_until("the slow save started", Duration::from_secs(10), || {
        dirs.slow_save() == Some("started")
    });

    // The pass the user asks for stages the update and defers it: the
    // record lists it as waiting, and the toast ends — the pass has
    // nothing more it can do now.
    block_on(launcher.check_extension_updates());
    assert_eq!(dirs.installed_version(), "0.1.0");
    let recorded = launcher.update_results();
    assert_eq!(recorded.waiting.len(), 1, "{recorded:#?}");
    assert_eq!(
        recorded.waiting[0].detail,
        "Waiting until Settings from npm is not in use"
    );
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        Some("Extensions are up to date".into())
    );

    // The command finishes and the user leaves: the update applies, and
    // the waiting row becomes the updated one.
    thread.join().unwrap();
    to_root(&launcher);
    wait_until(
        "the deferred update applied",
        Duration::from_secs(30),
        || {
            dirs.installed_version() == "0.2.0"
                && launcher.update_results().updated.len() == 1
                && launcher.update_results().waiting.is_empty()
        },
    );
    assert_eq!(launcher.update_results().updated[0].detail, "0.1.0 → 0.2.0");
}

#[test]
fn retry_checks_that_extension_alone() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.1.0");
    block_on(launcher.install_npm(SECOND_NAME));
    drop(launcher);

    // A new Pane on the same data whose registry is down: the pass it is
    // asked for fails both, and its ending toast is their announcement.
    let closed = unreachable::ClosedPort::new();
    let url = format!("{}/", closed.url());
    let launcher = Launcher::with_packages(Ok(dirs.runtime.clone()), vec![], dirs.packages_dir())
        .with_npm_registry(NpmRegistry::local(&url).unwrap())
        .with_clock(dirs.clock.clone());
    block_on(launcher.check_extension_updates());
    let recorded = launcher.update_results();
    assert_eq!(recorded.failed.len(), 2, "{recorded:#?}");
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        Some("2 extension updates failed".into())
    );
    // Retry is offered on a failed row.
    let retried = recorded.failed[1].identity.key();
    assert_eq!(
        launcher.update_results_row_actions(&retried),
        vec![UpdateResultsAction::Retry]
    );
    drop(launcher);

    // A Pane on the same data with its registry back: retrying the one
    // row checks that extension alone — only its metadata is asked for,
    // and it updates — while the other stays as it was, its failure gone
    // from the record (the retry's pass replaced it).
    dirs.publish("0.2.0", "0.1");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.2.0");
    let launcher = dirs.launcher();
    let asked = dirs.registry.requests().len();
    block_on(launcher.check_extension_update_of(&retried));
    // Only that extension was checked: its metadata — the check's own
    // reading, and the staging's again as an install does — and the
    // tarball of the update it found; nothing of the other's.
    let made: Vec<String> = dirs.registry.requests()[asked..].to_vec();
    assert_eq!(
        made,
        vec![
            metadata_path(SECOND_NAME),
            metadata_path(SECOND_NAME),
            "/@pane-tests/second/-/second-0.2.0.tgz".to_owned(),
        ],
        "the retry's requests"
    );
    assert_eq!(dirs.version_of(SECOND_NAME), "0.2.0");
    assert_eq!(dirs.installed_version(), "0.1.0", "only that one");
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1, "{recorded:#?}");
    assert_eq!(
        recorded.updated[0].identity.key(),
        PackageIdentity::npm(SECOND_NAME).key()
    );
    // Its ending toast is the retry's own summary.
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        Some("Updated 1 extension".into())
    );
}

#[test]
fn update_now_updates_an_extension_skipped_only_for_the_users_switch() {
    let dirs = Dirs::new();
    let launcher = dirs.launcher();
    dirs.install(&launcher, "0.1.0");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.1.0");
    block_on(launcher.install_npm(SECOND_NAME));

    // A newer version of both is published, and the second package's
    // automatic updates are turned off — the toggle checks at once, so
    // the automatic pass that follows updates the first and records the
    // second as skipped for the user's switch.
    dirs.publish("0.2.0", "0.1");
    dirs.publish_as(SECOND_NAME, "Second settings", "0.2.0");
    manage(&launcher);
    activate(&launcher, "Update Second settings automatically");
    to_root(&launcher);
    assert!(
        launcher.wait_for_updates(Duration::from_secs(30)),
        "the toggle's check settled"
    );
    assert_eq!(dirs.installed_version(), "0.2.0");
    assert_eq!(dirs.version_of(SECOND_NAME), "0.1.0");
    let recorded = launcher.update_results();
    let second = PackageIdentity::npm(SECOND_NAME).key();
    let skipped = recorded
        .skipped
        .iter()
        .find(|row| row.identity.key() == second)
        .expect("the skipped row");
    assert_eq!(skipped.detail, "Automatic updates of it are off");

    // Update Now is offered on that row, on no other: the updated row
    // offers nothing, a row whose only reason is not the user's switch
    // (say a pinned one) would not either.
    assert_eq!(
        launcher.update_results_row_actions(&second),
        vec![UpdateResultsAction::UpdateNow]
    );
    assert_eq!(
        launcher.update_results_row_actions(&PackageIdentity::npm(NAME).key()),
        Vec::new()
    );

    // It updates the extension now: an asked pass whose scope is that
    // one, its outcome landing in the record and its toast as any asked
    // pass's.
    block_on(launcher.check_extension_update_of(&second));
    assert_eq!(dirs.version_of(SECOND_NAME), "0.2.0");
    let recorded = launcher.update_results();
    assert_eq!(recorded.updated.len(), 1, "{recorded:#?}");
    assert_eq!(recorded.updated[0].identity.key(), second);
    assert_eq!(
        launcher.toast().map(|toast| toast.toast.title),
        Some("Updated 1 extension".into())
    );
}
