//! The launcher window's announcer (#132): one hidden, zero-size node of
//! Pane's own that is a polite live region, saying what a screen reader
//! user needs to hear while the focus stays in a field, a menu or a list.
//!
//! Root search keeps the accessibility focus on its query field, so a
//! screen reader echoes what is typed, and no row reports itself as focused
//! (GPUI CE implements an active descendant by reporting it as the focused
//! node, which would read results instead of the typing). A command's list,
//! the Actions panel, the footer menu, Clipboard History and Search Files
//! do the same: the focus stays in their field, menu or list. What the
//! selection does is said here instead, as both the node's name and its
//! value: AccessKit's Windows and Linux adapters announce a live region's
//! name, and its macOS adapter its value, so Narrator, NVDA, Orca and
//! VoiceOver all get the same text. Each text replaces the last, so fast
//! arrowing collapses to where the user stopped.
//!
//! What is said, in the launcher's language (English today):
//!
//! - a selection move by the user (the arrows and every other key that
//!   moves it, a number chord, the pointer): "<title>, <i> of <n>", with
//!   ", unavailable" after a row that cannot run, and the section's name
//!   first when the move enters another section ("Fallbacks: Search the
//!   web, 1 of 2");
//! - opening a command, a page or the Actions panel: "<name>, <n> results"
//!   ("<n> commands" in the panel), then the selected row;
//! - a list that becomes empty: "No results";
//! - typing: nothing for each keystroke. Once the query's results have
//!   settled, or [`SETTLE`] after the last keystroke, whichever is later,
//!   the selected row, if it is not the one last said;
//! - a selection Pane changes itself (a late result, a refreshed list):
//!   nothing, unless the selected row is another one.
//!
//! Root search says nothing as it comes back on screen: the screen reader
//! reads its field as the field takes the focus. Forms and custom views
//! keep their own accessibility; nothing is said for them.
//!
//! The footer's strip keeps its status role and carries its message (the
//! toast, or the status line) as its name. AccessKit announces only a node
//! with a live setting of its own, and the strip's children would inherit
//! one, so the announcer says the message too. When the message and the
//! selection change together, the message is said first and the
//! selection's text waits [`STATUS_LEAD`] for it.

use std::time::{Duration, Instant};

use gpui::accesskit::Live;
use gpui::{App, Context, Div, Role, Stateful, Task, div, prelude::*, px};
use pane_core::{LauncherView, Screen};

use crate::app::LauncherWindow;
use crate::features::root_search;
use crate::ui::shell::SectionLabel;

/// How long after the last keystroke the selected row may be said, once
/// the query's results have settled.
pub(crate) const SETTLE: Duration = Duration::from_millis(300);

/// How long a selection's text waits behind the footer's message it
/// changed with: a screen reader that reads the node's name when it hears
/// of the change (Narrator and NVDA through UI Automation) has read the
/// message before the selection replaces it.
pub(crate) const STATUS_LEAD: Duration = Duration::from_millis(500);

/// What is said of a list with nothing in it.
pub(crate) const NO_RESULTS: &str = "No results";

/// What a list's rows are, as its opening counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Noun {
    /// A command's list, a page or a split view: "3 results".
    Results,
    /// The Actions panel: "4 commands".
    Commands,
}

impl Noun {
    /// `count` of these, "1 result" or "3 results".
    fn counted(self, count: usize) -> String {
        let (one, many) = match self {
            Noun::Results => ("result", "results"),
            Noun::Commands => ("command", "commands"),
        };
        format!("{count} {}", if count == 1 { one } else { many })
    }
}

/// What is said when a list comes on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Opening {
    /// Nothing: root search, whose field the screen reader reads as it
    /// takes the focus.
    Silent,
    /// The selected row alone: the footer menu, whose name the screen
    /// reader reads as it takes the focus.
    Selection,
    /// The list's name and how many rows it has, then the selected row.
    Named(String, Noun),
}

/// The selected row, as it is said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selected {
    /// The row's identity: a selection that stays on it is not said again.
    pub(crate) id: String,
    /// What the row is called, as its accessible name.
    pub(crate) title: String,
    /// Its place in the whole list, from 1.
    pub(crate) position: usize,
    /// Whether it cannot run here.
    pub(crate) unavailable: bool,
    /// The section it is in, if the list has sections.
    pub(crate) section: Option<String>,
}

impl Selected {
    /// "<title>, <i> of <count>", with the section's name first when
    /// `entered`, and ", unavailable" after a row that cannot run.
    fn said(&self, count: usize, entered: bool) -> String {
        let section = match (&self.section, entered) {
            (Some(section), true) => format!("{section}: "),
            _ => String::new(),
        };
        let mut said = format!("{section}{}, {} of {count}", self.title, self.position);
        if self.unavailable {
            said.push_str(", unavailable");
        }
        said
    }
}

/// What a list has selected, as the announcer follows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// A row.
    Row(Selected),
    /// Nothing, because the list has nothing to select: no rows, or root
    /// search's notice that nothing matched.
    NoResults,
    /// Nothing, with rows listed (root search's fallbacks are never
    /// selected by themselves).
    Nothing,
}

impl Target {
    /// What is remembered of it once said.
    fn said(&self) -> Said {
        match self {
            Target::Row(row) => Said::Row {
                id: row.id.clone(),
                section: row.section.clone(),
            },
            Target::NoResults => Said::NoResults,
            Target::Nothing => Said::Nothing,
        }
    }
}

/// One list as the announcer follows it in a frame: the launcher's screen,
/// or a panel or menu over it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listing {
    /// Whether it lies over the screen (the Actions panel, the footer
    /// menu): closing it gives the screen's list back as it was.
    pub(crate) over: bool,
    /// What it shows: another key is another list, which opens.
    pub(crate) key: String,
    /// What is said as it opens.
    pub(crate) opening: Opening,
    /// How many rows it lists.
    pub(crate) count: usize,
    /// What it has selected.
    pub(crate) target: Target,
    /// Its search field's text, if it has one: a change is typing.
    pub(crate) query: Option<String>,
    /// Whether the results of its search have all arrived.
    pub(crate) settled: bool,
}

/// What the announcer last said of a list's selection.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Said {
    Row {
        id: String,
        section: Option<String>,
    },
    NoResults,
    Nothing,
}

/// A list the announcer follows: what it shows, its field's text and what
/// was last said of its selection.
struct Layer {
    key: String,
    query: Option<String>,
    said: Said,
}

/// A selection's text waiting behind the footer's message.
struct Held {
    text: String,
    until: Instant,
}

/// The announcer's state, kept by the launcher window.
#[derive(Default)]
pub(crate) struct Announcer {
    /// The node's name and value: what was said last.
    text: String,
    /// The screen's list.
    screen: Option<Layer>,
    /// The panel or menu over it, while one is open.
    over: Option<Layer>,
    /// When the field's text last changed, while the selection's text
    /// waits for typing to settle.
    typed: Option<Instant>,
    /// Whether the user moved the selection since the last frame.
    moved: bool,
    /// The searches the query field started whose results are still
    /// arriving.
    searching: usize,
    /// The footer's message as last said.
    status: Option<String>,
    /// A selection's text waiting behind the footer's message.
    held: Option<Held>,
    /// When the window draws again for the announcer, if it waits.
    wake_at: Option<Instant>,
    _wake: Option<Task<()>>,
}

impl Announcer {
    /// The user moved the selection (a key, a number chord, the pointer):
    /// it is said at once, even while typing settles.
    pub(crate) fn user_moved(&mut self) {
        self.moved = true;
    }

    /// The query field started a search, whose results arrive until the
    /// future it gave resolves.
    pub(crate) fn search_started(&mut self) {
        self.searching += 1;
    }

    /// A search the query field started has all its results.
    pub(crate) fn search_ended(&mut self) {
        self.searching = self.searching.saturating_sub(1);
    }

    /// Whether every search the query field started has its results.
    pub(crate) fn settled(&self) -> bool {
        self.searching == 0
    }

    /// Follows one frame: `listing` is what the frame shows (`None` on a
    /// form or a custom view), `status` the footer's message. Answers when
    /// the window must draw again for something waiting, if anything does.
    pub(crate) fn frame(
        &mut self,
        listing: Option<Listing>,
        status: Option<&str>,
        now: Instant,
    ) -> Option<Instant> {
        if let Some(held) = self.held.take_if(|held| now >= held.until) {
            self.text = held.text;
        }
        let moved = std::mem::take(&mut self.moved);
        let selection = listing.and_then(|listing| self.follow(listing, moved, now));
        let message = status
            .filter(|text| self.status.as_deref() != Some(*text))
            .map(str::to_owned);
        self.status = status.map(str::to_owned);
        match (message, selection) {
            // The message first; the selection, this frame's or one
            // already waiting, after it.
            (Some(message), selection) => {
                self.text = message;
                let waiting = selection.or_else(|| self.held.take().map(|held| held.text));
                self.held = waiting.map(|text| Held {
                    text,
                    until: now + STATUS_LEAD,
                });
            }
            (None, Some(selection)) => match self.held.as_mut() {
                Some(held) => held.text = selection,
                None => self.text = selection,
            },
            (None, None) => {}
        }
        // Once typing has waited its time, the search that settles it
        // draws the window itself.
        [
            self.typed.map(|typed| typed + SETTLE),
            self.held.as_ref().map(|held| held.until),
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .min()
    }

    /// Follows `listing` this frame: what is to be said of its selection,
    /// if anything.
    fn follow(&mut self, listing: Listing, moved: bool, now: Instant) -> Option<String> {
        // A panel or menu closed: the screen's list is followed again as
        // it was, and typing in the panel's field is over.
        if !listing.over && self.over.take().is_some() {
            self.typed = None;
        }
        let layer = if listing.over {
            &mut self.over
        } else {
            &mut self.screen
        };
        let target = listing.target.said();
        let same = layer.as_ref().is_some_and(|layer| layer.key == listing.key);
        if !same {
            self.typed = None;
            *layer = Some(Layer {
                key: listing.key,
                query: listing.query,
                said: target,
            });
            return opening(&listing.opening, listing.count, &listing.target);
        }
        let layer = layer.as_mut()?;
        if layer.query != listing.query {
            layer.query = listing.query;
            self.typed = Some(now);
        }
        if let Some(typed) = self.typed {
            let settled = listing.settled && now >= typed + SETTLE;
            if !moved && !settled {
                return None;
            }
            self.typed = None;
        }
        if target == layer.said {
            return None;
        }
        let before = std::mem::replace(&mut layer.said, target);
        match listing.target {
            Target::Row(row) => {
                let entered = moved
                    && match before {
                        Said::Row { section, .. } => section != row.section,
                        Said::NoResults | Said::Nothing => true,
                    };
                Some(row.said(listing.count, entered))
            }
            Target::NoResults => Some(NO_RESULTS.to_owned()),
            Target::Nothing => None,
        }
    }

    /// The node: hidden (zero-size), a polite live region whose name and
    /// value are both the text said last.
    fn node(&self) -> Stateful<Div> {
        let text = self.text.clone();
        div()
            .id("announcer")
            .absolute()
            .w(px(0.))
            .h(px(0.))
            .role(Role::Label)
            .aria_live(Live::Polite)
            .aria_live_atomic(true)
            .when(!text.is_empty(), |node| {
                node.aria_label(text.clone()).aria_value(text)
            })
    }
}

/// What is said as a list opens with `opening`, `count` rows and `target`
/// selected.
fn opening(opening: &Opening, count: usize, target: &Target) -> Option<String> {
    let row = match target {
        Target::Row(row) => Some(row.said(count, false)),
        Target::NoResults | Target::Nothing => None,
    };
    match opening {
        Opening::Silent => None,
        Opening::Selection => row,
        Opening::Named(name, noun) => {
            let opened = format!("{name}, {}", noun.counted(count));
            Some(match row {
                Some(row) => format!("{opened}. {row}"),
                None => opened,
            })
        }
    }
}

/// The name of the section row `index` is in, among `sections`.
pub(crate) fn section_at(sections: &[SectionLabel], index: usize) -> Option<String> {
    sections
        .iter()
        .rev()
        .find(|section| section.first <= index)
        .map(|section| section.label.to_string())
}

impl LauncherWindow {
    /// What the announcer follows in the launcher's own frame: the open
    /// Actions panel or footer menu, else the screen's list (`None` on a
    /// form or a custom view). `sections` labels the screen's rows, and
    /// `nothing_found` says root search shows its notice that nothing
    /// matched.
    pub(crate) fn followed_list(
        &self,
        view: &LauncherView,
        sections: &[SectionLabel],
        nothing_found: bool,
        cx: &App,
    ) -> Option<Listing> {
        self.panel_listing(cx)
            .or_else(|| self.menu_listing())
            .or_else(|| self.screen_listing(view, sections, nothing_found))
    }

    /// The screen's list as the announcer follows it: root search opens
    /// silently, every other list with its title and count.
    fn screen_listing(
        &self,
        view: &LauncherView,
        sections: &[SectionLabel],
        nothing_found: bool,
    ) -> Option<Listing> {
        let root = matches!(view.screen, Screen::Root { .. });
        let opening = match &view.screen {
            // Forms, custom views and a package's Logs screen (#213) keep
            // their own accessibility.
            Screen::Form(_) | Screen::CustomView(_) | Screen::ExtensionLog { .. } => return None,
            Screen::Root { .. } => Opening::Silent,
            _ => Opening::Named(view.title.clone(), Noun::Results),
        };
        let selected = view
            .selected
            .and_then(|index| Some((index, view.rows.get(index)?)));
        let target = match selected {
            Some((index, row)) => {
                // A computed answer is said as its card is named.
                let answer = if root {
                    self.launcher.present_row(index).answer
                } else {
                    None
                };
                Target::Row(Selected {
                    id: row.id.clone(),
                    title: answer.map_or_else(
                        || row.title.clone(),
                        |answer| root_search::layouts::answer_label(&answer),
                    ),
                    position: index + 1,
                    unavailable: row.unavailable.is_some(),
                    section: section_at(sections, index),
                })
            }
            None if view.rows.is_empty() || nothing_found => Target::NoResults,
            None => Target::Nothing,
        };
        Some(Listing {
            over: false,
            key: format!("{:?} {}", std::mem::discriminant(&view.screen), view.title),
            opening,
            count: view.rows.len(),
            target,
            query: view.search_field().map(str::to_owned),
            settled: self.announcer.settled(),
        })
    }

    /// The announcer's node for this frame, once it has followed `listing`
    /// and the footer's message `status`; the window draws again when
    /// something it says waits for its time.
    pub(crate) fn announce(
        &mut self,
        listing: Option<Listing>,
        status: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let now = cx.background_executor().now();
        let wake = self.announcer.frame(listing, status, now);
        if wake != self.announcer.wake_at {
            self.announcer.wake_at = wake;
            self.announcer._wake = wake.map(|at| {
                let wait = at.saturating_duration_since(now);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    this.update(cx, |this, cx| {
                        this.announcer.wake_at = None;
                        cx.notify();
                    })
                    .ok();
                })
            });
        }
        self.announcer.node()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Root search's list with `count` rows, row `selected` selected (from
    /// 0), for `query`.
    fn root(query: &str, count: usize, selected: Option<usize>) -> Listing {
        Listing {
            over: false,
            key: "root".into(),
            opening: Opening::Silent,
            count,
            target: match selected {
                Some(index) => Target::Row(row(index, None)),
                None if count == 0 => Target::NoResults,
                None => Target::Nothing,
            },
            query: Some(query.into()),
            settled: true,
        }
    }

    /// Row `index` (from 0), titled "Row <index>", in `section`.
    fn row(index: usize, section: Option<&str>) -> Selected {
        Selected {
            id: format!("row-{index}"),
            title: format!("Row {index}"),
            position: index + 1,
            unavailable: false,
            section: section.map(str::to_owned),
        }
    }

    /// The announcer after root search's first frame, at `now`.
    fn started(now: Instant) -> Announcer {
        let mut announcer = Announcer::default();
        announcer.frame(Some(root("", 3, Some(0))), None, now);
        announcer
    }

    #[test]
    fn root_search_opens_silently_and_a_move_is_said_with_its_place() {
        let now = Instant::now();
        let mut announcer = started(now);
        assert_eq!(announcer.text, "");

        announcer.user_moved();
        announcer.frame(Some(root("", 3, Some(1))), None, now);
        assert_eq!(announcer.text, "Row 1, 2 of 3");
    }

    #[test]
    fn an_unavailable_row_and_a_new_section_are_said() {
        let now = Instant::now();
        let mut announcer = started(now);
        let mut listing = root("", 3, None);
        listing.target = Target::Row(Selected {
            unavailable: true,
            ..row(2, Some("Fallbacks"))
        });
        announcer.user_moved();
        announcer.frame(Some(listing), None, now);
        assert_eq!(announcer.text, "Fallbacks: Row 2, 3 of 3, unavailable");
    }

    #[test]
    fn typing_waits_for_the_results_and_says_only_another_row() {
        let now = Instant::now();
        let mut announcer = started(now);

        // The same row stays first: nothing, however long.
        let wake = announcer.frame(Some(root("r", 2, Some(0))), None, now);
        assert_eq!(wake, Some(now + SETTLE));
        announcer.frame(Some(root("r", 2, Some(0))), None, now + SETTLE);
        assert_eq!(announcer.text, "");

        // Another first row: nothing until the results settle and the
        // time has run, then said once.
        let mut typed = root("ro", 2, Some(0));
        typed.target = Target::Row(Selected {
            id: "other".into(),
            ..row(0, None)
        });
        let later = now + SETTLE * 2;
        announcer.frame(Some(typed.clone()), None, later);
        let unsettled = Listing {
            settled: false,
            ..typed.clone()
        };
        announcer.frame(Some(unsettled), None, later + SETTLE);
        assert_eq!(announcer.text, "");
        announcer.frame(Some(typed.clone()), None, later + SETTLE);
        assert_eq!(announcer.text, "Row 0, 1 of 2");
        announcer.text.clear();
        announcer.frame(Some(typed), None, later + SETTLE * 2);
        assert_eq!(announcer.text, "", "said once");
    }

    #[test]
    fn a_move_while_typing_is_said_at_once() {
        let now = Instant::now();
        let mut announcer = started(now);
        announcer.frame(Some(root("r", 3, Some(0))), None, now);
        announcer.user_moved();
        announcer.frame(Some(root("r", 3, Some(1))), None, now);
        assert_eq!(announcer.text, "Row 1, 2 of 3");
    }

    #[test]
    fn pane_moving_the_same_row_says_nothing_and_another_row_is_said() {
        let now = Instant::now();
        let mut announcer = started(now);
        // The selected row moved down a place, as a late result above it
        // does: still the same row.
        let mut moved_down = root("", 4, Some(0));
        moved_down.target = Target::Row(Selected {
            position: 2,
            ..row(0, None)
        });
        announcer.frame(Some(moved_down), None, now);
        assert_eq!(announcer.text, "");
        announcer.frame(Some(root("", 4, Some(3))), None, now);
        assert_eq!(announcer.text, "Row 3, 4 of 4");
    }

    #[test]
    fn an_empty_list_says_no_results() {
        let now = Instant::now();
        let mut announcer = started(now);
        announcer.frame(Some(root("zzz", 0, None)), None, now);
        announcer.frame(Some(root("zzz", 0, None)), None, now + SETTLE);
        assert_eq!(announcer.text, NO_RESULTS);
    }

    #[test]
    fn a_list_opens_with_its_name_and_count_then_its_row() {
        let now = Instant::now();
        let mut announcer = started(now);
        let command = Listing {
            key: "command".into(),
            opening: Opening::Named("Hello".into(), Noun::Results),
            query: None,
            ..root("", 7, Some(0))
        };
        announcer.frame(Some(command), None, now);
        assert_eq!(announcer.text, "Hello, 7 results. Row 0, 1 of 7");

        let panel = Listing {
            over: true,
            key: "panel".into(),
            opening: Opening::Named("Actions for Row 0".into(), Noun::Commands),
            query: Some(String::new()),
            ..root("", 1, Some(0))
        };
        announcer.frame(Some(panel), None, now);
        assert_eq!(
            announcer.text,
            "Actions for Row 0, 1 command. Row 0, 1 of 1"
        );
    }

    #[test]
    fn closing_a_panel_gives_the_screen_back_without_opening_it_again() {
        let now = Instant::now();
        let mut announcer = started(now);
        let panel = Listing {
            over: true,
            key: "panel".into(),
            opening: Opening::Named("Actions".into(), Noun::Commands),
            ..root("", 2, Some(0))
        };
        announcer.frame(Some(panel), None, now);
        announcer.text.clear();
        announcer.frame(Some(root("", 3, Some(0))), None, now);
        assert_eq!(announcer.text, "");
    }

    #[test]
    fn the_footers_message_is_said_before_the_selection_it_came_with() {
        let now = Instant::now();
        let mut announcer = started(now);
        announcer.user_moved();
        let wake = announcer.frame(Some(root("", 3, Some(1))), Some("Copied"), now);
        assert_eq!(announcer.text, "Copied");
        assert_eq!(wake, Some(now + STATUS_LEAD));
        let later = now + STATUS_LEAD;
        announcer.frame(Some(root("", 3, Some(1))), Some("Copied"), later);
        assert_eq!(announcer.text, "Row 1, 2 of 3");
        // The same message is not said again while it stays.
        announcer.user_moved();
        announcer.frame(Some(root("", 3, Some(2))), Some("Copied"), later);
        assert_eq!(announcer.text, "Row 2, 3 of 3");
    }

    #[test]
    fn fast_moves_leave_only_the_last_row() {
        let now = Instant::now();
        let mut announcer = started(now);
        for index in 1..3 {
            announcer.user_moved();
            announcer.frame(Some(root("", 3, Some(index))), None, now);
        }
        assert_eq!(announcer.text, "Row 2, 3 of 3");
    }
}
