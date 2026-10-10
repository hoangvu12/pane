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
//! the view's `render` reads the state the event's listener changed. A
//! view that changes by itself asks for that: the answer of its render
//! names when Pane asks again ([`IntoNode::refresh_after`], #236), with
//! [`Pending`] presenting data on its way as a loading state — see the
//! timer sample (`sample-timer`).
//!
//! ```ignore
//! struct Counter {
//!     count: u32,
//! }
//!
//! impl pane_extension::view::View for Counter {
//!     fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoAnswer {
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

use alloc::borrow::ToOwned as _;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::time::Duration;

use crate::exports::pane::extension::command as wit;
use wit::{GuestView, Outcome, Rendered, UiEvent};

/// The version of the UI component set this SDK writes
/// (`docs/designed-tree.md`).
const COMPONENT_SET: &str = "1.0";

/// How long a loading state waits for its data before it is asked for
/// again: the floor of Pane's refreshes (#236).
const PROMPT_REFRESH_MS: u32 = 100;

/// A designed view: the screen a `"mode": "designed"` command opens. Pane
/// asks for its tree when the view opens and after each event, with a
/// [`Cx`] through which the tree's listeners are named; the view's state
/// is the author's, kept in the view's resource for as long as it is
/// open. A view that changes by itself asks for that: the answer of its
/// render names when Pane asks again ([`IntoNode::refresh_after`],
/// #236), so timers, clocks, progress and polling need no change to the
/// extension runtime.
pub trait View: Sized + 'static {
    /// The view's tree, as it is now. The buttons' `on_click` listeners
    /// are named through `cx`; the tree is drawn whole again after each
    /// event, so what the view's state now says is what the user sees.
    /// The answer may ask Pane to draw it again after some time.
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoAnswer;
}

/// The context of one render: where the tree's listeners are named, each
/// by a fresh callback id, kept by the view until the next-but-one render,
/// and whether the render answers the refresh the view asked for (where
/// an interval's work runs).
pub struct Cx<'a, V: View> {
    listeners: &'a mut Vec<Run<V>>,
    refreshed: bool,
}

impl<V: View> Cx<'_, V> {
    /// The listener `run` becomes, which a press of the button it is given
    /// to runs with the view's state mutably: `cx.listener(|this: &mut V|
    /// ...)`, handed to [`Button::on_click`].
    pub fn listener(&mut self, run: impl FnOnce(&mut V) + 'static) -> Listener {
        let id = self.listeners.len() as u32 + 1;
        self.listeners.push(Box::new(run));
        Listener(id)
    }

    /// Whether this render answers the refresh the view asked for by
    /// [`IntoNode::refresh_after`]: `false` on the view's first render,
    /// on the one after an event, and on the prompt a loading state asks
    /// for (which awaits its data instead, see [`Pending`]). This is
    /// where an interval's work runs — counting a tick, advancing a
    /// clock — so that it runs once per interval and never twice for one
    /// drawing.
    pub fn refreshed(&self) -> bool {
        self.refreshed
    }
}

/// A named listener of the tree: what [`Cx::listener`] answers, handed to
/// [`Button::on_click`].
#[derive(Debug)]
pub struct Listener(u32);

/// What a listener runs, once, with the view's state mutably.
type Run<V> = Box<dyn FnOnce(&mut V)>;

/// One node of the tree: a layout primitive or UI component, with the
/// properties it was given. Built with [`column`], [`row`], [`text`] and
/// [`button`], and combined with [`Container::child`] and
/// [`Container::children`].
#[derive(Debug)]
pub enum Node {
    /// A column of children.
    Column {
        layout: Layout,
        children: Vec<Node>,
    },
    /// A row of children.
    Row {
        layout: Layout,
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

    /// Pane draws this tree again after `after`: the view's own ask, the
    /// render it answers named by [`Cx::refreshed`] (#236). The answer of
    /// a render is one: the ask belongs to its root, and a tree that is a
    /// child of another cannot ask.
    fn refresh_after(self, after: Duration) -> Answer
    where
        Self: Sized,
    {
        Answer {
            node: self.into_node(),
            refresh: Some(Refresh::After(after)),
        }
    }
}

/// What a view's `render` answers: the tree it drew, and when Pane asks
/// for it again. Every tree an [`IntoNode`] builds is one with no ask;
/// [`IntoNode::refresh_after`] — the view's own ask, an interval — and
/// [`loading`] — the prompt a loading state asks for, awaiting its data
/// — are the two that ask.
pub struct Answer {
    node: Node,
    refresh: Option<Refresh>,
}

/// When Pane asks for a view's tree again.
enum Refresh {
    /// Promptly: a loading state waiting for its data ([`loading`]).
    Prompt,
    /// After `after`: the view's own ask, whose render [`Cx::refreshed`]
    /// names.
    After(Duration),
}

impl Answer {
    /// Pane draws this answer's tree again after `after`, whatever it
    /// asked for before.
    pub fn refresh_after(mut self, after: Duration) -> Answer {
        self.refresh = Some(Refresh::After(after));
        self
    }
}

/// What a view's `render` answers: every tree (anything [`IntoNode`])
/// is one, as is [`Answer`].
pub trait IntoAnswer {
    /// The tree drawn, and when Pane asks for it again.
    fn into_answer(self) -> Answer;
}

impl<N: IntoNode> IntoAnswer for N {
    fn into_answer(self) -> Answer {
        Answer {
            node: self.into_node(),
            refresh: None,
        }
    }
}

impl IntoAnswer for Answer {
    fn into_answer(self) -> Answer {
        self
    }
}

/// A view's loading state (#236): `tree` is shown at once, and Pane is
/// asked for the tree again promptly — the render the pending data the
/// view shows it for is awaited in ([`Pending::ready`]), unlike an
/// interval's ask, whose work runs then instead. What `tree` draws is
/// the author's; a text naming what is loading is the usual one.
pub fn loading(tree: impl IntoNode) -> Answer {
    Answer {
        node: tree.into_node(),
        refresh: Some(Refresh::Prompt),
    }
}

/// Data a view is still waiting for, shown as a loading state until it
/// arrives (#236): [`Pending::loading`] holds the work, and each render
/// reads it through [`Pending::ready`] — `None` while the work has not
/// answered, so the view renders its loading state ([`loading`]), and its
/// answer from then on. The work is polled once per render: a guest has
/// no clock of its own, so the prompt refresh the loading state asks for
/// is what waits for it, and each poll is bounded by the call's limits as
/// any compute is.
///
/// The work runs once; a view that wants it again builds another.
pub struct Pending<T> {
    work: Option<Pin<Box<dyn Future<Output = T>>>>,
    answer: Option<T>,
}

impl<T> Pending<T> {
    /// Data answered by `work`, which the view waits for: a future the
    /// view's `open_designed_view` can start, of a file read, a service
    /// called, or work of its own.
    pub fn loading(work: impl Future<Output = T>) -> Pending<T> {
        Pending {
            work: Some(Box::pin(work)),
            answer: None,
        }
    }

    /// The work's answer, once it has answered: each call polls the work
    /// once, taking its answer when it is ready. `None` while it is
    /// pending — the loading state, whose prompt refresh asks for this
    /// render again.
    pub fn ready(&mut self) -> Option<&T> {
        if self.answer.is_none()
            && let Some(work) = self.work.as_mut()
        {
            // The waker does nothing: nothing schedules the work, the
            // prompt refresh's render polls it again.
            let mut context = Context::from_waker(Waker::noop());
            if let Poll::Ready(answer) = work.as_mut().poll(&mut context) {
                self.answer = Some(answer);
                self.work = None;
            }
        }
        self.answer.as_ref()
    }
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
        children: Vec::new(),
    })
}

/// A row: children beside each other.
pub fn row() -> Container {
    Container(Node::Row {
        layout: Layout::default(),
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
    fn render(&mut self, _cx: &mut Cx<Self>) -> impl IntoAnswer {
        column()
    }
}

/// What the view's last render asked Pane for: nothing, a prompt refresh
/// (to await pending data, see [`Pending`]), or the refresh the view
/// asked for by [`IntoNode::refresh_after`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    None,
    Prompt,
    Asked,
}

/// The state of one designed view the extension opened, as Pane's resource
/// holds it: the view, and the listeners of its last two renders. The SDK
/// makes it around the state [`Command::open_designed_view`] answers
/// with; the resource's one type.
pub struct Open<V: View> {
    state: RefCell<V>,
    /// The listeners of the last two renders, newest last: the render
    /// number and its listeners by callback id.
    tables: RefCell<Vec<(u64, Vec<Run<V>>)>>,
    /// What the view's last render asked Pane for (see [`Ask`]): the
    /// render that answers it knows why it was asked.
    ask: Cell<Ask>,
    /// Whether an event was handled since the last render: its render is
    /// not one that answers a refresh.
    after_event: Cell<bool>,
}

impl<V: View> Open<V> {
    /// The resource around `state`, its listener tables empty.
    pub(crate) fn new(state: V) -> Open<V> {
        Open {
            state: RefCell::new(state),
            tables: RefCell::new(Vec::new()),
            ask: Cell::new(Ask::None),
            after_event: Cell::new(false),
        }
    }
}

impl<V: View> GuestView for Open<V> {
    async fn render(&self, context: String) -> Result<Rendered, String> {
        let render = render_of(&context);
        let mut listeners: Vec<Run<V>> = Vec::new();
        // This render answers the refresh the view asked for by
        // `refresh_after` — not the prompt a loading state asked for (that
        // one awaits its data, see `Pending`), and not one an event's
        // answer asked for — which is where an interval's work runs.
        let refreshed = self.ask.get() == Ask::Asked && !self.after_event.replace(false);
        let answer = {
            let mut cx = Cx {
                listeners: &mut listeners,
                refreshed,
            };
            self.state.borrow_mut().render(&mut cx).into_answer()
        };
        self.ask.set(match answer.refresh {
            Some(Refresh::Prompt) => Ask::Prompt,
            Some(Refresh::After(_)) => Ask::Asked,
            None => Ask::None,
        });
        let mut tree = String::new();
        write_node(&mut tree, &answer.node)?;
        let document = format!("{{\"version\":\"{COMPONENT_SET}\",\"root\":{tree}}}");
        // Keep the last two renders' listeners: an event raised on the tree
        // the user saw is delivered even if the view has rendered since.
        self.tables
            .borrow_mut()
            .retain(|(number, _)| *number + 1 >= render);
        self.tables.borrow_mut().push((render, listeners));
        Ok(Rendered {
            tree: document,
            // The ask, as milliseconds: a duration Pane clamps to the
            // floor and the ceiling of refreshes.
            refresh_after_ms: answer.refresh.map(|refresh| match refresh {
                Refresh::Prompt => PROMPT_REFRESH_MS,
                Refresh::After(after) => ms_of(after),
            }),
        })
    }

    async fn handle_event(&self, event: UiEvent) -> Result<Outcome, String> {
        // The render that follows this answers the event, not a refresh.
        self.after_event.set(true);
        let UiEvent {
            render, callback, ..
        } = event;
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
        if let Some(run) = taken {
            run(&mut self.state.borrow_mut());
        }
        Ok(Outcome {
            // The navigation stack lands with #239.
            push: None,
            replace: None,
            pop: None,
        })
    }
}

/// `after` as milliseconds, capped where Pane clamps the ceiling of
/// refreshes anyway.
fn ms_of(after: Duration) -> u32 {
    u32::try_from(after.as_millis()).unwrap_or(u32::MAX)
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
        Node::Column { layout, children } => {
            tree.push_str("{\"type\":\"column\"");
            write_layout(tree, layout)?;
            tree.push_str(",\"children\":[");
            write_nodes(tree, children)?;
            tree.push_str("]}");
        }
        Node::Row { layout, children } => {
            tree.push_str("{\"type\":\"row\"");
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
