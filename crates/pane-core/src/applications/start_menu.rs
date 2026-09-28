//! Windows: the shortcuts (`.lnk` files) in the Start menu's Programs
//! folders, opened as Explorer opens them (the shell's `ShellExecuteEx`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Application, Applications, env_dir, has_extension, id_path, sorted_entries};

/// How deep Pane looks into the Programs folders' subfolders.
const MAX_DEPTH: usize = 8;

/// The shortcuts in some Start menu Programs folders.
#[derive(Clone, Debug)]
pub struct StartMenu {
    /// The Programs folders, the one whose shortcuts win first: a shortcut
    /// at the same place below a later folder is not listed again.
    folders: Vec<PathBuf>,
}

impl StartMenu {
    /// The shortcuts in `folders` and their subfolders, the first winning.
    pub fn new(folders: Vec<PathBuf>) -> StartMenu {
        StartMenu { folders }
    }

    /// The current user's Start menu
    /// (`%APPDATA%\Microsoft\Windows\Start Menu\Programs`), then every
    /// user's (`%ProgramData%\Microsoft\Windows\Start Menu\Programs`).
    pub fn from_env() -> StartMenu {
        let programs = |root: PathBuf| root.join(r"Microsoft\Windows\Start Menu\Programs");
        let folders = env_dir("APPDATA")
            .into_iter()
            .chain(env_dir("ProgramData"))
            .map(programs)
            .collect();
        StartMenu { folders }
    }
}

fn collect(
    root: &Path,
    dir: &Path,
    depth: usize,
    seen: &mut HashSet<String>,
    found: &mut Vec<Application>,
) {
    for entry in sorted_entries(dir) {
        let path = entry.path();
        if path.is_dir() {
            if depth < MAX_DEPTH {
                collect(root, &path, depth + 1, seen, found);
            }
            continue;
        }
        if !has_extension(&path, "lnk") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        // Uninstallers are shortcuts too, but not applications to open.
        if name.to_lowercase().starts_with("uninstall") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if !seen.insert(relative.to_string_lossy().to_lowercase()) {
            continue;
        }
        found.push(Application {
            id: path.to_string_lossy().into_owned(),
            name,
            location: dir.display().to_string(),
        });
    }
}

impl Applications for StartMenu {
    fn installed(&self) -> Result<Vec<Application>, String> {
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for folder in &self.folders {
            collect(folder, folder, 0, &mut seen, &mut found);
        }
        Ok(found)
    }

    fn open(&self, id: &str) -> Result<(), String> {
        let path = id_path(id, "lnk", "a Start menu shortcut")?;
        open_shortcut(&path)
    }
}

#[cfg(windows)]
fn open_shortcut(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let file: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // The shell may use COM to open a shortcut; the calling thread is Pane's
    // own short-lived one, so initializing it here is safe.
    unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
    }
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // Wait until the shell has started it (the thread ends next), and
        // report a failure here instead of in a dialog.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: file.as_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    // SAFETY: `info` is initialized with its size and a valid,
    // NUL-terminated file name that outlives the call.
    if unsafe { ShellExecuteExW(&mut info) } != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(windows))]
fn open_shortcut(path: &Path) -> Result<(), String> {
    Err(format!(
        "{} is a Windows shortcut; Pane opens those only on Windows",
        path.display()
    ))
}
