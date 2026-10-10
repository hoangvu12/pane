//! Pane's timer sample in Rust: a clock whose screen changes by itself —
//! the view asks Pane to render it again after every second
//! (`refresh-after-ms`, #236) — with its caption arriving as pending data
//! shown first as a loading state (`Pending` and `loading`). The
//! behaviour matches the JavaScript and TypeScript samples
//! (`sample-timer-js`, `sample-timer-ts`): the same texts, the same
//! timings.
#![no_std]

use core::cell::Cell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use pane_extension::alloc::{format, string::String};
use pane_extension::view::{
    Cx, IntoAnswer, Pending, Space, TextLevel, TextStyle, View, column, loading, text,
};
use pane_extension::{Command, LaunchRecord};

/// What the timer says while its caption is still loading.
const LOADING: &str = "Loading…";

/// The timer the screen shows: a caption loaded before it runs, and the
/// seconds passed since, one per refresh.
struct Timer {
    caption: Pending<String>,
    elapsed: Cell<u32>,
}

impl View for Timer {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoAnswer {
        match self.caption.ready() {
            Some(caption) => {
                // The render that answers the view's own ask is where the
                // interval's work runs: one tick per second asked for.
                if cx.refreshed() {
                    self.elapsed.set(self.elapsed.get() + 1);
                }
                column()
                    .gap(Space::M)
                    .child(text(caption.as_str()).level(TextLevel::Secondary))
                    .child(
                        text(format!("Elapsed: {}s", self.elapsed.get()))
                            .style(TextStyle::Title),
                    )
                    .refresh_after(Duration::from_secs(1))
            }
            // The loading state: shown at once, with the prompt refresh
            // that awaits the caption asked for.
            None => loading(text(LOADING).level(TextLevel::Secondary)),
        }
    }
}

/// The work that fills the screen when it answers: the timer's caption,
/// ready the second time it is polled — after the prompt refresh Pane asks
/// for — so the loading state shows once. (Real work would await a file or
/// a service; a guest has no clock of its own, so a refresh is the wait.)
fn load_caption() -> impl Future<Output = String> {
    SecondPoll {
        answer: "A second at a time",
        polled: false,
    }
}

/// A future that answers on its second poll.
struct SecondPoll {
    answer: &'static str,
    polled: bool,
}

impl Future for SecondPoll {
    type Output = String;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<String> {
        let this = self.get_mut();
        if this.polled {
            Poll::Ready(String::from(this.answer))
        } else {
            this.polled = true;
            Poll::Pending
        }
    }
}

struct Sample;
pane_extension::export!(Sample);

impl Command for Sample {
    type CustomView = pane_extension::NoCustomView;
    type DesignedView = Timer;

    async fn open_designed_view(
        _command: String,
        _launch: LaunchRecord,
    ) -> Result<Timer, String> {
        Ok(Timer {
            caption: Pending::loading(load_caption()),
            elapsed: Cell::new(0),
        })
    }
}
