//! The selected result's actions: what the launcher's Actions panel lists
//! for root search's selected row, and the flows its entries open.
//!
//! The list holds only what Pane can do for that row now: its primary
//! action (the footer's, the same definition and dispatch), then, for an
//! installed command, the hotkey and alias configuration Manage
//! extensions already offers. Nothing is listed that has no working
//! operation behind it (#100): no quit, new window or hide, and no pin
//! until quick slots exist.
//!
//! An alias or hotkey flow opened here returns to the search it came from
//! — the same rows, the target still selected, the outcome in the status —
//! where the same flows opened from Manage extensions return there.

use super::shortcuts;
use super::{Entry, Launcher, LauncherView, Screen, State, selected_action};

/// One kind of action on a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultAction {
    /// The result's primary action: what the footer's button and Enter do.
    Invoke,
    /// Records or changes the command's global hotkey.
    Hotkey,
    /// Sets or changes the command's alias.
    Alias,
}

impl ResultAction {
    /// The action's name in records and reports: "invoke", "hotkey",
    /// "alias".
    pub fn id(self) -> &'static str {
        match self {
            ResultAction::Invoke => "invoke",
            ResultAction::Hotkey => "hotkey",
            ResultAction::Alias => "alias",
        }
    }

    /// A configuration entry's label, by whether the command already has
    /// that configuration: "Assign Hotkey…" or "Change Hotkey…", "Add
    /// Alias…" or "Change Alias…". `None` for [`ResultAction::Invoke`],
    /// which is named by the result's own action.
    pub fn configuration_label(self, configured: bool) -> Option<&'static str> {
        match (self, configured) {
            (ResultAction::Invoke, _) => None,
            (ResultAction::Hotkey, false) => Some("Assign Hotkey…"),
            (ResultAction::Hotkey, true) => Some("Change Hotkey…"),
            (ResultAction::Alias, false) => Some("Add Alias…"),
            (ResultAction::Alias, true) => Some("Change Alias…"),
        }
    }
}

/// One entry of the Actions panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultActionItem {
    pub action: ResultAction,
    /// What the entry says: "Open command", "Assign Hotkey…".
    pub label: String,
    /// Whether it can run now; the primary action of an unavailable result
    /// cannot.
    pub available: bool,
}

/// The actions of root search's selected row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultActions {
    /// The row's stable id: the target the panel opened for, which must
    /// still be the selected row when an entry runs.
    pub target: String,
    /// The row's title, as the panel's header names it.
    pub title: String,
    /// The primary action first, then the command's configuration.
    pub items: Vec<ResultActionItem>,
}

impl ResultActions {
    /// The entries whose label holds `query`, ignoring case and the
    /// spaces around it; all of them for a blank one.
    pub fn matching(&self, query: &str) -> Vec<&ResultActionItem> {
        let query = query.trim().to_lowercase();
        self.items
            .iter()
            .filter(|item| item.label.to_lowercase().contains(&query))
            .collect()
    }
}

/// The search to restore when an alias or hotkey flow opened from the
/// Actions panel ends.
pub(super) struct Return {
    view: LauncherView,
    entries: Vec<Entry>,
    /// The flow screen's epoch: a flow left any other way (Return to
    /// root, a hotkey pressed) moved past it, and this is stale then.
    epoch: u64,
}

impl Launcher {
    /// The actions of root search's selected row, or `None` off root
    /// search or with nothing selected. See the module docs for what is
    /// listed.
    pub fn result_actions(&self) -> Option<ResultActions> {
        let state = self.lock();
        result_actions(self, &state)
    }

    /// Whether `action` can run on `target` now: `target` is still root
    /// search's selected row, and the action is listed for it and
    /// available.
    pub fn result_action_ready(&self, target: &str, action: ResultAction) -> bool {
        let state = self.lock();
        ready(self, &state, target, action)
    }

    /// Opens the hotkey screen or the alias form of `target`, an installed
    /// command, when the action is [ready](Launcher::result_action_ready);
    /// the flow returns to this search when it ends. Whether it opened:
    /// nothing changes otherwise, and [`ResultAction::Invoke`] is never
    /// opened here — it is the window's primary action.
    pub fn open_result_action(&self, target: &str, action: ResultAction) -> bool {
        let mut state = self.lock();
        if action == ResultAction::Invoke || !ready(self, &state, target, action) {
            return false;
        }
        let view = state.view.clone();
        let entries = state.entries.clone();
        match action {
            ResultAction::Hotkey => self.show_hotkey(&mut state, target),
            ResultAction::Alias => self.show_alias_form(&mut state, target),
            ResultAction::Invoke => unreachable!("not opened here"),
        }
        // Set after the flow opened: opening it clears what a visit from
        // Manage extensions would otherwise inherit.
        state.actions_return = Some(Return {
            view,
            entries,
            epoch: state.screen_epoch,
        });
        true
    }

    /// Ends an alias or hotkey flow back on the search the Actions panel
    /// opened it from, if it did and the flow is still the screen it
    /// opened: whether it did.
    pub(super) fn return_from_actions_flow(&self, state: &mut State) -> bool {
        let Some(back) = state
            .actions_return
            .take()
            .filter(|back| back.epoch == state.screen_epoch)
        else {
            return false;
        };
        state.next_screen();
        state.entries = back.entries;
        state.view = back.view;
        true
    }
}

fn result_actions(launcher: &Launcher, state: &State) -> Option<ResultActions> {
    if !matches!(state.view.screen, Screen::Root { .. }) {
        return None;
    }
    let index = state.view.selected?;
    let row = state.view.rows.get(index)?;
    let primary = selected_action(state);
    let mut items = vec![ResultActionItem {
        action: ResultAction::Invoke,
        label: primary.label,
        available: primary.available,
    }];
    // An installed command's own row — not one that sends text through
    // an alias, and not this build's samples, which take no configuration.
    let command = matches!(
        state.entries.get(index),
        Some(Entry::Open(_) | Entry::Unavailable(_))
    )
    .then(|| {
        shortcuts::catalog(launcher, state)
            .groups
            .into_iter()
            .flat_map(|group| group.commands)
            .find(|command| command.id == row.id)
    })
    .flatten();
    if let Some(command) = command {
        if command.hotkey_editable {
            items.push(configuration(
                ResultAction::Hotkey,
                command.hotkey.is_some(),
            ));
        }
        if command.editable {
            items.push(configuration(ResultAction::Alias, command.alias.is_some()));
        }
    }
    Some(ResultActions {
        target: row.id.clone(),
        title: row.title.clone(),
        items,
    })
}

/// A configuration entry, which can always be opened while listed.
fn configuration(action: ResultAction, configured: bool) -> ResultActionItem {
    ResultActionItem {
        action,
        label: action
            .configuration_label(configured)
            .expect("a configuration entry")
            .to_owned(),
        available: true,
    }
}

fn ready(launcher: &Launcher, state: &State, target: &str, action: ResultAction) -> bool {
    result_actions(launcher, state).is_some_and(|actions| {
        actions.target == target
            && actions
                .items
                .iter()
                .any(|item| item.action == action && item.available)
    })
}
