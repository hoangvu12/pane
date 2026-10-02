//! Presentation-only row chrome; callers retain selection and dispatch.

use gpui::{Div, Role, Stateful, div, prelude::*, rgb};

pub(crate) struct ResultRow {
    pub title: String,
    pub subtitle: Option<String>,
    pub unavailable_reason: Option<String>,
}

pub(crate) fn result_row(index: usize, row: ResultRow, selected: bool) -> Stateful<Div> {
    let reason_selector = format!("unavailable-reason-{}", row.title);
    div()
        .id(("row", index))
        .debug_selector(|| format!("row-{}", row.title))
        .flex()
        .flex_col()
        .px_3()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .role(Role::ListBoxOption)
        .aria_label(row.title.clone())
        .aria_selected(selected)
        .when(selected, |row| {
            row.aria_active_descendant().bg(rgb(0x364355))
        })
        // Hovering never hides which row is selected.
        .when(!selected, |row| row.hover(|row| row.bg(rgb(0x2e3a48))))
        .child(
            div()
                .when(row.unavailable_reason.is_some(), |title| {
                    title.text_color(rgb(0x8a96a3))
                })
                .child(row.title),
        )
        .when_some(row.subtitle.clone(), |element, subtitle| {
            element.child(div().text_sm().text_color(rgb(0xaab4c0)).child(subtitle))
        })
        // An unavailable row stays listed and selectable; it says why it
        // cannot run here, on screen and to assistive technology.
        .when_some(row.unavailable_reason.clone(), |element, reason| {
            element.aria_disabled(true).child(
                div()
                    .id(("unavailable", index))
                    .debug_selector(|| reason_selector)
                    .text_sm()
                    .text_color(rgb(0xd6a36a))
                    .child(reason),
            )
        })
        .when_some(
            match (row.subtitle, row.unavailable_reason) {
                (Some(subtitle), Some(reason)) => Some(format!("{subtitle}. {reason}")),
                (subtitle, reason) => subtitle.or(reason),
            },
            |element, description| element.aria_description(description),
        )
}
