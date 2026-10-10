//! What a designed view renders, as its `view.render` answers it: a JSON
//! document naming the version of the UI component set it uses, and a root
//! node (`docs/designed-tree.md`, ADR 0036's envelope for the tree #121
//! introduces). Reading it here keeps the JSON at the edge of the runtime:
//! the launcher sees only the typed [`DesignedTree`], and the window draws
//! from it without meeting JSON at all.
//!
//! Reading is strict in the document's shape and lenient exactly where
//! versioning asks for it. A property Pane does not know is ignored; a
//! node whose type Pane does not know, or whose `requires` it does not
//! meet, draws the `fallback` the author gave, else its children; a
//! document of Pane's major version and any minor is read for what Pane
//! understands of it. A document that is not JSON, lacks a field Pane
//! needs (`version`, `root`, a node's `type`, a text's `text`, a button's
//! `label`) or gives one of the wrong type is unreadable, which the runtime
//! answers as [`super::CallError::Unreadable`]: the command's failure,
//! never a crash, so it does not count towards pausing the package. A
//! document over one of the limits ([`MAX_NODES`], [`MAX_DEPTH`],
//! [`MAX_TREE_BYTES`], [`MAX_TEXT_CHARS`], [`MAX_MARKDOWN_CHARS`],
//! [`MAX_INLINE_IMAGE`]), or of another major version, is the extension's
//! error ([`ReadError::Guest`]): the view keeps its last good tree either
//! way.
//!
//! The component set of this Pane (1.1) adds the layout primitives
//! (`stack`, `scroll`, `spacer`, `divider`), the sizing and surface every
//! node may carry with their `hover` and `pressed` variants, the shared
//! UI components, the tone token's semantic names, raw colours in every
//! form and Markdown — everything #237 names.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::icons::{self, Icon, Tint};
use crate::markdown;
use crate::tokens::{IconSize, Radius, Space, TextLevel, TextStyle};

/// The version of the UI component set this Pane renders: major 1, minor 1.
/// A document of this major and any minor is read (unknown fields and nodes
/// degrading); a document of another major is refused naming both versions.
pub const COMPONENT_SET: (u64, u64) = (1, 1);

/// The most nodes one document may hold, counting `fallback` subtrees.
pub const MAX_NODES: usize = 10_000;
/// How deep one document may be, counting its root.
pub const MAX_DEPTH: usize = 64;
/// The most bytes of JSON one document may be.
pub const MAX_TREE_BYTES: usize = 4 * 1024 * 1024;
/// The most characters one text node may hold.
pub const MAX_TEXT_CHARS: usize = 64 * 1024;
/// The most characters one markdown node's source may hold.
pub const MAX_MARKDOWN_CHARS: usize = 1024 * 1024;
/// The most bytes of inline image data one icon's `data:` URL may hold.
pub const MAX_INLINE_IMAGE: usize = 1024 * 1024;
/// The most pixels a raw length may name, and the least.
pub const MAX_PX: f32 = 4096.;

/// A designed view's tree: its root node. The version it named is checked
/// while reading; a tree that was read is of a version Pane renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignedTree {
    pub root: Node,
}

/// One node of a designed view's tree: a layout primitive or UI component,
/// or a type this Pane does not know (drawn by its `fallback`, else its
/// children). Every node may carry a `key` (the stable identity Pane keeps
/// node state under, #238), a `name` assistive technology reads it by, a
/// `requires` minimum minor version, a `fallback` subtree, and the
/// [`Style`] every node shares; a child of a `stack` may also say where in
/// it it is placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    /// The sizing and surface every node may carry, with their `hover` and
    /// `pressed` variants.
    pub style: Style,
    /// Where in a `stack` this node is placed; `None` for the stack's own
    /// placement. Nowhere else is it read.
    pub place: Option<Place>,
    /// How far this node sits from its place in a `stack`, in its
    /// placement's directions.
    pub offset: Option<Offset>,
    /// The node's key, unique among its siblings; `None` for a static node.
    pub key: Option<String>,
    /// The name assistive technology reads the node by, when the node's own
    /// content does not name it (a column's or row's).
    pub name: Option<String>,
    /// The minimum minor version of the UI component set this node needs.
    pub requires: Option<u64>,
    /// Drawn instead of this node when Pane does not know it.
    pub fallback: Option<Box<Node>>,
    pub children: Vec<Node>,
}

/// What a node is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Column(Layout),
    Row(Layout),
    /// Children drawn over each other, each placed by its [`Node::place`].
    Stack(Place),
    /// A scrolling region, whose position Pane keeps by key.
    Scroll {
        orientation: Orientation,
    },
    /// Space: it grows to fill what it is given.
    Spacer,
    /// A hairline rule.
    Divider {
        orientation: Orientation,
    },
    Text(Text),
    Button(Button),
    Link(Link),
    /// One icon, by name, file, URL or system reference.
    Icon(IconNode),
    /// An icon on Pane's tile.
    IconTile(IconNode),
    /// An image, with its fit and its placeholder while it loads.
    Image(Image),
    /// A row the launcher's own results are: an icon, a title, a subtitle
    /// and accessories.
    RichRow(RichRow),
    Keycap(Keycap),
    /// Keycaps side by side, one per key.
    KeySequence(KeySequence),
    Tag(Tag),
    Badge(Badge),
    Toggle(Toggle),
    Checkbox(Checkbox),
    /// A choice of segments, one of them chosen.
    Segmented(Segmented),
    Slider(Slider),
    Progress(Progress),
    /// A loading indicator, indeterminate.
    Loading(Loading),
    Markdown(Markdown),
    /// A card of children, on Pane's own card surface.
    Card(Layout),
    SectionHeader(SectionHeader),
    MetadataList(MetadataList),
    EmptyState(EmptyState),
    TextInput(TextInput),
    PasswordInput(TextInput),
    TextArea(TextInput),
    Select(Select),
    /// A node whose type Pane does not know, or whose `requires` it does
    /// not meet: its `fallback` and children decide what is drawn.
    Unknown(String),
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

/// What a `column`, `row` or `card` says about its layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// The gap between its children.
    pub gap: Option<Space>,
    /// The padding inside it.
    pub padding: Padding,
    /// How children are laid out along the cross axis.
    pub align: Option<Align>,
    /// How children share the main axis.
    pub justify: Option<Justify>,
    /// Whether children wrap onto further lines.
    pub wrap: bool,
}

/// The padding inside a node, per side: `{"x": "m", "y": "s"}` names the
/// sides, `{"top": ..., "right": ..., "bottom": ..., "left": ...}` each
/// one, and a bare token all four.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Padding {
    pub top: Option<Space>,
    pub right: Option<Space>,
    pub bottom: Option<Space>,
    pub left: Option<Space>,
}

/// One of the nine places a `stack`'s child may be put.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    TopStart,
    Top,
    TopEnd,
    Start,
    Center,
    End,
    BottomStart,
    Bottom,
    BottomEnd,
}

/// How far a `stack`'s child sits from its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offset {
    pub x: Length,
    pub y: Length,
}

/// Which way a `scroll` scrolls, or a `divider` runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

/// One text node: what it says, plain or in spans, and how it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub content: TextContent,
    pub style: Option<TextStyle>,
    pub level: Option<TextLevel>,
    /// The colour it is drawn in, a tone or a raw colour.
    pub color: Option<Paint>,
    /// Its size in pixels, when the styles do not say what it needs.
    pub size: Option<Finite>,
    /// Its weight, 100 to 900.
    pub weight: Option<Finite>,
    /// Whether it is drawn on one line with an ellipsis.
    pub truncate: bool,
}

/// What a text says: one string, or runs of styled and linked spans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextContent {
    Plain(String),
    Spans(Vec<Span>),
}

/// One span of a text: a run of its content, styled, and a link when it
/// carries `onPress`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Option<TextStyle>,
    pub level: Option<TextLevel>,
    pub color: Option<Paint>,
    /// Whether it is drawn in the mono family, as code is.
    pub code: bool,
    /// The callback id a press of the link runs.
    pub on_press: Option<u32>,
}

/// One button: its label, its tone, the icon and keycaps it carries, and
/// the callback a `press` of it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub tone: Option<Tone>,
    /// The icon drawn before its label.
    pub icon: Option<Icon>,
    /// The keys drawn after its label, one keycap each.
    pub keys: Option<Vec<String>>,
    /// The callback id the tree named; `None` for a button that cannot be
    /// pressed.
    pub on_press: Option<u32>,
    /// Whether it can be pressed.
    pub enabled: bool,
}

impl Default for Button {
    fn default() -> Button {
        Button {
            label: String::new(),
            tone: None,
            icon: None,
            keys: None,
            on_press: None,
            enabled: true,
        }
    }
}

/// One link: its label and the callback a `press` of it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub on_press: Option<u32>,
    pub color: Option<Paint>,
}

/// One icon or image node: the icon it draws, and how big.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IconNode {
    /// The icon, read leniently: one Pane cannot read is not drawn.
    pub icon: Option<Icon>,
    pub size: Option<IconExtent>,
}

/// One image node: the image it draws, how it fits its box, and its
/// placeholder (its children) while it loads or cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub image: Option<Icon>,
    pub size: Option<IconExtent>,
    pub fit: Fit,
}

/// How an image fits the box it is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    #[default]
    Contain,
    Cover,
    Fill,
}

/// How big an icon or an image is: a token, or pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconExtent {
    Token(IconSize),
    Px(Finite),
}

/// A rich row: the launcher's own result row as a component.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichRow {
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<Icon>,
    pub accessories: Vec<RowAccessory>,
    pub on_press: Option<u32>,
}

/// One accessory at a rich row's end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowAccessory {
    pub text: String,
    /// Whether it is drawn as a tag.
    pub tag: bool,
    pub color: Option<Paint>,
}

/// One keycap: the key its cap shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keycap {
    pub key: String,
}

/// A key sequence: the keys its caps show, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySequence {
    pub keys: Vec<String>,
}

/// One tag: a short label in a chip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub text: String,
    pub color: Option<Paint>,
}

/// One badge: a short count or state in a filled chip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub text: String,
    pub color: Option<Paint>,
}

/// One toggle: on or off, and the callback a change of it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toggle {
    pub on: bool,
    pub on_change: Option<u32>,
    /// Its label, drawn beside it and naming it to assistive technology.
    pub label: Option<String>,
}

/// One checkbox: checked or not, and the callback a change of it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkbox {
    pub checked: bool,
    pub on_change: Option<u32>,
    /// Its label, drawn beside it and naming it to assistive technology.
    pub label: Option<String>,
}

/// A segmented control: its segments and the one chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segmented {
    pub options: Vec<Segment>,
    pub value: Option<String>,
    pub on_change: Option<u32>,
    /// Its label, naming it to assistive technology.
    pub label: Option<String>,
}

/// A select: its options and the one chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Select {
    pub options: Vec<Segment>,
    pub value: Option<String>,
    pub on_change: Option<u32>,
    /// Its label, naming it to assistive technology.
    pub label: Option<String>,
}

/// One option of a segmented control or a select.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub value: String,
    /// What is drawn for it; its `value` when the tree gives none.
    pub label: Option<String>,
}

/// One slider: its value between its bounds, and the callback a change of
/// it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slider {
    pub value: Finite,
    pub min: Finite,
    pub max: Finite,
    pub step: Finite,
    pub on_change: Option<u32>,
    /// Its label, naming it to assistive technology.
    pub label: Option<String>,
}

/// One progress bar: how far along it is, 0 to 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub value: Finite,
    /// Its label, naming it to assistive technology.
    pub label: Option<String>,
}

/// One loading indicator, indeterminate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loading {
    pub label: Option<String>,
}

/// One markdown node: its source, parsed into blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markdown {
    pub blocks: Vec<markdown::Block>,
}

/// A section header: a title over a group, with an optional note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionHeader {
    pub title: String,
    pub note: Option<String>,
}

/// A metadata list: rows of a label and its value, link or tags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetadataList {
    pub items: Vec<MetadataItem>,
}

/// One row of a metadata list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetadataItem {
    pub label: Option<String>,
    pub value: Option<String>,
    /// The callback a `press` of the value runs, making it a link.
    pub on_press: Option<u32>,
    /// Tag chips drawn as the row's value.
    pub tags: Vec<String>,
    /// Whether the row is a separator alone.
    pub separator: bool,
}

/// An empty state: an icon, a title and a description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyState {
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<Icon>,
}

/// One text input, password field or text area, as the tree draws it: its
/// value and placeholder. Editing arrives with #238; until then the value
/// the tree names is what is drawn, and a commit tells the extension that
/// value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInput {
    pub value: String,
    pub placeholder: Option<String>,
    /// The callback a commit of the field runs.
    pub on_change: Option<u32>,
    /// Its label, naming it to assistive technology.
    pub label: Option<String>,
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

/// A finite number as the tree gives one: a pixel length, a fraction, a
/// weight, a value. [`Eq`] holds because reading keeps every number
/// finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finite(pub f32);

impl Eq for Finite {}

/// The sizing every node may set: how it takes space and how big it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sizing {
    pub grow: Option<Finite>,
    pub shrink: Option<Finite>,
    pub basis: Option<Length>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_height: Option<Length>,
    /// Its width over its height; a non-positive one is left out.
    pub aspect_ratio: Option<Finite>,
}

/// The surface every node may draw: its background, border, corner radius
/// and opacity, with the `hover` and `pressed` variants Pane applies
/// itself, without calling the extension.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Surface {
    pub background: Option<Paint>,
    pub border: Option<Border>,
    pub radius: Option<RadiusLength>,
    /// 0 to 1.
    pub opacity: Option<Finite>,
}

/// The border every node may draw: how wide, and in which colour.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Border {
    /// One pixel when the tree gives none.
    pub width: Option<Length>,
    /// The theme's own edge colour when the tree gives none.
    pub color: Option<Paint>,
}

/// A corner radius: a token, or pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadiusLength {
    Token(Radius),
    Px(Finite),
}

/// The style every node may carry: its [`Sizing`] and [`Surface`], and the
/// variants of the surface Pane applies itself while the pointer is over
/// the node or it is pressed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub sizing: Sizing,
    pub surface: Surface,
    pub hover: Option<Surface>,
    pub pressed: Option<Surface>,
}

impl Style {
    /// Whether nothing is set: a node whose style is this is drawn with no
    /// wrapper.
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

/// One length: a space token, pixels, or a fraction of the parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Space(Space),
    Px(Finite),
    Fraction(Finite),
}

/// One colour property's value: the colour, and whether Pane corrects its
/// contrast against the surface it is drawn on. A colour given as
/// `{"raw": ...}` is drawn exactly as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paint {
    pub tint: Tint,
    /// Whether contrast correction is off for this colour.
    pub exact: bool,
}

impl DesignedTree {
    /// The tree `document` describes, or why Pane cannot read it: an
    /// over-limit tree, or a document of another major version, is the
    /// extension's error ([`ReadError::Guest`]); a document Pane cannot
    /// read says why ([`ReadError::Unreadable`]).
    pub fn read(document: &str) -> Result<DesignedTree, ReadError> {
        if document.len() > MAX_TREE_BYTES {
            return Err(ReadError::Guest(format!(
                "the view's tree is {} bytes; at most {MAX_TREE_BYTES} are read",
                document.len()
            )));
        }
        // Serde's own recursion bound (128) sits below a tree as deep as
        // the limit allows: each level is an object and an array. The
        // document's bracket nesting is counted over its text first — no
        // recursion, whatever the document holds — and a document nesting
        // deeper than a tree at the limit can is over the depth limit, the
        // extension's error, never parsed. What is left is bounded, so the
        // parse runs with serde's recursion bound lifted: it cannot
        // overflow, and the tree's own depth is checked as it is read.
        if nesting_of(document) > 2 * MAX_DEPTH + 8 {
            return Err(ReadError::Guest(format!(
                "the view's tree is deeper than the {MAX_DEPTH} levels that are drawn"
            )));
        }
        let mut parser = serde_json::Deserializer::from_str(document);
        parser.disable_recursion_limit();
        let wire: WireDocument = WireDocument::deserialize(&mut parser)
            .map_err(|error| ReadError::Unreadable(format!("its tree: {error}")))?;
        let (major, minor) = version(&wire.version).ok_or_else(|| {
            ReadError::Unreadable(format!(
                "its tree names version {:?}, which is not \"<major>.<minor>\"",
                wire.version
            ))
        })?;
        if major != COMPONENT_SET.0 {
            return Err(ReadError::Guest(format!(
                "its tree uses UI component set {major}.{minor}; this Pane renders {}.{}",
                COMPONENT_SET.0, COMPONENT_SET.1
            )));
        }
        let mut nodes = 0;
        let root = node(wire.root, 1, &mut nodes)?;
        Ok(DesignedTree { root })
    }
}

/// Why a tree could not be read: [`ReadError::Guest`] is the extension's
/// error (the view keeps its last good tree), [`ReadError::Unreadable`]
/// the failure Pane reports as its own reading of what the extension
/// answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    Guest(String),
    Unreadable(String),
}

impl ReadError {
    /// The error's message.
    pub fn message(&self) -> &str {
        match self {
            ReadError::Guest(message) | ReadError::Unreadable(message) => message,
        }
    }
}

/// The deepest nesting of brackets in `document`, counted over its text:
/// a bound on the depth its parse reaches, without parsing it. Text in
/// strings does not count.
fn nesting_of(document: &str) -> usize {
    let mut nesting: usize = 0;
    let mut deepest: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    for character in document.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => {
                nesting += 1;
                deepest = deepest.max(nesting);
            }
            '}' | ']' => nesting = nesting.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

/// `text`, a version like `"1.0"`, as (major, minor).
fn version(text: &str) -> Option<(u64, u64)> {
    let (major, minor) = text.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The document's root: its version and its root node.
#[derive(Deserialize)]
struct WireDocument {
    version: String,
    root: WireNode,
}

/// A node as the tree gives it: its type, its common fields, and whatever
/// else it says, for its own kind to read.
#[derive(Deserialize)]
struct WireNode {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    requires: Option<u64>,
    #[serde(default)]
    fallback: Option<Box<WireNode>>,
    #[serde(default)]
    children: Option<Vec<Value>>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The node `wire` describes at `depth`, counting it in `nodes`, or why it
/// cannot be read.
fn node(wire: WireNode, depth: usize, nodes: &mut usize) -> Result<Node, ReadError> {
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(ReadError::Guest(format!(
            "the view has {nodes} nodes; at most {MAX_NODES} are drawn"
        )));
    }
    if depth > MAX_DEPTH {
        return Err(ReadError::Guest(format!(
            "the view's tree is {depth} levels deep; at most {MAX_DEPTH} are drawn"
        )));
    }
    // A node whose `requires` this Pane does not meet is one it does not
    // know: it asks for a later minor version than the one Pane renders,
    // and degrades as an unknown node does.
    let meets = wire
        .requires
        .is_none_or(|required| required <= COMPONENT_SET.1);
    let kind = if !meets {
        NodeKind::Unknown(wire.kind.clone())
    } else {
        match wire.kind.as_str() {
            "column" => NodeKind::Column(layout(&wire)?),
            "row" => NodeKind::Row(layout(&wire)?),
            "stack" => NodeKind::Stack(place_of(&wire, "align")?.unwrap_or_default()),
            "scroll" => NodeKind::Scroll {
                orientation: orientation_of(&wire, "orientation")?.unwrap_or_default(),
            },
            "spacer" => NodeKind::Spacer,
            "divider" => NodeKind::Divider {
                orientation: orientation_of(&wire, "orientation")?.unwrap_or_default(),
            },
            "text" => NodeKind::Text(text(&wire)?),
            "button" => NodeKind::Button(button(&wire)?),
            "link" => NodeKind::Link(link(&wire)?),
            "icon" => NodeKind::Icon(icon_node(&wire, "icon")?),
            "icon-tile" => NodeKind::IconTile(icon_node(&wire, "icon")?),
            "image" => NodeKind::Image(image(&wire)?),
            "rich-row" => NodeKind::RichRow(rich_row(&wire)?),
            "keycap" => NodeKind::Keycap(keycap(&wire)?),
            "key-sequence" => NodeKind::KeySequence(key_sequence(&wire)?),
            "tag" => NodeKind::Tag(texted(&wire, "text")?),
            "badge" => NodeKind::Badge(badge(&wire)?),
            "toggle" => NodeKind::Toggle(Toggle {
                on: boolean(&wire, "on")?.unwrap_or(false),
                on_change: callback(&wire, "onChange")?,
                label: string(&wire, "label")?,
            }),
            "checkbox" => NodeKind::Checkbox(Checkbox {
                checked: boolean(&wire, "checked")?.unwrap_or(false),
                on_change: callback(&wire, "onChange")?,
                label: string(&wire, "label")?,
            }),
            "segmented" => NodeKind::Segmented(segmented(&wire)?),
            "slider" => NodeKind::Slider(slider(&wire)?),
            "progress" => NodeKind::Progress(Progress {
                value: number(&wire, "value")?.ok_or(unreadable("a progress node has no value"))?,
                label: string(&wire, "label")?,
            }),
            "loading" => NodeKind::Loading(Loading {
                label: string(&wire, "label")?,
            }),
            "markdown" => NodeKind::Markdown(markdown_node(&wire)?),
            "card" => NodeKind::Card(layout(&wire)?),
            "section-header" => NodeKind::SectionHeader(section_header(&wire)?),
            "metadata-list" => NodeKind::MetadataList(metadata_list(&wire)?),
            "empty-state" => NodeKind::EmptyState(empty_state(&wire)?),
            "text-input" => NodeKind::TextInput(text_input(&wire)?),
            "password-input" => NodeKind::PasswordInput(text_input(&wire)?),
            "text-area" => NodeKind::TextArea(text_input(&wire)?),
            "select" => NodeKind::Select(select(&wire)?),
            _ => NodeKind::Unknown(wire.kind.clone()),
        }
    };
    let style = style(&wire)?;
    let place = place_of(&wire, "place")?;
    let offset = offset_of(&wire)?;
    let children = wire
        .children
        .unwrap_or_default()
        .into_iter()
        .map(|child| {
            serde_json::from_value::<WireNode>(child)
                .map_err(|error| ReadError::Unreadable(format!("its tree: {error}")))
                .and_then(|child| node(child, depth + 1, nodes))
        })
        .collect::<Result<Vec<Node>, ReadError>>()?;
    let fallback = wire
        .fallback
        .map(|fallback| node(*fallback, depth + 1, nodes).map(Box::new))
        .transpose()?;
    Ok(Node {
        kind,
        style,
        place,
        offset,
        key: wire.key,
        name: wire.name,
        requires: wire.requires,
        fallback,
        children,
    })
}

/// The layout a column, row or card's other properties give it: `gap`,
/// `padding`, `align`, `justify` and `wrap`. A property of the wrong type
/// is unreadable; a token this version does not know is left out, as a
/// newer minor version may add one.
fn layout(wire: &WireNode) -> Result<Layout, ReadError> {
    let over = |message: String| ReadError::Unreadable(message);
    let rest = &wire.rest;
    Ok(Layout {
        gap: token(rest.get("gap"), "gap", Space::named)?,
        padding: padding(rest.get("padding")).map_err(over)?,
        align: token(rest.get("align"), "align", align)?,
        justify: token(rest.get("justify"), "justify", justify)?,
        wrap: match rest.get("wrap") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(wrap)) => *wrap,
            Some(_) => return Err(over("its wrap is not a boolean".into())),
        },
    })
}

/// The text a text node's other properties give it: its `text` or its
/// `spans`, and its style, level, colour, size, weight and truncation.
fn text(wire: &WireNode) -> Result<Text, ReadError> {
    let rest = &wire.rest;
    let content = match (rest.get("text"), rest.get("spans")) {
        (Some(Value::String(content)), None) => TextContent::Plain(content.clone()),
        (None, Some(Value::Array(spans))) => TextContent::Spans(
            spans
                .iter()
                .map(|span| span_of(span))
                .collect::<Result<Vec<Span>, ReadError>>()?,
        ),
        (None, None) => return Err(unreadable("a text node has no text")),
        (Some(Value::String(_)), Some(_)) => {
            return Err(unreadable("a text node names both text and spans"))
        }
        (Some(_), _) => return Err(unreadable("its text is not a string")),
        (None, Some(_)) => return Err(unreadable("its spans are not a list")),
    };
    let length = match &content {
        TextContent::Plain(content) => content.chars().count(),
        TextContent::Spans(spans) => spans.iter().map(|span| span.text.chars().count()).sum(),
    };
    if length > MAX_TEXT_CHARS {
        return Err(ReadError::Guest(format!(
            "a text of the view has {length} characters; at most {MAX_TEXT_CHARS} are drawn"
        )));
    }
    Ok(Text {
        content,
        style: token(rest.get("style"), "style", text_style)?,
        level: token(rest.get("level"), "level", text_level)?,
        color: paint(rest.get("color"), "color")?,
        size: sized(rest.get("size"), "size")?,
        weight: weighted(rest.get("weight"), "weight")?,
        truncate: boolean(wire, "truncate")?.unwrap_or(false),
    })
}

/// One span of a text: its `text`, its style, level and colour, whether it
/// is code, and its `onPress` when it is a link.
fn span_of(value: &Value) -> Result<Span, ReadError> {
    let Value::Object(fields) = value else {
        return Err(unreadable("a span of its text is not an object"));
    };
    let text = match fields.get("text") {
        Some(Value::String(text)) => text.clone(),
        _ => return Err(unreadable("a span of its text has no text")),
    };
    let wire = WireNode {
        kind: "text".into(),
        key: None,
        name: None,
        requires: None,
        fallback: None,
        children: None,
        rest: fields.clone(),
    };
    Ok(Span {
        text,
        style: token(fields.get("style"), "style", text_style)?,
        level: token(fields.get("level"), "level", text_level)?,
        color: paint(fields.get("color"), "color")?,
        code: boolean(&wire, "code")?.unwrap_or(false),
        on_press: callback(&wire, "onPress")?,
    })
}

/// The button a button node's other properties give it: its `label`,
/// `tone`, `icon`, `keys`, `onPress` and `enabled`.
fn button(wire: &WireNode) -> Result<Button, ReadError> {
    let rest = &wire.rest;
    let label = match rest.get("label") {
        Some(Value::String(label)) => label.clone(),
        Some(_) => return Err(unreadable("its label is not a string")),
        None => return Err(unreadable("a button node has no label")),
    };
    let keys = match rest.get("keys") {
        None | Some(Value::Null) => None,
        Some(Value::Array(keys)) => {
            let keys = keys
                .iter()
                .map(|key| match key {
                    Value::String(key) => Ok(key.clone()),
                    _ => Err(unreadable("a key of its keys is not a string")),
                })
                .collect::<Result<Vec<String>, ReadError>>()?;
            (!keys.is_empty()).then_some(keys)
        }
        Some(_) => return Err(unreadable("its keys are not a list")),
    };
    Ok(Button {
        label,
        tone: token(rest.get("tone"), "tone", tone)?,
        icon: icon_of(rest.get("icon"), "icon")?,
        keys,
        on_press: callback(wire, "onPress")?,
        enabled: boolean(wire, "enabled")?.unwrap_or(true),
    })
}

/// The link a link node's other properties give it.
fn link(wire: &WireNode) -> Result<Link, ReadError> {
    let rest = &wire.rest;
    let label = match rest.get("label") {
        Some(Value::String(label)) => label.clone(),
        Some(_) => return Err(unreadable("its label is not a string")),
        None => return Err(unreadable("a link node has no label")),
    };
    Ok(Link {
        label,
        on_press: callback(wire, "onPress")?,
        color: paint(rest.get("color"), "color")?,
    })
}

/// The icon an icon or icon-tile node draws: its `icon` and `size`. The
/// icon is read leniently, as the list tree's are: one Pane cannot read is
/// not drawn, its node drawing nothing (or, for an image, its
/// placeholder).
fn icon_node(wire: &WireNode, property: &str) -> Result<IconNode, ReadError> {
    let rest = &wire.rest;
    Ok(IconNode {
        icon: icon_of(rest.get(property), property)?,
        size: extent(rest.get("size"), "size")?,
    })
}

/// The image an image node draws: its `image`, `size` and `fit`, with its
/// children drawn as its placeholder.
fn image(wire: &WireNode) -> Result<Image, ReadError> {
    let rest = &wire.rest;
    let fit = match rest.get("fit") {
        None | Some(Value::Null) => Fit::Contain,
        Some(Value::String(fit)) => match fit.as_str() {
            "contain" => Fit::Contain,
            "cover" => Fit::Cover,
            "fill" => Fit::Fill,
            other => {
                return Err(unreadable(&format!(
                    "its fit is {other}; a fit is \"contain\", \"cover\" or \"fill\""
                )))
            }
        },
        Some(_) => return Err(unreadable("its fit is not a fit")),
    };
    Ok(Image {
        image: icon_of(rest.get("image"), "image")?,
        size: extent(rest.get("size"), "size")?,
        fit,
    })
}

/// The rich row a rich-row node's other properties give it.
fn rich_row(wire: &WireNode) -> Result<RichRow, ReadError> {
    let rest = &wire.rest;
    let title = match rest.get("title") {
        Some(Value::String(title)) => title.clone(),
        Some(_) => return Err(unreadable("its title is not a string")),
        None => return Err(unreadable("a rich-row node has no title")),
    };
    let accessories = match rest.get("accessories") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(accessories)) => accessories
            .iter()
            .map(|accessory| {
                let Value::Object(fields) = accessory else {
                    return Err(unreadable("an accessory of its row is not an object"));
                };
                let text = match fields.get("text") {
                    Some(Value::String(text)) => text.clone(),
                    _ => return Err(unreadable("an accessory of its row has no text")),
                };
                Ok(RowAccessory {
                    text,
                    tag: match fields.get("tag") {
                        None | Some(Value::Null) => false,
                        Some(Value::Bool(tag)) => *tag,
                        Some(_) => {
                            return Err(unreadable("an accessory's tag is not a boolean"))
                        }
                    },
                    color: paint(fields.get("color"), "color")?,
                })
            })
            .collect::<Result<Vec<RowAccessory>, ReadError>>()?,
        Some(_) => return Err(unreadable("its accessories are not a list")),
    };
    Ok(RichRow {
        title,
        subtitle: string(wire, "subtitle")?,
        icon: icon_of(rest.get("icon"), "icon")?,
        accessories,
        on_press: callback(wire, "onPress")?,
    })
}

/// The keycap a keycap node shows.
fn keycap(wire: &WireNode) -> Result<Keycap, ReadError> {
    let rest = &wire.rest;
    let key = match rest.get("key") {
        Some(Value::String(key)) => key.clone(),
        Some(_) => return Err(unreadable("its key is not a string")),
        None => return Err(unreadable("a keycap node has no key")),
    };
    Ok(Keycap { key })
}

/// The keys a key-sequence node shows.
fn key_sequence(wire: &WireNode) -> Result<KeySequence, ReadError> {
    let rest = &wire.rest;
    let keys = match rest.get("keys") {
        Some(Value::Array(keys)) => keys
            .iter()
            .map(|key| match key {
                Value::String(key) => Ok(key.clone()),
                _ => Err(unreadable("a key of its keys is not a string")),
            })
            .collect::<Result<Vec<String>, ReadError>>()?,
        Some(_) => return Err(unreadable("its keys are not a list")),
        None => return Err(unreadable("a key-sequence node has no keys")),
    };
    Ok(KeySequence { keys })
}

/// A tag or badge node: its `text` and `color`.
fn texted(wire: &WireNode, property: &str) -> Result<Tag, ReadError> {
    let rest = &wire.rest;
    let text = match rest.get(property) {
        Some(Value::String(text)) => text.clone(),
        Some(_) => return Err(unreadable("its text is not a string")),
        None => return Err(unreadable("a tag node has no text")),
    };
    Ok(Tag {
        text,
        color: paint(rest.get("color"), "color")?,
    })
}

/// A badge, drawn from a tag's reading.
fn badge(wire: &WireNode) -> Result<Badge, ReadError> {
    let tag = texted(wire, "text")?;
    Ok(Badge {
        text: tag.text,
        color: tag.color,
    })
}

/// A segmented control's options, value and callback.
fn segmented(wire: &WireNode) -> Result<Segmented, ReadError> {
    Ok(Segmented {
        options: options(wire, "options")?,
        value: string(wire, "value")?,
        on_change: callback(wire, "onChange")?,
        label: string(wire, "label")?,
    })
}

/// A select's options, value and callback.
fn select(wire: &WireNode) -> Result<Select, ReadError> {
    Ok(Select {
        options: options(wire, "options")?,
        value: string(wire, "value")?,
        on_change: callback(wire, "onChange")?,
        label: string(wire, "label")?,
    })
}

/// The options a segmented control or select names.
fn options(wire: &WireNode, property: &str) -> Result<Vec<Segment>, ReadError> {
    let rest = &wire.rest;
    let options = match rest.get(property) {
        Some(Value::Array(options)) => options
            .iter()
            .map(|option| {
                let Value::Object(fields) = option else {
                    return Err(unreadable("an option of it is not an object"));
                };
                let value = match fields.get("value") {
                    Some(Value::String(value)) => value.clone(),
                    _ => return Err(unreadable("an option of it has no value")),
                };
                Ok(Segment {
                    value,
                    label: match fields.get("label") {
                        Some(Value::String(label)) => Some(label.clone()),
                        None | Some(Value::Null) => None,
                        Some(_) => {
                            return Err(unreadable("an option's label is not a string"))
                        }
                    },
                })
            })
            .collect::<Result<Vec<Segment>, ReadError>>()?,
        Some(_) => return Err(unreadable(&format!("its {property} are not a list"))),
        None => return Err(unreadable(&format!("it has no {property}"))),
    };
    Ok(options)
}

/// The slider a slider node's other properties give it.
fn slider(wire: &WireNode) -> Result<Slider, ReadError> {
    let value = number(wire, "value")?.ok_or_else(|| unreadable("a slider node has no value"))?;
    let min = number(wire, "min")?.unwrap_or(Finite(0.));
    let max = number(wire, "max")?.unwrap_or(Finite(1.));
    let step = number(wire, "step")?.unwrap_or(Finite(0.1));
    Ok(Slider {
        value,
        min,
        max,
        step,
        on_change: callback(wire, "onChange")?,
        label: string(wire, "label")?,
    })
}

/// The markdown a markdown node holds, parsed into blocks.
fn markdown_node(wire: &WireNode) -> Result<Markdown, ReadError> {
    let rest = &wire.rest;
    let source = match rest.get("markdown") {
        Some(Value::String(source)) => source.clone(),
        Some(_) => return Err(unreadable("its markdown is not a string")),
        None => return Err(unreadable("a markdown node has no markdown")),
    };
    if source.chars().count() > MAX_MARKDOWN_CHARS {
        return Err(ReadError::Guest(format!(
            "the view's markdown has {} characters; at most {MAX_MARKDOWN_CHARS} are read",
            source.chars().count()
        )));
    }
    Ok(Markdown {
        blocks: markdown::parse(&source),
    })
}

/// The section header a section-header node's properties give it.
fn section_header(wire: &WireNode) -> Result<SectionHeader, ReadError> {
    let rest = &wire.rest;
    let title = match rest.get("title") {
        Some(Value::String(title)) => title.clone(),
        Some(_) => return Err(unreadable("its title is not a string")),
        None => return Err(unreadable("a section-header node has no title")),
    };
    Ok(SectionHeader {
        title,
        note: string(wire, "note")?,
    })
}

/// The metadata list a metadata-list node's items give it.
fn metadata_list(wire: &WireNode) -> Result<MetadataList, ReadError> {
    let rest = &wire.rest;
    let items = match rest.get("items") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let Value::Object(fields) = item else {
                    return Err(unreadable("an item of its list is not an object"));
                };
                let separator = match fields.get("separator") {
                    None | Some(Value::Null) => false,
                    Some(Value::Bool(separator)) => *separator,
                    Some(_) => return Err(unreadable("an item's separator is not a boolean")),
                };
                let tags = match fields.get("tags") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(tags)) => tags
                        .iter()
                        .map(|tag| match tag {
                            Value::String(tag) => Ok(tag.clone()),
                            _ => Err(unreadable("a tag of its item is not a string")),
                        })
                        .collect::<Result<Vec<String>, ReadError>>()?,
                    Some(_) => return Err(unreadable("its item's tags are not a list")),
                };
                let wire = WireNode {
                    kind: "item".into(),
                    key: None,
                    name: None,
                    requires: None,
                    fallback: None,
                    children: None,
                    rest: fields.clone(),
                };
                Ok(MetadataItem {
                    label: string(&wire, "label")?,
                    value: string(&wire, "value")?,
                    on_press: callback(&wire, "onPress")?,
                    tags,
                    separator,
                })
            })
            .collect::<Result<Vec<MetadataItem>, ReadError>>()?,
        Some(_) => return Err(unreadable("its items are not a list")),
    };
    Ok(MetadataList { items })
}

/// The empty state an empty-state node's properties give it.
fn empty_state(wire: &WireNode) -> Result<EmptyState, ReadError> {
    let rest = &wire.rest;
    let title = match rest.get("title") {
        Some(Value::String(title)) => title.clone(),
        Some(_) => return Err(unreadable("its title is not a string")),
        None => return Err(unreadable("an empty-state node has no title")),
    };
    Ok(EmptyState {
        title,
        description: string(wire, "description")?,
        icon: icon_of(rest.get("icon"), "icon")?,
    })
}

/// The text input a text-input, password-input or text-area node's
/// properties give it.
fn text_input(wire: &WireNode) -> Result<TextInput, ReadError> {
    Ok(TextInput {
        value: string(wire, "value")?.unwrap_or_default(),
        placeholder: string(wire, "placeholder")?,
        on_change: callback(wire, "onChange")?,
        label: string(wire, "label")?,
    })
}

/// The style every node carries: its sizing, its surface, and its hover
/// and pressed variants.
fn style(wire: &WireNode) -> Result<Style, ReadError> {
    let rest = &wire.rest;
    let mut sizing = Sizing {
        grow: number(wire, "grow")?.map(|Finite(grow)| Finite(grow.max(0.))),
        shrink: number(wire, "shrink")?.map(|Finite(shrink)| Finite(shrink.max(0.))),
        basis: length(rest.get("basis"), "basis")?,
        width: length(rest.get("width"), "width")?,
        height: length(rest.get("height"), "height")?,
        min_width: length(rest.get("minWidth"), "minWidth")?,
        max_width: length(rest.get("maxWidth"), "maxWidth")?,
        min_height: length(rest.get("minHeight"), "minHeight")?,
        max_height: length(rest.get("maxHeight"), "maxHeight")?,
        aspect_ratio: None,
    };
    sizing.aspect_ratio = number(wire, "aspectRatio")?.filter(|Finite(ratio)| *ratio > 0.);
    Ok(Style {
        sizing,
        surface: surface_of(wire)?,
        hover: variant(rest.get("hover"))?,
        pressed: variant(rest.get("pressed"))?,
    })
}

/// The surface a node draws: `background`, `border`, `radius` and
/// `opacity`. `within` names the properties' owner for the messages, as
/// the variants name themselves.
fn surface_of(wire: &WireNode) -> Result<Surface, ReadError> {
    let rest = &wire.rest;
    let border = match rest.get("border") {
        None | Some(Value::Null) => None,
        Some(Value::Object(fields)) => Some(Border {
            width: length(fields.get("width"), "width")?,
            color: paint(fields.get("color"), "color")?,
        })
        .filter(|border| border.width.is_some() || border.color.is_some()),
        Some(_) => return Err(unreadable("its border is not a width and a colour")),
    };
    Ok(Surface {
        background: paint(rest.get("background"), "background")?,
        border,
        radius: radius(rest.get("radius"), "radius")?,
        opacity: number(wire, "opacity")?
            .map(|Finite(opacity)| Finite(opacity.clamp(0., 1.))),
    })
}

/// One variant of a node's surface, for `hover` or `pressed`: the surface
/// properties it restates, applied by Pane itself.
fn variant(value: Option<&Value>) -> Result<Option<Surface>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Value::Object(fields) = value else {
        return Err(unreadable("its variant is not an object of surface properties"));
    };
    let wire = WireNode {
        kind: "node".into(),
        key: None,
        name: None,
        requires: None,
        fallback: None,
        children: None,
        rest: fields.clone(),
    };
    let surface = surface_of(&wire)?;
    Ok((!surface.background.is_none()
        || surface.border.is_some()
        || surface.radius.is_some()
        || surface.opacity.is_some())
    .then_some(surface))
}

/// A token property, as the tree gives it: `Err` when the value is not of
/// the property's type, `None` when it names a token this version does not
/// know. `read` maps the token's name to it.
fn token<T>(
    value: Option<&Value>,
    name: &str,
    read: fn(&str) -> Option<T>,
) -> Result<Option<T>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Value::String(token) = value else {
        return Err(ReadError::Unreadable(format!(
            "its {name} is not one of its names"
        )));
    };
    Ok(read(token))
}

/// A string property: `Err` when it is not a string.
fn string(wire: &WireNode, name: &str) -> Result<Option<String>, ReadError> {
    match wire.rest.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(unreadable(&format!("its {name} is not a string"))),
    }
}

/// A boolean property: `Err` when it is not a boolean.
fn boolean(wire: &WireNode, name: &str) -> Result<Option<bool>, ReadError> {
    match wire.rest.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(unreadable(&format!("its {name} is not a boolean"))),
    }
}

/// A number property: `Err` when it is not a number.
fn number(wire: &WireNode, name: &str) -> Result<Option<Finite>, ReadError> {
    match wire.rest.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => Ok(value
            .as_f64()
            .filter(|value| value.is_finite())
            .map(|value| Finite(value as f32))),
        Some(_) => Err(unreadable(&format!("its {name} is not a number"))),
    }
}

/// A callback id property: `Err` when it is not one.
fn callback(wire: &WireNode, name: &str) -> Result<Option<u32>, ReadError> {
    match wire.rest.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(id)) => Ok(Some(
            id.as_u64()
                .filter(|id| *id <= u32::MAX as u64)
                .ok_or_else(|| {
                    unreadable(&format!("its {name} is not a callback id"))
                })? as u32,
        )),
        Some(_) => Err(unreadable(&format!("its {name} is not a callback id"))),
    }
}

/// A length property: a space token, pixels, or a fraction of the parent
/// (`"1/2"`, `"50%"`), as a number or a string. Raw pixels are clamped to
/// 0–4096.
fn length(value: Option<&Value>, name: &str) -> Result<Option<Length>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    length_of(value, name).map(Some)
}

/// One length value.
fn length_of(value: &Value, name: &str) -> Result<Length, ReadError> {
    let over = |why: String| unreadable(&format!("its {name} {why}"));
    match value {
        Value::Null => Err(over("is not a length".into())),
        Value::Number(number) => Ok(Length::Px(px_of(
            number
                .as_f64()
                .ok_or_else(|| over("is not a length".into()))?
                as f32,
        ))),
        Value::String(text) => {
            let text = text.trim();
            if let Some(space) = Space::named(text) {
                return Ok(Length::Space(space));
            }
            if let Some(fraction) = text.strip_suffix('%').and_then(|digits| {
                digits.trim().parse::<f32>().ok().map(|value| value / 100.)
            }) {
                return Ok(Length::Fraction(Finite(fraction.clamp(0., 1.))));
            }
            if let Some((top, bottom)) = text.split_once('/') {
                if let (Ok(top), Ok(bottom)) =
                    (top.trim().parse::<f32>(), bottom.trim().parse::<f32>())
                {
                    if top >= 0. && bottom > 0. {
                        return Ok(Length::Fraction(Finite(
                            (top / bottom).clamp(0., 1.),
                        )));
                    }
                }
                return Err(over("is not a fraction of the parent".into()));
            }
            let pixels = text
                .strip_suffix("px")
                .unwrap_or(text)
                .trim()
                .parse::<f32>()
                .map_err(|_| over("is not a length".into()))?;
            Ok(Length::Px(px_of(pixels)))
        }
        _ => Err(over("is not a length".into())),
    }
}

/// `value` as pixels, clamped to Pane's bounds.
fn px_of(value: f32) -> Finite {
    Finite(value.clamp(0., MAX_PX))
}

/// An icon size property: a token, or pixels.
fn extent(value: Option<&Value>, name: &str) -> Result<Option<IconExtent>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::String(text) => {
            let text = text.trim();
            if let Some(size) = IconSize::named(text) {
                return Ok(Some(IconExtent::Token(size)));
            }
            let pixels = text
                .strip_suffix("px")
                .unwrap_or(text)
                .trim()
                .parse::<f32>()
                .map_err(|_| unreadable(&format!("its {name} is not a size")))?;
            Ok(Some(IconExtent::Px(px_of(pixels))))
        }
        Value::Number(number) => Ok(Some(IconExtent::Px(px_of(
            number
                .as_f64()
                .ok_or_else(|| unreadable(&format!("its {name} is not a size")))?
                as f32,
        )))),
        _ => Err(unreadable(&format!("its {name} is not a size"))),
    }
}

/// A radius property: a token, or pixels.
fn radius(value: Option<&Value>, name: &str) -> Result<Option<RadiusLength>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::String(text) => {
            let text = text.trim();
            if let Some(radius) = Radius::named(text) {
                return Ok(Some(RadiusLength::Token(radius)));
            }
            let pixels = text
                .strip_suffix("px")
                .unwrap_or(text)
                .trim()
                .parse::<f32>()
                .map_err(|_| unreadable(&format!("its {name} is not a radius")))?;
            Ok(Some(RadiusLength::Px(px_of(pixels))))
        }
        Value::Number(number) => Ok(Some(RadiusLength::Px(px_of(
            number
                .as_f64()
                .ok_or_else(|| unreadable(&format!("its {name} is not a radius")))?
                as f32,
        )))),
        _ => Err(unreadable(&format!("its {name} is not a radius"))),
    }
}

/// A text's `size`: pixels, clamped to what a screen draws.
fn sized(value: Option<&Value>, name: &str) -> Result<Option<Finite>, ReadError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => Ok(number
            .as_f64()
            .map(|value| Finite((value as f32).clamp(1., 128.))),
        ),
        Some(_) => Err(unreadable(&format!("its {name} is not a number"))),
    }
}

/// A text's `weight`: 100 to 900, clamped.
fn weighted(value: Option<&Value>, name: &str) -> Result<Option<Finite>, ReadError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => Ok(number
            .as_f64()
            .map(|value| Finite((value as f32).clamp(100., 900.))),
        ),
        Some(_) => Err(unreadable(&format!("its {name} is not a number"))),
    }
}

/// One colour property: a tone or a raw colour (corrected), a
/// `{"light": ..., "dark": ...}` pair (corrected per appearance), or
/// `{"raw": ...}` — a colour, or a pair, drawn exactly as it is, with
/// contrast correction off. A string naming neither a tone nor a colour
/// is left out, as a token a later version may add.
fn paint(value: Option<&Value>, name: &str) -> Result<Option<Paint>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        // A name this version does not know: a token a later version may
        // add, left out.
        Value::String(text) => Ok(icons::color_of(text).map(|color| Paint {
            tint: Tint::Same(color),
            exact: false,
        })),
        _ => paint_of(value, name, false).map(Some),
    }
}

/// One colour value, `exact` when correction is off for it.
fn paint_of(value: &Value, name: &str, exact: bool) -> Result<Paint, ReadError> {
    let over = |why: String| unreadable(&format!("its {name} {why}"));
    match value {
        Value::String(text) => icons::color_of(text)
            .map(|color| Paint {
                tint: Tint::Same(color),
                exact,
            })
            .ok_or_else(|| over("is not a colour this Pane draws".into())),
        Value::Object(fields) => {
            if let Some(raw) = fields.get("raw") {
                return paint_of(raw, name, true);
            }
            match (fields.get("light"), fields.get("dark")) {
                (Some(Value::String(light)), Some(Value::String(dark))) => {
                    let read = |text: &str| {
                        icons::color_of(text).ok_or_else(|| {
                            over("names a colour this Pane does not draw".into())
                        })
                    };
                    Ok(Paint {
                        tint: Tint::Pair {
                            light: read(light)?,
                            dark: read(dark)?,
                        },
                        exact,
                    })
                }
                (None, None) => Err(over("is not a colour".into())),
                _ => Err(over("names only one of a light and a dark colour".into())),
            }
        }
        _ => Err(over("is not a colour".into())),
    }
}

/// An icon property, read leniently as the list tree reads one: an icon
/// Pane cannot read is `None` and is not drawn. An inline `data:` URL over
/// the bound is the extension's error.
fn icon_of(value: Option<&Value>, name: &str) -> Result<Option<Icon>, ReadError> {
    let Some(value) = value else {
        return Ok(None);
    };
    bounded_data(value)?;
    Ok(icons::read(value))
}

/// Whether `value`'s `data:` URLs stay within the bound of inline image
/// data, checking its fallbacks.
fn bounded_data(value: &Value) -> Result<(), ReadError> {
    match value {
        Value::Object(fields) => {
            if let Some(Value::String(url)) = fields.get("url")
                && icons::is_data_url(url)
                && url.len() > MAX_INLINE_IMAGE
            {
                return Err(ReadError::Guest(format!(
                    "an image of the view holds {} bytes of inline data; at most \
                     {MAX_INLINE_IMAGE} are read",
                    url.len()
                )));
            }
            if let Some(fallback) = fields.get("fallback") {
                bounded_data(fallback)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// A padding: a bare token for all four sides, or `{x, y}` or
/// `{top, right, bottom, left}` naming some of them. `Err` when the tree
/// gives something that is not one of these.
fn padding(value: Option<&Value>) -> Result<Padding, String> {
    let side = |fields: &Map<String, Value>, name: &str| match fields.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => space_of(value)
            .map(Some)
            .ok_or_else(|| format!("its padding's {name} is not a space token")),
    };
    match value {
        None | Some(Value::Null) => Ok(Padding::default()),
        Some(Value::Object(fields)) => {
            let mut padding = Padding {
                top: side(fields, "top")?,
                right: side(fields, "right")?,
                bottom: side(fields, "bottom")?,
                left: side(fields, "left")?,
            };
            let x = side(fields, "x")?;
            let y = side(fields, "y")?;
            padding.left = padding.left.or(x);
            padding.right = padding.right.or(x);
            padding.top = padding.top.or(y);
            padding.bottom = padding.bottom.or(y);
            Ok(padding)
        }
        Some(value) => space_of(value)
            .map(|all| Padding {
                top: Some(all),
                right: Some(all),
                bottom: Some(all),
                left: Some(all),
            })
            .ok_or_else(|| "its padding is not a space token or an object".into()),
    }
}

/// A place a stack's child is put at.
fn place_of(wire: &WireNode, name: &str) -> Result<Option<Place>, ReadError> {
    token(wire.rest.get(name), name, place)
}

/// An offset from a stack child's place: a length for both axes, or
/// `{x, y}`.
fn offset_of(wire: &WireNode) -> Result<Option<Offset>, ReadError> {
    match wire.rest.get("offset") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let offset = match value {
                Value::Object(fields) => {
                    let x = match fields.get("x") {
                        None | Some(Value::Null) => Length::Px(Finite(0.)),
                        Some(value) => length_of(value, "offset")?,
                    };
                    let y = match fields.get("y") {
                        None | Some(Value::Null) => Length::Px(Finite(0.)),
                        Some(value) => length_of(value, "offset")?,
                    };
                    Offset { x, y }
                }
                value => {
                    let both = length_of(value, "offset")?;
                    Offset { x: both, y: both }
                }
            };
            Ok(Some(offset))
        }
    }
}

/// An orientation a scroll or divider takes.
fn orientation_of(wire: &WireNode, name: &str) -> Result<Option<Orientation>, ReadError> {
    token(wire.rest.get(name), name, orientation)
}

/// A space token, as the tree gives it: a string naming it, else nothing.
fn space_of(value: &Value) -> Option<Space> {
    let Value::String(token) = value else {
        return None;
    };
    Space::named(token)
}

fn place(name: &str) -> Option<Place> {
    match name {
        "top-start" | "top-left" => Some(Place::TopStart),
        "top" | "top-center" => Some(Place::Top),
        "top-end" | "top-right" => Some(Place::TopEnd),
        "start" | "left" | "middle-start" | "middle-left" => Some(Place::Start),
        "center" | "middle" | "middle-center" => Some(Place::Center),
        "end" | "right" | "middle-end" | "middle-right" => Some(Place::End),
        "bottom-start" | "bottom-left" => Some(Place::BottomStart),
        "bottom" | "bottom-center" => Some(Place::Bottom),
        "bottom-end" | "bottom-right" => Some(Place::BottomEnd),
        _ => None,
    }
}

fn orientation(name: &str) -> Option<Orientation> {
    match name {
        "vertical" => Some(Orientation::Vertical),
        "horizontal" => Some(Orientation::Horizontal),
        _ => None,
    }
}

fn align(name: &str) -> Option<Align> {
    match name {
        "start" => Some(Align::Start),
        "center" => Some(Align::Center),
        "end" => Some(Align::End),
        "stretch" => Some(Align::Stretch),
        "baseline" => Some(Align::Baseline),
        _ => None,
    }
}

fn justify(name: &str) -> Option<Justify> {
    match name {
        "start" => Some(Justify::Start),
        "center" => Some(Justify::Center),
        "end" => Some(Justify::End),
        "space-between" => Some(Justify::SpaceBetween),
        "space-around" => Some(Justify::SpaceAround),
        _ => None,
    }
}

fn text_style(name: &str) -> Option<TextStyle> {
    TextStyle::named(name)
}

fn text_level(name: &str) -> Option<TextLevel> {
    TextLevel::named(name)
}

fn tone(name: &str) -> Option<Tone> {
    match name {
        "default" => Some(Tone::Default),
        "secondary" => Some(Tone::Secondary),
        "ghost" => Some(Tone::Ghost),
        "accent" => Some(Tone::Accent),
        "destructive" => Some(Tone::Destructive),
        _ => None,
    }
}

/// A reading error, as the messages say it.
fn unreadable(message: &str) -> ReadError {
    ReadError::Unreadable(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Color;

    /// A minimal document with the given JSON for its root.
    fn tree(root: &str) -> Result<DesignedTree, ReadError> {
        DesignedTree::read(&format!(r#"{{"version":"1.0","root":{root}}}"#))
    }

    #[test]
    fn a_column_reads_with_its_layout_and_children() {
        let tree = tree(
            r#"{"type":"column","key":"main","gap":"m","padding":{"x":"l","y":"s"},
                "align":"center","justify":"space-between",
                "children":[
                    {"type":"text","text":"Count: 3","style":"title","level":"secondary"},
                    {"type":"row","gap":"xs","wrap":true,"children":[
                        {"type":"button","label":"Up","onPress":7},
                        {"type":"button","label":"Down","tone":"destructive","onPress":8}
                    ]}
                ]}"#,
        )
        .unwrap();

        let NodeKind::Column(layout) = &tree.root.kind else {
            panic!("a column: {:?}", tree.root);
        };
        assert_eq!(tree.root.key.as_deref(), Some("main"));
        assert_eq!(layout.gap, Some(Space::M));
        assert_eq!(layout.padding.left, Some(Space::L));
        assert_eq!(layout.padding.top, Some(Space::S));
        assert_eq!(layout.align, Some(Align::Center));
        assert_eq!(layout.justify, Some(Justify::SpaceBetween));
        assert!(!layout.wrap);
        let NodeKind::Text(text) = &tree.root.children[0].kind else {
            panic!("a text: {:?}", tree.root.children[0]);
        };
        assert_eq!(text.content, TextContent::Plain("Count: 3".into()));
        assert_eq!(text.style, Some(TextStyle::Title));
        assert_eq!(text.level, Some(TextLevel::Secondary));
        let row = &tree.root.children[1];
        let NodeKind::Row(row_layout) = &row.kind else {
            panic!("a row: {row:?}");
        };
        assert!(row_layout.wrap);
        let NodeKind::Button(up) = &row.children[0].kind else {
            panic!("a button: {:?}", row.children[0]);
        };
        assert_eq!(up.label, "Up");
        assert_eq!(up.on_press, Some(7));
        assert_eq!(up.tone, None);
        let NodeKind::Button(down) = &row.children[1].kind else {
            panic!("a button: {:?}", row.children[1]);
        };
        assert_eq!(down.tone, Some(Tone::Destructive));
    }

    #[test]
    fn padding_reads_a_bare_token_and_each_side() {
        let read = |root: &str| {
            tree(root)
                .map(|tree| match tree.root.kind {
                    NodeKind::Column(layout) => layout.padding,
                    _ => panic!("a column"),
                })
                .map_err(|error| error.message().to_owned())
        };
        assert_eq!(
            read(r#"{"type":"column","padding":"m"}"#).unwrap(),
            Padding {
                top: Some(Space::M),
                right: Some(Space::M),
                bottom: Some(Space::M),
                left: Some(Space::M),
            }
        );
        assert_eq!(
            read(r#"{"type":"column","padding":{"top":"xs","bottom":"l"}}"#).unwrap(),
            Padding {
                top: Some(Space::Xs),
                bottom: Some(Space::L),
                ..Padding::default()
            }
        );
        assert_eq!(read(r#"{"type":"column"}"#).unwrap(), Padding::default());
        assert_eq!(
            read(r#"{"type":"column","padding":5}"#).unwrap_err(),
            "its padding is not a space token or an object"
        );
        assert_eq!(
            read(r#"{"type":"column","padding":{"x":"huge"}}"#).unwrap_err(),
            "its padding's x is not a space token"
        );
    }

    #[test]
    fn the_layout_primitives_read_their_properties() {
        let tree = tree(
            r#"{"type":"stack","align":"center","children":[
                {"type":"image","image":{"builtin":"star"},"size":"l",
                 "place":"top-right","offset":{"x":8,"y":"4px"},"fit":"cover"},
                {"type":"scroll","orientation":"horizontal","children":[
                    {"type":"spacer","grow":2},
                    {"type":"divider"}
                ]},
                {"type":"divider","orientation":"vertical"}
            ]}"#,
        )
        .unwrap();
        assert!(matches!(tree.root.kind, NodeKind::Stack(Place::Center)));
        let image = &tree.root.children[0];
        let NodeKind::Image(image) = &image.kind else {
            panic!("an image: {image:?}");
        };
        assert_eq!(image.fit, Fit::Cover);
        assert_eq!(image.size, Some(IconExtent::Token(IconSize::L)));
        assert_eq!(tree.root.children[0].place, Some(Place::TopEnd));
        assert_eq!(
            tree.root.children[0].offset,
            Some(Offset {
                x: Length::Px(Finite(8.)),
                y: Length::Px(Finite(4.)),
            })
        );
        let scroll = &tree.root.children[1];
        assert!(matches!(
            scroll.kind,
            NodeKind::Scroll {
                orientation: Orientation::Horizontal
            }
        ));
        assert_eq!(scroll.children[0].style.sizing.grow, Some(Finite(2.)));
        assert!(matches!(scroll.children[0].kind, NodeKind::Spacer));
        assert!(matches!(scroll.children[1].kind, NodeKind::Divider { .. }));
        assert!(matches!(
            tree.root.children[2].kind,
            NodeKind::Divider {
                orientation: Orientation::Vertical
            }
        ));
    }

    #[test]
    fn every_node_carries_its_sizing_and_surface_with_variants() {
        let read = tree(
            r##"{"type":"text","text":"Hi","grow":1,"shrink":0,"basis":"40px",
                "width":"1/2","height":64,"minWidth":"s","maxWidth":4096,
                "aspectRatio":1.5,
                "background":"danger","border":{"width":"2px","color":"#0f0"},
                "radius":"full","opacity":0.8,
                "hover":{"background":{"raw":"#ff0000"}},"pressed":{"opacity":1}"##,
        )
        .unwrap();
        let style = &read.root.style;
        assert_eq!(style.sizing.grow, Some(Finite(1.)));
        assert_eq!(style.sizing.shrink, Some(Finite(0.)));
        assert_eq!(style.sizing.basis, Some(Length::Px(Finite(40.))));
        assert_eq!(style.sizing.width, Some(Length::Fraction(Finite(0.5))));
        assert_eq!(style.sizing.height, Some(Length::Px(Finite(64.))));
        assert_eq!(style.sizing.min_width, Some(Length::Space(Space::S)));
        assert_eq!(style.sizing.max_width, Some(Length::Px(Finite(4096.))));
        assert_eq!(style.sizing.aspect_ratio, Some(Finite(1.5)));
        assert_eq!(
            style.surface.background,
            Some(Paint {
                tint: Tint::Same(Color::Tone(icons::Tone::Danger)),
                exact: false,
            })
        );
        assert_eq!(
            style.surface.border,
            Some(Border {
                width: Some(Length::Px(Finite(2.))),
                color: Some(Paint {
                    tint: Tint::Same(Color::Rgba(0x00FF00FF)),
                    exact: false,
                }),
            })
        );
        assert_eq!(style.surface.radius, Some(RadiusLength::Token(Radius::Full)));
        assert_eq!(style.surface.opacity, Some(Finite(0.8)));
        assert_eq!(
            style.hover.as_ref().unwrap().background,
            Some(Paint {
                tint: Tint::Same(Color::Rgba(0xFF0000FF)),
                exact: true,
            })
        );
        assert_eq!(style.pressed.as_ref().unwrap().opacity, Some(Finite(1.)));

        // Raw lengths clamp, opacity clamps, and a non-positive aspect
        // ratio is left out.
        let clamped = tree(
            r#"{"type":"text","text":"Hi","width":99999,"minWidth":-5,"opacity":9,
                "aspectRatio":0}"#,
        )
        .unwrap();
        let style = &clamped.root.style;
        assert_eq!(style.sizing.width, Some(Length::Px(Finite(4096.))));
        assert_eq!(style.sizing.min_width, Some(Length::Px(Finite(0.))));
        assert_eq!(style.surface.opacity, Some(Finite(1.)));
        assert_eq!(style.sizing.aspect_ratio, None);
    }

    #[test]
    fn colours_read_as_tones_raws_pairs_and_exact_raws() {
        let read = |root: &str| {
            let field = |tree: &DesignedTree| match &tree.root.kind {
                NodeKind::Text(text) => text.color,
                _ => panic!("a text"),
            };
            tree(root)
                .map(|tree| field(&tree))
                .map_err(|e| e.message().to_owned())
        };
        assert_eq!(
            read(r#"{"type":"text","text":"a","color":"success"}"#).unwrap(),
            Some(Paint {
                tint: Tint::Same(Color::Tone(icons::Tone::Success)),
                exact: false,
            })
        );
        assert_eq!(
            read(r#"{"type":"text","text":"a","color":"hsl(0, 100%, 50%)"}"#).unwrap(),
            Some(Paint {
                tint: Tint::Same(Color::Rgba(0xFF0000FF)),
                exact: false,
            })
        );
        assert_eq!(
            read(r##"{"type":"text","text":"a","color":{"light":"#111","dark":"secondary"}}"##)
            .unwrap(),
            Some(Paint {
                tint: Tint::Pair {
                    light: Color::Rgba(0x111111FF),
                    dark: Color::Tone(icons::Tone::Secondary),
                },
                exact: false,
            })
        );
        assert_eq!(
            read(r##"{"type":"text","text":"a","color":{"raw":{"light":"#111","dark":"#eee"}}}"##)
            .unwrap(),
            Some(Paint {
                tint: Tint::Pair {
                    light: Color::Rgba(0x111111FF),
                    dark: Color::Rgba(0xEEEEEEFF),
                },
                exact: true,
            })
        );
        // A name this version does not know is left out; a broken pair or
        // object says why.
        assert_eq!(
            read(r#"{"type":"text","text":"a","color":"sparkle"}"#).unwrap(),
            None
        );
        assert!(read(r##"{"type":"text","text":"a","color":{"light":"#111"}}"##)
            .unwrap_err()
            .contains("only one of a light and a dark"));
        assert!(read(r#"{"type":"text","text":"a","color":5}"#)
            .unwrap_err()
            .contains("is not a colour"));
    }

    #[test]
    fn the_shared_components_read_their_properties() {
        let tree = tree(
            r#"{"type":"column","children":[
                {"type":"text","text":"a","spans":[
                    {"text":"Accept the "},
                    {"text":"terms","color":"accent","onPress":3},
                    {"text":" now","code":true}
                ]},
                {"type":"button","label":"Save","icon":{"builtin":"check"},
                 "keys":["ctrl","s"],"enabled":false,"onPress":1},
                {"type":"link","label":"Docs","onPress":2},
                {"type":"icon","icon":"star","size":18},
                {"type":"icon-tile","icon":{"builtin":"bell"}},
                {"type":"rich-row","title":"Pane","subtitle":"A launcher",
                 "icon":{"builtin":"blocks"},
                 "accessories":[{"text":"beta","tag":true,"color":"blue"}],"onPress":4},
                {"type":"keycap","key":"ctrl"},
                {"type":"key-sequence","keys":["ctrl","shift","p"]},
                {"type":"tag","text":"new","color":"magenta"},
                {"type":"badge","text":"3"},
                {"type":"toggle","on":true,"label":"Dark mode","onChange":5},
                {"type":"checkbox","checked":true,"label":"Remember","onChange":6},
                {"type":"segmented","options":[{"value":"a","label":"A"},{"value":"b"}],
                 "value":"b","onChange":7},
                {"type":"slider","value":0.4,"min":0,"max":2,"step":0.2,"onChange":8},
                {"type":"progress","value":0.6},
                {"type":"loading","label":"Loading"},
                {"type":"card","gap":"m","children":[{"type":"text","text":"in"}]},
                {"type":"section-header","title":"General","note":"How Pane starts"},
                {"type":"metadata-list","items":[
                    {"label":"Author","value":"Vu","onPress":9},
                    {"label":"Tags","tags":["one","two"]},
                    {"separator":true}
                ]},
                {"type":"empty-state","title":"Nothing","description":"Empty",
                 "icon":{"builtin":"search-none"}},
                {"type":"text-input","value":"typed","placeholder":"Type","onChange":10},
                {"type":"password-input","placeholder":"Password"},
                {"type":"text-area","value":"two\nlines"},
                {"type":"select","options":[{"value":"x","label":"X"}],"value":"x",
                 "onChange":11}
            ]}"#,
        )
        .unwrap();
        let nodes = &tree.root.children;
        let text = match &nodes[0].kind {
            NodeKind::Text(text) => text,
            _ => panic!("a text"),
        };
        let TextContent::Spans(spans) = &text.content else {
            panic!("spans: {:?}", text.content);
        };
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].on_press, Some(3));
        assert_eq!(spans[2].code, true);
        let NodeKind::Button(save) = &nodes[1].kind else {
            panic!("a button");
        };
        assert!(save.icon.is_some());
        assert_eq!(save.keys.as_deref(), Some(&["ctrl".to_owned(), "s".to_owned()][..]));
        assert!(!save.enabled);
        let NodeKind::Link(link) = &nodes[2].kind else {
            panic!("a link");
        };
        assert_eq!(link.on_press, Some(2));
        let NodeKind::Icon(icon) = &nodes[3].kind else {
            panic!("an icon");
        };
        assert_eq!(icon.size, Some(IconExtent::Px(Finite(18.))));
        assert!(matches!(&nodes[4].kind, NodeKind::IconTile(_)));
        let NodeKind::RichRow(row) = &nodes[5].kind else {
            panic!("a rich row");
        };
        assert_eq!(row.title, "Pane");
        assert_eq!(row.accessories.len(), 1);
        assert!(row.accessories[0].tag);
        assert!(matches!(&nodes[6].kind, NodeKind::Keycap(key) if key.key == "ctrl"));
        assert!(matches!(
            &nodes[7].kind,
            NodeKind::KeySequence(keys) if keys.keys.len() == 3
        ));
        let NodeKind::Tag(tag) = &nodes[8].kind else {
            panic!("a tag");
        };
        assert_eq!(tag.text, "new");
        assert!(matches!(&nodes[9].kind, NodeKind::Badge(badge) if badge.text == "3"));
        assert!(matches!(&nodes[10].kind, NodeKind::Toggle(t) if t.on));
        assert!(matches!(&nodes[11].kind, NodeKind::Checkbox(c) if c.checked));
        let NodeKind::Segmented(segmented) = &nodes[12].kind else {
            panic!("a segmented control");
        };
        assert_eq!(segmented.value.as_deref(), Some("b"));
        assert_eq!(segmented.options[1].label, None);
        let NodeKind::Slider(slider) = &nodes[13].kind else {
            panic!("a slider");
        };
        assert_eq!(slider.max, Finite(2.));
        assert!(matches!(&nodes[14].kind, NodeKind::Progress(p) if p.value == Finite(0.6)));
        assert!(matches!(&nodes[15].kind, NodeKind::Loading(l) if l.label.is_some()));
        assert!(matches!(&nodes[16].kind, NodeKind::Card(_)));
        assert!(matches!(&nodes[17].kind, NodeKind::SectionHeader(h) if h.note.is_some()));
        let NodeKind::MetadataList(list) = &nodes[18].kind else {
            panic!("a metadata list");
        };
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.items[1].tags, vec!["one", "two"]);
        assert!(list.items[2].separator);
        assert!(matches!(&nodes[19].kind, NodeKind::EmptyState(e) if e.description.is_some()));
        assert!(matches!(&nodes[20].kind, NodeKind::TextInput(i) if i.value == "typed"));
        assert!(matches!(&nodes[21].kind, NodeKind::PasswordInput(p) if p.placeholder.is_some()));
        assert!(matches!(&nodes[22].kind, NodeKind::TextArea(a) if a.value == "two\nlines"));
        assert!(matches!(&nodes[23].kind, NodeKind::Select(s) if s.options.len() == 1));
    }

    #[test]
    fn markdown_reads_and_is_bounded() {
        let read = tree(
            r##"{"type":"markdown","markdown":"# Hi

Some *prose*.

- [x] done"}"##,
        )
        .unwrap();
        let NodeKind::Markdown(markdown) = &read.root.kind else {
            panic!("markdown: {:?}", read.root.kind);
        };
        assert!(matches!(markdown.blocks[0], markdown::Block::Heading { level: 1, .. }));
        assert!(matches!(markdown.blocks[2], markdown::Block::List { .. }));
        let long = format!(
            r#"{{"type":"markdown","markdown":"{}"}}"#,
            "a".repeat(MAX_MARKDOWN_CHARS + 1)
        );
        assert!(
            tree(&long).unwrap_err().message().contains("markdown has"),
            "{}",
            tree(&long).unwrap_err().message()
        );
    }

    #[test]
    fn unknown_nodes_fall_back_or_draw_their_children() {
        let tree = tree(
            r#"{"type":"marquee","children":[
                {"type":"marquee","fallback":{"type":"text","text":"Buy now"}},
                {"type":"marquee","children":[{"type":"text","text":"Hi"}]},
                {"type":"video","requires":9}
            ]}"#,
        )
        .unwrap();

        // An unknown node with a fallback draws it; without one its
        // children; one whose `requires` Pane does not meet is unknown too,
        // and with no fallback and no children draws nothing.
        assert!(matches!(&tree.root.kind, NodeKind::Unknown(name) if name == "marquee"));
        assert!(matches!(
            tree.root.children[0]
                .fallback
                .as_ref()
                .map(|fallback| fallback.kind.clone()),
            Some(NodeKind::Text(_))
        ));
        assert!(matches!(
            &tree.root.children[1].children[0].kind,
            NodeKind::Text(_)
        ));
        let video = &tree.root.children[2];
        assert!(matches!(video.kind, NodeKind::Unknown(_)));
        assert!(video.fallback.is_none());
        assert!(video.children.is_empty());
    }

    #[test]
    fn a_newer_minor_reads_and_another_major_is_refused() {
        // A newer minor: read for what Pane understands.
        let newer = DesignedTree::read(
            r#"{"version":"1.9","extra":true,"root":{"type":"column","novelty":{},
                "children":[{"type":"text","text":"Hi","sparkle":true}]}}"#,
        )
        .unwrap();
        assert!(matches!(newer.root.kind, NodeKind::Column(_)));

        // Another major: refused, naming both versions.
        let error = DesignedTree::read(r#"{"version":"2.0","root":{"type":"column"}}"#)
            .unwrap_err()
            .message()
            .to_owned();
        assert!(
            error.contains("UI component set 2.0"),
            "{error}"
        );

        // A version Pane cannot read.
        let error = DesignedTree::read(r#"{"version":"one","root":{"type":"column"}}"#)
            .unwrap_err()
            .message()
            .to_owned();
        assert!(error.contains("version"), "{error}");
    }

    #[test]
    fn trees_over_a_limit_say_what_the_limit_is() {
        let over = |root: &str| tree(root).unwrap_err().message().to_owned();
        // Depth: 64 levels are drawn, 65 are not.
        let nest = |levels: usize| {
            let inner = r#"{"type":"text","text":"a"}"#;
            (0..levels).fold(inner.to_owned(), |nested, _| {
                format!(r#"{{"type":"column","children":[{nested}]}}"#)
            })
        };
        assert!(
            tree(&nest(MAX_DEPTH - 1)).is_ok(),
            "a tree 64 levels deep is drawn: {:?}",
            tree(&nest(MAX_DEPTH - 1))
        );
        let error = over(&nest(MAX_DEPTH));
        assert!(
            error.contains("65 levels deep; at most 64 are drawn"),
            "{error}"
        );
        // Text: one node with too much in it.
        let long = format!(
            r#"{{"type":"text","text":"{}"}}"#,
            "a".repeat(MAX_TEXT_CHARS + 1)
        );
        assert!(
            over(&long).contains(&format!("characters; at most {MAX_TEXT_CHARS} are drawn")),
            "{}",
            over(&long)
        );
        // Nodes: one more than the ceiling.
        let many = vec![r#"{"type":"text","text":"a"}"#; MAX_NODES + 1].join(",");
        let many = format!(r#"{{"type":"column","children":[{many}]}}"#);
        assert!(
            over(&many).contains("10001 nodes; at most 10000 are drawn"),
            "{}",
            over(&many)
        );
        // Inline image data: a data URL over the bound.
        let big = format!(
            r#"{{"type":"icon","icon":{{"url":"data:image/png;base64,{}"}}}}"#,
            "a".repeat(MAX_INLINE_IMAGE + 1)
        );
        assert!(
            over(&big).contains("bytes of inline data"),
            "{}",
            over(&big)
        );
    }

    #[test]
    fn a_tree_pane_cannot_read_says_why() {
        let error = |document: &str| {
            DesignedTree::read(document)
                .unwrap_err()
                .message()
                .to_owned()
        };
        for (document, why) in [
            ("not json", "its tree:"),
            (r#"{"root":{}}"#, "missing field"),
            (r#"{"version":"1.0"}"#, "missing field `root`"),
            (r#"{"version":"1.0","root":{}}"#, "missing field `type`"),
            (
                r#"{"version":"1.0","root":{"type":"text"}}"#,
                "a text node has no text",
            ),
            (
                r#"{"version":"1.0","root":{"type":"text","text":5}}"#,
                "its text is not a string",
            ),
            (
                r#"{"version":"1.0","root":{"type":"button"}}"#,
                "a button node has no label",
            ),
            (
                r#"{"version":"1.0","root":{"type":"button","label":"Up","onPress":"up"}}"#,
                "its onPress is not a callback id",
            ),
            (
                r#"{"version":"1.0","root":{"type":"column","gap":12}}"#,
                "its gap is not one of its names",
            ),
            (
                r#"{"version":"1.0","root":{"type":"column","wrap":"yes"}}"#,
                "its wrap is not a boolean",
            ),
            (
                r#"{"version":"1.0","root":{"type":"column","children":"none"}}"#,
                "invalid type",
            ),
            (
                r#"{"version":"1.0","root":{"type":"text","text":"a","width":"lots"}}"#,
                "its width is not a length",
            ),
            (
                r#"{"version":"1.0","root":{"type":"text","text":"a","opacity":"high"}}"#,
                "its opacity is not a number",
            ),
            (
                r#"{"version":"1.0","root":{"type":"image","fit":"stretch"}}"#,
                "a fit is",
            ),
            (
                r#"{"version":"1.0","root":{"type":"markdown"}}"#,
                "a markdown node has no markdown",
            ),
            (
                r#"{"version":"1.0","root":{"type":"select"}}"#,
                "it has no options",
            ),
            (
                r#"{"version":"1.0","root":{"type":"progress"}}"#,
                "a progress node has no value",
            ),
        ] {
            let message = error(document);
            assert!(message.contains(why), "{document}: {message}");
        }
    }

    #[test]
    fn an_unknown_token_or_property_is_left_out() {
        // A token a newer minor added, or a property Pane does not know: the
        // field is treated as absent, never the tree unreadable.
        let tree = tree(
            r#"{"type":"column","gap":"huge","align":"diag","tooltip":"Later",
                "children":[{"type":"text","text":"Hi","style":"fancy","level":"dim"},
                    {"type":"button","label":"Go","tone":"neon","onPress":1}]}"#,
        )
        .unwrap();
        let NodeKind::Column(layout) = tree.root.kind else {
            panic!("a column");
        };
        assert_eq!(layout.gap, None);
        assert_eq!(layout.align, None);
        let NodeKind::Text(text) = &tree.root.children[0].kind else {
            panic!("a text");
        };
        assert_eq!(text.style, None);
        assert_eq!(text.level, None);
        let NodeKind::Button(button) = &tree.root.children[1].kind else {
            panic!("a button");
        };
        assert_eq!(button.tone, None);
    }
}
