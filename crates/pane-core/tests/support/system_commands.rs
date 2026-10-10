//! A fake of the system's session, power and audio commands for the tests
//! (#255, #265): it answers what the test sets and records what it was
//! asked to do, so a test never locks, logs out, restarts, shuts down,
//! sleeps, hibernates, turns off the displays of a real session, changes
//! the volume or mutes a microphone. The default is a computer that
//! suspends (not Modern Standby) and has a hibernation file, with the
//! volume at 50 and unmuted and one unmuted microphone, and every call
//! succeeds; `set_capabilities`, `set_volume_state`, `set_microphones`,
//! `vanish` and `fail` change that.

#![allow(dead_code)]

use std::sync::Mutex;

use pane_core::system_commands::{Capabilities, Microphone, PowerRequest, SystemCommands, Volume};

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
    /// Set the default output device's volume, to the volume given.
    SetVolume(Volume),
    /// Set the microphone's mute, by its id, to the mute given.
    SetMicrophoneMute(String, bool),
}

/// What a vanished microphone answers when it is set, as one that went
/// since it was listed does.
const GONE: &str = "The microphone is gone";

/// The session, power and audio commands, recorded: what `capabilities`,
/// `volume` and `microphones` say is set by the test, and so is any
/// failure. It starts as a computer that suspends (not Modern Standby)
/// and has a hibernation file, with the volume at 50 and unmuted and one
/// unmuted microphone, with every call succeeding.
pub struct RecordingSystemCommands {
    done: Mutex<Vec<Done>>,
    capabilities: Mutex<Capabilities>,
    failure: Mutex<Option<String>>,
    volume: Mutex<Volume>,
    microphones: Mutex<Vec<Microphone>>,
    vanished: Mutex<Vec<String>>,
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
            volume: Mutex::new(Volume {
                level: 50,
                muted: false,
            }),
            microphones: Mutex::new(vec![Microphone {
                id: "microphone".into(),
                muted: false,
            }]),
            vanished: Mutex::new(Vec::new()),
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

    /// What `volume` answers from now on.
    pub fn set_volume_state(&self, volume: Volume) {
        *self.volume.lock().unwrap() = volume;
    }

    /// The microphones there are from now on, each with whether it is
    /// muted; none at all answers that no microphone is connected.
    pub fn set_microphones(&self, microphones: Vec<Microphone>) {
        *self.microphones.lock().unwrap() = microphones;
    }

    /// The microphone `id` answers that it is gone from now on, as one
    /// that vanished since it was listed does.
    pub fn vanish(&self, id: &str) {
        self.vanished.lock().unwrap().push(id.into());
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

    fn volume(&self) -> Result<Volume, String> {
        match self.failure.lock().unwrap().clone() {
            Some(why) => Err(why),
            None => Ok(*self.volume.lock().unwrap()),
        }
    }

    fn set_volume(&self, to: Volume) -> Result<Volume, String> {
        self.answer(Done::SetVolume(to), Ok(()))?;
        *self.volume.lock().unwrap() = to;
        Ok(to)
    }

    fn microphones(&self) -> Result<Vec<Microphone>, String> {
        match self.failure.lock().unwrap().clone() {
            Some(why) => Err(why),
            None => Ok(self.microphones.lock().unwrap().clone()),
        }
    }

    fn set_microphone_mute(&self, id: &str, muted: bool) -> Result<(), String> {
        if self.vanished.lock().unwrap().iter().any(|gone| gone == id) {
            return Err(GONE.into());
        }
        self.answer(Done::SetMicrophoneMute(id.into(), muted), Ok(()))?;
        if let Some(device) = self
            .microphones
            .lock()
            .unwrap()
            .iter_mut()
            .find(|device| device.id == id)
        {
            device.muted = muted;
        }
        Ok(())
    }
}
