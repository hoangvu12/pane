//! Pane's designed view sample in Rust: a counter whose screen is the
//! tree its `view.render` answers — a column of a text and a row of
//! buttons, drawn by Pane, not by the guest (`docs/designed-tree.md`,
//! ADR 0036). The behaviour matches the JavaScript and TypeScript samples
//! (`sample-view-js`, `sample-view-ts`): the same text, the same buttons.
#![no_std]

use core::cell::Cell;

use pane_extension::alloc::{format, string::String};
use pane_extension::view::{
    Cx, IntoNode, Space, TextLevel, TextStyle, Tone, View, button, column, row, text,
};
use pane_extension::{Command, LaunchRecord};

/// The counter the screen shows.
struct Counter {
    count: Cell<u32>,
}

impl View for Counter {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoNode {
        column()
            .gap(Space::M)
            .child(
                text(format!("Count: {}", self.count.get()))
                    .style(TextStyle::Title)
                    .level(TextLevel::Primary),
            )
            .child(
                row()
                    .gap(Space::S)
                    .children([
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
    }
}

struct Sample;
pane_extension::export!(Sample);

impl Command for Sample {
    type CustomView = pane_extension::NoCustomView;
    type DesignedView = Counter;

    async fn open_designed_view(
        _command: String,
        _launch: LaunchRecord,
    ) -> Result<Counter, String> {
        Ok(Counter {
            count: Cell::new(0),
        })
    }
}
