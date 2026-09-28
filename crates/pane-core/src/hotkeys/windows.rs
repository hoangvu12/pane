//! Global hotkeys on Windows: `RegisterHotKey` on a thread of Pane's own,
//! whose message loop receives `WM_HOTKEY` whichever window has focus. A
//! shortcut another application registered already is refused with
//! `ERROR_HOTKEY_ALREADY_REGISTERED`, which is how a conflict shows; Windows
//! also refuses some of its own shortcuts that way. No permission is needed.
//!
//! A hotkey belongs to the thread that registered it, so registering and
//! releasing are done by that thread: the caller queues the request, wakes
//! the thread with a thread message and waits for its answer.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{ERROR_HOTKEY_ALREADY_REGISTERED, GetLastError};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY, WM_USER,
};

use super::{HotkeyError, Hotkeys, PressSender, Shortcut};

/// Wakes the hotkey thread to serve its queued requests.
const WM_REQUEST: u32 = WM_APP + 1;

enum Request {
    Register(Shortcut, mpsc::Sender<Result<(), HotkeyError>>),
    Unregister(Shortcut, mpsc::Sender<()>),
}

/// The adapter: a thread that registers the hotkeys and receives their
/// presses.
pub struct WindowsHotkeys {
    thread: u32,
    requests: Arc<Mutex<VecDeque<Request>>>,
}

/// The virtual-key code of `key` (see `Shortcut::key`).
fn virtual_key(key: &str) -> Option<u32> {
    match key.as_bytes() {
        [c @ b'a'..=b'z'] => Some(u32::from(c.to_ascii_uppercase())),
        [c @ b'0'..=b'9'] => Some(u32::from(*c)),
        _ if key == "space" => Some(0x20),
        _ => {
            let number: u32 = key.strip_prefix('f')?.parse().ok()?;
            // VK_F1 is 0x70.
            (1..=12).contains(&number).then(|| 0x70 + number - 1)
        }
    }
}

fn modifiers(shortcut: &Shortcut) -> u32 {
    // Holding the keys reports one press, not one per repeat.
    let mut modifiers = MOD_NOREPEAT;
    if shortcut.control() {
        modifiers |= MOD_CONTROL;
    }
    if shortcut.alt() {
        modifiers |= MOD_ALT;
    }
    if shortcut.shift() {
        modifiers |= MOD_SHIFT;
    }
    if shortcut.super_key() {
        modifiers |= MOD_WIN;
    }
    modifiers
}

impl WindowsHotkeys {
    /// Starts the hotkey thread.
    pub fn start(presses: PressSender) -> Result<WindowsHotkeys, String> {
        let requests: Arc<Mutex<VecDeque<Request>>> = Arc::default();
        let served = requests.clone();
        let (started, thread) = mpsc::channel();
        std::thread::Builder::new()
            .name("pane-hotkeys".into())
            .spawn(move || serve(&served, &presses, &started))
            .map_err(|error| error.to_string())?;
        let thread = thread
            .recv()
            .map_err(|_| "the hotkey thread did not start".to_string())?;
        Ok(WindowsHotkeys { thread, requests })
    }

    /// Queues `request` and wakes the thread; false if it is gone.
    fn send(&self, request: Request) -> bool {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(request);
        // SAFETY: posting a message with no pointers to a thread id.
        unsafe { PostThreadMessageW(self.thread, WM_REQUEST, 0, 0) != 0 }
    }
}

impl Hotkeys for WindowsHotkeys {
    fn unavailable(&self) -> Option<String> {
        None
    }

    fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError> {
        let (answer, answered) = mpsc::channel();
        if !self.send(Request::Register(shortcut.clone(), answer)) {
            return Err(HotkeyError::Refused("the hotkey thread stopped".into()));
        }
        answered
            .recv()
            .unwrap_or_else(|_| Err(HotkeyError::Refused("the hotkey thread stopped".into())))
    }

    fn unregister(&self, shortcut: &Shortcut) {
        let (answer, answered) = mpsc::channel();
        if self.send(Request::Unregister(shortcut.clone(), answer)) {
            let _ = answered.recv();
        }
    }
}

/// The hotkey thread: registers and releases hotkeys as asked and reports
/// their presses, until the process ends.
fn serve(requests: &Mutex<VecDeque<Request>>, presses: &PressSender, started: &mpsc::Sender<u32>) {
    // SAFETY: plain Win32 calls on this thread with a valid MSG buffer.
    unsafe {
        let mut message: MSG = std::mem::zeroed();
        // Makes the thread's message queue, so posts to it are not lost.
        PeekMessageW(
            &mut message,
            std::ptr::null_mut(),
            WM_USER,
            WM_USER,
            PM_NOREMOVE,
        );
        if started.send(GetCurrentThreadId()).is_err() {
            return;
        }
        let mut registered: HashMap<i32, Shortcut> = HashMap::new();
        let mut next_id: i32 = 1;
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            match message.message {
                WM_HOTKEY => {
                    if let Some(shortcut) = registered.get(&(message.wParam as i32)) {
                        presses.send(shortcut.clone());
                    }
                }
                WM_REQUEST => loop {
                    let request = requests
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .pop_front();
                    match request {
                        None => break,
                        Some(Request::Register(shortcut, answer)) => {
                            if registered.values().any(|done| *done == shortcut) {
                                let _ = answer.send(Ok(()));
                                continue;
                            }
                            let Some(key) = virtual_key(shortcut.key()) else {
                                let refused = format!("{shortcut} has no Windows key");
                                let _ = answer.send(Err(HotkeyError::Refused(refused)));
                                continue;
                            };
                            let id = next_id;
                            let result = if RegisterHotKey(
                                std::ptr::null_mut(),
                                id,
                                modifiers(&shortcut),
                                key,
                            ) != 0
                            {
                                next_id += 1;
                                registered.insert(id, shortcut);
                                Ok(())
                            } else {
                                let error = GetLastError();
                                if error == ERROR_HOTKEY_ALREADY_REGISTERED {
                                    Err(HotkeyError::Taken)
                                } else {
                                    Err(HotkeyError::Refused(
                                        std::io::Error::from_raw_os_error(error as i32).to_string(),
                                    ))
                                }
                            };
                            let _ = answer.send(result);
                        }
                        Some(Request::Unregister(shortcut, answer)) => {
                            let ids: Vec<i32> = registered
                                .iter()
                                .filter(|(_, done)| **done == shortcut)
                                .map(|(id, _)| *id)
                                .collect();
                            for id in ids {
                                UnregisterHotKey(std::ptr::null_mut(), id);
                                registered.remove(&id);
                            }
                            let _ = answer.send(());
                        }
                    }
                },
                _ => {}
            }
        }
    }
}
