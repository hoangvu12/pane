//! Pane's Rust sample command: one list and one action per item.
#![no_std]

use pane_guest::alloc::{format, string::String, vec};
use pane_guest::{Guest, Item, View};

struct Sample;
pane_guest::export!(Sample);

impl Guest for Sample {
    async fn get_view() -> Result<View, String> {
        Ok(View {
            title: "Rust sample".into(),
            items: vec![
                Item {
                    id: "greet".into(),
                    title: "Say hello".into(),
                    subtitle: Some("Answer from the Rust guest".into()),
                },
                Item {
                    id: "wait".into(),
                    title: "Wait briefly".into(),
                    subtitle: Some("Await a WASI 0.3 clock, then answer".into()),
                },
            ],
        })
    }

    async fn run_action(item_id: String) -> Result<String, String> {
        match item_id.as_str() {
            "greet" => Ok("Hello from the Rust guest".into()),
            "wait" => {
                // A native component-model async import; the guest suspends here.
                wasip3::clocks::monotonic_clock::wait_for(50_000_000).await;
                Ok("Waited 50 ms inside the Rust guest".into())
            }
            other => Err(format!("unknown item: {other}")),
        }
    }
}
