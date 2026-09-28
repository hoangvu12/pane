//! The extension runtime: a Wasmtime engine that registers only WASI 0.3.
//!
//! The runtime owns every engine, store and guest instance on one dedicated
//! thread. Callers hold a cheap [`Runtime`] handle and await replies, so a slow
//! or failing guest never blocks the caller's thread.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};

use tokio::sync::{mpsc, oneshot};
use wasmtime::component::{Component, Linker, ResourceAny, ResourceTable};
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
    /// When set (and `form` is not), activating the item opens this custom
    /// view instead of running its action.
    pub custom_view: Option<CustomViewInfo>,
}

/// What Pane shows of an item's custom view besides the view's drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomViewInfo {
    /// The screen's title.
    pub title: String,
    /// Names the view to assistive technology.
    pub label: String,
    pub role: CustomViewRole,
}

/// What kind of control a custom view is to assistive technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomViewRole {
    /// A color chooser; its frame's value names the chosen color.
    ColorWell,
}

/// What a custom view shows: shapes painted in order over a
/// `width` x `height` area of logical pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub shapes: Vec<Shape>,
    /// The view's current value for assistive technology.
    pub value: String,
}

/// One thing a custom view draws. Coordinates are logical pixels from the
/// view's top-left corner; colors are 0xRRGGBB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Rect {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        fill: u32,
    },
    /// One line of text, its top-left corner at `x`, `y`.
    Text {
        x: i32,
        y: i32,
        content: String,
        color: u32,
    },
}

/// A position in a custom view, in logical pixels from its top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// The keys a focused custom view receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// The user's input to a custom view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewEvent {
    Key(Key),
    /// The primary pointer button was pressed over the view.
    PointerDown(Point),
    /// The pointer moved while that button is held.
    PointerMove(Point),
    /// That button was released.
    PointerUp(Point),
}

/// Identifies a custom view open in a [`Runtime`]. Ids are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewId(u64);

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
    /// The custom view was closed, or its guest instance has stopped, so it
    /// cannot handle events any more.
    ViewClosed,
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
            CallError::ViewClosed => f.write_str("The extension's view is no longer open"),
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
    OpenView {
        component: PathBuf,
        item_id: String,
        reply: oneshot::Sender<Result<(ViewId, Frame), CallError>>,
    },
    ViewEvent {
        view: ViewId,
        event: ViewEvent,
        reply: oneshot::Sender<Result<Frame, CallError>>,
    },
    CloseView {
        view: ViewId,
    },
    OpenViews {
        reply: oneshot::Sender<usize>,
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

    /// Opens the custom view of `item_id` in the command in `component` and
    /// draws it. The view stays open, holding its state in the guest, until
    /// [`Runtime::close_view`] or until its instance stops.
    pub async fn open_view(
        &self,
        component: &Path,
        item_id: &str,
    ) -> Result<(ViewId, Frame), CallError> {
        let (reply, response) = oneshot::channel();
        self.send(Request::OpenView {
            component: component.to_path_buf(),
            item_id: item_id.to_owned(),
            reply,
        })?;
        response.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Has the open custom view `view` handle `event`, then draws it again.
    /// A view that has closed answers [`CallError::ViewClosed`].
    ///
    /// The event is sent when this is called, not when the returned future
    /// is first polled: events are handled one at a time, in the order of
    /// these calls.
    pub fn view_event(
        &self,
        view: ViewId,
        event: ViewEvent,
    ) -> impl Future<Output = Result<Frame, CallError>> + Send + 'static {
        let (reply, response) = oneshot::channel();
        let sent = self.send(Request::ViewEvent { view, event, reply });
        async move {
            sent?;
            response.await.unwrap_or_else(|_| Err(stopped()))
        }
    }

    /// Closes the custom view `view`: the guest's view is dropped, after any
    /// event already sent to it, and later events are refused.
    pub fn close_view(&self, view: ViewId) {
        // A stopped runtime holds no views.
        let _ = self.send(Request::CloseView { view });
    }

    /// How many custom views are open, counting the requests sent before
    /// this call; for diagnostics and tests.
    pub async fn open_views(&self) -> usize {
        let (reply, response) = oneshot::channel();
        if self.send(Request::OpenViews { reply }).is_err() {
            return 0;
        }
        response.await.unwrap_or(0)
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

/// A custom view open in a guest instance.
struct OpenView {
    /// The component whose instance holds the view.
    component: PathBuf,
    /// The guest's `custom-view` resource.
    resource: ResourceAny,
}

/// Runtime-thread state: compiled components, their live instances and the
/// custom views open in them.
struct Host {
    engine: Engine,
    linker: Linker<GuestState>,
    components: HashMap<PathBuf, Component>,
    instances: HashMap<PathBuf, Instance>,
    views: HashMap<ViewId, OpenView>,
    next_view: u64,
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
            views: HashMap::new(),
            next_view: 0,
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
                        self.drop_instance(component);
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
                Request::OpenView {
                    component,
                    item_id,
                    reply,
                } => {
                    let result = self.open_view(&component, item_id).await;
                    let _ = reply.send(result);
                }
                Request::ViewEvent { view, event, reply } => {
                    let result = self.view_event(view, event).await;
                    let _ = reply.send(result);
                }
                Request::CloseView { view } => self.close_view(view).await,
                Request::OpenViews { reply } => {
                    let _ = reply.send(self.views.len());
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

    async fn open_view(
        &mut self,
        path: &Path,
        item_id: String,
    ) -> Result<(ViewId, Frame), CallError> {
        let instance = self.instance(path).await?;
        let command = instance.bindings.pane_extension_command();
        let result = instance
            .store
            .run_concurrent(async |store| command.call_open_view(store, item_id).await)
            .await;
        let resource = self.settle(path, result, CallError::Guest)?;
        let view = ViewId(self.next_view);
        self.next_view += 1;
        self.views.insert(
            view,
            OpenView {
                component: path.to_path_buf(),
                resource,
            },
        );
        match self.render(view).await {
            Ok(frame) => Ok((view, frame)),
            Err(error) => {
                self.close_view(view).await;
                Err(error)
            }
        }
    }

    async fn view_event(&mut self, view: ViewId, event: ViewEvent) -> Result<Frame, CallError> {
        let (path, resource) = self.view(view)?;
        let instance = self.instance(&path).await?;
        let custom_view = instance.bindings.pane_extension_command().custom_view();
        let event = command::ViewEvent::from(event);
        let result = instance
            .store
            .run_concurrent(async |store| {
                custom_view.call_handle_event(store, resource, event).await
            })
            .await;
        self.settle(&path, result, CallError::Guest)?;
        self.render(view).await
    }

    /// Asks the guest to draw the open view `view`.
    async fn render(&mut self, view: ViewId) -> Result<Frame, CallError> {
        let (path, resource) = self.view(view)?;
        let instance = self.instance(&path).await?;
        let custom_view = instance.bindings.pane_extension_command().custom_view();
        let result = instance
            .store
            .run_concurrent(async |store| custom_view.call_render(store, resource).await)
            .await;
        let frame = self.settle(&path, result.map(|frame| frame.map(Ok)), |never| never)?;
        Ok(Frame::from(frame))
    }

    /// The component and guest resource of the open view `view`.
    fn view(&self, view: ViewId) -> Result<(PathBuf, ResourceAny), CallError> {
        let open = self.views.get(&view).ok_or(CallError::ViewClosed)?;
        Ok((open.component.clone(), open.resource))
    }

    /// Drops the guest's view `view`, running its destructor.
    async fn close_view(&mut self, view: ViewId) {
        let Some(open) = self.views.remove(&view) else {
            return;
        };
        if let Some(instance) = self.instances.get_mut(&open.component)
            && open
                .resource
                .resource_drop_async(&mut instance.store)
                .await
                .is_err()
        {
            // The destructor trapped: the instance cannot be re-entered.
            self.drop_instance(&open.component);
        }
    }

    /// Drops the live instance of `path` and forgets the views open in it,
    /// which went with it.
    fn drop_instance(&mut self, path: &Path) {
        self.instances.remove(path);
        self.views.retain(|_, view| view.component != path);
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
                self.drop_instance(path);
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
            custom_view: item.custom_view.map(|info| CustomViewInfo {
                title: info.title,
                label: info.label,
                role: match info.role {
                    command::CustomViewRole::ColorWell => CustomViewRole::ColorWell,
                },
            }),
        }
    }
}

impl From<command::Frame> for Frame {
    fn from(frame: command::Frame) -> Frame {
        let shapes = frame
            .shapes
            .into_iter()
            .map(|shape| match shape {
                command::Shape::Rect(rect) => Shape::Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    fill: rect.fill,
                },
                command::Shape::Text(text) => Shape::Text {
                    x: text.x,
                    y: text.y,
                    content: text.content,
                    color: text.color,
                },
            })
            .collect();
        Frame {
            width: frame.width,
            height: frame.height,
            shapes,
            value: frame.value,
        }
    }
}

impl From<ViewEvent> for command::ViewEvent {
    fn from(event: ViewEvent) -> command::ViewEvent {
        let point = |Point { x, y }| command::Point { x, y };
        match event {
            ViewEvent::Key(key) => command::ViewEvent::Key(match key {
                Key::Left => command::Key::Left,
                Key::Right => command::Key::Right,
                Key::Up => command::Key::Up,
                Key::Down => command::Key::Down,
                Key::Home => command::Key::Home,
                Key::End => command::Key::End,
            }),
            ViewEvent::PointerDown(at) => command::ViewEvent::PointerDown(point(at)),
            ViewEvent::PointerMove(at) => command::ViewEvent::PointerMove(point(at)),
            ViewEvent::PointerUp(at) => command::ViewEvent::PointerUp(point(at)),
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
