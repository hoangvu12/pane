//! What the launcher shows when Pane's extension runtime thread crashes
//! (#17), and restarting it.
//!
//! A crash of the shared runtime thread is not attributed to any package:
//! it is a fault in Pane or Wasmtime, not a guest trap, and whichever
//! extension ran last did not necessarily cause it. So the launcher pauses
//! nothing and names no extension. It says what happened in the status line
//! and in Manage extensions, where the details and, when Pane did not
//! restart the runtime by itself, **Restart the extension runtime** are.
//! Nothing that was running is run again by itself: the runtime answers
//! every call it held that it stopped, and the launcher never sends one
//! again. Navigation, Manage extensions and every management action (which
//! run no extension) keep working meanwhile.

use super::{Entry, Launcher, LauncherView, Row, Screen, State, Status};
use crate::runtime::{RESTART_WINDOW, RuntimeStatus};

/// The id of the extension list's row restarting the runtime.
const RESTART_ROW: &str = "pane.runtime.restart";
/// The id of the extension list's row showing why the runtime stopped.
const DETAILS_ROW: &str = "pane.runtime.details";

impl Launcher {
    /// Has the runtime tell this launcher of each crash of its thread.
    pub(super) fn report_runtime_crashes(&self) {
        let Ok(runtime) = &self.runtime else {
            return;
        };
        let launcher = self.downgrade();
        runtime.set_crash_report(std::sync::Arc::new(move |status| {
            if let Some(launcher) = launcher.upgrade() {
                launcher.note_runtime_crash(status);
            }
        }));
    }

    /// The runtime thread crashed and was restarted or not (`status`):
    /// says so, closes a custom view its instance held, updates the
    /// screens about it, and has the window redraw. Called on the crashed
    /// thread, once the helpers it ran were ended.
    fn note_runtime_crash(&self, status: &RuntimeStatus) {
        let mut state = self.lock();
        let toast = Status::Error(toast(status));
        if state.custom_view.is_some() {
            // Its guest instance, and the view with it, is gone.
            self.return_from_custom_view(&mut state, toast.clone());
        }
        match &state.view.screen {
            Screen::Extensions { .. } => self.refresh_extensions(&mut state),
            Screen::RuntimeDetails { .. } => self.keep_runtime_details(&mut state),
            _ => {}
        }
        state.view.status = toast;
        drop(state);
        self.developing.changed();
    }

    /// What the runtime does, when it failed; `None` while it runs as it
    /// started, or had no runtime to begin with.
    fn runtime_status(&self) -> Option<RuntimeStatus> {
        let status = self.runtime.as_ref().ok()?.status();
        (status != RuntimeStatus::Running).then_some(status)
    }

    /// The extension list's first rows after a crash of the runtime:
    /// restarting it, when Pane did not, and why it stopped.
    pub(super) fn runtime_rows(&self) -> Vec<(Row, Entry)> {
        let Some(status) = self.runtime_status() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        let state = match status {
            RuntimeStatus::Stopped { .. } => {
                rows.push((restart_row(), Entry::RestartRuntime));
                "Stopped after crashing"
            }
            _ => "Restarted after crashing",
        };
        let details = Row {
            id: DETAILS_ROW.into(),
            title: "Why the extension runtime stopped".into(),
            subtitle: Some(format!("{state} · The error and its diagnostics")),
            unavailable: None,
        };
        rows.push((details, Entry::RuntimeDetails));
        rows
    }

    /// Shows why the runtime stopped, and what Pane did, with a row that
    /// restarts it if Pane did not. A runtime running as it started (the
    /// user restarted it meanwhile) shows the extension list instead.
    pub(super) fn show_runtime_details(&self, state: &mut State) {
        let Some(status) = self.runtime_status() else {
            self.show_extensions(state);
            return;
        };
        let (why, what) = match &status {
            RuntimeStatus::Restarted { why } => (why, "Pane started it again by itself.".into()),
            RuntimeStatus::Stopped { why, not_restarted } => (
                why,
                format!(
                    "Pane did not start it again: {not_restarted}. Extensions run nothing until \
                     you restart it."
                ),
            ),
            RuntimeStatus::Running => unreachable!("checked above"),
        };
        let details = vec![
            "Pane's extension runtime, which runs every extension, stopped unexpectedly.".into(),
            what,
            "Pane cannot tell which extension, if any, caused it, so none is named or paused."
                .into(),
            "Calls that were running or waiting were stopped and are not run again by \
             themselves. An action may have done its work, such as saving, before its answer \
             was lost; run it again only if you want it done again."
                .into(),
            "The native helpers it ran were ended. Extensions' settings and saved data are kept."
                .into(),
            format!("Diagnostics (also written to standard error): {why}"),
        ];
        let rows = match status {
            RuntimeStatus::Stopped { .. } => vec![restart_row()],
            _ => Vec::new(),
        };
        state.entries = match rows.is_empty() {
            true => Vec::new(),
            false => vec![Entry::RestartRuntime],
        };
        self.leave_command(state);
        state.view = LauncherView::new(
            Screen::RuntimeDetails { details },
            "Why the extension runtime stopped",
        )
        .with_rows(rows);
    }

    /// Shows the runtime details again after they changed, keeping the
    /// screen epoch as refreshing does.
    pub(super) fn keep_runtime_details(&self, state: &mut State) {
        let epoch = state.screen_epoch;
        self.show_runtime_details(state);
        state.screen_epoch = epoch;
    }

    /// Restarts the runtime at the user's request and shows the extension
    /// list, at its first row.
    pub(super) fn restart_runtime(&self, state: &mut State) {
        let restarted = match &self.runtime {
            Ok(runtime) => runtime.restart(),
            Err(error) => Err(error.clone()),
        };
        self.show_extensions(state);
        state.view.status = match restarted {
            Ok(()) => Status::Result("Restarted the extension runtime".into()),
            Err(error) => {
                Status::Error(format!("Could not restart the extension runtime: {error}"))
            }
        };
    }
}

fn restart_row() -> Row {
    Row {
        id: RESTART_ROW.into(),
        title: "Restart the extension runtime".into(),
        subtitle: Some(
            "Start it again; nothing that was running when it stopped is run again".into(),
        ),
        unavailable: None,
    }
}

/// The status line when the runtime thread crashed, naming no extension.
fn toast(status: &RuntimeStatus) -> String {
    match status {
        RuntimeStatus::Stopped { .. } => format!(
            "Pane's extension runtime stopped unexpectedly again within {} minutes and was not \
             restarted; saved data is kept. Restart it in Manage extensions.",
            RESTART_WINDOW.as_secs() / 60
        ),
        _ => "Pane's extension runtime stopped unexpectedly and was started again; what was \
              running was stopped and is not run again. Saved data is kept; details are in \
              Manage extensions."
            .into(),
    }
}
