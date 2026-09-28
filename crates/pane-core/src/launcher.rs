//! The launcher model: the public host interface driven by the native window
//! and by tests alike.
//!
//! Every user action is a method on [`Launcher`]; [`Launcher::view`] returns a
//! snapshot of what the window should show. Actions that call into an
//! extension update the snapshot immediately (for example to "running") and
//! return a future that applies the extension's reply when awaited.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::packages::{InstalledPackage, PackageError, SourcePackage, Store, folder_name};
use crate::runtime::{CallError, Runtime};

/// The id of the root row that installs a package from a local folder.
const INSTALL_FROM_FOLDER: &str = "pane.install-from-folder";

/// A command offered in root search, backed by one extension component.
#[derive(Clone, Debug)]
pub struct CommandRegistration {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub component: PathBuf,
}

/// Which screen the launcher shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Root search: the installed commands.
    Root,
    /// An opened command's list view.
    Command,
    /// A package folder's identity and compatibility, before installing it.
    Package,
}

/// A selectable row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
}

/// Feedback about the most recent action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    /// An extension call or package operation is in progress.
    Running,
    /// The outcome of the most recent action, such as the extension's answer.
    Result(String),
    /// Why the most recent action failed.
    Error(String),
}

/// A snapshot of what the launcher shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherView {
    pub screen: Screen,
    pub title: String,
    /// Lines of information under the title, such as a package's source and
    /// compatibility.
    pub details: Vec<String>,
    pub rows: Vec<Row>,
    /// Index into `rows`; `None` when there are no rows.
    pub selected: Option<usize>,
    pub status: Status,
}

/// The launcher. Cloning shares the same state.
#[derive(Clone)]
pub struct Launcher {
    runtime: Result<Runtime, CallError>,
    /// Commands supplied by this build rather than by installed packages.
    commands: Arc<[CommandRegistration]>,
    /// Pane's managed package location, when installing packages is on.
    store: Option<Arc<Mutex<Store>>>,
    state: Arc<Mutex<State>>,
}

struct State {
    view: LauncherView,
    /// What activating each row of the current screen does.
    entries: Vec<Entry>,
    /// The component of the command whose view is open.
    open: Option<PathBuf>,
    /// Incremented on every navigation, so a reply that arrives after the
    /// user has left the screen it was requested from is discarded.
    screen_generation: u64,
    packages: Vec<InstalledPackage>,
    /// Why the installed packages could not be read, if they could not.
    store_problem: Option<String>,
}

/// What activating a row does.
#[derive(Clone)]
enum Entry {
    /// Open the command with this component (root).
    Open(PathBuf),
    /// Explain why this installed package cannot load (root).
    Broken(String),
    /// Nothing in the launcher: the window asks for a folder (root).
    InstallFromFolder,
    /// Run the open command's item with this id.
    Run(String),
    /// Install the previewed package from this folder, or replace its
    /// installed copy.
    Install(PathBuf, Mode),
}

#[derive(Clone, Copy)]
enum Mode {
    Install,
    Update,
}

impl Launcher {
    /// Creates a launcher at root search. A runtime that failed to start
    /// leaves navigation usable and explains the failure when a command opens.
    pub fn new(runtime: Result<Runtime, CallError>, commands: Vec<CommandRegistration>) -> Self {
        Launcher::create(runtime, commands, None)
    }

    /// Creates a launcher that also offers the commands of the packages
    /// installed in `packages_dir` and installs packages there. Listing them
    /// reads only their manifests: no guest runs until a command opens.
    pub fn with_packages(
        runtime: Result<Runtime, CallError>,
        commands: Vec<CommandRegistration>,
        packages_dir: PathBuf,
    ) -> Self {
        Launcher::create(runtime, commands, Some(Store::open(packages_dir)))
    }

    fn create(
        runtime: Result<Runtime, CallError>,
        commands: Vec<CommandRegistration>,
        store: Option<Store>,
    ) -> Self {
        let state = State {
            view: LauncherView {
                screen: Screen::Root,
                title: String::new(),
                details: Vec::new(),
                rows: Vec::new(),
                selected: None,
                status: Status::Idle,
            },
            entries: Vec::new(),
            open: None,
            screen_generation: 0,
            packages: store.as_ref().map(Store::installed).unwrap_or_default(),
            store_problem: store.as_ref().and_then(Store::problem),
        };
        let launcher = Launcher {
            runtime,
            commands: commands.into(),
            store: store.map(|store| Arc::new(Mutex::new(store))),
            state: Arc::new(Mutex::new(state)),
        };
        launcher.show_root(&mut launcher.lock(), None);
        launcher
    }

    pub fn view(&self) -> LauncherView {
        self.lock().view.clone()
    }

    /// The installed packages, as read from their managed copies.
    pub fn packages(&self) -> Vec<InstalledPackage> {
        self.lock().packages.clone()
    }

    /// Shows `message` as the outcome of the most recent action; for
    /// failures outside the launcher, such as a folder picker that could not
    /// open.
    pub fn show_error(&self, message: impl Into<String>) {
        self.lock().view.status = Status::Error(message.into());
    }

    /// Moves the selection by `delta` rows, clamped to the list.
    pub fn move_selection(&self, delta: isize) {
        let mut state = self.lock();
        let view = &mut state.view;
        if let Some(selected) = view.selected {
            let last = view.rows.len() - 1;
            view.selected = Some(selected.saturating_add_signed(delta).min(last));
        }
    }

    /// Selects the row at `index`, if there is one.
    pub fn select(&self, index: usize) {
        let mut state = self.lock();
        if index < state.view.rows.len() {
            state.view.selected = Some(index);
        }
    }

    /// Whether the selected row installs a package from a folder the user
    /// chooses. Activating it does nothing in the launcher: the window asks
    /// for a folder and calls [`Launcher::preview_package`].
    pub fn selected_asks_for_folder(&self) -> bool {
        let state = self.lock();
        let entry = state
            .view
            .selected
            .and_then(|index| state.entries.get(index));
        matches!(entry, Some(Entry::InstallFromFolder))
    }

    /// Leaves an open command or package preview for root search.
    pub fn back(&self) {
        let mut state = self.lock();
        if state.view.screen != Screen::Root {
            self.show_root(&mut state, None);
        }
    }

    /// Opens the selected command (root), runs the selected item's action
    /// (command view) or installs or updates the previewed package. Await
    /// the returned future to apply the reply.
    ///
    /// A row that [asks for a folder](Launcher::selected_asks_for_folder)
    /// does nothing here.
    pub fn activate_selected(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let entry = state
            .view
            .selected
            .and_then(|index| state.entries.get(index).cloned());
        let entry = match entry {
            Some(Entry::Broken(problem)) => {
                state.view.status = Status::Error(problem);
                None
            }
            Some(Entry::InstallFromFolder) | None => None,
            Some(entry) => {
                state.view.status = Status::Running;
                Some(entry)
            }
        };
        let generation = state.screen_generation;
        let open = state.open.clone();
        drop(state);
        let launcher = self.clone();
        async move {
            match entry {
                Some(Entry::Open(component)) => launcher.open_command(generation, component).await,
                Some(Entry::Run(item_id)) => {
                    if let Some(component) = open {
                        launcher.run_action(generation, component, item_id).await
                    }
                }
                Some(Entry::Install(folder, mode)) => {
                    launcher.install(generation, folder, mode).await
                }
                Some(Entry::Broken(_) | Entry::InstallFromFolder) | None => {}
            }
        }
    }

    /// Reads the package in `folder` and shows its identity, version,
    /// commands and compatibility, offering Install, or Update when a
    /// package with the same identity is installed. No guest code runs. An
    /// invalid or incompatible package is explained instead.
    pub fn preview_package(&self, folder: &Path) -> impl Future<Output = ()> + Send + 'static {
        let generation = self.start_running();
        let launcher = self.clone();
        let folder = folder.to_path_buf();
        async move {
            let checked = launcher.read_and_check(folder.clone()).await;
            let mut state = launcher.lock();
            if state.screen_generation != generation {
                return;
            }
            let installed = checked.as_ref().ok().and_then(|package| {
                state
                    .packages
                    .iter()
                    .find(|installed| installed.identity == package.identity)
                    .cloned()
            });
            let (view, entries) = preview_view(&folder, checked, installed);
            state.screen_generation += 1;
            state.open = None;
            state.view = view;
            state.entries = entries;
        }
    }

    /// Installs the package in `folder` as an explicit install request. A
    /// package whose identity is already installed is rejected: replacing
    /// it is an update.
    pub fn install_package(&self, folder: &Path) -> impl Future<Output = ()> + Send + 'static {
        let generation = self.start_running();
        let launcher = self.clone();
        let folder = folder.to_path_buf();
        async move { launcher.install(generation, folder, Mode::Install).await }
    }

    fn start_running(&self) -> u64 {
        let mut state = self.lock();
        state.view.status = Status::Running;
        state.screen_generation
    }

    async fn install(&self, generation: u64, folder: PathBuf, mode: Mode) {
        let result = match &self.store {
            None => Err(PackageError::Storage(
                "this launcher does not install packages".into(),
            )),
            Some(store) => match self.read_and_check(folder).await {
                Ok(package) => {
                    let store = store.clone();
                    off_thread(move || {
                        let mut store = store.lock().unwrap_or_else(|p| p.into_inner());
                        match mode {
                            Mode::Install => store.install(&package),
                            Mode::Update => store.update(&package),
                        }
                    })
                    .await
                }
                Err(error) => Err(error),
            },
        };
        let mut state = self.lock();
        let current = state.screen_generation == generation;
        match result {
            Ok(installed) => {
                let message = match (mode, installed.version()) {
                    (Mode::Install, _) => format!("Installed {}", installed.title()),
                    (Mode::Update, Some(version)) => {
                        format!("Updated {} to {version}", installed.title())
                    }
                    (Mode::Update, None) => format!("Updated {}", installed.title()),
                };
                let first = installed.commands().first().map(|c| c.component.clone());
                match state
                    .packages
                    .iter_mut()
                    .find(|package| package.identity == installed.identity)
                {
                    Some(package) => {
                        // The replaced copy's code is not run again. A command
                        // of it that is open is not coordinated with (#11, #14).
                        if let Ok(runtime) = self.runtime() {
                            runtime.forget(package.commands().into_iter().map(|c| c.component));
                        }
                        *package = installed;
                    }
                    None => state.packages.push(installed),
                }
                if current {
                    self.show_root(&mut state, first);
                    state.view.status = Status::Result(message);
                } else if state.view.screen == Screen::Root {
                    self.refresh_root(&mut state);
                }
            }
            Err(error) if current => state.view.status = Status::Error(error.to_string()),
            Err(_) => {}
        }
    }

    /// Reads the package in `folder` off the calling thread, then has the
    /// runtime check each component without running it.
    async fn read_and_check(&self, folder: PathBuf) -> Result<SourcePackage, PackageError> {
        let package = off_thread(move || SourcePackage::read(&folder)).await?;
        for (command, component) in package.components() {
            let checked = match self.runtime() {
                Ok(runtime) => runtime.check(&component).await,
                Err(error) => Err(error),
            };
            checked.map_err(|error| PackageError::Component {
                command: command.title.clone(),
                error,
            })?;
        }
        Ok(package)
    }

    /// Shows root search: this build's commands, then the installed
    /// packages' commands, then the install row. Selects the command with
    /// component `select` if given, else the first row.
    fn show_root(&self, state: &mut State, select: Option<PathBuf>) {
        let (rows, entries) = self.root_rows(state);
        let selected = select
            .and_then(|component| {
                entries
                    .iter()
                    .position(|entry| matches!(entry, Entry::Open(c) if *c == component))
            })
            .or_else(|| first_index(&rows));
        state.open = None;
        state.screen_generation += 1;
        state.entries = entries;
        state.view = LauncherView {
            screen: Screen::Root,
            title: "Pane".into(),
            details: Vec::new(),
            rows,
            selected,
            status: match &state.store_problem {
                Some(problem) => Status::Error(problem.clone()),
                None => Status::Idle,
            },
        };
    }

    /// Updates the rows of the root search on screen after the installed
    /// packages changed in the background. Unlike navigating, it keeps the
    /// screen generation, so an action the user started from root still
    /// applies, and it keeps the selection on the same row.
    fn refresh_root(&self, state: &mut State) {
        let selected_id = state
            .view
            .selected
            .and_then(|index| state.view.rows.get(index))
            .map(|row| row.id.clone());
        let (rows, entries) = self.root_rows(state);
        let selected = selected_id
            .and_then(|id| rows.iter().position(|row| row.id == id))
            .or_else(|| first_index(&rows));
        state.entries = entries;
        state.view.rows = rows;
        state.view.selected = selected;
    }

    /// The rows of root search and what activating each does.
    fn root_rows(&self, state: &State) -> (Vec<Row>, Vec<Entry>) {
        let mut rows = Vec::new();
        let mut entries = Vec::new();
        let mut add = |row: Row, entry: Entry| {
            rows.push(row);
            entries.push(entry);
        };
        let installed = state.packages.iter().flat_map(InstalledPackage::commands);
        for command in self.commands.iter().cloned().chain(installed) {
            let entry = Entry::Open(command.component);
            let row = Row {
                id: command.id,
                title: command.title,
                subtitle: command.subtitle,
            };
            add(row, entry);
        }
        for package in &state.packages {
            if let Err(error) = &package.manifest {
                let row = Row {
                    id: package.identity.key(),
                    title: package.title(),
                    subtitle: Some("Cannot load this installed extension".into()),
                };
                let problem = format!(
                    "{} cannot load from {}: {error}",
                    package.title(),
                    package.location.display()
                );
                add(row, Entry::Broken(problem));
            }
        }
        if self.store.is_some() {
            let row = Row {
                id: INSTALL_FROM_FOLDER.into(),
                title: "Install extension from folder…".into(),
                subtitle: Some("Choose a local extension package to install".into()),
            };
            add(row, Entry::InstallFromFolder);
        }
        (rows, entries)
    }

    async fn run_action(&self, generation: u64, component: PathBuf, item_id: String) {
        let result = match self.runtime() {
            Ok(runtime) => runtime.run_action(&component, &item_id).await,
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
        state.view.status = match result {
            Ok(answer) => Status::Result(answer),
            Err(error) => Status::Error(error.to_string()),
        };
    }

    async fn open_command(&self, generation: u64, component: PathBuf) {
        let result = match self.runtime() {
            Ok(runtime) => runtime.get_view(&component).await,
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
        match result {
            Ok(view) => {
                let rows: Vec<Row> = view
                    .items
                    .into_iter()
                    .map(|item| Row {
                        id: item.id,
                        title: item.title,
                        subtitle: item.subtitle,
                    })
                    .collect();
                state.entries = rows.iter().map(|row| Entry::Run(row.id.clone())).collect();
                state.open = Some(component);
                state.screen_generation += 1;
                state.view = LauncherView {
                    screen: Screen::Command,
                    title: view.title,
                    details: Vec::new(),
                    selected: first_index(&rows),
                    rows,
                    status: Status::Idle,
                };
            }
            Err(error) => state.view.status = Status::Error(error.to_string()),
        }
    }

    fn runtime(&self) -> Result<&Runtime, CallError> {
        self.runtime.as_ref().map_err(Clone::clone)
    }

    /// Locks the state only if the screen is still the one of `generation`.
    fn lock_if_current(&self, generation: u64) -> Option<MutexGuard<'_, State>> {
        let state = self.lock();
        (state.screen_generation == generation).then_some(state)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The package screen for `folder`: what the package is and whether it can
/// be installed, or why it cannot.
fn preview_view(
    folder: &Path,
    checked: Result<SourcePackage, PackageError>,
    installed: Option<InstalledPackage>,
) -> (LauncherView, Vec<Entry>) {
    let package = match checked {
        Ok(package) => package,
        Err(error) => {
            let view = LauncherView {
                screen: Screen::Package,
                title: format!("Cannot install {}", folder_name(folder)),
                details: vec![format!("Folder: {}", folder.display())],
                rows: Vec::new(),
                selected: None,
                status: Status::Error(error.to_string()),
            };
            return (view, Vec::new());
        }
    };
    let manifest = &package.manifest;
    let mut details = vec![format!("Source: {}", package.identity)];
    if let Some(version) = &manifest.version {
        details.push(format!("Version: {version}"));
    }
    let titles: Vec<&str> = manifest.commands.iter().map(|c| c.title.as_str()).collect();
    details.push(format!("Commands: {}", titles.join(", ")));
    details.push(format!(
        "Compatible: needs extension API {}, and its components import only WASI 0.3",
        manifest.api_version
    ));
    let (row, entry) = match installed {
        Some(installed) => {
            details.push(match installed.version() {
                Some(version) => format!("Installed: version {version} from this folder"),
                None => "Installed from this folder".into(),
            });
            let row = Row {
                id: "update".into(),
                title: "Update".into(),
                subtitle: Some("Replace the installed copy with this folder's contents".into()),
            };
            (row, Entry::Install(package.folder.clone(), Mode::Update))
        }
        None => {
            let row = Row {
                id: "install".into(),
                title: "Install".into(),
                subtitle: Some("Copy the package into Pane and add its commands".into()),
            };
            (row, Entry::Install(package.folder.clone(), Mode::Install))
        }
    };
    let view = LauncherView {
        screen: Screen::Package,
        title: manifest.title.clone(),
        details,
        rows: vec![row],
        selected: Some(0),
        status: Status::Idle,
    };
    (view, vec![entry])
}

/// Runs blocking file work on its own thread, so the caller's thread (the
/// window's) never waits on the file system.
async fn off_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (reply, response) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = reply.send(work());
    });
    response.await.expect("package file work panicked")
}

fn first_index(rows: &[Row]) -> Option<usize> {
    (!rows.is_empty()).then_some(0)
}
