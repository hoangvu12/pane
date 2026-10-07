//! The applications installed on the system: finding them, knowing which
//! application each is, and opening them.
//!
//! A WASI guest cannot see or start the system's applications, so Pane's host
//! does it for the Applications default extension through the
//! `pane:extension/applications` import ([`Applications`]). Each system has
//! its own adapter, a [`Discovery`] chosen by [`native`], which reports the
//! [`Source`]s it finds:
//!
//! - Windows: the shortcuts in the Start menu's Programs folders and the
//!   packaged apps of the Apps folder ([`StartMenu`]);
//! - macOS: the application bundles in the Applications folders ([`AppBundles`]);
//! - Linux: the XDG desktop entries in the `applications` data folders
//!   ([`DesktopEntries`]).
//!
//! The host turns those sources into applications with a stable identity
//! ([`identity`], ADR 0038): sources with one [`Key`] are one application,
//! whose id is a digest of the key, and the host keeps the map from each id
//! to the source that opens it ([`Cached`]), so an id survives an update that
//! moves the program, and an id from before identities (a source's path)
//! still finds its application.
//!
//! Finding is plain file system work and is compiled on every system, so each
//! adapter's discovery is tested everywhere with fixture folders; opening uses
//! the system's own launcher and works only on its system.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

mod app_bundles;
mod cached;
mod desktop_entries;
pub mod identity;
mod plist;
mod start_menu;

pub use app_bundles::AppBundles;
pub use cached::Cached;
pub use desktop_entries::DesktopEntries;
pub use identity::{Catalog, Identified, Key, Source};
pub use start_menu::{Place, Shortcut, ShortcutFolder, StartMenu};

/// An installed application, as an extension receives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    /// Its stable id ([`Key::id`]), which identifies it to
    /// [`Applications::open`]: opaque, and the same across updates and
    /// restarts.
    pub id: String,
    /// Its name, as the system shows it.
    pub name: String,
    /// Where it was found, for people.
    pub location: String,
}

/// Finds and opens the system's installed applications.
pub trait Applications: Send + Sync + 'static {
    /// The installed applications, in no particular order. A location that is
    /// missing or cannot be read adds none.
    fn installed(&self) -> Result<Vec<Application>, String>;

    /// Opens (launches) the application `id`, as [`Applications::installed`]
    /// gave it or as it was given before identities (the path of its
    /// shortcut, bundle or desktop entry), without waiting for it to finish.
    /// An error explains why the system did not open it.
    fn open(&self, id: &str) -> Result<(), String>;

    /// What the system opens for the application `id` (current, or a path
    /// from before identities): its primary source's path, which the
    /// system's opener takes as the application to open something with.
    /// `None` when `id` names no application this host knows. It may scan
    /// the system's folders, so call it off the window's thread.
    fn source(&self, _id: &str) -> Option<String> {
        None
    }

    /// The current id of the application `id` names, when `id` is another
    /// id for it, such as the path its id was before identities. It looks
    /// only at the list already kept and never scans, so it may be called
    /// anywhere; `None` when no list is kept or `id` names nothing in it.
    fn current_id(&self, _id: &str) -> Option<String> {
        None
    }
}

/// What the system opens for `application`, an installed application's id
/// or a path: the application's source when `applications` knows it, else
/// `application` as it is.
pub fn opener(applications: &dyn Applications, application: &str) -> String {
    applications
        .source(application)
        .unwrap_or_else(|| application.to_owned())
}

/// One system's way of finding its applications: an adapter, which the
/// host's list ([`Cached`]) turns into applications by identity.
pub trait Discovery: Send + Sync + 'static {
    /// Every source of an application found, in a deterministic order. A
    /// location that is missing or cannot be read adds none.
    fn sources(&self) -> Result<Vec<Source>, String>;

    /// Opens (launches) the source at `path` ([`Source::path`]), without
    /// waiting for it to finish. An error explains why the system did not
    /// open it.
    fn open(&self, path: &str) -> Result<(), String>;
}

/// How old the kept list of applications may get before a guest asking for
/// it has it rescanned in the background ([`Cached`]).
pub const RESCAN_AFTER: Duration = Duration::from_secs(10);

/// This system's adapter, reading the usual locations from the environment,
/// behind the host's [`Cached`] list of applications by identity, rescanned
/// in the background once older than [`RESCAN_AFTER`].
pub fn native() -> Arc<dyn Applications> {
    let adapter: Arc<dyn Discovery> = if cfg!(target_os = "windows") {
        Arc::new(StartMenu::from_env())
    } else if cfg!(target_os = "macos") {
        Arc::new(AppBundles::from_env())
    } else if cfg!(target_os = "linux") {
        Arc::new(DesktopEntries::from_env())
    } else {
        Arc::new(Unsupported)
    };
    Arc::new(Cached::new(adapter, RESCAN_AFTER))
}

/// A system Pane does not find applications on.
struct Unsupported;

impl Discovery for Unsupported {
    fn sources(&self) -> Result<Vec<Source>, String> {
        Err(unsupported())
    }

    fn open(&self, _path: &str) -> Result<(), String> {
        Err(unsupported())
    }
}

fn unsupported() -> String {
    format!(
        "Pane finds applications only on Windows, macOS and Linux, not on {}",
        std::env::consts::OS
    )
}

/// The folder in environment variable `name`, if it is set and not empty.
fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The entries of `dir`, sorted by name so that discovery is deterministic;
/// none if it cannot be read.
fn sorted_entries(dir: &Path) -> Vec<std::fs::DirEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    entries
}

/// Whether `path`'s extension is `extension`, ignoring case.
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// `path` as text, or why it is not an application Pane found.
fn id_path(id: &str, extension: &str, kind: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(id);
    if !path.is_absolute() || !has_extension(&path, extension) {
        return Err(format!("{id} is not {kind}"));
    }
    if !path.exists() {
        return Err(format!("{id} no longer exists"));
    }
    Ok(path)
}
