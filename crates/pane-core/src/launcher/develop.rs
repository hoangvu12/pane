//! Development mode: building a local package after each save and reloading
//! it (ADR 0004; #12, #13).
//!
//! The author turns it on for one installed, enabled package in Manage
//! extensions. Pane then watches the package's source folder with the
//! system's file watcher. After a save (and a short pause, so that an
//! editor's several writes are one save) it runs the package's build (see
//! `crate::develop`):
//!
//! - A build that fails replaces nothing: the package keeps running its
//!   installed code, and the diagnostics are shown ("Why <title> did not
//!   build") and written to standard error.
//! - A build that succeeds is reloaded exactly as the Reload row does
//!   (`reload`): checked as an install, then replacing the managed copy and
//!   starting; a start that fails pauses the package with Retry, and the
//!   earlier code is not restored.
//! - A save while a build runs makes that build obsolete: it runs to its end
//!   but is never reloaded, and the folder is built again. Builds of one
//!   package run one at a time, and each reload ends before the next build
//!   starts, so an older build never replaces a newer one.
//!
//! Development ends when the author stops it, when the package is disabled
//! or uninstalled, and when the launcher goes: its watcher is dropped and a
//! running build is stopped with every process it started. It is not
//! recorded, so it also ends when Pane quits. Nothing else changes: another
//! installed copy of the same package (another source) is never built,
//! reloaded, disabled or swapped for this one.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use notify::event::{EventKind, MetadataKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::{Entry, Launcher, LauncherView, Row, Screen, State, Status, WeakLauncher, off_thread};
use crate::develop::{Build, BuildStop, Builder, ChangeSender, first_error, is_save};
use crate::packages::{InstalledPackage, PackageError, PackageIdentity};

/// How long the folder must stay unchanged after a save before it is built,
/// so that an editor's several writes are one save.
const SETTLE: Duration = Duration::from_millis(150);

/// How many lines of a failed build's output its details show; all of it
/// goes to standard error.
const DETAIL_LINES: usize = 60;

/// A package being developed, as [`Launcher::development`] reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Development {
    /// The source folder watched.
    pub folder: PathBuf,
    /// The build command run after each save.
    pub command: String,
    /// Whether a build is running.
    pub building: bool,
    /// How many builds have ended and been acted on: reloaded, reported as
    /// failed, or found obsolete and followed by a newer build that was.
    pub handled: u64,
    /// How many builds were obsolete when they ended, because the source
    /// was saved again meanwhile; they were not reloaded.
    pub obsolete: u64,
    /// What the last build printed, if it failed; `None` after one
    /// succeeds.
    pub failure: Option<String>,
}

/// The launcher's development: which packages are developed, and how they
/// are built.
pub(super) struct Developing {
    builder: Option<Arc<dyn Builder>>,
    changes: Option<ChangeSender>,
    sessions: Mutex<HashMap<PackageIdentity, Session>>,
}

/// One developed package.
struct Session {
    report: Development,
    /// Reaches the session's thread.
    signals: Sender<Signal>,
    /// Stops the session's build.
    stop: BuildStop,
}

/// What the session's thread is told.
enum Signal {
    /// A file in the folder was saved.
    Saved,
    /// A folder was created at the top of the source folder; it is watched
    /// too.
    Folder(PathBuf),
    /// The running build ended.
    Built(Result<String, String>),
    /// Development ends.
    Stop,
}

impl Developing {
    pub(super) fn new(builder: Option<Arc<dyn Builder>>, changes: Option<ChangeSender>) -> Self {
        Developing {
            builder,
            changes,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn sessions(&self) -> MutexGuard<'_, HashMap<PackageIdentity, Session>> {
        self.sessions.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn report(&self, identity: &PackageIdentity) -> Option<Development> {
        self.sessions()
            .get(identity)
            .map(|session| session.report.clone())
    }

    /// Changes the report of the package being developed, if it still is.
    fn update(&self, identity: &PackageIdentity, change: impl FnOnce(&mut Development)) {
        if let Some(session) = self.sessions().get_mut(identity) {
            change(&mut session.report);
        }
    }

    /// Ends the development of the package with `identity`, if it is
    /// developed: its thread drops the watcher and stops the build.
    /// Returns whether it was.
    pub(super) fn stop(&self, identity: &PackageIdentity) -> bool {
        let session = self.sessions().remove(identity);
        match session {
            Some(session) => {
                session.stop.stop();
                let _ = session.signals.send(Signal::Stop);
                true
            }
            None => false,
        }
    }

    fn stop_all(&self) {
        let sessions: Vec<Session> = self.sessions().drain().map(|(_, s)| s).collect();
        for session in sessions {
            session.stop.stop();
            let _ = session.signals.send(Signal::Stop);
        }
    }

    fn changed(&self) {
        if let Some(changes) = &self.changes {
            changes.changed();
        }
    }
}

impl Drop for Developing {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Launcher {
    /// This launcher building local packages on save with `builder`
    /// (normally [`crate::develop::Toolchains`]), and telling the window
    /// through `changes` when that changed what it shows. Without it,
    /// developing a package explains that this Pane cannot.
    pub fn with_development(self, builder: Arc<dyn Builder>, changes: ChangeSender) -> Self {
        self.developing.stop_all();
        let launcher = Launcher {
            developing: Arc::new(Developing::new(Some(builder), Some(changes))),
            ..self
        };
        // The runtime reports failures to this launcher, not the one it
        // replaces, whose development is gone.
        launcher.report_failures();
        launcher
    }

    /// Develops the installed package with `identity`: from now on, each
    /// save in its source folder builds it and, if the build succeeds,
    /// reloads it (see the module documentation). Await the returned future
    /// for the outcome; the builds happen in the background. A package
    /// already developed is left as it is.
    pub fn start_developing(
        &self,
        identity: &PackageIdentity,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let start = self.begin_developing(&mut state, identity);
        drop(state);
        let launcher = self.clone();
        let identity = identity.clone();
        async move {
            if let Some((builder, folder)) = start {
                launcher.finish_developing(identity, builder, folder).await;
            }
        }
    }

    /// Ends the development of the package with `identity`: its folder is
    /// no longer watched, and a build that is running is stopped.
    pub fn stop_developing(&self, identity: &PackageIdentity) {
        let mut state = self.lock();
        self.end_developing(&mut state, identity);
        self.refresh(&mut state);
    }

    /// Ends every package's development, as when Pane quits.
    pub fn stop_all_development(&self) {
        self.developing.stop_all();
    }

    /// The development of the package with `identity`, if it is developed.
    pub fn development(&self, identity: &PackageIdentity) -> Option<Development> {
        self.developing.report(identity)
    }

    /// Checks that the package can be developed now, explaining why not.
    pub(super) fn begin_developing(
        &self,
        state: &mut State,
        identity: &PackageIdentity,
    ) -> Option<(Arc<dyn Builder>, PathBuf)> {
        if self.installation.is_none() {
            let error = PackageError::Storage("this launcher does not install packages".into());
            state.view.status = Status::Error(error.to_string());
            return None;
        }
        let Some(package) = state.package(identity) else {
            let error = PackageError::NotInstalled(identity.clone());
            state.view.status = Status::Error(error.to_string());
            return None;
        };
        let title = package.title();
        if !package.enabled {
            state.view.status =
                Status::Error(format!("{title} is disabled; enable it to develop it"));
            return None;
        }
        if self.developing.report(identity).is_some() {
            return None;
        }
        let Some(builder) = self.developing.builder.clone() else {
            state.view.status = Status::Error(format!(
                "Cannot develop {title}: this Pane does not build extensions"
            ));
            return None;
        };
        let Some(folder) = identity.local_folder() else {
            state.view.status = Status::Error(format!(
                "Cannot develop {title}: it has no local source folder"
            ));
            return None;
        };
        state.view.status = Status::Running;
        Some((builder, folder.to_path_buf()))
    }

    /// Finds how the package builds and watches its folder, off the
    /// caller's thread, then starts its session.
    pub(super) async fn finish_developing(
        &self,
        identity: PackageIdentity,
        builder: Arc<dyn Builder>,
        folder: PathBuf,
    ) {
        let (signals, received) = mpsc::channel();
        let prepared = {
            let folder = folder.clone();
            let signals = signals.clone();
            off_thread(move || {
                let build = builder.build_for(&folder)?;
                let watcher = watch(&folder, build.clone(), signals)?;
                Ok::<_, String>((build, watcher))
            })
            .await
        };
        let mut state = self.lock();
        let title = state.title_of(&identity);
        let (build, watcher) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => {
                state.view.status = Status::Error(format!("Cannot develop {title}: {reason}"));
                return;
            }
        };
        // Disabled, uninstalled or developed meanwhile: the new watcher goes.
        let enabled = state.package(&identity).is_some_and(|p| p.enabled);
        if !enabled || self.developing.report(&identity).is_some() {
            return;
        }
        let stop = BuildStop::default();
        let report = Development {
            folder: folder.clone(),
            command: build.command(),
            building: false,
            handled: 0,
            obsolete: 0,
            failure: None,
        };
        self.developing.sessions().insert(
            identity.clone(),
            Session {
                report,
                signals: signals.clone(),
                stop: stop.clone(),
            },
        );
        let session = Worker {
            launcher: self.downgrade(),
            identity,
            build: build.clone(),
            watcher,
            signals,
            received,
            stop,
        };
        std::thread::Builder::new()
            .name("pane-develop".into())
            .spawn(move || session.run())
            .expect("the development thread could not start");
        state.view.status = Status::Result(format!(
            "Developing {title}: each save in {} runs `{}`, then reloads it",
            folder.display(),
            build.command()
        ));
        self.refresh(&mut state);
    }

    /// Ends the package's development, if it is developed.
    pub(super) fn end_developing(&self, state: &mut State, identity: &PackageIdentity) {
        if self.developing.stop(identity) {
            state.view.status =
                Status::Result(format!("Stopped developing {}", state.title_of(identity)));
        }
    }

    /// Whether the package with `identity` is developed.
    pub(super) fn is_developed(&self, identity: &PackageIdentity) -> bool {
        self.developing.report(identity).is_some()
    }

    /// The extension list's development rows: for each enabled package,
    /// one that develops it or stops developing it, and one that shows why
    /// its last build failed, if it did.
    pub(super) fn development_rows(&self, packages: &[InstalledPackage]) -> Vec<(Row, Entry)> {
        let mut rows = Vec::new();
        for package in packages.iter().filter(|package| package.enabled) {
            let identity = &package.identity;
            let title = package.title();
            let source = match identity.local_folder() {
                Some(folder) => folder.display().to_string(),
                None => identity.to_string(),
            };
            let id = |kind: &str| format!("{kind}:{}", identity.key());
            match self.developing.report(identity) {
                None => rows.push((
                    Row {
                        id: id("develop"),
                        title: format!("Develop {title}"),
                        subtitle: Some(format!("Build and reload it after each save in {source}")),
                        unavailable: None,
                    },
                    Entry::Develop(identity.clone()),
                )),
                Some(development) => {
                    rows.push((
                        // The same id as the row that started it, so it
                        // stays selected.
                        Row {
                            id: id("develop"),
                            title: format!("Stop developing {title}"),
                            subtitle: Some(format!(
                                "Each save in {source} runs `{}`",
                                development.command
                            )),
                            unavailable: None,
                        },
                        Entry::StopDeveloping(identity.clone()),
                    ));
                    if development.failure.is_some() {
                        rows.push((
                            Row {
                                id: id("build-failed"),
                                title: build_details_title(&title),
                                subtitle: Some("The build's diagnostics".into()),
                                unavailable: None,
                            },
                            Entry::BuildDetails(identity.clone()),
                        ));
                    }
                }
            }
        }
        rows
    }

    /// Shows why the last build of the package with `identity` failed: the
    /// command, the folder and the end of what the build printed, with a
    /// row that builds it again. Without a failure (it built meanwhile),
    /// the extension list.
    pub(super) fn show_build_details(&self, state: &mut State, identity: &PackageIdentity) {
        let report = self.developing.report(identity);
        let Some((development, failure)) =
            report.and_then(|d| d.failure.clone().map(|failure| (d, failure)))
        else {
            self.show_extensions(state);
            return;
        };
        let title = state.title_of(identity);
        let mut details = vec![
            format!("{title} did not build, so it keeps running its installed code."),
            format!("Folder: {}", development.folder.display()),
            format!("Build command: {}", development.command),
        ];
        let lines: Vec<&str> = failure
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .collect();
        let shown = lines.len().saturating_sub(DETAIL_LINES);
        if shown > 0 {
            details.push(format!(
                "({shown} earlier lines are on Pane's standard error.)"
            ));
        }
        details.extend(lines[shown..].iter().map(|line| line.to_string()));
        let again = Row {
            id: format!("build-again:{}", identity.key()),
            title: format!("Build {title} again"),
            subtitle: Some(format!("Run `{}` now", development.command)),
            unavailable: None,
        };
        state.screen_epoch += 1;
        state.entries = vec![Entry::BuildAgain(identity.clone())];
        let screen = Screen::BuildDetails {
            identity: identity.clone(),
            details,
        };
        state.view = LauncherView::new(screen, build_details_title(&title)).with_rows(vec![again]);
    }

    /// Builds the developed package with `identity` now, as a save would.
    pub(super) fn build_again(&self, state: &mut State, identity: &PackageIdentity) {
        let sessions = self.developing.sessions();
        match sessions.get(identity) {
            Some(session) => {
                let _ = session.signals.send(Signal::Saved);
            }
            None => {
                drop(sessions);
                state.view.status = Status::Error(format!(
                    "{} is not being developed",
                    state.title_of(identity)
                ));
            }
        }
    }

    /// Shows `status` for the package's development, and redraws.
    fn show_development(&self, status: Status) {
        let mut state = self.lock();
        state.view.status = status;
        self.refresh(&mut state);
        drop(state);
        self.developing.changed();
    }
}

/// "Why <title> did not build".
pub(super) fn build_details_title(title: &str) -> String {
    format!("Why {title} did not build")
}

/// Watches `folder` for saves, as `build` tells them from its output: the
/// folder itself and every top-level folder the build does not own (not
/// `target`, `node_modules` or hidden ones), so the build's own writes are
/// never watched. Each save is sent to `signals`.
fn watch(
    folder: &Path,
    build: Arc<dyn Build>,
    signals: Sender<Signal>,
) -> Result<RecommendedWatcher, String> {
    let root = folder.to_path_buf();
    let filter = build.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        // Reading a file (as the build does) is not a save.
        let read = matches!(
            event.kind,
            EventKind::Access(_)
                | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
        );
        if read {
            return;
        }
        let saved = event.paths.iter().find(|path| {
            path.strip_prefix(&root)
                .is_ok_and(|relative| is_save(relative, &*filter))
        });
        let Some(saved) = saved else { return };
        let top = saved
            .strip_prefix(&root)
            .is_ok_and(|relative| relative.components().count() == 1);
        if matches!(event.kind, EventKind::Create(_)) && top && saved.is_dir() {
            let _ = signals.send(Signal::Folder(saved.clone()));
        }
        let _ = signals.send(Signal::Saved);
    })
    .map_err(|error| format!("Pane could not watch {}: {error}", folder.display()))?;
    let watch = |watcher: &mut RecommendedWatcher, path: &Path, mode| {
        watcher
            .watch(path, mode)
            .map_err(|error| format!("Pane could not watch {}: {error}", path.display()))
    };
    watch(&mut watcher, folder, RecursiveMode::NonRecursive)?;
    let entries = std::fs::read_dir(folder)
        .map_err(|error| format!("Pane could not read {}: {error}", folder.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = PathBuf::from(entry.file_name());
        if path.is_dir() && is_save(&name, &*build) {
            watch(&mut watcher, &path, RecursiveMode::Recursive)?;
        }
    }
    Ok(watcher)
}

/// A developed package's thread: waits for saves, builds, and reloads.
struct Worker {
    launcher: WeakLauncher,
    identity: PackageIdentity,
    build: Arc<dyn Build>,
    watcher: RecommendedWatcher,
    signals: Sender<Signal>,
    received: Receiver<Signal>,
    stop: BuildStop,
}

/// How a wait for signals ended.
enum Waited {
    Saved,
    Stopped,
}

impl Worker {
    fn run(mut self) {
        // Reloads are futures; this thread waits for them.
        let Ok(executor) = tokio::runtime::Builder::new_current_thread().build() else {
            return;
        };
        loop {
            if let Waited::Stopped = self.wait_for_save() {
                return;
            }
            // Build until a build ends with no newer save.
            let outcome = loop {
                if let Waited::Stopped = self.settle() {
                    return;
                }
                let Some(launcher) = self.launcher.upgrade() else {
                    return;
                };
                let title = launcher.title_of(&self.identity);
                launcher
                    .developing
                    .update(&self.identity, |d| d.building = true);
                launcher.show_development(Status::Progress(format!(
                    "Building {title}: {}…",
                    self.build.command()
                )));
                drop(launcher);
                match self.build_once() {
                    None => return,
                    Some((outcome, false)) => break outcome,
                    Some((_, true)) => {
                        // Obsolete: the source changed while it built.
                        if let Some(launcher) = self.launcher.upgrade() {
                            launcher
                                .developing
                                .update(&self.identity, |d| d.obsolete += 1);
                        }
                    }
                }
            };
            let Some(launcher) = self.launcher.upgrade() else {
                return;
            };
            match outcome {
                Err(diagnostics) => {
                    launcher.build_failed(&self.identity, &*self.build, diagnostics)
                }
                Ok(_) => {
                    launcher.developing.update(&self.identity, |d| {
                        d.failure = None;
                        d.building = false;
                    });
                    let reload = launcher.reload(&self.identity);
                    launcher.developing.changed();
                    executor.block_on(reload);
                }
            }
            launcher.developing.update(&self.identity, |d| {
                d.building = false;
                d.handled += 1;
            });
            launcher.developing.changed();
        }
    }

    /// Waits for a save; development may end first.
    fn wait_for_save(&mut self) -> Waited {
        loop {
            match self.received.recv() {
                Ok(Signal::Saved) => return Waited::Saved,
                Ok(Signal::Folder(folder)) => self.watch_folder(&folder),
                Ok(Signal::Built(_)) => {}
                Ok(Signal::Stop) | Err(_) => return Waited::Stopped,
            }
        }
    }

    /// Waits until no save arrived for [`SETTLE`].
    fn settle(&mut self) -> Waited {
        loop {
            match self.received.recv_timeout(SETTLE) {
                Ok(Signal::Saved | Signal::Built(_)) => {}
                Ok(Signal::Folder(folder)) => self.watch_folder(&folder),
                Err(RecvTimeoutError::Timeout) => return Waited::Saved,
                Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return Waited::Stopped,
            }
        }
    }

    /// Runs the build on a thread of its own while listening for saves.
    /// Returns its outcome and whether a save arrived meanwhile, or `None`
    /// if development ended, after stopping the build.
    fn build_once(&mut self) -> Option<(Result<String, String>, bool)> {
        let build = self.build.clone();
        let stop = self.stop.clone();
        let signals = self.signals.clone();
        let running: JoinHandle<()> = std::thread::Builder::new()
            .name("pane-build".into())
            .spawn(move || {
                let _ = signals.send(Signal::Built(build.run(&stop)));
            })
            .expect("the build thread could not start");
        let mut saved = false;
        loop {
            match self.received.recv() {
                Ok(Signal::Saved) => saved = true,
                Ok(Signal::Folder(folder)) => self.watch_folder(&folder),
                Ok(Signal::Built(outcome)) => {
                    let _ = running.join();
                    if self.stop.is_stopped() {
                        return None;
                    }
                    return Some((outcome, saved));
                }
                Ok(Signal::Stop) | Err(_) => {
                    self.stop.stop();
                    let _ = running.join();
                    return None;
                }
            }
        }
    }

    fn watch_folder(&mut self, folder: &Path) {
        let _ = self.watcher.watch(folder, RecursiveMode::Recursive);
    }
}

impl Launcher {
    /// Reports that the developed package's build failed: its code is not
    /// replaced.
    fn build_failed(&self, identity: &PackageIdentity, build: &dyn Build, diagnostics: String) {
        let title = self.title_of(identity);
        eprintln!(
            "pane: {title} did not build with `{}`:\n{diagnostics}",
            build.command()
        );
        let first = first_error(&diagnostics).trim_end_matches('.').to_owned();
        self.developing.update(identity, |d| {
            d.failure = Some(diagnostics);
            d.building = false;
        });
        self.show_development(Status::Error(format!(
            "{title} did not build: {first}. It keeps running its installed code; the \
             diagnostics are under \"{}\" in Manage extensions.",
            build_details_title(&title)
        )));
    }
}
