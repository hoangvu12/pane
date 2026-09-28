//! Searching inside an opened command: a command whose manifest entry sets
//! `"search": true`, such as one searching an online service, gets a search
//! field of its own once the user opens it. Pane sends it the text typed
//! there and lists what it finds; root search never asks it, so what the
//! user types there reaches no such command or its service.
//!
//! Each change of the text starts a new search and stops the one before, in
//! the runtime (where it waits, with its web requests) as well as here: an
//! answer to an older text is never shown, whether it arrives late or is
//! the older search's error. Leaving the command stops its search too. An
//! error the command answers with, such as a service that is down, is shown
//! in place of results, and counts against the extension no more than any
//! error it answers with: it is not paused for it.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use super::{Entry, Launcher, Row, Screen, State, Status, stopped};
use crate::extension_data::PackageData;
use crate::runtime::{CallError, SearchResult, StopSearch};

/// The search of an open command that searches as the user types.
pub(super) struct Searching {
    /// The command's manifest id, sent with each search.
    command: String,
    /// The command's own list, shown while its search field is blank.
    own: (Vec<Row>, Vec<Entry>),
    /// Stops the search in progress, if one is; dropping it stops it.
    in_progress: Option<StopSearch>,
}

impl Searching {
    pub(super) fn new(command: String, rows: Vec<Row>, entries: Vec<Entry>) -> Searching {
        Searching {
            command,
            own: (rows, entries),
            in_progress: None,
        }
    }
}

/// A search started, to be awaited for its answer to be shown.
type Pending = Pin<Box<dyn Future<Output = ()> + Send>>;

impl Launcher {
    /// Sets the text of the open command's search field to `query`: the
    /// search in progress, if any, is stopped. A blank text shows the
    /// command's own list again; any other asks the command, whose answer
    /// the returned future shows (the rows listed meanwhile stay, with a
    /// running status). `None` when nothing is asked.
    pub(super) fn search_in_command(&self, state: &mut State, query: &str) -> Option<Pending> {
        let searching = state.searching.as_mut()?;
        // Stopped at once: its answer, if it still comes, is not shown.
        searching.in_progress = None;
        state.search_epoch += 1;
        state.view.screen = Screen::CommandSearch {
            query: query.to_owned(),
        };
        if query.trim().is_empty() {
            let (rows, entries) = searching.own.clone();
            state.view.selected = super::first_index(&rows);
            state.view.rows = rows;
            state.entries = entries;
            state.view.status = Status::Idle;
            return None;
        }
        let command = searching.command.clone();
        let component = state.open.clone()?;
        // The package's generation as of now: disabling or reloading it
        // stops the search.
        let data = self.data_in(state, &component);
        let runtime = match self.runtime() {
            Ok(runtime) => runtime,
            Err(error) => {
                state.view.status = Status::Error(error.to_string());
                return None;
            }
        };
        let (stop, answer) = runtime.search_with(&component, &command, query.trim(), data.clone());
        if let Some(searching) = state.searching.as_mut() {
            searching.in_progress = Some(stop);
        }
        state.view.status = Status::Running;
        let epoch = state.screen_epoch;
        let search = state.search_epoch;
        let launcher = self.clone();
        Some(Box::pin(async move {
            let answer = answer.await;
            launcher.show_search_results(epoch, search, component, data, answer);
        }))
    }

    /// Lists the open command's `answer` to the search `search`, unless the
    /// screen or its text has changed since.
    fn show_search_results(
        &self,
        epoch: u64,
        search: u64,
        component: PathBuf,
        data: Option<PackageData>,
        answer: Result<Vec<SearchResult>, CallError>,
    ) {
        let Some(mut state) = self.lock_if_current(epoch) else {
            return;
        };
        if state.search_epoch != search {
            return;
        }
        let state = &mut *state;
        if let Some(searching) = state.searching.as_mut() {
            searching.in_progress = None;
        }
        let (rows, entries, status) = match (stopped(state, &component, &data), answer) {
            // Stopped while it was running: its answer is not shown.
            (Some(problem), _) => (Vec::new(), Vec::new(), Status::Error(problem)),
            // Replaced by a newer search, whose answer is shown instead.
            (None, Err(CallError::SearchStopped)) => return,
            (None, Ok(results)) => {
                let (rows, entries) = results
                    .into_iter()
                    .map(|result| {
                        let entry = Entry::Run(result.id.clone());
                        let row = Row {
                            id: result.id,
                            title: result.title,
                            subtitle: result.subtitle,
                            unavailable: None,
                        };
                        (row, entry)
                    })
                    .unzip();
                (rows, entries, Status::Idle)
            }
            (None, Err(error)) => (Vec::new(), Vec::new(), Status::Error(error.to_string())),
        };
        state.view.selected = super::first_index(&rows);
        state.view.rows = rows;
        state.entries = entries;
        state.view.status = status;
    }
}
