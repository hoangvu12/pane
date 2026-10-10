//! Refreshing a designed view on the delay its render answered
//! (`refresh-after-ms`, #236): a view that shows a timer, a clock, a
//! progress bar or a poll of a service asks Pane to call `render` again
//! after some milliseconds, so the screen changes by itself with no
//! change to the extension runtime.
//!
//! The refresh is one guest call — `render` again, with no event — sent
//! through the view's event numbering (see `designed_views`): at most one
//! is asked for at a time, and a late answer never replaces a newer tree.
//! Each answer carries the next ask, so the view paces itself; the delay
//! is clamped to the 100 ms floor and the 24 h ceiling, and each refresh
//! waits the delay after the answer that asked for it (the period is the
//! delay plus the render's own time, as "render me again after this"
//! reads). Time that passes while a refresh runs, or in one jump of the
//! clock, is served by the next refresh, not replayed.
//!
//! A refresh runs only while the view is on top — today the one designed
//! view the command opened; the navigation stack (#239) will refine this
//! to the stack's top, and the check lives in one place for that — and
//! while the launcher's window is shown ([`WindowPresence`]): a hidden
//! screen refreshes nothing, so it uses no CPU or battery. A refresh that
//! fell due while hidden waits for the next showing, which runs it at
//! once, one refresh from the clock's current time, not the ticks it
//! missed. Leaving the view cancels its refresh: the ask belongs to the
//! view, and a view opened afresh asks anew.
//!
//! This thread of Pane's own is the same shape as the scheduler's and the
//! services' (see `schedules` and `services`): it looks for work due and
//! starts each refresh on a thread of its own, so neither the window nor
//! this thread ever waits for a guest, and it is driven by the launcher's
//! clock ([`crate::clipboard::Clock`]) — the system's, or the one tests
//! and development builds give through [`Launcher::with_clock`], which
//! tells it when the clock is set other than by time passing. The window
//! is woken for each answer through the change channel the continuing
//! services use ([`Launcher::changed`]), so the new tree is drawn although
//! no user action waited for it.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use super::schedules::Wake;
use super::{Launcher, WeakLauncher};
use crate::clipboard::Clock;
use crate::feedback::WindowPresence;

/// The shortest refresh a view may ask Pane for: 100 ms, the proposed
/// default the user delegated (#121). A view that asks for less is
/// refreshed at this pace.
pub(super) const REFRESH_FLOOR_MS: u64 = 100;

/// The longest refresh a view may ask Pane for: 24 hours, so a view
/// always refreshes eventually. A view that asks for more is refreshed at
/// this pace.
pub(super) const REFRESH_CEILING_MS: u64 = 24 * 60 * 60 * 1000;

/// The longest the refresh thread waits before it looks again, so that a
/// change of the system's time, or a computer waking from sleep, delays a
/// refresh by at most this much (what is due is always run when it is
/// looked at).
const MAX_WAIT: Duration = Duration::from_secs(3600);

/// Runs the designed views' refreshes. Kept by the launcher; dropping it
/// stops the thread.
pub(super) struct Refresh {
    /// The clock refreshes follow, and what is scheduled now. Locked with
    /// the launcher's state held, never the other way round.
    state: Mutex<Refreshing>,
    /// Wakes the refresh thread, and tells when it settled.
    wake: Arc<Wake>,
}

/// What the refresh thread keeps: the clock it follows and where the open
/// view's next refresh stands.
struct Refreshing {
    clock: Arc<dyn Clock>,
    /// When the next refresh is due, in clock milliseconds, if one is
    /// scheduled. A due time that has passed stays until the refresh can
    /// run: one that fell due while the view was not shown waits for the
    /// next showing (see [`Refresh::look`]).
    next: Option<u64>,
    /// Whether a refresh was sent and has not answered: no second one is
    /// asked for meanwhile.
    in_flight: bool,
}

impl Refresh {
    /// The refresh thread for the launcher's designed views, following
    /// `clock`. [`Refresh::run`] starts its thread.
    pub(super) fn start(clock: Arc<dyn Clock>) -> Arc<Refresh> {
        let refresh = Arc::new(Refresh {
            state: Mutex::new(Refreshing {
                clock: clock.clone(),
                next: None,
                in_flight: false,
            }),
            wake: Arc::default(),
        });
        // The clock tells when it is set other than by time passing: look
        // again at that, as the scheduler and the services do.
        clock.on_change(Box::new(waking(Arc::downgrade(&refresh))));
        refresh
    }

    /// Starts the refresh thread, which runs until this launcher stops.
    pub(super) fn run(self: &Arc<Self>, launcher: WeakLauncher) {
        let refresh = Arc::downgrade(self);
        let started = std::thread::Builder::new()
            .name("pane-view-refresh".into())
            .spawn(move || refresh_until_stopped(launcher, refresh));
        if let Err(error) = started {
            crate::diagnostic!(
                "Pane cannot re-render extension views in the background: {error}"
            );
        }
    }

    /// The clock the view's refreshes follow from now on
    /// ([`Launcher::with_clock`]). A refresh already scheduled keeps its
    /// due time: a clock that stands past it runs the refresh at once.
    pub(super) fn follow(self: &Arc<Self>, clock: Arc<dyn Clock>) {
        self.lock().clock = clock.clone();
        clock.on_change(Box::new(waking(Arc::downgrade(self))));
        self.wake.poke();
    }

    /// The view's render answer asked Pane to draw it again after
    /// `after_ms`: the refresh is scheduled, clamped to the floor and the
    /// ceiling. The answer that asked rules: one that names no refresh
    /// (see [`Refresh::cancel`]), or failed, ends any asked before it.
    pub(super) fn asked(&self, after_ms: u32) {
        {
            let mut refreshing = self.lock();
            let now = refreshing.clock.now();
            let after = u64::from(after_ms)
                .clamp(REFRESH_FLOOR_MS, REFRESH_CEILING_MS);
            refreshing.next = Some(now.saturating_add(after));
        }
        self.wake.poke();
    }

    /// The view's render answer asked Pane for no drawing again: any
    /// scheduled refresh is cancelled. Leaving the view does the same.
    pub(super) fn cancel(&self) {
        self.lock().next = None;
        self.wake.poke();
    }

    /// Wakes the refresh thread: something it decides by may have changed
    /// (the launcher's window is shown again, which runs a refresh that
    /// fell due while it was hidden).
    pub(super) fn poke(&self) {
        self.wake.poke();
    }

    /// Notes that the refresh the thread started has answered, so another
    /// may be asked for: the answer's own ask was scheduled when it was
    /// shown (see `designed_views`).
    fn answered(&self) {
        self.lock().in_flight = false;
        self.wake.finished();
    }

    /// Waits until the thread looked at every change of the clock so far,
    /// and every refresh it sent has been answered and shown; `false` if
    /// it did not within `limit`. For tests and development builds, which
    /// so wait for a view's refresh without timing it.
    pub(super) fn settled(&self, limit: Duration) -> bool {
        self.wake.settled(limit)
    }

    fn lock(&self) -> MutexGuard<'_, Refreshing> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sends the refresh that is due now, if one is and it can run, and
    /// returns it for the thread the caller starts: the launcher's state
    /// is locked while it decides, as when the user acts, so the refresh
    /// belongs to the screen current now.
    ///
    /// A refresh is sent only while the view is on top and the launcher's
    /// window is shown; one that fell due otherwise waits, and the next
    /// showing runs it at once. None is sent while another is in flight.
    /// A poisoned state is not recovered here (as the scheduler's looks
    /// are not): a thread that panicked while holding it is recovered by
    /// the next caller that changes something, and the next look sees the
    /// state whole.
    fn look(self: &Arc<Self>, launcher: &WeakLauncher) -> Option<RefreshRun> {
        let launcher = launcher.upgrade()?;
        // A poisoned state is not recovered here (a plain lock, not the
        // launcher's recovering one).
        let mut state = match launcher.state.lock() {
            Ok(state) => state,
            Err(_) => return None,
        };
        // The launcher's state is held while the refresh's own is taken,
        // never the other way round (as the scheduler's is).
        let mut refreshing = self.lock();
        if refreshing.in_flight {
            return None;
        }
        let now = refreshing.clock.now();
        if !refreshing.next.is_some_and(|next| next <= now) {
            return None;
        }
        if !on_top_and_shown(&state) {
            return None;
        }
        let sent = launcher.start_view_refresh(&mut state)?;
        refreshing.next = None;
        refreshing.in_flight = true;
        drop(state);
        drop(refreshing);
        let refresh = Arc::downgrade(self);
        Some(RefreshRun {
            launcher,
            epoch: sent.0,
            event: sent.1,
            refresh,
        })
    }

    /// How long the thread may wait before it looks again: until the next
    /// refresh is due, at most [`MAX_WAIT`]. A refresh that is due but
    /// cannot run (the view is not on top, or the launcher is hidden), and
    /// one in flight, wait for a poke: what wakes them is the next
    /// showing, the answer, or a change of the clock.
    fn next_wait(&self) -> Duration {
        let refreshing = self.lock();
        let now = refreshing.clock.now();
        match refreshing.next {
            Some(next) if next > now => Duration::from_millis(next - now).min(MAX_WAIT),
            _ => MAX_WAIT,
        }
    }
}

/// Whether the open designed view is the screen on display and the
/// launcher's window is shown, which is when a refresh runs: the view is
/// "on top" — today the one designed view the command opened, the
/// navigation stack (#239) will refine this to the stack's top — and the
/// window is shown expanded, not hidden or collapsed to its search field,
/// where the view is not drawn. Kept in this one place so #239 adapts it.
fn on_top_and_shown(state: &super::State) -> bool {
    state.designed_view.is_some()
        && matches!(state.view.screen, super::Screen::DesignedView(_))
        && state.feedback.presence == WindowPresence::Shown
}

/// One refresh the thread started: its reply is awaited on a thread of its
/// own, so neither the window nor the refresh thread ever waits for a
/// guest.
struct RefreshRun {
    launcher: Launcher,
    /// The screen epoch when the refresh was sent: an answer arriving
    /// after the user left the screen it was asked from is discarded.
    epoch: u64,
    event: super::designed_views::SentDesignedEvent,
    /// The refresh thread's state, to mark the refresh answered.
    refresh: Weak<Refresh>,
}

impl RefreshRun {
    /// Runs the refresh to its answer: the reply is awaited, the answer
    /// shown (which schedules the view's next ask), the window woken
    /// through the change channel, and the refresh marked answered.
    fn run(self) {
        let result = futures::executor::block_on(self.event.reply);
        self.launcher
            .show_designed_answer(self.epoch, self.event.number, result);
        // The window redraws although no user action waited for the
        // answer, as it does for a service's or a schedule's change.
        self.launcher.changed();
        if let Some(refresh) = self.refresh.upgrade() {
            refresh.answered();
        }
    }
}

impl Drop for Refresh {
    fn drop(&mut self) {
        self.wake.stop();
    }
}

/// A closure that pokes the refresh thread `weak` names awake, holding it
/// weakly so that a clock does not keep it running past the launcher: for
/// [`Clock::on_change`], which asks for another look.
fn waking(weak: Weak<Refresh>) -> impl Fn() + Send + Sync + 'static {
    move || {
        if let Some(refresh) = weak.upgrade() {
            refresh.wake.poke();
        }
    }
}

/// The refresh thread: starts the refresh that is due, then waits until
/// one is, something changes, or this Pane stops.
fn refresh_until_stopped(launcher: WeakLauncher, refresh: Weak<Refresh>) {
    while let Some(current) = refresh.upgrade() {
        let Some(seen) = current.wake.pokes() else {
            return;
        };
        // The launcher being gone ends the thread through the wake: this
        // Pane re-renders no extension view from here on.
        let run = current.look(&launcher);
        current.wake.scanned(seen, usize::from(run.is_some()));
        if let Some(run) = run {
            start_run(run);
        }
        let wait = current.next_wait();
        if !current.wake.wait(seen, wait) {
            return;
        }
    }
}

/// Spawns the thread that runs `run` to its answer. The refresh thread
/// never waits for it: a slow render delays neither the window nor the
/// next look for due work (the runtime serves one guest call at a time,
/// and numbers the view's renders so a late answer never replaces a newer
/// tree). A thread that cannot start counts as answered, so the view's
/// next ask is not stuck behind it.
fn start_run(run: RefreshRun) {
    let refresh = run.refresh.clone();
    let started = std::thread::Builder::new()
        .name("pane-view-refresh-run".into())
        .spawn(move || run.run());
    if let Err(error) = started {
        crate::diagnostic!("Pane could not re-render an extension view: {error}");
        if let Some(refresh) = refresh.upgrade() {
            refresh.answered();
        }
    }
}
