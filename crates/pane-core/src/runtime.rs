//! The extension runtime: a Wasmtime engine that registers only WASI 0.3.
//!
//! The runtime owns every engine, store and guest instance on one dedicated
//! thread. Callers hold a cheap [`Runtime`] handle and await replies, so a slow
//! or failing guest never blocks the caller's thread.
//!
//! Calls are served one at a time. A call into an installed package belongs
//! to the package's generation (see `generation`): when it ends, a pending
//! call of it stops where the guest waits, and one queued behind is never
//! started. Component checks run on a checker thread of their own, so a
//! reload's check never waits behind the call it is about to stop.
//!
//! A crash of the runtime thread itself (a panic, not a guest trap) stops
//! every call it held without sending any again; Pane restarts the thread,
//! unless it crashed shortly before (see `supervisor`).
//!
//! A guest that stops cooperating is bounded too (see `deadlines`): every
//! guest yields to the runtime thread at each epoch tick, a call computing
//! for too long without finishing is stopped as unresponsive (its package's
//! own failure), and a runtime thread that stops responding altogether is
//! given up on and replaced, as a crashed one is.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use crate::packages::paused_reason;
use wasmtime::component::{Component, Linker, ResourceAny, ResourceTable};
use wasmtime::{Cache, CacheConfig, Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::platform::Platform;

mod deadlines;
mod faults;
mod supervisor;

pub use deadlines::{COMPUTE_LIMIT, UNRESPONSIVE_LIMIT};
use deadlines::{Doing, Meter, Watch};
#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
pub use faults::Fault;
#[cfg(any(test, debug_assertions))]
use faults::Fault as InjectedFault;
use faults::Faults;
pub(crate) use supervisor::CRASH_WINDOW;
use supervisor::{NotSent, Shared};
pub use supervisor::{RuntimeFailure, RuntimeStatus};

pub(crate) mod bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "extension-with-helpers",
        imports: { "pane:extension/operations": store, "pane:extension/helpers": store },
        exports: { default: async | store },
    });
}

/// The `root-results` export of a command that computes root results.
mod root_bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "root-results-provider",
        exports: { default: async | store },
    });
}

/// The `indexed-results` export of a command that supplies root results
/// ahead of the query.
mod indexed_bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "indexed-results-provider",
        exports: { default: async | store },
    });
}

/// The `query-command` export of a command that takes a query.
mod query_bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "query-command-provider",
        exports: { default: async | store },
    });
}

/// The `published-operations` export of a component serving operations.
mod operations_bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "operations-provider",
        exports: { default: async | store },
    });
}

use bindings::exports::pane::extension::command;
use bindings::pane::extension::{applications, cache, content, credentials, settings};
use indexed_bindings::exports::pane::extension::indexed_results;
use root_bindings::exports::pane::extension::root_results;

use crate::applications::Applications;
use crate::extension_data::{DataKind, PackageData};
use crate::generation::{End, Generation};
use crate::helpers;
use crate::helpers::runner::{self, HelperError, HelperErrorKind, Helpers, Running, Spec};
use crate::operations::{self, Directory, OperationCall, OperationError, Target};
use crate::packages::EXTENSION_API;

/// Interface-version prefix every imported WASI interface must carry.
const WASI_VERSION: &str = "@0.3.";

/// The interface an extension command exports.
const COMMAND_INTERFACE: &str = "pane:extension/command@0.1.0";

/// The interface a command that computes root results also exports.
const ROOT_RESULTS_INTERFACE: &str = "pane:extension/root-results@0.1.0";

/// The interface a command that supplies root results ahead of the query
/// also exports.
const INDEXED_RESULTS_INTERFACE: &str = "pane:extension/indexed-results@0.1.0";

/// The interface a command that takes a query also exports.
const QUERY_COMMAND_INTERFACE: &str = "pane:extension/query-command@0.1.0";

/// The interface a component serving published operations also exports.
const OPERATIONS_INTERFACE: &str = "pane:extension/published-operations@0.1.0";

/// A result a command computed from root search's query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RootResult {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub action: RootAction,
}

/// What invoking a computed root result does; Pane performs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RootAction {
    /// Copy this text to the clipboard.
    Copy(String),
    /// Open this web address with the system's link handler.
    OpenUrl(String),
}

/// A root result a command supplies ahead of the query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IndexedResult {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub action: IndexedAction,
}

/// What invoking an indexed root result does; Pane performs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IndexedAction {
    /// Open the installed application with this id.
    OpenApplication(String),
}

/// What a component exports besides `command`, as its package manifest
/// says, for [`Runtime::check_with`] to confirm.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Exports {
    /// `root-results`: it computes root results from the query.
    pub root_results: bool,
    /// `indexed-results`: it supplies root results ahead of the query.
    pub indexed_results: bool,
    /// `query-command`: it takes a query when invoked from root search.
    pub query_command: bool,
    /// `published-operations`: it serves published operations.
    pub operations: bool,
}

/// The system's applications as the runtime's guests and the launcher see
/// them; replaceable, for tests.
type SharedApplications = Arc<Mutex<Arc<dyn Applications>>>;

/// The installed packages as the launcher has them, once it has said, for
/// resolving operation calls and finding a guest's helpers.
type SharedDirectory = Arc<Mutex<Option<Directory>>>;

/// One entry in a command's list view, as produced by the guest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// When set, activating the item opens this form instead of running its
    /// action.
    pub form: Option<Form>,
    /// The operating systems the item's action works on; `None` for every
    /// system.
    pub platforms: Option<Vec<Platform>>,
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

/// The most shapes a frame may have.
pub const MAX_FRAME_SHAPES: usize = 4096;
/// The most characters a text shape may have.
pub const MAX_TEXT_CHARS: usize = 256;
/// The largest width and height of a frame, in logical pixels.
pub const MAX_FRAME_SIZE: u32 = 4096;

impl Frame {
    /// Why the frame is over one of Pane's limits, if it is. The window
    /// draws each shape as an element, so a frame from an extension is
    /// bounded before it reaches the window.
    fn over_limits(&self) -> Option<String> {
        if self.shapes.len() > MAX_FRAME_SHAPES {
            return Some(format!(
                "the frame has {} shapes; at most {MAX_FRAME_SHAPES} are drawn",
                self.shapes.len()
            ));
        }
        if self.width > MAX_FRAME_SIZE || self.height > MAX_FRAME_SIZE {
            return Some(format!(
                "the frame is {} x {} pixels; at most {MAX_FRAME_SIZE} x {MAX_FRAME_SIZE} are drawn",
                self.width, self.height
            ));
        }
        self.shapes.iter().find_map(|shape| match shape {
            Shape::Text { content, .. } if content.chars().count() > MAX_TEXT_CHARS => {
                Some(format!(
                    "a text of the frame has {} characters; at most {MAX_TEXT_CHARS} are drawn",
                    content.chars().count()
                ))
            }
            _ => None,
        })
    }
}

/// One thing a custom view draws. Coordinates are logical pixels from the
/// view's top-left corner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Rect {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        fill: Rgb,
    },
    /// One line of text, its top-left corner at `x`, `y`.
    Text {
        x: i32,
        y: i32,
        content: String,
        color: Rgb,
    },
}

/// An opaque color as 0xRRGGBB, the WIT `rgb`; the top byte is ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u32);

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

/// Identifies a custom view open in a [`Runtime`]. Ids are never reused,
/// even by a runtime thread that replaced a crashed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewId {
    /// The number of the runtime thread holding the view.
    thread: u64,
    id: u64,
}

impl ViewId {
    /// The number of the runtime thread that holds the view: a crash of
    /// that thread (see [`supervisor::CrashReport`]) closes it.
    pub(crate) fn thread(&self) -> u64 {
        self.thread
    }
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
    /// The component exports Pane's extension interface, but with functions
    /// or types of another shape of the same API version: it was built
    /// against an older contract, and must be rebuilt.
    OlderApiShape(String),
    /// The guest ran and reported an error.
    Guest(String),
    /// The guest did not accept a submitted form.
    Form(FormError),
    /// The guest trapped or otherwise failed while running.
    Trap(String),
    /// The guest computed for too long without finishing (see
    /// [`COMPUTE_LIMIT`]), so Pane stopped the call and dropped its
    /// instance.
    Unresponsive(String),
    /// The command's package is disabled, so none of its code runs. A call
    /// pending when it was disabled is stopped with this.
    Disabled,
    /// The command's code was replaced by a reload or an update while the
    /// call was pending, so the call was stopped and its answer discarded.
    Replaced,
    /// The command's package was uninstalled while the call was pending, so
    /// the call was stopped and its answer discarded.
    Uninstalled,
    /// Pane paused the command's package after it failed (it could not
    /// start, or crashed too often), so none of its code runs
    /// until the user retries it. A call pending when it was paused is
    /// stopped with this.
    Paused,
    /// The custom view was closed, or its guest instance has stopped, so it
    /// cannot handle events any more.
    ViewClosed,
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallError::Disabled => write!(f, "The extension is disabled"),
            CallError::Replaced => write!(
                f,
                "The extension was reloaded or updated while this was running; try again"
            ),
            CallError::Uninstalled => write!(f, "The extension was uninstalled"),
            CallError::Paused => f.write_str(&paused_reason("The extension")),
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
            CallError::OlderApiShape(reason) => write!(
                f,
                "Incompatible extension: it was built for an older extension API shape: \
                 rebuild it against Pane's current extension API {}.{} ({reason})",
                EXTENSION_API.0, EXTENSION_API.1
            ),
            CallError::Guest(message) => write!(f, "The extension reported an error: {message}"),
            CallError::Form(error) => f.write_str(&error.message),
            CallError::Trap(reason) => write!(f, "The extension crashed: {reason}"),
            CallError::Unresponsive(reason) => {
                write!(f, "The extension stopped responding: {reason}")
            }
            CallError::ViewClosed => f.write_str("The extension's view is no longer open"),
        }
    }
}

impl std::error::Error for CallError {}

/// A handle to the runtime thread. Cloning shares the same runtime, also
/// once it was restarted after its thread crashed.
#[derive(Clone)]
pub struct Runtime {
    /// The thread serving calls now, and the helper processes guests
    /// started, which end once every handle is dropped.
    shared: Arc<Shared>,
    /// Component checks, served one at a time by the checker thread, apart
    /// from the runtime thread: a reload's check must not wait behind the
    /// guest call the reload is about to stop.
    checks: std::sync::mpsc::Sender<Check>,
}

/// A handle to the runtime thread that does not keep it running: the
/// thread stops once every [`Runtime`] is dropped, even while something it
/// holds (such as its health report) holds one of these.
#[derive(Clone)]
pub(crate) struct WeakRuntime {
    shared: std::sync::Weak<Shared>,
    checks: std::sync::mpsc::Sender<Check>,
}

impl WeakRuntime {
    /// The runtime, unless every handle to it was dropped.
    pub(crate) fn upgrade(&self) -> Option<Runtime> {
        Some(Runtime {
            shared: self.shared.upgrade()?,
            checks: self.checks.clone(),
        })
    }
}

/// A component check for the checker thread.
struct Check {
    component: PathBuf,
    exports: Exports,
    reply: oneshot::Sender<Result<(), CallError>>,
}

enum Request {
    GetView {
        component: PathBuf,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<View, CallError>>,
    },
    RunAction {
        component: PathBuf,
        item_id: String,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<String, CallError>>,
    },
    IndexedResults {
        component: PathBuf,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<Vec<IndexedResult>, CallError>>,
    },
    RootResults {
        component: PathBuf,
        query: String,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<Vec<RootResult>, CallError>>,
    },
    RunQuery {
        component: PathBuf,
        command: String,
        query: String,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<String, CallError>>,
    },
    Forget {
        components: Vec<PathBuf>,
    },
    SubmitForm {
        component: PathBuf,
        item_id: String,
        values: Vec<FieldValue>,
        data: Option<PackageData>,
        reply: oneshot::Sender<Result<String, CallError>>,
    },
    OpenView {
        component: PathBuf,
        item_id: String,
        data: Option<PackageData>,
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
    ViewCount {
        reply: oneshot::Sender<usize>,
    },
    Running {
        reply: oneshot::Sender<Vec<PathBuf>>,
    },
}

/// How a call into an installed package's code failed, for deciding
/// whether to pause the package (see the launcher's `pausing`). Only the
/// package's own failures are reported: an error the guest answers with is
/// not one, nor is a call stopped because its generation, or that of a
/// caller in its chain, ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Health {
    /// The guest trapped while running: a crash.
    Crashed(CallError),
    /// The guest computed for too long without finishing, and was stopped
    /// ([`CallError::Unresponsive`]). Wasmtime was running its code, so the
    /// failure is its own.
    Unresponsive(CallError),
    /// The component could not be loaded or instantiated.
    FailedToStart(CallError),
}

/// Told of each failure of a call into an installed package's code: its
/// component, the extension data (and so the generation) it ran with, and
/// how it failed. Called on the runtime thread before the call's answer is
/// sent.
pub(crate) type HealthReport = Arc<dyn Fn(&Path, &PackageData, Health) + Send + Sync>;

impl Runtime {
    /// A handle that does not keep the runtime thread running.
    pub(crate) fn downgrade(&self) -> WeakRuntime {
        WeakRuntime {
            shared: Arc::downgrade(&self.shared),
            checks: self.checks.clone(),
        }
    }

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
        let engine = engine(cache_dir.clone())?;
        let applications: SharedApplications = Arc::new(Mutex::new(crate::applications::native()));
        let code = Arc::new(Code::new(engine));
        let (checks, pending_checks) = std::sync::mpsc::channel::<Check>();
        let checker = code.clone();
        std::thread::Builder::new()
            .name("pane-extension-check".into())
            .spawn(move || {
                for check in pending_checks {
                    // A check that panics answers so, and the next one is
                    // still checked.
                    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        checker.check(&check.component, check.exports)
                    }))
                    .unwrap_or_else(|_| {
                        Err(CallError::RuntimeUnavailable(
                            "checking the component crashed Pane's checker".into(),
                        ))
                    });
                    let _ = check.reply.send(checked);
                }
            })
            .map_err(unavailable)?;
        let shared = Shared::start(code, applications, Helpers::default(), cache_dir)?;
        Ok(Runtime { shared, checks })
    }

    /// What the runtime is doing after a crash of its thread, if it had one.
    pub fn status(&self) -> RuntimeStatus {
        self.shared.status()
    }

    /// Starts the runtime again after its thread crashed and Pane did not
    /// restart it by itself ([`RuntimeStatus::Stopped`]); nothing that was
    /// running before is run again. A runtime that runs is left as it is.
    pub fn restart(&self) -> Result<(), CallError> {
        self.shared.restart()
    }

    /// Injects `fault` into the runtime thread serving calls now, to check
    /// that Pane recovers. For tests and the native smokes only; debug
    /// builds only.
    #[cfg(any(test, debug_assertions))]
    #[doc(hidden)]
    pub fn inject(&self, fault: InjectedFault) {
        self.shared.inject(fault);
    }

    /// Injects a fault each time a file appears at `file`, then removes it:
    /// `crash` injects [`Fault::Crash`], `crash-before-answer:<item>`
    /// [`Fault::CrashBeforeAnswer`] for the action `<item>`, `hang`
    /// [`Fault::Hang`] and `release` [`Fault::Release`]. For the native
    /// smokes, which set `PANE_TEST_RUNTIME_FAULTS`; the file is looked for
    /// every 100 ms, by a thread that stops once every handle to the
    /// runtime is dropped. Debug builds only.
    #[cfg(any(test, debug_assertions))]
    #[doc(hidden)]
    pub fn watch_fault_file(&self, file: PathBuf) {
        let runtime = self.downgrade();
        let _ = std::thread::Builder::new()
            .name("pane-runtime-faults".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let Some(runtime) = runtime.upgrade() else {
                        return;
                    };
                    let Ok(text) = std::fs::read_to_string(&file) else {
                        continue;
                    };
                    let _ = std::fs::remove_file(&file);
                    let text = text.trim();
                    match text.strip_prefix("crash-before-answer:") {
                        Some(item) => runtime.inject(InjectedFault::CrashBeforeAnswer {
                            item: item.to_owned(),
                        }),
                        None if text == "crash" => runtime.inject(InjectedFault::Crash),
                        None if text == "hang" => runtime.inject(InjectedFault::Hang),
                        None if text == "release" => runtime.inject(InjectedFault::Release),
                        None => eprintln!("PANE_TEST_RUNTIME_FAULTS: unknown fault {text:?}"),
                    }
                }
            });
    }

    /// Tells `report` of each crash of the runtime thread, after Pane
    /// restarted it or chose not to.
    pub(crate) fn set_crash_report(&self, report: supervisor::CrashReport) {
        self.shared.set_crash_report(report);
    }

    /// Asks the command in `component` for its list view. The command has
    /// no extension data.
    pub async fn get_view(&self, component: &Path) -> Result<View, CallError> {
        self.get_view_with(component, None).await
    }

    /// Runs the action of `item_id` in the command in `component`. The
    /// command has no extension data.
    pub async fn run_action(&self, component: &Path, item_id: &str) -> Result<String, CallError> {
        self.run_action_with(component, item_id, None).await
    }

    /// Like [`Runtime::get_view`]; the command reads and saves `data`.
    /// An instance keeps the data it was started with.
    pub(crate) async fn get_view_with(
        &self,
        component: &Path,
        data: Option<PackageData>,
    ) -> Result<View, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::GetView {
                component: component.to_path_buf(),
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Like [`Runtime::run_action`]; the command reads and saves `data`.
    pub(crate) async fn run_action_with(
        &self,
        component: &Path,
        item_id: &str,
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::RunAction {
                component: component.to_path_buf(),
                item_id: item_id.to_owned(),
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Checks, without running any guest code, that `component` is a
    /// component Pane can run: it compiles, imports only WASI 0.3 and exports
    /// the extension interface, with the function types of the current
    /// contract ([`CallError::OlderApiShape`] otherwise). The check keeps
    /// nothing loaded, and runs on Pane's checker thread, one check at a
    /// time: it does not wait for guest calls in progress, such as one a
    /// reload is about to stop.
    pub async fn check(&self, component: &Path) -> Result<(), CallError> {
        self.check_with(component, Exports::default()).await
    }

    /// Like [`Runtime::check`]; the component must also export each
    /// interface `exports` names, with the current function types.
    pub(crate) async fn check_with(
        &self,
        component: &Path,
        exports: Exports,
    ) -> Result<(), CallError> {
        let (reply, response) = oneshot::channel();
        self.checks
            .send(Check {
                component: component.to_path_buf(),
                exports,
                reply,
            })
            .map_err(|_| checker_stopped())?;
        response.await.unwrap_or_else(|_| Err(checker_stopped()))
    }

    /// Asks the command in `component`, which supplies root results ahead
    /// of the query, for all of them; the command uses
    /// its `data`. Starts its instance if it has none.
    pub(crate) async fn indexed_results_with(
        &self,
        component: &Path,
        data: Option<PackageData>,
    ) -> Result<Vec<IndexedResult>, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::IndexedResults {
                component: component.to_path_buf(),
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Has the runtime's guests, and the launcher opening their results,
    /// find and open applications through `applications` from now on,
    /// instead of this system's own ([`crate::applications::native`]).
    pub fn set_applications(&self, applications: Arc<dyn Applications>) {
        *lock(&self.shared.applications) = applications;
    }

    /// Finds and opens the system's applications.
    pub(crate) fn applications(&self) -> Arc<dyn Applications> {
        lock(&self.shared.applications).clone()
    }

    /// Asks the command in `component`, which computes root results, for
    /// its results for `query`; the command reads and saves `data`.
    /// Starts its instance if it has none.
    pub(crate) async fn root_results_with(
        &self,
        component: &Path,
        query: &str,
        data: Option<PackageData>,
    ) -> Result<Vec<RootResult>, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::RootResults {
                component: component.to_path_buf(),
                query: query.to_owned(),
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Runs the command with manifest id `command` in `component`, which
    /// takes a query, with `query`; the command reads and saves `data`.
    /// Starts its instance if it has none.
    pub(crate) async fn run_query_with(
        &self,
        component: &Path,
        command: &str,
        query: &str,
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::RunQuery {
                component: component.to_path_buf(),
                command: command.to_owned(),
                query: query.to_owned(),
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Submits the form of `item_id` in the command in `component`. A
    /// rejection by the guest is [`CallError::Form`]. The command has no
    /// extension data.
    pub async fn submit_form(
        &self,
        component: &Path,
        item_id: &str,
        values: Vec<FieldValue>,
    ) -> Result<String, CallError> {
        self.submit_form_with(component, item_id, values, None)
            .await
    }

    /// Like [`Runtime::submit_form`]; the command reads and saves `data`.
    pub(crate) async fn submit_form_with(
        &self,
        component: &Path,
        item_id: &str,
        values: Vec<FieldValue>,
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::SubmitForm {
                component: component.to_path_buf(),
                item_id: item_id.to_owned(),
                values,
                data,
                reply,
            },
            response,
        )
        .await
    }

    /// Opens the custom view of `item_id` in the command in `component` and
    /// draws it. The view stays open, holding its state in the guest, until
    /// [`Runtime::close_view`] or until its instance stops.
    /// The command has no extension data.
    pub async fn open_view(
        &self,
        component: &Path,
        item_id: &str,
    ) -> Result<(ViewId, Frame), CallError> {
        self.open_view_with(component, item_id, None).await
    }

    /// Like [`Runtime::open_view`]; the command reads and saves `data`.
    pub(crate) async fn open_view_with(
        &self,
        component: &Path,
        item_id: &str,
        data: Option<PackageData>,
    ) -> Result<(ViewId, Frame), CallError> {
        let (reply, response) = oneshot::channel();
        self.call(
            Request::OpenView {
                component: component.to_path_buf(),
                item_id: item_id.to_owned(),
                data,
                reply,
            },
            response,
        )
        .await
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
        self.call(Request::ViewEvent { view, event, reply }, response)
    }

    /// Closes the custom view `view`: the guest's view is dropped, after any
    /// event already sent to it, and later events are refused.
    pub fn close_view(&self, view: ViewId) {
        // A stopped runtime holds no views.
        let _ = self.send(Request::CloseView { view });
    }

    /// How many custom views are open in guest instances, counting the
    /// requests sent before this call. A diagnostic for tests and logs, not
    /// part of how the launcher decides anything: it tracks its own open
    /// view.
    pub async fn view_count(&self) -> usize {
        let (reply, response) = oneshot::channel();
        if self.send(Request::ViewCount { reply }).is_err() {
            return 0;
        }
        response.await.unwrap_or(0)
    }

    /// The components that have a live guest instance, counting the
    /// requests sent before this call, in no particular order. A diagnostic
    /// for tests and logs, like [`Runtime::view_count`]: listing or
    /// searching commands starts none; invoking a command starts its own.
    pub async fn running(&self) -> Vec<PathBuf> {
        let (reply, response) = oneshot::channel();
        if self.send(Request::Running { reply }).is_err() {
            return Vec::new();
        }
        response.await.unwrap_or_default()
    }

    /// The process ids of the native helpers guests started that are still
    /// running (not yet ended and reaped), in no particular order. A
    /// diagnostic for tests and logs, like [`Runtime::running`].
    pub fn helper_processes(&self) -> Vec<u32> {
        self.shared.helpers.running()
    }

    /// Ends every native helper process guests started, waiting until each
    /// is reaped, for Pane quitting: its threads stop with it, and nothing
    /// would end them otherwise. The calls that ran them answer that they
    /// were stopped.
    pub fn stop_helpers(&self) {
        self.shared.helpers.stop_all();
    }

    /// Ends the native helper processes running a file inside `folder`,
    /// waiting (briefly) until each is reaped: before a replaced managed
    /// copy is removed, so no running program keeps it in use.
    pub(crate) fn stop_helpers_in(&self, folder: &Path) {
        self.shared.helpers.stop_in(folder);
    }

    /// Drops the compiled code and live instances of `components`, for
    /// example after their files were replaced or removed; a later call
    /// loads the file again. Calls made afterwards see the effect; a call
    /// already in progress finishes first, unless its generation ends (as
    /// the launcher does before forgetting a disabled or replaced package),
    /// which stops it. Nothing coordinates this with a command the user has
    /// open: its instance's state is lost.
    pub fn forget(&self, components: impl IntoIterator<Item = PathBuf>) {
        let components = components.into_iter().collect();
        // A stopped runtime holds nothing to forget.
        let _ = self.send(Request::Forget { components });
    }

    /// Resolves the operation calls guests make against `directory` from
    /// now on. Without one, every call is answered that nothing is
    /// installed. It applies at once, not in the order of the requests:
    /// a call already queued resolves against it too. It is shared by
    /// every runtime thread, so a restarted one keeps it. The launcher sets
    /// it once, as it is created, before it asks for any call.
    pub(crate) fn set_directory(&self, directory: Directory) {
        *lock(&self.shared.directory) = Some(directory);
    }

    /// Tells `health` of each later failure of a call into an installed
    /// package's code (see [`Health`]). Like [`Runtime::set_directory`], it
    /// applies at once, to calls already queued too, and holds for a
    /// restarted runtime thread.
    pub(crate) fn set_health(&self, health: HealthReport) {
        *lock(&self.shared.health) = Some(health);
    }

    /// How many runtime threads Pane gave up on, because they stopped
    /// responding, are still stuck, holding what they held. A diagnostic
    /// for tests and logs, like [`Runtime::running`].
    pub fn abandoned_threads(&self) -> usize {
        self.shared.abandoned()
    }

    fn send(&self, request: Request) -> Result<(), CallError> {
        self.shared
            .send(request)
            .map(|_| ())
            .map_err(NotSent::error)
    }

    /// Sends `request`, when this is called, and returns its answer from
    /// `response`. An answer lost because the runtime thread crashed or
    /// stopped responding is known once Pane has restarted it or chosen
    /// not to, and says which; the request is never sent again.
    fn call<T: Send + 'static>(
        &self,
        request: Request,
        response: oneshot::Receiver<Result<T, CallError>>,
    ) -> impl Future<Output = Result<T, CallError>> + Send + 'static + use<T> {
        let handled = self.shared.handled();
        let sent = self.shared.send(request);
        let shared = Arc::downgrade(&self.shared);
        async move {
            let thread = match sent {
                Err(NotSent::Stopped) => return Err(supervisor::stopped()),
                Err(NotSent::Lost(thread)) => thread,
                Ok(thread) => {
                    // A thread Pane gave up on never answers: its failure
                    // being handled stands for the answer.
                    let mut response = response;
                    let mut given_up =
                        std::pin::pin!(supervisor::handled_after(handled.clone(), thread));
                    let answer = std::future::poll_fn(|cx| {
                        if let std::task::Poll::Ready(answer) =
                            std::pin::Pin::new(&mut response).poll(cx)
                        {
                            return std::task::Poll::Ready(answer.ok());
                        }
                        given_up.as_mut().poll(cx).map(|()| None)
                    })
                    .await;
                    if let Some(answer) = answer {
                        return answer;
                    }
                    thread
                }
            };
            Err(supervisor::lost(shared, handled, thread).await)
        }
    }
}

/// Locks `mutex`, taking it over if a thread panicked while holding it:
/// the runtime thread may crash while a lock is held (see `supervisor`),
/// and Pane carries on with what the lock guarded.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The runtime could not do something because of `error`.
pub(crate) fn unavailable(error: impl fmt::Display) -> CallError {
    CallError::RuntimeUnavailable(error.to_string())
}

/// The engine every runtime thread runs guests with: WASI 0.3 and
/// component-model async, keeping compiled code in `cache_dir`, if given.
fn engine(cache_dir: Option<PathBuf>) -> Result<Engine, CallError> {
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .wasm_component_model_async(true)
        // Every guest yields to the runtime thread at each tick (see
        // `deadlines`), so none holds it by computing.
        .epoch_interruption(true);
    if let Some(dir) = cache_dir {
        let mut cache = CacheConfig::new();
        cache.with_directory(dir);
        let cache = Cache::new(cache).map_err(unavailable)?;
        config.cache(Some(cache));
    }
    let engine = Engine::new(&config).map_err(unavailable)?;
    deadlines::tick(&engine);
    Ok(engine)
}

/// How a check answers once the checker thread has stopped.
fn checker_stopped() -> CallError {
    CallError::RuntimeUnavailable("Pane's component checker has stopped".into())
}

/// How a call of a generation that ended for `end` answers.
fn ended(end: End) -> CallError {
    match end {
        End::Disabled => CallError::Disabled,
        End::Replaced => CallError::Replaced,
        End::Uninstalled => CallError::Uninstalled,
        End::Paused => CallError::Paused,
    }
}

pub(crate) struct GuestState {
    wasi: WasiCtx,
    table: ResourceTable,
    /// The extension data of the package the command belongs to; `None` for a
    /// command built into Pane.
    data: Option<PackageData>,
    /// The guest's component, which identifies it as a caller.
    pub(crate) component: PathBuf,
    /// Where the guest's operation calls go, to be served while it waits.
    pub(crate) calls: mpsc::UnboundedSender<OperationCall>,
    /// Whether Pane is running a call of this guest, whose frame serves the
    /// guest's operation calls. A call made at any other time, such as while
    /// the component starts, is refused.
    pub(crate) serving: bool,
    /// Finds and opens the system's applications for the guest.
    applications: SharedApplications,
    /// The installed packages, for finding the guest's helpers.
    directory: SharedDirectory,
    /// The runtime's helper processes; those of this instance are ended
    /// with it.
    helpers: Helpers,
    /// Identifies this instance as the owner of the helpers it starts.
    owner: u64,
    /// The runtime thread running the instance: once Pane gave up on it
    /// (it stopped responding), the instance starts no host work.
    watch: Arc<Watch>,
}

impl Drop for GuestState {
    /// The instance is going (its generation ended, it crashed, it was
    /// forgotten or the runtime stopped): so do the helpers it started.
    fn drop(&mut self) {
        self.helpers.stop_owned_by(self.owner);
    }
}

impl GuestState {
    /// Starts the helper `name` of the guest's own package with `args` and
    /// `input`. Its process belongs to this instance and to the generation
    /// of its code.
    pub(crate) fn start_helper(
        &mut self,
        name: String,
        args: Vec<String>,
        input: String,
    ) -> Result<Running, HelperError> {
        // Code whose generation ended starts no more work.
        if let Some(end) = self.stopped() {
            return Err(runner::stopped_code(end));
        }
        self.check_runtime()
            .map_err(|problem| HelperError::new(HelperErrorKind::Refused, problem))?;
        if self.data.is_none() {
            return Err(HelperError::new(
                HelperErrorKind::Refused,
                "only installed packages ship helpers; this command is built into Pane",
            ));
        }
        runner::check_limits(&args, &input)?;
        let directory = lock(&self.directory).clone();
        let installed = directory.map(|directory| directory()).unwrap_or_default();
        let program = helpers::find(&installed, &self.component, &name)?;
        self.helpers.start(Spec {
            name,
            program,
            args,
            input,
            generation: self.generation().cloned(),
            owner: self.owner,
            limit: runner::HELPER_TIME_LIMIT,
        })
    }

    /// The generation the instance belongs to; `None` for a command built
    /// into Pane, which runs as long as Pane.
    pub(crate) fn generation(&self) -> Option<&Generation> {
        self.data.as_ref().map(PackageData::generation)
    }

    /// Why the instance's generation ended, if it has: its code may no
    /// longer run, save data or call operations.
    pub(crate) fn stopped(&self) -> Option<End> {
        self.data.as_ref().and_then(PackageData::stopped)
    }

    /// Refuses host work for code of a runtime thread Pane gave up on: it
    /// stopped responding, a fresh thread replaced it, and whatever it
    /// still runs must change nothing.
    pub(crate) fn check_runtime(&self) -> Result<(), String> {
        match self.watch.given_up() {
            true => Err(
                "Pane's extension runtime stopped responding and was replaced while \
                         this code ran; it no longer changes anything"
                    .into(),
            ),
            false => Ok(()),
        }
    }

    fn data(&self) -> Result<&PackageData, String> {
        self.check_runtime()?;
        self.data.as_ref().ok_or_else(|| {
            "only installed packages keep settings or data; this command is built into Pane".into()
        })
    }
}

/// Implements one kind of data's interface over the package's extension data.
macro_rules! data_host {
    ($interface:ident, $kind:expr) => {
        impl $interface::Host for GuestState {
            fn get(&mut self, key: String) -> Result<Option<String>, String> {
                self.data()?.get($kind, &key)
            }

            fn set(&mut self, key: String, value: String) -> Result<(), String> {
                self.data()?.set($kind, &key, &value)
            }
        }
    };
}

data_host!(settings, DataKind::Settings);
data_host!(content, DataKind::Content);
data_host!(cache, DataKind::Cache);
data_host!(credentials, DataKind::LocalCredentials);

impl GuestState {
    fn applications(&self) -> Arc<dyn Applications> {
        lock(&self.applications).clone()
    }
}

impl applications::Host for GuestState {
    fn installed(&mut self) -> Result<Vec<applications::Application>, String> {
        // Code whose generation ended starts no more work.
        if self.stopped().is_some() {
            return Err(
                "this code of the extension was stopped (disabled, reloaded or updated)".into(),
            );
        }
        self.check_runtime()?;
        Ok(self
            .applications()
            .installed()?
            .into_iter()
            .map(|application| applications::Application {
                id: application.id,
                name: application.name,
                location: application.location,
            })
            .collect())
    }

    fn open(&mut self, id: String) -> Result<(), String> {
        // Code whose generation ended starts no more work.
        if self.stopped().is_some() {
            return Err(
                "this code of the extension was stopped (disabled, reloaded or updated)".into(),
            );
        }
        self.check_runtime()?;
        self.applications().open(&id)
    }
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
    bindings: bindings::ExtensionWithHelpers,
    /// Its root results export, if it has one.
    root_results: Option<root_bindings::RootResultsProvider>,
    /// Its indexed results export, if it has one.
    indexed_results: Option<indexed_bindings::IndexedResultsProvider>,
    /// Its query-taking export, if it has one.
    query_command: Option<query_bindings::QueryCommandProvider>,
    /// Its published operations export, if it has one.
    operations: Option<operations_bindings::OperationsProvider>,
}

/// A custom view open in a guest instance.
struct LiveView {
    /// The component whose instance holds the view.
    component: PathBuf,
    /// The guest's `custom-view` resource.
    resource: ResourceAny,
}

/// The engine and the host interfaces guests link against, shared by the
/// runtime thread and the checker thread.
struct Code {
    engine: Engine,
    linker: Linker<GuestState>,
}

/// Runtime-thread state: compiled components, their live instances and the
/// custom views open in them.
struct Host {
    code: Arc<Code>,
    components: HashMap<PathBuf, Component>,
    instances: HashMap<PathBuf, Instance>,
    views: HashMap<ViewId, LiveView>,
    /// The next view id, shared with the threads that replace this one.
    next_view: Arc<AtomicU64>,
    /// The installed packages operation calls are resolved against, and
    /// guests' helpers found in.
    directory: SharedDirectory,
    /// The helper processes guests started.
    helpers: Helpers,
    /// Told of each failure of a call into an installed package's code.
    health: Arc<Mutex<Option<HealthReport>>>,
    /// Faults injected into this thread, to check recovery.
    faults: Arc<Faults>,
    /// What the watchdog knows of this thread; whether Pane gave up on it.
    watch: Arc<Watch>,
    /// This thread's number among those the runtime started.
    number: u64,
    /// Handed to every guest, for its operation calls.
    calls: mpsc::UnboundedSender<OperationCall>,
    /// Operation calls guests made, served while their callers wait.
    calls_sent: mpsc::UnboundedReceiver<OperationCall>,
    /// Calls taken from `calls_sent` whose caller's frame has not served
    /// them yet (see [`Host::run_guest`]).
    waiting_calls: VecDeque<OperationCall>,
    /// The components running a guest call, outermost first: a chain of
    /// operation calls. Each is busy until its call returns.
    chain: Vec<PathBuf>,
    /// The generations of the calls in the chain that have one, outermost
    /// first: when any ends, the calls from it inward stop.
    owners: Vec<Generation>,
    /// Finds and opens the system's applications for guests.
    applications: SharedApplications,
}

impl Code {
    fn new(engine: Engine) -> Code {
        let mut linker = Linker::new(&engine);
        // Only WASI 0.3 is registered: no P2 linker and no stubs for unknown
        // imports, so a mixed P2/P3 component cannot instantiate.
        wasmtime_wasi::p3::add_to_linker(&mut linker)
            .expect("registering WASI 0.3 in a fresh linker cannot conflict");
        settings::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)
            .expect("registering settings in a fresh linker cannot conflict");
        content::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)
            .expect("registering content in a fresh linker cannot conflict");
        cache::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)
            .expect("registering the cache in a fresh linker cannot conflict");
        credentials::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| {
            state
        })
        .expect("registering credentials in a fresh linker cannot conflict");
        bindings::pane::extension::operations::add_to_linker::<_, operations::Calls>(
            &mut linker,
            |state| state,
        )
        .expect("registering operations in a fresh linker cannot conflict");
        applications::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| {
            state
        })
        .expect("registering applications in a fresh linker cannot conflict");
        bindings::pane::extension::helpers::add_to_linker::<_, helpers::Runs>(
            &mut linker,
            |state| state,
        )
        .expect("registering helpers in a fresh linker cannot conflict");
        Code { engine, linker }
    }

    /// Compiles `path` and rejects components that import non-0.3 WASI.
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
        self.check_exports(&component)?;
        Ok(component)
    }

    /// Type-checks the functions of the component's command interface
    /// against those Pane calls, from the component's type alone, so no
    /// guest code runs. Instantiating checks the same, but only when a
    /// command opens; a component built for an older shape of the same API
    /// version is refused here instead. A component without the interface
    /// is left to [`Host::check`], which says so.
    fn check_exports(&self, component: &Component) -> Result<(), CallError> {
        use wasmtime::component::types::{ComponentFunc, ComponentItem};
        use wasmtime::component::{ComponentNamedList, Lift, Lower, ResourceAny};

        let ty = component.component_type();
        let Some(ComponentItem::ComponentInstance(interface)) = ty
            .get_export(&self.engine, COMMAND_INTERFACE)
            .map(|export| export.ty)
        else {
            return Ok(());
        };
        let cx = ty.instance_type();
        let older = |problem: String| CallError::OlderApiShape(problem);
        let func = |name: &str| match interface.get_export(&self.engine, name).map(|e| e.ty) {
            Some(ComponentItem::ComponentFunc(func)) => Ok(func),
            _ => Err(older(format!("it has no function `{name}`"))),
        };
        fn check<P: ComponentNamedList + Lower, R: ComponentNamedList + Lift>(
            name: &str,
            func: ComponentFunc,
            cx: &wasmtime::component::__internal::InstanceType<'_>,
        ) -> Result<(), CallError> {
            func.typecheck::<P, R>(cx)
                .map_err(|error| CallError::OlderApiShape(format!("`{name}`: {error:#}")))
        }
        check::<(), (Result<command::View, String>,)>("get-view", func("get-view")?, &cx)?;
        check::<(String,), (Result<String, String>,)>("run-action", func("run-action")?, &cx)?;
        check::<(String, Vec<command::FieldValue>), (Result<String, command::FormError>,)>(
            "submit-form",
            func("submit-form")?,
            &cx,
        )?;
        check::<(String,), (Result<ResourceAny, String>,)>("open-view", func("open-view")?, &cx)?;
        let render = "[method]custom-view.render";
        check::<(ResourceAny,), (command::Frame,)>(render, func(render)?, &cx)?;
        let handle_event = "[method]custom-view.handle-event";
        check::<(ResourceAny, command::ViewEvent), (Result<(), String>,)>(
            handle_event,
            func(handle_event)?,
            &cx,
        )
    }

    /// Type-checks `path` against the linker and the extension world, and
    /// against each interface `exports` names too, without instantiating it,
    /// so no guest code runs.
    fn check(&self, path: &Path, exports: Exports) -> Result<(), CallError> {
        let component = self.compile(path)?;
        let interface = |error: wasmtime::Error| CallError::Interface(format!("{error:#}"));
        let pre = self.linker.instantiate_pre(&component).map_err(interface)?;
        if exports.root_results {
            root_bindings::RootResultsProviderPre::new(pre.clone()).map_err(|error| {
                CallError::Interface(format!(
                    "its manifest says it computes root results, but it does not export \
                     {ROOT_RESULTS_INTERFACE} with the functions Pane calls: {error:#}"
                ))
            })?;
        }
        if exports.indexed_results {
            indexed_bindings::IndexedResultsProviderPre::new(pre.clone()).map_err(|error| {
                CallError::Interface(format!(
                    "its manifest says it supplies indexed results, but it does not export \
                     {INDEXED_RESULTS_INTERFACE} with the functions Pane calls: {error:#}"
                ))
            })?;
        }
        if exports.query_command {
            query_bindings::QueryCommandProviderPre::new(pre.clone()).map_err(|error| {
                CallError::Interface(format!(
                    "its manifest says it takes a query, but it does not export \
                     {QUERY_COMMAND_INTERFACE} with the functions Pane calls: {error:#}"
                ))
            })?;
        }
        if exports.operations {
            operations_bindings::OperationsProviderPre::new(pre.clone()).map_err(|error| {
                CallError::Interface(format!(
                    "its manifest publishes operations it serves, but it does not export \
                     {OPERATIONS_INTERFACE} with the functions Pane calls: {error:#}"
                ))
            })?;
        }
        bindings::ExtensionWithHelpersPre::new(pre).map_err(interface)?;
        Ok(())
    }
}

impl Host {
    fn new(
        code: Arc<Code>,
        shared: &Shared,
        number: u64,
        faults: Arc<Faults>,
        watch: Arc<Watch>,
    ) -> Host {
        let (calls, calls_sent) = operations::channel();
        Host {
            code,
            components: HashMap::new(),
            instances: HashMap::new(),
            views: HashMap::new(),
            next_view: shared.next_view.clone(),
            directory: shared.directory.clone(),
            helpers: shared.helpers.clone(),
            health: shared.health.clone(),
            faults,
            watch,
            number,
            calls,
            calls_sent,
            waiting_calls: VecDeque::new(),
            chain: Vec::new(),
            owners: Vec::new(),
            applications: shared.applications.clone(),
        }
    }

    /// The next request, or `None` once every handle is gone. An injected
    /// [`Fault::Crash`] panics here while the thread waits.
    async fn next_request(
        &self,
        requests: &mut mpsc::UnboundedReceiver<Request>,
    ) -> Option<Request> {
        let faults = self.faults.clone();
        let mut waiting = std::pin::pin!(faults.waiting());
        std::future::poll_fn(|cx| {
            faults.check(waiting.as_mut(), cx);
            // Given up on while it was stuck: it serves nothing more.
            if self.watch.given_up() {
                return std::task::Poll::Ready(None);
            }
            requests.poll_recv(cx)
        })
        .await
    }

    async fn serve(mut self, mut requests: mpsc::UnboundedReceiver<Request>) {
        while let Some(request) = self.next_request(&mut requests).await {
            let watch = self.watch.clone();
            let _handling = watch.doing(Doing::Handling);
            self.drop_stopped();
            match request {
                Request::GetView {
                    component,
                    data,
                    reply,
                } => {
                    let result = self.get_view(&component, data).await;
                    let _ = reply.send(result);
                }
                Request::RunAction {
                    component,
                    item_id,
                    data,
                    reply,
                } => {
                    let result = self.run_action(&component, item_id.clone(), data).await;
                    // An injected fault may lose this answer, after the
                    // action ran.
                    self.faults.before_answer(&item_id);
                    let _ = reply.send(result);
                }
                Request::IndexedResults {
                    component,
                    data,
                    reply,
                } => {
                    let result = self.indexed_results(&component, data).await;
                    let _ = reply.send(result);
                }
                Request::RootResults {
                    component,
                    query,
                    data,
                    reply,
                } => {
                    let result = self.root_results(&component, query, data).await;
                    let _ = reply.send(result);
                }
                Request::RunQuery {
                    component,
                    command,
                    query,
                    data,
                    reply,
                } => {
                    let result = self.run_query(&component, command, query, data).await;
                    let _ = reply.send(result);
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
                    data,
                    reply,
                } => {
                    let result = self.submit_form(&component, item_id, values, data).await;
                    let _ = reply.send(result);
                }
                Request::OpenView {
                    component,
                    item_id,
                    data,
                    reply,
                } => {
                    let result = self.open_view(&component, item_id, data).await;
                    let _ = reply.send(result);
                }
                Request::ViewEvent { view, event, reply } => {
                    let result = self.view_event(view, event).await;
                    let _ = reply.send(result);
                }
                Request::CloseView { view } => self.close_view(view).await,
                Request::ViewCount { reply } => {
                    let _ = reply.send(self.views.len());
                }
                Request::Running { reply } => {
                    let _ = reply.send(self.instances.keys().cloned().collect());
                }
            }
        }
    }

    async fn get_view(
        &mut self,
        path: &Path,
        data: Option<PackageData>,
    ) -> Result<View, CallError> {
        self.instance(path, data).await?;
        let result = self
            .run_guest(path, async |instance| {
                let command = instance.bindings.pane_extension_command();
                instance
                    .store
                    .run_concurrent(async |store| command.call_get_view(store).await)
                    .await
            })
            .await?;
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
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        self.instance(path, data).await?;
        let values = values
            .into_iter()
            .map(|FieldValue { id, value }| command::FieldValue { id, value })
            .collect();
        let result = self
            .run_guest(path, async |instance| {
                let command = instance.bindings.pane_extension_command();
                instance
                    .store
                    .run_concurrent(async |store| {
                        command.call_submit_form(store, item_id, values).await
                    })
                    .await
            })
            .await?;
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
        data: Option<PackageData>,
    ) -> Result<(ViewId, Frame), CallError> {
        self.instance(path, data).await?;
        let result = self
            .run_guest(path, async |instance| {
                let command = instance.bindings.pane_extension_command();
                instance
                    .store
                    .run_concurrent(async |store| command.call_open_view(store, item_id).await)
                    .await
            })
            .await?;
        let resource = self.settle(path, result, CallError::Guest)?;
        let view = ViewId {
            thread: self.number,
            id: self.next_view.fetch_add(1, Ordering::Relaxed),
        };
        self.views.insert(
            view,
            LiveView {
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
        let event = command::ViewEvent::from(event);
        let result = self
            .run_guest(&path, async |instance| {
                let custom_view = instance.bindings.pane_extension_command().custom_view();
                instance
                    .store
                    .run_concurrent(async |store| {
                        custom_view.call_handle_event(store, resource, event).await
                    })
                    .await
            })
            .await?;
        self.settle(&path, result, CallError::Guest)?;
        self.render(view).await
    }

    /// Asks the guest to draw the open view `view`.
    async fn render(&mut self, view: ViewId) -> Result<Frame, CallError> {
        let (path, resource) = self.view(view)?;
        let result = self
            .run_guest(&path, async |instance| {
                let custom_view = instance.bindings.pane_extension_command().custom_view();
                instance
                    .store
                    .run_concurrent(async |store| custom_view.call_render(store, resource).await)
                    .await
            })
            .await?;
        let frame = self.settle(&path, result.map(|frame| frame.map(Ok)), |never| never)?;
        let frame = Frame::from(frame);
        match frame.over_limits() {
            // The view stays open; its next drawing may be within them.
            Some(problem) => Err(CallError::Guest(problem)),
            None => Ok(frame),
        }
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
        let Some(instance) = self.instances.get_mut(&open.component) else {
            return;
        };
        let data = instance.store.data().data.clone();
        let dropped =
            deadlines::metered(open.resource.resource_drop_async(&mut instance.store)).await;
        let health = match dropped {
            Ok(Ok(())) => return,
            // The destructor trapped: the instance cannot be re-entered. It
            // is a crash of the package, reported as any other.
            Ok(Err(trap)) => Health::Crashed(CallError::Trap(format!("{trap:#}"))),
            // It computed for too long: it is stopped where it yielded.
            Err(reason) => Health::Unresponsive(CallError::Unresponsive(reason)),
        };
        self.drop_instance(&open.component);
        if data.as_ref().is_some_and(|data| data.stopped().is_none()) {
            self.report(&open.component, data.as_ref(), health);
        }
    }

    /// Drops the live instance of `path` and forgets the views open in it,
    /// which went with it.
    fn drop_instance(&mut self, path: &Path) {
        self.instances.remove(path);
        self.views.retain(|_, view| view.component != path);
    }

    /// Drops the instances whose generation has ended, with everything
    /// their stores hold.
    fn drop_stopped(&mut self) {
        let stopped: Vec<PathBuf> = self
            .instances
            .iter()
            .filter(|(_, instance)| instance.store.data().stopped().is_some())
            .map(|(path, _)| path.clone())
            .collect();
        for path in stopped {
            self.drop_instance(&path);
        }
    }

    async fn root_results(
        &mut self,
        path: &Path,
        query: String,
        data: Option<PackageData>,
    ) -> Result<Vec<RootResult>, CallError> {
        let instance = self.instance(path, data).await?;
        let provider = instance
            .root_results
            .as_ref()
            .map(|provider| provider.pane_extension_root_results().clone())
            .ok_or_else(|| {
                CallError::Interface(format!("it does not export {ROOT_RESULTS_INTERFACE}"))
            })?;
        let result = self
            .run_guest(path, async |instance| {
                instance
                    .store
                    .run_concurrent(async |store| provider.call_results_for(store, query).await)
                    .await
            })
            .await?;
        let results = self.settle(path, result, CallError::Guest)?;
        Ok(results
            .into_iter()
            .map(|result| RootResult {
                id: result.id,
                title: result.title,
                subtitle: result.subtitle,
                action: match result.action {
                    root_results::RootAction::Copy(text) => RootAction::Copy(text),
                    root_results::RootAction::OpenUrl(url) => RootAction::OpenUrl(url),
                },
            })
            .collect())
    }

    async fn run_query(
        &mut self,
        path: &Path,
        id: String,
        query: String,
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        let instance = self.instance(path, data).await?;
        let command = instance
            .query_command
            .as_ref()
            .map(|provider| provider.pane_extension_query_command().clone())
            .ok_or_else(|| {
                CallError::Interface(format!("it does not export {QUERY_COMMAND_INTERFACE}"))
            })?;
        let result = self
            .run_guest(path, async |instance| {
                instance
                    .store
                    .run_concurrent(async |store| command.call_run_query(store, id, query).await)
                    .await
            })
            .await?;
        self.settle(path, result, CallError::Guest)
    }

    async fn indexed_results(
        &mut self,
        path: &Path,
        data: Option<PackageData>,
    ) -> Result<Vec<IndexedResult>, CallError> {
        let instance = self.instance(path, data).await?;
        if instance.indexed_results.is_none() {
            return Err(CallError::Interface(format!(
                "it does not export {INDEXED_RESULTS_INTERFACE}"
            )));
        }
        let result = self
            .run_guest(path, async |instance| {
                let provider = instance
                    .indexed_results
                    .as_ref()
                    .expect("checked above")
                    .pane_extension_indexed_results();
                instance
                    .store
                    .run_concurrent(async |store| provider.call_results(store).await)
                    .await
            })
            .await?;
        let results = self.settle(path, result, CallError::Guest)?;
        Ok(results
            .into_iter()
            .map(|result| IndexedResult {
                id: result.id,
                title: result.title,
                subtitle: result.subtitle,
                action: match result.action {
                    indexed_results::IndexedAction::OpenApplication(id) => {
                        IndexedAction::OpenApplication(id)
                    }
                },
            })
            .collect())
    }

    async fn run_action(
        &mut self,
        path: &Path,
        item_id: String,
        data: Option<PackageData>,
    ) -> Result<String, CallError> {
        self.instance(path, data).await?;
        let result = self
            .run_guest(path, async |instance| {
                let command = instance.bindings.pane_extension_command();
                instance
                    .store
                    .run_concurrent(async |store| command.call_run_action(store, item_id).await)
                    .await
            })
            .await?;
        self.settle(path, result, CallError::Guest)
    }

    /// Runs `call` on the live instance of `path`, serving the operation
    /// calls its guest makes while it runs. Without a live instance (a view's
    /// instance has stopped) it is [`CallError::ViewClosed`].
    ///
    /// The instance is taken out of the host for the call, so the host can
    /// serve an operation call its guest makes, on this same thread, while
    /// the guest waits for the answer: the guest's call is not polled until
    /// the operation's answer is sent, and then resumes. This frame serves
    /// only its own guest's calls, one after another; a call another guest
    /// sent meanwhile waits for that guest's frame. The component is on the
    /// call chain meanwhile, so a call back into its package is refused
    /// rather than waiting on itself.
    ///
    /// The call stops as soon as the instance's generation, or that of any
    /// call further out in the chain, ends: the guest's call is dropped where
    /// it waits, and so is the instance, since Wasmtime keeps a dropped call's
    /// task in the store, where it would resume on the next call. A result
    /// that completes after its generation ended is discarded the same way.
    /// The generation is checked each time the guest yields, which it does
    /// at every epoch tick even while it computes (see `deadlines`).
    ///
    /// A call whose guest computed for [`COMPUTE_LIMIT`] without finishing
    /// is stopped the same way, as unresponsive: a failure of the guest's
    /// own package, reported as such ([`Health::Unresponsive`]). Time spent
    /// serving its operation calls counts for their targets, not for it.
    async fn run_guest<R>(
        &mut self,
        path: &Path,
        call: impl AsyncFnOnce(&mut Instance) -> R,
    ) -> Result<R, CallError> {
        use std::task::Poll;

        /// What happened next while the guest's call ran.
        enum Next<R> {
            Returned(R),
            Stopped(End),
            Called(OperationCall),
            /// It computed for too long.
            Unresponsive,
            /// Pane gave up on this thread while it was stuck.
            GivenUp,
        }

        /// Why the call ended without its result.
        enum Halt {
            Stopped(End),
            Unresponsive,
            GivenUp,
        }

        let mut instance = self.instances.remove(path).ok_or(CallError::ViewClosed)?;
        let own = instance.store.data().generation().cloned();
        if let Some(generation) = &own {
            self.owners.push(generation.clone());
        }
        let mut ends: Vec<std::pin::Pin<Box<dyn Future<Output = End>>>> = self
            .owners
            .iter()
            .map(|owner| Box::pin(owner.wait_end()) as _)
            .collect();
        self.chain.push(path.to_path_buf());
        instance.store.data_mut().serving = true;
        let faults = self.faults.clone();
        let watch = self.watch.clone();
        let _running = watch.doing(Doing::Running);
        let mut meter = Meter::default();
        let result = {
            let mut running = std::pin::pin!(call(&mut instance));
            let mut waiting = std::pin::pin!(faults.waiting());
            loop {
                let next = std::future::poll_fn(|cx| {
                    faults.check(waiting.as_mut(), cx);
                    // Given up on while it was stuck: the guest runs no more.
                    if watch.given_up() {
                        return Poll::Ready(Next::GivenUp);
                    }
                    for end in &mut ends {
                        if let Poll::Ready(end) = end.as_mut().poll(cx) {
                            return Poll::Ready(Next::Stopped(end));
                        }
                    }
                    if let Poll::Ready(result) = meter.measure(|| running.as_mut().poll(cx)) {
                        return Poll::Ready(Next::Returned(result));
                    }
                    if meter.exhausted() {
                        return Poll::Ready(Next::Unresponsive);
                    }
                    while let Poll::Ready(Some(call)) = self.calls_sent.poll_recv(cx) {
                        self.waiting_calls.push_back(call);
                    }
                    match self
                        .waiting_calls
                        .iter()
                        .position(|call| call.caller == path)
                    {
                        Some(index) => Poll::Ready(Next::Called(
                            self.waiting_calls.remove(index).expect("found above"),
                        )),
                        None => Poll::Pending,
                    }
                })
                .await;
                match next {
                    Next::Returned(result) => match own.as_ref().and_then(Generation::ended) {
                        Some(end) => break Err(Halt::Stopped(end)),
                        None => break Ok(result),
                    },
                    Next::Stopped(end) => break Err(Halt::Stopped(end)),
                    Next::Unresponsive => break Err(Halt::Unresponsive),
                    Next::GivenUp => break Err(Halt::GivenUp),
                    Next::Called(operation_call) => {
                        Box::pin(self.serve_operation(operation_call)).await;
                    }
                }
            }
        };
        instance.store.data_mut().serving = false;
        // A helper runs no longer than the call that started it: one the
        // guest left running when its call ended is ended too.
        let state = instance.store.data();
        state.helpers.stop_owned_by(state.owner);
        self.chain.pop();
        if own.is_some() {
            self.owners.pop();
        }
        // A call the guest sent but did not wait for before its call ended
        // has no frame to serve it.
        let (stranded, waiting) = std::mem::take(&mut self.waiting_calls)
            .into_iter()
            .partition(|call| call.caller == path);
        self.waiting_calls = waiting;
        for call in stranded {
            let _ = call.reply.send(Err(operations::outside_a_call()));
        }
        match result {
            Ok(result) => {
                self.instances.insert(path.to_path_buf(), instance);
                Ok(result)
            }
            Err(halt) => {
                let data = instance.store.data().data.clone();
                // The instance is dropped with its store: the abandoned
                // task, its host tasks, streams, futures and views.
                drop(instance);
                self.views.retain(|_, view| view.component != path);
                match halt {
                    Halt::Stopped(end) => Err(ended(end)),
                    Halt::Unresponsive => {
                        let error = CallError::Unresponsive(deadlines::computed_too_long());
                        eprintln!("pane: {} stopped responding: {error}", path.display());
                        self.report(path, data.as_ref(), Health::Unresponsive(error.clone()));
                        Err(error)
                    }
                    Halt::GivenUp => Err(CallError::RuntimeUnavailable(
                        "it stopped responding and was replaced".into(),
                    )),
                }
            }
        }
    }

    /// Serves one operation call a guest made, answering it.
    async fn serve_operation(&mut self, call: OperationCall) {
        // Its caller gave up on it before it started: it is not started.
        if call.reply.is_closed() {
            return;
        }
        let result = self.operation(&call).await;
        let _ = call.reply.send(result);
    }

    /// Serves `call`: checks it, resolves its target, runs the operation
    /// (starting the target if it is not running) and checks the answer.
    async fn operation(&mut self, call: &OperationCall) -> Result<String, OperationError> {
        self.check_call(call)?;
        let target = self.resolve_target(call)?;
        // Disabled or replaced while it was serving the call, it was
        // stopped, and its answer is not passed on.
        let answer = self.run_operation(&target, call).await?;
        operations::check_json(&answer, &format!("result of {}", target.title))?;
        Ok(answer)
    }

    /// Refuses a call whose input is not JSON within the limit, or that would
    /// make the chain too deep.
    fn check_call(&self, call: &OperationCall) -> Result<(), OperationError> {
        operations::check_json(&call.input, "input")?;
        if self.chain.len() >= operations::MAX_CALL_DEPTH {
            return Err(OperationError::refused(format!(
                "the chain of calls is {} deep; Pane allows at most {}",
                self.chain.len(),
                operations::MAX_CALL_DEPTH
            )));
        }
        Ok(())
    }

    /// The installed package and component serving `call`, unless its
    /// package already serves a call in the chain, through whichever of its
    /// components.
    fn resolve_target(&self, call: &OperationCall) -> Result<Target, OperationError> {
        let directory = lock(&self.directory).clone();
        let installed = match directory {
            Some(directory) => directory(),
            None => operations::Installed::default(),
        };
        let target =
            installed.resolve(&call.caller, &call.source, &call.operation, call.version)?;
        let in_chain = self.chain.iter().any(|component| {
            *component == target.component
                || installed.package_of(component) == Some(&target.identity)
        });
        if in_chain {
            return Err(OperationError::refused(format!(
                "{} is already serving a call in this chain; an extension cannot be \
                 called back while its own call waits",
                target.title
            )));
        }
        Ok(target)
    }

    /// Runs the operation of `call` in `target`'s component, starting it if
    /// it is not running, and returns its answer.
    async fn run_operation(
        &mut self,
        target: &Target,
        call: &OperationCall,
    ) -> Result<String, OperationError> {
        let failed = |error| OperationError::from_call(&target.title, error);
        let instance = self
            .instance(&target.component, target.data.clone())
            .await
            .map_err(failed)?;
        // The install check requires the export, so only a component replaced
        // behind Pane's back lacks it.
        let provider = instance
            .operations
            .as_ref()
            .map(|provider| provider.pane_extension_published_operations().clone())
            .ok_or_else(|| {
                failed(CallError::Interface(format!(
                    "it does not export {OPERATIONS_INTERFACE}"
                )))
            })?;
        let (name, input) = (call.operation.clone(), call.input.clone());
        let result = self
            .run_guest(&target.component, async |instance| {
                instance
                    .store
                    .run_concurrent(async |store| {
                        provider.call_run_operation(store, name, input).await
                    })
                    .await
            })
            .await
            .map_err(failed)?;
        self.settle(&target.component, result, CallError::Guest)
            .map_err(failed)
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
        let data = self
            .instances
            .get(path)
            .and_then(|instance| instance.store.data().data.clone());
        match outcome.and_then(|inner| inner) {
            Ok(result) => result.map_err(guest_error),
            Err(trap) => {
                self.drop_instance(path);
                let error = CallError::Trap(format!("{trap:#}"));
                self.report(path, data.as_ref(), Health::Crashed(error.clone()));
                Err(error)
            }
        }
    }

    /// Tells the health report how a call into `path`, of an installed
    /// package whose extension data is `data`, failed; nothing for a command
    /// built into Pane.
    fn report(&self, path: &Path, data: Option<&PackageData>, health: Health) {
        let report = lock(&self.health).clone();
        if let (Some(report), Some(data)) = (report, data) {
            report(path, data, health);
        }
    }

    /// Returns the live instance for `path` in the generation of `data`,
    /// instantiating it on first use. A call whose generation has ended (its
    /// package was disabled, reloaded or updated since it was asked for) gets
    /// none and is not started: it cannot bring its instance back.
    async fn instance(
        &mut self,
        path: &Path,
        data: Option<PackageData>,
    ) -> Result<&mut Instance, CallError> {
        // An instance of an ended generation goes; one of the package's
        // current generation stays, even for a stale call, which is refused.
        let stopped = self
            .instances
            .get(path)
            .is_some_and(|instance| instance.store.data().stopped().is_some());
        if stopped {
            self.drop_instance(path);
        }
        if let Some(end) = data.as_ref().and_then(PackageData::stopped) {
            return Err(ended(end));
        }
        if !self.instances.contains_key(path) {
            let started = self.start_instance(path, data.clone()).await;
            if let Err(error) = &started {
                self.report(path, data.as_ref(), Health::FailedToStart(error.clone()));
            }
            started?;
        }
        Ok(self.instances.get_mut(path).expect("inserted above"))
    }

    /// Loads and instantiates `path` as a live instance with `data`.
    async fn start_instance(
        &mut self,
        path: &Path,
        data: Option<PackageData>,
    ) -> Result<(), CallError> {
        let component = self.component(path)?.clone();
        let watch = self.watch.clone();
        let _starting = watch.doing(Doing::Starting);
        let mut store = Store::new(
            &self.code.engine,
            GuestState {
                wasi: WasiCtx::builder().build(),
                table: ResourceTable::new(),
                data,
                component: path.to_path_buf(),
                calls: self.calls.clone(),
                serving: false,
                applications: self.applications.clone(),
                directory: self.directory.clone(),
                owner: self.helpers.new_owner(),
                helpers: self.helpers.clone(),
                watch: self.watch.clone(),
            },
        );
        // The guest yields to this thread at every epoch tick, however long
        // it computes (see `deadlines`).
        store.set_epoch_deadline(1);
        store.epoch_deadline_async_yield_and_update(1);
        let load = |error: wasmtime::Error| CallError::Load(format!("{error:#}"));
        let instance =
            deadlines::metered(self.code.linker.instantiate_async(&mut store, &component))
                .await
                .map_err(CallError::Unresponsive)?
                .map_err(load)?;
        let bindings = bindings::ExtensionWithHelpers::new(&mut store, &instance).map_err(load)?;
        // Only a command that computes root results exports them.
        let root_results = root_bindings::RootResultsProvider::new(&mut store, &instance).ok();
        // Only a command that supplies results ahead of the query exports
        // them.
        let indexed_results =
            indexed_bindings::IndexedResultsProvider::new(&mut store, &instance).ok();
        // Only a command that takes a query exports it.
        let query_command = query_bindings::QueryCommandProvider::new(&mut store, &instance).ok();
        // Only a component serving published operations exports them.
        let operations = operations_bindings::OperationsProvider::new(&mut store, &instance).ok();
        self.instances.insert(
            path.to_path_buf(),
            Instance {
                store,
                bindings,
                root_results,
                indexed_results,
                query_command,
                operations,
            },
        );
        Ok(())
    }

    /// Compiles `path` once (see [`Code::compile`]).
    fn component(&mut self, path: &Path) -> Result<&Component, CallError> {
        if !self.components.contains_key(path) {
            // Compiling may take long without anything being stuck.
            let _compiling = self.watch.exempt();
            let component = self.code.compile(path)?;
            self.components.insert(path.to_path_buf(), component);
        }
        Ok(&self.components[path])
    }
}

impl From<command::Item> for Item {
    fn from(item: command::Item) -> Item {
        Item {
            id: item.id,
            title: item.title,
            subtitle: item.subtitle,
            form: item.form.map(Form::from),
            platforms: item
                .platforms
                .map(|platforms| platforms.into_iter().map(Platform::from).collect()),
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

impl From<command::Platform> for Platform {
    fn from(platform: command::Platform) -> Platform {
        match platform {
            command::Platform::Windows => Platform::Windows,
            command::Platform::Macos => Platform::Macos,
            command::Platform::Linux => Platform::Linux,
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
                    fill: Rgb(rect.fill),
                },
                command::Shape::Text(text) => Shape::Text {
                    x: text.x,
                    y: text.y,
                    content: text.content,
                    color: Rgb(text.color),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension_data::ExtensionData;
    use crate::packages::PackageIdentity;
    use futures::executor::block_on;

    fn settings_sample() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/guests/sample_settings.wasm");
        assert!(
            path.exists(),
            "{} is missing; run `cargo xtask guests`",
            path.display()
        );
        path
    }

    /// Quitting Pane drops the launcher, and with it the last runtime
    /// handle: a helper still running then ends too.
    #[test]
    fn dropping_the_last_runtime_handle_ends_running_helpers() {
        let target = pane_target::Target::current().expect("a known target");
        let program = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/guests/packages/sample-helper/helpers")
            .join(target.id())
            .join(format!("pane-echo{}", target.exe_suffix()));
        assert!(
            program.exists(),
            "{} is missing; run `cargo xtask guests`",
            program.display()
        );
        let runtime = Runtime::start().unwrap();
        let helpers = runtime.shared.helpers.clone();
        let running = helpers
            .start(runner::Spec {
                name: "echo".into(),
                program,
                args: vec!["--wait".into(), "5".into()],
                input: "hi".into(),
                generation: None,
                owner: helpers.new_owner(),
                limit: runner::HELPER_TIME_LIMIT,
            })
            .unwrap();
        let clone = runtime.clone();
        drop(runtime);
        assert_eq!(helpers.running().len(), 1, "a clone keeps the runtime");

        let dropped = std::time::Instant::now();
        drop(clone);

        assert_eq!(helpers.running(), Vec::<u32>::new());
        let error = block_on(running.finish()).unwrap_err();
        assert_eq!(error.kind, HelperErrorKind::Refused, "{error:?}");
        assert!(dropped.elapsed() < std::time::Duration::from_secs(4));
        // Nothing starts once it is gone.
        assert!(
            helpers
                .start(runner::Spec {
                    name: "echo".into(),
                    program: PathBuf::from("unused"),
                    args: vec![],
                    input: String::new(),
                    generation: None,
                    owner: 0,
                    limit: runner::HELPER_TIME_LIMIT,
                })
                .is_err()
        );
    }

    /// A call for a package that was disabled, served after its instances
    /// were dropped, must not start a new instance of it.
    #[test]
    fn a_disabled_package_command_starts_no_instance() {
        let data = tempfile::tempdir().unwrap();
        let settings = ExtensionData::open(data.path());
        let identity = PackageIdentity::local(data.path()).unwrap();
        let owned = settings.owned_by(&identity);
        let component = settings_sample();
        let runtime = Runtime::start().unwrap();
        block_on(runtime.get_view_with(&component, Some(owned.clone()))).unwrap();

        settings.set_enabled(&identity, false);
        runtime.forget([component.clone()]);
        let queued = runtime.get_view_with(&component, Some(owned));

        assert_eq!(block_on(queued), Err(CallError::Disabled));
    }

    fn guest(file: &str) -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/guests")
            .join(file);
        assert!(path.exists(), "{} is missing", path.display());
        path
    }

    /// Stopping a call releases what its instance holds: a stream open to
    /// the host with the future of its write pending, the clock the call
    /// awaits and a custom view open in the same instance.
    #[test]
    fn stopping_a_call_releases_the_stream_future_and_view_its_instance_holds() {
        use std::time::{Duration, Instant};

        let data = tempfile::tempdir().unwrap();
        let packages = ExtensionData::open(data.path());
        let identity = PackageIdentity::local(data.path()).unwrap();
        let owned = packages.owned_by(&identity);
        let component = guest("faulty.wasm");
        let runtime = Runtime::start().unwrap();
        let (view, _) =
            block_on(runtime.open_view_with(&component, "view", Some(owned.clone()))).unwrap();
        let holding = {
            let (runtime, component) = (runtime.clone(), component.clone());
            std::thread::spawn(move || {
                block_on(runtime.run_action_with(&component, "hold", Some(owned)))
            })
        };
        let started = Instant::now();
        let saved =
            || std::fs::read_to_string(data.path().join("settings.json")).unwrap_or_default();
        while !saved().contains("\"holding\": \"started\"") {
            assert!(
                started.elapsed() < Duration::from_secs(6),
                "it did not start"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        packages.set_enabled(&identity, false);

        assert_eq!(holding.join().unwrap(), Err(CallError::Disabled));
        assert!(started.elapsed() < Duration::from_secs(6));
        assert!(!saved().contains("finished"));
        assert_eq!(block_on(runtime.running()), Vec::<PathBuf>::new());
        assert_eq!(block_on(runtime.view_count()), 0);
        assert_eq!(
            block_on(runtime.view_event(view, ViewEvent::Key(Key::Up))),
            Err(CallError::ViewClosed)
        );
    }

    /// A custom view the window still shows from a crashed runtime thread
    /// names no view of the thread that replaced it: ids are never reused.
    #[test]
    fn a_restarted_runtime_never_reuses_a_view_id_of_the_crashed_one() {
        let component = guest("sample_rust.wasm");
        let runtime = Runtime::start().unwrap();
        let (old, _) = block_on(runtime.open_view(&component, "color")).unwrap();

        runtime.inject(Fault::Crash);
        let started = std::time::Instant::now();
        while runtime.status() == RuntimeStatus::Running {
            assert!(started.elapsed() < std::time::Duration::from_secs(8));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert!(matches!(runtime.status(), RuntimeStatus::Restarted { .. }));
        let (new, _) = block_on(runtime.open_view(&component, "color")).unwrap();
        assert_ne!(old, new);
        assert_eq!(
            block_on(runtime.view_event(old, ViewEvent::Key(Key::Up))),
            Err(CallError::ViewClosed)
        );
        assert!(block_on(runtime.view_event(new, ViewEvent::Key(Key::Up))).is_ok());
        assert_eq!(block_on(runtime.view_count()), 1);
    }

    /// A settings sample instance of a package in `data`, and its data.
    fn settings_package(data: &tempfile::TempDir) -> (ExtensionData, PackageIdentity) {
        let packages = ExtensionData::open(data.path());
        let identity = PackageIdentity::local(data.path()).unwrap();
        (packages, identity)
    }

    /// What the settings sample saved under `key` in `identity`'s settings.
    fn saved(packages: &ExtensionData, identity: &PackageIdentity, key: &str) -> Option<String> {
        packages
            .owned_by(identity)
            .get(DataKind::Settings, key)
            .unwrap()
    }

    /// Waits, generously, until `done` holds.
    fn until(what: &str, done: impl Fn() -> bool) {
        let started = std::time::Instant::now();
        while !done() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(60),
                "{what} did not happen"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// A guest computing without waiting yields at every tick: after the
    /// compute limit its call is stopped as unresponsive, reported as its
    /// package's failure, and its instance is gone, while the runtime
    /// serves the next call.
    #[test]
    fn a_guest_computing_without_waiting_is_stopped_after_the_compute_limit() {
        let data = tempfile::tempdir().unwrap();
        let (packages, identity) = settings_package(&data);
        let component = settings_sample();
        let runtime = Runtime::start().unwrap();
        let reported = Arc::new(Mutex::new(Vec::new()));
        {
            let reported = reported.clone();
            runtime.set_health(Arc::new(move |_, _, health| {
                lock(&reported).push(health);
            }));
        }
        // Another command is active meanwhile, and stays so.
        let other = guest("sample_rust.wasm");
        let (view, _) = block_on(runtime.open_view(&other, "color")).unwrap();

        let started = std::time::Instant::now();
        let busy = block_on(runtime.run_action_with(
            &component,
            "busy",
            Some(packages.owned_by(&identity)),
        ));

        let took = started.elapsed();
        let Err(CallError::Unresponsive(reason)) = &busy else {
            panic!("expected it stopped as unresponsive, got {busy:?}");
        };
        assert!(reason.contains("computed for 5 seconds"), "{reason}");
        assert!(took >= COMPUTE_LIMIT, "{took:?}");
        assert!(took < COMPUTE_LIMIT * 3, "{took:?}");
        assert_eq!(
            saved(&packages, &identity, "busy").as_deref(),
            Some("started")
        );
        assert!(matches!(
            lock(&reported).as_slice(),
            [Health::Unresponsive(CallError::Unresponsive(_))]
        ));
        assert_eq!(block_on(runtime.running()), vec![other.clone()]);
        // The other command's view is still open, and the package runs
        // again from a fresh instance.
        assert!(block_on(runtime.view_event(view, ViewEvent::Key(Key::Up))).is_ok());
        assert!(
            block_on(runtime.get_view_with(&component, Some(packages.owned_by(&identity)))).is_ok()
        );
    }

    /// Ending a generation stops a guest that computes without waiting at
    /// its next tick, rather than when it yields by itself.
    #[test]
    fn disabling_a_package_stops_its_computing_guest_at_once() {
        let data = tempfile::tempdir().unwrap();
        let (packages, identity) = settings_package(&data);
        let component = settings_sample();
        let runtime = Runtime::start().unwrap();
        let busy = {
            let (runtime, owned) = (runtime.clone(), packages.owned_by(&identity));
            std::thread::spawn(move || {
                block_on(runtime.run_action_with(&component, "busy", Some(owned)))
            })
        };
        until("it started computing", || {
            saved(&packages, &identity, "busy").as_deref() == Some("started")
        });

        let disabled = std::time::Instant::now();
        packages.set_enabled(&identity, false);

        assert_eq!(busy.join().unwrap(), Err(CallError::Disabled));
        assert!(disabled.elapsed() < std::time::Duration::from_secs(2));
        packages.set_enabled(&identity, true);
        assert_eq!(
            saved(&packages, &identity, "busy").as_deref(),
            Some("started")
        );
    }

    /// A runtime thread stuck outside any guest (made to hang) is given up
    /// on: the call it held answers that the runtime stopped responding, a
    /// fresh thread serves, no package is reported, and once the stuck
    /// thread returns it runs nothing more: the guest waiting in it never
    /// saves "finished", though its wait has passed.
    #[test]
    fn a_runtime_thread_that_stops_responding_is_replaced_and_runs_nothing_more() {
        let data = tempfile::tempdir().unwrap();
        let (packages, identity) = settings_package(&data);
        let component = settings_sample();
        let runtime = Runtime::start().unwrap();
        let reported = Arc::new(Mutex::new(Vec::new()));
        {
            let reported = reported.clone();
            runtime.set_health(Arc::new(move |_, _, health| {
                lock(&reported).push(health);
            }));
        }
        let slow = {
            let (runtime, owned, component) = (
                runtime.clone(),
                packages.owned_by(&identity),
                component.clone(),
            );
            std::thread::spawn(move || {
                block_on(runtime.run_action_with(&component, "slow", Some(owned)))
            })
        };
        until("it started waiting", || {
            saved(&packages, &identity, "slow-save").as_deref() == Some("started")
        });
        let waiting = std::time::Instant::now();

        runtime.inject(Fault::Hang);

        let Err(CallError::RuntimeUnavailable(reason)) = slow.join().unwrap() else {
            panic!("expected the runtime unavailable");
        };
        assert!(
            reason.contains("stopped responding before answering and was started again"),
            "{reason}"
        );
        assert!(waiting.elapsed() >= UNRESPONSIVE_LIMIT);
        let RuntimeStatus::Restarted { failure, why } = runtime.status() else {
            panic!("expected a restart, got {:?}", runtime.status());
        };
        assert_eq!(failure, RuntimeFailure::Unresponsive);
        assert!(why.contains("did not respond for 10 seconds"), "{why}");
        assert_eq!(runtime.abandoned_threads(), 1);
        assert!(lock(&reported).is_empty(), "no package is named");
        // A fresh thread serves.
        assert!(
            block_on(runtime.get_view_with(&component, Some(packages.owned_by(&identity)))).is_ok()
        );

        // The guest's wait has passed by now; the stuck thread returns.
        until("the guest's wait passed", || {
            waiting.elapsed() > std::time::Duration::from_secs(11)
        });
        runtime.inject(Fault::Release);
        until("the stuck thread ended", || {
            runtime.abandoned_threads() == 0
        });

        assert_eq!(
            saved(&packages, &identity, "slow-save").as_deref(),
            Some("started")
        );
        assert!(lock(&reported).is_empty());
    }

    /// A call of an ended generation served after the package's next
    /// generation started an instance at the same component is refused, and
    /// leaves that instance and its open view alone.
    #[test]
    fn a_stale_call_leaves_the_current_generation_running() {
        let data = tempfile::tempdir().unwrap();
        let packages = ExtensionData::open(data.path());
        let identity = PackageIdentity::local(data.path()).unwrap();
        let old = packages.owned_by(&identity);
        packages.set_enabled(&identity, false);
        packages.set_enabled(&identity, true);
        let current = packages.owned_by(&identity);
        let component = guest("sample_rust.wasm");
        let runtime = Runtime::start().unwrap();
        let (view, _) =
            block_on(runtime.open_view_with(&component, "color", Some(current))).unwrap();

        let stale = runtime.get_view_with(&component, Some(old));

        assert_eq!(block_on(stale), Err(CallError::Disabled));
        assert_eq!(block_on(runtime.view_count()), 1);
        assert!(block_on(runtime.view_event(view, ViewEvent::Key(Key::Right))).is_ok());
    }
}
