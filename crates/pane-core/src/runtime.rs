//! The extension runtime: a Wasmtime engine that registers only WASI 0.3.
//!
//! The runtime owns every engine, store and guest instance on one dedicated
//! thread. Callers hold a cheap [`Runtime`] handle and await replies, so a slow
//! or failing guest never blocks the caller's thread.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use tokio::sync::{mpsc, oneshot};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Cache, CacheConfig, Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "extension",
        exports: { default: async | store },
    });
}

use bindings::exports::pane::extension::command;

/// Interface-version prefix every imported WASI interface must carry.
const WASI_VERSION: &str = "@0.3.";

/// One entry in a command's list view, as produced by the guest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// When set, activating the item opens this form instead of running its
    /// action.
    pub form: Option<Form>,
}

/// A form an item opens, as produced by the guest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub title: String,
    pub fields: Vec<Field>,
    pub submit_label: String,
}

/// One field of a [`Form`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
}

/// What a field holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A single-line text field, which starts empty.
    Text { placeholder: Option<String> },
    /// Exactly one of these options; the first starts chosen.
    Choice(Vec<Choice>),
}

/// An option of a choice field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

/// A field's submitted value: its text, or the chosen option's id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldValue {
    pub id: String,
    pub value: String,
}

/// Why the guest did not accept a submitted form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError {
    /// The field the message is about; `None` for the form as a whole.
    pub field: Option<String>,
    pub message: String,
}

/// A command's list view, as produced by the guest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub title: String,
    pub items: Vec<Item>,
}

/// Why a call into an extension did not produce a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallError {
    /// The runtime could not start, or has stopped.
    RuntimeUnavailable(String),
    /// The component could not be read or compiled.
    Load(String),
    /// The component needs interfaces Pane does not provide, such as WASI 0.2.
    Incompatible(Vec<String>),
    /// The component does not implement Pane's extension interface.
    Interface(String),
    /// The guest ran and reported an error.
    Guest(String),
    /// The guest did not accept a submitted form.
    Form(FormError),
    /// The guest trapped or otherwise failed while running.
    Trap(String),
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallError::RuntimeUnavailable(reason) => {
                write!(f, "Extension runtime unavailable: {reason}")
            }
            CallError::Load(reason) => write!(f, "Could not load the extension: {reason}"),
            CallError::Incompatible(imports) => write!(
                f,
                "Incompatible extension: Pane supports only WASI 0.3, but it imports {}",
                imports.join(", ")
            ),
            CallError::Interface(reason) => write!(
                f,
                "Incompatible extension: it does not implement Pane's extension interface: {reason}"
            ),
            CallError::Guest(message) => write!(f, "The extension reported an error: {message}"),
            CallError::Form(error) => f.write_str(&error.message),
            CallError::Trap(reason) => write!(f, "The extension crashed: {reason}"),
        }
    }
}

impl std::error::Error for CallError {}

/// A handle to the runtime thread. Cloning shares the same runtime.
#[derive(Clone)]
pub struct Runtime {
    requests: mpsc::UnboundedSender<Request>,
}

enum Request {
    GetView {
        component: PathBuf,
        reply: oneshot::Sender<Result<View, CallError>>,
    },
    RunAction {
        component: PathBuf,
        item_id: String,
        reply: oneshot::Sender<Result<String, CallError>>,
    },
    Check {
        component: PathBuf,
        reply: oneshot::Sender<Result<(), CallError>>,
    },
    Forget {
        components: Vec<PathBuf>,
    },
    SubmitForm {
        component: PathBuf,
        item_id: String,
        values: Vec<FieldValue>,
        reply: oneshot::Sender<Result<String, CallError>>,
    },
}

impl Runtime {
    /// Starts the runtime thread. Extensions are compiled on every start.
    pub fn start() -> Result<Runtime, CallError> {
        Runtime::start_with(None)
    }

    /// Starts the runtime thread, keeping compiled extension code in
    /// `cache_dir` so later runtimes load it instead of recompiling. The
    /// directory holds only disposable data.
    pub fn start_with_cache(cache_dir: PathBuf) -> Result<Runtime, CallError> {
        Runtime::start_with(Some(cache_dir))
    }

    fn start_with(cache_dir: Option<PathBuf>) -> Result<Runtime, CallError> {
        let unavailable =
            |error: &dyn fmt::Display| CallError::RuntimeUnavailable(error.to_string());
        let mut config = Config::new();
        config
            .wasm_component_model(true)
            .wasm_component_model_async(true);
        if let Some(dir) = cache_dir {
            let mut cache = CacheConfig::new();
            cache.with_directory(dir);
            let cache = Cache::new(cache).map_err(|error| unavailable(&error))?;
            config.cache(Some(cache));
        }
        let engine = Engine::new(&config).map_err(|error| unavailable(&error))?;
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| unavailable(&error))?;
        let (requests, receiver) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("pane-extension-runtime".into())
            .spawn(move || executor.block_on(Host::new(engine).serve(receiver)))
            .map_err(|error| unavailable(&error))?;
        Ok(Runtime { requests })
    }

    /// Asks the command in `component` for its list view.
    pub async fn get_view(&self, component: &Path) -> Result<View, CallError> {
        let (reply, response) = oneshot::channel();
        self.send(Request::GetView {
            component: component.to_path_buf(),
            reply,
        })?;
        response.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Runs the action of `item_id` in the command in `component`.
    pub async fn run_action(&self, component: &Path, item_id: &str) -> Result<String, CallError> {
        let (reply, response) = oneshot::channel();
        self.send(Request::RunAction {
            component: component.to_path_buf(),
            item_id: item_id.to_owned(),
            reply,
        })?;
        response.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Checks, without running any guest code, that `component` is a
    /// component Pane can run: it compiles, imports only WASI 0.3 and exports
    /// the extension interface. The check keeps nothing loaded.
    pub async fn check(&self, component: &Path) -> Result<(), CallError> {
        let (reply, response) = oneshot::channel();
        self.send(Request::Check {
            component: component.to_path_buf(),
            reply,
        })?;
        response.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Submits the form of `item_id` in the command in `component`. A
    /// rejection by the guest is [`CallError::Form`].
    pub async fn submit_form(
        &self,
        component: &Path,
        item_id: &str,
        values: Vec<FieldValue>,
    ) -> Result<String, CallError> {
        let (reply, response) = oneshot::channel();
        self.send(Request::SubmitForm {
            component: component.to_path_buf(),
            item_id: item_id.to_owned(),
            values,
            reply,
        })?;
        response.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Drops the compiled code and live instances of `components`, for
    /// example after their files were replaced or removed; a later call
    /// loads the file again. Calls made afterwards see the effect; a call
    /// already in progress finishes first. Nothing coordinates this with a
    /// command the user has open: its instance's state is lost.
    pub fn forget(&self, components: impl IntoIterator<Item = PathBuf>) {
        let components = components.into_iter().collect();
        // A stopped runtime holds nothing to forget.
        let _ = self.send(Request::Forget { components });
    }

    fn send(&self, request: Request) -> Result<(), CallError> {
        self.requests.send(request).map_err(|_| stopped())
    }
}

fn stopped() -> CallError {
    CallError::RuntimeUnavailable("the runtime has stopped".into())
}

struct GuestState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for GuestState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// A running guest instance of one component.
struct Instance {
    store: Store<GuestState>,
    bindings: bindings::Extension,
}

/// Runtime-thread state: compiled components and their live instances.
struct Host {
    engine: Engine,
    linker: Linker<GuestState>,
    components: HashMap<PathBuf, Component>,
    instances: HashMap<PathBuf, Instance>,
}

impl Host {
    fn new(engine: Engine) -> Host {
        let mut linker = Linker::new(&engine);
        // Only WASI 0.3 is registered: no P2 linker and no stubs for unknown
        // imports, so a mixed P2/P3 component cannot instantiate.
        wasmtime_wasi::p3::add_to_linker(&mut linker)
            .expect("registering WASI 0.3 in a fresh linker cannot conflict");
        Host {
            engine,
            linker,
            components: HashMap::new(),
            instances: HashMap::new(),
        }
    }

    async fn serve(mut self, mut requests: mpsc::UnboundedReceiver<Request>) {
        while let Some(request) = requests.recv().await {
            match request {
                Request::GetView { component, reply } => {
                    let result = self.get_view(&component).await;
                    let _ = reply.send(result);
                }
                Request::RunAction {
                    component,
                    item_id,
                    reply,
                } => {
                    let result = self.run_action(&component, item_id).await;
                    let _ = reply.send(result);
                }
                Request::Check { component, reply } => {
                    let _ = reply.send(self.check(&component));
                }
                Request::Forget { components } => {
                    for component in &components {
                        self.components.remove(component);
                        self.instances.remove(component);
                    }
                }
                Request::SubmitForm {
                    component,
                    item_id,
                    values,
                    reply,
                } => {
                    let result = self.submit_form(&component, item_id, values).await;
                    let _ = reply.send(result);
                }
            }
        }
    }

    async fn get_view(&mut self, path: &Path) -> Result<View, CallError> {
        let instance = self.instance(path).await?;
        let command = instance.bindings.pane_extension_command();
        let result = instance
            .store
            .run_concurrent(async |store| command.call_get_view(store).await)
            .await;
        let view = self.settle(path, result, CallError::Guest)?;
        Ok(View {
            title: view.title,
            items: view.items.into_iter().map(Item::from).collect(),
        })
    }

    async fn submit_form(
        &mut self,
        path: &Path,
        item_id: String,
        values: Vec<FieldValue>,
    ) -> Result<String, CallError> {
        let instance = self.instance(path).await?;
        let command = instance.bindings.pane_extension_command();
        let values = values
            .into_iter()
            .map(|FieldValue { id, value }| command::FieldValue { id, value })
            .collect();
        let result = instance
            .store
            .run_concurrent(async |store| command.call_submit_form(store, item_id, values).await)
            .await;
        self.settle(path, result, |error: command::FormError| {
            CallError::Form(FormError {
                field: error.field,
                message: error.message,
            })
        })
    }

    async fn run_action(&mut self, path: &Path, item_id: String) -> Result<String, CallError> {
        let instance = self.instance(path).await?;
        let command = instance.bindings.pane_extension_command();
        let result = instance
            .store
            .run_concurrent(async |store| command.call_run_action(store, item_id).await)
            .await;
        self.settle(path, result, CallError::Guest)
    }

    /// Maps a call outcome to the caller's result, turning the guest's own
    /// error with `guest_error`. A trapped instance cannot be re-entered, so
    /// it is dropped and the next call starts a fresh one.
    fn settle<T, E>(
        &mut self,
        path: &Path,
        outcome: wasmtime::Result<wasmtime::Result<Result<T, E>>>,
        guest_error: impl FnOnce(E) -> CallError,
    ) -> Result<T, CallError> {
        match outcome.and_then(|inner| inner) {
            Ok(result) => result.map_err(guest_error),
            Err(trap) => {
                self.instances.remove(path);
                Err(CallError::Trap(format!("{trap:#}")))
            }
        }
    }

    /// Returns the live instance for `path`, instantiating it on first use.
    async fn instance(&mut self, path: &Path) -> Result<&mut Instance, CallError> {
        if !self.instances.contains_key(path) {
            let component = self.component(path)?.clone();
            let mut store = Store::new(
                &self.engine,
                GuestState {
                    wasi: WasiCtx::builder().build(),
                    table: ResourceTable::new(),
                },
            );
            let bindings =
                bindings::Extension::instantiate_async(&mut store, &component, &self.linker)
                    .await
                    .map_err(|error| CallError::Load(format!("{error:#}")))?;
            self.instances
                .insert(path.to_path_buf(), Instance { store, bindings });
        }
        Ok(self.instances.get_mut(path).expect("inserted above"))
    }

    /// Compiles `path` once and rejects components that import non-0.3 WASI.
    fn component(&mut self, path: &Path) -> Result<&Component, CallError> {
        if !self.components.contains_key(path) {
            let component = self.compile(path)?;
            self.components.insert(path.to_path_buf(), component);
        }
        Ok(&self.components[path])
    }

    fn compile(&self, path: &Path) -> Result<Component, CallError> {
        let component = Component::from_file(&self.engine, path)
            .map_err(|error| CallError::Load(format!("{}: {error:#}", path.display())))?;
        let unsupported: Vec<String> = component
            .component_type()
            .imports(&self.engine)
            .map(|(name, _)| name.to_owned())
            .filter(|name| name.starts_with("wasi:") && !name.contains(WASI_VERSION))
            .collect();
        if !unsupported.is_empty() {
            return Err(CallError::Incompatible(unsupported));
        }
        Ok(component)
    }

    /// Type-checks `path` against the linker and the extension world without
    /// instantiating it, so no guest code runs.
    fn check(&self, path: &Path) -> Result<(), CallError> {
        let component = self.compile(path)?;
        let interface = |error: wasmtime::Error| CallError::Interface(format!("{error:#}"));
        let pre = self.linker.instantiate_pre(&component).map_err(interface)?;
        bindings::ExtensionPre::new(pre).map_err(interface)?;
        Ok(())
    }
}

impl From<command::Item> for Item {
    fn from(item: command::Item) -> Item {
        Item {
            id: item.id,
            title: item.title,
            subtitle: item.subtitle,
            form: item.form.map(Form::from),
        }
    }
}

impl From<command::Form> for Form {
    fn from(form: command::Form) -> Form {
        let fields = form
            .fields
            .into_iter()
            .map(|field| Field {
                id: field.id,
                label: field.label,
                kind: match field.kind {
                    command::FieldKind::Text(text) => FieldKind::Text {
                        placeholder: text.placeholder,
                    },
                    command::FieldKind::Choice(choices) => FieldKind::Choice(
                        choices
                            .into_iter()
                            .map(|choice| Choice {
                                id: choice.id,
                                label: choice.label,
                            })
                            .collect(),
                    ),
                },
            })
            .collect();
        Form {
            title: form.title,
            fields,
            submit_label: form.submit_label,
        }
    }
}
