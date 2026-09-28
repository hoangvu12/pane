//! The launcher model: the public host interface driven by the native window
//! and by tests alike.
//!
//! Every user action is a method on [`Launcher`]; [`Launcher::view`] returns a
//! snapshot of what the window should show. Actions that call into an
//! extension update the snapshot immediately (for example to "running") and
//! return a future that applies the extension's reply when awaited.

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::runtime::{CallError, FieldKind, FieldValue, Form, Item, Runtime};

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
    /// A form opened from an item of the command's list view.
    Form,
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
    /// An extension call is in progress.
    Running,
    /// The extension's answer to the most recent action.
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
    commands: Arc<[CommandRegistration]>,
    state: Arc<Mutex<State>>,
}

struct State {
    view: LauncherView,
    /// The command whose view is open, as an index into `commands`.
    open: Option<usize>,
    /// The open command's items, as its extension produced them.
    items: Vec<Item>,
    /// While a form is open: the item it belongs to and the command view to
    /// return to.
    form: Option<(String, LauncherView)>,
    /// Incremented on every navigation, so a reply that arrives after the
    /// user has left the screen it was requested from is discarded.
    screen_generation: u64,
}

impl Launcher {
    /// Creates a launcher at root search. A runtime that failed to start
    /// leaves navigation usable and explains the failure when a command opens.
    pub fn new(runtime: Result<Runtime, CallError>, commands: Vec<CommandRegistration>) -> Self {
        let commands: Arc<[CommandRegistration]> = commands.into();
        let state = State {
            view: root_view(&commands),
            open: None,
            items: Vec::new(),
            form: None,
            screen_generation: 0,
        };
        Launcher {
            runtime,
            commands,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn view(&self) -> LauncherView {
        self.lock().view.clone()
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

    /// Leaves an open form for its command's list, or an open command for
    /// root search.
    pub fn back(&self) {
        let mut state = self.lock();
        match state.view.screen {
            Screen::Form => {
                let (_, command_view) = state.form.take().expect("a form is open");
                state.screen_generation += 1;
                state.view = LauncherView {
                    status: Status::Idle,
                    ..command_view
                };
            }
            Screen::Command => {
                state.open = None;
                state.items.clear();
                state.screen_generation += 1;
                state.view = root_view(&self.commands);
            }
            Screen::Root => {}
        }
    }

    /// Opens the selected command (root), or opens the selected item's form
    /// or runs its action (command view). Await the returned future to apply
    /// the reply.
    pub fn activate_selected(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let selected = state.view.selected;
        let call = match (state.view.screen, state.open, selected) {
            (Screen::Root, _, Some(index)) => Some(Call::Open(index)),
            (Screen::Command, Some(command), Some(index)) => {
                let id = state.view.rows[index].id.clone();
                let form = state
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .and_then(|item| item.form.clone());
                match form {
                    Some(form) => {
                        open_form(&mut state, id, form);
                        None
                    }
                    None => Some(Call::Run(command, id)),
                }
            }
            _ => None,
        };
        if call.is_some() {
            state.view.status = Status::Running;
        }
        let generation = state.screen_generation;
        drop(state);
        let launcher = self.clone();
        async move {
            match call {
                Some(Call::Open(index)) => launcher.open_command(generation, index).await,
                Some(Call::Run(command, item_id)) => {
                    launcher.run_action(generation, command, item_id).await
                }
                None => {}
            }
        }
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
    /// rejection next to its field.
    pub fn submit_form(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let submission = match (&state.view.form, &state.form, state.open) {
            (Some(form), Some((item_id, _)), Some(command)) => {
                let values: Vec<FieldValue> = form
                    .fields
                    .iter()
                    .map(|field| FieldValue {
                        id: field.id.clone(),
                        value: field.value.clone(),
                    })
                    .collect();
                Some((command, item_id.clone(), values))
            }
            _ => None,
        };
        if submission.is_some() {
            state.view.status = Status::Running;
        }
        let generation = state.screen_generation;
        drop(state);
        let launcher = self.clone();
        async move {
            if let Some((command, item_id, values)) = submission {
                launcher.submit(generation, command, item_id, values).await
            }
        }
    }

    async fn submit(
        &self,
        generation: u64,
        index: usize,
        item_id: String,
        values: Vec<FieldValue>,
    ) {
        let component = &self.commands[index].component;
        let result = match self.runtime() {
            Ok(runtime) => runtime.submit_form(component, &item_id, values).await,
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
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
                        field.error = Some(error.message);
                        Status::Error(status)
                    }
                    None => Status::Error(error.message),
                }
            }
            Err(error) => Status::Error(error.to_string()),
        };
    }

    async fn run_action(&self, generation: u64, index: usize, item_id: String) {
        let component = &self.commands[index].component;
        let result = match self.runtime() {
            Ok(runtime) => runtime.run_action(component, &item_id).await,
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

    async fn open_command(&self, generation: u64, index: usize) {
        let component = &self.commands[index].component;
        let result = match self.runtime() {
            Ok(runtime) => runtime.get_view(component).await,
            Err(error) => Err(error),
        };
        let Some(mut state) = self.lock_if_current(generation) else {
            return;
        };
        match result {
            Ok(view) => {
                let rows: Vec<Row> = view
                    .items
                    .iter()
                    .map(|item| Row {
                        id: item.id.clone(),
                        title: item.title.clone(),
                        subtitle: item.subtitle.clone(),
                    })
                    .collect();
                state.open = Some(index);
                state.items = view.items;
                state.screen_generation += 1;
                state.view = LauncherView {
                    screen: Screen::Command,
                    title: view.title,
                    selected: first_index(&rows),
                    rows,
                    status: Status::Idle,
                    form: None,
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

enum Call {
    /// Open the command at this index.
    Open(usize),
    /// Run an item's action in the command at this index.
    Run(usize, String),
}

/// Replaces the command view with `form`, which belongs to item `item_id`.
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
        rows: Vec::new(),
        selected: None,
        status: Status::Idle,
        form: Some(FormView {
            fields,
            submit_label: form.submit_label,
        }),
    };
    let command_view = std::mem::replace(&mut state.view, form_view);
    state.form = Some((item_id, command_view));
    state.screen_generation += 1;
}

fn root_view(commands: &[CommandRegistration]) -> LauncherView {
    let rows: Vec<Row> = commands
        .iter()
        .map(|command| Row {
            id: command.id.clone(),
            title: command.title.clone(),
            subtitle: command.subtitle.clone(),
        })
        .collect();
    LauncherView {
        screen: Screen::Root,
        title: "Pane".into(),
        selected: first_index(&rows),
        rows,
        status: Status::Idle,
        form: None,
    }
}

fn first_index(rows: &[Row]) -> Option<usize> {
    (!rows.is_empty()).then_some(0)
}
