//! Global hotkeys on Windows: `RegisterHotKey` on a thread of Pane's own
//! for the chords Windows accepts, and a low-level keyboard hook of
//! Pane's own (`WH_KEYBOARD_LL`) for the chords Windows refuses — another
//! application has the shortcut, or Windows keeps it for itself — so a
//! refused shortcut is never an error: the binding works while Pane runs
//! (ADR 0039). A conflict shows as `ERROR_HOTKEY_ALREADY_REGISTERED`,
//! which is the hook's cue rather than the user's problem. No permission
//! is needed.
//!
//! A hotkey belongs to the thread that registered it, so registering and
//! releasing are done by that thread: the caller queues the request, wakes
//! the thread with a thread message and waits for its answer. Dropping the
//! adapter ends the thread, which releases its hotkeys as it ends. The
//! thread is a `threads::windows::MessageThread`, like the clipboard
//! listener's.
//!
//! The hook lives on a thread of its own, started when the first binding
//! needs it and stopped when none does. Its callback runs at the highest
//! thread priority — not a raised process priority, so the rest of Pane is
//! unaffected — and only steps the recognizer (see `recognizer`) and
//! posts a message to the adapter's thread when a chord fires: it never
//! allocates, takes no lock another thread holds, and calls nothing
//! outside `user32`'s hook machinery. Its code and data pages are locked
//! in memory after the process's minimum working set is raised by what
//! they need, so a trimmed working set cannot fault the callback past
//! Windows' hook timeout; if Windows refuses, the diagnostic says the
//! pages are not pinned. A watchdog window that receives raw keyboard
//! input notices when key events stop reaching the hook — Windows has
//! silently removed it — and installs it again, giving up with a
//! diagnostic after repeated failures within a short time. The
//! recognizer's modifiers are re-read from the system on a session
//! unlock, on a resume and when an event contradicts them, so a release
//! missed while the secure desktop had the keyboard never leaves a
//! modifier stuck down. Keys Pane injects carry [`INJECTED_TAG`], which
//! the recognizer answers by passing them through untouched.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ::windows::Win32::Foundation::{
    ERROR_HOTKEY_ALREADY_REGISTERED, HWND, LPARAM, LRESULT, WPARAM,
};
use ::windows::Win32::System::Memory::{
    GetProcessWorkingSetSizeEx, SETPROCESSWORKINGSETSIZEEX_FLAGS, SetProcessWorkingSetSizeEx,
    VirtualLock,
};
use ::windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use ::windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, SetThreadPriority,
    THREAD_PRIORITY_TIME_CRITICAL,
};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};
use ::windows::Win32::UI::Input::{RAWINPUTDEVICE, RIDEV_INPUTSINK, RegisterRawInputDevices};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DefWindowProcW, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLKHF_UP, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_HOTKEY, WM_INPUT, WM_POWERBROADCAST,
    WM_WTSSESSION_CHANGE,
};
use ::windows::core::HRESULT;

use super::{
    Decision, HookHealth, HotkeyError, Hotkeys, KeyEvent, PressSender, Recognizer, Route, Shortcut,
};
use crate::threads::windows::{MessageThread, WM_FIRED, WM_WAKE, Window, WindowClass, stop_sent};

/// How many raw key events arrive without the hook reporting one before
/// the watchdog concludes that Windows removed the hook and installs it
/// again.
const RAW_BEFORE_REINSTALL: u32 = 6;

/// How long a run of reinstallations may last before Pane counts them
/// afresh: only failures within a short time give up.
const REINSTALL_WINDOW: Duration = Duration::from_secs(30);

/// How many reinstallations within [`REINSTALL_WINDOW`] Pane gives up
/// after, leaving the hook-based hotkeys dead until Pane starts again.
const REINSTALLS_BEFORE_GIVING_UP: u32 = 5;

/// How much the process's minimum working set is raised by, for the pages
/// the hook pins: a lock that would not fit the working set fails, so the
/// allowance covers the pages with headroom.
const PIN_ALLOWANCE: usize = 256 * 1024;

/// One memory page, as every Windows Pane runs on uses.
const PAGE: usize = 4096;

/// The `WM_WTSSESSION_CHANGE` parameter that says the session was
/// unlocked: the secure desktop below the lock screen had the keyboard,
/// so key events were not delivered to the hook.
const SESSION_UNLOCK: usize = 8;

enum Request {
    Register(Shortcut, mpsc::Sender<Result<Route, HotkeyError>>),
    Unregister(Shortcut, mpsc::Sender<()>),
}

type Requests = Arc<Mutex<VecDeque<Request>>>;

/// How the adapter answers [`Hotkeys::route`]: each registered shortcut's
/// route, as the hotkey thread answered when it registered it.
type Routes = Arc<Mutex<HashMap<Shortcut, Route>>>;

/// A binding the hotkey thread asks the hook thread to watch: the
/// binding's id, the key's virtual-key code, and the modifiers held.
enum HookRequest {
    Add(u32, u32, [bool; 4], mpsc::Sender<Result<(), String>>),
    Remove(u32, mpsc::Sender<()>),
}

type HookRequests = Arc<Mutex<VecDeque<HookRequest>>>;

/// The adapter: a thread that registers the hotkeys and receives their
/// presses, and the routes its bindings are dispatched by, for the rows
/// to say.
pub struct WindowsHotkeys {
    thread: MessageThread,
    requests: Requests,
    /// How each registered shortcut is dispatched, as the hotkey thread
    /// answered when it registered it.
    routes: Routes,
    /// The keyboard hook's state, shared with the hook's own thread.
    report: Arc<Mutex<Report>>,
}

/// The hook's state, as the adapter answers [`Hotkeys::hook_health`] and
/// the opt-in real-input tests: written on the hook's own thread (never
/// in its callback), read wherever Pane shows it.
#[derive(Default)]
struct Report {
    /// How many times Windows removed the hook and Pane installed it
    /// again.
    reinstalled: u32,
    /// Whether the hook's code and data pages are pinned in memory.
    pinned: bool,
    /// Why Pane gave up reinstalling the hook, if it did.
    given_up: Option<String>,
    /// The low-level hook's handle, for the tests to remove behind the
    /// adapter's back; 0 while none is installed.
    hook: usize,
    /// The watchdog's window, for the tests to deliver the raw key events
    /// the keyboard would; 0 until it is made.
    watchdog: usize,
}

/// Changes the shared report, which only the hook thread writes.
fn update(report: &Mutex<Report>, change: impl FnOnce(&mut Report)) {
    change(&mut *report.lock().unwrap_or_else(|p| p.into_inner()));
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

fn modifiers(shortcut: &Shortcut) -> HOT_KEY_MODIFIERS {
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

/// The chord `shortcut` binds, as the recognizer takes it: the modifiers
/// held and the key's virtual-key code.
fn mask(shortcut: &Shortcut) -> [bool; 4] {
    [
        shortcut.control(),
        shortcut.alt(),
        shortcut.shift(),
        shortcut.super_key(),
    ]
}

impl WindowsHotkeys {
    /// Starts the hotkey thread. The keyboard hook's thread is started
    /// only when the first binding Windows refuses needs it, and stopped
    /// when none does.
    pub fn start(presses: PressSender) -> Result<WindowsHotkeys, String> {
        let requests: Requests = Arc::default();
        let served = requests.clone();
        let routes: Routes = Arc::default();
        let report = Arc::new(Mutex::new(Report::default()));
        let told = report.clone();
        let thread = MessageThread::spawn(
            "pane-hotkeys",
            move || {
                // SAFETY: it only reads the calling thread's id.
                let adapter = unsafe { GetCurrentThreadId() };
                Ok((Registered::new(adapter, told), None))
            },
            move |registered, message| registered.serve(message, &served, &presses),
            Registered::release_all,
        )?;
        Ok(WindowsHotkeys {
            thread,
            requests,
            routes,
            report,
        })
    }

    /// Queues `request` and wakes the thread; false if it is gone.
    fn send(&self, request: Request) -> bool {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(request);
        self.thread.post(WM_WAKE)
    }

    /// The low-level keyboard hook's handle, for the opt-in real-input
    /// tests (`PANE_TEST_REAL_INPUT`) to remove behind the adapter's
    /// back, so the watchdog has a removal to notice and repair. `None`
    /// while no binding needs the hook.
    pub fn hook_handle(&self) -> Option<usize> {
        let report = self.report.lock().unwrap_or_else(|p| p.into_inner());
        (report.hook != 0).then_some(report.hook)
    }

    /// The watchdog's window, for the opt-in real-input tests to deliver
    /// the raw key events the keyboard would: a test's injected keys are
    /// a tool's, not the keyboard's, so the watchdog cannot see them any
    /// other way. `None` while no binding needs the hook.
    pub fn watchdog_window(&self) -> Option<usize> {
        let report = self.report.lock().unwrap_or_else(|p| p.into_inner());
        (report.watchdog != 0).then_some(report.watchdog)
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
        let registered = answered
            .recv()
            .unwrap_or_else(|_| Err(HotkeyError::Refused("the hotkey thread stopped".into())));
        if let Ok(route) = &registered {
            self.routes
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(shortcut.clone(), *route);
        }
        registered.map(|_| ())
    }

    fn unregister(&self, shortcut: &Shortcut) {
        self.routes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(shortcut);
        let (answer, answered) = mpsc::channel();
        if self.send(Request::Unregister(shortcut.clone(), answer)) {
            let _ = answered.recv();
        }
    }

    fn route(&self, shortcut: &Shortcut) -> Route {
        self.routes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(shortcut)
            .copied()
            .unwrap_or(Route::System)
    }

    fn hook_health(&self) -> Option<HookHealth> {
        let hooked = self
            .routes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(|route| *route == Route::Hook);
        if !hooked {
            // No binding needs the hook, so none is installed.
            return None;
        }
        let report = self.report.lock().unwrap_or_else(|p| p.into_inner());
        Some(HookHealth {
            reinstalls: report.reinstalled,
            pinned: report.pinned,
            given_up: report.given_up.clone(),
        })
    }
}

impl Drop for WindowsHotkeys {
    /// Ends the thread, which releases every hotkey and stops the
    /// keyboard hook as it ends. It never waits on another program, so
    /// this waits until it has.
    fn drop(&mut self) {
        self.thread.stop(None);
    }
}

/// The hotkeys the thread registered, by their id.
struct Registered {
    shortcuts: HashMap<i32, Shortcut>,
    last_id: i32,
    /// The bindings the keyboard hook watches, by their id.
    hooked: HashMap<u32, Shortcut>,
    next_hook: u32,
    /// The keyboard hook's thread, while a binding needs it.
    hook: Option<Hook>,
    /// This thread's id, which the hook posts the presses it recognizes
    /// to.
    adapter: u32,
    /// The hook's state, shared with its thread and the adapter.
    report: Arc<Mutex<Report>>,
}

impl Registered {
    fn new(adapter: u32, report: Arc<Mutex<Report>>) -> Registered {
        Registered {
            shortcuts: HashMap::new(),
            last_id: 0,
            hooked: HashMap::new(),
            next_hook: 0,
            hook: None,
            adapter,
            report,
        }
    }

    fn release(&mut self, id: i32) {
        // SAFETY: `id` was registered by this thread with no window.
        let _ = unsafe { UnregisterHotKey(None, id) };
        self.shortcuts.remove(&id);
    }

    /// The route of a shortcut this thread already holds, if it does.
    fn route_of(&self, shortcut: &Shortcut) -> Option<Route> {
        if self.shortcuts.values().any(|done| *done == *shortcut) {
            return Some(Route::System);
        }
        self.hooked
            .values()
            .any(|done| *done == *shortcut)
            .then_some(Route::Hook)
    }

    /// Releases the hook bindings of `shortcut`, stopping the hook's
    /// thread when none is left: the hook is removed when no binding
    /// needs it.
    fn release_hooked(&mut self, shortcut: &Shortcut) {
        let ids: Vec<u32> = self
            .hooked
            .iter()
            .filter(|(_, done)| **done == *shortcut)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(hook) = &self.hook {
                hook.remove(id);
            }
            self.hooked.remove(&id);
        }
        if self.hooked.is_empty()
            && let Some(mut hook) = self.hook.take()
        {
            hook.stop();
        }
    }

    fn release_all(mut self) {
        // The hook's thread first: it stops watching the keyboard before
        // the registrations it stood in for go.
        if let Some(mut hook) = self.hook.take() {
            hook.stop();
        }
        let ids: Vec<i32> = self.shortcuts.keys().copied().collect();
        for id in ids {
            self.release(id);
        }
    }

    /// Reports a hotkey's press, or serves the queued requests.
    fn serve(&mut self, message: &MSG, requests: &Mutex<VecDeque<Request>>, presses: &PressSender) {
        match message.message {
            WM_HOTKEY => {
                if let Some(shortcut) = self.shortcuts.get(&(message.wParam.0 as i32)) {
                    presses.send(shortcut.clone());
                }
            }
            WM_FIRED => {
                // A chord the keyboard hook recognized: the binding the
                // hotkey thread holds for it.
                if let Some(shortcut) = self.hooked.get(&(message.wParam.0 as u32)) {
                    presses.send(shortcut.clone());
                }
            }
            WM_WAKE => loop {
                let request = requests
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .pop_front();
                match request {
                    None => break,
                    Some(Request::Register(shortcut, answer)) => {
                        let _ = answer.send(self.register(shortcut));
                    }
                    Some(Request::Unregister(shortcut, answer)) => {
                        let ids: Vec<i32> = self
                            .shortcuts
                            .iter()
                            .filter(|(_, done)| **done == shortcut)
                            .map(|(id, _)| *id)
                            .collect();
                        for id in ids {
                            self.release(id);
                        }
                        self.release_hooked(&shortcut);
                        let _ = answer.send(());
                    }
                }
            },
            _ => {}
        }
    }

    /// Registers `shortcut` with the system: `RegisterHotKey` when Windows
    /// accepts the chord, and Pane's own keyboard hook when Windows
    /// refuses it — another application has the shortcut, or Windows keeps
    /// it — which is not an error (ADR 0039): the binding works while
    /// Pane runs.
    fn register(&mut self, shortcut: Shortcut) -> Result<Route, HotkeyError> {
        if let Some(route) = self.route_of(&shortcut) {
            return Ok(route);
        }
        let Some(key) = virtual_key(shortcut.key()) else {
            return Err(HotkeyError::Refused(format!(
                "{shortcut} has no Windows key"
            )));
        };
        let id = self.last_id + 1;
        let taken = HRESULT::from_win32(ERROR_HOTKEY_ALREADY_REGISTERED.0);
        // SAFETY: plain values; with no window the hotkey belongs to this
        // thread, whose loop receives it.
        match unsafe { RegisterHotKey(None, id, modifiers(&shortcut), key) } {
            Ok(()) => {
                self.last_id = id;
                self.shortcuts.insert(id, shortcut);
                Ok(Route::System)
            }
            Err(error) if error.code() == taken => self.through_hook(shortcut, key),
            Err(error) => Err(HotkeyError::Refused(error.message())),
        }
    }

    /// Recognizes `shortcut`, whose key is `key`, through Pane's own
    /// keyboard hook instead: Windows refused the registration, and the
    /// hook can still see the chord pressed. The hook's thread starts
    /// when the first binding needs it.
    fn through_hook(&mut self, shortcut: Shortcut, key: u32) -> Result<Route, HotkeyError> {
        if self.hook.is_none() {
            self.hook = Some(Hook::start(self.adapter, self.report.clone()).map_err(
                |problem| {
                    HotkeyError::Refused(format!(
                        "Pane's keyboard hook is not available: {problem}"
                    ))
                },
            )?);
        }
        let id = self.next_hook + 1;
        if let Err(problem) =
            self.hook
                .as_mut()
                .expect("just started")
                .add(id, key, mask(&shortcut))
        {
            return Err(HotkeyError::Refused(format!(
                "Pane's keyboard hook could not take it: {problem}"
            )));
        }
        self.next_hook = id;
        self.hooked.insert(id, shortcut);
        Ok(Route::Hook)
    }
}

/// The keyboard hook's thread, as the hotkey thread drives it: it is
/// started when the first binding Windows refuses needs it and stopped
/// when none does, and its bindings are added and removed by request.
struct Hook {
    thread: MessageThread,
    requests: HookRequests,
}

impl Hook {
    /// Starts the hook's thread: the hook, the watchdog and the pinned
    /// pages are its start's work.
    fn start(adapter: u32, report: Arc<Mutex<Report>>) -> Result<Hook, String> {
        let requests: HookRequests = Arc::default();
        let served = requests.clone();
        let thread = MessageThread::spawn(
            "pane-keyboard-hook",
            move || start_hook(adapter, report),
            move |_, message| serve_hook(message, &served),
            finish_hook,
        )?;
        Ok(Hook { thread, requests })
    }

    /// Queues `request` and wakes the thread; false if it is gone.
    fn send(&self, request: HookRequest) -> bool {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(request);
        self.thread.post(WM_WAKE)
    }

    /// Binds the chord of `mask` with `key` as binding `id`, which the
    /// hook reports when it is pressed.
    fn add(&self, id: u32, key: u32, mask: [bool; 4]) -> Result<(), String> {
        let (answer, answered) = mpsc::channel();
        if !self.send(HookRequest::Add(id, key, mask, answer)) {
            return Err("the keyboard hook thread stopped".into());
        }
        answered
            .recv()
            .unwrap_or_else(|_| Err("the keyboard hook thread stopped".into()))
    }

    /// Unbinds the binding `id`.
    fn remove(&self, id: u32) {
        let (answer, answered) = mpsc::channel();
        if self.send(HookRequest::Remove(id, answer)) {
            let _ = answered.recv();
        }
    }

    /// Stops the thread, which releases the hook and its watchdog as it
    /// ends. It never waits on another program, so this waits until it
    /// has.
    fn stop(&mut self) {
        self.thread.stop(None);
    }
}

/// The hook thread's finish: its state is dropped with the thread, which
/// unhooks the hook and destroys the watchdog window.
fn finish_hook(_: ()) {}

/// The hook thread's state: the recognizer, the hook, the watchdog window
/// and the watchdog's counters. On the hook thread only, which the
/// callback, the watchdog window and the request serving all share.
struct Hooked {
    /// The bound chords and the keyboard state, stepped by the callback.
    recognizer: Recognizer,
    /// The low-level keyboard hook, replaced when Windows removes it.
    hook: HHOOK,
    /// The watchdog's window, which receives the raw keyboard input and
    /// the session and power notifications. Held here for as long as the
    /// state lives, so the window — and with it the raw-input registration
    /// and the session notification — ends with the hook; nothing else
    /// reads it.
    #[allow(dead_code)]
    window: Window,
    /// The adapter's thread, which a recognized chord is posted to.
    adapter: u32,
    /// The hook's state, shared with the adapter.
    report: Arc<Mutex<Report>>,
    /// The key events the hook last reported seeing, for the watchdog's
    /// comparison.
    last_seen: u64,
    /// The raw key events since the hook last reported seeing one.
    unseen: u32,
    /// When the first of the current run of reinstallations happened.
    first: Option<Instant>,
    /// How many times the hook has been reinstalled in the current run,
    /// which gives up after too many within a short time.
    attempts: u32,
    /// Whether Pane has given up reinstalling it.
    given_up: bool,
}

impl Drop for Hooked {
    /// Releases the hook; the raw-input registration and the session
    /// notification go with the window, and the handles the adapter
    /// answers for are gone.
    fn drop(&mut self) {
        // SAFETY: this thread's own hook.
        let _ = unsafe { UnhookWindowsHookEx(self.hook) };
        update(&self.report, |report| {
            report.hook = 0;
            report.watchdog = 0;
        });
    }
}

thread_local! {
    /// The hook thread's state, placed by its start and dropped with the
    /// thread.
    static HOOKED: std::cell::RefCell<Option<Hooked>> =
        const { std::cell::RefCell::new(None) };
}

/// The watchdog's window class.
static WATCHDOG_CLASS: WindowClass = WindowClass::new("PaneKeyboardWatchdog", watchdog);

/// The hook thread's start: its priority, the hook, the watchdog window
/// and its notifications, and the pinned pages.
fn start_hook(adapter: u32, report: Arc<Mutex<Report>>) -> Result<((), Option<HWND>), String> {
    // The highest thread priority, not a raised process priority, so the
    // rest of Pane is unaffected: the callback must return before
    // Windows' hook timeout.
    // SAFETY: the calling thread's own pseudo-handle, and plain values.
    let priority = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) };
    if priority.is_err() {
        crate::diagnostics::report_line(
            "Windows did not give Pane's keyboard hook thread its highest priority",
        );
    }
    let hook = install()?;
    let window = match WATCHDOG_CLASS.message_window() {
        Ok(window) => window,
        Err(problem) => {
            // SAFETY: this thread's own hook, just installed.
            let _ = unsafe { UnhookWindowsHookEx(hook) };
            return Err(problem);
        }
    };
    let watchdog = window.handle();
    if let Err(problem) = watch_input(watchdog) {
        // SAFETY: this thread's own hook, just installed.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        return Err(format!(
            "the watchdog could not watch the keyboard: {problem}"
        ));
    }
    // A session notification that fails is a lost resync, not a lost hook:
    // the resume and contradiction re-reads still cover it.
    // SAFETY: plain values; the notification reaches this window.
    let notified = unsafe { WTSRegisterSessionNotification(watchdog, NOTIFY_FOR_THIS_SESSION) };
    if notified.is_err() {
        crate::diagnostics::report_line(
            "Windows did not tell Pane's keyboard hook of session changes; its modifiers are \
             re-read on a resume and on a contradiction only",
        );
    }
    let handle = hook.0 as usize;
    HOOKED.with(|hooked| {
        hooked.borrow_mut().replace(Hooked {
            recognizer: Recognizer::default(),
            hook,
            window,
            adapter,
            report: report.clone(),
            last_seen: 0,
            unseen: 0,
            first: None,
            attempts: 0,
            given_up: false,
        });
    });
    // The pages are pinned where the state now lives, in the thread's own
    // storage: it does not move again, so the lock holds.
    let mut pinned = true;
    HOOKED.with(|hooked| {
        if let Some(state) = hooked.borrow().as_ref()
            && let Err(problem) = pin(state as *const Hooked as usize)
        {
            pinned = false;
            crate::diagnostics::report_line(&format!(
                "Pane's keyboard hook pages are not pinned in memory: {problem}"
            ));
        }
    });
    update(&report, |report| {
        report.pinned = pinned;
        report.hook = handle;
        report.watchdog = watchdog.0 as usize;
    });
    Ok(((), Some(watchdog)))
}

/// Serves the hook thread's queued requests.
fn serve_hook(message: &MSG, requests: &Mutex<VecDeque<HookRequest>>) {
    if message.message != WM_WAKE {
        return;
    }
    loop {
        let request = requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front();
        match request {
            None => break,
            Some(HookRequest::Add(id, key, mask, answer)) => {
                let _ = answer.send(bind(id, key, mask));
            }
            Some(HookRequest::Remove(id, answer)) => {
                unbind(id);
                let _ = answer.send(());
            }
        }
    }
}

/// Adds the binding to the recognizer of the hook thread's state.
fn bind(id: u32, key: u32, mask: [bool; 4]) -> Result<(), String> {
    HOOKED
        .try_with(|hooked| {
            let Ok(mut held) = hooked.try_borrow_mut() else {
                return Err("the hook's state is in use".into());
            };
            let Some(state) = held.as_mut() else {
                return Err("the hook's state is gone".into());
            };
            state.recognizer.add(id, mask, key);
            Ok(())
        })
        .unwrap_or_else(|_| Err("the hook's state is gone".into()))
}

/// Removes the binding from the recognizer of the hook thread's state.
fn unbind(id: u32) {
    let _ = HOOKED.try_with(|hooked| {
        let Ok(mut held) = hooked.try_borrow_mut() else {
            return;
        };
        if let Some(state) = held.as_mut() {
            state.recognizer.remove(id);
        }
    });
}

/// The low-level keyboard hook's callback, called by the system on the
/// thread that installed it while it pumps its messages. It only steps
/// the recognizer and posts the press to the adapter's thread: it never
/// allocates, takes no lock another thread holds, or calls anything
/// outside the hook machinery. A panic must not unwind into Windows,
/// which would end Pane, so it catches one and lets the event through.
unsafe extern "system" fn hooked(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        // SAFETY: forwarded, as the hook contract asks for a code it does
        // not handle.
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let decided = std::panic::catch_unwind(|| {
        // SAFETY: the system passes a KBDLLHOOKSTRUCT for a keyboard
        // event.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        decide(event)
    });
    let (swallow, fired) = decided.unwrap_or((false, None));
    if let Some(binding) = fired
        && let Some(adapter) = adapter_thread()
    {
        // SAFETY: plain values, posted to the adapter's thread, which
        // pumps its messages.
        let _ =
            unsafe { PostThreadMessageW(adapter, WM_FIRED, WPARAM(binding as usize), LPARAM(0)) };
    }
    if swallow {
        LRESULT(1)
    } else {
        // SAFETY: forwarded, as an event Pane does not take must be.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

/// Steps the recognizer with one keyboard event, saying whether to
/// swallow the event and which binding it fired.
fn decide(event: &KBDLLHOOKSTRUCT) -> (bool, Option<u32>) {
    HOOKED
        .try_with(|hooked| {
            let mut held = hooked.try_borrow_mut().ok()?;
            let state = held.as_mut()?;
            let key = KeyEvent {
                key: event.vkCode,
                scan: event.scanCode,
                pressed: !event.flags.contains(LLKHF_UP),
                injected: event.flags.contains(LLKHF_INJECTED),
                tag: event.dwExtraInfo,
                time: u64::from(event.time),
            };
            Some(match state.recognizer.step(key) {
                Decision::Fire(binding) => (true, Some(binding)),
                Decision::Injected(binding) => (false, Some(binding)),
                Decision::Swallow => (true, None),
                Decision::Pass => (false, None),
                Decision::Resync => {
                    // The event contradicts the state Pane holds: re-read
                    // the modifiers' real state, as an unlock or a resume
                    // does.
                    resync(&mut state.recognizer);
                    (false, None)
                }
            })
        })
        .ok()
        .flatten()
        .unwrap_or((false, None))
}

/// The adapter's thread, to post the presses to; `None` if the hook
/// thread's state is gone.
fn adapter_thread() -> Option<u32> {
    HOOKED
        .try_with(|hooked| {
            let held = hooked.try_borrow().ok()?;
            held.as_ref().map(|state| state.adapter)
        })
        .unwrap_or(None)
}

/// The watchdog's window: the raw keyboard input that tells the keyboard
/// is in use, the session notification of an unlock, the power
/// notification of a resume, and the stop message. A panic must not
/// unwind into Windows, which would end Pane, so it catches one.
extern "system" fn watchdog(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handled = std::panic::catch_unwind(|| watched(window, message, wparam, lparam));
    match handled {
        Ok(result) => result,
        Err(_) => {
            crate::diagnostics::report_line(
                "Pane's keyboard hook watchdog failed on a message; it goes on watching",
            );
            LRESULT(0)
        }
    }
}

/// The watchdog's messages: the raw input, the session and power
/// notifications that re-read the modifiers, and the stop message.
fn watched(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if stop_sent(message) {
        return LRESULT(0);
    }
    if message == WM_INPUT {
        // The keyboard delivered an event; the hook must have seen the
        // ones before it. The count is the message itself — Pane reads
        // no raw input data — so zero is the whole handling of it.
        let _ = HOOKED.try_with(|hooked| {
            let Ok(mut held) = hooked.try_borrow_mut() else {
                return;
            };
            if let Some(state) = held.as_mut() {
                watch(state);
            }
        });
        return LRESULT(0);
    }
    // An unlock — the secure desktop had the keyboard, so releases were
    // missed — or a resume: the modifiers' real state is re-read.
    let unlocked = message == WM_WTSSESSION_CHANGE && wparam.0 == SESSION_UNLOCK;
    let resumed = message == WM_POWERBROADCAST
        && (wparam.0 == PBT_APMRESUMEAUTOMATIC as usize
            || wparam.0 == PBT_APMRESUMESUSPEND as usize);
    if unlocked || resumed {
        resync_hooked();
        return LRESULT(0);
    }
    def(window, message, wparam, lparam)
}

/// The default processing of a message the watchdog window does not
/// handle.
fn def(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the arguments are those the procedure was called with.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// The watchdog's check on one raw key event: the hook reports each
/// physical key event to its recognizer, so when several raw events
/// arrive with the hook reporting none, Windows has removed the hook and
/// it is installed again.
fn watch(state: &mut Hooked) {
    if state.given_up {
        return;
    }
    let seen = state.recognizer.physical_events();
    if seen != state.last_seen {
        state.last_seen = seen;
        state.unseen = 0;
    }
    state.unseen += 1;
    if state.unseen < RAW_BEFORE_REINSTALL {
        return;
    }
    state.unseen = 0;
    reinstall(state);
}

/// Installs the hook again: Windows has removed it. The attempts within a
/// short time are counted, and after too many Pane gives up and says so,
/// leaving the hook-based hotkeys dead until Pane starts again.
fn reinstall(state: &mut Hooked) {
    let now = Instant::now();
    if state
        .first
        .is_none_or(|first| now - first > REINSTALL_WINDOW)
    {
        state.first = Some(now);
        state.attempts = 0;
    }
    state.attempts += 1;
    if state.attempts > REINSTALLS_BEFORE_GIVING_UP {
        state.given_up = true;
        // SAFETY: this thread's own hook; Windows has removed it, so this
        // fails harmlessly and ends Pane's claim on it.
        let _ = unsafe { UnhookWindowsHookEx(state.hook) };
        let why = format!(
            "Windows removed Pane's keyboard hook {} times within {} seconds; Pane gave up \
             reinstalling it, and the hotkeys it dispatches do nothing until Pane starts again",
            state.attempts,
            REINSTALL_WINDOW.as_secs()
        );
        crate::diagnostics::report_line(&why);
        update(&state.report, |report| {
            report.hook = 0;
            report.given_up = Some(why);
        });
        return;
    }
    // SAFETY: this thread's own hook; Windows has removed it, so this
    // fails harmlessly and a fresh one takes its place either way.
    let _ = unsafe { UnhookWindowsHookEx(state.hook) };
    match install() {
        Ok(hook) => {
            state.hook = hook;
            update(&state.report, |report| {
                report.reinstalled += 1;
                report.hook = hook.0 as usize;
            });
            // The state went stale while the hook was gone: re-read the
            // modifiers, as a resume does.
            resync(&mut state.recognizer);
        }
        Err(problem) => {
            crate::diagnostics::report_line(&format!(
                "Windows would not reinstall Pane's keyboard hook: {problem}"
            ));
        }
    }
}

/// Re-reads the recognizer of the hook thread's state.
fn resync_hooked() {
    let _ = HOOKED.try_with(|hooked| {
        let Ok(mut held) = hooked.try_borrow_mut() else {
            return;
        };
        if let Some(state) = held.as_mut() {
            resync(&mut state.recognizer);
        }
    });
}

/// Takes the modifiers' real state into the recognizer: for a session
/// unlock, a resume, a reinstall, or the re-read a contradictory event
/// asked for.
fn resync(recognizer: &mut Recognizer) {
    recognizer.resync(real());
}

/// The modifiers' real state, as the system reports it: either side of
/// each one held counts as held.
fn real() -> [bool; 4] {
    let down = |key: VIRTUAL_KEY| {
        // SAFETY: the call only reads that key's state.
        unsafe { (GetAsyncKeyState(key.0 as i32) as u16) & 0x8000 != 0 }
    };
    let either = |a: VIRTUAL_KEY, b: VIRTUAL_KEY| down(a) || down(b);
    [
        either(VK_LCONTROL, VK_RCONTROL),
        either(VK_LMENU, VK_RMENU),
        either(VK_LSHIFT, VK_RSHIFT),
        either(VK_LWIN, VK_RWIN),
    ]
}

/// Installs the low-level keyboard hook on this thread, which the system
/// calls from the thread's message loop.
fn install() -> Result<HHOOK, String> {
    // SAFETY: the procedure is this module's, kept for as long as the
    // process runs; a low-level hook needs no module handle and watches
    // every thread.
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hooked), None, 0) }
        .map_err(|error| error.message())
}

/// Registers the watchdog window for the raw input of the keyboard, so it
/// receives `WM_INPUT` for every key event whatever window has focus: the
/// evidence the hook is still seeing them.
fn watch_input(window: HWND) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        // The generic desktop page, and the keyboard on it.
        usUsagePage: 0x01,
        usUsage: 0x06,
        // Received without focus: the hook watches the whole desktop, and
        // so must the watchdog.
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: window,
    };
    // SAFETY: one device, registered for this window.
    unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        .map_err(|error| error.message())
}

/// Locks the pages the hook's callback needs in memory: those its code
/// and the recognizer's are on, and those of the state it reads, with a
/// page of margin — after raising the process's minimum working set by an
/// allowance for them, since a lock that would not fit it fails. If
/// Windows refuses, `Err` says the pages are not pinned.
fn pin(state: usize) -> Result<(), String> {
    raise_working_set(PIN_ALLOWANCE)?;
    // The callback and the recognizer's step are this module's code; the
    // pages around them, with a margin, are what runs on the hot path.
    let callback: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT = hooked;
    let step: fn(&mut Recognizer, KeyEvent) -> Decision = Recognizer::step;
    lock_pages(callback as usize, 4)
        .and_then(|_| lock_pages(step as usize, 4))
        .and_then(|_| lock_pages(state, 3))
}

/// Locks the `pages` pages `address` is in the middle of, for as long as
/// the process runs.
fn lock_pages(address: usize, pages: usize) -> Result<(), String> {
    let base = (address / PAGE) * PAGE;
    // SAFETY: the range is this module's code or the hook thread's own
    // state, kept for as long as the process runs.
    unsafe { VirtualLock(base as *const _, (pages + 1) * PAGE) }.map_err(|error| error.message())
}

/// Raises the process's minimum working set by `by`, so the pages the
/// hook pins fit in it.
fn raise_working_set(by: usize) -> Result<(), String> {
    // SAFETY: the calling process's own pseudo-handle.
    let process = unsafe { GetCurrentProcess() };
    let (mut minimum, mut maximum, mut flags) = (0, 0, 0);
    // SAFETY: the sizes are writable out-parameters it reads.
    let read =
        unsafe { GetProcessWorkingSetSizeEx(process, &mut minimum, &mut maximum, &mut flags) };
    if !read.as_bool() {
        return Err("Windows would not say the working set".into());
    }
    // SAFETY: the sizes it read, raised by the allowance.
    let raised = unsafe {
        SetProcessWorkingSetSizeEx(
            process,
            minimum + by,
            maximum + by,
            SETPROCESSWORKINGSETSIZEEX_FLAGS(flags),
        )
    };
    raised.map_err(|error| error.message())
}
