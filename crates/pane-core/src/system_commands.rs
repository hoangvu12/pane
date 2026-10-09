//! The system's session and power commands, as a command reaches them
//! through Pane (`wit/system-commands.wit`, the Windows power features'
//! System Commands, #125, ADR 0040): locking the screen, logging out,
//! restarting, shutting down, sleeping, hibernating, turning the displays
//! off and starting the screen saver.
//!
//! The decisions are here, as pure functions compiled on every system and
//! tested there: which commands force applications closed (a restart and
//! a shutdown do, a log out does not, the user's choice in ADR 0040),
//! which the extension confirms first, how `sleep` sleeps (a computer
//! that enters Modern Standby when its displays turn off is slept by
//! turning them off, so Windows enters its standby as it does by itself;
//! any other is suspended), and whether `hibernate` can happen at all
//! (only with a hibernation file). [`run`] turns a decision into calls on
//! the [`SystemCommands`] trait, so an extension cannot get one wrong;
//! each call answers what it ended in ([`Outcome`]) — the state the
//! system is in now, or why nothing changed — never an error, and never a
//! reason to pause the extension.
//!
//! The launcher reaches the system through that one trait, given to it as
//! the system is ([`crate::Launcher::with_system_commands`]): the app
//! passes [`native`], tests a recording fake. A launcher given none
//! answers each command with [`none`]'s explanation. Each adapter is
//! called off the runtime's thread, so it may block for as long as the
//! system takes.
//!
//! - Windows: the documented APIs, in `system_commands/windows.rs`
//!   (`LockWorkStation`; `ExitWindowsEx` with a planned reason and the
//!   shutdown privilege enabled for the call alone; `SetSuspendState`;
//!   `GetPwrCapabilities`; monitor-power and screen-saver messages sent
//!   with a timeout to a window of Pane's own, never a broadcast).
//! - macOS and Linux: [`native`] answers that the commands are not
//!   available there yet, which the System Commands default extension's
//!   `pane.json` keeps out of those systems' default sets.

use std::sync::Arc;

#[cfg(target_os = "windows")]
mod windows;

/// A session or power command, as the `system-commands` host functions
/// name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Locks the screen; the session stays.
    LockScreen,
    /// Logs the user out, not forcing applications closed.
    LogOut,
    /// Restarts the computer, forcing applications closed.
    Restart,
    /// Powers the computer off, forcing applications closed.
    ShutDown,
    /// Sleeps the computer, as its Modern Standby signal decides.
    Sleep,
    /// Hibernates the computer, which a hibernation file makes possible.
    Hibernate,
    /// Turns the displays off.
    TurnOffDisplays,
    /// Starts the screen saver the user chose.
    StartScreenSaver,
}

impl Command {
    /// Whether it is one of the destructive set, which the System Commands
    /// extension confirms first, destructively, with "Don't ask again"
    /// (ADR 0037's confirm host function, ADR 0040's choice to keep it):
    /// logging out, restarting and shutting down. A mistyped query never
    /// ends a session.
    pub fn destructive(self) -> bool {
        matches!(self, Command::LogOut | Command::Restart | Command::ShutDown)
    }

    /// Whether it forces applications closed: a restart and a shutdown do
    /// (the user's choice, ADR 0040, like Raycast), so an application
    /// with unsaved work cannot hold one up; a log out does not, so an
    /// application can still save or veto it.
    pub fn forces_applications_closed(self) -> bool {
        matches!(self, Command::Restart | Command::ShutDown)
    }
}

/// What the computer can do, which decides how some commands act.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Whether the computer enters Modern Standby when its displays turn
    /// off, so `sleep` turns them off rather than suspending.
    pub modern_standby: bool,
    /// Whether the computer has a hibernation file, without which there
    /// is no hibernation.
    pub hibernation_file: bool,
}

/// What `sleep` does on a computer with some [`Capabilities`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepPlan {
    /// Turns the displays off, which puts a Modern Standby computer into
    /// its standby.
    DisplaysOff,
    /// Suspends the computer (S3 sleep).
    Suspend,
}

/// What `hibernate` does on a computer with some [`Capabilities`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HibernatePlan {
    /// Hibernates: the computer has a hibernation file.
    Hibernate,
    /// Nothing changes: there is no hibernation file, without which there
    /// is no hibernation, and the answer says so.
    Explain,
}

/// What a command ended in, as the command answers it: the state the
/// system is in now, or why nothing changed (`outcome` in the WIT). The
/// text is what the command shows the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It happened: what the system is doing now, for the HUD.
    Done(String),
    /// Nothing changed: why, for the HUD.
    Explained(String),
}

/// How a session ends, as [`SystemCommands::power_off`] carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerRequest {
    /// Logs the user out; applications are asked to close, not forced.
    LogOut,
    /// Restarts the computer; applications are forced closed.
    Restart,
    /// Powers the computer off; applications are forced closed.
    ShutDown,
}

/// The session and power commands of the system Pane runs on, as the
/// `system-commands` host functions act on it. Each is called off the
/// runtime's thread and may block; an error explains why nothing changed.
/// The decisions are Pane's ([`run`]); the adapter only carries them out.
pub trait SystemCommands: Send + Sync + 'static {
    /// What the computer can do, which decides how some commands act.
    fn capabilities(&self) -> Result<Capabilities, String>;

    /// Locks the screen, keeping the user's session.
    fn lock_screen(&self) -> Result<(), String>;

    /// Ends the user's session: logs the user out, restarts the computer
    /// or powers it off, as `request` says. `force` closes applications
    /// without giving them the chance to save, which the caller decides
    /// per command ([`Command::forces_applications_closed`]); a session
    /// that ends the system needs the shutdown privilege, which the
    /// adapter enables for the call alone.
    fn power_off(&self, request: PowerRequest, force: bool) -> Result<(), String>;

    /// Suspends the computer, or hibernates it when `hibernate` is set
    /// (which only a computer with a hibernation file is asked for).
    fn suspend(&self, hibernate: bool) -> Result<(), String>;

    /// Turns the displays off, which also puts a Modern Standby computer
    /// into its standby.
    fn displays_off(&self) -> Result<(), String>;

    /// Starts the screen saver the user chose, or answers that none is
    /// set.
    fn screen_saver(&self) -> Result<(), String>;
}

/// Why `hibernate` answers when the computer has no hibernation file.
pub const NO_HIBERNATION_FILE: &str =
    "Hibernation is not available on this computer: there is no hibernation file";

/// What `sleep` says when it happened.
const SLEEPING: &str = "Sleeping";
/// What `hibernate` says when it happened.
const HIBERNATING: &str = "Hibernating";

/// What `sleep` does on a computer with `capabilities`: one that enters
/// Modern Standby when its displays turn off is slept by turning them off,
/// so Windows enters its standby as it does by itself; any other is
/// suspended.
pub fn sleep_plan(capabilities: &Capabilities) -> SleepPlan {
    if capabilities.modern_standby {
        SleepPlan::DisplaysOff
    } else {
        SleepPlan::Suspend
    }
}

/// What `hibernate` does on a computer with `capabilities`: only one with
/// a hibernation file can hibernate; without one, nothing changes and the
/// answer says so.
pub fn hibernate_plan(capabilities: &Capabilities) -> HibernatePlan {
    if capabilities.hibernation_file {
        HibernatePlan::Hibernate
    } else {
        HibernatePlan::Explain
    }
}

/// What `command` does through `commands`: the state it ended in, or why
/// nothing changed. The decisions are Pane's — which commands force
/// applications closed, how `sleep` sleeps, whether `hibernate` can
/// happen — so an extension cannot get one wrong, whatever system it runs
/// on.
pub fn run(command: Command, commands: &dyn SystemCommands) -> Outcome {
    let answer = match command {
        Command::LockScreen => commands.lock_screen(),
        Command::LogOut | Command::Restart | Command::ShutDown => {
            commands.power_off(power_request(command), command.forces_applications_closed())
        }
        Command::Sleep => return sleep(commands),
        Command::Hibernate => return hibernate(commands),
        Command::TurnOffDisplays => commands.displays_off(),
        Command::StartScreenSaver => commands.screen_saver(),
    };
    ended(answer, done_text(command))
}

/// How the session-ending `command` ends it.
fn power_request(command: Command) -> PowerRequest {
    match command {
        Command::LogOut => PowerRequest::LogOut,
        Command::Restart => PowerRequest::Restart,
        _ => PowerRequest::ShutDown,
    }
}

/// What `command` says when it happened, for the HUD.
fn done_text(command: Command) -> &'static str {
    match command {
        Command::LockScreen => "Locking the screen",
        Command::LogOut => "Logging out",
        Command::Restart => "Restarting",
        Command::ShutDown => "Shutting down",
        Command::Sleep => SLEEPING,
        Command::Hibernate => HIBERNATING,
        Command::TurnOffDisplays => "Turning off the displays",
        Command::StartScreenSaver => "Starting the screen saver",
    }
}

/// `answer` as the outcome of a command that says `text` when it happens.
fn ended(answer: Result<(), String>, text: &str) -> Outcome {
    match answer {
        Ok(()) => Outcome::Done(text.into()),
        Err(why) => Outcome::Explained(why),
    }
}

/// What `sleep` does through `commands`: as its Modern Standby signal
/// decides ([`sleep_plan`]), turning the displays off or suspending.
fn sleep(commands: &dyn SystemCommands) -> Outcome {
    let capabilities = match commands.capabilities() {
        Ok(capabilities) => capabilities,
        Err(why) => return Outcome::Explained(why),
    };
    let answer = match sleep_plan(&capabilities) {
        SleepPlan::DisplaysOff => commands.displays_off(),
        SleepPlan::Suspend => commands.suspend(false),
    };
    ended(answer, SLEEPING)
}

/// What `hibernate` does through `commands`: only a computer with a
/// hibernation file hibernates ([`hibernate_plan`]); without one nothing
/// changes and the answer says why.
fn hibernate(commands: &dyn SystemCommands) -> Outcome {
    let capabilities = match commands.capabilities() {
        Ok(capabilities) => capabilities,
        Err(why) => return Outcome::Explained(why),
    };
    match hibernate_plan(&capabilities) {
        HibernatePlan::Hibernate => ended(commands.suspend(true), HIBERNATING),
        HibernatePlan::Explain => Outcome::Explained(NO_HIBERNATION_FILE.into()),
    }
}

/// This system's adapter: Windows', or one that explains that the
/// session and power commands are Windows-only for now.
pub fn native() -> Arc<dyn SystemCommands> {
    #[cfg(target_os = "windows")]
    {
        Arc::new(windows::WindowsSystemCommands)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Arc::new(Unavailable(format!(
            "Not available on {}: the session and power commands are Windows-only for now",
            system_name()
        )))
    }
}

/// The commands of a launcher given none: every command answers that this
/// Pane reaches no session and power commands.
pub fn none() -> Arc<dyn SystemCommands> {
    Arc::new(Unavailable(
        "Not available: this Pane reaches no session and power commands".into(),
    ))
}

/// This system's name as a sentence says it.
#[cfg(not(target_os = "windows"))]
fn system_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        std::env::consts::OS
    }
}

/// A system Pane reaches no session and power commands of, saying why.
struct Unavailable(String);

impl SystemCommands for Unavailable {
    fn capabilities(&self) -> Result<Capabilities, String> {
        Err(self.0.clone())
    }

    fn lock_screen(&self) -> Result<(), String> {
        Err(self.0.clone())
    }

    fn power_off(&self, _request: PowerRequest, _force: bool) -> Result<(), String> {
        Err(self.0.clone())
    }

    fn suspend(&self, _hibernate: bool) -> Result<(), String> {
        Err(self.0.clone())
    }

    fn displays_off(&self) -> Result<(), String> {
        Err(self.0.clone())
    }

    fn screen_saver(&self) -> Result<(), String> {
        Err(self.0.clone())
    }
}
