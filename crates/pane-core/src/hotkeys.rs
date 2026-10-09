//! Global hotkeys: a shortcut the user assigns to an installed command,
//! which opens that command in Pane's window even while another application
//! has focus.
//!
//! The launcher decides which shortcuts are bound and which should be
//! registered; the system is reached through one small trait, [`Hotkeys`],
//! with one adapter per system, chosen by [`native`]:
//!
//! - Windows: `RegisterHotKey` on a thread of Pane's own for the chords
//!   Windows accepts, and a low-level keyboard hook of Pane's own
//!   (`WH_KEYBOARD_LL`) for the chords Windows refuses — another
//!   application has the shortcut, or Windows keeps it — so a refused
//!   shortcut is never an error ([`windows`], ADR 0039);
//! - macOS: Carbon's `RegisterEventHotKey`, through the `global-hotkey`
//!   crate, which needs no Accessibility permission ([`macos`]);
//! - Linux on X11: a passive key grab on the root window (`XGrabKey`)
//!   ([`X11Hotkeys`]). Wayland has no way for an application to watch keys
//!   pressed in other applications except the desktop portal's global
//!   shortcuts, which Pane does not use yet, so there the adapter explains
//!   that hotkeys are unavailable.
//!
//! An adapter reports each press of a registered shortcut to a
//! [`PressSender`]; the window reads them from the matching [`Presses`] and
//! asks the launcher to open the bound command. How a shortcut is
//! dispatched — the system's registration or Pane's keyboard hook — is the
//! adapter's to say ([`Hotkeys::route`]), and the binding's row shows it
//! ([`Route::note_on`]). The hook's own state is the adapter's to say
//! too ([`Hotkeys::hook_health`]), and the Settings pages and Copy
//! Diagnostics show it ([`HookHealth::note`]).

use std::fmt;
use std::sync::Arc;

use crate::platform::Platform;

/// The tag Pane puts in the extra information of every key it injects
/// (SendInput's `dwExtraInfo`): "PANE". Pane's keyboard hook passes
/// tagged events through untouched, so Pane never reacts to its own
/// input — the Start-menu mask, a paste, a simulated copy — and the tools
/// that remap keys can tell Pane's injections from the user's. Keys
/// another tool injects carry that tool's marker, or none.
pub const INJECTED_TAG: usize = 0x50414E45;

/// The binding recognizer: the pure state machine that decides, from the
/// key events the Windows hook sees, whether one of the bound chords was
/// pressed. Compiled on every system, so its logic is tested everywhere.
mod recognizer;
pub use recognizer::{Decision, KeyEvent, Recognizer};

#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use x11::X11Hotkeys;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsHotkeys;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacHotkeys;

/// The keys a hotkey can end with.
const FUNCTION_KEYS: u8 = 12;

/// A key combination: modifier keys held while one other key is pressed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    control: bool,
    alt: bool,
    shift: bool,
    /// The Windows key, Command on macOS, Super on Linux.
    super_key: bool,
    key: String,
}

impl Shortcut {
    /// The shortcut of the modifiers held and `key`, as the window reports a
    /// key press: `a` to `z`, `0` to `9`, `f1` to `f12` or `space`, in any
    /// case. Another key is explained.
    pub fn new(
        control: bool,
        alt: bool,
        shift: bool,
        super_key: bool,
        key: &str,
    ) -> Result<Shortcut, String> {
        let key = key.to_ascii_lowercase();
        // Ctrl+Alt+Delete, the one shortcut the key set cannot express
        // that is refused for a reason of its own: Windows keeps it on
        // the secure screen, and no program can take it. On any system,
        // so the reason is tested everywhere.
        if key == "delete" && control && alt && !shift && !super_key {
            return Err("Ctrl+Alt+Delete is reserved: no program can intercept it".into());
        }
        let supported = match key.as_bytes() {
            [b'a'..=b'z' | b'0'..=b'9'] => true,
            _ => key == "space" || (1..=FUNCTION_KEYS).any(|number| key == format!("f{number}")),
        };
        if !supported {
            return Err(format!(
                "Pane cannot use {} in a hotkey: end it with a letter, a digit, F1 to F12 or Space",
                key_name(&key)
            ));
        }
        Ok(Shortcut {
            control,
            alt,
            shift,
            super_key,
            key,
        })
    }

    /// The provisional Open Pane default: Ctrl+Alt+Space on Windows and
    /// Linux, Option+Space on macOS. The parent specification records it
    /// as a synthesis default, not a separately confirmed product
    /// decision: it stays clear of the combinations each system keeps for
    /// itself (the Windows key, Spotlight, the window menu) and of plain
    /// typing, and the General page can change it.
    pub fn open_pane_default() -> Shortcut {
        if cfg!(target_os = "macos") {
            Shortcut::parse("alt+space").expect("a valid default")
        } else {
            Shortcut::parse("ctrl+alt+space").expect("a valid default")
        }
    }

    /// Reads a shortcut as [`Shortcut::id`] writes it, such as
    /// `ctrl+alt+p`.
    pub fn parse(text: &str) -> Result<Shortcut, String> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts.pop().filter(|key| !key.is_empty());
        let Some(key) = key else {
            return Err(format!("“{text}” names no key"));
        };
        let (mut control, mut alt, mut shift, mut super_key) = (false, false, false, false);
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => control = true,
                "alt" | "option" => alt = true,
                "shift" => shift = true,
                "super" | "win" | "cmd" | "command" => super_key = true,
                other => return Err(format!("“{text}”: {other} is not a modifier key")),
            }
        }
        Shortcut::new(control, alt, shift, super_key, key)
    }

    /// The same text on every system, for records: the modifiers in a fixed
    /// order, then the key, such as `ctrl+alt+p`.
    pub fn id(&self) -> String {
        let mut parts = Vec::new();
        for (held, name) in self.modifiers() {
            if held {
                parts.push(name);
            }
        }
        parts.push(&self.key);
        parts.join("+")
    }

    pub fn control(&self) -> bool {
        self.control
    }

    pub fn alt(&self) -> bool {
        self.alt
    }

    pub fn shift(&self) -> bool {
        self.shift
    }

    /// Whether the Windows key (Windows), Command (macOS) or Super (Linux)
    /// is held.
    pub fn super_key(&self) -> bool {
        self.super_key
    }

    /// `a` to `z`, `0` to `9`, `f1` to `f12` or `space`.
    pub fn key(&self) -> &str {
        &self.key
    }

    fn modifiers(&self) -> [(bool, &'static str); 4] {
        [
            (self.control, "ctrl"),
            (self.alt, "alt"),
            (self.shift, "shift"),
            (self.super_key, "super"),
        ]
    }

    /// Why Pane does not bind this shortcut on this system, if it does not:
    /// it has no Ctrl, Alt or Super (Command) key, so it would take over
    /// typing, or no program can intercept it (Win+L on Windows,
    /// Ctrl+Alt+Delete everywhere). On Windows a shortcut the system keeps
    /// for itself is no longer refused: Pane's own keyboard hook takes it
    /// instead and its row warns what Windows does with it (#252,
    /// ADR 0039).
    pub fn refusal(&self) -> Option<String> {
        self.refusal_on(Platform::current())
    }

    /// Why Pane does not bind this shortcut on `platform`, if it does
    /// not; the rules are named by platform so every system's are tested
    /// on every system.
    fn refusal_on(&self, platform: Option<Platform>) -> Option<String> {
        if !(self.control || self.alt || self.super_key) {
            let modifiers = match platform {
                Some(Platform::Macos) => "Control, Option or Command",
                Some(Platform::Windows) => "Ctrl, Alt or the Windows key",
                _ => "Ctrl, Alt or Super",
            };
            return Some(format!(
                "{self} needs {modifiers}, so that it does not take over typing"
            ));
        }
        reserved(platform)
            .iter()
            .find(|(id, _)| Shortcut::parse(id).is_ok_and(|reserved| reserved == *self))
            .map(|(_, why)| format!("{self} is reserved: {why}"))
            .or_else(|| self.editing_refusal(platform))
    }

    /// What the system does with this shortcut while Pane does not take
    /// it first, where `platform` keeps it for itself and Pane's own
    /// keyboard hook can take it (Windows, #252): the warning a
    /// hook-dispatched binding's row shows beside its route. `None` where
    /// the system does not keep it.
    fn kept_by(&self, platform: Option<Platform>) -> Option<&'static str> {
        kept(platform)
            .iter()
            .find(|(id, _)| Shortcut::parse(id).is_ok_and(|kept| kept == *self))
            .map(|(_, why)| *why)
    }

    /// Ctrl (Command on macOS) with only a letter or digit: every
    /// application uses those for its own commands, such as copying.
    fn editing_refusal(&self, platform: Option<Platform>) -> Option<String> {
        let primary = if platform == Some(Platform::Macos) {
            self.super_key && !self.control
        } else {
            self.control && !self.super_key
        };
        let single = primary && !self.alt && !self.shift;
        let character = self.key.len() == 1;
        (single && character).then(|| {
            format!(
                "{self} is used by applications for their own commands, such as copying; add Alt \
                 or Shift"
            )
        })
    }
}

/// Shortcuts no program can intercept, which stay refused, each with why,
/// on `platform`.
fn reserved(platform: Option<Platform>) -> &'static [(&'static str, &'static str)] {
    match platform {
        Some(Platform::Windows) => &[(
            "super+l",
            "Windows locks the computer with it, and no program can intercept it",
        )],
        Some(Platform::Macos) => &[
            ("super+space", "macOS opens Spotlight with it"),
            ("ctrl+space", "macOS switches input sources with it"),
            ("super+alt+space", "macOS opens a Finder search with it"),
            ("shift+super+3", "macOS takes screenshots with it"),
            ("shift+super+4", "macOS takes screenshots with it"),
            ("shift+super+5", "macOS takes screenshots with it"),
            ("ctrl+super+q", "macOS locks the screen with it"),
        ],
        _ => &[
            ("alt+f4", "desktops close the active window with it"),
            ("super+l", "desktops lock the screen with it"),
            ("ctrl+alt+f1", "Linux switches to another console with it"),
            ("ctrl+alt+f2", "Linux switches to another console with it"),
            ("ctrl+alt+f3", "Linux switches to another console with it"),
            ("ctrl+alt+f4", "Linux switches to another console with it"),
            ("ctrl+alt+f5", "Linux switches to another console with it"),
            ("ctrl+alt+f6", "Linux switches to another console with it"),
            ("ctrl+alt+f7", "Linux switches to another console with it"),
            ("ctrl+alt+f8", "Linux switches to another console with it"),
            ("ctrl+alt+f9", "Linux switches to another console with it"),
            ("ctrl+alt+f10", "Linux switches to another console with it"),
            ("ctrl+alt+f11", "Linux switches to another console with it"),
            ("ctrl+alt+f12", "Linux switches to another console with it"),
        ],
    }
}

/// Shortcuts the system keeps for itself that Pane still takes through
/// its own keyboard hook (Windows, #252), each with what the system does
/// with the shortcut while Pane does not take it first — the warning a
/// hook-dispatched binding's row shows. Windows refuses to register these
/// for any application, but the hook recognizes the chord all the same,
/// so they are warnings rather than refusals. Empty everywhere but
/// Windows: the other systems' adapters have no hook, so their kept
/// shortcuts stay refused (see [`reserved`]).
fn kept(platform: Option<Platform>) -> &'static [(&'static str, &'static str)] {
    match platform {
        Some(Platform::Windows) => &[
            ("alt+f4", "Windows closes the active window with it"),
            ("super+d", "Windows shows the desktop with it"),
            ("super+e", "Windows opens File Explorer with it"),
            ("super+r", "Windows opens Run with it"),
            ("alt+space", "Windows opens the window menu with it"),
        ],
        _ => &[],
    }
}

/// People's name for `key`: `P`, `5`, `F5`, `Space`.
fn key_name(key: &str) -> String {
    match key {
        "space" => "Space".into(),
        key if key.starts_with('f') && key.len() > 1 => key.to_ascii_uppercase(),
        key => {
            let mut chars = key.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

/// People's names on this system: "Ctrl+Alt+P" on Linux and Windows (with
/// "Super" or "Win"), "Control+Option+Command+P" on macOS.
impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: [&str; 4] = match Platform::current() {
            Some(Platform::Macos) => ["Control", "Option", "Shift", "Command"],
            Some(Platform::Windows) => ["Ctrl", "Alt", "Shift", "Win"],
            _ => ["Ctrl", "Alt", "Shift", "Super"],
        };
        for ((held, _), name) in self.modifiers().into_iter().zip(names) {
            if held {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(&self.key))
    }
}

/// Why the system did not register a shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyError {
    /// Another application, or the system, already uses it.
    Taken,
    /// The system refused it, with its reason.
    Refused(String),
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotkeyError::Taken => f.write_str("another application or the system already uses it"),
            HotkeyError::Refused(reason) => write!(f, "the system refused it: {reason}"),
        }
    }
}

/// Registers shortcuts with the system, so that pressing one is reported
/// even while another application has focus. Presses go to the
/// [`PressSender`] the adapter was made with.
///
/// The launcher calls it on the window's thread (macOS registers hotkeys
/// with the main run loop). Pane keeps one adapter for the whole process;
/// dropping a system adapter releases every shortcut it registered and
/// stops its thread (Windows, X11), so tests and a replaced adapter leave no
/// registration behind.
pub trait Hotkeys: Send + Sync + 'static {
    /// Why global hotkeys cannot be used here at all, such as on Wayland;
    /// `None` when they can.
    fn unavailable(&self) -> Option<String>;

    /// Registers `shortcut` system-wide. On Windows a registration the
    /// system refuses — another application has the shortcut, or Windows
    /// keeps it — is not an error: the adapter falls back to its own
    /// keyboard hook and answers `Ok` (ADR 0039), so only what no adapter
    /// can take is refused.
    fn register(&self, shortcut: &Shortcut) -> Result<(), HotkeyError>;

    /// Releases `shortcut`, registered earlier, so that other applications
    /// may use it and it is no longer reported.
    fn unregister(&self, shortcut: &Shortcut);

    // The rest is the seam the keyboard hook fallback adds (#252): until
    // an adapter answers them, each says the system's registration and
    // that no hook is in use.

    /// How `shortcut`, registered earlier, is dispatched: through the
    /// system's own registration, or through Pane's own keyboard hook,
    /// where the system refused the registration or cannot express the
    /// binding (Windows, #252). Asked once a registration succeeded, so
    /// the binding's row can say which — [`Route::note_on`].
    fn route(&self, _shortcut: &Shortcut) -> Route {
        Route::System
    }

    /// The state of Pane's own keyboard hook, where this system's adapter
    /// uses one; `None` where none is, which is everywhere but Windows'
    /// adapter with a binding the system refused. The Settings pages and
    /// Copy Diagnostics show it (#259).
    fn hook_health(&self) -> Option<HookHealth> {
        None
    }
}

/// How a registered shortcut is dispatched: through the system's own
/// registration, or through Pane's own low-level keyboard hook, where the
/// system refused the registration or cannot express the binding
/// (Windows, #252). Rows and the Shortcuts page say which, so a binding
/// that behaves differently — the hook's does nothing while an elevated
/// application is in front, since Windows does not deliver those keys to
/// the hook of a normal process — is understood.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Route {
    /// The system's own registration: `RegisterHotKey` on Windows,
    /// Carbon's hot keys on macOS, an `XGrabKey` grab on X11.
    #[default]
    System,
    /// Pane's own `WH_KEYBOARD_LL` keyboard hook, which works while Pane
    /// runs and takes the shortcut from whoever had it, Windows included.
    Hook,
}

impl Route {
    /// What a hotkey's row appends for a binding dispatched this way on
    /// `platform`: nothing for the system's registration; for the hook,
    /// the route and the elevated-application limit, with what the system
    /// does with `shortcut` where it keeps it for itself, which Pane
    /// takes first while it runs. The platform is named so the wording is
    /// tested on every system.
    pub fn note_on(&self, shortcut: &Shortcut, platform: Option<Platform>) -> Option<String> {
        if *self == Route::System {
            return None;
        }
        let mut note = "through Pane's keyboard hook, which does nothing while an \
                        elevated application is in front"
            .to_string();
        if let Some(kept) = shortcut.kept_by(platform) {
            note.push_str("; ");
            note.push_str(kept);
            note.push_str(", which Pane takes first while it runs");
        }
        Some(note)
    }
}

/// The state of Pane's own keyboard hook, where this system's adapter
/// uses one (Windows, #252): what the Settings pages and Copy Diagnostics
/// say of it. `None` where no hook is in use.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HookHealth {
    /// How many times Windows removed the hook and Pane installed it
    /// again.
    pub reinstalls: u32,
    /// Whether the hook's code and data pages are pinned in memory, so a
    /// trimmed working set cannot fault the callback past Windows' hook
    /// timeout.
    pub pinned: bool,
    /// Why Pane gave up keeping the hook installed, if it did: its
    /// hook-based bindings do nothing until Pane starts again.
    pub given_up: Option<String>,
}

impl HookHealth {
    /// The note the Settings window's Keyboard page holds under the
    /// hook's name and Copy Diagnostics after "Keyboard hook: " (#259):
    /// that the hook is installed and its pages pinned in memory, how
    /// many times Windows removed it and Pane installed it again, and —
    /// in place of all of that — why Pane gave up keeping it installed,
    /// if it did.
    pub fn note(&self) -> String {
        if let Some(why) = &self.given_up {
            return why.clone();
        }
        let mut note = format!(
            "Installed, its pages {} pinned in memory",
            if self.pinned { "are" } else { "are not" }
        );
        match self.reinstalls {
            0 => {}
            1 => note.push_str("; Windows removed it once and Pane installed it again"),
            count => note.push_str(&format!(
                "; Windows removed it {count} times and Pane installed it again"
            )),
        }
        note
    }
}

/// Where an adapter reports presses of registered shortcuts.
#[derive(Clone)]
pub struct PressSender(tokio::sync::mpsc::UnboundedSender<Shortcut>);

impl PressSender {
    /// Reports a press of `shortcut`. Once the [`Presses`] are gone, it is
    /// dropped.
    pub fn send(&self, shortcut: Shortcut) {
        let _ = self.0.send(shortcut);
    }
}

/// The presses an adapter reports, in order.
pub struct Presses(tokio::sync::mpsc::UnboundedReceiver<Shortcut>);

impl Presses {
    /// The next press, waiting for it; `None` once no adapter can send any.
    pub async fn next(&mut self) -> Option<Shortcut> {
        self.0.recv().await
    }

    /// A press already reported, without waiting.
    pub fn try_next(&mut self) -> Option<Shortcut> {
        self.0.try_recv().ok()
    }
}

/// A channel from an adapter to the window.
pub fn channel() -> (PressSender, Presses) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (PressSender(sender), Presses(receiver))
}

/// This system's adapter, reporting presses to `presses`. On macOS it must
/// be made on the main thread, whose run loop receives the presses.
pub fn native(presses: PressSender) -> Arc<dyn Hotkeys> {
    #[cfg(target_os = "linux")]
    {
        let wayland = env_set("WAYLAND_DISPLAY");
        if wayland {
            return Arc::new(Unavailable(
                "Not available on Linux with Wayland: Wayland does not let an application see \
                 keys pressed in other applications, and Pane does not use the desktop's global \
                 shortcuts portal yet. Assign a shortcut in the desktop's keyboard settings \
                 instead, or run Pane on X11."
                    .into(),
            ));
        }
        match std::env::var("DISPLAY") {
            Ok(display) if !display.is_empty() => match X11Hotkeys::connect(&display, presses) {
                Ok(hotkeys) => Arc::new(hotkeys),
                Err(problem) => Arc::new(Unavailable(format!(
                    "Not available: Pane could not reach the X11 display {display}: {problem}"
                ))),
            },
            _ => Arc::new(Unavailable(
                "Not available: Pane is not running on an X11 or Wayland display".into(),
            )),
        }
    }
    #[cfg(target_os = "windows")]
    {
        match WindowsHotkeys::start(presses) {
            Ok(hotkeys) => Arc::new(hotkeys),
            Err(problem) => Arc::new(Unavailable(format!("Not available: {problem}"))),
        }
    }
    #[cfg(target_os = "macos")]
    {
        match MacHotkeys::start(presses) {
            Ok(hotkeys) => Arc::new(hotkeys),
            Err(problem) => Arc::new(Unavailable(format!("Not available: {problem}"))),
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = presses;
        Arc::new(Unavailable(format!(
            "Not available on this system ({}): Pane has global hotkeys only on Windows, macOS \
             and Linux",
            std::env::consts::OS
        )))
    }
}

#[cfg(target_os = "linux")]
fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// An adapter for where global hotkeys cannot be used, saying why.
pub struct Unavailable(pub String);

impl Hotkeys for Unavailable {
    fn unavailable(&self) -> Option<String> {
        Some(self.0.clone())
    }

    fn register(&self, _shortcut: &Shortcut) -> Result<(), HotkeyError> {
        Err(HotkeyError::Refused(self.0.clone()))
    }

    fn unregister(&self, _shortcut: &Shortcut) {}
}

/// The adapter of a launcher that was given none.
pub(crate) fn none() -> Arc<dyn Hotkeys> {
    Arc::new(Unavailable(
        "Not available: this Pane has no global hotkeys".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{HookHealth, INJECTED_TAG, Route, Shortcut};
    use crate::platform::Platform;

    fn shortcut(text: &str) -> Shortcut {
        Shortcut::parse(text).expect("a valid shortcut")
    }

    /// The Windows rules are checked on every system: `refusal_on` and
    /// `note_on` take the platform, so what Windows does is pinned without
    /// a Windows machine.
    const WINDOWS: Option<Platform> = Some(Platform::Windows);

    #[test]
    fn windows_reserved_shortcuts_are_warnings_rather_than_refusals() {
        // What Windows keeps for itself is no longer refused: the hook
        // takes the chord, and the row warns what Windows does with it.
        for (id, does) in [
            ("alt+f4", "Windows closes the active window with it"),
            ("super+d", "Windows shows the desktop with it"),
            ("super+e", "Windows opens File Explorer with it"),
            ("super+r", "Windows opens Run with it"),
            ("alt+space", "Windows opens the window menu with it"),
        ] {
            let shortcut = shortcut(id);
            assert_eq!(shortcut.refusal_on(WINDOWS), None, "{id} is refused");
            let note = Route::Hook.note_on(&shortcut, WINDOWS).unwrap();
            assert!(note.contains("through Pane's keyboard hook"), "{note}");
            assert!(note.contains("elevated application"), "{note}");
            assert!(note.contains(does), "{note}");
            assert!(note.contains("Pane takes first while it runs"), "{note}");
        }
        // A shortcut Windows does not keep warns of nothing but the route.
        let note = Route::Hook
            .note_on(&shortcut("ctrl+alt+g"), WINDOWS)
            .unwrap();
        assert_eq!(
            note,
            "through Pane's keyboard hook, which does nothing while an elevated application \
             is in front"
        );
        // The system's registration says nothing.
        assert_eq!(
            Route::System.note_on(&shortcut("ctrl+alt+g"), WINDOWS),
            None
        );
    }

    #[test]
    fn what_no_program_can_intercept_is_still_refused_with_the_reason() {
        // Win+L: Windows locks the computer with it, and the lock screen
        // is below every hook.
        assert_eq!(
            shortcut("super+l").refusal_on(WINDOWS),
            Some(
                "Win+L is reserved: Windows locks the computer with it, and no program can \
                 intercept it"
                    .into()
            )
        );
        // Ctrl+Alt+Delete: the secure screen owns it, on every system, so
        // it is refused before the key set's own wording.
        assert_eq!(
            Shortcut::parse("ctrl+alt+delete").unwrap_err(),
            "Ctrl+Alt+Delete is reserved: no program can intercept it"
        );
        // Other systems keep their own refusals: only Windows' list moved
        // to warnings.
        assert!(
            shortcut("alt+f4")
                .refusal_on(Some(Platform::Linux))
                .is_some()
        );
        assert!(
            shortcut("super+l")
                .refusal_on(Some(Platform::Linux))
                .is_some()
        );
        assert!(
            shortcut("super+space")
                .refusal_on(Some(Platform::Macos))
                .is_some()
        );
        assert_eq!(shortcut("alt+f4").refusal_on(WINDOWS), None);
    }

    #[test]
    fn the_injected_tag_spells_pane() {
        // "PANE" as four bytes: the marker every key Pane injects carries
        // in the event's extra information, spelled for a tool that reads
        // the markers other tools leave on their injections.
        let spelled: [u8; 4] = (INJECTED_TAG as u32).to_be_bytes();
        assert_eq!(spelled, *b"PANE");
        // The hook health an adapter answers for starts neutral.
        let health = HookHealth::default();
        assert_eq!(health.reinstalls, 0);
        assert!(!health.pinned);
        assert_eq!(health.given_up, None);
    }

    #[test]
    fn the_hook_s_state_is_said_plainly() {
        // What the Keyboard page holds under the hook's name and Copy
        // Diagnostics after "Keyboard hook: " (#259): the same words on
        // every system, since the hook is Windows' but the words are
        // Pane's.
        let quiet = HookHealth {
            reinstalls: 0,
            pinned: true,
            given_up: None,
        };
        assert_eq!(quiet.note(), "Installed, its pages are pinned in memory");
        let busier = HookHealth {
            reinstalls: 2,
            pinned: false,
            given_up: None,
        };
        assert_eq!(
            busier.note(),
            "Installed, its pages are not pinned in memory; Windows removed it 2 times and \
             Pane installed it again"
        );
        let once = HookHealth {
            reinstalls: 1,
            pinned: true,
            given_up: None,
        };
        assert_eq!(
            once.note(),
            "Installed, its pages are pinned in memory; Windows removed it once and Pane \
             installed it again"
        );
        // Giving up replaces the state: the reason, which says what
        // happened and what it leaves behind.
        let why = "Windows removed Pane's keyboard hook 6 times within 30 seconds; Pane gave \
                   up reinstalling it, and the hotkeys it dispatches do nothing until Pane \
                   starts again";
        let given_up = HookHealth {
            reinstalls: 6,
            pinned: true,
            given_up: Some(why.into()),
        };
        assert_eq!(given_up.note(), why);
    }
}
