//! Keeping Pane usable when its extension runtime thread crashes (#17).
//!
//! The runtime thread serves every guest call. A panic on it (a fault in
//! Pane's host code or in Wasmtime, not a guest trap, which is a crash of
//! one package) unwinds it: its instances, views and pending calls are
//! dropped, so every call that was running or queued answers
//! [`CallError::RuntimeUnavailable`] and none is sent again. The thread
//! catches its own unwinding, ends the native helpers still running (they
//! were all started by its guests), and then:
//!
//! - starts a fresh runtime thread, so the next call the user asks for runs
//!   there; or,
//! - when it crashed within [`RESTART_WINDOW`] of the previous crash, starts
//!   none: repeated automatic restarts are suppressed, and the runtime stays
//!   stopped until the user restarts it ([`Runtime::restart`]).
//!
//! Either way the launcher is told ([`CrashReport`]), without naming any
//! package: which one, if any, caused a crash of the shared thread is not
//! known, so nothing is paused. Extension data is untouched.

use std::any::Any;
use std::future::Future as _;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::sync::{Notify, mpsc, watch};

use super::{CallError, Code, HealthReport, Host, Request, SharedApplications, SharedDirectory};
use crate::helpers::runner::Helpers;

/// How soon after a crash a second one stops the runtime instead of
/// restarting it: an explicit choice (provisional), the same window as for
/// pausing a package.
pub(crate) const RESTART_WINDOW: Duration = Duration::from_secs(5 * 60);

/// What the runtime is doing, as far as crashes of its thread go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStatus {
    /// It runs, and has not crashed since it started or was last restarted
    /// by the user.
    Running,
    /// It crashed, and Pane started it again by itself. `why` is what its
    /// thread reported as it stopped.
    Restarted { why: String },
    /// It crashed and was not started again: it runs nothing until the user
    /// restarts it. `not_restarted` says why Pane did not.
    Stopped { why: String, not_restarted: String },
}

/// Told on the crashed runtime thread, once its helpers were ended and it
/// was restarted or not, with what the runtime does now.
pub(crate) type CrashReport = Arc<dyn Fn(&RuntimeStatus) + Send + Sync>;

/// A fault Pane injects into its own runtime to check that it recovers,
/// for tests and the native smokes (`PANE_TEST_RUNTIME_FAULTS`). Nothing an
/// extension can cause.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The runtime thread panics at once, wherever it is: waiting for the
    /// next request, or for a guest call it runs (on a clock, a helper, an
    /// operation).
    Crash,
    /// The runtime thread panics once the next guest call has returned,
    /// before its answer is sent: what the call did (saving data, say) is
    /// done, but its answer is lost.
    CrashBeforeAnswer,
}

/// The faults injected into one runtime thread.
#[derive(Default)]
pub(super) struct Faults {
    crash: AtomicBool,
    crash_before_answer: AtomicBool,
    /// Wakes the thread where it waits, for [`Fault::Crash`].
    woken: Notify,
}

impl Faults {
    /// Panics if [`Fault::Crash`] was injected. `waiting` is polled first,
    /// so an injection after this check wakes the task that polled it.
    pub(super) fn check(
        &self,
        waiting: std::pin::Pin<&mut tokio::sync::futures::Notified<'_>>,
        cx: &mut Context<'_>,
    ) {
        let _: Poll<()> = waiting.poll(cx);
        if self.crash.load(Ordering::SeqCst) {
            panic!(
                "Pane's extension runtime was made to crash (a fault injected to check recovery)"
            );
        }
    }

    pub(super) fn waiting(&self) -> tokio::sync::futures::Notified<'_> {
        self.woken.notified()
    }

    /// Panics if [`Fault::CrashBeforeAnswer`] was injected: called once a
    /// guest call has returned, before its answer is sent.
    pub(super) fn before_answer(&self) {
        if self.crash_before_answer.swap(false, Ordering::SeqCst) {
            panic!(
                "Pane's extension runtime was made to crash before answering (a fault injected \
                 to check recovery)"
            );
        }
    }
}

/// What every [`super::Runtime`] handle shares: the runtime thread now
/// serving calls, if one is, and what a new one needs.
pub(super) struct Shared {
    /// The native helper processes guests started, which end once every
    /// handle is dropped.
    pub(super) helpers: Helpers,
    pub(super) applications: SharedApplications,
    pub(super) directory: SharedDirectory,
    pub(super) health: Arc<Mutex<Option<HealthReport>>>,
    /// Custom view ids, never reused, even by a restarted thread: a view
    /// the window still shows from a crashed one must not name a new view.
    pub(super) next_view: Arc<AtomicU64>,
    crashes: Mutex<Option<CrashReport>>,
    /// Counts the crashed threads Pane is done with (restarted or not, the
    /// launcher told), for a call whose answer a crash lost.
    handled: watch::Sender<u64>,
    cache_dir: Option<PathBuf>,
    current: Mutex<Current>,
}

/// Why a request was not sent to the runtime thread.
pub(super) enum NotSent {
    /// The runtime is stopped after crashing.
    Stopped,
    /// The thread has just crashed; Pane is handling it.
    Lost,
}

impl NotSent {
    /// How a request that answers nothing says it was not sent.
    pub(super) fn error(self) -> CallError {
        match self {
            NotSent::Stopped => stopped(),
            NotSent::Lost => lost_in(&RuntimeStatus::Running),
        }
    }
}

/// Counts a crashed thread as handled when it ends, however its handling
/// ends.
struct Handled(Weak<Shared>);

impl Drop for Handled {
    fn drop(&mut self) {
        if let Some(shared) = self.0.upgrade() {
            shared.handled.send_modify(|handled| *handled += 1);
        }
    }
}

impl Drop for Shared {
    /// The last handle is gone (Pane is quitting): so are the helpers,
    /// which would otherwise outlive it. The thread stops with its requests.
    fn drop(&mut self) {
        self.helpers.stop_all();
    }
}

struct Current {
    /// The thread serving calls; `None` while the runtime is stopped.
    thread: Option<Thread>,
    /// Counts the threads started, to tell a report of an old one.
    started: u64,
    /// When the runtime last crashed, since the user last restarted it.
    last_crash: Option<Instant>,
    status: RuntimeStatus,
}

/// A runtime thread's end of its requests.
#[derive(Clone)]
struct Thread {
    requests: mpsc::UnboundedSender<Request>,
    faults: Arc<Faults>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shared {
    /// Starts the first runtime thread with `code`.
    pub(super) fn start(
        code: Arc<Code>,
        applications: SharedApplications,
        helpers: Helpers,
        cache_dir: Option<PathBuf>,
    ) -> Result<Arc<Shared>, CallError> {
        let shared = Arc::new(Shared {
            helpers,
            applications,
            directory: SharedDirectory::default(),
            health: Arc::default(),
            next_view: Arc::default(),
            crashes: Mutex::new(None),
            handled: watch::Sender::new(0),
            cache_dir,
            current: Mutex::new(Current {
                thread: None,
                started: 0,
                last_crash: None,
                status: RuntimeStatus::Running,
            }),
        });
        {
            let mut current = lock(&shared.current);
            let thread = shared.spawn(code, 1)?;
            current.thread = Some(thread);
            current.started = 1;
        }
        Ok(shared)
    }

    /// Sends `request` to the runtime thread.
    pub(super) fn send(&self, request: Request) -> Result<(), NotSent> {
        let requests = match &lock(&self.current).thread {
            Some(thread) => thread.requests.clone(),
            None => return Err(NotSent::Stopped),
        };
        // A thread that just crashed has dropped its requests: this one is
        // not sent, nor sent again to the next thread.
        requests.send(request).map_err(|_| NotSent::Lost)
    }

    /// Resolves, through [`lost`], once a thread that crashes after this
    /// call has been handled.
    pub(super) fn handled(&self) -> watch::Receiver<u64> {
        self.handled.subscribe()
    }

    pub(super) fn status(&self) -> RuntimeStatus {
        lock(&self.current).status.clone()
    }

    pub(super) fn set_crash_report(&self, report: CrashReport) {
        *lock(&self.crashes) = Some(report);
    }

    pub(super) fn inject(&self, fault: Fault) {
        let Some(thread) = lock(&self.current).thread.clone() else {
            return;
        };
        match fault {
            Fault::Crash => {
                thread.faults.crash.store(true, Ordering::SeqCst);
                thread.faults.woken.notify_waiters();
            }
            Fault::CrashBeforeAnswer => {
                thread
                    .faults
                    .crash_before_answer
                    .store(true, Ordering::SeqCst);
            }
        }
    }

    /// Starts the runtime again after it stopped, at the user's request:
    /// crashes before this no longer count against restarting it by
    /// itself. Nothing that ran before is run again. A runtime that runs is
    /// left as it is.
    pub(super) fn restart(self: &Arc<Shared>) -> Result<(), CallError> {
        let mut current = lock(&self.current);
        if current.thread.is_some() {
            return Ok(());
        }
        if self.helpers.quitting() {
            return Err(CallError::RuntimeUnavailable("Pane is quitting".into()));
        }
        let number = current.started + 1;
        let thread = self.spawn(Arc::new(Code::new(self.engine()?)), number)?;
        current.thread = Some(thread);
        current.started = number;
        current.last_crash = None;
        current.status = RuntimeStatus::Running;
        Ok(())
    }

    fn engine(&self) -> Result<wasmtime::Engine, CallError> {
        super::engine(self.cache_dir.clone())
    }

    /// Starts runtime thread number `number`, serving calls with `code`.
    fn spawn(self: &Arc<Shared>, code: Arc<Code>, number: u64) -> Result<Thread, CallError> {
        let unavailable =
            |error: &dyn std::fmt::Display| CallError::RuntimeUnavailable(error.to_string());
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| unavailable(&error))?;
        let (requests, receiver) = mpsc::unbounded_channel();
        let faults = Arc::new(Faults::default());
        let host = Host::new(code, self, faults.clone());
        let shared = Arc::downgrade(self);
        std::thread::Builder::new()
            .name("pane-extension-runtime".into())
            .spawn(move || {
                // Dropped last, once the crash (if any) was handled.
                let _handled = Handled(shared.clone());
                let served = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    executor.block_on(host.serve(receiver))
                }));
                drop(executor);
                if let Err(panic) = served {
                    crashed(&shared, number, panic_message(&*panic));
                }
            })
            .map_err(|error| unavailable(&error))?;
        Ok(Thread { requests, faults })
    }
}

/// Runtime thread `number` crashed with `why`, and its unwinding dropped
/// everything it held: ends the helpers its guests left running, restarts
/// it unless it crashed within [`RESTART_WINDOW`] of its previous crash,
/// and tells the launcher.
fn crashed(shared: &Weak<Shared>, number: u64, why: String) {
    let Some(shared) = shared.upgrade() else {
        // Pane is quitting.
        return;
    };
    // Every helper still running was started by the crashed thread's
    // guests: no other thread runs until this one has been handled.
    shared.helpers.stop_running();
    let status = {
        let mut current = lock(&shared.current);
        if current.started != number {
            return;
        }
        current.thread = None;
        let now = Instant::now();
        let again = restarts_automatically(current.last_crash, now);
        current.last_crash = Some(now);
        let restarted = if shared.helpers.quitting() {
            Err("Pane is quitting".to_owned())
        } else if !again {
            Err(format!(
                "it crashed twice within {} minutes; repeated automatic restarts are suppressed",
                RESTART_WINDOW.as_secs() / 60
            ))
        } else {
            let next = number + 1;
            shared
                .engine()
                .and_then(|engine| shared.spawn(Arc::new(Code::new(engine)), next))
                .map(|thread| {
                    current.thread = Some(thread);
                    current.started = next;
                })
                .map_err(|error| format!("starting it again failed: {error}"))
        };
        current.status = match restarted {
            Ok(()) => RuntimeStatus::Restarted { why },
            Err(not_restarted) => RuntimeStatus::Stopped { why, not_restarted },
        };
        current.status.clone()
    };
    eprintln!("Pane's extension runtime stopped unexpectedly: {status:?}");
    let report = lock(&shared.crashes).clone();
    if let Some(report) = report {
        report(&status);
    }
}

/// Whether a crash at `now` restarts the runtime by itself: not when it
/// already crashed within [`RESTART_WINDOW`] before (`previous`, since the
/// user last restarted it).
pub(crate) fn restarts_automatically(previous: Option<Instant>, now: Instant) -> bool {
    previous.is_none_or(|previous| now.saturating_duration_since(previous) >= RESTART_WINDOW)
}

/// The message of a panic, for diagnostics.
fn panic_message(panic: &(dyn Any + Send)) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else {
        "the runtime thread panicked".to_owned()
    }
}

/// How a call answers when the runtime is stopped: it is not sent.
pub(super) fn stopped() -> CallError {
    CallError::RuntimeUnavailable(
        "it stopped after crashing and runs nothing until you restart it in Manage extensions"
            .into(),
    )
}

/// How a call answers when the runtime thread crashed before answering
/// it, once Pane has handled the crash (`handled` changed): whether it
/// restarted the runtime. It is not sent again.
pub(super) async fn lost(shared: Weak<Shared>, mut handled: watch::Receiver<u64>) -> CallError {
    // The thread counts itself as handled however its handling ends; the
    // count's sender goes only with the last runtime handle.
    let _ = handled.changed().await;
    let status = match shared.upgrade() {
        Some(shared) => shared.status(),
        None => RuntimeStatus::Running,
    };
    lost_in(&status)
}

/// How a call whose answer was lost answers, while the runtime does
/// `status`.
fn lost_in(status: &RuntimeStatus) -> CallError {
    CallError::RuntimeUnavailable(match status {
        RuntimeStatus::Restarted { .. } => "it stopped before answering and was started again; \
                                            Pane does not run this again by itself"
            .into(),
        RuntimeStatus::Stopped { not_restarted, .. } => format!(
            "it stopped before answering and was not restarted ({not_restarted}); Pane does not \
             run this again by itself. Restart it in Manage extensions"
        ),
        RuntimeStatus::Running => {
            "it stopped before answering; Pane does not run this again by itself".into()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_crash_restarts_the_runtime() {
        assert!(restarts_automatically(None, Instant::now()));
    }

    #[test]
    fn a_second_crash_within_the_window_does_not() {
        let first = Instant::now();
        let soon = first + RESTART_WINDOW - Duration::from_secs(1);
        assert!(!restarts_automatically(Some(first), soon));
    }

    #[test]
    fn a_crash_after_the_window_restarts_it_again() {
        let first = Instant::now();
        assert!(restarts_automatically(Some(first), first + RESTART_WINDOW));
    }

    #[test]
    fn a_panic_message_is_kept() {
        let panic = std::panic::catch_unwind(|| panic!("broke {}", 1)).unwrap_err();
        assert_eq!(panic_message(&*panic), "broke 1");
        let panic = std::panic::catch_unwind(|| panic!("static")).unwrap_err();
        assert_eq!(panic_message(&*panic), "static");
    }
}
