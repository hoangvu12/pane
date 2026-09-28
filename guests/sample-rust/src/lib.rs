//! Pane's Rust sample command: a list with one action per item, and a form.
//! Items, titles, results and errors match the JavaScript and TypeScript
//! samples.
#![no_std]

use pane_guest::alloc::{format, string::String, vec, vec::Vec};
use pane_guest::{
    Choice, Field, FieldKind, FieldValue, Form, FormError, Guest, Item, TextField, View,
};

struct Sample;
pane_guest::export!(Sample);

/// Settings the "validate" item checks; the port is out of range on purpose.
struct Settings {
    name: &'static str,
    port: u32,
}

impl Settings {
    fn validate(&self) -> Result<(), &'static str> {
        if self.name.is_empty() {
            return Err("name must not be empty");
        }
        if !(1..=65535).contains(&self.port) {
            return Err("port must be between 1 and 65535");
        }
        Ok(())
    }
}

/// The greeting form's options: (id, label).
const GREETINGS: [(&str, &str); 3] = [
    ("hello", "Hello"),
    ("morning", "Good morning"),
    ("welcome", "Welcome"),
];

/// The "form" item's form: a name to greet and a greeting to choose.
fn greeting_form() -> Form {
    Form {
        title: "Greet someone".into(),
        fields: vec![
            Field {
                id: "name".into(),
                label: "Name".into(),
                kind: FieldKind::Text(TextField {
                    placeholder: Some("Ada Lovelace".into()),
                }),
            },
            Field {
                id: "greeting".into(),
                label: "Greeting".into(),
                kind: FieldKind::Choice(
                    GREETINGS
                        .iter()
                        .map(|&(id, label)| Choice {
                            id: id.into(),
                            label: label.into(),
                        })
                        .collect(),
                ),
            },
        ],
        submit_label: "Greet".into(),
    }
}

/// An error about the field `field`.
fn invalid(field: &str, message: &str) -> FormError {
    FormError {
        field: Some(field.into()),
        message: message.into(),
    }
}

impl Guest for Sample {
    async fn get_view() -> Result<View, String> {
        let item = |id: &str, title: &str, subtitle: &str| Item {
            id: id.into(),
            title: title.into(),
            subtitle: Some(subtitle.into()),
            form: None,
        };
        Ok(View {
            title: "Rust sample".into(),
            items: vec![
                item("greet", "Say hello", "Answer from the Rust guest"),
                item(
                    "wait",
                    "Wait briefly",
                    "Await a WASI 0.3 clock, then answer",
                ),
                item(
                    "validate",
                    "Validate settings",
                    "Reject settings with an out-of-range port",
                ),
                item(
                    "random",
                    "Roll a number",
                    "A random number from this instance",
                ),
                Item {
                    form: Some(greeting_form()),
                    ..item("form", "Greet someone", "Fill in a form the guest checks")
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
            "validate" => {
                let settings = Settings {
                    name: "Pane",
                    port: 70000,
                };
                settings
                    .validate()
                    .map_err(|problem| format!("Invalid settings: {problem}"))?;
                Ok(format!(
                    "Settings are valid: {} on port {}",
                    settings.name, settings.port
                ))
            }
            "random" => {
                // A number in [0, 1) from 53 random bits, like `Math.random()`.
                let bits = wasip3::random::random::get_random_u64() >> 11;
                Ok(format!("{}", bits as f64 / (1u64 << 53) as f64))
            }
            other => Err(format!("unknown item: {other}")),
        }
    }

    async fn submit_form(item_id: String, values: Vec<FieldValue>) -> Result<String, FormError> {
        if item_id != "form" {
            return Err(FormError {
                field: None,
                message: format!("unknown form: {item_id}"),
            });
        }
        let value = |id: &str| {
            values
                .iter()
                .find(|field| field.id == id)
                .map_or("", |field| field.value.as_str())
        };
        let name = value("name").trim();
        if name.is_empty() {
            return Err(invalid("name", "Enter a name"));
        }
        if name.chars().count() > 40 {
            return Err(invalid("name", "Use at most 40 characters"));
        }
        let greeting = value("greeting");
        let Some(&(_, greeting)) = GREETINGS.iter().find(|&&(id, _)| id == greeting) else {
            return Err(invalid("greeting", "Choose a greeting"));
        };
        Ok(format!("{greeting}, {name}, from the Rust guest"))
    }
}
