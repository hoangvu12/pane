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
//!
//! A render's answer may also ask Pane to draw the view again after some
//! milliseconds (`refresh-after-ms`, #236): the refresh thread (see
//! `refresh`) sends that drawing numbered with these events, at most one
//! at a time, only while the view is on top and the launcher is shown.

use std::pin::Pin;

use super::{Launcher, Opening, State, Status, stopped};
use crate::extension_data::PackageData;
use crate::runtime::{CallError, DesignedEvent, DesignedRendered, Runtime, ViewId};

/// The designed view on screen, if one is open: the command's own screen.
pub(super) struct OpenDesignedView {
    /// The view in the runtime; closed when the view leaves the screen.
    id: ViewId,
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
}

/// One event sent to the designed view, with its reply: a user's event, or
/// the view's refresh (see [`OpenDesignedView::send_refresh`]).
pub(super) struct SentDesignedEvent {
    /// The event's number among those sent to the view.
    pub(super) number: u64,
    pub(super) reply: Pin<Box<dyn Future<Output = Result<DesignedRendered, CallError>> + Send>>,
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

    /// Sends the view's refresh — `render` again, with no event — numbered
    /// for ordering as an event is: at most one is in flight (see
    /// `refresh`), and a late answer is dropped with the numbering.
    fn send_refresh(&mut self, runtime: &Runtime) -> SentDesignedEvent {
        self.sent += 1;
        SentDesignedEvent {
            number: self.sent,
            reply: Box::pin(runtime.refresh_designed_view(self.id)),
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
                let view = super::LauncherView::new(
                    super::Screen::DesignedView(super::DesignedViewSnapshot {
                        id,
                        tree: rendered.tree,
                    }),
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
                    rendered: rendered.render,
                    sent: 0,
                    shown: 0,
                });
                state.next_screen();
                state.view = view;
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
                payload: "{}".to_owned(),
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

    /// Sends the open designed view's refresh — the `render` it asked Pane
    /// for again by `refresh-after-ms` — numbered with its events, and
    /// returns it for the refresh thread to run to its answer (see
    /// `refresh`). The launcher's state must be held by the caller while
    /// it decides. `None` when no view is open or the runtime is gone.
    pub(super) fn start_view_refresh(&self, state: &mut State) -> Option<(u64, SentDesignedEvent)> {
        let epoch = state.screen_epoch;
        let open = state.designed_view.as_mut()?;
        let runtime = self.runtime().ok()?;
        Some((epoch, open.send_refresh(runtime)))
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

    /// Shows the open designed view's answer to its event number `number`:
    /// the tree it re-rendered, the error it answered with (the view keeps
    /// its last good tree), or its end (the view is gone, and the command
    /// with it). A refresh's answer is shown the same way, numbered with
    /// the events.
    pub(super) fn show_designed_answer(
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
                let super::Screen::DesignedView(snapshot) = &mut state.view.screen else {
                    unreachable!("a designed view is open");
                };
                snapshot.tree = rendered.tree;
                state.view.status = Status::Idle;
                // The answer's own ask paces the view's next refresh.
                self.view_refresh_asked(rendered.refresh_after_ms);
            }
            // The view refused the event, or answered a tree Pane cannot
            // read: it keeps its last good tree either way, and its next
            // answer — this one carried no ask — must ask for a refresh
            // again.
            Err(error @ CallError::Guest(_)) | Err(error @ CallError::Unreadable(_)) => {
                open.shown = number;
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

    /// Closes the open designed view, if one is open: the view is dropped
    /// in the runtime, its screen leaves with it (the caller shows what
    /// replaces it), and its refresh is cancelled — the ask belongs to the
    /// view, and a view opened afresh asks anew.
    pub(super) fn close_designed_view(&self, state: &mut State) {
        let open = state.designed_view.take();
        if let Some(open) = open
            && let Ok(runtime) = self.runtime()
        {
            runtime.close_designed_view(open.id);
        }
        if let Some(refresh) = &self.refresh {
            refresh.cancel();
        }
    }
}
