//! A designed view, as an author writes it: a [`View`] whose `render`
//! answers a tree of [`Node`]s, built with [`column`], [`row`], [`text`]
//! and [`button`], whose buttons' `on_click` closures run with the view's
//! state mutably through [`Cx::listener`]. The SDK answers Pane's `render`
//! with the tree as the versioned JSON document of `docs/designed-tree.md`
//! (ADR 0036's envelope), names each listener by a per-render callback id,
//! and runs the closure Pane hands back — so an author never sees the JSON
//! or the ids.
//!
//! A command whose `pane.json` entry says `"mode": "designed"` opens a
//! view as its screen: [`Command::open_designed_view`] answers with the
//! view's state (its [`Command::DesignedView`]), and Pane never calls the
//! command's `render`. Pane asks for the tree again after each event, so
//! the view's `render` reads the state the event's listener changed.
//!
//! ```ignore
//! struct Counter {
//!     count: u32,
//! }
//!
//! impl pane_extension::view::View for Counter {
//!     fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoNode {
//!         column()
//!             .gap(Space::M)
//!             .child(text(format!("Count: {}", self.count)).style(TextStyle::Title))
//!             .child(
//!                 row().gap(Space::S).children([
//!                     button("Increment").on_click(cx.listener(|this: &mut Self| {
//!                         this.count += 1;
//!                     })),
//!                     button("Reset")
//!                         .tone(Tone::Destructive)
//!                         .on_click(cx.listener(|this: &mut Self| this.count = 0)),
//!                 ]),
//!             )
//!     }
//! }
//!
//! impl pane_extension::Command for Sample {
//!     type CustomView = pane_extension::NoCustomView;
//!     type DesignedView = Counter;
//!
//!     async fn open_designed_view(
//!         command: String,
//!         launch: LaunchRecord,
//!     ) -> Result<Counter, String> {
//!         Ok(Counter { count: 0 })
//!     }
//! }
//! ```
//!
//! The listeners of the last two renders are kept: a press of what the
//! user could see is delivered even if the view has rendered since, and
//! an older event is dropped. The component set's version is written into
//! every document; a Pane that renders another major refuses it naming
//! both versions, and one that knows less draws what it understands.
//!
//! A view's answer may navigate (#239): [`Cx::push`] pushes a view above
//! this one, [`Cx::replace`] replaces this view with one, [`Cx::pop`] and
//! [`Cx::pop_with`] pop it — with a result answered to the view below,
//! whose own `on_pop` (given to [`Cx::push_with`]) runs with it. The back
//! key pops a view without asking, delivering the pop event with no
//! result. [`Container::navigation_title`] names the view, shown where a
//! screen's title is.

use alloc::borrow::ToOwned as _;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt::Write as _;

use crate::exports::pane::extension::command as wit;
use wit::{GuestView, Outcome, Rendered, UiEvent};

/// The version of the UI component set this SDK writes
/// (`docs/designed-tree.md`).
const COMPONENT_SET: &str = "1.0";

/// The callback id of the pop event, the event Pane sends a view when the
/// one above it popped: an id no tree names (this SDK's ids start at 1),
/// its payload `{"pop": <result>}`, the result the pop answered (`null`
/// when the back key popped, so no view's result reached the one below).
const POP_CALLBACK: u32 = 0;

/// A designed view: the screen a `"mode": "designed"` command opens. Pane
/// asks for its tree when the view opens and after each event, with a
/// [`Cx`] through which the tree's listeners are named; the view's state
/// is the author's, kept in the view's resource for as long as it is
/// open.
pub trait View: Sized + 'static {
    /// The view's tree, as it is now. The buttons' `on_click` listeners
    /// are named through `cx`; the tree is drawn whole again after each
    /// event, so what the view's state now says is what the user sees.
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoNode;
}

/// The context of one render: where the tree's listeners are named, each
/// by a fresh callback id, kept by the view until the next-but-one render.
pub struct Cx<'a, V: View> {
    listeners: &'a mut Vec<Run<V>>,
}

impl<V: View> Cx<'_, V> {
    /// The listener `run` becomes, which a press of the button it is given
    /// to runs with the view's state mutably: `cx.listener(|this: &mut V|
    /// ...)`, handed to [`Button::on_click`]. The view re-renders after
    /// the press, as it does after every event.
    pub fn listener(&mut self, run: impl FnOnce(&mut V) + 'static) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Run::Listener(Box::new(run)));
        Listener(id)
    }

    /// A press that pushes a view above this one: `open` builds the
    /// pushed view's state from this view's, this view staying below it
    /// (its state kept, its tree shown again when the pushed view pops).
    /// Handed to [`Button::on_click`], as a listener is.
    pub fn push(&mut self, open: impl FnOnce(&mut V) -> V + 'static) -> Listener {
        self.pushed(Box::new(open), None)
    }

    /// A press that pushes a view above this one, `on_pop` answering this
    /// view when it pops: with the result the pop answered (or `None`, a
    /// pop that carried none, as the back key's), this view re-rendering
    /// after. The Raycast-style `Action.Push`'s `onPop`.
    pub fn push_with(
        &mut self,
        open: impl FnOnce(&mut V) -> V + 'static,
        on_pop: impl FnOnce(&mut V, Option<&str>) + 'static,
    ) -> Listener {
        self.pushed(Box::new(open), Some(Box::new(on_pop)))
    }

    /// A press that pushes a view above this one, `on_pop` kept to answer
    /// this view when it pops.
    fn pushed(
        &mut self,
        open: Box<dyn FnOnce(&mut V) -> V>,
        on_pop: Option<Box<dyn FnOnce(&mut V, Option<&str>)>>,
    ) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Run::Push { open, on_pop });
        Listener(id)
    }

    /// A press that replaces this view with the state `open` builds: this
    /// view is dropped, never used again, the views below it staying.
    pub fn replace(&mut self, open: impl FnOnce(&mut V) -> V + 'static) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Run::Replace(Box::new(open)));
        Listener(id)
    }

    /// A press that pops this view: the view below's tree shows again and
    /// it is told the view above popped, with no result.
    pub fn pop(&mut self) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Run::Pop(None));
        Listener(id)
    }

    /// A press that pops this view, answering `result` — built from this
    /// view's state — to the view below, whose own `on_pop` runs with it
    /// before it re-renders.
    pub fn pop_with(&mut self, result: impl FnOnce(&mut V) -> String + 'static) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Run::Pop(Some(Box::new(result))));
        Listener(id)
    }
}

/// A named listener of the tree: what [`Cx::listener`], [`Cx::push`],
/// [`Cx::replace`], [`Cx::pop`] and their kinds answer, handed to
/// [`Button::on_click`].
#[derive(Debug)]
pub struct Listener(u32);

/// What a listener of the tree does when its node is pressed: run it and
/// answer nothing next (the view re-renders, as it always does), or
/// answer the navigation one of [`Cx`]'s push, replace and pop listeners
/// builds.
enum Run<V> {
    /// Runs the listener; nothing next.
    Listener(Box<dyn FnOnce(&mut V)>),
    /// Builds the state of a view pushed above this one, `on_pop` (when
    /// given) kept to answer this view when it pops.
    Push {
        open: Box<dyn FnOnce(&mut V) -> V>,
        on_pop: Option<Box<dyn FnOnce(&mut V, Option<&str>)>>,
    },
    /// Builds the state of the view replacing this one.
    Replace(Box<dyn FnOnce(&mut V) -> V>),
    /// Pops this view, answering the result the closure builds (an empty
    /// one when there is none).
    Pop(Option<Box<dyn FnOnce(&mut V) -> String>>),
}

/// One node of the tree: a layout primitive or UI component, with the
/// properties it was given. Built with [`column`], [`row`], [`text`] and
/// [`button`], and combined with [`Container::child`] and
/// [`Container::children`].
#[derive(Debug)]
pub enum Node {
    /// A column of children.
    Column {
        layout: Layout,
        /// The view's navigation title, read when this node is the tree's
        /// root: what names the view where a screen's title is shown.
        navigation_title: Option<String>,
        children: Vec<Node>,
    },
    /// A row of children.
    Row {
        layout: Layout,
        /// The view's navigation title, read when this node is the tree's
        /// root: what names the view where a screen's title is shown.
        navigation_title: Option<String>,
        children: Vec<Node>,
    },
    /// One line of text.
    Text(Text),
    /// A button.
    Button(Button),
}

/// The layout of a `column` or `row`: its gap, padding, alignment,
/// distribution and whether children wrap.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    gap: Option<Space>,
    padding: Padding,
    align: Option<Align>,
    justify: Option<Justify>,
    wrap: bool,
}

/// The padding inside a node: each side's space token, `None` for none.
#[derive(Clone, Copy, Debug, Default)]
pub struct Padding {
    /// The space above the node's content.
    pub top: Option<Space>,
    /// The space to its right.
    pub right: Option<Space>,
    /// The space below it.
    pub bottom: Option<Space>,
    /// The space to its left.
    pub left: Option<Space>,
}

/// One text node: what it says, in which style and level.
#[derive(Debug)]
pub struct Text {
    text: String,
    style: Option<TextStyle>,
    level: Option<TextLevel>,
}

/// One button: its label, its tone, and the listener a press of it runs.
#[derive(Debug)]
pub struct Button {
    label: String,
    tone: Option<Tone>,
    on_click: Option<Listener>,
}

/// A space token: the named distances of the UI component set, resolved
/// onto Pane's spacing rhythm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Xs,
    S,
    M,
    L,
    Xl,
    Xxl,
}

/// How a container's children are laid out along its cross axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

/// How a container's children share its main axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// The style of a text: its size, weight and family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    Heading,
    Title,
    Body,
    Caption,
    Mono,
    SmallMono,
}

/// The level of a text: its colour, through the alpha of the text colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLevel {
    Primary,
    Secondary,
    Tertiary,
    Quaternary,
}

/// The tone of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Default,
    Secondary,
    Ghost,
    Accent,
    Destructive,
}

/// Anything that becomes one [`Node`] of the tree: the builders
/// ([`column`] and its kind) and a `Node` itself.
pub trait IntoNode {
    fn into_node(self) -> Node;
}

impl IntoNode for Node {
    fn into_node(self) -> Node {
        self
    }
}

/// A column: children below each other.
pub fn column() -> Container {
    Container(Node::Column {
        layout: Layout::default(),
        navigation_title: None,
        children: Vec::new(),
    })
}

/// A row: children beside each other.
pub fn row() -> Container {
    Container(Node::Row {
        layout: Layout::default(),
        navigation_title: None,
        children: Vec::new(),
    })
}

/// One line of text.
pub fn text(text: impl Into<String>) -> Text {
    Text {
        text: text.into(),
        style: None,
        level: None,
    }
}

/// A button with `label`, which a press runs the listener it is given.
pub fn button(label: impl Into<String>) -> Button {
    Button {
        label: label.into(),
        tone: None,
        on_click: None,
    }
}

/// A `column` or `row` being built: the container the tree lays out.
#[derive(Debug)]
pub struct Container(Node);

/// The properties of a column or a row, as it is built.
impl Container {
    /// The gap between this container's children.
    pub fn gap(mut self, gap: Space) -> Container {
        self.layout(|layout| layout.gap = Some(gap));
        self
    }

    /// The padding inside this container, one token for all sides.
    pub fn padding(self, padding: Space) -> Container {
        self.padding_each(Padding {
            top: Some(padding),
            right: Some(padding),
            bottom: Some(padding),
            left: Some(padding),
        })
    }

    /// The padding inside this container, each side's.
    pub fn padding_each(mut self, padding: Padding) -> Container {
        self.layout(|layout| layout.padding = padding);
        self
    }

    /// How this container's children are laid out along its cross axis.
    pub fn align(mut self, align: Align) -> Container {
        self.layout(|layout| layout.align = Some(align));
        self
    }

    /// How this container's children share its main axis.
    pub fn justify(mut self, justify: Justify) -> Container {
        self.layout(|layout| layout.justify = Some(justify));
        self
    }

    /// Whether this container's children wrap onto further lines.
    pub fn wrap(mut self) -> Container {
        self.layout(|layout| layout.wrap = true);
        self
    }

    /// The navigation title of the view this container is the root of:
    /// what names the view where a screen's title is shown. Only the
    /// tree's root names one; a title on any other node is ignored.
    pub fn navigation_title(mut self, title: impl Into<String>) -> Container {
        let title = title.into();
        if let Node::Column {
            navigation_title, ..
        }
        | Node::Row {
            navigation_title, ..
        } = &mut self.0
        {
            *navigation_title = Some(title);
        }
        self
    }

    /// This container with `child` after its children.
    pub fn child(self, child: impl IntoNode) -> Container {
        self.children([child])
    }

    /// This container with `children` after its children.
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoNode>) -> Container {
        if let Node::Column { children: own, .. } | Node::Row { children: own, .. } = &mut self.0 {
            own.extend(children.into_iter().map(IntoNode::into_node));
        }
        self
    }

    /// This container's layout, changed by `change`.
    fn layout(&mut self, change: impl FnOnce(&mut Layout)) {
        if let Node::Column { layout, .. } | Node::Row { layout, .. } = &mut self.0 {
            change(layout);
        }
    }
}

impl IntoNode for Container {
    fn into_node(self) -> Node {
        self.0
    }
}

impl Text {
    /// This text in `style`: its size, weight and family.
    pub fn style(mut self, style: TextStyle) -> Text {
        self.style = Some(style);
        self
    }

    /// This text at `level`: its colour, through the alpha of the text
    /// colour.
    pub fn level(mut self, level: TextLevel) -> Text {
        self.level = Some(level);
        self
    }
}

impl IntoNode for Text {
    fn into_node(self) -> Node {
        Node::Text(self)
    }
}

impl Button {
    /// This button in `tone`.
    pub fn tone(mut self, tone: Tone) -> Button {
        self.tone = Some(tone);
        self
    }

    /// A press of this button runs `listener` ([`Cx::listener`]), with the
    /// view's state mutably. Without one, the button cannot be pressed.
    pub fn on_click(mut self, listener: Listener) -> Button {
        self.on_click = Some(listener);
        self
    }
}

impl IntoNode for Button {
    fn into_node(self) -> Node {
        Node::Button(self)
    }
}

/// The designed view type of a command that opens none: an empty type, as
/// [`crate::NoCustomView`] is for custom views.
pub enum NoDesignedView {}

impl View for NoDesignedView {
    // No value of the type exists, so this never runs: an empty column is
    // the node of nothing.
    fn render(&mut self, _cx: &mut Cx<Self>) -> impl IntoNode {
        column()
    }
}

/// The state of one designed view the extension opened, as Pane's resource
/// holds it: the view, the listeners of its last two renders, and the
/// `on_pop` of the view it pushed last. The SDK makes it around the state
/// [`Command::open_designed_view`] answers with; the resource's one type.
pub struct Open<V: View> {
    state: RefCell<V>,
    /// The listeners of the last two renders, newest last: the render
    /// number and its listeners by callback id.
    tables: RefCell<Vec<(u64, Vec<Run<V>>)>>,
    /// The `on_pop` this view's last push registered, run when the view it
    /// pushed pops, with the result that pop answered.
    on_pop: RefCell<Option<Box<dyn FnOnce(&mut V, Option<&str>)>>>,
}

impl<V: View> Open<V> {
    /// The resource around `state`, its listener tables empty.
    pub(crate) fn new(state: V) -> Open<V> {
        Open {
            state: RefCell::new(state),
            tables: RefCell::new(Vec::new()),
            on_pop: RefCell::new(None),
        }
    }
}

impl<V: View> GuestView for Open<V> {
    async fn render(&self, context: String) -> Result<Rendered, String> {
        let render = render_of(&context);
        let mut listeners: Vec<Run<V>> = Vec::new();
        let node = {
            let mut cx = Cx {
                listeners: &mut listeners,
            };
            self.state.borrow_mut().render(&mut cx).into_node()
        };
        let mut tree = String::new();
        write_node(&mut tree, &node)?;
        let document = format!("{{\"version\":\"{COMPONENT_SET}\",\"root\":{tree}}}");
        // Keep the last two renders' listeners: an event raised on the tree
        // the user saw is delivered even if the view has rendered since.
        self.tables
            .borrow_mut()
            .retain(|(number, _)| *number + 1 >= render);
        self.tables.borrow_mut().push((render, listeners));
        Ok(Rendered {
            tree: document,
            // Timers land with #236, which reschedules the view through it.
            refresh_after_ms: None,
        })
    }

    async fn handle_event(&self, event: UiEvent) -> Result<Outcome, String> {
        // The pop event: the view above this one popped, its payload the
        // result that pop answered. The view re-renders after it, as after
        // every event.
        if event.callback == POP_CALLBACK {
            let result = pop_result_of(&event.payload);
            let on_pop = self.on_pop.borrow_mut().take();
            if let Some(run) = on_pop {
                run(&mut self.state.borrow_mut(), result.as_deref());
            }
            return Ok(nothing_next());
        }
        let UiEvent { render, callback, .. } = event;
        // The listener the tree named, taken from the table of the render
        // the user saw: an event of an older render is stale, dropped.
        let id = usize::try_from(callback.saturating_sub(1)).ok();
        let taken = {
            let mut tables = self.tables.borrow_mut();
            match (
                tables
                    .iter_mut()
                    .find(|(number, _)| *number == render)
                    .map(|(_, listeners)| listeners),
                id,
            ) {
                (Some(listeners), Some(id)) if id < listeners.len() => {
                    Some(listeners.swap_remove(id))
                }
                _ => None,
            }
        };
        // Run what the tree named, answering the navigation it asked for.
        let next = match taken {
            Some(Run::Listener(run)) => {
                run(&mut self.state.borrow_mut());
                None
            }
            Some(Run::Push { open, on_pop }) => {
                let state = open(&mut self.state.borrow_mut());
                *self.on_pop.borrow_mut() = on_pop;
                Some(Next::Push(Open::new(state)))
            }
            Some(Run::Replace(open)) => {
                let state = open(&mut self.state.borrow_mut());
                Some(Next::Replace(Open::new(state)))
            }
            Some(Run::Pop(answer)) => {
                let result = answer.map(|answer| answer(&mut self.state.borrow_mut()));
                Some(Next::Pop(result.unwrap_or_default()))
            }
            None => None,
        };
        Ok(match next {
            Some(Next::Push(view)) => Outcome {
                push: Some(wit::View::new(view)),
                replace: None,
                pop: None,
            },
            Some(Next::Replace(view)) => Outcome {
                push: None,
                replace: Some(wit::View::new(view)),
                pop: None,
            },
            Some(Next::Pop(result)) => Outcome {
                push: None,
                replace: None,
                pop: Some(result),
            },
            None => nothing_next(),
        })
    }
}

/// What a listener's run answered the view does next: a view to push
/// above it or to replace it with, or a result to pop the view with.
enum Next<V: View> {
    Push(Open<V>),
    Replace(Open<V>),
    Pop(String),
}

/// The outcome with nothing next: what a listener that asked for no
/// navigation answers.
fn nothing_next() -> Outcome {
    Outcome {
        push: None,
        replace: None,
        pop: None,
    }
}

/// The result the pop event's payload carries: `{"pop": "…"}` for a pop
/// that answered one, `{"pop": null}` for a pop that carried none (the
/// back key's, so no view's result reached the one below). Read without
/// parsing the whole document, so a payload that grew a field still gives
/// its result.
fn pop_result_of(payload: &str) -> Option<String> {
    let at = payload.find("\"pop\"")?;
    let rest = payload[at + 5..].trim_start().strip_prefix(':')?.trim_start();
    if rest.starts_with("null") {
        return None;
    }
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let escaped = &rest[..end];
    let mut result = String::new();
    let mut characters = escaped.chars();
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\u{8}'),
                Some('f') => result.push('\u{c}'),
                Some('u') => {
                    let digits: String = characters.by_ref().take(4).collect();
                    let code = u32::from_str_radix(&digits, 16).ok()?;
                    result.push(char::from_u32(code)?);
                }
                _ => return None,
            },
            other => result.push(other),
        }
    }
    Some(result)
}

/// The render number `context` names, or 1 when it says none: the context
/// is JSON, `{"render": N, "ui": "1.0"}`, read without parsing the whole
/// document, so a context that grew a field still gives its number.
fn render_of(context: &str) -> u64 {
    let Some(at) = context.find("\"render\"") else {
        return 1;
    };
    let digits = context[at + 8..]
        .trim_start()
        .strip_prefix(':')
        .unwrap_or("")
        .trim_start();
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    digits[..end].parse().unwrap_or(1)
}

/// Writes `node` as the tree's JSON, naming each button's listener by the
/// id [`Cx::listener`] gave it.
fn write_node(tree: &mut String, node: &Node) -> Result<(), String> {
    match node {
        Node::Column {
            layout,
            navigation_title,
            children,
        } => {
            tree.push_str("{\"type\":\"column\"");
            if let Some(title) = navigation_title {
                tree.push_str(",\"navigationTitle\":");
                write_string(tree, title)?;
            }
            write_layout(tree, layout)?;
            tree.push_str(",\"children\":[");
            write_nodes(tree, children)?;
            tree.push_str("]}");
        }
        Node::Row {
            layout,
            navigation_title,
            children,
        } => {
            tree.push_str("{\"type\":\"row\"");
            if let Some(title) = navigation_title {
                tree.push_str(",\"navigationTitle\":");
                write_string(tree, title)?;
            }
            write_layout(tree, layout)?;
            tree.push_str(",\"children\":[");
            write_nodes(tree, children)?;
            tree.push_str("]}");
        }
        Node::Text(text) => {
            tree.push_str("{\"type\":\"text\",\"text\":");
            write_string(tree, &text.text)?;
            if let Some(style) = text.style {
                tree.push_str(",\"style\":");
                write_string(
                    tree,
                    match style {
                        TextStyle::Heading => "heading",
                        TextStyle::Title => "title",
                        TextStyle::Body => "body",
                        TextStyle::Caption => "caption",
                        TextStyle::Mono => "mono",
                        TextStyle::SmallMono => "small-mono",
                    },
                )?;
            }
            if let Some(level) = text.level {
                tree.push_str(",\"level\":");
                write_string(
                    tree,
                    match level {
                        TextLevel::Primary => "primary",
                        TextLevel::Secondary => "secondary",
                        TextLevel::Tertiary => "tertiary",
                        TextLevel::Quaternary => "quaternary",
                    },
                )?;
            }
            tree.push('}');
        }
        Node::Button(button) => {
            tree.push_str("{\"type\":\"button\",\"label\":");
            write_string(tree, &button.label)?;
            if let Some(tone) = button.tone {
                tree.push_str(",\"tone\":");
                write_string(
                    tree,
                    match tone {
                        Tone::Default => "default",
                        Tone::Secondary => "secondary",
                        Tone::Ghost => "ghost",
                        Tone::Accent => "accent",
                        Tone::Destructive => "destructive",
                    },
                )?;
            }
            if let Some(Listener(id)) = &button.on_click {
                tree.push_str(",\"onPress\":");
                write!(tree, "{id}").map_err(|_| "the tree is not writable".to_owned())?;
            }
            tree.push('}');
        }
    }
    Ok(())
}

/// Writes `children` as the tree's JSON array, comma-separated.
fn write_nodes(tree: &mut String, children: &[Node]) -> Result<(), String> {
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            tree.push(',');
        }
        write_node(tree, child)?;
    }
    Ok(())
}

/// Writes a container's layout properties: `gap`, `padding`, `align`,
/// `justify` and `wrap`.
fn write_layout(tree: &mut String, layout: &Layout) -> Result<(), String> {
    if let Some(gap) = layout.gap {
        tree.push_str(",\"gap\":");
        write_space(tree, gap)?;
    }
    let padding = &layout.padding;
    if padding.top.is_some()
        || padding.right.is_some()
        || padding.bottom.is_some()
        || padding.left.is_some()
    {
        tree.push_str(",\"padding\":{");
        let mut first = true;
        for (name, side) in [
            ("top", padding.top),
            ("right", padding.right),
            ("bottom", padding.bottom),
            ("left", padding.left),
        ] {
            if let Some(space) = side {
                if !first {
                    tree.push(',');
                }
                first = false;
                tree.push_str(name);
                tree.push(':');
                write_space(tree, space)?;
            }
        }
        tree.push('}');
    }
    if let Some(align) = layout.align {
        tree.push_str(",\"align\":");
        write_string(
            tree,
            match align {
                Align::Start => "start",
                Align::Center => "center",
                Align::End => "end",
                Align::Stretch => "stretch",
                Align::Baseline => "baseline",
            },
        )?;
    }
    if let Some(justify) = layout.justify {
        tree.push_str(",\"justify\":");
        write_string(
            tree,
            match justify {
                Justify::Start => "start",
                Justify::Center => "center",
                Justify::End => "end",
                Justify::SpaceBetween => "space-between",
                Justify::SpaceAround => "space-around",
            },
        )?;
    }
    if layout.wrap {
        tree.push_str(",\"wrap\":true");
    }
    Ok(())
}

/// Writes `space` as the tree's token name.
fn write_space(tree: &mut String, space: Space) -> Result<(), String> {
    write_string(
        tree,
        match space {
            Space::Xs => "xs",
            Space::S => "s",
            Space::M => "m",
            Space::L => "l",
            Space::Xl => "xl",
            Space::Xxl => "xxl",
        },
    )
}

/// Writes `text` as the tree's JSON string.
fn write_string(tree: &mut String, text: &str) -> Result<(), String> {
    tree.push('"');
    for character in text.chars() {
        match character {
            '"' => tree.push_str("\\\""),
            '\\' => tree.push_str("\\\\"),
            '\n' => tree.push_str("\\n"),
            '\r' => tree.push_str("\\r"),
            '\t' => tree.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                write!(tree, "\\u{:04x}", other as u32)
                    .map_err(|_| "the tree is not writable".to_owned())?;
            }
            other => tree.push(other),
        }
    }
    tree.push('"');
    Ok(())
}
