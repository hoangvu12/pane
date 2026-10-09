//! The binding recognizer: the pure state machine that decides, from the
//! stream of key events, whether one of the chords Pane bound was
//! pressed.
//!
//! It is compiled on every system and knows nothing of Windows: the
//! Windows half of the hotkey adapter feeds it the key events its
//! `WH_KEYBOARD_LL` hook sees ([`KeyEvent`]) and carries out what it
//! decides ([`Decision`]). The tests drive it with synthetic sequences on
//! every system, so the chord logic is checked without a Windows machine.
//!
//! The rules match what `RegisterHotKey` does for the chords Windows
//! accepts, so a binding behaves the same whichever way it is dispatched:
//!
//! - a chord fires when its key is pressed while exactly its modifiers
//!   are held, whatever keys were pressed between the modifiers and it;
//! - the key press that fires a chord, and its release, are swallowed, so
//!   the system does not also give the chord its own meaning; modifier
//!   presses and releases always pass through;
//! - a key pressed while already down is its auto-repeat: it fires
//!   nothing and repeats nothing;
//! - a key Pane itself injected, tagged [`INJECTED_TAG`], is passed
//!   through untouched: Pane never reacts to its own input;
//! - a key another tool injected is recognized like the user's — a
//!   remapper sends the keys the user pressed, and the binding must work —
//!   but never swallowed: the key is the tool's, not Pane's to take. Such
//!   keys do not count as the user's for the liveness watchdog either
//!   (they are not the keyboard's; see [`Recognizer::physical_events`]);
//! - a modifier event that contradicts the state Pane holds of the
//!   keyboard asks for the modifiers to be re-read from the system
//!   ([`Decision::Resync`]), as a session unlock and a resume do, so a
//!   release Pane missed never leaves a modifier stuck down for a
//!   phantom chord or up for a missed one.
//!
//! One gap is deliberate, for the slice that binds the Windows key alone
//! (#260): the release of a Windows key whose chord Pane swallowed still
//! reaches Windows, which then opens the Start menu as it would for a lone
//! tap — the tagged neutral key that masks it comes with that slice.

use super::INJECTED_TAG;

/// One key event, as the recognizer sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key, as a Windows virtual-key code — the codes the hook
    /// reports (`a` is 0x41, the left Windows key 0x5B).
    pub key: u32,
    /// The key's scan code, kept for the layout-independent keys later
    /// binding kinds need; the recognizer matches virtual-key codes.
    pub scan: u32,
    /// Whether the key was pressed or released.
    pub pressed: bool,
    /// Whether a tool injected the event rather than the keyboard
    /// reporting it (`LLKHF_INJECTED` in the hook).
    pub injected: bool,
    /// The tag the injector left in the event's extra information: 0 for
    /// a key the keyboard reported, Pane's own [`INJECTED_TAG`] for the
    /// keys Pane injects.
    pub tag: usize,
    /// When the event happened, in milliseconds of the hook's clock, kept
    /// for the timed bindings later kinds add; a chord needs no timing.
    pub time: u64,
}

/// What the recognizer decided about a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The event completes the chord of the binding it names (the id it
    /// was added with): the press is reported and the event swallowed, so
    /// the system does not also give the chord its own meaning. The key's
    /// release is swallowed too.
    Fire(u32),
    /// The event completes the chord of the binding it names, and a tool
    /// injected it rather than the keyboard: the press is reported — the
    /// binding works — but the event passes through, the key being the
    /// tool's, not Pane's to take. Pane's own tagged events are not this:
    /// they are passed through untouched, before any of this (see
    /// [`KeyEvent::tag`]).
    Injected(u32),
    /// Swallow the event without reporting anything: the release or the
    /// auto-repeat of a key Pane swallowed the press of.
    Swallow,
    /// Let the event through to the system and the focused application.
    Pass,
    /// The event contradicts the state Pane holds of the keyboard — a
    /// modifier pressed while Pane holds it down, or released while Pane
    /// holds it up, which can only mean Pane missed events (the secure
    /// desktop of a lock or a UAC prompt had the keyboard). The caller
    /// re-reads the modifiers' real state from the system and hands it to
    /// [`Recognizer::resync`]. The event itself passes through.
    Resync,
}

/// The modifiers' positions in the mask [`Recognizer`] holds: control,
/// alt, shift, then the Windows key (Super on Linux, Command on macOS),
/// as [`super::Shortcut`] orders them.
const CONTROL: usize = 0;
const ALT: usize = 1;
const SHIFT: usize = 2;
const SUPER: usize = 3;

/// The modifier a virtual-key code is, as one of the mask's positions: a
/// left and right Ctrl are the same to a chord, and the generic code an
/// injected event can carry (`VK_CONTROL`) is that modifier too. `None`
/// for any other key.
fn modifier(key: u32) -> Option<usize> {
    match key {
        // VK_SHIFT, VK_LSHIFT, VK_RSHIFT.
        0x10 | 0xA0 | 0xA1 => Some(SHIFT),
        // VK_CONTROL, VK_LCONTROL, VK_RCONTROL.
        0x11 | 0xA2 | 0xA3 => Some(CONTROL),
        // VK_MENU, VK_LMENU, VK_RMENU.
        0x12 | 0xA4 | 0xA5 => Some(ALT),
        // VK_LWIN, VK_RWIN.
        0x5B | 0x5C => Some(SUPER),
        _ => None,
    }
}

/// One bound chord: the binding's id, the modifiers held, and the key.
struct Binding {
    id: u32,
    /// Whether control, alt, shift and the Windows key are held, in that
    /// order.
    mask: [bool; 4],
    key: u32,
}

/// The state machine: the chords Pane bound, the keyboard state it last
/// saw, and the key events it counted for the liveness watchdog.
#[derive(Default)]
pub struct Recognizer {
    /// The bound chords, in the order they were added. The hot path only
    /// reads this, and never allocates.
    bindings: Vec<Binding>,
    /// Whether Pane last saw control, alt, shift and the Windows key
    /// held.
    modifiers: [bool; 4],
    /// The non-modifier keys Pane last saw down, as a bitmap by
    /// virtual-key code, so a key pressed while already down is its
    /// auto-repeat.
    keys: [u64; 4],
    /// The key whose press fired a chord and whose release Pane still
    /// swallows. A key another tool injected never enters it: such keys
    /// are not swallowed.
    swallowed: Option<u32>,
    /// How many key events the keyboard reported, injected ones apart.
    physical: u64,
}

impl Recognizer {
    /// Binds the chord of `mask` (control, alt, shift, Windows key) with
    /// `key` as binding `id`, which [`Decision::Fire`] and
    /// [`Decision::Injected`] report when the chord is pressed.
    pub fn add(&mut self, id: u32, mask: [bool; 4], key: u32) {
        self.bindings.push(Binding { id, mask, key });
    }

    /// Unbinds the binding `id`.
    pub fn remove(&mut self, id: u32) {
        self.bindings.retain(|binding| binding.id != id);
    }

    /// Whether no chord is bound, so the hook that watches the keyboard
    /// for them can be removed.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// One key event, and what to do about it.
    pub fn step(&mut self, event: KeyEvent) -> Decision {
        if event.tag == INJECTED_TAG {
            // Pane's own injected key: passed through untouched.
            return Decision::Pass;
        }
        if !event.injected {
            self.physical += 1;
        }
        let Some(at) = modifier(event.key) else {
            return self.step_key(event);
        };
        if self.modifiers[at] == event.pressed {
            // A modifier pressed while Pane holds it down, or released
            // while it holds it up: events were missed while they were
            // not delivered to the hook, so the state is not trusted.
            return Decision::Resync;
        }
        self.modifiers[at] = event.pressed;
        Decision::Pass
    }

    /// A non-modifier key event.
    fn step_key(&mut self, event: KeyEvent) -> Decision {
        let Some(word) = self.keys.get_mut((event.key / 64) as usize) else {
            // A virtual-key code past the bitmap: nothing binds it.
            return Decision::Pass;
        };
        let bit = 1u64 << (event.key % 64);
        if event.pressed {
            if *word & bit != 0 {
                // The key is already down: its auto-repeat, which fires
                // nothing and repeats nothing Pane swallowed.
                return if self.swallowed == Some(event.key) {
                    Decision::Swallow
                } else {
                    Decision::Pass
                };
            }
            *word |= bit;
            let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.key == event.key && binding.mask == self.modifiers)
            else {
                return Decision::Pass;
            };
            if event.injected {
                return Decision::Injected(binding.id);
            }
            self.swallowed = Some(event.key);
            Decision::Fire(binding.id)
        } else {
            *word &= !bit;
            if self.swallowed == Some(event.key) {
                self.swallowed = None;
                return Decision::Swallow;
            }
            Decision::Pass
        }
    }

    /// Takes the modifiers' real state as the system reports it, dropping
    /// the key state Pane held: for a session unlock, a resume, or the
    /// re-read a contradictory event asked for ([`Decision::Resync`]), so
    /// a release Pane missed never leaves a modifier down (a phantom
    /// chord) or up (a missed chord).
    pub fn resync(&mut self, modifiers: [bool; 4]) {
        self.modifiers = modifiers;
        self.keys = [0; 4];
        self.swallowed = None;
    }

    /// How many key events the keyboard reported since the recognizer was
    /// made, injected ones apart: the evidence the liveness watchdog
    /// compares with the raw input the keyboard delivers, so keys other
    /// tools inject are not counted as the user's.
    pub fn physical_events(&self) -> u64 {
        self.physical
    }
}
