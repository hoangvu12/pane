//! Contract checks every sample command passes alike, whether it is written
//! in Rust, JavaScript or TypeScript: the same items, answers and errors
//! through the launcher's public interface, a native WASI 0.3 async wait,
//! fresh state per instance, a form the guest validates, and WASI 0.3-only
//! imports.
//!
//! Components come from `cargo xtask guests`; the JavaScript and TypeScript
//! ones are the prebuilt components in `guests/prebuilt/`.

use std::path::PathBuf;

use futures::executor::block_on;
use pane_core::{
    CallError, Choice, CommandRegistration, FieldKind, FieldValue, FormError, FormField, Launcher,
    Runtime, Screen, Status,
};
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

#[path = "support/platforms.rs"]
mod platforms;

struct Sample {
    component: &'static str,
    language: &'static str,
}

const RUST: Sample = Sample {
    component: "sample_rust",
    language: "Rust",
};
const JAVASCRIPT: Sample = Sample {
    component: "sample_js",
    language: "JavaScript",
};
const TYPESCRIPT: Sample = Sample {
    component: "sample_ts",
    language: "TypeScript",
};

/// (item id, title) of every sample, in order.
const ITEMS: [(&str, &str); 7] = [
    ("greet", "Say hello"),
    ("wait", "Wait briefly"),
    ("validate", "Validate settings"),
    ("random", "Roll a number"),
    ("form", "Greet someone"),
    ("windows-only", "Windows-only action"),
    ("not-windows", "macOS and Linux action"),
];

fn guest(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests")
        .join(format!("{name}.wasm"));
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    path
}

impl Sample {
    fn path(&self) -> PathBuf {
        guest(self.component)
    }

    /// A launcher with this sample's command opened.
    fn open(&self) -> Launcher {
        let launcher = Launcher::new(
            Runtime::start(),
            vec![CommandRegistration {
                id: self.component.into(),
                title: format!("{} sample", self.language),
                subtitle: None,
                component: self.path(),
            }],
        );
        block_on(launcher.activate_selected());
        assert_eq!(launcher.view().screen, Screen::Command);
        launcher
    }

    /// Runs the item `id` in an opened launcher and returns the status.
    fn run(&self, launcher: &Launcher, id: &str) -> Status {
        let index = launcher
            .view()
            .rows
            .iter()
            .position(|row| row.id == id)
            .unwrap_or_else(|| panic!("the {} sample has no {id} item", self.language));
        launcher.select(index);
        block_on(launcher.activate_selected());
        launcher.view().status
    }

    /// A launcher with this sample's form opened.
    fn open_form(&self) -> Launcher {
        let launcher = self.open();
        assert_eq!(self.run(&launcher, "form"), Status::Idle);
        assert_eq!(launcher.view().screen, Screen::Form);
        launcher
    }

    /// Fills in the form's name and greeting, submits it and returns the status.
    fn submit(&self, launcher: &Launcher, name: &str, greeting: &str) -> Status {
        launcher.set_field_value("name", name);
        launcher.set_field_value("greeting", greeting);
        block_on(launcher.submit_form());
        launcher.view().status
    }

    /// The random item's answer in a runtime of its own.
    fn fresh_random(&self) -> f64 {
        let answer = block_on(Runtime::start().unwrap().run_action(&self.path(), "random"))
            .expect("the random item answers");
        let value: f64 = answer.parse().expect("the answer is a number");
        assert!((0.0..1.0).contains(&value), "{value} is not in [0, 1)");
        value
    }
}

fn opening_shows_the_samples_items(sample: &Sample) {
    let view = sample.open().view();

    assert_eq!(view.title, format!("{} sample", sample.language));
    let items: Vec<(&str, &str)> = view
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row.title.as_str()))
        .collect();
    assert_eq!(items, ITEMS);
    assert_eq!(view.selected, Some(0));
}

fn greeting_shows_the_guests_answer(sample: &Sample) {
    let launcher = sample.open();

    assert_eq!(
        sample.run(&launcher, "greet"),
        Status::Result(format!("Hello from the {} guest", sample.language))
    );
}

fn an_async_wasi_wait_shows_running_until_it_answers(sample: &Sample) {
    let launcher = sample.open();
    launcher.select(1);

    let pending = launcher.activate_selected();
    assert_eq!(launcher.view().status, Status::Running);
    block_on(pending);

    assert_eq!(
        launcher.view().status,
        Status::Result(format!("Waited 50 ms inside the {} guest", sample.language))
    );
}

fn a_validation_failure_is_shown_as_an_error(sample: &Sample) {
    let launcher = sample.open();

    assert_eq!(
        sample.run(&launcher, "validate"),
        Status::Error(
            "The extension reported an error: Invalid settings: port must be between 1 and 65535"
                .into()
        )
    );
    // The command keeps working after the error.
    assert!(matches!(sample.run(&launcher, "greet"), Status::Result(_)));
}

fn an_unknown_item_is_a_guest_error(sample: &Sample) {
    let runtime = Runtime::start().unwrap();

    let answer = block_on(runtime.run_action(&sample.path(), "missing"));

    assert_eq!(
        answer,
        Err(CallError::Guest("unknown item: missing".into()))
    );
}

fn separately_started_runtimes_roll_different_numbers(sample: &Sample) {
    // A snapshot that froze its random state would repeat the same number in
    // every fresh instance.
    assert_ne!(sample.fresh_random(), sample.fresh_random());
}

fn one_instance_rolls_a_new_number_each_time(sample: &Sample) {
    let launcher = sample.open();

    let first = sample.run(&launcher, "random");
    let second = sample.run(&launcher, "random");

    assert!(matches!(first, Status::Result(_)), "{first:?}");
    assert_ne!(first, second);
}

fn the_component_imports_only_wasi_0_3(sample: &Sample) {
    let imports = imports(&sample.path());

    assert!(!imports.is_empty());
    let other: Vec<&String> = imports
        .iter()
        .filter(|name| !(name.starts_with("wasi:") && name.contains("@0.3.")))
        .collect();
    assert!(other.is_empty(), "non-WASI 0.3 imports: {other:?}");
}

fn opening_the_form_shows_its_fields(sample: &Sample) {
    let view = sample.open_form().view();

    assert_eq!(view.title, "Greet someone");
    let form = view.form.expect("a form");
    assert_eq!(form.submit_label, "Greet");
    let choice = |id: &str, label: &str| Choice {
        id: id.into(),
        label: label.into(),
    };
    assert_eq!(
        form.fields,
        [
            FormField {
                id: "name".into(),
                label: "Name".into(),
                kind: FieldKind::Text {
                    placeholder: Some("Ada Lovelace".into())
                },
                value: String::new(),
                error: None,
            },
            FormField {
                id: "greeting".into(),
                label: "Greeting".into(),
                kind: FieldKind::Choice(vec![
                    choice("hello", "Hello"),
                    choice("morning", "Good morning"),
                    choice("welcome", "Welcome"),
                ]),
                value: "hello".into(),
                error: None,
            },
        ]
    );
}

fn a_valid_form_shows_the_guests_answer(sample: &Sample) {
    let launcher = sample.open_form();

    assert_eq!(
        sample.submit(&launcher, "Ada", "morning"),
        Status::Result(format!(
            "Good morning, Ada, from the {} guest",
            sample.language
        ))
    );
    assert_eq!(launcher.view().screen, Screen::Form);
}

fn an_invalid_field_is_marked_and_the_form_stays_open(sample: &Sample) {
    let launcher = sample.open_form();

    let status = sample.submit(&launcher, "   ", "welcome");

    assert_eq!(status, Status::Error("Name: Enter a name".into()));
    let view = launcher.view();
    assert_eq!(view.screen, Screen::Form);
    let fields = view.form.unwrap().fields;
    assert_eq!(fields[0].error.as_deref(), Some("Enter a name"));
    assert_eq!(fields[1].error, None);
    // The values survive the rejection, and a corrected form is accepted.
    assert_eq!(
        (fields[0].value.as_str(), fields[1].value.as_str()),
        ("   ", "welcome")
    );
    assert_eq!(
        sample.submit(&launcher, "Grace", "welcome"),
        Status::Result(format!(
            "Welcome, Grace, from the {} guest",
            sample.language
        ))
    );
}

fn a_too_long_name_is_rejected_by_the_guest(sample: &Sample) {
    let launcher = sample.open_form();

    let status = sample.submit(&launcher, &"x".repeat(41), "hello");

    assert_eq!(
        status,
        Status::Error("Name: Use at most 40 characters".into())
    );
}

fn an_unknown_choice_is_a_field_error_from_the_guest(sample: &Sample) {
    // The launcher only submits offered choices, so call the guest directly.
    let runtime = Runtime::start().unwrap();
    let values = [("name", "Ada"), ("greeting", "howdy")]
        .map(|(id, value)| FieldValue {
            id: id.into(),
            value: value.into(),
        })
        .to_vec();

    let answer = block_on(runtime.submit_form(&sample.path(), "form", values));

    assert_eq!(
        answer,
        Err(CallError::Form(FormError {
            field: Some("greeting".into()),
            message: "Choose a greeting".into(),
        }))
    );
}

/// Each sample declares one action for Windows only and one for macOS and
/// Linux only. On the system the test runs on, Pane runs the one declared
/// for it and explains the other without calling the guest, and every other
/// action keeps working.
fn a_platform_limited_action_runs_only_on_its_declared_systems(sample: &Sample) {
    let launcher = sample.open();
    let reason = |id: &str| {
        let view = launcher.view();
        let row = view.rows.into_iter().find(|row| row.id == id);
        row.expect("the item is listed").unavailable
    };
    let ran =
        |title: &str| Status::Result(format!("Ran the {title} in the {} guest", sample.language));
    let (available, (unavailable, _), explanation) = platforms::sample_items();

    assert_eq!(reason(available.0), None);
    assert_eq!(reason(unavailable), Some(explanation.clone()));
    assert_eq!(sample.run(&launcher, available.0), ran(available.1));
    assert_eq!(
        sample.run(&launcher, unavailable),
        Status::Error(explanation)
    );
    assert_eq!(launcher.view().screen, Screen::Command);
    assert_eq!(
        sample.run(&launcher, "greet"),
        Status::Result(format!("Hello from the {} guest", sample.language))
    );
}

/// Declares one test per check for each sample.
macro_rules! contract {
    ($($check:ident),* $(,)?) => {
        mod rust {
            $(#[test] fn $check() { super::$check(&super::RUST) })*
        }
        mod javascript {
            $(#[test] fn $check() { super::$check(&super::JAVASCRIPT) })*
        }
        mod typescript {
            $(#[test] fn $check() { super::$check(&super::TYPESCRIPT) })*
        }
    };
}

contract!(
    opening_shows_the_samples_items,
    greeting_shows_the_guests_answer,
    an_async_wasi_wait_shows_running_until_it_answers,
    a_validation_failure_is_shown_as_an_error,
    an_unknown_item_is_a_guest_error,
    separately_started_runtimes_roll_different_numbers,
    one_instance_rolls_a_new_number_each_time,
    the_component_imports_only_wasi_0_3,
    opening_the_form_shows_its_fields,
    a_valid_form_shows_the_guests_answer,
    an_invalid_field_is_marked_and_the_form_stays_open,
    a_too_long_name_is_rejected_by_the_guest,
    an_unknown_choice_is_a_field_error_from_the_guest,
    a_platform_limited_action_runs_only_on_its_declared_systems,
);

/// The names of the component's imports.
fn imports(path: &std::path::Path) -> Vec<String> {
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .wasm_component_model_async(true);
    let engine = Engine::new(&config).unwrap();
    let component = Component::from_file(&engine, path).unwrap();
    component
        .component_type()
        .imports(&engine)
        .map(|(name, _)| name.to_owned())
        .collect()
}

#[test]
fn the_import_check_detects_a_wasi_0_2_import() {
    // Negative control for `the_component_imports_only_wasi_0_3`.
    let imports = imports(&guest("mixed_p2"));

    assert!(
        imports
            .iter()
            .any(|name| name.starts_with("wasi:cli/stdout@0.2")),
        "{imports:?}"
    );
}
