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
mod markdown;
mod reconcile;
mod tree;

use gpui::prelude::*;
use gpui::{App, Context, KeyBinding, actions, div, px};

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

actions!(designed, [Press, Toggle, Move, Adjust, Commit]);

/// Registers the designed view's key bindings: Enter and Space press a
/// button or a link, change a toggle or a checkbox; a segmented
/// control's and a slider's arrows move them; Enter commits a text
/// field; Enter in a text area inserts a newline.
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
        let Some(controls) = &self.designed else {
            return div().into_any_element();
        };
        let visuals = crate::settings::launcher_visuals(cx);
        let theme = visuals.theme;
        let draw = Draw {
            theme: &theme,
            state: &controls.state,
            render: view.render,
            surface: theme.panel_solid,
        };
        let tree = {
            let mut path = String::new();
            tree::push(&mut path, view.tree.root.key.as_deref(), 0);
            tree::draw_node(&view.tree.root, &mut path, draw, cx)
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
            .child(tree)
            .into_any_element()
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
        let key = (!key.is_empty()).then_some(key);
        let pending = self.launcher.send_designed_seen(
            handler,
            callback,
            key.as_deref(),
            Some(seen),
            payload,
        );
        self.show_until_done(pending, window, cx);
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

    /// The select at `path`'s model, read live each frame it draws: the
    /// choices and the choice the tree on screen names.
    fn designed_select_model(&self, path: &str, cx: &gpui::App) -> crate::ui::select::Model {
        let visuals = crate::settings::launcher_visuals(cx);
        let tree = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some(view.tree),
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
