//! Disabling and re-enabling installed packages through the launcher's public
//! interface: a disabled package contributes nothing and runs nothing, stays
//! disabled across restarts, and keeps its settings for when it is enabled
//! again. Uses the real settings sample guest from `cargo xtask guests`.

use std::fs;
use std::path::{Path, PathBuf};

use futures::executor::block_on;
use pane_core::{CallError, Launcher, PackageIdentity, Runtime, Screen, Status};
use tempfile::TempDir;

const INSTALL_ROW: &str = "Install extension from folder…";
const MANAGE_ROW: &str = "Manage extensions…";

/// Copies the assembled settings sample package into `folder`, a package
/// with its own identity. `title` replaces the package title.
fn settings_package(folder: &Path, title: &str) -> PathBuf {
    let assembled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests/packages/sample-settings");
    assert!(
        assembled.exists(),
        "{} is missing; run `cargo xtask guests`",
        assembled.display()
    );
    fs::create_dir_all(folder).unwrap();
    let manifest = fs::read_to_string(assembled.join("pane.json"))
        .unwrap()
        .replace("\"Settings sample\"", &format!("\"{title}\""));
    fs::write(folder.join("pane.json"), manifest).unwrap();
    fs::copy(
        assembled.join("sample_settings.wasm"),
        folder.join("sample_settings.wasm"),
    )
    .unwrap();
    folder.to_path_buf()
}

struct Dirs {
    sources: TempDir,
    data: TempDir,
}

impl Dirs {
    fn new() -> Dirs {
        Dirs {
            sources: tempfile::tempdir().unwrap(),
            data: tempfile::tempdir().unwrap(),
        }
    }

    fn source(&self, name: &str) -> PathBuf {
        self.sources.path().join(name)
    }

    fn packages_dir(&self) -> PathBuf {
        self.data.path().join("extensions")
    }

    /// A launcher on this data folder; a new one is a restart of Pane.
    fn launcher(&self) -> Launcher {
        Launcher::with_packages(Runtime::start(), vec![], self.packages_dir())
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

fn subtitles(launcher: &Launcher) -> Vec<String> {
    launcher
        .view()
        .rows
        .into_iter()
        .map(|row| row.subtitle.unwrap_or_default())
        .collect()
}

fn select_title(launcher: &Launcher, title: &str) {
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    launcher.select(index);
}

/// From root search, opens the command titled `command` and runs its item
/// titled `item`, returning the outcome.
fn run(launcher: &Launcher, command: &str, item: &str) -> Status {
    launcher.back();
    select_title(launcher, command);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Command, "{command} opened");
    select_title(launcher, item);
    block_on(launcher.activate_selected());
    launcher.view().status
}

/// Opens the extension manager from root search.
fn manage(launcher: &Launcher) {
    launcher.back();
    select_title(launcher, MANAGE_ROW);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Extensions);
}

/// Enables or disables the package on row `index` of the extension manager.
fn toggle(launcher: &Launcher, index: usize) -> Status {
    manage(launcher);
    launcher.select(index);
    block_on(launcher.activate_selected());
    launcher.view().status
}

fn enabled(launcher: &Launcher) -> Vec<(PackageIdentity, bool)> {
    launcher
        .packages()
        .into_iter()
        .map(|package| (package.identity, package.enabled))
        .collect()
}

#[test]
fn a_disabled_package_leaves_root_search_and_stays_disabled_after_a_restart() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&folder));
    assert_eq!(titles(&launcher), ["Greeting", INSTALL_ROW, MANAGE_ROW]);

    manage(&launcher);
    let identity = PackageIdentity::local(&folder).unwrap();
    assert_eq!(titles(&launcher), ["Settings sample"]);
    assert_eq!(launcher.view().rows[0].id, identity.key());
    assert!(subtitles(&launcher)[0].starts_with("Enabled"));

    assert_eq!(
        toggle(&launcher, 0),
        Status::Result("Disabled Settings sample".into())
    );
    assert!(subtitles(&launcher)[0].starts_with("Disabled"));
    launcher.back();
    assert_eq!(titles(&launcher), [INSTALL_ROW, MANAGE_ROW]);

    // A restart without a runtime: listing runs no guest and keeps the choice.
    let unavailable = Err(CallError::RuntimeUnavailable("no engine".into()));
    let restarted = Launcher::with_packages(unavailable, vec![], dirs.packages_dir());
    assert_eq!(titles(&restarted), [INSTALL_ROW, MANAGE_ROW]);
    assert_eq!(enabled(&restarted), [(identity.clone(), false)]);
    manage(&restarted);
    assert!(subtitles(&restarted)[0].starts_with("Disabled"));
}

#[test]
fn re_enabling_after_a_restart_restores_the_saved_settings() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&folder));
    assert_eq!(
        run(&launcher, "Greeting", "Greet me"),
        Status::Error(
            "The extension reported an error: No greeting style is saved yet; choose one first"
                .into()
        )
    );
    assert_eq!(
        run(&launcher, "Greeting", "Use a formal greeting"),
        Status::Result("Saved the formal greeting".into())
    );
    toggle(&launcher, 0);
    drop(launcher);

    let restarted = dirs.launcher();
    assert_eq!(
        toggle(&restarted, 0),
        Status::Result("Enabled Settings sample".into())
    );
    restarted.back();
    assert_eq!(titles(&restarted), ["Greeting", INSTALL_ROW, MANAGE_ROW]);
    assert_eq!(
        run(&restarted, "Greeting", "Greet me"),
        Status::Result("Good day to you".into())
    );
    assert_eq!(restarted.view().title, "Greeting: formal");
    assert_eq!(
        enabled(&restarted),
        [(PackageIdentity::local(&folder).unwrap(), true)]
    );
}

#[test]
fn copies_with_the_same_title_are_enabled_and_keep_settings_by_identity() {
    let dirs = Dirs::new();
    let published = settings_package(&dirs.source("published"), "Greeter");
    let development = settings_package(&dirs.source("development"), "Greeter");
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&published));
    block_on(launcher.install_package(&development));
    launcher.back();
    // Each copy saves its own choice.
    launcher.select(0);
    block_on(launcher.activate_selected());
    select_title(&launcher, "Use a formal greeting");
    block_on(launcher.activate_selected());
    launcher.back();
    launcher.select(1);
    block_on(launcher.activate_selected());
    select_title(&launcher, "Use a casual greeting");
    block_on(launcher.activate_selected());

    manage(&launcher);
    assert_eq!(titles(&launcher), ["Greeter", "Greeter"]);
    let published_id = PackageIdentity::local(&published).unwrap();
    let development_id = PackageIdentity::local(&development).unwrap();
    let ids: Vec<String> = launcher.view().rows.into_iter().map(|r| r.id).collect();
    assert_eq!(ids, [published_id.key(), development_id.key()]);
    // The row says which copy it is.
    assert!(subtitles(&launcher)[0].contains(&published_id.to_string()));

    toggle(&launcher, 1);
    assert_eq!(
        enabled(&launcher),
        [
            (published_id.clone(), true),
            (development_id.clone(), false)
        ]
    );
    // The enabled copy is the one left in root search, with its own setting.
    assert_eq!(
        run(&launcher, "Greeting", "Greet me"),
        Status::Result("Good day to you".into())
    );

    toggle(&launcher, 0);
    let restarted = dirs.launcher();
    assert_eq!(titles(&restarted), [INSTALL_ROW, MANAGE_ROW]);
    // Enabling one copy leaves the other disabled.
    toggle(&restarted, 1);
    assert_eq!(
        enabled(&restarted),
        [(published_id, false), (development_id, true)]
    );
    assert_eq!(
        run(&restarted, "Greeting", "Greet me"),
        Status::Result("Hi there".into())
    );
}

#[test]
fn disabling_through_the_api_closes_the_package_command_and_updating_keeps_it_disabled() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&folder));
    let identity = PackageIdentity::local(&folder).unwrap();
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Command);

    block_on(launcher.set_enabled(&identity, false));

    let view = launcher.view();
    assert_eq!(view.screen, Screen::Root);
    assert_eq!(
        view.status,
        Status::Result("Disabled Settings sample".into())
    );
    assert_eq!(titles(&launcher), [INSTALL_ROW, MANAGE_ROW]);

    // An update replaces the code, not the user's choice.
    block_on(launcher.preview_package(&folder));
    assert!(
        launcher
            .view()
            .details
            .contains(&"Disabled: enable it in Manage extensions".to_owned()),
        "{:?}",
        launcher.view().details
    );
    block_on(launcher.activate_selected());
    assert_eq!(enabled(&launcher), [(identity.clone(), false)]);
    assert_eq!(titles(&launcher), [INSTALL_ROW, MANAGE_ROW]);
    assert_eq!(enabled(&dirs.launcher()), [(identity, false)]);
}

#[test]
fn enabling_an_identity_that_is_not_installed_is_explained() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    let launcher = dirs.launcher();

    block_on(launcher.set_enabled(&PackageIdentity::local(&folder).unwrap(), true));

    match launcher.view().status {
        Status::Error(message) => assert!(message.starts_with("Nothing is installed from")),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn commands_built_into_pane_have_no_settings() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    let command = pane_core::CommandRegistration {
        id: "greeting".into(),
        title: "Built-in greeting".into(),
        subtitle: None,
        component: folder.join("sample_settings.wasm"),
    };
    let launcher = Launcher::new(Runtime::start(), vec![command]);

    block_on(launcher.activate_selected());

    match launcher.view().status {
        Status::Error(message) => {
            assert!(message.contains("only installed packages"), "{message}")
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn unreadable_settings_are_explained_to_the_command_and_never_overwritten() {
    let dirs = Dirs::new();
    let folder = settings_package(&dirs.source("settings"), "Settings sample");
    block_on(dirs.launcher().install_package(&folder));
    let file = dirs.packages_dir().join("settings.json");
    fs::write(&file, "not settings").unwrap();

    let restarted = dirs.launcher();
    // The command's view reads the saved style.
    block_on(restarted.activate_selected());

    match restarted.view().status {
        Status::Error(message) => assert!(message.contains("Cannot read"), "{message}"),
        other => panic!("expected an error, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), "not settings");
}
