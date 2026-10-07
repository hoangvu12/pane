//! Windows: the shortcuts (`.lnk` files) in the Start menu's Programs
//! folders, and the packaged (AppX/MSIX) apps of the shell's Apps folder
//! (`shell:AppsFolder`), such as Calculator on Windows 11. Both are opened
//! as Explorer opens them (the shell's `ShellExecuteEx`): a shortcut by its
//! path, a packaged app by `shell:AppsFolder\<AppUserModelID>`.
//!
//! A shortcut is identified by what it opens: its target and arguments as
//! the shell's shortcut interface reads them, on single-threaded COM
//! apartments in parallel, with version folders as a wildcard
//! ([`Key::program`]); a shortcut whose AppUserModelID is a packaged app's
//! is that app. A packaged app is identified by its package family
//! ([`super::identity::packaged_keys`]). A shortcut Pane cannot read is
//! identified by its own path.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::identity::packaged_keys;
use super::{Discovery, Key, Source, env_dir, has_extension, id_path, sorted_entries};

/// How deep Pane looks into the Programs folders' subfolders.
const MAX_DEPTH: usize = 8;

/// What a packaged app's source path starts with, before its
/// AppUserModelID.
const APPS_FOLDER: &str = r"shell:AppsFolder\";

/// Where a shortcut was found, the most preferred first: the primary
/// source of an application found in several places is the one in the
/// earliest ([`super::identity::primary`]), as Explorer prefers the user's
/// own shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Place {
    /// The user's Desktop.
    UserDesktop,
    /// Every user's Desktop (`%PUBLIC%\Desktop`).
    AllUsersDesktop,
    /// The user's Start menu Programs folder.
    UserStartMenu,
    /// Every user's Start menu Programs folder.
    AllUsersStartMenu,
    /// The shortcuts pinned to the taskbar.
    TaskbarPins,
    /// The Apps folder's packaged apps.
    PackagedApps,
}

impl Place {
    /// The places sharing a layout: a shortcut at the same path below one
    /// of them as below an earlier one is not listed again (the all-users
    /// Start menu repeats the user's).
    fn layout(self) -> u8 {
        match self {
            Place::UserDesktop | Place::AllUsersDesktop => 0,
            Place::UserStartMenu | Place::AllUsersStartMenu => 1,
            Place::TaskbarPins => 2,
            Place::PackagedApps => 3,
        }
    }
}

/// A folder of shortcuts and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutFolder {
    pub path: PathBuf,
    pub place: Place,
    /// Whether its subfolders are looked into too (the Start menu's are;
    /// the Desktop's are not).
    pub subfolders: bool,
}

/// What a shortcut opens, as the shell's shortcut interface reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcut {
    /// The program or file it targets, with environment variables
    /// expanded; empty when it targets no file (a packaged app, a shell
    /// item).
    pub target: String,
    /// The arguments it passes.
    pub arguments: String,
    /// Its AppUserModelID, when it has one.
    pub app_user_model_id: Option<String>,
}

/// Reads shortcuts: for each path, what it opens, or `None` when it cannot
/// be read.
pub type Resolver = Arc<dyn Fn(&[PathBuf]) -> Vec<Option<Shortcut>> + Send + Sync>;

/// What a shortcut file was when it was read: its size and when it was
/// last written, so an unchanged shortcut is not read again.
type Fingerprint = (u64, Option<SystemTime>);

/// The shortcuts in some folders and, when enabled, the packaged apps in
/// the Apps folder.
#[derive(Clone)]
pub struct StartMenu {
    /// The shortcut folders.
    folders: Vec<ShortcutFolder>,
    /// Whether the Apps folder's packaged apps are listed too.
    packaged: bool,
    /// Reads what each shortcut opens.
    resolve: Resolver,
    /// The shortcuts read by earlier scans, by path.
    read: Arc<Mutex<HashMap<PathBuf, (Fingerprint, Option<Shortcut>)>>>,
}

impl std::fmt::Debug for StartMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartMenu")
            .field("folders", &self.folders)
            .field("packaged", &self.packaged)
            .finish_non_exhaustive()
    }
}

impl StartMenu {
    /// The shortcuts in `folders` and their subfolders, the first being the
    /// user's Start menu and the others every user's. Packaged apps are not
    /// listed.
    pub fn new(folders: Vec<PathBuf>) -> StartMenu {
        let folders = folders
            .into_iter()
            .enumerate()
            .map(|(index, path)| ShortcutFolder {
                path,
                place: if index == 0 {
                    Place::UserStartMenu
                } else {
                    Place::AllUsersStartMenu
                },
                subfolders: true,
            })
            .collect();
        StartMenu::with_folders(folders)
    }

    /// The shortcuts in `folders`. Packaged apps are not listed.
    pub fn with_folders(folders: Vec<ShortcutFolder>) -> StartMenu {
        StartMenu {
            folders,
            packaged: false,
            resolve: Arc::new(resolve_shortcuts),
            read: Arc::default(),
        }
    }

    /// The current user's Start menu
    /// (`%APPDATA%\Microsoft\Windows\Start Menu\Programs`), then every
    /// user's (`%ProgramData%\Microsoft\Windows\Start Menu\Programs`), then
    /// the Apps folder's packaged apps.
    pub fn from_env() -> StartMenu {
        let programs = |root: PathBuf| root.join(r"Microsoft\Windows\Start Menu\Programs");
        let mut menu = StartMenu::new(
            env_dir("APPDATA")
                .into_iter()
                .chain(env_dir("ProgramData"))
                .map(programs)
                .collect(),
        );
        menu.packaged = true;
        menu
    }

    /// This, reading shortcuts with `resolve` instead of the shell, which
    /// exists only on Windows: what tests use to give shortcuts targets on
    /// every system.
    pub fn with_resolver(
        mut self,
        resolve: impl Fn(&[PathBuf]) -> Vec<Option<Shortcut>> + Send + Sync + 'static,
    ) -> StartMenu {
        self.resolve = Arc::new(resolve);
        self.read = Arc::default();
        self
    }

    /// What each of `paths` opens: read again only when the file changed
    /// since an earlier scan read it.
    fn shortcuts(&self, paths: &[PathBuf]) -> Vec<Option<Shortcut>> {
        let fingerprints: Vec<Fingerprint> = paths
            .iter()
            .map(|path| {
                std::fs::metadata(path).map_or((0, None), |metadata| {
                    (metadata.len(), metadata.modified().ok())
                })
            })
            .collect();
        let mut read = self
            .read
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (unread, unread_fingerprints): (Vec<PathBuf>, Vec<Fingerprint>) = paths
            .iter()
            .zip(fingerprints)
            .filter(|(path, fingerprint)| {
                read.get(*path).is_none_or(|(known, _)| {
                    // A file whose time is unknown is always read again.
                    known != fingerprint || known.1.is_none()
                })
            })
            .map(|(path, fingerprint)| (path.clone(), fingerprint))
            .unzip();
        if !unread.is_empty() {
            let shortcuts = (self.resolve)(&unread);
            for ((path, fingerprint), shortcut) in
                unread.into_iter().zip(unread_fingerprints).zip(shortcuts)
            {
                read.insert(path, (fingerprint, shortcut));
            }
        }
        // Forget the shortcuts no longer found.
        let found: HashSet<&PathBuf> = paths.iter().collect();
        read.retain(|path, _| found.contains(path));
        paths
            .iter()
            .map(|path| read.get(path).and_then(|(_, shortcut)| shortcut.clone()))
            .collect()
    }
}

/// A shortcut file found, before Pane knows what it opens.
struct Found {
    path: PathBuf,
    name: String,
    location: String,
    place: Place,
}

fn collect(
    folder: &ShortcutFolder,
    dir: &Path,
    depth: usize,
    seen: &mut HashSet<(u8, String)>,
    found: &mut Vec<Found>,
) {
    for entry in sorted_entries(dir) {
        let path = entry.path();
        if path.is_dir() {
            if folder.subfolders && depth < MAX_DEPTH {
                collect(folder, &path, depth + 1, seen, found);
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
        let Ok(relative) = path.strip_prefix(&folder.path) else {
            continue;
        };
        let relative = relative.to_string_lossy().to_lowercase();
        if !seen.insert((folder.place.layout(), relative)) {
            continue;
        }
        found.push(Found {
            path,
            name,
            location: dir.display().to_string(),
            place: folder.place,
        });
    }
}

/// Whether `parsing`, an Apps folder item's parsing name, is a packaged
/// app's AppUserModelID (`<package family>!<app>`), not a file or a folder.
fn is_app_user_model_id(parsing: &str) -> bool {
    parsing.contains('!') && !parsing.contains(['\\', '/', ':', '{'])
}

/// The sources the shortcuts `found` and the Apps folder's `items` (each
/// display name and parsing name) are, given what each shortcut opens
/// (`shortcuts`, in the order of `found`).
///
/// A shortcut is keyed by its target and arguments, or by the packaged app
/// its AppUserModelID names, or, when it could not be read, by its own
/// path. The Apps folder's packaged apps are keyed by package family; one
/// that no shortcut opens but whose name a shortcut has is left out, as
/// the Apps folder lists the shortcuts' programs too.
fn sources(
    found: Vec<Found>,
    shortcuts: Vec<Option<Shortcut>>,
    items: Vec<(String, String)>,
) -> Vec<Source> {
    let mut packaged: Vec<(String, String)> = Vec::new();
    let mut aumids: HashSet<String> = HashSet::new();
    for (name, parsing) in items {
        if is_app_user_model_id(&parsing)
            && !name.trim().is_empty()
            && aumids.insert(parsing.to_lowercase())
        {
            packaged.push((name, parsing));
        }
    }
    let keys = packaged_keys(
        &packaged
            .iter()
            .map(|(_, parsing)| parsing.clone())
            .collect::<Vec<_>>(),
    );
    let package_key: HashMap<String, Key> = packaged
        .iter()
        .zip(&keys)
        .map(|((_, parsing), key)| (parsing.to_lowercase(), key.clone()))
        .collect();

    let mut sources: Vec<Source> = found
        .into_iter()
        .zip(shortcuts)
        .map(|(found, shortcut)| {
            let path = found.path.to_string_lossy().into_owned();
            let key = match shortcut {
                Some(Shortcut {
                    app_user_model_id: Some(aumid),
                    ..
                }) if package_key.contains_key(&aumid.to_lowercase()) => {
                    package_key[&aumid.to_lowercase()].clone()
                }
                Some(shortcut) if !shortcut.target.trim().is_empty() => {
                    Key::program(&shortcut.target, &shortcut.arguments)
                }
                _ => Key::Path(path.to_lowercase()),
            };
            Source {
                key,
                path,
                name: found.name,
                location: found.location,
                place: found.place as usize,
            }
        })
        .collect();

    let shortcut_keys: HashSet<Key> = sources.iter().map(|source| source.key.clone()).collect();
    let shortcut_names: HashSet<String> = sources
        .iter()
        .map(|source| source.name.to_lowercase())
        .collect();
    for ((name, parsing), key) in packaged.into_iter().zip(keys) {
        if !shortcut_keys.contains(&key) && shortcut_names.contains(&name.to_lowercase()) {
            continue;
        }
        sources.push(Source {
            key,
            path: format!("{APPS_FOLDER}{parsing}"),
            name,
            location: "Apps folder (packaged apps)".into(),
            place: Place::PackagedApps as usize,
        });
    }
    sources
}

impl Discovery for StartMenu {
    fn sources(&self) -> Result<Vec<Source>, String> {
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for folder in &self.folders {
            collect(folder, &folder.path, 0, &mut seen, &mut found);
        }
        let paths: Vec<PathBuf> = found.iter().map(|found| found.path.clone()).collect();
        let shortcuts = self.shortcuts(&paths);
        let items = if self.packaged {
            apps_folder()?
        } else {
            Vec::new()
        };
        Ok(sources(found, shortcuts, items))
    }

    fn open(&self, path: &str) -> Result<(), String> {
        if let Some(aumid) = path.strip_prefix(APPS_FOLDER) {
            if aumid.is_empty() || aumid.contains(['\\', '/']) {
                return Err(format!("{path} is not a packaged app"));
            }
            return shell_execute(path);
        }
        let path = id_path(path, "lnk", "a Start menu shortcut")?;
        shell_execute(&path.to_string_lossy())
    }
}

/// COM initialized on this thread for as long as it is held; the shell may
/// use COM to enumerate the Apps folder, read a shortcut or open one.
#[cfg(windows)]
struct Com {
    /// Whether this guard initialized COM and must uninitialize it: not when
    /// the thread already had it in another mode.
    initialized: bool,
}

#[cfg(windows)]
impl Com {
    fn new() -> Result<Com, String> {
        use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
        use windows::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
        };
        // SAFETY: no reserved pointer; paired with CoUninitialize in `drop`.
        let result =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if result == RPC_E_CHANGED_MODE {
            // Already initialized as multithreaded: usable as it is.
            return Ok(Com { initialized: false });
        }
        result
            .ok()
            .map_err(|error| format!("cannot start COM: {error}"))?;
        Ok(Com { initialized: true })
    }
}

#[cfg(windows)]
impl Drop for Com {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: paired with the successful CoInitializeEx in `new`.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// How many threads read shortcuts at once, at most.
#[cfg(windows)]
const READERS: usize = 8;

/// What each shortcut at `paths` opens, read by the shell's shortcut
/// interface on single-threaded COM apartments, several threads at once.
#[cfg(windows)]
fn resolve_shortcuts(paths: &[PathBuf]) -> Vec<Option<Shortcut>> {
    if paths.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .clamp(1, READERS)
        .min(paths.len());
    let chunk = paths.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let readers: Vec<_> = paths
            .chunks(chunk)
            .map(|chunk| (chunk.len(), scope.spawn(move || read_shortcuts(chunk))))
            .collect();
        readers
            .into_iter()
            .flat_map(|(count, reader)| reader.join().unwrap_or_else(|_| vec![None; count]))
            .collect()
    })
}

#[cfg(not(windows))]
fn resolve_shortcuts(paths: &[PathBuf]) -> Vec<Option<Shortcut>> {
    vec![None; paths.len()]
}

/// What each shortcut at `paths` opens, on this thread's own apartment.
#[cfg(windows)]
fn read_shortcuts(paths: &[PathBuf]) -> Vec<Option<Shortcut>> {
    let Ok(_com) = Com::new() else {
        return vec![None; paths.len()];
    };
    paths.iter().map(|path| read_shortcut(path).ok()).collect()
}

/// What the shortcut at `path` opens, without resolving a target that
/// moved (which could search the disk): its target as recorded, with
/// environment variables expanded, its arguments and its AppUserModelID.
#[cfg(windows)]
fn read_shortcut(path: &Path) -> windows::core::Result<Shortcut> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToString};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{GUID, Interface, PCWSTR};

    /// `PKEY_AppUserModel_ID`.
    const APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };
    let text = |buffer: &[u16]| -> String {
        let end = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end]).trim().to_owned()
    };
    let file: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: plain COM calls on interfaces the shell returns, on a thread
    // whose COM apartment outlives them; every buffer outlives its call.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.cast::<IPersistFile>()?
            .Load(PCWSTR(file.as_ptr()), STGM_READ)?;
        let mut target = vec![0u16; 4096];
        // No target file (a packaged app's shortcut) is not an error: the
        // target is empty then.
        let _ = link.GetPath(&mut target, std::ptr::null_mut(), 0);
        let mut arguments = vec![0u16; 4096];
        let _ = link.GetArguments(&mut arguments);
        let app_user_model_id = link.cast::<IPropertyStore>().ok().and_then(|store| {
            let mut value = store.GetValue(&APP_USER_MODEL_ID).ok()?;
            let mut buffer = vec![0u16; 1024];
            let read = PropVariantToString(&value, &mut buffer);
            let _ = PropVariantClear(&mut value);
            read.ok()?;
            Some(text(&buffer)).filter(|aumid| !aumid.is_empty())
        });
        Ok(Shortcut {
            target: text(&target),
            arguments: text(&arguments),
            app_user_model_id,
        })
    }
}

/// The Apps folder's items, each (display name, parsing name).
#[cfg(windows)]
fn apps_folder() -> Result<Vec<(String, String)>, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT,
        SHGetKnownFolderItem, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
    };

    let failed = |error: windows::core::Error| format!("cannot list the Apps folder: {error}");
    let _com = Com::new()?;
    let name = |item: &IShellItem, kind: SIGDN| -> Result<String, String> {
        // SAFETY: the returned string is owned by the caller and freed here.
        unsafe {
            let text = item.GetDisplayName(kind).map_err(failed)?;
            let name = text.to_string().unwrap_or_default();
            CoTaskMemFree(Some(text.0 as *const _));
            Ok(name)
        }
    };
    let mut items = Vec::new();
    // SAFETY: plain COM calls on interfaces the shell returns, on a thread
    // with COM initialized for their lifetime.
    unsafe {
        let folder: IShellItem =
            SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None).map_err(failed)?;
        let entries: IEnumShellItems = folder
            .BindToHandler(None, &BHID_EnumItems)
            .map_err(failed)?;
        loop {
            let mut item = [None];
            let mut fetched = 0;
            entries
                .Next(&mut item, Some(&mut fetched))
                .map_err(failed)?;
            let Some(item) = item[0].take().filter(|_| fetched == 1) else {
                break;
            };
            items.push((
                name(&item, SIGDN_NORMALDISPLAY)?,
                name(&item, SIGDN_PARENTRELATIVEPARSING)?,
            ));
        }
    }
    Ok(items)
}

#[cfg(not(windows))]
fn apps_folder() -> Result<Vec<(String, String)>, String> {
    Err("the Apps folder exists only on Windows".into())
}

/// Opens `file`, a shortcut's path or `shell:AppsFolder\<AppUserModelID>`,
/// as Explorer does.
#[cfg(windows)]
fn shell_execute(file: &str) -> Result<(), String> {
    use windows::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    let file: Vec<u16> = file.encode_utf16().chain([0]).collect();
    let _com = Com::new()?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // Wait until the shell has started it (the thread ends next), and
        // report a failure here instead of in a dialog.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is initialized with its size and a valid,
    // NUL-terminated file name that outlives the call.
    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| error.message())
}

#[cfg(not(windows))]
fn shell_execute(file: &str) -> Result<(), String> {
    Err(format!(
        "{file} is a Windows shortcut or app; Pane opens those only on Windows"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(name: &str, place: Place) -> Found {
        Found {
            path: PathBuf::from(format!(r"C:\Menu\{name}.lnk")),
            name: name.into(),
            location: r"C:\Menu".into(),
            place,
        }
    }

    fn target(target: &str) -> Option<Shortcut> {
        Some(Shortcut {
            target: target.into(),
            ..Shortcut::default()
        })
    }

    fn item(name: &str, parsing: &str) -> (String, String) {
        (name.to_owned(), parsing.to_owned())
    }

    #[test]
    fn packaged_apps_without_a_shortcut_are_added_by_their_package_family() {
        let found = vec![
            found("Notepad++", Place::UserStartMenu),
            found("Paint", Place::UserStartMenu),
        ];
        let shortcuts = vec![target(r"C:\Program Files\Notepad++\notepad++.exe"), None];
        let sources = sources(
            found,
            shortcuts,
            vec![
                item(
                    "Calculator",
                    "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
                ),
                // A shortcut of the same name is already listed.
                item("paint", "Microsoft.Paint_8wekyb3d8bbwe!App"),
                // A desktop program's entry: covered by its shortcut, or not an app.
                item("Notepad++", r"C:\Program Files\Notepad++\notepad++.exe"),
                item("Control Panel", "{26EE0668-A00A-44D7-9371-BEB064C98683}\\0"),
                item("Help", "https://example.com/help"),
                item("", "Nameless_1234!App"),
                // Listed twice by the shell: once.
                item(
                    "Calculator",
                    "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
                ),
            ],
        );

        let names: Vec<&str> = sources.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(names, ["Notepad++", "Paint", "Calculator"]);
        assert_eq!(
            sources[2].path,
            r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        );
        assert_eq!(
            sources[2].key,
            Key::Package {
                family: "microsoft.windowscalculator_8wekyb3d8bbwe".into(),
                app: None
            }
        );
        assert_eq!(sources[2].place, Place::PackagedApps as usize);
        assert_eq!(
            sources[0].key,
            Key::program(r"C:\Program Files\Notepad++\notepad++.exe", "")
        );
        // A shortcut that could not be read is keyed by its own path.
        assert_eq!(sources[1].key, Key::Path(r"c:\menu\paint.lnk".into()));
    }

    #[test]
    fn a_shortcut_to_a_packaged_app_is_that_app() {
        let shortcut = Some(Shortcut {
            app_user_model_id: Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".into()),
            ..Shortcut::default()
        });
        let sources = sources(
            vec![found("Terminal", Place::UserStartMenu)],
            vec![shortcut],
            vec![item(
                "Terminal",
                "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App",
            )],
        );

        // Both are sources of one application: the shortcut, then the app.
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].key, sources[1].key);
        let catalog = super::super::Catalog::new(sources);
        let applications = catalog.applications();
        assert_eq!(applications.len(), 1);
        assert_eq!(
            catalog.find(&applications[0].id).unwrap().primary().path,
            r"C:\Menu\Terminal.lnk"
        );
    }

    #[test]
    fn a_desktop_program_s_own_app_user_model_id_does_not_make_it_a_packaged_app() {
        let shortcut = Some(Shortcut {
            target: r"C:\Tool\tool.exe".into(),
            arguments: String::new(),
            app_user_model_id: Some("Vendor.Tool".into()),
        });
        let sources = sources(
            vec![found("Tool", Place::UserStartMenu)],
            vec![shortcut],
            vec![],
        );
        assert_eq!(sources[0].key, Key::program(r"C:\Tool\tool.exe", ""));
    }

    #[test]
    fn places_are_preferred_as_explorer_prefers_them() {
        assert!(Place::UserDesktop < Place::AllUsersDesktop);
        assert!(Place::AllUsersDesktop < Place::UserStartMenu);
        assert!(Place::UserStartMenu < Place::AllUsersStartMenu);
        assert!(Place::AllUsersStartMenu < Place::TaskbarPins);
        assert!(Place::TaskbarPins < Place::PackagedApps);
    }
}
