//! The designed view a command opens as its own screen (`"mode":
//! "designed"`, ADR 0036): a tree of layout nodes and UI components the
//! extension describes and Pane renders (`docs/designed-tree.md`).
//!
//! Opening the command calls the component's `open-view` once — passing its
//! launch record, as every launch does — and draws the view's first tree;
//! it never calls `render`. Pressing a button the tree named (`onPress`)
//! sends the press to the view's `handle-event`, with the sequence number
//! of the tree the user saw, and then asks for the tree again: the whole
//! screen is redrawn from the answer, the extension's state living in the
//! view's resource. Events are delivered one at a time, in order, exactly
//! as a custom view's are: an answer to an event older than another
//! already on screen is dropped, never replacing the newer tree, and an
//! answer for a view the user has left finds the screen gone and is
//! dropped with it.
//!
//! The screen holds a **navigation stack** (#239): the root view the
//! command opened, and each view a view's answer pushed above it
//! (`outcome.push`), the top of the stack on screen. A view's answer may
//! also replace itself (`outcome.replace`), or pop itself with a result
//! (`outcome.pop`); the back key pops the top view whatever answered,
//! showing the view below's last tree at once — no guest call first — and
//! then telling that view the one above it popped, the pop event, with
//! the result when the pop was the view's own. Popped and replaced views
//! are dropped in the runtime, never used again. The back key, the
//! `window::pop-to-root` host function and Shift+Esc drop the whole stack,
//! leaving the command as leaving its list does; a stack deeper than
//! [`MAX_NAVIGATION_DEPTH`](crate::runtime::MAX_NAVIGATION_DEPTH) refuses a
//! further push as the extension's error, the view keeping its last good
//! tree.
//!
//! An error the extension answers with — a failure of the call itself, a
//! tree Pane cannot read, over a limit, or of another major version of the
//! component set — is shown on the screen while the view stays open with
//! its last good tree. A crash closes it, as it closes a custom view.

use std::path::Path;
use std::pin::Pin;

use super::{Launcher, Opening, State, Status, stopped};
use crate::extension_data::PackageData;
use crate::icons::{Icon, IconSource, is_web_url};
use crate::packages::{InstalledPackage, PackageIdentity};
use crate::runtime::{
    CallError, DesignedEvent, DesignedNext, DesignedRendered, DesignedTree, Node, NodeKind,
    Runtime, ViewId,
};

/// The callback id of the pop event, the event that tells a view the one
/// above it popped: an id no tree names (the SDKs' ids start at 1), its
/// payload `{"pop": <result>}`, the result the pop answered with (`null`
/// when it carried none — the back key's pops). `docs/designed-tree.md`
/// documents the wire shape.
const POP_CALLBACK: u32 = 0;

/// The designed views the open command shows: the navigation stack, the
/// root view the command opened first and each view a push added above
/// it, the top of the stack on screen.
pub(super) struct DesignedStack {
    /// The stack's views, the root first and the top last; it always holds
    /// the root.
    views: Vec<OpenDesignedView>,
}

impl DesignedStack {
    /// The stack a command's root view opens as, holding its first tree —
    /// its icons already resolved, its loads already started, by the
    /// caller.
    #[allow(clippy::too_many_arguments)]
    fn root(
        id: ViewId,
        owner: Option<PackageIdentity>,
        loading: bool,
        rendered: u64,
        refresh_after_ms: Option<u32>,
        tree: DesignedTree,
    ) -> DesignedStack {
        DesignedStack {
            views: vec![OpenDesignedView {
                id,
                owner,
                rendered,
                sent: 0,
                shown: 0,
                loading,
                refresh_after_ms,
                tree,
            }],
        }
    }

    /// The top view: the one on screen.
    fn top(&self) -> &OpenDesignedView {
        self.views.last().expect("the stack holds the root")
    }

    /// The top view, changed.
    fn top_mut(&mut self) -> &mut OpenDesignedView {
        self.views.last_mut().expect("the stack holds the root")
    }

    /// The screen of the top view: its snapshot, and its navigation title
    /// as the screen's title.
    fn shown(&self) -> (super::DesignedViewSnapshot, String) {
        let top = self.top();
        (
            super::DesignedViewSnapshot {
                id: top.id,
                tree: top.tree.clone(),
            },
            top.tree.navigation_title().unwrap_or_default().to_owned(),
        )
    }
}

/// One view of the stack, as the launcher holds it.
struct OpenDesignedView {
    /// The view in the runtime; closed when the view leaves the screen.
    id: ViewId,
    /// The package the view's command belongs to: whose identity its
    /// web images and system icons load under, and whose folder its
    /// packaged images resolved in.
    owner: Option<PackageIdentity>,
    /// The sequence number of the render whose tree is on screen, which the
    /// events Pane sends carry back so the view can drop those older than
    /// it drew.
    rendered: u64,
    /// How many events were sent to the view.
    sent: u64,
    /// The number of the event whose answer is on screen, so an older
    /// answer arriving late does not replace a newer one.
    shown: u64,
    /// Whether the tree on screen holds an icon Pane loads (a web image,
    /// a system icon, an application's), whose arrivals redraw it.
    loading: bool,
    /// The `refresh-after-ms` the last render answered: carried and ignored
    /// until timers land (#236), which reschedules the view through it.
    refresh_after_ms: Option<u32>,
    /// The view's last good tree: what shows at once when the view above
    /// it pops, before the view re-renders.
    tree: DesignedTree,
}

/// One event sent to a designed view, with its reply.
struct SentDesignedEvent {
    /// The view the event was sent to.
    view: ViewId,
    /// The event's number among those sent to the view.
    number: u64,
    reply: Pin<Box<dyn Future<Output = Result<DesignedNext, CallError>> + Send>>,
}

impl OpenDesignedView {
    /// Sends `event` to the view, numbering it for ordering.
    fn send(&mut self, runtime: &Runtime, event: DesignedEvent) -> SentDesignedEvent {
        self.sent += 1;
        SentDesignedEvent {
            view: self.id,
            number: self.sent,
            reply: Box::pin(runtime.designed_view_event(self.id, event)),
        }
    }
}

impl Launcher {
    /// Opens the designed view of `opening`'s command and shows its first
    /// tree, or shows the error it answered with: the launch record is
    /// passed to `open-view`, as every launch's is, and the screen is the
    /// view's alone — `render` is never called. A view whose first render
    /// fails is closed again; the command stays at root search with the
    /// error, as a command whose list could not be read does.
    pub(super) async fn open_designed_command(
        &self,
        epoch: u64,
        opening: Opening,
        data: Option<PackageData>,
    ) {
        let Opening {
            component,
            command,
            launch,
            ..
        } = opening;
        // The launch record's command is the one opened, as a list
        // command's is.
        let launch = crate::launch::LaunchRecord {
            command: Some(command.clone()),
            ..launch
        };
        if let Some(problem) = self.updating(&component) {
            // Its package's code is being replaced (an update): opening the
            // command now would be stopped by the replacement, so it is
            // refused rather than interrupted.
            let mut state = self.lock();
            if state.screen_epoch == epoch {
                state.view.status = Status::Error(problem);
            }
            return;
        }
        let result = match self.runtime() {
            Ok(runtime) => {
                runtime
                    .open_designed_view_with(&component, &command, &launch, data.clone())
                    .await
            }
            Err(error) => Err(error),
        };
        let current = self.lock_if_current(epoch);
        let stopped = current
            .as_ref()
            .and_then(|state| stopped(state, &component, &data));
        let Some(mut state) = current.filter(|_| stopped.is_none()) else {
            if let (Ok((id, _)), Ok(runtime)) = (result, self.runtime()) {
                runtime.close_designed_view(id);
            }
            if let Some(problem) = stopped {
                // Stopped while it was opening.
                self.lock().view.status = Status::Error(problem);
            }
            return;
        };
        match result {
            Ok((id, rendered)) => {
                let owner = super::owner(&state.packages, &component)
                    .map(|package| package.identity.clone());
                let mut tree = rendered.tree;
                let loading = landed(
                    &state.packages,
                    &state.icon_loads,
                    &component,
                    owner.as_ref(),
                    &mut tree,
                );
                let stack = DesignedStack::root(
                    id,
                    owner,
                    loading,
                    rendered.render,
                    rendered.refresh_after_ms,
                    tree,
                );
                // The view's own content is its title, as a command's list
                // is: the navigation title its tree names, where a screen's
                // title is. The footer names the open command.
                let (screen, title) = stack.shown();
                let view = super::LauncherView::new(super::Screen::DesignedView(screen), title);
                state.open = Some(component);
                state.launch = launch;
                state.open_command = Some(command);
                state.searching = None;
                state.designed_view = Some(stack);
                state.next_screen();
                state.view = view;
            }
            Err(error) => state.view.status = Status::Error(error.to_string()),
        }
    }

    /// Sends the press of the button the open designed view's tree named
    /// with callback id `callback` (`onPress`) to the view — raised on the
    /// node with `key`, or none when the tree gave it none — and shows its
    /// answer. Await the returned future to show what the answer did next;
    /// anything sent when no view is open is ignored.
    ///
    /// Events are handled one at a time, in the order of these calls, and a
    /// tree is shown only if no later event's tree is on screen yet. An
    /// answer that pushed or replaced a view stacks it; one that popped the
    /// view shows the view below's last tree and tells it the view above
    /// popped. An error the extension reports is shown while the view stays
    /// open with its last good tree; a crash closes it.
    pub fn send_designed_event(
        &self,
        callback: u32,
        key: Option<&str>,
    ) -> impl Future<Output = ()> + Send + 'static {
        self.send_designed_payload(callback, key, "{}".to_owned())
    }

    /// Sends a change of the node the open designed view's tree named with
    /// callback id `callback` to the view, its `payload` naming what
    /// changed (a toggle's state, a slider's value, a field's commit), and
    /// shows its answer; as [`Launcher::send_designed_event`] does.
    pub fn send_designed_change(
        &self,
        callback: u32,
        key: Option<&str>,
        payload: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        self.send_designed_payload(callback, key, payload)
    }

    /// Sends one event to the open designed view, whatever changed.
    fn send_designed_payload(
        &self,
        callback: u32,
        key: Option<&str>,
        payload: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let epoch = state.screen_epoch;
        // The view as it is now: the event says which render's tree the
        // user saw, so the view can drop an event older than it drew.
        let event = state.designed_view.as_ref().map(|stack| {
            let key = key.unwrap_or_default().to_owned();
            DesignedEvent {
                render: stack.top().rendered,
                key,
                callback,
                payload,
            }
        });
        let sent = match (event, self.runtime()) {
            (Some(event), Ok(runtime)) => {
                let stack = state.designed_view.as_mut().expect("a view is open");
                Some(stack.top_mut().send(runtime, event))
            }
            _ => None,
        };
        drop(state);
        let launcher = self.clone();
        async move {
            if let Some(event) = sent {
                let result = event.reply.await;
                launcher.show_designed_answer(epoch, event.view, event.number, result);
            }
        }
    }

    /// Pops the open designed view's stack, as the back key does when the
    /// view has views below it: the top view is closed in the runtime
    /// (dropped, never used again) and the view below's last tree shown at
    /// once, which is then told that the view above it popped — the pop
    /// event, answered in the background. Whether a view was popped:
    /// `false` with no designed view open, or a stack holding only its
    /// root view.
    pub fn pop_designed_view(&self) -> bool {
        let mut state = self.lock();
        self.pop_designed_stack(&mut state)
    }

    /// As [`Launcher::pop_designed_view`], for the back key's own path
    /// through the launcher, with the state already locked.
    pub(super) fn pop_designed_stack(&self, state: &mut State) -> bool {
        let Some(stack) = state.designed_view.as_mut() else {
            return false;
        };
        // The root view is the command's own screen: popping it leaves the
        // command, which the back key's caller does.
        if stack.views.len() < 2 {
            return false;
        }
        let popped = stack
            .views
            .pop()
            .expect("the stack holds more than the root");
        if let Ok(runtime) = self.runtime() {
            runtime.close_designed_view(popped.id);
        }
        let (screen, title) = stack.shown();
        state.view.screen = super::Screen::DesignedView(screen);
        state.view.title = title;
        state.view.status = Status::Idle;
        let epoch = state.screen_epoch;
        self.deliver_designed_pop(state, epoch, None);
        true
    }

    /// Shows the answer to the event number `number` of the designed view
    /// `view`: the tree it re-rendered, the view the answer pushed or
    /// replaced (now the top of the stack), the view below's last tree when
    /// it popped, or the error it answered with (the view keeps its last
    /// good tree). An answer for a view that is no longer the top of the
    /// stack, or older than the answer on screen, is dropped — with any
    /// view the answer stacked closed again, so the runtime holds no view
    /// whose screen was never shown.
    fn show_designed_answer(
        &self,
        epoch: u64,
        view: ViewId,
        number: u64,
        result: Result<DesignedNext, CallError>,
    ) {
        // What the answer opened, when it pushed or replaced a view in:
        // stacked only if the answer is applied.
        let opened = match &result {
            Ok(DesignedNext::Pushed(opened, _)) | Ok(DesignedNext::Replaced(opened, _)) => {
                Some(*opened)
            }
            _ => None,
        };
        let Some(mut state) = self.lock_if_current(epoch) else {
            self.close_unstacked(opened);
            return;
        };
        let state = &mut *state;
        // The answer is for the view the event was sent to, which must
        // still be the top of the stack: a view the user has left, popped
        // or replaced does not answer the screen.
        let top = state
            .designed_view
            .as_ref()
            .is_some_and(|stack| stack.top().id == view);
        if !top {
            self.close_unstacked(opened);
            return;
        }
        // An answer to an event older than the one on screen is stale —
        // except the view's end, which closes it however old.
        let shown = state
            .designed_view
            .as_ref()
            .expect("a view is open")
            .top()
            .shown;
        if stale(number, shown, &result) {
            self.close_unstacked(opened);
            return;
        }
        match result {
            Ok(DesignedNext::Tree(rendered)) => {
                let component = state.open.clone().unwrap_or_default();
                let (screen, title) = {
                    let stack = state.designed_view.as_mut().expect("a view is open");
                    let top = stack.top_mut();
                    top.shown = number;
                    top.rendered = rendered.render;
                    top.refresh_after_ms = rendered.refresh_after_ms;
                    top.tree = rendered.tree;
                    let owner = top.owner.clone();
                    // A tree that lands on screen has its icons resolved in
                    // the open command's package folder and starts the
                    // loads its icons need, as a list's do.
                    top.loading = landed(
                        &state.packages,
                        &state.icon_loads,
                        &component,
                        owner.as_ref(),
                        &mut top.tree,
                    );
                    stack.shown()
                };
                state.view.screen = super::Screen::DesignedView(screen);
                state.view.title = title;
                state.view.status = Status::Idle;
            }
            Ok(DesignedNext::Pushed(opened, rendered)) => {
                let component = state.open.clone().unwrap_or_default();
                let (screen, title) = {
                    let stack = state.designed_view.as_mut().expect("a view is open");
                    stack.top_mut().shown = number;
                    let owner = stack.top().owner.clone();
                    stack.views.push(opened_view(
                        opened,
                        owner,
                        rendered,
                        &state.packages,
                        &state.icon_loads,
                        &component,
                    ));
                    stack.shown()
                };
                state.view.screen = super::Screen::DesignedView(screen);
                state.view.title = title;
                state.view.status = Status::Idle;
            }
            Ok(DesignedNext::Replaced(opened, rendered)) => {
                let component = state.open.clone().unwrap_or_default();
                let (screen, title) = {
                    let stack = state.designed_view.as_mut().expect("a view is open");
                    stack.top_mut().shown = number;
                    let owner = stack.top().owner.clone();
                    *stack.top_mut() = opened_view(
                        opened,
                        owner,
                        rendered,
                        &state.packages,
                        &state.icon_loads,
                        &component,
                    );
                    stack.shown()
                };
                state.view.screen = super::Screen::DesignedView(screen);
                state.view.title = title;
                state.view.status = Status::Idle;
            }
            Ok(DesignedNext::Popped { result }) => {
                // The view popped itself: dropped in the runtime (never
                // used again), the stack entry going with it. The view
                // below's last tree shows at once, then it is told the view
                // above popped, with the result the pop answered.
                let empty = {
                    let stack = state.designed_view.as_mut().expect("a view is open");
                    stack.views.pop();
                    stack.views.is_empty()
                };
                if empty {
                    // The root view popped: the command's screen went with
                    // it.
                    self.show_root(state, None);
                } else {
                    let (screen, title) = state
                        .designed_view
                        .as_ref()
                        .expect("a view is open")
                        .shown();
                    state.view.screen = super::Screen::DesignedView(screen);
                    state.view.title = title;
                    state.view.status = Status::Idle;
                    let epoch = state.screen_epoch;
                    self.deliver_designed_pop(state, epoch, Some(result));
                }
            }
            // The view refused the event, or answered a tree Pane cannot
            // read: it keeps its last good tree either way.
            Err(error @ CallError::Guest(_)) | Err(error @ CallError::Unreadable(_)) => {
                state
                    .designed_view
                    .as_mut()
                    .expect("a view is open")
                    .top_mut()
                    .shown = number;
                state.view.status = Status::Error(error.to_string());
            }
            // The guest instance, and the view with it, is gone: the
            // command's screen went with it, back to root search.
            Err(error) => {
                let status = Status::Error(error.to_string());
                self.show_root(state, None);
                state.view.status = status;
            }
        }
    }

    /// Tells the top view that the one above it popped — the pop event,
    /// callback id 0, its payload the result the pop answered with (or
    /// `null`, a pop that carried none) — and shows its answer: the event
    /// is sent now, in the stack's turn, and its answer applied in the
    /// background, the window woken when it lands. The view below's last
    /// tree is already on screen.
    fn deliver_designed_pop(&self, state: &mut State, epoch: u64, result: Option<String>) {
        let payload = serde_json::json!({ "pop": result }).to_string();
        let sent = {
            let Some(stack) = state.designed_view.as_mut() else {
                return;
            };
            let event = DesignedEvent {
                render: stack.top().rendered,
                key: String::new(),
                callback: POP_CALLBACK,
                payload,
            };
            match self.runtime() {
                Ok(runtime) => Some(stack.top_mut().send(runtime, event)),
                Err(_) => None,
            }
        };
        let Some(sent) = sent else {
            return;
        };
        let events = state.designed_events.clone();
        events.begin();
        let ended = events.clone();
        let launcher = self.clone();
        let started = std::thread::Builder::new()
            .name("pane-designed-pop".into())
            .spawn(move || {
                let result = futures::executor::block_on(sent.reply);
                launcher.show_designed_answer(epoch, sent.view, sent.number, result);
                launcher.changed();
                ended.end();
            });
        if started.is_err() {
            // The pop event could not be delivered; the view below keeps
            // the tree it already shows.
            events.end();
        }
    }

    /// Closes `opened`, a view an answer the launcher does not apply pushed
    /// or replaced in: the runtime would otherwise hold a view whose
    /// screen was never shown.
    fn close_unstacked(&self, opened: Option<ViewId>) {
        if let Some(opened) = opened
            && let Ok(runtime) = self.runtime()
        {
            runtime.close_designed_view(opened);
        }
    }

    /// How many designed views are open in guest instances. A diagnostic
    /// for tests and logs, as [`Runtime::view_count`] is.
    pub fn designed_view_count(&self) -> impl Future<Output = usize> + Send + 'static {
        let runtime = self.runtime().ok().cloned();
        async move {
            match runtime {
                Some(runtime) => runtime.designed_view_count().await,
                None => 0,
            }
        }
    }

    /// How deep the open designed view's navigation stack is: the root
    /// view is 1, each view a push added one deeper; 0 with no designed
    /// view open. A diagnostic for tests and logs, as
    /// [`Launcher::designed_view_count`] is;
    /// [`MAX_NAVIGATION_DEPTH`](crate::runtime::MAX_NAVIGATION_DEPTH) is
    /// the most it may be.
    pub fn designed_stack_depth(&self) -> usize {
        self.lock()
            .designed_view
            .as_ref()
            .map_or(0, |stack| stack.views.len())
    }

    /// Waits until every designed view event the launcher delivers in the
    /// background — the pop event the back key's or a view's own pop sends
    /// — has been answered and shown; `false` if one has not within
    /// `limit`. For tests and development builds, which so wait for the
    /// view below's re-render without timing it.
    #[cfg(any(test, debug_assertions))]
    #[doc(hidden)]
    pub fn wait_for_designed_events(&self, limit: std::time::Duration) -> bool {
        let events = self.lock().designed_events.clone();
        events.settled(limit)
    }

    /// Draws the loaded icons of the open designed view's tree, as a list
    /// draws the rows whose loads arrived (#142): a web image, a system
    /// icon or an application's own icon that is ready takes the place of
    /// the fallback shown for it until then. Whether any icon changed; a
    /// window that sees one draws again.
    pub fn refresh_designed_view(&self) -> bool {
        let mut guard = self.lock();
        let state = &mut *guard;
        let Some(stack) = state.designed_view.as_ref() else {
            return false;
        };
        if !stack.top().loading {
            return false;
        }
        let owner = stack.top().owner.as_ref().map(|identity| identity.key());
        let super::Screen::DesignedView(snapshot) = &mut state.view.screen else {
            return false;
        };
        shown(&mut snapshot.tree, owner.as_deref(), &state.icon_loads)
    }

    /// Closes the open designed view, if one is open: every view of its
    /// stack is dropped in the runtime, and its screen leaves with it (the
    /// caller shows what replaces it).
    pub(super) fn close_designed_view(&self, state: &mut State) {
        let stack = state.designed_view.take();
        if let Some(stack) = stack
            && let Ok(runtime) = self.runtime()
        {
            // The top first: the views' destructors run as pops would.
            for open in stack.views.iter().rev() {
                runtime.close_designed_view(open.id);
            }
        }
    }
}

/// The stack entry of the view the answer `opened` — pushed above the
/// stack's top or replacing it — holding its first tree, its icons
/// resolved and its loads started as any tree landing on screen has them.
fn opened_view(
    opened: ViewId,
    owner: Option<PackageIdentity>,
    rendered: DesignedRendered,
    packages: &[InstalledPackage],
    loads: &super::icon_loads::IconLoads,
    component: &Path,
) -> OpenDesignedView {
    let mut tree = rendered.tree;
    let loading = landed(packages, loads, component, owner.as_ref(), &mut tree);
    OpenDesignedView {
        id: opened,
        owner,
        rendered: rendered.render,
        sent: 0,
        shown: 0,
        loading,
        refresh_after_ms: rendered.refresh_after_ms,
        tree,
    }
}

/// Whether the answer `result` to the event number `number`, whose view's
/// answer on screen is number `shown`, is stale: an answer to an event
/// older than the one on screen, which the view's end is not — a crashed
/// view closes its screen however late its answer.
fn stale(number: u64, shown: u64, result: &Result<DesignedNext, CallError>) -> bool {
    number <= shown
        && matches!(
            result,
            Ok(_) | Err(CallError::Guest(_)) | Err(CallError::Unreadable(_))
        )
}

/// A designed view's tree landing on screen: its icons resolved in the
/// open command's package folder (or, for a command built into Pane, beside
/// its component), exactly as a list's looks are, and what they need of
/// the host's loads started (web images, system icons, applications' own
/// icons, #142) — the view never waits for them. Whether the tree holds
/// an icon Pane loads.
fn landed(
    packages: &[InstalledPackage],
    loads: &super::icon_loads::IconLoads,
    component: &Path,
    owner: Option<&PackageIdentity>,
    tree: &mut DesignedTree,
) -> bool {
    let package = super::owner(packages, component);
    let folder = package
        .map(|package| package.location.clone())
        .or_else(|| component.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    resolve_node(&mut tree.root, &folder);
    let identity = owner.or_else(|| package.map(|package| &package.identity));
    want_node(&tree.root, identity, loads)
}

/// Every icon `node` holds, resolved in `folder` (see [`Icon::resolved`]),
/// and its children's and its `fallback`'s with it.
fn resolve_node(node: &mut Node, folder: &Path) {
    let of = |icon: &mut Option<Icon>| {
        *icon = icon.take().and_then(|icon| icon.resolved(folder));
    };
    match &mut node.kind {
        NodeKind::Button(button) => of(&mut button.icon),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => of(&mut icon.icon),
        NodeKind::Image(image) => of(&mut image.image),
        NodeKind::RichRow(row) => of(&mut row.icon),
        NodeKind::EmptyState(empty) => of(&mut empty.icon),
        _ => {}
    }
    if let Some(fallback) = &mut node.fallback {
        resolve_node(fallback, folder);
    }
    for child in &mut node.children {
        resolve_node(child, folder);
    }
}

/// Whether any icon `node` or its descendants hold needs the host's loads,
/// starting them for `identity`'s package as a list does.
fn want_node(
    node: &Node,
    identity: Option<&PackageIdentity>,
    loads: &super::icon_loads::IconLoads,
) -> bool {
    let mut wanted = false;
    let want = |icon: Option<&Icon>, wanted: &mut bool| {
        if let Some(icon) = icon {
            loads.want(identity, icon);
            *wanted |= loads_icon(icon);
        }
    };
    match &node.kind {
        NodeKind::Button(button) => want(button.icon.as_ref(), &mut wanted),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => want(icon.icon.as_ref(), &mut wanted),
        NodeKind::Image(image) => want(image.image.as_ref(), &mut wanted),
        NodeKind::RichRow(row) => want(row.icon.as_ref(), &mut wanted),
        NodeKind::EmptyState(empty) => want(empty.icon.as_ref(), &mut wanted),
        _ => {}
    }
    if let Some(fallback) = node.fallback.as_deref() {
        wanted |= want_node(fallback, identity, loads);
    }
    for child in &node.children {
        wanted |= want_node(child, identity, loads);
    }
    wanted
}

/// Whether `icon` is one Pane loads: a web image, a system icon or an
/// application's own.
fn loads_icon(icon: &Icon) -> bool {
    match &icon.source {
        IconSource::Url(url) => is_web_url(url),
        IconSource::File(_) | IconSource::Application(_) => true,
        IconSource::Builtin { .. } | IconSource::Image { .. } | IconSource::Letter(_) => false,
    }
}

/// The icons of the tree on screen replaced by what arrived of their loads
/// (see [`IconLoads::shown`]): whether any of them changed.
fn shown(
    tree: &mut DesignedTree,
    owner: Option<&str>,
    loads: &super::icon_loads::IconLoads,
) -> bool {
    shown_node(&mut tree.root, owner, loads)
}

/// The icons `node` holds, shown as their loads allow, and its
/// descendants'.
fn shown_node(node: &mut Node, owner: Option<&str>, loads: &super::icon_loads::IconLoads) -> bool {
    let mut changed = false;
    let show = |icon: &mut Option<Icon>, changed: &mut bool| {
        if let Some(held) = icon.as_ref() {
            let shown = loads.shown(owner, held);
            *changed |= held != &shown;
            *icon = Some(shown);
        }
    };
    match &mut node.kind {
        NodeKind::Button(button) => show(&mut button.icon, &mut changed),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => show(&mut icon.icon, &mut changed),
        NodeKind::Image(image) => show(&mut image.image, &mut changed),
        NodeKind::RichRow(row) => show(&mut row.icon, &mut changed),
        NodeKind::EmptyState(empty) => show(&mut empty.icon, &mut changed),
        _ => {}
    }
    if let Some(fallback) = &mut node.fallback {
        changed |= shown_node(fallback, owner, loads);
    }
    for child in &mut node.children {
        changed |= shown_node(child, owner, loads);
    }
    changed
}
