//! Opening web links and files: the `open-url` and `open-file` actions of a
//! computed root result, such as a quicklink or a file the Files extension
//! found. Pane hands the address or path to a [`LinkOpener`], normally the
//! system's handlers (the window supplies it). It accepts only `http://` and
//! `https://` addresses as links, so an extension cannot have the system run
//! another URL scheme's handler that way, and only absolute paths of
//! existing files (not folders) as files.

use std::io;
use std::path::Path;

/// Opens web links and files; the launcher calls it off the window's
/// thread, since a system handler may take a moment to start.
pub trait LinkOpener: Send + Sync + 'static {
    /// Opens `url`, an `http://` or `https://` address, with the system's
    /// handler for web links. An error explains to the user why it could
    /// not, such as that no handler is installed or that the system refused.
    fn open(&self, url: &str) -> Result<(), String>;

    /// Opens `path`, the absolute path of an existing file, with the
    /// system's handler for its type, as the system's file manager would.
    /// An error explains why it could not. An opener that opens no files
    /// keeps this default, which says so.
    fn open_file(&self, path: &Path) -> Result<(), String> {
        let _ = path;
        Err("this Pane has no handler for files".into())
    }
}

/// The opener of a launcher that was given none.
pub(crate) struct NoOpener;

impl LinkOpener for NoOpener {
    fn open(&self, _url: &str) -> Result<(), String> {
        Err("this Pane has no link handler".into())
    }
}

/// Why Pane does not open `url` at all, if it does not: it is not a web
/// address.
pub(crate) fn refusal(url: &str) -> Option<String> {
    let scheme = url.split_once("://").map(|(scheme, _)| scheme);
    let web = scheme.is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    });
    (!web).then(|| "Pane opens only http:// and https:// links".into())
}

/// Why Pane does not open `path` as a file at all, if it does not, from the
/// path alone: it is not absolute.
pub(crate) fn file_refusal(path: &str) -> Option<String> {
    (!Path::new(path).is_absolute())
        .then(|| "Pane opens only files given by their full path".into())
}

/// The last name of `path`, for the user: the file's own name.
pub(crate) fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// Opens the file `path` with `links`, once it is an absolute path of a file
/// that still exists (a folder is refused: it would open a file manager).
/// Blocking; call it off the window's thread.
pub(crate) fn open_file(links: &dyn LinkOpener, path: &str) -> Result<(), String> {
    if let Some(reason) = file_refusal(path) {
        return Err(reason);
    }
    let path = Path::new(path);
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => links.open_file(path),
        Ok(metadata) if metadata.is_dir() => Err("it is a folder; Pane opens only files".into()),
        Ok(_) => Err("it is not a regular file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err("it no longer exists".into()),
        Err(error) => Err(format!("Pane cannot read it: {error}")),
    }
}
