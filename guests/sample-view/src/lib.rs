//! Pane's designed view sample in Rust: a counter whose screen is the
//! tree its `view.render` answers — a column of a text and a row of
//! buttons, drawn by Pane, not by the guest (`docs/designed-tree.md`,
//! ADR 0036). The behaviour matches the JavaScript and TypeScript samples
//! (`sample-view-js`, `sample-view-ts`): the same text, the same buttons.
//!
//! The `components` command of the same package answers a gallery of
//! every component of the UI component set (#237): the layout
//! primitives, the shared components, the tokens and the raw values,
//! with a toggle to show the tree changes.
#![no_std]

use core::cell::Cell;

use pane_extension::icon::Tone as Colour;
use pane_extension::view::{
    Cx, Color, Fit, Icon, IconSize, IntoAnswer, IntoNode, Length, Paint, Place, Radius, Space,
    TextLevel, TextStyle, Tone, View, badge, button, card, checkbox, choice, column, divider,
    empty_state, icon, icon_tile, image, key_sequence, keycap, link, loading, markdown,
    metadata_list, metadata, metadata_separator, metadata_tags, password_input, progress, rich_row,
    row, scroll, section_header, select, slider, spacer, span, spans, stack, tag, text, text_area,
    text_input, toggle,
};
use pane_extension::{Command, LaunchRecord};

/// The screen a command opens: the counter, or the gallery of components.
struct Screen {
    /// Whether this view is the counter.
    counter: bool,
    count: Cell<u32>,
    on: Cell<bool>,
}

impl View for Screen {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoAnswer {
        if self.counter {
            return column()
                .gap(Space::M)
                .child(
                    text(format!("Count: {}", self.count.get()))
                        .style(TextStyle::Title)
                        .level(TextLevel::Primary),
                )
                .child(
                    row().gap(Space::S).children([
                        button("Increment").on_click(cx.listener(|this: &mut Self| {
                            this.count.set(this.count.get() + 1);
                        })),
                        button("Decrement").on_click(cx.listener(|this: &mut Self| {
                            this.count.set(this.count.get().saturating_sub(1));
                        })),
                        button("Reset")
                            .tone(Tone::Destructive)
                            .on_click(cx.listener(|this: &mut Self| this.count.set(0))),
                    ]),
                )
                .into_node();
        }
        scroll().child(
            column()
                .gap(Space::L)
                .child(
                    text("The UI component set")
                        .style(TextStyle::Heading)
                        .level(TextLevel::Primary),
                )
                .child(
                    // A stack: a badge over an icon tile, placed.
                    stack()
                        .align(Place::TopEnd)
                        .child(icon_tile(Icon::builtin("layers")))
                        .child(
                            badge("4")
                                .place(Place::BottomEnd)
                                .offset(Length::Px(4.), Length::Px(4.)),
                        ),
                )
                .child(
                    // A text with spans, one a link.
                    spans([
                        span("Accept the "),
                        span("terms").on_click(cx.listener(|_: &mut Self| {})),
                        span(" before continuing.").code(),
                    ]),
                )
                .child(
                    row()
                        .gap(Space::S)
                        .children([
                            icon(Icon::builtin("star")).size(IconSize::L),
                            // A raw blue, corrected for contrast.
                            icon(Icon::builtin("bell").tint(Color::Raw("#88ccff".into()))),
                            keycap("ctrl"),
                            key_sequence(["ctrl", "shift", "p"]),
                            tag("beta").color(Colour::Blue),
                            badge("3"),
                        ])
                        .name("Marks"),
                )
                .child(
                    card().gap(Space::S).child(
                        rich_row("Pane")
                            .subtitle("A tree Pane renders")
                            .icon(Icon::builtin("layers"))
                            .tag("new")
                            .on_click(cx.listener(|_: &mut Self| {})),
                    ),
                )
                .child(
                    // Controls, one of them live.
                    column()
                        .gap(Space::S)
                        .child(section_header("Controls").note("Every one focusable"))
                        .child(
                            toggle(self.on.get())
                                .label("Dark mode")
                                .on_click(cx.listener(|this: &mut Self| {
                                    this.on.set(!this.on.get());
                                })),
                        )
                        .child(
                            checkbox(self.on.get())
                                .label("Remember")
                                .on_click(cx.listener(|this: &mut Self| {
                                    this.on.set(!this.on.get());
                                })),
                        )
                        .child(
                            select([
                                choice("daily").label("Daily"),
                                choice("weekly").label("Weekly"),
                            ])
                            .value("daily")
                            .label("Digest")
                            .on_click(cx.listener(|_: &mut Self| {})),
                        )
                        .child(
                            slider(0.4)
                                .label("Volume")
                                .on_click(cx.listener(|_: &mut Self| {})),
                        )
                        .child(progress(0.7).label("Installed"))
                        .child(loading().label("Checking")),
                )
                .child(
                    column()
                        .gap(Space::S)
                        .child(section_header("Fields"))
                        .child(
                            text_input("typed")
                                .placeholder("Type here")
                                .label("Name")
                                .on_click(cx.listener(|_: &mut Self| {})),
                        )
                        .child(password_input().label("Secret"))
                        .child(text_area("two lines").label("Notes")),
                )
                .child(
                    // Markdown.
                    markdown(concat!(
                        "# Markdown\n\n",
                        "Some *prose*, `code` and [a link](https://pane.dev).\n\n",
                        "- [x] drawn\n- [ ] still to do\n"
                    )),
                )
                .child(
                    metadata_list([
                        metadata("Author", "Vu").on_click(cx.listener(|_: &mut Self| {})),
                        metadata_tags("Tags", ["one", "two"]),
                        metadata_separator(),
                        metadata("Kind", "sample"),
                    ]),
                )
                .child(
                    empty_state("Nothing here")
                        .description("The gallery is over")
                        .icon(Icon::builtin("search-minus"))
                        .action(button("Start over").on_click(cx.listener(|_: &mut Self| {}))),
                )
                .child(
                    // An image, with a placeholder while it stands in.
                    image(Icon::builtin("image"))
                        .size(IconSize::Xl)
                        .fit(Fit::Cover)
                        .placeholder(text("Loading…").level(TextLevel::Tertiary)),
                )
                .child(
                    // Raw values: a surface with a variant, a tone, a
                    // corrected colour and an exact one.
                    row()
                        .gap(Space::S)
                        .children([
                            text("Surface")
                                .level(TextLevel::Secondary)
                                .background(Paint::Color(Color::Tone(Colour::Danger)))
                                .radius(Radius::M)
                                .hover(
                                    pane_extension::view::Surface::default()
                                        .background(Paint::Color(Color::Tone(Colour::Accent))),
                                ),
                            link("A link").on_click(cx.listener(|_: &mut Self| {})),
                        ]),
                )
                .child(
                    row()
                        .gap(Space::S)
                        .children([
                            text("Corrected").color(Color::Raw("#88ccff".into())),
                            text("Exact").color(Paint::Exact(Color::Raw("#ff6363".into()))),
                        ]),
                )
                .child(divider())
                .child(spacer()),
        )
        .into_node()
    }
}

struct Sample;
pane_extension::export!(Sample);

impl Command for Sample {
    type CustomView = pane_extension::NoCustomView;
    type DesignedView = Screen;

    async fn open_designed_view(
        command: String,
        _launch: LaunchRecord,
    ) -> Result<Screen, String> {
        match command.as_str() {
            "sample" => Ok(Screen {
                counter: true,
                count: Cell::new(0),
                on: Cell::new(false),
            }),
            "components" => Ok(Screen {
                counter: false,
                count: Cell::new(0),
                on: Cell::new(false),
            }),
            _ => Err("this command opens no designed view".into()),
        }
    }
}
