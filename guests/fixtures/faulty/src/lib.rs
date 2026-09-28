//! Test fixture: a guest whose actions fail in each way the host must report.
#![no_std]

use pane_guest::alloc::{string::String, vec, vec::Vec};
use pane_guest::{Field, FieldKind, FieldValue, Form, FormError, Guest, Item, TextField, View};

struct Faulty;
pane_guest::export!(Faulty);

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        title: id.into(),
        subtitle: None,
        form: None,
        platforms: None,
    }
}

impl Guest for Faulty {
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
}
