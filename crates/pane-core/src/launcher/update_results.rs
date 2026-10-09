//! The update results of the updater's passes (#256): what each pass
//! updated, skipped and failed, per package, kept as Pane's own record
//! beside `updates.json` (`update-results.json`) so the latest pass that
//! found something new survives a restart, and shown as a screen of the
//! launcher's ([`crate::Launcher::update_results`], the same record).
//!
//! The record is written by the updater's own thread as a pass collects
//! its outcomes (see `updates`): a pass that updated, failed or skipped
//! something it found replaces what the record held, and a pass that
//! found nothing new — every package up to date, or skipped before the
//! pass looked at it — keeps it. Each row names the package, its title
//! and what came of it: an update says the old and new version or commit,
//! a skip says why (a pinned version or revision, a local or development
//! copy, automatic updates off, disabled, paused, a newer version that
//! needs a newer Pane or is not available on this system), and a failure
//! says why, ending that the extension keeps running its installed code.
//!
//! **The announcement.** A background pass that failed something is
//! announced once, the next time the launcher is shown: a failure toast
//! ("1 extension update failed") carrying View Details, which opens the
//! results screen. Successes and skips stay quiet. The announcement is
//! keyed on the record's pass, so a new failing pass re-arms it; it is
//! not repeated for the same one.
//!
//! **The view.** The screen [`crate::Screen::UpdateResults`] lists the
//! groups in the order Updated, Skipped, Failed, hiding empty ones, each
//! row the extension's icon, title, detail and a status tag, searched by
//! what the user types (see [`crate::Launcher::set_query`]) and driven by
//! the launcher's own list keys. Each row's entry opens its extension's
//! page in Settings; the Actions panel offers that and copying the
//! details. The window draws it (see `pane`'s `features::update_results`);
//! Pane's Settings window draws it in place of the Extensions page when
//! its operation opens it (ADR 0043).

use std::path::PathBuf;

use super::{Entry, Launcher, LauncherView, Row, Screen, State, first_index};
use crate::atomic::{Readers, write_atomically};
use crate::feedback::{Toast, ToastAction, ToastDoes, ToastStyle, WindowPresence};
use crate::packages::{PackageIdentity, Pause};

/// The file the record is kept in, beside `updates.json`.
const FILE: &str = "update-results.json";

/// What every failure's detail ends with: the extension keeps running the
/// code it has installed, so a failure never looks like a loss.
pub(in crate::launcher) const KEEPS_RUNNING: &str = "It keeps running its installed code.";

/// The title of the failure that announces a pass's failures, for `failed`
/// of them.
fn failure_title(failed: usize) -> String {
    match failed {
        1 => "1 extension update failed".into(),
        failed => format!("{failed} extension updates failed"),
    }
}

/// The toast action that opens the results screen.
const VIEW_DETAILS: &str = "View Details";

/// One package's outcome in one update pass, as the record keeps it and
/// the results view shows it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    /// The package the outcome is about: what its row's icon and its
    /// Show Extension action resolve by.
    pub identity: PackageIdentity,
    /// Its title, as the pass found it.
    pub title: String,
    /// What the outcome says: the old and new version or commit of an
    /// update, the reason of a skip, the explanation of a failure.
    pub detail: String,
    /// What an update installed, the new version or commit: what a pause
    /// of that code records as the update's failure (see `Record::paused`).
    /// `None` on every other row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_to: Option<String>,
}

/// The update results of one pass, grouped as the view lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpdateResults {
    /// The packages the pass updated, with what it updated them from and
    /// to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<UpdateResult>,
    /// The packages the pass considered and did not update, each with why.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<UpdateResult>,
    /// The packages the pass failed to update, each with the explanation,
    /// ending [`KEEPS_RUNNING`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<UpdateResult>,
}

impl UpdateResults {
    /// Whether any group holds a row.
    pub fn is_empty(&self) -> bool {
        self.updated.is_empty() && self.skipped.is_empty() && self.failed.is_empty()
    }
}

/// One action the Actions panel offers in the update results view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateResultsAction {
    /// Opens the extension's page in Settings: what Enter does.
    ShowExtension,
    /// Copies the row's details, to report them to the extension's author.
    CopyDetails,
}

/// What Pane keeps of the latest pass that recorded: its results, which
/// pass they are from, and whether that pass's failure was announced.
/// Read at Pane's start; written by the updater's thread.
#[derive(Default)]
pub(in crate::launcher) struct Record {
    /// The pass the results are from: what the one-time announcement of a
    /// failure is keyed on, so a new failing pass re-arms it.
    pass: u64,
    /// Whether that pass's failure was announced already. Kept for this
    /// run only: a start of Pane announces a failure the record still
    /// holds once more, the first time it is shown.
    announced: bool,
    /// The results of that pass, as the view and the tests read them.
    pub(in crate::launcher) results: UpdateResults,
}

/// The record as `update-results.json` writes it. Each group is written
/// only when it holds anything, so the file stays small and a record from
/// before a group existed still reads.
#[derive(serde::Serialize, serde::Deserialize)]
struct Recorded {
    version: u64,
    pass: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    updated: Vec<UpdateResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<UpdateResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    failed: Vec<UpdateResult>,
}

impl Record {
    /// Reads the record kept in `dir`: none there, or one this Pane cannot
    /// read, is no record at all (a pass that records writes it anew).
    pub(in crate::launcher) fn open(dir: &std::path::Path) -> Record {
        let Ok(text) = std::fs::read_to_string(dir.join(FILE)) else {
            return Record::default();
        };
        let Ok(read) = serde_json::from_str::<Recorded>(&text) else {
            return Record::default();
        };
        (read.version == 1)
            .then(|| Record {
                pass: read.pass,
                announced: false,
                results: UpdateResults {
                    updated: read.updated,
                    skipped: read.skipped,
                    failed: read.failed,
                },
            })
            .unwrap_or_default()
    }

    /// The record's text, and where it goes.
    fn text(&self, dir: &std::path::Path) -> (PathBuf, String) {
        let recorded = Recorded {
            version: 1,
            pass: self.pass,
            updated: self.results.updated.clone(),
            skipped: self.results.skipped.clone(),
            failed: self.results.failed.clone(),
        };
        (
            dir.join(FILE),
            serde_json::to_string_pretty(&recorded).unwrap_or_default(),
        )
    }

    /// The identity of the pass after the one recorded.
    fn next_pass(&self) -> u64 {
        self.pass + 1
    }

    /// Records `pass`: whether it replaces what this record held. A pass
    /// that found something new does (see [`Pass`]); one that found
    /// nothing new does not, so what the last such pass recorded stays.
    fn record(&mut self, pass: &Pass) -> bool {
        if !pass.found_new {
            return false;
        }
        self.pass = pass.id;
        self.announced = false;
        self.results = pass.results.clone();
        true
    }

    /// The failures this record holds that were not announced yet, as a
    /// count: `None` when there are none, or they were.
    fn unannounced(&self) -> Option<usize> {
        let failed = self.results.failed.len();
        (!self.announced && failed > 0).then_some(failed)
    }

    /// Notes that the record's failure was announced.
    fn announce(&mut self) {
        self.announced = true;
    }

    /// Moves the record's Updated row for the package with `identity` at
    /// `pause`'s version to Failed, with the failure that paused it: the
    /// update's replacement passed its checks, but the code Pane paused
    /// after it could not run, and no older version is restored (Q31, ADR
    /// 0004). Whether a row moved.
    fn paused(&mut self, identity: &PackageIdentity, pause: &Pause, title: &str) -> bool {
        let version = pause.version.as_deref();
        let found = self
            .results
            .updated
            .iter()
            .position(|row| row.identity == *identity && row.updated_to.as_deref() == version);
        let Some(at) = found else {
            return false;
        };
        let mut row = self.results.updated.remove(at);
        row.detail = format!(
            "{} and is paused: Pane runs none of its code until you retry it. {}",
            pause.after.failure(title),
            pause.why
        );
        row.updated_to = None;
        self.results.failed.push(row);
        true
    }
}

/// One pass of the updater, as its outcomes collect (see `updates`): the
/// pass's identity and what it has come to for each package it
/// considered. The updater keeps the pass it is running; the outcomes of
/// what it applies land in it even after the pass's checks ended, until
/// the next pass begins.
#[derive(Clone)]
pub(in crate::launcher) struct Pass {
    /// The pass's identity: the recorded pass's number, which tells a new
    /// failing pass from one already announced.
    id: u64,
    /// Whether the pass found something new: updated, failed, or skipped a
    /// newer version it found. A pass that found nothing new records
    /// nothing (see [`Record::record`]).
    found_new: bool,
    /// What the pass has come to, per package.
    results: UpdateResults,
}

impl Pass {
    /// The pass after the one `recorded`.
    pub(in crate::launcher) fn after(recorded: &Record) -> Pass {
        Pass {
            id: recorded.next_pass(),
            found_new: false,
            results: UpdateResults::default(),
        }
    }

    /// The pass updated the package `identity`, titled `title`, from
    /// `from` to `to`.
    pub(in crate::launcher) fn updated(
        &mut self,
        identity: PackageIdentity,
        title: String,
        from: &str,
        to: &str,
    ) {
        self.found_new = true;
        self.results.updated.push(UpdateResult {
            identity,
            title,
            detail: format!("{from} → {to}"),
            updated_to: Some(to.to_owned()),
        });
    }

    /// The pass considered the package `identity`, titled `title`, and did
    /// not look at it, for `why`: its version or revision is pinned, it is
    /// a local or development copy, its automatic updates are off, it is
    /// disabled, paused, or something else is being done to it. Skipping
    /// never looks like a fault, and says why.
    pub(in crate::launcher) fn skipped(
        &mut self,
        identity: PackageIdentity,
        title: String,
        why: &str,
    ) {
        self.results.skipped.push(UpdateResult {
            identity,
            title,
            detail: why.to_owned(),
            updated_to: None,
        });
    }

    /// The pass found a newer version of the package `identity`, titled
    /// `title`, and did not take it, because `why`: it needs a newer Pane,
    /// or is not available on this system. Unlike the other skips, this
    /// one records the pass: the pass found something new.
    pub(in crate::launcher) fn refused(
        &mut self,
        identity: PackageIdentity,
        title: String,
        why: &str,
    ) {
        self.found_new = true;
        self.results.skipped.push(UpdateResult {
            identity,
            title,
            detail: format!("{why}. {KEEPS_RUNNING}"),
            updated_to: None,
        });
    }

    /// The pass failed the package `identity`, titled `title`: `why` says
    /// what failed, and the detail ends that it keeps running its
    /// installed code.
    pub(in crate::launcher) fn failed(
        &mut self,
        identity: PackageIdentity,
        title: String,
        why: &str,
    ) {
        self.found_new = true;
        self.results.failed.push(UpdateResult {
            identity,
            title,
            detail: format!("{why}. {KEEPS_RUNNING}"),
            updated_to: None,
        });
    }
}

impl Launcher {
    /// The update results of the latest pass that found something new,
    /// read without entering any flow: the same record the results screen
    /// and the failure announcement come from, for a second surface of it
    /// beside the launcher's (Pane's Settings window) and for tests.
    /// Nothing here records, shows or announces anything.
    pub fn update_results(&self) -> UpdateResults {
        self.lock().update_results.results.clone()
    }

    /// Records `pass`, the outcomes the updater collected for one pass,
    /// as Pane's own record beside `updates.json`: the latest pass that
    /// found something new replaces what the record held (a pass that
    /// found nothing new keeps it), a failure it holds is announced while
    /// the launcher is shown, and the file is written on the updater's own
    /// thread.
    pub(in crate::launcher) fn note_update_pass(&self, pass: &Pass) {
        let written = {
            let mut state = self.lock();
            let replaced = state.update_results.record(pass);
            let written = replaced
                .then(|| {
                    let installation = self.installation.as_ref()?;
                    Some(state.update_results.text(&installation.dir))
                })
                .flatten();
            // A failure recorded while the launcher is shown is announced
            // at once; one recorded while it is hidden waits for the next
            // time it is shown (see `set_window_presence`).
            if state.feedback.presence == WindowPresence::Shown {
                self.announce_update_failures(&mut state);
            }
            written
        };
        if let Some((file, text)) = written {
            let _ = write_atomically(&file, text.as_bytes(), Readers::Default);
        }
    }

    /// Announces a failure the record holds and has not announced yet, as
    /// a failure toast carrying View Details, which opens the results
    /// screen: once for the pass the failure is from, so showing the
    /// launcher again says nothing more. The announcement re-arms when a
    /// new pass records.
    pub(in crate::launcher) fn announce_update_failures(&self, state: &mut State) {
        let Some(failed) = state.update_results.unannounced() else {
            return;
        };
        state.update_results.announce();
        let toast = Toast {
            style: ToastStyle::Failure,
            title: failure_title(failed),
            message: None,
            primary: Some(ToastAction {
                title: VIEW_DETAILS.into(),
                shortcut: None,
                unbound: None,
                does: ToastDoes::ShowUpdateResults,
            }),
            secondary: None,
        };
        self.show_own_toast(state, toast);
        self.changed();
    }

    /// Notes that Pane paused the installed package with `identity`: the
    /// record's Updated row for the version that failed becomes a Failed
    /// row with the failure that paused it, and the record is written
    /// again off the calling thread (the runtime's).
    pub(in crate::launcher) fn update_results_paused(
        &self,
        state: &mut State,
        identity: &PackageIdentity,
        pause: &Pause,
    ) {
        let title = state.title_of(identity);
        let written = state
            .update_results
            .paused(identity, pause, &title)
            .then(|| {
                let installation = self.installation.as_ref()?;
                Some(state.update_results.text(&installation.dir))
            })
            .flatten();
        if let Some((file, text)) = written {
            std::thread::spawn(move || {
                let _ = write_atomically(&file, text.as_bytes(), Readers::Default);
            });
        }
    }

    /// Shows the update results of the latest pass that recorded, "Update
    /// Results", in place of whatever the launcher showed (Settings opens
    /// it too, over an open command, which is left): each group in the
    /// order Updated, Skipped, Failed, the empty ones hidden, its rows
    /// searched by what the user types.
    pub(in crate::launcher) fn show_update_results(&self, state: &mut State) {
        self.show_update_results_at(state, "");
    }

    /// [`Launcher::show_update_results`], under `query`.
    fn show_update_results_at(&self, state: &mut State, query: &str) {
        let results = state.update_results.results.clone();
        let (rows, entries) = result_rows(&results, query);
        self.leave_command(state);
        state.entries = entries;
        let screen = Screen::UpdateResults {
            query: query.to_owned(),
        };
        state.view = LauncherView::new(screen, "Update Results").with_rows(rows);
    }

    /// Searches the update results the screen lists by `query`, the text
    /// typed in its search field: the rows whose title or detail holds the
    /// trimmed text, ignoring case, keep the groups' order, and the row
    /// selected stays selected while it is still listed, else the first
    /// is. The screen is not left, and no reply is discarded.
    pub(in crate::launcher) fn search_update_results(&self, state: &mut State, query: &str) {
        let selected = state
            .view
            .selected
            .and_then(|index| state.view.rows.get(index))
            .map(|row| row.id.clone());
        let results = state.update_results.results.clone();
        let (rows, entries) = result_rows(&results, query);
        state.entries = entries;
        state.view.screen = Screen::UpdateResults {
            query: query.to_owned(),
        };
        state.view.rows = rows;
        state.view.selected = selected
            .and_then(|id| state.view.rows.iter().position(|row| row.id == id))
            .or_else(|| first_index(&state.view.rows));
    }

    /// Refreshes the update results the screen lists, after a later pass
    /// recorded or a package changed in the background, keeping the query
    /// and the screen's epoch.
    pub(in crate::launcher) fn refresh_update_results(&self, state: &mut State) {
        let query = match &state.view.screen {
            Screen::UpdateResults { query } => query.clone(),
            _ => return,
        };
        let selected = state
            .view
            .selected
            .and_then(|index| state.view.rows.get(index))
            .map(|row| row.id.clone());
        let epoch = state.screen_epoch;
        self.show_update_results_at(state, &query);
        state.screen_epoch = epoch;
        if let Some(id) = selected {
            if let Some(index) = state.view.rows.iter().position(|row| row.id == id) {
                state.view.selected = Some(index);
            }
        }
    }
}

/// The results screen's rows and entries for `results` under `query`:
/// each group in the order Updated, Skipped, Failed, the empty ones
/// hidden, the rows whose title or detail holds the trimmed `query`
/// (ignoring case), each opening its extension's page in Settings.
fn result_rows(results: &UpdateResults, query: &str) -> (Vec<Row>, Vec<Entry>) {
    // What the search holds: the rows whose title or detail holds the
    // trimmed query, ignoring case; all of them for a blank one.
    let needle = query.trim().to_lowercase();
    let mut rows = Vec::new();
    let mut entries = Vec::new();
    for group in [&results.updated, &results.skipped, &results.failed] {
        for row in group {
            if !needle.is_empty()
                && !row.title.to_lowercase().contains(&needle)
                && !row.detail.to_lowercase().contains(&needle)
            {
                continue;
            }
            rows.push(Row {
                id: row.identity.key(),
                title: row.title.clone(),
                subtitle: Some(row.detail.clone()),
                unavailable: None,
            });
            entries.push(Entry::ShowExtension(row.identity.clone()));
        }
    }
    (rows, entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_that_found_nothing_new_keeps_what_the_record_held() {
        let mut record = Record::default();
        let mut pass = Pass::after(&record);
        // A pinned package, a disabled one and an up-to-date one: nothing
        // new found, so nothing is recorded.
        pass.skipped(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "Its version is pinned",
        );
        assert!(!record.record(&pass));
        assert!(record.results.is_empty());

        // A pass that updated something records, with its skips.
        let mut pass = Pass::after(&record);
        pass.skipped(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "Its version is pinned",
        );
        pass.updated(
            PackageIdentity::npm("@pane-tests/greeter"),
            "Greeter from npm".into(),
            "0.1.0",
            "0.2.0",
        );
        assert!(record.record(&pass));
        assert_eq!(record.results.updated.len(), 1);
        assert_eq!(record.results.skipped.len(), 1);
        assert_eq!(record.results.updated[0].detail, "0.1.0 → 0.2.0");
        assert_eq!(
            record.results.updated[0].updated_to.as_deref(),
            Some("0.2.0")
        );

        // The next pass finds nothing new: the record stays.
        let mut quiet = Pass::after(&record);
        quiet.skipped(
            PackageIdentity::npm("@pane-tests/greeter"),
            "Greeter from npm".into(),
            "It is disabled",
        );
        assert!(!record.record(&quiet));
        assert_eq!(record.results.updated.len(), 1, "the record stays");
    }

    #[test]
    fn a_pass_that_refused_a_newer_version_records_but_does_not_fail() {
        let mut record = Record::default();
        let mut pass = Pass::after(&record);
        pass.refused(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "Incompatible package: it needs Pane extension API 0.2, but this Pane provides 0.1",
        );
        assert!(pass.found_new, "the pass found something new");
        assert!(record.record(&pass));
        assert!(record.results.failed.is_empty(), "skipping is no fault");
        assert_eq!(record.unannounced(), None, "nothing to announce");
        assert!(record.results.skipped[0].detail.ends_with(KEEPS_RUNNING));
    }

    #[test]
    fn a_failure_is_announced_once_until_a_new_pass_records() {
        let mut record = Record::default();
        let mut pass = Pass::after(&record);
        pass.failed(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "It was not checked for a newer version: no route to the registry",
        );
        assert!(record.record(&pass));
        assert_eq!(record.unannounced(), Some(1));
        record.announce();
        assert_eq!(record.unannounced(), None, "announced once");

        // A new failing pass re-arms it.
        let mut again = Pass::after(&record);
        again.failed(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "It was not checked for a newer version: no route to the registry",
        );
        assert!(record.record(&again));
        assert_eq!(record.unannounced(), Some(1));
        assert_eq!(failure_title(1), "1 extension update failed");
        assert_eq!(failure_title(2), "2 extension updates failed");
    }

    #[test]
    fn pausing_the_version_an_update_installed_records_the_failure() {
        let mut record = Record::default();
        let mut pass = Pass::after(&record);
        pass.updated(
            PackageIdentity::npm("@pane-tests/settings"),
            "Settings from npm".into(),
            "0.1.0",
            "0.2.0",
        );
        assert!(record.record(&pass));
        let pause = Pause {
            after: crate::packages::PauseCause::FailedToStart,
            why: "the component could not be instantiated".into(),
            version: Some("0.2.0".into()),
        };
        assert!(record.paused(
            &PackageIdentity::npm("@pane-tests/settings"),
            &pause,
            "Settings from npm"
        ));
        assert!(record.results.updated.is_empty());
        let [failed] = &record.results.failed[..] else {
            panic!("the update's row moved to Failed");
        };
        assert_eq!(failed.updated_to, None);
        assert_eq!(
            failed.detail,
            "Settings from npm could not start and is paused: Pane runs none of its code until \
             you retry it. the component could not be instantiated"
        );

        // Another version pausing, or a package the record did not
        // update, records nothing.
        let other = Pause {
            version: Some("0.3.0".into()),
            ..pause.clone()
        };
        assert!(!record.paused(
            &PackageIdentity::npm("@pane-tests/settings"),
            &other,
            "Settings from npm"
        ));
        assert!(!record.paused(
            &PackageIdentity::npm("@pane-tests/greeter"),
            &pause,
            "Greeter from npm"
        ));
    }
}
