//! Markdown, drawn: the blocks pane-core's small parser read from a
//! `markdown` node, laid out in the theme's own typography and chrome —
//! headings, paragraphs, code on a card, quotes with a rule, lists with
//! their bullets, numbers and task checkboxes, rules, and GitHub's
//! tables (#237, ADR 0036). A link is drawn as a link, its address its
//! tooltip; opening what it names is the standard views' to do (#240).
//!
//! Inline content flows as a wrapping row of runs, as a text node's
//! spans do: prose, code, emphasis, strong text and links beside each
//! other, a hard break wrapping the row.

use gpui::prelude::*;
use gpui::{AnyElement, Div, Role, TextAlign, div, px};

use pane_core::Space;
use pane_core::markdown::{Alignment, Block, Inline, Item};

use crate::ui::tokens;
use crate::ui::tooltip::{TooltipLook, text_tooltip};

use super::tree::Draw;

/// The blocks of one markdown node, drawn.
pub(super) fn blocks(blocks: &[Block], path: &str, draw: &Draw) -> AnyElement {
    let mut element = div()
        .id(path.to_owned())
        .flex()
        .flex_col()
        .min_w(px(0.))
        .gap(tokens::space(Space::S));
    for (index, block) in blocks.iter().enumerate() {
        element = element.child(block_element(block, &format!("{path}/{index}"), draw));
    }
    element.into_any_element()
}

/// One block, drawn.
fn block_element(block: &Block, path: &str, draw: &Draw) -> AnyElement {
    let theme = draw.theme;
    match block {
        Block::Heading { level, inlines } => {
            let (size, weight) = match level {
                1 => (px(16.), theme.typography.medium),
                2 => (theme.typography.row_title_size, theme.typography.medium),
                _ => (theme.typography.row_subtitle_size, theme.typography.medium),
            };
            let name = plain(inlines);
            div()
                .id(path.to_owned())
                .flex_none()
                .min_w(px(0.))
                .text_size(size)
                .font_weight(weight)
                .text_color(theme.text_title)
                .role(Role::Heading)
                .map(|heading| heading.aria_label(name))
                .map(|heading| heading.aria_level(*level))
                .child(inlines_element(inlines, draw))
                .into_any_element()
        }
        Block::Paragraph(inlines) => div()
            .id(path.to_owned())
            .flex_none()
            .min_w(px(0.))
            .text_size(theme.typography.row_subtitle_size)
            .text_color(theme.text_body)
            .child(inlines_element(inlines, draw))
            .into_any_element(),
        Block::Code { language, text } => div()
            .id(path.to_owned())
            .flex_none()
            .min_w(px(0.))
            .p(tokens::space(Space::S))
            .rounded(theme.geometry.settings.card_radius)
            .bg(theme.card_fill)
            .shadow(vec![crate::ui::controls::inset_ring(
                theme.card_edge,
                px(1.),
            )])
            .font_family(theme.typography.mono_family.clone())
            .text_size(theme.typography.keycap_size)
            .text_color(theme.text_title)
            .map(|code| {
                code.when_some(
                    language.clone().map(gpui::SharedString::from),
                    |code, language| code.aria_label(language),
                )
            })
            .child(text.clone())
            .into_any_element(),
        Block::Quote(inner) => div()
            .id(path.to_owned())
            .flex_none()
            .min_w(px(0.))
            .pl(tokens::space(Space::S))
            .border_l_2()
            .border_color(theme.hairline)
            .child(blocks(inner, &format!("{path}/quote"), draw))
            .into_any_element(),
        Block::List { ordered, items } => list(*ordered, items, path, draw),
        Block::Rule => div()
            .id(path.to_owned())
            .flex_none()
            .w_full()
            .h(px(1.))
            .bg(theme.hairline_soft)
            .into_any_element(),
        Block::Table { aligns, head, rows } => table(aligns, head, rows, path, draw),
    }
}

/// One list: its items, each with its bullet or number, and its task's
/// checkbox drawn as a mark.
fn list(ordered: bool, items: &[Item], path: &str, draw: &Draw) -> AnyElement {
    let theme = draw.theme;
    let mut element = div()
        .id(path.to_owned())
        .flex()
        .flex_col()
        .min_w(px(0.))
        .gap(tokens::space(Space::Xs));
    for (index, item) in items.iter().enumerate() {
        let mark: gpui::SharedString = match (ordered, item.checked) {
            (_, Some(checked)) => (if checked { "☑ " } else { "☐ " }).into(),
            (true, None) => format!("{}. ", index + 1).into(),
            (false, None) => "• ".into(),
        };
        element = element.child(
            div()
                .id(format!("{path}/{index}"))
                .flex()
                .min_w(px(0.))
                .gap(tokens::space(Space::Xs))
                .child(
                    div()
                        .flex_none()
                        .text_size(theme.typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child(mark),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(0.))
                        .flex_1()
                        .gap(tokens::space(Space::Xs))
                        .children(
                            item.blocks
                                .iter()
                                .enumerate()
                                .map(|(at, block)| {
                                    block_element(block, &format!("{path}/{index}/{at}"), draw)
                                })
                                .collect::<Vec<AnyElement>>(),
                        ),
                ),
        );
    }
    element.into_any_element()
}

/// One table: a head row, bold with a rule under it, and its body rows,
/// each column sharing the width.
fn table(
    aligns: &[Alignment],
    head: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    path: &str,
    draw: &Draw,
) -> AnyElement {
    let theme = draw.theme;
    let columns = aligns.len().max(head.len());
    let empty: Vec<Inline> = Vec::new();
    let no_align = Alignment::None;
    let mut element = div()
        .id(path.to_owned())
        .flex()
        .flex_col()
        .min_w(px(0.))
        .gap(tokens::space(Space::Xs));
    element = element.child(
        div()
            .id(format!("{path}/head"))
            .flex()
            .min_w(px(0.))
            .pb(tokens::space(Space::Xs))
            .border_b_1()
            .border_color(theme.hairline)
            .children(
                head.iter()
                    .chain(std::iter::repeat(&empty))
                    .take(columns)
                    .zip(aligns.iter().chain(std::iter::repeat(&no_align)))
                    .enumerate()
                    .map(|(at, (cell, align))| {
                        cell_element(cell, *align, true, &format!("{path}/head/{at}"), draw)
                    })
                    .collect::<Vec<AnyElement>>(),
            ),
    );
    for (index, row) in rows.iter().enumerate() {
        element = element.child(
            div()
                .id(format!("{path}/{index}"))
                .flex()
                .min_w(px(0.))
                .children(
                    row.iter()
                        .chain(std::iter::repeat(&empty))
                        .take(columns)
                        .zip(aligns.iter().chain(std::iter::repeat(&no_align)))
                        .enumerate()
                        .map(|(at, (cell, align))| {
                            cell_element(cell, *align, false, &format!("{path}/{index}/{at}"), draw)
                        })
                        .collect::<Vec<AnyElement>>(),
                ),
        );
    }
    element.into_any_element()
}

/// One cell of a table: its inline content, aligned as its column is.
fn cell_element(
    cell: &[Inline],
    align: Alignment,
    head: bool,
    path: &str,
    draw: &Draw,
) -> AnyElement {
    let theme = draw.theme;
    let element = div()
        .id(path.to_owned())
        .flex_1()
        .min_w(px(0.))
        .map(|cell| match align {
            Alignment::Left => cell.text_align(TextAlign::Left),
            Alignment::Center => cell.text_align(TextAlign::Center),
            Alignment::Right => cell.text_align(TextAlign::Right),
            Alignment::None => cell,
        })
        .text_size(theme.typography.row_subtitle_size)
        .map(|cell| {
            if head {
                cell.font_weight(theme.typography.medium)
                    .text_color(theme.text_title)
            } else {
                cell.text_color(theme.text_body)
            }
        })
        .child(inlines_element(cell, draw));
    element.into_any_element()
}

/// One run of inline content: prose, code, emphasis, strong text and
/// links, flowing beside each other and wrapping onto lines as the width
/// runs out.
fn inlines_element(inlines: &[Inline], draw: &Draw) -> Div {
    let theme = draw.theme;
    let mut element = div()
        .flex()
        .flex_wrap()
        .items_baseline()
        .min_w(px(0.))
        .text_size(theme.typography.row_subtitle_size)
        .text_color(theme.text_body);
    for inline in inlines {
        element = match inline {
            Inline::Text(text) => element.child(
                div()
                    .flex_none()
                    .min_w(px(0.))
                    .child(gpui::SharedString::from(text.clone())),
            ),
            Inline::Code(text) => element.child(
                div()
                    .flex_none()
                    .font_family(theme.typography.mono_family.clone())
                    .text_size(theme.typography.keycap_size)
                    .text_color(theme.text_title)
                    .child(gpui::SharedString::from(text.clone())),
            ),
            Inline::Emphasis(inner) => element.child(
                div()
                    .flex_none()
                    .min_w(px(0.))
                    .italic()
                    .child(inlines_element(inner, draw)),
            ),
            Inline::Strong(inner) => element.child(
                div()
                    .flex_none()
                    .min_w(px(0.))
                    .font_weight(theme.typography.medium)
                    .text_color(theme.text_title)
                    .child(inlines_element(inner, draw)),
            ),
            Inline::Link { text, href } => {
                let label = plain(text);
                element.child(
                    div()
                        .id(format!("markdown-link-{href}"))
                        .flex_none()
                        .min_w(px(0.))
                        .underline()
                        .text_color(theme.accent_text)
                        .role(Role::Link)
                        .map(|link| link.aria_label(label))
                        .map(|link| {
                            link.tooltip(text_tooltip(
                                gpui::SharedString::from(href.clone()),
                                TooltipLook::of(theme),
                            ))
                        })
                        .child(inlines_element(text, draw)),
                )
            }
            // A hard break wraps the row: a full-width, no-height run.
            Inline::Break => element.child(div().w_full().h(px(0.))),
        };
    }
    element
}

/// The plain text of inline content.
fn plain(inlines: &[Inline]) -> gpui::SharedString {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(part) | Inline::Code(part) => text.push_str(part),
            Inline::Emphasis(inner) | Inline::Strong(inner) => text.push_str(&plain(inner)),
            Inline::Link { text: inner, .. } => text.push_str(&plain(inner)),
            Inline::Break => text.push(' '),
        }
    }
    text.into()
}
