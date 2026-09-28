//! Extension settings: string values an installed package's commands save
//! through the `pane:extension/settings` interface.
//!
//! Pane keeps every package's settings in one file, `settings.json` next to
//! `installed.json`, under the package identity's key, so they belong to the
//! source identity rather than the title or the managed copy. They are kept
//! while the package is disabled, updated or Pane is not running. An
//! unreadable file is reported to the guest and never overwritten.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::atomic::write_atomically;
use crate::packages::PackageIdentity;

pub(crate) const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_VERSION: u64 = 1;

#[derive(Clone, Default, Serialize, Deserialize)]
struct SettingsJson {
    version: u64,
    /// Values by package identity key, then by the extension's own key.
    packages: BTreeMap<String, BTreeMap<String, String>>,
}

struct Store {
    path: PathBuf,
    /// The saved settings, or why they could not be read.
    file: Result<SettingsJson, String>,
    /// Identity keys of disabled packages, whose commands may not write.
    disabled: HashSet<String>,
}

/// Every installed package's settings. Cloning shares the same store.
#[derive(Clone)]
pub(crate) struct Settings(Arc<Mutex<Store>>);

impl Settings {
    /// Opens the settings kept in `dir`. Nothing is written until a command
    /// saves a value.
    pub fn open(dir: &Path) -> Settings {
        let path = dir.join(SETTINGS_FILE);
        let file = match fs::read_to_string(&path) {
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
        .map_err(|reason| format!("Cannot read {}: {reason}", path.display()));
        Settings(Arc::new(Mutex::new(Store {
            path,
            file,
            disabled: HashSet::new(),
        })))
    }

    /// The settings of the package with `identity`, as its commands see them.
    pub fn owned_by(&self, identity: &PackageIdentity) -> PackageSettings {
        PackageSettings {
            settings: self.clone(),
            owner: identity.key(),
        }
    }

    /// Records whether the package with `identity` is enabled; a disabled
    /// package's commands cannot save values.
    pub fn set_enabled(&self, identity: &PackageIdentity, enabled: bool) {
        let mut store = self.lock();
        if enabled {
            store.disabled.remove(&identity.key());
        } else {
            store.disabled.insert(identity.key());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One package's settings, handed to the runtime with each call into the
/// package's commands.
#[derive(Clone)]
pub(crate) struct PackageSettings {
    settings: Settings,
    owner: String,
}

impl PackageSettings {
    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        let store = self.settings.lock();
        let file = store.file.as_ref().map_err(Clone::clone)?;
        Ok(file
            .packages
            .get(&self.owner)
            .and_then(|values| values.get(key))
            .cloned())
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let mut store = self.settings.lock();
        if store.disabled.contains(&self.owner) {
            return Err("the extension is disabled; its settings are kept unchanged".into());
        }
        let file = store.file.as_ref().map_err(Clone::clone)?;
        let mut updated = file.clone();
        updated
            .packages
            .entry(self.owner.clone())
            .or_default()
            .insert(key.to_owned(), value.to_owned());
        write(&store.path, &updated)
            .map_err(|error| format!("Could not save the setting: {error}"))?;
        store.file = Ok(updated);
        Ok(())
    }
}

/// Replaces the settings file whole (see [`write_atomically`] for what a
/// crash or a second Pane process can do to it).
fn write(path: &Path, file: &SettingsJson) -> io::Result<()> {
    let text = serde_json::to_string_pretty(file).map_err(io::Error::other)?;
    write_atomically(path, text.as_bytes())
}
