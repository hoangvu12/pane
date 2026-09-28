//! Aliases and fallbacks through the launcher's public interface: the user
//! gives an installed command an alias, or makes it a fallback, in Manage
//! extensions, and reaches it from root search, where the text typed is sent
//! to a command that takes a query only when the user invokes it. The
//! command is a real guest: Echo, the query sample from `cargo xtask
//! guests`.

use std::fs;
use std::path::{Path, PathBuf};

use futures::executor::block_on;
use pane_core::{Launcher, Runtime, Screen, Status};
use tempfile::TempDir;

const MANAGE_ROW: &str = "Manage extensions…";

/// Copies the assembled package `name` under `target/guests/packages` to
/// `folder`.
fn package(name: &str, folder: &Path) -> PathBuf {
    let assembled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests/packages")
        .join(name);
    assert!(
        assembled.exists(),
        "{} is missing; run `cargo xtask guests`",
        assembled.display()
    );
    fs::create_dir_all(folder).unwrap();
    for entry in fs::read_dir(&assembled).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), folder.join(entry.file_name())).unwrap();
    }
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

    fn packages_dir(&self) -> PathBuf {
        self.data.path().join("extensions")
    }

    fn aliases_file(&self) -> PathBuf {
        self.packages_dir().join("aliases.json")
    }

    /// A launcher on this data folder with its own runtime, returned too;
    /// a new one is a restart of Pane.
    fn launcher(&self) -> (Launcher, Runtime) {
        let runtime = Runtime::start().unwrap();
        let launcher = Launcher::with_packages(Ok(runtime.clone()), vec![], self.packages_dir());
        (launcher, runtime)
    }

    /// Installs the package `name` from the source folder `folder` (under
    /// the sources).
    fn install(&self, launcher: &Launcher, name: &str, folder: &str) -> PathBuf {
        let folder = package(name, &self.sources.path().join(folder));
        block_on(launcher.install_package(&folder));
        assert!(
            matches!(launcher.view().status, Status::Result(_)),
            "{:?}",
            launcher.view().status
        );
        folder
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

fn selected_title(launcher: &Launcher) -> Option<String> {
    let view = launcher.view();
    view.selected.map(|index| view.rows[index].title.clone())
}

fn subtitle(launcher: &Launcher, index: usize) -> String {
    launcher.view().rows[index]
        .subtitle
        .clone()
        .unwrap_or_default()
}

fn row_subtitle(launcher: &Launcher, title: &str) -> String {
    let view = launcher.view();
    let row = view
        .rows
        .iter()
        .find(|row| row.title == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    row.subtitle.clone().unwrap_or_default()
}

fn activate(launcher: &Launcher, title: &str) {
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    launcher.select(index);
    block_on(launcher.activate_selected());
}

fn to_root(launcher: &Launcher) {
    while !matches!(launcher.view().screen, Screen::Root { .. }) {
        launcher.back();
    }
    block_on(launcher.set_query(""));
}

/// Opens the extension manager from root search.
fn manage(launcher: &Launcher) {
    to_root(launcher);
    activate(launcher, MANAGE_ROW);
    assert!(matches!(launcher.view().screen, Screen::Extensions { .. }));
}

/// In Manage extensions, submits `alias` in the alias form of the row
/// titled `row` (such as "Alias for Echo"); returns the status.
fn set_alias_at(launcher: &Launcher, row: &str, alias: &str) -> Status {
    manage(launcher);
    activate(launcher, row);
    let form = launcher.view().form().cloned().expect("the alias form");
    launcher.set_field_value(&form.fields[0].id, alias);
    block_on(launcher.submit_form());
    launcher.view().status
}

fn set_alias(launcher: &Launcher, alias: &str) -> Status {
    set_alias_at(launcher, "Alias for Echo", alias)
}

fn toggle_fallback(launcher: &Launcher) -> Status {
    manage(launcher);
    activate(launcher, "Fallback: Echo");
    launcher.view().status
}

fn search(launcher: &Launcher, query: &str) {
    to_root(launcher);
    block_on(launcher.set_query(query));
}

fn running(runtime: &Runtime) -> Vec<PathBuf> {
    block_on(runtime.running())
}

/// The component of the installed package from `folder`.
fn component_of(launcher: &Launcher, folder: &Path) -> PathBuf {
    let package = launcher
        .packages()
        .into_iter()
        .find(|package| package.identity.local_folder() == Some(folder))
        .expect("installed");
    package.location.join("sample_query.wasm")
}

#[test]
fn an_alias_finds_the_command_first_and_sends_the_text_after_it_only_when_invoked() {
    let dirs = Dirs::new();
    let (launcher, runtime) = dirs.launcher();
    let query = dirs.install(&launcher, "sample-query", "query");
    dirs.install(&launcher, "calculator", "calculator");
    let echo = component_of(&launcher, &query);

    assert_eq!(
        set_alias(&launcher, "ec"),
        Status::Result("Typing “ec” now finds Echo".into())
    );
    assert!(row_subtitle(&launcher, "Alias for Echo").starts_with("“ec” · local folder "));
    let recorded = fs::read_to_string(dirs.aliases_file()).unwrap();
    assert!(recorded.contains("#echo\": \"ec\""), "{recorded}");

    // The alias alone lists the command first; nothing runs.
    search(&launcher, "EC");
    assert_eq!(selected_title(&launcher).as_deref(), Some("Echo"));
    assert_eq!(launcher.view().selected, Some(0));
    assert!(!running(&runtime).contains(&echo));

    // The alias and text list a row that sends the text; still nothing
    // runs while typing.
    search(&launcher, "ec hello  world ");
    assert_eq!(titles(&launcher)[0], "Echo");
    assert_eq!(subtitle(&launcher, 0), "Send “hello  world” · alias ec");
    assert_eq!(launcher.view().selected, Some(0));
    assert!(!running(&runtime).contains(&echo));

    // Invoking it sends the text and shows the answer; root search stays.
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Result("Echo heard “hello  world”".into())
    );
    assert_eq!(launcher.view().query(), Some("ec hello  world "));

    // An error is shown as the failure.
    search(&launcher, "ec fail");
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Error(
            "The extension reported an error: Echo refuses “fail”, to show how an error looks"
                .into()
        )
    );

    // Kept after a restart.
    drop(launcher);
    let (launcher, _runtime) = dirs.launcher();
    search(&launcher, "ec again");
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Result("Echo heard “again”".into())
    );

    // Removed by leaving the field empty; its form starts with it.
    manage(&launcher);
    activate(&launcher, "Alias for Echo");
    assert_eq!(launcher.view().form().unwrap().fields[0].value, "ec");
    assert_eq!(
        set_alias(&launcher, ""),
        Status::Result("Echo has no alias now".into())
    );
    search(&launcher, "ec hello");
    assert!(!titles(&launcher).contains(&"Echo".to_string()));

    // What the alias names comes before a computed result too.
    set_alias(&launcher, "2+2");
    search(&launcher, "2+2");
    assert_eq!(titles(&launcher)[..2], ["Echo", "4"]);
}

#[test]
fn a_fallback_is_listed_last_for_any_text_and_is_never_chosen_by_itself() {
    let dirs = Dirs::new();
    let (launcher, runtime) = dirs.launcher();
    dirs.install(&launcher, "sample-query", "query");
    manage(&launcher);
    assert!(row_subtitle(&launcher, "Fallback: Echo").starts_with("Off · "));

    assert_eq!(
        toggle_fallback(&launcher),
        Status::Result("Echo is now offered for any text typed in root search".into())
    );
    assert!(row_subtitle(&launcher, "Fallback: Echo").starts_with("On · "));
    let recorded = fs::read_to_string(dirs.aliases_file()).unwrap();
    assert!(
        recorded.contains("\"fallbacks\": [\n    \"local:"),
        "{recorded}"
    );

    // Nothing else matches: the fallback is listed, not selected, so Enter
    // sends nothing.
    search(&launcher, "zqx words");
    assert_eq!(titles(&launcher), ["Echo"]);
    assert_eq!(subtitle(&launcher, 0), "Send “zqx words” · fallback");
    assert_eq!(launcher.view().selected, None);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().status, Status::Idle);
    assert_eq!(running(&runtime), Vec::<PathBuf>::new());

    // Choosing it sends the whole query.
    launcher.move_selection(1);
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Result("Echo heard “zqx words”".into())
    );

    // Below the matches, which stay selected first.
    search(&launcher, "manage");
    assert_eq!(titles(&launcher), [MANAGE_ROW, "Echo"]);
    assert_eq!(selected_title(&launcher).as_deref(), Some(MANAGE_ROW));
    // None for an empty query.
    search(&launcher, "");
    assert!(
        !launcher
            .view()
            .rows
            .iter()
            .any(|row| row.id.starts_with("fallback:"))
    );

    // No longer a fallback.
    assert_eq!(
        toggle_fallback(&launcher),
        Status::Result("Echo is no longer a fallback".into())
    );
    search(&launcher, "zqx words");
    assert_eq!(titles(&launcher), Vec::<String>::new());
}

#[test]
fn an_alias_that_is_not_one_word_or_is_another_commands_is_refused() {
    let dirs = Dirs::new();
    let (launcher, _runtime) = dirs.launcher();
    dirs.install(&launcher, "sample-query", "query");
    dirs.install(&launcher, "sample-settings", "settings");
    assert_eq!(
        set_alias_at(&launcher, "Alias for Greeting", "gr"),
        Status::Result("Typing “gr” now finds Greeting".into())
    );

    let status = set_alias(&launcher, "GR");
    assert_eq!(
        status,
        Status::Error(
            "Alias: “GR” is already the alias of Greeting: change it there first, or choose \
             another"
                .into()
        )
    );
    let form = launcher.view().form().cloned().expect("the form stays");
    assert!(form.fields[0].error.is_some());

    let status = set_alias(&launcher, "e c");
    assert_eq!(
        status,
        Status::Error("Alias: An alias is one word, without spaces".into())
    );
    let status = set_alias(&launcher, &"e".repeat(33));
    assert_eq!(
        status,
        Status::Error("Alias: An alias has at most 32 characters".into())
    );

    // Greeting takes no query: its alias finds it, and text after it does
    // not send anything; it has no fallback row.
    search(&launcher, "gr");
    assert_eq!(selected_title(&launcher).as_deref(), Some("Greeting"));
    search(&launcher, "gr hello");
    assert!(!titles(&launcher).contains(&"Greeting".to_string()));
    manage(&launcher);
    assert!(!titles(&launcher).contains(&"Fallback: Greeting".to_string()));
}

#[test]
fn a_conflict_in_the_record_is_shown_and_neither_alias_is_used() {
    let dirs = Dirs::new();
    let (launcher, _runtime) = dirs.launcher();
    let query = dirs.install(&launcher, "sample-query", "query");
    let settings = dirs.install(&launcher, "sample-settings", "settings");
    drop(launcher);
    let id = |folder: &Path, command: &str| {
        format!(
            "local:{}#{command}",
            fs::canonicalize(folder).unwrap().display()
        )
    };
    let record = serde_json::json!({
        "version": 1,
        "aliases": { id(&query, "echo"): "same", id(&settings, "greeting"): "SAME" },
        "fallbacks": [],
    });
    fs::write(dirs.aliases_file(), record.to_string()).unwrap();

    let (launcher, _runtime) = dirs.launcher();
    manage(&launcher);
    for row in ["Alias for Echo", "Alias for Greeting"] {
        assert!(
            row_subtitle(&launcher, row).contains("Not active: another command has the same alias"),
            "{}",
            row_subtitle(&launcher, row)
        );
    }
    search(&launcher, "same");
    assert_eq!(titles(&launcher), Vec::<String>::new());
}

#[test]
fn disabling_the_target_removes_its_alias_and_fallback_without_enabling_it_again() {
    let dirs = Dirs::new();
    let (launcher, runtime) = dirs.launcher();
    dirs.install(&launcher, "sample-query", "query");
    set_alias(&launcher, "ec");
    toggle_fallback(&launcher);

    manage(&launcher);
    activate(&launcher, "Query sample");
    assert_eq!(
        launcher.view().status,
        Status::Result("Disabled Query sample".into())
    );
    let not_active = "Not active: Query sample is disabled";
    assert!(row_subtitle(&launcher, "Alias for Echo").contains(not_active));
    assert!(row_subtitle(&launcher, "Fallback: Echo").contains(not_active));

    search(&launcher, "ec hello");
    assert_eq!(titles(&launcher), Vec::<String>::new());
    search(&launcher, "ec");
    assert_eq!(titles(&launcher), Vec::<String>::new());
    assert_eq!(running(&runtime), Vec::<PathBuf>::new());

    // Changing them does not enable it either.
    set_alias(&launcher, "echo2");
    toggle_fallback(&launcher);
    toggle_fallback(&launcher);
    assert!(!launcher.packages()[0].enabled);

    // Nor does a restart.
    drop(launcher);
    let (launcher, _runtime) = dirs.launcher();
    assert!(!launcher.packages()[0].enabled);
    search(&launcher, "echo2 hi");
    assert_eq!(titles(&launcher), Vec::<String>::new());

    // Enabled again by the user, both work again.
    manage(&launcher);
    activate(&launcher, "Query sample");
    search(&launcher, "echo2 hi");
    let ids: Vec<String> = launcher.view().rows.into_iter().map(|row| row.id).collect();
    assert!(
        ids[0].starts_with("alias:") && ids[1].starts_with("fallback:"),
        "{ids:?}"
    );
    search(&launcher, "zqx");
    assert_eq!(titles(&launcher), ["Echo"]);
}

#[test]
fn copies_from_other_sources_with_the_same_title_stay_distinct() {
    let dirs = Dirs::new();
    let (launcher, runtime) = dirs.launcher();
    let first = dirs.install(&launcher, "sample-query", "first");
    let second = dirs.install(&launcher, "sample-query", "second");
    let second_identity = launcher.packages()[1].identity.to_string();

    // The second copy's alias row, told apart by its source.
    manage(&launcher);
    let rows = launcher.view().rows;
    let index = rows
        .iter()
        .position(|row| {
            row.title == "Alias for Echo"
                && row.subtitle.as_deref().unwrap().ends_with(&second_identity)
        })
        .unwrap();
    launcher.select(index);
    block_on(launcher.activate_selected());
    launcher.set_field_value("alias", "ec");
    block_on(launcher.submit_form());

    search(&launcher, "ec");
    assert_eq!(titles(&launcher)[..2], ["Echo", "Echo"]);
    assert!(launcher.view().rows[0].id.ends_with("second#echo"));
    search(&launcher, "ec hi");
    assert_eq!(titles(&launcher), ["Echo"]);
    block_on(launcher.activate_selected());
    assert_eq!(running(&runtime), [component_of(&launcher, &second)]);
    assert!(!running(&runtime).contains(&component_of(&launcher, &first)));
}

#[test]
fn a_choice_whose_command_is_gone_is_shown_and_can_be_forgotten() {
    let dirs = Dirs::new();
    let (launcher, _runtime) = dirs.launcher();
    let query = dirs.install(&launcher, "sample-query", "query");
    drop(launcher);
    let key = format!("local:{}", fs::canonicalize(&query).unwrap().display());
    let record = serde_json::json!({
        "version": 1,
        "aliases": { format!("{key}#gone"): "gn", "local:/nowhere#echo": "nw" },
        "fallbacks": [format!("{key}#gone")],
    });
    fs::write(dirs.aliases_file(), record.to_string()).unwrap();

    let (launcher, _runtime) = dirs.launcher();
    manage(&launcher);
    let gone = "Alias “gn” and fallback of a missing command";
    assert!(
        row_subtitle(&launcher, gone)
            .starts_with("Not active: Query sample has no command `gone` now; Enter forgets it")
    );
    let elsewhere = "Alias “nw” of a missing command";
    assert!(
        row_subtitle(&launcher, elsewhere)
            .starts_with("Not active: its extension is not installed")
    );

    activate(&launcher, gone);
    assert_eq!(
        launcher.view().status,
        Status::Result("Forgot the alias and fallback of a missing command".into())
    );
    assert!(!titles(&launcher).contains(&gone.to_string()));
    let recorded = fs::read_to_string(dirs.aliases_file()).unwrap();
    assert!(!recorded.contains("#gone"), "{recorded}");
    assert!(recorded.contains("nowhere#echo"), "{recorded}");
}

#[test]
fn uninstalling_forgets_the_aliases_and_fallbacks() {
    let dirs = Dirs::new();
    let (launcher, _runtime) = dirs.launcher();
    dirs.install(&launcher, "sample-query", "query");
    set_alias(&launcher, "ec");
    toggle_fallback(&launcher);

    manage(&launcher);
    activate(&launcher, "Uninstall Query sample");
    activate(&launcher, "Uninstall and keep saved data");
    assert!(
        launcher.packages().is_empty(),
        "{:?}",
        launcher.view().status
    );
    let recorded = fs::read_to_string(dirs.aliases_file()).unwrap();
    assert!(!recorded.contains("#echo"), "{recorded}");
}

#[test]
fn a_command_declaring_a_query_without_the_interface_is_refused_at_install() {
    let dirs = Dirs::new();
    let (launcher, _runtime) = dirs.launcher();
    let folder = package("sample-settings", &dirs.sources.path().join("settings"));
    let manifest = fs::read_to_string(folder.join("pane.json")).unwrap();
    let manifest = manifest.replacen("\"component\"", "\"takesQuery\": true, \"component\"", 1);
    fs::write(folder.join("pane.json"), manifest).unwrap();
    block_on(launcher.install_package(&folder));
    let Status::Error(error) = launcher.view().status else {
        panic!("{:?}", launcher.view().status);
    };
    assert!(
        error.contains("its manifest says it takes a query, but it does not export pane:extension/query-command@0.1.0"),
        "{error}"
    );
}
