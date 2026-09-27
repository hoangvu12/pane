//! Drives the native launcher window through GPUI's test platform: real key
//! and mouse events dispatch to the window, which runs real guest components.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, prelude::*};
use pane::LauncherWindow;
use pane_core::{CommandRegistration, Launcher, Runtime, Screen, Status};

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

fn open(cx: &mut TestAppContext) -> (Entity<LauncherWindow>, &mut VisualTestContext) {
    open_with(cx, vec![command("Rust sample", "sample_rust")])
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
    let deadline = Instant::now() + Duration::from_secs(10);
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

#[gpui::test]
fn the_keyboard_opens_the_sample_and_runs_an_action(cx: &mut TestAppContext) {
    let (window, cx) = open(cx);

    cx.simulate_keystrokes("enter");
    let view = wait_for_answer(&window, cx);
    assert_eq!(view.screen, Screen::Command);
    assert!(
        cx.debug_bounds("row-Say hello").is_some(),
        "guest rows are rendered"
    );

    cx.simulate_keystrokes("enter");
    let view = wait_for_answer(&window, cx);
    assert_eq!(
        view.status,
        Status::Result("Hello from the Rust guest".into())
    );
    assert!(
        cx.debug_bounds("status-result").is_some(),
        "the answer is rendered"
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(wait_for_answer(&window, cx).screen, Screen::Root);
}

#[gpui::test]
fn clicking_a_row_runs_its_action(cx: &mut TestAppContext) {
    let (window, cx) = open(cx);
    cx.simulate_keystrokes("enter");
    wait_for_answer(&window, cx);

    let row = cx.debug_bounds("row-Wait briefly").expect("row rendered");
    cx.simulate_click(row.center(), Modifiers::none());

    let view = wait_for_answer(&window, cx);
    assert_eq!(view.selected, Some(1));
    assert_eq!(
        view.status,
        Status::Result("Waited 50 ms inside the Rust guest".into())
    );
}

#[gpui::test]
fn arrow_keys_move_the_selection(cx: &mut TestAppContext) {
    let (window, cx) = open(cx);
    cx.simulate_keystrokes("enter");
    wait_for_answer(&window, cx);

    cx.simulate_keystrokes("down");
    assert_eq!(wait_for_answer(&window, cx).selected, Some(1));
    cx.simulate_keystrokes("up");
    assert_eq!(wait_for_answer(&window, cx).selected, Some(0));
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
