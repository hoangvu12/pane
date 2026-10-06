//! Setting up the windows the way the binary does: the host settings
//! initialized from a record, and the keystroke that opens Settings.
//! Shared by the test binaries of `pane`.
#![allow(dead_code)]

use std::path::Path;

use gpui::TestAppContext;

/// The keystroke that opens Settings on this platform.
pub fn settings_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-,"
    } else {
        "ctrl-,"
    }
}

/// Initializes the settings record of `data` in `cx`, as the binary does
/// before its first window opens.
pub fn init_settings(data: Option<&Path>, cx: &mut TestAppContext) {
    cx.update(|cx| {
        pane::settings::init_with_overrides(
            data.map(|data| data.to_owned()),
            pane::settings::Overrides::default(),
            cx,
        )
    });
}
