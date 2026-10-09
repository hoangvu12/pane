//! The guest's side of the `run` host functions (`wit/run.wit`): running
//! what Windows' Run dialog (Win+R) runs, and the history the two share.
//! Each takes the launcher's [`Run`] (through its `HostFunctions`) and has
//! the work done on a thread of its own: the runtime thread awaits it,
//! serving other packages' calls meanwhile, and the wait is Pane's time,
//! never the guest's computing (#18, #136), since the shell may take as
//! long as it does to start a program, and Windows' elevation prompt
//! waits for the user.
//!
//! Stopped code does nothing more: each function answers that the code
//! was stopped. A runtime no launcher drives (tests of the runtime alone)
//! answers as a launcher given no run adapter does.

use std::sync::Arc;

use super::{GuestState, lock, run_host, stopped_code};
use crate::run::{self as host_run, Run, RunError};

impl GuestState {
    /// The launcher's run adapter, unless the instance's code is stopped
    /// (then why).
    fn run_adapter(&self) -> Result<Arc<dyn Run>, String> {
        let _host = self.host();
        if let Some(end) = self.stopped() {
            return Err(stopped_code(end));
        }
        // Taken out first: the launcher is locked to read its adapter,
        // and never while the runtime's handle on it is.
        let host = lock(&self.host_functions).clone();
        Ok(host.map_or_else(host_run::none, |host| host.run()))
    }
}

/// `error` as the WIT carries it.
fn wire_error(error: RunError) -> run_host::RunError {
    match error {
        RunError::NotAvailable(why) => run_host::RunError::NotAvailable(why),
        RunError::Failed(why) => run_host::RunError::Failed(why),
        RunError::Declined(why) => run_host::RunError::Declined(why),
    }
}

/// What a command is told when the adapter's thread could not start or
/// failed.
fn failed() -> RunError {
    RunError::Failed("Pane could not reach the Run dialog's work (its thread failed)".into())
}

/// Runs `work` on a thread of its own, since the shell and the registry
/// may block (a handler starting, the elevation prompt waiting for the
/// user), and answers what it answered.
async fn off_thread<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    gone: impl FnOnce() -> T,
) -> T {
    let (reply, response) = tokio::sync::oneshot::channel();
    let started = std::thread::Builder::new()
        .name("pane-run".into())
        .spawn(move || {
            let _ = reply.send(work());
        });
    if started.is_err() {
        return gone();
    }
    response.await.unwrap_or_else(|_| gone())
}

impl run_host::Host for GuestState {
    async fn run(&mut self, line: String, elevated: bool) -> Result<(), run_host::RunError> {
        let run = self.run_adapter().map_err(run_host::RunError::Failed)?;
        let answer = self
            .hosted(off_thread(
                move || run.run(&line, elevated),
                || Err(failed()),
            ))
            .await;
        answer.map_err(wire_error)
    }

    async fn history(&mut self) -> Result<Vec<String>, run_host::RunError> {
        let run = self.run_adapter().map_err(run_host::RunError::Failed)?;
        let answer = self
            .hosted(off_thread(move || run.history(), || Err(failed())))
            .await;
        answer.map_err(wire_error)
    }

    async fn delete_from_history(&mut self, line: String) -> Result<(), run_host::RunError> {
        let run = self.run_adapter().map_err(run_host::RunError::Failed)?;
        let answer = self
            .hosted(off_thread(
                move || run.delete_from_history(&line),
                || Err(failed()),
            ))
            .await;
        answer.map_err(wire_error)
    }
}
