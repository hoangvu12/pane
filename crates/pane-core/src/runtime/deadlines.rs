//! Keeping Pane's extension runtime responsive when a guest stops
//! cooperating (#18).
//!
//! The runtime thread serves one guest call at a time, so a call that never
//! finishes holds every other extension's calls behind it. Three limits
//! bound that, each for a different way of not finishing:
//!
//! - **A guest computing without waiting** (a busy loop). The engine counts
//!   epochs ([`TICK`] apart), and every store yields to the runtime thread
//!   at each one, so the thread keeps checking the call's generation and
//!   injected faults however busy the guest is. A call whose guest computed
//!   for [`COMPUTE_LIMIT`] in all without the call finishing is stopped as
//!   **unresponsive**: its instance is dropped (as a stopped call's is),
//!   and, since Wasmtime knows exactly which guest was executing, the
//!   failure is that package's own and counts towards pausing it, as a
//!   crash does. Time a guest spends waiting (on a clock, a helper, an
//!   operation) is not computing, so a slow call is never stopped for it.
//! - **A guest waiting on a native helper that does not exit.** The helper's
//!   supervising thread ends it after `helpers::runner::HELPER_TIME_LIMIT`,
//!   and the run answers an error the guest handles like any other helper
//!   failure: an expected slow-operation error, not a crash.
//! - **The runtime thread itself not responding**: it has not returned to
//!   its executor for [`UNRESPONSIVE_LIMIT`], so it is stuck in Pane's host
//!   code or in Wasmtime, not in a guest (which yields every tick). Which
//!   extension, if any, caused that is not known. A watchdog thread gives up
//!   on it, as the supervisor does on a crashed thread: no extension is
//!   named or paused, every call it held answers that the runtime stopped,
//!   its helpers are ended and a fresh thread serves calls (see
//!   `supervisor`). A thread cannot be ended from outside, so the stuck one
//!   is abandoned: if it ever returns, it runs nothing more (its guests'
//!   host calls are refused and it stops at its next check), and only then
//!   frees what it holds.
//!
//! These are explicit choices, not measurements (provisional).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// How often the engine's epoch advances, and so how often a computing
/// guest yields to the runtime thread.
pub(crate) const TICK: Duration = Duration::from_millis(10);

/// How long a guest call may compute, in all, without finishing: stopped
/// then as unresponsive. Generous for a launcher's calls, and short enough
/// that other extensions do not wait long behind a busy loop.
pub const COMPUTE_LIMIT: Duration = Duration::from_secs(5);

/// How long the runtime thread may go without returning to its executor
/// before Pane gives up on it. Compiling a component is not counted.
pub const UNRESPONSIVE_LIMIT: Duration = Duration::from_secs(10);

/// How often the watchdog looks at the runtime thread.
pub(super) const WATCH_EVERY: Duration = Duration::from_millis(100);

/// Advances `engine`'s epoch every [`TICK`] on a thread of its own, which
/// stops once the engine is gone.
pub(super) fn tick(engine: &wasmtime::Engine) {
    let engine = engine.weak();
    let _ = std::thread::Builder::new()
        .name("pane-runtime-epoch".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TICK);
                match engine.upgrade() {
                    Some(engine) => engine.increment_epoch(),
                    None => return,
                }
            }
        });
}

/// How long a guest call computed, from the time its polls took: the guest
/// yields at each epoch, so a poll that computes returns within a tick or
/// so, and a guest waiting on something is not polled at all.
#[derive(Default)]
pub(super) struct Meter {
    spent: Duration,
}

impl Meter {
    /// Polls with `poll`, counting the time it takes.
    pub(super) fn measure<T>(&mut self, poll: impl FnOnce() -> Poll<T>) -> Poll<T> {
        let started = Instant::now();
        let polled = poll();
        self.spent += started.elapsed();
        polled
    }

    /// Whether the call computed for [`COMPUTE_LIMIT`] or more.
    pub(super) fn exhausted(&self) -> bool {
        self.spent >= COMPUTE_LIMIT
    }
}

/// Why a guest call was stopped as unresponsive, for its error.
pub(super) fn computed_too_long() -> String {
    format!(
        "it computed for {} seconds without waiting for anything, so Pane stopped it; other \
         extensions' calls waited meanwhile",
        COMPUTE_LIMIT.as_secs()
    )
}

/// Runs `future`, guest code such as an instantiation or a destructor,
/// until it finishes or has computed for [`COMPUTE_LIMIT`].
pub(super) async fn metered<F: Future>(future: F) -> Result<F::Output, String> {
    let mut future = std::pin::pin!(future);
    let mut meter = Meter::default();
    std::future::poll_fn(|cx| match meter.measure(|| future.as_mut().poll(cx)) {
        Poll::Ready(output) => Poll::Ready(Ok(output)),
        Poll::Pending if meter.exhausted() => Poll::Ready(Err(computed_too_long())),
        Poll::Pending => Poll::Pending,
    })
    .await
}

/// What the watchdog knows of one runtime thread: whether it is inside a
/// poll of its work, and since when.
pub(super) struct Watch {
    origin: Instant,
    /// Milliseconds after `origin`, plus one, when the current poll began;
    /// 0 while the thread is not polling.
    polling_since: AtomicU64,
    /// How many exemptions (compiling a component) are in force.
    exempt: AtomicUsize,
    /// Set once Pane gave up on the thread: whatever it still runs does
    /// nothing more.
    given_up: AtomicBool,
    /// Set once the thread's end was handled: it stopped, crashed or was
    /// given up on, whichever came first.
    ended: AtomicBool,
    /// What the thread is doing ([`Doing`]), for diagnostics.
    doing: AtomicU8,
}

impl Default for Watch {
    fn default() -> Watch {
        Watch {
            origin: Instant::now(),
            polling_since: AtomicU64::new(0),
            exempt: AtomicUsize::new(0),
            given_up: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            doing: AtomicU8::new(Doing::Waiting as u8),
        }
    }
}

/// What a runtime thread is doing, for the diagnostics of one that stops
/// responding. It names no extension: which one, if any, caused a stuck
/// thread is not known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Doing {
    Waiting,
    Handling,
    Starting,
    Running,
}

impl Doing {
    fn from(value: u8) -> Doing {
        match value {
            1 => Doing::Handling,
            2 => Doing::Starting,
            3 => Doing::Running,
            _ => Doing::Waiting,
        }
    }

    /// "while running a guest call", for a sentence.
    pub(super) fn describe(self) -> &'static str {
        match self {
            Doing::Waiting => "while waiting for a request",
            Doing::Handling => "while handling a request",
            Doing::Starting => "while starting a guest instance",
            Doing::Running => "while running a guest call, between the guest's yields",
        }
    }
}

/// Puts back what the thread was doing before, when dropped.
pub(super) struct Done<'a> {
    watch: &'a Watch,
    before: u8,
}

impl Drop for Done<'_> {
    fn drop(&mut self) {
        self.watch.doing.store(self.before, Ordering::SeqCst);
    }
}

/// Keeps the watchdog from counting the time it lives, such as compiling a
/// component, which may take long without anything being stuck.
pub(super) struct Exempt<'a>(&'a Watch);

impl Drop for Exempt<'_> {
    fn drop(&mut self) {
        self.0.exempt.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Watch {
    fn now(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX - 1) + 1
    }

    /// Notes that the thread does `doing` until the guard is dropped.
    pub(super) fn doing(&self, doing: Doing) -> Done<'_> {
        Done {
            watch: self,
            before: self.doing.swap(doing as u8, Ordering::SeqCst),
        }
    }

    /// What the thread is doing.
    pub(super) fn what(&self) -> Doing {
        Doing::from(self.doing.load(Ordering::SeqCst))
    }

    /// Exempts what the thread does while the guard lives.
    pub(super) fn exempt(&self) -> Exempt<'_> {
        self.exempt.fetch_add(1, Ordering::SeqCst);
        Exempt(self)
    }

    /// How long the thread has been inside one poll, unless it is exempt.
    pub(super) fn stuck_for(&self) -> Option<Duration> {
        let since = self.polling_since.load(Ordering::SeqCst);
        if since == 0 || self.exempt.load(Ordering::SeqCst) > 0 {
            return None;
        }
        Some(Duration::from_millis(self.now().saturating_sub(since)))
    }

    /// Whether Pane gave up on the thread.
    pub(super) fn given_up(&self) -> bool {
        self.given_up.load(Ordering::SeqCst)
    }

    /// Gives up on the thread, unless its end was handled already; returns
    /// whether this did.
    pub(super) fn give_up(&self) -> bool {
        if self.ended.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.given_up.store(true, Ordering::SeqCst);
        true
    }

    /// Notes that the thread ended (it stopped or crashed); returns whether
    /// its end is this one to handle, rather than Pane having given up on
    /// it before.
    pub(super) fn end(&self) -> bool {
        !self.ended.swap(true, Ordering::SeqCst)
    }

    /// Whether the thread's end was handled.
    pub(super) fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
}

/// The runtime thread's work, noting in its [`Watch`] when each poll begins
/// and ends. Once Pane gave up on the thread, it ends at its next poll.
pub(super) struct Watched<'a, F> {
    pub(super) watch: &'a Watch,
    pub(super) work: Pin<Box<F>>,
}

impl<F: Future<Output = ()>> Future for Watched<'_, F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.watch.given_up() {
            return Poll::Ready(());
        }
        self.watch
            .polling_since
            .store(self.watch.now(), Ordering::SeqCst);
        let polled = self.work.as_mut().poll(cx);
        self.watch.polling_since.store(0, Ordering::SeqCst);
        if self.watch.given_up() {
            return Poll::Ready(());
        }
        polled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_inside_a_poll_is_seen_stuck_unless_exempt() {
        let watch = Watch::default();
        assert_eq!(watch.stuck_for(), None, "not polling");
        watch.polling_since.store(watch.now(), Ordering::SeqCst);
        assert!(watch.stuck_for().is_some());
        {
            let _compiling = watch.exempt();
            assert_eq!(watch.stuck_for(), None);
        }
        assert!(watch.stuck_for().is_some());
    }

    #[test]
    fn a_thread_given_up_on_is_not_handled_again_when_it_ends() {
        let watch = Watch::default();
        assert!(watch.give_up());
        assert!(watch.given_up());
        assert!(!watch.end(), "its end was handled when Pane gave up");
        assert!(!watch.give_up());

        let crashed = Watch::default();
        assert!(crashed.end());
        assert!(!crashed.give_up(), "a crash was handled first");
        assert!(!crashed.given_up());
    }

    #[test]
    fn a_meter_counts_the_time_of_its_polls() {
        let mut meter = Meter::default();
        let polled = meter.measure(|| {
            std::thread::sleep(Duration::from_millis(20));
            Poll::<()>::Pending
        });
        assert!(polled.is_pending());
        assert!(meter.spent >= Duration::from_millis(20));
        assert!(!meter.exhausted());
        meter.spent = COMPUTE_LIMIT;
        assert!(meter.exhausted());
    }
}
