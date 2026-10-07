//! File search's place in the launcher (#126, #175): the file index runs
//! while an enabled, unpaused package declares `"fileIndex": true`, its
//! first walk waits for the launcher to be shown, and uninstalling the last
//! package that uses it deletes it. In root search a package's file rows
//! (named by the ids the index gave them, titled with the entry's own name
//! and its folder) come after what is found by title, at most
//! [`ROOT_FILE_ROWS`] of them under "Files", followed by a row opening the
//! command that searches them all with the query typed. Each row shows the
//! system's icon for its path (#142) and reads File, or Folder.
//!
//! The rows' actions are Pane's own (see `own_actions`): Enter on a program
//! the index found shows it in the file manager, never runs it.

use std::collections::BTreeSet;
use std::future::Future;

use super::{CommandRegistration, Entry, Launcher, Opening, Row, State, off_thread};
use crate::file_index::{IndexStatus, Indexer, IndexerConfig, ScopeRules, UserRules};
use crate::icons::{Icon, IconSource};
use crate::launch::LaunchSource;
use crate::packages::InstalledPackage;

/// The most file rows of the file index one command lists in root search
/// (#126's proposed default).
pub(super) const ROOT_FILE_ROWS: usize = 5;

/// Whether `package` uses the file index (`"fileIndex": true`).
pub(super) fn uses_file_index(package: &InstalledPackage) -> bool {
    package
        .manifest
        .as_ref()
        .is_ok_and(|manifest| manifest.file_index)
}

/// The row after `command`'s file rows that opens it with `query` typed
/// into its own search field ("Search Files for “plan”"), if it searches.
pub(super) fn search_all_row(command: &CommandRegistration, query: &str) -> Option<(Row, Entry)> {
    let text = query.trim();
    if !command.search || text.is_empty() {
        return None;
    }
    let mut opening = Opening::of(command, false, LaunchSource::RootSearch);
    opening.initial_search = Some(text.to_owned());
    let row = Row {
        id: format!("{}:search-all-files", command.id),
        title: format!("{} for “{text}”", command.title),
        subtitle: Some("Searches every file Pane indexed".into()),
        unavailable: None,
    };
    Some((row, Entry::Open(opening)))
}

/// The icon of the file index's entry at `path`: the system's icon for it
/// (#142), a document's or a folder's outline until it is loaded.
pub(super) fn entry_icon(path: &std::path::Path, folder: bool) -> Icon {
    let stand_in = if folder { "folder" } else { "document" };
    Icon {
        fallback: Some(Box::new(Icon::new(IconSource::Builtin {
            name: stand_in.into(),
            filled: false,
        }))),
        ..Icon::new(IconSource::File(path.to_path_buf()))
    }
}

/// Starts loading the system icons of the file rows among `entries`.
pub(super) fn want_icons<'a>(state: &State, entries: impl IntoIterator<Item = &'a Entry>) {
    for entry in entries {
        if let Entry::File(file) = entry
            && let Some(path) = &file.path
        {
            state.icon_loads.want(None, &entry_icon(path, file.folder));
        }
    }
}

impl Launcher {
    /// Tells the file index which packages use it and may run now: it is
    /// kept current while there is one, and stops watching at once when
    /// there is none.
    pub(super) fn sync_file_index(&self, state: &State) {
        let Some(files) = &state.files else {
            return;
        };
        let users: BTreeSet<String> = state
            .packages
            .iter()
            .filter(|package| state.runs(package) && uses_file_index(package))
            .map(|package| package.identity.key())
            .collect();
        files.indexer().set_users(users);
    }

    /// This launcher keeping a file index as `config` says (in Pane's cache
    /// folder, of the home folder), with the user's rules recorded beside
    /// the installed packages. Without it, file search finds nothing.
    pub fn with_file_index(self, config: IndexerConfig) -> Self {
        if let Some(files) = self.lock().files.clone() {
            let rules = self
                .installation
                .as_ref()
                .map(|installation| UserRules::read(&installation.dir))
                .unwrap_or_default();
            files.indexer().configure(config, rules);
        }
        self.sync_file_index(&self.lock());
        self
    }

    /// The file index, for the File search page (#176) and Search Files
    /// (#177); `None` without an extension runtime.
    pub fn file_indexer(&self) -> Option<Indexer> {
        self.lock().files.as_ref().map(|files| files.indexer())
    }

    /// Where the file index is now.
    pub fn file_index_status(&self) -> IndexStatus {
        self.file_indexer()
            .map(|indexer| indexer.status())
            .unwrap_or_default()
    }

    /// The roots and rules in force (the defaults with the user's over
    /// them), and the user's own rules; `None` when this Pane keeps no file
    /// index.
    pub fn file_search_rules(&self) -> Option<(ScopeRules, UserRules)> {
        let indexer = self.file_indexer()?;
        Some((indexer.rules()?, indexer.user_rules()))
    }

    /// Records the user's file search `rules` in Pane's own record and
    /// applies them: the index is walked again under them, off the
    /// calling thread.
    pub fn set_file_search_rules(
        &self,
        rules: UserRules,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let indexer = self.file_indexer();
        let dir = self
            .installation
            .as_ref()
            .map(|installation| installation.dir.clone());
        async move {
            let indexer =
                indexer.ok_or_else(|| String::from("Pane's extension runtime is unavailable"))?;
            off_thread(move || {
                if let Some(dir) = &dir {
                    rules.write(dir)?;
                }
                indexer.set_user_rules(rules);
                Ok(())
            })
            .await
        }
    }

    /// Deletes the file index and builds it again (the File search page's
    /// Rebuild index), off the calling thread.
    pub fn rebuild_file_index(&self) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let indexer = self.file_indexer();
        async move {
            let indexer =
                indexer.ok_or_else(|| String::from("Pane's extension runtime is unavailable"))?;
            off_thread(move || indexer.rebuild()).await
        }
    }

    /// Waits until the file index has settled: caught up, walked and every
    /// change reported so far applied (or it is off or stopped); `false`
    /// if it did not within `limit`. For tests and development builds,
    /// which so wait for it without timing it.
    #[cfg(any(test, debug_assertions))]
    #[doc(hidden)]
    pub fn wait_for_file_index(&self, limit: std::time::Duration) -> bool {
        self.file_indexer()
            .is_none_or(|indexer| indexer.wait_until_settled(limit))
    }

    /// Deletes the file index once no installed package uses it (the last
    /// one was uninstalled). Blocking work runs off the calling thread.
    pub(super) async fn forget_file_index_if_unused(&self) -> Result<(), String> {
        let indexer = {
            let state = self.lock();
            if state.packages.iter().any(uses_file_index) {
                return Ok(());
            }
            state.files.as_ref().map(|files| files.indexer())
        };
        match indexer {
            Some(indexer) => off_thread(move || indexer.delete()).await,
            None => Ok(()),
        }
    }
}
