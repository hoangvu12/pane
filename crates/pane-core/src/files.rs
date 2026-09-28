//! The files in a folder the user chose: listing them for an extension.
//!
//! A WASI guest has no folders to read (its WASI context preopens none), so
//! Pane's host lists a folder for the Files default extension through the
//! `pane:extension/files` import ([`Folders`]), under one bounded scan
//! policy ([`Limits`], [`walk`]) that is the same on every system. The
//! extension matches the files against root search's query; opening one is
//! the launcher's `open-file` action, through its [`crate::LinkOpener`].
//!
//! A listing runs on a thread of its own, never on the runtime thread, and
//! stops as soon as the guest's call is dropped: when root search's query
//! changes, root search is left or the package's generation ends.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, HasData};

use crate::runtime::{GuestState, bindings};

use bindings::pane::extension::files as wit;

/// How many folders below the listed one Pane enters.
pub const MAX_DEPTH: usize = 8;

/// How many files one listing returns at most.
pub const MAX_FILES: usize = 5_000;

/// How many folder entries (files, folders, links, anything) one listing
/// looks at, at most.
pub const MAX_ENTRIES: usize = 20_000;

/// One file a listing found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundFile {
    /// Its absolute path, as the system writes it.
    pub path: String,
    /// Its path below the listed folder, with `/` between names.
    pub relative: String,
}

/// What a listing found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderListing {
    /// The files, breadth first, each folder's entries in name order.
    pub files: Vec<FoundFile>,
    /// The listing stopped at one of its [`Limits`] before it had looked at
    /// everything.
    pub truncated: bool,
}

/// Lists the files of folders; replaceable, for tests.
pub trait Folders: Send + Sync + 'static {
    /// Lists `folder`, as a guest gave it. Called on a thread of its own;
    /// once `cancelled` returns true, the caller has dropped the call and
    /// will discard the answer, so the listing should stop.
    fn list(&self, folder: &str, cancelled: &dyn Fn() -> bool) -> Result<FolderListing, String>;
}

/// This system's folders, listed by [`walk`] with the default [`Limits`].
pub fn native() -> Arc<dyn Folders> {
    Arc::new(Native)
}

struct Native;

impl Folders for Native {
    fn list(&self, folder: &str, cancelled: &dyn Fn() -> bool) -> Result<FolderListing, String> {
        walk(Path::new(folder), &Limits::default(), cancelled)
    }
}

/// The bounds of one listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How many folders below the listed one are entered.
    pub depth: usize,
    /// How many files are returned at most.
    pub files: usize,
    /// How many entries are looked at, at most.
    pub entries: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            depth: MAX_DEPTH,
            files: MAX_FILES,
            entries: MAX_ENTRIES,
        }
    }
}

/// Lists the regular files in `folder`, which must be an absolute path to a
/// folder, and its subfolders, breadth first and each folder in name order,
/// within `limits`. Hidden entries (a name starting with `.`; on Windows
/// also the hidden and system attributes), symbolic links and other links,
/// and names that are not Unicode are neither listed nor entered; a
/// subfolder that cannot be read is skipped. Checks `cancelled` before each
/// entry and stops when it says so (the result is then an error nobody
/// reads).
pub fn walk(
    folder: &Path,
    limits: &Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<FolderListing, String> {
    let shown = folder.display();
    if !folder.is_absolute() {
        return Err(format!(
            "“{shown}” is not a full path: enter the folder's whole path, such as {}",
            example_folder()
        ));
    }
    match fs::metadata(folder) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(format!("{shown} is a file, not a folder")),
        Err(error) => return Err(unreadable(&shown.to_string(), &error)),
    }
    let top = fs::read_dir(folder).map_err(|error| unreadable(&shown.to_string(), &error))?;
    let mut listing = FolderListing::default();
    let mut seen = 0;
    // Folders still to list: their entries, their path below `folder` and
    // how deep they are.
    let mut pending = std::collections::VecDeque::from([(top, String::new(), 0)]);
    while let Some((entries, below, depth)) = pending.pop_front() {
        let mut entries: Vec<fs::DirEntry> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            if cancelled() {
                return Err("the listing was cancelled".into());
            }
            seen += 1;
            if seen > limits.entries {
                listing.truncated = true;
                return Ok(listing);
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if hidden(&name, &entry) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let relative = if below.is_empty() {
                name
            } else {
                format!("{below}/{name}")
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if depth >= limits.depth {
                    listing.truncated = true;
                } else if let Ok(inner) = fs::read_dir(entry.path()) {
                    pending.push_back((inner, relative, depth + 1));
                }
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let Some(path) = entry.path().to_str().map(str::to_owned) else {
                continue;
            };
            if listing.files.len() == limits.files {
                listing.truncated = true;
                return Ok(listing);
            }
            listing.files.push(FoundFile { path, relative });
        }
    }
    Ok(listing)
}

/// A folder the user could type, for this system.
fn example_folder() -> &'static str {
    if cfg!(windows) {
        r"C:\Users\you\Documents"
    } else if cfg!(target_os = "macos") {
        "/Users/you/Documents"
    } else {
        "/home/you/Documents"
    }
}

/// Why `folder` cannot be listed, for the user.
fn unreadable(folder: &str, error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => format!("{folder} does not exist"),
        io::ErrorKind::PermissionDenied => format!("Pane may not read {folder}"),
        _ => format!("Pane cannot read {folder}: {error}"),
    }
}

/// Whether the entry `name` is hidden: its name starts with a dot, or on
/// Windows it has the hidden or system attribute.
fn hidden(name: &str, entry: &fs::DirEntry) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        // A directory entry's metadata describes the entry itself, not
        // what a link points to.
        if let Ok(metadata) = entry.metadata() {
            return metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM)
                != 0;
        }
    }
    #[cfg(not(windows))]
    let _ = entry;
    false
}

/// The host side of `pane:extension/files`: lists the folder on a thread of
/// its own and stops it if the guest's call is dropped.
pub(crate) struct Listings;

impl HasData for Listings {
    type Data<'a> = &'a mut GuestState;
}

impl wit::Host for GuestState {}

/// Tells a listing's thread to stop once the call waiting for it is gone.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl<T> wit::HostWithStore<T> for Listings {
    async fn list_folder(
        accessor: &Accessor<T, Self>,
        folder: String,
    ) -> Result<wit::FolderListing, String> {
        let folders = accessor.with(|mut view| {
            let state = view.get();
            // Code whose generation ended starts no more work.
            match state.stopped() {
                Some(_) => Err(String::from(
                    "this code of the extension was stopped (disabled, reloaded or updated)",
                )),
                None => Ok(state.folders()),
            }
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let _stop_on_drop = StopOnDrop(stop.clone());
        let (reply, answer) = oneshot::channel();
        std::thread::Builder::new()
            .name("pane-folder-listing".into())
            .spawn(move || {
                let cancelled = || stop.load(Ordering::Relaxed);
                let _ = reply.send(folders.list(&folder, &cancelled));
            })
            .map_err(|error| format!("Pane could not start listing the folder: {error}"))?;
        // Dropped here if the call is dropped: the thread is told to stop.
        let listing = answer
            .await
            .map_err(|_| String::from("the folder listing stopped unexpectedly"))??;
        Ok(wit::FolderListing {
            files: listing
                .files
                .into_iter()
                .map(|file| wit::FoundFile {
                    path: file.path,
                    relative: file.relative,
                })
                .collect(),
            truncated: listing.truncated,
        })
    }
}
