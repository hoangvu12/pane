//! Faults Pane injects into its own runtime thread to check that it
//! recovers from a crash (#17), for tests and the native smokes, which run
//! debug builds. A release build has none of it: [`Faults`] is then empty
//! and its checks do nothing.

use std::future::Future;
use std::pin::Pin;
use std::task::Context;

#[cfg(any(test, debug_assertions))]
use std::sync::Mutex;
#[cfg(any(test, debug_assertions))]
use std::sync::atomic::{AtomicBool, Ordering};

/// A fault Pane injects into its own runtime to check that it recovers.
/// Nothing an extension can cause. Debug builds only.
#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The runtime thread panics at once, wherever it is: waiting for the
    /// next request, or for a guest call it runs (on a clock, a helper, an
    /// operation).
    Crash,
    /// The runtime thread panics once the action `item` (of any command)
    /// has run, before its answer is sent: what the action did (saving
    /// data, say) is done, but its answer is lost. Other calls, such as
    /// root search's, answer as usual meanwhile.
    CrashBeforeAnswer { item: String },
}

/// The faults injected into one runtime thread.
#[derive(Default)]
pub(super) struct Faults {
    #[cfg(any(test, debug_assertions))]
    crash: AtomicBool,
    #[cfg(any(test, debug_assertions))]
    crash_before_answer: Mutex<Option<String>>,
    /// Wakes the thread where it waits, for [`Fault::Crash`].
    #[cfg(any(test, debug_assertions))]
    woken: tokio::sync::Notify,
}

impl Faults {
    #[cfg(any(test, debug_assertions))]
    pub(super) fn inject(&self, fault: Fault) {
        match fault {
            Fault::Crash => {
                self.crash.store(true, Ordering::SeqCst);
                self.woken.notify_waiters();
            }
            Fault::CrashBeforeAnswer { item } => {
                *super::lock(&self.crash_before_answer) = Some(item);
            }
        }
    }

    /// What [`Faults::check`] polls to be woken by an injection.
    #[cfg(any(test, debug_assertions))]
    pub(super) fn waiting(&self) -> impl Future<Output = ()> + '_ {
        self.woken.notified()
    }

    #[cfg(not(any(test, debug_assertions)))]
    pub(super) fn waiting(&self) -> impl Future<Output = ()> + '_ {
        std::future::pending()
    }

    /// Panics if [`Fault::Crash`] was injected. `waiting` is polled first,
    /// so an injection after this check wakes the task that polled it.
    #[cfg_attr(not(any(test, debug_assertions)), allow(unused_variables))]
    pub(super) fn check(&self, waiting: Pin<&mut impl Future<Output = ()>>, cx: &mut Context<'_>) {
        #[cfg(any(test, debug_assertions))]
        {
            let _ = waiting.poll(cx);
            if self.crash.load(Ordering::SeqCst) {
                panic!(
                    "Pane's extension runtime was made to crash (a fault injected to check \
                     recovery)"
                );
            }
        }
    }

    /// Panics if [`Fault::CrashBeforeAnswer`] was injected for the action
    /// `item`: called once it has run, before its answer is sent.
    #[cfg_attr(not(any(test, debug_assertions)), allow(unused_variables))]
    pub(super) fn before_answer(&self, item: &str) {
        #[cfg(any(test, debug_assertions))]
        {
            let mut target = super::lock(&self.crash_before_answer);
            if target.as_deref() == Some(item) {
                *target = None;
                drop(target);
                panic!(
                    "Pane's extension runtime was made to crash before answering `{item}` (a \
                     fault injected to check recovery)"
                );
            }
        }
    }
}
