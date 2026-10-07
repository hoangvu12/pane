//! macOS: the application bundles (`.app` folders) in the Applications
//! folders, opened by the system's `open` command (Launch Services). A
//! bundle is identified by its bundle identifier (`CFBundleIdentifier` in
//! its `Info.plist`), so moving or renaming it keeps the application; one
//! without an identifier is identified by its path.

use std::path::{Path, PathBuf};

use super::{Discovery, Key, Source, env_dir, has_extension, id_path, plist, sorted_entries};

/// How deep Pane looks into folders that are not bundles, such as
/// `/Applications/Utilities`.
const MAX_DEPTH: usize = 2;

/// The application bundles in some folders.
#[derive(Clone, Debug)]
pub struct AppBundles {
    folders: Vec<PathBuf>,
}

impl AppBundles {
    /// The bundles in `folders` and their subfolders (not inside bundles),
    /// a bundle in an earlier folder preferred to one with the same
    /// identifier in a later folder.
    pub fn new(folders: Vec<PathBuf>) -> AppBundles {
        AppBundles { folders }
    }

    /// The bundles in `/Applications`, `/System/Applications` and
    /// `~/Applications` (`$HOME/Applications`), and their subfolders such as
    /// `Utilities`.
    pub fn from_env() -> AppBundles {
        let mut folders = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];
        folders.extend(env_dir("HOME").map(|home| home.join("Applications")));
        AppBundles { folders }
    }
}

/// The key of the bundle at `bundle`: its identifier, in lowercase, or its
/// path when its `Info.plist` names none.
fn bundle_key(bundle: &Path) -> Key {
    std::fs::read(bundle.join("Contents/Info.plist"))
        .ok()
        .and_then(|info| plist::string(&info, "CFBundleIdentifier"))
        .map(|identifier| identifier.trim().to_lowercase())
        .filter(|identifier| !identifier.is_empty())
        .map_or_else(
            || Key::Path(bundle.to_string_lossy().into_owned()),
            Key::Bundle,
        )
}

fn collect(dir: &Path, depth: usize, place: usize, found: &mut Vec<Source>) {
    for entry in sorted_entries(dir) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if has_extension(&path, "app") {
            let Some(name) = path.file_stem() else {
                continue;
            };
            found.push(Source {
                key: bundle_key(&path),
                path: path.to_string_lossy().into_owned(),
                name: name.to_string_lossy().into_owned(),
                location: dir.display().to_string(),
                place,
            });
        } else if depth < MAX_DEPTH {
            collect(&path, depth + 1, place, found);
        }
    }
}

impl Discovery for AppBundles {
    fn sources(&self) -> Result<Vec<Source>, String> {
        let mut found = Vec::new();
        for (place, folder) in self.folders.iter().enumerate() {
            collect(folder, 0, place, &mut found);
        }
        Ok(found)
    }

    fn open(&self, path: &str) -> Result<(), String> {
        let path = id_path(path, "app", "an application bundle")?;
        open_bundle(&path)
    }
}

#[cfg(target_os = "macos")]
fn open_bundle(path: &Path) -> Result<(), String> {
    // `open` returns once Launch Services has started (or activated) the
    // application, and explains a failure on its standard error.
    let output = std::process::Command::new("/usr/bin/open")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("cannot run /usr/bin/open: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if reason.is_empty() {
        format!("/usr/bin/open failed with {}", output.status)
    } else {
        reason
    })
}

#[cfg(not(target_os = "macos"))]
fn open_bundle(path: &Path) -> Result<(), String> {
    Err(format!(
        "{} is a macOS application bundle; Pane opens those only on macOS",
        path.display()
    ))
}
