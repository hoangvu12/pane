//! Test fixture: a guest whose actions fail in each way the host must report.
#![no_std]

use core::cell::Cell;

use pane_guest::alloc::{format, string::String, vec, vec::Vec};
use pane_guest::{
    CustomView, CustomViewInfo, CustomViewRole, Field, FieldKind, FieldValue, Form, FormError,
    Frame, Guest, GuestCustomView, Item, Key, TextField, View, ViewEvent,
};

struct Faulty;
pane_guest::export!(Faulty);

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        title: id.into(),
        subtitle: None,
        form: None,
        platforms: None,
        custom_view: None,
    }
}

/// A custom view that counts the events it handled, refuses Left and traps
/// on Right.
struct Counter {
    events: Cell<u32>,
}

impl GuestCustomView for Counter {
    async fn render(&self) -> Frame {
        Frame {
            width: 100,
            height: 20,
            shapes: Vec::new(),
            value: format!("{} events", self.events.get()),
        }
    }

    async fn handle_event(&self, event: ViewEvent) -> Result<(), String> {
        match event {
            ViewEvent::Key(Key::Left) => Err("the view refused".into()),
            ViewEvent::Key(Key::Right) => panic!("view trap"),
            _ => {
                self.events.set(self.events.get() + 1);
                Ok(())
            }
        }
    }
}

impl Guest for Faulty {
    type CustomView = Counter;

    async fn get_view() -> Result<View, String> {
        // A form whose submission is always refused as a whole.
        let form = Form {
            title: "Refused".into(),
            fields: vec![Field {
                id: "text".into(),
                label: "Text".into(),
                kind: FieldKind::Text(TextField { placeholder: None }),
            }],
            submit_label: "Submit".into(),
        };
        Ok(View {
            title: "Faulty".into(),
            items: vec![
                item("ok"),
                item("error"),
                item("trap"),
                Item {
                    form: Some(form.clone()),
                    ..item("form")
                },
                // Declares no operating system, so it is unavailable on
                // every system; activating it must not open its form.
                Item {
                    form: Some(form),
                    platforms: Some(vec![]),
                    ..item("nowhere")
                },
                Item {
                    custom_view: Some(CustomViewInfo {
                        title: "Counter".into(),
                        label: "Counter".into(),
                        role: CustomViewRole::ColorWell,
                    }),
                    ..item("view")
                },
                Item {
                    custom_view: Some(CustomViewInfo {
                        title: "Refused view".into(),
                        label: "Refused".into(),
                        role: CustomViewRole::ColorWell,
                    }),
                    ..item("no-view")
                },
            ],
        })
    }

    async fn submit_form(_item_id: String, _values: Vec<FieldValue>) -> Result<String, FormError> {
        Err(FormError {
            field: None,
            message: "the guest refused the form".into(),
        })
    }

    async fn run_action(item_id: String) -> Result<String, String> {
        match item_id.as_str() {
            "error" => Err("the guest refused".into()),
            "trap" => panic!("guest trap"),
            _ => Ok("fine".into()),
        }
    }

    async fn open_view(item_id: String) -> Result<CustomView, String> {
        match item_id.as_str() {
            "view" => Ok(CustomView::new(Counter {
                events: Cell::new(0),
            })),
            _ => Err("the guest refused the view".into()),
        }
    }
}
