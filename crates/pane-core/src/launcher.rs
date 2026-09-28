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

use crate::packages::{
    InstalledPackage, PackageError, PackageIdentity, SourcePackage, Store, folder_name,
};
use crate::runtime::{CallError, FieldKind, FieldValue, Form, Runtime};
use crate::settings::{PackageSettings, Settings};

/// The id of the root row that installs a package from a local folder.
const INSTALL_FROM_FOLDER: &str = "pane.install-from-folder";

/// The id of the root row that lists installed packages to enable or
/// disable them.
const MANAGE_EXTENSIONS: &str = "pane.manage-extensions";

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
    /// A form opened from an item of the command's list view.
    Form,
    /// The installed packages, each enabled or disabled.
    Extensions,
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
    /// Why the most recent action failed. For a rejected form field this is
    /// "<field label>: <message>".
    Error(String),
}

/// An open form, as the user is filling it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormView {
    pub fields: Vec<FormField>,
    pub submit_label: String,
}

/// One field of an open form with its current value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    /// The text of a text field, or the id of the chosen option.
    pub value: String,
    /// Why the extension rejected this field on the last submission.
    pub error: Option<String>,
}

/// A snapshot of what the launcher shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherView {
    pub screen: Screen,
    pub title: String,
    /// Lines of information under the title, such as a package's source and
    /// compatibility.
    pub details: Vec<String>,
    /// Empty on the form screen.
    pub rows: Vec<Row>,
    /// Index into `rows`; `None` when there are no rows.
    pub selected: Option<usize>,
    pub status: Status,
    /// The open form; `Some` exactly on the form screen.
    pub form: Option<FormView>,
}

/// The launcher. Cloning shares the same state.
#[derive(Clone)]
pub struct Launcher {
    runtime: Result<Runtime, CallError>,
    /// Commands supplied by this build rather than by installed packages.
    commands: Arc<[CommandRegistration]>,
    /// Pane's managed package location, when installing packages is on.
    store: Option<Arc<Mutex<Store>>>,
    /// The installed packages' settings, kept with `store`.
    settings: Option<Settings>,
    state: Arc<Mutex<State>>,
}

struct State {
    view: LauncherView,
    /// What activating each row of the current screen does.
    entries: Vec<Entry>,
    /// The component of the command whose view is open.
    open: Option<PathBuf>,
    /// The form on screen, if one is open.
    form: Option<OpenForm>,
    /// Incremented on every navigation, so a reply that arrives after the
    /// user has left the screen it was requested from is discarded.
    screen_generation: u64,
    packages: Vec<InstalledPackage>,
    /// Why the installed packages could not be read, if they could not.
    store_problem: Option<String>,
}

/// What the launcher keeps about the open form besides its view.
struct OpenForm {
    /// The item of the open command that the form belongs to.
    item_id: String,
    /// The command view that Back returns to.
    return_to: LauncherView,
    /// Whether a submission is waiting for the extension's reply; further
    /// submissions are ignored meanwhile.
    submitting: bool,
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
    /// Open this form of the open command's item with this id.
    Form(String, Form),
    /// Install the previewed package from this folder, or replace its
    /// installed copy.
    Install(PathBuf, Mode),
    /// Show the installed packages (root).
    Manage,
    /// Enable this installed package if it is disabled, else disable it.
    Toggle(PackageIdentity),
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
        let settings = Settings::open(&packages_dir);
        Launcher::create(
            runtime,
            commands,
            Some((Store::open(packages_dir), settings)),
        )
    }

    fn create(
        runtime: Result<Runtime, CallError>,
        commands: Vec<CommandRegistration>,
        store: Option<(Store, Settings)>,
    ) -> Self {
        let (store, settings) = store.unzip();
        let state = State {
            view: LauncherView {
                screen: Screen::Root,
                title: String::new(),
                details: Vec::new(),
                rows: Vec::new(),
                selected: None,
                status: Status::Idle,
                form: None,
            },
            entries: Vec::new(),
            open: None,
            form: None,
            screen_generation: 0,
            packages: store.as_ref().map(Store::installed).unwrap_or_default(),
            store_problem: store.as_ref().and_then(Store::problem),
        };
        if let Some(settings) = &settings {
            for package in state.packages.iter().filter(|package| !package.enabled) {
                settings.set_enabled(&package.identity, false);
            }
        }
        let launcher = Launcher {
            runtime,
            commands: commands.into(),
            store: store.map(|store| Arc::new(Mutex::new(store))),
            settings,
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

    /// Leaves an open form for its command's list, or an open command,
    /// package preview or the extension list for root search.
    pub fn back(&self) {
        let mut state = self.lock();
        match state.view.screen {
            Screen::Form => {
                let form = state.form.take().expect("a form is open");
                state.screen_generation += 1;
                state.view = LauncherView {
                    status: Status::Idle,
                    ..form.return_to
                };
            }
            Screen::Command | Screen::Package | Screen::Extensions => {
                self.show_root(&mut state, None)
            }
            Screen::Root => {}
        }
    }

    /// Opens the selected command (root), opens the selected item's form or
    /// runs its action (command view), or installs or updates the previewed
    /// package. Await the returned future to apply the reply.
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
            Some(Entry::Form(item_id, form)) => {
                open_form(&mut state, item_id, form);
                None
            }
            Some(Entry::Manage) => {
                self.show_extensions(&mut state);
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
                Some(Entry::Toggle(identity)) => {
                    let enabled = launcher
                        .lock()
                        .packages
                        .iter()
                        .find(|package| package.identity == identity)
                        .is_some_and(|package| package.enabled);
                    launcher
                        .change_enabled(generation, identity, !enabled)
                        .await
                }
                Some(
                    Entry::Broken(_) | Entry::InstallFromFolder | Entry::Manage | Entry::Form(..),
                )
                | None => {}
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
            state.form = None;
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

    /// Enables or disables the installed package with `identity` and
    /// records the choice, so it holds after a restart. A disabled package's
    /// commands leave root search, an open one closes, and its running
    /// instances are dropped; its settings are kept for when it is enabled
    /// again. Other installations, even with the same title, are unaffected.
    pub fn set_enabled(
        &self,
        identity: &PackageIdentity,
        enabled: bool,
    ) -> impl Future<Output = ()> + Send + 'static {
        let generation = self.start_running();
        let launcher = self.clone();
        let identity = identity.clone();
        async move { launcher.change_enabled(generation, identity, enabled).await }
    }

    async fn change_enabled(&self, generation: u64, identity: PackageIdentity, enabled: bool) {
        let result = match &self.store {
            None => Err(PackageError::Storage(
                "this launcher does not install packages".into(),
            )),
            Some(store) => {
                let store = store.clone();
                let identity = identity.clone();
                off_thread(move || {
                    let mut store = store.lock().unwrap_or_else(|p| p.into_inner());
                    store.set_enabled(&identity, enabled)
                })
                .await
            }
        };
        let mut state = self.lock();
        let current = state.screen_generation == generation;
        if let Err(error) = result {
            if current {
                state.view.status = Status::Error(error.to_string());
            }
            return;
        }
        if let Some(settings) = &self.settings {
            settings.set_enabled(&identity, enabled);
        }
        let Some(package) = state
            .packages
            .iter_mut()
            .find(|package| package.identity == identity)
        else {
            return;
        };
        package.enabled = enabled;
        let message = match enabled {
            true => format!("Enabled {}", package.title()),
            false => format!("Disabled {}", package.title()),
        };
        let components: Vec<PathBuf> = package
            .commands()
            .into_iter()
            .map(|c| c.component)
            .collect();
        if !enabled {
            // Its instances stop; enabling it again starts fresh ones.
            if let Ok(runtime) = self.runtime() {
                runtime.forget(components.iter().cloned());
            }
            if state
                .open
                .as_ref()
                .is_some_and(|open| components.contains(open))
            {
                self.show_root(&mut state, None);
                state.view.status = Status::Result(message);
                return;
            }
        }
        match state.view.screen {
            Screen::Root => self.refresh_root(&mut state),
            Screen::Extensions => self.refresh_extensions(&mut state),
            // Other screens show no package state.
            _ => {}
        }
        if current {
            state.view.status = Status::Result(message);
        }
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
        state.form = None;
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
            form: None,
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
        // A disabled package contributes nothing to root search.
        let enabled = || state.packages.iter().filter(|package| package.enabled);
        let installed = enabled().flat_map(InstalledPackage::commands);
        for command in self.commands.iter().cloned().chain(installed) {
            let entry = Entry::Open(command.component);
            let row = Row {
                id: command.id,
                title: command.title,
                subtitle: command.subtitle,
            };
            add(row, entry);
        }
        for package in enabled() {
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
        if self.store.is_some() && !state.packages.is_empty() {
            let row = Row {
                id: MANAGE_EXTENSIONS.into(),
                title: "Manage extensions…".into(),
                subtitle: Some("Enable or disable installed extensions".into()),
            };
            add(row, Entry::Manage);
        }
        (rows, entries)
    }

    /// Sets the value of the open form's field `field_id`: a text field's
    /// text, or the id of an option of a choice field. Unknown fields and
    /// options are ignored. Editing a field clears its error.
    pub fn set_field_value(&self, field_id: &str, value: &str) {
        let mut state = self.lock();
        let Some(form) = state.view.form.as_mut() else {
            return;
        };
        let Some(field) = form.fields.iter_mut().find(|field| field.id == field_id) else {
            return;
        };
        if let FieldKind::Choice(choices) = &field.kind
            && !choices.iter().any(|choice| choice.id == value)
        {
            return;
        }
        field.value = value.to_owned();
        field.error = None;
    }

    /// Submits the open form to its extension. Await the returned future to
    /// apply the reply: the answer as the result, or the extension's
    /// rejection next to its field. While a submission is waiting for its
    /// reply, submitting again does nothing.
    pub fn submit_form(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let state = &mut *state;
        let submission = match (&state.view.form, &mut state.form, &state.open) {
            (Some(form), Some(open), Some(component)) if !open.submitting => {
                open.submitting = true;
                let values: Vec<FieldValue> = form
                    .fields
                    .iter()
                    .map(|field| FieldValue {
                        id: field.id.clone(),
                        value: field.value.clone(),
                    })
                    .collect();
                Some((component.clone(), open.item_id.clone(), values))
            }
            _ => None,
        };
        if submission.is_some() {
            state.view.status = Status::Running;
        }
        let generation = state.screen_generation;
        let launcher = self.clone();
        async move {
            if let Some((component, item_id, values)) = submission {
                launcher
                    .submit(generation, component, item_id, values)
                    .await
            }
        }
    }

    /// Sends `values` and applies the reply as though it had arrived before
    /// any edit made meanwhile: a rejected field that the user has changed
    /// since is not marked, since editing a field clears its error.
    async fn submit(
        &self,
        generation: u64,
        component: PathBuf,
        item_id: String,
        values: Vec<FieldValue>,
    ) {
        let settings = self.settings_of(&component);
        let result = match self.runtime() {
            Ok(runtime) => {
                runtime
                    .submit_form_with(&component, &item_id, values.clone(), settings)
                    .await
            }
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
        let state = &mut *state;
        state.form.as_mut().expect("a form is open").submitting = false;
        let view = &mut state.view;
        let fields = &mut view.form.as_mut().expect("a form is open").fields;
        for field in fields.iter_mut() {
            field.error = None;
        }
        view.status = match result {
            Ok(answer) => Status::Result(answer),
            Err(CallError::Form(error)) => {
                let field = error
                    .field
                    .as_deref()
                    .and_then(|id| fields.iter_mut().find(|field| field.id == id));
                match field {
                    Some(field) => {
                        let status = format!("{}: {}", field.label, error.message);
                        let unchanged = values
                            .iter()
                            .any(|sent| sent.id == field.id && sent.value == field.value);
                        if unchanged {
                            field.error = Some(error.message);
                        }
                        Status::Error(status)
                    }
                    // A rejection of the form as a whole is the extension's
                    // message to the user, shown as it is.
                    None => Status::Error(error.message),
                }
            }
            Err(error) => Status::Error(error.to_string()),
        };
    }

    /// Shows the installed packages, each enabled or disabled.
    fn show_extensions(&self, state: &mut State) {
        let (rows, entries) = extension_rows(&state.packages);
        state.open = None;
        state.form = None;
        state.screen_generation += 1;
        state.entries = entries;
        state.view = LauncherView {
            screen: Screen::Extensions,
            title: "Extensions".into(),
            details: vec![
                "A disabled extension adds no commands and runs nothing; it keeps its settings."
                    .into(),
            ],
            selected: first_index(&rows),
            rows,
            status: Status::Idle,
            form: None,
        };
    }

    /// Updates the installed packages on screen after one was enabled or
    /// disabled; the rows stay in place, and so does the selection.
    fn refresh_extensions(&self, state: &mut State) {
        let (rows, entries) = extension_rows(&state.packages);
        state.entries = entries;
        state.view.rows = rows;
    }

    async fn run_action(&self, generation: u64, component: PathBuf, item_id: String) {
        let settings = self.settings_of(&component);
        let result = match self.runtime() {
            Ok(runtime) => {
                runtime
                    .run_action_with(&component, &item_id, settings)
                    .await
            }
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
        let settings = self.settings_of(&component);
        let result = match self.runtime() {
            Ok(runtime) => runtime.get_view_with(&component, settings).await,
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
        if let Some(package) = owner(&state.packages, &component)
            && !package.enabled
        {
            // Disabled while it was opening.
            state.view.status = Status::Error(format!("{} is disabled", package.title()));
            return;
        }
        match result {
            Ok(view) => {
                let (rows, entries): (Vec<Row>, Vec<Entry>) = view
                    .items
                    .into_iter()
                    .map(|item| {
                        let entry = match item.form {
                            Some(form) => Entry::Form(item.id.clone(), form),
                            None => Entry::Run(item.id.clone()),
                        };
                        let row = Row {
                            id: item.id,
                            title: item.title,
                            subtitle: item.subtitle,
                        };
                        (row, entry)
                    })
                    .unzip();
                state.entries = entries;
                state.open = Some(component);
                state.screen_generation += 1;
                state.view = LauncherView {
                    screen: Screen::Command,
                    title: view.title,
                    details: Vec::new(),
                    selected: first_index(&rows),
                    rows,
                    status: Status::Idle,
                    form: None,
                };
            }
            Err(error) => state.view.status = Status::Error(error.to_string()),
        }
    }

    /// The settings of the installed package `component` belongs to; `None`
    /// for a command built into Pane.
    fn settings_of(&self, component: &Path) -> Option<PackageSettings> {
        let state = self.lock();
        let package = owner(&state.packages, component)?;
        Some(self.settings.as_ref()?.owned_by(&package.identity))
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

/// The installed package whose managed copy holds `component`.
fn owner<'a>(packages: &'a [InstalledPackage], component: &Path) -> Option<&'a InstalledPackage> {
    packages
        .iter()
        .find(|package| component.starts_with(&package.location))
}

/// One row per installed package, saying whether it is enabled and which
/// source it is, so copies with the same title can be told apart.
fn extension_rows(packages: &[InstalledPackage]) -> (Vec<Row>, Vec<Entry>) {
    packages
        .iter()
        .map(|package| {
            let state = match package.enabled {
                true => "Enabled",
                false => "Disabled",
            };
            let row = Row {
                id: package.identity.key(),
                title: package.title(),
                subtitle: Some(format!("{state} · {}", package.identity)),
            };
            (row, Entry::Toggle(package.identity.clone()))
        })
        .unzip()
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
                form: None,
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
            if !installed.enabled {
                details.push("Disabled: enable it in Manage extensions".into());
            }
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
        form: None,
    };
    (view, vec![entry])
}

/// Replaces the command view with `form`, which belongs to item `item_id`.
/// The command's row entries stay, for when the form closes.
fn open_form(state: &mut State, item_id: String, form: Form) {
    let fields = form
        .fields
        .into_iter()
        .map(|field| {
            let value = match &field.kind {
                FieldKind::Text { .. } => String::new(),
                FieldKind::Choice(choices) => choices
                    .first()
                    .map(|choice| choice.id.clone())
                    .unwrap_or_default(),
            };
            FormField {
                id: field.id,
                label: field.label,
                kind: field.kind,
                value,
                error: None,
            }
        })
        .collect();
    let form_view = LauncherView {
        screen: Screen::Form,
        title: form.title,
        details: Vec::new(),
        rows: Vec::new(),
        selected: None,
        status: Status::Idle,
        form: Some(FormView {
            fields,
            submit_label: form.submit_label,
        }),
    };
    let return_to = std::mem::replace(&mut state.view, form_view);
    state.form = Some(OpenForm {
        item_id,
        return_to,
        submitting: false,
    });
    state.screen_generation += 1;
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
