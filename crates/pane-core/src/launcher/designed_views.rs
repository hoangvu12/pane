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
    CallError, DesignedEvent, DesignedRendered, DesignedTree, Node, NodeKind, Runtime, ViewId,
};

/// The designed view on screen, if one is open: the command's own screen.
pub(super) struct OpenDesignedView {
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
}

/// One event sent to the designed view, with its reply.
struct SentDesignedEvent {
    /// The event's number among those sent to the view.
    number: u64,
    reply: Pin<Box<dyn Future<Output = Result<DesignedRendered, CallError>> + Send>>,
}

impl OpenDesignedView {
    /// Sends `event` to the view, numbering it for ordering.
    fn send(&mut self, runtime: &Runtime, event: DesignedEvent) -> SentDesignedEvent {
        self.sent += 1;
        SentDesignedEvent {
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
                let view = super::LauncherView::new(
                    super::Screen::DesignedView(super::DesignedViewSnapshot { id, tree }),
                    // The view's own content is its title, as a command's
                    // list is; the footer names the open command.
                    "",
                );
                state.open = Some(component);
                state.launch = launch;
                state.open_command = Some(command);
                state.searching = None;
                state.designed_view = Some(OpenDesignedView {
                    id,
                    owner,
                    rendered: rendered.render,
                    sent: 0,
                    shown: 0,
                    loading,
                    refresh_after_ms: rendered.refresh_after_ms,
                });
                state.next_screen();
                state.view = view;
            }
            Err(error) => state.view.status = Status::Error(error.to_string()),
        }
    }

    /// Sends the press of the button the open designed view's tree named
    /// with callback id `callback` (`onPress`) to the view — raised on the
    /// node with `key`, or none when the tree gave it none — and shows its
    /// answer. Await the returned future to show the new tree; anything
    /// sent when no view is open is ignored.
    ///
    /// Events are handled one at a time, in the order of these calls, and a
    /// tree is shown only if no later event's tree is on screen yet. An
    /// error the extension reports is shown while the view stays open with
    /// its last good tree; a crash closes it.
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
        let event = state.designed_view.as_ref().map(|open| {
            let key = key.unwrap_or_default().to_owned();
            DesignedEvent {
                render: open.rendered,
                key,
                callback,
                payload,
            }
        });
        let sent = match (event, self.runtime()) {
            (Some(event), Ok(runtime)) => {
                let open = state.designed_view.as_mut().expect("a view is open");
                Some(open.send(runtime, event))
            }
            _ => None,
        };
        drop(state);
        let launcher = self.clone();
        async move {
            if let Some(event) = sent {
                let result = event.reply.await;
                launcher.show_designed_answer(epoch, event.number, result);
            }
        }
    }

    /// Shows the open designed view's answer to its event number `number`:
    /// the tree it re-rendered, the error it answered with (the view keeps
    /// its last good tree), or its end (the view is gone, and the command
    /// with it).
    fn show_designed_answer(
        &self,
        epoch: u64,
        number: u64,
        result: Result<DesignedRendered, CallError>,
    ) {
        let Some(mut state) = self.lock_if_current(epoch) else {
            return;
        };
        let state = &mut *state;
        let Some(open) = state.designed_view.as_mut() else {
            return;
        };
        match result {
            // An answer to an event older than the one on screen is stale.
            Ok(_) | Err(CallError::Guest(_)) | Err(CallError::Unreadable(_))
                if number <= open.shown => {}
            Ok(rendered) => {
                open.shown = number;
                open.rendered = rendered.render;
                open.refresh_after_ms = rendered.refresh_after_ms;
                let owner = open.owner.clone();
                let super::Screen::DesignedView(snapshot) = &mut state.view.screen else {
                    unreachable!("a designed view is open");
                };
                snapshot.tree = rendered.tree;
                let component = state.open.clone().unwrap_or_default();
                let loading = landed(
                    &state.packages,
                    &state.icon_loads,
                    &component,
                    owner.as_ref(),
                    &mut snapshot.tree,
                );
                state.designed_view.as_mut().expect("a view is open").loading = loading;
                state.view.status = Status::Idle;
            }
            // The view refused the event, or answered a tree Pane cannot
            // read: it keeps its last good tree either way.
            Err(error @ CallError::Guest(_)) | Err(error @ CallError::Unreadable(_)) => {
                open.shown = number;
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

    /// Draws the loaded icons of the open designed view's tree, as a list
    /// draws the rows whose loads arrived (#142): a web image, a system
    /// icon or an application's own icon that is ready takes the place of
    /// the fallback shown for it until then. Whether any icon changed; a
    /// window that sees one draws again.
    pub fn refresh_designed_view(&self) -> bool {
        let mut guard = self.lock();
        let state = &mut *guard;
        let Some(open) = state.designed_view.as_ref() else {
            return false;
        };
        if !open.loading {
            return false;
        }
        let owner = open.owner.as_ref().map(|identity| identity.key());
        let super::Screen::DesignedView(snapshot) = &mut state.view.screen else {
            return false;
        };
        shown(&mut snapshot.tree, owner.as_deref(), &state.icon_loads)
    }

    /// Closes the open designed view, if one is open: the view is dropped
    /// in the runtime, and its screen leaves with it (the caller shows
    /// what replaces it).
    pub(super) fn close_designed_view(&self, state: &mut State) {
        let open = state.designed_view.take();
        if let Some(open) = open
            && let Ok(runtime) = self.runtime()
        {
            runtime.close_designed_view(open.id);
        }
    }
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
fn shown_node(
    node: &mut Node,
    owner: Option<&str>,
    loads: &super::icon_loads::IconLoads,
) -> bool {
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
