//! How the window presents root search's rows: a narrow, read-only
//! projection of what the launcher already knows about each row beyond
//! its title and subtitle — what kind of thing it is, the alias and the
//! global hotkey the user gave its command, where the query matched its
//! title — and how the rows group under section labels.
//!
//! Nothing here changes what root search lists, in which order, or what a
//! row does: the projection is computed from the same state the rows and
//! their entries come from, row for row. A row's kind comes from what
//! activating it does (its entry), never from its title; a part of the
//! projection the launcher has no data for is absent, not guessed. Only
//! root search is projected: the rows of an opened command, a command's
//! search, Manage extensions and the other screens present as they always
//! did, with no kind, alias, hotkey, match or section.

use std::ops::Range;

use super::aliases::{Sending, Via};
use super::{Entry, Screen, State};
use crate::hotkeys::Shortcut;
use crate::search::title_matches;

/// What kind of thing a root row is, from what activating it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A command — an extension's, or one of Pane's own rows.
    Command,
    /// An installed application, found ahead of the query.
    Application,
    /// A file a command found for the query.
    File,
    /// A web address a command answered with.
    Link,
    /// A fallback: a command the user chose to offer any text to.
    Fallback,
}

impl RowKind {
    /// The kind as the row's trailing label names it.
    pub fn label(self) -> &'static str {
        match self {
            RowKind::Command => "Command",
            RowKind::Application => "Application",
            RowKind::File => "File",
            RowKind::Link => "Link",
            RowKind::Fallback => "Fallback",
        }
    }
}

/// One row's presentation, beside its [`super::Row`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowPresentation {
    /// What kind of thing the row is; `None` where activating it says
    /// nothing a label could (a computed answer, an explanation).
    pub kind: Option<RowKind>,
    /// The alias the user gave the row's command, while it is active.
    pub alias: Option<String>,
    /// The global hotkey the user gave the row's command, while it is
    /// registered with the system — the one that actually opens it.
    pub hotkey: Option<Shortcut>,
    /// Where the query matched the row's title: byte ranges into the
    /// title, in order and not overlapping. Empty for a blank query, or a
    /// row found by its subtitle, package or alias alone.
    pub matched: Vec<Range<usize>>,
}

/// A section label over a run of rows: the rows from `first` up to the
/// next section's `first` (or the end).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// The label's title ("Commands", "Results").
    pub label: String,
    /// The label's note on its right ("3 matches"), if any.
    pub note: Option<String>,
    /// The index of the section's first row.
    pub first: usize,
}

/// The rows' presentation and their sections, as the launcher's view
/// lists them now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presentation {
    /// One entry per row of [`super::LauncherView::rows`], in order.
    pub rows: Vec<RowPresentation>,
    /// The section labels, in order; empty off root search.
    pub sections: Vec<Section>,
}

/// The presentation of `state`'s rows.
pub(super) fn presentation(state: &State) -> Presentation {
    let Screen::Root { query } = &state.view.screen else {
        return Presentation {
            rows: vec![RowPresentation::default(); state.view.rows.len()],
            sections: Vec::new(),
        };
    };
    let rows = state
        .view
        .rows
        .iter()
        .zip(&state.entries)
        .map(|(row, entry)| {
            let command = matches!(entry, Entry::Open(_) | Entry::Unavailable(_));
            RowPresentation {
                kind: kind(entry),
                alias: command
                    .then(|| state.aliases.chosen.active_alias(&row.id))
                    .flatten()
                    .map(str::to_owned),
                hotkey: command
                    .then(|| state.bindings.registered_of(&row.id))
                    .flatten(),
                matched: title_matches(&row.title, query),
            }
        })
        .collect();
    let shown = state.view.rows.len();
    let first_fallback = state
        .entries
        .iter()
        .position(|entry| {
            matches!(
                entry,
                Entry::Send(Sending {
                    via: Via::Fallback,
                    ..
                })
            )
        })
        .unwrap_or(shown);
    Presentation {
        rows,
        sections: root_sections(query, shown, first_fallback),
    }
}

/// What kind of thing activating `entry` from root search reaches.
fn kind(entry: &Entry) -> Option<RowKind> {
    match entry {
        Entry::Open(_) | Entry::Unavailable(_) => Some(RowKind::Command),
        Entry::Send(Sending {
            via: Via::Fallback, ..
        }) => Some(RowKind::Fallback),
        Entry::Send(Sending {
            via: Via::Alias, ..
        }) => Some(RowKind::Command),
        Entry::OpenApplication { .. } => Some(RowKind::Application),
        Entry::OpenFile { .. } => Some(RowKind::File),
        Entry::OpenUrl(_) => Some(RowKind::Link),
        // Pane's own rows are its commands.
        Entry::InstallFromFolder
        | Entry::AskNpm
        | Entry::AskGit
        | Entry::Acquire(_)
        | Entry::InstallUpdate
        | Entry::CheckUpdate
        | Entry::Manage
        | Entry::Settings => Some(RowKind::Command),
        _ => None,
    }
}

/// Root search's sections for `query`: every row under "Commands" for a
/// blank query — what root search lists then is its commands, in their
/// own order, not a suggestion of recent use — and for a query, the rows
/// it found under "Results" with their count, then the fallbacks the user
/// chose, under "Fallbacks" (after the window's own notice when nothing
/// else matched, as the reference's empty board composes them).
///
/// `rows` is how many rows are listed and `fallbacks` the index of the
/// first fallback (`rows` when none is listed).
pub fn root_sections(query: &str, rows: usize, fallbacks: usize) -> Vec<Section> {
    if rows == 0 {
        return Vec::new();
    }
    if query.trim().is_empty() {
        return vec![Section {
            label: "Commands".into(),
            note: None,
            first: 0,
        }];
    }
    let mut sections = Vec::new();
    if fallbacks > 0 {
        sections.push(Section {
            label: "Results".into(),
            note: Some(match fallbacks {
                1 => "1 match".into(),
                found => format!("{found} matches"),
            }),
            first: 0,
        });
    }
    if fallbacks < rows {
        sections.push(Section {
            label: "Fallbacks".into(),
            note: None,
            first: fallbacks,
        });
    }
    sections
}
