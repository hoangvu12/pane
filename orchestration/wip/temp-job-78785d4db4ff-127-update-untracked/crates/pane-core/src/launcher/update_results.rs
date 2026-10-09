//! The update results (#256): what each pass of the automatic updates
//! came to, kept as Pane's own record beside `updates.json`, and the
//! screen that lists them.
//!
//! A pass — automatic, as the updater runs it here — records one row per
//! package it has an outcome for: an **updated** package, with the
//! revision it left and the revision it installed; a **skipped** one,
//! with why (a newer version this Pane cannot run, or one not available
//! on this system); a **failed** one, with the explanation the status
//! line used to give. The rows of the latest pass that changed or failed
//! anything are kept across restarts in `extensions/update-results.json`;
//! a pass that found nothing new does not replace them, but its time is
//! noted either way, so a later pass can say when Pane last checked.
//!
//! The record also marks which failure set has been announced: a pass
//! that failed anything is told to the user **once**, the next time the
//! launcher is shown, as a failure toast ("1 extension update failed")
//! whose **View Details** opens the results view — never while it is
//! hidden, and never twice for the same failure. A later pass with new
//! failures announces again. Successful and skipped passes stay quiet.
//!
//! The view is a screen of the launcher's own, opened from the toast's
//! View Details and from the Extensions group in Settings: the groups
//! Updated, Skipped, Failed in that order, empty ones hidden, each row
//! with the extension's icon, title, detail and a status tag, and a
//! search field. It works as any list does (arrows, Enter, the Actions
//! panel); Enter and the panel's Show Extension open the extension's
//! page in Settings, and Copy Details puts the row's details on the
//! clipboard.

use std::path::PathBuf;

use super::{Entry, Launcher, LauncherView, Row, Screen, State, first_index};
use crate::atomic::{Readers, write_atomically};
use crate::feedback::{Toast, ToastAction, ToastDoes, ToastStyle};

/// The results view's title.
pub(in crate::launcher) const VIEW_TITLE: &str = "Update Results";

/// The title of the failure toast's action that opens the view.
pub(in crate::launcher) const VIEW_DETAILS: &str = "View Details";

/// One package's outcome in one pass of the automatic updates, as the
/// view lists it and Pane's record keeps it: which group it is in and
/// the sentence that says what happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    /// The package's identity key ([`crate::PackageIdentity::key`]), which
    /// the view's rows are named by and its extensions's page in Settings
    /// is reached through.
    pub identity: String,
    pub title: String,
    pub group: UpdateGroup,
    pub detail: String,
}

/// Which of the three groups a result is in, in the order the view lists
/// them: what changed first, then what was left alone and why, then what
/// needs attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UpdateGroup {
    Updated,
    Skipped,
    Failed,
}

impl UpdateGroup {
    /// The group's label over its rows and, as a row's status tag, beside
    /// them.
    pub fn label(self) -> &'static str {
        match self {
            UpdateGroup::Updated => "Updated",
            UpdateGroup::Skipped => "Skipped",
            UpdateGroup::Failed => "Failed",
        }
    }

    /// The group as the record writes it.
    fn recorded(self) -> &'static str {
        match self {
            UpdateGroup::Updated => "updated",
            UpdateGroup::Skipped => "skipped",
            UpdateGroup::Failed => "failed",
        }
    }

    /// The group as the record writes it, if it names one.
    fn of_recorded(text: &str) -> Option<UpdateGroup> {
        match text {
            "updated" => Some(UpdateGroup::Updated),
            "skipped" => Some(UpdateGroup::Skipped),
            "failed" => Some(UpdateGroup::Failed),
            _ => None,
        }
    }
}

/// What Pane knows of its extension updates' latest results, read without
/// entering any flow and without listing any row
/// ([`Launcher::update_results`]): when the last pass ran, and the
/// results of the latest pass that changed or failed anything — the same
/// results the view shows, kept across restarts until a pass replaces
/// them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateResults {
    /// When the last pass ran, however it ended, in clock milliseconds;
    /// `None` before any has.
    pub checked_at: Option<u64>,
    /// The results of the latest pass that updated or failed anything, in
    /// the view's order; empty until one has.
    pub results: Vec<UpdateResult>,
}

/// The results Pane keeps, in the launcher's state: the latest pass's,
/// and which of its failures has been announced (see
/// [`Kept::announce`]). Read at start from the record beside
/// `updates.json`, replaced by a pass that changed or failed something,
/// written back whenever either of those changes.
#[derive(Default)]
pub(in crate::launcher) struct Kept {
    /// When the last pass ran, however it ended, in clock milliseconds.
    checked_at: Option<u64>,
    /// The latest pass that changed or failed anything, in the view's
    /// order; a pass that found nothing new does not replace these.
    results: Vec<UpdateResult>,
    /// The failure set already announced, as its marker (see
    /// [`failures_of`]): the same failure is not announced twice, across
    /// restarts too.
    announced: Option<String>,
    /// Whether a failure waits to be announced the next time the launcher
    /// is shown.
    pending: bool,
}

impl Kept {
    /// Reads the record in `dir`; `None` when there is none, or one this
    /// Pane cannot read (which it never replaces).
    pub(in crate::launcher) fn open(dir: &std::path::Path) -> Option<Kept> {
        let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
        let read: Result<Recorded, _> = serde_json::from_str(&text);
        let read = read.ok()?;
        (read.version == 1).then(|| {
            let results: Vec<UpdateResult> = read
                .results
                .unwrap_or_default()
                .into_iter()
                .filter_map(|row| {
                    Some(UpdateResult {
                        identity: row.identity,
                        title: row.title,
                        group: UpdateGroup::of_recorded(&row.group)?,
                        detail: row.detail,
                    })
                })
                .collect();
            let failures = failures_of(&results);
            Kept {
                checked_at: read.checked_at,
                results,
                // A failure this start has not announced yet, whose
                // announcement the record never noted, waits for the
                // next showing as one that just happened does.
                announced: read.announced.clone(),
                pending: failures.is_some_and(|failures| Some(failures) != read.announced),
            }
        })
    }

    /// The record's text for these results, and where it goes.
    pub(in crate::launcher) fn text(&self, dir: &std::path::Path) -> (PathBuf, String) {
        let recorded = Recorded {
            version: 1,
            checked_at: self.checked_at,
            results: (!self.results.is_empty())
                .then(|| self.results.iter().map(RecordedResult::of).collect()),
            announced: self.announced.clone(),
        };
        (
            dir.join(FILE),
            serde_json::to_string_pretty(&recorded).unwrap_or_default(),
        )
    }

    /// Notes that the pass at `at` (clock milliseconds) ran, however it
    /// ended.
    pub(in crate::launcher) fn checked(&mut self, at: u64) {
        self.checked_at = Some(at);
    }

    /// Replaces the results with `rows`, the outcomes of a pass that
    /// changed or failed something (which [`failures_of`] answers):
    /// whether a failure in them waits to be announced, which it does
    /// unless exactly this failure set was announced before.
    pub(in crate::launcher) fn replaced_by(&mut self, rows: Vec<UpdateResult>) -> bool {
        self.results = ordered(rows);
        self.pending = failures_of(&self.results)
            .is_some_and(|failures| Some(failures) != self.announced);
        self.pending
    }

    /// Marks the failures the results hold as announced.
    pub(in crate::launcher) fn announce(&mut self) {
        self.announced = failures_of(&self.results);
        self.pending = false;
    }

    /// Notes that the package with the identity key `key`, whose update
    /// these results show as applied at `version`, failed to start and
    /// was paused: its row becomes a failure saying so. Whether anything
    /// changed.
    pub(in crate::launcher) fn failed_after_update(
        &mut self,
        key: &str,
        version: Option<&str>,
        why: &str,
    ) -> bool {
        let Some(version) = version else {
            return false;
        };
        let ended = format!(" → {version}");
        let Some(row) = self.results.iter_mut().find(|row| {
            row.identity == key && row.group == UpdateGroup::Updated && row.detail.ends_with(&ended)
        }) else {
            return false;
        };
        let title = row.title.clone();
        row.group = UpdateGroup::Failed;
        row.detail = format!(
            "{title} was updated to {version}, whose code failed to start: {why}. Pane paused \
             it; retry it from its page in Settings."
        );
        self.results = ordered(std::mem::take(&mut self.results));
        true
    }

    /// What the window reads of them (see [`UpdateResults`]).
    fn snapshot(&self) -> UpdateResults {
        UpdateResults {
            checked_at: self.checked_at,
            results: self.results.clone(),
        }
    }
}

/// The file the results are recorded in, beside `updates.json` and
/// `installed.json`.
const FILE: &str = "update-results.json";

/// The results as `update-results.json` records them: when the last pass
/// ran, the results of the latest pass that changed or failed anything,
/// and which failure set has been announced.
#[derive(serde::Serialize, serde::Deserialize)]
struct Recorded {
    version: u64,
    #[serde(default, rename = "checkedAt")]
    checked_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    results: Option<Vec<RecordedResult>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    announced: Option<String>,
}

/// One result as the record writes it.
#[derive(serde::Serialize, serde::Deserialize)]
struct RecordedResult {
    identity: String,
    title: String,
    group: String,
    detail: String,
}

impl RecordedResult {
    fn of(result: &UpdateResult) -> RecordedResult {
        RecordedResult {
            identity: result.identity.clone(),
            title: result.title.clone(),
            group: result.group.recorded().to_owned(),
            detail: result.detail.clone(),
        }
    }
}

/// The marker of a failure set: what tells one pass's failures from a
/// later pass's, so the same failure is not announced twice. Each failed
/// row's identity and detail, in order; `None` when nothing failed.
fn failures_of(rows: &[UpdateResult]) -> Option<String> {
    let marker = rows
        .iter()
        .filter(|row| row.group == UpdateGroup::Failed)
        .map(|row| format!("{}\u{1f}{}", row.identity, row.detail))
        .collect::<Vec<_>>()
        .join("\u{1e}");
    (!marker.is_empty()).then_some(marker)
}

/// `rows` in the view's order: the groups Updated, Skipped, Failed, each
/// group's rows in the pass's order.
fn ordered(mut rows: Vec<UpdateResult>) -> Vec<UpdateResult> {
    rows.sort_by_key(|row| row.group);
    rows
}

/// The results view's rows for `query`: the kept results in the view's
/// order, filtered to what `query` matches in a title, a detail or an
/// identity, each row opening its extension's page in Settings.
pub(in crate::launcher) fn rows(kept: &Kept, query: &str) -> (Vec<Row>, Vec<Entry>) {
    let query = query.trim().to_lowercase();
    let mut results: Vec<&UpdateResult> = kept.results.iter().collect();
    results.sort_by_key(|row| row.group);
    results
        .into_iter()
        .filter(|row| {
            query.is_empty()
                || row.title.to_lowercase().contains(&query)
                || row.detail.to_lowercase().contains(&query)
                || row.identity.to_lowercase().contains(&query)
        })
        .map(|row| {
            (
                Row {
                    id: row.identity.clone(),
                    title: row.title.clone(),
                    subtitle: Some(row.detail.clone()),
                    unavailable: None,
                },
                Entry::UpdateResult {
                    key: row.identity.clone(),
                    group: row.group,
                },
            )
        })
        .unzip()
}

/// Writes the record `text` to `file`, on a thread of its own: the
/// announcing caller (the window's thread) must not block on the file
/// system.
pub(in crate::launcher) fn write_record(file: PathBuf, text: String) {
    let written = std::thread::Builder::new()
        .name("pane-update-results".into())
        .spawn(move || {
            if let Err(error) = write_atomically(&file, text.as_bytes(), Readers::Default) {
                crate::diagnostic!("Pane could not record the update results: {error}");
            }
        });
    if let Err(error) = written {
        crate::diagnostic!("Pane could not record the update results: {error}");
    }
}

impl Launcher {
    /// What Pane knows of its extension updates' latest results, read
    /// without entering any flow and without listing any row: when the
    /// last pass ran, and the results of the latest pass that changed or
    /// failed anything — the same results the view shows, kept across
    /// restarts until a pass replaces them.
    pub fn update_results(&self) -> UpdateResults {
        self.lock().update_results.snapshot()
    }

    /// Shows the update results, wherever the launcher is: the screen the
    /// failure announcement's View Details opens, and the Extensions group
    /// in Settings (which draws it in place of its page). It lists the
    /// groups Updated, Skipped and Failed, empty ones hidden, with a
    /// search field; each row opens its extension's page in Settings.
    pub fn show_update_results(&self) {
        let mut state = self.lock();
        self.show_results_view(&mut state);
    }

    /// Shows the results view, as [`Launcher::show_update_results`] does,
    /// on the state the caller holds.
    pub(in crate::launcher) fn show_results_view(&self, state: &mut State) {
        self.leave_command(state);
        let (rows, entries) = rows(&state.update_results, "");
        state.entries = entries;
        state.view = LauncherView::new(
            Screen::UpdateResults {
                query: String::new(),
            },
            VIEW_TITLE,
        )
        .with_rows(rows);
    }

    /// Rebuilds the results view's rows, keeping its query and its
    /// selection on the same row: after a pass replaced the results, or
    /// one of its rows became a failure.
    pub(in crate::launcher) fn refresh_results_view(&self, state: &mut State) {
        let Screen::UpdateResults { query } = &state.view.screen else {
            return;
        };
        let (query, selected) = (
            query.clone(),
            state
                .view
                .selected
                .and_then(|index| state.view.rows.get(index))
                .map(|row| row.id.clone()),
        );
        let (rows, entries) = rows(&state.update_results, &query);
        state.entries = entries;
        state.view.selected = selected
            .and_then(|id| rows.iter().position(|row| row.id == id))
            .or_else(|| first_index(&rows));
        state.view.rows = rows;
    }

    /// Filters the results view by `query`, the text typed into its
    /// search field: the rows become those it matches in a title, a
    /// detail or an identity, best left where they were by keeping the
    /// selection on the same row when it is still listed.
    pub(in crate::launcher) fn search_results_view(&self, state: &mut State, query: &str) {
        let Screen::UpdateResults { query: shown } = &mut state.view.screen else {
            return;
        };
        *shown = query.to_owned();
        let selected = state
            .view
            .selected
            .and_then(|index| state.view.rows.get(index))
            .map(|row| row.id.clone());
        let (rows, entries) = rows(&state.update_results, query);
        state.entries = entries;
        state.view.selected = selected
            .and_then(|id| rows.iter().position(|row| row.id == id))
            .or_else(|| first_index(&rows));
        state.view.rows = rows;
    }

    /// The details the results view's Copy Details action copies for the
    /// row `target` (a package's identity key): the extension, its group
    /// and what happened, as the record holds them. `None` when the
    /// results hold no such row.
    pub fn update_result_details(&self, target: &str) -> Option<String> {
        let state = self.lock();
        state
            .update_results
            .results
            .iter()
            .find(|row| row.identity == target)
            .map(|row| {
                format!(
                    "{} ({})\n{}: {}",
                    row.title,
                    row.identity,
                    row.group.label(),
                    row.detail
                )
            })
    }

    /// Announces the failures the results hold, if any wait for the next
    /// showing: a failure toast — "1 extension update failed", or how many
    /// — whose View Details opens the results view. Nothing is announced
    /// while the launcher is hidden; the caller that shows it calls this,
    /// and so does a pass that fails while the launcher is shown. Returns
    /// the record to write when it announced, having marked the failures
    /// announced.
    pub(in crate::launcher) fn announce_update_failures(
        &self,
        state: &mut State,
    ) -> Option<(PathBuf, String)> {
        if !state.update_results.pending {
            return None;
        }
        let failures = state
            .update_results
            .results
            .iter()
            .filter(|row| row.group == UpdateGroup::Failed)
            .count();
        let title = match failures {
            1 => "1 extension update failed".to_owned(),
            failures => format!("{failures} extension updates failed"),
        };
        let toast = Toast {
            style: ToastStyle::Failure,
            title,
            message: None,
            primary: Some(ToastAction {
                title: VIEW_DETAILS.into(),
                shortcut: None,
                unbound: None,
                does: ToastDoes::UpdateResults,
            }),
            secondary: None,
        };
        self.show_own_toast(state, toast);
        state.update_results.announce();
        self.installation
            .as_ref()
            .map(|installation| state.update_results.text(&installation.dir))
    }
}
