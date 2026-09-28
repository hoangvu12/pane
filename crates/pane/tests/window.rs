//! Drives the native launcher window through GPUI's test platform: real key
//! and mouse events dispatch to the window, which runs real guest components.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, prelude::*};
use pane::LauncherWindow;
use pane_core::{CommandRegistration, Launcher, Runtime, Screen, Status};

/// A sample command: its component and the language it is written in.
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

fn command(title: &str, guest: &str) -> CommandRegistration {
    let component = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests")
        .join(format!("{guest}.wasm"));
    assert!(
        component.exists(),
        "{} is missing; run `cargo xtask guests`",
        component.display()
    );
    CommandRegistration {
        id: guest.into(),
        title: title.into(),
        subtitle: None,
        component,
    }
}

fn open<'a>(
    cx: &'a mut TestAppContext,
    sample: &Sample,
) -> (Entity<LauncherWindow>, &'a mut VisualTestContext) {
    let title = format!("{} sample", sample.language);
    open_with(cx, vec![command(&title, sample.component)])
}

fn open_with(
    cx: &mut TestAppContext,
    commands: Vec<CommandRegistration>,
) -> (Entity<LauncherWindow>, &mut VisualTestContext) {
    // Guest replies arrive from the real runtime thread, outside the test
    // scheduler's deterministic control.
    cx.executor().allow_parking();
    cx.update(pane::bind_keys);
    let launcher = Launcher::new(Runtime::start(), commands);
    cx.add_window_view(|window, cx| LauncherWindow::new(launcher, window, cx))
}

/// Lets the window apply guest replies, which arrive from the runtime thread.
fn wait_for_answer(
    window: &Entity<LauncherWindow>,
    cx: &mut VisualTestContext,
) -> pane_core::LauncherView {
    // Generous: opening a JS command compiles a 4 MB component first.
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        let view = cx.read_entity(window, |window, _| window.launcher().view());
        if view.status != Status::Running {
            return view;
        }
        assert!(Instant::now() < deadline, "the guest did not answer");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Opens the sample's command with Enter and clicks the row whose debug
/// selector is `row` (`row-<title>`).
fn click_row(
    window: &Entity<LauncherWindow>,
    cx: &mut VisualTestContext,
    row: &'static str,
) -> pane_core::LauncherView {
    cx.simulate_keystrokes("enter");
    wait_for_answer(window, cx);
    let row = cx.debug_bounds(row).expect("row rendered");
    cx.simulate_click(row.center(), Modifiers::none());
    wait_for_answer(window, cx)
}

fn the_keyboard_opens_the_sample_and_runs_an_action(cx: &mut TestAppContext, sample: &Sample) {
    let (window, cx) = open(cx, sample);

    cx.simulate_keystrokes("enter");
    let view = wait_for_answer(&window, cx);
    assert_eq!(view.screen, Screen::Command);
    assert_eq!(view.title, format!("{} sample", sample.language));
    assert!(
        cx.debug_bounds("row-Say hello").is_some(),
        "guest rows are rendered"
    );

    cx.simulate_keystrokes("enter");
    let view = wait_for_answer(&window, cx);
    assert_eq!(
        view.status,
        Status::Result(format!("Hello from the {} guest", sample.language))
    );
    assert!(
        cx.debug_bounds("status-result").is_some(),
        "the answer is rendered"
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(wait_for_answer(&window, cx).screen, Screen::Root);
}

fn clicking_a_row_runs_its_action(cx: &mut TestAppContext, sample: &Sample) {
    let (window, cx) = open(cx, sample);

    let view = click_row(&window, cx, "row-Wait briefly");

    assert_eq!(view.selected, Some(1));
    assert_eq!(
        view.status,
        Status::Result(format!("Waited 50 ms inside the {} guest", sample.language))
    );
}

fn a_validation_error_is_rendered(cx: &mut TestAppContext, sample: &Sample) {
    let (window, cx) = open(cx, sample);

    let view = click_row(&window, cx, "row-Validate settings");

    assert_eq!(
        view.status,
        Status::Error(
            "The extension reported an error: Invalid settings: port must be between 1 and 65535"
                .into()
        )
    );
    assert!(
        cx.debug_bounds("status-error").is_some(),
        "the error is rendered"
    );
}

/// Opens the sample's command and then its form ("Greet someone", the fifth
/// item) with the keyboard.
fn open_form(window: &Entity<LauncherWindow>, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    wait_for_answer(window, cx);
    cx.simulate_keystrokes("down down down down enter");
    let view = wait_for_answer(window, cx);
    assert_eq!(
        (view.screen, view.title.as_str()),
        (Screen::Form, "Greet someone")
    );
}

fn the_keyboard_fills_in_and_submits_the_form(cx: &mut TestAppContext, sample: &Sample) {
    let (window, cx) = open(cx, sample);
    open_form(&window, cx);
    assert!(
        cx.debug_bounds("field-name").is_some(),
        "the form is rendered"
    );

    // The name field has focus; Tab moves to the greeting, Down chooses the
    // next greeting, and Enter submits.
    cx.simulate_input("Ada");
    cx.simulate_keystrokes("tab down enter");

    let view = wait_for_answer(&window, cx);
    assert_eq!(
        view.status,
        Status::Result(format!(
            "Good morning, Ada, from the {} guest",
            sample.language
        ))
    );
    assert!(
        cx.debug_bounds("status-result").is_some(),
        "the answer is rendered"
    );
    cx.simulate_keystrokes("escape");
    let view = wait_for_answer(&window, cx);
    assert_eq!((view.screen, view.selected), (Screen::Command, Some(4)));
}

fn a_rejected_field_shows_its_error_and_takes_focus(cx: &mut TestAppContext, sample: &Sample) {
    let (window, cx) = open(cx, sample);
    open_form(&window, cx);

    // Submit from the greeting with the name left empty.
    cx.simulate_keystrokes("tab enter");

    let view = wait_for_answer(&window, cx);
    assert_eq!(view.status, Status::Error("Name: Enter a name".into()));
    assert!(
        cx.debug_bounds("field-error-name").is_some(),
        "the error is rendered next to the field"
    );
    let (nodes, focused) = accessibility_tree(cx);
    assert_eq!(focused.as_deref(), Some("Name"));
    assert!(
        nodes.contains(&("TextInput".into(), "Name".into(), "Enter a name".into())),
        "{nodes:?}"
    );

    // Focus is back on the name, so typing fixes it.
    cx.simulate_input("Grace");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        wait_for_answer(&window, cx).status,
        Status::Result(format!("Hello, Grace, from the {} guest", sample.language))
    );
}

/// Declares one window test per check for each sample.
macro_rules! for_each_sample {
    ($($check:ident),* $(,)?) => {
        mod rust {
            $(#[gpui::test] fn $check(cx: &mut gpui::TestAppContext) { super::$check(cx, &super::RUST) })*
        }
        mod javascript {
            $(#[gpui::test] fn $check(cx: &mut gpui::TestAppContext) { super::$check(cx, &super::JAVASCRIPT) })*
        }
        mod typescript {
            $(#[gpui::test] fn $check(cx: &mut gpui::TestAppContext) { super::$check(cx, &super::TYPESCRIPT) })*
        }
    };
}

for_each_sample!(
    the_keyboard_opens_the_sample_and_runs_an_action,
    clicking_a_row_runs_its_action,
    a_validation_error_is_rendered,
    the_keyboard_fills_in_and_submits_the_form,
    a_rejected_field_shows_its_error_and_takes_focus,
);

/// The label of the node assistive technology treats as focused.
fn focused_label(cx: &mut VisualTestContext) -> Option<String> {
    accessibility_tree(cx).1
}

/// The open form's value of field `id`.
fn field_value(window: &Entity<LauncherWindow>, cx: &mut VisualTestContext, id: &str) -> String {
    let view = cx.read_entity(window, |window, _| window.launcher().view());
    let form = view.form.expect("a form is open");
    let field = form.fields.into_iter().find(|field| field.id == id);
    field.expect("the field exists").value
}

#[gpui::test]
fn tab_and_shift_tab_visit_each_control_once_in_order(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);
    assert_eq!(focused_label(cx).as_deref(), Some("Name"));

    // Two full rounds each way: a control with two tab stops (such as the
    // text field and a wrapper tracking its focus) would appear twice in a
    // row. The greeting group reports its chosen option as focused, like a
    // list reports its selected row.
    let mut forward = Vec::new();
    for _ in 0..6 {
        cx.simulate_keystrokes("tab");
        forward.push(focused_label(cx));
    }
    let mut backward = Vec::new();
    for _ in 0..6 {
        cx.simulate_keystrokes("shift-tab");
        backward.push(focused_label(cx));
    }

    let labels = |order: [&str; 6]| order.map(|label| Some(label.to_owned()));
    assert_eq!(
        forward,
        labels(["Hello", "Greet", "Name", "Hello", "Greet", "Name"])
    );
    assert_eq!(
        backward,
        labels(["Greet", "Hello", "Name", "Greet", "Hello", "Name"])
    );
}

#[gpui::test]
fn editing_keys_change_the_text_field(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);

    cx.simulate_input("Ad");
    cx.simulate_keystrokes("left");
    cx.simulate_input("x");
    assert_eq!(field_value(&window, cx, "name"), "Axd");
    cx.simulate_keystrokes("backspace right");
    cx.simulate_input("a");
    assert_eq!(field_value(&window, cx, "name"), "Ada");

    // Space is text in the field, not a key for the form.
    cx.simulate_keystrokes("space");
    assert_eq!(field_value(&window, cx, "name"), "Ada ");
}

/// GPUI CE's test platform offers no public way to reach the window's
/// platform input handler, so composition is driven on the name field's
/// editing state, which the window's input handler forwards to. The test
/// checks that this state belongs to the focused field; typing through the
/// window's input handler is covered by the other form tests.
#[gpui::test]
fn input_method_composition_commits_into_the_text_field(cx: &mut TestAppContext) {
    use gpui::{EntityInputHandler, Focusable};

    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);
    let input = cx
        .read_entity(&window, |window, _| window.text_field("name"))
        .expect("the name field has an editing state");
    let focused = cx.update(|window, cx| input.focus_handle(cx).is_focused(window));
    assert!(focused, "the name field has keyboard focus");

    // What a platform input method does: mark composing text, then replace
    // it with the committed text.
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "にほ", None, window, cx);
        })
    });
    assert_eq!(field_value(&window, cx, "name"), "にほ");
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            assert_eq!(input.marked_text_range(window, cx), Some(0..2));
            input.replace_text_in_range(None, "日本", window, cx);
            assert_eq!(input.marked_text_range(window, cx), None);
        })
    });
    assert_eq!(field_value(&window, cx, "name"), "日本");

    cx.simulate_keystrokes("enter");
    assert_eq!(
        wait_for_answer(&window, cx).status,
        Status::Result("Hello, 日本, from the Rust guest".into())
    );
}

#[gpui::test]
fn clicking_a_choice_and_the_submit_button_submits_the_form(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);
    cx.simulate_input("Ada");

    let welcome = cx
        .debug_bounds("choice-greeting-welcome")
        .expect("choice rendered");
    cx.simulate_click(welcome.center(), Modifiers::none());
    assert_eq!(field_value(&window, cx, "greeting"), "welcome");
    let submit = cx.debug_bounds("submit").expect("submit button rendered");
    cx.simulate_click(submit.center(), Modifiers::none());

    assert_eq!(
        wait_for_answer(&window, cx).status,
        Status::Result("Welcome, Ada, from the Rust guest".into())
    );
}

#[gpui::test]
fn the_focused_submit_button_submits_with_space(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);
    cx.simulate_input("Ada");

    cx.simulate_keystrokes("tab tab space");

    assert_eq!(
        wait_for_answer(&window, cx).status,
        Status::Result("Hello, Ada, from the Rust guest".into())
    );
}

/// Every accessibility node's properties (`role`, `label`, `value`,
/// `toggled`, ...), as GPUI reports them to assistive technology.
fn accessible_nodes(cx: &mut VisualTestContext) -> Vec<serde_json::Value> {
    cx.update(|window, _| window.set_a11y_forced(true));
    cx.run_until_parked();
    let json = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .expect("an accessibility tree");
    let tree: serde_json::Value = serde_json::from_str(&json).unwrap();
    let nodes = tree["nodes"].as_object().unwrap();
    nodes.values().map(|node| node["aria"].clone()).collect()
}

/// The node with this role and label.
fn node<'a>(nodes: &'a [serde_json::Value], role: &str, label: &str) -> &'a serde_json::Value {
    nodes
        .iter()
        .find(|node| node["role"] == role && node["label"] == label)
        .unwrap_or_else(|| panic!("no {role} labelled {label:?} in {nodes:#?}"))
}

#[gpui::test]
fn assistive_technology_sees_the_forms_labelled_controls_and_values(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    open_form(&window, cx);
    cx.simulate_input("Ada");

    let nodes = accessible_nodes(cx);
    node(&nodes, "Form", "Greet someone");
    let name = node(&nodes, "TextInput", "Name");
    assert_eq!(
        (&name["value"], &name["placeholder"]),
        (&"Ada".into(), &"Ada Lovelace".into())
    );
    node(&nodes, "RadioGroup", "Greeting");
    assert_eq!(node(&nodes, "RadioButton", "Hello")["toggled"], "True");
    assert_eq!(
        node(&nodes, "RadioButton", "Good morning")["toggled"],
        "False"
    );
    node(&nodes, "Button", "Greet");
}

#[gpui::test]
fn the_launcher_offers_the_rust_javascript_and_typescript_samples(cx: &mut TestAppContext) {
    let (window, cx) = open_with(cx, pane::sample_commands());
    let root = wait_for_answer(&window, cx);
    let titles: Vec<&str> = root.rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Rust sample", "JavaScript sample", "TypeScript sample"]
    );

    for (index, title) in titles.iter().enumerate() {
        cx.simulate_keystrokes("enter");
        let view = wait_for_answer(&window, cx);
        assert_eq!(
            (view.screen, view.title.as_str()),
            (Screen::Command, *title)
        );

        cx.simulate_keystrokes("escape");
        let view = wait_for_answer(&window, cx);
        assert_eq!(view.screen, Screen::Root);
        for _ in 0..=index {
            cx.simulate_keystrokes("down");
        }
    }
}

#[gpui::test]
fn arrow_keys_move_the_selection(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    cx.simulate_keystrokes("enter");
    wait_for_answer(&window, cx);

    cx.simulate_keystrokes("down");
    assert_eq!(wait_for_answer(&window, cx).selected, Some(1));
    cx.simulate_keystrokes("up");
    assert_eq!(wait_for_answer(&window, cx).selected, Some(0));
}

/// Whether the element with debug selector `row` lies wholly inside the list.
fn row_is_visible(cx: &mut VisualTestContext, row: &'static str) -> bool {
    let list = cx.debug_bounds("rows").expect("the list is rendered");
    let row = cx.debug_bounds(row).expect("the row is rendered");
    row.top() >= list.top() && row.bottom() <= list.bottom()
}

#[gpui::test]
fn the_list_scrolls_to_keep_the_selected_row_visible(cx: &mut TestAppContext) {
    const TITLES: [&str; 12] = [
        "Row 1", "Row 2", "Row 3", "Row 4", "Row 5", "Row 6", "Row 7", "Row 8", "Row 9", "Row 10",
        "Row 11", "Row 12",
    ];
    let commands = TITLES
        .iter()
        .map(|title| command(title, "sample_rust"))
        .collect();
    let (window, cx) = open_with(cx, commands);
    // The size of Pane's window: fewer than half of the rows fit.
    cx.simulate_resize(gpui::size(gpui::px(640.), gpui::px(420.)));
    cx.run_until_parked();
    assert!(row_is_visible(cx, "row-Row 1"));
    assert!(!row_is_visible(cx, "row-Row 12"), "the list overflows");

    for _ in 1..TITLES.len() {
        cx.simulate_keystrokes("down");
    }
    cx.run_until_parked();
    assert_eq!(wait_for_answer(&window, cx).selected, Some(11));
    assert!(
        row_is_visible(cx, "row-Row 12"),
        "the last row is scrolled into view"
    );
    assert!(!row_is_visible(cx, "row-Row 1"));

    for _ in 1..TITLES.len() {
        cx.simulate_keystrokes("up");
    }
    cx.run_until_parked();
    assert!(row_is_visible(cx, "row-Row 1"), "and back to the first");
}

#[gpui::test]
fn a_rejected_extension_shows_an_error_and_navigation_keeps_working(cx: &mut TestAppContext) {
    let (window, cx) = open_with(
        cx,
        vec![
            command("Mixed", "mixed_p2"),
            command("Rust sample", "sample_rust"),
        ],
    );

    cx.simulate_keystrokes("enter");
    let view = wait_for_answer(&window, cx);
    assert_eq!(view.screen, Screen::Root);
    assert!(
        cx.debug_bounds("status-error").is_some(),
        "the error is rendered"
    );

    cx.simulate_keystrokes("down enter");
    assert_eq!(wait_for_answer(&window, cx).title, "Rust sample");
}

/// The window's accessibility tree, as (role, label, description) per node,
/// plus the label of the node assistive technology treats as focused.
fn accessibility_tree(
    cx: &mut VisualTestContext,
) -> (Vec<(String, String, String)>, Option<String>) {
    cx.update(|window, _| window.set_a11y_forced(true));
    cx.run_until_parked();
    let json = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .expect("an accessibility tree");
    let tree: serde_json::Value = serde_json::from_str(&json).unwrap();
    let field = |node: &serde_json::Value, key: &str| {
        node["aria"][key].as_str().unwrap_or_default().to_owned()
    };
    let nodes = tree["nodes"].as_object().unwrap();
    let focused = ["active_descendant_focus", "gpui_focus"]
        .iter()
        .find_map(|key| tree[key].as_str())
        .map(|id| field(&nodes[id], "label"));
    let nodes = nodes
        .values()
        .map(|node| {
            (
                field(node, "role"),
                field(node, "label"),
                field(node, "description"),
            )
        })
        .collect();
    (nodes, focused)
}

fn has(nodes: &[(String, String, String)], role: &str, label: &str) -> bool {
    nodes.iter().any(|(r, l, _)| r == role && l == label)
}

#[gpui::test]
fn assistive_technology_sees_the_list_the_selection_and_the_result(cx: &mut TestAppContext) {
    let (window, cx) = open(cx, &RUST);
    cx.simulate_keystrokes("enter");
    wait_for_answer(&window, cx);

    let (nodes, focused) = accessibility_tree(cx);
    assert!(has(&nodes, "ListBox", "Rust sample"), "{nodes:?}");
    assert!(has(&nodes, "ListBoxOption", "Say hello"), "{nodes:?}");
    assert!(
        nodes
            .iter()
            .any(|(_, label, description)| label == "Wait briefly"
                && description == "Await a WASI 0.3 clock, then answer"),
        "{nodes:?}"
    );
    assert_eq!(focused.as_deref(), Some("Say hello"));

    cx.simulate_keystrokes("down enter");
    wait_for_answer(&window, cx);
    let (nodes, focused) = accessibility_tree(cx);
    assert_eq!(focused.as_deref(), Some("Wait briefly"));
    assert!(
        has(&nodes, "Status", "Waited 50 ms inside the Rust guest"),
        "{nodes:?}"
    );
}
