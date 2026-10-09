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
//! [`MAX_TREE_BYTES`], [`MAX_TEXT_CHARS`]), or of another major version,
//! is the extension's error ([`ReadError::Guest`]): the view keeps its
//! last good tree either way.

use serde::Deserialize;
use serde_json::{Map, Value};

/// The version of the UI component set this Pane renders: major 1, minor 0.
/// A document of this major and any minor is read (unknown fields and nodes
/// degrading); a document of another major is refused naming both versions.
pub const COMPONENT_SET: (u64, u64) = (1, 0);

/// The most nodes one document may hold, counting `fallback` subtrees.
pub const MAX_NODES: usize = 10_000;
/// How deep one document may be, counting its root.
pub const MAX_DEPTH: usize = 64;
/// The most bytes of JSON one document may be.
pub const MAX_TREE_BYTES: usize = 4 * 1024 * 1024;
/// The most characters one text node may hold.
pub const MAX_TEXT_CHARS: usize = 64 * 1024;

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
/// `requires` minimum minor version, and a `fallback` subtree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
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
    Text(Text),
    Button(Button),
    /// A node whose type Pane does not know, or whose `requires` it does
    /// not meet: its `fallback` and children decide what is drawn.
    Unknown(String),
}

/// What a `column` or `row` says about its layout.
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

/// One text node: what it says, in which style and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub content: String,
    pub style: Option<TextStyle>,
    pub level: Option<TextLevel>,
}

/// One button: its label, its tone, and the callback a `press` of it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub tone: Option<Tone>,
    /// The callback id the tree named; `None` for a button that cannot be
    /// pressed.
    pub on_press: Option<u32>,
}

/// A space token: the named distances of the UI component set, mapped onto
/// Pane's theme by the window.
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

/// The style of a text node: its size, weight and family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    Heading,
    Title,
    Body,
    Caption,
    Mono,
    SmallMono,
}

/// The level of a text node: its colour, through the alpha of the text
/// colour, as ADR 0035's own levels are.
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
        let wire: WireDocument = serde_json::from_str(document)
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
    fallback: Option<WireNode>,
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
    let meets = wire.requires.is_none_or(|required| required <= COMPONENT_SET.1);
    let unknown = !matches!(wire.kind.as_str(), "column" | "row" | "text" | "button") || !meets;
    let kind = if unknown {
        NodeKind::Unknown(wire.kind.clone())
    } else {
        match wire.kind.as_str() {
            "column" => NodeKind::Column(layout(&wire)?),
            "row" => NodeKind::Row(layout(&wire)?),
            "text" => NodeKind::Text(text(&wire)?),
            "button" => NodeKind::Button(button(&wire)?),
            _ => unreachable!("the kinds above name every one that is not unknown"),
        }
    };
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
        .map(|fallback| node(fallback, depth + 1, nodes).map(Box::new))
        .transpose()?;
    Ok(Node {
        kind,
        key: wire.key,
        name: wire.name,
        requires: wire.requires,
        fallback,
        children,
    })
}

/// The layout a column or row's other properties give it: `gap`, `padding`,
/// `align`, `justify` and `wrap`. A property of the wrong type is
/// unreadable; a token this version does not know is left out, as a newer
/// minor version may add one.
fn layout(wire: &WireNode) -> Result<Layout, ReadError> {
    let over = |message: String| ReadError::Unreadable(message);
    let rest = &wire.rest;
    Ok(Layout {
        gap: token(rest.get("gap"), "gap", space)?,
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

/// The text a text node's other properties give it: its `text`, `style` and
/// `level`.
fn text(wire: &WireNode) -> Result<Text, ReadError> {
    let over = |message: String| ReadError::Unreadable(message);
    let rest = &wire.rest;
    let content = match rest.get("text") {
        Some(Value::String(content)) => content.clone(),
        Some(_) => return Err(over("its text is not a string".into())),
        None => return Err(over("a text node has no text".into())),
    };
    if content.chars().count() > MAX_TEXT_CHARS {
        return Err(ReadError::Guest(format!(
            "a text of the view has {} characters; at most {MAX_TEXT_CHARS} are drawn",
            content.chars().count()
        )));
    }
    Ok(Text {
        content,
        style: token(rest.get("style"), "style", text_style)?,
        level: token(rest.get("level"), "level", text_level)?,
    })
}

/// The button a button node's other properties give it: its `label`,
/// `tone` and `onPress`.
fn button(wire: &WireNode) -> Result<Button, ReadError> {
    let over = |message: String| ReadError::Unreadable(message);
    let rest = &wire.rest;
    let label = match rest.get("label") {
        Some(Value::String(label)) => label.clone(),
        Some(_) => return Err(over("its label is not a string".into())),
        None => return Err(over("a button node has no label".into())),
    };
    let on_press = match rest.get("onPress") {
        None | Some(Value::Null) => None,
        Some(Value::Number(id)) => Some(
            id.as_u64()
                .filter(|id| *id <= u32::MAX as u64)
                .ok_or_else(|| over("its onPress is not a callback id".into()))? as u32,
        ),
        Some(_) => return Err(over("its onPress is not a callback id".into())),
    };
    Ok(Button {
        label,
        tone: token(rest.get("tone"), "tone", tone)?,
        on_press,
    })
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

/// A padding: a bare token for all four sides, or `{x, y}` or
/// `{top, right, bottom, left}` naming some of them. `Err` when the tree
/// gives something that is not one of these.
fn padding(value: Option<&Value>) -> Result<Padding, String> {
    let side = |fields: &Map<String, Value>, name: &str| {
        match fields.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => space_of(value)
                .map(Some)
                .ok_or_else(|| format!("its padding's {name} is not a space token")),
        }
    };
    match value {
        None | Some(Value::Null) => Ok(Padding::default()),
        Some(value) => space_of(value)
            .map(|all| Padding {
                top: Some(all),
                right: Some(all),
                bottom: Some(all),
                left: Some(all),
            })
            .or_else(|| match value {
                Value::Object(fields) => Some({
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
                    padding
                }),
                _ => None,
            })
            .ok_or_else(|| "its padding is not a space token or an object".into()),
    }
}

/// A space token, as the tree gives it: a string naming it, else nothing.
fn space_of(value: &Value) -> Option<Space> {
    let Value::String(token) = value else {
        return None;
    };
    space(token)
}

/// A space token by its name in the tree.
fn space(name: &str) -> Option<Space> {
    match name {
        "xs" => Some(Space::Xs),
        "s" => Some(Space::S),
        "m" => Some(Space::M),
        "l" => Some(Space::L),
        "xl" => Some(Space::Xl),
        "xxl" => Some(Space::Xxl),
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
    match name {
        "heading" => Some(TextStyle::Heading),
        "title" => Some(TextStyle::Title),
        "body" => Some(TextStyle::Body),
        "caption" => Some(TextStyle::Caption),
        "mono" => Some(TextStyle::Mono),
        "small-mono" => Some(TextStyle::SmallMono),
        _ => None,
    }
}

fn text_level(name: &str) -> Option<TextLevel> {
    match name {
        "primary" => Some(TextLevel::Primary),
        "secondary" => Some(TextLevel::Secondary),
        "tertiary" => Some(TextLevel::Tertiary),
        "quaternary" => Some(TextLevel::Quaternary),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(text.content, "Count: 3");
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
    fn unknown_nodes_fall_back_or_draw_their_children() {
        let tree = tree(
            r#"{"type":"stack","children":[
                {"type":"marquee","fallback":{"type":"text","text":"Buy now"}},
                {"type":"marquee","children":[{"type":"text","text":"Hi"}]},
                {"type":"video","requires":9}
            ]}"#,
        )
        .unwrap();

        // An unknown node with a fallback draws it; without one its
        // children; one whose `requires` Pane does not meet is unknown too,
        // and with no fallback and no children draws nothing.
        assert!(matches!(&tree.root.kind, NodeKind::Unknown(name) if name == "stack"));
        assert!(matches!(
            &tree.root.children[0].fallback.map(|fallback| fallback.kind.clone()),
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
        assert_eq!(
            error,
            "its tree uses UI component set 2.0; this Pane renders 1.0"
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
            "a tree 64 levels deep is drawn"
        );
        let error = over(&nest(MAX_DEPTH));
        assert!(
            error.contains("65 levels deep; at most 64 are drawn"),
            "{error}"
        );
        // Text: one node with too much in it.
        let long = format!(r#"{{"type":"text","text":"{}"}}"#, "a".repeat(MAX_TEXT_CHARS + 1));
        assert!(
            over(&long)
                .contains(&format!("characters; at most {MAX_TEXT_CHARS} are drawn")),
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
            (r#"{"root":{}}"#, "missing field `version`"),
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
        let NodeKind::Text(text) = tree.root.children[0].kind else {
            panic!("a text");
        };
        assert_eq!(text.style, None);
        assert_eq!(text.level, None);
        let NodeKind::Button(button) = tree.root.children[1].kind else {
            panic!("a button");
        };
        assert_eq!(button.tone, None);
    }
}
