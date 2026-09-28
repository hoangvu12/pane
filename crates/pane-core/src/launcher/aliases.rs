//! Aliases and fallbacks the user gives installed commands, to reach them
//! from root search in fewer steps.
//!
//! - An **alias** is one word the user gives a command in Manage extensions.
//!   Typing it in root search lists the command first, above everything
//!   else. For a command that takes a query (`"takesQuery": true`), typing
//!   the alias, a space and more text lists a row that sends that text to
//!   the command when the user invokes it.
//! - A **fallback** is a command that takes a query, which the user chose to
//!   have offered for any text typed in root search: it is listed below every
//!   other result and never selected by itself, so the text reaches it only
//!   when the user chooses it.
//!
//! Nothing runs while the user types: the text is sent to the command only
//! when its row is invoked. Both are Pane's own records, not extension data:
//! `aliases.json` beside `installed.json`, by command id (the package
//! identity's key and the command's id in the manifest), so copies of a
//! package from other sources, even with the same titles, are distinct, and
//! a reinstalled or updated package keeps them. A disabled package's
//! command offers neither (and they never enable it); an uninstalled one's
//! are forgotten. A recorded choice whose command is gone, or no longer
//! takes a query, is shown in Manage extensions as not active.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::{Entry, FormField, FormView, Launcher, LauncherView, OpenForm, Row, Screen, State};
use super::{FormPurpose, Status, Unavailable, off_thread};
use crate::atomic::{Readers, write_atomically};
use crate::packages::{PackageIdentity, paused_reason};
use crate::runtime::FieldKind;
use crate::search::normalize;

const ALIASES_FILE: &str = "aliases.json";
const ALIASES_VERSION: u64 = 1;

/// The longest alias, in characters.
const MAX_ALIAS_CHARS: usize = 32;

/// The id prefix of the rows that offer fallbacks in root search.
const FALLBACK_ROW: &str = "fallback:";

/// The alias form's only field.
const ALIAS_FIELD: &str = "alias";

#[derive(Serialize, Deserialize)]
struct AliasesJson {
    version: u64,
    /// Each command's alias by command id.
    #[serde(default)]
    aliases: BTreeMap<String, String>,
    /// The fallback commands' ids, in the order they are offered.
    #[serde(default)]
    fallbacks: Vec<String>,
}

/// The user's aliases and fallbacks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Choices {
    /// Each command's alias, as the user typed it, by command id.
    aliases: BTreeMap<String, String>,
    /// The fallback commands' ids, in the order they are offered.
    fallbacks: Vec<String>,
}

impl Choices {
    /// `command`'s choices from `other`, replacing its own.
    fn restore(&mut self, command: &str, other: &Choices) {
        match other.aliases.get(command) {
            Some(alias) => self.aliases.insert(command.to_owned(), alias.clone()),
            None => self.aliases.remove(command),
        };
        let was = other.fallbacks.iter().any(|id| id == command);
        let is = self.fallbacks.iter().any(|id| id == command);
        if was && !is {
            self.fallbacks.push(command.to_owned());
        } else if is && !was {
            self.fallbacks.retain(|id| id != command);
        }
    }
}

/// The aliases and fallbacks, and where they are recorded.
#[derive(Default)]
pub(super) struct Aliases {
    /// Where they are recorded; `None` for a launcher that installs no
    /// packages.
    file: Option<PathBuf>,
    chosen: Choices,
    /// Why the record could not be read, if it could not; it is then never
    /// overwritten.
    unreadable: Option<String>,
    /// The choices as last recorded (or read). Held while the record is
    /// written, so writes happen one at a time; see
    /// [`Launcher::save_aliases`].
    recorded: Arc<Mutex<Choices>>,
}

impl Aliases {
    /// Reads the aliases and fallbacks recorded in `dir`.
    pub(super) fn open(dir: &Path) -> Aliases {
        let file = dir.join(ALIASES_FILE);
        let mut aliases = Aliases {
            file: Some(file.clone()),
            ..Aliases::default()
        };
        match std::fs::read_to_string(&file) {
            Ok(text) => match serde_json::from_str::<AliasesJson>(&text) {
                Ok(json) if json.version == ALIASES_VERSION => {
                    let mut fallbacks: Vec<String> = Vec::new();
                    for command in json.fallbacks {
                        if !fallbacks.contains(&command) {
                            fallbacks.push(command);
                        }
                    }
                    aliases.chosen = Choices {
                        aliases: json
                            .aliases
                            .into_iter()
                            .filter(|(_, alias)| !alias.trim().is_empty())
                            .collect(),
                        fallbacks,
                    };
                }
                Ok(json) => {
                    aliases.unreadable = Some(format!(
                        "{} has version {}, which this Pane does not read",
                        file.display(),
                        json.version
                    ))
                }
                Err(error) => {
                    aliases.unreadable = Some(format!("{} is invalid: {error}", file.display()))
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                aliases.unreadable = Some(format!("{} cannot be read: {error}", file.display()))
            }
        }
        *aliases.recorded.lock().unwrap_or_else(|p| p.into_inner()) = aliases.chosen.clone();
        aliases
    }

    /// The alias of `command` as root search matches it: none if another
    /// command has the same one (only in a record edited by hand).
    pub(super) fn active_alias(&self, command: &str) -> Option<&str> {
        let alias = self.chosen.aliases.get(command)?;
        (self.shared_with(command, alias).is_none()).then_some(alias.as_str())
    }

    /// Another command whose alias is `alias`, compared as root search
    /// compares text.
    fn shared_with(&self, command: &str, alias: &str) -> Option<&str> {
        let alias = normalize(alias);
        self.chosen
            .aliases
            .iter()
            .find(|(other, chosen)| other.as_str() != command && normalize(chosen) == alias)
            .map(|(other, _)| other.as_str())
    }

    fn is_fallback(&self, command: &str) -> bool {
        self.chosen.fallbacks.iter().any(|id| id == command)
    }

    /// The record of the choices as they are now, to be written by
    /// [`save`]; `Err` if there is nowhere to write them.
    fn record(&self) -> Result<(PathBuf, String), String> {
        if let Some(problem) = &self.unreadable {
            return Err(format!("Pane does not replace it: {problem}"));
        }
        let file = self
            .file
            .clone()
            .ok_or_else(|| "this launcher does not keep aliases".to_string())?;
        let json = AliasesJson {
            version: ALIASES_VERSION,
            aliases: self.chosen.aliases.clone(),
            fallbacks: self.chosen.fallbacks.clone(),
        };
        let text = serde_json::to_string_pretty(&json).map_err(|error| error.to_string())?;
        Ok((file, text))
    }
}

/// Writes a record made by [`Aliases::record`].
fn save(record: Result<(PathBuf, String), String>) -> Result<(), String> {
    let (file, text) = record?;
    write_atomically(&file, text.as_bytes(), Readers::Default).map_err(|error| error.to_string())
}

/// An installed command as its alias or fallback reaches it.
#[derive(Clone, Debug)]
pub(super) struct Target {
    /// The command id: the package identity's key and the manifest's id.
    id: String,
    title: String,
    component: PathBuf,
    takes_query: bool,
    /// Why it cannot run now: paused, or unavailable on this system.
    unavailable: Option<Unavailable>,
}

/// Every command of an enabled package, as aliases and fallbacks reach it;
/// a disabled package's commands are not reached.
pub(super) fn targets(state: &State) -> Vec<Target> {
    let mut targets = Vec::new();
    for package in state.packages.iter().filter(|package| package.enabled) {
        let Ok(manifest) = &package.manifest else {
            continue;
        };
        let paused = state
            .paused
            .is_paused(&package.identity)
            .then(|| Unavailable::Paused(paused_reason(&package.title())));
        for ((registration, unavailable), command) in package
            .available_commands()
            .into_iter()
            .zip(&manifest.commands)
        {
            targets.push(Target {
                id: registration.id,
                title: registration.title,
                component: registration.component,
                takes_query: command.takes_query,
                unavailable: paused
                    .clone()
                    .or(unavailable.map(Unavailable::OnThisSystem)),
            });
        }
    }
    targets
}

/// A row of root search that sends `text` to `target` when invoked, as an
/// alias or a fallback.
fn query_row(target: &Target, id: String, text: &str, how: &str) -> (Row, Entry) {
    let entry = match &target.unavailable {
        Some(reason) => Entry::Unavailable(reason.reason().to_owned()),
        None => Entry::Query {
            component: target.component.clone(),
            query: text.to_owned(),
        },
    };
    let row = Row {
        id,
        title: target.title.clone(),
        subtitle: Some(format!("Send “{text}” · {how}")),
        unavailable: target.unavailable.clone(),
    };
    (row, entry)
}

/// The rows for `query` typed as an alias followed by text: for the command
/// that takes a query whose alias is the query's first word, a row that
/// sends it the rest.
pub(super) fn alias_rows(state: &State, query: &str) -> Vec<(Row, Entry)> {
    let query = query.trim();
    let Some((word, text)) = query.split_once(char::is_whitespace) else {
        return Vec::new();
    };
    let (word, text) = (normalize(word), text.trim());
    state
        .targets
        .iter()
        .filter(|target| target.takes_query)
        .filter_map(|target| {
            let alias = state.aliases.active_alias(&target.id)?;
            (normalize(alias) == word).then(|| {
                let how = format!("alias {alias}");
                query_row(target, format!("alias:{}", target.id), text, &how)
            })
        })
        .collect()
}

/// The fallback rows for `query`, in the order the user chose them: each
/// sends the whole query to its command. None for a blank query.
pub(super) fn fallback_rows(state: &State, query: &str) -> Vec<(Row, Entry)> {
    let text = query.trim();
    if text.is_empty() {
        return Vec::new();
    }
    state
        .aliases
        .chosen
        .fallbacks
        .iter()
        .filter_map(|id| state.targets.iter().find(|target| target.id == *id))
        .filter(|target| target.takes_query)
        .map(|target| {
            let id = format!("{FALLBACK_ROW}{}", target.id);
            query_row(target, id, text, "fallback")
        })
        .collect()
}

/// The row root search selects by itself: the first one that is not a
/// fallback, whose command the user must choose.
pub(super) fn first_choice(rows: &[Row]) -> Option<usize> {
    rows.iter()
        .position(|row| !row.id.starts_with(FALLBACK_ROW))
}

/// An installed command as Manage extensions lists its alias and fallback.
struct Configured<'a> {
    id: String,
    title: String,
    takes_query: bool,
    identity: &'a PackageIdentity,
    /// Why its alias and fallback are not active, if they are not.
    inactive: Option<String>,
}

impl Launcher {
    /// The alias and fallback rows of the extension list: for each command
    /// of an enabled package, and of a disabled one with an alias or
    /// fallback, a row for its alias and, if it takes a query, one for
    /// whether it is a fallback; then one row for each recorded choice whose
    /// command is gone. Each row names its package's source, since copies of
    /// a package can share titles.
    pub(super) fn alias_rows(&self, state: &State) -> Vec<(Row, Entry)> {
        let aliases = &state.aliases;
        let chosen = |id: &str| aliases.chosen.aliases.contains_key(id) || aliases.is_fallback(id);
        let mut configured = Vec::new();
        for package in &state.packages {
            let Ok(manifest) = &package.manifest else {
                continue;
            };
            let inactive = (!package.enabled).then(|| format!("{} is disabled", package.title()));
            for (registration, command) in package.commands().into_iter().zip(&manifest.commands) {
                if package.enabled || chosen(&registration.id) {
                    configured.push(Configured {
                        id: registration.id,
                        title: registration.title,
                        takes_query: command.takes_query,
                        identity: &package.identity,
                        inactive: inactive.clone(),
                    });
                }
            }
        }
        let mut rows = Vec::new();
        for command in &configured {
            let inactive = |mut why: Option<String>| {
                why = command.inactive.clone().or(why);
                why.map_or_else(String::new, |why| format!(" · Not active: {why}"))
            };
            let alias = match aliases.chosen.aliases.get(&command.id) {
                Some(alias) => {
                    let shared = aliases
                        .shared_with(&command.id, alias)
                        .map(|_| "another command has the same alias".to_string());
                    format!("“{alias}”{}", inactive(shared))
                }
                None => "None · A word that finds it in root search".into(),
            };
            rows.push((
                Row {
                    id: format!("alias-setting:{}", command.id),
                    title: format!("Alias for {}", command.title),
                    subtitle: Some(format!("{alias} · {}", command.identity)),
                    unavailable: None,
                },
                Entry::AskAlias(command.id.clone()),
            ));
            let fallback = aliases.is_fallback(&command.id);
            if !(command.takes_query || fallback) {
                continue;
            }
            let state = match (fallback, command.takes_query) {
                (true, true) => format!(
                    "On · Offered below the results for any text typed{}",
                    inactive(None)
                ),
                (true, false) => format!(
                    "On{}",
                    inactive(Some(format!("{} no longer takes a query", command.title)))
                ),
                (false, _) => "Off · Offer it below the results for any text typed".into(),
            };
            rows.push((
                Row {
                    id: format!("fallback-setting:{}", command.id),
                    title: format!("Fallback: {}", command.title),
                    subtitle: Some(format!("{state} · {}", command.identity)),
                    unavailable: None,
                },
                Entry::ToggleFallback(command.id.clone()),
            ));
        }
        // Choices whose command is gone: dropped by an update, or recorded
        // for a package that is not installed.
        let mut missing: Vec<&String> = aliases
            .chosen
            .aliases
            .keys()
            .chain(&aliases.chosen.fallbacks)
            .filter(|id| !configured.iter().any(|command| command.id == **id))
            .collect();
        missing.sort();
        missing.dedup();
        for id in missing {
            let what = match (aliases.chosen.aliases.get(id), aliases.is_fallback(id)) {
                (Some(alias), true) => format!("Alias “{alias}” and fallback"),
                (Some(alias), false) => format!("Alias “{alias}”"),
                (None, _) => "Fallback".into(),
            };
            let (package, command) = id.rsplit_once('#').unwrap_or((id, ""));
            let why = match state.packages.iter().find(|p| p.identity.key() == package) {
                Some(owner) => format!("{} has no command `{command}` now", owner.title()),
                None => "its extension is not installed".into(),
            };
            rows.push((
                Row {
                    id: format!("missing-setting:{id}"),
                    title: format!("{what} of a missing command"),
                    subtitle: Some(format!("Not active: {why}; Enter forgets it · {id}")),
                    unavailable: None,
                },
                Entry::ForgetAlias(id.clone()),
            ));
        }
        rows
    }

    /// Shows the form that sets the alias of the command `command`, with
    /// its current alias filled in.
    pub(super) fn show_alias_form(&self, state: &mut State, command: &str) {
        let title = self.configured_title(state, command);
        let current = state
            .aliases
            .chosen
            .aliases
            .get(command)
            .cloned()
            .unwrap_or_default();
        let form = FormView {
            fields: vec![FormField {
                id: ALIAS_FIELD.into(),
                label: format!("Alias: one word that finds {title} in root search; empty for none"),
                kind: FieldKind::Text {
                    placeholder: Some("such as ec".into()),
                },
                value: current,
                error: None,
            }],
            submit_label: "Save alias".into(),
        };
        let view = LauncherView::new(Screen::Form(form), format!("Alias for {title}"));
        let return_to = std::mem::replace(&mut state.view, view);
        state.form = Some(OpenForm {
            purpose: FormPurpose::Alias(command.to_owned()),
            return_to,
            submitting: false,
        });
        state.screen_epoch += 1;
    }

    /// Applies the alias submitted for `command` in its form: refused, with
    /// the reason next to the field, if it is not one word, is too long or
    /// is another command's; else it takes effect at once and the extension
    /// list is shown, and the returned change records it.
    pub(super) fn submit_alias(&self, state: &mut State, command: &str) -> Option<AliasChange> {
        let Screen::Form(form) = &mut state.view.screen else {
            return None;
        };
        let field = form.fields.first_mut()?;
        let alias = field.value.trim().to_owned();
        let refusal = if alias.chars().any(char::is_whitespace) {
            Some("An alias is one word, without spaces".to_string())
        } else if alias.chars().count() > MAX_ALIAS_CHARS {
            Some(format!("An alias has at most {MAX_ALIAS_CHARS} characters"))
        } else {
            state.aliases.shared_with(command, &alias).map(|other| {
                let other = self.configured_title(state, other);
                format!("“{alias}” is already the alias of {other}: change it there first, or choose another")
            })
        };
        if let Some(refusal) = refusal.filter(|_| !alias.is_empty()) {
            let Screen::Form(form) = &mut state.view.screen else {
                unreachable!("the alias form is open");
            };
            let field = &mut form.fields[0];
            state.view.status = Status::Error(format!("Alias: {refusal}"));
            field.error = Some(refusal);
            return None;
        }
        let title = self.configured_title(state, command);
        let done = if alias.is_empty() {
            state.aliases.chosen.aliases.remove(command);
            format!("{title} has no alias now")
        } else {
            let done = format!("Typing “{alias}” now finds {title}");
            state
                .aliases
                .chosen
                .aliases
                .insert(command.to_owned(), alias);
            done
        };
        state.form = None;
        let at = |entry: &Entry| matches!(entry, Entry::AskAlias(id) if id == command);
        Some(self.aliases_changed(state, at, command, done))
    }

    /// Makes the command `command` a fallback if it is not one, else no
    /// longer one.
    pub(super) fn toggle_fallback(&self, state: &mut State, command: &str) -> AliasChange {
        let title = self.configured_title(state, command);
        let fallbacks = &mut state.aliases.chosen.fallbacks;
        let done = if fallbacks.iter().any(|id| id == command) {
            fallbacks.retain(|id| id != command);
            format!("{title} is no longer a fallback")
        } else {
            fallbacks.push(command.to_owned());
            format!("{title} is now offered for any text typed in root search")
        };
        let at = |entry: &Entry| matches!(entry, Entry::ToggleFallback(id) if id == command);
        self.aliases_changed(state, at, command, done)
    }

    /// Forgets the alias and fallback of `command`, whose command is gone.
    pub(super) fn forget_alias(&self, state: &mut State, command: &str) -> AliasChange {
        state.aliases.chosen.aliases.remove(command);
        state.aliases.chosen.fallbacks.retain(|id| id != command);
        let done = "Forgot the alias and fallback of a missing command".into();
        let at = |entry: &Entry| matches!(entry, Entry::ForgetAlias(_));
        self.aliases_changed(state, at, command, done)
    }

    /// Shows the extension list at the first row `at` accepts after the
    /// choices of `command` changed, with the change to record.
    fn aliases_changed(
        &self,
        state: &mut State,
        at: impl Fn(&Entry) -> bool,
        command: &str,
        done: String,
    ) -> AliasChange {
        self.show_extensions_at(state, at);
        state.view.status = Status::Running;
        AliasChange {
            command: command.to_owned(),
            done,
            epoch: state.screen_epoch,
        }
    }

    /// Records an alias or fallback change, restoring what was last
    /// recorded if it cannot (see [`Launcher::save_aliases`]).
    pub(super) async fn finish_alias_change(&self, change: AliasChange) {
        let AliasChange {
            command,
            done,
            epoch,
        } = change;
        let launcher = self.clone();
        let saved = off_thread(move || launcher.save_aliases(Some(&command))).await;
        let mut state = self.lock();
        let status = match saved {
            Ok(()) => Status::Result(done),
            Err(problem) => {
                self.refresh(&mut state);
                Status::Error(format!("Could not keep the change: {problem}"))
            }
        };
        if state.screen_epoch == epoch {
            state.view.status = status;
        }
    }

    /// Writes the choices as they are when the write begins, blocking: run
    /// it off the window's thread. Writes happen one at a time and each
    /// writes the latest choices, so the last one leaves the latest choices
    /// on disk. If it cannot write them and `undo` names a command, that
    /// command's choices in Pane go back to what was last recorded.
    fn save_aliases(&self, undo: Option<&str>) -> Result<(), String> {
        let recorded = self.lock().aliases.recorded.clone();
        let mut recorded = recorded.lock().unwrap_or_else(|p| p.into_inner());
        let (record, chosen) = {
            let state = self.lock();
            (state.aliases.record(), state.aliases.chosen.clone())
        };
        let saved = save(record);
        match (&saved, undo) {
            (Ok(()), _) => *recorded = chosen,
            (Err(_), Some(command)) => self.lock().aliases.chosen.restore(command, &recorded),
            (Err(_), None) => {}
        }
        saved
    }

    /// Forgets the aliases and fallbacks of the uninstalled package with
    /// `identity`. Returns what writes the record without them, to run off
    /// the window's thread; nothing to write if it had none.
    pub(super) fn forget_aliases_of(
        &self,
        state: &mut State,
        identity: &PackageIdentity,
    ) -> Option<impl FnOnce() -> Result<(), String> + Send + 'static> {
        let prefix = format!("{}#", identity.key());
        let chosen = &mut state.aliases.chosen;
        let before = (chosen.aliases.len(), chosen.fallbacks.len());
        chosen
            .aliases
            .retain(|command, _| !command.starts_with(&prefix));
        chosen
            .fallbacks
            .retain(|command| !command.starts_with(&prefix));
        if (chosen.aliases.len(), chosen.fallbacks.len()) == before {
            return None;
        }
        let launcher = self.clone();
        Some(move || launcher.save_aliases(None))
    }

    /// The title of the installed command `command`, whatever its package's
    /// state, or its id.
    fn configured_title(&self, state: &State, command: &str) -> String {
        state
            .packages
            .iter()
            .flat_map(|package| package.commands())
            .find(|registration| registration.id == command)
            .map_or_else(|| command.to_owned(), |registration| registration.title)
    }
}

/// An alias or fallback change that has taken effect and is being recorded.
pub(super) struct AliasChange {
    command: String,
    /// The outcome once recorded.
    done: String,
    epoch: u64,
}
