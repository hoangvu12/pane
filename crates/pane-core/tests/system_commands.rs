//! The `system-commands` host functions (#255, the Windows power
//! features' System Commands, ADR 0040): locking the screen, logging out,
//! restarting, shutting down, sleeping, hibernating, turning the displays
//! off and starting the screen saver, and the audio commands (#265): the
//! volume of the default output device (Volume Up, Volume Down, Toggle
//! Mute, Set Volume) and Toggle Microphone Mute. The decisions are pure
//! functions, tested here on every system; the commands themselves are
//! exercised through the launcher's public interface with a recording
//! fake of the system (`support/system_commands.rs`), so no test ever
//! locks, logs out, restarts, shuts down, sleeps, hibernates, turns off
//! the displays of a real session, changes the volume or mutes a
//! microphone: the samples in Rust, JavaScript and TypeScript call each
//! host function and say what it answered, and the real System Commands
//! default extension — the package `cargo xtask guests` assembles in
//! `target/guests/packages/system-commands`, acquired as a Windows
//! default extension from an artifact source on 127.0.0.1
//! (`support/artifacts.rs`) — answers with a HUD of the state it ended
//! in, confirms the destructive ones first (with "Don't ask again"
//! remembered), takes Set Volume's level through its argument form, runs
//! from a hotkey without showing the window, and is acquired enabled and
//! disableable on its own. The extension's package declares `windows`
//! alone, so those tests run on Windows only; on other systems acquiring
//! it is refused with the platform's explanation.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::executor::block_on;
use pane_core::system_commands::{
    self, Capabilities, Command, HibernatePlan, Microphone, MicrophonePlan, NO_MICROPHONE, Outcome,
    PowerRequest, SET_VOLUME_RANGE, SleepPlan, Volume, hibernate_plan, microphone_plan, run,
    sleep_plan, toggled_mute, volume_down, volume_up,
};
use pane_core::{Launcher, Runtime, Screen, Status};
use tempfile::TempDir;

#[path = "support/artifacts.rs"]
mod artifacts;
#[path = "support/feedback.rs"]
mod feedback;
#[path = "support/guests.rs"]
mod guests;
#[path = "support/system_commands.rs"]
mod recording;
#[path = "support/rows.rs"]
mod rows;

use feedback::shown;
use guests::guests;
use recording::{Done, RecordingSystemCommands};
use rows::select_title;

/// Every command, as the extension's `pane.json` lists them.
const COMMANDS: [Command; 13] = [
    Command::LockScreen,
    Command::LogOut,
    Command::Restart,
    Command::ShutDown,
    Command::Sleep,
    Command::Hibernate,
    Command::TurnOffDisplays,
    Command::StartScreenSaver,
    Command::VolumeUp,
    Command::VolumeDown,
    Command::ToggleMute,
    Command::SetVolume(40),
    Command::ToggleMicrophoneMute,
];

/// `capabilities` as a computer with `modern_standby` and
/// `hibernation_file` set as asked.
fn capabilities(modern_standby: bool, hibernation_file: bool) -> Capabilities {
    Capabilities {
        modern_standby,
        hibernation_file,
    }
}

/// The volume of the fake at `level`, unmuted (or muted as `muted` says).
fn volume(level: u8, muted: bool) -> Volume {
    Volume { level, muted }
}

/// A microphone called `id`, muted as `muted` says.
fn microphone(id: &str, muted: bool) -> Microphone {
    Microphone {
        id: id.into(),
        muted,
    }
}

#[test]
fn the_destructive_set_is_logging_out_restarting_and_shutting_down() {
    for command in COMMANDS {
        assert_eq!(
            command.destructive(),
            matches!(
                command,
                Command::LogOut | Command::Restart | Command::ShutDown
            ),
            "{command:?}"
        );
    }
}

#[test]
fn restart_and_shut_down_force_applications_closed_and_log_out_does_not() {
    assert!(Command::Restart.forces_applications_closed());
    assert!(Command::ShutDown.forces_applications_closed());
    assert!(!Command::LogOut.forces_applications_closed());
    for command in [
        Command::LockScreen,
        Command::Sleep,
        Command::Hibernate,
        Command::TurnOffDisplays,
        Command::StartScreenSaver,
        Command::VolumeUp,
        Command::VolumeDown,
        Command::ToggleMute,
        Command::SetVolume(40),
        Command::ToggleMicrophoneMute,
    ] {
        assert!(!command.forces_applications_closed(), "{command:?}");
    }
}

#[test]
fn sleep_turns_the_displays_off_only_on_a_modern_standby_computer() {
    assert_eq!(
        sleep_plan(&capabilities(true, true)),
        SleepPlan::DisplaysOff
    );
    assert_eq!(sleep_plan(&capabilities(false, true)), SleepPlan::Suspend);
}

#[test]
fn hibernate_needs_a_hibernation_file() {
    assert_eq!(
        hibernate_plan(&capabilities(false, true)),
        HibernatePlan::Hibernate
    );
    assert_eq!(
        hibernate_plan(&capabilities(false, false)),
        HibernatePlan::Explain
    );
}

#[test]
fn volume_steps_follow_windows_own_volume_keys_and_neither_pass_the_ends() {
    assert_eq!(volume_up(volume(50, false)), volume(52, false));
    assert_eq!(volume_up(volume(99, true)), volume(100, true));
    assert_eq!(volume_up(volume(100, false)), volume(100, false));
    assert_eq!(volume_down(volume(50, false)), volume(48, false));
    assert_eq!(volume_down(volume(1, true)), volume(0, true));
    assert_eq!(volume_down(volume(0, false)), volume(0, false));
    assert_eq!(toggled_mute(volume(50, false)), volume(50, true));
    assert_eq!(toggled_mute(volume(50, true)), volume(50, false));
}

#[test]
fn the_microphone_toggle_mutes_all_when_any_is_on_and_explains_none() {
    assert_eq!(
        microphone_plan(&[microphone("a", false), microphone("b", true)]),
        MicrophonePlan::Mute
    );
    assert_eq!(
        microphone_plan(&[microphone("a", true), microphone("b", true)]),
        MicrophonePlan::Unmute
    );
    assert_eq!(microphone_plan(&[]), MicrophonePlan::Explain);
}

#[test]
fn each_command_says_what_it_ended_in_and_the_adapter_what_it_was_asked() {
    let commands = RecordingSystemCommands::default();
    let cases: [(Command, &str, Done); 8] = [
        (Command::LockScreen, "Locking the screen", Done::Locked),
        (
            Command::LogOut,
            "Logging out",
            Done::Power(PowerRequest::LogOut, false),
        ),
        (
            Command::Restart,
            "Restarting",
            Done::Power(PowerRequest::Restart, true),
        ),
        (
            Command::ShutDown,
            "Shutting down",
            Done::Power(PowerRequest::ShutDown, true),
        ),
        (Command::Sleep, "Sleeping", Done::Suspended(false)),
        (Command::Hibernate, "Hibernating", Done::Suspended(true)),
        (
            Command::TurnOffDisplays,
            "Turning off the displays",
            Done::DisplaysOff,
        ),
        (
            Command::StartScreenSaver,
            "Starting the screen saver",
            Done::ScreenSaver,
        ),
    ];
    for (command, text, done) in cases {
        assert_eq!(
            run(command, &commands),
            Outcome::Done(text.into()),
            "{command:?}"
        );
        assert_eq!(commands.take(), [done], "{command:?}");
    }
}

#[test]
fn each_volume_command_answers_the_volume_it_ended_at() {
    let commands = RecordingSystemCommands::default();
    // The fake starts at volume 50, unmuted.
    assert_eq!(
        run(Command::VolumeUp, &commands),
        Outcome::Done("Volume 52%".into())
    );
    assert_eq!(commands.take(), [Done::SetVolume(volume(52, false))]);
    // Volume Down steps back down from where Volume Up left the volume.
    assert_eq!(
        run(Command::VolumeDown, &commands),
        Outcome::Done("Volume 50%".into())
    );
    assert_eq!(commands.take(), [Done::SetVolume(volume(50, false))]);
    assert_eq!(
        run(Command::SetVolume(40), &commands),
        Outcome::Done("Volume 40%".into())
    );
    assert_eq!(commands.take(), [Done::SetVolume(volume(40, false))]);
    // Toggle Mute says which it did, with the volume when it unmuted.
    assert_eq!(
        run(Command::ToggleMute, &commands),
        Outcome::Done("Muted".into())
    );
    assert_eq!(commands.take(), [Done::SetVolume(volume(40, true))]);
    assert_eq!(
        run(Command::ToggleMute, &commands),
        Outcome::Done("Unmuted, Volume 40%".into())
    );
    assert_eq!(commands.take(), [Done::SetVolume(volume(40, false))]);
}

#[test]
fn a_level_not_from_0_to_100_changes_nothing_and_is_explained() {
    let commands = RecordingSystemCommands::default();
    assert_eq!(
        run(Command::SetVolume(101), &commands),
        Outcome::Explained(SET_VOLUME_RANGE.into())
    );
    assert!(commands.take().is_empty(), "nothing was asked");
}

#[test]
fn the_microphone_toggle_mutes_or_unmutes_every_microphone_and_says_which() {
    let commands = RecordingSystemCommands::default();
    commands.set_microphones(vec![microphone("a", false), microphone("b", true)]);
    // One microphone unmuted: every microphone is muted.
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Done("Microphones muted".into())
    );
    assert_eq!(
        commands.take(),
        [
            Done::SetMicrophoneMute("a".into(), true),
            Done::SetMicrophoneMute("b".into(), true)
        ]
    );
    // None unmuted any more: every microphone is unmuted.
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Done("Microphones unmuted".into())
    );
    assert_eq!(
        commands.take(),
        [
            Done::SetMicrophoneMute("a".into(), false),
            Done::SetMicrophoneMute("b".into(), false)
        ]
    );
}

#[test]
fn a_microphone_that_vanishes_mid_toggle_is_skipped() {
    let commands = RecordingSystemCommands::default();
    commands.set_microphones(vec![microphone("a", false), microphone("b", false)]);
    commands.vanish("b");
    // The vanished microphone is skipped: the rest are muted.
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Done("Microphones muted".into())
    );
    assert_eq!(commands.take(), [Done::SetMicrophoneMute("a".into(), true)]);
    // Every microphone vanishing: nothing changed, and the answer says why.
    commands.vanish("a");
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Explained("The microphone is gone".into())
    );
}

#[test]
fn no_microphone_at_all_is_explained() {
    let commands = RecordingSystemCommands::default();
    commands.set_microphones(vec![]);
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Explained(NO_MICROPHONE.into())
    );
    assert!(commands.take().is_empty(), "nothing was asked");
}

#[test]
fn sleep_follows_the_computers_modern_standby_signal_and_hibernate_its_file() {
    let commands = RecordingSystemCommands::default();
    commands.set_capabilities(capabilities(true, false));
    assert_eq!(
        run(Command::Sleep, &commands),
        Outcome::Done("Sleeping".into())
    );
    assert_eq!(commands.take(), [Done::DisplaysOff]);
    // Without a hibernation file, hibernate changes nothing.
    assert_eq!(
        run(Command::Hibernate, &commands),
        Outcome::Explained(system_commands::NO_HIBERNATION_FILE.into())
    );
    assert!(commands.take().is_empty(), "nothing was asked");
}

#[test]
fn a_failing_system_is_explained_and_nothing_happens() {
    let commands = RecordingSystemCommands::default();
    commands.fail("Windows did not lock the screen");
    assert_eq!(
        run(Command::LockScreen, &commands),
        Outcome::Explained("Windows did not lock the screen".into())
    );
    // Sleep and hibernate follow the capabilities' own failure, and the
    // volume and microphone commands the volume's and the list's.
    assert_eq!(
        run(Command::Sleep, &commands),
        Outcome::Explained("Windows did not lock the screen".into())
    );
    assert_eq!(
        run(Command::Hibernate, &commands),
        Outcome::Explained("Windows did not lock the screen".into())
    );
    assert_eq!(
        run(Command::VolumeUp, &commands),
        Outcome::Explained("Windows did not lock the screen".into())
    );
    assert_eq!(
        run(Command::ToggleMicrophoneMute, &commands),
        Outcome::Explained("Windows did not lock the screen".into())
    );
    assert!(commands.take().is_empty(), "nothing was asked");
}

#[test]
fn a_launcher_given_no_commands_explains_every_one() {
    for command in COMMANDS {
        assert_eq!(
            run(command, system_commands::none().as_ref()),
            Outcome::Explained(
                "Not available: this Pane reaches no session and power commands".into()
            ),
            "{command:?}"
        );
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn other_systems_explain_that_the_commands_are_windows_only() {
    for command in COMMANDS {
        match run(command, system_commands::native().as_ref()) {
            Outcome::Explained(why) => {
                assert!(why.starts_with("Not available on "), "{why}");
                assert!(why.contains("Windows-only for now"), "{why}");
            }
            other => panic!("expected explained, got {other:?}"),
        }
    }
}

/// The Windows adapter's one reversible read: what the computer can do.
/// Nothing here locks, ends a session, sleeps or touches the displays.
#[cfg(target_os = "windows")]
#[test]
fn windows_says_what_this_computer_can_do() {
    assert!(
        system_commands::native().capabilities().is_ok(),
        "GetPwrCapabilities answers"
    );
}

/// The Windows adapter's reversible audio reads: the volume of the
/// default output device and the microphones there are. Nothing here
/// changes the volume or mutes anything; a machine with no audio device
/// is answered, never an error thrown.
#[cfg(target_os = "windows")]
#[test]
fn windows_says_the_volume_and_the_microphones() {
    if std::env::var("PANE_TEST_REAL_INPUT").as_deref() != Ok("1") {
        eprintln!("skipped: set PANE_TEST_REAL_INPUT=1 to let it read the audio devices");
        return;
    }
    let native = system_commands::native();
    match native.volume() {
        Ok(volume) => assert!(volume.level <= 100, "{:?}", volume),
        Err(why) => assert!(!why.is_empty()),
    }
    assert!(native.microphones().is_ok(), "EnumAudioEndpoints answers");
}

/// One language's system commands sample package.
struct Fixture {
    /// The assembled package under `target/guests/packages`.
    package: &'static str,
    /// Its command's title in root search.
    title: &'static str,
}

const RUST: Fixture = Fixture {
    package: "sample-system-commands",
    title: "System commands sample",
};
const JAVASCRIPT: Fixture = Fixture {
    package: "sample-system-commands-js",
    title: "JavaScript system commands sample",
};
const TYPESCRIPT: Fixture = Fixture {
    package: "sample-system-commands-ts",
    title: "TypeScript system commands sample",
};

/// Copies the assembled package `name` under `target/guests/packages` to
/// `folder`.
fn copy(name: &str, folder: &Path) -> PathBuf {
    let assembled = guests().join("packages").join(name);
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

/// One test's Pane: its folders, the fake of the system its commands
/// reach, and the launcher.
struct Pane {
    _sources: TempDir,
    _data: TempDir,
    commands: Arc<RecordingSystemCommands>,
    launcher: Launcher,
}

impl Pane {
    fn with(fixture: &Fixture) -> Pane {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let commands = Arc::new(RecordingSystemCommands::default());
        let launcher =
            Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"))
                .with_system_commands(commands.clone());
        let folder = copy(fixture.package, &sources.path().join(fixture.package));
        block_on(launcher.install_package(&folder));
        assert_eq!(
            launcher.view().status,
            Status::Result(format!("Installed {}", fixture.title))
        );
        Pane {
            _sources: sources,
            _data: data,
            commands,
            launcher,
        }
    }

    /// Root search, with `query` typed.
    fn search(&self, query: &str) {
        while !matches!(self.launcher.view().screen, Screen::Root { .. }) {
            self.launcher.back();
        }
        block_on(self.launcher.set_query(query));
    }

    /// Opens the sample's command from root search.
    fn open(&self, fixture: &Fixture) {
        self.search(fixture.title);
        select_title(&self.launcher, fixture.title);
        block_on(self.launcher.activate_selected());
        assert_eq!(self.launcher.view().screen, Screen::Command);
    }

    /// Runs the item titled `item` of the open command and says what it
    /// showed: its toast, or the status line.
    fn run(&self, item: &str) -> Status {
        select_title(&self.launcher, item);
        block_on(self.launcher.activate_selected());
        shown(&self.launcher)
    }
}

fn each_command_calls_the_capability_and_answers_its_state(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.open(fixture);
    let cases: [(&str, Status, Vec<Done>); 8] = [
        (
            "Lock Screen",
            Status::Result("Lock Screen: Locking the screen".into()),
            vec![Done::Locked],
        ),
        (
            "Log Out",
            Status::Result("Log Out: Logging out".into()),
            vec![Done::Power(PowerRequest::LogOut, false)],
        ),
        (
            "Restart",
            Status::Result("Restart: Restarting".into()),
            vec![Done::Power(PowerRequest::Restart, true)],
        ),
        (
            "Shut Down",
            Status::Result("Shut Down: Shutting down".into()),
            vec![Done::Power(PowerRequest::ShutDown, true)],
        ),
        (
            "Sleep",
            Status::Result("Sleep: Sleeping".into()),
            vec![Done::Suspended(false)],
        ),
        (
            "Hibernate",
            Status::Result("Hibernate: Hibernating".into()),
            vec![Done::Suspended(true)],
        ),
        (
            "Turn Off Displays",
            Status::Result("Turn Off Displays: Turning off the displays".into()),
            vec![Done::DisplaysOff],
        ),
        (
            "Start Screen Saver",
            Status::Result("Start Screen Saver: Starting the screen saver".into()),
            vec![Done::ScreenSaver],
        ),
    ];
    for (item, expected, done) in cases {
        assert_eq!(pane.run(item), expected, "{item}: {}", fixture.title);
        assert_eq!(pane.commands.take(), done, "{item}: {}", fixture.title);
    }
}

fn each_volume_item_answers_the_volume_it_ended_at(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.open(fixture);
    // The fake starts at volume 50, unmuted, which each case resets to.
    let cases: [(&str, Status, Done); 4] = [
        (
            "Volume Up",
            Status::Result("Volume Up: Volume 52%".into()),
            Done::SetVolume(volume(52, false)),
        ),
        (
            "Volume Down",
            Status::Result("Volume Down: Volume 48%".into()),
            Done::SetVolume(volume(48, false)),
        ),
        (
            "Toggle Mute",
            Status::Result("Toggle Mute: Muted".into()),
            Done::SetVolume(volume(50, true)),
        ),
        (
            "Set Volume",
            Status::Result("Set Volume: Volume 40%".into()),
            Done::SetVolume(volume(40, false)),
        ),
    ];
    for (item, expected, done) in cases {
        pane.commands.set_volume_state(volume(50, false));
        assert_eq!(pane.run(item), expected, "{item}: {}", fixture.title);
        assert_eq!(pane.commands.take(), [done], "{item}: {}", fixture.title);
    }
    // Muting once, the next unmute says the volume it ended at.
    assert_eq!(
        pane.run("Toggle Mute"),
        Status::Result("Toggle Mute: Muted".into()),
        "{}",
        fixture.title
    );
    assert_eq!(
        pane.run("Toggle Mute"),
        Status::Result("Toggle Mute: Unmuted, Volume 40%".into()),
        "{}",
        fixture.title
    );
}

fn the_microphone_toggle_mutes_all_unmutes_all_and_explains_none(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.open(fixture);
    // Two microphones, one muted: every microphone is muted.
    pane.commands
        .set_microphones(vec![microphone("a", false), microphone("b", true)]);
    assert_eq!(
        pane.run("Toggle Microphone Mute"),
        Status::Result("Toggle Microphone Mute: Microphones muted".into()),
        "{}",
        fixture.title
    );
    assert_eq!(
        pane.commands.take(),
        [
            Done::SetMicrophoneMute("a".into(), true),
            Done::SetMicrophoneMute("b".into(), true)
        ],
        "{}",
        fixture.title
    );
    // None unmuted any more: every microphone is unmuted.
    assert_eq!(
        pane.run("Toggle Microphone Mute"),
        Status::Result("Toggle Microphone Mute: Microphones unmuted".into()),
        "{}",
        fixture.title
    );
    assert_eq!(
        pane.commands.take(),
        [
            Done::SetMicrophoneMute("a".into(), false),
            Done::SetMicrophoneMute("b".into(), false)
        ],
        "{}",
        fixture.title
    );
    // No microphone at all: nothing happens and the answer says so.
    pane.commands.set_microphones(vec![]);
    assert_eq!(
        pane.run("Toggle Microphone Mute"),
        Status::Error(format!("Toggle Microphone Mute: {}", NO_MICROPHONE)),
        "{}",
        fixture.title
    );
    assert!(pane.commands.take().is_empty(), "nothing was asked");
}

fn sleep_follows_the_computers_modern_standby_signal(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.commands.set_capabilities(capabilities(true, true));
    pane.open(fixture);
    assert_eq!(
        pane.run("Sleep"),
        Status::Result("Sleep: Sleeping".into()),
        "{}",
        fixture.title
    );
    assert_eq!(
        pane.commands.take(),
        [Done::DisplaysOff],
        "{}",
        fixture.title
    );
}

fn hibernate_without_a_hibernation_file_is_explained(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.commands.set_capabilities(capabilities(false, false));
    pane.open(fixture);
    assert_eq!(
        pane.run("Hibernate"),
        Status::Error(format!(
            "Hibernate: {}",
            system_commands::NO_HIBERNATION_FILE
        )),
        "{}",
        fixture.title
    );
    assert!(pane.commands.take().is_empty(), "nothing was asked");
}

fn a_failing_system_answers_why_nothing_changed(fixture: &Fixture) {
    let pane = Pane::with(fixture);
    pane.commands.fail("Windows did not lock the screen");
    pane.open(fixture);
    assert_eq!(
        pane.run("Lock Screen"),
        Status::Error("Lock Screen: Windows did not lock the screen".into()),
        "{}",
        fixture.title
    );
    assert!(pane.commands.take().is_empty(), "nothing was asked");
}

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
    each_command_calls_the_capability_and_answers_its_state,
    each_volume_item_answers_the_volume_it_ended_at,
    the_microphone_toggle_mutes_all_unmutes_all_and_explains_none,
    sleep_follows_the_computers_modern_standby_signal,
    hibernate_without_a_hibernation_file_is_explained,
    a_failing_system_answers_why_nothing_changed,
);

/// The real System Commands default extension, acquired as Windows does:
/// from an artifact source on 127.0.0.1, with the default extension's
/// identity. The package declares `windows` alone, so these tests run on
/// Windows only.
#[cfg(target_os = "windows")]
mod extension {
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use futures::executor::block_on;
    use pane_core::defaults::ArtifactSource;
    use pane_core::feedback::WindowRequest;
    use pane_core::hotkeys::{HotkeyError, Hotkeys, Shortcut};
    use pane_core::system_commands::{
        NO_HIBERNATION_FILE, NO_MICROPHONE, PowerRequest, SET_VOLUME_RANGE,
    };
    use pane_core::{
        ConfirmAnswer, Confirmation, DefaultExtension, Hud, Launcher, PackageIdentity, Runtime,
        Screen, Status, ToastStyle,
    };

    use super::artifacts::Artifacts;
    use super::feedback::RecordingWindow;
    use super::recording::{Done, RecordingSystemCommands};
    use super::rows::{manage, select_title, titles};
    use super::{capabilities, guests, microphone, volume};

    /// How long a launch or a guest call may take: compiling the guest
    /// once is included; a slow, busy machine is not.
    const PROMPTLY: Duration = Duration::from_secs(60);

    /// A system whose global hotkeys always register.
    #[derive(Default)]
    struct FakeHotkeys {
        registered: std::sync::Mutex<Vec<Shortcut>>,
    }

    impl Hotkeys for FakeHotkeys {
        fn unavailable(&self) -> Option<String> {
            None
        }

        fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError> {
            self.registered.lock().unwrap().push(shortcut.clone());
            Ok(())
        }

        fn unregister(&self, shortcut: &Shortcut) {
            self.registered
                .lock()
                .unwrap()
                .retain(|kept| kept != shortcut);
        }
    }

    /// One test's Pane: its data folder, the artifact source the default
    /// extension is acquired from, the fake of the system its commands
    /// reach, and the launcher.
    struct Pane {
        _data: tempfile::TempDir,
        _artifacts: Artifacts,
        commands: Arc<RecordingSystemCommands>,
        launcher: Launcher,
        window: Arc<RecordingWindow>,
        identity: PackageIdentity,
    }

    impl Pane {
        fn new() -> Pane {
            let data = tempfile::tempdir().unwrap();
            let artifacts = Artifacts::start();
            let folder = guests().join("packages").join("system-commands");
            assert!(
                folder.is_dir(),
                "{} is missing; run `cargo xtask guests`",
                folder.display()
            );
            let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&folder)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.is_file())
                .map(|path| {
                    let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                    (name, std::fs::read(&path).unwrap())
                })
                .collect();
            files.sort();
            let manifest: serde_json::Value = serde_json::from_slice(
                &files
                    .iter()
                    .find(|(path, _)| path == "pane.json")
                    .expect("the package has a pane.json")
                    .1,
            )
            .unwrap();
            let borrowed: Vec<(&str, Vec<u8>)> = files
                .iter()
                .map(|(path, contents)| (path.as_str(), contents.clone()))
                .collect();
            artifacts.publish(
                "system-commands",
                manifest["version"].as_str().unwrap(),
                &borrowed,
            );
            let commands = Arc::new(RecordingSystemCommands::default());
            let launcher =
                Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"))
                    .with_defaults(
                        ArtifactSource::local(artifacts.url()).unwrap(),
                        vec![DefaultExtension {
                            id: "system-commands".into(),
                            title: "System Commands".into(),
                        }],
                    )
                    .with_system_commands(commands.clone())
                    .with_hotkeys(Arc::new(FakeHotkeys::default()));
            let window = RecordingWindow::attach(&launcher);
            block_on(launcher.acquire_defaults());
            assert!(
                matches!(launcher.view().status, Status::Result(_)),
                "{:?}",
                launcher.view().status
            );
            let identity = PackageIdentity::default_extension("system-commands");
            Pane {
                _data: data,
                _artifacts: artifacts,
                commands,
                launcher,
                window,
                identity,
            }
        }

        /// Root search, with `query` typed.
        fn search(&self, query: &str) {
            while !matches!(self.launcher.view().screen, Screen::Root { .. }) {
                self.launcher.back();
            }
            block_on(self.launcher.set_query(query));
        }

        /// The HUDs the window was asked to show so far, forgotten once
        /// read (the hides that closed the launcher for them are not).
        fn huds(&self) -> Vec<Hud> {
            self.window
                .take()
                .into_iter()
                .filter_map(|request| match request {
                    WindowRequest::Hud(hud) => Some(hud),
                    WindowRequest::Hide | WindowRequest::Confirmation => None,
                })
                .collect()
        }

        /// Types `title` in root search and runs the row titled `title`;
        /// what the window was asked to show.
        fn run(&self, title: &str) -> Vec<Hud> {
            self.search(title);
            select_title(&self.launcher, title);
            block_on(self.launcher.activate_selected());
            assert!(
                matches!(self.launcher.view().screen, Screen::Root { .. }),
                "{} opened no screen",
                title
            );
            self.huds()
        }

        /// Types `title` in root search, opens the command titled `title`'s
        /// argument form and submits it with `values`; what the window was
        /// asked to show.
        fn submit(&self, title: &str, values: &[(&str, &str)]) -> Vec<Hud> {
            self.search(title);
            select_title(&self.launcher, title);
            block_on(self.launcher.activate_selected());
            for (field, value) in values {
                self.launcher.set_field_value(field, value);
            }
            block_on(self.launcher.submit_form());
            self.huds()
        }

        /// Starts the command titled `title` from root search, on a thread
        /// of its own: the destructive ones wait on the user.
        fn start(&self, title: &str) -> Running {
            self.search(title);
            select_title(&self.launcher, title);
            let running = self.launcher.activate_selected();
            Running {
                thread: thread::spawn(move || {
                    let _ = block_on(running);
                }),
            }
        }

        /// The id of the command `command` of the extension.
        fn id(&self, command: &str) -> String {
            format!("{}#{command}", self.identity.key())
        }
    }

    /// A call that may wait on a confirmation, running on a thread of its
    /// own.
    struct Running {
        thread: thread::JoinHandle<()>,
    }

    impl Running {
        /// Waits for the call to end.
        fn ended(self) {
            let started = Instant::now();
            while !self.thread.is_finished() {
                assert!(started.elapsed() < PROMPTLY, "the call did not end");
                thread::sleep(std::time::Duration::from_millis(5));
            }
            self.thread.join().unwrap();
        }
    }

    /// The confirmation the launcher shows, once the command asked for it.
    fn asked(launcher: &Launcher) -> Confirmation {
        let started = Instant::now();
        loop {
            if let Some(confirmation) = launcher.confirmation() {
                return confirmation;
            }
            assert!(
                started.elapsed() < PROMPTLY,
                "no confirmation was asked: {:?}",
                launcher.view()
            );
            thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// The HUD titled `title` in `style`.
    fn hud(title: &str, style: ToastStyle) -> Hud {
        Hud {
            title: title.into(),
            style,
        }
    }

    /// The command titled `title`'s manifest id, as the confirmation's
    /// remembered key names it.
    fn command_id(title: &str) -> &'static str {
        match title {
            "Log Out" => "log-out",
            "Restart" => "restart",
            _ => "shut-down",
        }
    }

    #[test]
    fn the_extension_is_acquired_enabled_and_disableable_on_its_own() {
        let pane = Pane::new();

        // Acquired with the default extension's identity, enabled, its
        // thirteen commands root results the user can give an alias or a
        // hotkey.
        let packages = pane.launcher.packages();
        let package = packages
            .iter()
            .find(|package| package.identity == pane.identity)
            .expect("System Commands is installed");
        assert!(package.enabled, "enabled by default");
        for title in [
            "Lock Screen",
            "Log Out",
            "Restart",
            "Shut Down",
            "Sleep",
            "Hibernate",
            "Turn Off Displays",
            "Start Screen Saver",
            "Volume Up",
            "Volume Down",
            "Toggle Mute",
            "Set Volume",
            "Toggle Microphone Mute",
        ] {
            pane.search(title);
            assert!(
                titles(&pane.launcher).contains(&title.to_owned()),
                "{title} is no root result: {:?}",
                titles(&pane.launcher)
            );
        }

        // Its page in Settings lists its commands, and the extension's
        // switch is its own.
        manage(&pane.launcher);
        assert!(
            titles(&pane.launcher).contains(&"Reload System Commands".to_owned()),
            "{:?}",
            titles(&pane.launcher)
        );
        assert!(titles(&pane.launcher).contains(&"Uninstall System Commands".to_owned()));
        for title in [
            "Hotkey for Lock Screen",
            "Hotkey for Sleep",
            "Hotkey for Set Volume",
        ] {
            assert!(
                titles(&pane.launcher).contains(&title.to_owned()),
                "{title} is not on the page: {:?}",
                titles(&pane.launcher)
            );
        }
        let subtitle = pane
            .launcher
            .view()
            .rows
            .first()
            .and_then(|row| row.subtitle.clone())
            .unwrap_or_default();
        assert!(subtitle.starts_with("Enabled"), "{subtitle}");

        // Disabled, its commands leave root search; enabled again, they
        // return.
        block_on(pane.launcher.set_enabled(&pane.identity, false));
        pane.search("Lock Screen");
        assert!(!titles(&pane.launcher).contains(&"Lock Screen".to_owned()));
        block_on(pane.launcher.set_enabled(&pane.identity, true));
        pane.search("Lock Screen");
        assert!(titles(&pane.launcher).contains(&"Lock Screen".to_owned()));
    }

    #[test]
    fn each_command_answers_the_hud_of_the_state_it_ended_in() {
        let pane = Pane::new();
        let cases: [(&str, &str, Done); 5] = [
            ("Lock Screen", "Locking the screen", Done::Locked),
            ("Sleep", "Sleeping", Done::Suspended(false)),
            ("Hibernate", "Hibernating", Done::Suspended(true)),
            (
                "Turn Off Displays",
                "Turning off the displays",
                Done::DisplaysOff,
            ),
            (
                "Start Screen Saver",
                "Starting the screen saver",
                Done::ScreenSaver,
            ),
        ];
        for (command, text, done) in cases {
            assert_eq!(
                pane.run(command),
                [hud(text, ToastStyle::Success)],
                "{command}"
            );
            assert_eq!(pane.commands.take(), [done], "{command}");
        }
    }

    #[test]
    fn the_volume_commands_answer_the_hud_of_the_volume_they_ended_at() {
        let pane = Pane::new();
        // The fake starts at volume 50, unmuted; Volume Down steps back
        // down from where Volume Up left the volume.
        assert_eq!(
            pane.run("Volume Up"),
            [hud("Volume 52%", ToastStyle::Success)]
        );
        assert_eq!(pane.commands.take(), [Done::SetVolume(volume(52, false))]);
        assert_eq!(
            pane.run("Volume Down"),
            [hud("Volume 50%", ToastStyle::Success)]
        );
        assert_eq!(pane.commands.take(), [Done::SetVolume(volume(50, false))]);
        // Muting says which it did; unmuting says the volume it ended at.
        assert_eq!(pane.run("Toggle Mute"), [hud("Muted", ToastStyle::Success)]);
        assert_eq!(pane.commands.take(), [Done::SetVolume(volume(50, true))]);
        assert_eq!(
            pane.run("Toggle Mute"),
            [hud("Unmuted, Volume 50%", ToastStyle::Success)]
        );
        assert_eq!(pane.commands.take(), [Done::SetVolume(volume(50, false))]);
    }

    #[test]
    fn set_volume_takes_its_level_through_the_argument_form() {
        let pane = Pane::new();
        // Enter on Set Volume asks for its level first, as the command's
        // one argument, required.
        pane.search("Set Volume");
        select_title(&pane.launcher, "Set Volume");
        block_on(pane.launcher.activate_selected());
        let view = pane.launcher.view();
        assert_eq!(view.title, "Set Volume");
        let form = view.form().expect("the argument form is shown");
        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.fields[0].id, "level");
        assert_eq!(form.fields[0].label, "Volume level (0 to 100)");
        assert!(form.fields[0].required);
        assert!(pane.launcher.back());

        // Anything but a number from 0 to 100 changes nothing and is
        // explained — "forty" by the command, 150 by the host, the same
        // sentence either way.
        for level in ["forty", "150"] {
            assert_eq!(
                pane.submit("Set Volume", &[("level", level)]),
                [hud(SET_VOLUME_RANGE, ToastStyle::Failure)],
                "{level}"
            );
            assert!(pane.commands.take().is_empty(), "{level} asked nothing");
        }

        // A level from 0 to 100 is the volume it ends at.
        assert_eq!(
            pane.submit("Set Volume", &[("level", "40")]),
            [hud("Volume 40%", ToastStyle::Success)]
        );
        assert_eq!(pane.commands.take(), [Done::SetVolume(volume(40, false))]);
    }

    #[test]
    fn the_microphone_toggle_mutes_all_unmutes_all_and_explains_none() {
        let pane = Pane::new();
        pane.commands
            .set_microphones(vec![microphone("a", false), microphone("b", true)]);
        // One microphone unmuted: every microphone is muted.
        assert_eq!(
            pane.run("Toggle Microphone Mute"),
            [hud("Microphones muted", ToastStyle::Success)]
        );
        assert_eq!(
            pane.commands.take(),
            [
                Done::SetMicrophoneMute("a".into(), true),
                Done::SetMicrophoneMute("b".into(), true)
            ]
        );
        // None unmuted any more: every microphone is unmuted.
        assert_eq!(
            pane.run("Toggle Microphone Mute"),
            [hud("Microphones unmuted", ToastStyle::Success)]
        );
        assert_eq!(
            pane.commands.take(),
            [
                Done::SetMicrophoneMute("a".into(), false),
                Done::SetMicrophoneMute("b".into(), false)
            ]
        );
        // No microphone at all: nothing happens and the answer says so.
        pane.commands.set_microphones(vec![]);
        assert_eq!(
            pane.run("Toggle Microphone Mute"),
            [hud(NO_MICROPHONE, ToastStyle::Failure)]
        );
        assert!(pane.commands.take().is_empty(), "nothing was asked");
    }

    #[test]
    fn sleep_on_modern_standby_turns_the_displays_off_and_a_missing_file_is_explained() {
        let pane = Pane::new();
        pane.commands.set_capabilities(capabilities(true, false));
        assert_eq!(pane.run("Sleep"), [hud("Sleeping", ToastStyle::Success)]);
        assert_eq!(pane.commands.take(), [Done::DisplaysOff]);
        assert_eq!(
            pane.run("Hibernate"),
            [hud(NO_HIBERNATION_FILE, ToastStyle::Failure)]
        );
        assert!(pane.commands.take().is_empty(), "nothing was asked");
    }

    #[test]
    fn the_destructive_ones_confirm_first_and_remember_the_answer() {
        let pane = Pane::new();
        let cases: [(&str, &str, &str, &str, Done); 3] = [
            (
                "Log Out",
                "Log out?",
                "Your session ends; applications that need saving are asked to close.",
                "Logging out",
                Done::Power(PowerRequest::LogOut, false),
            ),
            (
                "Restart",
                "Restart?",
                "Windows restarts, closing every application whether saved or not.",
                "Restarting",
                Done::Power(PowerRequest::Restart, true),
            ),
            (
                "Shut Down",
                "Shut down?",
                "Windows powers off, closing every application whether saved or not.",
                "Shutting down",
                Done::Power(PowerRequest::ShutDown, true),
            ),
        ];
        for (command, title, message, done_text, done) in cases {
            // Asked first, with the destructive style and "Don't ask
            // again" remembered under the command's id.
            let running = pane.start(command);
            let confirmation = asked(&pane.launcher);
            assert_eq!(
                (
                    confirmation.title.as_str(),
                    confirmation.message.as_deref(),
                    confirmation.primary.as_str(),
                    confirmation.destructive,
                    confirmation.rememberable
                ),
                (title, Some(message), command, true, true),
                "{command}"
            );
            // Confirmed without ticking it: it happens, and asks again
            // next time.
            pane.launcher
                .answer_confirmation(confirmation.id, ConfirmAnswer::Confirmed, false);
            running.ended();
            assert_eq!(
                pane.huds(),
                [hud(done_text, ToastStyle::Success)],
                "{command}"
            );
            assert_eq!(pane.commands.take(), [done.clone()], "{command}");
            assert!(
                pane.launcher
                    .remembered_confirmations(&pane.identity)
                    .is_empty()
            );

            // Ticked, the answer is remembered under the command's id and
            // given at once from then on.
            let running = pane.start(command);
            let confirmation = asked(&pane.launcher);
            pane.launcher
                .answer_confirmation(confirmation.id, ConfirmAnswer::Confirmed, true);
            running.ended();
            assert!(
                pane.launcher
                    .remembered_confirmations(&pane.identity)
                    .contains(&command_id(command).to_owned()),
                "{command}: {:?}",
                pane.launcher.remembered_confirmations(&pane.identity)
            );
            let running = pane.start(command);
            running.ended();
            assert_eq!(
                pane.launcher.confirmation(),
                None,
                "{command} asked nothing"
            );
            assert_eq!(
                pane.huds(),
                [hud(done_text, ToastStyle::Success)],
                "{command}"
            );
            assert_eq!(pane.commands.take(), [done], "{command}");

            // A dismissal changes nothing, ticked or not.
            let running = pane.start(command);
            let confirmation = asked(&pane.launcher);
            pane.launcher
                .answer_confirmation(confirmation.id, ConfirmAnswer::Dismissed, true);
            running.ended();
            assert!(pane.huds().is_empty(), "{command} showed nothing");
            assert!(pane.commands.take().is_empty(), "{command} did nothing");
        }
        let remembered = pane.launcher.remembered_confirmations(&pane.identity);
        for id in ["log-out", "restart", "shut-down"] {
            assert!(remembered.contains(&id.to_owned()), "{id}: {remembered:?}");
        }
    }

    #[test]
    fn a_hotkey_runs_a_command_without_showing_the_window() {
        let pane = Pane::new();
        let shortcut = Shortcut::parse("ctrl+alt+l").unwrap();
        let set = pane
            .launcher
            .set_hotkey(&pane.id("lock-screen"), Some(shortcut.clone()));
        block_on(set.expect("the hotkey is accepted"));

        // Something else is typed, the window is where it is: the hotkey
        // runs the command without showing it.
        pane.search("abc");
        assert!(!pane.launcher.hotkey_shows_window(&shortcut));
        block_on(
            pane.launcher
                .press_hotkey(&shortcut)
                .expect("the hotkey is registered"),
        );
        assert_eq!(
            pane.huds(),
            [hud("Locking the screen", ToastStyle::Success)]
        );
        assert_eq!(pane.commands.take(), [Done::Locked]);
        assert_eq!(
            pane.launcher.view().screen,
            Screen::Root {
                query: "abc".into()
            }
        );
    }
}
