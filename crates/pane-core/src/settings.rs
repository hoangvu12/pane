//! Extension settings and the other data an installed package's commands
//! save through Pane: string values by key, of four kinds, each through its
//! own `pane:extension` interface (`wit/settings.wit`).
//!
//! | Kind | File | Clear cache |
//! |---|---|---|
//! | Settings | `settings.json` | kept |
//! | Content | `content.json` | kept |
//! | Cache | `cache.json` | removed |
//! | Credentials | `credentials.json` | kept |
//!
//! Each kind has one file next to `installed.json`, holding every package's
//! values under the package identity's key, so they belong to the source
//! identity rather than the title or the managed copy. They are kept while
//! the package is disabled, updated or Pane is not running. The kind decides
//! what a management action removes, and removing is done here by Pane,
//! never by running the package. An unreadable file is reported to the guest
//! and never overwritten.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::atomic::write_atomically;
use crate::packages::PackageIdentity;

const SETTINGS_VERSION: u64 = 1;

/// A kind of data a package keeps through Pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataKind {
    /// Values the package's commands save as its settings.
    Settings,
    /// The package's own durable records, such as notes or history.
    Content,
    /// Disposable values the package can compute or download again.
    Cache,
    /// Secrets kept on this computer, such as a sign-in token.
    Credentials,
}

impl DataKind {
    const ALL: [DataKind; 4] = [
        DataKind::Settings,
        DataKind::Content,
        DataKind::Cache,
        DataKind::Credentials,
    ];

    fn file_name(self) -> &'static str {
        match self {
            DataKind::Settings => "settings.json",
            DataKind::Content => "content.json",
            DataKind::Cache => "cache.json",
            DataKind::Credentials => "credentials.json",
        }
    }

    /// What one value of this kind is called.
    fn value(self) -> &'static str {
        match self {
            DataKind::Settings => "the setting",
            DataKind::Content => "the content",
            DataKind::Cache => "the cache value",
            DataKind::Credentials => "the credential",
        }
    }

    /// What all of a package's values of this kind are called, with a verb.
    fn kept_unchanged(self) -> &'static str {
        match self {
            DataKind::Settings => "its settings are kept unchanged",
            DataKind::Content => "its content is kept unchanged",
            DataKind::Cache => "its cache is kept unchanged",
            DataKind::Credentials => "its credentials are kept unchanged",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct SettingsJson {
    version: u64,
    /// Values by package identity key, then by the extension's own key.
    packages: BTreeMap<String, BTreeMap<String, String>>,
}

/// One kind's file as Pane last read or wrote it.
struct DataFile {
    path: PathBuf,
    /// The saved values, or why they could not be read.
    file: Result<SettingsJson, String>,
}

impl DataFile {
    fn open(path: PathBuf) -> DataFile {
        let file = read(&path);
        DataFile { path, file }
    }
}

/// The data files as Pane last read or wrote them.
struct SettingsFile {
    /// One per kind, in the order of [`DataKind::ALL`].
    files: Vec<DataFile>,
    /// Identity keys of disabled packages, whose commands may not run or
    /// save values. The launcher keeps it in step with its packages; the
    /// runtime reads it here, on its own thread.
    disabled: HashSet<String>,
}

impl SettingsFile {
    fn data(&mut self, kind: DataKind) -> &mut DataFile {
        &mut self.files[kind as usize]
    }
}

/// Every installed package's settings and other data. Cloning shares the
/// same files.
#[derive(Clone)]
pub(crate) struct Settings(Arc<Mutex<SettingsFile>>);

impl Settings {
    /// Opens the data kept in `dir`. Nothing is written until a command
    /// saves a value.
    pub fn open(dir: &Path) -> Settings {
        let files = DataKind::ALL
            .into_iter()
            .map(|kind| DataFile::open(dir.join(kind.file_name())))
            .collect();
        Settings(Arc::new(Mutex::new(SettingsFile {
            files,
            disabled: HashSet::new(),
        })))
    }

    /// The data of the package with `identity`, as its commands see it.
    pub fn owned_by(&self, identity: &PackageIdentity) -> PackageSettings {
        PackageSettings {
            settings: self.clone(),
            owner: identity.key(),
        }
    }

    /// Records whether the package with `identity` is enabled; a disabled
    /// package's commands cannot run or save values.
    pub fn set_enabled(&self, identity: &PackageIdentity, enabled: bool) {
        let mut file = self.lock();
        if enabled {
            file.disabled.remove(&identity.key());
        } else {
            file.disabled.insert(identity.key());
        }
    }

    /// Removes every cache value of the package with `identity`, and nothing
    /// else: its other kinds of data and other packages' caches stay. Works
    /// whether or not the package is enabled or its code loads. A cache file
    /// that could not be read is read again first, so one the user repaired
    /// or deleted can be cleared without restarting Pane. On failure nothing
    /// is removed, and the reason says what the user can do.
    pub fn clear_cache(&self, identity: &PackageIdentity) -> Result<(), String> {
        let mut store = self.lock();
        let data = store.data(DataKind::Cache);
        if data.file.is_err() {
            data.file = read(&data.path);
        }
        let file = data.file.as_ref().map_err(|reason| {
            format!(
                "{reason}. Nothing was deleted. That file holds only extension caches: repair \
                 or delete it, then clear the cache again."
            )
        })?;
        if !file.packages.contains_key(&identity.key()) {
            return Ok(());
        }
        let mut updated = file.clone();
        updated.packages.remove(&identity.key());
        write(&data.path, &updated).map_err(|error| {
            format!(
                "Cannot write {}: {error}. Nothing was deleted; check that Pane can write that \
                 folder, then clear the cache again.",
                data.path.display()
            )
        })?;
        data.file = Ok(updated);
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SettingsFile> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One package's settings and other data, handed to the runtime with each
/// call into the package's commands.
#[derive(Clone)]
pub(crate) struct PackageSettings {
    settings: Settings,
    owner: String,
}

impl PackageSettings {
    /// Whether the package is disabled, so its commands may not run.
    pub fn is_disabled(&self) -> bool {
        self.settings.lock().disabled.contains(&self.owner)
    }

    /// The value of `kind` saved under `key`, if any.
    pub fn get(&self, kind: DataKind, key: &str) -> Result<Option<String>, String> {
        let mut store = self.settings.lock();
        let file = store.data(kind).file.as_ref().map_err(Clone::clone)?;
        Ok(file
            .packages
            .get(&self.owner)
            .and_then(|values| values.get(key))
            .cloned())
    }

    /// Saves `value` of `kind` under `key`, unless the package is disabled.
    pub fn set(&self, kind: DataKind, key: &str, value: &str) -> Result<(), String> {
        let mut store = self.settings.lock();
        if store.disabled.contains(&self.owner) {
            return Err(format!(
                "the extension is disabled; {}",
                kind.kept_unchanged()
            ));
        }
        let data = store.data(kind);
        let file = data.file.as_ref().map_err(Clone::clone)?;
        let mut updated = file.clone();
        updated
            .packages
            .entry(self.owner.clone())
            .or_default()
            .insert(key.to_owned(), value.to_owned());
        write(&data.path, &updated)
            .map_err(|error| format!("Could not save {}: {error}", kind.value()))?;
        data.file = Ok(updated);
        Ok(())
    }
}

/// Reads one kind's file; a missing file holds no values.
fn read(path: &Path) -> Result<SettingsJson, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<SettingsJson>(&text)
            .map_err(|error| error.to_string())
            .and_then(|file| {
                if file.version == SETTINGS_VERSION {
                    Ok(file)
                } else {
                    Err(format!(
                        "it has version {}, this Pane reads {SETTINGS_VERSION}",
                        file.version
                    ))
                }
            }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SettingsJson {
            version: SETTINGS_VERSION,
            packages: BTreeMap::new(),
        }),
        Err(error) => Err(error.to_string()),
    }
    .map_err(|reason| format!("Cannot read {}: {reason}", path.display()))
}

/// Replaces one kind's file whole (see [`write_atomically`] for what a crash
/// or a second Pane process can do to it).
fn write(path: &Path, file: &SettingsJson) -> io::Result<()> {
    let text = serde_json::to_string_pretty(file).map_err(io::Error::other)?;
    write_atomically(path, text.as_bytes())
}
