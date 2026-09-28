//! Pausing an installed package that keeps failing (#16), so one broken
//! extension does not fail again and again while the rest of Pane runs.
//!
//! Pane pauses a package when a failure is attributable to it: its code ran
//! in this package's current generation and failed on its own.
//!
//! - **It could not start**: a component could not be loaded or
//!   instantiated, or its reloaded code trapped as it started (a startup
//!   failure). Paused at once: starting it again would fail the same way.
//! - **It crashed [`CRASHES_BEFORE_PAUSE`] times within
//!   [`CRASH_WINDOW`]**: a guest call trapped, whether the user ran it
//!   (opening a command, an action, a form, a view event), root search asked
//!   it for results, or another package called its operation. Calls that
//!   answer in between do not start the count again (opening a command
//!   before each crashing action answers); crashes further apart than the
//!   window, and those before Pane started or the package's generation
//!   began, are not counted together.
//!
//! What is not a failure of the package: an error the extension answers
//! with ("sign in first"), which is an ordinary outcome; and a call stopped
//! because its generation, or a caller's in its chain, ended (disable,
//! reload, update, uninstall), even though its instance restarts afresh
//! afterwards as after a crash. A crash of a package serving an operation is
//! its own, not its caller's. An unattributed failure of the runtime itself
//! (#17) pauses nothing.
//!
//! A paused package's generation ends, which stops its pending calls and
//! drops its instances; its commands stay listed in root search, saying why
//! they do not run, and it computes no root results and serves no
//! operations. Its settings and saved data are kept. The pause is recorded
//! with the package's version and managed copy, so it holds after a restart
//! for that code. It ends with Retry (which starts the same code again),
//! a reload or an update (new code), disabling the package (enabled again,
//! it starts afresh) or uninstalling it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{Launcher, State, Status, off_thread, owner};
use crate::extension_data::PackageData;
use crate::packages::{PackageIdentity, Pause, PauseCause};
use crate::runtime::Health;

/// How many crashes within [`CRASH_WINDOW`] pause a package. Small, so
/// that a broken command stops failing soon, and more than one, so that one
/// bad input does not stop an extension that otherwise works.
pub(super) const CRASHES_BEFORE_PAUSE: usize = 3;

/// How close together crashes count towards pausing a package: long enough
/// to catch a user trying a broken command again, or root search asking a
/// broken provider on each key, and short enough that rare crashes of a
/// long-running Pane never add up to a pause.
pub(super) const CRASH_WINDOW: Duration = Duration::from_secs(5 * 60);

/// The installed packages Pane paused, each with why, and when each
/// package's current generation last crashed, within [`CRASH_WINDOW`].
#[derive(Default)]
pub(super) struct Failures {
    paused: HashMap<PackageIdentity, Pause>,
    crashes: HashMap<PackageIdentity, Vec<Instant>>,
}

impl Failures {
    /// Why the package with `identity` is paused, if it is.
    pub(super) fn of(&self, identity: &PackageIdentity) -> Option<&Pause> {
        self.paused.get(identity)
    }

    pub(super) fn is_paused(&self, identity: &PackageIdentity) -> bool {
        self.paused.contains_key(identity)
    }

    /// Forgets the pause and crashes of the package with `identity`: it runs
    /// in a new generation, or not at all.
    pub(super) fn forget(&mut self, identity: &PackageIdentity) {
        self.paused.remove(identity);
        self.crashes.remove(identity);
    }

    /// Notes the pause of the package with `identity` recorded before Pane
    /// last stopped.
    pub(super) fn restore(&mut self, identity: PackageIdentity, pause: Pause) {
        self.paused.insert(identity, pause);
    }

    fn pause(&mut self, identity: PackageIdentity, pause: Pause) {
        self.crashes.remove(&identity);
        self.paused.insert(identity, pause);
    }
}

impl Launcher {
    /// Notes how a call into `component`, made with `data`, failed, pausing
    /// its package if the failure is its [`CRASHES_BEFORE_PAUSE`]th crash
    /// within [`CRASH_WINDOW`] or a failure to start (see the module
    /// documentation). Called on
    /// the runtime thread before the call's answer is sent, so whoever
    /// awaits the answer sees the pause.
    pub(super) fn note_health(&self, component: &Path, data: &PackageData, health: Health) {
        // A call of an ended generation is not the package's current code.
        if data.stopped().is_some() {
            return;
        }
        let mut state = self.lock();
        let Some(package) = owner(&state.packages, component) else {
            return;
        };
        let identity = package.identity.clone();
        let version = package.version();
        let title = package.title();
        let pause = match health {
            Health::Crashed(error) => {
                let now = Instant::now();
                let crashes = state.failed.crashes.entry(identity.clone()).or_default();
                crashes.retain(|crash| now.duration_since(*crash) < CRASH_WINDOW);
                crashes.push(now);
                if crashes.len() < CRASHES_BEFORE_PAUSE {
                    return;
                }
                Pause {
                    after: PauseCause::Crashes,
                    why: format!("Crashed {}; the last time: {error}", within()),
                    version,
                }
            }
            Health::FailedToStart(error) => Pause {
                after: PauseCause::FailedToStart,
                why: error.to_string(),
                version,
            },
        };
        let what = match pause.after {
            PauseCause::Crashes => format!("{title} crashed {}", within()),
            PauseCause::FailedToStart => format!("{title} could not start"),
        };
        self.pause(&mut state, &identity, pause);
        state.view.status = Status::Error(format!(
            "{what} and is paused: Pane runs none of its code until you retry it, and keeps its \
             saved data. Retry and the details are under \"Retry starting {title}\" in Manage \
             extensions."
        ));
        drop(state);
        self.record_pause(&identity);
    }

    /// Pauses the package with `identity` for `pause`, without recording
    /// it: its generation ends, which stops its calls and drops its
    /// instances, and a command of it that is open closes.
    pub(super) fn pause(&self, state: &mut State, identity: &PackageIdentity, pause: Pause) {
        state.failed.pause(identity.clone(), pause);
        if let Some(installation) = &self.installation {
            installation.data.pause(identity);
        }
        // Its results kept for root search go.
        Launcher::forget_indexes(state);
        let components: Vec<PathBuf> = state
            .package(identity)
            .map(|package| {
                package
                    .commands()
                    .into_iter()
                    .map(|command| command.component)
                    .collect()
            })
            .unwrap_or_default();
        if state
            .open
            .as_ref()
            .is_some_and(|open| components.contains(open))
        {
            self.show_root(state, None);
        } else {
            self.refresh(state);
        }
    }

    /// Runs the package with `identity` again, which Pane paused, without
    /// recording it: a new generation, with no failures counted.
    pub(super) fn unpause(&self, state: &mut State, identity: &PackageIdentity) {
        state.failed.forget(identity);
        if let Some(installation) = &self.installation {
            installation.data.resume(identity);
        }
    }

    /// Records whether the package with `identity` is paused now, so a
    /// restart finds it so. The package's latest state is recorded, whatever
    /// order the recordings run in. A failure to record is logged: the pause
    /// still holds until Pane stops.
    pub(super) fn record_pause(&self, identity: &PackageIdentity) {
        let Some(installation) = &self.installation else {
            return;
        };
        // The store before the state: nothing holds the state while it
        // waits for the store.
        let mut store = installation.store.lock().unwrap_or_else(|p| p.into_inner());
        let pause = self.lock().failed.of(identity).cloned();
        if let Err(error) = store.set_paused(identity, pause) {
            eprintln!("pane: could not record whether {identity} is paused: {error}");
        }
    }

    /// Like [`Launcher::record_pause`], off the calling thread.
    pub(super) async fn record_pause_off_thread(&self, identity: &PackageIdentity) {
        let launcher = self.clone();
        let identity = identity.clone();
        off_thread(move || launcher.record_pause(&identity)).await;
    }
}

/// "3 times within 5 minutes": how often a package crashes before Pane
/// pauses it.
fn within() -> String {
    format!(
        "{CRASHES_BEFORE_PAUSE} times within {} minutes",
        CRASH_WINDOW.as_secs() / 60
    )
}
