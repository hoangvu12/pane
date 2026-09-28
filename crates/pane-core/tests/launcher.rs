//! Drives the launcher through its public interface against real guest
//! components built by `cargo xtask guests`.

use std::path::PathBuf;

use futures::executor::block_on;
use pane_core::{CallError, CommandRegistration, Launcher, Runtime, Screen, Status};

#[path = "support/platforms.rs"]
mod platforms;

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

fn command(id: &str, component: PathBuf) -> CommandRegistration {
    CommandRegistration {
        id: id.into(),
        title: format!("{id} command"),
        subtitle: None,
        component,
    }
}

fn launcher(commands: Vec<CommandRegistration>) -> Launcher {
    Launcher::new(Runtime::start(), commands)
}

fn titles(launcher: &Launcher) -> Vec<String> {
    launcher
        .view()
        .rows
        .into_iter()
        .map(|row| row.title)
        .collect()
}

fn open_faulty_item(launcher: &Launcher, item: &str) {
    block_on(launcher.activate_selected());
    let index = titles(launcher)
        .iter()
        .position(|title| title == item)
        .unwrap();
    launcher.move_selection(index as isize);
}

fn error(launcher: &Launcher) -> String {
    match launcher.view().status {
        Status::Error(message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_mixed_wasi_02_component_is_rejected_and_root_stays_usable() {
    let launcher = launcher(vec![
        command("mixed", guest("mixed_p2")),
        command("sample", guest("sample_rust")),
    ]);

    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().screen, Screen::Root);
    let message = error(&launcher);
    assert!(message.contains("only WASI 0.3"), "{message}");
    assert!(message.contains("wasi:cli/stdout@0.2"), "{message}");

    launcher.move_selection(1);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().title, "Rust sample");
}

#[test]
fn back_returns_from_a_command_to_root_search() {
    let launcher = launcher(vec![command("sample", guest("sample_rust"))]);
    block_on(launcher.activate_selected());

    launcher.back();

    let view = launcher.view();
    assert_eq!(view.screen, Screen::Root);
    assert_eq!(view.status, Status::Idle);
    assert_eq!(titles(&launcher), ["sample command"]);
}

#[test]
fn a_guest_error_is_shown_as_an_error() {
    let launcher = launcher(vec![command("faulty", guest("faulty"))]);
    open_faulty_item(&launcher, "error");

    block_on(launcher.activate_selected());

    assert!(error(&launcher).contains("the guest refused"));
}

#[test]
fn a_guest_trap_is_shown_and_the_command_keeps_working() {
    let launcher = launcher(vec![command("faulty", guest("faulty"))]);
    open_faulty_item(&launcher, "trap");

    block_on(launcher.activate_selected());
    assert!(error(&launcher).contains("crashed"));

    launcher.move_selection(-2);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().status, Status::Result("fine".into()));
}

#[test]
fn a_missing_component_explains_itself_on_root() {
    let launcher = launcher(vec![command("gone", PathBuf::from("does-not-exist.wasm"))]);

    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().screen, Screen::Root);
    assert!(error(&launcher).contains("does-not-exist.wasm"));
}

#[test]
fn an_unavailable_runtime_leaves_root_navigable() {
    let unavailable = Err(CallError::RuntimeUnavailable("no engine".into()));
    let launcher = Launcher::new(unavailable, vec![command("sample", guest("sample_rust"))]);

    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().screen, Screen::Root);
    assert_eq!(error(&launcher), "Extension runtime unavailable: no engine");
}

#[test]
fn with_no_commands_root_is_empty_and_activation_does_nothing() {
    let launcher = launcher(vec![]);

    block_on(launcher.activate_selected());

    let view = launcher.view();
    assert_eq!(
        (view.rows.len(), view.selected, view.status),
        (0, None, Status::Idle)
    );
}

#[test]
fn going_back_while_an_action_runs_discards_its_answer() {
    let launcher = launcher(vec![command("sample", guest("sample_rust"))]);
    block_on(launcher.activate_selected());
    launcher.move_selection(1);

    let pending = launcher.activate_selected();
    launcher.back();
    block_on(pending);

    let view = launcher.view();
    assert_eq!((view.screen, view.status), (Screen::Root, Status::Idle));
}

/// A launcher with the Rust sample's form ("Greet someone") opened.
fn sample_form() -> Launcher {
    let launcher = launcher(vec![command("sample", guest("sample_rust"))]);
    block_on(launcher.activate_selected());
    launcher.select(4);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().screen, Screen::Form);
    launcher
}

fn field_errors(launcher: &Launcher) -> Vec<Option<String>> {
    let form = launcher.view().form.expect("a form");
    form.fields.into_iter().map(|field| field.error).collect()
}

#[test]
fn back_from_a_form_returns_to_the_command_with_its_item_selected() {
    let launcher = sample_form();

    launcher.back();

    let view = launcher.view();
    assert_eq!(
        (view.screen, view.title.as_str(), view.selected, view.form),
        (Screen::Command, "Rust sample", Some(4), None)
    );
    launcher.back();
    assert_eq!(launcher.view().screen, Screen::Root);
}

#[test]
fn editing_an_invalid_field_clears_its_error() {
    let launcher = sample_form();
    block_on(launcher.submit_form());
    assert_eq!(field_errors(&launcher), [Some("Enter a name".into()), None]);

    launcher.set_field_value("name", "A");

    assert_eq!(field_errors(&launcher), [None, None]);
}

#[test]
fn a_choice_the_form_does_not_offer_is_ignored() {
    let launcher = sample_form();

    launcher.set_field_value("greeting", "howdy");
    launcher.set_field_value("missing", "value");

    let values: Vec<String> = launcher
        .view()
        .form
        .unwrap()
        .fields
        .into_iter()
        .map(|field| field.value)
        .collect();
    assert_eq!(values, ["", "hello"]);
}

#[test]
fn a_form_level_error_is_shown_without_marking_a_field() {
    let launcher = launcher(vec![command("faulty", guest("faulty"))]);
    open_faulty_item(&launcher, "form");
    block_on(launcher.activate_selected());

    block_on(launcher.submit_form());

    assert_eq!(error(&launcher), "the guest refused the form");
    assert_eq!(field_errors(&launcher), [None]);
}

#[test]
fn going_back_while_a_form_is_submitted_discards_its_answer() {
    let launcher = sample_form();
    launcher.set_field_value("name", "Ada");

    let pending = launcher.submit_form();
    assert_eq!(launcher.view().status, Status::Running);
    launcher.back();
    block_on(pending);

    let view = launcher.view();
    assert_eq!((view.screen, view.status), (Screen::Command, Status::Idle));
}

#[test]
fn submitting_again_while_a_submission_is_pending_is_ignored() {
    let launcher = sample_form();
    let first = launcher.submit_form();
    launcher.set_field_value("name", "Ada");

    let second = launcher.submit_form();
    block_on(second);
    assert_eq!(launcher.view().status, Status::Running);
    block_on(first);

    // Only the empty submission ran. Its rejection reports the name, but
    // does not mark the field, which the user has edited since.
    assert_eq!(error(&launcher), "Name: Enter a name");
    assert_eq!(field_errors(&launcher), [None, None]);
    block_on(launcher.submit_form());
    assert_eq!(
        launcher.view().status,
        Status::Result("Hello, Ada, from the Rust guest".into())
    );
}

#[test]
fn submitting_outside_a_form_does_nothing() {
    let launcher = launcher(vec![command("sample", guest("sample_rust"))]);
    block_on(launcher.activate_selected());

    block_on(launcher.submit_form());

    assert_eq!(launcher.view().status, Status::Idle);
}

#[test]
fn selecting_a_row_directly_ignores_indexes_past_the_list() {
    let launcher = launcher(vec![command("sample", guest("sample_rust"))]);
    block_on(launcher.activate_selected());

    launcher.select(1);
    assert_eq!(launcher.view().selected, Some(1));
    launcher.select(7);
    assert_eq!(launcher.view().selected, Some(1));
}

#[test]
fn an_unavailable_form_explains_itself_instead_of_opening() {
    let launcher = launcher(vec![command("faulty", guest("faulty"))]);
    // The fixture's "nowhere" item has a form and declares no operating
    // system at all, so it is unavailable wherever the test runs.
    open_faulty_item(&launcher, "nowhere");
    let view = launcher.view();
    let reason = view.rows[view.selected.unwrap()].unavailable.clone();
    let reason = reason.expect("the row says why it is unavailable");
    assert_eq!(reason, platforms::nowhere("this action"));

    block_on(launcher.activate_selected());

    let view = launcher.view();
    assert_eq!(
        (view.screen, view.form, view.status),
        (Screen::Command, None, Status::Error(reason))
    );
    launcher.select(0);
    block_on(launcher.activate_selected());
    assert_eq!(launcher.view().status, Status::Result("fine".into()));
}

/// The pre-release extension API 0.1 changes shape between slices without a
/// version bump: `item` gained `platforms` in #19. A component built against
/// the older shape declares the same API version, and the type check at
/// instantiation refuses it when its command opens, naming the mismatch.
#[test]
fn a_component_of_an_older_api_shape_is_refused_when_it_loads() {
    let launcher = launcher(vec![command("old", guest("old_api"))]);

    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().screen, Screen::Root);
    let message = error(&launcher);
    assert!(
        message.starts_with("Could not load the extension: "),
        "{message}"
    );
    assert!(
        message.contains("type mismatch for field items: expected record of 5 fields"),
        "{message}"
    );
}
