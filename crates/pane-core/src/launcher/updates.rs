//! Automatic updates of installed packages from npm: Pane replaces the
//! managed copy of an eligible one with a compatible newer version from
//! its registry, without the user asking (US72–US75, T15).
//!
//! The updater is a thread of Pane's own, shaped like the scheduler's and
//! the services thread's, driven by the launcher's clock: it checks
//! shortly after Pane starts and then every
//! [`CHECK_EVERY`](CHECK_EVERY) hours, in the background, so the window
//! never waits for the registry. A check reads only the registry's
//! metadata; nothing is downloaded while the latest version is the
//! installed one. A newer version is fetched and checked exactly as an
//! install checks a package (its manifest and API, its components, its
//! platforms and helpers, and its dependencies as a plan), and what was
//! downloaded stays staged until it is applied.
//!
//! Applying a staged update waits for the safe activation boundary: no
//! screen of the package is on display (a command, its search, a form or
//! a custom view — a call the user is waiting on runs with one open), and
//! no call of it the user asked for is running (see
//! [`Runtime::busy`](crate::Runtime::busy)). The user is never
//! interrupted mid-command; the update is tried again every
//! [`RETRY_EVERY`](RETRY_EVERY) seconds until the package is quiet.
//! Managed background work (a scheduled run, a service cycle) is not
//! waited for: a replacement ends it with the package's generation and
//! the new code starts it again, exactly as a reload does. Applying the
//! update is an update as the preview's Update row is: the identity, the
//! saved data, the disabled state, the hotkeys and the aliases are kept,
//! and the old code's generation ends, stopping what is still pending
//! with its late answer discarded.
//!
//! Which packages are eligible: installed from npm, not pinned (a pinned
//! version is kept, whatever the latest is), enabled, not paused after a
//! failure, and not turned off — the user's controls are a global choice
//! and a per-package one (rows in the extension list, recorded in
//! `updates.json` beside `installed.json`). A local folder's or a
//! development copy's code is never replaced here: only npm packages
//! update by themselves. A check or an update that fails explains in the
//! status line, and the installed copy is left as it is.
//!
//! **Provisional, pending the user's decision:** the cadence (a check
//! shortly after Pane starts, then every 24 hours, by the launcher's
//! clock), the wait before the first check (a second, so that the
//! registry a development build is given is in place first), the retry
//! every second while a package is in use, and that a disabled or paused
//! package is not updated (updating a paused one would unpause it, which
//! is the user's choice to make).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use super::install::{self, Stopped};
use super::{
    Changing, Entry, Launcher, Mode, PackageIdentity, Row, Screen, State, Status, WeakLauncher,
    off_thread,
};
use crate::atomic::{Readers, write_atomically};
use crate::clipboard::Clock;
use crate::dependencies;
use crate::npm;
use crate::packages::InstalledPackage;

/// How long Pane waits after it starts before the first check, so that a
/// development build's registry ([`crate::npm::Registry::local`]) is in
/// place first: the launcher is configured within microseconds, while
/// this leaves a wide margin. Provisional.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(1);

/// How often Pane checks for a newer version of each eligible package:
/// the launcher's clock tells when. Provisional, like scheduled work's
/// intervals, pending the user's decision on update timing.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);

/// How often a staged update is tried again while the package is in use.
/// Provisional; a look is a lock and a comparison, so this is cheap.
const RETRY_EVERY: Duration = Duration::from_secs(1);

/// The longest the updater waits before it looks again, so that a change
/// of the system's time, or a computer waking from sleep, delays a check
/// by at most this much. As the scheduler's and the services thread's.
const MAX_WAIT: Duration = Duration::from_secs(3600);

/// The record of the user's automatic-update choices, `updates.json`
/// beside `installed.json`: whether every eligible package updates in the
/// background, and which packages the user turned it off for. Absent
/// fields mean the defaults: on, and none turned off — as no record at
/// all does, or one this Pane cannot read (the next choice the user makes
/// writes it anew).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UpdateControls {
    /// Whether eligible packages update automatically. On unless the user
    /// turned it off.
    pub(super) automatic: bool,
    /// The identity keys of the packages the user turned updates off for.
    pub(super) off: HashSet<String>,
}

impl Default for UpdateControls {
    fn default() -> UpdateControls {
        UpdateControls {
            automatic: true,
            off: HashSet::new(),
        }
    }
}

impl UpdateControls {
    /// Reads the controls recorded in `dir`; `None` when there is no
    /// record, or one this Pane cannot read (which it never replaces).
    pub(super) fn open(dir: &std::path::Path) -> Option<UpdateControls> {
        let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
        let read: Result<Recorded, _> = serde_json::from_str(&text);
        let read = read.ok()?;
        (read.version == 1).then(|| UpdateControls {
            automatic: read.automatic,
            off: read.off.unwrap_or_default().into_iter().collect(),
        })
    }

    /// The record's text for these controls, and where it goes.
    fn text(&self, dir: &std::path::Path) -> (PathBuf, String) {
        let recorded = Recorded {
            version: 1,
            automatic: self.automatic,
            off: (!self.off.is_empty()).then(|| self.off.iter().cloned().collect()),
        };
        (
            dir.join(FILE),
            serde_json::to_string_pretty(&recorded).unwrap_or_default(),
        )
    }
}

/// The file the controls are recorded in, beside `installed.json`.
const FILE: &str = "updates.json";

/// The controls as `updates.json` records them. `automatic` is written
/// only when off and `off` only when any is, so the file stays small and
/// a record from before a field existed still reads.
#[derive(serde::Serialize, serde::Deserialize)]
struct Recorded {
    version: u64,
    #[serde(default = "default_true", skip_serializing_if = "std::ops::Not::not")]
    automatic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    off: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

/// Runs the automatic updates of the launcher's installed npm packages.
/// Kept by the launcher; dropping it stops the thread.
pub(super) struct Updates {
    /// Where packages are read from: the registry a development build is
    /// given replaces the public one here too, once
    /// [`Launcher::with_npm_registry`] has run.
    sources: Mutex<install::Sources>,
    /// What the updater knows: the clock it follows, when it next checks,
    /// and what it has staged to apply.
    state: Mutex<Checking>,
    /// Wakes the updater thread, and tells when it settled.
    settled: Settled,
}

/// What the updater keeps: the clock it follows, when it next checks, and
/// one staged update per package.
struct Checking {
    clock: Arc<dyn Clock>,
    /// When the next check is, in clock milliseconds.
    next_check: u64,
    /// The updates Pane has downloaded and checked and not applied yet,
    /// each holding its download alive until then.
    staged: Vec<Staged>,
}

/// A new version of one package, downloaded and checked, waiting for the
/// package to be quiet so that it can replace the installed copy.
struct Staged {
    identity: PackageIdentity,
    /// The version the installed copy had when this was staged: an
    /// installed copy at another version since means this is not for it.
    installed: String,
    /// The package to install, at the version it would be.
    package: crate::packages::SourcePackage,
    /// What installing it means for its dependencies, as checked.
    plan: dependencies::Plan,
}

impl Updates {
    /// The updater that reads packages from `sources`, checking
    /// [`FIRST_CHECK_AFTER`] after it starts and then every
    /// [`CHECK_EVERY`]. [`Updates::run`] starts its thread.
    pub(super) fn start(sources: install::Sources) -> Arc<Updates> {
        let clock: Arc<dyn Clock> = Arc::new(crate::clipboard::SystemClock);
        let next_check = clock.now() + FIRST_CHECK_AFTER.as_millis() as u64;
        Arc::new(Updates {
            sources: Mutex::new(sources),
            state: Mutex::new(Checking {
                clock,
                next_check,
                staged: Vec::new(),
            }),
            settled: Settled::default(),
        })
    }

    /// Starts the updater's thread, which runs until this launcher stops.
    pub(super) fn run(self: &Arc<Self>, launcher: WeakLauncher) {
        let updates = Arc::downgrade(self);
        let started = std::thread::Builder::new()
            .name("pane-updates".into())
            .spawn(move || update_until_stopped(launcher, updates));
        if let Err(error) = started {
            eprintln!("Pane cannot check for extension updates in the background: {error}");
        }
    }

    /// The registry and downloads folder packages are read from now on
    /// ([`Launcher::with_npm_registry`], in development builds), and a
    /// look at once.
    pub(super) fn follow_registry(&self, sources: install::Sources) {
        *self.lock_sources() = sources;
        self.settled.poke();
    }

    /// The clock the updater follows from now on
    /// ([`Launcher::with_clock`]), checking again one
    /// [`FIRST_CHECK_AFTER`] after its now: the first check a clock given
    /// to a running Pane stands where the clock stands.
    pub(super) fn follow(self: &Arc<Self>, clock: Arc<dyn Clock>) {
        {
            let mut state = self.lock();
            state.clock = clock.clone();
            state.next_check = clock.now() + FIRST_CHECK_AFTER.as_millis() as u64;
        }
        clock.on_change(Box::new(poking(Arc::downgrade(self))));
        self.settled.poke();
    }

    /// Checks at once, whatever the cadence: the user's controls changed,
    /// so the packages they govern are looked at again.
    pub(super) fn check_now(&self) {
        self.lock().next_check = 0;
        self.settled.poke();
    }

    /// Waits until a check pass completed and the updater settled — every
    /// poke so far was looked at, and every update that pass staged was
    /// applied or deferred; `false` if it did not within `limit`, or it
    /// stopped. For tests and development builds, which so wait for the
    /// check a start, a cadence or a control change brings.
    pub(super) fn checked(&self, limit: Duration) -> bool {
        self.settled.checked(limit)
    }

    fn lock(&self) -> MutexGuard<'_, Checking> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_sources(&self) -> MutexGuard<'_, install::Sources> {
        self.sources
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// How long the updater may wait before it looks again: until the
    /// next check, or, while an update is deferred, no longer than
    /// [`RETRY_EVERY`]. A poke (the clock, the controls) wakes it sooner.
    fn next_wait(&self) -> Duration {
        let state = self.lock();
        let now = state.clock.now();
        let wait = Duration::from_millis(state.next_check.saturating_sub(now));
        if state.staged.is_empty() {
            wait.min(MAX_WAIT)
        } else {
            wait.min(RETRY_EVERY)
        }
    }

    /// One look: a check when one is due, then applying what is staged at
    /// the boundary each package allows. Blocks on the network, on the
    /// package checks and on the install's writes, so it runs on the
    /// updater's own thread, never the window's.
    fn pass(&self, launcher: &Launcher) {
        let (now, next) = {
            let state = self.lock();
            (state.clock.now(), state.next_check)
        };
        if now >= next {
            self.check(launcher);
        }
        self.apply(launcher);
    }

    /// Checks every eligible installed package for a newer version and
    /// stages what it finds, explaining what failed: a package whose
    /// metadata cannot be read, and a newer version that cannot be
    /// downloaded or does not pass the checks an install makes. The
    /// installed copy is left as it is either way.
    fn check(&self, launcher: &Launcher) {
        // What to check: the eligible installed npm packages, skipping any
        // something is already happening to (an install, an update, a
        // change the user asked for), which the next check catches.
        let candidates: Vec<InstalledPackage> = {
            let state = launcher.lock();
            state
                .packages
                .iter()
                .filter(|package| {
                    eligible(&state, package) && !state.changing.contains_key(&package.identity)
                })
                .cloned()
                .collect()
        };
        // A package that is no longer a candidate, or whose installed copy
        // is not at the version its staged update was staged against,
        // keeps nothing staged: the next check plans again.
        let at: Vec<(PackageIdentity, String)> = candidates
            .iter()
            .filter_map(|package| {
                package
                    .npm
                    .as_ref()
                    .map(|npm| (package.identity.clone(), npm.version.clone()))
            })
            .collect();
        self.lock().staged.retain(|staged| {
            at.iter().any(|(identity, version)| {
                identity == &staged.identity && version == &staged.installed
            })
        });
        for package in candidates {
            let Some(npm) = package.npm.clone() else {
                continue;
            };
            let registry = self.lock_sources().registry.clone();
            match npm::latest_version(&registry, &npm.name) {
                Err(reason) => {
                    report(
                        &mut launcher.lock(),
                        Status::Error(format!(
                            "{} was not checked for a newer version: {reason}",
                            package.title()
                        )),
                    );
                }
                Ok(latest) => {
                    // Up to date, or staged for that version already:
                    // nothing is fetched.
                    let staged_at_latest = self.lock().staged.iter().any(|staged| {
                        staged.identity == package.identity
                            && staged
                                .package
                                .npm
                                .as_ref()
                                .is_some_and(|origin| origin.package.version == latest)
                    });
                    if latest == npm.version || staged_at_latest {
                        continue;
                    }
                    if let Some(staged) = self.stage(launcher, &npm.name, &npm.version) {
                        self.lock().staged.push(staged);
                    }
                }
            }
        }
        let mut state = self.lock();
        state.next_check = state
            .clock
            .now()
            .saturating_add(CHECK_EVERY.as_millis() as u64);
        self.settled.checked_pass();
    }

    /// Downloads and checks the latest version of `name` as an install
    /// checks a package, working out what it means for its dependencies,
    /// against an installed copy at `installed`: the update to stage, or
    /// `None` with the status line saying why the installed copy stays.
    /// Blocks on the network and the checks; runs no code of the package.
    fn stage(&self, launcher: &Launcher, name: &str, installed: &str) -> Option<Staged> {
        let title = |launcher: &Launcher| {
            launcher
                .lock()
                .package(&PackageIdentity::npm(name))
                .map(InstalledPackage::title)
                .unwrap_or_else(|| name.to_owned())
        };
        let spec = crate::npm::NpmSpec {
            name: name.to_owned(),
            // No version: the latest, and not pinned by the update.
            version: None,
        };
        // The updater's own sources: a WeakLauncher keeps the launcher's
        // as they were when it was made, before a development build's
        // registry was given.
        let sources = self.lock_sources().clone();
        let checked = futures::executor::block_on(
            launcher.read_and_check_from(sources, install::Request::Npm(spec)),
        );
        let package = match checked {
            Ok(package) => package,
            Err(reason) => {
                // The message is built before the lock is taken: `title`
                // reads the state itself.
                let message = format!(
                    "{} was not updated: {reason}. It keeps running its installed code.",
                    title(launcher)
                );
                report(&mut launcher.lock(), Status::Error(message));
                return None;
            }
        };
        let version = package
            .npm
            .as_ref()
            .map(|origin| origin.package.version.clone())
            .unwrap_or_default();
        let sources = self.lock_sources().clone();
        let (package, plan) =
            futures::executor::block_on(launcher.plan_dependencies_from(sources, package));
        if !plan.problems.is_empty() {
            // The same checks an install makes, with the same words: what
            // the newer version needs is not there. The message is built
            // before the lock is taken: `title` reads the state itself.
            let problems: Vec<String> = plan.problems.iter().map(ToString::to_string).collect();
            let message = format!(
                "{} was not updated to {version}: {}. It keeps running its installed code.",
                title(launcher),
                problems.join("; ")
            );
            report(&mut launcher.lock(), Status::Error(message));
            return None;
        }
        Some(Staged {
            identity: package.identity.clone(),
            installed: installed.to_owned(),
            package,
            plan,
        })
    }

    /// Applies every staged update whose package is at the safe boundary,
    /// deferring the others (see the module documentation).
    fn apply(&self, launcher: &Launcher) {
        let staged = std::mem::take(&mut self.lock().staged);
        for update in staged {
            match self.apply_one(launcher, update) {
                Applied::Yes => {}
                Applied::Deferred(update) => self.lock().staged.push(*update),
                Applied::Dropped => {}
            }
        }
    }

    /// Applies one staged update: `Yes` when it replaced the installed
    /// copy (or was applied and its failure explained), `Deferred` when
    /// the package is in use or busy, so it is tried again, and `Dropped`
    /// when it is not for the package as it is now.
    fn apply_one(&self, launcher: &Launcher, update: Staged) -> Applied {
        let identity = update.package.identity.clone();
        // What the update installs, checked as it was staged.
        let target = update
            .package
            .npm
            .as_ref()
            .map(|origin| origin.package.version.clone())
            .unwrap_or_default();
        let mut claimed;
        {
            let mut state = launcher.lock();
            let Some(installed) = state.package(&identity) else {
                return Applied::Dropped;
            };
            let Some(npm) = &installed.npm else {
                return Applied::Dropped;
            };
            // Not the copy this was staged for, or already there: the next
            // check plans again.
            if npm.version != update.installed || npm.version == target {
                return Applied::Dropped;
            }
            // The user turned updates off for it, pinned, disabled or
            // paused it, or something else is happening to it: not now.
            if !eligible(&state, installed) || state.changing.contains_key(&identity) {
                return Applied::Dropped;
            }
            if !quiet(launcher, &state, installed) {
                return Applied::Deferred(Box::new(update));
            }
            // Claimed as an update, at the boundary: from here the
            // replacement is on its way, and no call the user makes into
            // the package is started (see `Launcher::open_command`).
            match install::claim(
                &mut state,
                &Mode::Update(identity.clone()),
                &update.plan.assumptions,
                Changing::BackgroundUpdating,
            ) {
                Ok(claims) => claimed = claims,
                Err(install::Refusal::Busy(_)) => return Applied::Deferred(Box::new(update)),
                Err(install::Refusal::Changed) => return Applied::Dropped,
            }
        }
        let store = launcher
            .installation
            .as_ref()
            .expect("an eligible package is installed")
            .store
            .clone();
        let result = futures::executor::block_on(launcher.install_plan(
            store,
            update.package,
            update.plan,
            &Mode::Update(identity.clone()),
            None,
            &mut claimed,
        ));
        let mut state = launcher.lock();
        for identity in &claimed {
            state.release(identity);
        }
        match result {
            Ok(outcome) => {
                let message = install::outcome_message(&Mode::Update(identity), &outcome);
                for dependency in outcome.dependencies {
                    launcher.put_installed(&mut state, dependency);
                }
                let package = outcome.package;
                let first = package.commands().first().map(|c| c.component.clone());
                let replaced_is_open = launcher.put_installed(&mut state, package);
                if replaced_is_open {
                    // A command of the replaced copy opened in the moment
                    // between the boundary check and the replacement.
                    launcher.show_root(&mut state, first);
                    state.view.status = Status::Result(message);
                } else {
                    // The list and root search show the new version's
                    // commands; another screen keeps its own status.
                    launcher.refresh(&mut state);
                    if matches!(
                        state.view.screen,
                        Screen::Root { .. } | Screen::Extensions { .. }
                    ) {
                        state.view.status = Status::Result(message);
                    }
                }
            }
            // What the update needs changed while it was being applied:
            // the next check plans again.
            Err(Stopped::Changed(_)) => return Applied::Yes,
            Err(Stopped::Failed(failure)) => {
                let message = launcher.install_left_behind(&mut state, &failure);
                launcher.refresh(&mut state);
                let title = state.title_of(&identity);
                report(
                    &mut state,
                    Status::Error(format!("{title} was not updated: {message}")),
                );
            }
        }
        launcher.changed();
        Applied::Yes
    }
}

/// Whether `package` is one Pane updates by itself: installed from npm,
/// not pinned, enabled, not paused after a failure, and not turned off
/// by the user's controls (the global one first).
fn eligible(state: &State, package: &InstalledPackage) -> bool {
    let Some(npm) = &package.npm else {
        return false;
    };
    !npm.pinned
        && package.enabled
        && !state.paused.is_paused(&package.identity)
        && state.update_controls.automatic
        && !state.update_controls.off.contains(&package.identity.key())
}

/// Whether `package` is at the safe boundary for replacing its code: no
/// screen of one of its commands is on display, and no call of it the
/// user asked for is running. Managed background work is not waited for
/// (a replacement ends it with the generation and the new code starts it
/// again); nor are the calls other packages make into it, which answer
/// that the package was updated and may be made again.
fn quiet(launcher: &Launcher, state: &State, package: &InstalledPackage) -> bool {
    // A command of the package on display: its items, searches, forms and
    // views run while it is open, and the user is in it.
    if state
        .open
        .as_ref()
        .is_some_and(|open| open.starts_with(&package.location))
    {
        return false;
    }
    // A call the user asked for that has not answered: opening a command,
    // running an item, a query sent from root, a search, a form.
    match launcher.runtime() {
        Ok(runtime) => {
            let busy = runtime.busy();
            !package
                .commands()
                .iter()
                .any(|command| busy.contains(&command.component))
        }
        Err(_) => true,
    }
}

/// Shows `status` where a background update's outcome belongs: the status
/// line of root search or the extension list. Another screen keeps its
/// own status, whatever the user is doing there.
fn report(state: &mut State, status: Status) {
    if matches!(
        state.view.screen,
        Screen::Root { .. } | Screen::Extensions { .. }
    ) {
        state.view.status = status;
    }
}

/// What applying one staged update came to.
enum Applied {
    /// The installed copy was replaced (or the update was applied and its
    /// failure explained).
    Yes,
    /// The package is in use; the update is tried again. Boxed, so the
    /// small outcomes carry no staged update's size.
    Deferred(Box<Staged>),
    /// The update is not for the package as it is now; it is dropped.
    Dropped,
}

/// The updater thread: looks when work may be due, then waits until
/// something is, this Pane stops, or the clock or the user's controls
/// change.
fn update_until_stopped(launcher: WeakLauncher, updates: Weak<Updates>) {
    while let Some(current) = updates.upgrade() {
        let Some(seen) = current.settled.begin_look() else {
            return;
        };
        if let Some(launcher) = launcher.upgrade() {
            current.pass(&launcher);
        }
        current.settled.ended(seen);
        let wait = current.next_wait();
        if !current.settled.wait(seen, wait) {
            return;
        }
    }
}

/// A closure that pokes the updater `weak` names awake, holding it weakly
/// so that a clock does not keep it running past the launcher. For
/// [`Clock::on_change`], which asks for another look when the clock is
/// set.
fn poking(weak: Weak<Updates>) -> impl Fn() + Send + Sync + 'static {
    move || {
        if let Some(updates) = weak.upgrade() {
            updates.settled.poke();
        }
    }
}

/// Wakes the updater thread when work may be due, and tells when it
/// settled: every poke so far was looked at, and a check pass after a
/// given one completed, applying or deferring everything it staged.
#[derive(Default)]
struct Settled {
    state: Mutex<Settling>,
    condvar: std::sync::Condvar,
}

#[derive(Default)]
struct Settling {
    stopped: bool,
    /// Counts pokes: each asks the updater to look again.
    pokes: u64,
    /// Looks the updater has begun. A look in flight is one of these the
    /// updater has not ended yet, so waiting for it to end is what tells
    /// the updater is quiet, not only that the pokes so far were begun
    /// before some earlier look.
    looks: u64,
    /// Looks the updater has ended.
    done: u64,
    /// The most pokes any ended look covered.
    covered: u64,
    /// Checks the updater completed.
    checks: u64,
}

impl Settled {
    fn lock(&self) -> MutexGuard<'_, Settling> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Asks the updater to look again.
    fn poke(&self) {
        self.lock().pokes += 1;
        self.condvar.notify_all();
    }

    /// Stops the updater.
    fn stop(&self) {
        self.lock().stopped = true;
        self.condvar.notify_all();
    }

    /// Begins a look, returning the pokes it is to cover, or `None` once
    /// stopped.
    fn begin_look(&self) -> Option<u64> {
        let mut state = self.lock();
        if state.stopped {
            return None;
        }
        state.looks += 1;
        Some(state.pokes)
    }

    /// Notes that the look covering the first `seen` pokes ended.
    fn ended(&self, seen: u64) {
        let mut state = self.lock();
        state.done += 1;
        state.covered = state.covered.max(seen);
        self.condvar.notify_all();
    }

    /// Notes that a check pass completed.
    fn checked_pass(&self) {
        self.lock().checks += 1;
        self.condvar.notify_all();
    }

    /// Waits at most `limit` for a poke after the `seen`th; returns
    /// whether the updater still runs.
    fn wait(&self, seen: u64, limit: Duration) -> bool {
        let state = self.lock();
        let (state, _) = self
            .condvar
            .wait_timeout_while(state, limit, |state| !state.stopped && state.pokes == seen)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !state.stopped
    }

    /// Waits until a check pass completed and the updater settled — every
    /// poke so far was covered by a look that ended, and no look is in
    /// flight; `false` if it did not within `limit`, or it stopped.
    fn checked(&self, limit: Duration) -> bool {
        let state = self.lock();
        let (state, _) = self
            .condvar
            .wait_timeout_while(state, limit, |state| {
                !state.stopped
                    && (state.checks < 1 || state.done < state.looks || state.covered < state.pokes)
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !state.stopped
            && state.checks >= 1
            && state.done >= state.looks
            && state.covered >= state.pokes
    }
}

impl Drop for Updates {
    fn drop(&mut self) {
        self.settled.stop();
    }
}

/// One change of the user's automatic-update controls, as the extension
/// list's rows make it: the global choice, or one package's.
pub(super) struct UpdateToggle {
    /// The controls as they were, to put back if the record cannot be
    /// written.
    before: UpdateControls,
    /// The message for the status line once the choice holds.
    done: String,
}

impl Launcher {
    /// Begins turning automatic updates of every eligible package (with
    /// `None`), or of the package with `identity`, on or off: the choice
    /// is made in Pane now, and [`Launcher::finish_update_toggle`]
    /// records it. No package is updated by this; the updater looks again
    /// once the choice is recorded.
    pub(super) fn begin_update_toggle(
        &self,
        state: &mut State,
        which: Option<PackageIdentity>,
    ) -> Option<UpdateToggle> {
        let before = state.update_controls.clone();
        let done = match &which {
            None => {
                state.update_controls.automatic = !state.update_controls.automatic;
                if state.update_controls.automatic {
                    "Automatic updates of extensions are on".into()
                } else {
                    "Automatic updates of extensions are off".into()
                }
            }
            Some(identity) => {
                let key = identity.key();
                let off = !state.update_controls.off.contains(&key);
                if off {
                    state.update_controls.off.insert(key);
                } else {
                    state.update_controls.off.remove(&key);
                }
                let title = state.title_of(identity);
                if off {
                    format!("Automatic updates of {title} are off")
                } else {
                    format!("Automatic updates of {title} are on")
                }
            }
        };
        // The row's subtitle says the new choice; the choice itself holds
        // once it is recorded.
        self.refresh(state);
        Some(UpdateToggle { before, done })
    }

    /// Records the choice [`Launcher::begin_update_toggle`] began, writing
    /// the record off the calling thread; if it cannot be written, the
    /// choice goes back to what it was and the reason is shown. Turning
    /// updates on asks the updater to check at once.
    pub(super) async fn finish_update_toggle(&self, toggle: UpdateToggle) {
        let UpdateToggle { before, done } = toggle;
        let saved = {
            let (file, text) = {
                let state = self.lock();
                match self.installation.as_ref() {
                    Some(installation) => state.update_controls.text(&installation.dir),
                    None => return,
                }
            };
            let written =
                off_thread(move || write_atomically(&file, text.as_bytes(), Readers::Default));
            written.await.is_ok()
        };
        let mut state = self.lock();
        if !saved {
            state.update_controls = before;
            self.refresh(&mut state);
            state.view.status = Status::Error(
                "Could not keep the choice: Pane could not write the extension list's update \
                 record"
                    .into(),
            );
        } else {
            state.view.status = Status::Result(done);
            // On: the packages it governs are looked at at once. Off: the
            // staged updates of a package turned off are dropped at that
            // look.
            if let Some(updates) = &self.updates {
                updates.check_now();
            }
        }
        self.changed();
    }
}

/// The extension list's rows for the update controls: one per installed
/// npm package that is not pinned, after the reload rows a local package
/// has (an npm package has none), saying whether it updates by itself.
pub(super) fn package_rows(
    packages: &[InstalledPackage],
    off: &HashSet<String>,
) -> Vec<(Row, Entry)> {
    packages
        .iter()
        .filter(|package| package.npm.as_ref().is_some_and(|npm| !npm.pinned))
        .map(|package| {
            let identity = package.identity.clone();
            let title = package.title();
            let off = off.contains(&identity.key());
            let row = Row {
                id: format!("updates:{}", identity.key()),
                title: format!("Update {title} automatically"),
                subtitle: Some(if off {
                    format!(
                        "Off · replace it yourself with Update on its preview · {}",
                        identity
                    )
                } else {
                    format!(
                        "On · a compatible newer npm version replaces it once no command of it \
                         runs · {}",
                        identity
                    )
                }),
                unavailable: None,
            };
            (row, Entry::ToggleUpdates(Some(identity)))
        })
        .collect()
}

/// The extension list's row for the global choice, last of all: after
/// every package's rows, as `extension_rows` places it.
pub(super) fn global_row(automatic: bool) -> (Row, Entry) {
    let row = Row {
        id: "updates".into(),
        title: "Update extensions automatically".into(),
        subtitle: Some(if automatic {
            "On · every eligible extension updates by itself, at its source's newer version".into()
        } else {
            "Off · no extension updates by itself; choose Update on a package's preview".into()
        }),
        unavailable: None,
    };
    (row, Entry::ToggleUpdates(None))
}
