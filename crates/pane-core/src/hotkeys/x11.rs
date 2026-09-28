//! Global hotkeys on X11: a passive grab of each shortcut's key on the root
//! window (`XGrabKey`), so the X server sends its presses to Pane whichever
//! window has focus. A shortcut another client has grabbed already is
//! refused by the server (`BadAccess`), which is how a conflict with another
//! application shows. X11 needs no permission for this.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, KeyButMask, Keycode, ModMask, Window};
use x11rb::protocol::{ErrorKind, Event};
use x11rb::rust_connection::RustConnection;

use super::{HotkeyError, Hotkeys, PressSender, Shortcut};

/// The adapter for one X11 display.
pub struct X11Hotkeys {
    connection: Arc<RustConnection>,
    root: Window,
    /// The grabbed shortcuts by their key and modifiers.
    grabs: Arc<Mutex<HashMap<(Keycode, u16), Grab>>>,
}

struct Grab {
    shortcut: Shortcut,
    /// Whether its key is held, so that holding it reports one press.
    held: bool,
}

/// The modifiers that decide a shortcut; Caps Lock and Num Lock do not.
fn modifiers(shortcut: &Shortcut) -> u16 {
    let mut mask = 0u16;
    if shortcut.shift() {
        mask |= u16::from(ModMask::SHIFT);
    }
    if shortcut.control() {
        mask |= u16::from(ModMask::CONTROL);
    }
    if shortcut.alt() {
        mask |= u16::from(ModMask::M1);
    }
    if shortcut.super_key() {
        mask |= u16::from(ModMask::M4);
    }
    mask
}

/// Caps Lock and Num Lock (Mod2) combinations: X11 matches a grab's
/// modifiers exactly, so each shortcut is grabbed with each of them too.
fn lock_variants() -> [u16; 4] {
    let lock = u16::from(ModMask::LOCK);
    let num = u16::from(ModMask::M2);
    [0, lock, num, lock | num]
}

/// The X keysym of `key` (see `Shortcut::key`).
fn keysym(key: &str) -> Option<u32> {
    match key.as_bytes() {
        [c @ (b'a'..=b'z' | b'0'..=b'9')] => Some(u32::from(*c)),
        _ if key == "space" => Some(0x20),
        _ => {
            let number: u32 = key.strip_prefix('f')?.parse().ok()?;
            // XK_F1 is 0xffbe.
            (1..=12).contains(&number).then(|| 0xffbe + number - 1)
        }
    }
}

impl X11Hotkeys {
    /// Connects to the X11 display `display` (such as `:0`) and starts the
    /// thread that receives the grabbed keys' presses.
    pub fn connect(display: &str, presses: PressSender) -> Result<X11Hotkeys, String> {
        let (connection, screen) =
            RustConnection::connect(Some(display)).map_err(|error| error.to_string())?;
        // Holding a key must not report it again and again: without this
        // the server repeats a press and a release while it is held.
        connection
            .xkb_use_extension(1, 0)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        connection
            .xkb_per_client_flags(
                xkb::ID::USE_CORE_KBD.into(),
                xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
                xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
                Default::default(),
                Default::default(),
                Default::default(),
            )
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        let connection = Arc::new(connection);
        let grabs: Arc<Mutex<HashMap<(Keycode, u16), Grab>>> = Arc::default();
        let events = connection.clone();
        let pressed = grabs.clone();
        std::thread::Builder::new()
            .name("pane-hotkeys".into())
            .spawn(move || receive(&events, &pressed, &presses))
            .map_err(|error| error.to_string())?;
        Ok(X11Hotkeys {
            connection,
            root,
            grabs,
        })
    }

    /// The key code of `shortcut`'s key on the current keyboard layout.
    fn keycode(&self, shortcut: &Shortcut) -> Result<Keycode, HotkeyError> {
        let refused = |reason: String| HotkeyError::Refused(reason);
        let wanted =
            keysym(shortcut.key()).ok_or_else(|| refused(format!("{shortcut} has no X11 key")))?;
        let setup = self.connection.setup();
        let (first, last) = (setup.min_keycode, setup.max_keycode);
        let mapping = self
            .connection
            .get_keyboard_mapping(first, last - first + 1)
            .map_err(|error| refused(error.to_string()))?
            .reply()
            .map_err(|error| refused(error.to_string()))?;
        let per_key = usize::from(mapping.keysyms_per_keycode.max(1));
        mapping
            .keysyms
            .chunks(per_key)
            .position(|keysyms| keysyms.contains(&wanted))
            .map(|index| first + index as u8)
            .ok_or_else(|| refused(format!("the keyboard layout has no key for {shortcut}")))
    }

    fn ungrab(&self, keycode: Keycode, modifiers: u16) {
        for lock in lock_variants() {
            if let Ok(cookie) =
                self.connection
                    .ungrab_key(keycode, self.root, ModMask::from(modifiers | lock))
            {
                cookie.ignore_error();
            }
        }
        let _ = self.connection.flush();
    }
}

impl Hotkeys for X11Hotkeys {
    fn unavailable(&self) -> Option<String> {
        None
    }

    fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError> {
        let keycode = self.keycode(shortcut)?;
        let modifiers = modifiers(shortcut);
        let mut grabs = self.grabs.lock().unwrap_or_else(|p| p.into_inner());
        if grabs.contains_key(&(keycode, modifiers)) {
            return Ok(());
        }
        for (done, lock) in lock_variants().into_iter().enumerate() {
            let checked = self
                .connection
                .grab_key(
                    false,
                    self.root,
                    ModMask::from(modifiers | lock),
                    keycode,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )
                .map_err(ReplyError::from)
                .and_then(|cookie| cookie.check());
            if let Err(error) = checked {
                // Release the variants grabbed before this one.
                for lock in lock_variants().into_iter().take(done) {
                    if let Ok(cookie) = self.connection.ungrab_key(
                        keycode,
                        self.root,
                        ModMask::from(modifiers | lock),
                    ) {
                        cookie.ignore_error();
                    }
                }
                let _ = self.connection.flush();
                return Err(match error {
                    ReplyError::X11Error(error) if error.error_kind == ErrorKind::Access => {
                        HotkeyError::Taken
                    }
                    error => HotkeyError::Refused(error.to_string()),
                });
            }
        }
        grabs.insert(
            (keycode, modifiers),
            Grab {
                shortcut: shortcut.clone(),
                held: false,
            },
        );
        Ok(())
    }

    fn unregister(&self, shortcut: &Shortcut) {
        let Ok(keycode) = self.keycode(shortcut) else {
            return;
        };
        let modifiers = modifiers(shortcut);
        let mut grabs = self.grabs.lock().unwrap_or_else(|p| p.into_inner());
        if grabs.remove(&(keycode, modifiers)).is_some() {
            self.ungrab(keycode, modifiers);
        }
    }
}

/// Reports each press of a grabbed shortcut until the connection closes.
fn receive(
    connection: &RustConnection,
    grabs: &Mutex<HashMap<(Keycode, u16), Grab>>,
    presses: &PressSender,
) {
    let decisive =
        u16::from(KeyButMask::SHIFT | KeyButMask::CONTROL | KeyButMask::MOD1 | KeyButMask::MOD4);
    while let Ok(event) = connection.wait_for_event() {
        let mut grabs = grabs.lock().unwrap_or_else(|p| p.into_inner());
        match event {
            Event::KeyPress(event) => {
                let key = (event.detail, u16::from(event.state) & decisive);
                if let Some(grab) = grabs.get_mut(&key)
                    && !grab.held
                {
                    grab.held = true;
                    presses.send(grab.shortcut.clone());
                }
            }
            Event::KeyRelease(event) => {
                for ((keycode, _), grab) in grabs.iter_mut() {
                    if *keycode == event.detail {
                        grab.held = false;
                    }
                }
            }
            _ => {}
        }
    }
}
