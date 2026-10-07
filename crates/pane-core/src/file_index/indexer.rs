//! The indexer coordinator (#126 "Where the index lives", #175): opens the
//! file index in Pane's cache folder while a package that uses it
//! (`"fileIndex": true`) is enabled and not paused, catches it up at start,
//! walks what needs walking once the launcher was first shown (or a while
//! after start), keeps it current from the system's change source, and
//! answers searches for the packages that use it, naming each entry by an
//! id the host gives it, which it checks again before anything is done
//! with it.
//!
//! One thread of its own per activation ("pane-file-index"), at background
//! priority, does all the writing; queries read the index at the same time
//! and never wait for a walk. Disabling the last package that uses the
//! index stops watching at once and keeps the index on disk; uninstalling
//! it deletes the index ([`Indexer::delete`]).

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use super::changes::{Caught, CaughtUpBy, ChangeSource, Changed, Sink, Watching};
use super::format::{EntryKind, Meta};
use super::journal::JournalCursor;
use super::reconcile::reconcile;
use super::scope::{Admitted, Scope, ScopeRules};
use super::store::{Change, FileIndex, Hit, IndexError, IndexRecord, Opened, Query};
use super::walker::{WalkOptions, WalkReport, walk, walk_folders};
use super::{Entry, lower_current_thread};
use crate::files::{canonical, is_network_path, program_named, runs_as_program};

/// How long after start the first full walk (or a reconciling walk the
/// catch-up asks for) waits for the launcher to be shown before it starts
/// anyway.
pub const FIRST_WALK_DELAY: Duration = Duration::from_secs(60);

/// How long changes are gathered before they are applied together.
pub const SETTLE: Duration = Duration::from_millis(100);

/// How often the folders Linux cannot watch are reconciled.
pub const RECONCILE_UNWATCHED: Duration = Duration::from_secs(5 * 60);

/// The most entries one search answers.
pub const MAX_RESULTS: usize = 200;

/// The ids Pane keeps per package, the newest; an older one is no longer
/// known ("search again").
const IDS_KEPT: usize = 10_000;

/// The record of the user's file search rules, beside `installed.json`.
pub const RULES_FILE: &str = "file-search.json";

/// The index's folder in Pane's cache folder.
pub const INDEX_DIR: &str = "file-index";

/// How the indexer runs: where the index is kept, what it covers, and how
/// it learns what changed.
#[derive(Clone)]
pub struct IndexerConfig {
    /// The index's own folder, in Pane's cache folder.
    pub dir: PathBuf,
    /// The roots and rules (see [`ScopeRules`]); the user's own
    /// ([`UserRules`]) are applied over them.
    pub rules: ScopeRules,
    pub source: Arc<dyn ChangeSource>,
    /// See [`FIRST_WALK_DELAY`]; a test sets none.
    pub first_walk_delay: Duration,
    /// See [`SETTLE`].
    pub settle: Duration,
    pub walk: WalkOptions,
    /// See [`RECONCILE_UNWATCHED`].
    pub reconcile_unwatched: Duration,
}

impl IndexerConfig {
    /// This system's: the index in `cache_dir`'s `file-index`, the home
    /// folder `home` as the root, Pane's own folders (`own`, its data and
    /// cache folders) never indexed, and this system's change source.
    pub fn native(cache_dir: &Path, home: PathBuf, own: Vec<PathBuf>) -> IndexerConfig {
        let mut rules = ScopeRules::for_home(home);
        rules.always_excluded = own;
        rules.always_excluded.push(cache_dir.to_path_buf());
        IndexerConfig {
            dir: cache_dir.join(INDEX_DIR),
            rules,
            source: super::changes::native(),
            first_walk_delay: FIRST_WALK_DELAY,
            settle: SETTLE,
            walk: WalkOptions::default(),
            reconcile_unwatched: RECONCILE_UNWATCHED,
        }
    }
}

/// The rules the user changes on the File search page (#176), kept in
/// Pane's own record [`RULES_FILE`], not in any extension's data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserRules {
    /// Folders indexed besides the home folder.
    pub added_roots: Vec<PathBuf>,
    pub excluded_folders: Vec<PathBuf>,
    /// In `.gitignore` syntax.
    pub excluded_patterns: Vec<String>,
    pub include_hidden: bool,
    pub use_ignore_files: bool,
    pub default_exclusions: bool,
    pub include_other_volumes: bool,
}

impl Default for UserRules {
    fn default() -> UserRules {
        UserRules {
            added_roots: Vec::new(),
            excluded_folders: Vec::new(),
            excluded_patterns: Vec::new(),
            include_hidden: false,
            use_ignore_files: true,
            default_exclusions: true,
            include_other_volumes: false,
        }
    }
}

impl UserRules {
    /// `base` with these rules over it.
    pub fn applied_to(&self, base: &ScopeRules) -> ScopeRules {
        let mut rules = base.clone();
        for root in &self.added_roots {
            if !rules.roots.contains(root) {
                rules.roots.push(root.clone());
            }
        }
        rules.excluded_folders = self.excluded_folders.clone();
        rules.excluded_patterns = self.excluded_patterns.clone();
        rules.include_hidden = self.include_hidden;
        rules.use_ignore_files = self.use_ignore_files;
        rules.default_exclusions = self.default_exclusions;
        rules.include_other_volumes = self.include_other_volumes;
        rules
    }

    /// The rules recorded in `dir`'s [`RULES_FILE`]; the defaults when
    /// there is none or it cannot be read.
    pub fn read(dir: &Path) -> UserRules {
        #[derive(Deserialize)]
        struct Record {
            rules: UserRules,
        }
        std::fs::read(dir.join(RULES_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            .map(|record| record.rules)
            .unwrap_or_default()
    }

    /// Records these rules in `dir`'s [`RULES_FILE`].
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        let record = serde_json::json!({ "version": 1, "rules": self });
        let text = serde_json::to_vec_pretty(&record).expect("rules are JSON");
        crate::atomic::write_atomically(
            &dir.join(RULES_FILE),
            &text,
            crate::atomic::Readers::Default,
        )
        .map_err(|error| format!("Pane could not record the file search rules: {error}"))
    }
}

/// Where the index is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndexState {
    /// No enabled package uses it (or this Pane keeps none): nothing is
    /// indexed or watched.
    #[default]
    Off,
    /// Being built or caught up; what is indexed is searched meanwhile.
    Building,
    /// Current, and kept so while Pane runs.
    Current,
    /// Stopped; [`IndexStatus::reason`] says why.
    Stopped,
}

/// What the File search page, Search Files and extensions are told.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexStatus {
    pub state: IndexState,
    /// Entries in the index (an estimate while it changes).
    pub entries: u64,
    /// Entries a walk in progress has found so far.
    pub found: u64,
    /// Why it is off, waiting or stopped, or what to know about it.
    pub reason: Option<String>,
    /// Waiting for the launcher to be shown (or a while) before walking.
    pub waiting: bool,
    /// How it was last caught up, and when.
    pub caught_up: Option<(CaughtUpBy, SystemTime)>,
    /// Folders that could not be read, counted, and the first ones named.
    pub unreadable: u64,
    pub unreadable_folders: Vec<PathBuf>,
    /// Folders not watched (Linux's watch limit), reconciled every few
    /// minutes instead.
    pub unwatched: u64,
    /// A walk stopped at the ceiling of 5 million entries.
    pub ceiling_reached: bool,
}

/// How to search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub kind: Option<EntryKind>,
    pub category: Option<Category>,
    pub sort: Sort,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchOptions {
    fn default() -> SearchOptions {
        SearchOptions {
            kind: None,
            category: None,
            sort: Sort::Relevance,
            limit: 20,
            offset: 0,
        }
    }
}

/// How results are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// Best match first; for a blank query, newest first.
    #[default]
    Relevance,
    /// Most recently modified first.
    Modified,
}

/// A kind of file, told from its name's extension, the same table on
/// every system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Documents,
    Images,
    Audio,
    Video,
    Archives,
    /// Programs, scripts, shortcuts, installers and application bundles.
    Applications,
}

const DOCUMENTS: &[&str] = &[
    "pdf", "doc", "docx", "odt", "rtf", "txt", "md", "markdown", "pages", "xls", "xlsx", "ods",
    "csv", "numbers", "ppt", "pptx", "odp", "key", "epub", "tex", "html", "htm", "json", "xml",
    "yaml", "yml", "log",
];
const IMAGES: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp", "heic", "heif", "svg", "ico", "raw",
    "cr2", "nef", "arw", "dng", "psd", "ai", "avif",
];
const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "aac", "m4a", "ogg", "oga", "opus", "wma", "aiff", "aif", "mid", "midi",
];
const VIDEO: &[&str] = &[
    "mp4", "mov", "mkv", "avi", "wmv", "webm", "m4v", "mpg", "mpeg", "flv", "3gp", "ogv",
];
const ARCHIVES: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "iso", "dmg", "cab", "lz", "lzma",
];

impl Category {
    /// Whether the entry at `path`, of `kind`, is of this category.
    pub fn holds(self, path: &Path, kind: EntryKind) -> bool {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let table = match self {
            Category::Documents => DOCUMENTS,
            Category::Images => IMAGES,
            Category::Audio => AUDIO,
            Category::Video => VIDEO,
            Category::Archives => ARCHIVES,
            Category::Applications => {
                return match kind {
                    EntryKind::Folder => extension == "app",
                    EntryKind::File | EntryKind::Link => program_named(path),
                };
            }
        };
        kind != EntryKind::Folder && table.contains(&extension.as_str())
    }
}

/// An entry a search found, for an extension: the id Pane gave it and what
/// the index keeps of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub path: PathBuf,
    /// Its own name, with replacement characters where the system's name
    /// is not valid Unicode.
    pub name: String,
    /// Its folder for people: below the home folder as `~/…`.
    pub folder: String,
    pub kind: EntryKind,
    /// Whether opening it would run a program, told from its name (a
    /// program by its executable bit alone is told when it is opened).
    pub program: bool,
    pub size: u64,
    pub modified: u64,
    pub volume: u64,
}

/// An entry by the id Pane gave it, as the host names it in a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownEntry {
    pub path: PathBuf,
    pub name: String,
    pub folder: String,
    pub kind: EntryKind,
    pub program: bool,
}

/// An entry checked again, ready to act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// Its canonical path.
    pub path: PathBuf,
    pub kind: EntryKind,
    /// Opening it would run a program (its name, or on macOS and Linux its
    /// executable bit, says so).
    pub program: bool,
}

/// What the coordinator thread is told.
pub(crate) enum Message {
    Changed(Changed),
    /// The launcher was shown: a deferred walk may start.
    Shown,
    Stop,
}

/// The file index, its coordinator and the ids it gave. Cloning shares it;
/// the default is one with no configuration, which indexes nothing.
#[derive(Clone, Default)]
pub struct Indexer(Arc<Inner>);

#[derive(Default)]
struct Inner {
    shared: Mutex<Shared>,
    /// Told whenever the status or the work in flight changes.
    changed: Condvar,
    /// Messages sent and not yet handled by the coordinator.
    queued: AtomicUsize,
}

#[derive(Default)]
struct Shared {
    config: Option<IndexerConfig>,
    user_rules: UserRules,
    /// The packages that use the index and may run, by identity key.
    users: BTreeSet<String>,
    /// Whether the launcher was shown since Pane started.
    shown: bool,
    run: Option<Run>,
    index: Option<Arc<FileIndex>>,
    scope: Option<Arc<Scope>>,
    status: IndexStatus,
    /// The coordinator is doing something (catching up, walking,
    /// applying).
    busy: bool,
    /// Counts the indexes opened: an id of another one is not known.
    generation: u64,
    issued: HashMap<String, Issued>,
    /// Whether each root other than the home folder was there when last
    /// looked, and when.
    roots_seen: Option<(Instant, Vec<(PathBuf, bool)>)>,
}

impl Drop for Inner {
    /// The last handle gone (Pane quits, a test's launcher is dropped):
    /// the coordinator stops, lets go of the index and saves its cursors.
    fn drop(&mut self) {
        let shared = self
            .shared
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(run) = shared.run.take() {
            run.stop();
        }
    }
}

/// The ids given one package, the newest kept.
#[derive(Default)]
struct Issued {
    next: u64,
    by_id: HashMap<String, (PathBuf, EntryKind)>,
    order: VecDeque<String>,
}

/// One activation's coordinator thread.
struct Run {
    stop: Arc<AtomicBool>,
    sender: Sender<Message>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Run {
    /// Asks the thread to stop; it drops its watch at once.
    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.sender.send(Message::Stop);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Indexer {
    fn shared(&self) -> MutexGuard<'_, Shared> {
        lock(&self.0.shared)
    }

    /// Runs the index as `config` says from now on, with the user's
    /// `rules`; an activation in progress starts again with them.
    pub fn configure(&self, config: IndexerConfig, rules: UserRules) {
        let restart = {
            let mut shared = self.shared();
            shared.config = Some(config);
            shared.user_rules = rules;
            shared.run.is_some()
        };
        if restart {
            self.restart();
        } else {
            self.follow();
        }
    }

    /// Whether this Pane keeps a file index at all.
    pub fn configured(&self) -> bool {
        self.shared().config.is_some()
    }

    /// The packages (identity keys) that use the index and may run now: the
    /// index is kept current while there is one, and they alone may search
    /// it. Starts or stops watching as that changes.
    pub fn set_users(&self, users: BTreeSet<String>) {
        let changed = {
            let mut shared = self.shared();
            let changed = shared.users != users;
            shared.users = users;
            changed
        };
        if changed {
            self.follow();
        }
    }

    /// Notes that the launcher was shown: a deferred walk may start.
    pub fn launcher_shown(&self) {
        let mut shared = self.shared();
        if shared.shown {
            return;
        }
        shared.shown = true;
        if let Some(run) = &shared.run {
            self.0.queued.fetch_add(1, Ordering::SeqCst);
            if run.sender.send(Message::Shown).is_err() {
                self.0.queued.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Starts the coordinator if a package uses the index and none runs, or
    /// stops it if none does.
    fn follow(&self) {
        let mut shared = self.shared();
        let wanted = !shared.users.is_empty() && shared.config.is_some();
        match (wanted, shared.run.is_some()) {
            (true, false) => self.start(&mut shared),
            (false, true) => {
                if let Some(run) = shared.run.take() {
                    run.stop();
                }
                // Let go of once the coordinator has stopped: nothing of the
                // index stays open while no package uses it.
                shared.index = None;
                shared.scope = None;
                shared.generation += 1;
                shared.issued.clear();
                shared.status = IndexStatus {
                    state: IndexState::Off,
                    entries: shared.status.entries,
                    caught_up: shared.status.caught_up,
                    reason: Some("no enabled extension uses file search".into()),
                    ..IndexStatus::default()
                };
                shared.busy = false;
                self.0.changed.notify_all();
            }
            (true, true) | (false, false) => {}
        }
    }

    fn start(&self, shared: &mut Shared) {
        let Some(config) = shared.config.clone() else {
            return;
        };
        let rules = shared.user_rules.applied_to(&config.rules);
        let (sender, receiver) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        shared.status.state = IndexState::Building;
        shared.status.reason = None;
        shared.status.found = 0;
        shared.busy = true;
        let weak = Arc::downgrade(&self.0);
        let thread_stop = stop.clone();
        let thread_sender = sender.clone();
        let spawned = std::thread::Builder::new()
            .name("pane-file-index".into())
            .spawn(move || {
                lower_current_thread();
                coordinate(weak, config, rules, thread_sender, receiver, thread_stop);
            });
        match spawned {
            Ok(thread) => {
                shared.run = Some(Run {
                    stop,
                    sender,
                    thread: Some(thread),
                })
            }
            Err(error) => {
                shared.busy = false;
                shared.status.state = IndexState::Stopped;
                shared.status.reason = Some(format!("Pane could not start indexing: {error}"));
            }
        }
        self.0.changed.notify_all();
    }

    /// Stops the coordinator and waits (a few seconds at most) for its
    /// thread to end, so that the index is no longer written.
    fn stop_and_wait(&self) {
        let run = self.shared().run.take();
        if let Some(mut run) = run {
            run.stop();
            if let Some(thread) = run.thread.take() {
                let deadline = Instant::now() + Duration::from_secs(10);
                while !thread.is_finished() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                if thread.is_finished() {
                    let _ = thread.join();
                }
            }
        }
    }

    /// Starts the coordinator again, as after a change of the rules.
    fn restart(&self) {
        self.stop_and_wait();
        self.close_index();
        self.follow();
    }

    /// Lets go of the open index, so that it can be opened again or
    /// deleted; the ids it gave are no longer known.
    fn close_index(&self) {
        let mut shared = self.shared();
        shared.index = None;
        shared.scope = None;
        shared.generation += 1;
        shared.issued.clear();
    }

    /// The user's rules now.
    pub fn user_rules(&self) -> UserRules {
        self.shared().user_rules.clone()
    }

    /// The roots and rules in force now (the defaults with the user's
    /// over them); `None` when this Pane keeps no index.
    pub fn rules(&self) -> Option<ScopeRules> {
        let shared = self.shared();
        let config = shared.config.as_ref()?;
        Some(shared.user_rules.applied_to(&config.rules))
    }

    /// Applies the user's new `rules`: the index is walked again under them
    /// (only an added root is walked alone; a removed root's entries go),
    /// now if a package uses it, else when one next does (the index keeps
    /// the rules it was built under). Blocking: waits for the coordinator to
    /// stop first.
    pub fn set_user_rules(&self, rules: UserRules) {
        if self.shared().user_rules == rules {
            return;
        }
        self.stop_and_wait();
        self.shared().user_rules = rules;
        // The index's roots changed: it is opened again with them.
        self.close_index();
        self.follow();
    }

    /// Deletes the index (the last package that used it was uninstalled):
    /// stops the coordinator, waits for it, and removes the index's folder.
    /// Blocking.
    pub fn delete(&self) -> Result<(), String> {
        self.stop_and_wait();
        self.close_index();
        let dir = self
            .shared()
            .config
            .as_ref()
            .map(|config| config.dir.clone());
        {
            let mut shared = self.shared();
            shared.status = IndexStatus {
                reason: Some("no installed extension uses file search".into()),
                ..IndexStatus::default()
            };
            self.0.changed.notify_all();
        }
        match dir {
            Some(dir) if dir.exists() => std::fs::remove_dir_all(&dir)
                .map_err(|error| format!("Pane could not delete the file index: {error}")),
            _ => Ok(()),
        }
    }

    /// Deletes the index and builds it again, if a package uses it.
    /// Blocking.
    pub fn rebuild(&self) -> Result<(), String> {
        self.delete()?;
        self.follow();
        Ok(())
    }

    /// Where the index is.
    pub fn status(&self) -> IndexStatus {
        self.shared().status.clone()
    }

    /// The index's folder, if this Pane keeps one.
    pub fn dir(&self) -> Option<PathBuf> {
        self.shared()
            .config
            .as_ref()
            .map(|config| config.dir.clone())
    }

    /// Waits until the index has settled: caught up, walked, every change
    /// reported so far applied, or stopped or off. `false` if it did not
    /// within `limit`. For tests (`Launcher::wait_for_file_index`).
    pub fn wait_until_settled(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut shared = self.shared();
        loop {
            let settled = !shared.busy
                && self.0.queued.load(Ordering::SeqCst) == 0
                && shared.status.state != IndexState::Building;
            if settled {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            shared = self
                .0
                .changed
                .wait_timeout(shared, left.min(Duration::from_millis(50)))
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    /// The entries `query` finds for the package with identity key `owner`,
    /// each with the id Pane gives it. Never waits for a walk: answers from
    /// what is indexed. A blank query lists the most recently modified.
    pub fn search(
        &self,
        owner: &str,
        query: &str,
        options: SearchOptions,
    ) -> Result<Vec<Found>, String> {
        let (index, home, generation) = {
            let shared = self.shared();
            if !shared.users.contains(owner) {
                return Err("Pane's file index is for extensions whose pane.json sets \
                            \"fileIndex\": true"
                    .into());
            }
            let Some(index) = shared.index.clone() else {
                return Ok(Vec::new());
            };
            let home = shared
                .config
                .as_ref()
                .and_then(|config| config.rules.home.clone());
            (index, home, shared.generation)
        };
        let away = self.roots_away();
        let limit = options.limit.clamp(1, MAX_RESULTS);
        let wanted = options.offset.saturating_add(limit);
        let keep = |hit: &Hit| {
            options
                .category
                .is_none_or(|category| category.holds(&hit.path, hit.meta.kind))
                && !away.iter().any(|root| hit.path.starts_with(root))
        };
        let blank = query.trim().is_empty();
        // Asked for more than wanted, as entries are left out after the
        // query (a category, a root away), up to a bound.
        let mut asked = if options.category.is_some() || options.sort == Sort::Modified {
            wanted.max(500)
        } else {
            wanted
        };
        let mut hits = loop {
            let page = if blank {
                index.recent(asked, options.kind)
            } else {
                index.search(&Query {
                    text: query,
                    limit: asked,
                    offset: 0,
                    kind: options.kind,
                })
            };
            let exhausted = page.len() < asked;
            let kept: Vec<Hit> = page.into_iter().filter(keep).collect();
            if kept.len() >= wanted || exhausted || asked >= 20_000 {
                break kept;
            }
            asked *= 4;
        };
        if options.sort == Sort::Modified {
            hits.sort_by(|a, b| b.meta.modified.cmp(&a.meta.modified));
        }
        let page: Vec<Hit> = hits.into_iter().skip(options.offset).take(limit).collect();
        let mut shared = self.shared();
        if shared.generation != generation {
            // The index was closed meanwhile: its entries are not given ids.
            return Ok(Vec::new());
        }
        let issued = shared.issued.entry(owner.to_owned()).or_default();
        Ok(page
            .into_iter()
            .map(|hit| {
                let id = issue(issued, generation, &hit.path, hit.meta.kind);
                let (name, folder) = describe(&hit.path, home.as_deref());
                Found {
                    id,
                    program: hit.meta.kind != EntryKind::Folder && program_named(&hit.path),
                    name,
                    folder,
                    kind: hit.meta.kind,
                    size: hit.meta.size,
                    modified: hit.meta.modified,
                    volume: hit.meta.volume,
                    path: hit.path,
                }
            })
            .collect())
    }

    /// The roots other than the home folder that are away now (an
    /// unplugged drive): their entries are kept but not found. Looked at
    /// at most once every two seconds.
    fn roots_away(&self) -> Vec<PathBuf> {
        let roots = {
            let shared = self.shared();
            if let Some((when, seen)) = &shared.roots_seen
                && when.elapsed() < Duration::from_secs(2)
            {
                return seen
                    .iter()
                    .filter(|(_, there)| !there)
                    .map(|(root, _)| root.clone())
                    .collect();
            }
            let Some(scope) = &shared.scope else {
                return Vec::new();
            };
            let home = scope.rules().home.clone();
            scope
                .rules()
                .roots
                .iter()
                .filter(|root| Some(*root) != home.as_ref())
                .cloned()
                .collect::<Vec<_>>()
        };
        let seen: Vec<(PathBuf, bool)> = roots
            .into_iter()
            .map(|root| {
                let there = root.is_dir();
                (root, there)
            })
            .collect();
        let away = seen
            .iter()
            .filter(|(_, there)| !there)
            .map(|(root, _)| root.clone())
            .collect();
        self.shared().roots_seen = Some((Instant::now(), seen));
        away
    }

    /// The entry with id `id` that a search of the package with identity
    /// key `owner` found, as the host names it; `None` for an id Pane did
    /// not give that package from the index open now.
    pub fn known(&self, owner: &str, id: &str) -> Option<KnownEntry> {
        let shared = self.shared();
        let (path, kind) = lookup(&shared, owner, id)?;
        let home = shared
            .config
            .as_ref()
            .and_then(|config| config.rules.home.clone());
        let (name, folder) = describe(&path, home.as_deref());
        Some(KnownEntry {
            program: kind != EntryKind::Folder && program_named(&path),
            path,
            name,
            folder,
            kind,
        })
    }

    /// The entry with id `id` of the package with identity key `owner`,
    /// checked again now that Pane is to act on it: Pane gave the id from
    /// the index open now, the path is not a network path, it is still
    /// there, of the kind indexed and not a link, it is still in the index
    /// scope, and its canonical path is under a root. Blocking.
    pub fn checked(&self, owner: &str, id: &str) -> Result<Checked, String> {
        let (path, kind, scope) = {
            let shared = self.shared();
            let (path, kind) =
                lookup(&shared, owner, id).ok_or("Pane no longer knows it; search again")?;
            (path, kind, shared.scope.clone())
        };
        if is_network_path(&path) {
            return Err("it is on a network location".into());
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err("it no longer exists".into());
            }
            Err(error) => return Err(format!("Pane cannot read it: {error}")),
        };
        if metadata.file_type().is_symlink() && kind != EntryKind::Link {
            return Err("it is now a link".into());
        }
        let now = if metadata.is_dir() {
            EntryKind::Folder
        } else if metadata.file_type().is_symlink() {
            EntryKind::Link
        } else {
            EntryKind::File
        };
        match (kind, now) {
            (EntryKind::File, EntryKind::Folder) => return Err("it is now a folder".into()),
            (EntryKind::Folder, EntryKind::File) => return Err("it is now a file".into()),
            (EntryKind::Link, _) => {
                return Err("it is a link, which Pane does not follow".into());
            }
            _ => {}
        }
        if let Some(scope) = &scope {
            if !scope.admits(&path, now == EntryKind::Folder, &mut Admitted::default()) {
                return Err("it is no longer in the folders file search covers".into());
            }
            let resolved =
                canonical(&path).map_err(|error| format!("Pane cannot read it: {error}"))?;
            let inside = scope
                .rules()
                .roots
                .iter()
                .any(|root| canonical(root).is_ok_and(|root| resolved.starts_with(root)));
            if !inside {
                return Err("it is no longer inside the folders file search covers".into());
            }
            let program = now == EntryKind::File && runs_as_program(&resolved, &metadata);
            return Ok(Checked {
                path: resolved,
                kind: now,
                program,
            });
        }
        Err("file search is off".into())
    }
}

/// The path and kind of the entry with id `id` of `owner`, if Pane gave it
/// from the index open now.
fn lookup(shared: &Shared, owner: &str, id: &str) -> Option<(PathBuf, EntryKind)> {
    let generation = id
        .strip_prefix('i')?
        .split_once('-')?
        .0
        .parse::<u64>()
        .ok()?;
    if generation != shared.generation || shared.index.is_none() {
        return None;
    }
    shared.issued.get(owner)?.by_id.get(id).cloned()
}

/// Gives the entry at `path` an id for one package, keeping the newest.
fn issue(issued: &mut Issued, generation: u64, path: &Path, kind: EntryKind) -> String {
    issued.next += 1;
    let id = format!("i{generation}-{}", issued.next);
    issued.by_id.insert(id.clone(), (path.to_path_buf(), kind));
    issued.order.push_back(id.clone());
    while issued.order.len() > IDS_KEPT {
        if let Some(old) = issued.order.pop_front() {
            issued.by_id.remove(&old);
        }
    }
    id
}

/// The name of the entry at `path` and its folder for people: below the
/// home folder `home` as `~/…` (`~` itself for an entry directly in it),
/// elsewhere its full path, with `/` between names below the home folder.
pub fn describe(path: &Path, home: Option<&Path>) -> (String, String) {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let parent = path.parent().unwrap_or(path);
    let folder = match home.and_then(|home| parent.strip_prefix(home).ok()) {
        Some(below) if below.as_os_str().is_empty() => "~".to_owned(),
        Some(below) => {
            let parts: Vec<String> = below
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            format!("~/{}", parts.join("/"))
        }
        None => parent.display().to_string(),
    };
    (name, folder)
}

/// The coordinator's thread (see the module docs). Holds the indexer only
/// weakly, so that dropping every handle ends it.
fn coordinate(
    inner: Weak<Inner>,
    config: IndexerConfig,
    rules: ScopeRules,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    stop: Arc<AtomicBool>,
) {
    let Some(mut coordinator) = Coordinator::open(inner, config, rules, sender, stop) else {
        return;
    };
    coordinator.run(receiver);
}

struct Coordinator {
    inner: Weak<Inner>,
    config: IndexerConfig,
    scope: Arc<Scope>,
    index: Arc<FileIndex>,
    sender: Sender<Message>,
    stop: Arc<AtomicBool>,
    started: Instant,
    watching: Option<Box<dyn Watching>>,
    /// The cursors to save with the index next.
    cursors: Vec<JournalCursor>,
    /// Work waiting for the launcher to be shown, or the delay.
    full_walk: bool,
    walk: Vec<PathBuf>,
    reconcile: Vec<PathBuf>,
    /// Folders Linux cannot watch, and when they were last reconciled.
    unwatched: Vec<PathBuf>,
    unwatched_reconciled: Instant,
    caught_up: Option<CaughtUpBy>,
}

impl Coordinator {
    /// Opens the index and catches it up; `None` (with the status saying
    /// why) when it cannot be opened or the run was stopped.
    fn open(
        inner: Weak<Inner>,
        config: IndexerConfig,
        rules: ScopeRules,
        sender: Sender<Message>,
        stop: Arc<AtomicBool>,
    ) -> Option<Coordinator> {
        let scope = Arc::new(Scope::new(rules));
        let existing = {
            let inner = inner.upgrade()?;
            lock(&inner.shared).index.clone()
        };
        let index = match existing {
            Some(index) => index,
            None => match open_index(&config.dir, &scope.rules().roots, &stop) {
                Ok((index, opened)) => {
                    let index = Arc::new(index);
                    let inner = inner.upgrade()?;
                    let mut shared = lock(&inner.shared);
                    if stop.load(Ordering::Relaxed) {
                        return None;
                    }
                    shared.index = Some(index.clone());
                    if let Opened::Rebuilt(why) = opened {
                        shared.status.reason =
                            Some(format!("Pane rebuilds the file index, since {why}"));
                    }
                    index
                }
                Err(why) => {
                    if let Some(inner) = inner.upgrade() {
                        let mut shared = lock(&inner.shared);
                        shared.status.state = IndexState::Stopped;
                        shared.status.reason = Some(why);
                        shared.busy = false;
                        inner.changed.notify_all();
                    }
                    return None;
                }
            },
        };
        {
            let inner = inner.upgrade()?;
            let mut shared = lock(&inner.shared);
            shared.scope = Some(scope.clone());
            shared.roots_seen = None;
        }
        // Built under other rules (the user changed them, or the home
        // folder moved): brought to these first.
        let mut record = index.record();
        if record.rules.as_ref() != Some(scope.rules()) {
            if let Some(before) = &record.rules {
                apply_rule_change(&index, before, scope.rules());
                record = index.record();
            }
            record.rules = Some(scope.rules().clone());
            let _ = index.set_record(record);
        }
        if let Some(inner) = inner.upgrade() {
            lock(&inner.shared).status.entries = index.stats().stored;
        }
        let mut coordinator = Coordinator {
            inner,
            started: Instant::now(),
            cursors: Vec::new(),
            full_walk: false,
            walk: Vec::new(),
            reconcile: Vec::new(),
            unwatched: Vec::new(),
            unwatched_reconciled: Instant::now(),
            watching: None,
            caught_up: None,
            config,
            scope,
            index,
            sender,
            stop,
        };
        coordinator.catch_up();
        Some(coordinator)
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Changes the status, and tells whoever waits.
    fn status(&self, change: impl FnOnce(&mut Shared)) {
        if let Some(inner) = self.inner.upgrade() {
            let mut shared = lock(&inner.shared);
            if self.stopped() {
                return;
            }
            change(&mut shared);
            inner.changed.notify_all();
        }
    }

    /// Catches up with what changed while Pane was not running (or notes
    /// that a first walk is due), and starts watching.
    fn catch_up(&mut self) {
        let record = self.index.record();
        if !record.built {
            self.full_walk = true;
        } else {
            match self
                .config
                .source
                .catch_up(&self.index, &self.scope, &record.cursors, &self.stop)
            {
                Caught::Changes {
                    changes,
                    walk,
                    reconcile,
                    cursors,
                    how,
                    note,
                } => {
                    let _ = self.index.apply(&changes);
                    self.walk = walk;
                    self.reconcile = reconcile;
                    self.walk_added_roots();
                    self.cursors = cursors;
                    self.caught_up = Some(how);
                    if let Some(note) = note {
                        self.status(|shared| shared.status.reason = Some(note));
                    }
                }
                Caught::Reconcile(why) => {
                    self.cursors = self.config.source.cursors(&self.scope);
                    self.reconcile = self.scope.rules().roots.clone();
                    self.caught_up = Some(CaughtUpBy::ReconcilingWalk);
                    self.status(|shared| shared.status.reason = Some(why));
                }
            }
        }
        // Watching starts before any walk, so that what changes during it
        // is not lost (Linux adds its folders' watches once they are known).
        let folders = if self.full_walk {
            Vec::new()
        } else {
            self.indexed_folders()
        };
        let sink = Sink::new(self.sender.clone(), self.queued());
        match self
            .config
            .source
            .watch(&self.scope, &self.cursors, folders, sink)
        {
            Ok(watching) => self.watching = Some(watching),
            Err(why) => self.status(|shared| shared.status.reason = Some(why)),
        }
        if !self.deferred() {
            self.save();
            self.settled();
        }
    }

    /// The counter of queued messages the indexer waits on.
    fn queued(&self) -> Option<Weak<dyn Counter>> {
        self.inner
            .upgrade()
            .map(|inner| Arc::downgrade(&inner) as Weak<dyn Counter>)
    }

    /// Walks the roots that are there but not indexed yet: added since the
    /// index was built, whatever the system's records say.
    fn walk_added_roots(&mut self) {
        for root in &self.scope.rules().roots {
            if self.index.get(root).is_none()
                && root.is_dir()
                && !self.walk.contains(root)
                && !self.reconcile.contains(root)
            {
                self.walk.push(root.clone());
            }
        }
    }

    /// Every folder indexed now, shallowest first.
    fn indexed_folders(&self) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = self.index.folder_ids().into_values().collect();
        folders.sort_by_key(|folder| folder.components().count());
        folders
    }

    /// Whether walks wait to run.
    fn deferred(&self) -> bool {
        self.full_walk || !self.walk.is_empty() || !self.reconcile.is_empty()
    }

    /// Whether a deferred walk may run now: the launcher was shown, or the
    /// delay passed.
    fn may_walk(&self) -> bool {
        let shown = self
            .inner
            .upgrade()
            .is_some_and(|inner| lock(&inner.shared).shown);
        shown || self.started.elapsed() >= self.config.first_walk_delay
    }

    fn run(&mut self, receiver: Receiver<Message>) {
        loop {
            if self.stopped() {
                break;
            }
            if self.deferred() {
                if self.may_walk() {
                    self.do_walks();
                    continue;
                }
                self.status(|shared| {
                    shared.status.waiting = true;
                    shared.status.state = IndexState::Building;
                    shared.busy = false;
                });
            }
            let timeout = if self.deferred() {
                Some(
                    self.config
                        .first_walk_delay
                        .saturating_sub(self.started.elapsed())
                        .max(Duration::from_millis(10)),
                )
            } else if !self.unwatched.is_empty() {
                Some(
                    self.config
                        .reconcile_unwatched
                        .saturating_sub(self.unwatched_reconciled.elapsed())
                        .max(Duration::from_millis(10)),
                )
            } else {
                None
            };
            let first = match timeout {
                Some(timeout) => match receiver.recv_timeout(timeout) {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match receiver.recv() {
                    Ok(message) => Some(message),
                    Err(_) => break,
                },
            };
            let Some(first) = first else {
                if !self.deferred() && !self.unwatched.is_empty() {
                    self.reconcile_unwatched();
                }
                continue;
            };
            // What comes within the settle time is applied together.
            let mut batch = vec![first];
            let settle = Instant::now() + self.config.settle;
            while let Some(left) = settle.checked_duration_since(Instant::now()) {
                match receiver.recv_timeout(left) {
                    Ok(message) => batch.push(message),
                    Err(_) => break,
                }
            }
            let count = batch.len();
            let stop = self.handle(batch);
            if let Some(inner) = self.inner.upgrade() {
                inner.queued.fetch_sub(count, Ordering::SeqCst);
                inner.changed.notify_all();
            }
            if stop {
                break;
            }
        }
        // Stopped: watching stops now; what is in the log is kept, and the
        // cursors are saved once it is a segment.
        self.watching = None;
        let _ = self.index.flush();
        let mut record = self.index.record();
        if !self.cursors.is_empty() && record.built {
            record.cursors = self.cursors.clone();
            let _ = self.index.set_record(record);
        }
    }

    /// Handles a batch of messages; `true` to stop.
    fn handle(&mut self, batch: Vec<Message>) -> bool {
        let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
        let mut rescan: Vec<PathBuf> = Vec::new();
        let mut stop = false;
        for message in batch {
            match message {
                Message::Stop => stop = true,
                Message::Shown => {}
                Message::Changed(Changed::Paths(changed)) => paths.extend(changed),
                Message::Changed(Changed::Rescan(folder)) => rescan.push(folder),
                Message::Changed(Changed::HistoryDone(cursors)) => {
                    self.cursors = cursors;
                    self.caught_up = Some(CaughtUpBy::EventHistory);
                    self.save();
                }
                Message::Changed(Changed::Cursors(cursors)) => self.cursors = cursors,
                Message::Changed(Changed::Unwatched(folders)) => {
                    self.unwatched.extend(folders);
                    self.unwatched.sort();
                    self.unwatched.dedup();
                    let count = self.unwatched.len() as u64;
                    self.status(|shared| shared.status.unwatched = count);
                }
            }
        }
        if stop || self.stopped() {
            return true;
        }
        if paths.is_empty() && rescan.is_empty() {
            return false;
        }
        self.status(|shared| shared.busy = true);
        let mut changes = Vec::new();
        let mut new_folders = Vec::new();
        self.look_at(paths, &mut changes, &mut new_folders);
        if !rescan.is_empty() {
            let folders = self.rescan_folders(rescan);
            let reconciled = reconcile(
                &self.index,
                &self.scope,
                &folders,
                &self.config.walk,
                &self.stop,
            );
            changes.extend(reconciled.changes);
        }
        let walked = self.walk_new(&new_folders);
        changes.extend(walked);
        self.add_watches(&changes);
        let _ = self.index.apply(&changes);
        self.settled();
        false
    }

    /// The folders a rescan asks for, within the roots: a folder above a
    /// root rescans that root.
    fn rescan_folders(&self, asked: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = Vec::new();
        for folder in asked {
            if self.scope.root_of(&folder).is_some() {
                folders.push(folder);
            } else {
                folders.extend(
                    self.scope
                        .rules()
                        .roots
                        .iter()
                        .filter(|root| root.starts_with(&folder))
                        .cloned(),
                );
            }
        }
        folders.sort();
        folders.dedup();
        // A folder under another one asked for is covered by it.
        let mut covering: Vec<PathBuf> = Vec::new();
        for folder in folders {
            if !covering.iter().any(|outer| folder.starts_with(outer)) {
                covering.push(folder);
            }
        }
        covering
    }

    /// Looks at each changed path now: indexed if it exists and the rules
    /// admit it, removed otherwise; a folder new to the index is walked.
    fn look_at(
        &self,
        paths: BTreeSet<PathBuf>,
        changes: &mut Vec<Change>,
        new_folders: &mut Vec<PathBuf>,
    ) {
        let mut known = Admitted::default();
        for path in paths {
            if self.scope.root_of(&path).is_none() {
                continue;
            }
            let indexed = self.index.get(&path);
            let gone = |changes: &mut Vec<Change>, meta: Option<Meta>| match meta {
                Some(meta) if meta.kind == EntryKind::Folder => {
                    changes.push(Change::RemoveUnder(path.clone()))
                }
                Some(_) => changes.push(Change::Remove(path.clone())),
                None => {}
            };
            let is_root = self.scope.rules().roots.contains(&path);
            match Entry::read(&path) {
                // A root away (an unplugged drive) keeps its entries.
                Err(_) if is_root => {}
                Ok(entry) => {
                    let is_dir = entry.meta.kind == EntryKind::Folder;
                    if !self.scope.admits(&path, is_dir, &mut known) {
                        gone(changes, indexed);
                        continue;
                    }
                    match indexed {
                        Some(meta) if meta.kind == EntryKind::Folder && is_dir => {
                            changes.push(Change::Put(entry));
                        }
                        Some(meta) if is_dir || meta.kind == EntryKind::Folder => {
                            gone(changes, Some(meta));
                            if is_dir {
                                new_folders.push(path);
                            } else {
                                changes.push(Change::Put(entry));
                            }
                        }
                        _ if is_dir => new_folders.push(path),
                        _ => changes.push(Change::Put(entry)),
                    }
                }
                Err(_) => gone(changes, indexed),
            }
        }
    }

    /// Walks `folders`, new to the index, whole: their entries to put.
    fn walk_new(&self, folders: &[PathBuf]) -> Vec<Change> {
        if folders.is_empty() {
            return Vec::new();
        }
        let walked = Mutex::new(Vec::new());
        walk_folders(
            &self.scope,
            folders,
            &self.config.walk,
            &self.stop,
            &|entries: Vec<Entry>| lock(&walked).extend(entries),
        );
        walked
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .into_iter()
            .map(Change::Put)
            .collect()
    }

    /// Watches the folders `changes` put (Linux watches each folder).
    fn add_watches(&mut self, changes: &[Change]) {
        let folders: Vec<PathBuf> = changes
            .iter()
            .filter_map(|change| match change {
                Change::Put(entry) if entry.meta.kind == EntryKind::Folder => {
                    Some(entry.path.clone())
                }
                _ => None,
            })
            .collect();
        if let Some(watching) = &mut self.watching
            && !folders.is_empty()
        {
            watching.add_folders(&folders);
        }
    }

    /// Runs the walks that waited: the first full walk, then the folders
    /// to walk and to reconcile the catch-up asked for.
    fn do_walks(&mut self) {
        self.status(|shared| {
            shared.busy = true;
            shared.status.waiting = false;
            shared.status.state = IndexState::Building;
        });
        if self.full_walk {
            // Taken before the walk starts, so that what changes during it
            // is caught up next time.
            self.cursors = self.config.source.cursors(&self.scope);
            let report = self.full_walk();
            if self.stopped() {
                return;
            }
            self.full_walk = false;
            self.caught_up = Some(CaughtUpBy::FullWalk);
            let mut record = self.index.record();
            record.built = !report.cancelled;
            let _ = self.index.flush();
            let _ = self.index.set_record(record);
            let folders = self.indexed_folders();
            if let Some(watching) = &mut self.watching {
                watching.add_folders(&folders);
            }
            self.status(|shared| {
                shared.status.unreadable = report.unreadable;
                shared.status.unreadable_folders = report.unreadable_folders.clone();
                shared.status.ceiling_reached = report.ceiling_reached;
                if report.ceiling_reached {
                    shared.status.reason = Some(
                        "Pane stopped indexing at 5 million entries; exclude folders to index \
                         the rest"
                            .into(),
                    );
                }
            });
        }
        let mut changes = Vec::new();
        if !self.reconcile.is_empty() {
            let folders = std::mem::take(&mut self.reconcile);
            let reconciled = reconcile(
                &self.index,
                &self.scope,
                &folders,
                &self.config.walk,
                &self.stop,
            );
            changes.extend(reconciled.changes);
        }
        if !self.walk.is_empty() {
            let folders = std::mem::take(&mut self.walk);
            changes.extend(self.walk_new(&folders));
        }
        if self.stopped() {
            return;
        }
        self.add_watches(&changes);
        let _ = self.index.apply(&changes);
        self.save();
        self.settled();
    }

    /// The first full walk, written straight into segments; queries see
    /// each segment as it is written.
    fn full_walk(&self) -> WalkReport {
        let index = &*self.index;
        let scope = &*self.scope;
        let options = &self.config.walk;
        let (batches, received) = std::sync::mpsc::sync_channel::<super::PreparedBatch>(64);
        let inner = self.inner.clone();
        let stop = &*self.stop;
        std::thread::scope(|threads| {
            let walker = threads.spawn(move || {
                walk(scope, options, stop, &|batch: Vec<Entry>| {
                    let count = batch.len() as u64;
                    let _ = batches.send(index.prepare(batch));
                    if let Some(inner) = inner.upgrade() {
                        let mut shared = lock(&inner.shared);
                        shared.status.found += count;
                    }
                })
            });
            let mut bulk = index.bulk().ok();
            for batch in received {
                if let Some(writer) = &mut bulk
                    && writer.add(batch).is_err()
                {
                    bulk = None;
                }
            }
            let report = walker.join().unwrap_or_default();
            if let Some(writer) = bulk {
                let _ = writer.finish();
            }
            report
        })
    }

    /// Reconciles the folders Linux cannot watch.
    fn reconcile_unwatched(&mut self) {
        self.unwatched_reconciled = Instant::now();
        let folders = self.unwatched.clone();
        let reconciled = reconcile(
            &self.index,
            &self.scope,
            &folders,
            &self.config.walk,
            &self.stop,
        );
        if !reconciled.changes.is_empty() {
            let _ = self.index.apply(&reconciled.changes);
        }
        self.settled();
    }

    /// Saves the cursors with the index, once what they cover is in a
    /// segment.
    fn save(&self) {
        let _ = self.index.flush();
        let mut record: IndexRecord = self.index.record();
        if record.built && !self.cursors.is_empty() {
            record.cursors = self.cursors.clone();
            let _ = self.index.set_record(record);
        }
    }

    /// Notes that the index is current, with how it was caught up.
    fn settled(&self) {
        if self.deferred() {
            return;
        }
        let entries = self.index.stats().stored;
        let how = self.caught_up;
        self.status(|shared| {
            shared.busy = false;
            shared.status.state = IndexState::Current;
            shared.status.waiting = false;
            shared.status.entries = entries;
            shared.status.found = 0;
            if let Some(how) = how
                && shared.status.caught_up.map(|(by, _)| by) != Some(how)
            {
                shared.status.caught_up = Some((how, SystemTime::now()));
            }
        });
    }
}

/// Brings the index, built under the rules `before`, to the rules `after`:
/// a removed root's entries go; an added root alone is walked as new (the
/// catch-up finds it not indexed); any other change walks every root again.
fn apply_rule_change(index: &FileIndex, before: &ScopeRules, after: &ScopeRules) {
    let mut changes: Vec<Change> = before
        .roots
        .iter()
        .filter(|root| !after.roots.contains(root))
        .cloned()
        .map(Change::RemoveUnder)
        .collect();
    let only_roots = ScopeRules {
        roots: after.roots.clone(),
        ..before.clone()
    } == *after;
    if !only_roots {
        changes.extend(after.roots.iter().cloned().map(Change::RemoveUnder));
    }
    let _ = index.apply(&changes);
    if !only_roots {
        let mut record = index.record();
        record.built = false;
        let _ = index.flush();
        let _ = index.set_record(record);
    }
}

/// Opens the index in `dir`, trying again for a moment while the index of
/// this same Pane is still being let go (a restart of the coordinator).
fn open_index(
    dir: &Path,
    roots: &[PathBuf],
    stop: &AtomicBool,
) -> Result<(FileIndex, Opened), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match FileIndex::open(dir, roots) {
            Ok(opened) => return Ok(opened),
            Err(IndexError::InUse)
                if Instant::now() < deadline && !stop.load(Ordering::Relaxed) =>
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(IndexError::InUse) => {
                return Err("Another Pane is using file search on this computer".into());
            }
            Err(IndexError::Io(error)) => {
                return Err(format!("Pane could not open the file index: {error}"));
            }
        }
    }
}

/// What counts the messages a sink sends, for [`Indexer::wait_until_settled`].
pub(crate) trait Counter: Send + Sync {
    fn sent(&self);
    fn unsent(&self);
}

impl Counter for Inner {
    fn sent(&self) {
        self.queued.fetch_add(1, Ordering::SeqCst);
    }

    fn unsent(&self) {
        self.queued.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests;
