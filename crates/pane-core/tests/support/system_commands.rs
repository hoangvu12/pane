//! A fake of the system's session and power commands for the tests
//! (#255): it answers what the test sets and records what it was asked to
//! do, so a test never locks, logs out, restarts, shuts down, sleeps,
//! hibernates or turns off the displays of a real session. The default is
//! a computer that suspends (not Modern Standby) and has a hibernation
//! file, and every call succeeds; `set_capabilities` and `fail` change
//! that.

#![allow(dead_code)]

use std::sync::Mutex;

use pane_core::system_commands::{Capabilities, PowerRequest, SystemCommands};

/// What the fake was asked to do, in order: each adapter call, with how it
/// was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Done {
    /// Locked the screen.
    Locked,
    /// Ended the session, with how it ends it and whether applications
    /// are forced closed.
    Power(PowerRequest, bool),
    /// Suspended the computer, or hibernated it.
    Suspended(bool),
    /// Turned the displays off.
    DisplaysOff,
    /// Started the screen saver.
    ScreenSaver,
}

/// The session and power commands, recorded: what `capabilities` says is
/// set by the test, and so is any failure. It starts as a computer that
/// suspends (not Modern Standby) and has a hibernation file, with every
/// call succeeding.
pub struct RecordingSystemCommands {
    done: Mutex<Vec<Done>>,
    capabilities: Mutex<Capabilities>,
    failure: Mutex<Option<String>>,
}

impl Default for RecordingSystemCommands {
    fn default() -> RecordingSystemCommands {
        RecordingSystemCommands {
            done: Mutex::new(Vec::new()),
            capabilities: Mutex::new(Capabilities {
                modern_standby: false,
                hibernation_file: true,
            }),
            failure: Mutex::new(None),
        }
    }
}

impl RecordingSystemCommands {
    /// What the fake was asked to do so far, in order, forgotten once read.
    pub fn take(&self) -> Vec<Done> {
        std::mem::take(&mut *self.done.lock().unwrap())
    }

    /// What `capabilities` answers from now on.
    pub fn set_capabilities(&self, capabilities: Capabilities) {
        *self.capabilities.lock().unwrap() = capabilities;
    }

    /// Every call fails with `why` from now on.
    pub fn fail(&self, why: &str) {
        *self.failure.lock().unwrap() = Some(why.into());
    }

    /// `answer` as the fake answers it, recording `done` when it
    /// succeeds.
    fn answer(&self, done: Done, result: Result<(), String>) -> Result<(), String> {
        match (&*self.failure.lock().unwrap(), result) {
            (Some(why), _) => Err(why.clone()),
            (None, Ok(())) => {
                self.done.lock().unwrap().push(done);
                Ok(())
            }
            (None, Err(why)) => Err(why),
        }
    }
}

impl SystemCommands for RecordingSystemCommands {
    fn capabilities(&self) -> Result<Capabilities, String> {
        match self.failure.lock().unwrap().clone() {
            Some(why) => Err(why),
            None => Ok(*self.capabilities.lock().unwrap()),
        }
    }

    fn lock_screen(&self) -> Result<(), String> {
        self.answer(Done::Locked, Ok(()))
    }

    fn power_off(&self, request: PowerRequest, force: bool) -> Result<(), String> {
        self.answer(Done::Power(request, force), Ok(()))
    }

    fn suspend(&self, hibernate: bool) -> Result<(), String> {
        self.answer(Done::Suspended(hibernate), Ok(()))
    }

    fn displays_off(&self) -> Result<(), String> {
        self.answer(Done::DisplaysOff, Ok(()))
    }

    fn screen_saver(&self) -> Result<(), String> {
        self.answer(Done::ScreenSaver, Ok(()))
    }
}
