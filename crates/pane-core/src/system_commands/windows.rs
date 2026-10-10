//! The session and power commands on Windows (see the parent module):
//! each decision the parent makes is carried out through a documented
//! Windows API. The privilege a session-ending call needs is enabled for
//! that call alone and put back as it was after.
//!
//! - **Lock Screen**: `LockWorkStation`.
//! - **Log Out, Restart, Shut Down**: `ExitWindowsEx` with a planned
//!   reason (`SHTDN_REASON_FLAG_PLANNED`, an application's maintenance),
//!   `EWX_FORCE` — applications closed without the chance to save — for
//!   a restart and a shutdown only, as the user chose (ADR 0040), and the
//!   shutdown privilege (`SeShutdownPrivilege`) enabled around the call.
//! - **Sleep, Hibernate**: `SetSuspendState`, or the displays'
//!   monitor-power message on a computer that enters Modern Standby when
//!   they turn off.
//! - **Volume Up, Volume Down, Toggle Mute, Set Volume**: Core Audio: the
//!   default output endpoint's `IAudioEndpointVolume`, its level set as a
//!   scalar of 100 and its mute set with it.
//! - **Toggle Microphone Mute**: the active capture endpoints, each
//!   reached through its own `IAudioEndpointVolume` to set its mute.
//! - **Turn Off Displays, Start Screen Saver**: a `WM_SYSCOMMAND`
//!   (`SC_MONITORPOWER` or `SC_SCREENSAVE`) sent with `SendMessageTimeoutW`
//!   to a hidden window of Pane's own, never a broadcast that a hung
//!   application could block; Windows' own procedure (`DefWindowProcW`)
//!   turns the command into what it does. A screen saver that is off in
//!   Windows' settings (`SPI_GETSCREENSAVEACTIVE`) is explained rather
//!   than started.
//! - **What the computer can do**: `GetPwrCapabilities`, whose `AoAc`
//!   says the computer enters Modern Standby when its displays turn off,
//!   and whose `HiberFilePresent` says there is a hibernation file.

use std::mem::size_of;

use ::windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, LUID, WPARAM};
use ::windows::Win32::Graphics::Gdi::SC_SCREENSAVE;
use ::windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use ::windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole,
    eRender,
};
use ::windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_PRIVILEGES_ATTRIBUTES,
    TOKEN_QUERY,
};
use ::windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use ::windows::Win32::System::Power::{
    GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES, SetSuspendState,
};
use ::windows::Win32::System::Shutdown::{
    EWX_FORCE, EWX_LOGOFF, EWX_POWEROFF, EWX_REBOOT, ExitWindowsEx, LockWorkStation,
    SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_APPLICATION, SHTDN_REASON_MINOR_MAINTENANCE,
    SHUTDOWN_REASON,
};
use ::windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use ::windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, SC_MONITORPOWER, SMTO_ABORTIFHUNG, SPI_GETSCREENSAVEACTIVE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageTimeoutW, SystemParametersInfoW, WM_SYSCOMMAND,
};
use ::windows::core::{BOOL, PCWSTR};

use super::{Capabilities, Microphone, PowerRequest, SystemCommands, Volume};
use crate::threads::windows::WindowClass;
use crate::util::wide;
use crate::windows_shell::Com;

/// The class of the hidden windows the commands' messages are sent
/// through: top-level windows of Pane's own, never shown, whose messages
/// Windows' own procedure handles.
static COMMANDS: WindowClass = WindowClass::new("PaneSystemCommands", procedure);

/// The procedure of the commands' windows: Windows' own, which turns the
/// monitor-power and screen-saver commands into what they do.
extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: the arguments are those this procedure was called with.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// How long a command's message is given to be handled, so that a hung
/// path cannot hold the calling thread up.
const MESSAGE_TIMEOUT_MS: u32 = 1_000;

/// What the monitor-power command turns the displays to: off.
const MONITOR_OFF: isize = 2;

/// The session and power commands on Windows.
pub(super) struct WindowsSystemCommands;

impl SystemCommands for WindowsSystemCommands {
    fn capabilities(&self) -> Result<Capabilities, String> {
        let mut power = SYSTEM_POWER_CAPABILITIES::default();
        // SAFETY: `power` is a valid, writable struct for the call.
        if !unsafe { GetPwrCapabilities(&mut power) } {
            return Err("Windows would not say what this computer can do".into());
        }
        Ok(Capabilities {
            modern_standby: power.AoAc,
            hibernation_file: power.HiberFilePresent,
        })
    }

    fn lock_screen(&self) -> Result<(), String> {
        // SAFETY: no arguments.
        unsafe { LockWorkStation() }
            .map_err(|error| format!("Windows did not lock the screen: {}", error.message()))
    }

    fn power_off(&self, request: PowerRequest, force: bool) -> Result<(), String> {
        let (flags, doing) = match request {
            PowerRequest::LogOut => (EWX_LOGOFF, "log the user out"),
            PowerRequest::Restart => (EWX_REBOOT, "restart the computer"),
            PowerRequest::ShutDown => (EWX_POWEROFF, "power the computer off"),
        };
        let flags = if force { flags | EWX_FORCE } else { flags };
        // The privilege the call needs, enabled for it alone: put back as
        // it was once the call has been made.
        let _privilege = ShutdownPrivilege::enable()?;
        // SAFETY: plain values; the flags are as decided above.
        unsafe { ExitWindowsEx(flags, PLANNED) }
            .map_err(|error| format!("Windows did not {doing}: {}", error.message()))
    }

    fn suspend(&self, hibernate: bool) -> Result<(), String> {
        // SAFETY: plain flags: not forced, so Windows asks applications,
        // and wake events are left on.
        let suspended = unsafe { SetSuspendState(hibernate, false, false) };
        if !suspended {
            return Err(if hibernate {
                "Windows did not hibernate the computer".into()
            } else {
                "Windows did not suspend the computer".into()
            });
        }
        Ok(())
    }

    fn displays_off(&self) -> Result<(), String> {
        // SC_MONITORPOWER, the monitor-power hint, which Windows' own
        // procedure carries to the display driver.
        command(SC_MONITORPOWER, MONITOR_OFF, "turn the displays off")
    }

    fn screen_saver(&self) -> Result<(), String> {
        let mut active = BOOL::default();
        // SAFETY: `active` is a valid, writable BOOL for the call; the
        // action reads only it.
        unsafe {
            SystemParametersInfoW(
                SPI_GETSCREENSAVEACTIVE,
                0,
                Some(&mut active as *mut _ as _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            )
        }
        .map_err(|error| format!("Windows did not say whether a screen saver is set: {error}"))?;
        if !active.as_bool() {
            return Err(
                "No screen saver is set: choose one in Windows' lock screen settings".into(),
            );
        }
        // SC_SCREENSAVE, the screen-saver command, which Windows' own
        // procedure runs the chosen screen saver for.
        command(SC_SCREENSAVE, 0, "start the screen saver")
    }

    fn volume(&self) -> Result<Volume, String> {
        let _com = Com::new()?;
        let control = default_output()?;
        // SAFETY: plain reads of the endpoint the enumerator answered: its
        // level as a scalar, and its mute.
        let scalar = unsafe { control.GetMasterVolumeLevelScalar() }
            .map_err(|error| format!("Windows did not say the volume: {}", error.message()))?;
        // SAFETY: as above, of its mute.
        let muted = unsafe { control.GetMute() }
            .map_err(|error| format!("Windows did not say the volume: {}", error.message()))?;
        Ok(Volume {
            level: (scalar * 100.0).round() as u8,
            muted: muted.as_bool(),
        })
    }

    fn set_volume(&self, volume: Volume) -> Result<Volume, String> {
        let _com = Com::new()?;
        let control = default_output()?;
        // SAFETY: plain values: the level as a scalar of 100 and the mute,
        // with no event context naming a caller.
        unsafe {
            control
                .SetMasterVolumeLevelScalar(volume.level as f32 / 100.0, std::ptr::null())
                .map_err(|error| format!("Windows did not set the volume: {}", error.message()))?;
            control
                .SetMute(volume.muted, std::ptr::null())
                .map_err(|error| format!("Windows did not set the volume: {}", error.message()))?;
        }
        Ok(volume)
    }

    fn microphones(&self) -> Result<Vec<Microphone>, String> {
        let _com = Com::new()?;
        let enumerator = audio_enumerator()?;
        let not_listed = |error: ::windows::core::Error| {
            format!("Windows did not list the microphones: {}", error.message())
        };
        // SAFETY: the capture endpoints that are active, which are the
        // microphones there are.
        let devices = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }
            .map_err(not_listed)?;
        // SAFETY: the collection's own count.
        let count = unsafe { devices.GetCount() }.map_err(not_listed)?;
        let mut microphones = Vec::with_capacity(count as usize);
        for index in 0..count {
            // SAFETY: an index within the count the collection answered.
            let device = unsafe { devices.Item(index) }.map_err(not_listed)?;
            // SAFETY: the id the enumerator itself answered for the device,
            // copied out of the string it points to, which is freed here.
            let id = unsafe { device.GetId() }.map_err(not_listed)?;
            // SAFETY: the string the enumerator gave is valid UTF-16, read
            // before the handle that owns it is freed below.
            let text = unsafe { id.to_string() }.ok();
            // SAFETY: the handle `GetId` answered, whose string was copied.
            unsafe { CoTaskMemFree(Some(id.0 as *const _)) };
            let Some(id) = text.filter(|id| !id.is_empty()) else {
                return Err("Windows did not name a microphone".into());
            };
            let control = endpoint_volume(&device)?;
            // SAFETY: a plain read of the endpoint's mute.
            let muted = unsafe { control.GetMute() }.map_err(not_listed)?;
            microphones.push(Microphone {
                id,
                muted: muted.as_bool(),
            });
        }
        Ok(microphones)
    }

    fn set_microphone_mute(&self, id: &str, muted: bool) -> Result<(), String> {
        let _com = Com::new()?;
        let enumerator = audio_enumerator()?;
        let wide_id = wide(id);
        // SAFETY: an id the enumerator itself answered, as `GetId` gave it.
        let device =
            unsafe { enumerator.GetDevice(PCWSTR(wide_id.as_ptr())) }.map_err(|error| {
                format!("Windows did not reach the microphone: {}", error.message())
            })?;
        let control = endpoint_volume(&device)?;
        // SAFETY: a plain value, with no event context naming a caller.
        unsafe { control.SetMute(muted, std::ptr::null()) }.map_err(|error| {
            format!(
                "Windows did not set the microphone's mute: {}",
                error.message()
            )
        })
    }
}

/// Core Audio's device enumerator. The calling thread must hold [`Com`]
/// for as long as it uses the interfaces the enumerator answers.
fn audio_enumerator() -> Result<IMMDeviceEnumerator, String> {
    // SAFETY: plain COM creation: the class is the device enumerator, and
    // the context any in-process server.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(|error| {
        format!(
            "Windows did not reach the audio devices: {}",
            error.message()
        )
    })
}

/// The default output device's endpoint volume, which the volume commands
/// read and set. The calling thread must hold [`Com`].
fn default_output() -> Result<IAudioEndpointVolume, String> {
    let enumerator = audio_enumerator()?;
    // SAFETY: the default output endpoint, of the kind and the role
    // Windows plays console sound through.
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|error| format!("Windows did not say the volume: {}", error.message()))?;
    endpoint_volume(&device)
}

/// The endpoint volume of `device`, which its volume and its mute are
/// read and set through. The calling thread must hold [`Com`].
fn endpoint_volume(device: &IMMDevice) -> Result<IAudioEndpointVolume, String> {
    // SAFETY: plain COM activation of the device's own endpoint volume.
    unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|error| {
        format!(
            "Windows did not reach an audio endpoint: {}",
            error.message()
        )
    })
}

/// The shutdown reason every session-ending call carries: a planned one,
/// an application's maintenance, as the user asked for by name.
const PLANNED: SHUTDOWN_REASON = SHUTDOWN_REASON(
    SHTDN_REASON_FLAG_PLANNED.0
        | SHTDN_REASON_MAJOR_APPLICATION.0
        | SHTDN_REASON_MINOR_MAINTENANCE.0,
);

/// Sends a `WM_SYSCOMMAND` (`command`, with `value`) to a hidden window
/// of Pane's own and answers whether it was handled — never a broadcast
/// that a hung application's window could block, and never a wait without
/// an end. The window is made for this call and destroyed with it, and
/// Windows' own procedure ([`procedure`]) does what the command says.
fn command(code: u32, value: isize, doing: &str) -> Result<(), String> {
    let window = COMMANDS.hidden_window()?;
    let mut result = 0usize;
    // SAFETY: the window is this thread's own; `result` is writable, and
    // the values plain.
    let sent = unsafe {
        SendMessageTimeoutW(
            window.handle(),
            WM_SYSCOMMAND,
            WPARAM(code as usize),
            LPARAM(value),
            SMTO_ABORTIFHUNG,
            MESSAGE_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    if sent.0 == 0 {
        return Err(format!(
            "Windows did not {doing}: the message was not handled in time"
        ));
    }
    Ok(())
}

/// The shutdown privilege (`SeShutdownPrivilege`), which ending the
/// session through `ExitWindowsEx` needs: enabled while held and put back
/// as it was when dropped.
struct ShutdownPrivilege {
    token: HANDLE,
    /// What the privilege said before, put back as it was on drop.
    was: TOKEN_PRIVILEGES,
}

impl ShutdownPrivilege {
    /// Enables the shutdown privilege of this process, answering the
    /// holder, which restores it when dropped.
    fn enable() -> Result<ShutdownPrivilege, String> {
        // SAFETY: no arguments; the handle is this process's own.
        let process = unsafe { GetCurrentProcess() };
        let mut token = HANDLE::default();
        // SAFETY: `token` is a valid, writable handle for the call; the
        // process handle is this process's own.
        unsafe { OpenProcessToken(process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) }
            .map_err(|error| {
                format!(
                    "Pane could not open its own access token: {}",
                    error.message()
                )
            })?;
        let enabled = match privileges(SE_PRIVILEGE_ENABLED) {
            Ok(enabled) => enabled,
            Err(problem) => {
                close(token);
                return Err(problem);
            }
        };
        let mut was = TOKEN_PRIVILEGES::default();
        let mut length = 0u32;
        // SAFETY: `enabled` is fully initialized and outlives the call;
        // `was` is a valid, writable buffer of the length given.
        let adjusted = unsafe {
            AdjustTokenPrivileges(
                token,
                false,
                Some(&enabled),
                size_of::<TOKEN_PRIVILEGES>() as u32,
                Some(&mut was),
                Some(&mut length),
            )
        };
        if let Err(error) = adjusted {
            close(token);
            return Err(format!(
                "Pane could not enable the shutdown privilege: {}",
                error.message()
            ));
        }
        Ok(ShutdownPrivilege { token, was })
    }
}

impl Drop for ShutdownPrivilege {
    fn drop(&mut self) {
        // SAFETY: `self.was` is fully initialized and outlives the call.
        let _ = unsafe {
            AdjustTokenPrivileges(
                self.token,
                false,
                Some(&self.was),
                size_of::<TOKEN_PRIVILEGES>() as u32,
                None,
                None,
            )
        };
        close(self.token);
    }
}

/// The state of the `SeShutdownPrivilege` privilege, as `attributes` say
/// it, for [`AdjustTokenPrivileges`].
fn privileges(attributes: TOKEN_PRIVILEGES_ATTRIBUTES) -> Result<TOKEN_PRIVILEGES, String> {
    let mut id = LUID::default();
    // SAFETY: `id` is a valid, writable LUID for the call.
    let found = unsafe { LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut id) };
    found.map_err(|error| {
        format!(
            "Pane could not find the shutdown privilege: {}",
            error.message()
        )
    })?;
    Ok(TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: id,
            Attributes: attributes,
        }],
    })
}

/// Closes `token`; whether it closed is nothing to report here.
fn close(token: HANDLE) {
    // SAFETY: a handle `OpenProcessToken` answered that its holder will
    // not use again.
    let _ = unsafe { CloseHandle(token) };
}
