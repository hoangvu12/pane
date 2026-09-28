//! Root search's query field and its results.
//!
//! The query field is GPUI CE's single-line editable text element (typing,
//! editing keys, clipboard, undo and input-method composition). It has
//! keyboard focus whenever root search is on screen; every change searches
//! root at once. Up and Down move the selection through the results instead
//! of the caret, Enter opens the selected result and Escape clears the
//! query.
//!
//! For assistive technology the field and the result list form one
//! `EditableComboBox` node, which tracks the field's focus and carries the
//! query as its value; the selected result is its active descendant, so it
//! is reported as focused, and with no result the combo box itself is.

use gpui::{
    AnyElement, App, Context, Div, Entity, Focusable, KeyBinding, Role, Stateful, Subscription,
    Window, div, prelude::*, px, rgb,
};
use gpui_elements::editable_text::actions::DEFAULT_INPUT_CONTEXT;
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged, text_input};

use crate::{LauncherWindow, SelectNext, SelectPrevious};

const CONTEXT: &str = "RootSearch";
const PLACEHOLDER: &str = "Search commands";

/// Registers Up and Down in the query field to move the selection. They are
/// registered after, and so take precedence over, the text element's own
/// Up and Down, which in a single-line field move the caret to its start or
/// end.
pub(crate) fn bind_keys(cx: &mut App) {
    let context = format!("{CONTEXT} > {DEFAULT_INPUT_CONTEXT}");
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some(&context)),
        KeyBinding::new("up", SelectPrevious, Some(&context)),
    ]);
}

/// Root search's query field, kept for the window's lifetime.
pub(crate) struct QueryField {
    input: Entity<EditableTextState>,
    /// Whether root search was on screen when focus last followed the
    /// launcher's screen.
    shown: bool,
    _changes: Subscription,
}

impl QueryField {
    /// A query field whose every change searches root.
    pub(crate) fn new(cx: &mut Context<LauncherWindow>) -> QueryField {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        input.focus_handle(cx).tab_stop(true);
        let changes = cx.subscribe(&input, |this, input, _: &TextChanged, cx| {
            this.launcher.set_query(input.read(cx).as_str());
            cx.notify();
        });
        QueryField {
            input,
            shown: true,
            _changes: changes,
        }
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.input.focus_handle(cx), cx);
    }
}

impl LauncherWindow {
    /// Test support: the query field's editing state, which a platform
    /// input method talks to while composing text (see
    /// [`LauncherWindow::text_field`]).
    #[doc(hidden)]
    pub fn query_field(&self) -> Entity<EditableTextState> {
        self.query.input.clone()
    }

    /// Makes the query field follow the launcher: it shows the launcher's
    /// query (empty again after navigating back to root search or Escape)
    /// and takes focus when root search comes on screen; focus moves to the
    /// list when root search leaves the screen.
    pub(crate) fn sync_root_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.launcher.view().query;
        let was_shown = self.query.shown;
        self.query.shown = query.is_some();
        match query {
            Some(query) => {
                let input = self.query.input.clone();
                if input.read(cx).as_str() != query {
                    input.update(cx, |input, cx| input.emplace(&query, cx));
                }
                if !was_shown {
                    self.query.focus(window, cx);
                }
            }
            None if was_shown => window.focus(&self.focus_handle, cx),
            None => {}
        }
    }

    /// Root search: the query field above `list`, the results.
    pub(crate) fn render_root_search(
        &self,
        query: String,
        list: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = &self.query.input;
        div()
            .id("search")
            .debug_selector(|| "search".into())
            .key_context(CONTEXT)
            // The editable text element has no accessibility node of its
            // own; this node is the field's and tracks its focus handle.
            .track_focus(&input.focus_handle(cx))
            .role(Role::EditableComboBox)
            .aria_label("Search")
            .aria_value(query)
            .aria_placeholder(PLACEHOLDER)
            .flex_1()
            // Lets the results shrink below their content and scroll.
            .min_h(px(0.))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_2()
                    .border_color(rgb(0x8ab4f8))
                    .bg(rgb(0x1a1e24))
                    .child(
                        text_input("query")
                            .state(input.downgrade())
                            .placeholder(PLACEHOLDER)
                            .w_full()
                            .whitespace_nowrap()
                            .overflow_x_scroll(),
                    ),
            )
            .child(list)
            .into_any_element()
    }
}
