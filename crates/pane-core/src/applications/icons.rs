//! The installed applications' own icons (#172, ADR 0038's "an icon
//! cache"): extracted by the host at 256 pixels, one adapter per system,
//! kept in Pane's cache folder, and refreshed in the background, so that
//! root search and quick slots draw each application's icon bare (ADR
//! 0035) without typing ever waiting for one.
//!
//! - **Extraction** ([`IconExtractor`], [`NativeExtractor`]): on Windows the
//!   shell's image of the application's primary source at 256 pixels,
//!   rejecting a small icon padded into a large canvas ([`covers_enough`])
//!   for the next source (a shortcut's own icon location, its target
//!   program, the shell's file information icon); a packaged app's logo from
//!   its manifest, with its light and dark variants ([`appx`]). On macOS
//!   the workspace's icon of the bundle. On Linux the desktop entry's `Icon`
//!   in the user's icon theme, its parents and `hicolor`, then `pixmaps`
//!   ([`theme`]).
//! - **The cache** ([`IconCache`]): image files in Pane's cache folder
//!   ([`FOLDER`]), keyed by the application's id and a fingerprint of its
//!   source (its path, size and modification time), each written
//!   atomically, with an index of them. An index that cannot be read is
//!   deleted with every image and rebuilt; files the index does not name
//!   are removed. It holds at most [`MAX_BYTES`] and [`MAX_ICONS`], the
//!   least recently drawn going first. Deleting it loses nothing but the
//!   time to extract the icons again.
//! - **Refreshing**: one worker thread at low priority extracts a small
//!   batch at a time ([`BATCH`]). After each start it re-extracts every
//!   listed application's icon once; an icon a row on screen wants goes
//!   first, drawn from the cache at once when its source has not changed
//!   and extracted at once when it has. A failed extraction is remembered
//!   for the session, and the row keeps its placeholder (or the icon kept
//!   from before). Extraction never runs on the window's thread or the
//!   extension runtime's: asking what an icon shows only looks at what is
//!   kept.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::system_icons::SystemIcon;

pub mod appx;
pub mod theme;
#[cfg(windows)]
mod windows;

/// The folder in Pane's cache folder holding the applications' icons.
pub const FOLDER: &str = "application-icons";

/// The most bytes of images the cache keeps.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// The most applications whose icons the cache keeps.
pub const MAX_ICONS: usize = 10_000;

/// Up to this size, in pixels each way, an icon is drawn whatever its
/// content covers ([`covers_enough`]): the system's small sizes are drawn
/// edge to edge.
pub const CHECKED_ABOVE: u32 = 48;

/// How many icons the worker extracts before it looks again at what rows
/// on screen want.
pub const BATCH: usize = 8;

/// The alpha above which a pixel is visible content ([`covers_enough`]).
const VISIBLE_ALPHA: u8 = 16;

/// The index of the kept icons, in [`FOLDER`].
const INDEX: &str = "index.json";

/// The index's format.
const INDEX_VERSION: u32 = 1;

/// How long the worker rests between two batches refreshing in the
/// background, so that a refresh after a start never competes with what
/// the user does.
const BACKGROUND_REST: Duration = Duration::from_millis(10);

/// Whether an icon of `width` × `height` pixels of straight RGBA (row by
/// row) fills its box enough to be drawn as the application's icon: one at
/// most [`CHECKED_ABOVE`] pixels each way always does; a larger one must
/// have visible content (pixels whose alpha is above a faint 16) spanning
/// at least half its width or half its height. A small icon the system
/// padded into a large canvas, a 32-pixel image in the middle of a
/// 256-pixel square, does not: it would be drawn as a tiny picture in an
/// empty place.
pub fn covers_enough(width: u32, height: u32, rgba: &[u8]) -> bool {
    if width <= CHECKED_ABOVE && height <= CHECKED_ABOVE {
        return true;
    }
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return false;
    }
    let (mut left, mut right, mut top, mut bottom) = (width, 0, height, 0);
    for y in 0..height {
        let row = &rgba[y * width * 4..(y + 1) * width * 4];
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            if pixel[3] > VISIBLE_ALPHA {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    if left > right || top > bottom {
        // Nothing visible at all.
        return false;
    }
    (right - left + 1) * 2 >= width || (bottom - top + 1) * 2 >= height
}

/// An application's icon as its system gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    /// Drawn in the light theme, and in the dark one when there is no
    /// `dark`.
    pub light: SystemIcon,
    /// Drawn in the dark theme, when the application ships an icon of its
    /// own for it (a packaged app's dark-background logo).
    pub dark: Option<SystemIcon>,
}

impl Extracted {
    /// One icon for both themes.
    pub fn one(icon: SystemIcon) -> Extracted {
        Extracted {
            light: icon,
            dark: None,
        }
    }
}

/// Extracts the applications' icons: the system's ([`NativeExtractor`]), or
/// a test's.
pub trait IconExtractor: Send + Sync + 'static {
    /// What the source at `source` (an application's primary source path,
    /// [`super::Applications::icon_source`]) is now: when it changes, the
    /// icon kept for it is extracted again at once. `None` when it cannot
    /// be read, which extracts again on every start like an unchanged one.
    fn fingerprint(&self, source: &str) -> Option<String> {
        file_fingerprint(Path::new(source))
    }

    /// The icon of the application whose source is at `source`, or why the
    /// system has none. Blocks while the system draws it; never called on
    /// the window's or the extension runtime's thread.
    fn extract(&self, source: &str) -> Result<Extracted, String>;
}

/// The fingerprint of the file at `path`: its path, size and modification
/// time. `None` when it cannot be read.
pub fn file_fingerprint(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |at| at.as_millis());
    Some(format!(
        "{}\n{}\n{modified}",
        path.display(),
        metadata.len()
    ))
}

/// This system's extraction (see the module docs).
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeExtractor;

impl IconExtractor for NativeExtractor {
    fn fingerprint(&self, source: &str) -> Option<String> {
        platform::fingerprint(source)
    }

    fn extract(&self, source: &str) -> Result<Extracted, String> {
        platform::extract(source)
    }
}

#[cfg(windows)]
use self::windows as platform;

#[cfg(target_os = "macos")]
mod platform {
    //! macOS: the workspace's icon of the bundle (`NSWorkspace`), its
    //! largest image up to twice 256 pixels, as Finder and the Dock show
    //! it. A bundle's fingerprint is its `Info.plist`'s, which an update
    //! rewrites.
    use std::path::Path;

    use super::{Extracted, file_fingerprint};
    use crate::system_icons::{NativeIcons, SystemIcons};

    pub(super) fn fingerprint(source: &str) -> Option<String> {
        let bundle = Path::new(source);
        file_fingerprint(&bundle.join("Contents/Info.plist")).or_else(|| file_fingerprint(bundle))
    }

    pub(super) fn extract(source: &str) -> Result<Extracted, String> {
        NativeIcons.icon(Path::new(source)).map(Extracted::one)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    //! Linux: the icon a desktop entry names, looked up in the user's icon
    //! theme ([`super::theme`]).
    use std::path::Path;

    use super::theme::{IconThemes, entry_icon};
    use super::{Extracted, file_fingerprint};
    use crate::system_icons::SystemIcon;

    pub(super) fn fingerprint(source: &str) -> Option<String> {
        file_fingerprint(Path::new(source))
    }

    pub(super) fn extract(source: &str) -> Result<Extracted, String> {
        let text = std::fs::read_to_string(source)
            .map_err(|error| format!("cannot read {source}: {error}"))?;
        let name = entry_icon(&text).ok_or_else(|| format!("{source} names no icon"))?;
        IconThemes::from_env()
            .find(&name)
            .map(|file| Extracted::one(SystemIcon::File(file)))
            .ok_or_else(|| format!("the icon themes have no icon {name}"))
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    use super::Extracted;

    pub(super) fn fingerprint(_source: &str) -> Option<String> {
        None
    }

    pub(super) fn extract(source: &str) -> Result<Extracted, String> {
        Err(format!("this system gives Pane no icon for {source}"))
    }
}

/// Makes the calling thread one of low priority, for the CPU and, where
/// the system has it, the disk: the worker refreshing icons never competes
/// with what the user does.
fn lower_priority() {
    #[cfg(windows)]
    {
        use ::windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };
        // SAFETY: the pseudo handle of the calling thread, always valid.
        let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: plain system calls on the calling thread's own id.
        let _ = unsafe { libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10) };
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: a plain system call on the calling thread (`who` 0).
        let _ = unsafe { libc::setpriority(libc::PRIO_DARWIN_THREAD, 0, libc::PRIO_DARWIN_BG) };
    }
}

/// What the application whose id it is given opens: its icon's source
/// ([`super::Applications::icon_source`]). It may scan the system's
/// folders: only the worker calls it.
pub type SourceOf = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Told when icons changed: rows draw them again.
pub type Changed = Arc<dyn Fn() + Send + Sync>;

/// What an application's kept icon draws: an image file for each theme
/// (the same file when it has one for both).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub light: PathBuf,
    pub dark: PathBuf,
}

/// The applications' icons as Pane keeps them (see the module docs).
/// Cloning shares them.
#[derive(Clone)]
pub struct IconCache {
    shared: Arc<Shared>,
}

struct Shared {
    folder: PathBuf,
    extractor: Arc<dyn IconExtractor>,
    source_of: SourceOf,
    changed: Changed,
    limits: (u64, usize),
    state: Mutex<State>,
}

/// One kept icon, as the index records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    /// The fingerprint of the source it was extracted from.
    fingerprint: String,
    /// Its image for the light theme (and the dark without `dark`), a file
    /// name in the folder.
    light: String,
    dark: Option<String>,
    /// The bytes of its images.
    bytes: u64,
    /// When a row last drew it, as the cache counts ([`Index::tick`]).
    drawn: u64,
}

/// The index file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    version: u32,
    /// Moves on with every batch, so a larger `drawn` is more recent.
    tick: u64,
    icons: BTreeMap<String, Kept>,
}

/// What this start did about an application's icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Session {
    /// To be refreshed in the background.
    Queued,
    /// Wanted by a row on screen, ahead of the background.
    Wanted,
    /// Wanted, and drawn from the cache, its source unchanged; still to be
    /// refreshed in the background.
    Kept,
    /// Extracted this start.
    Refreshed,
    /// Its extraction failed this start: not tried again until the next.
    Failed,
}

#[derive(Default)]
struct State {
    /// Whether the index was read (or rebuilt).
    loaded: bool,
    index: Index,
    session: HashMap<String, Session>,
    /// Wanted by rows on screen, in the order they asked.
    urgent: VecDeque<String>,
    /// The background refresh, in the order the applications were listed.
    background: VecDeque<String>,
    /// The applications listed now: only theirs are refreshed in the
    /// background.
    listed: HashSet<String>,
    /// Whether the worker is running.
    working: bool,
    /// Whether the index changed since it was last written.
    dirty: bool,
}

/// One extraction to run, and why.
struct Job {
    id: String,
    /// Wanted by a row on screen: drawn from the cache when its source has
    /// not changed.
    urgent: bool,
}

impl IconCache {
    /// The icons kept in `folder` (Pane's cache folder's [`FOLDER`]),
    /// extracted with `extractor` from the source `source_of` gives each
    /// application, telling `changed` when what a row shows changed.
    /// Nothing is read or extracted until an icon is wanted or listed.
    pub fn new(
        folder: PathBuf,
        extractor: Arc<dyn IconExtractor>,
        source_of: SourceOf,
        changed: Changed,
    ) -> IconCache {
        IconCache {
            shared: Arc::new(Shared {
                folder,
                extractor,
                source_of,
                changed,
                limits: (MAX_BYTES, MAX_ICONS),
                state: Mutex::default(),
            }),
        }
    }

    /// These icons, keeping at most `bytes` of images and `icons`
    /// applications' icons instead of [`MAX_BYTES`] and [`MAX_ICONS`]: a
    /// test's bounds. Call it before using them.
    pub fn with_limits(self, bytes: u64, icons: usize) -> IconCache {
        let shared = &self.shared;
        IconCache {
            shared: Arc::new(Shared {
                folder: shared.folder.clone(),
                extractor: shared.extractor.clone(),
                source_of: shared.source_of.clone(),
                changed: shared.changed.clone(),
                limits: (bytes, icons),
                state: Mutex::default(),
            }),
        }
    }

    /// The folder the icons are kept in.
    pub fn folder(&self) -> &Path {
        &self.shared.folder
    }

    /// What the icon of the application `id` draws now, if one is kept;
    /// `None` while it is not (yet): the row shows its placeholder. A row
    /// on screen asks this, so the application's icon is wanted ahead of
    /// the background refresh. Never extracts or touches the disk.
    pub fn shown(&self, id: &str) -> Option<Shown> {
        let mut state = self.shared.lock();
        self.want_locked(&mut state, id);
        let tick = state.index.tick;
        let folder = &self.shared.folder;
        let kept = state.index.icons.get_mut(id)?;
        kept.drawn = tick;
        let light = folder.join(&kept.light);
        let dark = kept
            .dark
            .as_ref()
            .map_or_else(|| light.clone(), |dark| folder.join(dark));
        Some(Shown { light, dark })
    }

    /// Wants the icon of the application `id`, as a row on screen does,
    /// without asking what it draws.
    pub fn want(&self, id: &str) {
        let mut state = self.shared.lock();
        self.want_locked(&mut state, id);
    }

    fn want_locked(&self, state: &mut State, id: &str) {
        match state.session.get(id) {
            None | Some(Session::Queued) => {
                state.session.insert(id.to_owned(), Session::Wanted);
                state.urgent.push_back(id.to_owned());
                self.start_worker(state);
            }
            Some(_) => {}
        }
    }

    /// The applications listed now, by id: each one's icon not refreshed
    /// since this start is refreshed in the background, in this order;
    /// what was queued for an application no longer listed is dropped.
    pub fn listed(&self, ids: impl IntoIterator<Item = String>) {
        let mut state = self.shared.lock();
        let listed: Vec<String> = ids.into_iter().collect();
        state.listed = listed.iter().cloned().collect();
        let State {
            background,
            session,
            listed: now,
            ..
        } = &mut *state;
        background.retain(|id| now.contains(id));
        session.retain(|id, done| !matches!(done, Session::Queued) || now.contains(id));
        for id in listed {
            if !state.session.contains_key(&id) {
                state.session.insert(id.clone(), Session::Queued);
                state.background.push_back(id);
            }
        }
        if !state.background.is_empty() {
            self.start_worker(&mut state);
        }
    }

    /// Starts the worker, unless it runs.
    fn start_worker(&self, state: &mut State) {
        if state.working {
            return;
        }
        state.working = true;
        let shared = self.shared.clone();
        let started = std::thread::Builder::new()
            .name("pane-application-icons".into())
            .spawn(move || {
                lower_priority();
                shared.work();
            });
        if let Err(error) = started {
            state.working = false;
            eprintln!("pane: could not start the thread refreshing application icons: {error}");
        }
    }

    /// Waits up to `limit` until the worker has nothing left to do; whether
    /// it has not. For tests.
    pub fn wait_idle(&self, limit: Duration) -> bool {
        let deadline = std::time::Instant::now() + limit;
        loop {
            {
                let state = self.shared.lock();
                if !state.working && state.urgent.is_empty() && state.background.is_empty() {
                    return true;
                }
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The worker: reads the index, then extracts batch after batch, rows
    /// on screen first, until nothing is left.
    fn work(&self) {
        if !self.lock().loaded {
            let index = self.load();
            let mut state = self.lock();
            state.index = index;
            state.loaded = true;
            drop(state);
            // What was kept draws at once.
            (self.changed)();
        }
        loop {
            let (jobs, background) = {
                let mut state = self.lock();
                let jobs = next_batch(&mut state);
                if jobs.is_empty() {
                    state.working = false;
                    return;
                }
                let background = jobs.iter().any(|job| !job.urgent);
                (jobs, background)
            };
            for job in jobs {
                self.run(job);
            }
            {
                let mut state = self.lock();
                state.index.tick += 1;
                self.keep_within_limits(&mut state);
                if state.dirty {
                    let written = serde_json::to_vec(&state.index)
                        .map_err(|error| error.to_string())
                        .and_then(|json| {
                            crate::atomic::write_atomically(
                                &self.folder.join(INDEX),
                                &json,
                                crate::atomic::Readers::Default,
                            )
                            .map_err(|error| error.to_string())
                        });
                    match written {
                        Ok(()) => state.dirty = false,
                        Err(why) => eprintln!(
                            "pane: could not keep the application icons' index in {}: {why}",
                            self.folder.display()
                        ),
                    }
                }
            }
            (self.changed)();
            if background {
                std::thread::sleep(BACKGROUND_REST);
            }
        }
    }

    /// The index in the folder: empty when there is none; deleted with
    /// every image and rebuilt when it cannot be read. Images it does not
    /// name, and icons whose images are gone, are dropped.
    fn load(&self) -> Index {
        let path = self.folder.join(INDEX);
        let read = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Index>(&bytes)
                .ok()
                .filter(|index| index.version == INDEX_VERSION),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Index::default()),
            Err(_) => None,
        };
        let mut index = match read {
            Some(index) => index,
            None => {
                eprintln!(
                    "pane: the application icons' cache in {} could not be read; it is rebuilt",
                    self.folder.display()
                );
                let _ = std::fs::remove_dir_all(&self.folder);
                Index::default()
            }
        };
        index.version = INDEX_VERSION;
        let folder = &self.folder;
        index.icons.retain(|_, kept| {
            folder.join(&kept.light).is_file()
                && kept
                    .dark
                    .as_ref()
                    .is_none_or(|dark| folder.join(dark).is_file())
        });
        let named: HashSet<String> = index
            .icons
            .values()
            .flat_map(|kept| std::iter::once(kept.light.clone()).chain(kept.dark.clone()))
            .collect();
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name != INDEX && !named.contains(&name) && entry.path().is_file() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        index
    }

    /// Runs one extraction: keeps what it extracted, or remembers that it
    /// failed for this start.
    fn run(&self, job: Job) {
        let Some(source) = (self.source_of)(&job.id) else {
            self.lock().session.insert(job.id, Session::Failed);
            return;
        };
        let fingerprint = self.extractor.fingerprint(&source).unwrap_or_default();
        if job.urgent {
            let mut state = self.lock();
            let unchanged = state
                .index
                .icons
                .get(&job.id)
                .is_some_and(|kept| kept.fingerprint == fingerprint);
            if unchanged {
                // Drawn from the cache; refreshed in the background later.
                state.session.insert(job.id.clone(), Session::Kept);
                if !state.background.contains(&job.id) {
                    state.background.push_back(job.id);
                }
                return;
            }
        }
        let outcome = self
            .extractor
            .extract(&source)
            .and_then(|extracted| self.keep(&job.id, &fingerprint, extracted));
        let mut state = self.lock();
        match outcome {
            Ok(kept) => {
                let drawn = state
                    .index
                    .icons
                    .get(&job.id)
                    .map_or(state.index.tick, |old| old.drawn);
                let old = state
                    .index
                    .icons
                    .insert(job.id.clone(), Kept { drawn, ..kept });
                if let Some(old) = old {
                    let now = &state.index.icons[&job.id];
                    for file in std::iter::once(&old.light).chain(old.dark.as_ref()) {
                        if *file != now.light && Some(file) != now.dark.as_ref() {
                            let _ = std::fs::remove_file(self.folder.join(file));
                        }
                    }
                }
                state.dirty = true;
                state.session.insert(job.id, Session::Refreshed);
            }
            Err(why) => {
                eprintln!("pane: no icon for the application {source}: {why}");
                state.session.insert(job.id, Session::Failed);
            }
        }
    }

    /// Writes `extracted`, the icon of the application `id` from a source
    /// with `fingerprint`, into the folder.
    fn keep(&self, id: &str, fingerprint: &str, extracted: Extracted) -> Result<Kept, String> {
        let stem = crate::icons::web_image_stem(&format!("{id}\n{fingerprint}"));
        let (light, light_bytes) = self.write(&stem, extracted.light)?;
        let (dark, dark_bytes) = match extracted.dark {
            Some(dark) => {
                let (file, bytes) = self.write(&format!("{stem}-dark"), dark)?;
                (Some(file), bytes)
            }
            None => (None, 0),
        };
        Ok(Kept {
            fingerprint: fingerprint.to_owned(),
            light,
            dark,
            bytes: light_bytes + dark_bytes,
            drawn: 0,
        })
    }

    /// Writes `icon` as the image `<stem>.<its kind>` in the folder,
    /// atomically; its file name and size.
    fn write(&self, stem: &str, icon: SystemIcon) -> Result<(String, u64), String> {
        let bytes = match icon {
            SystemIcon::Png(png) => png,
            SystemIcon::File(file) => std::fs::read(&file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?,
        };
        let kind = match crate::icons::image_kind(&bytes) {
            Some(kind @ ("png" | "svg")) => kind,
            _ => return Err("it is not a PNG or SVG image".into()),
        };
        let name = format!("{stem}.{kind}");
        crate::atomic::write_atomically(
            &self.folder.join(&name),
            &bytes,
            crate::atomic::Readers::Default,
        )
        .map_err(|error| format!("cannot keep it in {}: {error}", self.folder.display()))?;
        Ok((name, bytes.len() as u64))
    }

    /// Removes the least recently drawn icons until the cache is within
    /// its bounds.
    fn keep_within_limits(&self, state: &mut State) {
        let (max_bytes, max_icons) = self.limits;
        let mut bytes: u64 = state.index.icons.values().map(|kept| kept.bytes).sum();
        while state.index.icons.len() > max_icons || bytes > max_bytes {
            let Some(oldest) = state
                .index
                .icons
                .iter()
                .min_by_key(|(id, kept)| (kept.drawn, (*id).clone()))
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            if let Some(kept) = state.index.icons.remove(&oldest) {
                bytes = bytes.saturating_sub(kept.bytes);
                for file in std::iter::once(kept.light).chain(kept.dark) {
                    let _ = std::fs::remove_file(self.folder.join(file));
                }
                state.dirty = true;
            }
        }
    }
}

/// The next jobs: what rows on screen want first, then the background
/// refresh of the applications listed, at most [`BATCH`].
fn next_batch(state: &mut State) -> Vec<Job> {
    let mut jobs = Vec::new();
    while jobs.len() < BATCH {
        if let Some(id) = state.urgent.pop_front() {
            if state.session.get(&id) == Some(&Session::Wanted) {
                jobs.push(Job { id, urgent: true });
            }
            continue;
        }
        let Some(id) = state.background.pop_front() else {
            break;
        };
        let due = matches!(
            state.session.get(&id),
            Some(Session::Queued | Session::Kept | Session::Wanted)
        );
        if due && state.listed.contains(&id) {
            jobs.push(Job { id, urgent: false });
        }
    }
    jobs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `size` × `size` RGBA image, transparent but for an opaque square
    /// of `content` pixels in its middle.
    fn padded(size: u32, content: u32) -> Vec<u8> {
        let start = (size - content) / 2;
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        for y in start..start + content {
            for x in start..start + content {
                let at = ((y * size + x) * 4) as usize;
                rgba[at..at + 4].copy_from_slice(&[200, 100, 50, 255]);
            }
        }
        rgba
    }

    #[test]
    fn a_small_icon_padded_into_a_large_canvas_does_not_cover_enough() {
        assert!(covers_enough(256, 256, &padded(256, 256)));
        assert!(covers_enough(256, 256, &padded(256, 200)));
        assert!(covers_enough(256, 256, &padded(256, 128)));
        assert!(!covers_enough(256, 256, &padded(256, 48)));
        assert!(!covers_enough(256, 256, &padded(256, 32)));
        // Nothing visible at all.
        assert!(!covers_enough(256, 256, &padded(256, 0)));
        // Small sizes are drawn whatever they hold.
        assert!(covers_enough(48, 48, &padded(48, 8)));
        assert!(covers_enough(32, 32, &padded(32, 0)));
        // A wide logo spans half the width.
        let mut wide = vec![0u8; 256 * 256 * 4];
        for x in 40..216 {
            let at = (128 * 256 + x) * 4;
            wide[at + 3] = 255;
        }
        assert!(covers_enough(256, 256, &wide));
        // Faint pixels are not content.
        let mut faint = padded(256, 32);
        for pixel in faint.chunks_exact_mut(4) {
            if pixel[3] == 0 {
                pixel[3] = 10;
            }
        }
        assert!(!covers_enough(256, 256, &faint));
        // Too few bytes for the size given.
        assert!(!covers_enough(256, 256, &[0; 16]));
    }

    #[test]
    fn a_file_s_fingerprint_changes_with_it() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("app.lnk");
        assert_eq!(file_fingerprint(&file), None);
        std::fs::write(&file, b"one").unwrap();
        let first = file_fingerprint(&file).unwrap();
        assert!(first.contains("app.lnk"), "{first}");
        std::fs::write(&file, b"three").unwrap();
        assert_ne!(file_fingerprint(&file).unwrap(), first);
    }

    #[test]
    fn rows_on_screen_go_before_the_background_and_a_batch_is_small() {
        let mut state = State::default();
        for index in 0..20 {
            let id = format!("app{index}");
            state.session.insert(id.clone(), Session::Queued);
            state.listed.insert(id.clone());
            state.background.push_back(id);
        }
        state.session.insert("app15".into(), Session::Wanted);
        state.urgent.push_back("app15".into());
        // Wanted twice: once.
        state.urgent.push_back("app15".into());
        state.session.insert("app3".into(), Session::Refreshed);
        state.listed.remove("app4");

        let batch = next_batch(&mut state);
        let ids: Vec<(&str, bool)> = batch
            .iter()
            .map(|job| (job.id.as_str(), job.urgent))
            .collect();
        assert_eq!(ids.len(), BATCH);
        assert_eq!(ids[0], ("app15", true));
        assert!(ids[1..].iter().all(|(_, urgent)| !urgent));
        // Done this start, or no longer listed: skipped.
        assert!(!ids.iter().any(|(id, _)| *id == "app3" || *id == "app4"));
    }
}
