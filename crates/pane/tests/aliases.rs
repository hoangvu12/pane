//! Giving a command an alias and making it a fallback through the native
//! window, on GPUI's test platform, then reaching it from root search with
//! real key events: the alias and some text, and a fallback the user moves
//! to. The command is Echo, the query sample from `cargo xtask guests`,
//! which answers the text it is sent.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext, prelude::*};
use pane::LauncherWindow;
use pane_core::{Launcher, LauncherView, Runtime, Screen, Status};

/// Copies the assembled query sample to `folder`.
fn package(folder: &Path) -> PathBuf {
    let assembled =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/guests/packages/sample-query");
    assert!(
        assembled.exists(),
        "{} is missing; run `cargo xtask guests`",
        assembled.display()
    );
    fs::create_dir_all(folder).unwrap();
    for entry in fs::read_dir(&assembled).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), folder.join(entry.file_name())).unwrap();
    }
    folder.to_path_buf()
}

/// Lets the window apply replies that arrive from other threads.
fn settle(window: &Entity<LauncherWindow>, cx: &mut VisualTestContext) -> LauncherView {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        let view = cx.read_entity(window, |window, _| window.launcher().view());
        if view.status != Status::Running {
            return view;
        }
        assert!(Instant::now() < deadline, "the launcher did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn titles(view: &LauncherView) -> Vec<&str> {
    view.rows.iter().map(|row| row.title.as_str()).collect()
}

#[gpui::test]
fn an_alias_and_a_fallback_set_in_the_window_send_the_typed_text_to_the_command(
    cx: &mut TestAppContext,
) {
    let (sources, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let folder = package(&sources.path().join("query"));
    cx.executor().allow_parking();
    cx.update(pane::bind_keys);
    let launcher =
        Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"));
    let (window, cx) = cx.add_window_view(|window, cx| {
        let mut launcher = LauncherWindow::new(launcher, window, cx);
        launcher.preview_package(&folder, window, cx);
        launcher
    });
    settle(&window, cx);
    cx.simulate_keystrokes("enter");
    settle(&window, cx);

    // Root lists Echo, the install row, then Manage extensions…; the
    // extension list holds the package's state, reload, cache and uninstall
    // rows, Echo's hotkey, then its alias and fallback.
    cx.simulate_keystrokes("down down enter");
    let view = settle(&window, cx);
    assert_eq!(
        titles(&view)[4..],
        ["Hotkey for Echo", "Alias for Echo", "Fallback: Echo"]
    );
    cx.simulate_keystrokes("down down down down down enter");
    let view = settle(&window, cx);
    assert_eq!(view.title, "Alias for Echo");
    assert!(matches!(view.screen, Screen::Form(_)));
    cx.simulate_input("ec");
    cx.simulate_keystrokes("enter");
    let view = settle(&window, cx);
    assert_eq!(
        view.status,
        Status::Result("Typing “ec” now finds Echo".into())
    );
    assert_eq!(view.selected, Some(5));

    // Its form starts with the alias.
    cx.simulate_keystrokes("enter");
    settle(&window, cx);
    let field = cx.read_entity(&window, |window, _| window.text_field("alias").unwrap());
    let text = cx.read_entity(&field, |field, _| field.as_str().to_owned());
    assert_eq!(text, "ec");
    cx.simulate_keystrokes("escape");
    settle(&window, cx);

    cx.simulate_keystrokes("down enter");
    let view = settle(&window, cx);
    assert_eq!(
        view.status,
        Status::Result("Echo is now offered for any text typed in root search".into())
    );

    // The alias and text: the row that sends it is selected; Enter sends it.
    cx.simulate_keystrokes("escape");
    settle(&window, cx);
    cx.simulate_input("ec hello");
    let view = settle(&window, cx);
    assert_eq!(view.selected, Some(0));
    assert_eq!(titles(&view)[0], "Echo");
    assert_eq!(
        view.rows[0].subtitle.as_deref(),
        Some("Send “hello” · alias ec")
    );
    cx.simulate_keystrokes("enter");
    let view = settle(&window, cx);
    assert_eq!(view.status, Status::Result("Echo heard “hello”".into()));
    assert!(cx.debug_bounds("status-result").is_some());

    // Text nothing matches: "No results", then the fallback, which Enter
    // does not choose by itself; Down does.
    cx.simulate_keystrokes("escape");
    settle(&window, cx);
    cx.simulate_input("zqx");
    let view = settle(&window, cx);
    assert_eq!(titles(&view), ["Echo"]);
    assert_eq!(view.selected, None);
    assert!(cx.debug_bounds("no-results").is_some());
    let before = view.status.clone();
    cx.simulate_keystrokes("enter");
    let view = settle(&window, cx);
    assert_eq!(view.status, before);
    cx.simulate_keystrokes("down enter");
    let view = settle(&window, cx);
    assert_eq!(view.status, Status::Result("Echo heard “zqx”".into()));
    assert_eq!(view.query(), Some("zqx"));
}
