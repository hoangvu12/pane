//! File search, the Files default extension, through the launcher's public
//! interface: the folder chosen in its form, the files root search finds in
//! it, opening one with the system's handler for files (replaced here by a
//! recording fake, so nothing opens), the bounded scan policy on controlled
//! folder fixtures, and cancelling a pending search when the query changes,
//! root search is left or the extension is disabled (with a fake folder
//! lister the test holds up). The packages are the ones `cargo xtask guests`
//! assembles in `target/guests/packages`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::files::{self, FolderListing, Folders, FoundFile, Limits};
use pane_core::{Launcher, LinkOpener, PackageIdentity, Runtime, Screen, Status};
use tempfile::TempDir;

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

/// A handler that records the files it is asked to open.
#[derive(Clone, Default)]
struct FakeOpener {
    files: Arc<Mutex<Vec<PathBuf>>>,
}

impl FakeOpener {
    fn files(&self) -> Vec<PathBuf> {
        self.files.lock().unwrap().clone()
    }
}

impl LinkOpener for FakeOpener {
    fn open(&self, url: &str) -> Result<(), String> {
        panic!("no link is opened here: {url}")
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        self.files.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// The same file as Pane reported it and as the test made it.
fn same_file(reported: &Path, made: &Path) -> bool {
    fs::canonicalize(reported).unwrap() == fs::canonicalize(made).unwrap()
}

/// Pane's data location for one test, which outlives restarts.
struct Pane {
    data: TempDir,
    opener: FakeOpener,
    folders: Option<Arc<dyn Folders>>,
}

impl Pane {
    fn new() -> Pane {
        Pane {
            data: tempfile::tempdir().unwrap(),
            opener: FakeOpener::default(),
            folders: None,
        }
    }

    /// Pane whose guests list folders through `folders`.
    fn with_folders(folders: Arc<dyn Folders>) -> Pane {
        Pane {
            folders: Some(folders),
            ..Pane::new()
        }
    }

    /// Starts Pane on this data location, as after a restart.
    fn start(&self) -> (Launcher, Runtime) {
        let runtime = Runtime::start().unwrap();
        if let Some(folders) = &self.folders {
            runtime.set_folders(folders.clone());
        }
        let launcher = Launcher::with_packages(
            Ok(runtime.clone()),
            vec![],
            self.data.path().join("extensions"),
        )
        .with_link_opener(Arc::new(self.opener.clone()));
        (launcher, runtime)
    }

    /// Starts Pane with the package in `target/guests/packages/<package>`
    /// installed.
    fn with(&self, package: &str) -> (Launcher, Runtime) {
        let (launcher, runtime) = self.start();
        block_on(launcher.install_package(&built(&format!("packages/{package}"))));
        assert!(
            matches!(launcher.view().status, Status::Result(_)),
            "{:?}",
            launcher.view().status
        );
        launcher.back();
        (launcher, runtime)
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

fn search(launcher: &Launcher, query: &str) {
    block_on(launcher.set_query(query));
}

/// Opens the command titled `title` from root search, then its "Choose
/// folder" form.
fn open_choose_form(launcher: &Launcher, title: &str) {
    launcher.back();
    search(launcher, title);
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("{title} is not listed: {:?}", titles(launcher)));
    launcher.select(index);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Command);
    let index = titles(launcher)
        .iter()
        .position(|row| row == "Choose folder")
        .expect("the command offers Choose folder");
    launcher.select(index);
    block_on(launcher.activate_selected());
    assert!(
        launcher.view().form().is_some(),
        "Choose folder opens a form"
    );
}

/// Submits `folder` in the open form and returns the status.
fn submit_folder(launcher: &Launcher, folder: &str) -> Status {
    launcher.set_field_value("folder", folder);
    block_on(launcher.submit_form());
    launcher.view().status
}

/// Chooses `folder` in the command titled `title`, then returns to root
/// search.
fn choose(launcher: &Launcher, title: &str, folder: &Path) -> Status {
    open_choose_form(launcher, title);
    let status = submit_folder(launcher, folder.to_str().unwrap());
    launcher.back();
    launcher.back();
    status
}

/// A folder of controlled fixtures, with a space and non-ASCII letters in
/// its own name and in a file's:
///
/// ```text
/// Pane files — ñ/
///   Résumé plan ü.txt
///   files index.txt
///   .hidden plan.txt
///   .git/plan.txt
///   notes/plan.md
///   notes/todo.txt
/// ```
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Pane files — ñ");
        for (file, text) in [
            ("Résumé plan ü.txt", "résumé"),
            ("files index.txt", "index"),
            (".hidden plan.txt", "hidden"),
            (".git/plan.txt", "git"),
            ("notes/plan.md", "plan"),
            ("notes/todo.txt", "todo"),
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Fixture { _dir: dir, root }
    }

    fn file(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

fn never() -> bool {
    false
}

fn relatives(listing: &FolderListing) -> Vec<&str> {
    listing
        .files
        .iter()
        .map(|file| file.relative.as_str())
        .collect()
}

#[test]
fn a_listing_is_breadth_first_in_name_order_without_hidden_entries() {
    let fixture = Fixture::new();
    let listing = files::walk(&fixture.root, &Limits::default(), &never).unwrap();
    assert_eq!(
        relatives(&listing),
        [
            "Résumé plan ü.txt",
            "files index.txt",
            "notes/plan.md",
            "notes/todo.txt"
        ]
    );
    assert!(!listing.truncated);
    // Each path is the absolute path of the file listed.
    for file in &listing.files {
        assert!(Path::new(&file.path).is_absolute(), "{}", file.path);
        assert!(same_file(
            Path::new(&file.path),
            &fixture.file(&file.relative)
        ));
    }
}

#[cfg(unix)]
#[test]
fn links_are_neither_listed_nor_followed() {
    let fixture = Fixture::new();
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("outside.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), fixture.root.join("linked folder")).unwrap();
    std::os::unix::fs::symlink(
        fixture.file("notes/todo.txt"),
        fixture.root.join("linked todo.txt"),
    )
    .unwrap();
    let listing = files::walk(&fixture.root, &Limits::default(), &never).unwrap();
    assert!(
        !relatives(&listing)
            .iter()
            .any(|relative| relative.contains("linked") || relative.contains("outside")),
        "{:?}",
        relatives(&listing)
    );
}

#[cfg(windows)]
#[test]
fn hidden_attributes_and_junctions_are_skipped_on_windows() {
    use std::process::Command;
    let fixture = Fixture::new();
    let hidden = fixture.root.join("attribute plan.txt");
    fs::write(&hidden, "hidden by its attribute").unwrap();
    let status = Command::new("attrib")
        .arg("+h")
        .arg(&hidden)
        .status()
        .unwrap();
    assert!(status.success());
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("outside.txt"), "outside").unwrap();
    // A junction needs no privilege, unlike a symbolic link.
    let status = Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(fixture.root.join("junction"))
        .arg(elsewhere.path())
        .status()
        .unwrap();
    assert!(status.success());
    let listing = files::walk(&fixture.root, &Limits::default(), &never).unwrap();
    assert!(
        !relatives(&listing)
            .iter()
            .any(|relative| { relative.contains("attribute") || relative.contains("junction") }),
        "{:?}",
        relatives(&listing)
    );
}

#[test]
fn a_listing_stops_at_its_limits_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    for level in 0..4 {
        fs::write(deep.join(format!("level {level}.txt")), "").unwrap();
        deep = deep.join(format!("sub{level}"));
        fs::create_dir(&deep).unwrap();
    }
    let everything = files::walk(dir.path(), &Limits::default(), &never).unwrap();
    assert_eq!(everything.files.len(), 4);
    assert!(!everything.truncated);

    let shallow = Limits {
        depth: 2,
        ..Limits::default()
    };
    let listing = files::walk(dir.path(), &shallow, &never).unwrap();
    assert_eq!(
        relatives(&listing),
        ["level 0.txt", "sub0/level 1.txt", "sub0/sub1/level 2.txt"]
    );
    assert!(listing.truncated, "a folder deeper than the limit was left");

    let few = Limits {
        files: 2,
        ..Limits::default()
    };
    let listing = files::walk(dir.path(), &few, &never).unwrap();
    assert_eq!(listing.files.len(), 2);
    assert!(listing.truncated);

    let entries = Limits {
        entries: 3,
        ..Limits::default()
    };
    let listing = files::walk(dir.path(), &entries, &never).unwrap();
    assert_eq!(relatives(&listing), ["level 0.txt", "sub0/level 1.txt"]);
    assert!(listing.truncated);

    // Cancelled, it stops at once.
    assert!(files::walk(dir.path(), &Limits::default(), &|| true).is_err());
}

#[test]
fn a_folder_that_cannot_be_listed_is_explained() {
    let fixture = Fixture::new();
    let relative = files::walk(Path::new("notes"), &Limits::default(), &never).unwrap_err();
    assert!(
        relative.starts_with("“notes” is not a full path"),
        "{relative}"
    );
    let missing = fixture.root.join("missing");
    let error = files::walk(&missing, &Limits::default(), &never).unwrap_err();
    assert!(error.ends_with("does not exist"), "{error}");
    let file = fixture.file("notes/todo.txt");
    let error = files::walk(&file, &Limits::default(), &never).unwrap_err();
    assert!(error.ends_with("is a file, not a folder"), "{error}");
}

#[test]
fn a_chosen_folder_is_searched_and_a_found_file_opened() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");

    // No folder chosen yet: nothing is found, and nothing fails.
    search(&launcher, "plan");
    assert_eq!(titles(&launcher), Vec::<String>::new());
    assert_eq!(
        launcher.view().status,
        Status::Result("Installed Files".into()),
        "the status is untouched"
    );

    let status = choose(&launcher, "Files", &fixture.root);
    assert_eq!(
        status,
        Status::Result("Searching “Pane files — ñ”: 4 files".into())
    );

    search(&launcher, "plan");
    let view = launcher.view();
    assert_eq!(titles(&launcher), ["Résumé plan ü.txt", "plan.md"]);
    assert_eq!(
        view.rows[0].subtitle.as_deref(),
        Some("File in Pane files — ñ")
    );
    assert_eq!(
        view.rows[1].subtitle.as_deref(),
        Some("File in Pane files — ñ/notes")
    );
    assert_eq!(view.selected, Some(0));
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Result("Opened Résumé plan ü.txt".into())
    );
    let opened = pane.opener.files();
    assert_eq!(opened.len(), 1);
    assert!(same_file(&opened[0], &fixture.file("Résumé plan ü.txt")));
    assert_eq!(launcher.view().query(), Some("plan"), "root search stays");

    // Words may be in the folders below the chosen one; case is ignored.
    search(&launcher, "NOTES todo");
    assert_eq!(titles(&launcher), ["todo.txt"]);
    // Hidden files and folders are not found.
    search(&launcher, "hidden");
    assert_eq!(titles(&launcher), Vec::<String>::new());
}

#[test]
fn files_are_listed_after_the_results_found_by_title() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");
    choose(&launcher, "Files", &fixture.root);
    search(&launcher, "files");
    assert_eq!(titles(&launcher), ["Files", "files index.txt"]);
    assert_eq!(launcher.view().selected, Some(0));
}

#[test]
fn a_folder_the_form_cannot_use_is_marked_on_its_field() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");
    open_choose_form(&launcher, "Files");

    let error = |status: Status| match status {
        Status::Error(message) => message,
        other => panic!("not rejected: {other:?}"),
    };
    assert_eq!(
        error(submit_folder(&launcher, "  ")),
        "Folder: Enter the folder's full path"
    );
    let relative = error(submit_folder(&launcher, "notes"));
    assert!(
        relative.starts_with("Folder: “notes” is not a full path"),
        "{relative}"
    );
    let missing = fixture.root.join("missing");
    let message = error(submit_folder(&launcher, missing.to_str().unwrap()));
    assert!(message.ends_with("does not exist"), "{message}");
    let file = fixture.file("notes/todo.txt");
    let message = error(submit_folder(&launcher, file.to_str().unwrap()));
    assert!(message.ends_with("is a file, not a folder"), "{message}");
    assert!(launcher.view().form().is_some(), "the form stays open");

    // Nothing rejected was chosen.
    launcher.back();
    launcher.back();
    search(&launcher, "plan");
    assert_eq!(titles(&launcher), Vec::<String>::new());
}

#[test]
fn a_folder_gone_since_it_was_chosen_is_explained_in_root_search() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");
    choose(&launcher, "Files", &fixture.root);
    fs::remove_dir_all(&fixture.root).unwrap();

    search(&launcher, "plan");
    let view = launcher.view();
    assert_eq!(titles(&launcher), ["Files"]);
    let subtitle = view.rows[0].subtitle.clone().unwrap();
    assert!(
        subtitle.starts_with("Could not answer:") && subtitle.ends_with("does not exist"),
        "{subtitle}"
    );
}

#[test]
fn a_file_removed_after_it_was_found_is_explained() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");
    choose(&launcher, "Files", &fixture.root);
    search(&launcher, "todo");
    assert_eq!(titles(&launcher), ["todo.txt"]);
    fs::remove_file(fixture.file("notes/todo.txt")).unwrap();
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().status,
        Status::Error("Could not open todo.txt: it no longer exists".into())
    );
    assert!(pane.opener.files().is_empty());
}

#[test]
fn the_folder_is_kept_across_a_restart_and_disabling_removes_the_files() {
    let fixture = Fixture::new();
    let pane = Pane::new();
    let (launcher, _runtime) = pane.with("files");
    choose(&launcher, "Files", &fixture.root);
    drop(launcher);

    let (launcher, _runtime) = pane.start();
    search(&launcher, "todo");
    assert_eq!(titles(&launcher), ["todo.txt"]);

    let identity = files_identity(&launcher, "Files");
    block_on(launcher.set_enabled(&identity, false));
    search(&launcher, "tod");
    assert_eq!(titles(&launcher), Vec::<String>::new());
    block_on(launcher.set_enabled(&identity, true));
    search(&launcher, "todo");
    assert_eq!(titles(&launcher), ["todo.txt"]);
}

fn files_identity(launcher: &Launcher, title: &str) -> PackageIdentity {
    launcher
        .packages()
        .into_iter()
        .find(|package| package.title() == title)
        .expect("the package is installed")
        .identity
}

#[test]
fn the_javascript_and_typescript_samples_find_and_open_files_too() {
    let fixture = Fixture::new();
    for (package, title, language) in [
        ("sample-files-js", "Find files (JavaScript)", "JavaScript"),
        ("sample-files-ts", "Find files (TypeScript)", "TypeScript"),
    ] {
        let pane = Pane::new();
        let (launcher, _runtime) = pane.with(package);
        open_choose_form(&launcher, title);
        let relative = submit_folder(&launcher, "notes");
        assert!(
            matches!(&relative, Status::Error(message)
                if message.starts_with("Folder: “notes” is not a full path")),
            "{relative:?}"
        );
        let status = submit_folder(&launcher, fixture.root.to_str().unwrap());
        assert_eq!(
            status,
            Status::Result(format!("Searching “Pane files — ñ”: 4 files ({language})"))
        );
        launcher.back();
        launcher.back();
        search(&launcher, "résumé");
        assert_eq!(titles(&launcher), ["Résumé plan ü.txt"], "{language}");
        assert_eq!(
            launcher.view().rows[0].subtitle,
            Some(format!("File in Pane files — ñ ({language} sample)"))
        );
        block_on(launcher.activate_selected());
        assert_eq!(
            launcher.view().status,
            Status::Result("Opened Résumé plan ü.txt".into())
        );
        let opened = pane.opener.files();
        assert_eq!(opened.len(), 1, "{language}");
        assert!(same_file(&opened[0], &fixture.file("Résumé plan ü.txt")));
    }
}

/// A folder lister the test holds up: each listing waits until the test
/// lets it finish, or until it is cancelled (unless it ignores that). Its
/// n-th listing finds one file, named by `names[n]`.
struct HeldFolders {
    names: Vec<&'static str>,
    ignores_cancelling: bool,
    state: Mutex<Held>,
    changed: Condvar,
}

#[derive(Default)]
struct Held {
    started: usize,
    cancelled: usize,
    released: bool,
}

impl HeldFolders {
    fn new(names: Vec<&'static str>, ignores_cancelling: bool) -> Arc<HeldFolders> {
        Arc::new(HeldFolders {
            names,
            ignores_cancelling,
            state: Mutex::new(Held {
                // Choosing the folder lists it without waiting.
                released: true,
                ..Held::default()
            }),
            changed: Condvar::new(),
        })
    }

    fn hold(&self) {
        self.state.lock().unwrap().released = false;
    }

    fn release(&self) {
        self.state.lock().unwrap().released = true;
        self.changed.notify_all();
    }

    /// Waits until `done` holds of the state, for at most ten seconds.
    fn wait_for(&self, what: &str, done: impl Fn(&Held) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = self.state.lock().unwrap();
        while !done(&state) {
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("timed out waiting until {what}"));
            state = self.changed.wait_timeout(state, left).unwrap().0;
        }
    }

    fn started(&self) -> usize {
        self.state.lock().unwrap().started
    }

    fn cancelled(&self) -> usize {
        self.state.lock().unwrap().cancelled
    }
}

impl Folders for HeldFolders {
    fn list(&self, folder: &str, cancelled: &dyn Fn() -> bool) -> Result<FolderListing, String> {
        let mut state = self.state.lock().unwrap();
        let number = state.started;
        state.started += 1;
        self.changed.notify_all();
        loop {
            if state.released {
                break;
            }
            if cancelled() && !self.ignores_cancelling {
                state.cancelled += 1;
                self.changed.notify_all();
                return Err("cancelled".into());
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(10))
                .unwrap()
                .0;
        }
        let name = self.names[number.min(self.names.len() - 1)];
        Ok(FolderListing {
            files: vec![FoundFile {
                path: Path::new(folder).join(name).to_string_lossy().into_owned(),
                relative: name.into(),
            }],
            truncated: false,
        })
    }
}

/// Runs `launcher.set_query(query)` on a thread of its own; the receiver
/// hears once it has finished.
fn search_in_background(launcher: &Launcher, query: &str) -> mpsc::Receiver<()> {
    let searching = launcher.set_query(query);
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        block_on(searching);
        let _ = done.send(());
    });
    finished
}

fn finishes(search: &mpsc::Receiver<()>) {
    search
        .recv_timeout(Duration::from_secs(10))
        .expect("the search finished");
}

/// Pane with Files installed and a folder chosen, listed through `folders`;
/// the next listings wait for the test.
fn held(folders: &Arc<HeldFolders>) -> (Pane, Launcher, Runtime, TempDir) {
    let pane = Pane::with_folders(folders.clone());
    let (launcher, runtime) = pane.with("files");
    let folder = tempfile::tempdir().unwrap();
    let status = choose(&launcher, "Files", folder.path());
    assert!(matches!(status, Status::Result(_)), "{status:?}");
    folders.hold();
    (pane, launcher, runtime, folder)
}

#[test]
fn a_new_query_cancels_the_pending_search_whose_late_files_never_show() {
    let folders = HeldFolders::new(
        vec!["chosen.txt", "report late.txt", "report current.txt"],
        false,
    );
    let (_pane, launcher, _runtime, _folder) = held(&folders);

    let first = search_in_background(&launcher, "repor");
    folders.wait_for("the first search lists the folder", |held| {
        held.started == 2
    });
    let second = search_in_background(&launcher, "report");
    // The first search's listing is told to stop, and its search ends
    // without waiting for it.
    folders.wait_for("the first listing is cancelled", |held| held.cancelled == 1);
    finishes(&first);
    folders.wait_for("the second search lists the folder", |held| {
        held.started == 3
    });
    folders.release();
    finishes(&second);
    assert_eq!(titles(&launcher), ["report current.txt"]);
    assert_eq!(folders.cancelled(), 1);
}

#[test]
fn a_listing_that_ignores_cancelling_holds_up_nothing_and_its_answer_is_discarded() {
    let folders = HeldFolders::new(
        vec!["chosen.txt", "report late.txt", "report current.txt"],
        true,
    );
    let (_pane, launcher, _runtime, _folder) = held(&folders);

    let first = search_in_background(&launcher, "repor");
    folders.wait_for("the first search lists the folder", |held| {
        held.started == 2
    });
    let second = search_in_background(&launcher, "report");
    finishes(&first);
    // The second search reaches the extension while the first listing still
    // runs: the runtime does not wait for it.
    folders.wait_for("the second search lists the folder", |held| {
        held.started == 3
    });
    folders.release();
    finishes(&second);
    assert_eq!(titles(&launcher), ["report current.txt"]);
    // Nothing the first listing found appears later either.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(titles(&launcher), ["report current.txt"]);
}

#[test]
fn leaving_root_search_cancels_the_pending_search() {
    let folders = HeldFolders::new(vec!["chosen.txt", "report late.txt"], false);
    let (_pane, launcher, runtime, _folder) = held(&folders);

    let pending = search_in_background(&launcher, "report");
    folders.wait_for("the search lists the folder", |held| held.started == 2);
    block_on(launcher.preview_package(&built("packages/calculator")));
    assert!(matches!(launcher.view().screen, Screen::Package { .. }));
    folders.wait_for("the listing is cancelled", |held| held.cancelled == 1);
    finishes(&pending);
    // The cancelled call's instance went with it.
    let files = built("packages/files/files.wasm");
    let running = block_on(runtime.running());
    assert!(
        !running
            .iter()
            .any(|path| path.file_name() == files.file_name()),
        "{running:?}"
    );

    // Back in root search, the next query searches afresh.
    folders.release();
    launcher.back();
    search(&launcher, "report");
    assert_eq!(titles(&launcher), ["report late.txt"]);
    assert_eq!(folders.started(), 3);
}

#[test]
fn disabling_files_cancels_the_pending_search() {
    let folders = HeldFolders::new(vec!["chosen.txt", "report late.txt"], false);
    let (_pane, launcher, runtime, _folder) = held(&folders);

    let pending = search_in_background(&launcher, "report");
    folders.wait_for("the search lists the folder", |held| held.started == 2);
    let identity = files_identity(&launcher, "Files");
    block_on(launcher.set_enabled(&identity, false));
    folders.wait_for("the listing is cancelled", |held| held.cancelled == 1);
    finishes(&pending);
    assert_eq!(titles(&launcher), Vec::<String>::new());
    assert!(block_on(runtime.running()).is_empty());
    // Released now, nothing arrives.
    folders.release();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(titles(&launcher), Vec::<String>::new());
    assert_eq!(folders.started(), 2);
}
