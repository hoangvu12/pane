//! A designed view's tree, drawn with GPUI's flex layout: the layout
//! primitives, the shared UI components, and the tokens and raw values
//! the tree styles with, resolved onto Pane's theme in light and dark
//! (`docs/designed-tree.md`, ADR 0036, #237). The tree the launcher holds
//! is typed ([`pane_core::DesignedTree`]); the window never meets the
//! JSON the extension answered with. Layout is computed here, whole: a
//! `column` and a `row` are flex containers with the space tokens
//! resolved onto Pane's spacing rhythm, a `stack` places its children
//! over each other, a `scroll` scrolls, and the shared components draw
//! from Pane's own control families (`ui::controls`, `ui::keycap`,
//! `ui::extension_icon`), so an extension's screen is Pane's design, not
//! a copy of it. Every node may carry a [`Style`] — sizing and a surface
//! with `hover` and `pressed` variants Pane applies itself, never asking
//! the extension.
//!
//! What the window keeps for the tree is kept by key (#238,
//! [`reconcile`]): the focus of each focusable control, the editing state
//! of each text field — the text, the caret, the selection, the input
//! method's composition — the select's own open state, and each scroll
//! region's position, so a re-render that still draws a node keeps what
//! the user was doing with it. The fields are partially controlled: they
//! edit at once, their input events are coalesced to the latest while
//! one is in flight (and throttled, when a field asks), their commits go
//! out on Enter and blur, and a value the extension sets — one that
//! differs from the node's value in the extension's previous render —
//! replaces the text.
//!
//! The interactive components are focusable, in tree order: Tab and
//! Shift+Tab move through them, and Enter and Space press the focused
//! one (bound in [`bind_keys`], under each control's key context, which
//! sits below the launcher's window context). Escape stays with Pane, as
//! it does for a custom view: it pops the navigation stack (#239),
//! leaving the screen when only the root view is on it. Events are
//! raised on the tree the user saw — each control carries the render it
//! was drawn from, and a node's handler still named by the view's tree
//! receives the event even after a later render, while one that went is
//! dropped (the launcher's stale-event rule).
//!
//! Pressing a control hands its callback id to the launcher
//! ([`Launcher::send_designed_seen`]), which sends the event to the view
//! and shows the tree it answers with; the extension's state lives in
//! its view, and the whole screen is redrawn from the answer.

mod components;
mod fields;
mod markdown;
mod reconcile;
mod tree;

use gpui::prelude::*;
use gpui::{App, Context, KeyBinding, PathPromptOptions, actions, div, px};

use gpui_elements::editable_text::EditableTextState;
use pane_core::{DesignedHandler, DesignedViewSnapshot};

use crate::app::LauncherWindow;

use reconcile::{FieldEvents, Held};
use tree::Draw;

pub(crate) use reconcile::DesignedControls;

/// The key context of a designed view's button and link: Enter and Space
/// press them there, below the launcher's window context (whose Escape
/// and Tab bubble).
const BUTTON_CONTEXT: &str = "DesignedButton";
/// The key context of a designed view's toggle and checkbox: Enter and
/// Space change them.
const TOGGLE_CONTEXT: &str = "DesignedToggle";
/// The key context of a designed view's checkbox.
const CHECKBOX_CONTEXT: &str = "DesignedCheckbox";
/// The key context of a designed view's segmented control: its arrows
/// move the choice.
const SEGMENTED_CONTEXT: &str = "DesignedSegmented";
/// The key context of a designed view's slider: its arrows adjust it.
const SLIDER_CONTEXT: &str = "DesignedSlider";
/// The key context of a designed view's text fields: Enter commits.
const INPUT_CONTEXT: &str = "DesignedInput";
/// The key context of a designed view's text areas: Enter inserts a
/// newline, as a text area's does (the commit is a blur's; #241's forms
/// bind Ctrl+Enter to submit).
const AREA_CONTEXT: &str = "DesignedTextArea";
/// The key context of a designed view's select: the searchable select's
/// trigger takes its keys from `ui::select`'s bindings.
const SELECT_CONTEXT: &str = "DesignedSelect";
/// The key context of a designed form's date and date-time fields: their
/// arrows step the date.
const DATE_CONTEXT: &str = "DesignedDate";
/// The key context of a designed form's tag picker: Enter commits the
/// highlighted option.
const TAGS_CONTEXT: &str = "DesignedTags";
/// The key context of a designed form's picker of many paths: Enter opens
/// the system's dialog.
const PATHS_CONTEXT: &str = "DesignedPaths";

actions!(
    designed,
    [
        Press,
        Toggle,
        Move,
        Adjust,
        Commit,
        SubmitForm,
        StepDateUp,
        StepDateDown,
        CommitTag,
        ChoosePath
    ]
);

/// Registers the designed view's key bindings: Enter and Space press a
/// button or a link, change a toggle or a checkbox; a segmented
/// control's and a slider's arrows move them; Enter commits a text
/// field; Enter in a text area inserts a newline. A form's keys submit:
/// Ctrl+Enter in any field (#241), and a date field's arrows step the
/// date, a tag picker's Enter commits its highlighted option, a picker
/// of many paths' Enter opens the system's dialog.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Press, Some(BUTTON_CONTEXT)),
        KeyBinding::new("space", Press, Some(BUTTON_CONTEXT)),
        KeyBinding::new("enter", Toggle, Some(TOGGLE_CONTEXT)),
        KeyBinding::new("space", Toggle, Some(TOGGLE_CONTEXT)),
        KeyBinding::new("enter", Toggle, Some(CHECKBOX_CONTEXT)),
        KeyBinding::new("space", Toggle, Some(CHECKBOX_CONTEXT)),
        KeyBinding::new("left", Move, Some(SEGMENTED_CONTEXT)),
        KeyBinding::new("right", Move, Some(SEGMENTED_CONTEXT)),
        KeyBinding::new("left", Adjust, Some(SLIDER_CONTEXT)),
        KeyBinding::new("right", Adjust, Some(SLIDER_CONTEXT)),
        KeyBinding::new("enter", Commit, Some(INPUT_CONTEXT)),
        KeyBinding::new("enter", Commit, Some(SELECT_CONTEXT)),
        KeyBinding::new("space", Commit, Some(SELECT_CONTEXT)),
        KeyBinding::new(
            "enter",
            gpui_elements::editable_text::actions::Enter,
            Some(AREA_CONTEXT),
        ),
        KeyBinding::new("ctrl-enter", SubmitForm, Some(INPUT_CONTEXT)),
        KeyBinding::new("ctrl-enter", SubmitForm, Some(AREA_CONTEXT)),
        KeyBinding::new("up", StepDateUp, Some(DATE_CONTEXT)),
        KeyBinding::new("down", StepDateDown, Some(DATE_CONTEXT)),
        KeyBinding::new("enter", CommitTag, Some(TAGS_CONTEXT)),
        KeyBinding::new("enter", ChoosePath, Some(PATHS_CONTEXT)),
    ]);
}

impl LauncherWindow {
    /// Creates or drops the designed view's controls to match the
    /// launcher's screen, and reconciles the keyed state with the tree it
    /// now shows: a newly opened view takes the keyboard on its first
    /// focusable control (or the one that asks for it), and focus returns
    /// to the list when the view closes. The tree's loaded icons are
    /// drawn first, as a list's rows are (#142).
    pub(crate) fn sync_designed_view(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.launcher.refresh_designed_view();
        // The screen alone: the tree changes with each answer, and the
        // keys of two trees of one view decide which state is kept.
        let shown = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some((view.id, view.render, view.tree)),
            _ => None,
        };
        let open = shown
            .as_ref()
            .is_some_and(|(id, _, _)| Some(*id) == self.designed.as_ref().map(|c| c.view));
        if open {
            // The same view: the keyed state is reconciled with the tree
            // it now shows, and a field's value placed under the
            // partially controlled rule.
            if let Some((_, render, tree)) = shown {
                let controls = self.designed.as_mut().expect("a view is open");
                controls.reconcile(&tree, render, window, cx);
            }
            return;
        }
        let closed = shown.is_none();
        self.designed = shown
            .map(|(view, render, tree)| DesignedControls::new(view, &tree, render, window, cx));
        if closed {
            window.focus(&self.focus_handle, cx);
        }
    }

    /// Test support: how many focusable controls the open designed view's
    /// keyed state holds.
    #[doc(hidden)]
    pub fn designed_button_count(&self) -> usize {
        self.designed
            .as_ref()
            .map(|controls| controls.state.len())
            .unwrap_or(0)
    }

    /// Test support: the editing state of the open designed view's field
    /// keyed `key`, which a platform input method talks to while
    /// composing text. GPUI CE's test platform cannot reach the window's
    /// input handler, so the window tests compose through this instead.
    #[doc(hidden)]
    pub fn designed_field(&self, key: &str) -> Option<gpui::Entity<EditableTextState>> {
        self.designed_field_at(key).map(|(entity, _, _)| entity)
    }

    /// Test support: the value the open designed view's field keyed `key`
    /// holds — its live text, what the user typed.
    #[doc(hidden)]
    pub fn designed_field_text(&self, key: &str, cx: &gpui::App) -> Option<String> {
        self.designed_field_at(key)
            .map(|(entity, _, _)| entity.read(cx).as_str().to_owned())
    }

    /// The editing state, the focus and the bookkeeping of the open
    /// designed view's field keyed `key`.
    fn designed_field_at(
        &self,
        key: &str,
    ) -> Option<(
        gpui::Entity<EditableTextState>,
        gpui::FocusHandle,
        &FieldEvents,
    )> {
        let controls = self.designed.as_ref()?;
        let (_, entry) = controls
            .state
            .iter()
            .find(|(path, _)| path.ends_with(&format!("/{key}")))?;
        match &entry.held {
            Held::Field {
                editing,
                focus,
                events,
            } => Some((editing.clone(), focus.clone(), events)),
            _ => None,
        }
    }

    /// Test support: focuses the open designed view's field keyed `key`,
    /// as the keyboard would.
    #[doc(hidden)]
    pub fn focus_designed_field(
        &mut self,
        key: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.designed_field_at(key).map(|(_, focus, _)| focus);
        if let Some(focus) = focus {
            window.focus(&focus, cx);
            cx.notify();
        }
    }

    /// Test support: how far the open designed view's scroll region keyed
    /// `key` is scrolled, from its top-left.
    #[doc(hidden)]
    pub fn designed_scroll(&self, key: &str) -> Option<gpui::Point<gpui::Pixels>> {
        let controls = self.designed.as_ref()?;
        let (_, entry) = controls
            .state
            .iter()
            .find(|(path, _)| path.ends_with(&format!("/{key}")))?;
        match &entry.held {
            Held::Scroll(handle) => Some(handle.offset()),
            _ => None,
        }
    }

    /// The designed view's fields' editing states, which a platform input
    /// method talks to while composing text — for the launcher's back
    /// key, which cancels an active composition before it acts.
    pub(crate) fn designed_text_fields(&self) -> Vec<gpui::Entity<EditableTextState>> {
        let Some(controls) = &self.designed else {
            return Vec::new();
        };
        controls
            .state
            .values()
            .filter_map(|entry| match &entry.held {
                Held::Field { editing, .. } => Some(editing.clone()),
                _ => None,
            })
            .collect()
    }

    /// Whether the open designed view holds the keyboard in a control
    /// that takes Backspace for itself — a field or a select, whose
    /// editing keys never reach the window's key handling, and whose
    /// other states (a select's trigger, its popup closed) are guarded
    /// here: Backspace pops the view only from what does not edit with
    /// it.
    pub(crate) fn designed_takes_backspace(&self, window: &gpui::Window) -> bool {
        let Some(controls) = &self.designed else {
            return false;
        };
        controls.state.values().any(|entry| match &entry.held {
            Held::Field { focus, .. } | Held::Select { focus, .. } => focus.is_focused(window),
            _ => false,
        })
    }

    /// The designed view screen for the launcher's snapshot of the view.
    pub(crate) fn render_designed_view(
        &self,
        view: DesignedViewSnapshot,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.render_designed_tree(&view.tree, view.render, cx)
    }

    /// A form Pane itself asks, as its tree says (#241): the same drawing
    /// as an extension's designed view, its submission routed to the
    /// launcher rather than the view.
    pub(crate) fn render_pane_form(
        &self,
        form: pane_core::PaneForm,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.render_designed_tree(&form.tree, 0, cx)
    }

    /// One designed tree, drawn with the keyed state the window keeps for
    /// it.
    fn render_designed_tree(
        &self,
        tree: &pane_core::DesignedTree,
        render: u64,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(controls) = &self.designed else {
            return div().into_any_element();
        };
        let visuals = crate::settings::launcher_visuals(cx);
        let theme = visuals.theme;
        let draw = Draw {
            theme: &theme,
            state: &controls.state,
            render,
            surface: theme.panel_solid,
        };
        let drawn = {
            let mut path = String::new();
            tree::push(&mut path, tree.root.key.as_deref(), 0);
            tree::draw_node(&tree.root, &mut path, draw, cx)
        };
        // Fills the body as the list and the form do, so the status line
        // stays at the bottom; the tree's own `scroll` regions scroll
        // inside it, the screen scrolling what grows past it otherwise.
        let geometry = &theme.geometry;
        div()
            .id("designed-view")
            .key_context("DesignedView")
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .items_start()
            .overflow_y_scroll()
            .px(geometry.search_padding_x)
            .pt(crate::ui::tokens::space(pane_core::Space::S))
            .pb(geometry.list_padding_bottom)
            .child(drawn)
            .into_any_element()
    }

    /// Submits the form the screen holds (#241): the values of its
    /// fields, collected from the state the window keeps for them, run
    /// the form's action — an extension form's `onSubmit`, with the
    /// values as its event's payload, or a form Pane itself asks, which
    /// the launcher answers. Enter in a single-line field of the form
    /// runs this, as Ctrl+Enter does in a text area and the form's submit
    /// button and the footer's primary action do.
    pub(crate) fn submit_designed_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((form, values)) = self.collect_form() else {
            return;
        };
        if matches!(self.launcher.screen(), pane_core::Screen::PaneForm(_)) {
            let values: Vec<(String, String)> = values
                .into_iter()
                .map(|(key, value)| (key, value.as_text()))
                .collect();
            let pending = self.launcher.submit_pane_form(values);
            self.show_until_done(pending, window, cx);
        } else {
            let pending = self.launcher.submit_designed_form(Some(&form), values);
            self.show_until_done(pending, window, cx);
        }
    }

    /// The form the screen holds, and the values its fields hold as a
    /// submission collects them: keyed by the fields' keys — the text a
    /// field edits, the state a checkbox or toggle last told the
    /// extension, the tags or paths a picker chose. `None` when no form
    /// is on screen.
    fn collect_form(&self) -> Option<(String, Vec<(String, pane_core::FormValue)>)> {
        let tree = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some(view.tree),
            pane_core::Screen::PaneForm(form) => Some(form.tree),
            _ => None,
        }?;
        let controls = self.designed.as_ref()?;
        let mut collected = Collected {
            form: None,
            values: Vec::new(),
        };
        let mut path = String::new();
        tree::push(&mut path, tree.root.key.as_deref(), 0);
        collect_node(
            &tree.root,
            None,
            &mut path,
            &controls.state,
            &controls.sent,
            &mut collected,
            cx,
        );
        let form = collected.form?;
        Some((form, collected.values))
    }

    /// Whether the field at `path` sits inside a form on this screen
    /// (#241): Enter in it submits the form.
    fn field_in_form(&self, path: &str) -> bool {
        let tree = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some(view.tree),
            pane_core::Screen::PaneForm(form) => Some(form.tree),
            _ => None,
        };
        tree.is_some_and(|tree| tree::inside_form(&tree, path))
    }

    /// The date field at `path`, stepped by `days` days (a date and time
    /// by as many minutes): its text rewritten, its caret at the end.
    /// Nothing steps a text that does not parse as the field's format.
    fn designed_date_stepped(
        &mut self,
        path: &str,
        minutes: i64,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let date_time = self.designed.as_ref().and_then(|controls| {
            controls.state.get(path).and_then(|entry| match &entry.held {
                Held::Field { editing, .. } => Some(editing.clone()),
                _ => None,
            })
        });
        let Some(editing) = date_time else {
            return;
        };
        let text = editing.read(cx).as_str().to_owned();
        let Some(stepped) = fields::step_date(&text, minutes) else {
            return;
        };
        let len = stepped.len();
        editing.update(cx, |editing, cx| {
            editing.emplace(&stepped, cx);
            editing.move_to(len, cx);
        });
        let _ = window;
        cx.notify();
    }

    /// Adds the tag `value` to the tag picker at `path`, telling the
    /// extension what it now holds.
    fn designed_tag_added(
        &mut self,
        path: &str,
        value: &str,
        callback: Option<u32>,
        key: &str,
        seen: u64,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let tags = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let Held::Tags { chosen, .. } = &mut entry.held else {
                return None;
            };
            if chosen.iter().any(|chosen| chosen == value) {
                return None;
            }
            chosen.push(value.to_owned());
            Some(chosen.clone())
        });
        let Some(tags) = tags else {
            return;
        };
        self.designed_tags_changed(path, tags, callback, key, seen, window, cx);
    }

    /// Removes the tag `value` from the tag picker at `path`, telling the
    /// extension what it now holds.
    fn designed_tag_removed(
        &mut self,
        path: &str,
        value: &str,
        callback: Option<u32>,
        key: &str,
        seen: u64,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let tags = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let Held::Tags { chosen, .. } = &mut entry.held else {
                return None;
            };
            chosen.retain(|chosen| chosen != value);
            Some(chosen.clone())
        });
        let Some(tags) = tags else {
            return;
        };
        self.designed_tags_changed(path, tags, callback, key, seen, window, cx);
    }

    /// The tag picker at `path`'s query's Enter: the highlighted option is
    /// added, the query cleared.
    fn designed_tag_committed(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some((value, callback, key, seen)) = self.designed.as_ref().and_then(|controls| {
            let entry = controls.state.get(path)?;
            let Held::Tags {
                query, highlighted, ..
            } = &entry.held
            else {
                return None;
            };
            let text = query.read(cx).as_str().to_owned();
            let tree = match self.launcher.screen() {
                pane_core::Screen::DesignedView(view) => Some(view.tree),
                pane_core::Screen::PaneForm(form) => Some(form.tree),
                _ => None,
            }?;
            let picker = tree::node_at(&tree, path)?;
            let pane_core::NodeKind::TagPicker(picker) = &picker.kind else {
                return None;
            };
            let options = fields::matching(&picker.options, &text);
            let at = highlighted.filter(|at| *at < options.len()).unwrap_or(0);
            let chosen = options.get(at).map(|option| option.value.clone())?;
            let (callback, key, seen) = (picker.on_change, picker.key.clone()?, entry.render);
            Some((chosen, callback, key, seen))
        })
        else {
            return;
        };
        let query = self.designed.as_ref().and_then(|controls| {
            controls.state.get(path).and_then(|entry| match &entry.held {
                Held::Tags { query, .. } => Some(query.clone()),
                _ => None,
            })
        });
        if let Some(query) = query {
            query.update(cx, |query, cx| query.emplace("", cx));
        }
        self.designed_tag_added(path, &value, callback, &key, seen, window, cx);
    }

    /// Tells the extension the tag picker at `path` now holds `tags`.
    fn designed_tags_changed(
        &mut self,
        path: &str,
        tags: Vec<String>,
        callback: Option<u32>,
        key: &str,
        seen: u64,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let _ = path;
        cx.notify();
        let Some(callback) = callback else {
            return;
        };
        let payload = list_payload(&tags);
        self.designed_event(
            DesignedHandler::Change,
            callback,
            key.to_owned(),
            seen,
            payload,
            window,
            cx,
        );
    }

    /// Removes the last path of the picker of many at `path`.
    fn designed_path_removed(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let _ = window;
        let removed = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let Held::Paths { paths, .. } = &mut entry.held else {
                return None;
            };
            (!paths.is_empty()).then(|| paths.pop().expect("just checked"))
        });
        if removed.is_some() {
            cx.notify();
        }
    }

    /// The system's dialog for the picker at `path`, choosing what `pick`
    /// asks: the path (or paths, for one that allows many) fills the
    /// field.
    fn designed_paths_chosen(
        &mut self,
        path: &str,
        key: &str,
        pick: PathPick,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: pick.files,
            directories: pick.directories,
            multiple: pick.multiple,
            prompt: Some("Choose".into()),
        });
        let (path, key) = (path.to_owned(), key.to_owned());
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                this.designed_paths_arrived(&path, &key, paths, window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The paths the system's dialog chose, filling the picker at `path`:
    /// one path fills the field's text, as typing it is; several join the
    /// picker's chips, telling the extension what it now holds.
    fn designed_paths_arrived(
        &mut self,
        path: &str,
        key: &str,
        paths: Vec<std::path::PathBuf>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let single = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            match &mut entry.held {
                Held::Field { editing, .. } => Some(editing.clone()),
                _ => None,
            }
        });
        if let (Some(editing), Some(picked)) = (single, paths.into_iter().next()) {
            let text = picked.to_string_lossy().into_owned();
            let len = text.len();
            editing.update(cx, |editing, cx| {
                editing.emplace(&text, cx);
                editing.move_to(len, cx);
            });
            cx.notify();
            return;
        }
        let picked = self
            .designed
            .as_mut()
            .and_then(|controls| {
                let entry = controls.state.get_mut(path)?;
                let Held::Paths { paths, .. } = &mut entry.held else {
                    return None;
                };
                let before = paths.len();
                for arrived in &paths {
                    let text = arrived.to_string_lossy().into_owned();
                    if !paths.contains(&text) {
                        paths.push(text);
                    }
                }
                (paths.len() != before).then(|| paths.clone())
            });
        if let Some(picked) = picked
            && let Some((callback, seen)) = self.designed.as_ref().and_then(|controls| {
                let entry = controls.state.get(path)?;
                match &entry.held {
                    Held::Paths { .. } => Some((path_node_of(self, path)?.on_change, entry.render)),
                    _ => None,
                }
            })
            && let Some(callback) = callback
        {
            let payload = list_payload(&picked);
            self.designed_event(
                DesignedHandler::Change,
                callback,
                key.to_owned(),
                seen,
                payload,
                window,
                cx,
            );
            return;
        }
        cx.notify();
    }

    /// Sends one event of the open designed view — the callback id its
    /// tree named, of `handler`'s kind, raised on the node with `key` (or
    /// none, when the tree gave it none), on the tree of the render
    /// `seen` (the tree the user saw the node in) — and redraws when its
    /// answer arrives.
    fn designed_event(
        &mut self,
        handler: DesignedHandler,
        callback: u32,
        key: String,
        seen: u64,
        payload: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        // A form Pane itself asks is answered by the launcher: its
        // controls' changes move into the tree the screen holds, as an
        // extension's change moves into the tree its answer holds
        // (#241).
        if handler == DesignedHandler::Change
            && matches!(self.launcher.screen(), pane_core::Screen::PaneForm(_))
        {
            if let Some(key) = (!key.is_empty()).then_some(key) {
                self.note_sent(&key, payload_value(&payload));
                self.launcher.pane_form_changed(&key, &payload_value(&payload));
                cx.notify();
            }
            return;
        }
        let key = (!key.is_empty()).then_some(key);
        if handler == DesignedHandler::Change {
            if let Some(named) = key.as_deref() {
                self.note_sent(named, &payload_value(&payload));
            }
        }
        let pending = self.launcher.send_designed_seen(
            handler,
            callback,
            key.as_deref(),
            Some(seen),
            payload,
        );
        self.show_until_done(pending, window, cx);
    }

    /// Notes the value a discrete control reported, so a submission reads
    /// it while the tree that carries it has not landed (#241).
    fn note_sent(&mut self, key: &str, value: String) {
        if let Some(controls) = self.designed.as_mut() {
            controls.sent.insert(key.to_owned(), value);
        }
    }

    /// The field at `path`'s text changed, as the user typed or composed
    /// it: the frame redraws its live value, and the input event its tree
    /// asked for is sent — coalesced to the latest while one is in
    /// flight, and held to the throttle's time when the field asks for
    /// one.
    fn designed_input_changed(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let changed = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let Held::Field {
                events, editing, ..
            } = &mut entry.held
            else {
                return None;
            };
            if events.placing {
                // The extension set the value; its own echo is not the
                // user's typing.
                events.placing = false;
                return None;
            }
            if events.on_input.is_none() {
                return None;
            }
            events.pending = Some(editing.read(cx).as_str().to_owned());
            Some(())
        });
        if changed.is_some() {
            self.flush_designed_input(path, window, cx);
        }
    }

    /// Sends the field at `path`'s waiting input event, if its state
    /// allows one now: none while another is in flight (the waiting value
    /// replaces whatever waited before, and goes out when the in-flight
    /// event's answer lands), and none before the field's throttle time
    /// has passed, when it asks for one — the timer armed here fires then
    /// and sends it.
    fn flush_designed_input(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let send = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let (render, key) = (entry.render, entry.key.clone());
            let Held::Field { events, .. } = &mut entry.held else {
                return None;
            };
            if events.in_flight {
                return None;
            }
            let value = events.pending.clone()?;
            if let (Some(sent), Some(throttle)) = (events.sent, events.throttle) {
                let elapsed = cx.background_executor().now().duration_since(sent);
                if elapsed < throttle {
                    // Held to the throttle's time; one timer, armed once,
                    // sends whatever is waiting when it fires.
                    if !events.armed {
                        events.armed = true;
                        let waits = throttle - elapsed;
                        let path = path.to_owned();
                        cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor().timer(waits).await;
                            let _ = this.update_in(cx, |this, window, cx| {
                                this.designed_input_timer(&path, window, cx);
                            });
                        })
                        .detach();
                    }
                    return None;
                }
            }
            events.pending = None;
            events.in_flight = true;
            events.sent = Some(cx.background_executor().now());
            events.reported(&value);
            Some((events.on_input?, render, key, value))
        });
        let Some((callback, render, key, value)) = send else {
            return;
        };
        let payload = payload(&value);
        let pending = self.launcher.send_designed_seen(
            DesignedHandler::Input,
            callback,
            (!key.is_empty()).then_some(key.as_str()),
            Some(render),
            payload,
        );
        // The answer's arrival clears the way for the waiting value, if
        // the user typed one while this event was in flight.
        let path = path.to_owned();
        cx.spawn_in(window, async move |this, cx| {
            pending.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.designed_input_answered(&path, window, cx);
            });
        })
        .detach();
    }

    /// The throttle's time passed: whatever input value was held goes out
    /// now, if its state allows one.
    fn designed_input_timer(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(controls) = self.designed.as_mut() {
            if let Some(entry) = controls.state.get_mut(path) {
                if let Held::Field { events, .. } = &mut entry.held {
                    events.armed = false;
                }
            }
        }
        self.flush_designed_input(path, window, cx);
    }

    /// The in-flight input event of the field at `path` was answered: the
    /// tree its answer drew is reconciled (a value it named is placed),
    /// and the value waiting behind it goes out now.
    fn designed_input_answered(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(controls) = self.designed.as_mut() {
            if let Some(entry) = controls.state.get_mut(path) {
                if let Held::Field { events, .. } = &mut entry.held {
                    events.in_flight = false;
                }
            }
        }
        self.sync_screen(window, cx);
        cx.notify();
        self.flush_designed_input(path, window, cx);
    }

    /// Commits the field at `path`, as Enter and a blur do: its value, if
    /// it moved since the last commit, is told to the extension through
    /// the change handler its tree names, and any input waiting to be
    /// told goes with it — the commit carries the newest value.
    fn designed_field_committed(
        &mut self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        // Enter in a single-line field of a form submits it (#241): the
        // form's primary action, not the field's own commit — the
        // submission carries the field's value.
        if self.field_in_form(path) {
            self.submit_designed_form(window, cx);
            return;
        }
        let send = self.designed.as_mut().and_then(|controls| {
            let entry = controls.state.get_mut(path)?;
            let (render, key) = (entry.render, entry.key.clone());
            let Held::Field {
                events, editing, ..
            } = &mut entry.held
            else {
                return None;
            };
            let live = editing.read(cx).as_str().to_owned();
            if live == events.committed {
                return None;
            }
            events.committed = live.clone();
            events.reported(&live);
            // The commit carries the newest value; the input waiting
            // behind it would tell an older one.
            events.pending = None;
            Some((events.on_change, render, key, live))
        });
        let Some((Some(callback), render, key, value)) = send else {
            return;
        };
        let payload = payload(&value);
        self.designed_event(
            DesignedHandler::Change,
            callback,
            key,
            render,
            payload,
            window,
            cx,
        );
    }

    /// A focus or blur of the node at `path`: a field's blur commits it
    /// (as Enter does), and the handler its tree names is raised.
    fn designed_focus_event(
        &mut self,
        path: &str,
        focused: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if !focused {
            self.designed_field_committed(path, window, cx);
        }
        let send = self.designed.as_ref().and_then(|controls| {
            let entry = controls.state.get(path)?;
            let watching = entry.watching.as_ref()?;
            let callback = if focused {
                watching.on_focus
            } else {
                watching.on_blur
            }?;
            Some((callback, entry.render, entry.key.clone()))
        });
        let Some((callback, render, key)) = send else {
            return;
        };
        let handler = if focused {
            DesignedHandler::Focus
        } else {
            DesignedHandler::Blur
        };
        self.designed_event(handler, callback, key, render, "{}".into(), window, cx);
    }

    /// A keystroke over the designed view, for the focused node that asks
    /// for key events: the key, as a key sequence spells it, is told to
    /// its handler. Tab, Enter and Escape stay with Pane, and so does any
    /// key that acted (an action's binding matched it); a field's typing
    /// is its text, not its keys.
    fn designed_key_event(
        &mut self,
        event: &gpui::KeystrokeEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if event.action.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        if matches!(keystroke.key.as_str(), "tab" | "enter" | "escape") {
            return;
        }
        let send = self.designed.as_ref().and_then(|controls| {
            controls
                .state
                .iter()
                .find(|(_, entry)| {
                    entry
                        .focus_handle()
                        .is_some_and(|handle| handle.is_focused(window))
                })
                .and_then(|(_, entry)| {
                    let callback = entry.on_key?;
                    // A field's typing is its text; its keys are the ones
                    // that do not type.
                    let field = matches!(entry.held, Held::Field { .. });
                    (!field || keystroke.key_char.is_none())
                        .then(|| (callback, entry.render, entry.key.clone()))
                })
        });
        let Some((callback, render, key)) = send else {
            return;
        };
        let payload = key_payload(keystroke);
        self.designed_event(
            DesignedHandler::Key,
            callback,
            key,
            render,
            payload,
            window,
            cx,
        );
    }

    /// The select at `path`'s choice was committed by its user: the
    /// change handler its tree names is told the value chosen.
    fn designed_select_committed(
        &mut self,
        path: &str,
        value: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let send = self.designed.as_ref().and_then(|controls| {
            let entry = controls.state.get(path)?;
            let Held::Select { on_change, .. } = &entry.held else {
                return None;
            };
            let callback = *on_change;
            Some((callback?, entry.render, entry.key.clone()))
        });
        let Some((callback, render, key)) = send else {
            return;
        };
        let payload = payload(value);
        self.designed_event(
            DesignedHandler::Change,
            callback,
            key,
            render,
            payload,
            window,
            cx,
        );
    }

    /// The select at `path`'s popup query, as the user typed it into a
    /// dropdown whose search the extension handles (#241): the query is
    /// told to the extension through the input handler its tree names,
    /// its answer replacing the choices the popup shows.
    fn designed_select_queried(
        &mut self,
        path: &str,
        text: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let send = self.designed.as_ref().and_then(|controls| {
            let entry = controls.state.get(path)?;
            let Held::Select { on_input, .. } = &entry.held else {
                return None;
            };
            Some((*on_input?, entry.render, entry.key.clone()))
        });
        let Some((callback, render, key)) = send else {
            return;
        };
        let payload = payload(text);
        self.designed_event(
            DesignedHandler::Input,
            callback,
            key,
            render,
            payload,
            window,
            cx,
        );
    }

    /// The select at `path`'s model, read live each frame it draws: the
    /// choices and the choice the tree on screen names.
    fn designed_select_model(&self, path: &str, cx: &gpui::App) -> crate::ui::select::Model {
        let visuals = crate::settings::launcher_visuals(cx);
        let tree = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some(view.tree),
            pane_core::Screen::PaneForm(form) => Some(form.tree),
            _ => None,
        };
        let select = tree
            .as_ref()
            .and_then(|tree| tree::node_at(tree, path))
            .and_then(|node| match &node.kind {
                pane_core::NodeKind::Select(select) => Some(select),
                _ => None,
            });
        let (choices, committed) = select
            .map(|select| {
                (
                    select
                        .options
                        .iter()
                        .map(|option| crate::ui::select::Choice {
                            id: option.value.clone().into(),
                            label: option
                                .label
                                .clone()
                                .unwrap_or_else(|| option.value.clone())
                                .into(),
                            subtitle: None,
                            keywords: Vec::new(),
                            unavailable_reason: None,
                            section: option.section.clone().map(gpui::SharedString::from),
                        })
                        .collect::<Vec<_>>(),
                    select.value.clone().map(gpui::SharedString::from),
                )
            })
            .unwrap_or_default();
        crate::ui::select::Model {
            theme: visuals.theme,
            material: visuals.material,
            choices,
            committed,
        }
    }
}

/// What the system's dialog for a designed form's picker chooses: files
/// or folders (a macOS application is a folder, its bundle), one path or
/// several (#241).
#[derive(Clone, Copy)]
pub(super) struct PathPick {
    pub(super) files: bool,
    pub(super) directories: bool,
    pub(super) multiple: bool,
}

/// What a form's collection found: the form's key and each field's
/// value, keyed by its key.
struct Collected {
    form: Option<String>,
    values: Vec<(String, pane_core::FormValue)>,
}

/// Collects the values of the fields under `node` — the form `form`
/// holds, when one is given — into `collected`, with the keyed state the
/// window keeps for them. A field without a key is not collected: its
/// value has nowhere to be named by.
fn collect_node(
    node: &Node,
    form: Option<&str>,
    path: &mut String,
    state: &std::collections::HashMap<String, reconcile::KeyedState>,
    sent: &std::collections::HashMap<String, String>,
    collected: &mut Collected,
    cx: &gpui::App,
) {
    let form = match &node.kind {
        pane_core::NodeKind::Form(_) => node.key.as_deref().or(Some("form")),
        _ => form,
    };
    if form.is_some()
        && let Some(key) = node.key.clone()
        && let Some(value) = field_value(node, path, state, sent, cx)
    {
        collected.values.push((key, value));
    }
    if let pane_core::NodeKind::Form(_) = &node.kind
        && collected.form.is_none()
    {
        collected.form = Some(node.key.clone().unwrap_or_else(|| "form".into()));
    }
    let duplicates = tree::duplicate_keys(node);
    for (index, child) in node.children.iter().enumerate() {
        let start = path.len();
        tree::place_child(path, child, index, &duplicates);
        collect_node(child, form, path, state, sent, collected, cx);
        path.truncate(start);
    }
}

/// The value the field `node` holds, from the state the window keeps for
/// it: the text a field edits, the state a checkbox or toggle last told
/// the extension (until the tree carries it), the choice a dropdown
/// last committed, the tags or paths a picker chose.
fn field_value(
    node: &Node,
    path: &str,
    state: &std::collections::HashMap<String, reconcile::KeyedState>,
    sent: &std::collections::HashMap<String, String>,
    cx: &gpui::App,
) -> Option<pane_core::FormValue> {
    use pane_core::FormValue;
    let key = node.key.as_deref()?;
    let text = state.get(path).and_then(|entry| match &entry.held {
        Held::Field { editing, .. } => Some(editing.read(cx).as_str().to_owned()),
        _ => None,
    });
    Some(match &node.kind {
        pane_core::NodeKind::TextInput(_)
        | pane_core::NodeKind::PasswordInput(_)
        | pane_core::NodeKind::TextArea(_) => FormValue::Text(text?),
        pane_core::NodeKind::DatePicker(date) | pane_core::NodeKind::DateTimePicker(date) => {
            FormValue::Text(text.unwrap_or_else(|| date.value.clone()))
        }
        pane_core::NodeKind::Select(select) => FormValue::Text(
            sent.get(key)
                .cloned()
                .or_else(|| select.value.clone())
                .unwrap_or_default(),
        ),
        pane_core::NodeKind::Toggle(toggle) => FormValue::On(
            sent.get(key).map(|value| value == "true").unwrap_or(toggle.on),
        ),
        pane_core::NodeKind::Checkbox(checkbox) => FormValue::On(
            sent
                .get(key)
                .map(|value| value == "true")
                .unwrap_or(checkbox.checked),
        ),
        pane_core::NodeKind::TagPicker(picker) => {
            let tags = state.get(path).and_then(|entry| match &entry.held {
                Held::Tags { chosen, .. } => Some(chosen.clone()),
                _ => None,
            });
            FormValue::List(tags.unwrap_or_else(|| picker.tags.clone()))
        }
        pane_core::NodeKind::FilePicker(picker) | pane_core::NodeKind::FolderPicker(picker) => {
            if picker.multiple {
                let paths = state.get(path).and_then(|entry| match &entry.held {
                    Held::Paths { paths, .. } => Some(paths.clone()),
                    _ => None,
                });
                FormValue::List(paths.unwrap_or_else(|| picker.paths.clone()))
            } else {
                FormValue::Text(text?)
            }
        }
        _ => return None,
    })
}

/// A list as a change event's payload names it: `{"value": ["…", …]}`,
/// a picker's chosen tags or paths.
pub(super) fn list_payload(values: &[String]) -> String {
    let joined = values
        .iter()
        .map(|value| format!("\"{}\"", escape(value)))
        .collect::<Vec<String>>()
        .join(",");
    format!("{{\"value\":[{joined}]}}")
}

/// A string's JSON escapes, for a payload's list.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// A value as a change event's payload names it: `{"value": …}`, a string
/// escaped as JSON.
pub(super) fn payload(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                escaped.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => escaped.push(other),
        }
    }
    format!("{{\"value\":\"{escaped}\"}}")
}

/// A value as a change event's payload names it: `{"value": …}`, a
/// boolean or a number.
pub(super) fn plain_payload(value: impl std::fmt::Display) -> String {
    format!("{{\"value\":{value}}}")
}

/// The file or folder picker node at `path` of the screen's tree.
fn path_node_of(window: &LauncherWindow, path: &str) -> Option<&pane_core::FilePicker> {
    let tree = match window.launcher.screen() {
        pane_core::Screen::DesignedView(view) => Some(view.tree),
        pane_core::Screen::PaneForm(form) => Some(form.tree),
        _ => None,
    }?;
    match &tree::node_at(&tree, path)?.kind {
        pane_core::NodeKind::FilePicker(picker) | pane_core::NodeKind::FolderPicker(picker) => {
            Some(picker)
        }
        _ => None,
    }
}

/// The value a change event's payload names, as a string: a string as it
/// is, a boolean as `true` or `false`. A submission's collection reads
/// the same payload a control's change carries.
pub(super) fn payload_value(payload: &str) -> String {
    let value = payload
        .split_once("\"value\"")
        .map(|(_, rest)| rest.trim_start().trim_start_matches(':').trim_start())
        .unwrap_or("");
    if value.starts_with('"') {
        let rest = &value[1..];
        let mut result = String::new();
        let mut characters = rest.chars();
        while let Some(character) = characters.next() {
            match character {
                '"' => break,
                '\\' => match characters.next() {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some(escaped) => result.push(escaped),
                    None => {}
                },
                other => result.push(other),
            }
        }
        return result;
    }
    value
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric())
        .collect()
}

/// A key as a key event's payload names it: `{"key": …}`, the keystroke
/// as a key sequence spells it (its modifiers, then its key).
pub(super) fn key_payload(keystroke: &gpui::Keystroke) -> String {
    let mut keys: Vec<&str> = Vec::new();
    if keystroke.modifiers.control {
        keys.push("ctrl");
    }
    if keystroke.modifiers.alt {
        keys.push("alt");
    }
    if keystroke.modifiers.function {
        keys.push("fn");
    }
    if keystroke.modifiers.shift {
        keys.push("shift");
    }
    keys.push(keystroke.key.as_str());
    let joined = keys.join("+");
    let mut escaped = String::with_capacity(joined.len() + 2);
    for character in joined.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            other => escaped.push(other),
        }
    }
    format!("{{\"key\":\"{escaped}\"}}")
}
