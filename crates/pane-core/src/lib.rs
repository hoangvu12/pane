//! Pane's core: the launcher model, extension packages and the extension
//! runtime the launcher drives.

mod launcher;
mod packages;
mod runtime;

pub use launcher::{CommandRegistration, Launcher, LauncherView, Row, Screen, Status};
pub use packages::{
    EXTENSION_API, InstalledPackage, MANIFEST_FILE, MANIFEST_VERSION, Manifest, ManifestCommand,
    PackageError, PackageIdentity,
};
pub use runtime::{CallError, Item, Runtime, View};
