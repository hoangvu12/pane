//! Reloading one installed package while Pane and other packages keep
//! running (ADR 0004).
//!
//! A reload has two stages, reported differently:
//!
//! 1. The package is read again from its source folder and checked as an
//!    install would check it, without running it. If that fails, nothing
//!    changes: the installed code stays in use and the reason is shown.
//! 2. Otherwise the checked package replaces the managed copy, the old
//!    instances stop (closing a command of it that is open), and the
//!    replacement starts: each of its commands available here is started
//!    and asked for its view. If that fails, its instances are stopped
//!    again and the package is reported as failed to start, with Retry; the
//!    older code is not restored.
//!
//! Settings belong to the package identity, so they are kept throughout.

use std::future::Future;
use std::path::PathBuf;

use super::{Launcher, Screen, State, Status, off_thread};
use crate::packages::{InstalledPackage, PackageError, PackageIdentity};
use crate::runtime::CallError;

/// What a reload does.
#[derive(Clone, Copy)]
pub(super) enum Attempt {
    /// Replace the package's code from its source folder, then start it.
    Reload,
    /// Start again the package's code, whose start failed.
    Retry,
}

/// A reload or retry begun by [`Launcher::begin_reload`].
pub(super) struct Reload {
    identity: PackageIdentity,
    attempt: Attempt,
}

impl Launcher {
    /// Reloads the installed package with `identity` from its source folder
    /// while Pane and other packages keep running: the package is checked as
    /// an install checks it, then replaces the installed copy and starts (see
    /// the module documentation). Its settings are kept. A command of it
    /// that is open closes; its state is not carried over. Await the
    /// returned future for the outcome.
    ///
    /// A disabled package is not reloaded. While the package is being
    /// reloaded, enabled or disabled, this does nothing.
    pub fn reload(&self, identity: &PackageIdentity) -> impl Future<Output = ()> + Send + 'static {
        self.reload_as(identity, Attempt::Reload)
    }

    /// Starts again the package with `identity`, whose reloaded code failed
    /// to start, without reading its source folder again.
    pub fn retry_start(
        &self,
        identity: &PackageIdentity,
    ) -> impl Future<Output = ()> + Send + 'static {
        self.reload_as(identity, Attempt::Retry)
    }

    fn reload_as(
        &self,
        identity: &PackageIdentity,
        attempt: Attempt,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut state = self.lock();
        let reload = self.begin_reload(&mut state, identity.clone(), attempt);
        let generation = state.screen_generation;
        drop(state);
        let launcher = self.clone();
        async move {
            if let Some(reload) = reload {
                launcher.finish_reload(generation, reload).await;
            }
        }
    }

    /// Checks that the package can be reloaded now, explaining why not.
    pub(super) fn begin_reload(
        &self,
        state: &mut State,
        identity: PackageIdentity,
        attempt: Attempt,
    ) -> Option<Reload> {
        if self.installation.is_none() {
            let error = PackageError::Storage("this launcher does not install packages".into());
            state.view.status = Status::Error(error.to_string());
            return None;
        }
        let Some(package) = state.packages.iter().find(|p| p.identity == identity) else {
            state.view.status = Status::Error(PackageError::NotInstalled(identity).to_string());
            return None;
        };
        if !package.enabled {
            state.view.status = Status::Error(format!(
                "{} is disabled; enable it to reload it",
                package.title()
            ));
            return None;
        }
        if state.changing.contains(&identity) {
            return None;
        }
        state.changing.push(identity.clone());
        state.view.status = Status::Running;
        Some(Reload { identity, attempt })
    }

    /// Carries out a reload begun by [`Launcher::begin_reload`]. The outcome
    /// is shown if the user is still on the screen it started from, or was
    /// taken to root search because a command of the package closed.
    pub(super) async fn finish_reload(&self, generation: u64, reload: Reload) {
        let Reload { identity, attempt } = reload;
        let title = self.title_of(&identity);
        let generation = match attempt {
            Attempt::Retry => {
                self.lock().failed.retain(|(failed, _)| *failed != identity);
                generation
            }
            Attempt::Reload => match self.replace(generation, &identity).await {
                Ok(generation) => generation,
                Err(error) => {
                    let message = format!(
                        "{title} was not reloaded: {error}. It keeps running its installed code."
                    );
                    self.end_reload(generation, &identity, Status::Error(message));
                    return;
                }
            },
        };
        // The replacement may have another title.
        let title = self.title_of(&identity);
        let status = match (self.start(&identity).await, attempt) {
            (Ok(()), Attempt::Reload) => Status::Result(format!("Reloaded {title}")),
            (Ok(()), Attempt::Retry) => Status::Result(format!("Started {title}")),
            (Err(error), _) => {
                let mut state = self.lock();
                let message = error.to_string();
                // The log: the diagnostics, such as a trap's backtrace, also
                // go to Pane's standard error.
                eprintln!("pane: {title} failed to start: {message}");
                state.failed.push((identity.clone(), message));
                let failed = match attempt {
                    Attempt::Reload => format!("Reloaded {title}, but it failed to start"),
                    Attempt::Retry => format!("{title} failed to start again"),
                };
                // The diagnostics can be long, so they are shown under Retry
                // in the extension list rather than here.
                Status::Error(format!(
                    "{failed}; its earlier code is not restored. Retry, or fix it and reload \
                     it; the diagnostics are under \"Retry starting {title}\"."
                ))
            }
        };
        self.end_reload(generation, &identity, status);
    }

    /// Checks the package in its source folder and makes it the installed
    /// copy, stopping the old instances. Returns the screen generation the
    /// outcome belongs to: a new one if an open command of the package
    /// closed for root search.
    async fn replace(&self, generation: u64, identity: &PackageIdentity) -> Result<u64, String> {
        let folder = identity
            .local_folder()
            .expect("a local identity has a folder")
            .to_path_buf();
        let package = self
            .read_and_check(folder)
            .await
            .map_err(|error| error.to_string())?;
        let store = self
            .installation
            .as_ref()
            .expect("begin_reload checked there is an installation")
            .store
            .clone();
        let installed = off_thread(move || {
            let mut store = store.lock().unwrap_or_else(|p| p.into_inner());
            store.update(&package)
        })
        .await
        .map_err(|error| error.to_string())?;
        let mut state = self.lock();
        let current = state.screen_generation == generation;
        let first = installed.commands().first().map(|c| c.component.clone());
        if self.put_installed(&mut state, installed) {
            self.show_root(&mut state, first);
            return Ok(state.screen_generation);
        }
        self.refresh(&mut state);
        Ok(if current {
            state.screen_generation
        } else {
            generation
        })
    }

    /// Starts each command of the package that is available on this system
    /// and asks it for its view. If one fails, the package's instances are
    /// stopped again, so a retry starts afresh. A package disabled meanwhile
    /// is not started, and that is not a failure.
    async fn start(&self, identity: &PackageIdentity) -> Result<(), CallError> {
        let components: Vec<PathBuf> = {
            let state = self.lock();
            state
                .packages
                .iter()
                .find(|package| package.identity == *identity)
                .map(|package| {
                    package
                        .available_commands()
                        .into_iter()
                        .filter(|(_, unavailable)| unavailable.is_none())
                        .map(|(command, _)| command.component)
                        .collect()
                })
                .unwrap_or_default()
        };
        let runtime = self.runtime()?;
        for component in &components {
            let settings = self.settings_of(component);
            match runtime.get_view_with(component, settings).await {
                Ok(_) | Err(CallError::Disabled) => {}
                Err(error) => {
                    runtime.forget(components.iter().cloned());
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    /// Ends a reload with `status`, shown if the screen is still the one of
    /// `generation`.
    fn end_reload(&self, generation: u64, identity: &PackageIdentity, status: Status) {
        let mut state = self.lock();
        state.changing.retain(|changing| changing != identity);
        self.refresh(&mut state);
        if state.screen_generation == generation {
            state.view.status = status;
        }
    }

    /// Updates root search or the extension list on screen after a package
    /// changed; other screens show no package state.
    fn refresh(&self, state: &mut State) {
        match &state.view.screen {
            Screen::Root { .. } => self.refresh_root(state),
            Screen::Extensions { .. } => self.refresh_extensions(state),
            Screen::Command | Screen::Package { .. } | Screen::Form(_) | Screen::CustomView(_) => {}
        }
    }

    fn title_of(&self, identity: &PackageIdentity) -> String {
        self.lock()
            .packages
            .iter()
            .find(|package| package.identity == *identity)
            .map(InstalledPackage::title)
            .unwrap_or_default()
    }
}
