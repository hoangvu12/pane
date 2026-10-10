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
//!
//! An event is raised on the tree the user saw (#238): the window's
//! controls carry the render they were drawn from, and an event raised on
//! a node the user could see is delivered even if the view has rendered
//! since — while the view holds no render more than one past it (the two
//! the SDKs keep) and the node with that key still names a handler of the
//! event's kind; otherwise the event is dropped, and reported in the
//! extension's log while its package is developed. The same log holds a
//! developed package's key problems (keys shared by siblings, stateful
//! nodes without one), reported by the runtime as it reads each tree.
//!
//! A render's answer may also ask Pane to draw the view again after some
//! milliseconds (`refresh-after-ms`, #236): the refresh thread (see
//! `refresh`) sends that drawing numbered with these events, at most one
//! at a time, only while the view is on top and the launcher is shown.

use std::path::Path;
use std::pin::Pin;

use super::{Launcher, Opening, State, Status, stopped};
use crate::extension_data::PackageData;
use crate::extension_log::LogLevel;
use crate::icons::{Icon, IconSource, is_web_url};
use crate::packages::{InstalledPackage, PackageIdentity};
use crate::runtime::{
    CallError, DesignedEvent, DesignedHandler, DesignedNext, DesignedRendered, DesignedTree, Node,
    NodeKind, Runtime, ViewId,
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
                list: super::designed_list::ListHeld::default(),
            }],
        }
    }

    /// The top view: the one on screen.
    fn top(&self) -> &OpenDesignedView {
        self.views.last().expect("the stack holds the root")
    }

    /// The top view, changed.
    pub(super) fn top_mut(&mut self) -> &mut OpenDesignedView {
        self.views.last_mut().expect("the stack holds the root")
    }

    /// The id of the top view, as its render context named it: the view
    /// the refresh thread draws a pushed drawing for (#243).
    pub(super) fn top_id(&self) -> u64 {
        self.top().id.number()
    }

    /// Whether the view `view` — the id its render context named — is one
    /// of the stack's, so an ask for it is one to hold; one for any other
    /// view is dropped.
    pub(super) fn holds(&self, view: u64) -> bool {
        self.views.iter().any(|open| open.id.number() == view)
    }

    /// The screen of the top view: its snapshot, and its navigation title
    /// as the screen's title. The snapshot's list is filled by
    /// [`super::Launcher::present_designed_list`], which every path that
    /// shows a screen follows.
    fn shown(&self) -> (super::DesignedViewSnapshot, String) {
        let top = self.top();
        (
            super::DesignedViewSnapshot {
                id: top.id,
                render: top.rendered,
                tree: top.tree.clone(),
                list: None,
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
    /// answer arriving late does not replace a newer one. A refresh sent
    /// by the view's own ask (`refresh-after-ms`, see `refresh`) is
    /// numbered with them, so a late refresh answer never replaces a newer
    /// tree either.
    shown: u64,
    /// Whether the tree on screen holds an icon Pane loads (a web image,
    /// a system icon, an application's), whose arrivals redraw it.
    loading: bool,
    /// The `refresh-after-ms` the last render answered: the view's refresh
    /// is rescheduled through it when its answer lands (see `refresh`).
    refresh_after_ms: Option<u32>,
    /// The view's last good tree: what shows at once when the view above
    /// it pops, before the view re-renders.
    tree: DesignedTree,
    /// The List or Grid this view's tree names: its search text and
    /// selection, which Pane owns (#240), with the bookkeeping of the
    /// events it raises. A tree that names no list holds it unused.
    list: super::designed_list::ListHeld,
}

/// One event sent to a designed view, with its reply: a user's event, or
/// one of its drawings with no event — the refresh its answer asked for
/// (see [`OpenDesignedView::send_refresh`]) or the push its own work
/// asked for (see [`OpenDesignedView::send_push`]).
pub(super) struct SentDesignedEvent {
    /// The view the event was sent to: one of the stack's, which must
    /// still be its top when the answer lands.
    pub(super) view: ViewId,
    /// The event's number among those sent to the view.
    pub(super) number: u64,
    pub(super) reply: Pin<Box<dyn Future<Output = Result<DesignedNext, CallError>> + Send>>,
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

    /// Sends the view's refresh — the `render` it asked Pane for again by
    /// `refresh-after-ms` — numbered with its events, so a late answer
    /// never replaces a newer tree. A refresh answers a tree, never a
    /// navigation, so its answer is shown as the tree it re-rendered.
    pub(super) fn send_refresh(&mut self, runtime: &Runtime) -> SentDesignedEvent {
        self.sent += 1;
        // The refresh is sent now, not when the reply is first polled.
        let reply = runtime.refresh_designed_view(self.id);
        SentDesignedEvent {
            view: self.id,
            number: self.sent,
            reply: Box::pin(async move { reply.await.map(DesignedNext::Tree) }),
        }
    }

    /// Sends the drawing the view's own work asked for (#243), numbered as
    /// a refresh's is: the same rules, with the render's context saying
    /// the view's push asked for it, so an interval's work does not run in
    /// it.
    pub(super) fn send_push(&mut self, runtime: &Runtime) -> SentDesignedEvent {
        self.sent += 1;
        // The drawing is sent now, not when the reply is first polled.
        let reply = runtime.push_designed_view(self.id);
        SentDesignedEvent {
            view: self.id,
            number: self.sent,
            reply: Box::pin(async move { reply.await.map(DesignedNext::Tree) }),
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
                state.browsing = None;
                state.designed_view = Some(stack);
                state.next_screen();
                state.view = view;
                // The List or Grid the view's tree names is presented, its
                // rows filling the view (#240).
                self.present_designed_list(&mut *state);
                // The first render's own ask schedules the view's refresh
                // (see `refresh`).
                self.view_refresh_asked(rendered.refresh_after_ms);
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
        self.send_designed_seen(DesignedHandler::Press, callback, key, None, "{}".to_owned())
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
        self.send_designed_seen(DesignedHandler::Change, callback, key, None, payload)
    }

    /// Sends one event of the open designed view, raised on the tree the
    /// user saw — the tree of the render `seen`, which the node was drawn
    /// from (the view may have rendered since; `None` names the render on
    /// screen now). The event is the node's handler of `handler`'s kind,
    /// run by callback id `callback`, on the node with `key` — or none,
    /// when the tree gave it none — and its `payload` names what changed.
    ///
    /// An event raised on a node the user could see is delivered even if
    /// the view has rendered since — while the view holds no render more
    /// than one past `seen` (the two the SDKs keep) and the node with that
    /// key still names a handler of that kind; otherwise it is dropped,
    /// logged in development, and its answer never comes. The tree the
    /// user saw decides the callback id and the render the event carries.
    pub fn send_designed_seen(
        &self,
        handler: DesignedHandler,
        callback: u32,
        key: Option<&str>,
        seen: Option<u64>,
        payload: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let epoch = state.screen_epoch;
        // The view as it is now: the tree the user saw (named by `seen`,
        // else the one on screen), the event built from it, and whether
        // the stale-event rule drops that event.
        let held = state.designed_view.as_ref().map(|stack| {
            let top = stack.top();
            let seen = seen.unwrap_or(top.rendered);
            let key = key.map(str::to_owned);
            let dropped = if seen + 1 < top.rendered {
                Some("the view has rendered twice since")
            } else if key
                .as_deref()
                .is_some_and(|key| !tree_holds(&top.tree, key, handler))
            {
                Some("the view no longer names a handler for its key")
            } else {
                None
            };
            let event = DesignedEvent {
                render: seen,
                key: key.unwrap_or_default(),
                callback,
                payload,
            };
            (event, dropped)
        });
        // A dropped event never reaches the view: its node went (or its
        // handler), or the view rendered twice since the user saw it.
        if let Some((_, Some(why))) = &held {
            let dropped = held
                .as_ref()
                .map(|(event, _)| (event.key.as_str(), event.render));
            self.note_dropped_event(&mut state, handler, dropped, *why);
        }
        let sent = match (held, self.runtime()) {
            (Some((event, None)), Ok(runtime)) => {
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

    /// Notes a dropped designed event in the developed package's log:
    /// what was raised, on which key of which render, and why it went.
    fn note_dropped_event(
        &self,
        state: &mut State,
        handler: DesignedHandler,
        dropped: Option<(&str, u64)>,
        why: &str,
    ) {
        let Some((key, render)) = dropped else {
            return;
        };
        let component = state.open.clone().unwrap_or_default();
        let owner = super::owner(&state.packages, &component).map(|package| package.identity.key());
        let Some(owner) = owner else {
            return;
        };
        if !self.developing.logs.developed(&owner) {
            return;
        }
        let what = match handler {
            DesignedHandler::Press => "a press",
            DesignedHandler::Change => "a change",
            DesignedHandler::Input => "an input",
            DesignedHandler::Focus => "a focus",
            DesignedHandler::Blur => "a blur",
            DesignedHandler::Key => "a key",
            DesignedHandler::Selection => "a selection",
            DesignedHandler::More => "a load-more",
        };
        self.developing.logs.pane(
            &owner,
            0,
            LogLevel::Warn,
            &format!("{what} on the key \"{key}\" of render {render} was dropped: {why}"),
        );
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
        self.reschedule_view_refresh(stack);
        state.view.screen = super::Screen::DesignedView(screen);
        state.view.title = title;
        state.view.status = Status::Idle;
        // The view below's list, as it was when the view above it was
        // pushed: shown again with its rows.
        self.present_designed_list(&mut *state);
        let epoch = state.screen_epoch;
        self.deliver_designed_pop(state, epoch, None);
        true
    }

    /// The popped view's ask went with it; the view below's own ask is
    /// live again, rescheduled from its entry as its answer would schedule
    /// it (an empty stack closes the screen, which cancels the refresh).
    fn reschedule_view_refresh(&self, stack: &DesignedStack) {
        self.view_refresh_asked(stack.top().refresh_after_ms);
    }

    /// Sends the open designed view's refresh — the `render` the view on
    /// top asked Pane for again by `refresh-after-ms` — numbered with its
    /// events, and returns it for the refresh thread to run to its answer
    /// (see `refresh`). The launcher's state must be held by the caller
    /// while it decides. `None` when no view is open or the runtime is
    /// gone.
    pub(super) fn start_view_refresh(&self, state: &mut State) -> Option<(u64, SentDesignedEvent)> {
        let epoch = state.screen_epoch;
        let stack = state.designed_view.as_mut()?;
        let runtime = self.runtime().ok()?;
        Some((epoch, stack.top_mut().send_refresh(runtime)))
    }

    /// As [`Launcher::start_view_refresh`], for the drawing the view's own
    /// work asked for (#243, `pane:extension/view`): served by the same
    /// thread, under the same rules.
    pub(super) fn start_view_push(&self, state: &mut State) -> Option<(u64, SentDesignedEvent)> {
        let epoch = state.screen_epoch;
        let stack = state.designed_view.as_mut()?;
        let runtime = self.runtime().ok()?;
        Some((epoch, stack.top_mut().send_push(runtime)))
    }

    /// The designed view's answer asked Pane to render it again after
    /// `after_ms`, or not again (`None`): the view's refresh is scheduled
    /// — clamped to the floor and the ceiling — or cancelled (see
    /// `refresh`). The answer that asked rules: one that failed carries no
    /// ask, so it ends any asked before it.
    fn view_refresh_asked(&self, after_ms: Option<u32>) {
        if let Some(refresh) = &self.refresh {
            match after_ms {
                Some(after) => refresh.asked(after),
                None => refresh.cancel(),
            }
        }
    }

    /// Shows the answer to the event number `number` of the designed view
    /// `view`: the tree it re-rendered, the view the answer pushed or
    /// replaced (now the top of the stack), the view below's last tree when
    /// it popped, or the error it answered with (the view keeps its last
    /// good tree). An answer for a view that is no longer the top of the
    /// stack, or older than the answer on screen, is dropped — with any
    /// view the answer stacked closed again, so the runtime holds no view
    /// whose screen was never shown. A refresh's answer is shown the same
    /// way, numbered with the events.
    pub(super) fn show_designed_answer(
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
                self.present_designed_list(&mut *state);
                // The answer's own ask paces the view's next refresh.
                self.view_refresh_asked(rendered.refresh_after_ms);
            }
            Ok(DesignedNext::Pushed(opened, rendered)) => {
                let component = state.open.clone().unwrap_or_default();
                // The pushed view's first render asks for its own refresh.
                let ask = rendered.refresh_after_ms;
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
                self.present_designed_list(&mut *state);
                self.view_refresh_asked(ask);
            }
            Ok(DesignedNext::Replaced(opened, rendered)) => {
                let component = state.open.clone().unwrap_or_default();
                // The replacing view's first render asks for its own
                // refresh.
                let ask = rendered.refresh_after_ms;
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
                self.present_designed_list(&mut *state);
                self.view_refresh_asked(ask);
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
                    // it, and closing it cancels the refresh.
                    self.show_root(state, None);
                } else {
                    let (screen, title, ask) = {
                        let stack = state.designed_view.as_ref().expect("a view is open");
                        let shown = stack.shown();
                        (shown.0, shown.1, stack.top().refresh_after_ms)
                    };
                    state.view.screen = super::Screen::DesignedView(screen);
                    state.view.title = title;
                    state.view.status = Status::Idle;
                    self.present_designed_list(&mut *state);
                    // The popped view's ask went with it; the view below's
                    // own ask is live again.
                    self.view_refresh_asked(ask);
                    let epoch = state.screen_epoch;
                    self.deliver_designed_pop(state, epoch, Some(result));
                }
            }
            // The view refused the event, or answered a tree Pane cannot
            // read: it keeps its last good tree either way, and its next
            // answer — this one carried no ask — must ask for a refresh
            // again.
            Err(error @ CallError::Guest(_)) | Err(error @ CallError::Unreadable(_)) => {
                state
                    .designed_view
                    .as_mut()
                    .expect("a view is open")
                    .top_mut()
                    .shown = number;
                state.view.status = Status::Error(error.to_string());
                self.view_refresh_asked(None);
            }
            // The guest instance, and the view with it, is gone: the
            // command's screen went with it, back to root search. Its
            // closing cancels the refresh.
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
    /// stack is dropped in the runtime, its screen leaves with it (the
    /// caller shows what replaces it), and its refresh is cancelled — the
    /// ask belongs to the view, and a view opened afresh asks anew.
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
        if let Some(refresh) = &self.refresh {
            refresh.left();
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
        list: super::designed_list::ListHeld::default(),
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

/// Whether `tree` holds a node with `key` naming a handler of `handler`'s
/// kind — the stale-event rule's check: an event raised on a node the user
/// could see is delivered while the node with that key still has a handler
/// for it, wherever in the tree the node now sits.
fn tree_holds(tree: &DesignedTree, key: &str, handler: DesignedHandler) -> bool {
    fn held(node: &Node, key: &str, handler: DesignedHandler) -> bool {
        (node.key.as_deref() == Some(key) && node.handles(handler))
            || node
                .fallback
                .as_deref()
                .is_some_and(|fallback| held(fallback, key, handler))
            || node.children.iter().any(|child| held(child, key, handler))
    }
    held(&tree.root, key, handler)
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
/// and its children's and its `fallback`'s with it: a List item's and a
/// Grid cell's, and a Markdown node's images, with the rest (#240).
fn resolve_node(node: &mut Node, folder: &Path) {
    let of = |icon: &mut Option<Icon>| {
        *icon = icon.take().and_then(|icon| icon.resolved(folder));
    };
    let images = |blocks: &mut Vec<crate::markdown::Block>| {
        crate::markdown::visit_images(blocks, &mut |held, source| {
            if held.is_none() {
                *held = markdown_icon(source);
            }
            of(held);
        });
    };
    match &mut node.kind {
        NodeKind::Button(button) => of(&mut button.icon),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => of(&mut icon.icon),
        NodeKind::Image(image) => of(&mut image.image),
        NodeKind::RichRow(row) => of(&mut row.icon),
        NodeKind::EmptyState(empty) => of(&mut empty.icon),
        NodeKind::ListItem(item) => of(&mut item.icon),
        NodeKind::GridItem(item) => of(&mut item.image),
        NodeKind::Markdown(markdown) => images(&mut markdown.blocks),
        _ => {}
    }
    if let Some(fallback) = &mut node.fallback {
        resolve_node(fallback, folder);
    }
    for child in &mut node.children {
        resolve_node(child, folder);
    }
    if let NodeKind::ListItem(item) = &mut node.kind
        && let Some(detail) = &mut item.detail
    {
        resolve_node(detail, folder);
    }
}

/// The icon a Markdown image's `source` reads as: a URL as a URL, anything
/// else a packaged image's path.
fn markdown_icon(source: &str) -> Option<Icon> {
    let value = if source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("data:")
    {
        serde_json::json!({ "url": source })
    } else {
        serde_json::json!({ "path": source })
    };
    crate::icons::read(&value)
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
    let images = |blocks: &[crate::markdown::Block], wanted: &mut bool| {
        crate::markdown::each_image(blocks, &mut |icon: &Icon| {
            loads.want(identity, icon);
            *wanted |= loads_icon(icon);
        });
    };
    match &node.kind {
        NodeKind::Button(button) => want(button.icon.as_ref(), &mut wanted),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => want(icon.icon.as_ref(), &mut wanted),
        NodeKind::Image(image) => want(image.image.as_ref(), &mut wanted),
        NodeKind::RichRow(row) => want(row.icon.as_ref(), &mut wanted),
        NodeKind::EmptyState(empty) => want(empty.icon.as_ref(), &mut wanted),
        NodeKind::ListItem(item) => want(item.icon.as_ref(), &mut wanted),
        NodeKind::GridItem(item) => want(item.image.as_ref(), &mut wanted),
        NodeKind::Markdown(markdown) => images(&markdown.blocks, &mut wanted),
        _ => {}
    }
    if let Some(fallback) = node.fallback.as_deref() {
        wanted |= want_node(fallback, identity, loads);
    }
    for child in &node.children {
        wanted |= want_node(child, identity, loads);
    }
    if let NodeKind::ListItem(item) = &node.kind
        && let Some(detail) = item.detail.as_deref()
    {
        wanted |= want_node(detail, identity, loads);
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
    let images = |blocks: &mut Vec<crate::markdown::Block>, changed: &mut bool| {
        crate::markdown::visit_images(blocks, &mut |held, _| {
            if let Some(icon) = held.as_ref() {
                let shown = loads.shown(owner, icon);
                *changed |= icon != &shown;
                *held = Some(shown);
            }
        });
    };
    match &mut node.kind {
        NodeKind::Button(button) => show(&mut button.icon, &mut changed),
        NodeKind::Icon(icon) | NodeKind::IconTile(icon) => show(&mut icon.icon, &mut changed),
        NodeKind::Image(image) => show(&mut image.image, &mut changed),
        NodeKind::RichRow(row) => show(&mut row.icon, &mut changed),
        NodeKind::EmptyState(empty) => show(&mut empty.icon, &mut changed),
        NodeKind::ListItem(item) => show(&mut item.icon, &mut changed),
        NodeKind::GridItem(item) => show(&mut item.image, &mut changed),
        NodeKind::Markdown(markdown) => images(&mut markdown.blocks, &mut changed),
        _ => {}
    }
    if let Some(fallback) = &mut node.fallback {
        changed |= shown_node(fallback, owner, loads);
    }
    for child in &mut node.children {
        changed |= shown_node(child, owner, loads);
    }
    if let NodeKind::ListItem(item) = &mut node.kind
        && let Some(detail) = &mut item.detail
    {
        changed |= shown_node(detail, owner, loads);
    }
    changed
}
