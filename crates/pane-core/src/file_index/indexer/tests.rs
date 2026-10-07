//! The coordinator driven through its change source's seam: a fake source
//! the test scripts (what a catch-up finds) and drives (the live changes it
//! reports), over a fixture folder standing for the home folder, with the
//! real index and walker.

use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::AtomicUsize;

use super::*;
use crate::file_index::Entry;

/// A change source the test scripts and drives.
#[derive(Default)]
struct Fake {
    /// What the next catch-up answers; by default, nothing changed.
    caught: Mutex<Option<Caught>>,
    /// The latest watch's sink.
    sink: Mutex<Option<Sink>>,
    /// Watches running now.
    watching: Arc<AtomicUsize>,
    /// Catch-ups made.
    catch_ups: AtomicUsize,
}

struct FakeWatch(Arc<AtomicUsize>);

impl Watching for FakeWatch {}

impl Drop for FakeWatch {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn cursor() -> JournalCursor {
    JournalCursor {
        volume: "fake".into(),
        journal_id: 1,
        next_usn: 7,
    }
}

impl ChangeSource for Fake {
    fn cursors(&self, _scope: &Scope) -> Vec<JournalCursor> {
        vec![cursor()]
    }

    fn catch_up(
        &self,
        _index: &FileIndex,
        _scope: &Scope,
        cursors: &[JournalCursor],
        _cancel: &AtomicBool,
    ) -> Caught {
        assert_eq!(cursors, [cursor()], "the cursors saved with the index");
        self.catch_ups.fetch_add(1, Ordering::SeqCst);
        lock(&self.caught).take().unwrap_or(Caught::Changes {
            changes: Vec::new(),
            walk: Vec::new(),
            reconcile: Vec::new(),
            cursors: vec![cursor()],
            how: CaughtUpBy::Journal,
            note: None,
        })
    }

    fn watch(
        &self,
        _scope: &Scope,
        _cursors: &[JournalCursor],
        _folders: Vec<PathBuf>,
        sink: Sink,
    ) -> Result<Box<dyn Watching>, String> {
        *lock(&self.sink) = Some(sink);
        self.watching.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeWatch(self.watching.clone())))
    }
}

impl Fake {
    fn report(&self, changed: Changed) {
        lock(&self.sink)
            .as_ref()
            .expect("a watch is running")
            .send(changed)
            .expect("the coordinator listens");
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    home: PathBuf,
    index_dir: PathBuf,
    fake: Arc<Fake>,
    indexer: Indexer,
}

const OWNER: &str = "owner";
const LIMIT: Duration = Duration::from_secs(20);

fn users(owners: &[&str]) -> BTreeSet<String> {
    owners.iter().map(|owner| (*owner).to_owned()).collect()
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    for (file, text) in [
        ("Documents/plan.txt", "plan"),
        ("Documents/Invoices 2026/march.pdf", "pdf"),
        ("Music/song.mp3", "mp3"),
        (".hidden notes.txt", "hidden"),
        ("node_modules/left out.js", "js"),
    ] {
        let path = home.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let index_dir = dir.path().join("cache").join(INDEX_DIR);
    let fake = Arc::new(Fake::default());
    let indexer = Indexer::default();
    indexer.configure(
        config(&index_dir, &home, fake.clone()),
        UserRules::default(),
    );
    Fixture {
        _dir: dir,
        home,
        index_dir,
        fake,
        indexer,
    }
}

fn config(index_dir: &Path, home: &Path, fake: Arc<Fake>) -> IndexerConfig {
    IndexerConfig {
        dir: index_dir.to_path_buf(),
        rules: ScopeRules::for_home(home.to_path_buf()),
        source: fake,
        // The launcher's showing starts the walk here, never the clock.
        first_walk_delay: Duration::from_secs(3600),
        settle: Duration::from_millis(10),
        walk: WalkOptions {
            background: false,
            ..WalkOptions::default()
        },
        reconcile_unwatched: Duration::from_secs(3600),
    }
}

impl Fixture {
    /// The fixture indexed: used by a package, shown, settled.
    fn indexed() -> Fixture {
        let fixture = fixture();
        fixture.indexer.launcher_shown();
        fixture.indexer.set_users(users(&[OWNER]));
        assert!(fixture.indexer.wait_until_settled(LIMIT));
        fixture
    }

    fn names(&self, query: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .indexer
            .search(OWNER, query, SearchOptions::default())
            .unwrap()
            .into_iter()
            .map(|found| found.name)
            .collect();
        names.sort();
        names
    }

    fn settle(&self) {
        assert!(
            self.indexer.wait_until_settled(LIMIT),
            "{:?}",
            self.indexer.status()
        );
    }
}

/// Waits until `done`, at most [`LIMIT`].
fn until(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + LIMIT;
    while !done() {
        assert!(Instant::now() < deadline, "it never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_first_walk_waits_for_the_launcher_to_be_shown() {
    let fixture = fixture();
    fixture.indexer.set_users(users(&[OWNER]));
    until(|| fixture.indexer.status().waiting);
    assert_eq!(fixture.indexer.status().state, IndexState::Building);
    assert!(fixture.names("plan").is_empty(), "nothing is walked yet");
    assert!(
        !fixture
            .indexer
            .wait_until_settled(Duration::from_millis(50))
    );

    fixture.indexer.launcher_shown();
    fixture.settle();
    let status = fixture.indexer.status();
    assert_eq!(status.state, IndexState::Current);
    assert_eq!(
        status.caught_up.map(|(by, _)| by),
        Some(CaughtUpBy::FullWalk)
    );
    assert!(status.entries > 0);
    assert_eq!(fixture.names("plan"), ["plan.txt"]);
    assert_eq!(fixture.names("invoices march"), ["march.pdf"]);
    // Hidden entries and node_modules are left out by default.
    assert!(fixture.names("hidden").is_empty());
    assert!(fixture.names("left").is_empty());
}

#[test]
fn only_a_package_that_uses_the_index_may_search_it_and_ids_are_its_own() {
    let fixture = Fixture::indexed();
    assert!(
        fixture
            .indexer
            .search("other", "plan", SearchOptions::default())
            .is_err()
    );
    let found = fixture
        .indexer
        .search(OWNER, "plan", SearchOptions::default())
        .unwrap();
    let plan = &found[0];
    assert_eq!(plan.name, "plan.txt");
    assert_eq!(plan.folder, "~/Documents");
    assert_eq!(plan.kind, EntryKind::File);
    assert_eq!(plan.size, 4);
    assert!(!plan.program);
    let known = fixture.indexer.known(OWNER, &plan.id).unwrap();
    assert_eq!(known.name, "plan.txt");
    assert_eq!(fixture.indexer.known("other", &plan.id), None);
    assert_eq!(fixture.indexer.known(OWNER, "i99-1"), None);
    let checked = fixture.indexer.checked(OWNER, &plan.id).unwrap();
    assert_eq!(
        checked.path,
        canonical(&fixture.home.join("Documents/plan.txt")).unwrap()
    );
    assert!(fixture.indexer.checked("other", &plan.id).is_err());

    // Changed since it was found: explained, never acted on.
    fs::remove_file(fixture.home.join("Documents/plan.txt")).unwrap();
    assert_eq!(
        fixture.indexer.checked(OWNER, &plan.id),
        Err("it no longer exists".into())
    );
    fs::create_dir(fixture.home.join("Documents/plan.txt")).unwrap();
    assert_eq!(
        fixture.indexer.checked(OWNER, &plan.id),
        Err("it is now a folder".into())
    );
}

#[test]
fn folders_kinds_categories_sorting_and_pages() {
    let fixture = Fixture::indexed();
    let search = |query: &str, options: SearchOptions| -> Vec<String> {
        fixture
            .indexer
            .search(OWNER, query, options)
            .unwrap()
            .into_iter()
            .map(|found| found.name)
            .collect()
    };
    let folders = SearchOptions {
        kind: Some(EntryKind::Folder),
        ..SearchOptions::default()
    };
    assert_eq!(search("invoices", folders), ["Invoices 2026"]);
    let audio = SearchOptions {
        category: Some(Category::Audio),
        ..SearchOptions::default()
    };
    assert_eq!(search("", audio), ["song.mp3"]);
    // A blank query lists the newest first.
    fs::write(fixture.home.join("Music/newest.txt"), "new").unwrap();
    fixture
        .fake
        .report(Changed::Paths(vec![fixture.home.join("Music/newest.txt")]));
    fixture.settle();
    let recent = search("", SearchOptions::default());
    assert_eq!(recent.first().map(String::as_str), Some("newest.txt"));
    // Pages follow each other.
    let first = search(
        "",
        SearchOptions {
            limit: 2,
            ..SearchOptions::default()
        },
    );
    let second = search(
        "",
        SearchOptions {
            limit: 2,
            offset: 2,
            ..SearchOptions::default()
        },
    );
    assert_eq!(first.len(), 2);
    assert!(second.iter().all(|name| !first.contains(name)));
}

#[test]
fn live_changes_are_found_once_applied() {
    let fixture = Fixture::indexed();
    let home = &fixture.home;
    // Created.
    fs::write(home.join("Documents/Résumé.txt"), "cv").unwrap();
    fixture
        .fake
        .report(Changed::Paths(vec![home.join("Documents/Résumé.txt")]));
    fixture.settle();
    assert_eq!(fixture.names("resume"), ["Résumé.txt"]);
    // A folder moved in, with files in it, is walked.
    let outside = fixture._dir.path().join("outside");
    fs::create_dir_all(outside.join("deep")).unwrap();
    fs::write(outside.join("deep/brought.txt"), "x").unwrap();
    fs::rename(&outside, home.join("Projects")).unwrap();
    fixture
        .fake
        .report(Changed::Paths(vec![home.join("Projects")]));
    fixture.settle();
    assert_eq!(fixture.names("brought"), ["brought.txt"]);
    // Renamed: the old name gone with everything under it.
    fs::rename(home.join("Projects"), home.join("Work")).unwrap();
    fixture.fake.report(Changed::Paths(vec![
        home.join("Projects"),
        home.join("Work"),
    ]));
    fixture.settle();
    let found = fixture
        .indexer
        .search(OWNER, "brought", SearchOptions::default())
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].folder, "~/Work/deep");
    // Deleted.
    fs::remove_file(home.join("Documents/Résumé.txt")).unwrap();
    fixture
        .fake
        .report(Changed::Paths(vec![home.join("Documents/Résumé.txt")]));
    fixture.settle();
    assert!(fixture.names("resume").is_empty());
    // A hidden file is reported but not indexed.
    fs::write(home.join(".secret plan"), "x").unwrap();
    fixture
        .fake
        .report(Changed::Paths(vec![home.join(".secret plan")]));
    fixture.settle();
    assert_eq!(fixture.names("plan"), ["plan.txt"]);
}

#[cfg(unix)]
#[test]
fn an_entry_replaced_by_a_link_since_it_was_found_is_refused() {
    let fixture = Fixture::indexed();
    let found = fixture
        .indexer
        .search(OWNER, "plan", SearchOptions::default())
        .unwrap();
    let plan = fixture.home.join("Documents/plan.txt");
    let elsewhere = fixture._dir.path().join("elsewhere.txt");
    fs::write(&elsewhere, "x").unwrap();
    fs::remove_file(&plan).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &plan).unwrap();
    assert_eq!(
        fixture.indexer.checked(OWNER, &found[0].id),
        Err("it is now a link".into())
    );
}

#[test]
fn folders_the_source_cannot_watch_are_counted_and_reconciled_every_few_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join("Deep/Deeper")).unwrap();
    let fake = Arc::new(Fake::default());
    let indexer = Indexer::default();
    indexer.configure(
        IndexerConfig {
            // Reconciled at once, as the few minutes are up.
            reconcile_unwatched: Duration::from_millis(50),
            ..config(
                &dir.path().join("cache").join(INDEX_DIR),
                &home,
                fake.clone(),
            )
        },
        UserRules::default(),
    );
    indexer.launcher_shown();
    indexer.set_users(users(&[OWNER]));
    assert!(indexer.wait_until_settled(LIMIT));
    // Linux's watch limit reached: the source says which folders it could
    // not watch.
    let deeper = home.join("Deep/Deeper");
    fake.report(Changed::Unwatched(vec![deeper.clone()]));
    until(|| indexer.status().unwatched == 1);
    // A change there is reported by nothing, and found all the same.
    fs::write(deeper.join("unreported plan.txt"), "x").unwrap();
    touch(&deeper);
    until(|| {
        indexer
            .search(OWNER, "unreported", SearchOptions::default())
            .is_ok_and(|found| found.len() == 1)
    });
}

#[test]
fn an_overflow_reconciles_the_folder() {
    let fixture = Fixture::indexed();
    let documents = fixture.home.join("Documents");
    fs::write(documents.join("unreported.txt"), "x").unwrap();
    fs::remove_file(documents.join("plan.txt")).unwrap();
    touch(&documents);
    fixture.fake.report(Changed::Rescan(documents));
    fixture.settle();
    assert_eq!(fixture.names("unreported"), ["unreported.txt"]);
    assert!(fixture.names("plan").is_empty());
}

#[test]
fn disabling_stops_watching_and_enabling_again_catches_up_without_walking() {
    let fixture = Fixture::indexed();
    assert_eq!(fixture.fake.watching.load(Ordering::SeqCst), 1);
    fixture.indexer.set_users(BTreeSet::new());
    assert_eq!(fixture.indexer.status().state, IndexState::Off);
    until(|| fixture.fake.watching.load(Ordering::SeqCst) == 0);
    assert!(fixture.index_dir.exists(), "the index stays on disk");

    // Changed while off: not found until the index is caught up.
    let new = fixture.home.join("Documents/while off.txt");
    fs::write(&new, "x").unwrap();
    *lock(&fixture.fake.caught) = Some(Caught::Changes {
        changes: vec![Change::Put(Entry::read(&new).unwrap())],
        walk: Vec::new(),
        reconcile: Vec::new(),
        cursors: vec![cursor()],
        how: CaughtUpBy::Journal,
        note: None,
    });
    fixture.indexer.set_users(users(&[OWNER]));
    fixture.settle();
    assert_eq!(fixture.names("while off"), ["while off.txt"]);
    assert_eq!(
        fixture.indexer.status().caught_up.map(|(by, _)| by),
        Some(CaughtUpBy::Journal)
    );
    assert_eq!(fixture.fake.catch_ups.load(Ordering::SeqCst), 1);
}

#[test]
fn records_that_are_gone_lead_to_a_reconciling_walk_with_the_same_result() {
    let fixture = Fixture::indexed();
    fixture.indexer.set_users(BTreeSet::new());
    until(|| fixture.fake.watching.load(Ordering::SeqCst) == 0);
    fs::write(fixture.home.join("Music/missed.mp3"), "x").unwrap();
    touch(&fixture.home.join("Music"));
    *lock(&fixture.fake.caught) = Some(Caught::Reconcile("the journal was recreated".into()));
    fixture.indexer.set_users(users(&[OWNER]));
    fixture.settle();
    assert_eq!(fixture.names("missed"), ["missed.mp3"]);
    let status = fixture.indexer.status();
    assert_eq!(
        status.caught_up.map(|(by, _)| by),
        Some(CaughtUpBy::ReconcilingWalk)
    );
}

#[test]
fn a_restart_over_the_same_cache_folder_catches_up() {
    let fixture = Fixture::indexed();
    let (index_dir, home) = (fixture.index_dir.clone(), fixture.home.clone());
    fixture.indexer.set_users(BTreeSet::new());
    drop(fixture.indexer);
    // Another start of Pane, as after a restart.
    let fake = Arc::new(Fake::default());
    let indexer = Indexer::default();
    // Its index may be let go by the old coordinator a moment later.
    indexer.configure(
        config(&index_dir, &home, fake.clone()),
        UserRules::default(),
    );
    indexer.set_users(users(&[OWNER]));
    assert!(indexer.wait_until_settled(LIMIT), "{:?}", indexer.status());
    assert_eq!(
        fake.catch_ups.load(Ordering::SeqCst),
        1,
        "caught up, not walked"
    );
    let found = indexer
        .search(OWNER, "plan", SearchOptions::default())
        .unwrap();
    assert_eq!(found.len(), 1);
}

#[test]
fn a_second_pane_on_the_same_cache_folder_does_not_index() {
    let fixture = Fixture::indexed();
    let second = Indexer::default();
    second.configure(
        config(&fixture.index_dir, &fixture.home, Arc::new(Fake::default())),
        UserRules::default(),
    );
    second.set_users(users(&[OWNER]));
    until(|| second.status().state == IndexState::Stopped);
    assert_eq!(
        second.status().reason.as_deref(),
        Some("Another Pane is using file search on this computer")
    );
}

#[test]
fn deleting_the_index_removes_it_and_forgets_its_ids() {
    let fixture = Fixture::indexed();
    let found = fixture
        .indexer
        .search(OWNER, "plan", SearchOptions::default())
        .unwrap();
    fixture.indexer.set_users(BTreeSet::new());
    fixture.indexer.delete().unwrap();
    assert!(!fixture.index_dir.exists());
    assert_eq!(fixture.indexer.known(OWNER, &found[0].id), None);
    assert!(fixture.indexer.checked(OWNER, &found[0].id).is_err());
}

#[test]
fn the_users_rules_change_what_is_indexed() {
    let fixture = Fixture::indexed();
    assert!(fixture.names("hidden").is_empty());
    let extra = fixture._dir.path().join("Second drive");
    fs::create_dir_all(&extra).unwrap();
    fs::write(extra.join("far plan.txt"), "x").unwrap();
    fixture.indexer.set_user_rules(UserRules {
        include_hidden: true,
        added_roots: vec![extra.clone()],
        excluded_folders: vec![fixture.home.join("Music")],
        ..UserRules::default()
    });
    fixture.settle();
    assert_eq!(fixture.names("hidden"), [".hidden notes.txt"]);
    assert_eq!(fixture.names("far"), ["far plan.txt"]);
    assert!(fixture.names("song").is_empty());

    // A root removed takes its entries with it.
    fixture.indexer.set_user_rules(UserRules {
        include_hidden: true,
        ..UserRules::default()
    });
    fixture.settle();
    assert!(fixture.names("far").is_empty());
    assert_eq!(fixture.names("song"), ["song.mp3"]);
}

#[test]
fn an_added_root_away_is_hidden_and_back_when_it_returns() {
    let fixture = Fixture::indexed();
    let drive = fixture._dir.path().join("USB stick");
    fs::create_dir_all(&drive).unwrap();
    fs::write(drive.join("holiday.jpg"), "x").unwrap();
    fixture.indexer.set_user_rules(UserRules {
        added_roots: vec![drive.clone()],
        ..UserRules::default()
    });
    fixture.settle();
    assert_eq!(fixture.names("holiday"), ["holiday.jpg"]);
    let unplugged = fixture._dir.path().join("unplugged");
    fs::rename(&drive, &unplugged).unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    assert!(fixture.names("holiday").is_empty());
    fs::rename(&unplugged, &drive).unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    assert_eq!(fixture.names("holiday"), ["holiday.jpg"]);
}

#[test]
fn categories_and_folders_for_people() {
    let path = Path::new("/home/me/Pictures/cat.JPG");
    assert!(Category::Images.holds(path, EntryKind::File));
    assert!(!Category::Images.holds(path, EntryKind::Folder));
    assert!(Category::Applications.holds(Path::new("/x/setup.exe"), EntryKind::File));
    assert!(Category::Applications.holds(Path::new("/x/Pane.app"), EntryKind::Folder));
    assert!(Category::Archives.holds(Path::new("/x/a.tar.gz"), EntryKind::File));
    assert!(!Category::Documents.holds(Path::new("/x/song.mp3"), EntryKind::File));
    // Text and Other (#177): plain text is not a document, and a file of no
    // category is Other; a folder is never Other.
    assert!(Category::Text.holds(Path::new("/x/notes.md"), EntryKind::File));
    assert!(Category::Text.holds(Path::new("/x/main.rs"), EntryKind::File));
    assert!(!Category::Documents.holds(Path::new("/x/notes.txt"), EntryKind::File));
    assert!(Category::Documents.holds(Path::new("/x/report.PDF"), EntryKind::File));
    assert!(Category::Other.holds(Path::new("/x/data.bin"), EntryKind::File));
    assert!(Category::Other.holds(Path::new("/x/README"), EntryKind::File));
    assert!(!Category::Other.holds(Path::new("/x/cat.png"), EntryKind::File));
    assert!(!Category::Other.holds(Path::new("/x/Projects"), EntryKind::Folder));
    assert_eq!(
        Category::of(Path::new("/x/setup.exe"), EntryKind::File),
        Some(Category::Applications)
    );
    assert_eq!(
        Category::of(Path::new("/x/notes.txt"), EntryKind::File),
        Some(Category::Text)
    );
    assert_eq!(
        Category::of(Path::new("/x/data.bin"), EntryKind::File),
        Some(Category::Other)
    );
    assert_eq!(
        Category::of(Path::new("/x/Projects"), EntryKind::Folder),
        None
    );

    let home = Path::new("/home/me");
    assert_eq!(
        describe(Path::new("/home/me/notes.txt"), Some(home)),
        ("notes.txt".to_owned(), "~".to_owned())
    );
    assert_eq!(
        describe(
            &home.join("Documents").join("Invoices").join("a.pdf"),
            Some(home)
        ),
        ("a.pdf".to_owned(), "~/Documents/Invoices".to_owned())
    );
}

/// Sets `folder`'s modified time a minute later, so that a reconciling
/// walk sees it changed whatever the clock's resolution.
fn touch(folder: &Path) {
    let time = fs::metadata(folder).unwrap().modified().unwrap() + Duration::from_secs(60);
    let mut options = fs::OpenOptions::new();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.access_mode(0x100).custom_flags(0x0200_0000);
    }
    #[cfg(not(windows))]
    options.read(true);
    options
        .open(folder)
        .and_then(|file| file.set_modified(time))
        .expect("the folder's time is set");
}
