//! The clipboard on Windows: a clipboard format listener
//! (`AddClipboardFormatListener`) on a message-only window of a thread of
//! Pane's own, which receives `WM_CLIPBOARDUPDATE` after every change of the
//! clipboard, whichever application made it. No permission is needed.
//!
//! On each change the thread opens the clipboard and reads, in this order:
//! the formats applications use to say that a clipboard monitor or
//! clipboard history must not keep what they copied
//! (`ExcludeClipboardContentFromMonitorProcessing`, the older `Clipboard
//! Viewer Ignore`, `CanIncludeInClipboardHistory` and
//! `CanUploadToCloudClipboard` as a DWORD of 0); then, only if none of them
//! forbids it, the text (`CF_UNICODETEXT`); and the file name of the
//! process whose window owns the clipboard. What is kept is decided by
//! `clipboard::accept`, the same on every system.
//!
//! Dropping the watch ends the thread: it stops listening and destroys its
//! window before the drop returns.

use std::cell::RefCell;
use std::sync::Once;
use std::sync::mpsc;
use std::time::Duration;

use ::windows::Win32::Foundation::{
    CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM,
};
use ::windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardOwner, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use ::windows::Win32::System::Threading::{
    GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowThreadProcessId, HWND_MESSAGE, MSG, PostThreadMessageW, RegisterClassW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLIPBOARDUPDATE, WNDCLASSW,
};
use ::windows::core::{PCWSTR, PWSTR, w};

use super::{ClipboardSystem, Content, MAX_TEXT_BYTES, Markers, Observation, Sink, Watch};

/// `CF_UNICODETEXT`: text as UTF-16, ending with a NUL.
const CF_UNICODETEXT: u32 = 13;

/// Ends the listener thread.
const WM_STOP: u32 = WM_APP + 2;

/// The class of the listener's message-only window.
const CLASS: PCWSTR = w!("PaneClipboardListener");

/// How often, and how long apart, Pane tries to open the clipboard while
/// another application holds it open.
const OPEN_TRIES: u32 = 10;
const OPEN_WAIT: Duration = Duration::from_millis(20);

/// The system's clipboard on Windows.
pub struct WindowsClipboard;

impl ClipboardSystem for WindowsClipboard {
    fn unavailable(&self) -> Option<String> {
        None
    }

    fn watch(&self, sink: Sink) -> Result<Watch, String> {
        let (started, thread) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("pane-clipboard".into())
            .spawn(move || listen(sink, &started))
            .map_err(|error| error.to_string())?;
        match thread.recv() {
            Ok(Ok(thread)) => Ok(Box::new(Listening {
                thread,
                handle: Some(handle),
            })),
            Ok(Err(problem)) => {
                let _ = handle.join();
                Err(problem)
            }
            Err(_) => Err("the clipboard thread did not start".into()),
        }
    }

    fn write_text(&self, text: &str) -> Result<(), String> {
        let _open = Open::clipboard(None)?;
        // SAFETY: the clipboard is open by this thread.
        unsafe { EmptyClipboard() }.map_err(|error| error.message())?;
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        put(CF_UNICODETEXT, bytes_of(&units))
    }
}

/// The listener thread, until dropped.
struct Listening {
    thread: u32,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Listening {
    fn drop(&mut self) {
        // SAFETY: posting a message with no pointers to a thread id.
        let posted = unsafe { PostThreadMessageW(self.thread, WM_STOP, WPARAM(0), LPARAM(0)) };
        if posted.is_ok()
            && let Some(handle) = self.handle.take()
        {
            let _ = handle.join();
        }
    }
}

/// What the listener's window procedure uses, on the listener thread.
struct Listener {
    sink: Sink,
    formats: Formats,
    /// The clipboard's sequence number when it was last read, so a change
    /// reported twice is read once.
    last: u32,
}

thread_local! {
    static LISTENER: RefCell<Option<Listener>> = const { RefCell::new(None) };
}

/// The registered formats that carry an application's markers.
#[derive(Clone, Copy)]
struct Formats {
    exclude: u32,
    viewer_ignore: u32,
    history: u32,
    cloud: u32,
}

impl Formats {
    fn register() -> Formats {
        // SAFETY: each name is a valid NUL-terminated wide string.
        unsafe {
            Formats {
                exclude: RegisterClipboardFormatW(w!(
                    "ExcludeClipboardContentFromMonitorProcessing"
                )),
                viewer_ignore: RegisterClipboardFormatW(w!("Clipboard Viewer Ignore")),
                history: RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
                cloud: RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
            }
        }
    }
}

/// Registers the listener's window class, once per process.
fn register_class() -> Result<(), String> {
    static REGISTERED: Once = Once::new();
    let mut problem = None;
    REGISTERED.call_once(|| {
        // SAFETY: no arguments; the module is this process's executable.
        let instance = match unsafe { GetModuleHandleW(PCWSTR::null()) } {
            Ok(instance) => instance,
            Err(error) => {
                problem = Some(error.message());
                return;
            }
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_procedure),
            hInstance: instance.into(),
            lpszClassName: CLASS,
            ..WNDCLASSW::default()
        };
        // SAFETY: `class` is fully initialized and its strings are static.
        if unsafe { RegisterClassW(&class) } == 0 {
            problem = Some(::windows::core::Error::from_thread().message());
        }
    });
    match problem {
        Some(problem) => Err(format!("Pane could not watch the clipboard: {problem}")),
        None => Ok(()),
    }
}

/// A message-only window of the listener's class, on the calling thread.
fn message_window() -> Result<HWND, String> {
    register_class()?;
    // SAFETY: no arguments; the module is this process's executable.
    let instance = unsafe { GetModuleHandleW(PCWSTR::null()) }.map_err(|error| error.message())?;
    // SAFETY: the class is registered and its name is static; a
    // message-only window has no size, menu or creation data.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            w!("Pane clipboard"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|error| format!("Pane could not watch the clipboard: {}", error.message()))
}

/// The listener thread: listens to clipboard changes, reporting each to
/// `sink`, until told to stop.
fn listen(sink: Sink, started: &mpsc::Sender<Result<u32, String>>) {
    let window = match message_window() {
        Ok(window) => window,
        Err(problem) => {
            let _ = started.send(Err(problem));
            return;
        }
    };
    // SAFETY: `window` is this thread's own window.
    if let Err(error) = unsafe { AddClipboardFormatListener(window) } {
        // SAFETY: as above.
        let _ = unsafe { DestroyWindow(window) };
        let _ = started.send(Err(format!(
            "Pane could not watch the clipboard: {}",
            error.message()
        )));
        return;
    }
    LISTENER.with(|listener| {
        *listener.borrow_mut() = Some(Listener {
            sink,
            formats: Formats::register(),
            // SAFETY: no arguments.
            last: unsafe { GetClipboardSequenceNumber() },
        });
    });
    // SAFETY: no arguments; it only reads the calling thread's id.
    let thread = unsafe { GetCurrentThreadId() };
    if started.send(Ok(thread)).is_ok() {
        let mut message = MSG::default();
        loop {
            // SAFETY: `message` is a valid, writable MSG for the call's
            // duration; with no window it receives all of this thread's
            // messages.
            if unsafe { GetMessageW(&mut message, None, 0, 0) }.0 <= 0 {
                break;
            }
            if message.hwnd.is_invalid() && message.message == WM_STOP {
                break;
            }
            // SAFETY: a message this thread's queue returned.
            unsafe { DispatchMessageW(&message) };
        }
    }
    // SAFETY: `window` is this thread's own window, still listening.
    unsafe {
        let _ = RemoveClipboardFormatListener(window);
        let _ = DestroyWindow(window);
    }
    LISTENER.with(|listener| listener.borrow_mut().take());
}

extern "system" fn window_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLIPBOARDUPDATE {
        LISTENER.with(|listener| {
            if let Some(listener) = listener.borrow_mut().as_mut() {
                // SAFETY: no arguments.
                let sequence = unsafe { GetClipboardSequenceNumber() };
                if sequence == listener.last {
                    return;
                }
                listener.last = sequence;
                match read(window, listener.formats) {
                    Ok(observation) => (listener.sink)(observation),
                    // Never what was copied: only that it was not read.
                    Err(problem) => eprintln!("Pane could not read the clipboard: {problem}"),
                }
            }
        });
        return LRESULT(0);
    }
    // SAFETY: the arguments are those this procedure was called with.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// The clipboard, open by this thread until dropped.
struct Open;

impl Open {
    /// Opens the clipboard for `owner` (none: the calling task), trying
    /// again for a moment while another application holds it open.
    fn clipboard(owner: Option<HWND>) -> Result<Open, String> {
        let mut last = String::new();
        for _ in 0..OPEN_TRIES {
            // SAFETY: `owner` is a window of this thread, or none.
            match unsafe { OpenClipboard(owner) } {
                Ok(()) => return Ok(Open),
                Err(error) => last = error.message(),
            }
            std::thread::sleep(OPEN_WAIT);
        }
        Err(format!(
            "another application keeps the clipboard open ({last})"
        ))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: the clipboard is open by this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Whether `format` is on the clipboard.
fn available(format: u32) -> bool {
    // SAFETY: a plain value.
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// The bytes of `format`'s global memory on the open clipboard, at most
/// `limit` of them, if it has any.
fn bytes(format: u32, limit: usize) -> Option<Vec<u8>> {
    if !available(format) {
        return None;
    }
    // SAFETY: the clipboard is open; the handle stays the clipboard's.
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let memory = HGLOBAL(handle.0);
    // SAFETY: a clipboard format with global memory, locked while read and
    // read within its size.
    unsafe {
        let size = GlobalSize(memory).min(limit);
        let data = GlobalLock(memory);
        if data.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
        let _ = GlobalUnlock(memory);
        Some(bytes)
    }
}

/// A DWORD format as a yes (not 0) or no (0), if it is on the clipboard.
fn flag(format: u32) -> Option<bool> {
    let bytes = bytes(format, 4)?;
    let value: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(value) != 0)
}

/// Reads the open clipboard's markers, then its text only if they allow
/// it, and its owner.
fn read(window: HWND, formats: Formats) -> Result<Observation, String> {
    let _open = Open::clipboard(Some(window))?;
    let markers = Markers {
        exclude_from_monitoring: available(formats.exclude) || available(formats.viewer_ignore),
        include_in_history: flag(formats.history),
        upload_to_cloud: flag(formats.cloud),
    };
    let content = if !markers.allow() {
        Content::Withheld
    } else {
        // One unit more than Pane keeps is enough to know it is too long:
        // UTF-8 never has fewer bytes than UTF-16 has units.
        match bytes(CF_UNICODETEXT, (MAX_TEXT_BYTES + 1) * 2) {
            Some(bytes) => {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .take_while(|unit| *unit != 0)
                    .collect();
                Content::Text(String::from_utf16_lossy(&units))
            }
            None => Content::Other,
        }
    };
    Ok(Observation {
        content,
        markers,
        source: owner_program(),
    })
}

/// The file name of the process whose window owns the clipboard, if the
/// system says.
fn owner_program() -> Option<String> {
    // SAFETY: no arguments.
    let owner = unsafe { GetClipboardOwner() }.ok()?;
    if owner.is_invalid() {
        return None;
    }
    let mut process = 0u32;
    // SAFETY: `owner` is a window handle; `process` is writable.
    unsafe { GetWindowThreadProcessId(owner, Some(&raw mut process)) };
    if process == 0 {
        return None;
    }
    // SAFETY: plain values; the handle is closed below.
    let handle: HANDLE =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) }.ok()?;
    let mut name = [0u16; 1024];
    let mut length = name.len() as u32;
    // SAFETY: `name` is writable for `length` units.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: opened above.
    let _ = unsafe { CloseHandle(handle) };
    queried.ok()?;
    let path = String::from_utf16_lossy(&name[..length as usize]);
    path.rsplit(['\\', '/']).next().map(str::to_owned)
}

fn bytes_of(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

/// Puts `bytes` on the open, emptied clipboard as `format`.
fn put(format: u32, bytes: Vec<u8>) -> Result<(), String> {
    // SAFETY: a new moveable block of at least one byte, written within its
    // size while locked; the clipboard owns it once set, and it is freed
    // here otherwise.
    unsafe {
        let memory =
            GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|error| error.message())?;
        let data = GlobalLock(memory);
        if data.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err("could not lock the clipboard's memory".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(memory);
        if let Err(error) = SetClipboardData(format, Some(HANDLE(memory.0))) {
            let _ = GlobalFree(Some(memory));
            return Err(error.message());
        }
    }
    Ok(())
}

/// For the Windows tests: putting text with markers on the clipboard, as a
/// password manager does, from a window of this process, and saving and
/// restoring what was on it, so that a test leaves the user's clipboard as
/// it was. The saved contents are held in memory only.
#[doc(hidden)]
pub mod testing {
    use super::*;
    use ::windows::Win32::System::DataExchange::EnumClipboardFormats;

    /// A window of this process that owns the clipboard, until dropped.
    pub struct Owner(HWND);

    impl Drop for Owner {
        fn drop(&mut self) {
            // SAFETY: this thread's own window.
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    /// Puts `text` on the clipboard with each of `markers`, a registered
    /// format's name and its DWORD value, owned by a window of this process
    /// while the returned owner is kept.
    pub fn set_text(text: &str, markers: &[(&str, u32)]) -> Result<Owner, String> {
        let owner = Owner(message_window()?);
        let window = owner.0;
        (|| -> Result<(), String> {
            let _open = Open::clipboard(Some(window))?;
            // SAFETY: the clipboard is open by this thread.
            unsafe { EmptyClipboard() }.map_err(|error| error.message())?;
            let mut units: Vec<u16> = text.encode_utf16().collect();
            units.push(0);
            put(CF_UNICODETEXT, bytes_of(&units))?;
            for (name, value) in markers {
                let mut wide: Vec<u16> = name.encode_utf16().collect();
                wide.push(0);
                // SAFETY: `wide` is NUL-terminated and outlives the call.
                let format = unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) };
                put(format, value.to_le_bytes().to_vec())?;
            }
            Ok(())
        })()?;
        Ok(owner)
    }

    /// What was on the clipboard, format by format, in memory only.
    pub struct Saved(Vec<(u32, Vec<u8>)>);

    /// Formats whose data is a GDI handle, not global memory; not saved.
    fn gdi(format: u32) -> bool {
        matches!(format, 2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E)
            || (0x300..=0x3FF).contains(&format)
    }

    /// Saves every format on the clipboard held in global memory. Bitmaps
    /// are saved as their device-independent form, which Windows provides
    /// alongside.
    pub fn save() -> Result<Saved, String> {
        let _open = Open::clipboard(None)?;
        let mut saved = Vec::new();
        let mut format = 0;
        loop {
            // SAFETY: the clipboard is open by this thread.
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            if gdi(format) {
                continue;
            }
            if let Some(bytes) = bytes(format, usize::MAX) {
                saved.push((format, bytes));
            }
        }
        Ok(Saved(saved))
    }

    impl Saved {
        /// Puts back what [`save`] saved, emptying the clipboard first.
        pub fn restore(self) -> Result<(), String> {
            let _open = Open::clipboard(None)?;
            // SAFETY: the clipboard is open by this thread.
            unsafe { EmptyClipboard() }.map_err(|error| error.message())?;
            for (format, bytes) in self.0 {
                put(format, bytes)?;
            }
            Ok(())
        }
    }
}
