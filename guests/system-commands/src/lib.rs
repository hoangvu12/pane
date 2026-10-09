//! Pane's System Commands, a default extension of no-view commands (ADR
//! 0037, ADR 0040): one for each session and power command the
//! `system-commands` host functions offer — Lock Screen, Log Out,
//! Restart, Shut Down, Sleep, Hibernate, Turn Off Displays and Start
//! Screen Saver — all eight sharing this component. A command's hotkey
//! runs it without showing Pane's window, and it answers with a HUD of
//! the state the system ended in, or why nothing changed (1.2 seconds, or
//! 3 for the failure style, ADR 0035): "Restarting", "Sleeping",
//! "Hibernation is not available on this computer: there is no hibernation
//! file".
//!
//! Log Out, Restart and Shut Down ask first, with a destructive
//! confirmation offering "Don't ask again" (ADR 0040): a mistyped query
//! never ends a session. Restart and Shut Down then force applications
//! closed, so an application with unsaved work cannot hold one up, while
//! Log Out leaves applications the chance to save — the host's decision,
//! not this extension's; each command only calls the host function named
//! for it. A background launch (a schedule) is answered with an error,
//! since no confirmation is available there: nothing happens.
//!
//! The extension declares `windows` alone in its pane.json, and only the
//! Windows default set lists it (ADR 0040).

#![no_std]

use pane_extension::alloc::{format, string::String};
use pane_extension::feedback::{self, Confirmation, ToastStyle};
use pane_extension::system_commands::{self, Outcome};
use pane_extension::{Command, LaunchRecord, NoCustomView};

struct SystemCommands;
pane_extension::export!(SystemCommands);

/// The commands' ids in `pane.json`, which `run` dispatches by.
const LOCK_SCREEN: &str = "lock-screen";
const LOG_OUT: &str = "log-out";
const RESTART: &str = "restart";
const SHUT_DOWN: &str = "shut-down";
const SLEEP: &str = "sleep";
const HIBERNATE: &str = "hibernate";
const DISPLAYS: &str = "turn-off-displays";
const SCREEN_SAVER: &str = "start-screen-saver";

/// The destructive set, which asks first (ADR 0040): logging out,
/// restarting and shutting down. The host's `Command` holds the same
/// decision; this list follows it.
const DESTRUCTIVE: [&str; 3] = [LOG_OUT, RESTART, SHUT_DOWN];

/// One command's answer: the HUD of the state it ended in, or why nothing
/// changed (3 seconds for the failure style).
fn answer(outcome: Outcome) {
    match outcome {
        Outcome::Done(text) => feedback::show_hud(&text, ToastStyle::Success),
        Outcome::Explained(text) => feedback::show_hud(&text, ToastStyle::Failure),
    }
}

impl Command for SystemCommands {
    type CustomView = NoCustomView;

    async fn run(command: String, _launch: LaunchRecord) -> Result<(), String> {
        // The destructive ones ask first, offering "Don't ask again"; a
        // background launch is answered that no confirmation is
        // available there, so nothing happens.
        if DESTRUCTIVE.contains(&command.as_str()) && !confirm(&command).await? {
            return Ok(());
        }
        answer(match command.as_str() {
            LOCK_SCREEN => system_commands::lock_screen(),
            LOG_OUT => system_commands::log_out(),
            RESTART => system_commands::restart(),
            SHUT_DOWN => system_commands::shut_down(),
            SLEEP => system_commands::sleep(),
            HIBERNATE => system_commands::hibernate(),
            DISPLAYS => system_commands::turn_off_displays(),
            SCREEN_SAVER => system_commands::start_screen_saver(),
            other => return Err(format!("`{other}` is no system command")),
        });
        Ok(())
    }
}

/// Asks the user to confirm the destructive command `id`, offering "Don't
/// ask again" remembered under the command's id: once the user ticks it
/// and confirms, Pane answers `true` at once from then on.
async fn confirm(id: &str) -> Result<bool, String> {
    let (title, message, primary) = match id {
        LOG_OUT => (
            "Log out?",
            "Your session ends; applications that need saving are asked to close.",
            "Log Out",
        ),
        RESTART => (
            "Restart?",
            "Windows restarts, closing every application whether saved or not.",
            "Restart",
        ),
        _ => (
            "Shut down?",
            "Windows powers off, closing every application whether saved or not.",
            "Shut Down",
        ),
    };
    let asked = Confirmation::new(title)
        .message(message)
        .primary(primary)
        .destructive()
        .remember(id);
    feedback::confirm(asked).await
}
