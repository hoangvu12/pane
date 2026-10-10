//! A designed view's tree, drawn with GPUI's flex layout: the layout
//! primitives, the shared UI components, and the tokens and raw values
//! the tree styles with, resolved onto Pane's theme in light and dark
//! (`docs/designed-tree.md`, ADR 0036, #237).
//!
//! The tree the launcher holds is typed ([`pane_core::DesignedTree`]); the
//! window never meets the JSON the extension answered with. Layout is
//! computed here, whole: a `column` and a `row` are flex containers with
//! the space tokens resolved onto Pane's spacing rhythm, a `stack` places
//! its children over each other, a `scroll` scrolls, and the shared
//! components draw from Pane's own control families (`ui::controls`,
//! `ui::keycap`, `ui::extension_icon`), so an extension's screen is
//! Pane's design, not a copy of it. Every node may carry a [`Style`] —
//! sizing and a surface with `hover` and `pressed` variants Pane applies
//! itself, never asking the extension.
//!
//! The interactive components are focusable, in tree order: Tab and
//! Shift+Tab move through them, and Enter and Space press the focused
//! one (bound in [`bind_keys`], under each control's key context, which
//! sits below the launcher's window context). Escape stays with Pane, as
//! it does for a custom view: it pops the navigation stack (#239), leaving
//! the screen when only the root view is on it. Each control's focus
//! handle is kept by its path in the tree ([`DesignedControls`]), so a
//! re-render whose tree still draws that control keeps the keyboard on
//! it — the shape-keyed reconciliation of `form.rs`, until #238's keyed
//! reconciler owns it.
//!
//! Pressing a control hands its callback id to the launcher
//! ([`Launcher::send_designed_change`]), which sends the event to the
//! view — a press's payload empty, a change's naming the value the user
//! chose — and shows the tree it answers with; the extension's state
//! lives in its view, and the whole screen is redrawn from the answer.

mod components;
mod markdown;
mod tree;

use std::collections::HashMap;

use gpui::prelude::*;
use gpui::{App, Context, FocusHandle, KeyBinding, actions, div, px};

use pane_core::{DesignedTree, DesignedViewSnapshot, Node, NodeKind, ViewId};

use crate::app::LauncherWindow;

use tree::Draw;

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
/// The key context of a designed view's select: Enter and Space step
/// through its options.
const SELECT_CONTEXT: &str = "DesignedSelect";

actions!(designed, [Press, Toggle, Move, Adjust, Commit]);

/// Registers the designed view's key bindings: Enter and Space press a
/// button or a link, change a toggle or a checkbox, and step a select's
/// options; a segmented control's and a slider's arrows move them; Enter
/// commits a text field.
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
    ]);
}

/// What the window keeps for the open designed view: the keyboard focus
/// of each focusable control it draws, by the control's path in the tree.
pub(crate) struct DesignedControls {
    /// The opened view these controls are for.
    view: ViewId,
    /// The focus of each focusable control the last tree named, by its
    /// path: a re-render that still draws the control at that path keeps
    /// it focused, and one that does not loses it.
    focus: HashMap<String, FocusHandle>,
}

impl LauncherWindow {
    /// Creates or drops the designed view's controls to match the
    /// launcher's screen, and reconciles the focus handles with the tree
    /// it now shows: a newly opened view takes the keyboard on its first
    /// focusable control, and focus returns to the list when the view
    /// closes. The tree's loaded icons are drawn first, as a list's rows
    /// are (#142).
    pub(crate) fn sync_designed_view(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.launcher.refresh_designed_view();
        // The screen alone: the tree changes with each answer, and the
        // shapes of two trees of one view decide which focus handles are
        // kept.
        let shown = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some((view.id, shape(&view.tree))),
            _ => None,
        };
        if shown.as_ref().map(|(id, _)| *id) == self.designed.as_ref().map(|c| c.view) {
            // The same view: keep the focus handles whose controls are
            // still drawn, and add the ones the new tree added.
            if let Some((_, shape)) = shown {
                let controls = self.designed.as_mut().expect("a view is open");
                controls.focus.retain(|path, _| shape.contains(path));
                for path in &shape {
                    controls.focus.entry(path.clone()).or_insert_with(|| {
                        let handle = cx.focus_handle().tab_stop(true);
                        if shape.first() == Some(path) {
                            window.focus(&handle, cx);
                        }
                        handle
                    });
                }
            }
            return;
        }
        let closed = shown.is_none();
        self.designed = shown.map(|(view, shape)| {
            // A newly opened view takes the keyboard on its first
            // focusable control.
            let mut focus = HashMap::new();
            let first = shape.first();
            for path in &shape {
                let handle = cx.focus_handle().tab_stop(true);
                if Some(path) == first {
                    window.focus(&handle, cx);
                }
                focus.insert(path.clone(), handle);
            }
            DesignedControls { view, focus }
        });
        if closed {
            window.focus(&self.focus_handle, cx);
        }
    }

    /// Test support: how many focusable controls the open designed view's
    /// controls hold.
    #[doc(hidden)]
    pub fn designed_button_count(&self) -> usize {
        self.designed
            .as_ref()
            .map(|controls| controls.focus.len())
            .unwrap_or(0)
    }

    /// Sends one event of the open designed view — the callback id its
    /// tree named, raised on the node with `key`, with `payload` naming
    /// what changed — and redraws when its answer arrives.
    fn send_designed_event(
        &mut self,
        callback: u32,
        key: String,
        payload: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let pending = self.launcher.send_designed_change(
            callback,
            if key.is_empty() { None } else { Some(&key) },
            payload,
        );
        self.show_until_done(pending, window, cx);
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
            focus: &controls.focus,
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
}

/// The paths of the tree's focusable controls, in order: the paths whose
/// focus handles [`DesignedControls`] keeps.
fn shape(tree: &DesignedTree) -> Vec<String> {
    let mut shape = Vec::new();
    let mut path = String::new();
    tree::push(&mut path, tree.root.key.as_deref(), 0);
    shape_of(&tree.root, &mut path, &mut shape);
    shape
}

/// The focusable controls of `node`, whose place in the tree `path`
/// already names, appended to `shape`.
fn shape_of(node: &Node, path: &mut String, shape: &mut Vec<String>) {
    let start = path.len();
    if focusable(&node.kind) {
        shape.push(path.clone());
    }
    match &node.kind {
        // A node Pane does not know draws its fallback, in its place, else
        // its children.
        NodeKind::Unknown(_) => match &node.fallback {
            Some(fallback) => shape_of(fallback, path, shape),
            None => shape_children(node, path, shape),
        },
        _ => shape_children(node, path, shape),
    }
    path.truncate(start);
}

/// Whether a node of this kind is a focusable control.
fn focusable(kind: &NodeKind) -> bool {
    match kind {
        NodeKind::Button(button) => button.on_press.is_some() && button.enabled,
        NodeKind::Link(link) => link.on_press.is_some(),
        NodeKind::RichRow(row) => row.on_press.is_some(),
        NodeKind::Toggle(toggle) => toggle.on_change.is_some(),
        NodeKind::Checkbox(checkbox) => checkbox.on_change.is_some(),
        NodeKind::Segmented(segmented) => segmented.on_change.is_some(),
        NodeKind::Slider(slider) => slider.on_change.is_some(),
        // A text field or a select is a control whether or not the tree
        // gave it a callback: it takes the keyboard and holds the focus
        // order as any control does.
        NodeKind::TextInput(_) | NodeKind::PasswordInput(_) | NodeKind::TextArea(_) => true,
        NodeKind::Select(select) => !select.options.is_empty(),
        _ => false,
    }
}

/// The focusable controls of `node`'s children, each with its own place
/// under `path`.
fn shape_children(node: &Node, path: &mut String, shape: &mut Vec<String>) {
    for (index, child) in node.children.iter().enumerate() {
        let start = path.len();
        tree::push(path, child.key.as_deref(), index);
        shape_of(child, path, shape);
        path.truncate(start);
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
