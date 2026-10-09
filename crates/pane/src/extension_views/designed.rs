//! A designed view's tree, drawn with GPUI's flex layout: rows, columns,
//! text and buttons, laid out by Pane and styled with Pane's theme tokens
//! (`docs/designed-tree.md`, ADR 0036).
//!
//! The tree the launcher holds is typed ([`pane_core::DesignedTree`]); the
//! window never meets the JSON the extension answered with. Layout is
//! computed here, whole: a `column` and a `row` are flex containers with
//! the space tokens resolved onto Pane's spacing rhythm, a `text` carries
//! the typography and text level its style names, and a `button` draws in
//! the tone it names. A node Pane does not know draws the `fallback` the
//! tree gave, else its children. The token resolutions live in the
//! functions below, one mapping per token, so #237's public token layer
//! can extract them when it extracts the shared components.
//!
//! Buttons are focusable, in tree order: Tab and Shift+Tab move through
//! them, and Enter and Space press the focused one (bound in
//! [`bind_keys`], under the button's key context, which sits below the
//! launcher's window context). Escape stays with Pane, as it does for a
//! custom view: it leaves the screen. Each button's focus handle is kept
//! by its path in the tree ([`DesignedControls`]), so a re-render whose
//! tree still draws that button keeps the keyboard on it — the
//! shape-keyed reconciliation of `form.rs`, until #238's keyed reconciler
//! owns it.
//!
//! Pressing a button hands its callback id to the launcher
//! ([`Launcher::send_designed_event`]), which sends the press to the view
//! and shows the tree it answers with; the extension's state lives in its
//! view, and the whole screen is redrawn from the answer.

use std::collections::HashMap;

use gpui::ColorExt as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Div, FocusHandle, Hsla, KeyBinding, Pixels, Role, SharedString,
    Stateful, actions, div, px,
};
use pane_core::{
    Align, Button as ButtonNode, ButtonTone, DesignedTree, DesignedViewSnapshot, Justify, Layout,
    Node, NodeKind, Space, Text as TextNode, TextLevel, TextStyle, ViewId,
};

use crate::app::LauncherWindow;
use crate::ui::controls::{self, inset_ring};
use crate::ui::theme::{Theme, pressed};

/// The key context of a designed view's button: Enter and Space press it
/// there, below the launcher's window context (whose Escape and Tab bubble).
const BUTTON_CONTEXT: &str = "DesignedButton";

actions!(designed, [Press]);

/// Registers the designed view's key bindings: Enter and Space press the
/// focused button.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Press, Some(BUTTON_CONTEXT)),
        KeyBinding::new("space", Press, Some(BUTTON_CONTEXT)),
    ]);
}

/// What the window keeps for the open designed view: the keyboard focus of
/// each button it draws, by the button's path in the tree.
pub(crate) struct DesignedControls {
    /// The opened view these controls are for.
    view: ViewId,
    /// The focus of each button the last tree named, by its path: a
    /// re-render that still draws the button at that path keeps it
    /// focused, and one that does not loses it.
    focus: HashMap<String, FocusHandle>,
}

impl LauncherWindow {
    /// Creates or drops the designed view's controls to match the
    /// launcher's screen, and reconciles the button focus handles with the
    /// tree it now shows: a newly opened view takes the keyboard on its
    /// first button, and focus returns to the list when the view closes.
    pub(crate) fn sync_designed_view(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        // The screen alone: the tree changes with each answer, and the
        // shapes of two trees of one view decide which focus handles are
        // kept.
        let shown = match self.launcher.screen() {
            pane_core::Screen::DesignedView(view) => Some((view.id, shape(&view.tree))),
            _ => None,
        };
        if shown.as_ref().map(|(id, _)| *id) == self.designed.as_ref().map(|c| c.view) {
            // The same view: keep the focus handles whose buttons are still
            // drawn, and add the ones the new tree added.
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
            // A newly opened view takes the keyboard on its first button.
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

    /// Test support: how many focusable buttons the open designed view's
    /// controls hold.
    #[doc(hidden)]
    pub fn designed_button_count(&self) -> usize {
        self.designed
            .as_ref()
            .map(|controls| controls.focus.len())
            .unwrap_or(0)
    }

    /// Test support: the paths of the designed view's focusable buttons.
    #[doc(hidden)]
    pub fn designed_paths(&self) -> Vec<String> {
        self.designed
            .as_ref()
            .map(|controls| {
                let mut paths: Vec<String> = controls.focus.keys().cloned().collect();
                paths.sort();
                paths
            })
            .unwrap_or_default()
    }

    /// Sends the press of the button with callback id `callback` to the
    /// open designed view, raised on the node with `key`, and redraws when
    /// its answer arrives.
    fn send_designed_press(
        &mut self,
        callback: u32,
        key: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let pending = self
            .launcher
            .send_designed_event(callback, if key.is_empty() { None } else { Some(&key) });
        self.show_until_done(pending, window, cx);
    }

    /// The designed view screen for the launcher's snapshot of the view.
    pub(crate) fn render_designed_view(
        &self,
        view: DesignedViewSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(controls) = &self.designed else {
            return div().into_any_element();
        };
        let visuals = crate::settings::launcher_visuals(cx);
        let theme = visuals.theme;
        let focus = &controls.focus;
        let tree = {
            let mut path = String::new();
            push(&mut path, view.tree.root.key.as_deref(), 0);
            draw(&view.tree.root, &mut path, focus, &theme, cx)
        };
        // Fills the body as the list and the form do, so the status line
        // stays at the bottom, and scrolls what the tree grows past it.
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
            .pt(space(Space::S))
            .pb(geometry.list_padding_bottom)
            .child(tree)
            .into_any_element()
    }
}

/// One node of the tree, drawn: `path` is the node's place in the tree
/// (its key, else its index among its siblings), already ending with its
/// own segment, which keeps the focus of the buttons Pane drew before.
fn draw(
    node: &Node,
    path: &mut String,
    focus: &HashMap<String, FocusHandle>,
    theme: &Theme,
    cx: &mut Context<LauncherWindow>,
) -> AnyElement {
    let start = path.len();
    let name = node.name.clone();
    // A node Pane does not know: the `fallback` the tree gave, drawn in its
    // place, else its children, drawn as they are.
    let drawn = match &node.kind {
        NodeKind::Unknown(_) => match &node.fallback {
            Some(fallback) => draw(fallback, path, focus, theme, cx),
            None => {
                let children = children(node, path, focus, theme, cx);
                div()
                    .id(path.clone())
                    .flex()
                    .flex_col()
                    .map(|group| named(group, name.as_deref()))
                    .children(children)
                    .into_any_element()
            }
        },
        NodeKind::Column(layout) => {
            let own = path.clone();
            let children = children(node, path, focus, theme, cx);
            named(container(true, layout, children).id(own), name.as_deref()).into_any_element()
        }
        NodeKind::Row(layout) => {
            let own = path.clone();
            let children = children(node, path, focus, theme, cx);
            named(container(false, layout, children).id(own), name.as_deref()).into_any_element()
        }
        NodeKind::Text(text) => {
            let element = text_element(text, theme, path);
            let element = match name.as_deref() {
                Some(name) => element.aria_label(name),
                None => element,
            };
            element.into_any_element()
        }
        NodeKind::Button(button) => {
            let handle = focus.get(path).cloned();
            let path = path.clone();
            button_element(button, &path, node.key.clone(), handle, theme, cx)
        }
    };
    path.truncate(start);
    drawn
}

/// Gives `node` the group role and name assistive technology reads it by:
/// every node is reported, with its own name when the tree gave one.
fn named(node: Stateful<Div>, name: Option<&str>) -> Stateful<Div> {
    node.role(Role::Group)
        .when_some(name, |node, name| node.aria_label(name))
}

/// The children of a container, each drawn with its place in `path`.
fn children(
    parent: &Node,
    path: &mut String,
    focus: &HashMap<String, FocusHandle>,
    theme: &Theme,
    cx: &mut Context<LauncherWindow>,
) -> Vec<AnyElement> {
    parent
        .children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let start = path.len();
            push(path, child.key.as_deref(), index);
            let drawn = draw(child, path, focus, theme, cx);
            path.truncate(start);
            drawn
        })
        .collect()
}

/// Appends the place of the child at `index` with `key` to `path`.
fn push(path: &mut String, key: Option<&str>, index: usize) {
    path.push('/');
    match key {
        Some(key) => path.push_str(key),
        None => path.push_str(&index.to_string()),
    }
}

/// A container of children: a `column` when `column`, else a `row`, with
/// the layout its properties give it.
fn container(column: bool, layout: &Layout, children: Vec<AnyElement>) -> Div {
    let div = div()
        .flex()
        .when(column, |div| div.flex_col())
        .min_w(px(0.))
        .when_some(layout.gap, |div, gap| div.gap(space(gap)));
    let padding = layout.padding;
    let div = div
        .when_some(padding.top, |div, top| div.pt(space(top)))
        .when_some(padding.right, |div, right| div.pr(space(right)))
        .when_some(padding.bottom, |div, bottom| div.pb(space(bottom)))
        .when_some(padding.left, |div, left| div.pl(space(left)))
        .map(|div| match layout.align {
            Some(Align::Start) | None => div.items_start(),
            Some(Align::Center) => div.items_center(),
            Some(Align::End) => div.items_end(),
            Some(Align::Stretch) => div.items_stretch(),
            Some(Align::Baseline) => div.items_baseline(),
        })
        .map(|div| match layout.justify {
            Some(Justify::Start) | None => div.justify_start(),
            Some(Justify::Center) => div.justify_center(),
            Some(Justify::End) => div.justify_end(),
            Some(Justify::SpaceBetween) => div.justify_between(),
            Some(Justify::SpaceAround) => div.justify_around(),
        })
        .when(layout.wrap, |div| div.flex_wrap());
    div.children(children)
}

/// One text node: what it says, in the style and level its properties
/// name, named by its place in the tree.
fn text_element(text: &TextNode, theme: &Theme, path: &str) -> Stateful<Div> {
    let typography = &theme.typography;
    let (size, weight, family) = match text.style {
        None | Some(TextStyle::Body) => (
            typography.row_subtitle_size,
            typography.regular,
            typography.family.clone(),
        ),
        Some(TextStyle::Heading) => (px(16.), typography.medium, typography.family.clone()),
        Some(TextStyle::Title) => (
            typography.row_title_size,
            typography.medium,
            typography.family.clone(),
        ),
        Some(TextStyle::Caption) => (
            typography.row_kind_size,
            typography.regular,
            typography.family.clone(),
        ),
        Some(TextStyle::Mono) => (
            typography.row_subtitle_size,
            typography.regular,
            typography.mono_family.clone(),
        ),
        Some(TextStyle::SmallMono) => (
            typography.keycap_size,
            typography.regular,
            typography.mono_family.clone(),
        ),
    };
    let label: SharedString = text.content.clone().into();
    let debug = format!("designed-text-{}", text.content);
    div()
        .id(path.to_owned())
        .flex_none()
        .min_w(px(0.))
        .debug_selector(move || debug)
        .text_size(size)
        .font_weight(weight)
        .font_family(family)
        .line_height(size * typography.line_height)
        .text_color(text_level(text.level, theme))
        .role(Role::Label)
        .aria_label(label.clone())
        .child(label)
}

/// One button: its label in the tone its properties name, focusable, Enter
/// and Space (and a click) pressing it by the callback id its tree named.
fn button_element(
    button: &ButtonNode,
    path: &str,
    key: Option<String>,
    focus: Option<FocusHandle>,
    theme: &Theme,
    cx: &mut Context<LauncherWindow>,
) -> AnyElement {
    let Some(callback) = button.on_press else {
        // A button that cannot be pressed: its label, drawn as a text.
        return text_element(
            &TextNode {
                content: button.label.clone(),
                style: None,
                level: Some(TextLevel::Secondary),
            },
            theme,
            path,
        )
        .into_any_element();
    };
    let label: SharedString = button.label.clone().into();
    let debug = match focus {
        Some(_) => format!("designed-button-{}", button.label),
        None => format!("designed-button-static-{}-{}", path, button.label),
    };
    let ring = controls::focus_ring(theme);
    let (fill, edge, ink) = tone(button.tone, theme);
    let hover = hover_fill(button.tone, theme);
    let pill = &theme.geometry.results;
    let line = theme.typography.results.pill;
    // A press of the button: the callback id its tree named, raised on the
    // node with `key` (or none when the tree gave it none).
    let key = key.unwrap_or_default();
    let (for_keys, key_for_keys) = (callback, key.clone());
    let (for_click, key_for_click) = (callback, key);
    let (press, click) = (
        cx.listener(move |this, _: &Press, window, cx| {
            this.send_designed_press(for_keys, key_for_keys.clone(), window, cx);
        }),
        cx.listener(move |this, _, window, cx| {
            this.send_designed_press(for_click, key_for_click.clone(), window, cx);
        }),
    );
    let element = div()
        .id(path.to_owned())
        .debug_selector(move || debug)
        .key_context(BUTTON_CONTEXT)
        .role(Role::Button)
        .aria_label(label.clone())
        // A Settings button's box (`.pill`), filled in the button's tone.
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(theme.geometry.controls.button_gap)
        .h(pill.pill_height)
        .px(pill.pill_padding_x)
        .rounded(pill.pill_radius)
        .text_size(line.size)
        .line_height(line.line_height)
        .font_weight(theme.typography.medium)
        .whitespace_nowrap()
        .cursor_pointer()
        .text_color(ink)
        .bg(fill)
        .when_some(edge, |button, edge| {
            button.shadow(vec![inset_ring(edge, px(1.))])
        })
        .hover(move |button| button.bg(hover))
        .active(move |button| button.bg(pressed(hover)))
        .when_some(focus, |button, focus| {
            button
                .track_focus(&focus)
                .focus(move |button| button.shadow(ring))
        })
        .on_action(press)
        .on_click(click)
        .child(label);
    element.into_any_element()
}

/// The colors of a button's tone, resolved onto the theme: its fill, its
/// 1px inner edge (a plain button's ring), and its label's ink. #237's
/// token layer extracts this mapping with the shared button.
fn tone(tone: Option<ButtonTone>, theme: &Theme) -> (Hsla, Option<Hsla>, Hsla) {
    match tone {
        // The plain pill a Settings button draws.
        None | Some(ButtonTone::Default) => (
            theme.results.pill_fill,
            Some(theme.results.pill_edge),
            theme.text_title,
        ),
        // The ghost pill: transparent, its label in the body ink.
        Some(ButtonTone::Secondary) | Some(ButtonTone::Ghost) => {
            (gpui::transparent_black(), None, theme.text_body)
        }
        // The accent: the accent's fill, its ink on it.
        Some(ButtonTone::Accent) => (theme.accent, None, theme.accent_ink),
        // The destructive tone: the danger colour, at the fill an icon's
        // danger disc uses.
        Some(ButtonTone::Destructive) => (
            Hsla::opacity(&theme.danger, 0.18),
            Some(theme.danger),
            theme.danger,
        ),
    }
}

/// What a button of `tone` fills with while the pointer is over it.
fn hover_fill(tone: Option<ButtonTone>, theme: &Theme) -> Hsla {
    match tone {
        Some(ButtonTone::Secondary) | Some(ButtonTone::Ghost) => theme.control_hover,
        Some(ButtonTone::Accent) => Hsla::opacity(&theme.accent, 0.9),
        Some(ButtonTone::Destructive) => Hsla::opacity(&theme.danger, 0.24),
        None | Some(ButtonTone::Default) => theme.results.pill_hover,
    }
}

/// The paths of the tree's focusable nodes (its buttons), in order: the
/// paths whose focus handles [`DesignedControls`] keeps.
fn shape(tree: &DesignedTree) -> Vec<String> {
    let mut shape = Vec::new();
    let mut path = String::new();
    push(&mut path, tree.root.key.as_deref(), 0);
    shape_of(&tree.root, &mut path, &mut shape);
    shape
}

/// The focusable nodes of `node`, whose place in the tree `path` already
/// names, appended to `shape`.
fn shape_of(node: &Node, path: &mut String, shape: &mut Vec<String>) {
    let start = path.len();
    match &node.kind {
        NodeKind::Button(_) => shape.push(path.clone()),
        // A node Pane does not know draws its fallback, in its place, else
        // its children.
        NodeKind::Unknown(_) => match &node.fallback {
            Some(fallback) => shape_of(fallback, path, shape),
            None => {
                for (index, child) in node.children.iter().enumerate() {
                    push(path, child.key.as_deref(), index);
                    shape_of(child, path, shape);
                }
            }
        },
        _ => {
            for (index, child) in node.children.iter().enumerate() {
                push(path, child.key.as_deref(), index);
                shape_of(child, path, shape);
            }
        }
    }
    path.truncate(start);
}

/// A space token, resolved onto Pane's spacing rhythm (the reference's
/// 4-pixel steps): the named distances a tree's gaps, paddings and sizes
/// use, following the theme's appearance with it. #237's public token
/// layer extracts this mapping with the shared components.
fn space(token: pane_core::Space) -> Pixels {
    use pane_core::Space;
    match token {
        Space::Xs => px(4.),
        Space::S => px(8.),
        Space::M => px(12.),
        Space::L => px(16.),
        Space::Xl => px(24.),
        Space::Xxl => px(32.),
    }
}

/// A text level, resolved onto the theme's text colours (ADR 0035's
/// colour-through-alpha levels): the primary is a row title's, the
/// secondary a subtitle's, the tertiary a section label's, the quaternary
/// a placeholder's.
fn text_level(level: Option<TextLevel>, theme: &Theme) -> Hsla {
    match level {
        None | Some(TextLevel::Primary) => theme.text_title,
        Some(TextLevel::Secondary) => theme.text_body,
        Some(TextLevel::Tertiary) => theme.text_muted,
        Some(TextLevel::Quaternary) => theme.text_placeholder,
    }
}
